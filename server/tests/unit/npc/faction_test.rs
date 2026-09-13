use bong_server::npc::faction::*;
use serde_json::json;
use valence::prelude::App;

#[test]
fn default_store_bootstraps_exactly_three_stable_factions() {
    let store = FactionStore::default();

    assert_eq!(store.factions.len(), 3);
    assert_eq!(store.factions[0].id, FactionId::Attack);
    assert_eq!(store.factions[1].id, FactionId::Defend);
    assert_eq!(store.factions[2].id, FactionId::Neutral);
    assert!(store
        .factions
        .iter()
        .all(|state| state.leader_lineage.is_none()));
    assert!(store
        .factions
        .iter()
        .all(|state| (state.loyalty_bias - 0.5).abs() < f64::EPSILON));
    assert!(store
        .factions
        .iter()
        .all(|state| state.mission_queue.pending.is_empty()));
}

#[test]
fn disciple_membership_roundtrips_with_passive_runtime_fields() {
    let membership = FactionMembership {
        faction_id: FactionId::Attack,
        rank: FactionRank::Disciple,
        reputation: Reputation { loyalty: 0.72 },
        lineage: Some(Lineage {
            master_id: Some("npc_master_001".to_string()),
            disciple_ids: vec!["npc_peer_001".to_string(), "npc_peer_002".to_string()],
        }),
        mission_queue: MissionQueue {
            pending: vec![MissionId("mission:sweep_blood_valley".to_string())],
        },
    };

    let value = serde_json::to_value(&membership).expect("membership should serialize");
    assert_eq!(value["faction_id"], json!("attack"));
    assert_eq!(value["rank"], json!("disciple"));
    assert_eq!(value["reputation"]["loyalty"], json!(0.72));
    assert_eq!(value["lineage"]["master_id"], json!("npc_master_001"));
    assert_eq!(
        value["mission_queue"]["pending"][0],
        json!("mission:sweep_blood_valley")
    );

    let roundtrip: FactionMembership =
        serde_json::from_value(value).expect("membership should deserialize");
    assert_eq!(roundtrip, membership);
}

#[test]
fn faction_store_applies_minimal_events_without_external_runtime() {
    let mut store = FactionStore::default();

    let leader = store
        .apply_event(FactionEventCommand {
            faction_id: FactionId::Neutral,
            kind: FactionEventKind::SetLeaderLineage,
            subject_id: Some("npc_master_001".to_string()),
            mission_id: None,
            loyalty_delta: None,
        })
        .expect("set leader lineage should succeed");
    assert_eq!(leader.leader_id.as_deref(), Some("npc_master_001"));

    let enqueue = store
        .apply_event(FactionEventCommand {
            faction_id: FactionId::Neutral,
            kind: FactionEventKind::EnqueueMission,
            subject_id: None,
            mission_id: Some("mission:hold_spawn_gate".to_string()),
            loyalty_delta: None,
        })
        .expect("enqueue mission should succeed");
    assert_eq!(enqueue.mission_queue_size, 1);

    let adjust = store
        .apply_event(FactionEventCommand {
            faction_id: FactionId::Neutral,
            kind: FactionEventKind::AdjustLoyaltyBias,
            subject_id: None,
            mission_id: None,
            loyalty_delta: Some(0.2),
        })
        .expect("adjust loyalty should succeed");
    assert!((adjust.loyalty_bias - 0.7).abs() < 1e-9);
}

#[test]
fn pop_mission_removes_head_not_tail() {
    let mut store = FactionStore::default();

    for mission in ["mission_a", "mission_b"] {
        store
            .apply_event(FactionEventCommand {
                faction_id: FactionId::Neutral,
                kind: FactionEventKind::EnqueueMission,
                subject_id: None,
                mission_id: Some(mission.to_string()),
                loyalty_delta: None,
            })
            .expect("enqueue should succeed");
    }

    let popped = store
        .apply_event(FactionEventCommand {
            faction_id: FactionId::Neutral,
            kind: FactionEventKind::PopMission,
            subject_id: None,
            mission_id: None,
            loyalty_delta: None,
        })
        .expect("pop should succeed");
    assert_eq!(popped.mission_queue_size, 1);

    let queue = &store.faction_mut(FactionId::Neutral).unwrap().mission_queue;
    assert_eq!(queue.top_mission_id(), Some("mission_b"));

    store
        .apply_event(FactionEventCommand {
            faction_id: FactionId::Neutral,
            kind: FactionEventKind::PopMission,
            subject_id: None,
            mission_id: None,
            loyalty_delta: None,
        })
        .expect("pop on single-entry queue should succeed");
    store
        .apply_event(FactionEventCommand {
            faction_id: FactionId::Neutral,
            kind: FactionEventKind::PopMission,
            subject_id: None,
            mission_id: None,
            loyalty_delta: None,
        })
        .expect("pop on empty queue should be a no-op, not a panic");
}

#[test]
fn hostility_matrix_is_only_attack_vs_defend() {
    let store = FactionStore::default();
    assert!(store.is_hostile_pair(FactionId::Attack, FactionId::Defend));
    assert!(store.is_hostile_pair(FactionId::Defend, FactionId::Attack));
    assert!(!store.is_hostile_pair(FactionId::Neutral, FactionId::Attack));
    assert!(!store.is_hostile_pair(FactionId::Neutral, FactionId::Defend));
    assert!(!store.is_hostile_pair(FactionId::Attack, FactionId::Attack));
}

#[test]
fn are_hostile_distinct_groups_are_hostile() {
    // §十灵气零和：不同涌现群体争同 zone 灵气 ⇒ 敌对。
    let store = FactionStore::default();
    assert!(
        store.are_hostile(EmergentGroupId(0), EmergentGroupId(1)),
        "distinct emergent groups must be hostile (different groups compete for the same finite qi); 0 vs 1 returned false"
    );
    // 验证 > 2 群体：2 与 3 也敌对（解锁多群体互殴，不止 Attack↔Defend 两家）。
    assert!(
        store.are_hostile(EmergentGroupId(2), EmergentGroupId(3)),
        "any two distinct group ids must be hostile to support >2-group melee; 2 vs 3 returned false"
    );
}

#[test]
fn are_hostile_same_group_is_not_hostile() {
    // §七散修利己仅指向异群体——同一涌现群体的散修同进退，不内斗。
    let store = FactionStore::default();
    assert!(
        !store.are_hostile(EmergentGroupId(2), EmergentGroupId(2)),
        "same emergent group must NOT be hostile to itself (members of one group do not fight each other); 2 vs 2 returned true"
    );
}

#[test]
fn are_hostile_is_symmetric() {
    // 敌对是对称关系：are_hostile(a,b) == are_hostile(b,a)，不区分先后。
    let store = FactionStore::default();
    let a = EmergentGroupId(0);
    let b = EmergentGroupId(3);
    assert_eq!(
        store.are_hostile(a, b),
        store.are_hostile(b, a),
        "are_hostile must be symmetric: hostility does not depend on argument order"
    );
}

#[test]
fn emergent_group_from_faction_attack_maps_to_group_zero() {
    let store = FactionStore::default();
    assert_eq!(
        store.emergent_group_from_faction(FactionId::Attack),
        Some(EmergentGroupId(0)),
        "non-breaking migration: legacy Attack faction must derive to emergent group 0"
    );
}

#[test]
fn emergent_group_from_faction_defend_maps_to_group_one() {
    let store = FactionStore::default();
    assert_eq!(
        store.emergent_group_from_faction(FactionId::Defend),
        Some(EmergentGroupId(1)),
        "non-breaking migration: legacy Defend faction must derive to emergent group 1"
    );
}

#[test]
fn emergent_group_from_faction_neutral_is_none() {
    // 关键非破坏点：Neutral → None，保中立散修离屏仍非战斗（None 进不了 are_hostile）。
    let store = FactionStore::default();
    assert_eq!(
        store.emergent_group_from_faction(FactionId::Neutral),
        None,
        "Neutral must derive to None (not a group) so neutral rogues stay non-combatant offscreen, as no_hostile_pair_yields_no_combat depends on"
    );
}

#[test]
fn group_status_each_variant_roundtrips_via_serde() {
    // 每个 GroupStatus 变体一条专属 serde 往返 + as_str / from_str_name 双向对拍。
    for (status, name) in [
        (GroupStatus::Rising, "rising"),
        (GroupStatus::Stable, "stable"),
        (GroupStatus::Waning, "waning"),
    ] {
        assert_eq!(
            status.as_str(),
            name,
            "GroupStatus::{status:?}.as_str() must equal {name:?}"
        );
        assert_eq!(
            GroupStatus::from_str_name(name),
            Some(status),
            "GroupStatus::from_str_name({name:?}) must roundtrip back to {status:?}"
        );
        let value = serde_json::to_value(status).expect("GroupStatus should serialize");
        assert_eq!(
            value,
            json!(name),
            "GroupStatus::{status:?} must serialize to snake_case string {name:?}, got {value}"
        );
        let back: GroupStatus =
            serde_json::from_value(value).expect("GroupStatus should deserialize");
        assert_eq!(
            back, status,
            "GroupStatus serde roundtrip must return the same variant {status:?}, got {back:?}"
        );
    }
}

#[test]
fn group_status_from_str_name_rejects_unknown() {
    // 错误分支：非法字符串 → None（不 panic、不静默归到某变体）。
    assert_eq!(
        GroupStatus::from_str_name("ascending"),
        None,
        "an unknown status string must yield None, not silently map to a variant"
    );
    assert_eq!(
        GroupStatus::from_str_name(""),
        None,
        "an empty status string must yield None"
    );
}

#[test]
fn emergent_group_id_serializes_as_transparent_bare_number() {
    // serde(transparent)：序列化成裸数字 `3`（不是 `{"0":3}`），与紧凑 Redis 快照对齐。
    let id = EmergentGroupId(3);
    let value = serde_json::to_value(id).expect("EmergentGroupId should serialize");
    assert_eq!(
        value,
        json!(3),
        "EmergentGroupId must serialize transparently as a bare number `3`, not a wrapper object like {{\"0\":3}}; got {value}"
    );
    let back: EmergentGroupId =
        serde_json::from_value(value).expect("EmergentGroupId should deserialize from bare number");
    assert_eq!(
        back, id,
        "EmergentGroupId must roundtrip through transparent serde back to the same id, got {back:?}"
    );
}

#[test]
fn test_named_faction_id_display_name() {
    // 三变体 display_name 各==正典串。
    // 正典依据：宗门残息.json（青云外门），北荒坍缩渊记.json（北荒漂流者）。
    // 沧渊商会为 plan 创作设定，正典无「沧渊/盐商」直接记载。
    assert_eq!(
        NamedFactionId::QingyunHunters.display_name(),
        "青云猎盟",
        "QingyunHunters display_name 必须=「青云猎盟」(正典：宗门残息.json 外门残脉)"
    );
    assert_eq!(
        NamedFactionId::CangyuanMerchants.display_name(),
        "沧渊商会",
        "CangyuanMerchants display_name 必须=「沧渊商会」(plan 创作设定，非正典直载)"
    );
    assert_eq!(
        NamedFactionId::NorthWasteDrifters.display_name(),
        "北荒漂流者",
        "NorthWasteDrifters display_name 必须=「北荒漂流者」(正典：北荒坍缩渊记.json)"
    );
}

#[test]
fn test_named_faction_id_zone_anchor() {
    // 三变体 zone_anchor 各对应 zone.rs 字符串（防止与 zone 体系漂移）。
    assert_eq!(
        NamedFactionId::QingyunHunters.zone_anchor(),
        "qingyun_peaks",
        "QingyunHunters zone_anchor 必须对齐 zone.rs 「qingyun_peaks」terrain profile"
    );
    assert_eq!(
        NamedFactionId::CangyuanMerchants.zone_anchor(),
        "blood_valley",
        "CangyuanMerchants zone_anchor 必须对齐 zone.rs 「blood_valley」（裂谷·血谷）"
    );
    assert_eq!(
        NamedFactionId::NorthWasteDrifters.zone_anchor(),
        "north_wastes",
        "NorthWasteDrifters zone_anchor 必须对齐 zone.rs 「north_wastes」（北荒主锚）"
    );
}

#[test]
fn test_named_faction_id_lore_tag() {
    // 三变体 lore_tag 非空且含锚区关键词；CangyuanMerchants 注释标注非正典。
    let q = NamedFactionId::QingyunHunters.lore_tag();
    assert!(!q.is_empty(), "QingyunHunters lore_tag 不能空");
    assert!(
        q.contains("青云") || q.contains("外门") || q.contains("猎"),
        "QingyunHunters lore_tag 应含「青云/外门/猎」相关词，实际：{q}"
    );

    let c = NamedFactionId::CangyuanMerchants.lore_tag();
    assert!(!c.is_empty(), "CangyuanMerchants lore_tag 不能空");
    // 沧渊商会 plan 创作设定，验证 lore_tag 含「血谷」（地理锚点）。
    assert!(
        c.contains("血谷") || c.contains("矿"),
        "CangyuanMerchants lore_tag 应含「血谷/矿」（地理依据），实际：{c}"
    );

    let n = NamedFactionId::NorthWasteDrifters.lore_tag();
    assert!(!n.is_empty(), "NorthWasteDrifters lore_tag 不能空");
    assert!(
        n.contains("北荒") || n.contains("坍缩") || n.contains("漂流"),
        "NorthWasteDrifters lore_tag 应含「北荒/坍缩/漂流」，实际：{n}"
    );
}

#[test]
fn test_faction_status_three_variants() {
    // Active/Headless/Decayed 各 as_str↔from_str_name 往返 + 非法串返 None（三 state 专属 case）。
    for (status, wire) in [
        (FactionStatus::Active, "active"),
        (FactionStatus::Headless, "headless"),
        (FactionStatus::Decayed, "decayed"),
    ] {
        assert_eq!(
            status.as_str(),
            wire,
            "FactionStatus::{status:?}.as_str() 必须返回 {wire:?}"
        );
        assert_eq!(
            FactionStatus::from_str_name(wire),
            Some(status),
            "FactionStatus::from_str_name({wire:?}) 必须返回 {status:?}"
        );
        let serialized = serde_json::to_value(status).expect("FactionStatus should serialize");
        assert_eq!(
            serialized,
            json!(wire),
            "FactionStatus::{status:?} 序列化必须是 snake_case {wire:?}，实际 {serialized}"
        );
        let back: FactionStatus =
            serde_json::from_value(serialized).expect("FactionStatus should deserialize");
        assert_eq!(
            back, status,
            "FactionStatus serde roundtrip 必须还原 {status:?}，实际 {back:?}"
        );
    }
    // 非法串返 None，不 panic。
    assert_eq!(
        FactionStatus::from_str_name("alive"),
        None,
        "非法 status 字符串 \"alive\" 必须返回 None"
    );
    assert_eq!(FactionStatus::from_str_name(""), None, "空串必须返回 None");
}

#[test]
fn test_named_faction_registry_registers_three() {
    // startup_default 返回 3 条；北荒漂流者初始 Headless，其余 Active；
    // display_name/zone_anchor 经 from_id 派生正确。
    let registry = NamedFactionRegistry::startup_default();
    assert_eq!(
        registry.iter().count(),
        3,
        "startup_default 必须注册 3 条具名势力，实际 {}",
        registry.iter().count()
    );
    let north = registry
        .get(NamedFactionId::NorthWasteDrifters)
        .expect("NorthWasteDrifters 必须存在于注册表");
    assert_eq!(
        north.status,
        FactionStatus::Headless,
        "NorthWasteDrifters 初始 status 必须是 Headless（正典：坍缩渊记无法组织化），实际 {:?}",
        north.status
    );
    for faction_id in [
        NamedFactionId::QingyunHunters,
        NamedFactionId::CangyuanMerchants,
    ] {
        let f = registry
            .get(faction_id)
            .unwrap_or_else(|| panic!("{faction_id:?} 必须存在"));
        assert_eq!(
            f.status,
            FactionStatus::Active,
            "{faction_id:?} 初始 status 必须是 Active，实际 {:?}",
            f.status
        );
        assert_eq!(
            f.display_name,
            faction_id.display_name(),
            "{faction_id:?} display_name 必须从 id 派生"
        );
        assert_eq!(
            f.zone_anchor,
            faction_id.zone_anchor(),
            "{faction_id:?} zone_anchor 必须从 id 派生"
        );
    }
}

#[test]
fn test_registry_inserted_at_app_boot() {
    // 防孤岛 #1：build 最小 App→register()→断言 world.get_resource::<NamedFactionRegistry>()
    // Some 且 3 条（非仅类型存在，必须可查内容）。
    let mut app = App::new();
    register(&mut app);
    let registry = app
        .world()
        .get_resource::<NamedFactionRegistry>()
        .expect("NamedFactionRegistry 必须在 register() 后存在于 World（防孤岛 #1）");
    assert_eq!(
        registry.iter().count(),
        3,
        "注册表 App boot 后必须有 3 条，实际 {}",
        registry.iter().count()
    );
}

#[test]
fn test_faction_id_for_war_maps_to_hostile() {
    // 防孤岛 #2：faction_id_for_war 返回喂现有 FactionStore::is_hostile_pair，
    // 单测真打通到现有 war 逻辑（assert 理由：兼容层必须打通到 is_hostile_pair 否则孤岛）。
    let store = FactionStore::default();
    let (attack, defend) = FactionStore::faction_id_for_war(
        NamedFactionId::QingyunHunters,
        NamedFactionId::CangyuanMerchants,
    );
    assert!(
        store.is_hostile_pair(attack, defend),
        "faction_id_for_war(Qingyun, Cangyuan) 返回的 ({attack:?},{defend:?}) 必须满足 \
         is_hostile_pair==true（兼容层必须打通到 is_hostile_pair，否则为孤岛桩）"
    );
    // 对称：进攻/防守互换方向也要正确。
    let (attack2, defend2) = FactionStore::faction_id_for_war(
        NamedFactionId::CangyuanMerchants,
        NamedFactionId::NorthWasteDrifters,
    );
    assert!(
        store.is_hostile_pair(attack2, defend2),
        "faction_id_for_war(Cangyuan, North) 也必须打通到 is_hostile_pair"
    );
}

#[test]
fn relation_matrix_startup_default_three_pairs_correct() {
    // v1 初值三对关系：(Qingyun,Cangyuan)=Neutral / (Qingyun,North)=Hostile /
    // (Cangyuan,North)=Neutral。正典依据见 FactionRelationMatrix::startup_default。
    let matrix = FactionRelationMatrix::startup_default();
    assert_eq!(
        matrix.get(
            NamedFactionId::QingyunHunters,
            NamedFactionId::CangyuanMerchants
        ),
        FactionRelation::Neutral,
        "v1 初值：(QingyunHunters, CangyuanMerchants) 必须 = Neutral；猎盟与商会各守一方互不开战"
    );
    assert_eq!(
        matrix.get(
            NamedFactionId::QingyunHunters,
            NamedFactionId::NorthWasteDrifters
        ),
        FactionRelation::Hostile,
        "v1 初值：(QingyunHunters, NorthWasteDrifters) 必须 = Hostile；猎盟排斥闯入的游荡者"
    );
    assert_eq!(
        matrix.get(
            NamedFactionId::CangyuanMerchants,
            NamedFactionId::NorthWasteDrifters
        ),
        FactionRelation::Neutral,
        "v1 初值：(CangyuanMerchants, NorthWasteDrifters) 必须 = Neutral；商会有时雇佣漂流者"
    );
}

#[test]
fn relation_matrix_are_hostile_symmetric() {
    // are_hostile 对称性：(a,b) 与 (b,a) 返回相同结果。
    let matrix = FactionRelationMatrix::startup_default();
    let a = NamedFactionId::QingyunHunters;
    let b = NamedFactionId::NorthWasteDrifters;
    assert_eq!(
        matrix.are_hostile(a, b),
        matrix.are_hostile(b, a),
        "are_hostile 必须对称：(Qingyun, North) 与 (North, Qingyun) 应相等，不区分参数顺序"
    );
    // 中立对组同样对称。
    let c = NamedFactionId::CangyuanMerchants;
    assert_eq!(
        matrix.are_hostile(a, c),
        matrix.are_hostile(c, a),
        "are_hostile 对称性：(Qingyun, Cangyuan) 与 (Cangyuan, Qingyun) 应相等"
    );
}

#[test]
fn relation_matrix_set_updates_relation() {
    // set() 动态更新关系（运行时/测试用）。
    let mut matrix = FactionRelationMatrix::startup_default();
    // 初始 Neutral，改为 Pact。
    matrix.set(
        NamedFactionId::QingyunHunters,
        NamedFactionId::CangyuanMerchants,
        FactionRelation::Pact,
    );
    assert_eq!(
        matrix.get(
            NamedFactionId::QingyunHunters,
            NamedFactionId::CangyuanMerchants
        ),
        FactionRelation::Pact,
        "set() 后关系应变为 Pact，初始 Neutral 被覆盖"
    );
    // 对称性不变。
    assert_eq!(
        matrix.get(
            NamedFactionId::CangyuanMerchants,
            NamedFactionId::QingyunHunters
        ),
        FactionRelation::Pact,
        "set() 写入规范化 key 后 (b,a) 方向也应反映更新（对称写入）"
    );
}

#[test]
fn faction_relation_all_variants_serde_roundtrip() {
    // 每个 FactionRelation 变体 serde 往返 + as_str/from_str_name 对拍。
    for (relation, wire) in [
        (FactionRelation::Hostile, "hostile"),
        (FactionRelation::Neutral, "neutral"),
        (FactionRelation::Pact, "pact"),
    ] {
        assert_eq!(
            relation.as_str(),
            wire,
            "FactionRelation::{relation:?}.as_str() 必须=「{wire}」"
        );
        assert_eq!(
            FactionRelation::from_str_name(wire),
            Some(relation),
            "FactionRelation::from_str_name({wire:?}) 必须=Some({relation:?})"
        );
        let value = serde_json::to_value(relation).expect("FactionRelation should serialize");
        assert_eq!(
            value,
            json!(wire),
            "FactionRelation::{relation:?} 序列化应为 {wire:?}，实际 {value}"
        );
        let back: FactionRelation =
            serde_json::from_value(value).expect("FactionRelation should deserialize");
        assert_eq!(
            back, relation,
            "FactionRelation serde roundtrip 应还原 {relation:?}，实际 {back:?}"
        );
    }
    assert_eq!(
        FactionRelation::from_str_name("unknown"),
        None,
        "非法 relation 字符串必须返回 None"
    );
}

#[test]
fn relation_matrix_inserted_at_app_boot() {
    // register() 后 FactionRelationMatrix 必须存在于 World 且三对初值正确（防孤岛）。
    let mut app = App::new();
    register(&mut app);
    let matrix = app
        .world()
        .get_resource::<FactionRelationMatrix>()
        .expect("FactionRelationMatrix 必须在 register() 后存在于 World（防孤岛）");
    assert!(
        matrix.are_hostile(
            NamedFactionId::QingyunHunters,
            NamedFactionId::NorthWasteDrifters
        ),
        "App boot 后矩阵的 (Qingyun, North) 必须 = Hostile"
    );
    assert!(
        !matrix.are_hostile(
            NamedFactionId::QingyunHunters,
            NamedFactionId::CangyuanMerchants
        ),
        "App boot 后矩阵的 (Qingyun, Cangyuan) 必须 = Neutral（不敌对）"
    );
}

#[test]
fn faction_zone_claims_match_registry_zone_anchors() {
    let registry = NamedFactionRegistry::startup_default();
    let claims = FactionZoneClaims::from_registry(&registry);
    for faction in registry.iter() {
        let claim = claims
            .get(faction.id)
            .unwrap_or_else(|| panic!("{:?} 必须有 FactionZoneClaim", faction.id));
        assert_eq!(
            claim.zone, faction.zone_anchor,
            "{:?} 的 FactionZoneClaim 必须与 NamedFactionRegistry.zone_anchor 一致",
            faction.id
        );
    }
}
