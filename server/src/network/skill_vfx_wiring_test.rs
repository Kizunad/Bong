//! plan-skill-anim-fidelity-v1 P5 —— 招式粒子接线矩阵的 server 侧闭环。
//!
//! 本文件负责矩阵的**全局性质**（清单同步 / 去复用真的发生了 / id 形态 / 优先级档位）；
//! 「驱动真实 resolver 断言发出正确 event_id」的逐招用例放在各自模块的 test mod 里
//! （harness 在那边：`zhenmai_v2::tests::app_with_events` / `burst_meridian::tests::full_app`
//! / `npc_skill::tests::world_with_events`），同样由 `P5_SKILL_VFX_WIRING` 表驱动。
//!
//! 清单**不可手改**——唯一重生成入口：
//! `cd server && BONG_REGEN_VFX_MANIFEST=1 cargo test skill_vfx_wiring`。

use std::collections::BTreeSet;
use std::path::PathBuf;

use super::skill_vfx_wiring::{wiring_for, SkillVfxWiring, P5_SKILL_VFX_WIRING};
use super::vfx_event_emit::{vfx_default_priority, VfxPriority};

const REGEN_HINT: &str = "清单由 P5_SKILL_VFX_WIRING 单向生成、禁止手改；重生成：\
    cd server && BONG_REGEN_VFX_MANIFEST=1 cargo test skill_vfx_wiring";

fn manifest_path() -> PathBuf {
    PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../client/src/test/resources/bong/skill_vfx_wiring_manifest.json"
    ))
}

/// 清单的规范字节形态：按 event_id 排序 + serde_json pretty + 尾随换行。
fn canonical_manifest_content() -> String {
    let mut rows: Vec<&SkillVfxWiring> = P5_SKILL_VFX_WIRING.iter().collect();
    rows.sort_by_key(|wiring| wiring.event_id);
    let mut json =
        serde_json::to_string_pretty(&rows).expect("wiring manifest serialization must not fail");
    json.push('\n');
    json
}

#[test]
fn skill_vfx_wiring_manifest_matches_table_exactly() {
    let path = manifest_path();
    if std::env::var("BONG_REGEN_VFX_MANIFEST").as_deref() == Ok("1") {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap_or_else(|error| {
                panic!("重生成清单建目录失败（{}）：{error}", parent.display())
            });
        }
        std::fs::write(&path, canonical_manifest_content())
            .unwrap_or_else(|error| panic!("重生成清单写盘失败（{}）：{error}", path.display()));
    }

    let content = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "粒子接线清单缺失或不可读（{}，错误：{error}）——{REGEN_HINT}",
            path.display()
        )
    });

    // 字节级锁：排序 / 缩进 / 尾随换行被手改同样判红（防"语义相同的手工整理"破坏稳定 diff，
    // 也防有人只改清单不改常量表——client 侧读的是清单，两边就此分叉）。
    assert_eq!(
        content,
        canonical_manifest_content(),
        "粒子接线清单与 P5_SKILL_VFX_WIRING 不一致（内容漂移或字节格式手改）——{REGEN_HINT}"
    );
}

#[test]
fn every_wiring_row_dropped_its_legacy_borrow() {
    // P5 的核心交付：11 招从「借别家粒子」改成专属 id。任何一行 event_id 回退成
    // legacy_event_id，都是去复用被撤销。
    for wiring in P5_SKILL_VFX_WIRING {
        assert_ne!(
            wiring.event_id, wiring.legacy_event_id,
            "去复用回归锁：{} 的粒子 event_id 退回了借用值 `{}`——P5 要求专属 id",
            wiring.skill_id, wiring.legacy_event_id
        );
    }
}

#[test]
fn wiring_event_ids_are_unique_per_skill() {
    // 11 招 11 个 id：任意两招共用 id 就是新的借用（旁观者无法读招）。
    let ids: BTreeSet<&str> = P5_SKILL_VFX_WIRING
        .iter()
        .map(|wiring| wiring.event_id)
        .collect();
    assert_eq!(
        ids.len(),
        P5_SKILL_VFX_WIRING.len(),
        "接线表出现共用 event_id（去复用不彻底）：{:?}",
        P5_SKILL_VFX_WIRING
            .iter()
            .map(|w| (w.skill_id, w.event_id))
            .collect::<Vec<_>>()
    );

    let skills: BTreeSet<&str> = P5_SKILL_VFX_WIRING
        .iter()
        .map(|wiring| wiring.skill_id)
        .collect();
    assert_eq!(
        skills.len(),
        P5_SKILL_VFX_WIRING.len(),
        "接线表出现重复 skill_id——同一招注册了两行接线"
    );
}

#[test]
fn wiring_event_ids_are_bong_namespaced_identifiers() {
    for wiring in P5_SKILL_VFX_WIRING {
        let (namespace, path) = wiring.event_id.split_once(':').unwrap_or_else(|| {
            panic!(
                "粒子 event_id `{}` 缺少 `namespace:path` 形态的冒号分隔",
                wiring.event_id
            )
        });
        assert_eq!(
            namespace, "bong",
            "粒子 event_id `{}` 命名空间必须是 `bong`——client `new Identifier(\"bong\", ...)` \
             只认该命名空间",
            wiring.event_id
        );
        assert!(
            !path.is_empty(),
            "粒子 event_id `{}` 的 path 为空串——client Identifier 解析会失败",
            wiring.event_id
        );
        // MC Identifier path 合法字符集（非法字符会让 client 端解析静默失败 → bridgeMiss）。
        assert!(
            path.chars()
                .all(|c| matches!(c, 'a'..='z' | '0'..='9' | '_' | '.' | '-' | '/')),
            "粒子 event_id `{}` 的 path `{path}` 含 MC Identifier 非法字符",
            wiring.event_id
        );
    }
}

#[test]
fn wiring_colors_are_six_digit_hex() {
    for wiring in P5_SKILL_VFX_WIRING {
        let color = wiring.color;
        assert!(
            color.len() == 7 && color.starts_with('#'),
            "{} 的粒子颜色 `{color}` 不是 `#RRGGBB` 形态——client `colorRgb()` 解析会落空回 fallback",
            wiring.skill_id
        );
        assert!(
            color[1..].chars().all(|c| c.is_ascii_hexdigit()),
            "{} 的粒子颜色 `{color}` 含非十六进制字符",
            wiring.skill_id
        );
    }
}

#[test]
fn zhenmai_family_shares_gold_hue_but_every_skill_is_distinguishable() {
    // plan §P5.1 ①：真脉 5 招共用金脉色系（anchor #D4AF6A）但逐招明度不同——
    // 同族可认 + 逐招可辨，两个性质都要锁。
    let zhenmai: Vec<&SkillVfxWiring> = P5_SKILL_VFX_WIRING
        .iter()
        .filter(|wiring| wiring.player_class == "ZhenmaiPulsePlayer")
        .collect();
    assert_eq!(
        zhenmai.len(),
        5,
        "真脉应有 5 招接线，实际 {}",
        zhenmai.len()
    );

    let colors: BTreeSet<&str> = zhenmai.iter().map(|wiring| wiring.color).collect();
    assert_eq!(
        colors.len(),
        5,
        "真脉 5 招颜色必须两两不同（旁观读招依赖明度阶梯），实际 {colors:?}"
    );

    // 金脉色系判据：R > G > B 且 R 与 B 差值 ≥ 0x40（暖金，不会被读成红/灰/青）。
    for wiring in &zhenmai {
        let (r, g, b) = parse_rgb(wiring.color);
        assert!(
            r > g && g > b,
            "{} 的颜色 `{}` 不满足金脉色系 R>G>B，实际 R={r} G={g} B={b}",
            wiring.skill_id,
            wiring.color
        );
        assert!(
            r - b >= 0x40,
            "{} 的颜色 `{}` 冷暖对比不足（R-B={}，要求 ≥ 64），会被读成灰而非金脉",
            wiring.skill_id,
            wiring.color,
            r - b
        );
    }

    // anchor 必须真在表里出现（plan 字面锁定 #D4AF6A 为金脉本色）。
    assert!(
        zhenmai.iter().any(|wiring| wiring.color == "#D4AF6A"),
        "真脉色系 anchor #D4AF6A 未出现在任何一招上——plan §P5.1 ① 指定其为金脉本色"
    );
}

#[test]
fn burst_meridian_family_shares_one_color_and_differs_only_by_form() {
    // plan §P5.1 ②：爆脉 3 招**共用** #C58B3F，读招完全靠形态分化。
    // 与真脉相反的设计，所以断言方向也相反：颜色必须全同、id 必须全异。
    let burst: Vec<&SkillVfxWiring> = P5_SKILL_VFX_WIRING
        .iter()
        .filter(|wiring| wiring.player_class == "BurstMeridianFamilyPlayer")
        .collect();
    assert_eq!(burst.len(), 3, "爆脉应有 3 招接线，实际 {}", burst.len());

    for wiring in &burst {
        assert_eq!(
            wiring.color, "#C58B3F",
            "{} 的颜色应为爆脉家族统一识别色 #C58B3F（形态分化而非配色分化），实际 {}",
            wiring.skill_id, wiring.color
        );
    }

    let ids: BTreeSet<&str> = burst.iter().map(|wiring| wiring.event_id).collect();
    assert_eq!(
        ids.len(),
        3,
        "爆脉 3 招共用颜色时 event_id 必须全异——否则形态也无从分化，实际 {ids:?}"
    );
}

#[test]
fn npc_skills_have_independent_ids_and_widely_separated_colors() {
    // plan §P5.1 ③：npc 3 招「id 与颜色必须独立，保证旁观读招」。
    // 仅仅"hex 字符串不相等"不够——旧 #9FD8C8 vs #A8E6CF 就不相等却远距离不可辨。
    // 这里用色相通道差做可辨性判据。
    let npc: Vec<&SkillVfxWiring> = P5_SKILL_VFX_WIRING
        .iter()
        .filter(|wiring| wiring.player_class == "NpcSkillAuraPlayer")
        .collect();
    assert_eq!(npc.len(), 3, "NPC 应有 3 招接线，实际 {}", npc.len());

    let ids: BTreeSet<&str> = npc.iter().map(|wiring| wiring.event_id).collect();
    assert_eq!(ids.len(), 3, "NPC 3 招 event_id 必须两两不同，实际 {ids:?}");

    for (i, left) in npc.iter().enumerate() {
        for right in npc.iter().skip(i + 1) {
            let (lr, lg, lb) = parse_rgb(left.color);
            let (rr, rg, rb) = parse_rgb(right.color);
            let distance = (lr - rr).abs() + (lg - rg).abs() + (lb - rb).abs();
            assert!(
                distance >= 0x60,
                "{} `{}` 与 {} `{}` 的配色距离仅 {distance}（要求 ≥ 96）——两招远距离不可辨，\
                 违背「颜色必须独立、保证旁观读招」",
                left.skill_id,
                left.color,
                right.skill_id,
                right.color
            );
        }
    }
}

#[test]
fn player_skill_particles_are_important_while_npc_particles_stay_normal() {
    // 档位是 P5 的有意设计（plan §P5.1 ④），不是自然掉落的结果，所以逐行 pin：
    // 玩家流派招（真脉/爆脉）必须 Important，NPC 背景 cosmetic 必须 Normal。
    for wiring in P5_SKILL_VFX_WIRING {
        let expected = if wiring.player_class == "NpcSkillAuraPlayer" {
            VfxPriority::Normal
        } else {
            VfxPriority::Important
        };
        assert_eq!(
            vfx_default_priority(wiring.event_id),
            expected,
            "{}（{}）的粒子优先级应为 {expected:?}，实际 {:?}——玩家技能粒子归 Important 才不会\
             在拥挤 chunk 被优先丢弃；NPC 粒子归 Normal 才不会挤掉玩家自己的技能反馈",
            wiring.skill_id,
            wiring.event_id,
            vfx_default_priority(wiring.event_id)
        );
    }
}

#[test]
fn wiring_lookup_finds_every_registered_skill_and_rejects_unknown() {
    for wiring in P5_SKILL_VFX_WIRING {
        let found = wiring_for(wiring.skill_id)
            .unwrap_or_else(|| panic!("wiring_for 查不到已登记的招式 {}", wiring.skill_id));
        assert_eq!(
            found.event_id, wiring.event_id,
            "wiring_for({}) 返回了错误的接线行",
            wiring.skill_id
        );
    }
    assert!(
        wiring_for("zhenmai.not_a_real_skill").is_none(),
        "wiring_for 对未登记招式必须返回 None（防查表落空时静默取到相邻行）"
    );
    assert!(wiring_for("").is_none(), "wiring_for 对空串必须返回 None");
}

/// `#RRGGBB` → (R, G, B)，取 i32 便于做差值比较。
fn parse_rgb(hex: &str) -> (i32, i32, i32) {
    let value = i32::from_str_radix(&hex[1..], 16)
        .unwrap_or_else(|error| panic!("颜色 `{hex}` 解析失败：{error}"));
    ((value >> 16) & 0xFF, (value >> 8) & 0xFF, value & 0xFF)
}
