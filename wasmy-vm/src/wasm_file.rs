use std::{
    collections::HashMap,
    fmt::{Display, Formatter},
    ops::Deref,
    path::PathBuf,
    sync::{RwLock, RwLockReadGuard},
};

use lazy_static::lazy_static;
use wasmer::{CompileError, WasmError};

#[derive(Clone, Hash, Eq, PartialEq, Debug)]
pub struct WasmUri(String);

impl Deref for WasmUri {
    type Target = String;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for WasmUri {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl AsRef<str> for WasmUri {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl From<String> for WasmUri {
    fn from(s: String) -> Self {
        WasmUri(s)
    }
}

pub trait WasmFile<B: AsRef<[u8]>> {
    fn into_parts(self) -> anyhow::Result<(WasmUri, B)>;
}

impl WasmFile<Vec<u8>> for PathBuf {
    fn into_parts(self) -> anyhow::Result<(WasmUri, Vec<u8>)> {
        Ok((
            WasmUri(if let Ok(p) = self.canonicalize() {
                p.to_string_lossy().to_string()
            } else {
                self.to_string_lossy().to_string()
            }),
            std::fs::read(&self)?,
        ))
    }
}

impl<B: AsRef<[u8]>> WasmFile<B> for (&str, B) {
    fn into_parts(self) -> anyhow::Result<(WasmUri, B)> {
        Ok((WasmUri(self.0.to_string()), self.1))
    }
}

lazy_static! {
    static ref GLOBAL_FILES: RwLock<HashMap<WasmUri, Vec<u8>>> = RwLock::new(HashMap::new());
}

pub fn register_file<F: WasmFile<B>, B: AsRef<[u8]>>(file: F) -> anyhow::Result<WasmUri> {
    let (uri, bytes) = file.into_parts()?;
    let bytes = wat::parse_bytes(bytes.as_ref()).map_err(|e| {
        CompileError::Wasm(WasmError::Generic(format!("Error when converting wat: {}", e)))
    })?;
    GLOBAL_FILES.write().unwrap().insert(uri.clone(), bytes.to_vec());
    Ok(uri)
}

pub fn get_files() -> RwLockReadGuard<'static, HashMap<WasmUri, Vec<u8>>> {
    GLOBAL_FILES.read().unwrap()
}
