#![feature(try_trait_v2)]

pub use abi::*;
pub use types::*;
pub use wasm::*;
pub use wasmy_macros::wasm_entry;

pub mod abi;
pub mod types;
pub mod test;
mod wasm;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
