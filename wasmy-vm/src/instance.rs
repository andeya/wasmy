use core::ops::FnOnce;
use std::alloc::{alloc, Layout};
use std::cell::{Cell, RefCell, RefMut};
use std::collections::HashMap;
use std::sync::RwLock;
use std::thread;
use std::thread::ThreadId;

use anyhow::anyhow;
use lazy_static;
use wasmer::{Function, import_namespace, ImportObject, Memory, MemoryView, Module, WasmerEnv};
use wasmer_wasi::WasiState;

use crate::{modules, WasmUri};
use crate::handler::*;
use crate::wasm_info::WasmInfo;

lazy_static::lazy_static! {
    static ref INSTANCES: RwLock<HashMap<LocalInstanceKey, Box<Instance>>> = RwLock::new(HashMap::new());
}

#[derive(Hash, Eq, PartialEq, Clone, Debug)]
struct LocalInstanceKey {
    wasm_uri: WasmUri,
    thread_id: ThreadId,
}

impl LocalInstanceKey {
    fn from(wasm_uri: WasmUri) -> LocalInstanceKey {
        LocalInstanceKey { wasm_uri, thread_id: thread::current().id() }
    }
}

#[derive(Clone, WasmerEnv, Debug)]
struct InstanceEnv {
    key: LocalInstanceKey,
    ptr: *mut Instance,
}

unsafe impl Sync for InstanceEnv {}

unsafe impl Send for InstanceEnv {}

impl InstanceEnv {
    fn from(key: LocalInstanceKey) -> InstanceEnv {
        unsafe {
            let ptr = alloc(Layout::new::<Instance>()) as *mut Instance;
            InstanceEnv {
                ptr,
                key,
            }
        }
    }
    fn init_instance(&self, instance: Instance) {
        unsafe {
            self.ptr.write(instance);
            INSTANCES.write().unwrap().insert(self.key.clone(), Box::from_raw(self.ptr));
        }
    }
    fn as_instance(&self) -> &Instance {
        unsafe { &*self.ptr }
    }
}

pub(crate) fn load<B, W>(wasm_info: W) -> Result<WasmUri>
    where B: AsRef<[u8]>,
          W: WasmInfo<B>,
{
    let ins = Instance::load_and_new_local(wasm_info)?;
    Ok(ins.key.wasm_uri.clone())
}


pub(crate) fn with<F, R>(wasm_uri: WasmUri, callback: F) -> Result<R>
    where F: FnOnce(&Instance) -> Result<R>
{
    let key = LocalInstanceKey::from(wasm_uri);
    {
        if let Some(ins) = INSTANCES.read().unwrap().get(&key) {
            return callback(ins);
        }
    }
    return callback(Instance::new_local(key)?.as_instance());
    // return ERR_CODE_NONE.to_result("not found vm instance by wasm_uri")
}

#[derive(Clone, Debug)]
pub(crate) struct Instance {
    key: LocalInstanceKey,
    instance: wasmer::Instance,
    next_ctx_id: Cell<i32>,
    ctx_memory: RefCell<HashMap<i32, Vec<u8>>>,
}

unsafe impl Sync for Instance {}

unsafe impl Send for Instance {}

impl Instance {
    fn new_local(key: LocalInstanceKey) -> anyhow::Result<InstanceEnv> {
        if let Some(module) = modules::MODULES.read().unwrap().get(&key.wasm_uri) {
            Self::from_module(module, key)
        } else {
            Err(anyhow!("not found module"))
        }
    }

    fn load_and_new_local<B, W>(wasm_info: W) -> anyhow::Result<InstanceEnv>
        where B: AsRef<[u8]>,
              W: WasmInfo<B>,
    {
        let wasm_uri = modules::load(wasm_info)?;
        Self::from_module(
            modules::MODULES.read().unwrap().get(&wasm_uri).as_ref().unwrap(),
            LocalInstanceKey::from(wasm_uri),
        )
    }

    fn from_module(module: &Module, key: LocalInstanceKey) -> anyhow::Result<InstanceEnv> {
        let ins_env = InstanceEnv::from(key);

        let import_object = Self::new_import_object(&module, &ins_env)?;

        let instance = Instance {
            key: ins_env.key.clone(),
            instance: wasmer::Instance::new(&module, &import_object)?,
            ctx_memory: RefCell::new(HashMap::with_capacity(1024)),
            next_ctx_id: Cell::new(0),
        };
        #[cfg(debug_assertions)]println!("[{:?}] created instance: wasm_uri={}", ins_env.key.thread_id, ins_env.key.wasm_uri);

        ins_env.init_instance(instance);

        // only call once
        if let Err(e) = ins_env.as_instance().init() {
            INSTANCES.write().unwrap().remove(&ins_env.key);
            return Err(e);
        }
        return Ok(ins_env)
    }

    fn new_import_object(module: &Module, ins_env: &InstanceEnv) -> anyhow::Result<ImportObject> {
        let mut import_object = WasiState::new(&ins_env.key.wasm_uri)
            // First, we create the `WasiEnv` with the stdio pipes
            .finalize()?
            // Then, we get the import object related to our WASI
            // and attach it to the Wasm instance.
            .import_object(&module)?;

        import_object.register("env", import_namespace!({
            "_wasm_host_recall" => Function::new_native_with_env(module.store(), ins_env.clone(), |ins_env: &InstanceEnv, ctx_id: i32, offset: i32| {
                let key = &ins_env.key;
                #[cfg(debug_assertions)] println!("[VM:{:?}]_wasm_host_recall: wasm_uri={}, ctx_id={}, offset={}", key.thread_id, key.wasm_uri, ctx_id, offset);
                let ins = ins_env.as_instance();
                let _ = ins.use_mut_buffer(ctx_id, 0, |data| {
                    ins.set_view_bytes(offset as usize, data.iter());
                    let len = data.len();
                    unsafe { data.set_len(0) };
                    len
                });
            }),
            "_wasm_host_restore" => Function::new_native_with_env(module.store(), ins_env.clone(), |ins_env: &InstanceEnv, ctx_id: i32, offset: i32, size: i32| {
                let key = &ins_env.key;
                #[cfg(debug_assertions)] println!("[VM:{:?}]_wasm_host_restore: wasm_uri={}, ctx_id={}, offset={}, size={}", key.thread_id, key.wasm_uri, ctx_id, offset, size);
                let ins = ins_env.as_instance();
                let _ = ins.use_mut_buffer(ctx_id, size as usize, |buffer|{
                    ins.read_view_bytes(offset as usize, size as usize, buffer);
                    buffer.len()
                });
            }),
            "_wasm_host_call" => Function::new_native_with_env(module.store(), ins_env.clone(), |ins_env: &InstanceEnv, ctx_id: i32, offset: i32, size: i32|-> i32 {
                let key = &ins_env.key;
                #[cfg(debug_assertions)] println!("[VM:{:?}]_wasm_host_call: wasm_uri={}, ctx_id={}, offset={}, size={}", key.thread_id, key.wasm_uri, ctx_id, offset, size);
                let ins = ins_env.as_instance();
                ins.use_mut_buffer(ctx_id, size as usize, |buffer| {
                    ins.read_view_bytes(offset as usize, size as usize, buffer);
                    write_to_vec(&host_call(buffer), buffer)
                }) as i32
            }),
        }));

        Ok(import_object)
    }

    fn init(&self) -> anyhow::Result<()> {
        let ret = self.invoke_instance(WasmHandlerAPI::onload_symbol(), None).map_or_else(|e| {
            if e.code == ERR_CODE_NONE.value() {
                #[cfg(debug_assertions)]println!("[{:?}]no need initialize instance: wasm_uri={}", self.key.thread_id, self.key.wasm_uri);
                Ok(())
            } else {
                Err(anyhow!("{}", e))
            }
        }, |_| {
            #[cfg(debug_assertions)]println!("[{:?}]initialized instance: wasm_uri={}", self.key.thread_id, self.key.wasm_uri);
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
