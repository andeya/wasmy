use rand::random;
use std::env;
use wasmy_abi::*;
use wasmy_abi::test::*;

#[no_mangle]
fn custom_0() {}

#[wasm_handle(0)]
fn multiply(ctx: WasmCtx, args: TestArgs) -> Result<TestRets> {
    println!("env::args: {:?}", env::args());
    for x in env::vars() {
        println!("ENV: {:?}", x);
    }
    let rid = random::<u8>() as i32;
    println!("[Wasm-Simple({})] handle wasm method({}) ctx={:?}, args={{{:?}}}", rid, 0, ctx, args);

    let mut vm_args = TestArgs::new();
    vm_args.a = rid;
    vm_args.b = rid;
    let vm_rets: TestRets = ctx.call_vm(0, vm_args)?;
    println!("[Wasm-Simple({})] call vm method({}): args={{{:?}}}, rets={}", rid, 0, vm_rets, vm_rets.get_c());

    let mut rets = TestRets::new();
    rets.set_c(args.a * args.b);
    Ok(rets)
}
