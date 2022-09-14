#![feature(unboxed_closures, fn_traits, thread_id_value)]

pub use entry::*;
pub use handler::*;
pub use instance::*;
pub use wasm_file::*;
pub use wasmer::{import_namespace, Exports, Function, Imports, Module, Store};
pub use wasmer_wasi::{WasiFunctionEnv, WasiStateBuilder};
pub use wasmy_abi::*;

mod context;
mod entry;
mod handler;
mod instance;
mod instance_env;
mod wasm_file;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
