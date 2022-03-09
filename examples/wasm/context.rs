use rand::random;

use wasmy_abi::*;
use wasmy_abi::test::*;

#[derive(Debug)]
struct MyContext {
    size: usize,
}

impl WasmContext<TestCtxValue> for MyContext {
    fn from_size(size: usize) -> Self {
        MyContext { size }
    }

    fn size(&self) -> usize {
        self.size
    }
}

impl MyContext {
    pub fn vm_add(&self, vm_args: TestArgs) -> Result<TestRets> {
        self.call_vm(0, vm_args)
    }
}

#[wasm_handle(method = 0)]
fn multiply(ctx: MyContext, args: TestArgs) -> Result<TestRets> {
    let rid = random::<u8>() as i32;
    match ctx.try_value() {
        Ok(value) => println!("[Wasm-Simple({})] method({}) ctx={:?}, ctx_value={:?}, args={{{:?}}}", rid, 0, ctx, value, args),
        Err(err) => match err.code {
            CODE_NONE => println!("[Wasm-Simple({})] method({}) ctx={:?}, args={{{:?}}}", rid, 0, ctx, args),
            _ => return err.into_result(),
        },
    }

    let mut vm_args = TestArgs::new();
    vm_args.a = rid;
    vm_args.b = rid;
    let vm_rets: TestRets = ctx.vm_add(vm_args)?;
    println!("[Wasm-Simple({})] call vm method({}): args={{{:?}}}, rets={}", rid, 0, vm_rets, vm_rets.get_c());

    let mut rets = TestRets::new();
    rets.set_c(args.a * args.b);
    Ok(rets)
}
