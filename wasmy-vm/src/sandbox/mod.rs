pub use wasmy_abi::*;

pub use crate::sandbox::instance::WasmInfo;

mod instance;

pub fn load_wasm(wasm_info: WasmInfo) -> Result<()> {
    instance::load(&wasm_info, |_ins| -> Result<()>{ Ok(()) })
}

pub fn call_wasm<A: Message, R: Message>(wasm_info: WasmInfo, method: Method, data: A) -> Result<R> {
    let guest_args = InArgs::try_new(method, data)?;
    instance::load(&wasm_info, |ins| -> Result<R>{
        let ctx_id = ins.gen_ctx_id();
        #[cfg(debug_assertions)] println!("ctx_id={}, guest_args={:?}", ctx_id, guest_args);
        let buffer_len = ins.use_mut_buffer(ctx_id, guest_args.compute_size() as usize, |buffer| {
            write_to_with_cached_sizes(&guest_args, buffer)
        });
        ins.call_wasm_main(ctx_id, buffer_len as i32);
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
