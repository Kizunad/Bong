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

use super::types::{BodyPartId, BodyPlan, HitGeometry, MeridianProfile, PartBox};

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

    // plan-race-system-v1 P1a：is_humanoid plan 的 meridian_profile 从 P0 的"可选"转为
    // "必填"（§P1 决议——humanoid.json 缺该字段不再合法）。非 humanoid plan（P0 现存
    // fixture / 未来非战斗构型）仍可留 `None`。
    match (&plan.meridian_profile, plan.is_humanoid) {
        (None, true) => {
            return Err(format!(
                "body plan {} is_humanoid=true but declares no meridian_profile (mandatory from \
                 plan-race-system-v1 P1 onward)",
                plan.id
            ));
        }
        (Some(profile), _) => validate_meridian_profile(plan, profile)?,
        (None, false) => {}
    }

    Ok(())
}

fn validate_meridian_profile(plan: &BodyPlan, profile: &MeridianProfile) -> Result<(), String> {
    use super::types::MeridianFamily;
    use crate::cultivation::components::MeridianChannelId;

    if profile.channels.is_empty() {
        return Err(format!(
            "body plan {} meridian_profile.channels must declare at least one channel",
            plan.id
        ));
    }

    let part_ids: HashSet<&BodyPartId> = plan.parts.iter().map(|p| &p.id).collect();
    let mut channel_ids: HashSet<MeridianChannelId> = HashSet::new();
    let mut regular_count = 0usize;
    let mut extraordinary_count = 0usize;
    for channel in &profile.channels {
        if channel.id.as_str().trim().is_empty() {
            return Err(format!(
                "body plan {} meridian_profile has a channel with an empty id",
                plan.id
            ));
        }
        if !channel_ids.insert(channel.id.clone()) {
            return Err(format!(
                "body plan {} meridian_profile has duplicate channel id {}",
                plan.id, channel.id
            ));
        }
        match channel.family {
            MeridianFamily::Regular => regular_count += 1,
            MeridianFamily::Extraordinary => extraordinary_count += 1,
        }
        if let Some(body_part) = &channel.body_part {
            if !part_ids.contains(body_part) {
                return Err(format!(
                    "body plan {} meridian_profile channel {} references unknown body_part {}",
                    plan.id, channel.id, body_part
                ));
            }
        }
    }

    for edge in &profile.topology_edges {
        if !channel_ids.contains(&edge.from) {
            return Err(format!(
                "body plan {} meridian_profile.topology_edges references unknown channel id {} \
                 (from)",
                plan.id, edge.from
            ));
        }
        if !channel_ids.contains(&edge.to) {
            return Err(format!(
                "body plan {} meridian_profile.topology_edges references unknown channel id {} \
                 (to)",
                plan.id, edge.to
            ));
        }
    }

    let total_channels = profile.channels.len();
    let mut previous_total: Option<u8> = None;
    for (index, req) in profile.realm_requirements.iter().enumerate() {
        if let Some(previous) = previous_total {
            if req.total < previous {
                return Err(format!(
                    "body plan {} meridian_profile.realm_requirements must be monotonically \
                     non-decreasing by total (index {index}: {} < previous {previous})",
                    plan.id, req.total
                ));
            }
        }
        previous_total = Some(req.total);
        if req.total as usize > total_channels {
            return Err(format!(
                "body plan {} meridian_profile.realm_requirements[{index}].total={} exceeds \
                 declared channel count {total_channels}",
                plan.id, req.total
            ));
        }
        if req.regular_min as usize > regular_count {
            return Err(format!(
                "body plan {} meridian_profile.realm_requirements[{index}].regular_min={} \
                 exceeds declared regular channel count {regular_count}",
                plan.id, req.regular_min
            ));
        }
        if req.extraordinary_min as usize > extraordinary_count {
            return Err(format!(
                "body plan {} meridian_profile.realm_requirements[{index}].extraordinary_min={} \
                 exceeds declared extraordinary channel count {extraordinary_count}",
                plan.id, req.extraordinary_min
            ));
        }
        if (req.regular_min as usize + req.extraordinary_min as usize) > req.total as usize {
            return Err(format!(
                "body plan {} meridian_profile.realm_requirements[{index}] regular_min+\
                 extraordinary_min ({}+{}) exceeds total ({})",
                plan.id, req.regular_min, req.extraordinary_min, req.total
            ));
        }
    }

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
            meridian_profile: Some(base_meridian_profile()),
            mutation_slot_mapping: HashMap::new(),
        }
    }

    /// 最小合法 `MeridianProfile`——1 条 Regular channel（映射到 `base_plan()` 的
    /// `head` 部位）+ 六境界配额全为 1/1/0（`base_plan()` 是 `is_humanoid: true`，
    /// P1a 起该字段必填，见 `validate_body_plan`）。
    fn base_meridian_profile() -> crate::body_plan::types::MeridianProfile {
        use crate::body_plan::types::{ChannelDef, MeridianFamily, RealmMeridianReq};
        crate::body_plan::types::MeridianProfile {
            channels: vec![ChannelDef {
                id: "lung".into(),
                family: MeridianFamily::Regular,
                body_part: Some(BodyPartId::new("head")),
                roles: vec![],
            }],
            topology_edges: vec![],
            realm_requirements: [RealmMeridianReq {
                total: 1,
                regular_min: 1,
                extraordinary_min: 0,
            }; 6],
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

    // ───────────────────── meridian_profile（plan-race-system-v1 P1a） ─────────────────────

    use crate::body_plan::types::{ChannelDef, MeridianFamily, RealmMeridianReq, TopologyEdge};

    #[test]
    fn is_humanoid_true_without_meridian_profile_rejected() {
        let mut plan = base_plan();
        plan.meridian_profile = None;
        let err = validate_body_plan(&plan).unwrap_err();
        assert!(
            err.contains("meridian_profile"),
            "is_humanoid=true 缺 meridian_profile 必须被拒绝，got: {err}"
        );
    }

    #[test]
    fn is_humanoid_false_without_meridian_profile_is_valid() {
        let mut plan = base_plan();
        plan.is_humanoid = false;
        plan.meridian_profile = None;
        assert!(
            validate_body_plan(&plan).is_ok(),
            "非 humanoid plan 缺 meridian_profile 应合法（P0 现存行为不应回归）"
        );
    }

    #[test]
    fn meridian_profile_happy_path_with_full_humanoid_style_data_passes() {
        let mut plan = base_plan();
        plan.meridian_profile = Some(crate::body_plan::types::MeridianProfile {
            channels: vec![
                ChannelDef {
                    id: "lung".into(),
                    family: MeridianFamily::Regular,
                    body_part: Some(BodyPartId::new("head")),
                    roles: vec![],
                },
                ChannelDef {
                    id: "ren".into(),
                    family: MeridianFamily::Extraordinary,
                    body_part: Some(BodyPartId::new("chest")),
                    roles: vec![crate::body_plan::types::ChannelRole::FormAnchor],
                },
            ],
            topology_edges: vec![TopologyEdge {
                from: "lung".into(),
                to: "ren".into(),
            }],
            realm_requirements: [RealmMeridianReq {
                total: 2,
                regular_min: 1,
                extraordinary_min: 1,
            }; 6],
        });
        assert!(validate_body_plan(&plan).is_ok());
    }

    #[test]
    fn meridian_profile_empty_channels_rejected() {
        let mut plan = base_plan();
        plan.meridian_profile = Some(crate::body_plan::types::MeridianProfile {
            channels: vec![],
            topology_edges: vec![],
            realm_requirements: [RealmMeridianReq::default(); 6],
        });
        let err = validate_body_plan(&plan).unwrap_err();
        assert!(err.contains("at least one channel"), "got: {err}");
    }

    #[test]
    fn meridian_profile_duplicate_channel_id_rejected() {
        let mut plan = base_plan();
        let mut profile = base_meridian_profile();
        let dup = profile.channels[0].clone();
        profile.channels.push(dup);
        plan.meridian_profile = Some(profile);
        let err = validate_body_plan(&plan).unwrap_err();
        assert!(err.contains("duplicate channel id"), "got: {err}");
    }

    #[test]
    fn meridian_profile_empty_channel_id_rejected() {
        let mut plan = base_plan();
        let mut profile = base_meridian_profile();
        profile.channels[0].id = "".into();
        plan.meridian_profile = Some(profile);
        let err = validate_body_plan(&plan).unwrap_err();
        assert!(err.contains("empty id"), "got: {err}");
    }

    #[test]
    fn meridian_profile_dangling_body_part_rejected() {
        let mut plan = base_plan();
        let mut profile = base_meridian_profile();
        profile.channels[0].body_part = Some(BodyPartId::new("ghost_part"));
        plan.meridian_profile = Some(profile);
        let err = validate_body_plan(&plan).unwrap_err();
        assert!(err.contains("unknown body_part"), "got: {err}");
    }

    #[test]
    fn meridian_profile_none_body_part_is_valid() {
        let mut plan = base_plan();
        let mut profile = base_meridian_profile();
        profile.channels[0].body_part = None;
        plan.meridian_profile = Some(profile);
        assert!(
            validate_body_plan(&plan).is_ok(),
            "body_part=None（如 6 条排除的奇经）必须合法"
        );
    }

    #[test]
    fn meridian_profile_topology_edge_dangling_from_rejected() {
        let mut plan = base_plan();
        let mut profile = base_meridian_profile();
        profile.topology_edges.push(TopologyEdge {
            from: "ghost_channel".into(),
            to: "lung".into(),
        });
        plan.meridian_profile = Some(profile);
        let err = validate_body_plan(&plan).unwrap_err();
        assert!(err.contains("unknown channel id"), "got: {err}");
    }

    #[test]
    fn meridian_profile_topology_edge_dangling_to_rejected() {
        let mut plan = base_plan();
        let mut profile = base_meridian_profile();
        profile.topology_edges.push(TopologyEdge {
            from: "lung".into(),
            to: "ghost_channel".into(),
        });
        plan.meridian_profile = Some(profile);
        let err = validate_body_plan(&plan).unwrap_err();
        assert!(err.contains("unknown channel id"), "got: {err}");
    }

    #[test]
    fn meridian_profile_non_monotonic_realm_requirements_rejected() {
        let mut plan = base_plan();
        let mut profile = base_meridian_profile();
        profile.realm_requirements[1] = RealmMeridianReq {
            total: 0,
            regular_min: 0,
            extraordinary_min: 0,
        };
        plan.meridian_profile = Some(profile);
        let err = validate_body_plan(&plan).unwrap_err();
        assert!(err.contains("monotonically"), "got: {err}");
    }

    #[test]
    fn meridian_profile_realm_requirements_total_exceeds_channel_count_rejected() {
        let mut plan = base_plan();
        let mut profile = base_meridian_profile();
        profile.realm_requirements[5].total = 5; // 只声明了 1 条 channel
        plan.meridian_profile = Some(profile);
        let err = validate_body_plan(&plan).unwrap_err();
        assert!(err.contains("exceeds declared channel count"), "got: {err}");
    }

    #[test]
    fn meridian_profile_realm_requirements_regular_min_exceeds_regular_count_rejected() {
        let mut plan = base_plan();
        let mut profile = base_meridian_profile(); // 1 条 Regular channel
        profile.realm_requirements[0].regular_min = 2;
        plan.meridian_profile = Some(profile);
        let err = validate_body_plan(&plan).unwrap_err();
        assert!(
            err.contains("exceeds declared regular channel count"),
            "got: {err}"
        );
    }

    #[test]
    fn meridian_profile_realm_requirements_extraordinary_min_exceeds_extraordinary_count_rejected()
    {
        let mut plan = base_plan();
        let mut profile = base_meridian_profile(); // 0 条 Extraordinary channel
        profile.realm_requirements[0].extraordinary_min = 1;
        plan.meridian_profile = Some(profile);
        let err = validate_body_plan(&plan).unwrap_err();
        assert!(
            err.contains("exceeds declared extraordinary channel count"),
            "got: {err}"
        );
    }

    #[test]
    fn meridian_profile_realm_requirements_sub_quota_sum_exceeds_total_rejected() {
        let mut plan = base_plan();
        // 2 条 Regular + 1 条 Extraordinary，保证 regular_min=2/extraordinary_min=1
        // 各自都不超过声明的 channel 数（不触发前两条更早的校验），专门孤立测试
        // "sum 超过 total" 这一条。
        let mut profile = crate::body_plan::types::MeridianProfile {
            channels: vec![
                ChannelDef {
                    id: "lung".into(),
                    family: MeridianFamily::Regular,
                    body_part: Some(BodyPartId::new("head")),
                    roles: vec![],
                },
                ChannelDef {
                    id: "heart".into(),
                    family: MeridianFamily::Regular,
                    body_part: Some(BodyPartId::new("chest")),
                    roles: vec![],
                },
                ChannelDef {
                    id: "ren".into(),
                    family: MeridianFamily::Extraordinary,
                    body_part: Some(BodyPartId::new("chest")),
                    roles: vec![],
                },
            ],
            topology_edges: vec![],
            realm_requirements: [RealmMeridianReq {
                total: 2,
                regular_min: 1,
                extraordinary_min: 1,
            }; 6],
        };
        // total=2 但 regular_min+extraordinary_min=1+1=2，恰好等于 total（应通过）；
        // 提高 regular_min 到 2（仍 <= 声明的 2 条 Regular，不触发 regular_min 越界）
        // 使 sum=2+1=3 > total=2（专门触发"sum 超过 total"）。
        profile.realm_requirements[3].regular_min = 2;
        plan.meridian_profile = Some(profile);
        let err = validate_body_plan(&plan).unwrap_err();
        assert!(err.contains("exceeds total"), "got: {err}");
    }

    #[test]
    fn meridian_profile_realm_requirements_equal_totals_across_realms_is_valid() {
        // 单调性要求"非递减"，允许相邻境界 total 相等（不要求严格递增）。
        let mut plan = base_plan();
        let profile = base_meridian_profile(); // 全部 6 项 total=1，天然相等
        plan.meridian_profile = Some(profile);
        assert!(validate_body_plan(&plan).is_ok());
    }
}
