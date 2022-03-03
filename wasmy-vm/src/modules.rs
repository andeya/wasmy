use lazy_static;
use wasmer::{Module, Store, Type};
use wasmer_compiler_cranelift::Cranelift;
use wasmer_engine_universal::Universal;
use std::sync::RwLock;
use std::collections::HashMap;
use wasmy_abi::WasmHandlerAPI;

use crate::{VmHandlerAPI, WasmInfo, WasmUri};

lazy_static::lazy_static! {
   pub(crate) static ref MODULES: RwLock<HashMap<WasmUri, Module>> = RwLock::new(HashMap::new());
}

pub(crate) fn load<B, W>(wasm_info: W) -> anyhow::Result<WasmUri>
    where B: AsRef<[u8]>,
          W: WasmInfo<B>,
{
    // collect and register handlers once
    VmHandlerAPI::collect_and_register_once();
    let wasm_uri = wasm_info.wasm_uri();

    #[cfg(debug_assertions)] println!("compiling module, wasm_uri={}...", wasm_uri);
    let store: Store = Store::new(&Universal::new(Cranelift::default()).engine());
    let mut module = Module::new(&store, wasm_info.into_wasm_bytes()?)?;
    module.set_name(wasm_uri.as_str());
    #[cfg(debug_assertions)] println!("compiled module, wasm_uri={}...", wasm_uri);

    for function in module.exports().functions() {
        let name = function.name();
        if name == WasmHandlerAPI::onload_symbol() {
            let ty = function.ty();
            if ty.params().len() > 0 || ty.results().len() > 0 {
                return Err(anyhow::Error::msg(format!("Incompatible Export Type: fn {}(){{}}", WasmHandlerAPI::onload_symbol())));
            }
            continue;
        }
        WasmHandlerAPI::symbol_to_method(name).map_or_else(|| {
            #[cfg(debug_assertions)]println!("module exports function(invalid for vm): {:?}", function);
        }, |_method| {
            let ty = function.ty();
            if ty.results().len() == 0 && ty.params().eq(&[Type::I32, Type::I32]) {
                #[cfg(debug_assertions)]println!("module exports function(valid for vm): {:?}", function);
            } else {
                #[cfg(debug_assertions)]println!("module exports function(invalid for vm): {:?}", function);
            }
        });
    }

    MODULES.write().unwrap().insert(wasm_uri.clone(), module);
    Ok(wasm_uri.clone())
}