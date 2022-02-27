#![feature(try_trait_v2)]

pub use abi::*;
pub use types::*;
pub use wasm::*;
pub use wasmy_macros::wasm_handler;

pub mod abi;
pub mod types;
pub mod test;
mod wasm;

pub struct WasmHandlerAPI();

impl WasmHandlerAPI {
    pub fn method_to_symbol(method: Method) -> String {
        format!("_wasm_handler_{}", method)
    }
    pub fn symbol_to_method(symbol: &str) -> Option<Method> {
        symbol.rsplit(|r| r == '_').next().and_then(|s| s.parse().ok())
    }
}

#[cfg(test)]
mod tests {
    use crate::WasmHandlerAPI;

    #[test]
    fn method_to_symbol() {
        let method = WasmHandlerAPI::method_to_symbol(10);
        assert_eq!(method, "_wasm_handler_10");
    }

    #[test]
    fn symbol_to_method() {
        let method = WasmHandlerAPI::symbol_to_method("_wasm_handler_10");
        assert_eq!(method, Some(10));
    }
}
