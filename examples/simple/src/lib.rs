use std::env;

use rand::random;

use wasmy_abi::*;
use wasmy_abi::test;

#[wasm_entry]
fn main(ctx: Ctx, args: InArgs) -> Result<Any> {
    let rid: i32 = random();
    println!("[Simple({})] env={:?}", rid, env::args().collect::<Vec<String>>());
    println!("[Simple({})] ctx={:?}, args={{{:?}}}", rid, ctx, args);

    match args.get_method() {
        0 => {
            let args: test::TestArgs = args.get_args()?;
            let res: test::TestResult = ctx.call_host(0, &args)?;
            println!("[Simple({})] call host method({}): args={{{:?}}}, result={}", rid, 0, args, res.get_c());
            pack_any(res)
        }
        _ => { pack_empty() }
    }
}
