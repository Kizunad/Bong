use bong_server::body_plan::types::{
    BodyPartDef, BodyPlanId, HeightBand, HeightBandAssignment, PartConsequence, StandingAabbSpec,
};
use bong_server::body_plan::types::{BodyPartId, BodyPlan, HitGeometry, PartBox};
use bong_server::body_plan::validate::{validate_body_plan, validate_body_plan_layout};
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
fn base_meridian_profile() -> bong_server::body_plan::types::MeridianProfile {
    use bong_server::body_plan::types::{ChannelDef, MeridianFamily, RealmMeridianReq};
    bong_server::body_plan::types::MeridianProfile {
        channels: vec![ChannelDef {
            id: "lung".into(),
            family: MeridianFamily::Regular,
            body_part: Some(BodyPartId::new("head")),
            roles: vec![],
        }],
        topology_edges: vec![],
        dugu_injection: vec![],
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
    use bong_server::dandao::mutation::BodySlot;
    let mut plan = base_plan();
    plan.mutation_slot_mapping
        .insert(BodySlot::Head, "ghost_part".into());
    assert!(validate_body_plan(&plan)
        .unwrap_err()
        .contains("mutation_slot_mapping"));
}

#[test]
fn mutation_slot_mapping_valid_reference_accepted() {
    use bong_server::dandao::mutation::BodySlot;
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
    use bong_server::dandao::mutation::BodySlot;
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
    use bong_server::dandao::mutation::BodySlot;
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
    use bong_server::dandao::mutation::BodySlot;
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
    use bong_server::dandao::mutation::BodySlot;
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

use bong_server::body_plan::types::{ChannelDef, MeridianFamily, RealmMeridianReq, TopologyEdge};

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
    plan.meridian_profile = Some(bong_server::body_plan::types::MeridianProfile {
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
                roles: vec![bong_server::body_plan::types::ChannelRole::FormAnchor],
            },
        ],
        topology_edges: vec![TopologyEdge {
            from: "lung".into(),
            to: "ren".into(),
        }],
        dugu_injection: vec![],
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
    plan.meridian_profile = Some(bong_server::body_plan::types::MeridianProfile {
        channels: vec![],
        topology_edges: vec![],
        dugu_injection: vec![],
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
fn meridian_profile_realm_requirements_extraordinary_min_exceeds_extraordinary_count_rejected() {
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
    let mut profile = bong_server::body_plan::types::MeridianProfile {
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
        dugu_injection: vec![],
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

// ───────────────────────── validate_body_plan_layout ─────────────────────────

mod layout_tests {
    use super::*;
    use bong_server::schema::server_data::{
        BodyPlanLayoutV1, BodyPlanMeridianPathV1, BodyPlanPartAnchorV1,
        BodyPlanPartDisplayMappingV1, BodyPlanPoint2V1, BodyPlanSilhouettePartV1,
    };

    fn p(x: f64, y: f64) -> BodyPlanPoint2V1 {
        BodyPlanPoint2V1 { x, y }
    }

    fn happy_layout() -> BodyPlanLayoutV1 {
        BodyPlanLayoutV1 {
            body_plan_id: "fixture".to_string(),
            silhouette: vec![BodyPlanSilhouettePartV1 {
                part_id: "chest".to_string(),
                polygon: vec![p(0.3, 0.1), p(0.7, 0.1), p(0.7, 0.3), p(0.3, 0.3)],
            }],
            anchors: vec![BodyPlanPartAnchorV1 {
                part_id: "chest".to_string(),
                point: p(0.5, 0.2),
            }],
            meridian_paths: vec![BodyPlanMeridianPathV1 {
                channel_id: "lung".to_string(),
                points: vec![p(0.4, 0.2), p(0.4, 0.4)],
            }],
            part_display_map: vec![BodyPlanPartDisplayMappingV1 {
                server_part_id: "chest".to_string(),
                display_segment_id: "chest".to_string(),
            }],
            hud_anchors: vec![BodyPlanPartAnchorV1 {
                part_id: "chest".to_string(),
                point: p(0.5, 0.25),
            }],
        }
    }

    fn plan_with_lung_channel() -> BodyPlan {
        // 外层 `base_plan()` 已声明 parts head/chest + meridian_profile 含
        // channel "lung"（映射到 head），本 fixture 只需改 id 与 happy_layout 对齐。
        let mut plan = base_plan();
        plan.id = "fixture".into();
        plan
    }

    #[test]
    fn happy_path_passes() {
        let plan = plan_with_lung_channel();
        assert!(validate_body_plan_layout(&happy_layout(), &plan).is_ok());
    }

    #[test]
    fn real_humanoid_layout_passes_against_real_humanoid_plan() {
        // 与磁盘上真实 layouts/humanoid.json + plans/humanoid.json 对拍。
        let layout_json = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("assets/body_plans/layouts/humanoid.json"),
        )
        .expect("real humanoid layout json should exist");
        let layout: BodyPlanLayoutV1 =
            serde_json::from_str(&layout_json).expect("real humanoid layout should parse");
        let plan = bong_server::body_plan::humanoid_plan_static();
        assert!(
            validate_body_plan_layout(&layout, plan).is_ok(),
            "real humanoid.json layout must validate against real humanoid.json plan"
        );
    }

    #[test]
    fn body_plan_id_mismatch_rejected() {
        let mut layout = happy_layout();
        layout.body_plan_id = "other".to_string();
        let err = validate_body_plan_layout(&layout, &plan_with_lung_channel()).unwrap_err();
        assert!(
            err.contains("does not match target body plan id"),
            "got: {err}"
        );
    }

    #[test]
    fn empty_silhouette_rejected() {
        let mut layout = happy_layout();
        layout.silhouette.clear();
        let err = validate_body_plan_layout(&layout, &plan_with_lung_channel()).unwrap_err();
        assert!(err.contains("at least one silhouette part"), "got: {err}");
    }

    #[test]
    fn silhouette_empty_part_id_rejected() {
        let mut layout = happy_layout();
        layout.silhouette[0].part_id = String::new();
        let err = validate_body_plan_layout(&layout, &plan_with_lung_channel()).unwrap_err();
        assert!(err.contains("empty part_id"), "got: {err}");
    }

    #[test]
    fn silhouette_polygon_with_two_vertices_rejected() {
        let mut layout = happy_layout();
        layout.silhouette[0].polygon = vec![p(0.3, 0.1), p(0.7, 0.1)];
        let err = validate_body_plan_layout(&layout, &plan_with_lung_channel()).unwrap_err();
        assert!(err.contains("at least 3 polygon vertices"), "got: {err}");
    }

    #[test]
    fn silhouette_out_of_range_vertex_rejected() {
        let mut layout = happy_layout();
        layout.silhouette[0].polygon[0] = p(1.2, 0.1);
        let err = validate_body_plan_layout(&layout, &plan_with_lung_channel()).unwrap_err();
        assert!(err.contains("out-of-range vertex"), "got: {err}");
    }

    #[test]
    fn duplicate_silhouette_part_id_rejected() {
        let mut layout = happy_layout();
        let dup = layout.silhouette[0].clone();
        layout.silhouette.push(dup);
        let err = validate_body_plan_layout(&layout, &plan_with_lung_channel()).unwrap_err();
        assert!(err.contains("duplicate silhouette part_id"), "got: {err}");
    }

    #[test]
    fn anchor_empty_part_id_rejected() {
        let mut layout = happy_layout();
        layout.anchors[0].part_id = String::new();
        let err = validate_body_plan_layout(&layout, &plan_with_lung_channel()).unwrap_err();
        assert!(err.contains("anchor with an empty part_id"), "got: {err}");
    }

    #[test]
    fn anchor_out_of_range_point_rejected() {
        let mut layout = happy_layout();
        layout.anchors[0].point = p(-0.1, 0.2);
        let err = validate_body_plan_layout(&layout, &plan_with_lung_channel()).unwrap_err();
        assert!(err.contains("out-of-range point"), "got: {err}");
    }

    #[test]
    fn duplicate_anchor_part_id_rejected() {
        let mut layout = happy_layout();
        let dup = layout.anchors[0].clone();
        layout.anchors.push(dup);
        let err = validate_body_plan_layout(&layout, &plan_with_lung_channel()).unwrap_err();
        assert!(err.contains("duplicate anchor"), "got: {err}");
    }

    #[test]
    fn hud_anchor_empty_layout_is_valid() {
        // hud_anchors 是可选的第二锚点组——空列表必须合法（未来非人 plan 不配的常态）。
        let mut layout = happy_layout();
        layout.hud_anchors.clear();
        assert!(validate_body_plan_layout(&layout, &plan_with_lung_channel()).is_ok());
    }

    #[test]
    fn hud_anchor_empty_part_id_rejected() {
        let mut layout = happy_layout();
        layout.hud_anchors[0].part_id = String::new();
        let err = validate_body_plan_layout(&layout, &plan_with_lung_channel()).unwrap_err();
        assert!(
            err.contains("hud_anchor with an empty part_id"),
            "got: {err}"
        );
    }

    #[test]
    fn hud_anchor_out_of_range_point_rejected() {
        let mut layout = happy_layout();
        layout.hud_anchors[0].point = p(1.5, 0.2);
        let err = validate_body_plan_layout(&layout, &plan_with_lung_channel()).unwrap_err();
        assert!(
            err.contains("hud_anchor") && err.contains("out-of-range point"),
            "got: {err}"
        );
    }

    #[test]
    fn duplicate_hud_anchor_part_id_rejected() {
        let mut layout = happy_layout();
        let dup = layout.hud_anchors[0].clone();
        layout.hud_anchors.push(dup);
        let err = validate_body_plan_layout(&layout, &plan_with_lung_channel()).unwrap_err();
        assert!(err.contains("duplicate hud_anchor"), "got: {err}");
    }

    #[test]
    fn meridian_path_empty_channel_id_rejected() {
        let mut layout = happy_layout();
        layout.meridian_paths[0].channel_id = String::new();
        let err = validate_body_plan_layout(&layout, &plan_with_lung_channel()).unwrap_err();
        assert!(err.contains("empty channel_id"), "got: {err}");
    }

    #[test]
    fn meridian_path_unknown_channel_rejected_when_profile_present() {
        let mut layout = happy_layout();
        layout.meridian_paths[0].channel_id = "heart".to_string();
        let err = validate_body_plan_layout(&layout, &plan_with_lung_channel()).unwrap_err();
        assert!(err.contains("references unknown channel id"), "got: {err}");
    }

    #[test]
    fn meridian_path_rejected_when_plan_has_no_meridian_profile_at_all() {
        let layout = happy_layout();
        let mut plan = plan_with_lung_channel();
        plan.meridian_profile = None;
        let err = validate_body_plan_layout(&layout, &plan).unwrap_err();
        assert!(err.contains("has no meridian_profile at all"), "got: {err}");
    }

    #[test]
    fn meridian_path_single_point_rejected() {
        let mut layout = happy_layout();
        layout.meridian_paths[0].points = vec![p(0.4, 0.2)];
        let err = validate_body_plan_layout(&layout, &plan_with_lung_channel()).unwrap_err();
        assert!(err.contains("at least 2 points"), "got: {err}");
    }

    #[test]
    fn meridian_path_out_of_range_point_rejected() {
        let mut layout = happy_layout();
        layout.meridian_paths[0].points[0] = p(2.0, 0.2);
        let err = validate_body_plan_layout(&layout, &plan_with_lung_channel()).unwrap_err();
        assert!(err.contains("out-of-range point"), "got: {err}");
    }

    #[test]
    fn duplicate_meridian_path_channel_rejected() {
        let mut layout = happy_layout();
        let dup = layout.meridian_paths[0].clone();
        layout.meridian_paths.push(dup);
        let err = validate_body_plan_layout(&layout, &plan_with_lung_channel()).unwrap_err();
        assert!(err.contains("duplicate meridian_paths entry"), "got: {err}");
    }

    #[test]
    fn part_display_map_empty_server_part_id_rejected() {
        let mut layout = happy_layout();
        layout.part_display_map[0].server_part_id = String::new();
        let err = validate_body_plan_layout(&layout, &plan_with_lung_channel()).unwrap_err();
        assert!(err.contains("empty server_part_id"), "got: {err}");
    }

    #[test]
    fn part_display_map_unknown_server_part_rejected() {
        let mut layout = happy_layout();
        layout.part_display_map[0].server_part_id = "does_not_exist".to_string();
        let err = validate_body_plan_layout(&layout, &plan_with_lung_channel()).unwrap_err();
        assert!(
            err.contains("references unknown server part id"),
            "got: {err}"
        );
    }

    #[test]
    fn duplicate_part_display_map_server_part_rejected() {
        let mut layout = happy_layout();
        let dup = layout.part_display_map[0].clone();
        layout.part_display_map.push(dup);
        let err = validate_body_plan_layout(&layout, &plan_with_lung_channel()).unwrap_err();
        assert!(
            err.contains("duplicate part_display_map entry"),
            "got: {err}"
        );
    }

    #[test]
    fn part_display_map_display_segment_id_need_not_exist_in_silhouette() {
        // 刻意的最小校验面：display_segment_id 是符号性字符串，不要求反向存在于
        // silhouette（见函数文档"人形 back 目前没有独立于 chest 的可视区块"）。
        let mut layout = happy_layout();
        layout.part_display_map[0].display_segment_id = "back".to_string();
        assert!(validate_body_plan_layout(&layout, &plan_with_lung_channel()).is_ok());
    }
}
