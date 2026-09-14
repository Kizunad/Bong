#![allow(dead_code, unused_imports)]
use bong_server::body_plan::types::*;
use serde::Deserialize;
use std::collections::HashMap;

#[test]
fn body_plan_id_display_and_accessors_round_trip() {
    let id = BodyPlanId::new("humanoid");
    assert_eq!(id.as_str(), "humanoid");
    assert_eq!(id.to_string(), "humanoid");
    assert_eq!(BodyPlanId::from("humanoid"), id);
    assert_eq!(BodyPlanId::from("humanoid".to_string()), id);
}

#[test]
fn body_part_id_display_and_accessors_round_trip() {
    let id = BodyPartId::new("head");
    assert_eq!(id.as_str(), "head");
    assert_eq!(id.to_string(), "head");
    assert_eq!(BodyPartId::from("head"), id);
}

#[test]
fn race_id_display_and_accessors_round_trip() {
    let id = RaceId::new("human");
    assert_eq!(id.as_str(), "human");
    assert_eq!(id.to_string(), "human");
    assert_eq!(RaceId::from("human".to_string()), id);
}

// ─────────────────────────────────────────────────────────────────
// RaceGate（const/&'static 形态）：三档 allows() 矩阵。
// ─────────────────────────────────────────────────────────────────

const WHALE_SPECIES: &[&str] = &["whale"];

#[test]
fn race_gate_any_allows_every_identity() {
    let gate = RaceGate::Any;
    assert!(gate.allows(&RaceId::new("human"), true));
    assert!(gate.allows(&RaceId::new("whale"), false));
}

#[test]
fn race_gate_humanoid_gates_on_is_humanoid_only() {
    let gate = RaceGate::Humanoid;
    assert!(
        gate.allows(&RaceId::new("human"), true),
        "is_humanoid=true 必须放行，不看具体 race_id"
    );
    assert!(
        gate.allows(&RaceId::new("some_other_humanoid_race"), true),
        "Humanoid 档判据只看 is_humanoid，任何人形构型种族均放行——非种族名单硬编码"
    );
    assert!(
        !gate.allows(&RaceId::new("whale"), false),
        "is_humanoid=false 必须拒绝，即便 race_id 未知"
    );
}

#[test]
fn race_gate_species_matches_exact_allowlist_only() {
    let gate = RaceGate::Species(WHALE_SPECIES);
    assert!(gate.allows(&RaceId::new("whale"), false));
    assert!(
        !gate.allows(&RaceId::new("human"), true),
        "Species 档不因 is_humanoid=true 放行——精确名单以外一律拒绝"
    );
    assert!(!gate.allows(&RaceId::new("beast_common"), false));
}

#[test]
fn race_gate_default_is_any() {
    assert_eq!(RaceGate::default(), RaceGate::Any);
}

// ─────────────────────────────────────────────────────────────────
// RaceGateOwned（serde owned 形态）：三变体正反 sample + 未知 kind fail-closed +
// 空 / 重复 species pin。
// ─────────────────────────────────────────────────────────────────

#[test]
fn race_gate_owned_serde_pin_any_variant() {
    let value = RaceGateOwned::Any;
    let json = serde_json::to_string(&value).expect("serialize");
    assert_eq!(json, r#"{"kind":"any"}"#, "Any 变体序列化形状漂移");
    let round_tripped: RaceGateOwned = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(round_tripped, value);
}

#[test]
fn race_gate_owned_serde_pin_humanoid_variant() {
    let value = RaceGateOwned::Humanoid;
    let json = serde_json::to_string(&value).expect("serialize");
    assert_eq!(
        json, r#"{"kind":"humanoid"}"#,
        "Humanoid 变体序列化形状漂移"
    );
    let round_tripped: RaceGateOwned = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(round_tripped, value);
}

#[test]
fn race_gate_owned_serde_pin_species_variant() {
    let value = RaceGateOwned::Species {
        species: vec![RaceId::new("whale")],
    };
    let json = serde_json::to_string(&value).expect("serialize");
    assert_eq!(
        json, r#"{"kind":"species","species":["whale"]}"#,
        "Species 变体序列化形状漂移"
    );
    let round_tripped: RaceGateOwned = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(round_tripped, value);
}

#[test]
fn race_gate_owned_species_empty_list_round_trips() {
    let value = RaceGateOwned::Species { species: vec![] };
    let json = serde_json::to_string(&value).expect("serialize");
    assert_eq!(json, r#"{"kind":"species","species":[]}"#);
    let round_tripped: RaceGateOwned = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(
        round_tripped, value,
        "空 species 列表是合法状态（虽无实际效果，但不得反序列化失败/被吞掉）"
    );
}

#[test]
fn race_gate_owned_species_duplicate_entries_preserved() {
    let json = r#"{"kind":"species","species":["whale","whale"]}"#;
    let value: RaceGateOwned = serde_json::from_str(json).expect("deserialize");
    match &value {
        RaceGateOwned::Species { species } => {
            assert_eq!(
                species.len(),
                2,
                "重复 species 条目原样保留，不做去重（去重是校验层的事，不是 wire 层）"
            );
        }
        other => panic!("expected Species, got {other:?}"),
    }
    let re_encoded = serde_json::to_string(&value).expect("serialize");
    assert_eq!(re_encoded, json, "重复条目往返序列化不变形");
}

#[test]
fn race_gate_owned_rejects_unknown_kind() {
    let err = serde_json::from_str::<RaceGateOwned>(r#"{"kind":"bogus"}"#)
        .expect_err("unknown kind must fail closed, not silently default to Any");
    assert!(
        err.to_string().contains("bogus") || err.to_string().contains("kind"),
        "错误信息应带上未知 kind 定位线索，实际: {err}"
    );
}

#[test]
fn race_gate_owned_species_missing_species_field_rejected() {
    // species 变体缺 species 字段——serde 内部标签枚举的必填字段校验。
    let err = serde_json::from_str::<RaceGateOwned>(r#"{"kind":"species"}"#)
        .expect_err("species kind 缺 species 字段应反序列化失败");
    assert!(err.to_string().contains("species"));
}

#[test]
fn race_gate_owned_default_is_any() {
    assert_eq!(RaceGateOwned::default(), RaceGateOwned::Any);
}

#[test]
fn race_gate_owned_allows_matches_race_gate_semantics() {
    // 两种形态（const &'static vs owned）判定逻辑必须一致——防止一处改了 allows()
    // 语义、另一处漏改的镜像漂移。
    let whale = RaceId::new("whale");
    let human = RaceId::new("human");

    assert_eq!(
        RaceGate::Any.allows(&whale, false),
        RaceGateOwned::Any.allows(&whale, false)
    );
    assert_eq!(
        RaceGate::Humanoid.allows(&human, true),
        RaceGateOwned::Humanoid.allows(&human, true)
    );
    assert_eq!(
        RaceGate::Humanoid.allows(&whale, false),
        RaceGateOwned::Humanoid.allows(&whale, false)
    );
    let species_const = RaceGate::Species(WHALE_SPECIES);
    let species_owned = RaceGateOwned::Species {
        species: vec![RaceId::new("whale")],
    };
    assert_eq!(
        species_const.allows(&whale, false),
        species_owned.allows(&whale, false)
    );
    assert_eq!(
        species_const.allows(&human, true),
        species_owned.allows(&human, true)
    );
}

/// plan-race-system-v1 P3a（§P3 身份快照 bullet）—— 两个不同 `RaceId` 共享同一
/// `BodyPlan`（即 `is_humanoid` 判定结果相同）的反例：Humanoid 档只看 `is_humanoid`
/// 放行两者，Species 档必须精确匹配 `RaceId`，共享者中不在白名单里的那个照样拒绝——
/// 证明 Species 档不会因为"底层构型相同"而被 Humanoid 档的宽松语义污染。
#[test]
fn two_race_ids_sharing_same_body_plan_diverge_under_species_gate() {
    // 模拟场景：human 与 human_variant 两个种族都挂在同一份 humanoid BodyPlan 下
    // （resolve_body_plan 返回同一个 &BodyPlan，is_humanoid=true 对两者都成立）。
    let human = RaceId::new("human");
    let human_variant = RaceId::new("human_variant");
    let shared_is_humanoid = true;

    // Humanoid 档：只看 is_humanoid，两个共享同一 BodyPlan 的种族都放行。
    let humanoid_gate = RaceGate::Humanoid;
    assert!(humanoid_gate.allows(&human, shared_is_humanoid));
    assert!(humanoid_gate.allows(&human_variant, shared_is_humanoid));

    // Species 档：即便两者 is_humanoid 相同（共享同一 BodyPlan），精确名单只认
    // "human"，human_variant 必须被拒绝——不因构型相同而放行。
    let species_gate = RaceGate::Species(&["human"]);
    assert!(species_gate.allows(&human, shared_is_humanoid));
    assert!(
        !species_gate.allows(&human_variant, shared_is_humanoid),
        "human_variant 与 human 共享同一 BodyPlan（is_humanoid 相同）也不应被 Species(\"human\") 放行"
    );

    // owned 形态同一断言（防两形态语义漂移）。
    let humanoid_owned = RaceGateOwned::Humanoid;
    assert!(humanoid_owned.allows(&human, shared_is_humanoid));
    assert!(humanoid_owned.allows(&human_variant, shared_is_humanoid));
    let species_owned = RaceGateOwned::Species {
        species: vec![RaceId::new("human")],
    };
    assert!(species_owned.allows(&human, shared_is_humanoid));
    assert!(!species_owned.allows(&human_variant, shared_is_humanoid));
}

#[test]
fn part_consequence_serde_pin_every_variant() {
    let cases: [(PartConsequence, &str); 4] = [
        (PartConsequence::Locomotion, r#"{"kind":"locomotion"}"#),
        (PartConsequence::Sensory, r#"{"kind":"sensory"}"#),
        (
            PartConsequence::Manipulator { main_hand: true },
            r#"{"kind":"manipulator","main_hand":true}"#,
        ),
        (PartConsequence::Core, r#"{"kind":"core"}"#),
    ];
    for (value, expected_json) in cases {
        let serialized = serde_json::to_string(&value).expect("serialize");
        assert_eq!(
            serialized, expected_json,
            "PartConsequence {value:?} 序列化形状漂移"
        );
        let deserialized: PartConsequence =
            serde_json::from_str(expected_json).expect("deserialize");
        assert_eq!(deserialized, value, "PartConsequence 反序列化往返不一致");
    }
}

#[test]
fn part_consequence_manipulator_main_hand_false_variant() {
    let value = PartConsequence::Manipulator { main_hand: false };
    let serialized = serde_json::to_string(&value).expect("serialize");
    assert_eq!(serialized, r#"{"kind":"manipulator","main_hand":false}"#);
}

#[test]
fn part_consequence_rejects_unknown_kind() {
    let err = serde_json::from_str::<PartConsequence>(r#"{"kind":"unknown_kind"}"#)
        .expect_err("unknown kind must fail closed, not silently default");
    assert!(err.to_string().contains("unknown_kind") || err.to_string().contains("kind"));
}

#[test]
fn height_band_assignment_referenced_part_ids_every_variant() {
    let single = HeightBandAssignment::Single {
        part: BodyPartId::new("head"),
    };
    assert_eq!(single.referenced_part_ids(), vec![&BodyPartId::new("head")]);

    let split_with_center = HeightBandAssignment::LateralSplitWithCenter {
        left: BodyPartId::new("arm_l"),
        right: BodyPartId::new("arm_r"),
        center: BodyPartId::new("chest"),
    };
    assert_eq!(
        split_with_center.referenced_part_ids(),
        vec![
            &BodyPartId::new("arm_l"),
            &BodyPartId::new("arm_r"),
            &BodyPartId::new("chest"),
        ]
    );

    let split = HeightBandAssignment::LateralSplit {
        left: BodyPartId::new("leg_l"),
        right: BodyPartId::new("leg_r"),
    };
    assert_eq!(
        split.referenced_part_ids(),
        vec![&BodyPartId::new("leg_l"), &BodyPartId::new("leg_r")]
    );
}

#[test]
fn hit_geometry_height_bands_serde_round_trip() {
    let geometry = HitGeometry::HeightBands {
        aabb: StandingAabbSpec {
            half_width: 0.3,
            height: 1.8,
        },
        bands: vec![HeightBand {
            min_rel_y: 0.88,
            assignment: HeightBandAssignment::Single {
                part: BodyPartId::new("head"),
            },
        }],
        lateral_threshold: 0.19,
    };
    let json = serde_json::to_value(&geometry).expect("serialize");
    assert_eq!(json["mode"], "height_bands");
    let round_tripped: HitGeometry = serde_json::from_value(json).expect("deserialize");
    assert_eq!(round_tripped, geometry);
}

#[test]
fn hit_geometry_part_boxes_serde_round_trip() {
    let geometry = HitGeometry::PartBoxes {
        boxes: vec![PartBox {
            part_id: BodyPartId::new("skull"),
            offset: [0.0, 1.5, 0.0],
            half_extents: [0.3, 0.3, 0.3],
            priority: 10,
        }],
    };
    let json = serde_json::to_value(&geometry).expect("serialize");
    assert_eq!(json["mode"], "part_boxes");
    let round_tripped: HitGeometry = serde_json::from_value(json).expect("deserialize");
    assert_eq!(round_tripped, geometry);
}

#[test]
fn meridian_profile_missing_field_defaults_to_none() {
    #[derive(Debug, Deserialize)]
    struct Wrapper {
        #[serde(default)]
        meridian_profile: Option<MeridianProfile>,
    }
    let missing: Wrapper =
        serde_json::from_str("{}").expect("missing field should deserialize");
    assert_eq!(missing.meridian_profile, None);
}

/// P1a 起 `MeridianProfile` 不再是空占位结构体——`channels`/`realm_requirements`
/// 是必填字段，空对象 `{}` 不再是合法内容（P0 时期曾合法，见该测试此前版本）。
#[test]
fn meridian_profile_present_but_empty_object_is_now_rejected() {
    #[derive(Debug, Deserialize)]
    struct Wrapper {
        #[serde(default)]
        meridian_profile: Option<MeridianProfile>,
    }
    let err = serde_json::from_str::<Wrapper>(r#"{"meridian_profile":{}}"#)
        .expect_err("P1a 起空对象缺少必填 channels/realm_requirements，必须拒绝而非静默默认");
    let message = err.to_string();
    assert!(
        message.contains("channels") || message.contains("missing field"),
        "错误消息应指出缺失字段，got: {message}"
    );
}

fn one_channel_profile() -> MeridianProfile {
    MeridianProfile {
        channels: vec![ChannelDef {
            id: "lung".into(),
            family: MeridianFamily::Regular,
            body_part: Some(BodyPartId::new("arm_l")),
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
fn meridian_profile_present_with_full_content_is_some() {
    #[derive(Debug, Deserialize)]
    struct Wrapper {
        #[serde(default)]
        meridian_profile: Option<MeridianProfile>,
    }
    let profile = one_channel_profile();
    let json = serde_json::json!({ "meridian_profile": profile });
    let present: Wrapper =
        serde_json::from_value(json).expect("fully populated profile should deserialize");
    assert_eq!(present.meridian_profile, Some(profile));
}

#[test]
fn meridian_family_serde_pin_both_variants() {
    assert_eq!(
        serde_json::to_string(&MeridianFamily::Regular).unwrap(),
        r#""regular""#
    );
    assert_eq!(
        serde_json::to_string(&MeridianFamily::Extraordinary).unwrap(),
        r#""extraordinary""#
    );
    assert_eq!(
        serde_json::from_str::<MeridianFamily>(r#""regular""#).unwrap(),
        MeridianFamily::Regular
    );
    assert_eq!(
        serde_json::from_str::<MeridianFamily>(r#""extraordinary""#).unwrap(),
        MeridianFamily::Extraordinary
    );
}

#[test]
fn meridian_family_rejects_unknown_variant() {
    assert!(serde_json::from_str::<MeridianFamily>(r#""mystic""#).is_err());
}

#[test]
fn channel_role_serde_pin() {
    assert_eq!(
        serde_json::to_string(&ChannelRole::FormAnchor).unwrap(),
        r#""form_anchor""#
    );
    assert_eq!(
        serde_json::from_str::<ChannelRole>(r#""form_anchor""#).unwrap(),
        ChannelRole::FormAnchor
    );
}

#[test]
fn channel_def_body_part_defaults_to_none_when_absent() {
    let json = serde_json::json!({
        "id": "chong",
        "family": "extraordinary",
    });
    let decoded: ChannelDef = serde_json::from_value(json).expect("deserialize");
    assert_eq!(decoded.body_part, None);
    assert!(decoded.roles.is_empty());
}

#[test]
fn channel_def_body_part_some_round_trips() {
    let def = ChannelDef {
        id: "ren".into(),
        family: MeridianFamily::Extraordinary,
        body_part: Some(BodyPartId::new("chest")),
        roles: vec![ChannelRole::FormAnchor],
    };
    let json = serde_json::to_value(&def).expect("serialize");
    let round_tripped: ChannelDef = serde_json::from_value(json).expect("deserialize");
    assert_eq!(round_tripped, def);
}

#[test]
fn topology_edge_serde_round_trip() {
    let edge = TopologyEdge {
        from: "lung".into(),
        to: "large_intestine".into(),
    };
    let json = serde_json::to_value(&edge).expect("serialize");
    let round_tripped: TopologyEdge = serde_json::from_value(json).expect("deserialize");
    assert_eq!(round_tripped, edge);
}

#[test]
fn realm_meridian_req_default_is_all_zero() {
    let req = RealmMeridianReq::default();
    assert_eq!(req.total, 0);
    assert_eq!(req.regular_min, 0);
    assert_eq!(req.extraordinary_min, 0);
}

#[test]
fn realm_meridian_req_sub_quota_fields_default_when_absent() {
    let json = serde_json::json!({ "total": 6 });
    let decoded: RealmMeridianReq = serde_json::from_value(json).expect("deserialize");
    assert_eq!(decoded.total, 6);
    assert_eq!(decoded.regular_min, 0);
    assert_eq!(decoded.extraordinary_min, 0);
}

#[test]
fn meridian_profile_channels_topology_realm_requirements_round_trip() {
    let profile = one_channel_profile();
    let json = serde_json::to_value(&profile).expect("serialize");
    let round_tripped: MeridianProfile = serde_json::from_value(json).expect("deserialize");
    assert_eq!(round_tripped, profile);
}

#[test]
fn intrinsic_race_component_equality() {
    let a = IntrinsicRace(RaceId::new("human"));
    let b = IntrinsicRace(RaceId::new("human"));
    let c = IntrinsicRace(RaceId::new("beast_common"));
    assert_eq!(a, b);
    assert_ne!(a, c);
}

// ───────────────────── BodyPlan::consequence_for / parts_matching ─────────────────────

fn plan_with_four_consequence_kinds() -> BodyPlan {
    BodyPlan {
        id: "fixture_consequences".into(),
        display_name: "四类后果测试构型".to_string(),
        is_humanoid: false,
        parts: vec![
            BodyPartDef {
                id: "tail_fin".into(),
                damage_mul: 0.6,
                contam_mul: 0.7,
                bleed_mul: 1.0,
                consequence: PartConsequence::Locomotion,
            },
            BodyPartDef {
                id: "skull".into(),
                damage_mul: 2.0,
                contam_mul: 1.5,
                bleed_mul: 1.5,
                consequence: PartConsequence::Sensory,
            },
            BodyPartDef {
                id: "left_pincer".into(),
                damage_mul: 0.7,
                contam_mul: 0.8,
                bleed_mul: 0.8,
                consequence: PartConsequence::Manipulator { main_hand: true },
            },
            BodyPartDef {
                id: "right_pincer".into(),
                damage_mul: 0.7,
                contam_mul: 0.8,
                bleed_mul: 0.8,
                consequence: PartConsequence::Manipulator { main_hand: false },
            },
            BodyPartDef {
                id: "carapace".into(),
                damage_mul: 1.0,
                contam_mul: 1.0,
                bleed_mul: 1.0,
                consequence: PartConsequence::Core,
            },
        ],
        hit_geometry: HitGeometry::PartBoxes { boxes: vec![] },
        equip_slots: vec![],
        meridian_profile: None,
        mutation_slot_mapping: HashMap::new(),
    }
}

#[test]
fn consequence_for_finds_every_declared_part_by_id() {
    let plan = plan_with_four_consequence_kinds();
    assert_eq!(
        plan.consequence_for(&BodyPartId::new("tail_fin")),
        Some(&PartConsequence::Locomotion)
    );
    assert_eq!(
        plan.consequence_for(&BodyPartId::new("skull")),
        Some(&PartConsequence::Sensory)
    );
    assert_eq!(
        plan.consequence_for(&BodyPartId::new("left_pincer")),
        Some(&PartConsequence::Manipulator { main_hand: true })
    );
    assert_eq!(
        plan.consequence_for(&BodyPartId::new("right_pincer")),
        Some(&PartConsequence::Manipulator { main_hand: false })
    );
    assert_eq!(
        plan.consequence_for(&BodyPartId::new("carapace")),
        Some(&PartConsequence::Core)
    );
}

#[test]
fn consequence_for_returns_none_for_unknown_part_id() {
    let plan = plan_with_four_consequence_kinds();
    assert_eq!(
        plan.consequence_for(&BodyPartId::new("does_not_exist")),
        None,
        "未知 part id 必须显式返回 None，调用方自行决定无后果策略，而不是猜一个默认后果"
    );
}

#[test]
fn consequence_for_returns_none_on_empty_parts() {
    let mut plan = plan_with_four_consequence_kinds();
    plan.parts.clear();
    assert_eq!(plan.consequence_for(&BodyPartId::new("tail_fin")), None);
}

#[test]
fn parts_matching_locomotion_finds_only_tail_fin() {
    let plan = plan_with_four_consequence_kinds();
    let found: Vec<&BodyPartId> = plan
        .parts_matching(|c| matches!(c, PartConsequence::Locomotion))
        .collect();
    assert_eq!(found, vec![&BodyPartId::new("tail_fin")]);
}

#[test]
fn parts_matching_manipulator_main_hand_true_finds_only_left_pincer() {
    let plan = plan_with_four_consequence_kinds();
    let found: Vec<&BodyPartId> = plan
        .parts_matching(|c| matches!(c, PartConsequence::Manipulator { main_hand: true }))
        .collect();
    assert_eq!(found, vec![&BodyPartId::new("left_pincer")]);
}

#[test]
fn parts_matching_manipulator_main_hand_false_finds_only_right_pincer() {
    let plan = plan_with_four_consequence_kinds();
    let found: Vec<&BodyPartId> = plan
        .parts_matching(|c| matches!(c, PartConsequence::Manipulator { main_hand: false }))
        .collect();
    assert_eq!(found, vec![&BodyPartId::new("right_pincer")]);
}

#[test]
fn parts_matching_sensory_finds_only_skull() {
    let plan = plan_with_four_consequence_kinds();
    let found: Vec<&BodyPartId> = plan
        .parts_matching(|c| matches!(c, PartConsequence::Sensory))
        .collect();
    assert_eq!(found, vec![&BodyPartId::new("skull")]);
}

#[test]
fn parts_matching_core_finds_only_carapace() {
    let plan = plan_with_four_consequence_kinds();
    let found: Vec<&BodyPartId> = plan
        .parts_matching(|c| matches!(c, PartConsequence::Core))
        .collect();
    assert_eq!(found, vec![&BodyPartId::new("carapace")]);
}

#[test]
fn parts_matching_no_match_yields_empty_iterator() {
    let plan = plan_with_four_consequence_kinds();
    let found: Vec<&BodyPartId> = plan.parts_matching(|_| false).collect();
    assert!(found.is_empty());
}

#[test]
fn parts_matching_preserves_declaration_order() {
    // humanoid.json 声明双腿为 leg_l 后 leg_r（parts 数组序）；parts_matching 必须
    // 保持这个声明顺序，不做隐式排序——`arm_wound`/`leg_wound` 的调用方依赖这一点
    // 做"取最重伤"归约时结果与遍历顺序无关，但顺序本身仍需可预期以便调试。
    let plan = bong_server::body_plan::humanoid_plan_static();
    let legs: Vec<&BodyPartId> = plan
        .parts_matching(|c| matches!(c, PartConsequence::Locomotion))
        .collect();
    assert_eq!(
        legs,
        vec![&BodyPartId::new("leg_l"), &BodyPartId::new("leg_r")],
        "humanoid.json 的 parts 数组顺序必须是 leg_l 在前"
    );
}
