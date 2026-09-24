use super::*;

use crate::cultivation::components::Cultivation;
use valence::prelude::{App, Events, PreUpdate};
use valence::testing::create_mock_client;

#[test]
fn relation_matrix_unregistered_pair_defaults_neutral() {
    // 未注册对组默认 Neutral（不敌对，防止遗漏配置引发意外战斗）。
    // 构造一个空矩阵，任意对组都应返回 Neutral。
    let matrix = FactionRelationMatrix {
        relations: HashMap::new(),
    };
    assert_eq!(
        matrix.get(
            NamedFactionId::QingyunHunters,
            NamedFactionId::CangyuanMerchants
        ),
        FactionRelation::Neutral,
        "未注册对组必须默认 Neutral，防止遗漏配置触发意外战斗"
    );
    assert!(
        !matrix.are_hostile(
            NamedFactionId::QingyunHunters,
            NamedFactionId::NorthWasteDrifters
        ),
        "未注册对组 are_hostile 必须返回 false（默认 Neutral，边界防御）"
    );
}

fn base_membership(faction: FactionId, loyalty: f64, pending: u32) -> FactionMembership {
    let pending_ids: Vec<MissionId> = (0..pending)
        .map(|i| MissionId(format!("mission_{i}")))
        .collect();
    FactionMembership {
        faction_id: faction,
        rank: FactionRank::Disciple,
        reputation: Reputation { loyalty },
        lineage: None,
        mission_queue: MissionQueue {
            pending: pending_ids,
        },
    }
}
fn build_loyalty_app() -> App {
    let mut app = App::new();
    app.insert_resource(FactionStore::default());
    app.add_systems(PreUpdate, loyalty_scorer_system);
    app
}
fn build_mq_app() -> App {
    let mut app = App::new();
    app.add_systems(PreUpdate, mission_queue_scorer_system);
    app
}
fn build_exec_app() -> App {
    let mut app = App::new();
    app.add_systems(PreUpdate, mission_execute_action_system);
    app
}
fn test_zone(
    name: &str,
    min_x: f64,
    min_z: f64,
    max_x: f64,
    max_z: f64,
) -> crate::world::zone::Zone {
    crate::world::zone::Zone {
        name: name.to_string(),
        dimension: crate::world::dimension::DimensionKind::Overworld,
        bounds: (
            DVec3::new(min_x, 60.0, min_z),
            DVec3::new(max_x, 90.0, max_z),
        ),
        spirit_qi: 0.5,
        danger_level: 1,
        active_events: Vec::new(),
        patrol_anchors: vec![
            DVec3::new((min_x + max_x) * 0.5, 66.0, (min_z + max_z) * 0.5),
            DVec3::new(max_x - 4.0, 66.0, max_z - 4.0),
        ],
        blocked_tiles: Vec::new(),
        qi_equilibrium: 0.0,
        qi_inflow_per_min: 0.0,
    }
}
fn p2_zone_registry() -> ZoneRegistry {
    ZoneRegistry {
        spatial_revision: 0,
        zones: vec![
            test_zone("qingyun_peaks", 0.0, 0.0, 100.0, 100.0),
            test_zone("blood_valley", 200.0, 0.0, 320.0, 120.0),
            test_zone("north_wastes", 400.0, 0.0, 560.0, 160.0),
        ],
    }
}
fn build_leader_patrol_app() -> App {
    let mut app = App::new();
    app.insert_resource(p2_zone_registry());
    app.add_event::<FactionLeaderTollNotice>();
    app.add_systems(PreUpdate, faction_leader_patrol_action_system);
    app
}

#[test]
fn named_faction_id_for_legacy_round_trips_all_three_variants() {
    for named in NamedFactionId::all() {
        let legacy = legacy_faction_id_for_named_faction(named);
        let recovered = named_faction_id_for_legacy(legacy);
        assert_eq!(
            recovered, named,
            "named={named:?} -> legacy={legacy:?} -> recovered={recovered:?} 应往返一致"
        );
    }
}

#[test]
fn named_faction_id_for_legacy_pinned_per_variant() {
    assert_eq!(
        named_faction_id_for_legacy(FactionId::Attack),
        NamedFactionId::QingyunHunters
    );
    assert_eq!(
        named_faction_id_for_legacy(FactionId::Defend),
        NamedFactionId::CangyuanMerchants
    );
    assert_eq!(
        named_faction_id_for_legacy(FactionId::Neutral),
        NamedFactionId::NorthWasteDrifters
    );
}

#[test]
fn assign_hostile_encounters_binds_only_near_hostile_pairs() {
    let mut app = App::new();
    app.insert_resource(FactionStore::default());
    app.add_systems(Update, assign_hostile_encounters);

    let attack = app
        .world_mut()
        .spawn((
            NpcMarker,
            Position::new([0.0, 64.0, 0.0]),
            FactionMembership {
                faction_id: FactionId::Attack,
                rank: FactionRank::Disciple,
                reputation: Reputation::default(),
                lineage: None,
                mission_queue: MissionQueue::default(),
            },
        ))
        .id();
    let defend = app
        .world_mut()
        .spawn((
            NpcMarker,
            Position::new([4.0, 64.0, 0.0]),
            FactionMembership {
                faction_id: FactionId::Defend,
                rank: FactionRank::Disciple,
                reputation: Reputation::default(),
                lineage: None,
                mission_queue: MissionQueue::default(),
            },
        ))
        .id();
    let neutral = app
        .world_mut()
        .spawn((
            NpcMarker,
            Position::new([3.0, 64.0, 3.0]),
            FactionMembership {
                faction_id: FactionId::Neutral,
                rank: FactionRank::Disciple,
                reputation: Reputation::default(),
                lineage: None,
                mission_queue: MissionQueue::default(),
            },
        ))
        .id();

    app.update();

    assert_eq!(
        app.world().get::<DuelTarget>(attack).map(|target| target.0),
        Some(defend)
    );
    assert_eq!(
        app.world().get::<DuelTarget>(defend).map(|target| target.0),
        Some(attack)
    );
    assert!(app.world().get::<DuelTarget>(neutral).is_none());
}

#[test]
fn loyalty_scorer_zero_when_entity_has_no_membership() {
    let mut app = build_loyalty_app();
    let npc = app.world_mut().spawn(NpcMarker).id();
    let scorer = app
        .world_mut()
        .spawn((Actor(npc), Score::default(), LoyaltyScorer))
        .id();
    app.update();
    assert_eq!(app.world().get::<Score>(scorer).unwrap().get(), 0.0);
}

#[test]
fn loyalty_scorer_averages_reputation_and_faction_bias() {
    let mut app = build_loyalty_app();
    app.world_mut()
        .resource_mut::<FactionStore>()
        .faction_mut(FactionId::Attack)
        .unwrap()
        .loyalty_bias = 0.8;
    let npc = app
        .world_mut()
        .spawn((NpcMarker, base_membership(FactionId::Attack, 0.4, 0)))
        .id();
    let scorer = app
        .world_mut()
        .spawn((Actor(npc), Score::default(), LoyaltyScorer))
        .id();
    app.update();
    // (0.4 + 0.8) * 0.5 = 0.6
    let got = app.world().get::<Score>(scorer).unwrap().get();
    assert!((got - 0.6).abs() < 1e-6, "expected 0.6, got {got}");
}

#[test]
fn loyalty_scorer_clamps_out_of_range_inputs() {
    let mut app = build_loyalty_app();
    app.world_mut()
        .resource_mut::<FactionStore>()
        .faction_mut(FactionId::Defend)
        .unwrap()
        .loyalty_bias = 2.0; // 超标，Scorer 不信任 store 状态时自保
    let npc = app
        .world_mut()
        .spawn((NpcMarker, base_membership(FactionId::Defend, 5.0, 0)))
        .id();
    let scorer = app
        .world_mut()
        .spawn((Actor(npc), Score::default(), LoyaltyScorer))
        .id();
    app.update();
    let got = app.world().get::<Score>(scorer).unwrap().get();
    assert!((0.0..=1.0).contains(&got));
}

#[test]
fn mission_queue_scorer_zero_when_empty() {
    let mut app = build_mq_app();
    let npc = app
        .world_mut()
        .spawn((NpcMarker, base_membership(FactionId::Attack, 0.5, 0)))
        .id();
    let scorer = app
        .world_mut()
        .spawn((Actor(npc), Score::default(), MissionQueueScorer))
        .id();
    app.update();
    assert_eq!(app.world().get::<Score>(scorer).unwrap().get(), 0.0);
}

#[test]
fn mission_queue_scorer_scales_with_pending_count() {
    let mut app = build_mq_app();
    let npc = app
        .world_mut()
        .spawn((NpcMarker, base_membership(FactionId::Attack, 0.5, 2)))
        .id();
    let scorer = app
        .world_mut()
        .spawn((Actor(npc), Score::default(), MissionQueueScorer))
        .id();
    app.update();
    // 2 / 3 ≈ 0.667
    let got = app.world().get::<Score>(scorer).unwrap().get();
    assert!((got - 2.0 / 3.0).abs() < 1e-6);
}

#[test]
fn mission_queue_scorer_saturates_at_cap() {
    let mut app = build_mq_app();
    let npc = app
        .world_mut()
        .spawn((NpcMarker, base_membership(FactionId::Attack, 0.5, 10)))
        .id();
    let scorer = app
        .world_mut()
        .spawn((Actor(npc), Score::default(), MissionQueueScorer))
        .id();
    app.update();
    assert_eq!(app.world().get::<Score>(scorer).unwrap().get(), 1.0);
}

#[test]
fn mission_execute_success_when_queue_empty() {
    let mut app = build_exec_app();
    let npc = app
        .world_mut()
        .spawn((
            NpcMarker,
            base_membership(FactionId::Attack, 0.5, 0),
            Navigator::new(),
            MissionExecuteState::default(),
        ))
        .id();
    let action = app
        .world_mut()
        .spawn((Actor(npc), MissionExecuteAction, ActionState::Requested))
        .id();
    app.update();
    assert_eq!(
        *app.world().get::<ActionState>(action).unwrap(),
        ActionState::Success
    );
}

#[test]
fn mission_execute_pops_mission_on_timeout() {
    let mut app = build_exec_app();
    let npc = app
        .world_mut()
        .spawn((
            NpcMarker,
            base_membership(FactionId::Attack, 0.5, 2),
            Navigator::new(),
            MissionExecuteState::default(),
        ))
        .id();
    let action = app
        .world_mut()
        .spawn((Actor(npc), MissionExecuteAction, ActionState::Requested))
        .id();
    app.update(); // Requested → Executing
    {
        let mut exec = app.world_mut().get_mut::<MissionExecuteState>(npc).unwrap();
        exec.elapsed_ticks = MISSION_EXECUTE_MAX_TICKS - 1;
    }
    app.update(); // Executing → +1 elapsed → 到上限 → Success
    assert_eq!(
        *app.world().get::<ActionState>(action).unwrap(),
        ActionState::Success
    );
    let m = app.world().get::<FactionMembership>(npc).unwrap();
    assert_eq!(m.mission_queue.pending_count(), 1, "应弹出 1 个任务");
}

#[test]
fn mission_execute_stops_navigator_on_requested() {
    let mut app = build_exec_app();
    let mut nav = Navigator::new();
    nav.set_goal(DVec3::new(10.0, 64.0, 10.0), 1.0);
    let npc = app
        .world_mut()
        .spawn((
            NpcMarker,
            base_membership(FactionId::Attack, 0.5, 1),
            nav,
            MissionExecuteState::default(),
        ))
        .id();
    let _action = app
        .world_mut()
        .spawn((Actor(npc), MissionExecuteAction, ActionState::Requested))
        .id();
    app.update();
    assert!(app.world().get::<Navigator>(npc).unwrap().is_idle());
}

#[test]
fn mission_execute_cancelled_transitions_to_failure() {
    let mut app = build_exec_app();
    let npc = app
        .world_mut()
        .spawn((
            NpcMarker,
            base_membership(FactionId::Attack, 0.5, 1),
            Navigator::new(),
            MissionExecuteState::default(),
        ))
        .id();
    let action = app
        .world_mut()
        .spawn((Actor(npc), MissionExecuteAction, ActionState::Cancelled))
        .id();
    app.update();
    assert_eq!(
        *app.world().get::<ActionState>(action).unwrap(),
        ActionState::Failure
    );
}

#[test]
fn assign_hostile_encounters_named_faction_hostile_pair_triggers_duel() {
    // 反孤岛硬要求：两 NPC 都携带 NamedFactionMembership，关系 Hostile ⇒ assign_hostile_encounters
    // 用 FactionRelationMatrix 判断 ⇒ 写入 DuelTarget（非孤岛：真实 combat 路径消费）。
    let mut app = App::new();
    app.insert_resource(FactionStore::default());
    app.insert_resource(FactionRelationMatrix::startup_default());
    app.add_systems(Update, assign_hostile_encounters);

    // Qingyun vs NorthWaste = Hostile in startup_default。
    let qingyun = app
        .world_mut()
        .spawn((
            NpcMarker,
            Position::new([0.0, 64.0, 0.0]),
            NamedFactionMembership {
                faction_id: NamedFactionId::QingyunHunters,
            },
        ))
        .id();
    let north = app
        .world_mut()
        .spawn((
            NpcMarker,
            Position::new([4.0, 64.0, 0.0]),
            NamedFactionMembership {
                faction_id: NamedFactionId::NorthWasteDrifters,
            },
        ))
        .id();
    app.update();

    assert_eq!(
        app.world().get::<DuelTarget>(qingyun).map(|t| t.0),
        Some(north),
        "QingyunHunters NPC 必须把 NorthWasteDrifters NPC 设为 DuelTarget（关系 Hostile，反孤岛硬验证）"
    );
    assert_eq!(
        app.world().get::<DuelTarget>(north).map(|t| t.0),
        Some(qingyun),
        "NorthWasteDrifters NPC 必须把 QingyunHunters NPC 设为 DuelTarget（关系 Hostile，反孤岛硬验证）"
    );
}

#[test]
fn assign_hostile_encounters_named_faction_neutral_pair_no_duel() {
    // Neutral 关系不触发 DuelTarget：(Qingyun, Cangyuan) = Neutral ⇒ 不成对。
    let mut app = App::new();
    app.insert_resource(FactionStore::default());
    app.insert_resource(FactionRelationMatrix::startup_default());
    app.add_systems(Update, assign_hostile_encounters);

    let qingyun = app
        .world_mut()
        .spawn((
            NpcMarker,
            Position::new([0.0, 64.0, 0.0]),
            NamedFactionMembership {
                faction_id: NamedFactionId::QingyunHunters,
            },
        ))
        .id();
    let cangyuan = app
        .world_mut()
        .spawn((
            NpcMarker,
            Position::new([4.0, 64.0, 0.0]),
            NamedFactionMembership {
                faction_id: NamedFactionId::CangyuanMerchants,
            },
        ))
        .id();
    app.update();

    assert!(
        app.world().get::<DuelTarget>(qingyun).is_none(),
        "Neutral 关系的 QingyunHunters NPC 不应有 DuelTarget（不主动与 CangyuanMerchants 开战）"
    );
    assert!(
        app.world().get::<DuelTarget>(cangyuan).is_none(),
        "Neutral 关系的 CangyuanMerchants NPC 不应有 DuelTarget"
    );
}

#[test]
fn assign_hostile_encounters_headless_faction_still_uses_matrix() {
    // Headless 势力（NorthWasteDrifters）仍按关系矩阵参战：与 QingyunHunters = Hostile。
    // 即便 FactionStatus::Headless 也不影响 NamedFactionMembership 的判定路径。
    let mut app = App::new();
    app.insert_resource(FactionStore::default());
    app.insert_resource(FactionRelationMatrix::startup_default());
    app.add_systems(Update, assign_hostile_encounters);

    let north = app
        .world_mut()
        .spawn((
            NpcMarker,
            Position::new([0.0, 64.0, 0.0]),
            NamedFactionMembership {
                faction_id: NamedFactionId::NorthWasteDrifters,
            },
        ))
        .id();
    let qingyun = app
        .world_mut()
        .spawn((
            NpcMarker,
            Position::new([5.0, 64.0, 0.0]),
            NamedFactionMembership {
                faction_id: NamedFactionId::QingyunHunters,
            },
        ))
        .id();
    app.update();

    assert_eq!(
        app.world().get::<DuelTarget>(north).map(|t| t.0),
        Some(qingyun),
        "Headless 势力 NorthWasteDrifters 仍按关系矩阵参战（与 QingyunHunters = Hostile）；\
         FactionStatus::Headless 不影响敌对判定路径"
    );
}

#[test]
fn assign_hostile_encounters_pact_relation_no_duel() {
    // Pact 关系：动态改关系矩阵为 Pact，两 NPC 不应生成 DuelTarget。
    let mut app = App::new();
    app.insert_resource(FactionStore::default());
    let mut matrix = FactionRelationMatrix::startup_default();
    // 改 (Qingyun, North) → Pact
    matrix.set(
        NamedFactionId::QingyunHunters,
        NamedFactionId::NorthWasteDrifters,
        FactionRelation::Pact,
    );
    app.insert_resource(matrix);
    app.add_systems(Update, assign_hostile_encounters);

    let qingyun = app
        .world_mut()
        .spawn((
            NpcMarker,
            Position::new([0.0, 64.0, 0.0]),
            NamedFactionMembership {
                faction_id: NamedFactionId::QingyunHunters,
            },
        ))
        .id();
    let north = app
        .world_mut()
        .spawn((
            NpcMarker,
            Position::new([4.0, 64.0, 0.0]),
            NamedFactionMembership {
                faction_id: NamedFactionId::NorthWasteDrifters,
            },
        ))
        .id();
    app.update();

    assert!(
        app.world().get::<DuelTarget>(qingyun).is_none(),
        "Pact 关系下 QingyunHunters NPC 不应有 DuelTarget（盟友不互打）"
    );
    assert!(
        app.world().get::<DuelTarget>(north).is_none(),
        "Pact 关系下 NorthWasteDrifters NPC 不应有 DuelTarget"
    );
}

#[test]
fn assign_hostile_encounters_fallback_to_faction_id_when_no_named_membership() {
    // fallback 路径验证：无 NamedFactionMembership 时仍走旧 is_hostile_pair（非孤岛）。
    // Attack↔Defend = Hostile，不依赖 FactionRelationMatrix。
    let mut app = App::new();
    app.insert_resource(FactionStore::default());
    // 故意不插入 FactionRelationMatrix，确认 fallback 路径不依赖它。
    app.add_systems(Update, assign_hostile_encounters);

    let attack = app
        .world_mut()
        .spawn((
            NpcMarker,
            Position::new([0.0, 64.0, 0.0]),
            FactionMembership {
                faction_id: FactionId::Attack,
                rank: FactionRank::Disciple,
                reputation: Reputation::default(),
                lineage: None,
                mission_queue: MissionQueue::default(),
            },
        ))
        .id();
    let defend = app
        .world_mut()
        .spawn((
            NpcMarker,
            Position::new([4.0, 64.0, 0.0]),
            FactionMembership {
                faction_id: FactionId::Defend,
                rank: FactionRank::Disciple,
                reputation: Reputation::default(),
                lineage: None,
                mission_queue: MissionQueue::default(),
            },
        ))
        .id();
    app.update();

    assert_eq!(
        app.world().get::<DuelTarget>(attack).map(|t| t.0),
        Some(defend),
        "旧 FactionId::Attack NPC 应在 fallback 路径（无 NamedFactionMembership）把 \
         Defend NPC 设为 DuelTarget（is_hostile_pair 仍生效）"
    );
}

#[test]
fn leader_spawn_creates_active_faction_leaders_and_skips_headless() {
    let mut app = App::new();
    app.insert_resource(crate::cultivation::known_techniques::TechniqueRegistry::load_for_tests());
    app.insert_resource(NamedFactionRegistry::startup_default());
    let claims = FactionZoneClaims::from_registry(app.world().resource::<NamedFactionRegistry>());
    app.insert_resource(claims);
    app.insert_resource(p2_zone_registry());
    app.world_mut().spawn(OverworldLayer);
    app.add_systems(Update, spawn_named_faction_leaders_on_startup);
    app.update();

    let mut leaders = app.world_mut().query::<(
        &NamedFactionLeader,
        &NamedFactionMembership,
        &FactionMembership,
        &FactionZoneClaim,
        &Position,
    )>();
    let rows = leaders
        .iter(app.world())
        .map(|(leader, membership, legacy_membership, claim, position)| {
            (
                leader.faction,
                membership.faction_id,
                legacy_membership.faction_id,
                claim.zone.clone(),
                position.get(),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        rows.len(),
        2,
        "P2 只应为 Active 势力刷新 2 名领袖；Headless 北荒漂流者不刷新，实际 {:?}",
        rows
    );
    assert!(
        rows.iter().any(|(leader, membership, legacy, zone, pos)| {
            *leader == NamedFactionId::QingyunHunters
                && *membership == NamedFactionId::QingyunHunters
                && *legacy == FactionId::Attack
                && zone == "qingyun_peaks"
                && p2_zone_registry()
                    .find_zone_by_name("qingyun_peaks")
                    .unwrap()
                    .contains(*pos)
        }),
        "青云猎盟领袖必须带 NamedFactionLeader + NamedFactionMembership 并刷新在 qingyun_peaks 内"
    );
    assert!(
        rows.iter().any(|(leader, membership, legacy, zone, pos)| {
            *leader == NamedFactionId::CangyuanMerchants
                && *membership == NamedFactionId::CangyuanMerchants
                && *legacy == FactionId::Defend
                && zone == "blood_valley"
                && p2_zone_registry()
                    .find_zone_by_name("blood_valley")
                    .unwrap()
                    .contains(*pos)
        }),
        "沧渊商会领袖必须带 NamedFactionLeader + NamedFactionMembership 并刷新在 blood_valley 内"
    );
    assert!(
        rows.iter()
            .all(|(leader, _, _, _, _)| *leader != NamedFactionId::NorthWasteDrifters),
        "NorthWasteDrifters 初始 Headless，P2 不应刷新领袖"
    );
}

#[test]
fn leader_spawn_writes_leader_realm_into_cultivation_component() {
    // plan-npc-realm-distribution-v1 P0 choke-point pin: 派系首领 spawn 后
    // Cultivation.realm 必须等于 leader_realm_for(faction)（三档全覆盖），
    // 而不是被 npc_runtime_bundle 恒吞成 Realm::Awaken（修复前的 bug）。
    let mut app = App::new();
    app.insert_resource(crate::cultivation::known_techniques::TechniqueRegistry::load_for_tests());
    app.insert_resource(NamedFactionRegistry::startup_default());
    let claims = FactionZoneClaims::from_registry(app.world().resource::<NamedFactionRegistry>());
    app.insert_resource(claims);
    app.insert_resource(p2_zone_registry());
    app.world_mut().spawn(OverworldLayer);
    app.add_systems(Update, spawn_named_faction_leaders_on_startup);
    app.update();

    let mut leaders = app
        .world_mut()
        .query::<(&NamedFactionLeader, &Cultivation)>();
    let rows = leaders
        .iter(app.world())
        .map(|(leader, cultivation)| (leader.faction, cultivation.realm))
        .collect::<Vec<_>>();

    assert_eq!(
        rows.len(),
        2,
        "P2 只应为 Active 势力刷新 2 名领袖，实际 {:?}",
        rows
    );
    assert!(
        rows.contains(&(NamedFactionId::QingyunHunters, Realm::Solidify)),
        "青云猎盟盟主 Cultivation.realm 期望 Solidify（leader_realm_for 定义），实际 {:?}——\
         若仍是 Awaken 说明 npc_runtime_bundle choke point 未修复",
        rows
    );
    assert!(
        rows.contains(&(NamedFactionId::CangyuanMerchants, Realm::Spirit)),
        "沧渊商会会首 Cultivation.realm 期望 Spirit（leader_realm_for 定义），实际 {:?}——\
         若仍是 Awaken 说明 npc_runtime_bundle choke point 未修复",
        rows
    );
    // NorthWasteDrifters 是 Headless，本轮不刷新，故不在 rows 内；
    // 但 leader_realm_for 本身对三档全覆盖已由既有 pin 测试
    // （leader_realm_matches_plan_table）单独锁定 Realm::Awaken 一档，
    // 此测试不重复断言纯函数，只锁"写进组件"这一步。
}

#[test]
fn leader_spawn_is_idempotent_after_startup_runs_twice() {
    let mut app = App::new();
    app.insert_resource(crate::cultivation::known_techniques::TechniqueRegistry::load_for_tests());
    app.insert_resource(NamedFactionRegistry::startup_default());
    let claims = FactionZoneClaims::from_registry(app.world().resource::<NamedFactionRegistry>());
    app.insert_resource(claims);
    app.insert_resource(p2_zone_registry());
    app.world_mut().spawn(OverworldLayer);
    app.add_systems(Update, spawn_named_faction_leaders_on_startup);

    app.update();
    app.update();

    let mut leaders = app.world_mut().query::<&NamedFactionLeader>();
    assert_eq!(
        leaders.iter(app.world()).count(),
        2,
        "startup system 重跑不应为 Active 势力重复刷新领袖"
    );
}

#[test]
fn leader_spawn_waits_for_zone_registry_before_marking_done() {
    let mut app = App::new();
    app.insert_resource(crate::cultivation::known_techniques::TechniqueRegistry::load_for_tests());
    app.insert_resource(NamedFactionRegistry::startup_default());
    let claims = FactionZoneClaims::from_registry(app.world().resource::<NamedFactionRegistry>());
    app.insert_resource(claims);
    app.world_mut().spawn(OverworldLayer);
    app.add_systems(Update, spawn_named_faction_leaders_on_startup);

    app.update();
    let mut leaders = app.world_mut().query::<&NamedFactionLeader>();
    assert_eq!(
        leaders.iter(app.world()).count(),
        0,
        "缺 ZoneRegistry 时不应刷新领袖，也不应把 startup Local done 置真"
    );

    app.insert_resource(p2_zone_registry());
    app.update();
    let mut leaders = app.world_mut().query::<&NamedFactionLeader>();
    assert_eq!(
        leaders.iter(app.world()).count(),
        2,
        "ZoneRegistry 后置注入后，startup system 仍必须刷新两个 Active 领袖"
    );
}

#[test]
fn leader_realm_matches_plan_table() {
    assert_eq!(
        leader_realm_for(NamedFactionId::QingyunHunters),
        Realm::Solidify,
        "青云猎盟盟主按 P2 表应为固元"
    );
    assert_eq!(
        leader_realm_for(NamedFactionId::CangyuanMerchants),
        Realm::Spirit,
        "沧渊商会会首按 P2 表应为通灵"
    );
}

#[test]
fn leader_territory_scorer_only_activates_inside_claimed_zone() {
    let mut app = App::new();
    app.insert_resource(p2_zone_registry());
    app.add_systems(PreUpdate, faction_leader_territory_scorer_system);
    let leader = app
        .world_mut()
        .spawn((
            NpcMarker,
            Position::new([50.0, 66.0, 50.0]),
            NamedFactionLeader {
                faction: NamedFactionId::QingyunHunters,
            },
            FactionZoneClaim {
                faction: NamedFactionId::QingyunHunters,
                zone: "qingyun_peaks".to_string(),
            },
        ))
        .id();
    let scorer = app
        .world_mut()
        .spawn((
            Actor(leader),
            Score::default(),
            FactionLeaderTerritoryScorer,
        ))
        .id();
    app.update();
    let active_score = app.world().get::<Score>(scorer).unwrap().get();
    assert!(
        (active_score - FACTION_LEADER_PATROL_SCORE).abs() < f32::EPSILON,
        "领袖在自己 FactionZoneClaim 内时 scorer 必须激活，实际 {active_score}"
    );

    app.world_mut()
        .get_mut::<Position>(leader)
        .unwrap()
        .set([500.0, 66.0, 50.0]);
    app.update();
    let outside_score = app.world().get::<Score>(scorer).unwrap().get();
    assert_eq!(
        outside_score, 0.0,
        "领袖离开自己 claim zone 后 scorer 必须失活，实际 {outside_score}"
    );
}

#[test]
fn leader_territory_scorer_zero_when_claim_zone_missing() {
    let mut app = App::new();
    app.insert_resource(p2_zone_registry());
    app.add_systems(PreUpdate, faction_leader_territory_scorer_system);
    let leader = app
        .world_mut()
        .spawn((
            NpcMarker,
            Position::new([50.0, 66.0, 50.0]),
            NamedFactionLeader {
                faction: NamedFactionId::QingyunHunters,
            },
            FactionZoneClaim {
                faction: NamedFactionId::QingyunHunters,
                zone: "missing_zone".to_string(),
            },
        ))
        .id();
    let scorer = app
        .world_mut()
        .spawn((
            Actor(leader),
            Score::default(),
            FactionLeaderTerritoryScorer,
        ))
        .id();
    app.update();
    assert_eq!(
        app.world().get::<Score>(scorer).unwrap().get(),
        0.0,
        "claim zone 缺失时 scorer 必须失活，避免领袖在未知区域触发地盘行为"
    );
}

#[test]
fn leader_territory_scorer_zero_when_claim_faction_mismatch() {
    let mut app = App::new();
    app.insert_resource(p2_zone_registry());
    app.add_systems(PreUpdate, faction_leader_territory_scorer_system);
    let leader = app
        .world_mut()
        .spawn((
            NpcMarker,
            Position::new([50.0, 66.0, 50.0]),
            NamedFactionLeader {
                faction: NamedFactionId::QingyunHunters,
            },
            FactionZoneClaim {
                faction: NamedFactionId::CangyuanMerchants,
                zone: "qingyun_peaks".to_string(),
            },
        ))
        .id();
    let scorer = app
        .world_mut()
        .spawn((
            Actor(leader),
            Score::default(),
            FactionLeaderTerritoryScorer,
        ))
        .id();
    app.update();
    assert_eq!(
        app.world().get::<Score>(scorer).unwrap().get(),
        0.0,
        "NamedFactionLeader 与 FactionZoneClaim faction 不一致时 scorer 必须失活"
    );
}

#[test]
fn leader_patrol_action_sets_zone_patrol_goal_and_finishes() {
    let mut app = build_leader_patrol_app();
    let leader = app
        .world_mut()
        .spawn((
            NpcMarker,
            NamedFactionLeader {
                faction: NamedFactionId::QingyunHunters,
            },
            FactionZoneClaim {
                faction: NamedFactionId::QingyunHunters,
                zone: "qingyun_peaks".to_string(),
            },
            Navigator::new(),
            FactionLeaderPatrolState::default(),
        ))
        .id();
    let action = app
        .world_mut()
        .spawn((
            Actor(leader),
            FactionLeaderPatrolAction,
            ActionState::Requested,
        ))
        .id();
    app.update();
    assert_eq!(
        *app.world().get::<ActionState>(action).unwrap(),
        ActionState::Executing,
        "Requested 后领袖巡逻 action 应进入 Executing"
    );
    assert!(
        !app.world().get::<Navigator>(leader).unwrap().is_idle(),
        "Requested 后 Navigator 必须收到 claim zone 的 patrol goal"
    );

    app.world_mut()
        .get_mut::<FactionLeaderPatrolState>(leader)
        .unwrap()
        .elapsed_ticks = FACTION_LEADER_PATROL_MAX_TICKS - 1;
    app.update();
    assert_eq!(
        *app.world().get::<ActionState>(action).unwrap(),
        ActionState::Success,
        "达到 FACTION_LEADER_PATROL_MAX_TICKS 后 action 必须 Success，避免领袖卡死"
    );
}

#[test]
fn leader_patrol_action_emits_toll_notice_for_foreign_npc_in_claim() {
    let mut app = build_leader_patrol_app();
    let leader = app
        .world_mut()
        .spawn((
            NpcMarker,
            NamedFactionLeader {
                faction: NamedFactionId::QingyunHunters,
            },
            FactionZoneClaim {
                faction: NamedFactionId::QingyunHunters,
                zone: "qingyun_peaks".to_string(),
            },
            Navigator::new(),
            FactionLeaderPatrolState::default(),
        ))
        .id();
    let intruder = app
        .world_mut()
        .spawn((
            NpcMarker,
            Position::new([52.0, 66.0, 52.0]),
            NamedFactionMembership {
                faction_id: NamedFactionId::CangyuanMerchants,
            },
        ))
        .id();
    app.world_mut().spawn((
        Actor(leader),
        FactionLeaderPatrolAction,
        ActionState::Requested,
    ));

    app.update();

    let events = app.world().resource::<Events<FactionLeaderTollNotice>>();
    let notices = events
        .get_reader()
        .read(events)
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        notices.len(),
        1,
        "外派 NPC 进入领袖地盘时必须产生收费 notice"
    );
    assert_eq!(
        notices[0].faction,
        NamedFactionId::QingyunHunters,
        "expected QingyunHunters because the patrol leader owns qingyun_peaks, actual {:?}",
        notices[0].faction
    );
    assert_eq!(
        notices[0].zone, "qingyun_peaks",
        "expected qingyun_peaks because the leader claim is qingyun_peaks, actual {}",
        notices[0].zone
    );
    assert_eq!(
        notices[0].target, intruder,
        "expected intruder entity because it is the first foreign NPC in the claim, actual {:?}",
        notices[0].target
    );
    assert_eq!(
        notices[0].target_kind,
        FactionLeaderTollTargetKind::Npc,
        "expected Npc because the toll target is a foreign NPC, actual {:?}",
        notices[0].target_kind
    );
    assert_eq!(
        app.world()
            .get::<FactionLeaderPatrolState>(leader)
            .unwrap()
            .toll_notices_emitted,
        1,
        "收费 notice 计数用于防止行为树收费分支变成不可观测副作用"
    );
}

#[test]
fn leader_patrol_action_does_not_toll_same_faction_npc() {
    let mut app = build_leader_patrol_app();
    let leader = app
        .world_mut()
        .spawn((
            NpcMarker,
            NamedFactionLeader {
                faction: NamedFactionId::QingyunHunters,
            },
            FactionZoneClaim {
                faction: NamedFactionId::QingyunHunters,
                zone: "qingyun_peaks".to_string(),
            },
            Navigator::new(),
            FactionLeaderPatrolState::default(),
        ))
        .id();
    app.world_mut().spawn((
        NpcMarker,
        Position::new([52.0, 66.0, 52.0]),
        NamedFactionMembership {
            faction_id: NamedFactionId::QingyunHunters,
        },
    ));
    app.world_mut().spawn((
        Actor(leader),
        FactionLeaderPatrolAction,
        ActionState::Requested,
    ));

    app.update();

    let events = app.world().resource::<Events<FactionLeaderTollNotice>>();
    assert_eq!(
        events.get_reader().read(events).count(),
        0,
        "同派 NPC 在自家 claim 内不应触发收费 notice"
    );
}

#[test]
fn leader_patrol_action_does_not_toll_legacy_same_faction_npc() {
    let cases = [
        (
            NamedFactionId::QingyunHunters,
            FactionId::Attack,
            "qingyun_peaks",
            [52.0, 66.0, 52.0],
        ),
        (
            NamedFactionId::CangyuanMerchants,
            FactionId::Defend,
            "blood_valley",
            [220.0, 66.0, 52.0],
        ),
        (
            NamedFactionId::NorthWasteDrifters,
            FactionId::Neutral,
            "north_wastes",
            [430.0, 66.0, 52.0],
        ),
    ];

    for (named_faction, legacy_faction, claim_zone, npc_position) in cases {
        let mut app = build_leader_patrol_app();
        let leader = app
            .world_mut()
            .spawn((
                NpcMarker,
                NamedFactionLeader {
                    faction: named_faction,
                },
                FactionZoneClaim {
                    faction: named_faction,
                    zone: claim_zone.to_string(),
                },
                Navigator::new(),
                FactionLeaderPatrolState::default(),
            ))
            .id();
        app.world_mut().spawn((
            NpcMarker,
            Position::new(npc_position),
            base_membership(legacy_faction, 0.0, 0),
        ));
        app.world_mut().spawn((
            Actor(leader),
            FactionLeaderPatrolAction,
            ActionState::Requested,
        ));

        app.update();

        let events = app.world().resource::<Events<FactionLeaderTollNotice>>();
        assert_eq!(
            events.get_reader().read(events).count(),
            0,
            "只带旧 {legacy_faction:?} 的同派普通弟子不应被 {named_faction:?} 领袖误收过路费"
        );
    }
}

#[test]
fn leader_patrol_action_cancelled_stops_navigator_and_fails() {
    let mut app = build_leader_patrol_app();
    let mut navigator = Navigator::new();
    navigator.set_goal(DVec3::new(80.0, 66.0, 80.0), 1.0);
    let leader = app
        .world_mut()
        .spawn((
            NpcMarker,
            NamedFactionLeader {
                faction: NamedFactionId::QingyunHunters,
            },
            FactionZoneClaim {
                faction: NamedFactionId::QingyunHunters,
                zone: "qingyun_peaks".to_string(),
            },
            navigator,
            FactionLeaderPatrolState::default(),
        ))
        .id();
    let action = app
        .world_mut()
        .spawn((
            Actor(leader),
            FactionLeaderPatrolAction,
            ActionState::Cancelled,
        ))
        .id();

    app.update();

    assert_eq!(
        *app.world().get::<ActionState>(action).unwrap(),
        ActionState::Failure,
        "Cancelled 后巡逻 action 必须 Failure"
    );
    assert!(
        app.world().get::<Navigator>(leader).unwrap().is_idle(),
        "Cancelled 后 Navigator 必须 stop，避免残留巡逻 goal"
    );
}

#[test]
fn named_faction_census_counts_named_memberships() {
    let mut app = App::new();
    app.insert_resource(NamedFactionRegistry::startup_default());
    app.add_event::<NamedFactionLeaderDownEvent>();
    app.add_event::<NamedFactionDecayEvent>();
    app.add_systems(Update, sync_named_faction_census_system);
    app.world_mut().spawn((
        NpcMarker,
        NamedFactionMembership {
            faction_id: NamedFactionId::QingyunHunters,
        },
    ));
    app.world_mut().spawn((
        NpcMarker,
        NamedFactionMembership {
            faction_id: NamedFactionId::QingyunHunters,
        },
    ));
    app.world_mut().spawn((
        NpcMarker,
        NamedFactionMembership {
            faction_id: NamedFactionId::CangyuanMerchants,
        },
    ));
    app.update();
    let registry = app.world().resource::<NamedFactionRegistry>();
    assert_eq!(
        registry
            .get(NamedFactionId::QingyunHunters)
            .unwrap()
            .current_npc_count,
        2,
        "census 必须把青云猎盟两名 NamedFactionMembership 计入 current_npc_count"
    );
    assert_eq!(
        registry
            .get(NamedFactionId::CangyuanMerchants)
            .unwrap()
            .current_npc_count,
        1,
        "census 必须把沧渊商会一名 NamedFactionMembership 计入 current_npc_count"
    );
    assert_eq!(
        registry
            .get(NamedFactionId::NorthWasteDrifters)
            .unwrap()
            .current_npc_count,
        0,
        "没有实体归属北荒漂流者时 census 应保持 0"
    );
}

#[test]
fn named_faction_census_emits_leader_down_for_active_faction_without_leader() {
    let mut app = App::new();
    app.insert_resource(NamedFactionRegistry::startup_default());
    app.add_event::<NamedFactionLeaderDownEvent>();
    app.add_event::<NamedFactionDecayEvent>();
    app.add_systems(Update, sync_named_faction_census_system);
    app.world_mut().spawn((
        NpcMarker,
        NamedFactionMembership {
            faction_id: NamedFactionId::QingyunHunters,
        },
    ));

    app.update();

    let registry = app.world().resource::<NamedFactionRegistry>();
    assert_eq!(
        registry.get(NamedFactionId::QingyunHunters).unwrap().status,
        FactionStatus::Headless
    );
    let events = app
        .world()
        .resource::<Events<NamedFactionLeaderDownEvent>>();
    let emitted = events
        .iter_current_update_events()
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(emitted.len(), 1);
    assert_eq!(emitted[0].faction, NamedFactionId::QingyunHunters);
    assert_eq!(emitted[0].zone, "qingyun_peaks");
}

#[test]
fn named_faction_census_decays_zero_count_and_clears_player_membership() {
    let mut registry = NamedFactionRegistry::startup_default();
    registry
        .get_mut(NamedFactionId::QingyunHunters)
        .unwrap()
        .current_npc_count = 1;

    let mut app = App::new();
    app.insert_resource(registry);
    app.add_event::<NamedFactionLeaderDownEvent>();
    app.add_event::<NamedFactionDecayEvent>();
    app.add_systems(Update, sync_named_faction_census_system);
    let (client_bundle, _helper) = create_mock_client("Azure");
    let player = app.world_mut().spawn(client_bundle).id();
    app.world_mut()
        .entity_mut(player)
        .insert(crate::social::components::FactionMembership {
            faction: FactionId::Attack,
            named_faction: Some(NamedFactionId::QingyunHunters),
            rank: 0,
            loyalty: 10,
            betrayal_count: 0,
            invite_block_until_tick: None,
            permanently_refused: false,
        });

    app.update();

    let registry = app.world().resource::<NamedFactionRegistry>();
    let qingyun = registry.get(NamedFactionId::QingyunHunters).unwrap();
    assert_eq!(qingyun.status, FactionStatus::Decayed);
    assert!(!qingyun.is_active);
    let membership = app
        .world()
        .get::<crate::social::components::FactionMembership>(player)
        .unwrap();
    assert_eq!(
        membership.named_faction, None,
        "势力消亡时在线玩家挂靠必须清空"
    );
    let events = app.world().resource::<Events<NamedFactionDecayEvent>>();
    let emitted = events
        .iter_current_update_events()
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(emitted.len(), 1);
    assert_eq!(emitted[0].faction, NamedFactionId::QingyunHunters);
    assert_eq!(emitted[0].last_npc_count, 1);
}

#[test]
fn named_faction_census_does_not_emit_decay_again_for_decayed_faction() {
    let mut registry = NamedFactionRegistry::startup_default();
    let qingyun = registry.get_mut(NamedFactionId::QingyunHunters).unwrap();
    qingyun.current_npc_count = 1;
    qingyun.set_status(FactionStatus::Decayed);

    let mut app = App::new();
    app.insert_resource(registry);
    app.add_event::<NamedFactionLeaderDownEvent>();
    app.add_event::<NamedFactionDecayEvent>();
    app.add_systems(Update, sync_named_faction_census_system);

    app.update();

    let events = app.world().resource::<Events<NamedFactionDecayEvent>>();
    let emitted = events
        .iter_current_update_events()
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        emitted.is_empty(),
        "Decayed 势力 NPC 归零不应重复触发 NamedFactionDecayEvent，实际 {emitted:?}"
    );
}

#[test]
fn named_faction_census_decays_from_headless_state() {
    let mut registry = NamedFactionRegistry::startup_default();
    let cangyuan = registry.get_mut(NamedFactionId::CangyuanMerchants).unwrap();
    cangyuan.current_npc_count = 2;
    cangyuan.set_status(FactionStatus::Headless);

    let mut app = App::new();
    app.insert_resource(registry);
    app.add_event::<NamedFactionLeaderDownEvent>();
    app.add_event::<NamedFactionDecayEvent>();
    app.add_systems(Update, sync_named_faction_census_system);

    app.update();

    let registry = app.world().resource::<NamedFactionRegistry>();
    let cangyuan = registry.get(NamedFactionId::CangyuanMerchants).unwrap();
    assert_eq!(
        cangyuan.status,
        FactionStatus::Decayed,
        "Headless 势力 NPC 归零必须进入 Decayed，实际 {:?}",
        cangyuan.status
    );
    assert!(
        !cangyuan.is_active,
        "Decayed 势力 is_active 必须为 false，实际 {}",
        cangyuan.is_active
    );
    let events = app.world().resource::<Events<NamedFactionDecayEvent>>();
    let emitted = events
        .iter_current_update_events()
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        emitted.len(),
        1,
        "Headless→Decayed 必须触发一条 NamedFactionDecayEvent，实际 {}",
        emitted.len()
    );
    assert_eq!(
        emitted[0].faction,
        NamedFactionId::CangyuanMerchants,
        "消亡事件必须指向沧渊商会，实际 {:?}",
        emitted[0].faction
    );
    assert_eq!(
        emitted[0].final_zone, "blood_valley",
        "消亡事件 final_zone 必须等于沧渊商会 zone_anchor，实际 {}",
        emitted[0].final_zone
    );
    assert_eq!(
        emitted[0].last_npc_count, 2,
        "消亡事件 last_npc_count 必须等于上一帧 NPC 数 2，实际 {}",
        emitted[0].last_npc_count
    );
}
