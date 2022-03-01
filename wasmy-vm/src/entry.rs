use wasmy_abi::*;

use crate::{instance, WasmURI};
use crate::wasm_info::WasmInfo;

pub fn load_wasm<B, W>(wasm_info: W) -> Result<WasmURI>
    where B: AsRef<[u8]>,
          W: WasmInfo<B>,
{
    let wasm_uri = wasm_info.wasm_uri();
    instance::load_with(wasm_info, |_| Ok(wasm_uri))
}

impl WasmURI {
    pub fn call_wasm<A: Message, R: Message>(&self, method: Method, data: A) -> Result<R> {
        call_wasm(self, method, data)
    }
}

fn call_wasm<A: Message, R: Message>(wasm_uri: &WasmURI, method: Method, data: A) -> Result<R> {
    let in_args = InArgs::try_new(method, data)?;
    instance::with(wasm_uri, |ins| -> Result<R>{
        let ctx_id = ins.gen_ctx_id();
        #[cfg(debug_assertions)] println!("ctx_id={}, method={}, data={:?}", ctx_id, in_args.get_method(), in_args.get_data());
        let buffer_len = ins.use_mut_buffer(ctx_id, in_args.compute_size() as usize, |buffer| {
            instance::write_to_with_cached_sizes(&in_args, buffer)
        });
        ins.call_wasm_handler(method, ctx_id, buffer_len as i32)?;
        let buffer = ins.take_buffer(ctx_id).unwrap_or(vec![]);
        let res = if buffer.len() > 0 {
            OutResult::parse_from_bytes(buffer.as_slice()).unwrap()
        } else {
            OutResult::new()
        };
        ins.try_reuse_buffer(buffer);
        res.into()
    })
}



