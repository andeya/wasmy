#![feature(try_trait_v2)]

pub use abi::*;
pub use types::*;
pub use wasm::*;
pub use wasmy_macros::{wasm_handle, wasm_onload};

pub mod abi;
pub mod types;
pub mod test;
mod wasm;
