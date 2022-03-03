pub use protobuf::{CodedOutputStream, Message, ProtobufEnum};
pub use protobuf::well_known_types::Any;

use crate::abi::*;
use crate::types::*;

/// The ABI interaction functions of the virtual machine.
// #[cfg(target_arch = "wasm32")]
extern "C" {
    pub(crate) fn _vm_recall(ctx_id: CtxID, offset: i32);
    pub(crate) fn _vm_restore(ctx_id: CtxID, offset: i32, size: i32);
    pub(crate) fn _vm_invoke(ctx_id: CtxID, offset: i32, size: i32) -> i32;
}

/// The underlying function of wasm to handle requests.
pub fn wasm_handle<F>(ctx_id: CtxID, size: i32, handle: F)
    where
        F: Fn(Ctx, InArgs) -> Result<Any>,
{
    if size <= 0 {
        return;
    }
    let mut buffer = vec![0u8; size as usize];
    unsafe { _vm_recall(ctx_id, buffer.as_ptr() as i32) };
    let res: OutRets = match InArgs::parse_from_bytes(&buffer) {
        Ok(guest_args) => {
            handle(Ctx::from_id(ctx_id), guest_args).into()
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
    unsafe { _vm_restore(ctx_id, buffer.as_ptr() as i32, buffer.len() as i32) };
}

impl Ctx {
    pub fn call_vm<M: Message, R: Message>(&self, method: VmMethod, data: M) -> Result<R> {
        let args = InArgs::try_new(method, data)?;
        let mut buffer = args.write_to_bytes().unwrap();
        let size = unsafe { _vm_invoke(self.get_id(), buffer.as_ptr() as i32, buffer.len() as i32) };
        if size <= 0 {
            return Ok(R::new());
        }
        buffer.resize(size as usize, 0);
        unsafe { _vm_recall(self.get_id(), buffer.as_ptr() as i32) };
        OutRets::parse_from_bytes(buffer.as_slice())?
            .into()
    }
}
