//! simple vm example

use wasmy_vm::*;

use crate::{
    test::{TestArgs, TestCtxValue, TestRets},
    vm::{run, Mode},
};

mod vm;

fn main() {
    link_mod();
    run(
        Mode::THREAD,
        |wasm_uri| -> WasmCaller {
            custom_load_wasm(wasm_uri, Some(|module: &Module| -> Result<()>{
                for x in module.exports().functions() {
                    println!("export function: {:?}", x);
                }
                Ok(())
            }), Some(|builder: &mut WasiStateBuilder, store: &mut Store, module: &mut Module, env: &FunctionEnv| -> Result<(WasiFunctionEnv, Imports)>{
                let wasi = builder
                    .arg("-v true")
                    .env("AUTHOR", "andeya")
                    .finalize(store)?;
                let mut imports = wasi.import_object(store, module)?;
                imports.register_namespace("env", import_namespace!({
                    "custom_a" => Function::new_typed_with_env(store, env, |env: FunctionEnvMut, a: i32| {
                        #[cfg(debug_assertions)] println!("[VM:{:?}]custom_a: wasm_uri={}, a={}", env.data().thread_id(), env.data().wasm_uri(), a);
                    }),
                }));
                Ok((wasi, imports))
            })).unwrap()
        },
        |index: usize, wasm_caller: WasmCaller| {
            let mut ctx_value = TestCtxValue::new();
            ctx_value.set_value(env!("CARGO_PKG_VERSION").to_string());
            let mut data = TestArgs::new();
            data.set_a(2);
            data.set_b(5);
            let rets: TestRets = wasm_caller.ctx_call(ctx_value.clone(), 0, data.clone()).unwrap();
            println!("NO.{}: {}+{}={}", index, data.get_a(), data.get_b(), rets.get_c());
            let rets = wasm_caller.raw_call(
                "opposite_sign",
                |ctx| {
                    ctx.set_value_ptr(&ctx_value);
                    ctx.value_bytes = ctx_value.write_to_bytes().unwrap();
                    println!("set ctx: {:?}", ctx);
                    Ok(vec![(index as i32).into()].into_boxed_slice())
                },
                |_ctx, rets| Ok(rets[0].unwrap_i32()),
            );
            match rets {
                Ok(r) => println!("NO.{}: -{}={}", index, index, r),
                Err(e) => eprintln!("{}", e),
            }
        },
    );
}

// Make sure the mod is linked
fn link_mod() {
    #[vm_handle(0)]
    fn add(ctx: Option<&TestCtxValue>, args: TestArgs) -> Result<TestRets> {
        println!("[VM] add handler, ctx={:?}", ctx);
        let mut rets = TestRets::new();
        rets.set_c(args.a + args.b);
        Ok(rets)
    }
    // more #[vm_handle(i32)] fn ...
}
