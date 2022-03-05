//! detailed vm example
//!
use std::{env, thread};
use std::path::PathBuf;

use structopt::StructOpt;

use wasmy_vm::*;

use crate::test::{TestArgs, TestRets};

/// vm cli flags
#[derive(StructOpt, Debug)]
#[structopt(name = "basic")]
struct Opt {
    #[structopt(short = "p", long = "path_prefix", parse(from_os_str))]
    path_prefix: Option<PathBuf>,
    #[structopt(short = "t", long = "thread")]
    thread_num: Option<usize>,
    /// Number of executions per thread
    #[structopt(short = "n", long = "number")]
    number: Option<usize>,
    #[structopt(parse(from_os_str))]
    wasm_path: PathBuf,
}

fn main() {
    link_mod();
    println!("wasmy, easily customize my wasm app!");
    let mut opt: Opt = Opt::from_args();
    if let Some(p) = opt.path_prefix {
        opt.wasm_path = p.join(&opt.wasm_path);
    };
    opt.wasm_path.set_extension("wasm");
    let wasm_path = PathBuf::from(env::args().next().unwrap()).parent().unwrap().join(opt.wasm_path);
    println!("wasm file path: {:?}", wasm_path);

    let wasm_caller = load_wasm(wasm_path).unwrap();
    let mut data = TestArgs::new();
    data.set_a(2);
    data.set_b(5);

    fn call(count: usize, data: TestArgs, wasm_caller: WasmCaller) {
        for i in 1..=count {
            let rets: TestRets = wasm_caller.call(0, data.clone()).unwrap();
            println!("NO.{}: {}+{}={}", i, data.get_a(), data.get_b(), rets.get_c());
        }
    }

    let mut hdls = vec![];
    let number = opt.number
                    .and_then(|c| Some(if c == 0 { 1 } else { c }))
                    .unwrap_or(1);

    for _ in 1..=opt
        .thread_num
        .and_then(|c| Some(if c == 0 { 1 } else { c }))
        .unwrap_or(1)
    {
        let data = data.clone();
        let wasm_caller = wasm_caller.clone();
        hdls.push(thread::spawn(move || {
            call(number, data, wasm_caller)
        }))
    }
    for h in hdls {
        let _ = h.join();
    }
}

// Make sure the mod is linked
fn link_mod() {
    #[vm_handle(0)]
    fn add(args: TestArgs) -> Result<TestRets> {
        let mut rets = TestRets::new();
        rets.set_c(args.a + args.b);
        Ok(rets)
    }
    // more #[vm_handle(i32)] fn ...
}

// Expanded codes:
//
// fn add(args: TestArgs) -> Result<TestRets>
// {
//     let mut rets = TestRets::new();
//     rets.set_c(args.a + args.b);
//     Ok(rets)
// }
//
// #[allow(redundant_semicolons)]
// fn
// _vm_handle_0(_ctx_ptr: usize, args: &::wasmy_vm::Any) -> ::wasmy_vm::
// Result<::wasmy_vm::Any>
// {
//     add(::wasmy_vm::VmHandlerApi::unpack_any(args)
//         ?).and_then(|res| ::wasmy_vm::VmHandlerApi::pack_any(res))
// } ::wasmy_vm::submit_handler! { :: wasmy_vm :: VmHandlerApi :: new(0i32, _vm_handle_0) }
