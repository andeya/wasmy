use std::marker::PhantomData;

pub use protobuf::{CodedOutputStream, Message, ProtobufEnum};
pub use protobuf::well_known_types::Any;

use crate::abi::*;
use crate::types::*;

/// The ABI interaction functions of the virtual machine.
// #[cfg(target_arch = "wasm32")]
extern "C" {
    pub(crate) fn _vm_recall(is_ctx: i32, offset: i32);
    pub(crate) fn _vm_restore(offset: i32, size: i32);
    pub(crate) fn _vm_invoke(offset: i32, size: i32) -> i32;
}

const NON_CTX: i32 = 0;
const IS_CTX: i32 = 1;

/// The underlying function of wasm to handle requests.
pub fn wasm_handle<F, C>(ctx_size: i32, args_size: i32, handle: F)
    where
        F: Fn(WasmCtx<C>, InArgs) -> Result<Any>,
        C: Message,
{
    if args_size <= 0 {
        return;
    }
    let mut buffer = vec![0u8; args_size as usize];
    unsafe { _vm_recall(NON_CTX, buffer.as_ptr() as i32) };
    let res: OutRets = match InArgs::parse_from_bytes(&buffer) {
        Ok(args) => {
            handle(WasmCtx::from_size(ctx_size as usize), args).into()
        }
        Err(err) => {
            ERR_CODE_PROTO.to_code_msg(err).into()
        }
    };
    let size = res.compute_size() as usize;
    if size > buffer.capacity() {
        buffer.resize(size, 0);
    } else {
        unsafe { buffer.set_len(size) };
    }
    let mut os = CodedOutputStream::bytes(&mut buffer);
    res.write_to_with_cached_sizes(&mut os).unwrap();
    os.flush().unwrap();
    unsafe { _vm_restore(buffer.as_ptr() as i32, buffer.len() as i32) };
}

impl<C: Message> WasmCtx<C> {
    fn from_size(size: usize) -> Self {
        Self { size, _priv: PhantomData }
    }
    pub fn try_value(&self) -> Result<C> {
        if self.size == 0 {
            Ok(C::new())
        } else {
            let buffer = vec![0u8; self.size];
            unsafe { _vm_recall(IS_CTX, buffer.as_ptr() as i32) };
            match C::parse_from_bytes(&buffer) {
                Ok(ctx) => {
                    Ok(ctx)
                }
                Err(err) => {
                    Err(ERR_CODE_PROTO.to_code_msg(err))
                }
            }
        }
    }
    pub fn call_vm<M: Message, R: Message>(&self, method: VmMethod, data: M) -> Result<R> {
        let args = InArgs::try_new(method, data)?;
        let mut buffer = args.write_to_bytes().unwrap();
        let size = unsafe { _vm_invoke(buffer.as_ptr() as i32, buffer.len() as i32) };
        if size <= 0 {
            return Ok(R::new());
        }
        buffer.resize(size as usize, 0);
        unsafe { _vm_recall(NON_CTX, buffer.as_ptr() as i32) };
        OutRets::parse_from_bytes(buffer.as_slice())?
            .into()
    }
}
