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
            }), Some(|module: &Module, key: &LocalInstanceKey| -> Result<ImportObject>{
                let mut builder: WasiStateBuilder = WasiState::new(key.wasm_uri());
                let mut import_object = builder
                    .arg("-v true")
                    .env("AUTHOR", "henrylee2cn")
                    .finalize()?
                    .import_object(module)?;
                import_object.register("env", import_namespace!({
                    "custom_a" => Function::new_native_with_env(module.store(), key.clone(), |key: &LocalInstanceKey, a: i32| {
                        #[cfg(debug_assertions)] println!("[VM:{:?}]custom_a: wasm_uri={}, a={}", key.thread_id(), key.wasm_uri(), a);
                    }),
                }));
                Ok(import_object)
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
            let rets = wasm_caller.raw_call("opposite_sign", &[(index as i32).into()], |ctx| {
                ctx.set_value_ptr(&ctx_value);
                ctx.value_bytes = ctx_value.write_to_bytes().unwrap();
                println!("set ctx: {:?}", ctx);
            });
            match rets {
                Ok(r) => println!("NO.{}: -{}={}", index, index, r[0].unwrap_i32()),
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
