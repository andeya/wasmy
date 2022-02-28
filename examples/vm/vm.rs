use std::env;
use std::path::PathBuf;

use structopt::StructOpt;

use wasmy_vm::*;

use crate::test::{TestArgs, TestResult};

/// vm cli flags
#[derive(StructOpt, Debug)]
#[structopt(name = "basic")]
struct Opt {
    #[structopt(short = "p", long = "path_prefix", parse(from_os_str))]
    path_prefix: Option<PathBuf>,
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

    let ins_key = load_wasm(wasm_path).unwrap();
    let mut data = TestArgs::new();
    data.set_a(2);
    data.set_b(5);
    let res: TestResult = call_wasm(ins_key.into_wasm_uri(), 0, data.clone()).unwrap();
    println!("{}+{}={}", data.get_a(), data.get_b(), res.get_c())
}

#[vm_handler(0)]
fn add(args: TestArgs) -> Result<TestResult> {
    let mut res = TestResult::new();
    res.set_c(args.a + args.b);
    Ok(res)
}


// Expanded codes:
//
// fn add(args: TestArgs) -> Result<TestResult>
// {
//     let mut res = TestResult::new();
//     res.set_c(args.a + args.b);
//     Ok(res)
// }
//
// #[allow(redundant_semicolons)]
// fn _vm_handler_0(args: &::wasmy_vm::Any)
//                  -> ::wasmy_vm::Result<::wasmy_vm::Any>
// {
//     add(::wasmy_vm::VmHandlerAPI::unpack_any(args)
//         ?).and_then(|res| ::wasmy_vm::VmHandlerAPI::pack_any(res))
// } ::wasmy_vm::submit_handler! { :: wasmy_vm :: VmHandlerAPI :: new(0i32, _vm_handler_0) }
