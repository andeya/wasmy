use std::{collections::HashMap, sync::RwLock};

use lazy_static;
use wasmer::{ImportObject, Store, Type};
use wasmer_compiler_cranelift::Cranelift;
use wasmer_engine_universal::Universal;
use wasmer_wasi::{WasiState, WasiStateBuilder};
use wasmy_abi::{CodeMsg, Result, CODE_EXPORTS};

use crate::{handler::WasmHandlerApi, instance::LocalInstanceKey, VmHandlerApi, WasmFile, WasmUri};

pub(crate) struct Module {
    pub(crate) module: wasmer::Module,
    fn_build_import_object: FnBuildImportObject,
}

impl Module {
    pub(crate) fn build_import_object(&self, key: &LocalInstanceKey) -> Result<ImportObject> {
        (self.fn_build_import_object)(&self.module, key)
    }
}

pub type FnCheckModule = fn(&wasmer::Module) -> Result<()>;
pub type FnBuildImportObject = fn(&wasmer::Module, &LocalInstanceKey) -> Result<ImportObject>;

lazy_static::lazy_static! {
   pub(crate) static ref MODULES: RwLock<HashMap<WasmUri, Module>> = RwLock::new(HashMap::new());
}

pub(crate) fn load<B, W>(
    wasm_file: W,
    check_module: Option<FnCheckModule>,
    build_import_object: Option<FnBuildImportObject>,
) -> Result<WasmUri>
where
    B: AsRef<[u8]>,
    W: WasmFile<B>,
{
    // collect and register handlers once
    VmHandlerApi::collect_and_register_once();
    let (wasm_uri, bytes) = wasm_file.into_parts()?;

    #[cfg(debug_assertions)]
    println!("compiling module, wasm_uri={}...", wasm_uri);
    let store: Store = Store::new(&Universal::new(Cranelift::default()).engine());
    let mut module = wasmer::Module::new(&store, bytes)?;
    module.set_name(wasm_uri.as_str());
    if let Some(cf) = check_module {
        cf(&module)?;
    };
    for function in module.exports().functions() {
        let name = function.name();
        if name == WasmHandlerApi::onload_symbol() {
            let ty = function.ty();
            if ty.params().len() > 0 || ty.results().len() > 0 {
                return CodeMsg::result(
                    CODE_EXPORTS,
                    format!(
                        "Incompatible Export Type: fn {}(){{}}",
                        WasmHandlerApi::onload_symbol()
                    ),
                );
            }
            continue;
        }
        WasmHandlerApi::symbol_to_method(name).map_or_else(
            || {
                #[cfg(debug_assertions)]
                {
                    println!("module exports non-wasmy function: {:?}", function);
                    Ok(())
                }
            },
            |_method| {
                let ty = function.ty();
                if ty.results().len() == 0 && ty.params().eq(&[Type::I32, Type::I32]) {
                    #[cfg(debug_assertions)]
                    println!("module exports wasmy function: {:?}", function);
                    Ok(())
                } else {
                    return CodeMsg::result(
                        CODE_EXPORTS,
                        format!("Incompatible Export Type: {:?}", function),
                    );
                }
            },
        )?;
    }

    MODULES.write().unwrap().insert(
        wasm_uri.clone(),
        Module {
            module,
            fn_build_import_object: build_import_object.unwrap_or(default_import_object),
        },
    );

    #[cfg(debug_assertions)]
    println!("loaded module, wasm_uri={}", wasm_uri);

    Ok(wasm_uri.clone())
}

fn default_import_object(module: &wasmer::Module, key: &LocalInstanceKey) -> Result<ImportObject> {
    let mut builder: WasiStateBuilder = WasiState::new(key.wasm_uri());
    return Ok(builder
        // First, we create the `WasiEnv` with the stdio pipes
        .finalize()?
        // Then, we get the import object related to our WASI
        // and attach it to the Wasm instance.
        .import_object(module)?);
}
