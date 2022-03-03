use std::env;
use std::path::PathBuf;

use structopt::StructOpt;
use tokio;

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

    let thread_num = opt
        .thread_num
        .and_then(|c| Some(if c == 0 { 1 } else { c }))
        .unwrap_or(1);
    let number = opt.number
                    .and_then(|c| Some(if c == 0 { 1 } else { c }))
                    .unwrap_or(1);

    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(thread_num)
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            for _ in 1..=thread_num {
                let data = data.clone();
                let wasm_caller = wasm_caller.clone();
                tokio::spawn(async move {
                    for i in 1..=number {
                        let rets: TestRets = wasm_caller.call(0, data.clone()).unwrap();
                        println!("NO.{}: {}+{}={}", i, data.get_a(), data.get_b(), rets.get_c());
                    }
                });
            }
        });
}


#[vm_handle(0)]
fn add(args: TestArgs) -> Result<TestRets> {
    let mut rets = TestRets::new();
    rets.set_c(args.a + args.b);
    Ok(rets)
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
// fn _vm_handle_0(args: &::wasmy_vm::Any) ->
// ::wasmy_vm::Result<::wasmy_vm::Any>
// {
//     add(::wasmy_vm::VmHandlerApi::unpack_any(args)
//         ?).and_then(|res| ::wasmy_vm::VmHandlerApi::pack_any(res))
// } ::wasmy_vm::submit_handler! { :: wasmy_vm :: VmHandlerApi :: new(0i32, _vm_handle_0) }
