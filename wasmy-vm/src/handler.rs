use std::any::Any as _;
use std::collections::HashMap;
use std::sync::RwLock;

pub use inventory::submit as submit_handler;
use lazy_static::lazy_static;
pub use wasmy_macros::vm_handler;

pub use wasmy_abi::*;

pub type Handler = fn(&Any) -> Result<Any>;

pub struct HandlerAPI {
    method: Method,
    handler: Handler,
}

impl HandlerAPI {
    pub const fn new(method: Method, handler: Handler) -> Self {
        HandlerAPI { method, handler }
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
    pub fn collect_and_register_all() {
        collect_and_register_handlers()
    }
}

lazy_static! {
    static ref MUX: RwLock<HashMap<Method, Handler>> = RwLock::new(HashMap::<Method, Handler>::new());
}

fn collect_and_register_handlers() {
    inventory::collect!(HandlerAPI);
    for info in inventory::iter::<HandlerAPI> {
        info.register();
    }
    for (method, hdl) in MUX.read().unwrap().iter() {
        println!("collect_and_register_handlers: method={}, hdl={:?}", method, hdl.type_id());
    }
}

pub fn set_handler(method: Method, hdl: Handler) {
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


#[cfg(test)]
mod test {
    use wasmy_abi::test::{TestArgs, TestResult};

    use super::*;

    #[test]
    fn add() {
        #[vm_handler(1)]
        fn add1(args: TestArgs) -> Result<TestResult> {
            let mut res = TestResult::new();
            res.set_sum(args.a + args.b);
            Ok(res)
        }
        HandlerAPI::collect_and_register_all()
    }
    // Expanded codes:
    // #[allow(redundant_semicolons)] fn add1(args : & Any) -> Result < Any >
    // {
    //     fn add1(args : TestArgs) -> Result < TestResult >
    //     {
    //         let mut res = TestResult :: new() ; res.set_sum(args.a + args.b) ;
    //         Ok(res)
    //     } ; let args : TestArgs = HandlerAPI :: unpack_any(args) ? ;
    //     add1(args).and_then(| res | HandlerAPI :: pack_any(& res))
    // } submit_handler! { HandlerAPI :: new(1i32, add1) }
}
