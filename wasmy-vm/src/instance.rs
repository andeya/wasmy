use core::ops::FnOnce;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::sync::RwLock;
use std::thread;

use lazy_static;
use wasmer::{Function, import_namespace, ImportObject, Memory, MemoryView, Module, Store, Type, WasmerEnv};
use wasmer_compiler_cranelift::Cranelift;
use wasmer_engine_universal::Universal;
use wasmer_wasi::WasiState;

use crate::handler::*;

lazy_static::lazy_static! {
    static ref INSTANCES: RwLock<HashMap<Key, Instance>> = RwLock::new(HashMap::<Key, Instance>::new());
}

pub(crate) fn load<F, R>(wasm_info: &WasmInfo, callback: F) -> Result<R>
    where F: FnOnce(&Instance) -> Result<R> + Copy
{
    let key = Key { wasm_path: wasm_info.wasm_path.clone(), thread_id: current_thread_id() };
    {
        if let Some(ins) = INSTANCES.read().unwrap().get(&key) {
            return callback(ins)
        }
    };
    Instance::new_unlock(wasm_info)?;
    #[cfg(debug_assertions)] println!("created instance, and getting it");
    callback(INSTANCES.read().unwrap().get(&key).unwrap())
}

#[derive(Debug, Clone)]
pub struct WasmInfo {
    pub wasm_path: String,
}

#[derive(Clone)]
pub(crate) struct Instance {
    instance: wasmer::Instance,
    function_map: HashMap<Method, String>,
    message_cache: RefCell<HashMap<i32, Vec<u8>>>,
    ctx_id_count: RefCell<i32>,
}

unsafe impl Send for Instance {}

unsafe impl Sync for Instance {}

#[derive(Debug, Hash, Eq, PartialEq, Clone, WasmerEnv)]
struct Key {
    wasm_path: String,
    thread_id: u64,
}


impl Instance {
    fn new_unlock(wasm_info: &WasmInfo) -> anyhow::Result<()> {
        let mut key = Key { wasm_path: wasm_info.wasm_path.clone(), thread_id: 0 };
        let ins = {
            let rlock = INSTANCES.read().unwrap();
            rlock.get(&key).map(|ins| ins.clone())
        };
        if let Some(ins) = ins {
            key.thread_id = current_thread_id();
            INSTANCES.write().unwrap().insert(key.clone(), ins);
            println!("[{}] clone instance: {}", key.thread_id, key.wasm_path);
            return Ok(())
        }

        // collect and register handlers once
        VmHandlerAPI::collect_and_register_once();

        let file_ref: &Path = wasm_info.wasm_path.as_ref();
        let canonical = file_ref.canonicalize()?;
        let wasm_bytes = std::fs::read(file_ref)?;
        let filename = canonical.as_path().to_str().unwrap();

        let store: Store = Store::new(&Universal::new(Cranelift::default()).engine());

        println!("compiling module {}...", filename);

        let mut module = Module::new(&store, wasm_bytes)?;
        module.set_name(filename);
        key.thread_id = current_thread_id();

        let mut function_map = HashMap::new();
        for function in module.exports().functions() {
            let name = function.name();
            WasmHandlerAPI::symbol_to_method(name).map_or_else(|| {
                #[cfg(debug_assertions)]
                println!("module exports function(invalid for vm): {:?}", function);
            }, |method| {
                let ty = function.ty();
                if ty.results().len() == 0 && ty.params().eq(&[Type::I32, Type::I32]) {
                    function_map.insert(method, name.to_string());
                    #[cfg(debug_assertions)]
                    println!("module exports function(valid for vm): {:?}", function);
                } else {
                    #[cfg(debug_assertions)]
                    println!("module exports function(invalid for vm): {:?}", function);
                }
            });
        }


        // First, we create the `WasiEnv` with the stdio pipes
        let mut wasi_env = WasiState::new(&wasm_info.wasm_path).finalize()?;

        // Then, we get the import object related to our WASI
        // and attach it to the Wasm instance.
        let mut import_object = wasi_env.import_object(&module)?;
        Self::register_import_object(&mut import_object, &store, key.clone());

        let instance = Instance {
            instance: wasmer::Instance::new(&module, &import_object)?,
            function_map,
            message_cache: RefCell::new(HashMap::with_capacity(1024)),
            ctx_id_count: RefCell::new(0),
        };
        println!("[{}] created instance: {}", key.thread_id, key.wasm_path);

        INSTANCES.write().unwrap().insert(key, instance);
        Ok(())
    }

    fn register_import_object(import_object: &mut ImportObject, store: &Store, key: Key) {
        import_object.register("env", import_namespace!({
            "_wasm_host_recall" => Function::new_native_with_env(store, key.clone(), |key: &Key, ctx_id: i32, offset: i32| {
                #[cfg(debug_assertions)]
                println!("_wasm_host_recall: key={:?}, ctx_id={}, offset={}", key, ctx_id, offset);
                let rlock = INSTANCES.read().unwrap();
                let ins = rlock.get(key).unwrap();
                let _ = ins.use_mut_buffer(ctx_id, 0, |data| {
                    ins.set_view_bytes(offset as usize, data.iter());
                    let len = data.len();
                    unsafe { data.set_len(0) };
                    len
                });
            }),
            "_wasm_host_restore" => Function::new_native_with_env(store, key.clone(), |key: &Key, ctx_id: i32, offset: i32, size: i32| {
                #[cfg(debug_assertions)]
                println!("_wasm_host_restore: key={:?}, ctx_id={}, offset={}, size={}", key, ctx_id, offset, size);
                let rlock = INSTANCES.read().unwrap();
                let ins = rlock.get(key).unwrap();
                let _ = ins.use_mut_buffer(ctx_id, size as usize, |buffer|{
                    ins.read_view_bytes(offset as usize, size as usize, buffer);
                    buffer.len()
                });
            }),
            "_wasm_host_call" => Function::new_native_with_env(store, key.clone(), |key: &Key, ctx_id: i32, offset: i32, size: i32|-> i32 {
                #[cfg(debug_assertions)]
                println!("_wasm_host_call: key={:?}, ctx_id={}, offset={}, size={}", key, ctx_id, offset, size);
                let rlock = INSTANCES.read().unwrap();
                let ins = rlock.get(key).unwrap();
                ins.use_mut_buffer(ctx_id, size as usize, |buffer| {
                    ins.read_view_bytes(offset as usize, size as usize, buffer);
                    write_to_vec(&host_call(buffer), buffer)
                }) as i32
            }),
        }));
    }
    pub(crate) fn use_mut_buffer<F: FnOnce(&mut Vec<u8>) -> usize>(&self, ctx_id: i32, size: usize, call: F) -> usize {
        let mut cache = self.message_cache.borrow_mut();
        if let Some(buffer) = cache.get_mut(&ctx_id) {
            if size > 0 {
                resize_with_capacity(buffer, size);
            }
            return call(buffer);
        }
        cache.insert(ctx_id, vec![0; size]);
        call(cache.get_mut(&ctx_id).unwrap())
    }
    pub(crate) fn take_buffer(&self, ctx_id: i32) -> Option<Vec<u8>> {
        self.message_cache.borrow_mut().remove(&ctx_id)
    }
    pub(crate) fn call_wasm_handler(&self, method: Method, ctx_id: i32, size: i32) -> bool {
        let hdl = self.function_map.get(&method);
        if hdl.is_none() {
            return false
        }
        let hdl = hdl.unwrap();
        loop {
            if let Err(e) = self
                .instance
                .exports
                .get_native_function::<(i32, i32), ()>(hdl)
                .unwrap()
                .call(ctx_id, size)
            {
                let estr = format!("{:?}", e);
                eprintln!("call {} error: {}", hdl, estr);
                if estr.contains("OOM") {
                    match self.get_memory().grow(1) {
                        Ok(p) => {
                            println!("memory grow, previous memory size: {:?}", p);
                        }
                        Err(e) => {
                            eprintln!("failed to memory grow: {:?}", e);
                        }
                    }
                }
            } else {
                return true;
            }
        }
    }
    fn get_memory(&self) -> &Memory {
        self.instance.exports.get_memory("memory").unwrap()
    }
    fn get_view(&self) -> MemoryView<u8> {
        self.get_memory().view::<u8>()
    }
    fn set_view_bytes<'a>(&self, offset: usize, data: impl IntoIterator<Item=&'a u8> + ExactSizeIterator) {
        let view = self.get_view();
        for (cell, b) in view[offset..offset + data.len()].iter().zip(data) {
            cell.set(*b);
        }
    }
    fn read_view_bytes(&self, offset: usize, size: usize, buffer: &mut Vec<u8>) {
        // println!("read_view_bytes: offset:{}, size:{}", offset, size);
        if size == 0 {
            resize_with_capacity(buffer, size);
            return;
        }
        let view = self.get_view();
        for x in view[offset..(offset + size)]
            .iter()
            .map(|c| c.get()).enumerate() {
            buffer[x.0] = x.1;
        }
    }
    pub(crate) fn gen_ctx_id(&self) -> i32 {
        self.ctx_id_count.replace_with(|v| *v + 1)
    }
    fn next_ctx_id(&self) -> i32 {
        self.ctx_id_count.borrow_mut().clone() + 1
    }
    pub(crate) fn try_reuse_buffer(&self, buffer: Vec<u8>) {
        let next_id = self.next_ctx_id();
        let mut cache = self.message_cache.borrow_mut();
        if !cache.contains_key(&next_id) {
            cache.insert(next_id, buffer);
        }
    }
}


fn current_thread_id() -> u64 {
    thread::current().id().as_u64().get()
}

fn write_to_vec(msg: &dyn Message, buffer: &mut Vec<u8>) -> usize {
    let size = msg.compute_size() as usize;
    resize_with_capacity(buffer, size);
    write_to_with_cached_sizes(msg, buffer)
}

pub(crate) fn write_to_with_cached_sizes(msg: &dyn Message, buffer: &mut Vec<u8>) -> usize {
    let mut os = CodedOutputStream::bytes(buffer);
    msg.write_to_with_cached_sizes(&mut os)
       .or_else(|e| Err(format!("{}", e))).unwrap();
    // os.flush().unwrap();
    buffer.len()
}

fn resize_with_capacity(buffer: &mut Vec<u8>, new_size: usize) {
    if new_size > buffer.capacity() {
        buffer.resize(new_size, 0);
    } else {
        unsafe { buffer.set_len(new_size) };
    }
}
