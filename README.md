# wasmy

wasmy, easily customize my wasm app!

## test

```shell
rustup target add wasm32-wasi

cd examples/simple
cargo +nightly build
cd ../vm
cargo +nightly run -- run ../simple/target/wasm32-wasi/debug/simple.wasm
```

## example

- wasm code (target = "wasm32-wasi")

```rust
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
```

- vm code

```rust
use structopt::{clap::AppSettings, StructOpt};
use wasmy_vm::*;

use crate::test::{TestArgs, TestResult};

#[derive(StructOpt, Debug)]
#[structopt(global_settings = & [AppSettings::VersionlessSubcommands, AppSettings::ColorAuto, AppSettings::ColoredHelp])]
enum Command {
    RUN(WasmInfo),
}

fn main() {
    println!("wasmy, easily customize my wasm app!");
    match Command::from_args() {
        Command::RUN(wasm_info) => {
            load_wasm(wasm_info.clone()).unwrap();
            let mut data = TestArgs::new();
            data.set_a(2);
            data.set_b(5);
            let guest_result: TestResult = call_wasm(wasm_info, 0, data.clone()).unwrap();
            println!("{}+{}={}", data.get_a(), data.get_b(), guest_result.get_c())
        }
    }
}

#[vm_handler(0)]
fn add(args: TestArgs) -> Result<TestResult> {
    let mut res = TestResult::new();
    res.set_c(args.a + args.b);
    Ok(res)
}
```
