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

/// plan-race-system-v1 P2a — `BodyPlanLayoutV1` 的跨 registry 校验（需要同时持有目标
/// `BodyPlan`，归属与 [`race_registry::RaceRegistry::load_file`] 的跨 registry 校验同一
/// 设计：`layout::BodyPlanLayoutRegistry::load_dir`/`layout::humanoid_layout_static` 是
/// 唯一调用方）。
///
/// 校验范围（**不**要求 `part_display_map.display_segment_id` 反向存在于 `silhouette`
/// ——展示段 id 是一个符号性字符串，允许没有专属剪影多边形，例如人形 `back` 目前没有
/// 独立于 `chest` 的可视区块，这是刻意的最小校验面，不是遗漏）：
/// - `silhouette`：非空、`part_id` 非空且唯一、多边形至少 3 个顶点、坐标落在 `[0,1]`
/// - `anchors`：`part_id` 非空且唯一、坐标落在 `[0,1]`
/// - `meridian_paths`：`channel_id` 非空且唯一、至少 2 个点、坐标落在 `[0,1]`、
///   引用的 channel id 必须存在于 `plan.meridian_profile.channels`（`plan` 未声明
///   `meridian_profile` 时任何 `meridian_paths` 条目都是悬空引用）
/// - `part_display_map`：`server_part_id`/`display_segment_id` 均非空、
///   `server_part_id` 唯一、`server_part_id` 必须存在于 `plan.parts`
pub fn validate_body_plan_layout(
    layout: &crate::schema::server_data::BodyPlanLayoutV1,
    plan: &BodyPlan,
) -> Result<(), String> {
    if layout.body_plan_id != plan.id.as_str() {
        return Err(format!(
            "body plan layout body_plan_id {} does not match target body plan id {}",
            layout.body_plan_id, plan.id
        ));
    }
    if layout.silhouette.is_empty() {
        return Err(format!(
            "body plan layout {} must declare at least one silhouette part",
            layout.body_plan_id
        ));
    }

    fn point_in_unit_range(x: f64, y: f64) -> bool {
        x.is_finite() && y.is_finite() && (0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y)
    }

    let mut silhouette_ids: HashSet<String> = HashSet::new();
    for part in &layout.silhouette {
        if part.part_id.trim().is_empty() {
            return Err(format!(
                "body plan layout {} has a silhouette part with an empty part_id",
                layout.body_plan_id
            ));
        }
        if part.polygon.len() < 3 {
            return Err(format!(
                "body plan layout {} silhouette part {} must declare at least 3 polygon vertices, got {}",
                layout.body_plan_id, part.part_id, part.polygon.len()
            ));
        }
        for p in &part.polygon {
            if !point_in_unit_range(p.x, p.y) {
                return Err(format!(
                    "body plan layout {} silhouette part {} has an out-of-range vertex ({}, {}) — coordinates must be normalized to [0,1]",
                    layout.body_plan_id, part.part_id, p.x, p.y
                ));
            }
        }
        if !silhouette_ids.insert(part.part_id.clone()) {
            return Err(format!(
                "body plan layout {} has duplicate silhouette part_id {}",
                layout.body_plan_id, part.part_id
            ));
        }
    }

    let mut anchor_ids: HashSet<String> = HashSet::new();
    for anchor in &layout.anchors {
        if anchor.part_id.trim().is_empty() {
            return Err(format!(
                "body plan layout {} has an anchor with an empty part_id",
                layout.body_plan_id
            ));
        }
        if !point_in_unit_range(anchor.point.x, anchor.point.y) {
            return Err(format!(
                "body plan layout {} anchor {} has an out-of-range point ({}, {}) — coordinates must be normalized to [0,1]",
                layout.body_plan_id, anchor.part_id, anchor.point.x, anchor.point.y
            ));
        }
        if !anchor_ids.insert(anchor.part_id.clone()) {
            return Err(format!(
                "body plan layout {} has duplicate anchor for part_id {}",
                layout.body_plan_id, anchor.part_id
            ));
        }
    }

    // plan-race-system-v1 P2 major 修复 — `hud_anchors` 是可选的第二套锚点组（mini
    // HUD 专用画布比例），校验规则与 `anchors` 完全对称（同样允许留空，同样不要求
    // 覆盖 plan.parts 全集——非人形构型可以只给部分部位配 mini HUD 锚点）。
    let mut hud_anchor_ids: HashSet<String> = HashSet::new();
    for anchor in &layout.hud_anchors {
        if anchor.part_id.trim().is_empty() {
            return Err(format!(
                "body plan layout {} has a hud_anchor with an empty part_id",
                layout.body_plan_id
            ));
        }
        if !point_in_unit_range(anchor.point.x, anchor.point.y) {
            return Err(format!(
                "body plan layout {} hud_anchor {} has an out-of-range point ({}, {}) — coordinates must be normalized to [0,1]",
                layout.body_plan_id, anchor.part_id, anchor.point.x, anchor.point.y
            ));
        }
        if !hud_anchor_ids.insert(anchor.part_id.clone()) {
            return Err(format!(
                "body plan layout {} has duplicate hud_anchor for part_id {}",
                layout.body_plan_id, anchor.part_id
            ));
        }
    }

    let plan_channel_ids: Option<HashSet<&str>> = plan
        .meridian_profile
        .as_ref()
        .map(|profile| profile.channels.iter().map(|c| c.id.as_str()).collect());
    let mut seen_channels: HashSet<String> = HashSet::new();
    for mp in &layout.meridian_paths {
        if mp.channel_id.trim().is_empty() {
            return Err(format!(
                "body plan layout {} has a meridian path with an empty channel_id",
                layout.body_plan_id
            ));
        }
        match &plan_channel_ids {
            Some(ids) => {
                if !ids.contains(mp.channel_id.as_str()) {
                    return Err(format!(
                        "body plan layout {} meridian_paths references unknown channel id {} \
                         (not declared in body plan {} meridian_profile)",
                        layout.body_plan_id, mp.channel_id, plan.id
                    ));
                }
            }
            None => {
                return Err(format!(
                    "body plan layout {} declares meridian_paths for channel {} but body plan {} \
                     has no meridian_profile at all",
                    layout.body_plan_id, mp.channel_id, plan.id
                ));
            }
        }
        if mp.points.len() < 2 {
            return Err(format!(
                "body plan layout {} meridian path {} must declare at least 2 points, got {}",
                layout.body_plan_id,
                mp.channel_id,
                mp.points.len()
            ));
        }
        for p in &mp.points {
            if !point_in_unit_range(p.x, p.y) {
                return Err(format!(
                    "body plan layout {} meridian path {} has an out-of-range point ({}, {}) — coordinates must be normalized to [0,1]",
                    layout.body_plan_id, mp.channel_id, p.x, p.y
                ));
            }
        }
        if !seen_channels.insert(mp.channel_id.clone()) {
            return Err(format!(
                "body plan layout {} has duplicate meridian_paths entry for channel {}",
                layout.body_plan_id, mp.channel_id
            ));
        }
    }

    let plan_part_ids: HashSet<&str> = plan.parts.iter().map(|p| p.id.as_str()).collect();
    let mut seen_server_parts: HashSet<String> = HashSet::new();
    for mapping in &layout.part_display_map {
        if mapping.server_part_id.trim().is_empty() || mapping.display_segment_id.trim().is_empty()
        {
            return Err(format!(
                "body plan layout {} has a part_display_map entry with an empty server_part_id or display_segment_id",
                layout.body_plan_id
            ));
        }
        if !plan_part_ids.contains(mapping.server_part_id.as_str()) {
            return Err(format!(
                "body plan layout {} part_display_map references unknown server part id {} \
                 (not declared on body plan {})",
                layout.body_plan_id, mapping.server_part_id, plan.id
            ));
        }
        if !seen_server_parts.insert(mapping.server_part_id.clone()) {
            return Err(format!(
                "body plan layout {} has duplicate part_display_map entry for server part {}",
                layout.body_plan_id, mapping.server_part_id
            ));
        }
    }

    Ok(())
}
