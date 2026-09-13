use bong_server::cultivation::components::{CrackCause, Meridian, MeridianId, MeridianSystem};
use bong_server::cultivation::meridian::severed::{
    apply_severed_event_system, check_meridian_dependencies, check_meridian_runtime_integrity,
    enforce_severed_state, meridian_severed_detection_tick, severed_source_from_crack,
    try_acupoint_repair, AcupointRepairOutcome, MeridianSeveredEvent, MeridianSeveredPermanent,
    SeveredSource, SkillMeridianDependencies,
};
use serde_json::{from_str, to_string};
use valence::prelude::{App, Entity, IntoSystemConfigs};

// --- MeridianSeveredPermanent: 写入 / 重复 / 持久化 / 跨周目重置 (8 tests) ---

#[test]
fn permanent_default_is_empty() {
    let p = MeridianSeveredPermanent::default();
    assert_eq!(p.severed_count(), 0);
    assert!(!p.is_severed(MeridianId::Lung));
    assert!(p.record_for(MeridianId::Lung).is_none());
}

#[test]
fn permanent_insert_records_tick_and_source() {
    let mut p = MeridianSeveredPermanent::default();
    let inserted = p.insert(MeridianId::Lung, SeveredSource::CombatWound, 100);
    assert!(inserted, "首次写入应返回 true");
    assert!(p.is_severed(MeridianId::Lung));
    let r = p.record_for(MeridianId::Lung).expect("record should exist");
    assert_eq!(r.at_tick, 100);
    assert_eq!(r.source, SeveredSource::CombatWound);
}

#[test]
fn permanent_insert_duplicate_keeps_first_record() {
    let mut p = MeridianSeveredPermanent::default();
    p.insert(MeridianId::Heart, SeveredSource::CombatWound, 50);
    let again = p.insert(MeridianId::Heart, SeveredSource::TribulationFail, 200);
    assert!(!again, "重复写入应返回 false");
    let r = p
        .record_for(MeridianId::Heart)
        .expect("first record retained");
    assert_eq!(r.at_tick, 50, "首次时戳保留");
    assert_eq!(r.source, SeveredSource::CombatWound, "首次来源保留");
}

#[test]
fn permanent_serde_round_trip_preserves_all_fields() {
    // 跨 server restart 持久化：serde JSON 完整往返
    let mut p = MeridianSeveredPermanent::default();
    p.insert(MeridianId::Lung, SeveredSource::CombatWound, 100);
    p.insert(MeridianId::Du, SeveredSource::TribulationFail, 200);
    p.dead_meridians.insert(MeridianId::Heart.channel_id());
    let s = to_string(&p).expect("serialize");
    let back: MeridianSeveredPermanent = from_str(&s).expect("deserialize");
    assert_eq!(back, p);
    assert!(back.is_severed(MeridianId::Lung));
    assert!(back.is_severed(MeridianId::Du));
    assert!(back.is_dead(MeridianId::Heart));
}

#[test]
fn permanent_reset_clears_all_state_for_cross_lifecycle() {
    // 决策门 #1 = B：跨周目新角色 SEVERED 重置
    let mut p = MeridianSeveredPermanent::default();
    p.insert(MeridianId::Lung, SeveredSource::CombatWound, 100);
    p.insert(MeridianId::Heart, SeveredSource::TribulationFail, 200);
    p.dead_meridians.insert(MeridianId::Heart.channel_id());
    p.reset();
    assert_eq!(p.severed_count(), 0);
    assert!(p.dead_meridians.is_empty());
    assert!(p.severed_at.is_empty());
}

#[test]
fn permanent_insert_independent_meridians_accumulates() {
    let mut p = MeridianSeveredPermanent::default();
    p.insert(MeridianId::Lung, SeveredSource::CombatWound, 100);
    p.insert(MeridianId::LargeIntestine, SeveredSource::OverloadTear, 110);
    p.insert(MeridianId::Du, SeveredSource::VoluntarySever, 120);
    assert_eq!(p.severed_count(), 3);
}

#[test]
fn permanent_handles_other_source_payload() {
    let mut p = MeridianSeveredPermanent::default();
    let src = SeveredSource::Other("unforeseen-cause".to_string());
    p.insert(MeridianId::Chong, src.clone(), 999);
    assert_eq!(p.record_for(MeridianId::Chong).unwrap().source, src);
}

#[test]
fn permanent_dead_state_queryable() {
    let mut p = MeridianSeveredPermanent::default();
    p.insert(MeridianId::Liver, SeveredSource::DuguDistortion, 500);
    assert!(!p.is_dead(MeridianId::Liver), "SEVERED ≠ dead 默认");
    p.dead_meridians.insert(MeridianId::Liver.channel_id());
    assert!(p.is_dead(MeridianId::Liver));
}

// --- check_meridian_dependencies: deps INTACT / SEVERED / 多依赖 / 无依赖 (8 tests) ---

#[test]
fn check_deps_no_severed_component_passes() {
    // 无 component 表示玩家从未受过 SEVERED 损伤 —— 检查通过。
    let deps = vec![MeridianId::Lung, MeridianId::Heart];
    assert!(check_meridian_dependencies(&deps, None).is_ok());
}

#[test]
fn check_deps_intact_passes() {
    let p = MeridianSeveredPermanent::default();
    let deps = vec![MeridianId::Lung];
    assert!(check_meridian_dependencies(&deps, Some(&p)).is_ok());
}

#[test]
fn check_deps_severed_rejects_with_offending_id() {
    let mut p = MeridianSeveredPermanent::default();
    p.insert(MeridianId::Lung, SeveredSource::CombatWound, 100);
    let deps = vec![MeridianId::Lung];
    assert_eq!(
        check_meridian_dependencies(&deps, Some(&p)),
        Err(MeridianId::Lung)
    );
}

#[test]
fn check_deps_multi_any_severed_rejects() {
    let mut p = MeridianSeveredPermanent::default();
    p.insert(MeridianId::Heart, SeveredSource::TribulationFail, 100);
    let deps = vec![MeridianId::Lung, MeridianId::Heart, MeridianId::Pericardium];
    assert_eq!(
        check_meridian_dependencies(&deps, Some(&p)),
        Err(MeridianId::Heart)
    );
}

#[test]
fn check_deps_returns_first_severed_in_declaration_order() {
    // 强约束：返回声明顺序中首条 SEVERED，方便招式 cast 站给玩家精确反馈
    let mut p = MeridianSeveredPermanent::default();
    p.insert(MeridianId::Heart, SeveredSource::CombatWound, 100);
    p.insert(MeridianId::Lung, SeveredSource::CombatWound, 100);
    let deps_a = vec![MeridianId::Lung, MeridianId::Heart];
    let deps_b = vec![MeridianId::Heart, MeridianId::Lung];
    assert_eq!(
        check_meridian_dependencies(&deps_a, Some(&p)),
        Err(MeridianId::Lung)
    );
    assert_eq!(
        check_meridian_dependencies(&deps_b, Some(&p)),
        Err(MeridianId::Heart)
    );
}

#[test]
fn check_deps_empty_dependencies_passes() {
    let mut p = MeridianSeveredPermanent::default();
    p.insert(MeridianId::Lung, SeveredSource::CombatWound, 100);
    // 无依赖招式：永远不应被 SEVERED 拦截
    assert!(check_meridian_dependencies(&[], Some(&p)).is_ok());
}

#[test]
fn check_runtime_integrity_passes_when_one_dep_intact() {
    // 退化路径：burst_meridian "任一右臂经脉 integrity > ε" 风格
    let mut meridians = MeridianSystem::default();
    meridians.get_mut(MeridianId::Lung).integrity = 0.0;
    meridians.get_mut(MeridianId::LargeIntestine).integrity = 0.5;
    let deps = vec![MeridianId::Lung, MeridianId::LargeIntestine];
    assert!(check_meridian_runtime_integrity(&deps, &meridians, None).is_ok());
}

#[test]
fn check_runtime_integrity_rejects_when_all_deps_at_zero() {
    let mut meridians = MeridianSystem::default();
    for id in [MeridianId::Lung, MeridianId::LargeIntestine] {
        meridians.get_mut(id).integrity = 0.0;
    }
    let deps = vec![MeridianId::Lung, MeridianId::LargeIntestine];
    assert_eq!(
        check_meridian_runtime_integrity(&deps, &meridians, None),
        Err(MeridianId::Lung)
    );
}

// --- MeridianSeveredEvent: 7 类来源 + 写入 component (10 tests) ---

fn run_event_through_system(events: Vec<MeridianSeveredEvent>) -> MeridianSeveredPermanent {
    let mut app = App::new();
    app.add_event::<MeridianSeveredEvent>();
    let entity = app
        .world_mut()
        .spawn((
            MeridianSeveredPermanent::default(),
            MeridianSystem::default(),
        ))
        .id();
    let events = events
        .into_iter()
        .map(|e| MeridianSeveredEvent { entity, ..e })
        .collect::<Vec<_>>();
    for ev in events {
        app.world_mut().send_event(ev);
    }
    app.add_systems(valence::prelude::Update, apply_severed_event_system);
    app.update();
    app.world()
        .entity(entity)
        .get::<MeridianSeveredPermanent>()
        .expect("component remains")
        .clone()
}

fn make_event(meridian: MeridianId, source: SeveredSource, tick: u64) -> MeridianSeveredEvent {
    MeridianSeveredEvent {
        entity: Entity::PLACEHOLDER,
        meridian_id: meridian,
        source,
        at_tick: tick,
    }
}

#[test]
fn event_voluntary_sever_writes_component() {
    let p = run_event_through_system(vec![make_event(
        MeridianId::Du,
        SeveredSource::VoluntarySever,
        10,
    )]);
    assert!(p.is_severed(MeridianId::Du));
    assert_eq!(
        p.record_for(MeridianId::Du).unwrap().source,
        SeveredSource::VoluntarySever
    );
}

#[test]
fn event_backfire_overload_writes_component() {
    let p = run_event_through_system(vec![make_event(
        MeridianId::Heart,
        SeveredSource::BackfireOverload,
        20,
    )]);
    assert!(p.is_severed(MeridianId::Heart));
}

#[test]
fn event_overload_tear_writes_component() {
    let p = run_event_through_system(vec![make_event(
        MeridianId::LargeIntestine,
        SeveredSource::OverloadTear,
        30,
    )]);
    assert!(p.is_severed(MeridianId::LargeIntestine));
}

#[test]
fn event_combat_wound_writes_component() {
    let p = run_event_through_system(vec![make_event(
        MeridianId::Bladder,
        SeveredSource::CombatWound,
        40,
    )]);
    assert!(p.is_severed(MeridianId::Bladder));
}

#[test]
fn event_tribulation_fail_writes_component() {
    let p = run_event_through_system(vec![make_event(
        MeridianId::Ren,
        SeveredSource::TribulationFail,
        50,
    )]);
    assert!(p.is_severed(MeridianId::Ren));
}

#[test]
fn event_dugu_distortion_writes_component() {
    let p = run_event_through_system(vec![make_event(
        MeridianId::Liver,
        SeveredSource::DuguDistortion,
        60,
    )]);
    assert!(p.is_severed(MeridianId::Liver));
}

#[test]
fn event_other_writes_component() {
    let p = run_event_through_system(vec![make_event(
        MeridianId::Chong,
        SeveredSource::Other("test-source".to_string()),
        70,
    )]);
    assert!(p.is_severed(MeridianId::Chong));
}

#[test]
fn event_clamps_meridian_integrity_to_zero() {
    // SEVERED event 要把 Meridian.integrity 钳到 0 + opened 标 false
    let mut app = App::new();
    app.add_event::<MeridianSeveredEvent>();
    let mut ms = MeridianSystem::default();
    ms.get_mut(MeridianId::Lung).integrity = 1.0;
    ms.get_mut(MeridianId::Lung).opened = true;
    let entity = app
        .world_mut()
        .spawn((MeridianSeveredPermanent::default(), ms))
        .id();
    app.world_mut().send_event(MeridianSeveredEvent {
        entity,
        meridian_id: MeridianId::Lung,
        source: SeveredSource::CombatWound,
        at_tick: 1,
    });
    app.add_systems(valence::prelude::Update, apply_severed_event_system);
    app.update();
    let ms = app.world().entity(entity).get::<MeridianSystem>().unwrap();
    assert_eq!(ms.get(MeridianId::Lung).integrity, 0.0);
    assert!(!ms.get(MeridianId::Lung).opened);
}

#[test]
fn event_drops_when_component_missing() {
    // 无 MeridianSeveredPermanent 的 entity 不应 panic，event 静默丢弃
    let mut app = App::new();
    app.add_event::<MeridianSeveredEvent>();
    let entity = app.world_mut().spawn(()).id();
    app.world_mut().send_event(MeridianSeveredEvent {
        entity,
        meridian_id: MeridianId::Lung,
        source: SeveredSource::CombatWound,
        at_tick: 1,
    });
    app.add_systems(valence::prelude::Update, apply_severed_event_system);
    app.update();
    // 没 panic 即通过
}

#[test]
fn event_multiple_in_one_tick_writes_all_unique() {
    let p = run_event_through_system(vec![
        make_event(MeridianId::Lung, SeveredSource::CombatWound, 100),
        make_event(MeridianId::Heart, SeveredSource::CombatWound, 100),
        make_event(MeridianId::Du, SeveredSource::TribulationFail, 100),
        // 重复同条经脉应保留首次
        make_event(MeridianId::Lung, SeveredSource::TribulationFail, 999),
    ]);
    assert_eq!(p.severed_count(), 3);
    assert_eq!(
        p.record_for(MeridianId::Lung).unwrap().source,
        SeveredSource::CombatWound,
        "首次 CombatWound 来源被保留"
    );
    assert_eq!(p.record_for(MeridianId::Lung).unwrap().at_tick, 100);
}

// --- 持久化 (跨 restart serde 完整 + 组合状态) (6 tests) ---

#[test]
fn serde_round_trip_with_seven_source_variants() {
    let mut p = MeridianSeveredPermanent::default();
    let pairs: &[(MeridianId, SeveredSource)] = &[
        (MeridianId::Lung, SeveredSource::VoluntarySever),
        (MeridianId::LargeIntestine, SeveredSource::BackfireOverload),
        (MeridianId::Heart, SeveredSource::OverloadTear),
        (MeridianId::SmallIntestine, SeveredSource::CombatWound),
        (MeridianId::Du, SeveredSource::TribulationFail),
        (MeridianId::Liver, SeveredSource::DuguDistortion),
        (
            MeridianId::Chong,
            SeveredSource::Other("ancient-curse".to_string()),
        ),
    ];
    for (i, (m, s)) in pairs.iter().enumerate() {
        p.insert(*m, s.clone(), i as u64 * 100);
    }
    let s = to_string(&p).expect("serialize");
    let back: MeridianSeveredPermanent = from_str(&s).expect("deserialize");
    assert_eq!(back, p);
    for (m, s) in pairs {
        assert_eq!(back.record_for(*m).unwrap().source, *s);
    }
}

#[test]
fn serde_preserves_dead_meridians_subset() {
    let mut p = MeridianSeveredPermanent::default();
    p.insert(MeridianId::Lung, SeveredSource::CombatWound, 1);
    p.dead_meridians.insert(MeridianId::Lung.channel_id());
    let back: MeridianSeveredPermanent = from_str(&to_string(&p).unwrap()).unwrap();
    assert!(back.is_dead(MeridianId::Lung));
    assert!(back.is_severed(MeridianId::Lung));
}

#[test]
fn serde_default_round_trip_is_empty() {
    let p = MeridianSeveredPermanent::default();
    let back: MeridianSeveredPermanent = from_str(&to_string(&p).unwrap()).unwrap();
    assert_eq!(back.severed_count(), 0);
}

#[test]
fn cross_lifecycle_reset_via_terminate_then_default() {
    // 模拟跨周目：终结时 reset，下一角色 component default 空
    let mut p = MeridianSeveredPermanent::default();
    p.insert(MeridianId::Lung, SeveredSource::TribulationFail, 100);
    p.dead_meridians.insert(MeridianId::Lung.channel_id());
    p.reset();
    assert!(!p.is_severed(MeridianId::Lung));
    let new_char = MeridianSeveredPermanent::default();
    assert_eq!(new_char.severed_count(), 0);
}

// ─────────────────────────────────────────────────────────────────────────
// plan-race-system-v1 P5/PR-6a —— 休眠登记（RaceChange 迁移旁路）。
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn dormant_default_is_empty() {
    let p = MeridianSeveredPermanent::default();
    assert_eq!(p.dormant_count(), 0);
    assert!(!p.is_dormant(MeridianId::Lung));
    assert!(p.dormant(MeridianId::Lung).is_none());
}

#[test]
fn dormant_register_then_query() {
    let mut p = MeridianSeveredPermanent::default();
    let mut m = Meridian::from_meridian_id(MeridianId::Lung);
    m.opened = true;
    m.integrity = 0.73;
    m.flow_rate = 5.0;
    let replaced = p.register_dormant(m.clone());
    assert!(replaced.is_none(), "首次登记不应有被替换的旧值");
    assert!(p.is_dormant(MeridianId::Lung));
    assert_eq!(p.dormant_count(), 1);
    let stored = p
        .dormant(MeridianId::Lung)
        .expect("dormant record should exist");
    assert!(stored.opened);
    assert_eq!(stored.integrity, 0.73);
    assert_eq!(stored.flow_rate, 5.0);
}

#[test]
fn dormant_register_duplicate_id_overwrites_and_returns_old() {
    let mut p = MeridianSeveredPermanent::default();
    let mut first = Meridian::from_meridian_id(MeridianId::Lung);
    first.integrity = 0.1;
    p.register_dormant(first.clone());

    let mut second = Meridian::from_meridian_id(MeridianId::Lung);
    second.integrity = 0.9;
    let replaced = p.register_dormant(second.clone());

    assert_eq!(replaced, Some(first), "覆盖登记应返回被替换的旧值");
    assert_eq!(p.dormant(MeridianId::Lung).unwrap().integrity, 0.9);
    assert_eq!(p.dormant_count(), 1, "覆盖同 id 不应产生第二条记录");
}

#[test]
fn dormant_take_removes_and_returns_full_state() {
    let mut p = MeridianSeveredPermanent::default();
    let mut m = Meridian::from_meridian_id(MeridianId::Heart);
    m.opened = true;
    m.rate_tier = 4;
    m.capacity_tier = 2;
    p.register_dormant(m.clone());

    let taken = p.take_dormant(MeridianId::Heart);
    assert_eq!(taken, Some(m), "take_dormant 必须原样返回完整状态");
    assert!(!p.is_dormant(MeridianId::Heart), "取出后应从休眠表移除");
    assert_eq!(p.dormant_count(), 0);
}

#[test]
fn dormant_take_missing_id_returns_none_without_panic() {
    let mut p = MeridianSeveredPermanent::default();
    assert_eq!(p.take_dormant(MeridianId::Lung), None);
}

#[test]
fn dormant_multiple_independent_entries_accumulate() {
    let mut p = MeridianSeveredPermanent::default();
    p.register_dormant(Meridian::from_meridian_id(MeridianId::Lung));
    p.register_dormant(Meridian::from_meridian_id(MeridianId::Heart));
    p.register_dormant(Meridian::from_meridian_id(MeridianId::Du));
    assert_eq!(p.dormant_count(), 3);
    assert!(p.is_dormant(MeridianId::Lung));
    assert!(p.is_dormant(MeridianId::Heart));
    assert!(p.is_dormant(MeridianId::Du));
}

#[test]
fn dormant_is_orthogonal_to_severed_and_dead() {
    // 休眠 ≠ SEVERED ≠ dead —— 三者互不干扰，同一 channel id 理论上可能同时
    // 出现在不同集合里（如已 SEVERED 的经脉恰好也是当前形态不含的经脉）。
    let mut p = MeridianSeveredPermanent::default();
    p.insert(MeridianId::Lung, SeveredSource::CombatWound, 10);
    p.register_dormant(Meridian::from_meridian_id(MeridianId::Lung));
    assert!(p.is_severed(MeridianId::Lung));
    assert!(p.is_dormant(MeridianId::Lung));
    assert!(!p.is_dead(MeridianId::Lung));
}

#[test]
fn dormant_serde_round_trip_preserves_full_meridian_state() {
    let mut p = MeridianSeveredPermanent::default();
    let mut m = Meridian::from_meridian_id(MeridianId::Kidney);
    m.opened = true;
    m.integrity = 0.42;
    m.cracks
        .push(bong_server::cultivation::components::MeridianCrack {
            severity: 0.3,
            healing_progress: 0.1,
            cause: CrackCause::Overload,
            created_at: 77,
        });
    p.register_dormant(m.clone());

    let s = serde_json::to_string(&p).expect("serialize");
    let back: MeridianSeveredPermanent = serde_json::from_str(&s).expect("deserialize");
    assert_eq!(back, p);
    assert_eq!(back.dormant(MeridianId::Kidney), Some(&m));
}

#[test]
fn dormant_reset_clears_dormant_registry_for_cross_lifecycle() {
    let mut p = MeridianSeveredPermanent::default();
    p.register_dormant(Meridian::from_meridian_id(MeridianId::Lung));
    p.reset();
    assert_eq!(
        p.dormant_count(),
        0,
        "跨周目重置必须清空休眠登记——新角色不应继承旧角色的休眠经脉状态"
    );
}

#[test]
fn dormant_default_round_trip_via_missing_field_migration() {
    // 旧存档（无 dormant_meridians 字段）反序列化必须落空 HashMap，不报错
    // （`#[serde(default)]` 迁移契约）。
    let json = r#"{"severed_meridians":[],"severed_at":{},"dead_meridians":[]}"#;
    let back: MeridianSeveredPermanent =
        serde_json::from_str(json).expect("legacy save without dormant_meridians must parse");
    assert_eq!(back.dormant_count(), 0);
}

#[test]
fn enforce_severed_state_clamps_integrity_and_opened() {
    let mut ms = MeridianSystem::default();
    let m = ms.get_mut(MeridianId::Du);
    m.integrity = 0.7;
    m.opened = true;
    m.throughput_current = 5.0;
    let did = enforce_severed_state(&mut ms, MeridianId::Du);
    assert!(did);
    let m = ms.get(MeridianId::Du);
    assert_eq!(m.integrity, 0.0);
    assert!(!m.opened);
    assert_eq!(m.throughput_current, 0.0);
}

#[test]
fn enforce_severed_state_idempotent() {
    let mut ms = MeridianSystem::default();
    let m = ms.get_mut(MeridianId::Du);
    m.integrity = 0.0;
    m.opened = false;
    let did = enforce_severed_state(&mut ms, MeridianId::Du);
    assert!(!did, "已 SEVERED 状态再调用返回 false");
}

// --- AcupointRepair: 成功 / 失败升级死脉 / 边界 (8 tests) ---

#[test]
fn repair_not_severed_returns_not_severed() {
    let mut p = MeridianSeveredPermanent::default();
    let outcome = try_acupoint_repair(&mut p, MeridianId::Lung, 0.0, 0.5);
    assert_eq!(outcome, AcupointRepairOutcome::NotSevered);
}

#[test]
fn repair_success_removes_severed() {
    let mut p = MeridianSeveredPermanent::default();
    p.insert(MeridianId::Lung, SeveredSource::CombatWound, 100);
    // success_roll < success_threshold → 成功
    let outcome = try_acupoint_repair(&mut p, MeridianId::Lung, 0.1, 0.7);
    assert_eq!(outcome, AcupointRepairOutcome::Restored);
    assert!(!p.is_severed(MeridianId::Lung));
    assert!(p.record_for(MeridianId::Lung).is_none());
}

#[test]
fn repair_failure_marks_dead() {
    let mut p = MeridianSeveredPermanent::default();
    p.insert(MeridianId::Heart, SeveredSource::TribulationFail, 50);
    let outcome = try_acupoint_repair(&mut p, MeridianId::Heart, 0.9, 0.3);
    assert_eq!(outcome, AcupointRepairOutcome::Failed);
    assert!(p.is_dead(MeridianId::Heart));
    assert!(p.is_severed(MeridianId::Heart), "死脉仍在 SEVERED 集合");
}

#[test]
fn repair_already_dead_rejects() {
    let mut p = MeridianSeveredPermanent::default();
    p.insert(MeridianId::Lung, SeveredSource::CombatWound, 100);
    p.dead_meridians.insert(MeridianId::Lung.channel_id());
    let outcome = try_acupoint_repair(&mut p, MeridianId::Lung, 0.0, 1.0);
    assert_eq!(outcome, AcupointRepairOutcome::AlreadyDead);
}

#[test]
fn repair_threshold_zero_always_fails() {
    let mut p = MeridianSeveredPermanent::default();
    p.insert(MeridianId::Lung, SeveredSource::CombatWound, 1);
    let outcome = try_acupoint_repair(&mut p, MeridianId::Lung, 0.0, 0.0);
    assert_eq!(outcome, AcupointRepairOutcome::Failed);
}

#[test]
fn repair_threshold_one_always_succeeds_for_zero_roll() {
    let mut p = MeridianSeveredPermanent::default();
    p.insert(MeridianId::Lung, SeveredSource::CombatWound, 1);
    let outcome = try_acupoint_repair(&mut p, MeridianId::Lung, 0.0, 1.0);
    assert_eq!(outcome, AcupointRepairOutcome::Restored);
}

#[test]
fn repair_failure_does_not_remove_other_severed() {
    let mut p = MeridianSeveredPermanent::default();
    p.insert(MeridianId::Lung, SeveredSource::CombatWound, 100);
    p.insert(MeridianId::Heart, SeveredSource::CombatWound, 110);
    let _ = try_acupoint_repair(&mut p, MeridianId::Lung, 0.99, 0.5);
    assert!(
        p.is_severed(MeridianId::Heart),
        "Heart 不应受 Lung repair 影响"
    );
}

#[test]
fn repair_success_then_re_sever_starts_clean() {
    // 成功修复后，再次 SEVERED 应记录新时戳与新来源
    let mut p = MeridianSeveredPermanent::default();
    p.insert(MeridianId::Lung, SeveredSource::CombatWound, 100);
    let _ = try_acupoint_repair(&mut p, MeridianId::Lung, 0.1, 0.9);
    let _ = p.insert(MeridianId::Lung, SeveredSource::TribulationFail, 500);
    let r = p.record_for(MeridianId::Lung).unwrap();
    assert_eq!(r.at_tick, 500);
    assert_eq!(r.source, SeveredSource::TribulationFail);
}

// --- SkillMeridianDependencies（未声明 / 显式空 / 重复拒绝 / 查询）(5 tests) ---

#[test]
fn dependencies_default_empty_lookup_returns_empty_slice() {
    let table = SkillMeridianDependencies::default();
    assert!(table.lookup("zhenmai.parry").is_empty());
    assert!(!table.is_declared("zhenmai.parry"));
}

#[test]
fn dependencies_declare_and_lookup() {
    let mut table = SkillMeridianDependencies::default();
    table.declare(
        "zhenmai.parry",
        vec![MeridianId::Lung, MeridianId::LargeIntestine],
    );
    assert_eq!(
        table.lookup("zhenmai.parry"),
        &[MeridianId::Lung, MeridianId::LargeIntestine]
    );
    assert!(table.is_declared("zhenmai.parry"));
}

#[test]
#[should_panic(expected = "duplicate meridian dependency declaration for skill: baomai.beng_quan")]
fn dependencies_reject_duplicate_declaration() {
    let mut table = SkillMeridianDependencies::default();
    table.declare("baomai.beng_quan", vec![MeridianId::LargeIntestine]);
    table.declare(
        "baomai.beng_quan",
        vec![
            MeridianId::LargeIntestine,
            MeridianId::SmallIntestine,
            MeridianId::TripleEnergizer,
        ],
    );
}

#[test]
fn dependencies_distinguish_explicit_empty_from_undeclared() {
    let mut table = SkillMeridianDependencies::default();
    table.declare("movement.dash", Vec::new());

    assert!(table.lookup("movement.dash").is_empty());
    assert!(
        table.is_declared("movement.dash"),
        "an explicitly empty dependency declaration must remain distinguishable from absence"
    );
    assert!(table.lookup("missing.skill").is_empty());
    assert!(
        !table.is_declared("missing.skill"),
        "an undeclared skill must not be admitted as explicitly dependency-free"
    );
}

#[test]
fn dependencies_check_via_check_meridian_dependencies() {
    // 端到端：声明 + check_meridian_dependencies 联合用法
    let mut table = SkillMeridianDependencies::default();
    table.declare(
        "zhenmai.parry",
        vec![MeridianId::Lung, MeridianId::LargeIntestine],
    );
    let mut p = MeridianSeveredPermanent::default();
    p.insert(MeridianId::Lung, SeveredSource::CombatWound, 1);
    let deps = table.lookup("zhenmai.parry").to_vec();
    assert_eq!(
        check_meridian_dependencies(&deps, Some(&p)),
        Err(MeridianId::Lung)
    );
}

// --- severed_source_from_crack: 7 CrackCause → SeveredSource 映射 (7 tests) ---

#[test]
fn severed_source_from_attack_is_combat_wound() {
    assert_eq!(
        severed_source_from_crack(CrackCause::Attack),
        SeveredSource::CombatWound
    );
}

#[test]
fn severed_source_from_overload_is_backfire_overload() {
    assert_eq!(
        severed_source_from_crack(CrackCause::Overload),
        SeveredSource::BackfireOverload
    );
}

#[test]
fn severed_source_from_backfire_is_backfire_overload() {
    assert_eq!(
        severed_source_from_crack(CrackCause::Backfire),
        SeveredSource::BackfireOverload
    );
}

#[test]
fn severed_source_from_forge_failure_is_other() {
    assert_eq!(
        severed_source_from_crack(CrackCause::ForgeFailure),
        SeveredSource::Other("forge_failure".to_string())
    );
}

#[test]
fn severed_source_from_voluntary_sever_is_voluntary() {
    assert_eq!(
        severed_source_from_crack(CrackCause::VoluntarySever),
        SeveredSource::VoluntarySever
    );
}

#[test]
fn severed_source_from_tribulation_fail_is_tribulation() {
    assert_eq!(
        severed_source_from_crack(CrackCause::TribulationFail),
        SeveredSource::TribulationFail
    );
}

#[test]
fn severed_source_from_dugu_distortion_is_dugu() {
    assert_eq!(
        severed_source_from_crack(CrackCause::DuguDistortion),
        SeveredSource::DuguDistortion
    );
}

// --- meridian_severed_detection_tick: 端到端 detection + apply 链路 (6 tests) ---

fn run_detection_chain(
    meridian: MeridianId,
    integrity: f64,
    cracks: Vec<CrackCause>,
    tick: u64,
) -> MeridianSeveredPermanent {
    use bong_server::cultivation::components::MeridianCrack;
    use bong_server::cultivation::tick::CultivationClock;

    let mut app = App::new();
    app.add_event::<MeridianSeveredEvent>();
    app.insert_resource(CultivationClock { tick });

    let mut ms = MeridianSystem::default();
    let m = ms.get_mut(meridian);
    m.integrity = integrity;
    m.opened = integrity > f64::EPSILON;
    for cause in cracks {
        m.cracks.push(MeridianCrack {
            severity: 0.5,
            healing_progress: 0.0,
            cause,
            created_at: tick,
        });
    }
    let entity = app
        .world_mut()
        .spawn((ms, MeridianSeveredPermanent::default()))
        .id();

    app.add_systems(
        valence::prelude::Update,
        (
            meridian_severed_detection_tick,
            apply_severed_event_system.after(meridian_severed_detection_tick),
        ),
    );
    app.update();

    app.world()
        .entity(entity)
        .get::<MeridianSeveredPermanent>()
        .expect("component exists")
        .clone()
}

#[test]
fn detection_emits_combat_wound_for_attack_crack_when_integrity_zero() {
    let p = run_detection_chain(MeridianId::Lung, 0.0, vec![CrackCause::Attack], 100);
    assert!(p.is_severed(MeridianId::Lung));
    assert_eq!(
        p.record_for(MeridianId::Lung).unwrap().source,
        SeveredSource::CombatWound
    );
    assert_eq!(p.record_for(MeridianId::Lung).unwrap().at_tick, 100);
}

#[test]
fn detection_emits_backfire_overload_for_overload_crack() {
    let p = run_detection_chain(MeridianId::Heart, 0.0, vec![CrackCause::Overload], 50);
    assert_eq!(
        p.record_for(MeridianId::Heart).unwrap().source,
        SeveredSource::BackfireOverload
    );
}

#[test]
fn detection_uses_latest_crack_cause_when_multiple_present() {
    // Attack first, then Overload (later tick) → SEVERED 应取 Overload→BackfireOverload
    use bong_server::cultivation::components::MeridianCrack;
    use bong_server::cultivation::tick::CultivationClock;
    let mut app = App::new();
    app.add_event::<MeridianSeveredEvent>();
    app.insert_resource(CultivationClock { tick: 200 });
    let mut ms = MeridianSystem::default();
    let m = ms.get_mut(MeridianId::Du);
    m.integrity = 0.0;
    m.opened = false;
    m.cracks.push(MeridianCrack {
        severity: 0.3,
        healing_progress: 0.0,
        cause: CrackCause::Attack,
        created_at: 100,
    });
    m.cracks.push(MeridianCrack {
        severity: 0.7,
        healing_progress: 0.0,
        cause: CrackCause::Overload,
        created_at: 150,
    });
    let entity = app
        .world_mut()
        .spawn((ms, MeridianSeveredPermanent::default()))
        .id();
    app.add_systems(
        valence::prelude::Update,
        (
            meridian_severed_detection_tick,
            apply_severed_event_system.after(meridian_severed_detection_tick),
        ),
    );
    app.update();
    let p = app
        .world()
        .entity(entity)
        .get::<MeridianSeveredPermanent>()
        .unwrap();
    assert_eq!(
        p.record_for(MeridianId::Du).unwrap().source,
        SeveredSource::BackfireOverload,
        "最新 crack(Overload @ 150) 决定来源，而非更早的 Attack"
    );
}

#[test]
fn detection_skips_when_integrity_above_epsilon() {
    let p = run_detection_chain(MeridianId::Lung, 0.5, vec![CrackCause::Attack], 100);
    assert!(
        !p.is_severed(MeridianId::Lung),
        "integrity > ε 不应触发 SEVERED"
    );
}

#[test]
fn detection_skips_when_no_cracks() {
    // integrity = 0 但无 cracks（出生 default 或被 close_meridian 直接置零）
    // → detection 不主动 SEVERED；调用方应显式 emit event
    let p = run_detection_chain(MeridianId::Lung, 0.0, vec![], 100);
    assert!(!p.is_severed(MeridianId::Lung));
}

#[test]
fn detection_skips_already_severed_no_double_record() {
    use bong_server::cultivation::components::MeridianCrack;
    use bong_server::cultivation::tick::CultivationClock;
    let mut app = App::new();
    app.add_event::<MeridianSeveredEvent>();
    app.insert_resource(CultivationClock { tick: 500 });
    let mut ms = MeridianSystem::default();
    let m = ms.get_mut(MeridianId::Lung);
    m.integrity = 0.0;
    m.opened = false;
    m.cracks.push(MeridianCrack {
        severity: 0.5,
        healing_progress: 0.0,
        cause: CrackCause::Attack,
        created_at: 500,
    });
    let mut perm = MeridianSeveredPermanent::default();
    perm.insert(MeridianId::Lung, SeveredSource::TribulationFail, 100);
    let entity = app.world_mut().spawn((ms, perm)).id();
    app.add_systems(
        valence::prelude::Update,
        (
            meridian_severed_detection_tick,
            apply_severed_event_system.after(meridian_severed_detection_tick),
        ),
    );
    app.update();
    let p = app
        .world()
        .entity(entity)
        .get::<MeridianSeveredPermanent>()
        .unwrap();
    // 首次记录（TribulationFail @ 100）保留，detection 看到已 SEVERED 直接跳过
    let r = p.record_for(MeridianId::Lung).unwrap();
    assert_eq!(r.source, SeveredSource::TribulationFail);
    assert_eq!(r.at_tick, 100);
}
