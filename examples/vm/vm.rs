use std::{env, path::PathBuf, thread};

use structopt::StructOpt;
use wasmy_vm::*;

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

#[allow(dead_code)]
pub enum Mode {
    TOKIO,
    THREAD,
}

pub fn run<L, C>(model: Mode, loader: L, callback: C)
where
    L: Fn(PathBuf) -> WasmCaller,
    C: Fn(usize, WasmCaller) + Sync + Copy + Send + 'static,
{
    println!("wasmy, easily customize my wasm app!");
    let mut opt: Opt = Opt::from_args();
    if let Some(p) = opt.path_prefix {
        opt.wasm_path = p.join(&opt.wasm_path);
    };
    opt.wasm_path.set_extension("wasm");
    let wasm_path =
        PathBuf::from(env::args().next().unwrap()).parent().unwrap().join(opt.wasm_path);
    println!("wasm file path: {:?}", wasm_path);

    let caller = loader(wasm_path);

    let thread_num = opt.thread_num.and_then(|c| Some(if c == 0 { 1 } else { c })).unwrap_or(1);
    let number = opt.number.and_then(|c| Some(if c == 0 { 1 } else { c })).unwrap_or(1);

    match model {
        Mode::TOKIO => {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(thread_num)
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    for i in 0..thread_num {
                        let caller2 = caller.clone();
                        tokio::spawn(async move {
                            for j in 0..number {
                                callback(i * number + j, caller2.clone())
                            }
                        });
                    }
                });
        }
        Mode::THREAD => {
            let mut hdls = vec![];
            let number = opt.number.and_then(|c| Some(if c == 0 { 1 } else { c })).unwrap_or(1);

            for i in 1..=opt.thread_num.and_then(|c| Some(if c == 0 { 1 } else { c })).unwrap_or(1)
            {
                let caller2 = caller.clone();
                hdls.push(thread::spawn(move || {
                    for j in 0..number {
                        callback(i * number + j, caller2.clone())
                    }
                }))
            }
            for h in hdls {
                let _ = h.join();
            }
        }
    };
}
