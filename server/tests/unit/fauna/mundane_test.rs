use bong_server::fauna::mundane::{
    entity_kind_for_mundane, mundane_biome_pool, mundane_fauna_negative_zone_wither_system,
    mundane_habitat_allows_spawn, mundane_passive_budget_fn, mundane_season_weight_multiplier,
    mundane_species_for_position, mundane_species_for_position_seasonal, register,
    select_mundane_species, select_mundane_species_weighted, MundaneFaunaKind, MundaneFaunaMarker,
    MundaneFaunaSpecies, MundaneFaunaWithering, NEGATIVE_ZONE_WITHER_SOUND_RECIPE_ID,
    NEGATIVE_ZONE_WITHER_TICKS,
};
use bong_server::network::audio_event_emit::PlaySoundRecipeRequest;
use bong_server::network::vfx_event_emit::VfxEventRequest;
use bong_server::npc::spawn::ambient_scheduler::{
    AmbientMarkerData, AmbientSchedulerConfig, AmbientSchedulerState, ThreatBudget,
};
use bong_server::world::calamity::EVENT_REALM_COLLAPSE;
use bong_server::world::dimension::DimensionKind;
use bong_server::world::season::Season;
use bong_server::world::zone::{Zone, ZoneRegistry};
use valence::prelude::{App, DVec3, Despawned, EntityKind, Events, Position, Update};

fn zone_with_qi(name: &str, spirit_qi: f64) -> Zone {
    Zone {
        name: name.to_string(),
        dimension: DimensionKind::Overworld,
        bounds: (
            DVec3::new(-500.0, 0.0, -500.0),
            DVec3::new(500.0, 200.0, 500.0),
        ),
        spirit_qi,
        danger_level: 1,
        active_events: Vec::new(),
        patrol_anchors: Vec::new(),
        blocked_tiles: Vec::new(),
        qi_equilibrium: 0.0,
        qi_inflow_per_min: 0.0,
    }
}

fn make_zone_registry_with_negative_zone() -> ZoneRegistry {
    let mut zone = zone_with_qi("negative_zone", -0.5);
    zone.bounds = (
        DVec3::new(-500.0, 0.0, -500.0),
        DVec3::new(500.0, 200.0, 500.0),
    );
    ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone],
    }
}

fn wither_test_app() -> App {
    let mut app = App::new();
    app.add_event::<VfxEventRequest>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.add_systems(Update, mundane_fauna_negative_zone_wither_system);
    app
}

#[test]
fn all_nine_variants_present_and_distinct() {
    assert_eq!(MundaneFaunaKind::ALL.len(), 9, "§8.1 #1 锁定 9 变体终表");
    let mut as_strs: Vec<&str> = MundaneFaunaKind::ALL.iter().map(|k| k.as_str()).collect();
    as_strs.sort_unstable();
    as_strs.dedup();
    assert_eq!(
        as_strs.len(),
        9,
        "9 个变体的 as_str() 必须两两不同，否则 narration/loot 文案会撞名"
    );
}

#[test]
fn entity_kind_for_mundane_pins_all_nine_to_native_valence_kind() {
    let expected = [
        (MundaneFaunaKind::Cow, EntityKind::COW),
        (MundaneFaunaKind::Pig, EntityKind::PIG),
        (MundaneFaunaKind::Sheep, EntityKind::SHEEP),
        (MundaneFaunaKind::Chicken, EntityKind::CHICKEN),
        (MundaneFaunaKind::Rabbit, EntityKind::RABBIT),
        (MundaneFaunaKind::Goat, EntityKind::GOAT),
        (MundaneFaunaKind::Frog, EntityKind::FROG),
        (MundaneFaunaKind::Fox, EntityKind::FOX),
        (MundaneFaunaKind::Wolf, EntityKind::WOLF),
    ];
    for (kind, expected_entity_kind) in expected {
        assert_eq!(
            entity_kind_for_mundane(kind),
            expected_entity_kind,
            "{kind:?} 必须映射到原生 {expected_entity_kind:?}（Rail A，client 零改渲染）"
        );
    }
}

#[test]
fn health_max_is_strictly_lower_for_chicken_than_wolf() {
    assert!(
        MundaneFaunaKind::Chicken.health_max() < MundaneFaunaKind::Wolf.health_max(),
        "鸡 health_max={} 必须严格低于狼 health_max={}",
        MundaneFaunaKind::Chicken.health_max(),
        MundaneFaunaKind::Wolf.health_max()
    );
}

#[test]
fn health_max_is_differentiated_across_all_nine_variants() {
    let chicken = MundaneFaunaKind::Chicken.health_max();
    let rabbit = MundaneFaunaKind::Rabbit.health_max();
    let frog = MundaneFaunaKind::Frog.health_max();
    let sheep = MundaneFaunaKind::Sheep.health_max();
    let pig = MundaneFaunaKind::Pig.health_max();
    let goat = MundaneFaunaKind::Goat.health_max();
    let cow = MundaneFaunaKind::Cow.health_max();
    let fox = MundaneFaunaKind::Fox.health_max();
    let wolf = MundaneFaunaKind::Wolf.health_max();

    let t0_max = chicken.max(rabbit).max(frog);
    let t1_min = sheep.min(pig).min(goat).min(cow);
    let t1_max = sheep.max(pig).max(goat).max(cow);
    assert!(
        t0_max < t1_min,
        "T0(鸡/兔/蛙, max={t0_max}) 应严格低于 T1(牛/猪/羊/山羊, min={t1_min})"
    );
    assert!(t1_max < fox, "T1(max={t1_max}) 应低于 T2(狐, {fox})");
    assert!(
        fox < wolf,
        "T2(狐, {fox})应低于 T2.5(狼, {wolf})——狼是凡兽里的真威胁"
    );
}

#[test]
fn health_max_all_positive() {
    for kind in MundaneFaunaKind::ALL {
        assert!(
            kind.health_max() > 0.0,
            "{kind:?} health_max 必须为正数，实际 {}",
            kind.health_max()
        );
    }
}

#[test]
fn plains_biome_pool_matches_expected_species_set() {
    let pool = mundane_biome_pool("spawn");
    let mut got: Vec<&str> = pool.iter().map(|k| k.as_str()).collect();
    got.sort_unstable();
    assert_eq!(got, vec!["chicken", "cow", "pig", "rabbit", "sheep"]);
}

#[test]
fn marsh_biome_pool_matches_expected_species_set() {
    let mut got: Vec<&str> = mundane_biome_pool("lingquan_marsh")
        .iter()
        .map(|k| k.as_str())
        .collect();
    got.sort_unstable();
    assert_eq!(got, vec!["frog", "rabbit"]);
}

#[test]
fn peaks_biome_pool_matches_expected_species_set() {
    let mut got: Vec<&str> = mundane_biome_pool("qingyun_peaks")
        .iter()
        .map(|k| k.as_str())
        .collect();
    got.sort_unstable();
    assert_eq!(got, vec!["goat", "sheep"]);
}

#[test]
fn wastes_biome_pool_matches_expected_species_set() {
    let mut got: Vec<&str> = mundane_biome_pool("north_wastes")
        .iter()
        .map(|k| k.as_str())
        .collect();
    got.sort_unstable();
    assert_eq!(got, vec!["fox", "rabbit", "wolf"]);
}

#[test]
fn wolf_only_appears_in_wastes_pool() {
    for name in ["spawn", "lingquan_marsh", "qingyun_peaks"] {
        assert!(
            !mundane_biome_pool(name).contains(&MundaneFaunaKind::Wolf),
            "zone={name} 不应含狼（狼只在 north_wastes 池）"
        );
    }
}

#[test]
fn select_mundane_species_returns_none_for_empty_pool() {
    assert_eq!(select_mundane_species(&[], 42), None);
}

#[test]
fn species_for_position_is_deterministic_for_same_zone_and_position() {
    let pos = DVec3::new(12.0, 64.0, -8.0);
    assert_eq!(
        mundane_species_for_position("north_wastes", pos),
        mundane_species_for_position("north_wastes", pos),
        "同一 (zone_name, position) 必须复现同一物种——dormant 复活依赖此确定性，\
         不持久化 MundaneFaunaKind 字段"
    );
}

#[test]
fn mundane_passive_budget_matches_8_1_4_decision() {
    for danger in [0u8, 1, 4, 7, 255] {
        let budget = mundane_passive_budget_fn(danger);
        assert_eq!(
            budget,
            ThreatBudget {
                max_alive: 3,
                spawn_interval_ticks: 400,
                pack_size_range: (1, 1),
            },
            "danger={danger} 不应改变凡兽 passive 预算（凡兽不分 danger 分级）"
        );
    }
}

#[test]
fn marker_new_and_home_zone_round_trip() {
    let marker = MundaneFaunaMarker::new(1234, "test_zone".to_string());
    assert_eq!(marker.spawned_at, 1234);
    assert_eq!(marker.home_zone(), "test_zone");
}

#[test]
fn habitat_blocks_dead_zone_narrow_band() {
    let zone = zone_with_qi("test_zone", 0.005);
    assert!(
        !mundane_habitat_allows_spawn(&zone),
        "spirit_qi=0.005 落在 is_dead_zone 窄带 [0,0.01) 内，必须拦截凡兽生成"
    );
}

#[test]
fn habitat_allows_boundary_at_dead_zone_threshold() {
    use bong_server::cultivation::dead_zone::DEAD_ZONE_QI_THRESHOLD;
    let zone = zone_with_qi("test_zone", DEAD_ZONE_QI_THRESHOLD);
    assert!(
        mundane_habitat_allows_spawn(&zone),
        "spirit_qi == DEAD_ZONE_QI_THRESHOLD（边界，exclusive）不应被判定为死域"
    );
}

#[test]
fn habitat_allows_negative_qi_zone_spawn_not_gated_here() {
    let zone = zone_with_qi("test_zone", -0.5);
    assert!(
        mundane_habitat_allows_spawn(&zone),
        "负灵域不应被 habitat 门槛拦截（由负灵域灭杀系统另行处理）"
    );
}

#[test]
fn habitat_allows_normal_zone() {
    let zone = zone_with_qi("test_zone", 0.5);
    assert!(mundane_habitat_allows_spawn(&zone));
}

#[test]
fn habitat_blocks_realm_collapse_zone_even_with_healthy_qi() {
    let mut zone = zone_with_qi("test_zone", 0.9);
    zone.active_events.push(EVENT_REALM_COLLAPSE.to_string());
    assert!(
        !mundane_habitat_allows_spawn(&zone),
        "REALM_COLLAPSE 事件期间即便 spirit_qi 健康也应拦截凡兽生成"
    );
}

#[test]
fn mundane_pool_fn_returns_none_for_dead_zone() {
    let mut app = App::new();
    let layer = app.world_mut().spawn_empty().id();
    let zone = zone_with_qi("test_zone", 0.0);
    let spawned = {
        let mut commands = app.world_mut().commands();
        bong_server::fauna::mundane::mundane_pool_fn(
            &mut commands,
            layer,
            &zone,
            DVec3::new(10.0, 64.0, 10.0),
            DVec3::new(10.0, 64.0, 10.0),
            Season::Summer,
        )
    };
    assert_eq!(
        spawned, None,
        "死域 zone 必须恒返回 None（调用方 ambient_scheduler_system 已处理该分支）"
    );
}

#[test]
fn season_weight_summer_favors_frog() {
    assert_eq!(
        mundane_season_weight_multiplier(MundaneFaunaKind::Frog, Season::Summer),
        3
    );
    assert_eq!(
        mundane_season_weight_multiplier(MundaneFaunaKind::Cow, Season::Summer),
        1,
        "夏季不应给非蛙物种加权"
    );
}

#[test]
fn season_weight_winter_favors_rabbit_and_goat() {
    for kind in [MundaneFaunaKind::Rabbit, MundaneFaunaKind::Goat] {
        assert_eq!(
            mundane_season_weight_multiplier(kind, Season::Winter),
            3,
            "{kind:?} 冬季应加权（plan 原文明确点名兔/山羊）"
        );
    }
    assert_eq!(
        mundane_season_weight_multiplier(MundaneFaunaKind::Frog, Season::Winter),
        1,
        "冬季不应给蛙加权（夏季专属）"
    );
}

#[test]
fn season_weight_xizhuan_transition_is_neutral() {
    for kind in MundaneFaunaKind::ALL {
        assert_eq!(
            mundane_season_weight_multiplier(kind, Season::SummerToWinter),
            1,
            "汐转期（夏→冬）应中性无偏权，{kind:?} 不应例外"
        );
        assert_eq!(
            mundane_season_weight_multiplier(kind, Season::WinterToSummer),
            1,
            "汐转期（冬→夏）应中性无偏权，{kind:?} 不应例外"
        );
    }
}

#[test]
fn select_mundane_species_weighted_returns_none_for_empty_pool() {
    assert_eq!(
        select_mundane_species_weighted(&[], Season::Summer, 42),
        None
    );
}

#[test]
fn fixed_bot_ambient_witness_selects_cow_in_summer() {
    let position = DVec3::new(5.0, 73.0, 3.0);
    assert_eq!(
        mundane_species_for_position_seasonal("spawn", position, Season::Summer),
        MundaneFaunaKind::Cow,
        "bot ambient witness (season=Summer, zone=spawn,x=5,z=3) must remain Cow; \
         changing this pin also requires an explicit protocol witness update"
    );
}

#[test]
fn wither_system_ignores_entities_outside_negative_zone_missing_registry() {
    let mut app = wither_test_app();
    let entity = app
        .world_mut()
        .spawn((
            Position::new([0.0, 64.0, 0.0]),
            MundaneFaunaSpecies(MundaneFaunaKind::Rabbit),
        ))
        .id();
    app.update();
    assert!(
        app.world().get::<MundaneFaunaWithering>(entity).is_none(),
        "无 ZoneRegistry 时不应插入枯萎计时器（也不应 panic）"
    );
}

#[test]
fn wither_system_starts_timer_on_entering_negative_zone() {
    let mut app = wither_test_app();
    app.insert_resource(make_zone_registry_with_negative_zone());
    let entity = app
        .world_mut()
        .spawn((
            Position::new([0.0, 64.0, 0.0]),
            MundaneFaunaSpecies(MundaneFaunaKind::Rabbit),
        ))
        .id();
    app.update();
    let withering = app
        .world()
        .get::<MundaneFaunaWithering>(entity)
        .expect("负灵域内应插入 MundaneFaunaWithering 计时器");
    assert_eq!(withering.elapsed_ticks, 0, "首 tick 计时器应从 0 起步");
}

#[test]
fn wither_system_despawns_after_full_countdown() {
    let mut app = wither_test_app();
    app.insert_resource(make_zone_registry_with_negative_zone());
    let entity = app
        .world_mut()
        .spawn((
            Position::new([0.0, 64.0, 0.0]),
            MundaneFaunaSpecies(MundaneFaunaKind::Rabbit),
        ))
        .id();
    for _ in 0..=NEGATIVE_ZONE_WITHER_TICKS {
        app.update();
    }
    assert!(
        app.world().get::<Despawned>(entity).is_some(),
        "枯萎倒计时（{NEGATIVE_ZONE_WITHER_TICKS} tick）到期后必须 insert(Despawned)"
    );
}

#[test]
fn wither_system_does_not_despawn_before_countdown_completes() {
    let mut app = wither_test_app();
    app.insert_resource(make_zone_registry_with_negative_zone());
    let entity = app
        .world_mut()
        .spawn((
            Position::new([0.0, 64.0, 0.0]),
            MundaneFaunaSpecies(MundaneFaunaKind::Rabbit),
        ))
        .id();
    for _ in 0..(NEGATIVE_ZONE_WITHER_TICKS - 1) {
        app.update();
    }
    assert!(
        app.world().get::<Despawned>(entity).is_none(),
        "倒计时未到期前不应 despawn"
    );
}

#[test]
fn wither_system_recovers_timer_when_zone_qi_improves() {
    let mut app = wither_test_app();
    app.insert_resource(make_zone_registry_with_negative_zone());
    let entity = app
        .world_mut()
        .spawn((
            Position::new([0.0, 64.0, 0.0]),
            MundaneFaunaSpecies(MundaneFaunaKind::Rabbit),
        ))
        .id();
    app.update();
    assert!(app.world().get::<MundaneFaunaWithering>(entity).is_some());

    app.insert_resource(ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone_with_qi("negative_zone", 0.5)],
    });
    app.update();
    assert!(
        app.world().get::<MundaneFaunaWithering>(entity).is_none(),
        "zone qi 回升离开负灵域后应移除计时器（恢复）"
    );
    assert!(
        app.world().get::<Despawned>(entity).is_none(),
        "恢复的实体不应被 despawn"
    );
}

#[test]
fn wither_system_emits_burst_particle_on_entering_and_final_burst_plus_sound_on_despawn() {
    let mut app = wither_test_app();
    app.insert_resource(make_zone_registry_with_negative_zone());
    app.world_mut().spawn((
        Position::new([0.0, 64.0, 0.0]),
        MundaneFaunaSpecies(MundaneFaunaKind::Rabbit),
    ));
    app.update();
    {
        let events = app.world().resource::<Events<VfxEventRequest>>();
        let count = events.get_reader().read(events).count();
        assert!(count >= 1, "首次进入负灵域应发至少一次 burst 粒子事件");
    }

    for _ in 0..NEGATIVE_ZONE_WITHER_TICKS {
        app.update();
    }
    let audio_events = app.world().resource::<Events<PlaySoundRecipeRequest>>();
    let audio: Vec<_> = audio_events
        .get_reader()
        .read(audio_events)
        .cloned()
        .collect();
    assert_eq!(audio.len(), 1, "despawn 瞬间应恰好发一次消亡音效");
    assert_eq!(audio[0].recipe_id, NEGATIVE_ZONE_WITHER_SOUND_RECIPE_ID);
}

#[test]
fn is_predator_true_only_for_fox_and_wolf() {
    for kind in MundaneFaunaKind::ALL {
        let expected = matches!(kind, MundaneFaunaKind::Fox | MundaneFaunaKind::Wolf);
        assert_eq!(
            kind.is_predator(),
            expected,
            "{kind:?} is_predator 不符预期"
        );
    }
}

#[test]
fn is_t0_true_only_for_chicken_rabbit_frog() {
    for kind in MundaneFaunaKind::ALL {
        let expected = matches!(
            kind,
            MundaneFaunaKind::Chicken | MundaneFaunaKind::Rabbit | MundaneFaunaKind::Frog
        );
        assert_eq!(kind.is_t0(), expected, "{kind:?} is_t0 不符预期");
    }
}

#[test]
fn register_wires_mundane_scheduler_independent_of_threat_budget() {
    let mut app = App::new();
    register(&mut app);

    let config = app
        .world()
        .resource::<AmbientSchedulerConfig<MundaneFaunaMarker>>();
    assert!(
        !config.counts_against_threat_budget,
        "MundaneFaunaMarker 调度必须 counts_against_threat_budget=false（§8.1 #3）——\
         否则凡兽活体会污染 plan-ambient-threat-v1 的 zone 威胁密度统计，两套 marker \
         预算必须完全独立。若有人把它翻成 true 请改 §8.1 #3 决议而非本测试"
    );
    assert!(
        app.world()
            .get_resource::<AmbientSchedulerState<MundaneFaunaMarker>>()
            .is_some(),
        "register 必须安装 MundaneFaunaMarker 专属调度状态资源（按 M 单态化，\
         与 AmbientThreatMarker 的 AmbientSchedulerState 结构隔离）"
    );
}
