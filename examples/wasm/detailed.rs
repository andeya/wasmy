#![feature(thread_id_value)]

use std::thread;

use rand::random;

use wasmy_abi::*;
use wasmy_abi::test::*;

static mut STATE: u64 = 0;

/// initialization
#[wasm_onload]
fn init() {
    unsafe {
        STATE = thread::current().id().as_u64().get();
        println!("[Wasm-Simple] initialized STATE to thread id: {}", STATE);
    }
}

#[wasm_handle(method = 0)]
fn multiply(ctx: WasmCtx<TestCtxValue>, args: TestArgs) -> Result<TestRets> {
    unsafe { STATE += 1; }
    let rid = random::<u8>() as i32;
    match ctx.try_value() {
        Ok(value) => unsafe {
            println!("[Wasm-Simple({})] STATE={}, method({}) ctx={:?}, ctx_value={:?}, args={{{:?}}}", rid, STATE, 0, ctx, value, args);
        },
        Err(err) => match err.code {
            CODE_NONE => unsafe {
                println!("[Wasm-Simple({})] STATE={}, method({}) ctx={:?}, args={{{:?}}}", rid, STATE, 0, ctx, args);
            }
            _ => { return err.into_result(); }
        },
    }

    let mut vm_args = TestArgs::new();
    vm_args.a = rid;
    vm_args.b = rid;
    let vm_rets: TestRets = ctx.call_vm(0, vm_args)?;
    println!("[Wasm-Simple({})] call vm method({}): args={{{:?}}}, rets={}", rid, 0, vm_rets, vm_rets.get_c());

    let mut rets = TestRets::new();
    rets.set_c(args.a * args.b);
    Ok(rets)
}


// Expanded codes:
//
// #[allow(redundant_semicolons)]
// #[inline]
// #[no_mangle]
// pub extern "C" fn
// _wasmy_wasm_onload()
// {
//     /// initialization
//     fn init()
//     {
//         unsafe
//             {
//                 STATE = thread::current().id().as_u64().get();
//                 println!("[Wasm-Simple] initialized STATE to thread id: {}", STATE);
//             }
//     }
//     ;
//     init();
// }
//
// fn multiply(ctx: WasmCtx<TestCtxValue>, args: TestArgs) -> Result<
//     TestRets>
// {
//     unsafe { STATE += 1; }
//     let rid = random::<u8>() as i32;
//     match
//     ctx.try_value()
//     {
//         Ok(value) => unsafe
//             {
//                 println!("[Wasm-Simple({})] STATE={}, method({}) ctx={:?}, ctx_value={:?}, args={{{:?}}}",
//                          rid, STATE, 0, ctx, value, args);
//             },
//         Err(err) => match err.code
//         {
//             CODE_NONE => unsafe
//                 {
//                     println!("[Wasm-Simple({})] STATE={}, method({}) ctx={:?}, args={{{:?}}}",
//                              rid, STATE, 0, ctx, args);
//                 }
//             _ => { return err.into_result(); }
//         },
//     }
//     let mut vm_args = TestArgs::new();
//     vm_args.a = rid;
//     vm_args.b = rid
//     ;
//     let vm_rets: TestRets = ctx.call_vm(0, vm_args)?;
//     println!("[Wasm-Simple({})] call vm method({}): args={{{:?}}}, rets={}", rid, 0,
//              vm_rets, vm_rets.get_c());
//     let mut rets = TestRets::new();
//     rets.set_c(args.a * args.b);
//     Ok(rets)
// }
//
// #[allow(redundant_semicolons)]
// #[inline]
// #[no_mangle]
// pub extern "C" fn
// _wasmy_wasm_handle_0(ctx_size: i32, args_size: i32)
// {
//     #[allow(unused_mut)]
//     #[inline]
//     fn
//     _inner(ctx: WasmCtx<TestCtxValue>, args: ::wasmy_abi::InArgs) ->
//     ::wasmy_abi::Result<::wasmy_abi::Any>
//     { ::wasmy_abi::pack_any(multiply(ctx, args.get_args()?)?) }
//     ;
//     ::
//     wasmy_abi::wasm_handle(ctx_size, args_size, _inner)
// }

