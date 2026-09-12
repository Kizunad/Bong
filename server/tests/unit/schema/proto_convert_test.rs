use bong_server::body_plan::{RaceGateOwned, RaceId};

use bong_server::schema::proto_convert::{
    race_gate_owned_from_proto, race_gate_owned_to_proto, server_data_to_proto_payload,
    RaceGateDecodeError,
};

use bong_server::schema::proto_gen::bong;

use bong_server::schema::server_data::{AnqiHudKindV1, ServerDataPayloadV1};

use prost::Message;

#[test]
fn any_round_trips() {
    let value = RaceGateOwned::Any;
    let wire = race_gate_owned_to_proto(&value);
    assert_eq!(wire.kind, "any");
    assert!(wire.species.is_empty());
    assert_eq!(race_gate_owned_from_proto(&wire).unwrap(), value);
}

#[test]
fn humanoid_round_trips() {
    let value = RaceGateOwned::Humanoid;
    let wire = race_gate_owned_to_proto(&value);
    assert_eq!(wire.kind, "humanoid");
    assert!(wire.species.is_empty());
    assert_eq!(race_gate_owned_from_proto(&wire).unwrap(), value);
}

#[test]
fn species_round_trips() {
    let value = RaceGateOwned::Species {
        species: vec![RaceId::new("whale")],
    };
    let wire = race_gate_owned_to_proto(&value);
    assert_eq!(wire.kind, "species");
    assert_eq!(wire.species, vec!["whale".to_string()]);
    assert_eq!(race_gate_owned_from_proto(&wire).unwrap(), value);
}

#[test]
fn species_empty_list_round_trips() {
    let value = RaceGateOwned::Species { species: vec![] };
    let wire = race_gate_owned_to_proto(&value);
    assert_eq!(wire.species, Vec::<String>::new());
    assert_eq!(race_gate_owned_from_proto(&wire).unwrap(), value);
}

#[test]
fn species_duplicate_entries_preserved_through_wire() {
    let value = RaceGateOwned::Species {
        species: vec![RaceId::new("whale"), RaceId::new("whale")],
    };
    let wire = race_gate_owned_to_proto(&value);
    assert_eq!(wire.species, vec!["whale".to_string(), "whale".to_string()]);
    let decoded = race_gate_owned_from_proto(&wire).unwrap();
    match decoded {
        RaceGateOwned::Species { species } => assert_eq!(species.len(), 2),
        other => panic!("expected Species, got {other:?}"),
    }
}

#[test]
fn unknown_kind_decode_fails_closed() {
    let wire = bong::RaceGate {
        kind: "bogus".to_string(),
        species: Vec::new(),
    };
    let err = race_gate_owned_from_proto(&wire)
        .expect_err("unknown kind must fail closed, not silently default to Any");
    assert_eq!(err, RaceGateDecodeError::UnknownKind("bogus".to_string()));
}

/// plan-race-system-v1 P3a —— 施放门 race gate 拒绝的 wire pin：
/// `CastOutcomeV1::RejectRaceMismatch` 必须映射到新增 proto 变体
/// `bong::CastOutcome::RejectRaceMismatch`（非默认 `Unspecified`/其余已存在变体）。
fn s2c_encode_decode_roundtrip(payload: ServerDataPayloadV1) {
    let proto_payload = server_data_to_proto_payload(&payload);
    let envelope = bong::ServerDataEnvelope {
        payload: Some(proto_payload),
    };
    let bytes = envelope.encode_to_vec();
    let decoded = bong::ServerDataEnvelope::decode(bytes.as_slice())
        .expect("S2C proto decode should succeed");
    assert!(
        decoded.payload.is_some(),
        "decoded S2C envelope should contain a payload"
    );
}

#[test]
fn s2c_welcome_roundtrip() {
    s2c_encode_decode_roundtrip(ServerDataPayloadV1::Welcome {
        message: "hello".to_string(),
    });
}

#[test]
fn s2c_heartbeat_roundtrip() {
    s2c_encode_decode_roundtrip(ServerDataPayloadV1::Heartbeat {
        message: "ping".to_string(),
    });
}

#[test]
fn s2c_narration_roundtrip() {
    s2c_encode_decode_roundtrip(ServerDataPayloadV1::Narration {
        narrations: vec![bong_server::schema::narration::Narration {
            text: "test".to_string(),
            scope: bong_server::schema::common::NarrationScope::Broadcast,
            style: bong_server::schema::common::NarrationStyle::Narration,
            kind: None,
            target: None,
        }],
    });
}

#[test]
fn s2c_event_alert_roundtrip() {
    s2c_encode_decode_roundtrip(ServerDataPayloadV1::EventAlert {
        event: bong_server::schema::common::EventKind::ThunderTribulation,
        message: "watch out".to_string(),
        zone: Some("spawn".to_string()),
        duration_ticks: Some(100),
    });
}

#[test]
fn s2c_techniques_snapshot_preserves_legacy_f32_qi_cost() {
    use bong::server_data_envelope::Payload;
    use bong_server::schema::combat_hud::{TechniqueEntryV1, TechniquesSnapshotV1};
    use bong_server::schema::server_data::ServerDataPayloadV1;

    let payload = ServerDataPayloadV1::TechniquesSnapshot(TechniquesSnapshotV1 {
        entries: vec![TechniqueEntryV1 {
            id: "sword.cleave".to_string(),
            display_name: "劈".to_string(),
            grade: "common".to_string(),
            proficiency: 0.5,
            proficiency_label: "熟练".to_string(),
            active: true,
            description: String::new(),
            required_realm: "Awaken".to_string(),
            required_meridians: vec![],
            qi_cost: 0.4,
            stamina_cost: 0.0,
            cast_ticks: 1,
            cooldown_ticks: 1,
            range: 1.0,
        }],
    });
    let proto_payload = server_data_to_proto_payload(&payload);
    let envelope = bong::ServerDataEnvelope {
        payload: Some(proto_payload),
    };
    let bytes = envelope.encode_to_vec();
    let decoded = bong::ServerDataEnvelope::decode(bytes.as_slice())
        .expect("techniques snapshot proto decode should succeed");
    match decoded.payload {
        Some(Payload::TechniquesSnapshot(snapshot)) => {
            assert_eq!(snapshot.entries[0].qi_cost, 0.4_f32);
        }
        other => panic!("expected TechniquesSnapshot payload, got {other:?}"),
    }
}

#[test]
fn s2c_coffin_state_roundtrip() {
    use bong_server::schema::server_data::{CoffinGradeV1, CoffinStateV1};

    // mundane grade (baseline)
    s2c_encode_decode_roundtrip(ServerDataPayloadV1::CoffinState(CoffinStateV1 {
        in_coffin: true,
        lifespan_rate_multiplier: 1.0,
        coffin_grade: Some(CoffinGradeV1::Mundane),
    }));
}

/// Proto wire roundtrip must NOT drop coffin_grade for non-mundane grades.
/// Regression guard for blocker #1/#6：生产 wire 走 proto，grade 必须全程传递。

#[test]
fn s2c_coffin_state_grade_roundtrip_non_mundane() {
    use bong::server_data_envelope::Payload;
    use bong_server::schema::server_data::{CoffinGradeV1, CoffinStateV1};

    let cases: &[(CoffinGradeV1, &str)] = &[
        (CoffinGradeV1::Jade, "jade"),
        (CoffinGradeV1::Stone, "stone"),
        (CoffinGradeV1::Bronze, "bronze"),
    ];
    for (grade, expected_str) in cases {
        let payload = ServerDataPayloadV1::CoffinState(CoffinStateV1 {
            in_coffin: true,
            lifespan_rate_multiplier: 0.5,
            coffin_grade: Some(*grade),
        });
        let proto_payload = server_data_to_proto_payload(&payload);
        let envelope = bong::ServerDataEnvelope {
            payload: Some(proto_payload),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = bong::ServerDataEnvelope::decode(bytes.as_slice())
            .expect("S2C proto decode should succeed");
        match decoded.payload {
            Some(Payload::CoffinState(ref c)) => {
                assert_eq!(
                    c.coffin_grade.as_deref(),
                    Some(*expected_str),
                    "grade={expected_str} should survive proto encode/decode roundtrip, \
                         but decoded coffin_grade={:?}",
                    c.coffin_grade
                );
            }
            other => panic!("expected CoffinState payload, got {other:?}"),
        }
    }
}

/// Proto wire roundtrip: grade=None（出棺）→ coffin_grade 字段省略（不写入 wire）。

#[test]
fn s2c_coffin_state_grade_none_not_in_wire() {
    use bong::server_data_envelope::Payload;
    use bong_server::schema::server_data::CoffinStateV1;

    let payload = ServerDataPayloadV1::CoffinState(CoffinStateV1 {
        in_coffin: false,
        lifespan_rate_multiplier: 1.0,
        coffin_grade: None,
    });
    let proto_payload = server_data_to_proto_payload(&payload);
    let envelope = bong::ServerDataEnvelope {
        payload: Some(proto_payload),
    };
    let bytes = envelope.encode_to_vec();
    let decoded = bong::ServerDataEnvelope::decode(bytes.as_slice())
        .expect("S2C proto decode should succeed");
    match decoded.payload {
        Some(Payload::CoffinState(ref c)) => {
            assert!(
                c.coffin_grade.is_none(),
                "grade=None (出棺) should not be present in decoded proto, \
                     but decoded coffin_grade={:?}",
                c.coffin_grade
            );
        }
        other => panic!("expected CoffinState payload, got {other:?}"),
    }
}

// ─── fix/worn-pack-tab-missing：ContainerSnapshot proto wire 字段保真 ───────
//
// 根因回归锁：#779 把 owner_instance_id / quick_access 加进 serde 结构体
// ContainerSnapshotV1 + JSON 路径，但漏了 proto 定义 + proto 转换。生产 wire 走
// protobuf（agent_bridge cfg(not(test)) → to_proto_bytes），导致这两个字段在过线时
// 被静默丢弃 → client owner==null → 穿戴背包不出 tab。
//
// 这条若加在 #779 当时会 RED：proto struct 当时无该字段，构造 / 访问编译不过。

fn inv_snapshot_with_pack(
    owner_instance_id: Option<u64>,
    quick_access: bool,
) -> bong_server::schema::inventory::InventorySnapshotV1 {
    use bong_server::schema::inventory::{
        ContainerSnapshotV1, EquippedInventorySnapshotV1, InventoryWeightV1,
    };
    bong_server::schema::inventory::InventorySnapshotV1 {
        revision: 1,
        containers: vec![
            ContainerSnapshotV1 {
                id: "body_pocket".to_string(),
                name: "贴身口袋".to_string(),
                rows: 1,
                cols: 4,
                owner_instance_id: None,
                quick_access: true,
            },
            ContainerSnapshotV1 {
                id: "pack_12".to_string(),
                name: "破草包".to_string(),
                rows: 3,
                cols: 3,
                owner_instance_id,
                quick_access,
            },
        ],
        placed_items: vec![],
        equipped: EquippedInventorySnapshotV1::default(),
        hotbar: vec![],
        bone_coins: 0,
        weight: InventoryWeightV1 {
            current: 0.0,
            max: 100.0,
        },
        realm: "Induce".to_string(),
        qi_current: 0.0,
        qi_max: 100.0,
        body_level: 0.0,
    }
}

/// pack_<id> 容器的 owner_instance_id / quick_access 必须经 proto encode→decode 往返存活。
/// 锁死「proto 层不得再吞字段」——直击 worn-pack-tab-missing 根因。

#[test]
fn s2c_inventory_snapshot_container_owner_proto_roundtrip() {
    use bong::server_data_envelope::Payload;
    use prost::Message;

    let snapshot = inv_snapshot_with_pack(Some(12), true);
    let payload = ServerDataPayloadV1::InventorySnapshot(Box::new(snapshot));
    let proto_payload = server_data_to_proto_payload(&payload);
    let envelope = bong::ServerDataEnvelope {
        payload: Some(proto_payload),
    };
    let bytes = envelope.encode_to_vec();
    let decoded = bong::ServerDataEnvelope::decode(bytes.as_slice())
        .expect("S2C InventorySnapshot proto decode should succeed");
    match decoded.payload {
        Some(Payload::InventorySnapshot(ref snap)) => {
            let pack = snap
                .containers
                .iter()
                .find(|c| c.id == "pack_12")
                .expect("decoded proto 应含 pack_12 容器");
            assert_eq!(
                    pack.owner_instance_id,
                    Some(12),
                    "pack_12 的 owner_instance_id 必须经 proto wire 往返存活（期望 Some(12)，\
                     因为穿戴背包件 instance=12 是 client computeContainerDefs 判 isPackOwnerInWornSlot \
                     的唯一依据；丢失→owner==null→穿戴态不出 tab），实际 {:?}",
                    pack.owner_instance_id
                );
            assert!(
                pack.quick_access,
                "pack_12 的 quick_access 必须经 proto wire 往返存活（期望 true，\
                     因为这是容器内物品能进 F1-F9 快捷栏的资格位），实际 false"
            );
            // body_pocket：owner=None 应不写入 wire（proto3 optional 省键），quick_access=true 应保留。
            let body = snap
                .containers
                .iter()
                .find(|c| c.id == "body_pocket")
                .expect("decoded proto 应含 body_pocket 容器");
            assert!(
                body.owner_instance_id.is_none(),
                "body_pocket 非 pack_ 前缀，owner_instance_id 期望 None（不写 wire），实际 {:?}",
                body.owner_instance_id
            );
            assert!(
                body.quick_access,
                "body_pocket 期望 quick_access=true，实际 false"
            );
        }
        other => panic!("expected InventorySnapshot payload, got {other:?}"),
    }
}

/// 旧 server 兼容反向 case：owner=None + quick_access=false 时，
/// proto3 default 省键（optional uint64 None / bool false），decode 回 None/false。

#[test]
fn s2c_inventory_snapshot_container_owner_none_not_in_wire() {
    use bong::server_data_envelope::Payload;
    use prost::Message;

    let snapshot = inv_snapshot_with_pack(None, false);
    let payload = ServerDataPayloadV1::InventorySnapshot(Box::new(snapshot));
    let proto_payload = server_data_to_proto_payload(&payload);
    let envelope = bong::ServerDataEnvelope {
        payload: Some(proto_payload),
    };
    let bytes = envelope.encode_to_vec();
    let decoded = bong::ServerDataEnvelope::decode(bytes.as_slice())
        .expect("S2C InventorySnapshot proto decode should succeed");
    match decoded.payload {
        Some(Payload::InventorySnapshot(ref snap)) => {
            let pack = snap
                .containers
                .iter()
                .find(|c| c.id == "pack_12")
                .expect("decoded proto 应含 pack_12 容器");
            assert!(
                pack.owner_instance_id.is_none(),
                "owner=None 时 proto wire 不应携带 owner_instance_id，实际 {:?}",
                pack.owner_instance_id
            );
            assert!(
                !pack.quick_access,
                "quick_access=false 时 proto wire 应为 false，实际 true"
            );
        }
        other => panic!("expected InventorySnapshot payload, got {other:?}"),
    }
}

// ─── fix/alchemy-proto-drift：InventoryItemView.alchemy proto wire 保真 ─────
//
// 根因（同 #780 ContainerSnapshot.owner_instance_id 一类）：serde 结构体
// InventoryItemViewV1.alchemy 在 proto wire 上无对应字段 → inventory_item_view_to_proto
// 不搬 → 生产走 protobuf（agent_bridge cfg(not(test))）时 alchemy 被静默丢弃 →
// client readAlchemyLines 拿 null → 丹药 / 残卷 / 丹心 tooltip 真机不显。
// 单测吃 JSON 路径（cfg(test)）永远抓不到，故这里**直接调 inventory_item_view_to_proto
// + bong::InventoryItemView::decode**（绕开 cfg(test) JSON 短路），走真实 proto wire。
//
// 修复前 bong::InventoryItemView 无 alchemy 字段 → 构造 / 访问编译不过 → RED。

/// 最小可用 InventoryItemViewV1（仅填必填字段 + 给定 alchemy），供 proto wire pin 测试。
fn alchemy_view(
    alchemy: Option<bong_server::inventory::AlchemyItemData>,
) -> bong_server::schema::inventory::InventoryItemViewV1 {
    bong_server::schema::inventory::InventoryItemViewV1 {
        instance_id: 7,
        item_id: "qing_xin_dan".to_string(),
        display_name: "清心丹".to_string(),
        grid_width: 1,
        grid_height: 1,
        weight: 0.1,
        rarity: bong_server::schema::inventory::ItemRarityV1::Uncommon,
        description: String::new(),
        stack_count: 1,
        spirit_quality: 1.0,
        durability: 1.0,
        freshness: None,
        freshness_current: None,
        mineral_id: None,
        scroll_kind: None,
        scroll_skill_id: None,
        scroll_xp_grant: None,
        charges: None,
        forge_quality: None,
        forge_color: None,
        forge_side_effects: vec![],
        forge_achieved_tier: None,
        alchemy,
        lingering_owner_qi: None,
    }
}

/// 把 InventoryItemViewV1 过真实 proto wire（encode→decode），返回 decode 后的 alchemy。

#[test]
fn s2c_inventory_snapshot_placed_item_alchemy_survives_full_envelope_roundtrip() {
    use bong::server_data_envelope::Payload;
    use bong_server::inventory::AlchemyItemData;

    let mut snapshot = inv_snapshot_with_pack(Some(12), true);
    snapshot.placed_items = vec![bong_server::schema::inventory::PlacedInventoryItemV1 {
        container_id: "pack_12".to_string(),
        row: 0,
        col: 0,
        item: alchemy_view(Some(AlchemyItemData::Pill {
            recipe_id: "qing_xin_dan".to_string(),
            quality_tier: 1,
            effect_multiplier: 1.0,
            consecrated: false,
            side_effect: None,
        })),
    }];

    let payload = ServerDataPayloadV1::InventorySnapshot(Box::new(snapshot));
    let proto_payload = server_data_to_proto_payload(&payload);
    let envelope = bong::ServerDataEnvelope {
        payload: Some(proto_payload),
    };
    let bytes = envelope.encode_to_vec();
    let decoded = bong::ServerDataEnvelope::decode(bytes.as_slice())
        .expect("S2C InventorySnapshot proto decode should succeed");
    match decoded.payload {
        Some(Payload::InventorySnapshot(ref snap)) => {
            let placed = snap
                .placed_items
                .first()
                .expect("decoded proto 应含 1 个 placed_item");
            let item = placed.item.as_ref().expect("placed_item.item 应存在");
            let a = item.alchemy.as_ref().expect(
                    "placed item 的 alchemy 必须经整条 InventorySnapshot envelope 往返存活（修复前丢失）",
                );
            assert_eq!(
                a.kind, "pill",
                "整快照往返后 alchemy.kind 仍须为 pill，否则 client 丹药 tooltip 不显"
            );
            assert_eq!(a.recipe_id.as_deref(), Some("qing_xin_dan"));
        }
        other => panic!("expected InventorySnapshot payload, got {other:?}"),
    }
}

// ─── plan-exploration-probe-return-v1 S2C proto round-trip ─────────────────

/// P0: MineralProbeResult found — wire round-trip + 逐字段保真。

#[test]
fn s2c_mineral_probe_result_found_proto_roundtrip() {
    use bong::server_data_envelope::Payload;
    use prost::Message;
    let data = bong_server::schema::server_data::MineralProbeResultV1 {
        kind: "found".to_string(),
        mineral_id: Some("cu_tie".to_string()),
        remaining_units: Some(42),
        display_name_zh: Some("赤铜矿脉".to_string()),
        denial_reason: None,
    };
    let payload = ServerDataPayloadV1::MineralProbeResult(data);
    let proto = server_data_to_proto_payload(&payload);
    let envelope = bong::ServerDataEnvelope {
        payload: Some(proto),
    };
    let bytes = envelope.encode_to_vec();
    let decoded = bong::ServerDataEnvelope::decode(bytes.as_slice())
        .expect("MineralProbeResult proto decode");
    match decoded.payload {
        Some(Payload::MineralProbeResult(r)) => {
            assert_eq!(r.kind, "found", "kind should be 'found'");
            assert_eq!(
                r.mineral_id.as_deref(),
                Some("cu_tie"),
                "mineral_id should round-trip"
            );
            assert_eq!(
                r.remaining_units,
                Some(42),
                "remaining_units should round-trip"
            );
            assert_eq!(
                r.display_name_zh.as_deref(),
                Some("赤铜矿脉"),
                "display_name_zh should round-trip"
            );
            assert!(
                r.denial_reason.is_none(),
                "denial_reason should be None for found"
            );
        }
        other => panic!("expected MineralProbeResult payload, got {other:?}"),
    }
}

/// P0: MineralProbeResult denied — denial_reason 字段保真。

#[test]
fn s2c_mineral_probe_result_denied_proto_roundtrip() {
    use bong::server_data_envelope::Payload;
    use prost::Message;
    let data = bong_server::schema::server_data::MineralProbeResultV1 {
        kind: "denied".to_string(),
        mineral_id: None,
        remaining_units: None,
        display_name_zh: None,
        denial_reason: Some("realm_too_low".to_string()),
    };
    let payload = ServerDataPayloadV1::MineralProbeResult(data);
    let proto = server_data_to_proto_payload(&payload);
    let envelope = bong::ServerDataEnvelope {
        payload: Some(proto),
    };
    let bytes = envelope.encode_to_vec();
    let decoded = bong::ServerDataEnvelope::decode(bytes.as_slice())
        .expect("MineralProbeResult denied proto decode");
    match decoded.payload {
        Some(Payload::MineralProbeResult(r)) => {
            assert_eq!(r.kind, "denied", "kind should be 'denied'");
            assert_eq!(
                r.denial_reason.as_deref(),
                Some("realm_too_low"),
                "denial_reason should round-trip"
            );
            assert!(
                r.mineral_id.is_none(),
                "mineral_id should be None for denied"
            );
        }
        other => panic!("expected MineralProbeResult payload, got {other:?}"),
    }
}

/// P1: FreshnessUpdate — item_uuid + freshness + profile_name 字段保真。

#[test]
fn s2c_freshness_update_proto_roundtrip() {
    use bong_server::schema::processing::FreshnessUpdateV1;
    s2c_encode_decode_roundtrip(ServerDataPayloadV1::FreshnessUpdate(FreshnessUpdateV1 {
        item_uuid: "99".to_string(),
        freshness: 0.65,
        profile_name: "ling_cao".to_string(),
    }));
}

/// P2: InsightOffer — round-trip via helper（含 choice fields）。

#[test]
fn s2c_insight_offer_proto_roundtrip() {
    use bong_server::schema::cultivation::{InsightChoiceV1, InsightOfferV1};
    s2c_encode_decode_roundtrip(ServerDataPayloadV1::InsightOffer(InsightOfferV1 {
        offer_id: "insight:1:100".to_string(),
        trigger_id: "insight:1:100".to_string(),
        character_id: "offline:Kiz".to_string(),
        choices: vec![InsightChoiceV1 {
            category: "Qi".to_string(),
            effect_kind: "qi_max".to_string(),
            magnitude: 0.05,
            flavor_text: "气海微扩张。".to_string(),
            narrator_voice: None,
            alignment: None,
            cost_kind: None,
            cost_magnitude: None,
            cost_flavor: None,
        }],
    }));
}

#[test]
fn s2c_workbench_open_proto_roundtrip() {
    use bong::server_data_envelope::Payload;
    use prost::Message;
    let payload = ServerDataPayloadV1::WorkbenchOpen {
        entity_id: 42,
        position: [1, 64, -2],
    };
    let proto = server_data_to_proto_payload(&payload);
    let envelope = bong::ServerDataEnvelope {
        payload: Some(proto),
    };
    let bytes = envelope.encode_to_vec();
    let decoded =
        bong::ServerDataEnvelope::decode(bytes.as_slice()).expect("WorkbenchOpen proto decode");
    match decoded.payload {
        Some(Payload::WorkbenchOpen(open)) => {
            assert_eq!(open.entity_id, 42);
            assert_eq!([open.x, open.y, open.z], [1, 64, -2]);
        }
        other => panic!("expected WorkbenchOpen payload, got {other:?}"),
    }
}

// ─── F9 跨层修复：TutorialCoffinPos proto round-trip pin ───────────

/// Proves the F9 wire contract end to end: `ServerDataPayloadV1::TutorialCoffinPos`
/// → proto `Payload::TutorialCoffinPos` → bytes → decode → exact x/y/z survive.
/// Uses mixed-sign coordinates because spawn can legitimately sit west/south of
/// the origin; `sint32` must round-trip negatives without truncation.

#[test]
fn s2c_tutorial_coffin_pos_proto_roundtrip() {
    use bong::server_data_envelope::Payload;
    use prost::Message;
    let payload = ServerDataPayloadV1::TutorialCoffinPos {
        position: [-42, 65, 130],
    };
    let proto = server_data_to_proto_payload(&payload);
    let envelope = bong::ServerDataEnvelope {
        payload: Some(proto),
    };
    let bytes = envelope.encode_to_vec();
    let decoded =
        bong::ServerDataEnvelope::decode(bytes.as_slice()).expect("TutorialCoffinPos proto decode");
    match decoded.payload {
        Some(Payload::TutorialCoffinPos(pos)) => {
            assert_eq!(
                [pos.x, pos.y, pos.z],
                [-42, 65, 130],
                "negative x must survive the sint32 proto round trip untruncated"
            );
        }
        other => panic!("expected TutorialCoffinPos payload, got {other:?}"),
    }
}

/// `TutorialCoffinPos` is a plain proto-encoded S2C payload (join-time broadcast),
/// not a JSON-bypass variant — pin both classification signals so a future refactor
/// can't silently flip it into the bypass path (which would panic at
/// `to_proto_bytes()` per the `unreachable!()` guard).

#[test]
fn s2c_tutorial_coffin_pos_is_proto_not_bypass() {
    let payload = ServerDataPayloadV1::TutorialCoffinPos {
        position: [0, 69, 0],
    };
    assert!(
        !payload.is_json_bypass(),
        "TutorialCoffinPos has a proto definition; is_json_bypass() must be false"
    );
    assert_eq!(
        payload.payload_type(),
        bong_server::schema::server_data::ServerDataType::TutorialCoffinPos
    );
}

// ─── plan-inventory-hint-panel-v1 P0：InventoryMoveRejectedV1 proto round-trip pin ───

/// worn_cap_full 拒绝（slot + cap 都携带）→ proto → decode → 字段逐一保真。

#[test]
fn s2c_inventory_move_rejected_worn_cap_full_proto_roundtrip() {
    use bong::server_data_envelope::Payload;
    use bong_server::schema::server_data::InventoryMoveRejectedV1;
    use prost::Message;

    let payload = ServerDataPayloadV1::InventoryMoveRejected(InventoryMoveRejectedV1 {
        reason: "worn_cap_full".to_string(),
        required_realm: None,
        slot: Some("chest".to_string()),
        cap: Some(3),
    });
    let proto = server_data_to_proto_payload(&payload);
    let envelope = bong::ServerDataEnvelope {
        payload: Some(proto),
    };
    let bytes = envelope.encode_to_vec();
    let decoded = bong::ServerDataEnvelope::decode(bytes.as_slice())
        .expect("InventoryMoveRejected proto decode");
    match decoded.payload {
        Some(Payload::InventoryMoveRejected(r)) => {
            assert_eq!(r.reason, "worn_cap_full");
            assert!(r.required_realm.is_none());
            assert_eq!(r.slot.as_deref(), Some("chest"));
            assert_eq!(r.cap, Some(3));
        }
        other => panic!("expected InventoryMoveRejected payload, got {other:?}"),
    }
}

/// realm_too_low 拒绝（required_realm 携带，slot/cap 为 None）→ proto round-trip 保真。

#[test]
fn s2c_inventory_move_rejected_realm_too_low_proto_roundtrip() {
    use bong::server_data_envelope::Payload;
    use bong_server::schema::server_data::InventoryMoveRejectedV1;
    use prost::Message;

    let payload = ServerDataPayloadV1::InventoryMoveRejected(InventoryMoveRejectedV1 {
        reason: "realm_too_low".to_string(),
        required_realm: Some("Condense".to_string()),
        slot: None,
        cap: None,
    });
    let proto = server_data_to_proto_payload(&payload);
    let envelope = bong::ServerDataEnvelope {
        payload: Some(proto),
    };
    let bytes = envelope.encode_to_vec();
    let decoded = bong::ServerDataEnvelope::decode(bytes.as_slice())
        .expect("InventoryMoveRejected proto decode");
    match decoded.payload {
        Some(Payload::InventoryMoveRejected(r)) => {
            assert_eq!(r.reason, "realm_too_low");
            assert_eq!(r.required_realm.as_deref(), Some("Condense"));
            assert!(r.slot.is_none());
            assert!(r.cap.is_none());
        }
        other => panic!("expected InventoryMoveRejected payload, got {other:?}"),
    }
}

/// `InventoryMoveRejected` 走 proto 路径，不是 JSON-bypass —— pin 两个分类信号防回归。

#[test]
fn s2c_inventory_move_rejected_is_proto_not_bypass() {
    use bong_server::schema::server_data::InventoryMoveRejectedV1;
    let payload = ServerDataPayloadV1::InventoryMoveRejected(InventoryMoveRejectedV1 {
        reason: "hand_occupied".to_string(),
        required_realm: None,
        slot: None,
        cap: None,
    });
    assert!(
        !payload.is_json_bypass(),
        "InventoryMoveRejected has a proto definition; is_json_bypass() must be false"
    );
    assert_eq!(
        payload.payload_type(),
        bong_server::schema::server_data::ServerDataType::InventoryMoveRejected
    );
}

// ─── plan-scroll-reading-v1 P0：ScrollOpen proto round-trip pin（tag 138，§9）───

/// 3 页正文（经脉入门残卷实际形态）→ proto → decode → 字段逐一保真。

#[test]
fn s2c_scroll_open_three_pages_proto_roundtrip() {
    use bong::server_data_envelope::Payload;
    use prost::Message;

    let payload = ServerDataPayloadV1::ScrollOpen {
        scroll_id: "scroll_meridian_primer".to_string(),
        title: "《经脉浅述·残卷》".to_string(),
        body_pages: vec![
            "第一页".to_string(),
            "第二页".to_string(),
            "第三页".to_string(),
        ],
    };
    let proto = server_data_to_proto_payload(&payload);
    let envelope = bong::ServerDataEnvelope {
        payload: Some(proto),
    };
    let bytes = envelope.encode_to_vec();
    let decoded =
        bong::ServerDataEnvelope::decode(bytes.as_slice()).expect("ScrollOpen proto decode");
    match decoded.payload {
        Some(Payload::ScrollOpen(r)) => {
            assert_eq!(r.scroll_id, "scroll_meridian_primer");
            assert_eq!(r.title, "《经脉浅述·残卷》");
            assert_eq!(
                r.body_pages,
                vec![
                    "第一页".to_string(),
                    "第二页".to_string(),
                    "第三页".to_string()
                ]
            );
        }
        other => panic!("expected ScrollOpen payload, got {other:?}"),
    }
}

/// 边界：单页正文（`body_pages.len()==1`，§9 "至少 1 页"下界）proto round-trip 保真。

#[test]
fn s2c_scroll_open_single_page_proto_roundtrip() {
    use bong::server_data_envelope::Payload;
    use prost::Message;

    let payload = ServerDataPayloadV1::ScrollOpen {
        scroll_id: "scroll_x".to_string(),
        title: "t".to_string(),
        body_pages: vec!["only page".to_string()],
    };
    let proto = server_data_to_proto_payload(&payload);
    let envelope = bong::ServerDataEnvelope {
        payload: Some(proto),
    };
    let bytes = envelope.encode_to_vec();
    let decoded =
        bong::ServerDataEnvelope::decode(bytes.as_slice()).expect("ScrollOpen proto decode");
    match decoded.payload {
        Some(Payload::ScrollOpen(r)) => {
            assert_eq!(r.body_pages, vec!["only page".to_string()]);
        }
        other => panic!("expected ScrollOpen payload, got {other:?}"),
    }
}

/// `ScrollOpen` 走 proto 路径，不是 JSON-bypass —— pin 两个分类信号防回归。

#[test]
fn s2c_scroll_open_is_proto_not_bypass() {
    let payload = ServerDataPayloadV1::ScrollOpen {
        scroll_id: "scroll_x".to_string(),
        title: "t".to_string(),
        body_pages: vec!["p".to_string()],
    };
    assert!(
        !payload.is_json_bypass(),
        "ScrollOpen has a proto definition; is_json_bypass() must be false"
    );
    assert_eq!(
        payload.payload_type(),
        bong_server::schema::server_data::ServerDataType::ScrollOpen
    );
}

// ─── plan-scroll-reading-v1 P0：ScrollReadRequest proto round-trip pin（tag 100，§9）───

/// C2S ScrollReadRequest{v,instance_id} → proto → decode → 字段逐一保真。

#[test]
fn c2s_scroll_read_request_proto_roundtrip() {
    use bong::client_request_envelope::Payload;
    use bong_server::schema::client_request::ClientRequestV1;
    use prost::Message;

    let req = ClientRequestV1::ScrollReadRequest {
        v: 1,
        instance_id: 42,
    };
    let proto = Payload::from(&req);
    let envelope = bong::ClientRequestEnvelope {
        payload: Some(proto),
    };
    let bytes = envelope.encode_to_vec();
    let decoded = bong::ClientRequestEnvelope::decode(bytes.as_slice())
        .expect("ScrollReadRequest proto decode");
    match decoded.payload {
        Some(Payload::ScrollReadRequest(r)) => {
            assert_eq!(r.v, 1);
            assert_eq!(r.instance_id, 42);
        }
        other => panic!("expected ScrollReadRequest payload, got {other:?}"),
    }
}

/// 边界：instance_id=0（u64 下界）proto round-trip 保真。

#[test]
fn c2s_scroll_read_request_zero_instance_id_proto_roundtrip() {
    use bong::client_request_envelope::Payload;
    use bong_server::schema::client_request::ClientRequestV1;
    use prost::Message;

    let req = ClientRequestV1::ScrollReadRequest {
        v: 1,
        instance_id: 0,
    };
    let proto = Payload::from(&req);
    let envelope = bong::ClientRequestEnvelope {
        payload: Some(proto),
    };
    let bytes = envelope.encode_to_vec();
    let decoded = bong::ClientRequestEnvelope::decode(bytes.as_slice())
        .expect("ScrollReadRequest proto decode");
    match decoded.payload {
        Some(Payload::ScrollReadRequest(r)) => {
            assert_eq!(r.instance_id, 0);
        }
        other => panic!("expected ScrollReadRequest payload, got {other:?}"),
    }
}

#[test]
fn s2c_ui_open_roundtrip() {
    s2c_encode_decode_roundtrip(ServerDataPayloadV1::UiOpen {
        ui: Some("inspect".to_string()),
        xml: String::new(),
    });
}

#[test]
fn s2c_to_proto_bytes_method_encodes_successfully() {
    use bong_server::schema::server_data::ServerDataV1;
    let payload = ServerDataV1::welcome("hello world");
    let bytes = payload.to_proto_bytes();
    assert!(!bytes.is_empty(), "proto bytes should not be empty");
    let decoded = bong::ServerDataEnvelope::decode(bytes.as_slice())
        .expect("to_proto_bytes output should decode as ServerDataEnvelope");
    assert!(decoded.payload.is_some());
    match decoded.payload.unwrap() {
        bong::server_data_envelope::Payload::Welcome(w) => {
            assert_eq!(w.message, "hello world");
        }
        other => panic!("expected Welcome payload, got {other:?}"),
    }
}

/// C2S: encode -> decode roundtrip.
fn c2s_encode_decode_roundtrip(req: bong_server::schema::client_request::ClientRequestV1) {
    let proto_payload = bong::client_request_envelope::Payload::from(&req);
    let envelope = bong::ClientRequestEnvelope {
        payload: Some(proto_payload),
    };
    let bytes = envelope.encode_to_vec();
    let decoded = bong::ClientRequestEnvelope::decode(bytes.as_slice())
        .expect("C2S proto decode should succeed");
    assert!(
        decoded.payload.is_some(),
        "decoded C2S envelope should contain a payload for {req:?}"
    );
}

#[test]
fn c2s_set_meridian_target_roundtrip() {
    c2s_encode_decode_roundtrip(
        bong_server::schema::client_request::ClientRequestV1::SetMeridianTarget {
            v: 1,
            meridian: bong_server::cultivation::components::MeridianId::Lung.channel_id(),
        },
    );
}

#[test]
fn c2s_breakthrough_request_roundtrip() {
    c2s_encode_decode_roundtrip(
        bong_server::schema::client_request::ClientRequestV1::BreakthroughRequest { v: 1 },
    );
}

#[test]
fn c2s_alchemy_open_furnace_roundtrip() {
    c2s_encode_decode_roundtrip(
        bong_server::schema::client_request::ClientRequestV1::AlchemyOpenFurnace {
            v: 1,
            furnace_pos: (10, 64, -20),
        },
    );
}

#[test]
fn c2s_craft_start_roundtrip() {
    c2s_encode_decode_roundtrip(
        bong_server::schema::client_request::ClientRequestV1::CraftStart {
            v: 1,
            recipe_id: "bone_needle".to_string(),
            quantity: 3,
        },
    );
}

#[test]
fn c2s_zhenfa_place_roundtrip() {
    c2s_encode_decode_roundtrip(
        bong_server::schema::client_request::ClientRequestV1::ZhenfaPlace {
            v: 1,
            x: 100,
            y: 64,
            z: -50,
            kind: bong_server::zhenfa::ZhenfaKind::Trap,
            carrier: Some(bong_server::zhenfa::ZhenfaCarrierKind::CommonStone),
            qi_invest_ratio: 0.5,
            trigger: None,
            item_instance_id: Some(999),
            target_face: Some(bong_server::zhenfa::trap_content::TrapTargetFace::Top),
        },
    );
}

#[test]
fn c2s_block_place_roundtrip() {
    c2s_encode_decode_roundtrip(
        bong_server::schema::client_request::ClientRequestV1::BlockPlace {
            v: 1,
            x: 8,
            y: 64,
            z: -8,
            item_instance_id: 4242,
            target_face: bong_server::zhenfa::trap_content::TrapTargetFace::North,
        },
    );
}

#[test]
fn c2s_void_action_barrier_roundtrip() {
    use bong_server::cultivation::void::components::BarrierGeometry;
    c2s_encode_decode_roundtrip(
        bong_server::schema::client_request::ClientRequestV1::VoidAction {
            v: 1,
            request: bong_server::schema::void_actions::VoidActionRequestV1::Barrier {
                zone_id: "spawn".to_string(),
                geometry: BarrierGeometry::Circle {
                    center: [100.0, 64.0, -50.0],
                    radius: 10.0,
                },
            },
        },
    );
}

#[test]
fn c2s_movement_action_roundtrip() {
    c2s_encode_decode_roundtrip(
        bong_server::schema::client_request::ClientRequestV1::MovementAction {
            v: 1,
            action: bong_server::schema::movement::MovementActionRequestV1::Dash,
            yaw_degrees: Some(90.0),
        },
    );
}

#[test]
fn c2s_skill_config_intent_roundtrip() {
    use std::collections::BTreeMap;
    let mut config = BTreeMap::new();
    config.insert(
        "range".to_string(),
        serde_json::Value::Number(serde_json::Number::from(5)),
    );
    c2s_encode_decode_roundtrip(
        bong_server::schema::client_request::ClientRequestV1::SkillConfigIntent {
            v: 1,
            skill_id: "fireball".to_string(),
            config,
        },
    );
}

#[test]
fn c2s_apply_pill_self_roundtrip() {
    c2s_encode_decode_roundtrip(
        bong_server::schema::client_request::ClientRequestV1::ApplyPill {
            v: 1,
            instance_id: 42,
            target: bong_server::schema::client_request::ApplyPillTargetV1::SelfTarget,
        },
    );
}

#[test]
fn c2s_apply_pill_meridian_roundtrip() {
    c2s_encode_decode_roundtrip(
        bong_server::schema::client_request::ClientRequestV1::ApplyPill {
            v: 1,
            instance_id: 42,
            target: bong_server::schema::client_request::ApplyPillTargetV1::Meridian {
                meridian_id: bong_server::cultivation::components::MeridianId::Heart.channel_id(),
            },
        },
    );
}

// ─── plan-supply-coffin-loot-ui P1：外部容器 proto roundtrip ────

#[test]
fn s2c_loot_container_open_roundtrip() {
    let payload = ServerDataPayloadV1::LootContainerOpen(
        bong_server::schema::server_data::LootContainerOpenV1 {
            session_id: 42,
            source_kind:
                bong_server::schema::server_data::LootContainerSourceKindV1::SupplyCoffin {
                    grade: "common".to_string(),
                },
            rows: 3,
            cols: 4,
            placed_items: vec![],
            timeout_wall_secs: 1716872400,
        },
    );
    s2c_encode_decode_roundtrip(payload.clone());

    // Field-level verification on the decoded proto payload
    let proto_payload = server_data_to_proto_payload(&payload);
    let envelope = bong::ServerDataEnvelope {
        payload: Some(proto_payload),
    };
    let bytes = envelope.encode_to_vec();
    let decoded = bong::ServerDataEnvelope::decode(bytes.as_slice())
        .expect("S2C proto decode should succeed");
    match decoded.payload {
        Some(bong::server_data_envelope::Payload::LootContainerOpen(open)) => {
            assert_eq!(open.session_id, 42, "session_id should survive roundtrip");
            assert_eq!(open.rows, 3, "rows should survive roundtrip");
            assert_eq!(open.cols, 4, "cols should survive roundtrip");
            assert!(
                open.placed_items.is_empty(),
                "empty placed_items should survive roundtrip"
            );
            assert_eq!(
                open.timeout_wall_secs, 1716872400,
                "timeout_wall_secs should survive roundtrip"
            );
            assert!(
                open.source_kind.contains("supply_coffin"),
                "source_kind should contain supply_coffin, got: {}",
                open.source_kind
            );
        }
        other => panic!("expected LootContainerOpen payload, got {other:?}"),
    }
}

#[test]
fn s2c_loot_container_update_roundtrip() {
    let payload = ServerDataPayloadV1::LootContainerUpdate(
        bong_server::schema::server_data::LootContainerUpdateV1 {
            session_id: 7,
            placed_items: vec![],
        },
    );
    s2c_encode_decode_roundtrip(payload.clone());

    // Field-level verification on the decoded proto payload
    let proto_payload = server_data_to_proto_payload(&payload);
    let envelope = bong::ServerDataEnvelope {
        payload: Some(proto_payload),
    };
    let bytes = envelope.encode_to_vec();
    let decoded = bong::ServerDataEnvelope::decode(bytes.as_slice())
        .expect("S2C proto decode should succeed");
    match decoded.payload {
        Some(bong::server_data_envelope::Payload::LootContainerUpdate(upd)) => {
            assert_eq!(upd.session_id, 7, "session_id should survive roundtrip");
            assert!(
                upd.placed_items.is_empty(),
                "empty placed_items should survive roundtrip"
            );
        }
        other => panic!("expected LootContainerUpdate payload, got {other:?}"),
    }
}

#[test]
fn s2c_loot_container_close_roundtrip() {
    use bong_server::schema::server_data::{LootContainerCloseReasonV1, LootContainerCloseV1};

    let cases = [
        (LootContainerCloseReasonV1::Timeout, "timeout"),
        (LootContainerCloseReasonV1::Distance, "distance"),
        (LootContainerCloseReasonV1::PlayerClosed, "player_closed"),
        (
            LootContainerCloseReasonV1::CoffinDestroyed,
            "coffin_destroyed",
        ),
        (
            LootContainerCloseReasonV1::ContainerDestroyed,
            "container_destroyed",
        ),
    ];

    for (reason, expected_wire) in cases {
        let payload = ServerDataPayloadV1::LootContainerClose(LootContainerCloseV1 {
            session_id: 99,
            reason: reason.clone(),
        });
        s2c_encode_decode_roundtrip(payload.clone());

        // Field-level verification on the decoded proto payload
        let proto_payload = server_data_to_proto_payload(&payload);
        let envelope = bong::ServerDataEnvelope {
            payload: Some(proto_payload),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = bong::ServerDataEnvelope::decode(bytes.as_slice())
            .expect("S2C proto decode should succeed");
        match decoded.payload {
            Some(bong::server_data_envelope::Payload::LootContainerClose(close)) => {
                assert_eq!(
                    close.session_id, 99,
                    "session_id should survive roundtrip for {reason:?}"
                );
                assert_eq!(
                    close.reason, expected_wire,
                    "expected close reason wire `{expected_wire}` for {reason:?}, actual `{}`",
                    close.reason
                );
            }
            other => panic!("expected LootContainerClose payload, got {other:?}"),
        }
    }
}

#[test]
fn c2s_external_container_move_roundtrip() {
    c2s_encode_decode_roundtrip(
        bong_server::schema::client_request::ClientRequestV1::ExternalContainerMove {
            v: 1,
            session_id: 42,
            instance_id: 100,
            from: bong_server::schema::inventory::InventoryLocationV1::Container {
                container_id: "ext_42".to_string(),
                row: 0,
                col: 1,
            },
            to: bong_server::schema::inventory::InventoryLocationV1::Container {
                container_id: "body_pocket".to_string(),
                row: 1,
                col: 0,
            },
        },
    );
}

#[test]
fn c2s_external_container_close_roundtrip() {
    c2s_encode_decode_roundtrip(
        bong_server::schema::client_request::ClientRequestV1::ExternalContainerClose {
            v: 1,
            session_id: 99,
        },
    );
}

#[test]
fn c2s_supply_coffin_open_roundtrip() {
    c2s_encode_decode_roundtrip(
        bong_server::schema::client_request::ClientRequestV1::SupplyCoffinOpen {
            v: 1,
            entity_id: 42,
        },
    );
}

#[test]
fn c2s_supply_coffin_open_zero_entity_id_roundtrip() {
    c2s_encode_decode_roundtrip(
        bong_server::schema::client_request::ClientRequestV1::SupplyCoffinOpen {
            v: 1,
            entity_id: 0,
        },
    );
}

#[test]
fn c2s_supply_coffin_open_max_entity_id_roundtrip() {
    c2s_encode_decode_roundtrip(
        bong_server::schema::client_request::ClientRequestV1::SupplyCoffinOpen {
            v: 1,
            entity_id: i32::MAX,
        },
    );
}

#[test]
fn c2s_container_open_roundtrip() {
    c2s_encode_decode_roundtrip(
        bong_server::schema::client_request::ClientRequestV1::ContainerOpen {
            v: 1,
            entity_id: 42,
        },
    );
}

#[test]
fn c2s_container_open_zero_entity_id_roundtrip() {
    c2s_encode_decode_roundtrip(
        bong_server::schema::client_request::ClientRequestV1::ContainerOpen { v: 1, entity_id: 0 },
    );
}

#[test]
fn c2s_container_open_max_entity_id_roundtrip() {
    c2s_encode_decode_roundtrip(
        bong_server::schema::client_request::ClientRequestV1::ContainerOpen {
            v: 1,
            entity_id: i32::MAX,
        },
    );
}

#[test]
fn c2s_container_open_proto_roundtrip_asserts_variant_and_entity_id() {
    use prost::Message;
    let req = bong_server::schema::client_request::ClientRequestV1::ContainerOpen {
        v: 1,
        entity_id: 165,
    };
    let proto_payload = bong::client_request_envelope::Payload::from(&req);
    let envelope = bong::ClientRequestEnvelope {
        payload: Some(proto_payload),
    };
    let bytes = envelope.encode_to_vec();
    let decoded = bong::ClientRequestEnvelope::decode(bytes.as_slice())
        .expect("C2S ContainerOpen proto decode should succeed");
    match decoded.payload {
        Some(bong::client_request_envelope::Payload::ContainerOpen(open)) => {
            assert_eq!(
                open.entity_id, 165,
                "ContainerOpen.entity_id should survive proto roundtrip"
            );
        }
        other => panic!("expected ContainerOpen payload, got {other:?}"),
    }
}

#[test]
fn c2s_workbench_open_roundtrip() {
    c2s_encode_decode_roundtrip(
        bong_server::schema::client_request::ClientRequestV1::WorkbenchOpen {
            v: 1,
            entity_id: 42,
        },
    );
}

#[test]
fn c2s_workbench_open_proto_roundtrip_asserts_variant_and_entity_id() {
    use prost::Message;
    let req = bong_server::schema::client_request::ClientRequestV1::WorkbenchOpen {
        v: 1,
        entity_id: 165,
    };
    let proto_payload = bong::client_request_envelope::Payload::from(&req);
    let envelope = bong::ClientRequestEnvelope {
        payload: Some(proto_payload),
    };
    let bytes = envelope.encode_to_vec();
    let decoded = bong::ClientRequestEnvelope::decode(bytes.as_slice())
        .expect("C2S WorkbenchOpen proto decode should succeed");
    match decoded.payload {
        Some(bong::client_request_envelope::Payload::WorkbenchOpen(open)) => {
            assert_eq!(
                open.entity_id, 165,
                "WorkbenchOpen.entity_id should survive proto roundtrip"
            );
        }
        other => panic!("expected WorkbenchOpen payload, got {other:?}"),
    }
}

// ─── plan-coffin-tiers-v1 P3：延寿棺 C2S proto roundtrip ────────────────

#[test]
fn c2s_coffin_break_roundtrip() {
    c2s_encode_decode_roundtrip(
        bong_server::schema::client_request::ClientRequestV1::CoffinBreak {
            v: 1,
            x: 10,
            y: 64,
            z: -5,
        },
    );
}

/// CodeRabbit major D: oneof variant + 坐标逐字段保真断言（非零/负坐标）。

#[test]
fn c2s_coffin_break_proto_roundtrip_asserts_variant_and_coords() {
    use prost::Message;
    let req = bong_server::schema::client_request::ClientRequestV1::CoffinBreak {
        v: 1,
        x: 10,
        y: 64,
        z: -5,
    };
    let proto_payload = bong::client_request_envelope::Payload::from(&req);
    let envelope = bong::ClientRequestEnvelope {
        payload: Some(proto_payload),
    };
    let bytes = envelope.encode_to_vec();
    let decoded = bong::ClientRequestEnvelope::decode(bytes.as_slice())
        .expect("C2S CoffinBreak proto decode should succeed");
    match decoded.payload {
        Some(bong::client_request_envelope::Payload::CoffinBreak(b)) => {
            assert_eq!(
                b.x, 10,
                "CoffinBreak.x should survive roundtrip; expected 10, got {}",
                b.x
            );
            assert_eq!(
                b.y, 64,
                "CoffinBreak.y should survive roundtrip; expected 64, got {}",
                b.y
            );
            assert_eq!(
                b.z, -5,
                "CoffinBreak.z (negative) should survive roundtrip; expected -5, got {}",
                b.z
            );
        }
        other => panic!(
            "expected CoffinBreak oneof variant after roundtrip, got {other:?}; \
                 confirms the payload is not silently mapped to another variant"
        ),
    }
}

#[test]
fn c2s_coffin_menu_reclaim_roundtrip() {
    c2s_encode_decode_roundtrip(
        bong_server::schema::client_request::ClientRequestV1::CoffinMenuReclaim {
            v: 1,
            x: 3,
            y: 65,
            z: 7,
        },
    );
}

// ─── plan-exploration-probe-return-v1 P0/P1 C2S proto round-trip ─────────

/// P0: MineralProbe C2S proto round-trip + 坐标逐字段保真（含负坐标边界）。

#[test]
fn c2s_mineral_probe_proto_roundtrip_asserts_variant_and_coords() {
    use prost::Message;
    let req = bong_server::schema::client_request::ClientRequestV1::MineralProbe {
        v: 1,
        x: -16,
        y: 48,
        z: 255,
    };
    let proto_payload = bong::client_request_envelope::Payload::from(&req);
    let envelope = bong::ClientRequestEnvelope {
        payload: Some(proto_payload),
    };
    let bytes = envelope.encode_to_vec();
    let decoded = bong::ClientRequestEnvelope::decode(bytes.as_slice())
        .expect("C2S MineralProbe proto decode should succeed");
    match decoded.payload {
        Some(bong::client_request_envelope::Payload::MineralProbe(r)) => {
            assert_eq!(
                r.x, -16,
                "MineralProbe.x (negative) should survive roundtrip; expected -16, got {}",
                r.x
            );
            assert_eq!(
                r.y, 48,
                "MineralProbe.y should survive roundtrip; expected 48, got {}",
                r.y
            );
            assert_eq!(
                r.z, 255,
                "MineralProbe.z should survive roundtrip; expected 255, got {}",
                r.z
            );
        }
        other => panic!("expected MineralProbe oneof variant after roundtrip, got {other:?}"),
    }
}

/// P1: FreshnessProbe C2S proto round-trip + instance_id 逐字段保真（含边界值）。

#[test]
fn c2s_freshness_probe_proto_roundtrip_asserts_variant_and_instance_id() {
    use prost::Message;
    let req = bong_server::schema::client_request::ClientRequestV1::FreshnessProbe {
        v: 1,
        instance_id: 9_999_999_999u64,
    };
    let proto_payload = bong::client_request_envelope::Payload::from(&req);
    let envelope = bong::ClientRequestEnvelope {
        payload: Some(proto_payload),
    };
    let bytes = envelope.encode_to_vec();
    let decoded = bong::ClientRequestEnvelope::decode(bytes.as_slice())
        .expect("C2S FreshnessProbe proto decode should succeed");
    match decoded.payload {
        Some(bong::client_request_envelope::Payload::FreshnessProbe(r)) => {
            assert_eq!(
                    r.instance_id, 9_999_999_999u64,
                    "FreshnessProbe.instance_id large value should survive roundtrip; expected 9_999_999_999, got {}",
                    r.instance_id
                );
        }
        other => panic!("expected FreshnessProbe oneof variant after roundtrip, got {other:?}"),
    }
}

/// P1: FreshnessProbe instance_id=0 边界（最小合法值）。

#[test]
fn c2s_freshness_probe_zero_instance_id_roundtrip() {
    c2s_encode_decode_roundtrip(
        bong_server::schema::client_request::ClientRequestV1::FreshnessProbe {
            v: 1,
            instance_id: 0,
        },
    );
}

/// P0: MineralProbe basic round-trip（全走 helper）。

#[test]
fn c2s_mineral_probe_basic_roundtrip() {
    c2s_encode_decode_roundtrip(
        bong_server::schema::client_request::ClientRequestV1::MineralProbe {
            v: 1,
            x: 10,
            y: 64,
            z: -8,
        },
    );
}

/// CodeRabbit major D: oneof variant + 坐标逐字段保真断言（含负坐标）。

#[test]
fn c2s_coffin_menu_reclaim_proto_roundtrip_asserts_variant_and_coords() {
    use prost::Message;
    let req = bong_server::schema::client_request::ClientRequestV1::CoffinMenuReclaim {
        v: 1,
        x: -8,
        y: 65,
        z: 3,
    };
    let proto_payload = bong::client_request_envelope::Payload::from(&req);
    let envelope = bong::ClientRequestEnvelope {
        payload: Some(proto_payload),
    };
    let bytes = envelope.encode_to_vec();
    let decoded = bong::ClientRequestEnvelope::decode(bytes.as_slice())
        .expect("C2S CoffinMenuReclaim proto decode should succeed");
    match decoded.payload {
        Some(bong::client_request_envelope::Payload::CoffinMenuReclaim(r)) => {
            assert_eq!(
                r.x, -8,
                "CoffinMenuReclaim.x (negative) should survive roundtrip; expected -8, got {}",
                r.x
            );
            assert_eq!(
                r.y, 65,
                "CoffinMenuReclaim.y should survive roundtrip; expected 65, got {}",
                r.y
            );
            assert_eq!(
                r.z, 3,
                "CoffinMenuReclaim.z should survive roundtrip; expected 3, got {}",
                r.z
            );
        }
        other => panic!(
            "expected CoffinMenuReclaim oneof variant after roundtrip, got {other:?}; \
                 confirms the payload is not silently mapped to another variant"
        ),
    }
}

// ─── plan-agent-ui-data-v1 P0：AgentUi JSON-bypass 协议契约 pin 测试 ───────

/// AgentUiResponse C2S 协议契约：JSON CustomPayload 路径有效，proto 路径不可达。
///
/// 此测试锁死两条重要约束：
/// 1. ClientRequestV1::AgentUiResponse JSON 正样本 roundtrip 正确，且不含
///    server→agent 专属的 target_player。
/// 2. C2S proto 转换函数中 AgentUiResponse 分支标记为 unreachable（检查代码注释+结构）。
///
/// 保护目的：防止将来切换 proto 编码时无意触发此分支（footgun 消除 pin）。

#[test]
fn agent_ui_response_json_bypass_contract_pin() {
    use bong_server::schema::agent_ui::AgentUiActionType;
    use bong_server::schema::client_request::ClientRequestV1;
    use std::collections::HashMap;

    // 真实 Fabric C2S JSON 路径正样本 roundtrip。
    let response = ClientRequestV1::AgentUiResponse {
        v: 1,
        request_id: "pin-test-req".to_string(),
        action: AgentUiActionType::ButtonClick,
        params: {
            let mut m = HashMap::new();
            m.insert("button_id".to_string(), "confirm".to_string());
            m
        },
    };
    let json = serde_json::to_string(&response).expect("C2S AgentUiResponse JSON 序列化不应失败");
    assert!(
        !json.contains("target_player"),
        "真实 Fabric C2S JSON 不得携带 server→agent 权威字段 target_player：{json}"
    );
    let decoded: ClientRequestV1 =
        serde_json::from_str(&json).expect("C2S AgentUiResponse JSON 反序列化不应失败");

    match decoded {
        ClientRequestV1::AgentUiResponse {
            v,
            request_id,
            action,
            params,
        } => {
            assert_eq!(v, 1, "C2S AgentUiResponse wire version 应保持为 1");
            assert_eq!(request_id, "pin-test-req");
            assert_eq!(action, AgentUiActionType::ButtonClick);
            assert_eq!(params.get("button_id").map(String::as_str), Some("confirm"));
        }
        other => panic!("expected C2S AgentUiResponse after JSON roundtrip, got {other:?}"),
    }

    // S2C AgentUiRequest/Close 同样走 JSON bypass，不走 proto。
    // 以下验证 JSON bypass 路径对 AgentUiRequestPayloadV1 也正确。
    use bong_server::schema::agent_ui::AgentUiRequestPayloadV1;
    let request = AgentUiRequestPayloadV1 {
        request_id: "pin-req".to_string(),
        target_player: "pin-player".to_string(),
        xml: "<owo-ui><components><label>test</label></components></owo-ui>".to_string(),
        timeout_ticks: 600,
    };
    let json2 =
        serde_json::to_string(&request).expect("AgentUiRequestPayloadV1 JSON 序列化不应失败");
    let decoded2: AgentUiRequestPayloadV1 =
        serde_json::from_str(&json2).expect("AgentUiRequestPayloadV1 JSON 反序列化不应失败");
    assert_eq!(
        decoded2.timeout_ticks, 600,
        "AgentUiRequestPayloadV1 timeout_ticks 应在 JSON roundtrip 后保持 600"
    );
}

#[test]
#[should_panic(expected = "AgentUiRequest/AgentUiClose 经由 JSON CustomPayload 发送")]
fn agent_ui_request_panics_if_proto_path_is_used() {
    use bong_server::schema::agent_ui::AgentUiRequestPayloadV1;

    let payload = ServerDataPayloadV1::AgentUiRequest(AgentUiRequestPayloadV1 {
        request_id: "pin-req".to_string(),
        target_player: "offline:Kiz".to_string(),
        xml: "<owo-ui><components><label>test</label></components></owo-ui>".to_string(),
        timeout_ticks: 600,
    });
    let _: bong::server_data_envelope::Payload = (&payload).into();
}

#[test]
#[should_panic(expected = "AgentUiRequest/AgentUiClose 经由 JSON CustomPayload 发送")]
fn agent_ui_close_panics_if_proto_path_is_used() {
    use bong_server::schema::agent_ui::AgentUiClosePayloadV1;

    let payload = ServerDataPayloadV1::AgentUiClose(AgentUiClosePayloadV1 {
        request_id: "pin-req".to_string(),
        reason: Some("session_expired".to_string()),
    });
    let _: bong::server_data_envelope::Payload = (&payload).into();
}

#[test]
#[should_panic(expected = "AgentUiResponse 经由 JSON CustomPayload 接收")]
fn agent_ui_response_panics_if_proto_path_is_used() {
    use bong_server::schema::agent_ui::AgentUiActionType;
    use bong_server::schema::client_request::ClientRequestV1;

    let req = ClientRequestV1::AgentUiResponse {
        v: 1,
        request_id: "pin-req".to_string(),
        action: AgentUiActionType::ButtonClick,
        params: std::collections::HashMap::from([(
            "button_id".to_string(),
            "enter_realm".to_string(),
        )]),
    };
    let _: bong::client_request_envelope::Payload = (&req).into();
}

// ─── plan-test-coverage-guards-v1 P0：HalfStepRechallenge JSON-bypass pin ──

/// HalfStepRechallenge is a JSON-bypass variant — it must never reach the proto path.
///
/// MUTATION GUARD: If `HalfStepRechallenge` is accidentally removed from the JSON-bypass
/// list (`is_json_bypass`) and treated as a proto variant without adding a proto arm,
/// `to_proto_bytes()` will call the `From` impl which hits `unreachable!()` — this test
/// will change from #[should_panic] to an actual panic in the wrong place. Keep this test
/// pinned to detect such misclassifications.

#[test]
#[should_panic(expected = "HalfStepRechallenge 经由 JSON CustomPayload 发送")]
fn halfstep_rechallenge_panics_if_proto_path_is_used() {
    use bong_server::schema::server_data::HalfStepRechallengeV1;

    let payload = ServerDataPayloadV1::HalfStepRechallenge(HalfStepRechallengeV1 {
        active: true,
        char_id: "offline:Kiz".to_string(),
        rechallenge_window_until: 50_000,
        at_tick: 1_000,
    });
    let _: bong::server_data_envelope::Payload = (&payload).into();
}

// ─── plan-test-coverage-guards-v1 P0：exhaustive proto encoding guard ────────

/// Returns a minimum-viable fixture for every `ServerDataPayloadV1` variant (127 total).
///
/// **Why a list and not an exhaustive match?**
/// The list itself cannot be made compile-time exhaustive — Rust cannot iterate enum variants.
/// Exhaustiveness is enforced by TWO complementary mechanisms:
///   1. `ServerDataPayloadV1::is_json_bypass(&self)` has NO `_` catch-all; adding a new
///      variant without updating it causes rustc E0004 (compile failure).
///   2. `s2c_fixture_count_matches_variant_count()` cross-checks the fixture list length
///      against the number of `ServerDataType` discriminants derived from `payload_type()`.
///
/// Each element carries `(fixture, expected_bypass: bool)` so the guard test can route it
/// either to the proto-encode assertion or to the should-panic branch.
///
/// MUTATION GUARDS (documented for manual mutation testing):
///   - Delete a proto arm in `From<&ServerDataPayloadV1>` → the `From` match is exhaustive
///     with NO catch-all, so removing an arm is rustc E0004 (compile failure), caught even
///     earlier than a test. The `catch_unwind` in the guard test is defense-in-depth: if a
///     future refactor adds a `_ => unreachable!()` catch-all, a deleted arm would fall
///     through to it at runtime and `s2c_all_proto_variants_encode_without_panic` reds instead.
///   - Flip a bypass variant in `is_json_bypass` from `true` to `false` → guard calls
///     `to_proto_bytes()` → the `From` impl hits the bypass `unreachable!()` arm → RED.
///   - Add a new variant without updating `is_json_bypass` → rustc E0004 → compile failure.
///   - Add a new variant without adding a fixture here → `s2c_fixture_count_matches_variant_count` → RED.
fn s2c_all_fixtures() -> Vec<(ServerDataPayloadV1, bool)> {
    use bong_server::cultivation::components::ColorKind;
    use bong_server::schema::agent_ui::{AgentUiClosePayloadV1, AgentUiRequestPayloadV1};
    use bong_server::schema::alchemy::{
        AlchemyContaminationDataV1, AlchemyFurnaceDataV1, AlchemyOutcomeBucketV1,
        AlchemyOutcomeForecastDataV1, AlchemyOutcomeResolvedDataV1, AlchemyRecipeBookDataV1,
        AlchemySessionDataV1,
    };
    use bong_server::schema::combat_carrier::CarrierStateV1;
    use bong_server::schema::combat_hud::*;
    use bong_server::schema::craft::{
        CraftOutcomeV1, CraftSessionStateV1, RecipeListV1, RecipeUnlockedV1,
    };
    use bong_server::schema::cultivation::InsightOfferV1;
    use bong_server::schema::dugu::DuguPoisonStateV1;
    use bong_server::schema::forge::{
        ForgeBlueprintBookDataV1, ForgeOutcomeBucketV1, ForgeOutcomeDataV1, ForgeSessionDataV1,
        ForgeStepStateDataV1, ForgeStepV1, WeaponForgeStationDataV1,
    };
    use bong_server::schema::identity::IdentityPanelStateV1;
    use bong_server::schema::inventory::{EquippedInventorySnapshotV1, InventoryWeightV1};
    use bong_server::schema::lingtian::LingtianSessionDataV1;
    use bong_server::schema::movement::MovementStateV1;
    use bong_server::schema::movement::{MovementActionV1, MovementZoneKindV1};
    use bong_server::schema::poison_trait::{
        PoisonDoseEventV1, PoisonOverdoseEventV1, PoisonOverdoseSeverityV1, PoisonSideEffectTagV1,
        PoisonTraitStateV1,
    };
    use bong_server::schema::processing::FreshnessUpdateV1;
    use bong_server::schema::realm_vision::{
        FogShapeV1, RealmVisionParamsV1, SpiritualSenseTargetsV1,
    };
    use bong_server::schema::server_data::{
        AnqiHudV1, AscensionQuotaV1, BreakthroughCinematicS2cV1, BurstMeridianEventV1,
        CoffinStateV1, CombatEventFloaterV1, ContainerKindV1, ContainerStateV1, DuguV2HudQiDecayV1,
        DuguV2HudSelfCureV1, DuguV2HudShroudActiveV1, DuguV2HudSkillCastV1, ExtractAbortedReasonV1,
        ExtractAbortedV1, ExtractCompletedV1, ExtractFailedReasonV1, ExtractFailedV1,
        ExtractProgressV1, ExtractStartedV1, FactionWarStateV1, FullPowerChargingStateV1,
        FullPowerExhaustedStateV1, FullPowerReleaseV1, GatheringQualityHintV1,
        GatheringTargetTypeV1, HalfStepRechallengeV1, HeartDemonOfferV1, InventoryMoveRejectedV1,
        KnockbackSyncV1, LootContainerCloseReasonV1, LootContainerCloseV1, LootContainerOpenV1,
        LootContainerSourceKindV1, LootContainerUpdateV1, MineralProbeResultV1, PillBuffStatusV1,
        QiColorObservedV1, RiftPortalDirectionV1, RiftPortalKindV1, RiftPortalRemovedV1,
        RiftPortalStateV1, SearchAbortReasonV1, SearchAbortedV1, SearchCompletedV1,
        SearchProgressV1, SearchStartedV1, SwordBondHudStateV1, TechniqueProficiencyUpdateV1,
        TribulationBroadcastV1, TribulationStateV1, TsyCollapseStartedIpcV1, ZhenmaiHudV1,
    };
    use bong_server::schema::skill::{
        SkillCapChangedPayloadV1, SkillIdV1, SkillLvUpPayloadV1, SkillScrollUsedPayloadV1,
        SkillSnapshotPayloadV1, SkillXpGainPayloadV1, XpGainSourceV1,
    };
    use bong_server::schema::social::{
        ExposureKindV1, GuardianKindV1, NicheGuardianBrokenV1, NicheGuardianFatigueV1,
        NicheIntrusionEventV1, SocialAnonymityPayloadV1, SocialExposureEventV1, SocialFeudEventV1,
        SocialPactEventV1, SocialRenownDeltaV1, SparringInvitePayloadV1, TradeItemSummaryV1,
        TradeOfferPayloadV1,
    };
    use bong_server::schema::spirit_treasure::{
        SpiritTreasureDialoguePayloadV1, SpiritTreasureDialogueToneV1, SpiritTreasureDialogueV1,
        SpiritTreasureStatePayloadV1,
    };
    use bong_server::schema::tuike::FalseSkinStateV1;
    use bong_server::schema::woliu::VortexFieldStateV1;
    use bong_server::schema::world_state::PlayerPowerBreakdown;
    use bong_server::schema::yidao::{HealerNpcAiStateV1, YidaoHudStateV1};
    use bong_server::skill::config::SkillConfigSnapshot;

    macro_rules! fix {
        ($v:expr) => {
            ($v, $v.is_json_bypass())
        };
    }

    // Note: the bypass field is derived from is_json_bypass() on each fixture, so any
    // misclassification in is_json_bypass() will cause the proto path to panic or the
    // should-panic branch to succeed unexpectedly.
    vec![
        fix!(ServerDataPayloadV1::Welcome {
            message: "hello".to_string()
        }),
        fix!(ServerDataPayloadV1::Heartbeat {
            message: "ping".to_string()
        }),
        fix!(ServerDataPayloadV1::Narration {
            narrations: vec![bong_server::schema::narration::Narration {
                text: "test".to_string(),
                scope: bong_server::schema::common::NarrationScope::Broadcast,
                style: bong_server::schema::common::NarrationStyle::Narration,
                kind: None,
                target: None,
            }],
        }),
        fix!(ServerDataPayloadV1::ZoneInfo {
            zone: "spawn".to_string(),
            spirit_qi: 1.0,
            danger_level: 2,
            status: bong_server::schema::world_state::ZoneStatusV1::Normal,
            active_events: None,
            perception_text: None,
        }),
        fix!(ServerDataPayloadV1::EventAlert {
            event: bong_server::schema::common::EventKind::ThunderTribulation,
            message: "watch out".to_string(),
            zone: Some("spawn".to_string()),
            duration_ticks: Some(100),
        }),
        fix!(ServerDataPayloadV1::PlayerState {
            player: None,
            realm: "Induce".to_string(),
            spirit_qi: 1.0,
            spirit_qi_max: 1.0,
            karma: 0.0,
            composite_power: 1.0,
            breakdown: PlayerPowerBreakdown {
                combat: 1.0,
                wealth: 0.0,
                social: 0.0,
                karma: 0.0,
                territory: 0.0,
            },
            zone: "spawn".to_string(),
            local_neg_pressure: None,
            season_state: None,
            social: None,
            zone_spirit_qi: Some(0.42),
        }),
        fix!(ServerDataPayloadV1::CoffinState(CoffinStateV1 {
            in_coffin: false,
            lifespan_rate_multiplier: 1.0,
            coffin_grade: None,
        })),
        fix!(ServerDataPayloadV1::UiOpen {
            ui: None,
            xml: "<root/>".to_string(),
        }),
        fix!(ServerDataPayloadV1::CultivationDetail {
            realm: "Induce".to_string(),
            channel_ids: vec!["lung".to_string(); 20],
            opened: vec![false; 20],
            flow_rate: vec![0.0; 20],
            flow_capacity: vec![10.0; 20],
            integrity: vec![1.0; 20],
            open_progress: vec![0.0; 20],
            cracks_count: vec![0; 20],
            contamination_total: 0.0,
            lifespan: None,
            recent_skill_milestones_summary: String::new(),
            skill_milestones: vec![],
            qi_color_main: ColorKind::Intricate,
            qi_color_secondary: None,
            qi_color_chaotic: false,
            qi_color_hunyuan: false,
            practice_weights: vec![],
            target_meridian: None,
            body_plan_id: "humanoid".to_string(),
            race_id: String::new(),
            form_race_id: String::new(),
            form_body_plan_id: String::new(),
            intrinsic_is_humanoid: false,
            form_is_humanoid: false,
        }),
        fix!(ServerDataPayloadV1::QiColorObserved(QiColorObservedV1 {
            observer: "offline:Kiz".to_string(),
            observed: "offline:Azure".to_string(),
            main: ColorKind::Intricate,
            secondary: None,
            is_chaotic: false,
            is_hunyuan: false,
            realm_diff: 0,
        })),
        fix!(ServerDataPayloadV1::InventorySnapshot(Box::new(
            bong_server::schema::inventory::InventorySnapshotV1 {
                revision: 1,
                containers: vec![],
                placed_items: vec![],
                equipped: EquippedInventorySnapshotV1::default(),
                hotbar: vec![],
                bone_coins: 0,
                weight: InventoryWeightV1 {
                    current: 0.0,
                    max: 100.0
                },
                realm: "Induce".to_string(),
                qi_current: 0.0,
                qi_max: 100.0,
                body_level: 0.0,
            }
        ))),
        fix!(ServerDataPayloadV1::InventoryEvent(Box::new(
            bong_server::schema::inventory::InventoryEventV1::StackChanged {
                revision: 1,
                instance_id: 42,
                stack_count: 5,
            }
        ))),
        fix!(ServerDataPayloadV1::DroppedLootSync(vec![])),
        fix!(ServerDataPayloadV1::RemainsSync(vec![
            bong_server::schema::server_data::RemainsEntryV1 {
                remains_id: "3fa85f64-5717-4562-b3fc-2c963f66afa6".to_string(),
                world_pos: [1.0, 64.0, 1.0],
                dimension: "minecraft:overworld".to_string(),
                display_name: "遗骸".to_string(),
                item_count: 3,
                bone_coins: 12,
            }
        ])),
        // plan-race-system-v1 P2a：动态部位 / 经脉面板布局元数据。
        fix!(ServerDataPayloadV1::BodyPlanLayout(
            bong_server::schema::server_data::BodyPlanLayoutV1 {
                body_plan_id: "humanoid".to_string(),
                silhouette: vec![bong_server::schema::server_data::BodyPlanSilhouettePartV1 {
                    part_id: "chest".to_string(),
                    polygon: vec![
                        bong_server::schema::server_data::BodyPlanPoint2V1 { x: 0.37, y: 0.14 },
                        bong_server::schema::server_data::BodyPlanPoint2V1 { x: 0.63, y: 0.14 },
                        bong_server::schema::server_data::BodyPlanPoint2V1 { x: 0.63, y: 0.28 },
                        bong_server::schema::server_data::BodyPlanPoint2V1 { x: 0.37, y: 0.28 },
                    ],
                }],
                anchors: vec![bong_server::schema::server_data::BodyPlanPartAnchorV1 {
                    part_id: "chest".to_string(),
                    point: bong_server::schema::server_data::BodyPlanPoint2V1 { x: 0.5, y: 0.2 },
                }],
                meridian_paths: vec![bong_server::schema::server_data::BodyPlanMeridianPathV1 {
                    channel_id: "ren".to_string(),
                    points: vec![
                        bong_server::schema::server_data::BodyPlanPoint2V1 { x: 0.49, y: 0.42 },
                        bong_server::schema::server_data::BodyPlanPoint2V1 { x: 0.49, y: 0.13 },
                    ],
                }],
                part_display_map: vec![
                    bong_server::schema::server_data::BodyPlanPartDisplayMappingV1 {
                        server_part_id: "head".to_string(),
                        display_segment_id: "head".to_string(),
                    }
                ],
                hud_anchors: vec![bong_server::schema::server_data::BodyPlanPartAnchorV1 {
                    part_id: "chest".to_string(),
                    point: bong_server::schema::server_data::BodyPlanPoint2V1 { x: 0.5, y: 0.19 },
                }],
            }
        )),
        // plan-race-system-v1 P3c：种族门元数据表（含三档 gate pin）。
        fix!(ServerDataPayloadV1::RaceGateMeta(
            bong_server::schema::server_data::RaceGateMetaV1 {
                item_wearer_race: vec![bong_server::schema::server_data::RaceGateMetaEntryV1 {
                    id: "armor_whale_plate".to_string(),
                    gate: bong_server::schema::server_data::RaceGateWireV1 {
                        kind: "species".to_string(),
                        species: vec!["whale".to_string()],
                    },
                }],
                technique_required_race: vec![
                    bong_server::schema::server_data::RaceGateMetaEntryV1 {
                        id: "sword.cleave".to_string(),
                        gate: bong_server::schema::server_data::RaceGateWireV1 {
                            kind: "humanoid".to_string(),
                            species: vec![],
                        },
                    },
                ],
            }
        )),
        // plan-race-system-v1 P4：易形状态快照（此 fixture 列表每变体恰一条，见
        // s2c_fixture_count_matches_variant_count 的 1:1 约束；delta/active=false 的
        // 专项 round-trip 见 morph_state_delta_release_proto_round_trips）。
        fix!(ServerDataPayloadV1::MorphState(
            bong_server::schema::server_data::MorphStateV1 {
                mode: "full".to_string(),
                entries: vec![bong_server::schema::server_data::MorphStateEntryV1 {
                    entity_id: 42,
                    model_kind: 1,
                    form_race_id: "whale".to_string(),
                    form_body_plan_id: "whale".to_string(),
                    active: true,
                }],
            }
        )),
        fix!(ServerDataPayloadV1::BotanyHarvestProgress {
            session_id: "ses:1".to_string(),
            target_id: "entity:1".to_string(),
            target_name: "草".to_string(),
            plant_kind: "herb".to_string(),
            mode: "standard".to_string(),
            progress: 0.5,
            auto_selectable: false,
            request_pending: false,
            interrupted: false,
            completed: false,
            detail: String::new(),
            hazard_hints: vec![],
            target_pos: None,
        }),
        fix!(ServerDataPayloadV1::BotanyPlantV2RenderProfiles(vec![])),
        fix!(ServerDataPayloadV1::MiningProgress {
            session_id: "ses:2".to_string(),
            ore_pos: [0, 64, 0],
            progress: 0.2,
            interrupted: false,
            completed: false,
            mineral_id: "cu_tie".to_string(),
            display_name: "粗铁".to_string(),
        }),
        fix!(ServerDataPayloadV1::LumberProgress {
            session_id: "ses:3".to_string(),
            log_pos: [0, 64, 0],
            progress: 0.3,
            interrupted: false,
            completed: false,
            detail: String::new(),
        }),
        fix!(ServerDataPayloadV1::GatheringSession {
            session_id: "ses:4".to_string(),
            progress_ticks: 10,
            total_ticks: 40,
            target_name: "草".to_string(),
            target_type: GatheringTargetTypeV1::Herb,
            quality_hint: GatheringQualityHintV1::Normal,
            tool_used: None,
            interrupted: false,
            completed: false,
        }),
        fix!(ServerDataPayloadV1::BotanySkill {
            level: 1,
            xp: 0,
            xp_to_next_level: 100,
            auto_unlock_level: 3,
        }),
        fix!(ServerDataPayloadV1::AlchemyFurnace(Box::new(
            AlchemyFurnaceDataV1 {
                pos: None,
                tier: 1,
                integrity: 1.0,
                integrity_max: 1.0,
                owner_name: String::new(),
                has_session: false,
            }
        ))),
        fix!(ServerDataPayloadV1::AlchemySession(Box::new(
            AlchemySessionDataV1 {
                recipe_id: None,
                active: false,
                elapsed_ticks: 0,
                target_ticks: 0,
                temp_current: 0.0,
                temp_target: 0.0,
                temp_band: 0.0,
                qi_injected: 0.0,
                qi_target: 0.0,
                status_label: String::new(),
                stages: vec![],
                interventions_recent: vec![],
            }
        ))),
        fix!(ServerDataPayloadV1::AlchemyOutcomeForecast(Box::new(
            AlchemyOutcomeForecastDataV1 {
                perfect_pct: 0.0,
                good_pct: 0.0,
                flawed_pct: 0.0,
                waste_pct: 1.0,
                explode_pct: 0.0,
                perfect_note: String::new(),
                good_note: String::new(),
                flawed_note: String::new(),
            }
        ))),
        fix!(ServerDataPayloadV1::AlchemyOutcomeResolved(Box::new(
            AlchemyOutcomeResolvedDataV1 {
                bucket: AlchemyOutcomeBucketV1::Waste,
                recipe_id: None,
                pill: None,
                quality: None,
                toxin_amount: None,
                toxin_color: None,
                qi_gain: None,
                side_effect_tag: None,
                flawed_path: false,
                damage: None,
                meridian_crack: None,
            }
        ))),
        fix!(ServerDataPayloadV1::AlchemyRecipeBook(Box::new(
            AlchemyRecipeBookDataV1 {
                learned: vec![],
                current_index: 0,
            }
        ))),
        fix!(ServerDataPayloadV1::AlchemyContamination(Box::new(
            AlchemyContaminationDataV1 {
                levels: vec![],
                metabolism_note: String::new(),
            }
        ))),
        fix!(ServerDataPayloadV1::CombatHudState(CombatHudStateV1 {
            hp_percent: 1.0,
            qi_percent: 1.0,
            stamina_percent: 1.0,
            combat_active: false,
            derived: DerivedAttrFlagsV1::default(),
        })),
        fix!(ServerDataPayloadV1::WoundsSnapshot(WoundsSnapshotV1 {
            wounds: vec![]
        })),
        fix!(ServerDataPayloadV1::DefenseWindow(DefenseWindowV1 {
            duration_ms: 200,
            started_at_ms: 0,
            expires_at_ms: 200,
        })),
        fix!(ServerDataPayloadV1::CastSync(CastSyncV1 {
            phase: CastPhaseV1::Idle,
            slot: 0,
            duration_ms: 0,
            started_at_ms: 0,
            outcome: CastOutcomeV1::None,
        })),
        fix!(ServerDataPayloadV1::QuickSlotConfig(QuickSlotConfigV1 {
            slots: vec![None; bong_server::combat::components::QuickSlotBindings::SLOT_COUNT],
            cooldown_until_ms: vec![
                0;
                bong_server::combat::components::QuickSlotBindings::SLOT_COUNT
            ],
            ack_request_id: None,
            bind_accepted: None,
        })),
        fix!(ServerDataPayloadV1::SkillBarConfig(SkillBarConfigV1 {
            slots: vec![None; bong_server::combat::components::SkillBarBindings::SLOT_COUNT],
            cooldown_until_ms: vec![
                0;
                bong_server::combat::components::SkillBarBindings::SLOT_COUNT
            ],
        })),
        fix!(ServerDataPayloadV1::TechniquesSnapshot(
            TechniquesSnapshotV1 { entries: vec![] }
        )),
        fix!(ServerDataPayloadV1::SkillConfigSnapshot(
            SkillConfigSnapshot {
                configs: Default::default(),
            }
        )),
        fix!(ServerDataPayloadV1::UnlocksSync(UnlocksSyncV1::default())),
        fix!(ServerDataPayloadV1::DerivedAttrsSync(DerivedAttrsSyncV1 {
            flying: false,
            flying_qi_remaining: 0.0,
            flying_force_descent_at_ms: 0,
            phasing: false,
            phasing_until_ms: 0,
            tribulation_locked: false,
            tribulation_stage: String::new(),
            throughput_peak_norm: 0.0,
            tuike_layers: 0,
            vortex_active: false,
        })),
        fix!(ServerDataPayloadV1::EventStreamPush(EventStreamPushV1 {
            channel: EventChannelV1::Combat,
            priority: EventPriorityV1::P1Important,
            source_tag: String::new(),
            text: "test".to_string(),
            color: 0,
            created_at_ms: 0,
        })),
        fix!(ServerDataPayloadV1::WeaponEquipped(WeaponEquippedV1 {
            slot: "main_hand".to_string(),
            weapon: None,
        })),
        fix!(ServerDataPayloadV1::WeaponBroken(WeaponBrokenV1 {
            instance_id: 1,
            template_id: "sword_iron".to_string(),
        })),
        fix!(ServerDataPayloadV1::ShieldBroken(ShieldBrokenV1 {
            instance_id: 2,
            template_id: "shield_wood".to_string(),
        })),
        fix!(ServerDataPayloadV1::ShieldBlockHit(ShieldBlockHitV1 {
            template_id: "shield_wood".to_string(),
        })),
        fix!(ServerDataPayloadV1::TreasureEquipped(TreasureEquippedV1 {
            slot: "treasure_belt_0".to_string(),
            treasure: None,
        })),
        fix!(ServerDataPayloadV1::VortexState(VortexFieldStateV1 {
            caster: "entity:1".to_string(),
            active: false,
            center: [0.0, 64.0, 0.0],
            radius: 1.5,
            delta: 0.25,
            env_qi_at_cast: 0.9,
            maintain_remaining_ticks: 0,
            intercepted_count: 0,
            active_skill_id: "woliu.hold".to_string(),
            charge_progress: 0.0,
            cooldown_until_ms: 0,
            backfire_level: String::new(),
            turbulence_radius: 1.0,
            turbulence_intensity: 0.0,
            turbulence_until_ms: 0,
        })),
        fix!(ServerDataPayloadV1::DuguPoisonState(DuguPoisonStateV1 {
            target: "offline:Kiz".to_string(),
            active: false,
            meridian_id: "lung".to_string(),
            attacker: "offline:Azure".to_string(),
            attached_at_tick: 0,
            poisoner_realm_tier: 1,
            loss_per_tick: 0.0,
            flow_capacity_after: 10.0,
            qi_max_after: 100.0,
            server_tick: 0,
        })),
        fix!(ServerDataPayloadV1::PoisonDoseEvent(PoisonDoseEventV1 {
            v: 1,
            player_entity_id: 1,
            dose_amount: 1.0,
            side_effect_tag: PoisonSideEffectTagV1::QiFocusDrift2h,
            poison_level_after: 1.0,
            digestion_after: 0.0,
            at_tick: 1,
        })),
        fix!(ServerDataPayloadV1::PoisonOverdoseEvent(
            PoisonOverdoseEventV1 {
                v: 1,
                player_entity_id: 1,
                severity: PoisonOverdoseSeverityV1::Mild,
                overflow: 1.0,
                lifespan_penalty_years: 0.0,
                micro_tear_probability: 0.0,
                at_tick: 1,
            }
        )),
        fix!(ServerDataPayloadV1::PoisonTraitState(PoisonTraitStateV1 {
            v: 1,
            player_entity_id: 1,
            poison_toxicity: 0.0,
            digestion_current: 0.0,
            digestion_capacity: 100.0,
            toxicity_tier_unlocked: false,
        })),
        fix!(ServerDataPayloadV1::CarrierState(CarrierStateV1 {
            carrier: "none".to_string(),
            phase: bong_server::schema::combat_carrier::CarrierChargePhaseV1::Idle,
            progress: 0.0,
            sealed_qi: 0.0,
            sealed_qi_initial: 0.0,
            half_life_remaining_ticks: 0,
            item_instance_id: None,
        })),
        fix!(ServerDataPayloadV1::FalseSkinState(FalseSkinStateV1 {
            target_id: "offline:Kiz".to_string(),
            kind: None,
            layers_remaining: 0,
            contam_capacity_per_layer: 0.0,
            absorbed_contam: 0.0,
            equipped_at_tick: 0,
            layers: vec![],
        })),
        fix!(ServerDataPayloadV1::LingtianSession(Box::new(
            LingtianSessionDataV1 {
                active: false,
                kind: bong_server::schema::lingtian::LingtianSessionKindV1::Till,
                pos: [0, 64, 0],
                elapsed_ticks: 0,
                target_ticks: 100,
                plant_id: None,
                source: None,
                dye_contamination: None,
                dye_contamination_warning: false,
            }
        ))),
        fix!(ServerDataPayloadV1::DeathScreen {
            visible: false,
            cause: "fall".to_string(),
            luck_remaining: 1.0,
            final_words: vec![],
            countdown_until_ms: 0,
            can_reincarnate: true,
            can_terminate: false,
            stage: None,
            death_number: None,
            zone_kind: None,
            lifespan: None,
            cinematic: None,
        }),
        fix!(ServerDataPayloadV1::TerminateScreen {
            visible: true,
            final_words: String::new(),
            epilogue: String::new(),
            archetype_suggestion: String::new(),
            summary: Some(bong_server::schema::server_data::TerminationSummaryV1 {
                character_name: "LastName".into(),
                realm: "Condense".into(),
                death_count: 4,
                years_lived: Some(47.5),
                qi_max: Some(88.0),
                health_max: Some(72.0),
                meridians_open: Some(3),
                techniques_learned: None,
            }),
        }),
        fix!(ServerDataPayloadV1::RiftPortalState(RiftPortalStateV1 {
            entity_id: 1,
            kind: RiftPortalKindV1::MainRift,
            direction: RiftPortalDirectionV1::Exit,
            family_id: "tsy_01".to_string(),
            world_pos: [0.0, 64.0, 0.0],
            trigger_radius: 2.0,
            current_extract_ticks: 0,
            activation_window_end: None,
        })),
        fix!(ServerDataPayloadV1::RiftPortalRemoved(
            RiftPortalRemovedV1 { entity_id: 1 }
        )),
        fix!(ServerDataPayloadV1::ExtractStarted(ExtractStartedV1 {
            player_id: "offline:Kiz".to_string(),
            portal_entity_id: 1,
            portal_kind: RiftPortalKindV1::MainRift,
            required_ticks: 160,
            at_tick: 1,
        })),
        fix!(ServerDataPayloadV1::ExtractProgress(ExtractProgressV1 {
            player_id: "offline:Kiz".to_string(),
            portal_entity_id: 1,
            elapsed_ticks: 1,
            required_ticks: 160,
        })),
        fix!(ServerDataPayloadV1::ExtractCompleted(ExtractCompletedV1 {
            player_id: "offline:Kiz".to_string(),
            portal_kind: RiftPortalKindV1::MainRift,
            family_id: "tsy_01".to_string(),
            exit_world_pos: [0.0, 64.0, 0.0],
            at_tick: 161,
        })),
        fix!(ServerDataPayloadV1::ExtractAborted(ExtractAbortedV1 {
            player_id: "offline:Kiz".to_string(),
            reason: ExtractAbortedReasonV1::PortalOccupied,
        })),
        fix!(ServerDataPayloadV1::ExtractFailed(ExtractFailedV1 {
            player_id: "offline:Kiz".to_string(),
            reason: ExtractFailedReasonV1::SpiritQiDrained,
        })),
        fix!(ServerDataPayloadV1::TsyCollapseStartedIpc(
            TsyCollapseStartedIpcV1 {
                family_id: "tsy_01".to_string(),
                at_tick: 1,
                remaining_ticks: 100,
                collapse_tear_entity_ids: vec![],
            }
        )),
        fix!(ServerDataPayloadV1::ContainerState(ContainerStateV1 {
            entity_id: 1,
            kind: ContainerKindV1::StoragePouch,
            family_id: "tsy_01".to_string(),
            world_pos: [0.0, 64.0, 0.0],
            visual_entity_id: Some(1001),
            locked: None,
            depleted: false,
            searched_by_player_id: None,
        })),
        fix!(ServerDataPayloadV1::SearchStarted(SearchStartedV1 {
            player_id: "offline:Kiz".to_string(),
            container_entity_id: 1,
            required_ticks: 100,
            at_tick: 1,
        })),
        fix!(ServerDataPayloadV1::SearchProgress(SearchProgressV1 {
            player_id: "offline:Kiz".to_string(),
            container_entity_id: 1,
            elapsed_ticks: 10,
            required_ticks: 100,
        })),
        fix!(ServerDataPayloadV1::SearchCompleted(SearchCompletedV1 {
            player_id: "offline:Kiz".to_string(),
            container_entity_id: 1,
            family_id: "tsy_01".to_string(),
            loot_preview: vec![],
            at_tick: 101,
        })),
        fix!(ServerDataPayloadV1::SearchAborted(SearchAbortedV1 {
            player_id: "offline:Kiz".to_string(),
            container_entity_id: 1,
            reason: SearchAbortReasonV1::Cancelled,
            at_tick: 50,
        })),
        fix!(ServerDataPayloadV1::SkillXpGain(Box::new(
            SkillXpGainPayloadV1::new(
                1,
                SkillIdV1::Herbalism,
                10,
                XpGainSourceV1::Action {
                    plan_id: "p0".to_string(),
                    action: "harvest".to_string(),
                },
            )
        ))),
        fix!(ServerDataPayloadV1::SkillLvUp(SkillLvUpPayloadV1::new(
            1,
            SkillIdV1::Herbalism,
            2
        ))),
        fix!(ServerDataPayloadV1::SkillCapChanged(
            SkillCapChangedPayloadV1::new(1, SkillIdV1::Herbalism, 5)
        )),
        fix!(ServerDataPayloadV1::SkillScrollUsed(Box::new(
            SkillScrollUsedPayloadV1::new(1, "scroll:herb:1", SkillIdV1::Herbalism, 50, false)
        ))),
        fix!(ServerDataPayloadV1::SkillSnapshot(Box::new(
            SkillSnapshotPayloadV1 {
                v: 1,
                char_id: 1,
                skills: Default::default(),
                consumed_scrolls: vec![],
            }
        ))),
        fix!(ServerDataPayloadV1::ForgeStation(Box::new(
            WeaponForgeStationDataV1 {
                station_id: "forge:1".to_string(),
                tier: 1,
                integrity: 1.0,
                owner_name: String::new(),
                has_session: false,
                station_pos_x: 0,
                station_pos_y: 64,
                station_pos_z: 0,
            }
        ))),
        fix!(ServerDataPayloadV1::ForgeSession(Box::new(
            ForgeSessionDataV1 {
                session_id: 1,
                blueprint_id: "bp:sword".to_string(),
                blueprint_name: "凡铁剑".to_string(),
                active: false,
                current_step: ForgeStepV1::Billet,
                step_index: 0,
                achieved_tier: 0,
                step_state: ForgeStepStateDataV1::None,
            }
        ))),
        fix!(ServerDataPayloadV1::ForgeOutcome(Box::new(
            ForgeOutcomeDataV1 {
                session_id: 1,
                blueprint_id: "bp:sword".to_string(),
                bucket: ForgeOutcomeBucketV1::Good,
                weapon_item: None,
                quality: 0.5,
                color: None,
                side_effects: vec![],
                achieved_tier: 1,
                flawed_path: false,
            }
        ))),
        fix!(ServerDataPayloadV1::ForgeBlueprintBook(Box::new(
            ForgeBlueprintBookDataV1 {
                learned: vec![],
                current_index: 0,
            }
        ))),
        fix!(ServerDataPayloadV1::TribulationState(
            TribulationStateV1::clear()
        )),
        fix!(ServerDataPayloadV1::TribulationBroadcast(
            TribulationBroadcastV1::clear()
        )),
        fix!(ServerDataPayloadV1::AscensionQuota(AscensionQuotaV1::new(
            0, 3
        ))),
        fix!(ServerDataPayloadV1::HeartDemonOffer(HeartDemonOfferV1 {
            offer_id: "hd:1".to_string(),
            trigger_id: "hd:1".to_string(),
            trigger_label: "心魔".to_string(),
            realm_label: "渡虚".to_string(),
            composure: 0.5,
            quota_remaining: 1,
            quota_total: 1,
            expires_at_ms: 9_999_999_999_999,
            choices: vec![],
        })),
        fix!(ServerDataPayloadV1::BurstMeridianEvent(
            BurstMeridianEventV1 {
                skill: "beng_quan".to_string(),
                caster: "offline:Kiz".to_string(),
                target: None,
                tick: 1,
                overload_ratio: 1.0,
                integrity_snapshot: 1.0,
            }
        )),
        fix!(ServerDataPayloadV1::BreakthroughCinematic(
            BreakthroughCinematicS2cV1 {
                actor_id: "offline:Kiz".to_string(),
                phase: "apex".to_string(),
                phase_tick: 0,
                phase_duration_ticks: 80,
                realm_from: "Condense".to_string(),
                realm_to: "Solidify".to_string(),
                result: "success".to_string(),
                interrupted: false,
                world_pos: [0.0, 64.0, 0.0],
                visible_radius_blocks: 64.0,
                global: false,
                distant_billboard: false,
                particle_density: 1.0,
                intensity: 1.0,
                season_overlay: "adaptive".to_string(),
                style: "golden_core".to_string(),
                at_tick: 1,
            }
        )),
        fix!(ServerDataPayloadV1::FullPowerChargingState(
            FullPowerChargingStateV1 {
                caster_uuid: "00000000-0000-0000-0000-000000000001".to_string(),
                active: false,
                qi_committed: 0.0,
                target_qi: 100.0,
                started_tick: 0,
            }
        )),
        fix!(ServerDataPayloadV1::FullPowerRelease(FullPowerReleaseV1 {
            caster_uuid: "00000000-0000-0000-0000-000000000001".to_string(),
            target_uuid: None,
            qi_released: 100.0,
            tick: 1,
            hit_position: None,
        })),
        fix!(ServerDataPayloadV1::FullPowerExhaustedState(
            FullPowerExhaustedStateV1 {
                caster_uuid: "00000000-0000-0000-0000-000000000001".to_string(),
                active: false,
                started_tick: 0,
                recovery_at_tick: 0,
            }
        )),
        fix!(ServerDataPayloadV1::SocialAnonymity(
            SocialAnonymityPayloadV1 {
                viewer: "offline:Kiz".to_string(),
                remotes: vec![],
            }
        )),
        fix!(ServerDataPayloadV1::SocialExposure(SocialExposureEventV1 {
            v: 1,
            actor: "char:kiz".to_string(),
            kind: ExposureKindV1::Chat,
            witnesses: vec![],
            tick: 1,
            zone: None,
        })),
        fix!(ServerDataPayloadV1::SocialPact(SocialPactEventV1 {
            v: 1,
            left: "char:a".to_string(),
            right: "char:b".to_string(),
            terms: "mutual".to_string(),
            tick: 1,
            broken: false,
        })),
        fix!(ServerDataPayloadV1::SocialFeud(SocialFeudEventV1 {
            v: 1,
            left: "char:a".to_string(),
            right: "char:b".to_string(),
            tick: 1,
            place: None,
        })),
        fix!(ServerDataPayloadV1::SocialRenownDelta(
            SocialRenownDeltaV1 {
                v: 1,
                char_id: "char:kiz".to_string(),
                fame_delta: 1,
                notoriety_delta: 0,
                tags_added: vec![],
                tick: 1,
                reason: "pact".to_string(),
            }
        )),
        fix!(ServerDataPayloadV1::IdentityPanelState(
            IdentityPanelStateV1 {
                active_identity_id: 0,
                last_switch_tick: 0,
                cooldown_remaining_ticks: 0,
                identities: vec![],
            }
        )),
        fix!(ServerDataPayloadV1::NicheIntrusion(NicheIntrusionEventV1 {
            v: 1,
            niche_pos: [0, 64, 0],
            intruder_id: "char:thief".to_string(),
            items_taken: vec![],
            taint_delta: 0.0,
        })),
        fix!(ServerDataPayloadV1::NicheGuardianFatigue(
            NicheGuardianFatigueV1 {
                v: 1,
                guardian_kind: GuardianKindV1::Puppet,
                charges_remaining: 1,
            }
        )),
        fix!(ServerDataPayloadV1::NicheGuardianBroken(
            NicheGuardianBrokenV1 {
                v: 1,
                guardian_kind: GuardianKindV1::Puppet,
                intruder_id: "char:thief".to_string(),
            }
        )),
        fix!(ServerDataPayloadV1::SparringInvite(
            SparringInvitePayloadV1 {
                invite_id: "inv:1".to_string(),
                initiator: "char:a".to_string(),
                target: "char:b".to_string(),
                realm_band: "condense".to_string(),
                breath_hint: "匀".to_string(),
                terms: "standard".to_string(),
                expires_at_ms: 9_999_999_999_999,
            }
        )),
        fix!(ServerDataPayloadV1::TradeOffer(TradeOfferPayloadV1 {
            offer_id: "trade:1".to_string(),
            initiator: "char:a".to_string(),
            target: "char:b".to_string(),
            offered_item: TradeItemSummaryV1 {
                instance_id: 1,
                item_id: "bone_coin".to_string(),
                display_name: "骨币".to_string(),
                stack_count: 10,
            },
            requested_items: vec![],
            expires_at_ms: 9_999_999_999_999,
        })),
        fix!(ServerDataPayloadV1::RealmVisionParams(
            RealmVisionParamsV1 {
                fog_start: 30.0,
                fog_end: 60.0,
                fog_color_rgb: 0,
                fog_shape: FogShapeV1::Cylinder,
                vignette_alpha: 0.0,
                tint_color_argb: 0,
                particle_density: 0.0,
                transition_ticks: 0,
                server_view_distance_chunks: 4,
                post_fx_sharpen: 0.0,
            }
        )),
        fix!(ServerDataPayloadV1::SpiritualSenseTargets(
            SpiritualSenseTargetsV1 {
                generation: 0,
                entries: vec![],
            }
        )),
        fix!(ServerDataPayloadV1::HealerNpcAiState(HealerNpcAiStateV1 {
            healer_id: "npc:doc".to_string(),
            active_action: "idle".to_string(),
            queue_len: 0,
            reputation: 0,
            retreating: false,
        })),
        fix!(ServerDataPayloadV1::YidaoHudState(YidaoHudStateV1 {
            healer_id: "npc:doc".to_string(),
            reputation: 0,
            peace_mastery: 0.0,
            karma: 0.0,
            active_skill: None,
            patient_ids: vec![],
            patient_hp_percent: None,
            patient_contam_total: None,
            severed_meridian_count: 0,
            contract_count: 0,
            mass_preview_count: 0,
        })),
        fix!(ServerDataPayloadV1::MovementState(MovementStateV1 {
            current_speed_multiplier: 1.0,
            stamina_cost_active: false,
            movement_action: MovementActionV1::None,
            zone_kind: MovementZoneKindV1::Normal,
            dash_cooldown_remaining_ticks: 0,
            dash_cooldown_total_ticks: 0,
            hitbox_height_blocks: 1.8,
            stamina_current: 100.0,
            stamina_max: 100.0,
            low_stamina: false,
            last_action_tick: None,
            rejected_action: None,
        })),
        fix!(ServerDataPayloadV1::SpiritTreasureState(
            SpiritTreasureStatePayloadV1 { treasures: vec![] }
        )),
        fix!(ServerDataPayloadV1::SpiritTreasureDialogue(
            SpiritTreasureDialoguePayloadV1 {
                dialogue: SpiritTreasureDialogueV1 {
                    v: 1,
                    request_id: "req:1".to_string(),
                    character_id: "offline:Kiz".to_string(),
                    treasure_id: "jade_pendant".to_string(),
                    text: "汝何求".to_string(),
                    tone: SpiritTreasureDialogueToneV1::Cold,
                    affinity_delta: 0.0,
                },
                display_name: "翠玉佩".to_string(),
                zone: "spawn".to_string(),
            }
        )),
        fix!(ServerDataPayloadV1::CraftRecipeList(Box::new(
            RecipeListV1 {
                v: 1,
                player_id: "offline:Kiz".to_string(),
                recipes: vec![],
                ts: 0,
            }
        ))),
        fix!(ServerDataPayloadV1::CraftSessionState(
            CraftSessionStateV1 {
                v: 1,
                player_id: "offline:Kiz".to_string(),
                active: false,
                recipe_id: None,
                elapsed_ticks: 0,
                total_ticks: 0,
                completed_count: 0,
                total_count: 0,
                ts: 0,
            }
        )),
        fix!(ServerDataPayloadV1::CraftOutcome(
            CraftOutcomeV1::Completed {
                v: 1,
                player_id: "offline:Kiz".to_string(),
                recipe_id: "craft.example".to_string(),
                output_template: "example_item".to_string(),
                output_count: 1,
                completed_at_tick: 1,
                ts: 0,
            }
        )),
        fix!(ServerDataPayloadV1::RecipeUnlocked(RecipeUnlockedV1 {
            v: 1,
            player_id: "offline:Kiz".to_string(),
            recipe_id: "craft.example".to_string(),
            source: bong_server::schema::craft::UnlockEventSourceV1::Insight {
                trigger: bong_server::schema::craft::InsightTriggerV1::NearDeath,
            },
            unlocked_at_tick: 1,
            ts: 0,
        })),
        fix!(ServerDataPayloadV1::WorkbenchOpen {
            entity_id: 1,
            position: [0, 64, 0],
        }),
        fix!(ServerDataPayloadV1::CombatEventFloater(
            CombatEventFloaterV1 { events: vec![] }
        )),
        fix!(ServerDataPayloadV1::KnockbackSync(KnockbackSyncV1 {
            distance_blocks: 1.0,
            velocity_blocks_per_tick: 0.2,
            duration_ticks: 5,
            kinetic_energy: 5.0,
            collision_damage: None,
            chain_depth: 0,
            block_broken: false,
        })),
        fix!(ServerDataPayloadV1::TechniqueProficiencyUpdate(
            TechniqueProficiencyUpdateV1 {
                technique_id: "sword.cleave".to_string(),
                proficiency: 0.1,
                gain: 0.001,
            }
        )),
        fix!(ServerDataPayloadV1::PillBuffStatus(PillBuffStatusV1 {
            buff_id: "huo_xue_dan".to_string(),
            remaining_ticks: 100,
            effect_multiplier: 1.0,
        })),
        fix!(ServerDataPayloadV1::LootContainerOpen(
            LootContainerOpenV1 {
                session_id: 1,
                source_kind: LootContainerSourceKindV1::SupplyCoffin {
                    grade: "common".to_string(),
                },
                rows: 3,
                cols: 4,
                placed_items: vec![],
                timeout_wall_secs: 9_999_999,
            }
        )),
        fix!(ServerDataPayloadV1::LootContainerUpdate(
            LootContainerUpdateV1 {
                session_id: 1,
                placed_items: vec![],
            }
        )),
        fix!(ServerDataPayloadV1::LootContainerClose(
            LootContainerCloseV1 {
                session_id: 1,
                reason: LootContainerCloseReasonV1::Timeout,
            }
        )),
        fix!(ServerDataPayloadV1::FactionWarState(FactionWarStateV1 {
            war_id: 1,
            zone: "spawn".to_string(),
            region_descriptor: "东部荒野".to_string(),
            phase: "emerging".to_string(),
            groups: vec![],
            enlist_count: 0,
            mercenary_count: 0,
            intercept_count: 0,
            spectate_count: 0,
            winner_group: None,
            loser_group: None,
        })),
        fix!(ServerDataPayloadV1::AnqiHud(AnqiHudV1 {
            kind: AnqiHudKindV1::Echo,
            echo_count: 0,
            aim_progress: 0.0,
            charge_progress: 0.0,
            abrasion_container: String::new(),
            abrasion_qi_payload: 0.0,
            tick: 1,
        })),
        fix!(ServerDataPayloadV1::DuguV2SkillCast(DuguV2HudSkillCastV1 {
            kind: "eclipse".to_string(),
            caster: "offline:Kiz".to_string(),
            target: None,
            taint_tier: None,
            reveal_probability: 0.0,
            tick: 1,
        })),
        fix!(ServerDataPayloadV1::DuguV2SelfCure(DuguV2HudSelfCureV1 {
            caster: "offline:Kiz".to_string(),
            gain_percent: 0.0,
            self_revealed: false,
            tick: 1,
        })),
        fix!(ServerDataPayloadV1::DuguV2ShroudActive(
            DuguV2HudShroudActiveV1 {
                caster: "offline:Kiz".to_string(),
                strength: 0.0,
                expires_at_tick: 0,
                tick: 1,
            }
        )),
        fix!(ServerDataPayloadV1::PermanentQiMaxDecayApplied(
            DuguV2HudQiDecayV1 {
                target: "offline:Kiz".to_string(),
                loss: 1.0,
                qi_max_after: 99.0,
                tick: 1,
            }
        )),
        fix!(ServerDataPayloadV1::SwordBondHudState(
            SwordBondHudStateV1 {
                active: false,
                grade_index: 0,
                grade_name: "凡铁".to_string(),
                stored_qi_ratio: 0.0,
                bond_strength: 0.0,
                heaven_gate_ready: false,
            }
        )),
        fix!(ServerDataPayloadV1::ZhenmaiHud(ZhenmaiHudV1 {
            skill_id: "sever_chain".to_string(),
            meridian_id: "Heart".to_string(),
            contam_removed: 0.0,
            remaining_points: 0,
            damage_reduction: 0.0,
            k_drain: 1.5,
            duration_ms: 60_000,
            tick: 1,
        })),
        fix!(ServerDataPayloadV1::MineralProbeResult(
            MineralProbeResultV1 {
                kind: "denied".to_string(),
                mineral_id: None,
                remaining_units: None,
                display_name_zh: None,
                denial_reason: Some("no_skill".to_string()),
            }
        )),
        fix!(ServerDataPayloadV1::FreshnessUpdate(FreshnessUpdateV1 {
            item_uuid: "42".to_string(),
            freshness: 1.0,
            profile_name: "default".to_string(),
        })),
        fix!(ServerDataPayloadV1::InsightOffer(InsightOfferV1 {
            offer_id: "ins:1".to_string(),
            trigger_id: "ins:1".to_string(),
            character_id: "offline:Kiz".to_string(),
            choices: vec![],
        })),
        // F9 跨层修复：出生引导棺权威坐标广播。
        fix!(ServerDataPayloadV1::TutorialCoffinPos {
            position: [0, 69, 0],
        }),
        // plan-inventory-hint-panel-v1 P0：库存操作拒绝原因结构化 S2C。
        fix!(ServerDataPayloadV1::InventoryMoveRejected(
            InventoryMoveRejectedV1 {
                reason: "worn_cap_full".to_string(),
                required_realm: None,
                slot: Some("chest".to_string()),
                cap: Some(3),
            }
        )),
        // plan-scroll-reading-v1 P0：可阅读残卷阅读屏。
        fix!(ServerDataPayloadV1::ScrollOpen {
            scroll_id: "scroll_meridian_primer".to_string(),
            title: "《经脉浅述·残卷》".to_string(),
            body_pages: vec![
                "第一页".to_string(),
                "第二页".to_string(),
                "第三页".to_string()
            ],
        }),
        // ─── JSON-bypass variants (3) — is_json_bypass() returns true ───────────
        fix!(ServerDataPayloadV1::AgentUiRequest(
            AgentUiRequestPayloadV1 {
                request_id: "req:1".to_string(),
                target_player: "offline:Kiz".to_string(),
                xml: "<owo-ui/>".to_string(),
                timeout_ticks: 600,
            }
        )),
        fix!(ServerDataPayloadV1::AgentUiClose(AgentUiClosePayloadV1 {
            request_id: "req:1".to_string(),
            reason: None,
        })),
        fix!(ServerDataPayloadV1::HalfStepRechallenge(
            HalfStepRechallengeV1 {
                active: false,
                char_id: "offline:Kiz".to_string(),
                rechallenge_window_until: 0,
                at_tick: 0,
            }
        )),
    ]
}

/// Verifies that the fixture list covers every `ServerDataPayloadV1` variant (127 total).
///
/// This cross-checks the fixture count against `ServerDataType` discriminant count derived
/// from `payload_type()`. If a new variant is added and a fixture is not added to
/// `s2c_all_fixtures()`, this test will fail with a clear count mismatch message.
///
/// NOTE: This test uses the `ServerDataType` enum variant count as the ground truth.
/// That count is maintained by `payload_type()` which is also an exhaustive match.
/// plan-race-system-v1 P4 —— delta（`active=false`）易形解除快照的专项 proto 往返，
/// 补 `s2c_all_fixtures` 每变体 1:1 约束下无法放第二条 MorphState fixture 的覆盖。

#[test]
fn morph_state_delta_release_proto_round_trips() {
    use bong_server::schema::server_data::{
        MorphStateEntryV1, MorphStateV1, ServerDataPayloadV1, ServerDataV1,
    };
    use prost::Message;

    let payload = ServerDataV1::new(ServerDataPayloadV1::MorphState(MorphStateV1 {
        mode: "delta".to_string(),
        entries: vec![MorphStateEntryV1 {
            entity_id: 42,
            model_kind: 0,
            form_race_id: String::new(),
            form_body_plan_id: String::new(),
            active: false,
        }],
    }));
    let bytes = payload.to_proto_bytes();
    assert!(!bytes.is_empty());
    let decoded = bong::ServerDataEnvelope::decode(bytes.as_slice())
        .expect("delta morph_state envelope must decode");
    match decoded.payload {
        Some(bong::server_data_envelope::Payload::MorphState(state)) => {
            assert_eq!(state.mode, "delta");
            assert_eq!(state.entries.len(), 1);
            assert_eq!(state.entries[0].entity_id, 42);
            assert!(!state.entries[0].active, "解除态必须 active=false");
        }
        other => panic!("expected MorphState payload, got {other:?}"),
    }
}

#[test]
fn s2c_fixture_count_matches_variant_count() {
    use bong_server::schema::server_data::ServerDataType;
    use std::collections::HashSet;
    let fixtures = s2c_all_fixtures();
    // Collect all discriminants from the fixture list
    let fixture_discriminants: HashSet<ServerDataType> =
        fixtures.iter().map(|(f, _)| f.payload_type()).collect();

    // The authoritative variant count is determined by exhaustively listing all ServerDataType variants.
    // This list must be kept in sync with the ServerDataType enum.
    // If a new variant is added to ServerDataType but not here, this test will red on the
    // discriminant mismatch below — not here. The point of THIS assertion is to catch
    // duplicates (same variant appearing twice in fixtures, reducing coverage).
    let all_types: Vec<ServerDataType> = vec![
        ServerDataType::Welcome,
        ServerDataType::Heartbeat,
        ServerDataType::Narration,
        ServerDataType::ZoneInfo,
        ServerDataType::EventAlert,
        ServerDataType::PlayerState,
        ServerDataType::CoffinState,
        ServerDataType::UiOpen,
        ServerDataType::CultivationDetail,
        ServerDataType::QiColorObserved,
        ServerDataType::InventorySnapshot,
        ServerDataType::InventoryEvent,
        ServerDataType::DroppedLootSync,
        ServerDataType::RemainsSync,
        ServerDataType::BodyPlanLayout,
        ServerDataType::RaceGateMeta,
        ServerDataType::BotanyHarvestProgress,
        ServerDataType::BotanyPlantV2RenderProfiles,
        ServerDataType::MiningProgress,
        ServerDataType::LumberProgress,
        ServerDataType::GatheringSession,
        ServerDataType::BotanySkill,
        ServerDataType::AlchemyFurnace,
        ServerDataType::AlchemySession,
        ServerDataType::AlchemyOutcomeForecast,
        ServerDataType::AlchemyOutcomeResolved,
        ServerDataType::AlchemyRecipeBook,
        ServerDataType::AlchemyContamination,
        ServerDataType::CombatHudState,
        ServerDataType::WoundsSnapshot,
        ServerDataType::DefenseWindow,
        ServerDataType::CastSync,
        ServerDataType::QuickSlotConfig,
        ServerDataType::SkillBarConfig,
        ServerDataType::TechniquesSnapshot,
        ServerDataType::SkillConfigSnapshot,
        ServerDataType::UnlocksSync,
        ServerDataType::DerivedAttrsSync,
        ServerDataType::EventStreamPush,
        ServerDataType::WeaponEquipped,
        ServerDataType::WeaponBroken,
        ServerDataType::ShieldBroken,
        ServerDataType::ShieldBlockHit,
        ServerDataType::TreasureEquipped,
        ServerDataType::VortexState,
        ServerDataType::DuguPoisonState,
        ServerDataType::PoisonDoseEvent,
        ServerDataType::PoisonOverdoseEvent,
        ServerDataType::PoisonTraitState,
        ServerDataType::CarrierState,
        ServerDataType::FalseSkinState,
        ServerDataType::LingtianSession,
        ServerDataType::DeathScreen,
        ServerDataType::TerminateScreen,
        ServerDataType::RiftPortalState,
        ServerDataType::RiftPortalRemoved,
        ServerDataType::ExtractStarted,
        ServerDataType::ExtractProgress,
        ServerDataType::ExtractCompleted,
        ServerDataType::ExtractAborted,
        ServerDataType::ExtractFailed,
        ServerDataType::TsyCollapseStartedIpc,
        ServerDataType::ContainerState,
        ServerDataType::SearchStarted,
        ServerDataType::SearchProgress,
        ServerDataType::SearchCompleted,
        ServerDataType::SearchAborted,
        ServerDataType::SkillXpGain,
        ServerDataType::SkillLvUp,
        ServerDataType::SkillCapChanged,
        ServerDataType::SkillScrollUsed,
        ServerDataType::SkillSnapshot,
        ServerDataType::ForgeStation,
        ServerDataType::ForgeSession,
        ServerDataType::ForgeOutcome,
        ServerDataType::ForgeBlueprintBook,
        ServerDataType::TribulationState,
        ServerDataType::TribulationBroadcast,
        ServerDataType::AscensionQuota,
        ServerDataType::HeartDemonOffer,
        ServerDataType::BurstMeridianEvent,
        ServerDataType::BreakthroughCinematic,
        ServerDataType::FullPowerChargingState,
        ServerDataType::FullPowerRelease,
        ServerDataType::FullPowerExhaustedState,
        ServerDataType::SocialAnonymity,
        ServerDataType::SocialExposure,
        ServerDataType::SocialPact,
        ServerDataType::SocialFeud,
        ServerDataType::SocialRenownDelta,
        ServerDataType::IdentityPanelState,
        ServerDataType::NicheIntrusion,
        ServerDataType::NicheGuardianFatigue,
        ServerDataType::NicheGuardianBroken,
        ServerDataType::SparringInvite,
        ServerDataType::TradeOffer,
        ServerDataType::RealmVisionParams,
        ServerDataType::SpiritualSenseTargets,
        ServerDataType::HealerNpcAiState,
        ServerDataType::YidaoHudState,
        ServerDataType::MovementState,
        ServerDataType::SpiritTreasureState,
        ServerDataType::SpiritTreasureDialogue,
        ServerDataType::CraftRecipeList,
        ServerDataType::CraftSessionState,
        ServerDataType::CraftOutcome,
        ServerDataType::RecipeUnlocked,
        ServerDataType::WorkbenchOpen,
        ServerDataType::CombatEventFloater,
        ServerDataType::KnockbackSync,
        ServerDataType::TechniqueProficiencyUpdate,
        ServerDataType::PillBuffStatus,
        ServerDataType::LootContainerOpen,
        ServerDataType::LootContainerUpdate,
        ServerDataType::LootContainerClose,
        ServerDataType::FactionWarState,
        ServerDataType::AnqiHud,
        ServerDataType::DuguV2SkillCast,
        ServerDataType::DuguV2SelfCure,
        ServerDataType::DuguV2ShroudActive,
        ServerDataType::PermanentQiMaxDecayApplied,
        ServerDataType::SwordBondHudState,
        ServerDataType::ZhenmaiHud,
        ServerDataType::MineralProbeResult,
        ServerDataType::FreshnessUpdate,
        ServerDataType::InsightOffer,
        ServerDataType::AgentUiRequest,
        ServerDataType::AgentUiClose,
        ServerDataType::HalfStepRechallenge,
        ServerDataType::TutorialCoffinPos,
        ServerDataType::InventoryMoveRejected,
        ServerDataType::ScrollOpen,
        ServerDataType::MorphState,
    ];

    let total_variants = all_types.len();
    assert_eq!(
        fixtures.len(),
        total_variants,
        "Fixture list has {} entries but ServerDataType has {} variants. \
             Add a fixture for every new variant in s2c_all_fixtures(). \
             Current fixture discriminants: {:?}",
        fixtures.len(),
        total_variants,
        fixture_discriminants.iter().collect::<Vec<_>>()
    );

    let all_types_set: HashSet<ServerDataType> = all_types.into_iter().collect();
    let missing: Vec<&ServerDataType> = all_types_set
        .iter()
        .filter(|t| !fixture_discriminants.contains(t))
        .collect();
    assert!(
        missing.is_empty(),
        "These ServerDataType variants are missing from the fixture list: {:?}. \
             Add a fixture for each in s2c_all_fixtures().",
        missing
    );

    let extra: Vec<ServerDataType> = fixture_discriminants
        .iter()
        .filter(|t| !all_types_set.contains(t))
        .cloned()
        .collect();
    assert!(
            extra.is_empty(),
            "These discriminants appear in fixtures but not in the authoritative all_types list: {:?}. \
             Update the all_types list in this test.",
            extra
        );
}

/// Exhaustive proto encoding guard for all 128 `ServerDataPayloadV1` variants.
///
/// For each of the 125 proto-encodable variants (is_json_bypass=false):
///   - Calls `ServerDataV1::new(variant).to_proto_bytes()`.
///   - Asserts the bytes are non-empty (proto envelope was built).
///   - Decodes and asserts the envelope contains a payload (proto arm exists in From impl).
///
/// For each of the 3 JSON-bypass variants (is_json_bypass=true):
///   - Asserts that `to_proto_bytes()` panics with the expected `unreachable!()` message.
///   - This is done via `std::panic::catch_unwind` to avoid aborting the test process.
///
/// MUTATION GUARDS:
///   - Delete any proto arm in `From<&ServerDataPayloadV1>` → `to_proto_bytes()` calls
///     `unreachable!()` → this test panics with a clear variant name → RED.
///   - Flip a bypass variant in `is_json_bypass` from `true` to `false` → this test will
///     try to call `to_proto_bytes()` and hit `unreachable!()` → RED.
///   - Correctly labeled new bypass variant (true) → `catch_unwind` catches the panic → GREEN.

#[test]
fn s2c_all_proto_variants_encode_without_panic() {
    use bong_server::schema::server_data::ServerDataV1;
    use prost::Message;

    let fixtures = s2c_all_fixtures();
    let mut proto_count = 0usize;
    let mut bypass_count = 0usize;

    for (variant, is_bypass) in fixtures {
        let type_name = format!("{:?}", variant.payload_type());

        if is_bypass {
            // Bypass variants must panic on the proto path — verify with catch_unwind.
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let envelope = ServerDataV1::new(variant);
                let _ = envelope.to_proto_bytes();
            }));
            assert!(
                result.is_err(),
                "Bypass variant {type_name} should panic on to_proto_bytes() because it is \
                     a JSON-bypass variant with no proto definition. If this assertion fails, \
                     either is_json_bypass() is misclassified (should be true) or a proto arm \
                     was accidentally added."
            );
            bypass_count += 1;
        } else {
            // Proto-encodable variant: must encode to non-empty bytes and decode successfully.
            let envelope = ServerDataV1::new(variant);
            let bytes = envelope.to_proto_bytes();
            assert!(
                !bytes.is_empty(),
                "Proto-encodable variant {type_name} produced empty bytes from to_proto_bytes(). \
                     This indicates the proto arm in From<&ServerDataPayloadV1> is returning an \
                     empty payload or the encode step is broken."
            );
            let decoded = bong::ServerDataEnvelope::decode(bytes.as_slice())
                    .unwrap_or_else(|e| panic!(
                        "Proto-encodable variant {type_name} produced bytes that failed to decode: {e}. \
                         The proto arm may be constructing a malformed proto message."
                    ));
            assert!(
                decoded.payload.is_some(),
                "Proto-encodable variant {type_name} decoded to an envelope with no payload. \
                     The proto arm in From<&ServerDataPayloadV1> may be returning an empty oneof."
            );
            proto_count += 1;
        }
    }

    assert_eq!(
        proto_count, 130,
        "Expected 130 proto-encodable S2C variants, got {proto_count}. \
             The fixture list or is_json_bypass classification may have changed."
    );
    assert_eq!(
        bypass_count, 3,
        "Expected 3 JSON-bypass S2C variants, got {bypass_count}. \
             Check is_json_bypass() and s2c_all_fixtures()."
    );
}

// ─── plan-test-coverage-guards-v1 P0：C2S exhaustive proto encoding guard ──

/// Returns a minimum-viable fixture for every `ClientRequestV1` variant (102 total).
///
/// The same exhaustiveness strategy as `s2c_all_fixtures()` applies:
///   - No compile-time list exhaustiveness, but `c2s_fixture_count_matches_variant_count()`
///     cross-checks list length against a manually maintained variant set.
///   - `AgentUiResponse` is the only C2S JSON-bypass variant (is_c2s_json_bypass=true).
///
/// MUTATION GUARDS (see `c2s_all_proto_variants_encode_without_panic`):
///   - Delete a proto arm → fixture hits `unreachable!()` → test panics → RED.
///   - Add a new C2S variant without fixture → count assertion → RED.
fn c2s_all_fixtures() -> Vec<(bong_server::schema::client_request::ClientRequestV1, bool)> {
    use bong_server::cultivation::components::MeridianId;
    use bong_server::cultivation::forging::ForgeAxis;
    use bong_server::cultivation::void::components::BarrierGeometry;
    use bong_server::schema::agent_ui::AgentUiActionType;
    use bong_server::schema::client_request::{
        AnqiCarrierSlotV1, ApplyPillTargetV1, ClientRequestV1, SkillBarBindingV1,
    };
    use bong_server::schema::inventory::EquipSlotV1;
    use bong_server::schema::inventory::InventoryLocationV1;
    use bong_server::schema::movement::MovementActionRequestV1;
    use bong_server::schema::void_actions::VoidActionRequestV1;
    use bong_server::zhenfa::trap_content::TrapTargetFace;
    use bong_server::zhenfa::{ZhenfaCarrierKind, ZhenfaDisarmMode, ZhenfaKind};

    // `is_c2s_bypass(req)` — returns true only for AgentUiResponse (the sole C2S bypass).
    // There is no `const fn is_c2s_json_bypass` on ClientRequestV1 because there is only
    // 1 bypass variant, and a separate exhaustive fn would duplicate maintenance burden.
    // Instead, we track it via a closure here.
    let is_bypass = |req: &ClientRequestV1| matches!(req, ClientRequestV1::AgentUiResponse { .. });

    let build = |req: ClientRequestV1| -> (ClientRequestV1, bool) {
        let b = is_bypass(&req);
        (req, b)
    };

    vec![
        build(ClientRequestV1::SetMeridianTarget {
            v: 1,
            meridian: MeridianId::Lung.channel_id(),
        }),
        build(ClientRequestV1::BreakthroughRequest { v: 1 }),
        build(ClientRequestV1::StartDuXu { v: 1 }),
        build(ClientRequestV1::VoidAction {
            v: 1,
            request: VoidActionRequestV1::Barrier {
                zone_id: "spawn".to_string(),
                geometry: BarrierGeometry::Circle {
                    center: [0.0, 64.0, 0.0],
                    radius: 5.0,
                },
            },
        }),
        build(ClientRequestV1::MovementAction {
            v: 1,
            action: MovementActionRequestV1::Dash,
            yaw_degrees: None,
        }),
        build(ClientRequestV1::AbortTribulation { v: 1 }),
        build(ClientRequestV1::HeartDemonDecision {
            v: 1,
            choice_idx: None,
        }),
        build(ClientRequestV1::ForgeRequest {
            v: 1,
            meridian: MeridianId::Lung.channel_id(),
            axis: ForgeAxis::Rate,
        }),
        build(ClientRequestV1::InsightDecision {
            v: 1,
            trigger_id: "ins:1".to_string(),
            choice_idx: None,
        }),
        build(ClientRequestV1::BotanyHarvestRequest {
            v: 1,
            session_id: "ses:1".to_string(),
            mode: bong_server::schema::botany::BotanyHarvestModeV1::Manual,
        }),
        build(ClientRequestV1::AlchemyOpenFurnace {
            v: 1,
            furnace_pos: (0, 64, 0),
        }),
        build(ClientRequestV1::AlchemyFeedSlot {
            v: 1,
            furnace_pos: (0, 64, 0),
            slot_idx: 0,
            material: "herb_a".to_string(),
            count: 1,
        }),
        build(ClientRequestV1::AlchemyTakeBack {
            v: 1,
            furnace_pos: (0, 64, 0),
            slot_idx: 0,
        }),
        build(ClientRequestV1::AlchemyIgnite {
            v: 1,
            furnace_pos: (0, 64, 0),
            recipe_id: "recipe:1".to_string(),
        }),
        build(ClientRequestV1::AlchemyIntervention {
            v: 1,
            furnace_pos: (0, 64, 0),
            intervention: bong_server::schema::alchemy::AlchemyInterventionV1::AdjustTemp {
                temp: 100.0,
            },
        }),
        build(ClientRequestV1::AlchemyTurnPage { v: 1, delta: 1 }),
        build(ClientRequestV1::AlchemyLearnRecipe {
            v: 1,
            recipe_id: "recipe:1".to_string(),
        }),
        build(ClientRequestV1::AlchemyLearnRecipeFragment {
            v: 1,
            item_instance_id: 1,
        }),
        build(ClientRequestV1::AlchemyTakePill {
            v: 1,
            pill_item_id: "pill:1".to_string(),
        }),
        build(ClientRequestV1::AlchemyFurnacePlace {
            v: 1,
            x: 0,
            y: 64,
            z: 0,
            item_instance_id: 1,
        }),
        build(ClientRequestV1::CoffinOpen {
            v: 1,
            x: 0,
            y: 64,
            z: 0,
        }),
        build(ClientRequestV1::CoffinPlace {
            v: 1,
            x: 0,
            y: 64,
            z: 0,
            item_instance_id: 1,
        }),
        build(ClientRequestV1::BlockPlace {
            v: 1,
            x: 0,
            y: 64,
            z: 0,
            item_instance_id: 1,
            target_face: TrapTargetFace::Top,
        }),
        build(ClientRequestV1::CoffinEnter {
            v: 1,
            x: 0,
            y: 64,
            z: 0,
        }),
        build(ClientRequestV1::CoffinLeave { v: 1 }),
        build(ClientRequestV1::CoffinBreak {
            v: 1,
            x: 0,
            y: 64,
            z: 0,
        }),
        build(ClientRequestV1::CoffinMenuReclaim {
            v: 1,
            x: 0,
            y: 64,
            z: 0,
        }),
        build(ClientRequestV1::SpiritNichePlace {
            v: 1,
            x: 0,
            y: 64,
            z: 0,
            item_instance_id: 1,
        }),
        build(ClientRequestV1::SpiritNicheRepair {
            v: 1,
            x: 0,
            y: 64,
            z: 0,
            item_instance_id: 1,
        }),
        build(ClientRequestV1::SpiritNicheGaze {
            v: 1,
            x: 0,
            y: 64,
            z: 0,
        }),
        build(ClientRequestV1::SpiritNicheMarkCoordinate {
            v: 1,
            x: 0,
            y: 64,
            z: 0,
        }),
        build(ClientRequestV1::SpiritNicheActivateGuardian {
            v: 1,
            niche_pos: [0, 64, 0],
            guardian_kind: bong_server::schema::social::GuardianKindV1::Puppet,
            materials: vec![],
        }),
        build(ClientRequestV1::SparringInviteResponse {
            v: 1,
            invite_id: "inv:1".to_string(),
            accepted: true,
            timed_out: false,
        }),
        build(ClientRequestV1::TradeOfferRequest {
            v: 1,
            target: "char:b".to_string(),
            offered_instance_id: 1,
        }),
        build(ClientRequestV1::TradeOfferResponse {
            v: 1,
            offer_id: "trade:1".to_string(),
            accepted: true,
            requested_instance_id: None,
        }),
        build(ClientRequestV1::NpcInspectRequest {
            v: 1,
            npc_entity_id: 1,
        }),
        build(ClientRequestV1::NpcDialogueChoice {
            v: 1,
            npc_entity_id: 1,
            option_id: "opt_a".to_string(),
        }),
        build(ClientRequestV1::NpcTradeRequest {
            v: 1,
            npc_entity_id: 1,
            offered_items: vec![],
            requested_item_id: "item:1".to_string(),
        }),
        build(ClientRequestV1::ZhenfaPlace {
            v: 1,
            x: 0,
            y: 64,
            z: 0,
            kind: ZhenfaKind::Trap,
            carrier: Some(ZhenfaCarrierKind::CommonStone),
            qi_invest_ratio: 0.5,
            trigger: None,
            item_instance_id: None,
            target_face: None,
        }),
        build(ClientRequestV1::ZhenfaTrigger {
            v: 1,
            instance_id: None,
        }),
        build(ClientRequestV1::ZhenfaDisarm {
            v: 1,
            x: 0,
            y: 64,
            z: 0,
            mode: ZhenfaDisarmMode::Disarm,
        }),
        build(ClientRequestV1::QiScatterBeadUse {
            v: 1,
            item_instance_id: 1,
            x: Some(0),
            y: Some(64),
            z: Some(0),
        }),
        build(ClientRequestV1::LearnSkillScroll {
            v: 1,
            instance_id: 1,
        }),
        build(ClientRequestV1::TechniqueScrollUse {
            v: 1,
            instance_id: 1,
        }),
        build(ClientRequestV1::InventoryMoveIntent {
            v: 1,
            instance_id: 1,
            from: InventoryLocationV1::Container {
                container_id: "pack".to_string(),
                row: 0,
                col: 0,
            },
            to: InventoryLocationV1::Container {
                container_id: "pack".to_string(),
                row: 1,
                col: 0,
            },
            rotated: true,
        }),
        build(ClientRequestV1::EquipFalseSkin {
            v: 1,
            slot: EquipSlotV1::MainHand,
            item_instance_id: 1,
        }),
        build(ClientRequestV1::ForgeFalseSkin {
            v: 1,
            kind: bong_server::schema::tuike::FalseSkinKindV1::SpiderSilk,
        }),
        build(ClientRequestV1::InventoryDiscardItem {
            v: 1,
            instance_id: 1,
            from: InventoryLocationV1::Container {
                container_id: "pack".to_string(),
                row: 0,
                col: 0,
            },
        }),
        build(ClientRequestV1::TreasureActivate {
            v: 1,
            instance_id: 1,
            activate: true,
        }),
        build(ClientRequestV1::DropWeaponIntent {
            v: 1,
            instance_id: 1,
            from: InventoryLocationV1::Container {
                container_id: "pack".to_string(),
                row: 0,
                col: 0,
            },
        }),
        build(ClientRequestV1::RepairWeaponIntent {
            v: 1,
            instance_id: 1,
            station_pos: [0, 64, 0],
        }),
        build(ClientRequestV1::PickupDroppedItem {
            v: 1,
            instance_id: 1,
        }),
        build(ClientRequestV1::RemainsLoot {
            v: 1,
            remains_id: "3fa85f64-5717-4562-b3fc-2c963f66afa6".to_string(),
        }),
        build(ClientRequestV1::MineralProbe {
            v: 1,
            x: 0,
            y: 64,
            z: 0,
        }),
        build(ClientRequestV1::FreshnessProbe {
            v: 1,
            instance_id: 1,
        }),
        build(ClientRequestV1::ApplyPill {
            v: 1,
            instance_id: 1,
            target: ApplyPillTargetV1::SelfTarget,
        }),
        build(ClientRequestV1::SelfAntidote {
            v: 1,
            instance_id: 1,
        }),
        build(ClientRequestV1::DuoSheRequest {
            v: 1,
            target_id: "char:b".to_string(),
        }),
        build(ClientRequestV1::QiColorInspect {
            v: 1,
            observed: "char:b".to_string(),
        }),
        build(ClientRequestV1::UseLifeCore {
            v: 1,
            instance_id: 1,
        }),
        build(ClientRequestV1::Jiemai { v: 1 }),
        build(ClientRequestV1::ChargeCarrier {
            v: 1,
            slot: None,
            qi_target: 50.0,
        }),
        build(ClientRequestV1::ThrowCarrier {
            v: 1,
            slot: AnqiCarrierSlotV1::MainHand,
            dir_unit: [1.0, 0.0, 0.0],
            power: 1.0,
        }),
        build(ClientRequestV1::AnqiContainerSwitch { v: 1, to: None }),
        build(ClientRequestV1::UseQuickSlot { v: 1, slot: 0 }),
        build(ClientRequestV1::QuickSlotBind {
            v: 1,
            slot: 0,
            item_id: Some("herb_a".to_string()),
            request_id: "quick-bind-1".to_string(),
        }),
        build(ClientRequestV1::SkillBarCast {
            v: 1,
            slot: 0,
            target: None,
        }),
        build(ClientRequestV1::SkillBarBind {
            v: 1,
            slot: 0,
            binding: Some(SkillBarBindingV1::Item {
                template_id: "herb_a".to_string(),
            }),
        }),
        build(ClientRequestV1::SkillConfigIntent {
            v: 1,
            skill_id: "fireball".to_string(),
            config: Default::default(),
        }),
        build(ClientRequestV1::CombatReincarnate { v: 1 }),
        build(ClientRequestV1::CombatTerminate { v: 1 }),
        build(ClientRequestV1::CombatCreateNewCharacter { v: 1 }),
        build(ClientRequestV1::StartExtractRequest {
            v: 1,
            portal_entity_id: 1,
        }),
        build(ClientRequestV1::CancelExtractRequest { v: 1 }),
        build(ClientRequestV1::StartSearch {
            v: 1,
            container_entity_id: 1,
        }),
        build(ClientRequestV1::CancelSearch { v: 1 }),
        build(ClientRequestV1::SupplyCoffinOpen { v: 1, entity_id: 1 }),
        build(ClientRequestV1::ContainerOpen { v: 1, entity_id: 1 }),
        build(ClientRequestV1::LingtianStartTill {
            v: 1,
            x: 0,
            y: 64,
            z: 0,
            hoe_instance_id: 1,
            mode: "standard".to_string(),
        }),
        build(ClientRequestV1::LingtianStartRenew {
            v: 1,
            x: 0,
            y: 64,
            z: 0,
            hoe_instance_id: 1,
        }),
        build(ClientRequestV1::LingtianStartPlanting {
            v: 1,
            x: 0,
            y: 64,
            z: 0,
            plant_id: "herb_a".to_string(),
        }),
        build(ClientRequestV1::LingtianStartHarvest {
            v: 1,
            x: 0,
            y: 64,
            z: 0,
            mode: "standard".to_string(),
        }),
        build(ClientRequestV1::LingtianStartReplenish {
            v: 1,
            x: 0,
            y: 64,
            z: 0,
            source: "water_bucket".to_string(),
        }),
        build(ClientRequestV1::LingtianStartDrainQi {
            v: 1,
            x: 0,
            y: 64,
            z: 0,
        }),
        build(ClientRequestV1::ForgeStartSession {
            v: 1,
            station_pos: (0, 64, 0),
            blueprint_id: "bp:sword".to_string(),
            materials: vec![],
        }),
        build(ClientRequestV1::ForgeTemperingHit {
            v: 1,
            session_id: 1,
            beat: "hit".to_string(),
            ticks_remaining: 100,
        }),
        build(ClientRequestV1::ForgeInscriptionScroll {
            v: 1,
            session_id: 1,
            inscription_id: "ins:1".to_string(),
        }),
        build(ClientRequestV1::ForgeConsecrationInject {
            v: 1,
            session_id: 1,
            qi_amount: 10.0,
        }),
        build(ClientRequestV1::ForgeStepAdvance {
            v: 1,
            session_id: 1,
        }),
        build(ClientRequestV1::ForgeBlueprintTurnPage { v: 1, delta: 1 }),
        build(ClientRequestV1::ForgeLearnBlueprint {
            v: 1,
            blueprint_id: "bp:sword".to_string(),
        }),
        build(ClientRequestV1::ForgeStationPlace {
            v: 1,
            x: 0,
            y: 64,
            z: 0,
            item_instance_id: 1,
            station_tier: 1,
        }),
        build(ClientRequestV1::CraftStart {
            v: 1,
            recipe_id: "craft.example".to_string(),
            quantity: 1,
        }),
        build(ClientRequestV1::CraftCancel { v: 1 }),
        build(ClientRequestV1::ExternalContainerMove {
            v: 1,
            session_id: 1,
            instance_id: 1,
            from: InventoryLocationV1::Container {
                container_id: "pack".to_string(),
                row: 0,
                col: 0,
            },
            to: InventoryLocationV1::Container {
                container_id: "pack".to_string(),
                row: 1,
                col: 0,
            },
        }),
        build(ClientRequestV1::ExternalContainerClose {
            v: 1,
            session_id: 1,
        }),
        build(ClientRequestV1::GiveDanToElder {
            v: 1,
            pill_instance_id: 1,
            elder_entity_id: 2,
        }),
        build(ClientRequestV1::WorkbenchOpen { v: 1, entity_id: 1 }),
        build(ClientRequestV1::RaiseShield { v: 1 }),
        build(ClientRequestV1::LowerShield { v: 1 }),
        build(ClientRequestV1::ScrollReadRequest {
            v: 1,
            instance_id: 1,
        }),
        build(ClientRequestV1::ScrollReadClosed { v: 1 }),
        // ─── JSON-bypass variant (1) ────────────────────────────────────────
        build(ClientRequestV1::AgentUiResponse {
            v: 1,
            request_id: "req:1".to_string(),
            action: AgentUiActionType::ButtonClick,
            params: Default::default(),
        }),
    ]
}

/// Verifies that the C2S fixture list covers every `ClientRequestV1` variant (101 total).

#[test]
fn c2s_fixture_count_matches_variant_count() {
    use bong_server::schema::client_request::ClientRequestV1;
    use std::collections::HashSet;
    use std::mem::{discriminant, Discriminant};
    let fixtures = c2s_all_fixtures();
    // The authoritative count is 102 (101 proto + 1 AgentUiResponse bypass).
    let bypass_count = fixtures.iter().filter(|(_, b)| *b).count();
    let proto_count = fixtures.iter().filter(|(_, b)| !*b).count();
    assert_eq!(
        fixtures.len(),
        103,
        "C2S fixture list has {} entries but ClientRequestV1 has 103 variants. \
             Add a fixture for every new variant in c2s_all_fixtures().",
        fixtures.len()
    );
    assert_eq!(
        bypass_count, 1,
        "Expected exactly 1 C2S JSON-bypass variant (AgentUiResponse), got {bypass_count}. \
             If a new bypass variant is added, update c2s_all_fixtures() and this assertion."
    );
    assert_eq!(
        proto_count, 102,
        "Expected 102 proto-encodable C2S variants, got {proto_count}."
    );

    // Set-intersection coverage (mirrors the S2C `payload_type()` HashSet check, but keyed
    // on `mem::discriminant` since ClientRequestV1 has no payload_type() discriminant enum).
    // `discriminant` yields one key per enum variant regardless of field values, so duplicate
    // variants collapse. This closes the "swap escape" the length+count checks alone miss —
    // e.g. deleting variant A's fixture and duplicating variant B's keeps len==99 but drops
    // the DISTINCT-variant count to 98, reding this assertion. Without it, variant A would
    // silently lose its proto-encode coverage while the counts still look correct.
    let distinct: HashSet<Discriminant<ClientRequestV1>> =
        fixtures.iter().map(|(v, _)| discriminant(v)).collect();
    assert_eq!(
        distinct.len(),
        103,
        "C2S fixtures cover only {} DISTINCT ClientRequestV1 variants but there are 103. \
             A variant's fixture was likely deleted and another duplicated — every variant must \
             have its OWN fixture or the proto guard silently skips it.",
        distinct.len()
    );
}

/// Exhaustive proto encoding guard for all 102 `ClientRequestV1` variants.
///
/// Same strategy as `s2c_all_proto_variants_encode_without_panic`.
/// For the 101 proto-encodable variants: encode → decode → assert payload present.
/// For AgentUiResponse (bypass): assert `to_proto_bytes()` panics.
///
/// MUTATION GUARDS:
///   - Delete any proto arm in `From<&ClientRequestV1>` → the `From` match is exhaustive with
///     NO catch-all, so this is rustc E0004 (compile failure), caught earlier than any test.
///     The `catch_unwind` here is defense-in-depth: if a future refactor adds a
///     `_ => unreachable!()` catch-all, a deleted arm falls through to it at runtime and the
///     guard reds (it expects no panic) instead.
///   - Add a new C2S variant without proto arm → `From` impl is non-exhaustive → compile failure.

#[test]
fn c2s_all_proto_variants_encode_without_panic() {
    use prost::Message;

    let fixtures = c2s_all_fixtures();
    let mut proto_count = 0usize;
    let mut bypass_count = 0usize;

    for (variant, is_bypass) in fixtures {
        let variant_dbg = format!("{variant:?}").chars().take(60).collect::<String>();

        if is_bypass {
            // The only C2S bypass: AgentUiResponse must panic on the proto path.
            let variant_clone = variant.clone();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                let _: bong::client_request_envelope::Payload = (&variant_clone).into();
            }));
            assert!(
                result.is_err(),
                "C2S bypass variant ({variant_dbg}) should panic on the proto path but did not. \
                     Verify that the AgentUiResponse arm in From<&ClientRequestV1> still calls \
                     unreachable!()."
            );
            bypass_count += 1;
        } else {
            // Proto-encodable: encode → decode → assert payload present.
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let proto_payload = bong::client_request_envelope::Payload::from(&variant);
                let envelope = bong::ClientRequestEnvelope {
                    payload: Some(proto_payload),
                };
                envelope.encode_to_vec()
            }));
            let bytes = result.unwrap_or_else(|_| panic!(
                    "Proto-encodable C2S variant ({variant_dbg}) panicked during From conversion or \
                     encode. This means the proto arm in From<&ClientRequestV1> is missing or calls \
                     unreachable!(). Add a proto arm or check is_c2s_bypass classification."
                ));
            assert!(
                !bytes.is_empty(),
                "Proto-encodable C2S variant ({variant_dbg}) produced empty bytes."
            );
            let decoded =
                bong::ClientRequestEnvelope::decode(bytes.as_slice()).unwrap_or_else(|e| {
                    panic!("C2S variant ({variant_dbg}) produced bytes that failed to decode: {e}.")
                });
            assert!(
                decoded.payload.is_some(),
                "C2S variant ({variant_dbg}) decoded to an envelope with no payload."
            );
            proto_count += 1;
        }
    }

    assert_eq!(
        proto_count, 102,
        "Expected 102 proto-encodable C2S variants, got {proto_count}."
    );
    assert_eq!(
        bypass_count, 1,
        "Expected 1 C2S bypass variant, got {bypass_count}."
    );
}
