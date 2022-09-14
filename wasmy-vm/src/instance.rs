use core::ops::FnOnce;
use std::{
    cell::{RefCell, RefMut},
    collections::HashMap,
    sync::{Mutex, RwLock},
    thread,
    thread::ThreadId,
};

use lazy_static;
use wasmer::{Exports, Function, Imports, MemoryView, Module, Store, Type, Value};
use wasmer_wasi::{WasiFunctionEnv, WasiState, WasiStateBuilder};

use crate::{
    context, context::Context, handler::*, instance_env::InstanceEnv, wasm_file,
    wasm_file::WasmFile, WasmUri,
};

pub type FunctionEnvMut<'a> = wasmer::FunctionEnvMut<'a, InstanceEnv>;
pub type FunctionEnv = wasmer::FunctionEnv<InstanceEnv>;
pub type FnCheckModule = fn(&Module) -> Result<()>;
pub type FnBuildImports = fn(
    builder: &mut WasiStateBuilder,
    store: &mut Store,
    module: &mut Module,
    env: &FunctionEnv,
) -> Result<(WasiFunctionEnv, Imports)>;

lazy_static::lazy_static! {
    pub(crate) static ref INSTANCES: RwLock<HashMap<LocalInstanceKey, Mutex<Box<Instance>>>> = RwLock::new(HashMap::new());
}

#[derive(Debug)]
pub struct Instance {
    key: LocalInstanceKey,
    instance: wasmer::Instance,
    store: Store,
    context: RefCell<Context>,
}

#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub struct LocalInstanceKey {
    pub(crate) wasm_uri: WasmUri,
    pub(crate) thread_id: ThreadId,
}

impl LocalInstanceKey {
    pub(crate) fn from(wasm_uri: WasmUri) -> LocalInstanceKey {
        LocalInstanceKey { wasm_uri, thread_id: thread::current().id() }
    }
}

unsafe impl Sync for Instance {}

unsafe impl Send for Instance {}

impl Instance {
    pub fn thread_id(&self) -> &ThreadId {
        &self.key.thread_id
    }
    pub fn wasm_uri(&self) -> &WasmUri {
        &self.key.wasm_uri
    }
    pub(crate) fn install<B, W>(
        wasm_file: W,
        check_module: Option<FnCheckModule>,
        build_imports: Option<FnBuildImports>,
    ) -> Result<WasmUri>
    where
        B: AsRef<[u8]>,
        W: WasmFile<B>,
    {
        // collect and register handlers once
        VmHandlerApi::collect_and_register_once();
        // read and cache wasm file
        let wasm_uri = wasm_file::register_file(wasm_file)?;
        Self::create_local(
            wasm_uri.clone(),
            wasm_file::get_files().get(&wasm_uri).unwrap(),
            check_module,
            build_imports,
            true,
        )?;
        #[cfg(debug_assertions)]
        println!("loaded wasm, uri={}", wasm_uri);
        Ok(wasm_uri)
    }

    fn create_local(
        wasm_uri: WasmUri,
        wasm_bytes: &Vec<u8>,
        check_module: Option<FnCheckModule>,
        build_imports: Option<FnBuildImports>,
        first: bool,
    ) -> Result<()> {
        #[cfg(debug_assertions)]
        println!("compiling module, wasm_uri={}...", wasm_uri);

        let engine;
        #[cfg(not(feature = "llvm"))]
        {
            engine = wasmer_compiler_cranelift::Cranelift::default();
            #[cfg(debug_assertions)]
            if first {
                println!("======== wasmy cranelift feature ========")
            }
        }
        #[cfg(feature = "llvm")]
        {
            engine = wasmer_compiler_llvm::LLVM::default();
            #[cfg(debug_assertions)]
            if first {
                println!("======== wasmy llvm feature ========")
            }
        }

        let mut store: Store = Store::new(engine);
        let mut module = Module::from_binary(&store, wasm_bytes)?;
        module.set_name(wasm_uri.as_str());
        if let Some(cf) = check_module {
            cf(&module)?;
        };
        if first {
            for function in module.exports().functions() {
                let name = function.name();
                if name == WasmHandlerApi::onload_symbol() {
                    let ty = function.ty();
                    if ty.params().len() > 0 || ty.results().len() > 0 {
                        return CodeMsg::result(
                            CODE_EXPORTS,
                            format!(
                                "Incompatible Export Type: fn {}(){{}}",
                                WasmHandlerApi::onload_symbol()
                            ),
                        );
                    }
                    continue;
                }
                WasmHandlerApi::symbol_to_method(name).map_or_else(
                    || {
                        #[cfg(debug_assertions)]
                        {
                            println!("module exports non-wasmy function: {:?}", function);
                            Ok(())
                        }
                    },
                    |_method| {
                        let ty = function.ty();
                        if ty.results().len() == 0 && ty.params().eq(&[Type::I32, Type::I32]) {
                            #[cfg(debug_assertions)]
                            println!("module exports wasmy function: {:?}", function);
                            Ok(())
                        } else {
                            return CodeMsg::result(
                                CODE_EXPORTS,
                                format!("Incompatible Export Type: {:?}", function),
                            );
                        }
                    },
                )?;
            }
        }
        let key = LocalInstanceKey::from(wasm_uri);
        let ins_env = FunctionEnv::new(&mut store, InstanceEnv::default());
        let (wasi_env, imports) =
            Self::build_imports(&key, &mut module, &mut store, &ins_env, build_imports)?;
        #[cfg(debug_assertions)]
        if first {
            for ((namespace, name), r#extern) in imports.clone().into_iter() {
                println!(
                    "import: namespace={namespace}, name={name}, extern_type={:?}",
                    r#extern.ty(&store)
                );
            }
        }
        let mut instance = Instance {
            key,
            instance: wasmer::Instance::new(&mut store, &module, &imports)?,
            store,
            context: RefCell::new(Context::with_capacity(1024)),
        };

        // Attach the memory export
        let memory = instance.instance.exports.get_memory("memory").unwrap();
        wasi_env.data_mut(&mut instance.store).set_memory(memory.clone());

        // initialize
        instance.into_init(ins_env)?;

        return Ok(());
    }

    pub(crate) fn with<F, R>(wasm_uri: WasmUri, callback: F) -> Result<R>
    where
        F: FnOnce(&mut Instance) -> Result<R>,
    {
        let key = LocalInstanceKey::from(wasm_uri.clone());
        {
            if let Some(ins) = INSTANCES.read().unwrap().get(&key) {
                return callback(ins.lock().unwrap().as_mut());
            }
        }
        let files = wasm_file::get_files();
        let wasm_bytes = files.get(&key.wasm_uri);
        if wasm_bytes.is_none() {
            return Err(CodeMsg::new(
                CODE_WASI,
                format!("wasm file not found, wasm_uri={}", key.wasm_uri),
            ));
        }
        Self::create_local(key.wasm_uri.clone(), wasm_bytes.unwrap(), None, None, false)?;
        Self::with(wasm_uri, callback)
    }

    fn build_imports(
        key: &LocalInstanceKey,
        module: &mut Module,
        store: &mut Store,
        ins_env: &FunctionEnv,
        build_imports: Option<FnBuildImports>,
    ) -> Result<(WasiFunctionEnv, Imports)> {
        let (wasi_env, mut imports) = build_imports.unwrap_or(default_imports)(
            &mut WasiState::new(&key.wasm_uri),
            store,
            module,
            ins_env,
        )?;
        let mut env_namespace =
            imports.get_namespace_exports("env").unwrap_or_else(|| Exports::new());
        env_namespace.insert(
            "_wasmy_vm_recall",
            Function::new_typed_with_env(
                store,
                ins_env,
                |ins_env: FunctionEnvMut, is_ctx: i32, offset: i32| {
                    let ins_env = ins_env.data();
                    let key = &ins_env.key;
                    #[cfg(debug_assertions)]
                    println!(
                        "[VM:{:?}]_wasmy_vm_recall: wasm_uri={}, is_ctx={}, offset={}",
                        key.thread_id,
                        key.wasm_uri,
                        is_ctx != 0,
                        offset
                    );
                    ins_env.ctx_write_to(is_ctx != 0, offset as u64);
                },
            ),
        );
        env_namespace.insert(
            "_wasmy_vm_restore",
            Function::new_typed_with_env(
                store,
                ins_env,
                |ins_env: FunctionEnvMut, offset: i32, size: i32| {
                    let ins_env = ins_env.data();
                    let key = &ins_env.key;
                    #[cfg(debug_assertions)]
                    println!(
                        "[VM:{:?}]_wasmy_vm_restore: wasm_uri={}, offset={}, size={}",
                        key.thread_id, key.wasm_uri, offset, size
                    );
                    let _ = ins_env.use_ctx_swap_memory(size as usize, |buffer| {
                        ins_env.read_memory_bytes(offset as u64, size as usize, buffer);
                        buffer.len()
                    });
                },
            ),
        );
        env_namespace.insert(
            "_wasmy_vm_invoke",
            Function::new_typed_with_env(
                store,
                ins_env,
                |ins_env: FunctionEnvMut, offset: i32, size: i32| -> i32 {
                    let ins_env = ins_env.data();
                    let key = &ins_env.key;
                    #[cfg(debug_assertions)]
                    println!(
                        "[VM:{:?}]_wasmy_vm_invoke: wasm_uri={}, offset={}, size={}",
                        key.thread_id, key.wasm_uri, offset, size
                    );
                    let ctx_ptr = ins_env.context.borrow().value_ptr;
                    ins_env.use_ctx_swap_memory(size as usize, |buffer| {
                        ins_env.read_memory_bytes(offset as u64, size as usize, buffer);
                        context::write_to_vec(&vm_invoke(ctx_ptr, buffer), buffer)
                    }) as i32
                },
            ),
        );
        imports.register_namespace("env", env_namespace);
        Ok((wasi_env, imports))
    }

    fn into_init(mut self, ins_env: FunctionEnv) -> Result<()> {
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
        let mut x = INSTANCES.write().unwrap();
        let key = self.key.clone();
        x.insert(key.clone(), Mutex::new(Box::new(self)));
        let ins = x.get_mut(&key).unwrap().get_mut().unwrap().as_mut();
        let ptr = ins as *mut Instance;
        ins_env.as_mut(&mut ins.store).set(ptr);
        ret
    }

    #[inline]
    pub(crate) fn handle_wasm(&mut self, in_args: InArgs) -> Result<OutRets> {
        self.inner_handle_wasm(None::<Empty>, in_args)
    }
    #[inline]
    pub(crate) fn ctx_handle_wasm<C: Message>(
        &mut self,
        ctx_value: C,
        in_args: InArgs,
    ) -> Result<OutRets> {
        self.inner_handle_wasm(Some(ctx_value), in_args)
    }
    #[inline]
    fn inner_handle_wasm<C: Message>(
        &mut self,
        ctx_value: Option<C>,
        in_args: InArgs,
    ) -> Result<OutRets> {
        #[cfg(debug_assertions)]
        println!("method={}, data={:?}", in_args.get_method(), in_args.get_data());
        let sign_name = WasmHandlerApi::method_to_symbol(in_args.get_method());
        let (ctx_size, args_size) = self.context.borrow_mut().set_args(ctx_value.as_ref(), in_args);
        self.raw_call_wasm(
            sign_name.as_str(),
            &[Value::I32(ctx_size as i32), Value::I32(args_size as i32)],
        )?;
        Ok(self.context.borrow_mut().out_rets())
    }
    pub fn exports(&self) -> &Exports {
        &self.instance.exports
    }
    pub fn mut_context(&self) -> RefMut<'_, Context> {
        self.context.borrow_mut()
    }
    pub(crate) fn raw_call_wasm(
        &mut self,
        sign_name: &str,
        args: &[Value],
    ) -> Result<Box<[Value]>> {
        let exports = &mut self.instance.exports;
        let store = &mut self.store;
        let f = exports.get_function(sign_name).map_err(|e| CodeMsg::new(CODE_NONE, e))?;
        loop {
            let rets = f.call(store, &args);
            match rets {
                Ok(r) => return Ok(r),
                Err(e) => {
                    let estr = format!("{:?}", e);
                    if !estr.contains("OOM") {
                        return CodeMsg::from(e).into_result();
                    }
                    eprintln!("call {} error: {}", sign_name, estr);
                    match exports.get_memory("memory").unwrap().grow(store, 1) {
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

    fn ctx_write_to(&self, is_ctx: bool, offset: u64) {
        let mut ctx = self.context.borrow_mut();
        let cache: &mut Vec<u8> =
            if is_ctx { ctx.value_bytes.as_mut() } else { ctx.swap_memory.as_mut() };
        self.write_memory_bytes(offset, cache.as_slice());
        if !is_ctx {
            unsafe {
                cache.set_len(0);
            }
        }
    }
    fn use_ctx_swap_memory<F: FnOnce(&mut Vec<u8>) -> usize>(&self, size: usize, call: F) -> usize {
        let mut ctx = self.context.borrow_mut();
        let cache: &mut Vec<u8> = ctx.swap_memory.as_mut();
        if size > 0 {
            context::resize_with_capacity(cache, size);
        }
        return call(cache);
    }
    fn get_view(&self) -> MemoryView {
        self.instance.exports.get_memory("memory").unwrap().view(&self.store)
    }
    pub fn write_memory_bytes<'a>(&self, offset: u64, data: &[u8]) {
        self.get_view().write(offset, data).unwrap();
    }
    pub fn read_memory_bytes(&self, offset: u64, size: usize, buffer: &mut Vec<u8>) {
        if size == 0 {
            context::resize_with_capacity(buffer, size);
            return;
        }
        self.get_view().read(offset, buffer).unwrap();
    }
}

fn default_imports(
    builder: &mut WasiStateBuilder,
    store: &mut Store,
    module: &mut Module,
    _env: &FunctionEnv,
) -> Result<(WasiFunctionEnv, Imports)> {
    let wasi_env = builder
        // First, we create the `WasiEnv` with the stdio pipes
        .finalize(store)?;
    // Then, we get the import object related to our WASI
    // and attach it to the Wasm instance.
    let imports = wasi_env.import_object(store, module)?;
    Ok((wasi_env, imports))
}
