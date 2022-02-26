use structopt::{clap::AppSettings, StructOpt};

use wasmy_vm::*;

use crate::test::{TestArgs, TestResult};

#[derive(StructOpt, Debug)]
#[structopt(global_settings = & [AppSettings::VersionlessSubcommands, AppSettings::ColorAuto, AppSettings::ColoredHelp])]
enum Command {
    RUN(WasmInfo),
}

fn main() {
    HandlerAPI::collect_and_register_all();
    println!("Hello, world!");
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
