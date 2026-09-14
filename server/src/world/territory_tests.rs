use super::*;

fn assert_close(actual: f64, expected: f64, msg: &str) {
    assert!(
        (actual - expected).abs() < 1e-9,
        "{msg}: 期望 {expected:.12}, 实际 {actual:.12}"
    );
}

// ── 1. 停留累积 ──────────────────────────────────────────────────────────

#[test]
fn tiandao_attention_accelerates() {
    let no_attention = compute_influence_delta(0, false, false, 1.0, 0.0, 0);
    let max_attention = compute_influence_delta(0, false, false, 1.0, ATTENTION_MAX, 0);
    // attention_level=100 → 乘 (1 + 0.5*1.0) = 1.5
    assert_close(max_attention, no_attention * 1.5, "满天道注意力应加速 1.5x");
}

#[test]
fn throttle_first_eval_at_tick_zero() {
    // last_eval=0, now_tick=0 → 差=0 < TERRITORY_EVAL_INTERVAL_TICKS → 不 eval
    // 注意：last_eval=0 对「全新玩家」在 territory_tick 里走 last_eval=0 路径时
    // 用 `last_eval > 0` 守卫，所以此处仅测节流函数本身行为：
    // 同 tick 不满足间隔，返回 false。
    assert!(
            !should_eval_territory(0, 0),
            "now=0 last=0 差=0 < TERRITORY_EVAL_INTERVAL_TICKS({TERRITORY_EVAL_INTERVAL_TICKS}) 应返回 false"
        );
}

#[test]
fn throttle_before_interval_returns_false() {
    // 距上次 eval 未满 60s（1199 ticks < 1200）→ 不 eval
    let last = 1000u64;
    let now = last + TERRITORY_EVAL_INTERVAL_TICKS - 1;
    assert!(
        !should_eval_territory(now, last),
        "距上次 eval {TERRITORY_EVAL_INTERVAL_TICKS}-1 ticks 不应触发 eval"
    );
}

#[test]
fn throttle_exactly_at_interval_returns_true() {
    // 距上次 eval 恰好 60s（1200 ticks）→ 应 eval
    let last = 1000u64;
    let now = last + TERRITORY_EVAL_INTERVAL_TICKS;
    assert!(
            should_eval_territory(now, last),
            "距上次 eval 恰好 TERRITORY_EVAL_INTERVAL_TICKS({TERRITORY_EVAL_INTERVAL_TICKS}) ticks 应触发 eval"
        );
}

#[test]
fn throttle_over_interval_returns_true() {
    // 距上次 eval 超过 60s → 应 eval
    let last = 0u64;
    let now = TERRITORY_EVAL_INTERVAL_TICKS * 3;
    assert!(
        should_eval_territory(now, last),
        "超过 TERRITORY_EVAL_INTERVAL_TICKS 应触发 eval"
    );
}

#[test]
fn long_absence_decays_faster() {
    // 超过 24h（absence_ticks > TICKS_PER_DAY）→ 减 DECAY_PER_EVAL × DECAY_LONG_ABSENCE_MULT
    let result = compute_decay(50.0, TICKS_PER_DAY + 1);
    assert_close(
        result,
        50.0 - DECAY_PER_EVAL * DECAY_LONG_ABSENCE_MULT,
        ">24h 未回应快速衰减 ×DECAY_LONG_ABSENCE_MULT({DECAY_LONG_ABSENCE_MULT})",
    );
}

#[test]
fn decay_boundary_exactly_ticks_per_day() {
    // absence_ticks == TICKS_PER_DAY（恰好 24h，非超过）→ 用普通衰减
    let result = compute_decay(50.0, TICKS_PER_DAY);
    assert_close(
        result,
        50.0 - DECAY_PER_EVAL,
        "absence_ticks=TICKS_PER_DAY 恰好不超过应用普通衰减",
    );
}

#[test]
fn territory_tick_practice_source_emitted_when_recently_practicing() {
    use crate::cultivation::tick::{CultivationClock, CultivationSessionPracticeAccumulator};
    use crate::world::zone::{Zone, ZoneRegistry};
    use valence::prelude::{bevy_ecs, DVec3, Update};
    use valence::testing::create_mock_client;

    let mut app = App::new();
    app.add_event::<InfluenceChangedEvent>();
    app.add_event::<DominanceChangedEvent>();
    app.init_resource::<ZoneInfluenceMap>();

    let zone = Zone {
        name: "qingyun".to_string(),
        dimension: crate::world::dimension::DimensionKind::Overworld,
        bounds: (DVec3::new(-64.0, 60.0, -64.0), DVec3::new(64.0, 80.0, 64.0)),
        spirit_qi: 1.0,
        danger_level: 0,
        active_events: vec![],
        patrol_anchors: vec![],
        blocked_tiles: vec![],
        qi_equilibrium: 0.0,
        qi_inflow_per_min: 0.0,
    };
    app.insert_resource(ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone],
    });

    // 构造修炼累积器，记录玩家最近修炼
    let mut accumulator = CultivationSessionPracticeAccumulator::default();
    let eval_tick = TERRITORY_EVAL_INTERVAL_TICKS;
    // 预分配 entity id 需先 spawn 再插入，用占位 id 填写 accumulator
    // 通过 note_practice_tick_for_tests 在 eval_tick 记录
    app.insert_resource(CultivationClock { tick: eval_tick });
    app.add_systems(Update, territory_tick);

    let (mut client_bundle, _helper) = create_mock_client("Cultivator");
    client_bundle.player.position = valence::prelude::Position::new([0.0, 66.0, 0.0]);
    let player = app
        .world_mut()
        .spawn((
            client_bundle,
            crate::cultivation::components::Cultivation {
                realm: crate::cultivation::components::Realm::Induce,
                qi_current: 80.0,
                qi_max: 100.0,
                ..crate::cultivation::components::Cultivation::default()
            },
        ))
        .id();

    // 记录修炼 tick（在 eval_tick 时刻修炼）
    accumulator.note_practice_tick_for_tests(player, eval_tick);
    app.insert_resource(accumulator);

    app.update();

    // 断言事件 source=Practice
    let emitted: Vec<InfluenceChangedEvent> = app
        .world_mut()
        .resource_mut::<bevy_ecs::event::Events<InfluenceChangedEvent>>()
        .drain()
        .collect();
    let ev = emitted
        .iter()
        .find(|e| e.player_entity == player && e.zone_name == "qingyun")
        .expect("应有 qingyun zone 的 InfluenceChangedEvent");
    assert_eq!(
        ev.source,
        InfluenceSource::Practice,
        "正在修炼时 source 应为 Practice，实际={:?}",
        ev.source
    );
    assert!(
        ev.delta > 0.0,
        "修炼时 delta 应 > 0（修炼乘子 × 停留量），实际={}",
        ev.delta
    );
    let _ = player;
}
