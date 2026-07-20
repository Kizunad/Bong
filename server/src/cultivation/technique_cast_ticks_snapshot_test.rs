//! plan-skill-anim-fidelity-v1 P0 —— technique cast_ticks 快照单向同步测试（动画时长对拍链）。
//!
//! 快照文件 `client/src/test/resources/bong/technique_cast_ticks_snapshot.json` 是
//! `TECHNIQUE_DEFINITIONS` + yidao 5 招 spec（P4 增补，见 `definitions_cast_map`）
//! 的 `skill_id → cast_ticks` 单向导出（BTreeMap 排序 + pretty-print，稳定 diff），
//! 双端消费：
//! - server（本文件）：经 `CARGO_MANIFEST_DIR/../client` 路径读盘，断言快照与定义表
//!   完全一致——无缺失、无多余、无漂移、无字节级格式手改；
//! - client：`AnimCastTicksAlignmentTest` 经 classloader 读同一份快照做动画时长
//!   对拍（精度标准 #2 三套断言）+ 现状不达标 allowlist 棘轮。
//!
//! 快照**不可手改**——唯一重生成入口：
//! `cd server && BONG_REGEN_CAST_TICKS_SNAPSHOT=1 cargo test technique_cast_ticks_snapshot`。
//! 机制照抄 `technique_icon_snapshot_test.rs`（plan §8.1 #4 决议：梯队一双先例复用）。

use std::collections::BTreeMap;
use std::path::PathBuf;

use super::known_techniques::TECHNIQUE_DEFINITIONS;

const REGEN_HINT: &str = "快照由 TECHNIQUE_DEFINITIONS + yidao_skill_spec 具名双表\
    有序并集单向生成（plan-skill-anim-fidelity-v1 §8.1 #4a）、禁止手改；重生成：\
    cd server && BONG_REGEN_CAST_TICKS_SNAPSHOT=1 cargo test technique_cast_ticks_snapshot";

fn snapshot_path() -> PathBuf {
    PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../client/src/test/resources/bong/technique_cast_ticks_snapshot.json"
    ))
}

fn definitions_cast_map() -> BTreeMap<String, u32> {
    let mut map: BTreeMap<String, u32> = TECHNIQUE_DEFINITIONS
        .iter()
        .map(|def| (def.id.to_string(), def.cast_ticks))
        .collect();
    // plan-skill-anim-fidelity-v1 P4：yidao 5 招不在 TECHNIQUE_DEFINITIONS（功法
    // 定义表），其 cast 窗由 `yidao_skill_spec().cast_ticks_base` 声明（运行时经
    // `yidao_cast_ticks` 按 mastery/平和色缩放——快照 pin 基准值即可，对拍分类
    // 只依赖 cast ≥ 40 长引导域）。单向生成纪律不变：server 代码 → 快照。
    for skill in crate::combat::yidao::YidaoSkillId::ALL {
        let spec = crate::combat::yidao::yidao_skill_spec(skill);
        let cast_ticks = u32::try_from(spec.cast_ticks_base)
            .expect("yidao cast_ticks_base 应在 u32 域内（当前最大 1200）");
        let previous = map.insert(skill.skill_id().to_string(), cast_ticks);
        assert!(
            previous.is_none(),
            "yidao skill_id `{}` 与 TECHNIQUE_DEFINITIONS 撞名——两源合并会静默吞条目",
            skill.skill_id()
        );
    }
    map
}

/// 快照的规范字节形态：BTreeMap 键序 + serde_json pretty + 尾随换行。
fn canonical_snapshot_content() -> String {
    let mut json = serde_json::to_string_pretty(&definitions_cast_map())
        .expect("cast_ticks map serialization must not fail");
    json.push('\n');
    json
}

#[test]
fn technique_cast_ticks_snapshot_matches_definitions_exactly() {
    let expected = definitions_cast_map();
    assert_eq!(
        expected.len(),
        TECHNIQUE_DEFINITIONS.len() + crate::combat::yidao::YidaoSkillId::ALL.len(),
        "TECHNIQUE_DEFINITIONS / yidao spec 出现重复 id 会让快照吞条目——先修定义表再谈快照"
    );

    let path = snapshot_path();
    if std::env::var("BONG_REGEN_CAST_TICKS_SNAPSHOT").as_deref() == Ok("1") {
        std::fs::write(&path, canonical_snapshot_content())
            .unwrap_or_else(|error| panic!("重生成快照写盘失败（{}）：{error}", path.display()));
    }

    let content = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cast_ticks 快照缺失或不可读（{}，错误：{error}）——{REGEN_HINT}",
            path.display()
        )
    });
    let actual: BTreeMap<String, u32> = serde_json::from_str(&content).unwrap_or_else(|error| {
        panic!(
            "cast_ticks 快照不是合法的 {{skill_id: cast_ticks}} JSON 对象（{error}）——{REGEN_HINT}"
        )
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
        .filter(|(id, cast)| actual.get(*id) != Some(cast))
        .map(|(id, cast)| {
            format!(
                "{id}: 定义表={cast} 快照={}",
                actual
                    .get(id)
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "<无>".to_string())
            )
        })
        .collect();
    assert!(
        drifted.is_empty(),
        "快照 cast_ticks 与定义表漂移 {} 条：{drifted:#?}——{REGEN_HINT}\
         ——错误时长必须改动画对齐定义表，禁止「同步改快照混过关」（plan §8.1 #4）",
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
