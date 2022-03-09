#![feature(unboxed_closures, fn_traits, thread_id_value)]

pub use wasmer::{Function, import_namespace, ImportObject, Module};
pub use wasmer_wasi::{WasiState, WasiStateBuilder};

pub use entry::*;
pub use handler::*;
pub use instance::*;
pub use modules::*;
pub use wasm_file::*;
pub use wasmy_abi::*;

mod handler;
mod instance;
mod entry;
mod wasm_file;
mod modules;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
