//! QiRegenTick + ZoneQiDrainTick（plan §2 QiRegenTick / ZoneQiDrainTick）。
//!
//! 两者合并到一个 system 里执行以天然保证零和：玩家每 tick 吸纳的 qi
//! 必然等量从 zone.spirit_qi 扣除（按 qi_physics 底盘换算系数换算）。符合
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
use crate::combat::woliu_v2::state::TurbulenceExposure;
use crate::cultivation::full_power_strike::Exhausted;
use crate::network::{gameplay_vfx, vfx_event_emit::VfxEventRequest};
use crate::qi_physics::regen_from_zone;
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::events::EVENT_REALM_COLLAPSE;
use crate::world::zone::ZoneRegistry;

use super::color::{
    CultivationSessionPracticeEvent, CULTIVATION_SESSION_PRACTICE_TICKS_PER_MINUTE,
};
use super::components::{ColorKind, Cultivation, MeridianSystem, QiColor, Realm};
use super::lifespan::LifespanComponent;
use super::tribulation::JueBiAftershockDebuff;

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
    last_gain_tick_by_entity: HashMap<Entity, u64>,
}

impl CultivationSessionPracticeAccumulator {
    pub const AUDIO_RECENT_WINDOW_TICKS: u64 = 5 * 20;

    pub fn is_recently_practicing(&self, entity: Entity, now_tick: u64) -> bool {
        self.last_gain_tick_by_entity
            .get(&entity)
            .is_some_and(|last_tick| {
                now_tick >= *last_tick
                    && now_tick.saturating_sub(*last_tick) <= Self::AUDIO_RECENT_WINDOW_TICKS
            })
    }

    fn note_practice_tick(&mut self, entity: Entity, now_tick: u64) -> u64 {
        self.last_gain_tick_by_entity.insert(entity, now_tick);

        let ticks = self.ticks_by_entity.entry(entity).or_default();
        *ticks = ticks.saturating_add(1);

        let minutes = *ticks / CULTIVATION_SESSION_PRACTICE_TICKS_PER_MINUTE;
        if minutes == 0 {
            return 0;
        }

        *ticks %= CULTIVATION_SESSION_PRACTICE_TICKS_PER_MINUTE;
        minutes
    }

    #[cfg(test)]
    pub fn note_practice_tick_for_tests(&mut self, entity: Entity, now_tick: u64) {
        self.note_practice_tick(entity, now_tick);
    }
}

/// 纯函数：给定 zone 浓度、rate、可用额度（qi_max - qi_current - qi_max_frozen 等）
/// 计算本 tick 的实际 gain 与 zone 浓度变化量（均为非负）。
pub fn compute_regen(zone_qi: f64, rate: f64, avg_integrity: f64, qi_room: f64) -> (f64, f64) {
    regen_from_zone(zone_qi, rate, avg_integrity, qi_room)
}

/// QiRegenTick + ZoneQiDrainTick 合并实现。零和：玩家 qi 增量 = zone 浓度减量 × coef。
#[allow(clippy::type_complexity)]
pub fn qi_regen_and_zone_drain_tick(
    mut clock: ResMut<CultivationClock>,
    zone_registry: Option<ResMut<ZoneRegistry>>,
    mut practice_events: Option<ResMut<Events<CultivationSessionPracticeEvent>>>,
    mut practice_accumulator: Option<ResMut<CultivationSessionPracticeAccumulator>>,
    mut vfx_events: Option<ResMut<Events<VfxEventRequest>>>,
    mut players: Query<(
        Entity,
        &Position,
        Option<&CurrentDimension>,
        &MeridianSystem,
        &mut Cultivation,
        Option<&QiColor>,
        Option<&LifespanComponent>,
        Option<&StatusEffects>,
        Option<&Exhausted>,
        Option<&TurbulenceExposure>,
        Option<&JueBiAftershockDebuff>,
    )>,
) {
    clock.tick = clock.tick.wrapping_add(1);

    let Some(mut zones) = zone_registry else {
        return;
    };

    for (
        entity,
        pos,
        current_dim,
        meridians,
        mut cultivation,
        qi_color,
        lifespan,
        statuses,
        exhausted,
        turbulence,
        juebi_aftershock,
    ) in players.iter_mut()
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
        let qi_regen_pause_multiplier = statuses.map(qi_regen_pause_multiplier).unwrap_or(1.0);
        let exhausted_multiplier = exhausted
            .map(|state| state.qi_recovery_modifier)
            .unwrap_or(1.0)
            .clamp(0.05, 1.0);
        let turbulence_multiplier = turbulence
            .map(|exposure| exposure.absorption_multiplier())
            .unwrap_or(1.0);
        let juebi_aftershock_multiplier = juebi_aftershock
            .filter(|debuff| clock.tick <= debuff.until_tick)
            .map(|debuff| debuff.rhythm_multiplier.clamp(0.0, 1.0))
            .unwrap_or(1.0);
        let (gain, drain) = compute_regen(
            zone.spirit_qi,
            rate * wind_candle_multiplier
                * humility_multiplier
                * qi_regen_pause_multiplier
                * exhausted_multiplier
                * turbulence_multiplier
                * juebi_aftershock_multiplier,
            avg_integrity,
            qi_room,
        );
        if gain <= 0.0 {
            continue;
        }

        cultivation.qi_current += gain;
        zone.spirit_qi = (zone.spirit_qi - drain).max(0.0);
        if clock.tick.is_multiple_of(40) {
            if let Some(events) = vfx_events.as_deref_mut() {
                let origin = pos.get() + valence::prelude::DVec3::new(0.0, 0.9, 0.0);
                let spirit_qi = zone.spirit_qi.clamp(0.0, 1.0) as f32;
                let count = (spirit_qi * 10.0).round().clamp(1.0, 16.0) as u32;
                gameplay_vfx::send_spawn(
                    events,
                    gameplay_vfx::spawn_request(
                        gameplay_vfx::CULTIVATION_ABSORB,
                        origin,
                        None,
                        "#66FFCC",
                        spirit_qi.max(0.2),
                        count,
                        30,
                    ),
                );
            }
        }

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
                clock.tick,
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
    now_tick: u64,
    active_color: ColorKind,
) -> u64 {
    let minutes = accumulator.note_practice_tick(entity, now_tick);
    if minutes == 0 {
        return 0;
    }

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
    accumulator
        .last_gain_tick_by_entity
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

fn qi_regen_pause_multiplier(status_effects: &StatusEffects) -> f64 {
    if status_effects
        .active
        .iter()
        .any(|effect| effect.kind == StatusEffectKind::QiRegenPaused && effect.remaining_ticks > 0)
    {
        0.0
    } else {
        1.0
    }
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
        assert!((g - d * zone_unit_qi()).abs() < 1e-9);
    }

    #[test]
    fn qi_room_caps_gain() {
        let (g, d) = compute_regen(1.0, 100.0, 1.0, 0.5);
        assert!(g <= 0.5);
        // 即使被 qi_room 截断，drain 依然按 gain 换算
        assert!((g - d * zone_unit_qi()).abs() < 1e-9);
    }

    #[test]
    fn drain_clamped_to_zone_available() {
        // rate 巨大会把 drain 推到超过 zone_qi
        let zone_qi = 0.001;
        let (g, d) = compute_regen(zone_qi, 1e6, 1.0, 1e9);
        assert!(d <= zone_qi + 1e-12);
        assert!((g - d * zone_unit_qi()).abs() < 1e-6);
    }

    #[test]
    fn zero_sum_property() {
        // 多次 tick 后累积玩家 gain == 累积 zone drain × 底盘换算系数
        let mut zone_qi = 0.5;
        let mut player_qi = 0.0;
        for _ in 0..50 {
            let room = 1e9_f64;
            let (g, d) = compute_regen(zone_qi, 1.0, 1.0, room);
            player_qi += g;
            zone_qi -= d;
        }
        let leaked = player_qi - (0.5 - zone_qi) * zone_unit_qi();
        assert!(leaked.abs() < 1e-6);
    }

    #[test]
    fn regen_migration_preserves_spirit_qi_total_budget() {
        let zone_before = 0.5;
        let player_before = 10.0;
        let reserve_qi = crate::qi_physics::constants::DEFAULT_SPIRIT_QI_TOTAL
            - player_before
            - zone_before * zone_unit_qi();
        assert!(reserve_qi > 0.0);

        let (gain, drain) = compute_regen(zone_before, 1.0, 1.0, 100.0);
        let before_total = player_before + zone_before * zone_unit_qi() + reserve_qi;
        let after_total =
            (player_before + gain) + (zone_before - drain) * zone_unit_qi() + reserve_qi;

        assert!(
            (before_total - crate::qi_physics::constants::DEFAULT_SPIRIT_QI_TOTAL).abs() < 1e-9
        );
        assert!((before_total - after_total).abs() < 1e-6);
    }

    fn zone_unit_qi() -> f64 {
        crate::qi_physics::constants::QI_ZONE_UNIT_CAPACITY
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
    fn exhausted_qi_recovery_is_halved() {
        fn run_once(exhausted: Option<Exhausted>) -> f64 {
            let mut app = App::new();
            app.insert_resource(CultivationClock::default());
            app.insert_resource(ZoneRegistry::fallback());
            app.add_systems(Update, qi_regen_and_zone_drain_tick);

            let mut meridians = MeridianSystem::default();
            meridians.get_mut(MeridianId::Lung).opened = true;
            let mut entity = app.world_mut().spawn((
                Position::new([8.0, 66.0, 8.0]),
                meridians,
                Cultivation::default(),
            ));
            if let Some(state) = exhausted {
                entity.insert(state);
            }
            let entity = entity.id();

            app.update();

            app.world()
                .entity(entity)
                .get::<Cultivation>()
                .unwrap()
                .qi_current
        }

        let normal_qi = run_once(None);
        let exhausted_qi = run_once(Some(Exhausted::from_committed_qi(0, 50.0)));

        assert!(normal_qi > 0.0);
        assert!((exhausted_qi - normal_qi * 0.5).abs() < 1e-6);
    }

    #[test]
    fn juebi_aftershock_debuff_halves_qi_recovery_until_expired() {
        fn run_once(aftershock: Option<JueBiAftershockDebuff>) -> f64 {
            let mut app = App::new();
            app.insert_resource(CultivationClock::default());
            app.insert_resource(ZoneRegistry::fallback());
            app.add_systems(Update, qi_regen_and_zone_drain_tick);

            let mut meridians = MeridianSystem::default();
            meridians.get_mut(MeridianId::Lung).opened = true;
            let mut entity = app.world_mut().spawn((
                Position::new([8.0, 66.0, 8.0]),
                meridians,
                Cultivation::default(),
            ));
            if let Some(debuff) = aftershock {
                entity.insert(debuff);
            }
            let entity = entity.id();

            app.update();

            app.world()
                .entity(entity)
                .get::<Cultivation>()
                .unwrap()
                .qi_current
        }

        let normal_qi = run_once(None);
        let debuffed_qi = run_once(Some(JueBiAftershockDebuff {
            until_tick: 100,
            rhythm_multiplier: 0.5,
        }));
        let expired_qi = run_once(Some(JueBiAftershockDebuff {
            until_tick: 0,
            rhythm_multiplier: 0.5,
        }));

        assert!(normal_qi > 0.0);
        assert!((debuffed_qi - normal_qi * 0.5).abs() < 1e-6);
        assert!((expired_qi - normal_qi).abs() < 1e-6);
    }

    #[test]
    fn turbulence_exposure_blocks_qi_regen() {
        fn run_once(turbulence: Option<crate::combat::woliu_v2::state::TurbulenceExposure>) -> f64 {
            let mut app = App::new();
            app.insert_resource(CultivationClock::default());
            app.insert_resource(ZoneRegistry::fallback());
            app.add_systems(Update, qi_regen_and_zone_drain_tick);

            let mut meridians = MeridianSystem::default();
            meridians.get_mut(MeridianId::Lung).opened = true;
            let mut entity = app.world_mut().spawn((
                Position::new([8.0, 66.0, 8.0]),
                meridians,
                Cultivation::default(),
            ));
            if let Some(exposure) = turbulence {
                entity.insert(exposure);
            }
            let entity = entity.id();

            app.update();

            app.world()
                .entity(entity)
                .get::<Cultivation>()
                .unwrap()
                .qi_current
        }

        let normal_qi = run_once(None);
        let turbulent_qi = run_once(Some(
            crate::combat::woliu_v2::state::TurbulenceExposure::new(Entity::from_raw(99), 1.0, 1),
        ));

        assert!(normal_qi > 0.0);
        assert_eq!(turbulent_qi, 0.0);
    }

    #[test]
    fn meditate_emits_absorb_vfx() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 39 });
        app.insert_resource(ZoneRegistry::fallback());
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, qi_regen_and_zone_drain_tick);

        let mut meridians = MeridianSystem::default();
        meridians.get_mut(MeridianId::Lung).opened = true;
        app.world_mut().spawn((
            Position::new([8.0, 66.0, 8.0]),
            meridians,
            Cultivation::default(),
        ));

        app.update();

        let events = app.world().resource::<Events<VfxEventRequest>>();
        let emitted = events
            .iter_current_update_events()
            .next()
            .expect("qi regen tick should emit cultivation_absorb vfx");
        match &emitted.payload {
            crate::schema::vfx_event::VfxEventPayloadV1::SpawnParticle { event_id, .. } => {
                assert_eq!(event_id, gameplay_vfx::CULTIVATION_ABSORB);
            }
            other => panic!("expected SpawnParticle, got {other:?}"),
        }
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
            accumulator.last_gain_tick_by_entity.insert(live_entity, 12);
            accumulator
                .last_gain_tick_by_entity
                .insert(despawned_entity, 24);
            accumulator
                .last_gain_tick_by_entity
                .insert(uncultivated_entity, 30);
            accumulator
                .last_gain_tick_by_entity
                .insert(missing_entity, 36);
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
        assert_eq!(
            accumulator.last_gain_tick_by_entity.get(&live_entity),
            Some(&12)
        );
        assert!(!accumulator
            .last_gain_tick_by_entity
            .contains_key(&despawned_entity));
        assert!(!accumulator
            .last_gain_tick_by_entity
            .contains_key(&uncultivated_entity));
        assert!(!accumulator
            .last_gain_tick_by_entity
            .contains_key(&missing_entity));
    }

    #[test]
    fn practice_accumulator_exposes_recent_actual_gain_for_audio() {
        let entity = Entity::from_raw(7);
        let mut accumulator = CultivationSessionPracticeAccumulator::default();

        assert!(!accumulator.is_recently_practicing(entity, 10));

        accumulator.note_practice_tick_for_tests(entity, 20);

        assert!(accumulator.is_recently_practicing(entity, 20));
        assert!(accumulator.is_recently_practicing(
            entity,
            20 + CultivationSessionPracticeAccumulator::AUDIO_RECENT_WINDOW_TICKS
        ));
        assert!(!accumulator.is_recently_practicing(
            entity,
            21 + CultivationSessionPracticeAccumulator::AUDIO_RECENT_WINDOW_TICKS
        ));
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
