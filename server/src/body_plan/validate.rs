//! plan-race-system-v1 P0 — `validate_body_plan`：单个 `BodyPlan` 的全图校验。
//!
//! 校验范围（P0 交付；P1/P4 随 `meridian_profile` / `morph_pairs` 字段扩展而追加）：
//! - part id 在本 plan 内唯一
//! - 每个部位的倍率字段有限且非负
//! - `HitGeometry::PartBoxes` 的 `part_id` 悬空检测（每个 box 必须引用已声明的部位）
//! - `HitGeometry::HeightBands` 的带引用悬空检测 + 排序 + 全覆盖（`bands` 必须按
//!   `min_rel_y` 严格降序排列，且最低一条带的 `min_rel_y < 0.0`，保证 `rel_y` clamp 到
//!   `[0,1]` 后任意取值都能命中某一条带，`classify_height_bands` 不会返回 `None`）
//! - `mutation_slot_mapping` 的值（`BodyPartId`）悬空检测
//!
//! 跨 registry 的校验（`races.json` 的 `body_plan_id` 存在性 / `morph_pairs.part_mapping`
//! 端点）不在本文件——那需要同时持有 `BodyPlanRegistry` 与待校验的 `RaceRegistry` 内容，
//! 归属 `race_registry::RaceRegistry::load_file`（见该文件顶部注释）。

use std::collections::HashSet;

use super::types::{BodyPartId, BodyPlan, HitGeometry, PartBox};

pub fn validate_body_plan(plan: &BodyPlan) -> Result<(), String> {
    if plan.id.as_str().trim().is_empty() {
        return Err("body plan id must not be empty".to_string());
    }
    if plan.parts.is_empty() {
        return Err(format!(
            "body plan {} must declare at least one part",
            plan.id
        ));
    }

    let mut part_ids: HashSet<BodyPartId> = HashSet::new();
    for part in &plan.parts {
        if part.id.as_str().trim().is_empty() {
            return Err(format!("body plan {} has a part with an empty id", plan.id));
        }
        if !part_ids.insert(part.id.clone()) {
            return Err(format!(
                "body plan {} has duplicate part id {}",
                plan.id, part.id
            ));
        }
        for (label, value) in [
            ("damage_mul", part.damage_mul),
            ("contam_mul", part.contam_mul),
            ("bleed_mul", part.bleed_mul),
        ] {
            if !value.is_finite() || value < 0.0 {
                return Err(format!(
                    "body plan {} part {} has invalid {label}={value} (must be finite and >= 0)",
                    plan.id, part.id
                ));
            }
        }
    }

    match &plan.hit_geometry {
        HitGeometry::HeightBands {
            aabb,
            bands,
            lateral_threshold,
        } => {
            if !(aabb.half_width.is_finite() && aabb.half_width > 0.0) {
                return Err(format!(
                    "body plan {} hit_geometry.aabb.half_width must be positive finite, got {}",
                    plan.id, aabb.half_width
                ));
            }
            if !(aabb.height.is_finite() && aabb.height > 0.0) {
                return Err(format!(
                    "body plan {} hit_geometry.aabb.height must be positive finite, got {}",
                    plan.id, aabb.height
                ));
            }
            if !lateral_threshold.is_finite() || *lateral_threshold < 0.0 {
                return Err(format!(
                    "body plan {} hit_geometry.lateral_threshold must be finite and >= 0, got {lateral_threshold}",
                    plan.id
                ));
            }
            if bands.is_empty() {
                return Err(format!(
                    "body plan {} hit_geometry.bands must declare at least one band",
                    plan.id
                ));
            }

            let mut previous_min_rel_y: Option<f64> = None;
            for band in bands {
                if !band.min_rel_y.is_finite() {
                    return Err(format!(
                        "body plan {} band min_rel_y must be finite, got {}",
                        plan.id, band.min_rel_y
                    ));
                }
                if let Some(previous) = previous_min_rel_y {
                    if band.min_rel_y >= previous {
                        return Err(format!(
                            "body plan {} bands must be strictly descending by min_rel_y (saw {previous} then {})",
                            plan.id, band.min_rel_y
                        ));
                    }
                }
                previous_min_rel_y = Some(band.min_rel_y);

                for referenced in band.assignment.referenced_part_ids() {
                    if !part_ids.contains(referenced) {
                        return Err(format!(
                            "body plan {} height_bands references unknown part id {}",
                            plan.id, referenced
                        ));
                    }
                }
            }

            if previous_min_rel_y.is_none_or(|min| min >= 0.0) {
                return Err(format!(
                    "body plan {} lowest band must use min_rel_y < 0.0 to guarantee full [0,1] rel_y coverage (no gap at rel_y=0)",
                    plan.id
                ));
            }
        }
        HitGeometry::PartBoxes { boxes } => {
            validate_part_boxes(plan, boxes, &part_ids)?;
        }
    }

    for (slot, part_id) in &plan.mutation_slot_mapping {
        if !part_ids.contains(part_id) {
            return Err(format!(
                "body plan {} mutation_slot_mapping[{slot:?}] references unknown part id {}",
                plan.id, part_id
            ));
        }
    }

    // P1 起 meridian_profile.channels 落地后，在此追加 channel id 唯一性 /
    // realm_requirements 单调性 <= channel 总数的校验；P0 stub `MeridianProfile {}`
    // 无字段可校验，本函数当前对 meridian_profile 内容免检（存在性本身已由类型系统
    // 的 Option 语义 + types.rs 的 pin 测试锁定）。

    Ok(())
}

fn validate_part_boxes(
    plan: &BodyPlan,
    boxes: &[PartBox],
    part_ids: &HashSet<BodyPartId>,
) -> Result<(), String> {
    if boxes.is_empty() {
        return Err(format!(
            "body plan {} hit_geometry.boxes must declare at least one part box",
            plan.id
        ));
    }
    for part_box in boxes {
        if !part_ids.contains(&part_box.part_id) {
            return Err(format!(
                "body plan {} part_box references unknown part id {}",
                plan.id, part_box.part_id
            ));
        }
        for (axis, value) in ["x", "y", "z"].into_iter().zip(part_box.half_extents) {
            if !value.is_finite() || value <= 0.0 {
                return Err(format!(
                    "body plan {} part_box {} half_extents.{axis}={value} must be positive finite",
                    plan.id, part_box.part_id
                ));
            }
        }
        for (axis, value) in ["x", "y", "z"].into_iter().zip(part_box.offset) {
            if !value.is_finite() {
                return Err(format!(
                    "body plan {} part_box {} offset.{axis}={value} must be finite",
                    plan.id, part_box.part_id
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body_plan::types::{
        BodyPartDef, BodyPlanId, HeightBand, HeightBandAssignment, PartConsequence,
        StandingAabbSpec,
    };
    use std::collections::HashMap;

    fn base_plan() -> BodyPlan {
        BodyPlan {
            id: BodyPlanId::new("humanoid_like"),
            display_name: "测试人形".to_string(),
            is_humanoid: true,
            parts: vec![
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
                bands: vec![
                    HeightBand {
                        min_rel_y: 0.5,
                        assignment: HeightBandAssignment::Single {
                            part: "head".into(),
                        },
                    },
                    HeightBand {
                        min_rel_y: -1.0,
                        assignment: HeightBandAssignment::Single {
                            part: "chest".into(),
                        },
                    },
                ],
                lateral_threshold: 0.19,
            },
            equip_slots: vec![],
            meridian_profile: None,
            mutation_slot_mapping: HashMap::new(),
        }
    }

    #[test]
    fn happy_path_passes() {
        assert!(validate_body_plan(&base_plan()).is_ok());
    }

    #[test]
    fn empty_plan_id_rejected() {
        let mut plan = base_plan();
        plan.id = BodyPlanId::new("");
        assert!(validate_body_plan(&plan)
            .unwrap_err()
            .contains("must not be empty"));
    }

    #[test]
    fn empty_parts_rejected() {
        let mut plan = base_plan();
        plan.parts.clear();
        assert!(validate_body_plan(&plan)
            .unwrap_err()
            .contains("at least one part"));
    }

    #[test]
    fn duplicate_part_id_rejected() {
        let mut plan = base_plan();
        let dup = plan.parts[0].clone();
        plan.parts.push(dup);
        assert!(validate_body_plan(&plan)
            .unwrap_err()
            .contains("duplicate part id"));
    }

    #[test]
    fn non_finite_damage_mul_rejected() {
        let mut plan = base_plan();
        plan.parts[0].damage_mul = f32::NAN;
        assert!(validate_body_plan(&plan)
            .unwrap_err()
            .contains("damage_mul"));
    }

    #[test]
    fn negative_bleed_mul_rejected() {
        let mut plan = base_plan();
        plan.parts[0].bleed_mul = -0.1;
        assert!(validate_body_plan(&plan).unwrap_err().contains("bleed_mul"));
    }

    #[test]
    fn zero_multiplier_is_allowed() {
        let mut plan = base_plan();
        plan.parts[0].damage_mul = 0.0;
        assert!(validate_body_plan(&plan).is_ok(), "0.0 是合法的免疫倍率");
    }

    #[test]
    fn height_bands_non_positive_half_width_rejected() {
        let mut plan = base_plan();
        if let HitGeometry::HeightBands { aabb, .. } = &mut plan.hit_geometry {
            aabb.half_width = 0.0;
        }
        assert!(validate_body_plan(&plan)
            .unwrap_err()
            .contains("half_width"));
    }

    #[test]
    fn height_bands_non_positive_height_rejected() {
        let mut plan = base_plan();
        if let HitGeometry::HeightBands { aabb, .. } = &mut plan.hit_geometry {
            aabb.height = -1.0;
        }
        assert!(validate_body_plan(&plan).unwrap_err().contains("height"));
    }

    #[test]
    fn height_bands_negative_lateral_threshold_rejected() {
        let mut plan = base_plan();
        if let HitGeometry::HeightBands {
            lateral_threshold, ..
        } = &mut plan.hit_geometry
        {
            *lateral_threshold = -0.01;
        }
        assert!(validate_body_plan(&plan)
            .unwrap_err()
            .contains("lateral_threshold"));
    }

    #[test]
    fn height_bands_empty_bands_rejected() {
        let mut plan = base_plan();
        if let HitGeometry::HeightBands { bands, .. } = &mut plan.hit_geometry {
            bands.clear();
        }
        assert!(validate_body_plan(&plan)
            .unwrap_err()
            .contains("at least one band"));
    }

    #[test]
    fn height_bands_unsorted_rejected() {
        let mut plan = base_plan();
        if let HitGeometry::HeightBands { bands, .. } = &mut plan.hit_geometry {
            bands.reverse();
        }
        assert!(validate_body_plan(&plan)
            .unwrap_err()
            .contains("strictly descending"));
    }

    #[test]
    fn height_bands_duplicate_min_rel_y_rejected() {
        let mut plan = base_plan();
        if let HitGeometry::HeightBands { bands, .. } = &mut plan.hit_geometry {
            bands[1].min_rel_y = bands[0].min_rel_y;
        }
        assert!(validate_body_plan(&plan)
            .unwrap_err()
            .contains("strictly descending"));
    }

    #[test]
    fn height_bands_missing_full_coverage_rejected() {
        let mut plan = base_plan();
        if let HitGeometry::HeightBands { bands, .. } = &mut plan.hit_geometry {
            bands[1].min_rel_y = 0.1; // 不是 <0.0，rel_y=0 会落空
        }
        let err = validate_body_plan(&plan).unwrap_err();
        assert!(err.contains("min_rel_y < 0.0"), "got: {err}");
    }

    #[test]
    fn height_bands_dangling_part_reference_rejected() {
        let mut plan = base_plan();
        if let HitGeometry::HeightBands { bands, .. } = &mut plan.hit_geometry {
            bands[0].assignment = HeightBandAssignment::Single {
                part: "ghost_part".into(),
            };
        }
        assert!(validate_body_plan(&plan)
            .unwrap_err()
            .contains("unknown part id"));
    }

    #[test]
    fn height_bands_lateral_split_with_center_dangling_reference_rejected() {
        let mut plan = base_plan();
        if let HitGeometry::HeightBands { bands, .. } = &mut plan.hit_geometry {
            bands[0].assignment = HeightBandAssignment::LateralSplitWithCenter {
                left: "arm_l".into(),
                right: "arm_r".into(),
                center: "chest".into(),
            };
        }
        assert!(validate_body_plan(&plan)
            .unwrap_err()
            .contains("unknown part id"));
    }

    #[test]
    fn part_boxes_happy_path() {
        let mut plan = base_plan();
        plan.hit_geometry = HitGeometry::PartBoxes {
            boxes: vec![PartBox {
                part_id: "head".into(),
                offset: [0.0, 1.5, 0.0],
                half_extents: [0.3, 0.3, 0.3],
                priority: 0,
            }],
        };
        assert!(validate_body_plan(&plan).is_ok());
    }

    #[test]
    fn part_boxes_empty_rejected() {
        let mut plan = base_plan();
        plan.hit_geometry = HitGeometry::PartBoxes { boxes: vec![] };
        assert!(validate_body_plan(&plan)
            .unwrap_err()
            .contains("at least one part box"));
    }

    #[test]
    fn part_boxes_dangling_part_id_rejected() {
        let mut plan = base_plan();
        plan.hit_geometry = HitGeometry::PartBoxes {
            boxes: vec![PartBox {
                part_id: "ghost".into(),
                offset: [0.0, 0.0, 0.0],
                half_extents: [0.3, 0.3, 0.3],
                priority: 0,
            }],
        };
        assert!(validate_body_plan(&plan)
            .unwrap_err()
            .contains("unknown part id"));
    }

    #[test]
    fn part_boxes_non_positive_half_extent_rejected() {
        let mut plan = base_plan();
        plan.hit_geometry = HitGeometry::PartBoxes {
            boxes: vec![PartBox {
                part_id: "head".into(),
                offset: [0.0, 0.0, 0.0],
                half_extents: [0.0, 0.3, 0.3],
                priority: 0,
            }],
        };
        assert!(validate_body_plan(&plan)
            .unwrap_err()
            .contains("half_extents"));
    }

    #[test]
    fn part_boxes_non_finite_offset_rejected() {
        let mut plan = base_plan();
        plan.hit_geometry = HitGeometry::PartBoxes {
            boxes: vec![PartBox {
                part_id: "head".into(),
                offset: [f64::NAN, 0.0, 0.0],
                half_extents: [0.3, 0.3, 0.3],
                priority: 0,
            }],
        };
        assert!(validate_body_plan(&plan).unwrap_err().contains("offset"));
    }

    #[test]
    fn mutation_slot_mapping_dangling_reference_rejected() {
        use crate::dandao::mutation::BodySlot;
        let mut plan = base_plan();
        plan.mutation_slot_mapping
            .insert(BodySlot::Head, "ghost_part".into());
        assert!(validate_body_plan(&plan)
            .unwrap_err()
            .contains("mutation_slot_mapping"));
    }

    #[test]
    fn mutation_slot_mapping_valid_reference_accepted() {
        use crate::dandao::mutation::BodySlot;
        let mut plan = base_plan();
        plan.mutation_slot_mapping
            .insert(BodySlot::Head, "head".into());
        assert!(validate_body_plan(&plan).is_ok());
    }

    #[test]
    fn empty_mutation_slot_mapping_is_valid_for_non_humanoid_plans() {
        let plan = base_plan();
        assert!(plan.mutation_slot_mapping.is_empty());
        assert!(validate_body_plan(&plan).is_ok());
    }

    /// 每个 `BodySlot` 变体各自专属 case：悬空引用必须被拒绝，且错误消息带上具体是
    /// 哪个 slot 出的问题（`{slot:?}` 已格式进 reason，见 `validate_body_plan`）——
    /// 覆盖全部 5 个变体，而非只测 `Head` 就假设其余变体"应该也一样"。
    #[test]
    fn mutation_slot_mapping_dangling_reference_rejected_for_every_body_slot_variant() {
        use crate::dandao::mutation::BodySlot;
        for slot in [
            BodySlot::Head,
            BodySlot::Forearm,
            BodySlot::Back,
            BodySlot::Torso,
            BodySlot::Lower,
        ] {
            let mut plan = base_plan();
            plan.mutation_slot_mapping.insert(slot, "ghost_part".into());
            let err =
                validate_body_plan(&plan).expect_err(&format!("{slot:?} 映射到悬空部位必须被拒绝"));
            assert!(
                err.contains("mutation_slot_mapping"),
                "slot={slot:?}: 错误消息应带上 mutation_slot_mapping 定位信息，got: {err}"
            );
        }
    }

    /// 每个 `BodySlot` 变体各自专属 case：映射到已声明部位（`base_plan()` 的 `head`）
    /// 时必须通过校验——同样逐变体覆盖，不假设"Head 通过了其余变体也一定通过"。
    #[test]
    fn mutation_slot_mapping_valid_reference_accepted_for_every_body_slot_variant() {
        use crate::dandao::mutation::BodySlot;
        for slot in [
            BodySlot::Head,
            BodySlot::Forearm,
            BodySlot::Back,
            BodySlot::Torso,
            BodySlot::Lower,
        ] {
            let mut plan = base_plan();
            plan.mutation_slot_mapping.insert(slot, "head".into());
            assert!(
                validate_body_plan(&plan).is_ok(),
                "slot={slot:?}: 映射到已声明部位应通过校验"
            );
        }
    }

    /// 缺失映射契约：`mutation_slot_mapping` 不要求覆盖全部 5 个 `BodySlot` 变体——
    /// 只声明部分变体（其余变体查询走 `body_part_for_mutation_slot` 返回 `None`，
    /// 见 `resolve.rs` 对应测试）本身是合法状态，不是校验错误。
    #[test]
    fn mutation_slot_mapping_partial_coverage_is_valid() {
        use crate::dandao::mutation::BodySlot;
        let mut plan = base_plan();
        plan.mutation_slot_mapping
            .insert(BodySlot::Head, "head".into());
        // 故意不声明 Forearm/Back/Torso/Lower——部分映射对非人形构型是正常状态。
        assert!(
            validate_body_plan(&plan).is_ok(),
            "只声明部分 BodySlot 变体的映射必须合法（不要求全变体覆盖）"
        );
        assert_eq!(plan.mutation_slot_mapping.len(), 1);
    }

    /// 全部 5 个 `BodySlot` 变体同时映射到不同部位——多键场景下悬空检测必须逐一生效，
    /// 不因为其余键合法就漏检其中一个悬空键。
    #[test]
    fn mutation_slot_mapping_all_five_variants_mapped_with_one_dangling_still_rejected() {
        use crate::dandao::mutation::BodySlot;
        let mut plan = base_plan();
        plan.mutation_slot_mapping
            .insert(BodySlot::Head, "head".into());
        plan.mutation_slot_mapping
            .insert(BodySlot::Forearm, "chest".into());
        plan.mutation_slot_mapping
            .insert(BodySlot::Back, "chest".into());
        plan.mutation_slot_mapping
            .insert(BodySlot::Torso, "chest".into());
        // Lower 映射到悬空部位——即便其余 4 个变体都合法，也必须整体拒绝。
        plan.mutation_slot_mapping
            .insert(BodySlot::Lower, "ghost_part".into());
        let err = validate_body_plan(&plan).unwrap_err();
        assert!(err.contains("mutation_slot_mapping"), "got: {err}");
    }
}
