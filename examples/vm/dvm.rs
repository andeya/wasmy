//! detailed vm example
//!
use wasmy_vm::*;

use crate::test::{TestArgs, TestCtxValue, TestRets};
use crate::vm::{Mode, run};

mod vm;

fn main() {
    link_mod();
    run(
        Mode::TOKIO,
        |wasm_uri| -> WasmCaller {
            load_wasm(wasm_uri).unwrap()
        },
        |index: usize, wasm_caller: WasmCaller| {
            let mut ctx = TestCtxValue::new();
            ctx.set_value(env!("CARGO_PKG_VERSION").to_string());
            let mut data = TestArgs::new();
            data.set_a(2);
            data.set_b(5);
            let rets: TestRets = wasm_caller.ctx_call(ctx, 0, data.clone()).unwrap();
            // let rets: TestRets = wasm_caller.call(0, data.clone()).unwrap();
            println!("NO.{}: {}+{}={}", index, data.get_a(), data.get_b(), rets.get_c());
        })
}

// Make sure the mod is linked
fn link_mod() {
    #[vm_handle(method = 0)]
    fn add(ctx: Option<&TestCtxValue>, args: TestArgs) -> Result<TestRets> {
        println!("[VM] add handler, ctx={:?}", ctx);
        let mut rets = TestRets::new();
        rets.set_c(args.a + args.b);
        Ok(rets)
    }
    // more #[vm_handle(i32)] fn ...
}


// Expanded codes:
//
// fn add(ctx: Option<&TestCtxValue>, args: TestArgs) -> Result<TestRets
// >
// {
//     println!("[VM] add handler, ctx={:?}", ctx);
//     let mut rets = TestRets::
//     new();
//     rets.set_c(args.a + args.b);
//     Ok(rets)
// }
//
// #[allow(redundant_semicolons)]
// fn
// _wasmy_vm_handle_0(ctx_ptr: usize, args: &::wasmy_vm::Any) -> ::wasmy_vm::
// Result<::wasmy_vm::Any>
// {
//     add(unsafe { ::wasmy_vm::VmHandlerApi::try_as(ctx_ptr) }, ::wasmy_vm
//     ::VmHandlerApi::unpack_any(args)
//         ?).and_then(|res| ::wasmy_vm::VmHandlerApi::pack_any(res))
// } ::wasmy_vm::submit_handler! { :: wasmy_vm :: VmHandlerApi :: new(0i32, _wasmy_vm_handle_0) }
