//! RF-28 R6 P4：C2S registry 的跨 Rust / protobuf / TypeBox 对拍。
//!
//! 这里只验证契约集合和 JSON Schema 生成物，不接入 request handler，也不改变
//! 任一冻结域的运行时行为。灵田旧请求已从主线 live enum 移除，故不能为满足旧
//! 文档数量把它们重新加入协议。

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("server must live directly below repository root")
        .to_path_buf()
}

fn rust_client_request_types() -> BTreeSet<String> {
    let source = fs::read_to_string(repository_root().join("server/src/schema/client_request.rs"))
        .expect("Rust ClientRequestV1 source must be readable");
    let body = source
        .split_once("pub enum ClientRequestV1 {")
        .and_then(|(_, rest)| rest.split_once("\n}\n\nimpl ClientRequestV1"))
        .map(|(body, _)| body)
        .expect("ClientRequestV1 enum body must be present");

    body.lines()
        .filter_map(|line| {
            let candidate = line.trim();
            let name = candidate.strip_suffix('{')?.trim();
            if name.is_empty()
                || !name
                    .chars()
                    .next()
                    .is_some_and(|ch| ch.is_ascii_uppercase())
                || name.contains(char::is_whitespace)
            {
                return None;
            }
            Some(to_snake_case(name))
        })
        .collect()
}

fn to_snake_case(name: &str) -> String {
    name.chars()
        .enumerate()
        .flat_map(|(index, ch)| {
            let separator = (index > 0 && ch.is_ascii_uppercase()).then_some('_');
            separator
                .into_iter()
                .chain(std::iter::once(ch.to_ascii_lowercase()))
        })
        .collect()
}

fn protobuf_client_request_types() -> BTreeSet<String> {
    let source = fs::read_to_string(repository_root().join("proto/bong/envelope.proto"))
        .expect("ClientRequestEnvelope protobuf source must be readable");
    let oneof = source
        .split_once("message ClientRequestEnvelope {")
        .and_then(|(_, rest)| rest.split_once("oneof payload {"))
        .and_then(|(_, rest)| rest.split_once("\n  }"))
        .map(|(body, _)| body)
        .expect("ClientRequestEnvelope payload oneof must be present");

    oneof
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let _message = fields.next()?;
            let field = fields.next()?;
            let _equals = fields.next()?;
            let _tag = fields.next()?;
            if !field.ends_with('=') && line.contains('=') {
                Some(field.to_owned())
            } else {
                None
            }
        })
        .collect()
}

fn generated_type_set() -> BTreeSet<String> {
    let path = repository_root().join("agent/packages/schema/generated/client-request-v1.json");
    let generated: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(path).expect("generated client-request schema must be readable"),
    )
    .expect("generated client-request schema must be valid JSON");

    generated["anyOf"]
        .as_array()
        .expect("client-request schema must be an anyOf union")
        .iter()
        .map(|variant| {
            variant["properties"]["type"]["const"]
                .as_str()
                .expect("every C2S variant must pin a literal type")
                .to_owned()
        })
        .collect()
}

#[test]
fn c2s_registry_matches_live_rust_proto_and_generated_type_sets() {
    let rust = rust_client_request_types();
    let protobuf = protobuf_client_request_types();
    let generated = generated_type_set();

    assert_eq!(
        rust.len(),
        102,
        "live C2S registry count changed; review the source diff"
    );
    assert_eq!(
        generated, rust,
        "TypeBox generated C2S union drifted from Rust"
    );

    let rust_only: BTreeSet<_> = rust.difference(&protobuf).cloned().collect();
    assert_eq!(
        rust_only,
        BTreeSet::from([
            "agent_ui_response".to_owned(),
            "block_picker_give".to_owned(),
        ]),
        "only the documented JSON-only C2S paths may lack a protobuf oneof field"
    );
    assert!(
        protobuf.is_subset(&rust),
        "every protobuf C2S field must have a live Rust request variant"
    );
}
