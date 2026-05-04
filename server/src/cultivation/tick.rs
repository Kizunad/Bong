//! QiRegenTick + ZoneQiDrainTick（plan §2 QiRegenTick / ZoneQiDrainTick）。
//!
//! 两者合并到一个 system 里执行以天然保证零和：玩家每 tick 吸纳的 qi
//! 必然等量从 zone.spirit_qi 扣除（按 `QI_PER_ZONE_UNIT` 换算）。符合
//! worldview §一"灵气零和守恒"公理。
//!
//! P1 简化：无「静坐/行动」区分，全部按被动小系数回；静坐/打坐在 P1 末
//! 加客户端指令时再接入。

use std::collections::HashMap;

use valence::prelude::{
    bevy_ecs, Despawned, Entity, Events, Position, Query, ResMut, Resource, With, Without,
};

use crate::combat::components::StatusEffects;
use crate::combat::events::StatusEffectKind;
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::events::EVENT_REALM_COLLAPSE;
use crate::world::zone::ZoneRegistry;

use super::color::{
    CultivationSessionPracticeEvent, CULTIVATION_SESSION_PRACTICE_TICKS_PER_MINUTE,
};
use super::components::{ColorKind, Cultivation, MeridianSystem, QiColor, Realm};
use super::lifespan::LifespanComponent;

/// 全局 tick 计数器 — 用于标记 last_qi_zero_at 等时间戳。
#[derive(Debug, Default, Resource)]
pub struct CultivationClock {
    pub tick: u64,
}

/// 修炼 session 实际引气 tick 累计器。只有发生真实 qi gain 的 tick 才计入，
/// 避免玩家在全局分钟边界短暂在线也拿到整分钟 PracticeLog 进料。
#[derive(Debug, Default, Resource)]
pub struct CultivationSessionPracticeAccumulator {
    ticks_by_entity: HashMap<Entity, u64>,
}

/// 每 tick 真元回复的归一化系数。
pub const QI_REGEN_COEF: f64 = 0.01;
/// 1.0 单位 zone concentration 可支撑多少 qi 吸纳 — 决定 zone 枯竭速度。
/// 数值越大 zone 越耐抽。
pub const QI_PER_ZONE_UNIT: f64 = 50.0;

/// 纯函数：给定 zone 浓度、rate、可用额度（qi_max - qi_current - qi_max_frozen 等）
/// 计算本 tick 的实际 gain 与 zone 浓度变化量（均为非负）。
pub fn compute_regen(zone_qi: f64, rate: f64, avg_integrity: f64, qi_room: f64) -> (f64, f64) {
    if zone_qi <= 0.0 || rate <= 0.0 || qi_room <= 0.0 {
        return (0.0, 0.0);
    }
    let raw_gain = zone_qi * rate * avg_integrity * QI_REGEN_COEF;
    // 池容量上限
    let capped_gain = raw_gain.min(qi_room);
    // 该次 gain 对应 zone 浓度扣减
    let drain = capped_gain / QI_PER_ZONE_UNIT;
    // 若扣减将 zone 拉到负值，再等比回退 gain
    if drain > zone_qi {
        let actual_drain = zone_qi;
        let actual_gain = actual_drain * QI_PER_ZONE_UNIT;
        (actual_gain, actual_drain)
    } else {
        (capped_gain, drain)
    }
}

/// QiRegenTick + ZoneQiDrainTick 合并实现。零和：玩家 qi 增量 = zone 浓度减量 × coef。
#[allow(clippy::type_complexity)]
pub fn qi_regen_and_zone_drain_tick(
    mut clock: ResMut<CultivationClock>,
    zone_registry: Option<ResMut<ZoneRegistry>>,
    mut practice_events: Option<ResMut<Events<CultivationSessionPracticeEvent>>>,
    mut practice_accumulator: Option<ResMut<CultivationSessionPracticeAccumulator>>,
    mut players: Query<(
        Entity,
        &Position,
        Option<&CurrentDimension>,
        &MeridianSystem,
        &mut Cultivation,
        Option<&QiColor>,
        Option<&LifespanComponent>,
        Option<&StatusEffects>,
    )>,
) {
    clock.tick = clock.tick.wrapping_add(1);

    let Some(mut zones) = zone_registry else {
        return;
    };

    for (entity, pos, current_dim, meridians, mut cultivation, qi_color, lifespan, statuses) in
        players.iter_mut()
    {
        // 通过 pos 找到 zone 的 name（不持可变借用）；entity 缺 CurrentDimension
        // 时按 Overworld 处理（NPC 暂未跨位面）。Player 在 spawn 时一定带
        // CurrentDimension（apply_spawn_defaults / restore_player_dimension）。
        let dim = current_dim.map(|c| c.0).unwrap_or(DimensionKind::Overworld);
        let Some(zone_name) = zones.find_zone(dim, pos.0).map(|z| z.name.clone()) else {
            continue;
        };
        let Some(zone) = zones.find_zone_mut(&zone_name) else {
            continue;
        };
        if zone
            .active_events
            .iter()
            .any(|event| event == EVENT_REALM_COLLAPSE)
        {
            zone.spirit_qi = 0.0;
            continue;
        }
        if zone.spirit_qi <= 0.0 {
            continue;
        }

        let rate = {
            let sum = meridians.sum_rate();
            if sum > 0.0 {
                sum
            } else {
                0.1 // Awaken 期的「基础吸纳」
            }
        };
        let avg_integrity = {
            let total: f64 = meridians.iter().map(|m| m.integrity).sum();
            let n = meridians.iter().count() as f64;
            if n > 0.0 {
                total / n
            } else {
                1.0
            }
        };
        let effective_max = cultivation.qi_max - cultivation.qi_max_frozen.unwrap_or(0.0);
        let qi_room = (effective_max - cultivation.qi_current).max(0.0);

        let wind_candle_multiplier = if lifespan.is_some_and(LifespanComponent::is_wind_candle)
            || statuses.is_some_and(has_frailty_status)
        {
            frailty_qi_recovery_multiplier_for_realm(cultivation.realm)
        } else {
            1.0
        };
        let humility_multiplier = statuses.map(humility_qi_recovery_multiplier).unwrap_or(1.0);
        let (gain, drain) = compute_regen(
            zone.spirit_qi,
            rate * wind_candle_multiplier * humility_multiplier,
            avg_integrity,
            qi_room,
        );
        if gain <= 0.0 {
            continue;
        }

        cultivation.qi_current += gain;
        zone.spirit_qi = (zone.spirit_qi - drain).max(0.0);

        if cultivation.qi_current > 0.0 {
            cultivation.last_qi_zero_at = None;
        }

        if let (Some(events), Some(accumulator)) = (
            practice_events.as_deref_mut(),
            practice_accumulator.as_deref_mut(),
        ) {
            accumulate_cultivation_session_practice_tick(
                accumulator,
                events,
                entity,
                qi_color
                    .map(|color| color.main)
                    .unwrap_or(ColorKind::Mellow),
            );
        }
    }
}

pub fn accumulate_cultivation_session_practice_tick(
    accumulator: &mut CultivationSessionPracticeAccumulator,
    events: &mut Events<CultivationSessionPracticeEvent>,
    entity: Entity,
    active_color: ColorKind,
) -> u64 {
    let ticks = accumulator.ticks_by_entity.entry(entity).or_default();
    *ticks = ticks.saturating_add(1);

    let minutes = *ticks / CULTIVATION_SESSION_PRACTICE_TICKS_PER_MINUTE;
    if minutes == 0 {
        return 0;
    }

    *ticks %= CULTIVATION_SESSION_PRACTICE_TICKS_PER_MINUTE;
    events.send(CultivationSessionPracticeEvent {
        entity,
        active_color,
        elapsed_ticks: minutes * CULTIVATION_SESSION_PRACTICE_TICKS_PER_MINUTE,
    });
    minutes
}

pub fn prune_cultivation_session_practice_accumulator(
    mut accumulator: ResMut<CultivationSessionPracticeAccumulator>,
    live_cultivators: Query<(), (With<Cultivation>, Without<Despawned>)>,
) {
    accumulator
        .ticks_by_entity
        .retain(|entity, _| live_cultivators.get(*entity).is_ok());
}

fn humility_qi_recovery_multiplier(status_effects: &StatusEffects) -> f64 {
    status_effects
        .active
        .iter()
        .filter(|effect| effect.kind == StatusEffectKind::Humility && effect.remaining_ticks > 0)
        .fold(1.0, |acc, effect| {
            acc * (1.0 - f64::from(effect.magnitude).clamp(0.0, 0.95))
        })
        .clamp(0.05, 1.0)
}

pub fn frailty_qi_recovery_multiplier_for_realm(realm: Realm) -> f64 {
    match realm {
        Realm::Awaken | Realm::Induce => 0.7,
        Realm::Condense => 0.6,
        Realm::Solidify => 0.5,
        Realm::Spirit => 0.4,
        Realm::Void => 0.3,
    }
}

fn has_frailty_status(status_effects: &StatusEffects) -> bool {
    status_effects
        .active
        .iter()
        .any(|effect| effect.kind == StatusEffectKind::Frailty && effect.remaining_ticks > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::color::{
        record_cultivation_session_practice_events, CultivationSessionPracticeEvent, PracticeLog,
        STYLE_PRACTICE_AMOUNT,
    };
    use crate::cultivation::components::MeridianId;
    use crate::cultivation::lifespan::{LifespanCapTable, LifespanComponent};
    use crate::world::zone::ZoneRegistry;
    use valence::prelude::{App, IntoSystemConfigs, Update};

    #[test]
    fn no_gain_in_dead_zone() {
        let (g, d) = compute_regen(0.0, 1.0, 1.0, 100.0);
        assert_eq!(g, 0.0);
        assert_eq!(d, 0.0);
    }

    #[test]
    fn gain_drains_zone_by_ratio() {
        let (g, d) = compute_regen(0.5, 1.0, 1.0, 100.0);
        assert!(g > 0.0);
        // gain / QI_PER_ZONE_UNIT == drain
        assert!((g - d * QI_PER_ZONE_UNIT).abs() < 1e-9);
    }

    #[test]
    fn qi_room_caps_gain() {
        let (g, d) = compute_regen(1.0, 100.0, 1.0, 0.5);
        assert!(g <= 0.5);
        // 即使被 qi_room 截断，drain 依然按 gain 换算
        assert!((g - d * QI_PER_ZONE_UNIT).abs() < 1e-9);
    }

    #[test]
    fn drain_clamped_to_zone_available() {
        // rate 巨大会把 drain 推到超过 zone_qi
        let zone_qi = 0.001;
        let (g, d) = compute_regen(zone_qi, 1e6, 1.0, 1e9);
        assert!(d <= zone_qi + 1e-12);
        assert!((g - d * QI_PER_ZONE_UNIT).abs() < 1e-6);
    }

    #[test]
    fn zero_sum_property() {
        // 多次 tick 后累积玩家 gain == 累积 zone drain × QI_PER_ZONE_UNIT
        let mut zone_qi = 0.5;
        let mut player_qi = 0.0;
        for _ in 0..50 {
            let room = 1e9_f64;
            let (g, d) = compute_regen(zone_qi, 1.0, 1.0, room);
            player_qi += g;
            zone_qi -= d;
        }
        let leaked = player_qi - (0.5 - zone_qi) * QI_PER_ZONE_UNIT;
        assert!(leaked.abs() < 1e-6);
    }

    #[test]
    fn integrity_scales_gain() {
        let (g_full, _) = compute_regen(0.5, 1.0, 1.0, 1e9);
        let (g_half, _) = compute_regen(0.5, 1.0, 0.5, 1e9);
        assert!((g_half - g_full * 0.5).abs() < 1e-9);
    }

    #[test]
    fn wind_candle_applies_realm_specific_qi_regen_penalty() {
        fn run_once(lifespan: LifespanComponent) -> f64 {
            let mut app = App::new();
            app.insert_resource(CultivationClock::default());
            app.insert_resource(ZoneRegistry::fallback());
            app.add_systems(Update, qi_regen_and_zone_drain_tick);

            let mut meridians = MeridianSystem::default();
            meridians.get_mut(MeridianId::Lung).opened = true;
            let entity = app
                .world_mut()
                .spawn((
                    Position::new([8.0, 66.0, 8.0]),
                    meridians,
                    Cultivation::default(),
                    lifespan,
                ))
                .id();

            app.update();

            app.world()
                .entity(entity)
                .get::<Cultivation>()
                .unwrap()
                .qi_current
        }

        let normal_qi = run_once(LifespanComponent::new(LifespanCapTable::MORTAL));
        let mut wind_candle_lifespan = LifespanComponent::new(LifespanCapTable::MORTAL);
        wind_candle_lifespan.years_lived = 73.0;
        let wind_candle_qi = run_once(wind_candle_lifespan);

        assert!(normal_qi > 0.0);
        assert!((wind_candle_qi - normal_qi * 0.7).abs() < 1e-6);
    }

    #[test]
    fn qi_regen_emits_session_practice_after_actual_regen_minute() {
        let mut app = App::new();
        app.insert_resource(CultivationClock {
            tick: CULTIVATION_SESSION_PRACTICE_TICKS_PER_MINUTE - 1,
        });
        app.insert_resource(CultivationSessionPracticeAccumulator::default());
        app.insert_resource(ZoneRegistry::fallback());
        app.add_event::<CultivationSessionPracticeEvent>();
        app.add_systems(Update, qi_regen_and_zone_drain_tick);
        app.add_systems(
            Update,
            record_cultivation_session_practice_events.after(qi_regen_and_zone_drain_tick),
        );

        let mut meridians = MeridianSystem::default();
        meridians.get_mut(MeridianId::Lung).opened = true;
        let entity = app
            .world_mut()
            .spawn((
                Position::new([8.0, 66.0, 8.0]),
                meridians,
                Cultivation::default(),
                QiColor {
                    main: ColorKind::Heavy,
                    ..Default::default()
                },
                PracticeLog::default(),
            ))
            .id();

        app.update();
        let log = app.world().entity(entity).get::<PracticeLog>().unwrap();
        assert_eq!(log.weights.get(&ColorKind::Heavy).copied(), None);

        for _ in 0..CULTIVATION_SESSION_PRACTICE_TICKS_PER_MINUTE - 1 {
            app.update();
        }

        let log = app.world().entity(entity).get::<PracticeLog>().unwrap();
        assert_eq!(
            log.weights.get(&ColorKind::Heavy).copied(),
            Some(STYLE_PRACTICE_AMOUNT)
        );
    }

    #[test]
    fn session_practice_accumulator_prunes_stale_entities() {
        let mut app = App::new();
        app.insert_resource(CultivationSessionPracticeAccumulator::default());
        app.add_systems(Update, prune_cultivation_session_practice_accumulator);

        let live_entity = app.world_mut().spawn(Cultivation::default()).id();
        let despawned_entity = app.world_mut().spawn(Despawned).id();
        let uncultivated_entity = app.world_mut().spawn_empty().id();
        let missing_entity = Entity::from_raw(99_999);

        {
            let mut accumulator = app
                .world_mut()
                .resource_mut::<CultivationSessionPracticeAccumulator>();
            accumulator.ticks_by_entity.insert(live_entity, 12);
            accumulator.ticks_by_entity.insert(despawned_entity, 24);
            accumulator.ticks_by_entity.insert(uncultivated_entity, 30);
            accumulator.ticks_by_entity.insert(missing_entity, 36);
        }

        app.update();

        let accumulator = app
            .world()
            .resource::<CultivationSessionPracticeAccumulator>();
        assert_eq!(accumulator.ticks_by_entity.get(&live_entity), Some(&12));
        assert!(!accumulator.ticks_by_entity.contains_key(&despawned_entity));
        assert!(!accumulator
            .ticks_by_entity
            .contains_key(&uncultivated_entity));
        assert!(!accumulator.ticks_by_entity.contains_key(&missing_entity));
    }

    #[test]
    fn collapsed_zone_blocks_qi_regen_even_with_stale_qi() {
        let mut app = App::new();
        app.insert_resource(CultivationClock::default());
        let mut zones = ZoneRegistry::fallback();
        let zone = zones.find_zone_mut("spawn").unwrap();
        zone.spirit_qi = 0.9;
        zone.active_events.push(EVENT_REALM_COLLAPSE.to_string());
        app.insert_resource(zones);
        app.add_systems(Update, qi_regen_and_zone_drain_tick);

        let mut meridians = MeridianSystem::default();
        meridians.get_mut(MeridianId::Lung).opened = true;
        let player = app
            .world_mut()
            .spawn((
                Position::new([8.0, 66.0, 8.0]),
                meridians,
                Cultivation::default(),
            ))
            .id();

        app.update();

        assert_eq!(
            app.world()
                .entity(player)
                .get::<Cultivation>()
                .unwrap()
                .qi_current,
            0.0
        );
        assert_eq!(
            app.world()
                .resource::<ZoneRegistry>()
                .find_zone_by_name("spawn")
                .unwrap()
                .spirit_qi,
            0.0
        );
    }
}
