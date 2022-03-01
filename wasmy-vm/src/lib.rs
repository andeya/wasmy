#![feature(unboxed_closures, fn_traits, thread_id_value, const_fn_fn_ptr_basics)]

pub use entry::*;
pub use handler::*;
pub use wasm_info::*;
pub use wasmy_abi::*;

mod handler;
mod instance;
mod entry;
mod wasm_info;
#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
