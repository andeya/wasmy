use wasmy_abi::*;

use crate::{instance, WasmUri};
use crate::wasm_info::WasmInfo;

pub fn load_wasm<B, W>(wasm_info: W) -> Result<WasmCaller>
    where B: AsRef<[u8]>,
          W: WasmInfo<B>,
{
    Ok(WasmCaller(instance::load(wasm_info)?))
}

#[derive(Clone, Hash, Eq, PartialEq, Debug)]
pub struct WasmCaller(WasmUri);

impl WasmUri {
    pub fn into_caller(self) -> WasmCaller {
        WasmCaller(self)
    }
}

impl WasmCaller {
    pub fn from(wasm_uri: WasmUri) -> WasmCaller {
        WasmCaller(wasm_uri)
    }
    pub fn wasm_uri(&self) -> &WasmUri {
        &self.0
    }
    pub fn call<A: Message, R: Message>(&self, method: Method, data: A) -> Result<R> {
        let in_args = InArgs::try_new(method, data)?;
        instance::with(self.0.clone(), |ins| -> Result<R>{
            ins.call_wasm_handler(method, in_args)?.into()
        })
    }
}
