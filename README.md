# wasmy [![WasmGroup-QQ42726807](https://img.shields.io/badge/WasmGroup-QQ42726807-27a5ea.svg?style=flat-square)](https://jq.qq.com/?_wv=1027&k=dSmP3goX)

wasmy, easily customize my wasm app!

## features

- [x] Attribute macros implement automatic registration of handlers
- [x] ABI is loose, freely register handlers in vm or wasm
- [x] Completely shield vm-wasm interaction details
- [x] Use protobuf as the interaction protocol

## crates

- [wasmy-vm](https://docs.rs/wasmy-vm/latest/wasmy_vm/index.html) : vm dependencies

```toml
[dependencies]
wasmy-vm = "0.3.2"
```

- [wasmy-abi](https://docs.rs/wasmy-abi/latest/wasmy_abi/index.html) : wasm dependencies

```toml
[dependencies]
wasmy-abi = "0.3.2"
```

- [wasmy-macros](https://docs.rs/wasmy-macros/latest/wasmy_macros/index.html) : no direct dependency

```toml
wasmy-macros = "0.3.2"
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
use wasmy_vm::*;
use crate::test::{TestArgs, TestResult};

// ...

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

## test simple example

- raw cargo cmd:

```shell
rustup target add wasm32-wasi

cargo +nightly build --target=wasm32-wasi --example=simple
cargo +nightly run --example=vm -- ../../wasm32-wasi/debug/examples/simple.wasm
```

- alias cargo cmd:

```shell
rustup target add wasm32-wasi

cargo +nightly wasm simple
cargo +nightly vm simple
```
