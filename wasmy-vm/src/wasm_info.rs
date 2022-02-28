use std::path::PathBuf;

pub trait WasmInfo<B: AsRef<[u8]>> {
    fn wasm_uri(&self) -> String;
    fn into_wasm_bytes(self) -> anyhow::Result<B>;
}

impl WasmInfo<Vec<u8>> for PathBuf {
    fn wasm_uri(&self) -> String {
        if let Ok(p) = self.canonicalize() {
            p.to_string_lossy().to_string()
        } else {
            self.to_string_lossy().to_string()
        }
    }

    fn into_wasm_bytes(self) -> anyhow::Result<Vec<u8>> {
        Ok(std::fs::read(&self)?)
    }
}

impl<B: AsRef<[u8]>> WasmInfo<B> for (&str, B) {
    fn wasm_uri(&self) -> String {
        self.0.to_string()
    }
    fn into_wasm_bytes(self) -> anyhow::Result<B> {
        Ok(self.1)
    }
}

