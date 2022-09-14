use protobuf::{CodedOutputStream, Message};
use wasmy_abi::{InArgs, OutRets};

#[derive(Clone, Debug)]
pub struct Context {
    pub(crate) value_ptr: usize,
    pub value_bytes: Vec<u8>,
    pub swap_memory: Vec<u8>,
}

impl Context {
    pub(crate) fn with_capacity(capacity: usize) -> Self {
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

    pub(crate) fn set_args<C: Message>(
        &mut self,
        ctx_value: Option<&C>,
        in_args: InArgs,
    ) -> (usize, usize) {
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

    pub(crate) fn out_rets(&mut self) -> OutRets {
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

pub(crate) fn write_to_vec(msg: &dyn Message, buffer: &mut Vec<u8>) -> usize {
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

pub(crate) fn resize_with_capacity(buffer: &mut Vec<u8>, new_size: usize) {
    if new_size > buffer.capacity() {
        buffer.resize(new_size, 0);
    } else {
        unsafe { buffer.set_len(new_size) };
    }
}
