use std::convert::Infallible;
use std::fmt::Formatter;
use std::marker::PhantomData;
use std::mem;
use std::ops::FromResidual;

pub use protobuf::{CodedOutputStream, Message, ProtobufEnum};
pub use protobuf::well_known_types::Any;

use crate::abi::*;

pub type Method = i32;
pub type WasmMethod = Method;
pub type VmMethod = Method;
pub type CtxId = i32;
pub type Result<T> = std::result::Result<T, CodeMsg>;

#[derive(Debug, Clone)]
pub struct WasmCtx<C: Message = Empty> {
    pub(crate) size: usize,
    pub(crate) _priv: PhantomData<C>,
}

#[derive(Debug, Clone)]
pub struct CodeMsg {
    pub code: i32,
    pub msg: String,
}

impl CodeMsg {
    pub fn new<S: ToString>(code: i32, msg: S) -> CodeMsg {
        CodeMsg { code, msg: msg.to_string() }
    }
    #[inline]
    pub fn result<T, S: ToString>(code: i32, msg: S) -> Result<T> {
        Err(Self::new(code, msg))
    }
    pub fn into_result<T>(self) -> Result<T> {
        Err(self)
    }
}

pub const ERR_CODE_UNKNOWN: i32 = -1;
pub const ERR_CODE_PROTO: i32 = -2;
pub const ERR_CODE_NONE: i32 = -3;
pub const ERR_CODE_MEM: i32 = -4;

impl std::fmt::Display for CodeMsg {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "code={}, msg={})", self.code, self.msg)
    }
}

impl From<anyhow::Error> for CodeMsg {
    fn from(e: anyhow::Error) -> Self {
        if e.is::<CodeMsg>() {
            e.downcast().unwrap()
        } else {
            CodeMsg::new(ERR_CODE_UNKNOWN, e)
        }
    }
}

impl From<CodeMsg> for anyhow::Error {
    fn from(e: CodeMsg) -> Self {
        anyhow::Error::msg(e)
    }
}

impl From<std::io::Error> for CodeMsg {
    fn from(e: std::io::Error) -> Self {
        CodeMsg::new(ERR_CODE_UNKNOWN, format!("io: {}", e))
    }
}

impl From<protobuf::ProtobufError> for CodeMsg {
    fn from(e: protobuf::ProtobufError) -> Self {
        CodeMsg::new(ERR_CODE_PROTO, format!("protobuf: {}", e))
    }
}

impl<R: Message> From<OutRets> for Result<R> {
    fn from(out_rets: OutRets) -> Self {
        if out_rets.get_code() != 0 {
            return CodeMsg::result(out_rets.get_code(), out_rets.get_msg());
        }
        out_rets.get_data()
                .unpack::<R>()?
            .map_or_else(
                || CodeMsg::result(ERR_CODE_PROTO, "protobuf: the message type does not match the out_rets"),
                |data| Ok(data),
            )
    }
}

impl InArgs {
    pub fn try_new<M: Message>(method: Method, data: M) -> Result<InArgs> {
        let mut args = InArgs::new();
        args.set_method(method);
        args.set_data(pack_any(data)?);
        Ok(args)
    }
    pub fn get_args<R: Message>(&self) -> Result<R> {
        self.get_data()
            .unpack::<R>()?
            .map_or_else(
                || CodeMsg::result(ERR_CODE_PROTO, "protobuf: the message type does not match the in_args"),
                |data| Ok(data),
            )
    }
}

pub fn unpack_any<R: Message>(data: &Any) -> Result<R> {
    data.unpack::<R>()?
        .map_or_else(
            || CodeMsg::result(ERR_CODE_PROTO, "protobuf: the message type does not match the data"),
            |r| Ok(r),
        )
}

pub fn pack_any<R: Message>(mut data: R) -> Result<Any> {
    if data.as_any().is::<Any>() {
        Ok(unsafe { mem::take(&mut *(&mut data as *mut dyn core::any::Any as *mut Any)) })
    } else {
        Ok(Any::pack(&data)?)
    }
}

pub fn pack_empty() -> Result<Any> {
    Ok(Any::pack(&Empty::new())?)
}

impl From<CodeMsg> for OutRets {
    fn from(v: CodeMsg) -> Self {
        let mut res = OutRets::new();
        res.set_code(v.code);
        res.set_msg(v.msg);
        res
    }
}

impl From<Any> for OutRets {
    fn from(v: Any) -> Self {
        let mut res = OutRets::new();
        res.set_data(v);
        res
    }
}

impl<R: Message> From<Result<R>> for OutRets {
    fn from(v: Result<R>) -> Self {
        match v {
            Ok(data) => {
                let mut res = OutRets::new();
                match pack_any(data) {
                    Ok(data) => {
                        res.set_data(data);
                    }
                    Err(err) => {
                        res.set_code(ERR_CODE_PROTO);
                        res.set_msg(err.to_string());
                    }
                }
                res
            }
            Err(e) => { e.into() }
        }
    }
}

impl FromResidual<Option<Infallible>> for OutRets {
    fn from_residual(_residual: Option<Infallible>) -> Self {
        CodeMsg::new(ERR_CODE_NONE, "not found").into()
    }
}

impl FromResidual<Result<Infallible>> for OutRets {
    fn from_residual(residual: Result<Infallible>) -> Self {
        match residual {
            Err(e) => e.into(),
            _ => { unreachable!() }
        }
    }
}
