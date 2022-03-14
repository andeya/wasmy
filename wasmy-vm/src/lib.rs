#![feature(unboxed_closures, fn_traits, thread_id_value)]

pub use entry::*;
pub use handler::*;
pub use instance::*;
pub use modules::*;
pub use wasm_file::*;
pub use wasmer::{import_namespace, Exports, Function, ImportObject, Module, Val};
pub use wasmer_wasi::{WasiState, WasiStateBuilder};
pub use wasmy_abi::*;

mod entry;
mod handler;
mod instance;
mod modules;
mod wasm_file;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
