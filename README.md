# wasmy [![WasmGroup-QQ42726807](https://img.shields.io/badge/WasmGroup-QQ42726807-27a5ea.svg?style=flat-square)](https://jq.qq.com/?_wv=1027&k=dSmP3goX)

wasmy, easily customize my wasm app!

## features

- [x] Completely shield vm-wasm interaction details
- [x] Use protobuf as the interaction protocol
- [x] Simple and flexible ABI, supports freely adding vm and wasm handlers using attribute macros (`#[vm_handle(0)]`
  /`#[wasm_handle(0)]`)
- [x] Provide attribute macro `#[wasm_onload]` support to initialize wasm

## crates

- [wasmy-vm](https://docs.rs/wasmy-vm/latest/wasmy_vm/index.html) : vm dependencies

```toml
[dependencies]
wasmy-vm = "0.4.1"
```

- [wasmy-abi](https://docs.rs/wasmy-abi/latest/wasmy_abi/index.html) : wasm dependencies

```toml
[dependencies]
wasmy-abi = "0.4.1"
```

- [wasmy-macros](https://docs.rs/wasmy-macros/latest/wasmy_macros/index.html) : no direct dependency

```toml
wasmy-macros = "0.4.1"
```

## example

- wasm code (target = "wasm32-wasi")

```rust
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
```

- vm code

```rust
use wasmy_vm::*;
use crate::test::{TestArgs, TestRets};

...

fn main() {
    println!("wasmy, easily customize my wasm app!");
    ...
    let wasm_caller = load_wasm(wasm_path).unwrap();
    let mut data = TestArgs::new();
    data.set_a(2);
    data.set_b(5);
    for i in 1..=3 {
        let res: TestRets = wasm_caller.call(0, data.clone()).unwrap();
        println!("NO.{}: {}+{}={}", i, data.get_a(), data.get_b(), res.get_c())
    }
}

#[vm_handle(0)]
fn add(args: TestArgs) -> Result<TestRets> {
  let mut rets = TestRets::new();
  rets.set_c(args.a + args.b);
  Ok(rets)
}
```

## test simple example

- raw cargo cmd:

```shell
$ rustup target add wasm32-wasi

$ cargo +nightly build --target=wasm32-wasi --example=simple
$ cargo +nightly run --example=vm -- ../../wasm32-wasi/debug/examples/simple.wasm
```

- alias cargo cmd:

```shell
$ rustup target add wasm32-wasi

$ cargo +nightly wasm simple
$ cargo +nightly vm simple
```
