use rand::random;

use wasmy_abi::*;
use wasmy_abi::test::*;

#[wasm_handle(0)]
fn multiply(ctx: Ctx, args: TestArgs) -> Result<TestRets> {
    let rid = random::<u8>() as i32;
    println!("[Wasm-Simple({})] handle wasm method({}) ctx={:?}, args={{{:?}}}", rid, 0, ctx, args);

    let mut vm_args = TestArgs::new();
    vm_args.a = rid;
    vm_args.b = rid;
    let vm_rets: TestRets = ctx.call_vm(0, vm_args)?;
    println!("[Wasm-Simple({})] call vm method({}): args={{{:?}}}, rets={}", rid, 0, vm_rets, vm_rets.get_c());

    let mut res = TestRets::new();
    res.set_c(args.a * args.b);
    Ok(res)
}


// Expanded codes:
//
// fn multiply(ctx: Ctx, args: TestArgs) -> Result<TestRets>
// {
//     let rid = random::<u8>() as i32;
//     println!("[Wasm-Simple({})] handle wasm method({}) ctx={:?}, args={{{:?}}}", rid,
//              0, ctx, args);
//     let mut vm_args = TestArgs::new();
//     vm_args.a = rid;
//     vm_args.b = rid;
//     let vm_rets: TestRets = ctx.call_vm(0, vm_args)?;
//     println!("[Wasm-Simple({})] call vm method({}): args={{{:?}}}, rets={}", rid, 0,
//              vm_rets, vm_rets.get_c());
//     let mut res = TestRets::new();
//     res.set_c(args.a * args.b);
//     Ok(res)
// }
//
// #[allow(redundant_semicolons)]
// #[inline]
// #[no_mangle]
// pub extern "C" fn
// _wasm_handle_0(ctx_id: i32, size: i32)
// {
//     #[inline]
//     fn
//     _inner(ctx: ::wasmy_abi::Ctx, args: ::wasmy_abi::InArgs) -> ::
//     wasmy_abi::Result<::wasmy_abi::Any>
//     { ::wasmy_abi::pack_any(multiply(ctx, args.get_args()?)?) }
//     ;
//     ::
//     wasmy_abi::wasm_handle(ctx_id, size, _inner)
// }
