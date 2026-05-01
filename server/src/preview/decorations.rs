//! Preview 装饰加载器（plan-worldgen-snapshot-v1 §2.2-2.4）。
//!
//! 启动期（更准确：第一次 Update tick）读 `worldgen/preview/decorations.json` →
//! 调用 `ChunkLayer::set_block` 摆方块。仅在 `BONG_PREVIEW_MODE=1` 激活；JSON
//! 路径默认 `worldgen/preview/decorations.json`，可被 `BONG_PREVIEW_DECORATIONS`
//! env 覆盖。
//!
//! 支持两类装饰（plan §2.2 第三类 boundary_marker 留 v2 plan，AABB 点阵 spawn
//! 数百 block 影响 chunk 加载）：
//!   - **sign** — 4 行木牌（valence Sign block entity NBT，UTF-8 中文 OK）
//!   - **pillar** — 从 pos 起向 +Y 堆 N 块的方块柱
//!
//! 加载失败（JSON 缺失/解析挂/block name 不识别）打 warn log，不挂 server。

use std::path::PathBuf;

use serde::Deserialize;
use valence::block::PropName;
use valence::nbt::{compound, List};
use valence::prelude::*;
use valence::text::IntoText;

/// 相对 server cwd（`server/` 目录）的默认装饰 JSON 路径。
/// CI / 本地 `cargo run` 都从 `server/` 起跑，对应 repo 内 worldgen/preview/。
/// 想换路径用 `BONG_PREVIEW_DECORATIONS=/abs/path.json` env override。
const DEFAULT_PATH: &str = "../worldgen/preview/decorations.json";
const MAX_PILLAR_HEIGHT: u32 = 64;
const MAX_SIGN_LINES: usize = 4;

/// JSON top-level 结构。`_doc` / `_kinds` / `_pos_convention` 等下划线前缀字段
/// 是 schema 注释，serde 默认忽略。
#[derive(Debug, Deserialize, Clone)]
pub struct DecorationsConfig {
    #[serde(default)]
    pub items: Vec<DecorationItem>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DecorationItem {
    Sign {
        pos: [i32; 3],
        #[serde(default)]
        block: Option<String>,
        #[serde(default)]
        lines: Vec<String>,
    },
    Pillar {
        pos: [i32; 3],
        #[serde(default)]
        block: Option<String>,
        height: u32,
    },
}

/// 解析 JSON 路径成 config。pure I/O + parsing，不接 ECS。
pub fn load_from_path(path: &PathBuf) -> Result<DecorationsConfig, String> {
    let body =
        std::fs::read_to_string(path).map_err(|e| format!("read {} 失败: {e}", path.display()))?;
    serde_json::from_str(&body).map_err(|e| format!("parse {} 失败: {e}", path.display()))
}

/// env-driven 路径解析（默认 worldgen/preview/decorations.json）。
pub fn resolve_path() -> PathBuf {
    std::env::var("BONG_PREVIEW_DECORATIONS")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(DEFAULT_PATH))
}

/// 把 block name 字符串映射到 [`BlockState`]。当前只覆盖装饰常用几种；不识别
/// 时 fallback 到 default（sign → oak_sign / pillar → end_rod）。
fn resolve_block_state(name: &str, fallback: BlockState) -> BlockState {
    match name {
        "minecraft:oak_sign" => BlockState::OAK_SIGN,
        "minecraft:spruce_sign" => BlockState::SPRUCE_SIGN,
        "minecraft:birch_sign" => BlockState::BIRCH_SIGN,
        "minecraft:jungle_sign" => BlockState::JUNGLE_SIGN,
        "minecraft:acacia_sign" => BlockState::ACACIA_SIGN,
        "minecraft:dark_oak_sign" => BlockState::DARK_OAK_SIGN,
        "minecraft:end_rod" => BlockState::END_ROD,
        "minecraft:soul_lantern" => BlockState::SOUL_LANTERN,
        "minecraft:lantern" => BlockState::LANTERN,
        "minecraft:beacon" => BlockState::BEACON,
        _ => {
            tracing::warn!(
                "[bong][preview] decoration block 名不识别: {name}，fallback 到 default"
            );
            fallback
        }
    }
}

/// 在 layer 上 spawn 一个 sign。`lines` 截断 / pad 到 4，UTF-8 中文 OK。
fn spawn_sign(layer: &mut ChunkLayer, pos: [i32; 3], block_name: Option<&str>, lines: &[String]) {
    let block_state = block_name
        .map(|n| resolve_block_state(n, BlockState::OAK_SIGN))
        .unwrap_or(BlockState::OAK_SIGN);
    // valence sign API 要求 messages 恰好 4 项；缺少 pad 空字符串，多余截断。
    // 每项是 JSON-serialized Text（由 String → Text → String impl 自动 round-trip）。
    let mut messages: Vec<String> = Vec::with_capacity(MAX_SIGN_LINES);
    for i in 0..MAX_SIGN_LINES {
        let text_str = lines.get(i).cloned().unwrap_or_default();
        messages.push(text_str.into_text().into());
    }
    let block = Block {
        state: block_state.set(PropName::Rotation, valence::block::PropValue::_8),
        nbt: Some(compound! {
            "front_text" => compound! {
                "messages" => List::String(messages),
            }
        }),
    };
    layer.set_block(pos, block);
}

/// 在 layer 上 spawn 一根 pillar：`pos` 起向上堆 `height` 块（clamped 1..=64）。
fn spawn_pillar(layer: &mut ChunkLayer, pos: [i32; 3], block_name: Option<&str>, height: u32) {
    let height = height.clamp(1, MAX_PILLAR_HEIGHT);
    let block_state = block_name
        .map(|n| resolve_block_state(n, BlockState::END_ROD))
        .unwrap_or(BlockState::END_ROD);
    for dy in 0..height as i32 {
        layer.set_block([pos[0], pos[1] + dy, pos[2]], block_state);
    }
}

/// 把 config 全部装饰摆到 layer 上。返回 (signs spawned, pillars spawned)。
pub fn spawn_all(config: &DecorationsConfig, layer: &mut ChunkLayer) -> (usize, usize) {
    let mut signs = 0;
    let mut pillars = 0;
    for item in &config.items {
        match item {
            DecorationItem::Sign { pos, block, lines } => {
                spawn_sign(layer, *pos, block.as_deref(), lines);
                signs += 1;
            }
            DecorationItem::Pillar { pos, block, height } => {
                spawn_pillar(layer, *pos, block.as_deref(), *height);
                pillars += 1;
            }
        }
    }
    (signs, pillars)
}

/// 启动期（第一次有 ChunkLayer 时）跑一次的 system。Local<bool> 防止重复 spawn。
pub fn spawn_decorations_once_system(mut spawned: Local<bool>, mut layers: Query<&mut ChunkLayer>) {
    if *spawned {
        return;
    }
    let Some(mut layer) = layers.iter_mut().next() else {
        return; // ChunkLayer 还没就绪，下个 tick 再来
    };
    let path = resolve_path();
    let config = match load_from_path(&path) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(
                "[bong][preview] 加载 decorations failed ({e}); 跳过 — preview 仍可用，仅没装饰"
            );
            *spawned = true; // 标记已尝试，避免每 tick 报错
            return;
        }
    };
    let (signs, pillars) = spawn_all(&config, &mut layer);
    *spawned = true;
    tracing::info!(
        "[bong][preview] decorations spawned from {} — signs={signs} pillars={pillars}",
        path.display()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_decorations_with_sign_and_pillar() {
        let json = r#"{
            "items": [
                {"kind": "sign", "pos": [1, 2, 3], "lines": ["a", "b"]},
                {"kind": "pillar", "pos": [4, 5, 6], "height": 10}
            ]
        }"#;
        let config: DecorationsConfig = serde_json::from_str(json).expect("parse");
        assert_eq!(config.items.len(), 2);
        match &config.items[0] {
            DecorationItem::Sign { pos, lines, block } => {
                assert_eq!(*pos, [1, 2, 3]);
                assert_eq!(lines, &vec!["a".to_string(), "b".to_string()]);
                assert!(block.is_none(), "未给 block 应为 None（fallback default）");
            }
            _ => panic!("第 0 项应是 Sign 变体，实际 {:?}", config.items[0]),
        }
        match &config.items[1] {
            DecorationItem::Pillar { pos, height, block } => {
                assert_eq!(*pos, [4, 5, 6]);
                assert_eq!(*height, 10);
                assert!(block.is_none());
            }
            _ => panic!("第 1 项应是 Pillar 变体，实际 {:?}", config.items[1]),
        }
    }

    #[test]
    fn parse_decorations_ignores_underscore_doc_fields() {
        let json = r#"{
            "_doc": "顶层注释",
            "_kinds": {"sign": "..."},
            "items": [{"kind": "sign", "pos": [0, 0, 0], "lines": []}]
        }"#;
        let config: DecorationsConfig = serde_json::from_str(json).expect("parse");
        assert_eq!(
            config.items.len(),
            1,
            "_doc / _kinds 应被忽略不影响 items 解析"
        );
    }

    #[test]
    fn parse_decorations_empty_items_ok() {
        let json = r#"{"items": []}"#;
        let config: DecorationsConfig = serde_json::from_str(json).expect("parse");
        assert!(config.items.is_empty());
    }

    #[test]
    fn parse_decorations_missing_items_default_empty() {
        let json = r#"{}"#;
        let config: DecorationsConfig = serde_json::from_str(json).expect("parse");
        assert!(
            config.items.is_empty(),
            "items 缺失应默认空列表（serde default）"
        );
    }

    #[test]
    fn parse_decorations_unknown_kind_rejects() {
        let json = r#"{"items": [{"kind": "boundary_marker", "pos": [0, 0, 0]}]}"#;
        let result: Result<DecorationsConfig, _> = serde_json::from_str(json);
        assert!(
            result.is_err(),
            "boundary_marker 暂未实装，未知 kind 应拒绝（避免 silently drop）"
        );
    }

    #[test]
    fn parse_decorations_zhongwen_lines() {
        let json = r#"{
            "items": [{"kind": "sign", "pos": [0, 0, 0], "lines": ["初醒原", "灵气 0.3"]}]
        }"#;
        let config: DecorationsConfig = serde_json::from_str(json).expect("parse zh");
        match &config.items[0] {
            DecorationItem::Sign { lines, .. } => {
                assert_eq!(lines[0], "初醒原");
                assert_eq!(lines[1], "灵气 0.3");
            }
            _ => panic!("Sign expected"),
        }
    }

    #[test]
    fn resolve_block_state_known_names() {
        assert_eq!(
            resolve_block_state("minecraft:oak_sign", BlockState::OAK_LOG),
            BlockState::OAK_SIGN
        );
        assert_eq!(
            resolve_block_state("minecraft:end_rod", BlockState::OAK_LOG),
            BlockState::END_ROD
        );
        assert_eq!(
            resolve_block_state("minecraft:soul_lantern", BlockState::OAK_LOG),
            BlockState::SOUL_LANTERN
        );
    }

    #[test]
    fn resolve_block_state_unknown_falls_back() {
        let fallback = BlockState::OAK_SIGN;
        let result = resolve_block_state("minecraft:does_not_exist", fallback);
        assert_eq!(
            result, fallback,
            "未知 block name 应 fallback；实际 spawn 时按调用者指定的 default"
        );
    }

    #[test]
    fn resolve_path_default() {
        // SAFETY: 单测内 manipulate env
        unsafe {
            std::env::remove_var("BONG_PREVIEW_DECORATIONS");
        }
        let path = resolve_path();
        assert_eq!(path, PathBuf::from("../worldgen/preview/decorations.json"));
    }

    #[test]
    fn resolve_path_env_override() {
        unsafe {
            std::env::set_var("BONG_PREVIEW_DECORATIONS", "/tmp/custom.json");
        }
        let path = resolve_path();
        assert_eq!(path, PathBuf::from("/tmp/custom.json"));
        unsafe {
            std::env::remove_var("BONG_PREVIEW_DECORATIONS");
        }
    }
}
