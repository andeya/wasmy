use std::fmt::{Display, Formatter};
use std::ops::Deref;
use std::path::PathBuf;

use wasmer::WasmerEnv;

#[derive(Clone, WasmerEnv, Hash, Eq, PartialEq)]
pub struct WasmURI(String);

impl Deref for WasmURI {
    type Target = String;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for WasmURI {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl AsRef<str> for WasmURI {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl From<String> for WasmURI {
    fn from(s: String) -> Self {
        WasmURI(s)
    }
}

pub trait WasmInfo<B: AsRef<[u8]>> {
    fn wasm_uri(&self) -> WasmURI;
    fn into_wasm_bytes(self) -> anyhow::Result<B>;
}

impl WasmInfo<Vec<u8>> for PathBuf {
    fn wasm_uri(&self) -> WasmURI {
        WasmURI(if let Ok(p) = self.canonicalize() {
            p.to_string_lossy().to_string()
        } else {
            self.to_string_lossy().to_string()
        })
    }

    fn into_wasm_bytes(self) -> anyhow::Result<Vec<u8>> {
        Ok(std::fs::read(&self)?)
    }
}

impl<B: AsRef<[u8]>> WasmInfo<B> for (&str, B) {
    fn wasm_uri(&self) -> WasmURI {
        WasmURI(self.0.to_string())
    }
    fn into_wasm_bytes(self) -> anyhow::Result<B> {
        Ok(self.1)
    }
}

