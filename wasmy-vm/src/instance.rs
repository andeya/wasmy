use core::ops::FnOnce;
use std::cell::{Cell, RefCell, RefMut};
use std::collections::HashMap;
use std::sync::RwLock;
use std::thread;
use std::thread::ThreadId;

use anyhow::anyhow;
use lazy_static;
use wasmer::{Function, import_namespace, ImportObject, Memory, MemoryView, Module, Store, Type};
use wasmer_compiler_cranelift::Cranelift;
use wasmer_engine_universal::Universal;
use wasmer_wasi::WasiState;

use crate::handler::*;
use crate::wasm_info::WasmInfo;
use crate::WasmUri;

lazy_static::lazy_static! {
    static ref INSTANCES: RwLock<HashMap<LocalInstanceKey, Instance>> = RwLock::new(HashMap::new());
}

#[derive(Hash, Eq, PartialEq, Clone)]
struct LocalInstanceKey {
    wasm_uri: WasmUri,
    thread_id: ThreadId,
}

impl LocalInstanceKey {
    fn from(wasm_uri: WasmUri, zero_thread: bool) -> LocalInstanceKey {
        if zero_thread {
            let zero: u64 = 0;
            let tid = unsafe { &*(&zero as *const u64 as *const ThreadId) };
            LocalInstanceKey { wasm_uri, thread_id: tid.clone() }
        } else {
            LocalInstanceKey { wasm_uri, thread_id: thread::current().id() }
        }
    }
}

pub(crate) fn load<B, W>(wasm_info: W) -> Result<WasmUri>
    where B: AsRef<[u8]>,
          W: WasmInfo<B>,
{
    let key = Instance::new_unlock(wasm_info)?;
    Ok(key.wasm_uri)
}


pub(crate) fn with<F, R>(wasm_uri: WasmUri, callback: F) -> Result<R>
    where F: FnOnce(&Instance) -> Result<R>
{
    let key = LocalInstanceKey::from(wasm_uri, false);
    if let Some(ins) = INSTANCES.read().unwrap().get(&key) {
        return callback(ins)
    }
    let key = Instance::new_local_unlock(key.wasm_uri)?;
    if let Some(ins) = INSTANCES.read().unwrap().get(&key) {
        return callback(ins)
    }
    return ERR_CODE_NONE.to_result("not found vm instance by wasm_uri")
}

#[derive(Clone)]
pub(crate) struct Instance {
    key: LocalInstanceKey,
    instance: wasmer::Instance,
    next_ctx_id: Cell<i32>,
    ctx_memory: RefCell<HashMap<i32, Vec<u8>>>,
}

unsafe impl Sync for Instance {}

unsafe impl Send for Instance {}

impl Instance {
    fn new_unlock<B, W>(wasm_info: W) -> anyhow::Result<LocalInstanceKey>
        where B: AsRef<[u8]>,
              W: WasmInfo<B>,
    {
        let wasm_uri = wasm_info.wasm_uri();
        Self::new0_unlock(wasm_info)?;
        Self::new_local_unlock(wasm_uri)
    }
    fn new_local_unlock(wasm_uri: WasmUri) -> anyhow::Result<LocalInstanceKey> {
        let inner = INSTANCES.read()
                             .unwrap()
                             .get(&LocalInstanceKey::from(wasm_uri.clone(), true))
                             .unwrap()
                             .instance
                             .clone();
        let key = LocalInstanceKey::from(wasm_uri, false);
        let ins = Instance {
            key: key.clone(),
            instance: inner,
            ctx_memory: RefCell::new(HashMap::with_capacity(1024)),
            next_ctx_id: Cell::new(1),
        };
        INSTANCES.write().unwrap().insert(key.clone(), ins);
        Ok(key)
    }
    fn new0_unlock<B, W>(wasm_info: W) -> anyhow::Result<()>
        where B: AsRef<[u8]>,
              W: WasmInfo<B>,
    {
        // collect and register handlers once
        VmHandlerAPI::collect_and_register_once();
        let wasm_uri = wasm_info.wasm_uri();

        #[cfg(debug_assertions)] println!("compiling module, wasm_uri={}...", wasm_uri);
        let store: Store = Store::new(&Universal::new(Cranelift::default()).engine());
        let mut module = Module::new(&store, wasm_info.into_wasm_bytes()?)?;
        module.set_name(wasm_uri.as_str());
        #[cfg(debug_assertions)] println!("compiled module, wasm_uri={}...", wasm_uri);

        for function in module.exports().functions() {
            let name = function.name();
            if name == WasmHandlerAPI::onload_symbol() {
                let ty = function.ty();
                if ty.params().len() > 0 || ty.results().len() > 0 {
                    return Err(anyhow::Error::msg(format!("Incompatible Export Type: fn {}(){{}}", WasmHandlerAPI::onload_symbol())))
                }
                continue
            }
            WasmHandlerAPI::symbol_to_method(name).map_or_else(|| {
                #[cfg(debug_assertions)]println!("module exports function(invalid for vm): {:?}", function);
            }, |_method| {
                let ty = function.ty();
                if ty.results().len() == 0 && ty.params().eq(&[Type::I32, Type::I32]) {
                    #[cfg(debug_assertions)]println!("module exports function(valid for vm): {:?}", function);
                } else {
                    #[cfg(debug_assertions)]println!("module exports function(invalid for vm): {:?}", function);
                }
            });
        }

        let mut import_object = WasiState::new(&wasm_uri)
            // First, we create the `WasiEnv` with the stdio pipes
            .finalize()?
            // Then, we get the import object related to our WASI
            // and attach it to the Wasm instance.
            .import_object(&module)?;


        Self::register_import_object(&mut import_object, module.store(), &wasm_uri);

        let key = LocalInstanceKey::from(wasm_uri, true);
        let instance = Instance {
            key: key.clone(),
            instance: wasmer::Instance::new(&module, &import_object)?,
            ctx_memory: RefCell::new(HashMap::with_capacity(1024)),
            next_ctx_id: Cell::new(0),
        };
        #[cfg(debug_assertions)]println!("created instance: wasm_uri={}", key.wasm_uri);

        INSTANCES.write().unwrap().insert(key.clone(), instance);
        // only call once
        if let Err(e) = INSTANCES.read().unwrap().get(&key).unwrap().init() {
            INSTANCES.write().unwrap().remove(&key);
            return Err(e)
        }

        Ok(())
    }

    fn init(&self) -> anyhow::Result<()> {
        let ret = self.invoke_instance(WasmHandlerAPI::onload_symbol(), None).map_or_else(|e| {
            if e.code == ERR_CODE_NONE.value() {
                #[cfg(debug_assertions)]println!("no need initialize instance: wasm_uri={}", self.key.wasm_uri);
                Ok(())
            } else {
                Err(anyhow!("{}", e))
            }
        }, |_| {
            #[cfg(debug_assertions)]println!("initialized instance: wasm_uri={}", self.key.wasm_uri);
            Ok(())
        });
        self.next_ctx_id.set(1);
        ret
    }
    #[inline]
    pub(crate) fn call_wasm_handler(&self, method: Method, in_args: InArgs) -> Result<OutRets> {
        let ctx_id = self.gen_ctx_id()?;
        #[cfg(debug_assertions)] println!("ctx_id={}, method={}, data={:?}", ctx_id, in_args.get_method(), in_args.get_data());
        let buffer_len = self.use_mut_buffer(ctx_id, in_args.compute_size() as usize, |buffer| {
            write_to_with_cached_sizes(&in_args, buffer)
        });
        let sign_name = WasmHandlerAPI::method_to_symbol(method);
        self.invoke_instance(&sign_name, Some((ctx_id, buffer_len as i32)))?;
        let buffer = self.take_buffer(ctx_id).unwrap_or(vec![]);
        let res = if buffer.len() > 0 {
            OutRets::parse_from_bytes(buffer.as_slice()).unwrap()
        } else {
            OutRets::new()
        };
        self.try_reuse_buffer(buffer);
        Ok(res)
    }
    pub(crate) fn invoke_instance(&self, sign_name: &str, args: Option<(i32, i32)>) -> Result<()> {
        // return ERR_CODE_UNKNOWN.to_result("not found handler method in wasm");
        let exports = &self.instance.exports;
        loop {
            let ret = if let Some((ctx_id, size)) = args.clone() {
                exports
                    .get_native_function::<(i32, i32), ()>(sign_name)
                    .map_err(|e| ERR_CODE_NONE.to_code_msg(e))?
                    .call(ctx_id, size)
            } else {
                exports
                    .get_native_function::<(), ()>(sign_name)
                    .map_err(|e| ERR_CODE_NONE.to_code_msg(e))?
                    .call()
            };
            if let Err(e) = ret {
                let estr = format!("{:?}", e);
                eprintln!("call {} error: {}", sign_name, estr);
                if estr.contains("OOM") {
                    match self.get_memory().grow(1) {
                        Ok(p) => {
                            println!("memory grow, previous memory size: {:?}", p);
                        }
                        Err(e) => {
                            return ERR_CODE_MEM.to_result(format!("failed to memory grow: {:?}", e))
                        }
                    }
                }
            } else {
                return Ok(());
            }
        }
    }
    fn register_import_object(import_object: &mut ImportObject, store: &Store, wasm_uri: &WasmUri) {
        import_object.register("env", import_namespace!({
            "_wasm_host_recall" => Function::new_native_with_env(store, wasm_uri.clone(), |wasm_uri: &WasmUri, ctx_id: i32, offset: i32| {
                let key = LocalInstanceKey::from(wasm_uri.clone(), ctx_id<=0);
                #[cfg(debug_assertions)] println!("[VM:{:?}]_wasm_host_recall: wasm_uri={}, ctx_id={}, offset={}", key.thread_id, wasm_uri, ctx_id, offset);
                let rlock = INSTANCES.read().unwrap();
                let ins = rlock.get(&key).unwrap();
                let _ = ins.use_mut_buffer(ctx_id, 0, |data| {
                    ins.set_view_bytes(offset as usize, data.iter());
                    let len = data.len();
                    unsafe { data.set_len(0) };
                    len
                });
            }),
            "_wasm_host_restore" => Function::new_native_with_env(store, wasm_uri.clone(), |wasm_uri: &WasmUri, ctx_id: i32, offset: i32, size: i32| {
                let key = LocalInstanceKey::from(wasm_uri.clone(), ctx_id<=0);
                #[cfg(debug_assertions)] println!("[VM:{:?}]_wasm_host_restore: wasm_uri={}, ctx_id={}, offset={}, size={}", key.thread_id, wasm_uri, ctx_id, offset, size);
                let rlock = INSTANCES.read().unwrap();
                let ins = rlock.get(&key).unwrap();
                let _ = ins.use_mut_buffer(ctx_id, size as usize, |buffer|{
                    ins.read_view_bytes(offset as usize, size as usize, buffer);
                    buffer.len()
                });
            }),
            "_wasm_host_call" => Function::new_native_with_env(store, wasm_uri.clone(), |wasm_uri: &WasmUri, ctx_id: i32, offset: i32, size: i32|-> i32 {
                let key = LocalInstanceKey::from(wasm_uri.clone(), ctx_id<=0);
                #[cfg(debug_assertions)] println!("[VM:{:?}]_wasm_host_call: wasm_uri={}, ctx_id={}, offset={}, size={}", key.thread_id, wasm_uri, ctx_id, offset, size);
                let rlock = INSTANCES.read().unwrap();
                let ins = rlock.get(&key).unwrap();
                ins.use_mut_buffer(ctx_id, size as usize, |buffer| {
                    ins.read_view_bytes(offset as usize, size as usize, buffer);
                    write_to_vec(&host_call(buffer), buffer)
                }) as i32
            }),
        }));
    }
    fn borrow_mut_ctx_memory(&self) -> RefMut<HashMap<i32, Vec<u8>>> {
        self.ctx_memory.borrow_mut()
    }
    pub(crate) fn use_mut_buffer<F: FnOnce(&mut Vec<u8>) -> usize>(&self, ctx_id: i32, size: usize, call: F) -> usize {
        let mut cache = self.borrow_mut_ctx_memory();
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
        self.borrow_mut_ctx_memory().remove(&ctx_id)
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
    pub(crate) fn gen_ctx_id(&self) -> Result<i32> {
        let ctx_id = self.next_ctx_id.get();
        if ctx_id <= 0 {
            return ERR_CODE_NONE.to_result("has not completed initialization")
        }
        self.next_ctx_id.set(ctx_id + 1);
        Ok(ctx_id)
    }
    fn next_ctx_id(&self) -> i32 {
        self.next_ctx_id.get()
    }
    pub(crate) fn try_reuse_buffer(&self, buffer: Vec<u8>) {
        let next_id = self.next_ctx_id();
        let mut cache = self.borrow_mut_ctx_memory();
        if !cache.contains_key(&next_id) {
            cache.insert(next_id, buffer);
        }
    }
}


fn write_to_vec(msg: &dyn Message, buffer: &mut Vec<u8>) -> usize {
    let size = msg.compute_size() as usize;
    resize_with_capacity(buffer, size);
    write_to_with_cached_sizes(msg, buffer)
}

fn write_to_with_cached_sizes(msg: &dyn Message, buffer: &mut Vec<u8>) -> usize {
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
