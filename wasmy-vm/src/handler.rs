use std::any::Any as _;
use std::collections::HashMap;
use std::sync::{Once, RwLock};

pub use inventory::submit as submit_handler;
use lazy_static::lazy_static;

pub use wasmy_abi::{abi::*, types::*};
pub use wasmy_macros::vm_handle;

pub type VmHandler = fn(usize, &Any) -> Result<Any>;

pub struct VmHandlerApi {
    method: Method,
    handler: VmHandler,
}

static COLLECT_AND_REGISTER_ONCE: Once = Once::new();

impl VmHandlerApi {
    pub const fn new(method: Method, handler: VmHandler) -> Self {
        VmHandlerApi { method, handler }
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
    pub unsafe fn try_as<T: Message>(ptr: usize) -> Option<&'static T> {
        if ptr > 0 {
            Some(&*(ptr as *const T))
        } else {
            None
        }
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
    inventory::collect!(VmHandlerApi);
    for info in inventory::iter::<VmHandlerApi> {
        info.register();
    }
    for (method, hdl) in MUX.read().unwrap().iter() {
        println!("collect_and_register_handlers: method={}, hdl_type_id={:?}", method, hdl.type_id());
    }
}

pub fn set_handler(method: Method, hdl: VmHandler) {
    let ty = hdl.type_id();
    if let Some(old) = MUX.write().unwrap().insert(method, hdl) {
        if old.type_id() != ty {
            panic!("duplicate register handler: method={}, old_type_id={:?}, new_type_id={:?}", method, old.type_id(), ty);
        }
    }
}

#[allow(dead_code)]
pub(crate) fn vm_invoke(ctx_ptr: usize, args_pb: &Vec<u8>) -> OutRets {
    match InArgs::parse_from_bytes(&args_pb) {
        Ok(vm_args) => {
            handle(ctx_ptr, vm_args)
        }
        Err(err) => {
           CodeMsg::new(CODE_PROTO, err).into()
        }
    }
}


fn handle(ctx_ptr: usize, args: InArgs) -> OutRets {
    let res: Result<Any> = MUX.read().unwrap()
                              .get(&args.get_method())
                              .ok_or_else(|| {
                                  CodeMsg::new(CODE_NONE, format!("undefined virtual machine method({})", args.get_method()))
                              })?(ctx_ptr, args.get_data());
    match res {
        Ok(a) => a.into(),
        Err(e) => e.into(),
    }
}


pub(crate) struct WasmHandlerApi();

impl WasmHandlerApi {
    pub const fn onload_symbol() -> &'static str {
        "_wasmy_wasm_onload"
    }
    pub fn method_to_symbol(method: WasmMethod) -> String {
        format!("_wasmy_wasm_handle_{}", method)
    }
    pub fn symbol_to_method(symbol: &str) -> Option<WasmMethod> {
        if let Some(s) = symbol.strip_prefix("_wasmy_wasm_handle_") {
            s.parse().ok()
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::WasmHandlerApi;

    #[test]
    fn method_to_symbol() {
        let method = WasmHandlerApi::method_to_symbol(10);
        assert_eq!(method, "_wasmy_wasm_handle_10");
    }

    #[test]
    fn symbol_to_method() {
        let method = WasmHandlerApi::symbol_to_method("_wasmy_wasm_handle_10");
        assert_eq!(method, Some(10));
    }
}
