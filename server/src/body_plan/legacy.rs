//! plan-race-system-v1 P0b — legacy `combat::components::BodyPart`（8 段人形 unit
//! enum）↔ 通用 `BodyPartId`（string）双向转换 + `PartConsequence` 通用分发查询。
//!
//! wire 协议 / `Wound.location` 在 P0b 阶段仍是 legacy `BodyPart` enum（开放化为 string
//! id 是 P1 `CombatBodyPartV1` 的工作，见 plan §P1）。本模块只解决"战斗消费点内部要查
//! `BodyPlan` 数据表时，把 legacy enum 换成 `BodyPartId` 去查"这一件事——
//! `humanoid.json` 的 8 个部位 id 与 legacy enum 的 8 个变体一一对应（同一份 snake_case
//! 字符串既是各消费点历史 wire 形态也是 `BodyPartId`，见 `types.rs` 顶部文档），故本转换
//! 全双射、无损、无 fallback 分支——`id_to_legacy_body_part` 只在喂入非 humanoid 8 段
//! 之外的陌生 id（如未来 whale 部位 id）时才返回 `None`。

use crate::combat::components::BodyPart;

use super::types::{BodyPartId, BodyPlan, PartConsequence};

/// legacy `BodyPart` → `BodyPartId`。humanoid plan 的 8 个部位 id 全部覆盖，恒有值。
pub fn legacy_body_part_to_id(part: BodyPart) -> BodyPartId {
    BodyPartId::new(match part {
        BodyPart::Head => "head",
        BodyPart::Chest => "chest",
        BodyPart::Back => "back",
        BodyPart::Abdomen => "abdomen",
        BodyPart::ArmL => "arm_l",
        BodyPart::ArmR => "arm_r",
        BodyPart::LegL => "leg_l",
        BodyPart::LegR => "leg_r",
    })
}

/// `BodyPartId` → legacy `BodyPart`。仅 humanoid 8 段字符串命中，其余（未来非人形部位
/// id）返回 `None`（防御性——不 panic，调用方自行决定如何处理"这个 id 没有 legacy
/// 对应物"，例如非 humanoid plan 就不该被喂进任何 legacy 消费点）。
pub fn id_to_legacy_body_part(id: &BodyPartId) -> Option<BodyPart> {
    Some(match id.as_str() {
        "head" => BodyPart::Head,
        "chest" => BodyPart::Chest,
        "back" => BodyPart::Back,
        "abdomen" => BodyPart::Abdomen,
        "arm_l" => BodyPart::ArmL,
        "arm_r" => BodyPart::ArmR,
        "leg_l" => BodyPart::LegL,
        "leg_r" => BodyPart::LegR,
        _ => return None,
    })
}

/// 通用 `PartConsequence` 分发查询：给定 `plan`，返回所有满足 `predicate` 的部位对应的
/// legacy `BodyPart`（无 legacy 对应物的部位静默跳过——P0b 只喂 humanoid plan，恒全部
/// 有对应物）。`combat::arm_wound` / `movement::leg_wound` 用本函数把"主/副手臂"、
/// "双腿"这类身份判定从硬编码 `BodyPart::ArmL/ArmR`/`LegL/LegR` 改为按
/// `PartConsequence::Manipulator{main_hand}`/`Locomotion` 标签查询 humanoid plan 数据
/// （决议见 plan §P0 "战斗消费点改造"）——两个模块的公开函数签名（`&Wounds` only，无
/// entity/registry 参数）保持不变，因为它们的既有外部调用点（`combat/needle.rs`、
/// `combat/anqi_v2.rs`、`movement/mod.rs`）不在本阶段改动范围内；数据来源改为查询
/// [`super::registry::humanoid_plan_static`]。
pub fn legacy_body_parts_matching<'a>(
    plan: &'a BodyPlan,
    predicate: impl Fn(&PartConsequence) -> bool + 'a,
) -> impl Iterator<Item = BodyPart> + 'a {
    plan.parts
        .iter()
        .filter(move |def| predicate(&def.consequence))
        .filter_map(|def| id_to_legacy_body_part(&def.id))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_LEGACY_PARTS: [BodyPart; 8] = [
        BodyPart::Head,
        BodyPart::Chest,
        BodyPart::Back,
        BodyPart::Abdomen,
        BodyPart::ArmL,
        BodyPart::ArmR,
        BodyPart::LegL,
        BodyPart::LegR,
    ];

    #[test]
    fn every_legacy_body_part_round_trips_through_id() {
        for part in ALL_LEGACY_PARTS {
            let id = legacy_body_part_to_id(part);
            assert_eq!(
                id_to_legacy_body_part(&id),
                Some(part),
                "legacy BodyPart {part:?} must round-trip through BodyPartId {id:?}"
            );
        }
    }

    #[test]
    fn legacy_body_part_to_id_produces_expected_snake_case_strings() {
        assert_eq!(legacy_body_part_to_id(BodyPart::Head).as_str(), "head");
        assert_eq!(legacy_body_part_to_id(BodyPart::Chest).as_str(), "chest");
        assert_eq!(legacy_body_part_to_id(BodyPart::Back).as_str(), "back");
        assert_eq!(
            legacy_body_part_to_id(BodyPart::Abdomen).as_str(),
            "abdomen"
        );
        assert_eq!(legacy_body_part_to_id(BodyPart::ArmL).as_str(), "arm_l");
        assert_eq!(legacy_body_part_to_id(BodyPart::ArmR).as_str(), "arm_r");
        assert_eq!(legacy_body_part_to_id(BodyPart::LegL).as_str(), "leg_l");
        assert_eq!(legacy_body_part_to_id(BodyPart::LegR).as_str(), "leg_r");
    }

    #[test]
    fn id_to_legacy_body_part_returns_none_for_unknown_id() {
        assert_eq!(
            id_to_legacy_body_part(&BodyPartId::new("tail_fin")),
            None,
            "非 humanoid 8 段字符串（如未来 whale 部位）必须返回 None，不得 panic 或猜测"
        );
    }

    #[test]
    fn id_to_legacy_body_part_returns_none_for_empty_id() {
        assert_eq!(id_to_legacy_body_part(&BodyPartId::new("")), None);
    }

    #[test]
    fn id_to_legacy_body_part_is_case_sensitive() {
        // 大小写不同也是"陌生 id"——不做大小写归一化，明确失败而非静默匹配。
        assert_eq!(id_to_legacy_body_part(&BodyPartId::new("Head")), None);
        assert_eq!(id_to_legacy_body_part(&BodyPartId::new("HEAD")), None);
    }

    fn plan_fixture() -> BodyPlan {
        use super::super::types::{
            BodyPartDef, HeightBand, HeightBandAssignment, HitGeometry, StandingAabbSpec,
        };
        use std::collections::HashMap;

        BodyPlan {
            id: "fixture".into(),
            display_name: "测试构型".to_string(),
            is_humanoid: true,
            parts: vec![
                BodyPartDef {
                    id: "arm_r".into(),
                    damage_mul: 0.7,
                    contam_mul: 0.8,
                    bleed_mul: 0.8,
                    consequence: PartConsequence::Manipulator { main_hand: true },
                },
                BodyPartDef {
                    id: "arm_l".into(),
                    damage_mul: 0.7,
                    contam_mul: 0.8,
                    bleed_mul: 0.8,
                    consequence: PartConsequence::Manipulator { main_hand: false },
                },
                BodyPartDef {
                    id: "leg_l".into(),
                    damage_mul: 0.6,
                    contam_mul: 0.7,
                    bleed_mul: 1.0,
                    consequence: PartConsequence::Locomotion,
                },
                BodyPartDef {
                    id: "leg_r".into(),
                    damage_mul: 0.6,
                    contam_mul: 0.7,
                    bleed_mul: 1.0,
                    consequence: PartConsequence::Locomotion,
                },
                BodyPartDef {
                    id: "head".into(),
                    damage_mul: 2.0,
                    contam_mul: 1.5,
                    bleed_mul: 1.5,
                    consequence: PartConsequence::Sensory,
                },
                BodyPartDef {
                    id: "chest".into(),
                    damage_mul: 1.0,
                    contam_mul: 1.0,
                    bleed_mul: 1.0,
                    consequence: PartConsequence::Core,
                },
            ],
            hit_geometry: HitGeometry::HeightBands {
                aabb: StandingAabbSpec {
                    half_width: 0.3,
                    height: 1.8,
                },
                bands: vec![HeightBand {
                    min_rel_y: -1.0,
                    assignment: HeightBandAssignment::Single {
                        part: "chest".into(),
                    },
                }],
                lateral_threshold: 0.19,
            },
            equip_slots: vec![],
            meridian_profile: None,
            mutation_slot_mapping: HashMap::new(),
        }
    }

    #[test]
    fn legacy_body_parts_matching_locomotion_finds_both_legs() {
        let plan = plan_fixture();
        let mut legs: Vec<BodyPart> =
            legacy_body_parts_matching(&plan, |c| matches!(c, PartConsequence::Locomotion))
                .collect();
        legs.sort_by_key(|part| format!("{part:?}"));
        assert_eq!(legs, vec![BodyPart::LegL, BodyPart::LegR]);
    }

    #[test]
    fn legacy_body_parts_matching_manipulator_main_hand_true_finds_only_arm_r() {
        let plan = plan_fixture();
        let found: Vec<BodyPart> = legacy_body_parts_matching(&plan, |c| {
            matches!(c, PartConsequence::Manipulator { main_hand: true })
        })
        .collect();
        assert_eq!(found, vec![BodyPart::ArmR]);
    }

    #[test]
    fn legacy_body_parts_matching_manipulator_main_hand_false_finds_only_arm_l() {
        let plan = plan_fixture();
        let found: Vec<BodyPart> = legacy_body_parts_matching(&plan, |c| {
            matches!(c, PartConsequence::Manipulator { main_hand: false })
        })
        .collect();
        assert_eq!(found, vec![BodyPart::ArmL]);
    }

    #[test]
    fn legacy_body_parts_matching_sensory_finds_only_head() {
        let plan = plan_fixture();
        let found: Vec<BodyPart> =
            legacy_body_parts_matching(&plan, |c| matches!(c, PartConsequence::Sensory)).collect();
        assert_eq!(found, vec![BodyPart::Head]);
    }

    #[test]
    fn legacy_body_parts_matching_core_finds_only_chest() {
        let plan = plan_fixture();
        let found: Vec<BodyPart> =
            legacy_body_parts_matching(&plan, |c| matches!(c, PartConsequence::Core)).collect();
        assert_eq!(found, vec![BodyPart::Chest]);
    }

    #[test]
    fn legacy_body_parts_matching_no_match_yields_empty_iterator() {
        let plan = plan_fixture();
        // fixture 里没有任何部位是 Manipulator{main_hand:true} 且同时匹配一个恒假谓词。
        let found: Vec<BodyPart> = legacy_body_parts_matching(&plan, |_| false).collect();
        assert!(found.is_empty());
    }

    #[test]
    fn legacy_body_parts_matching_skips_parts_without_legacy_mapping() {
        use super::super::types::BodyPartDef;

        let mut plan = plan_fixture();
        // 追加一个非 humanoid 8 段字符串的部位（如未来 whale 的尾鳍）——即便它也标了
        // Locomotion，也不应出现在结果里（没有 legacy BodyPart 对应物）。
        plan.parts.push(BodyPartDef {
            id: "tail_fin".into(),
            damage_mul: 0.5,
            contam_mul: 0.5,
            bleed_mul: 0.5,
            consequence: PartConsequence::Locomotion,
        });

        let mut legs: Vec<BodyPart> =
            legacy_body_parts_matching(&plan, |c| matches!(c, PartConsequence::Locomotion))
                .collect();
        legs.sort_by_key(|part| format!("{part:?}"));
        assert_eq!(
            legs,
            vec![BodyPart::LegL, BodyPart::LegR],
            "tail_fin 没有 legacy BodyPart 对应物，必须被静默跳过而非出现在结果或 panic"
        );
    }
}
