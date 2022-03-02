use wasmy_abi::*;

use crate::{instance, WasmUri};
use crate::wasm_info::WasmInfo;

pub fn load_wasm<B, W>(wasm_info: W) -> Result<WasmUri>
    where B: AsRef<[u8]>,
          W: WasmInfo<B>,
{
    instance::load(wasm_info)
}

impl WasmUri {
    pub fn call_wasm<A: Message, R: Message>(&self, method: Method, data: A) -> Result<R> {
        let in_args = InArgs::try_new(method, data)?;
        instance::with(self.clone(), |ins| -> Result<R>{
            ins.call_wasm_handler(method, in_args)?.into()
        })
    }
}
