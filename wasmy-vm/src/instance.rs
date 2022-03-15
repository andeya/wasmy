use core::ops::FnOnce;
use std::{
    alloc::{alloc, Layout},
    cell::{Cell, RefCell, RefMut},
    collections::HashMap,
    sync::{Mutex, RwLock},
    thread,
    thread::ThreadId,
};

use lazy_static;
use wasmer::{Exports, Function, ImportObject, Memory, MemoryView, Val, WasmerEnv};

use crate::{
    handler::*,
    modules,
    modules::{FnBuildImportObject, FnCheckModule, Module},
    wasm_file::WasmFile,
    WasmUri,
};

lazy_static::lazy_static! {
    static ref INSTANCES: RwLock<HashMap<LocalInstanceKey, Mutex<Box<Instance>>>> = RwLock::new(HashMap::new());
}

#[derive(Hash, Eq, PartialEq, Clone, Debug, WasmerEnv)]
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
pub struct InstanceEnv {
    key: LocalInstanceKey,
    ptr: *mut Instance,
}

unsafe impl Sync for InstanceEnv {}

unsafe impl Send for InstanceEnv {}

impl InstanceEnv {
    fn from(key: LocalInstanceKey) -> Self {
        unsafe {
            let ptr = alloc(Layout::new::<Instance>()) as *mut Instance;
            InstanceEnv { ptr, key }
        }
    }
    fn init_instance(&self, instance: Instance) {
        unsafe {
            self.ptr.write(instance);
            INSTANCES
                .write()
                .unwrap()
                .insert(self.key.clone(), Mutex::new(Box::from_raw(self.ptr)));
        }
    }
    pub fn wasm_uri(&self) -> &WasmUri {
        &self.key.wasm_uri
    }
    pub fn thread_id(&self) -> ThreadId {
        self.key.thread_id
    }
    pub fn as_instance(&self) -> &Instance {
        unsafe { &*self.ptr }
    }
}

pub(crate) fn load<B, W>(
    wasm_file: W,
    check_module: Option<FnCheckModule>,
    build_import_object: Option<FnBuildImportObject>,
) -> Result<WasmUri>
where
    B: AsRef<[u8]>,
    W: WasmFile<B>,
{
    let ins = Instance::load_and_new_local(wasm_file, check_module, build_import_object)?;
    Ok(ins.key.wasm_uri.clone())
}

pub(crate) fn with<F, R>(wasm_uri: WasmUri, callback: F) -> Result<R>
where
    F: FnOnce(&Instance) -> Result<R>,
{
    let key = LocalInstanceKey::from(wasm_uri);
    {
        if let Some(ins) = INSTANCES.read().unwrap().get(&key) {
            return callback(&ins.lock().unwrap());
        }
    }
    return callback(Instance::new_local(key)?.as_instance());
}

#[derive(Clone, Debug)]
pub struct Instance {
    key: LocalInstanceKey,
    instance: wasmer::Instance,
    loaded: Cell<bool>,
    context: RefCell<Context>,
}

#[derive(Clone, Debug)]
pub struct Context {
    value_ptr: usize,
    pub value_bytes: Vec<u8>,
    pub swap_memory: Vec<u8>,
}

impl Context {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            value_ptr: 0,
            value_bytes: Vec::with_capacity(capacity),
            swap_memory: Vec::with_capacity(capacity),
        }
    }

    pub fn set_value_ptr<T>(&mut self, ptr: &T) {
        self.value_ptr = ptr as *const T as usize;
    }

    pub unsafe fn value_ptr<T>(&self) -> Option<&T> {
        let ptr = self.value_ptr as *const T;
        if ptr.is_null() { None } else { Some(&*ptr) }
    }

    fn set_args<C: Message>(&mut self, ctx_value: Option<&C>, in_args: InArgs) -> (usize, usize) {
        let args_size = write_to_vec(&in_args, &mut self.swap_memory);
        if args_size == 0 {
            unsafe { self.swap_memory.set_len(0) }
        }
        let ctx_size = if let Some(val) = ctx_value {
            self.value_ptr = val as *const C as usize;
            write_to_vec(val, &mut self.value_bytes)
        } else {
            unsafe { self.value_bytes.set_len(0) };
            0
        };
        (ctx_size, args_size)
    }

    fn out_rets(&mut self) -> OutRets {
        let res = if self.swap_memory.len() > 0 {
            OutRets::parse_from_bytes(self.swap_memory.as_slice()).unwrap()
        } else {
            OutRets::new()
        };
        self.reverted();
        res
    }

    pub(crate) fn reverted(&mut self) {
        unsafe {
            self.value_ptr = 0;
            self.value_bytes.set_len(0);
            self.swap_memory.set_len(0);
        };
    }
}

unsafe impl Sync for Instance {}

unsafe impl Send for Instance {}

impl Instance {
    fn new_local(key: LocalInstanceKey) -> Result<InstanceEnv> {
        if let Some(module) = modules::MODULES.read().unwrap().get(&key.wasm_uri) {
            Self::from_module(module, key, false)
        } else {
            CodeMsg::result(CODE_NONE, "not found module")
        }
    }

    fn load_and_new_local<B, W>(
        wasm_file: W,
        check_module: Option<FnCheckModule>,
        build_import_object: Option<FnBuildImportObject>,
    ) -> Result<InstanceEnv>
    where
        B: AsRef<[u8]>,
        W: WasmFile<B>,
    {
        let wasm_uri = modules::load(wasm_file, check_module, build_import_object)?;
        Self::from_module(
            modules::MODULES.read().unwrap().get(&wasm_uri).as_ref().unwrap(),
            LocalInstanceKey::from(wasm_uri),
            true,
        )
    }

    fn from_module(module: &Module, key: LocalInstanceKey, first: bool) -> Result<InstanceEnv> {
        let ins_env = InstanceEnv::from(key);

        let import_object = Self::build_import_object(&module, &ins_env)?;
        if first {
            for (namespace, name, r#extern) in import_object.externs_vec() {
                #[cfg(debug_assertions)]
                println!(
                    "import: namespace={namespace}, name={name}, extern_type={:?}",
                    r#extern.ty()
                );
            }
        }
        let instance = Instance {
            key: ins_env.key.clone(),
            instance: wasmer::Instance::new(&module.module, &import_object)?,
            loaded: Cell::new(false),
            context: RefCell::new(Context::with_capacity(1024)),
        };
        #[cfg(debug_assertions)]
        println!(
            "[{:?}] created instance: wasm_uri={}",
            ins_env.key.thread_id, ins_env.key.wasm_uri
        );

        ins_env.init_instance(instance);

        // only call once
        if let Err(e) = ins_env.as_instance().init() {
            INSTANCES.write().unwrap().remove(&ins_env.key);
            return Err(e);
        }
        return Ok(ins_env);
    }

    fn build_import_object(module: &Module, ins_env: &InstanceEnv) -> Result<ImportObject> {
        let mut import_object = module.build_import_object(ins_env)?;
        let mut env_namespace =
            import_object.get_namespace_exports("env").unwrap_or_else(|| Exports::new());
        env_namespace.insert(
            "_wasmy_vm_recall",
            Function::new_native_with_env(
                module.module.store(),
                ins_env.clone(),
                |ins_env: &InstanceEnv, is_ctx: i32, offset: i32| {
                    let key = &ins_env.key;
                    #[cfg(debug_assertions)]
                    println!(
                        "[VM:{:?}]_wasmy_vm_recall: wasm_uri={}, is_ctx={}, offset={}",
                        key.thread_id,
                        key.wasm_uri,
                        is_ctx != 0,
                        offset
                    );
                    let ins = ins_env.as_instance();
                    ins.ctx_write_to(is_ctx != 0, offset as usize);
                },
            ),
        );
        env_namespace.insert(
            "_wasmy_vm_restore",
            Function::new_native_with_env(
                module.module.store(),
                ins_env.clone(),
                |ins_env: &InstanceEnv, offset: i32, size: i32| {
                    let key = &ins_env.key;
                    #[cfg(debug_assertions)]
                    println!(
                        "[VM:{:?}]_wasmy_vm_restore: wasm_uri={}, offset={}, size={}",
                        key.thread_id, key.wasm_uri, offset, size
                    );
                    let ins = ins_env.as_instance();
                    let _ = ins.use_ctx_swap_memory(size as usize, |buffer| {
                        ins.read_memory_bytes(offset as usize, size as usize, buffer);
                        buffer.len()
                    });
                },
            ),
        );
        env_namespace.insert(
            "_wasmy_vm_invoke",
            Function::new_native_with_env(
                module.module.store(),
                ins_env.clone(),
                |ins_env: &InstanceEnv, offset: i32, size: i32| -> i32 {
                    let key = &ins_env.key;
                    #[cfg(debug_assertions)]
                    println!(
                        "[VM:{:?}]_wasmy_vm_invoke: wasm_uri={}, offset={}, size={}",
                        key.thread_id, key.wasm_uri, offset, size
                    );
                    let ins = ins_env.as_instance();
                    let ctx_ptr = ins.context.borrow().value_ptr;
                    ins.use_ctx_swap_memory(size as usize, |buffer| {
                        ins.read_memory_bytes(offset as usize, size as usize, buffer);
                        write_to_vec(&vm_invoke(ctx_ptr, buffer), buffer)
                    }) as i32
                },
            ),
        );
        import_object.register("env", env_namespace);
        Ok(import_object)
    }

    fn init(&self) -> Result<()> {
        let ret = self.raw_call_wasm(WasmHandlerApi::onload_symbol(), &[]).map_or_else(
            |e| {
                if e.code == CODE_NONE {
                    #[cfg(debug_assertions)]
                    println!(
                        "[{:?}]no need initialize instance: wasm_uri={}",
                        self.key.thread_id, self.key.wasm_uri
                    );
                    Ok(())
                } else {
                    e.into_result()
                }
            },
            |_| {
                #[cfg(debug_assertions)]
                println!(
                    "[{:?}]initialized instance: wasm_uri={}",
                    self.key.thread_id, self.key.wasm_uri
                );
                Ok(())
            },
        );
        self.loaded.set(true);
        ret
    }
    fn check_loaded(&self) -> Result<()> {
        if self.loaded.get() {
            Ok(())
        } else {
            CodeMsg::result(CODE_NONE, "instance has not completed initialization")
        }
    }
    #[inline]
    pub(crate) fn handle_wasm(&self, in_args: InArgs) -> Result<OutRets> {
        self.inner_handle_wasm(None::<Empty>, in_args)
    }
    #[inline]
    pub(crate) fn ctx_handle_wasm<C: Message>(
        &self,
        ctx_value: C,
        in_args: InArgs,
    ) -> Result<OutRets> {
        self.inner_handle_wasm(Some(ctx_value), in_args)
    }
    #[inline]
    fn inner_handle_wasm<C: Message>(
        &self,
        ctx_value: Option<C>,
        in_args: InArgs,
    ) -> Result<OutRets> {
        self.check_loaded()?;
        #[cfg(debug_assertions)]
        println!("method={}, data={:?}", in_args.get_method(), in_args.get_data());
        let sign_name = WasmHandlerApi::method_to_symbol(in_args.get_method());
        let (ctx_size, args_size) = self.context.borrow_mut().set_args(ctx_value.as_ref(), in_args);
        self.raw_call_wasm(
            sign_name.as_str(),
            &[Val::I32(ctx_size as i32), Val::I32(args_size as i32)],
        )?;
        Ok(self.context.borrow_mut().out_rets())
    }
    pub fn exports(&self) -> &Exports {
        &self.instance.exports
    }
    pub fn mut_context(&self) -> RefMut<'_, Context> {
        self.context.borrow_mut()
    }
    pub(crate) fn raw_call_wasm(&self, sign_name: &str, args: &[Val]) -> Result<Box<[Val]>> {
        let f = self.exports().get_function(sign_name).map_err(|e| CodeMsg::new(CODE_NONE, e))?;
        loop {
            let rets = f.call(&args);
            match rets {
                Ok(r) => return Ok(r),
                Err(e) => {
                    let estr = format!("{:?}", e);
                    if !estr.contains("OOM") {
                        return CodeMsg::from(e).into_result();
                    }
                    eprintln!("call {} error: {}", sign_name, estr);
                    match self.get_memory().grow(1) {
                        Ok(p) => {
                            println!("memory grow, previous memory size: {:?}", p);
                        }
                        Err(e) => {
                            return CodeMsg::result(
                                CODE_MEM,
                                format!("failed to memory grow: {:?}", e),
                            );
                        }
                    }
                }
            }
        }
    }
    fn ctx_write_to(&self, is_ctx: bool, offset: usize) {
        let mut ctx = self.context.borrow_mut();
        let cache: &mut Vec<u8> =
            if is_ctx { ctx.value_bytes.as_mut() } else { ctx.swap_memory.as_mut() };
        self.write_memory_bytes(offset as usize, cache.iter());
        if !is_ctx {
            unsafe {
                cache.set_len(0);
            }
        }
    }
    pub(crate) fn use_ctx_swap_memory<F: FnOnce(&mut Vec<u8>) -> usize>(
        &self,
        size: usize,
        call: F,
    ) -> usize {
        let mut ctx = self.context.borrow_mut();
        let cache: &mut Vec<u8> = ctx.swap_memory.as_mut();
        if size > 0 {
            resize_with_capacity(cache, size);
        }
        return call(cache);
    }
    fn get_memory(&self) -> &Memory {
        self.instance.exports.get_memory("memory").unwrap()
    }
    fn get_view(&self) -> MemoryView<u8> {
        self.get_memory().view::<u8>()
    }
    pub fn write_memory_bytes<'a>(
        &self,
        offset: usize,
        data: impl IntoIterator<Item = &'a u8> + ExactSizeIterator,
    ) {
        let view = self.get_view();
        for (cell, b) in view[offset..offset + data.len()].iter().zip(data) {
            cell.set(*b);
        }
    }
    pub fn read_memory_bytes(&self, offset: usize, size: usize, buffer: &mut Vec<u8>) {
        if size == 0 {
            resize_with_capacity(buffer, size);
            return;
        }
        let view = self.get_view();
        for x in view[offset..(offset + size)].iter().map(|c| c.get()).enumerate() {
            buffer[x.0] = x.1;
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
    msg.write_to_with_cached_sizes(&mut os).or_else(|e| Err(format!("{}", e))).unwrap();
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
