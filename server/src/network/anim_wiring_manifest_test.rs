//! plan-skill-av-relink-v1 P3 —— P1 接线 anim_id 共享清单单向同步测试（动画链）。
//!
//! 清单文件 `client/src/test/resources/bong/anim_wiring_manifest.json` 是
//! `vfx_animation_trigger::P1_WIRED_ANIM_IDS`（P1 全部 7 条新接线的唯一真相源）
//! 的单向导出（BTreeSet 排序 + pretty-print + 尾随换行，稳定 diff），双端消费：
//! - server（本文件）：经 `CARGO_MANIFEST_DIR/../client` 路径读盘，断言清单与常量表
//!   完全一致——无缺失、无多余、无字节级格式手改；
//! - client：`AnimWiringManifestTest` 经 classloader 读同一份清单，逐项断言
//!   `BongAnimationRegistry` 可注册解析 + `player_animation/<id>.json` 资产真实存在。
//!
//! 清单**不可手改**——唯一重生成入口：
//! `cd server && BONG_REGEN_ANIM_MANIFEST=1 cargo test anim_wiring_manifest`。

use std::collections::BTreeSet;
use std::path::PathBuf;

use super::vfx_animation_trigger::P1_WIRED_ANIM_IDS;

const REGEN_HINT: &str = "清单由 P1_WIRED_ANIM_IDS 单向生成、禁止手改；重生成：\
    cd server && BONG_REGEN_ANIM_MANIFEST=1 cargo test anim_wiring_manifest";

fn manifest_path() -> PathBuf {
    PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../client/src/test/resources/bong/anim_wiring_manifest.json"
    ))
}

fn wired_anim_ids() -> BTreeSet<String> {
    P1_WIRED_ANIM_IDS
        .iter()
        .map(|id| (*id).to_string())
        .collect()
}

/// 清单的规范字节形态：BTreeSet 排序 + serde_json pretty + 尾随换行。
fn canonical_manifest_content() -> String {
    let mut json = serde_json::to_string_pretty(&wired_anim_ids())
        .expect("anim id list serialization must not fail");
    json.push('\n');
    json
}

#[test]
fn anim_wiring_manifest_matches_wired_ids_exactly() {
    let expected = wired_anim_ids();
    assert_eq!(
        expected.len(),
        P1_WIRED_ANIM_IDS.len(),
        "P1_WIRED_ANIM_IDS 出现重复条目会让清单吞条目——先修常量表再谈清单"
    );

    let path = manifest_path();
    if std::env::var("BONG_REGEN_ANIM_MANIFEST").as_deref() == Ok("1") {
        std::fs::write(&path, canonical_manifest_content())
            .unwrap_or_else(|error| panic!("重生成清单写盘失败（{}）：{error}", path.display()));
    }

    let content = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "anim 接线清单缺失或不可读（{}，错误：{error}）——{REGEN_HINT}",
            path.display()
        )
    });
    let actual: Vec<String> = serde_json::from_str(&content).unwrap_or_else(|error| {
        panic!("anim 接线清单不是合法的 JSON 字符串数组（{error}）——{REGEN_HINT}")
    });
    let actual_set: BTreeSet<String> = actual.iter().cloned().collect();
    assert_eq!(
        actual.len(),
        actual_set.len(),
        "清单含重复条目（疑似手改）：{actual:?}——{REGEN_HINT}"
    );

    let missing: Vec<&String> = expected.difference(&actual_set).collect();
    assert!(
        missing.is_empty(),
        "清单缺少 {} 条接线 anim id（常量表有而清单没有）：{missing:?}——{REGEN_HINT}",
        missing.len()
    );
    let extra: Vec<&String> = actual_set.difference(&expected).collect();
    assert!(
        extra.is_empty(),
        "清单多出 {} 条 anim id（清单有而常量表没有，疑似手改清单或误删接线）：{extra:?}\
         ——{REGEN_HINT}",
        extra.len()
    );

    // 字节级锁格式：排序 / 缩进 / 尾随换行被手改同样判红（防"语义相同的手工整理"
    // 破坏稳定 diff）。
    assert_eq!(
        content,
        canonical_manifest_content(),
        "清单语义与常量表一致但字节格式漂移（排序/缩进/尾随换行被手改）——{REGEN_HINT}"
    );
}

#[test]
fn every_wired_anim_id_is_bong_namespaced_identifier() {
    for id in P1_WIRED_ANIM_IDS {
        let (namespace, path) = id
            .split_once(':')
            .unwrap_or_else(|| panic!("接线 anim id `{id}` 缺少 `namespace:path` 形态的冒号分隔"));
        assert_eq!(
            namespace, "bong",
            "接线 anim id `{id}` 命名空间必须是 `bong`——client \
             `BongAnimations.MOD_ID` / `player_animation/` 资产目录只认该命名空间"
        );
        assert!(
            !path.is_empty(),
            "接线 anim id `{id}` 的 path 为空串——client Identifier 解析会失败"
        );
        // MC Identifier path 合法字符集（Identifier.tryParse 在 client 端静默失败）。
        assert!(
            path.chars()
                .all(|c| matches!(c, 'a'..='z' | '0'..='9' | '_' | '.' | '-' | '/')),
            "接线 anim id `{id}` 的 path `{path}` 含 MC Identifier 非法字符"
        );
    }
}
