//! plan-skill-av-relink-v1 P3 —— technique 图标快照单向同步 + 映射约束测试（图标链）。
//!
//! 快照文件 `client/src/test/resources/bong/technique_icon_snapshot.json` 是
//! `TECHNIQUE_DEFINITIONS` 的 `skill_id → icon_texture` 单向导出（BTreeMap 排序 +
//! pretty-print，稳定 diff），双端消费：
//! - server（本文件）：经 `CARGO_MANIFEST_DIR/../client` 路径读盘，断言快照与定义表
//!   完全一致——无缺失、无多余、无漂移、无字节级格式手改；
//! - client：`SkillIconSnapshotAssetTest` 经 classloader 读同一份快照做 classpath
//!   资产存在性扫描 + 缺资产 allowlist 棘轮。
//!
//! 快照**不可手改**——唯一重生成入口：
//! `cd server && BONG_REGEN_ICON_SNAPSHOT=1 cargo test technique_icon_snapshot`。

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use super::known_techniques::TECHNIQUE_DEFINITIONS;

const REGEN_HINT: &str = "快照由 TECHNIQUE_DEFINITIONS 单向生成、禁止手改；重生成：\
    cd server && BONG_REGEN_ICON_SNAPSHOT=1 cargo test technique_icon_snapshot";

/// 图标允许的命名空间集合（plan-skill-av-relink-v1 P0 决议：skill_scroll 单一真相源
/// 在 `bong-client:`，既有 HUD 特化例外留 `bong:`；此外一律判红）。
const ALLOWED_NAMESPACES: [&str; 2] = ["bong", "bong-client"];

/// 重复映射白名单（两招显式共用同一图标文件才登记，当前空集——出现共用必须在此
/// 说明理由，否则视为复制粘贴事故判红）。client 侧 `SkillIconSnapshotAssetTest`
/// 持有同一份空集白名单。
const DUPLICATE_TEXTURE_ALLOWLIST: [&str; 0] = [];

/// P0 例外映射表（plan §8.1 #1）：既有专属图不重链，逐条写死期望文件名——例外集
/// 只许缩小，新增例外必须在此登记并说明理由（与 `known_techniques.rs` 模块注释
/// 的例外清单保持同步）。
const ICON_TEXTURE_EXCEPTIONS: [(&str, &str); 12] = [
    // woliu 基础六式 —— 既有 HUD 特化图保留在 bong:textures/gui/skill/（P0 #3 约定）。
    ("woliu.vortex", "bong:textures/gui/skill/woliu_vortex.png"),
    ("woliu.hold", "bong:textures/gui/skill/woliu_hold.png"),
    ("woliu.burst", "bong:textures/gui/skill/woliu_burst.png"),
    ("woliu.mouth", "bong:textures/gui/skill/woliu_mouth.png"),
    ("woliu.pull", "bong:textures/gui/skill/woliu_pull.png"),
    ("woliu.heart", "bong:textures/gui/skill/woliu_heart.png"),
    // body.guangbo_ticao —— 同上保留 bong: HUD 特化目录。
    (
        "body.guangbo_ticao",
        "bong:textures/gui/skill/body_guangbo_ticao.png",
    ),
    // zhenmai 五式 —— 既有专属图保留在 bong-client:textures/gui/skill/。
    (
        "zhenmai.parry",
        "bong-client:textures/gui/skill/zhenmai_parry.png",
    ),
    (
        "zhenmai.neutralize",
        "bong-client:textures/gui/skill/zhenmai_neutralize.png",
    ),
    (
        "zhenmai.multipoint",
        "bong-client:textures/gui/skill/zhenmai_multipoint.png",
    ),
    (
        "zhenmai.harden",
        "bong-client:textures/gui/skill/zhenmai_harden.png",
    ),
    (
        "zhenmai.sever_chain",
        "bong-client:textures/gui/skill/zhenmai_sever_chain.png",
    ),
    // morph.yixing 已于 P2（2026-07-18）生成 skill_scroll_morph_yixing.png 并
    // 收编规范路径，例外条目移除、client 侧 MISSING_ICON_ALLOWLIST 同步清零。
];

/// 非例外 technique 的规范图标路径（plan §8.1 #1 单一真相源约定）：
/// `bong-client:textures/gui/items/skill_scroll_<safe_id>.png`，`safe_id` =
/// 技能 id 的 `.`/`:`/`/` → `_`（与 client `SkillIconIds.safeSkillIconId` 镜像，
/// 两端改动必须同步）。
fn canonical_icon_texture(skill_id: &str) -> String {
    let safe_id: String = skill_id
        .chars()
        .map(|c| if matches!(c, '.' | ':' | '/') { '_' } else { c })
        .collect();
    format!("bong-client:textures/gui/items/skill_scroll_{safe_id}.png")
}

fn snapshot_path() -> PathBuf {
    PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../client/src/test/resources/bong/technique_icon_snapshot.json"
    ))
}

fn definitions_icon_map() -> BTreeMap<String, String> {
    TECHNIQUE_DEFINITIONS
        .iter()
        .map(|def| (def.id.to_string(), def.icon_texture.to_string()))
        .collect()
}

/// 快照的规范字节形态：BTreeMap 键序 + serde_json pretty + 尾随换行。
fn canonical_snapshot_content() -> String {
    let mut json = serde_json::to_string_pretty(&definitions_icon_map())
        .expect("icon map serialization must not fail");
    json.push('\n');
    json
}

#[test]
fn technique_icon_snapshot_matches_definitions_exactly() {
    let expected = definitions_icon_map();
    assert_eq!(
        expected.len(),
        TECHNIQUE_DEFINITIONS.len(),
        "TECHNIQUE_DEFINITIONS 出现重复 id 会让快照吞条目——先修定义表再谈快照"
    );

    let path = snapshot_path();
    if std::env::var("BONG_REGEN_ICON_SNAPSHOT").as_deref() == Ok("1") {
        std::fs::write(&path, canonical_snapshot_content())
            .unwrap_or_else(|error| panic!("重生成快照写盘失败（{}）：{error}", path.display()));
    }

    let content = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "图标快照文件缺失或不可读（{}，错误：{error}）——{REGEN_HINT}",
            path.display()
        )
    });
    let actual: BTreeMap<String, String> = serde_json::from_str(&content).unwrap_or_else(|error| {
        panic!("图标快照不是合法的 {{skill_id: icon_texture}} JSON 对象（{error}）——{REGEN_HINT}")
    });

    // 三类差异分别点名条目，撞红时不需要肉眼 diff 大 JSON。
    let missing: Vec<&String> = expected
        .keys()
        .filter(|id| !actual.contains_key(*id))
        .collect();
    assert!(
        missing.is_empty(),
        "快照缺少 {} 条 technique（定义表有而快照没有）：{missing:?}——{REGEN_HINT}",
        missing.len()
    );
    let extra: Vec<&String> = actual
        .keys()
        .filter(|id| !expected.contains_key(*id))
        .collect();
    assert!(
        extra.is_empty(),
        "快照多出 {} 条 technique（快照有而定义表没有，疑似手改快照或误删定义）：{extra:?}——{REGEN_HINT}",
        extra.len()
    );
    let drifted: Vec<String> = expected
        .iter()
        .filter(|(id, icon)| actual.get(*id) != Some(icon))
        .map(|(id, icon)| {
            format!(
                "{id}: 定义表={icon} 快照={}",
                actual.get(id).map(String::as_str).unwrap_or("<无>")
            )
        })
        .collect();
    assert!(
        drifted.is_empty(),
        "快照 icon_texture 与定义表漂移 {} 条：{drifted:#?}——{REGEN_HINT}",
        drifted.len()
    );

    // 字节级锁格式：键序 / 缩进 / 尾随换行被手改同样判红（防"语义相同的手工整理"
    // 破坏稳定 diff）。
    assert_eq!(
        content,
        canonical_snapshot_content(),
        "快照语义与定义表一致但字节格式漂移（键序/缩进/尾随换行被手改）——{REGEN_HINT}"
    );
}

#[test]
fn every_technique_icon_texture_is_non_empty() {
    for def in &TECHNIQUE_DEFINITIONS {
        assert!(
            !def.icon_texture.is_empty(),
            "technique `{}` 的 icon_texture 为空串——Skill 槽发射契约要求每条 technique \
             都有可下发的图标路径（A/V 差异化红线）",
            def.id
        );
    }
}

#[test]
fn every_technique_icon_texture_uses_allowed_namespace_and_png_path() {
    for def in &TECHNIQUE_DEFINITIONS {
        let (namespace, path) = def.icon_texture.split_once(':').unwrap_or_else(|| {
            panic!(
                "technique `{}` 的 icon_texture `{}` 缺少 `namespace:path` 形态的冒号分隔",
                def.id, def.icon_texture
            )
        });
        assert!(
            ALLOWED_NAMESPACES.contains(&namespace),
            "technique `{}` 的图标命名空间 `{namespace}` 不在允许集 {ALLOWED_NAMESPACES:?} 内\
             （icon_texture={}）——P0 决议只认 bong / bong-client 两个资源根",
            def.id,
            def.icon_texture
        );
        assert!(
            path.starts_with("textures/gui/"),
            "technique `{}` 的图标路径 `{path}` 未落在 textures/gui/ 下（icon_texture={}）",
            def.id,
            def.icon_texture
        );
        assert!(
            path.ends_with(".png"),
            "technique `{}` 的图标 `{}` 不是 .png 资产",
            def.id,
            def.icon_texture
        );
        // MC Identifier 合法字符集：namespace [a-z0-9_.-]，path 额外允许 '/'。
        assert!(
            namespace
                .chars()
                .all(|c| matches!(c, 'a'..='z' | '0'..='9' | '_' | '.' | '-')),
            "technique `{}` 的命名空间 `{namespace}` 含 MC Identifier 非法字符",
            def.id
        );
        assert!(
            path.chars()
                .all(|c| matches!(c, 'a'..='z' | '0'..='9' | '_' | '.' | '-' | '/')),
            "technique `{}` 的图标路径 `{path}` 含 MC Identifier 非法字符\
             （Identifier.tryParse 会在 client 端静默失败）",
            def.id
        );
        // 语义映射校验（plan §8.1 #1）：非例外条目必须严格等于 skill_scroll 规范
        // 路径（safe_id 派生），例外条目必须严格等于映射表写死的期望文件名——防
        // "命名空间/格式全合法但指错文件"的静默漂移。
        let expected_icon = ICON_TEXTURE_EXCEPTIONS
            .iter()
            .find(|(id, _)| *id == def.id)
            .map(|(_, texture)| (*texture).to_string())
            .unwrap_or_else(|| canonical_icon_texture(def.id));
        assert_eq!(
            def.icon_texture, expected_icon,
            "technique `{}` 的 icon_texture 偏离 P0 命名约定：期望 `{expected_icon}`\
             （例外条目以 ICON_TEXTURE_EXCEPTIONS 写死值为准，其余一律\
             skill_scroll_<safe_id> 规范路径），实际 `{}`——重链到规范资产，或\
             （确属新例外时）登记 ICON_TEXTURE_EXCEPTIONS 并说明理由",
            def.id, def.icon_texture
        );
    }

    // 例外映射表防僵尸：每条例外 id 必须仍存在于 TECHNIQUE_DEFINITIONS（技能改名/
    // 删除后条目不得残留，例外集只许缩小）。
    for (id, _) in &ICON_TEXTURE_EXCEPTIONS {
        assert!(
            TECHNIQUE_DEFINITIONS.iter().any(|def| def.id == *id),
            "ICON_TEXTURE_EXCEPTIONS 僵尸条目 `{id}`：TECHNIQUE_DEFINITIONS 里已无\
             此 technique，删掉该例外条目"
        );
    }
}

#[test]
fn no_two_techniques_share_one_icon_texture() {
    let mut by_texture: HashMap<&str, Vec<&str>> = HashMap::new();
    for def in &TECHNIQUE_DEFINITIONS {
        by_texture.entry(def.icon_texture).or_default().push(def.id);
    }
    let duplicated: Vec<String> = by_texture
        .iter()
        .filter(|(texture, ids)| ids.len() > 1 && !DUPLICATE_TEXTURE_ALLOWLIST.contains(*texture))
        .map(|(texture, ids)| format!("{texture} ← {ids:?}"))
        .collect();
    assert!(
        duplicated.is_empty(),
        "以下图标文件被多条 technique 共用（白名单外，玩家无法从 HUD 分辨招式）：\
         {duplicated:#?}——确属刻意共用需登记 DUPLICATE_TEXTURE_ALLOWLIST 并说明理由",
    );
}
