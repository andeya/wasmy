use std::any::Any as _;
use std::collections::HashMap;
use std::sync::{Once, RwLock};

pub use inventory::submit as submit_handler;
use lazy_static::lazy_static;
pub use wasmy_abi::*;
pub use wasmy_macros::vm_handler;

pub type VmHandler = fn(&Any) -> Result<Any>;

pub struct VmHandlerAPI {
    method: Method,
    handler: VmHandler,
}

static COLLECT_AND_REGISTER_ONCE: Once = Once::new();

impl VmHandlerAPI {
    pub const fn new(method: Method, handler: VmHandler) -> Self {
        VmHandlerAPI { method, handler }
    }
    pub fn register(&self) {
        set_handler(self.method, self.handler)
    }
    pub fn pack_any<R: Message>(data: R) -> Result<Any> {
        pack_any(data)
    }
    pub fn unpack_any<R: Message>(data: &Any) -> Result<R> {
        unpack_any(data)
    }
    pub(crate) fn collect_and_register_once() {
        COLLECT_AND_REGISTER_ONCE.call_once(|| {
            collect_and_register_handlers()
        });
    }
}

lazy_static! {
    static ref MUX: RwLock<HashMap<Method, VmHandler >> = RwLock::new(HashMap::<Method, VmHandler>::new());
}

fn collect_and_register_handlers() {
    inventory::collect!(VmHandlerAPI);
    for info in inventory::iter::<VmHandlerAPI> {
        info.register();
    }
    for (method, hdl) in MUX.read().unwrap().iter() {
        println!("collect_and_register_handlers: method={}, hdl={:?}", method, hdl.type_id());
    }
}

pub fn set_handler(method: Method, hdl: VmHandler) {
    MUX.write().unwrap().insert(method, hdl);
}

#[allow(dead_code)]
pub(crate) fn host_call(args_pb: &Vec<u8>) -> OutResult {
    match InArgs::parse_from_bytes(&args_pb) {
        Ok(host_args) => {
            handle(host_args)
        }
        Err(err) => {
            ERR_CODE_PROTO.to_code_msg(err).into()
        }
    }
}


fn handle(args: InArgs) -> OutResult {
    let res: Result<Any> = MUX.read().unwrap().get(&args.get_method())?(args.get_data());
    match res {
        Ok(a) => a.into(),
        Err(e) => e.into(),
    }
}
