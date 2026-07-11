//! plan-race-system-v1 P0b —— `combined_leg_factor` 的"哪些部位算腿"判定改为查询
//! **调用方传入的** `&BodyPlan` 的 `PartConsequence::Locomotion` 标签
//! （`assets/body_plans/plans/humanoid.json`），不再硬编码假设"`LegL`/`LegR` 这两个
//! enum 变体就是双腿"。humanoid plan 目前恰好只标了这两个部位为 `Locomotion`，故行为
//! bit-for-bit 不变；泛化后天然支持未来非人形 plan（如飞鲸尾鳍）声明任意数量的
//! locomotion 部位取最劣值。
//!
//! ## plan-race-system-v1 P0 review 修复 —— 不再固定读 humanoid 单例（BLOCKING-1）
//!
//! [`combined_leg_factor`] / [`combined_leg_factor_from_optional`] 现在接受一个
//! `plan: &BodyPlan` 参数，由**生产调用点**（`movement/mod.rs` 的
//! `apply_movement_speed_system` / `handle_movement_action_intents`）经
//! [`crate::body_plan::resolve_body_plan_for_target`] 按**目标实体**解析后传入——
//! 不再无条件绑死 [`crate::body_plan::humanoid_plan_static`]。资源缺失（大量既有单测
//! 未插入 `BodyPlanRegistry`/`RaceRegistry`）时优雅退化到 `humanoid_plan_static()`，
//! humanoid 行为 bit-for-bit 不变。

use crate::body_plan::{BodyPartId, BodyPlan, PartConsequence};
use crate::combat::components::{Wound, Wounds};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegWoundGrade {
    Intact,
    Bruise,
    Abrasion,
    Laceration,
    Fracture,
    Severed,
}

pub fn leg_wound_to_speed(wound: LegWoundGrade) -> f32 {
    match wound {
        LegWoundGrade::Intact | LegWoundGrade::Bruise => 1.0,
        LegWoundGrade::Abrasion => 0.9,
        LegWoundGrade::Laceration => 0.7,
        LegWoundGrade::Fracture => 0.4,
        LegWoundGrade::Severed => 0.0,
    }
}

pub fn combined_leg_factor_from_optional(wounds: Option<&Wounds>, plan: &BodyPlan) -> f32 {
    wounds
        .map(|wounds| combined_leg_factor(wounds, plan))
        .unwrap_or(1.0)
}

/// `plan` 必须是**目标实体**经 [`crate::body_plan::resolve_body_plan_for_target`]
/// 解析出的 `BodyPlanPurpose::Intrinsic` 结果——生产调用点不得再传入固定的
/// `humanoid_plan_static()`（见模块顶部"P0 review 修复"文档）。
pub fn combined_leg_factor(wounds: &Wounds, plan: &BodyPlan) -> f32 {
    // plan-race-system-v1 P0 review r2（BLOCKING-2 收口）—— 改用 `BodyPlan::parts_matching`
    // 直接产出部位 id，取代会静默跳过非人形部位 id 的 `legacy::legacy_body_parts_matching`
    // （`Wound.location` 已经是 `BodyPartId`，不再需要反压 legacy `BodyPart` 才能比较）。
    plan.parts_matching(|consequence| matches!(consequence, PartConsequence::Locomotion))
        .map(|part_id| leg_wound_to_speed(worst_wound_grade(wounds, part_id)))
        .fold(1.0_f32, f32::min)
}

pub fn leg_strain_magnitude(leg_wound_factor: f32) -> f32 {
    ((1.0 - leg_wound_factor.clamp(0.0, 1.0)) / 0.15).clamp(0.0, 1.0)
}

/// plan-race-system-v1 P0 review r2（BLOCKING-2 收口）—— 参数从 legacy `BodyPart` 迁移
/// 为 `&BodyPartId`，镜像 `combat::arm_wound::worst_wound_grade` 同一次迁移。
pub fn worst_wound_grade(wounds: &Wounds, part_id: &BodyPartId) -> LegWoundGrade {
    wounds
        .entries
        .iter()
        .filter(|wound| &wound.location == part_id)
        .map(wound_grade)
        .max_by_key(|grade| grade_rank(*grade))
        .unwrap_or(LegWoundGrade::Intact)
}

fn wound_grade(wound: &Wound) -> LegWoundGrade {
    wound_severity_to_grade(wound.severity)
}

pub fn wound_severity_to_grade(severity: f32) -> LegWoundGrade {
    if severity >= 70.0 {
        LegWoundGrade::Severed
    } else if severity >= 35.0 {
        LegWoundGrade::Fracture
    } else if severity >= 15.0 {
        LegWoundGrade::Laceration
    } else if severity >= 5.0 {
        LegWoundGrade::Abrasion
    } else if severity > 0.0 {
        LegWoundGrade::Bruise
    } else {
        LegWoundGrade::Intact
    }
}

const fn grade_rank(grade: LegWoundGrade) -> u8 {
    match grade {
        LegWoundGrade::Intact => 0,
        LegWoundGrade::Bruise => 1,
        LegWoundGrade::Abrasion => 2,
        LegWoundGrade::Laceration => 3,
        LegWoundGrade::Fracture => 4,
        LegWoundGrade::Severed => 5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::components::{BodyPart, WoundKind, Wounds};

    fn wound(location: BodyPart, severity: f32) -> Wound {
        Wound {
            location: crate::body_plan::legacy_body_part_to_id(location),
            kind: WoundKind::Blunt,
            severity,
            bleeding_per_sec: 0.0,
            created_at_tick: 0,
            inflicted_by: None,
        }
    }

    #[test]
    fn canonical_leg_wound_speed_table() {
        assert_eq!(leg_wound_to_speed(LegWoundGrade::Intact), 1.0);
        assert_eq!(leg_wound_to_speed(LegWoundGrade::Bruise), 1.0);
        assert_eq!(leg_wound_to_speed(LegWoundGrade::Abrasion), 0.9);
        assert_eq!(leg_wound_to_speed(LegWoundGrade::Laceration), 0.7);
        assert_eq!(leg_wound_to_speed(LegWoundGrade::Fracture), 0.4);
        assert_eq!(leg_wound_to_speed(LegWoundGrade::Severed), 0.0);
    }

    // ── plan-race-system-v1 P0b：LegL/LegR 必须在 humanoid.json 标 Locomotion，否则
    // combined_leg_factor 的 PartConsequence 分发会静默漏掉腿伤 ──────────────────

    #[test]
    fn leg_l_and_leg_r_are_tagged_locomotion_in_humanoid_plan() {
        let plan = crate::body_plan::humanoid_plan_static();
        for part in [BodyPart::LegL, BodyPart::LegR] {
            let id = crate::body_plan::legacy_body_part_to_id(part);
            let def = plan
                .parts
                .iter()
                .find(|def| def.id == id)
                .unwrap_or_else(|| panic!("humanoid.json must declare part {id}"));
            assert_eq!(
                def.consequence,
                PartConsequence::Locomotion,
                "{part:?} (id={id}) must be tagged Locomotion in humanoid.json for combined_leg_factor to see it"
            );
        }
    }

    #[test]
    fn combined_leg_factor_ignores_non_locomotion_wounds_even_when_severed() {
        // Chest/Arm 伤势即便判 Severed 也不该压低腿部速度系数。
        let wounds = Wounds {
            entries: vec![wound(BodyPart::Chest, 99.0), wound(BodyPart::ArmR, 99.0)],
            ..Default::default()
        };
        assert_eq!(
            combined_leg_factor(&wounds, crate::body_plan::humanoid_plan_static()),
            1.0
        );
    }

    #[test]
    fn combined_factor_takes_worst_leg() {
        let wounds = Wounds {
            entries: vec![
                wound(BodyPart::LegL, 18.0),
                wound(BodyPart::LegR, 42.0),
                wound(BodyPart::ArmL, 99.0),
            ],
            ..Default::default()
        };

        assert_eq!(
            combined_leg_factor(&wounds, crate::body_plan::humanoid_plan_static()),
            0.4
        );
    }

    #[test]
    fn healed_legs_return_to_normal() {
        assert_eq!(
            combined_leg_factor(&Wounds::default(), crate::body_plan::humanoid_plan_static()),
            1.0
        );
    }

    // ── plan-race-system-v1 P0 review 修复：`combined_leg_factor` 必须真的按调用方
    // 传入的 `plan` 分发，而不是内部悄悄查 `humanoid_plan_static()`（BLOCKING-1）───

    /// 合成一个"外星"身体构型：只把 legacy `ArmR`（humanoid.json 里是 `Manipulator`）
    /// 标为 `Locomotion`，`LegL`/`LegR`（humanoid.json 里的双腿）反而不挂
    /// `Locomotion` 标签——用于证明 [`combined_leg_factor`] 完全按传入的 `plan` 数据
    /// 分发，不是硬编码假设"`LegL`/`LegR` 这两个 enum 变体就是双腿"。
    fn alien_locomotion_plan() -> BodyPlan {
        use crate::body_plan::{BodyPartDef, BodyPlanId};

        BodyPlan {
            id: BodyPlanId::new("test_alien_locomotion"),
            display_name: "测试用外星构型".to_string(),
            is_humanoid: false,
            parts: vec![
                BodyPartDef {
                    id: crate::body_plan::legacy_body_part_to_id(BodyPart::ArmR),
                    damage_mul: 1.0,
                    contam_mul: 1.0,
                    bleed_mul: 1.0,
                    consequence: PartConsequence::Locomotion,
                },
                BodyPartDef {
                    id: crate::body_plan::legacy_body_part_to_id(BodyPart::LegL),
                    damage_mul: 1.0,
                    contam_mul: 1.0,
                    bleed_mul: 1.0,
                    consequence: PartConsequence::Core,
                },
                BodyPartDef {
                    id: crate::body_plan::legacy_body_part_to_id(BodyPart::LegR),
                    damage_mul: 1.0,
                    contam_mul: 1.0,
                    bleed_mul: 1.0,
                    consequence: PartConsequence::Core,
                },
            ],
            hit_geometry: crate::body_plan::humanoid_plan_static()
                .hit_geometry
                .clone(),
            equip_slots: vec![],
            meridian_profile: None,
            mutation_slot_mapping: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn combined_leg_factor_reads_locomotion_tag_from_supplied_plan_not_humanoid_static() {
        let alien = alien_locomotion_plan();

        // LegR Severed：humanoid.json 把 LegR 标为 Locomotion，但外星构型把它标成
        // Core——速度系数必须保持中性 1.0，证明不是硬编码"LegR 就是腿"。
        let leg_r_severed = Wounds {
            entries: vec![wound(BodyPart::LegR, 75.0)],
            ..Default::default()
        };
        assert_eq!(
            combined_leg_factor(&leg_r_severed, &alien),
            1.0,
            "外星构型里 LegR 是 Core 非 Locomotion，LegR 断裂不应压低速度系数"
        );

        // ArmR Severed：外星构型把 ArmR 标成 Locomotion——速度系数必须真的按 Severed
        // 分级压到 0.0，证明结果来自传入 plan 的数据，而不是 humanoid_plan_static()
        // （humanoid.json 里 ArmR 是 Manipulator，与 leg_wound 毫无关系）。
        let arm_r_severed = Wounds {
            entries: vec![wound(BodyPart::ArmR, 75.0)],
            ..Default::default()
        };
        assert_eq!(
            combined_leg_factor(&arm_r_severed, &alien),
            0.0,
            "外星构型的 Locomotion 标签落在 ArmR 上，ArmR 断裂必须把速度系数压到 0.0（Severed）"
        );

        // 同一份伤口喂给 humanoid_plan_static() 时行为必须完全不同（ArmR 在 humanoid
        // plan 里只挂 Manipulator，不影响移动速度）——同一个 Wounds 值、不同 plan
        // 产出不同结果，锁死"结果随 plan 数据变化"这条核心断言。
        assert_eq!(
            combined_leg_factor(&arm_r_severed, crate::body_plan::humanoid_plan_static()),
            1.0,
            "同一伤口在 humanoid plan 下 ArmR 不是 Locomotion，不应压低速度——\
             与外星构型的 0.0 形成对照，证明 combined_leg_factor 结果随 plan 数据变化"
        );
    }
}
