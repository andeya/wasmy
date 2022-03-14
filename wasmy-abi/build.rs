extern crate protoc_rust;

use protoc_rust::Customize;

fn main() {
    protoc_rust::Codegen::new()
        .out_dir("./src/")
        .include("./")
        .inputs(&["./abi.proto", "./test.proto"])
        .customize(Customize {
            carllerche_bytes_for_bytes: Some(true),
            serde_derive: Some(true),
            ..Default::default()
        })
        .run()
        .unwrap_or_else(|e| eprintln!("wasmy-abi build.sh error: {}", e));
}
