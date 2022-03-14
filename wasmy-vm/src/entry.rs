use wasmer::Val;
use wasmy_abi::*;

use crate::{
    instance,
    modules::{FnBuildImportObject, FnCheckModule},
    wasm_file::WasmFile,
    Instance, WasmUri,
};

pub fn load_wasm<B, W>(wasm_file: W) -> Result<WasmCaller>
where
    B: AsRef<[u8]>,
    W: WasmFile<B>,
{
    Ok(WasmCaller(instance::load(wasm_file, None, None)?))
}

pub fn custom_load_wasm<B, W>(
    wasm_file: W,
    check_module: Option<FnCheckModule>,
    build_import_object: Option<FnBuildImportObject>,
) -> Result<WasmCaller>
where
    B: AsRef<[u8]>,
    W: WasmFile<B>,
{
    Ok(WasmCaller(instance::load(wasm_file, check_module, build_import_object)?))
}

#[derive(Clone, Hash, Eq, PartialEq, Debug)]
pub struct WasmCaller(WasmUri);

impl WasmUri {
    pub fn into_caller(self) -> WasmCaller {
        WasmCaller(self)
    }
    pub fn as_caller(&self) -> &WasmCaller {
        unsafe { &*(self as *const WasmUri as *const WasmCaller) }
    }
}

impl WasmCaller {
    /// create wasm caller from URI.
    pub fn from(wasm_uri: WasmUri) -> WasmCaller {
        WasmCaller(wasm_uri)
    }
    /// Get the wasm URI.
    pub fn wasm_uri(&self) -> &WasmUri {
        &self.0
    }
    /// Call the wasm specified method.
    pub fn call<A: Message, R: Message>(&self, method: Method, data: A) -> Result<R> {
        let in_args = InArgs::try_new(method, data)?;
        instance::with(self.0.clone(), |ins| -> Result<R> {
            ins.handle_wasm(method, in_args)?.into()
        })
    }
    /// Carry the context to call the wasm specified method.
    pub fn ctx_call<C: Message, A: Message, R: Message>(
        &self,
        ctx: C,
        method: Method,
        data: A,
    ) -> Result<R> {
        let in_args = InArgs::try_new(method, data)?;
        instance::with(self.0.clone(), |ins| -> Result<R> {
            ins.ctx_handle_wasm(ctx, method, in_args)?.into()
        })
    }
    // // Execute the raw call to wasm.
    pub fn raw_call(&self, sign_name: &str, args: &[Val]) -> Result<Box<[Val]>> {
        instance::with(self.0.clone(), |ins| -> Result<Box<[Val]>> {
            ins.raw_call_wasm(sign_name, args)
        })
    }
    /// Get instance and do custom operations.
    pub fn with<F, R>(&self, callback: F) -> Result<R>
    where
        F: FnOnce(&Instance) -> Result<R>,
    {
        instance::with(self.0.clone(), |ins| -> Result<R> { callback(ins) })
    }
}
