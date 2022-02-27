use rand::random;
use wasmy_abi::*;
use wasmy_abi::test::*;

#[wasm_handler(0)]
fn multiply(ctx: Ctx, args: TestArgs) -> Result<TestResult> {
    let rid = random::<u8>() as i32;
    println!("[Wasm-Simple({})] handle guest method({}) ctx={:?}, args={{{:?}}}", rid, 0, ctx, args);

    let mut host_args = TestArgs::new();
    host_args.a = rid;
    host_args.b = rid;
    let host_res: TestResult = ctx.call_host(0, &host_args)?;
    println!("[Wasm-Simple({})] call host method({}): args={{{:?}}}, result={}", rid, 0, host_res, host_res.get_c());

    let mut res = TestResult::new();
    res.set_c(args.a * args.b);
    Ok(res)
}
