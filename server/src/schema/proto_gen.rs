//! Protobuf 生成代码入口 — prost codegen 输出。
//!
//! `build.rs` 编译 `proto/bong/*.proto`，prost 默认按 proto package 名
//! 输出到 `$OUT_DIR/bong.rs`。此文件通过 `include!` 引入。

/// Proto-generated types for `package bong;`.
pub mod bong {
    include!(concat!(env!("OUT_DIR"), "/bong.rs"));
}
