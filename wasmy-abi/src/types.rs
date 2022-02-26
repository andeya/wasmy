use std::convert::Infallible;
use std::fmt::Formatter;
use std::mem;
use std::ops::FromResidual;

pub use protobuf::{CodedOutputStream, Message, ProtobufEnum};
pub use protobuf::well_known_types::Any;

use crate::abi::*;

pub type Method = i32;
pub type CtxID = i32;
pub type Result<Data> = std::result::Result<Data, CodeMsg>;

#[derive(Debug, Copy, Clone)]
pub struct Ctx {
    id: CtxID,
}

impl Ctx {
    pub(crate) fn get_id(&self) -> CtxID {
        self.id
    }
    pub(crate) fn from_id(id: CtxID) -> Ctx {
        Ctx { id }
    }
}

#[derive(Debug)]
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
}

pub const ERR_CODE_UNKNOWN: ErrCode = ErrCode(-1);
pub const ERR_CODE_PROTO: ErrCode = ErrCode(-2);
pub const ERR_CODE_NONE: ErrCode = ErrCode(-3);

pub struct ErrCode(i32);

impl ErrCode {
    #[inline]
    pub fn to_code_msg<S: ToString>(&self, msg: S) -> CodeMsg {
        CodeMsg::new(self.0, msg)
    }
    #[inline]
    pub fn to_result<T, S: ToString>(&self, msg: S) -> Result<T> {
        CodeMsg::result(self.0, msg)
    }
}

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
            ERR_CODE_UNKNOWN.to_code_msg(e)
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
        ERR_CODE_UNKNOWN.to_code_msg(format!("io: {}", e))
    }
}

impl From<protobuf::ProtobufError> for CodeMsg {
    fn from(e: protobuf::ProtobufError) -> Self {
        ERR_CODE_PROTO.to_code_msg(format!("protobuf: {}", e))
    }
}

impl<R: Message> From<OutResult> for Result<R> {
    fn from(out_result: OutResult) -> Self {
        if out_result.get_code() != 0 {
            return CodeMsg::result(out_result.get_code(), out_result.get_msg());
        }
        out_result.get_data()
                  .unpack::<R>()?
            .map_or_else(
                || ERR_CODE_PROTO.to_result("protobuf: the message type does not match the out_result"),
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
                || ERR_CODE_PROTO.to_result("protobuf: the message type does not match the in_args"),
                |data| Ok(data),
            )
    }
}

pub fn unpack_any<R: Message>(data: &Any) -> Result<R> {
    data.unpack::<R>()?
        .map_or_else(
            || ERR_CODE_PROTO.to_result("protobuf: the message type does not match the data"),
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

impl From<CodeMsg> for OutResult {
    fn from(v: CodeMsg) -> Self {
        let mut res = OutResult::new();
        res.set_code(v.code);
        res.set_msg(v.msg);
        res
    }
}

impl From<Any> for OutResult {
    fn from(v: Any) -> Self {
        let mut res = OutResult::new();
        res.set_data(v);
        res
    }
}

impl<R: Message> From<Result<R>> for OutResult {
    fn from(v: Result<R>) -> Self {
        match v {
            Ok(data) => {
                let mut res = OutResult::new();
                match pack_any(data) {
                    Ok(data) => {
                        res.set_data(data);
                    }
                    Err(err) => {
                        res.set_code(ERR_CODE_PROTO.0);
                        res.set_msg(err.to_string());
                    }
                }
                res
            }
            Err(e) => { e.into() }
        }
    }
}

impl FromResidual<Option<Infallible>> for OutResult {
    fn from_residual(_residual: Option<Infallible>) -> Self {
        ERR_CODE_NONE.to_code_msg("not found").into()
    }
}

