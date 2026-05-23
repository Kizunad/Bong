//! Prost codegen — 编译 `proto/bong/*.proto` 生成 Rust struct/enum。
//!
//! 输出到 `$OUT_DIR/bong.rs`，由 `src/schema/proto_gen.rs` include!() 引入。
//! proto 目录在仓库根 `proto/`（server 和 client 共用）。

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_root = std::path::Path::new("../proto");

    prost_build::Config::new().compile_protos(
        &[
            proto_root.join("bong/common.proto"),
            proto_root.join("bong/envelope.proto"),
        ],
        &[proto_root],
    )?;

    // 当 proto 文件变化时触发重新编译。
    println!("cargo::rerun-if-changed=../proto/bong/common.proto");
    println!("cargo::rerun-if-changed=../proto/bong/envelope.proto");

    Ok(())
}
