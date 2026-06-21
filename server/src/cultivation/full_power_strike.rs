//! 全力一击：长蓄力、一次性灌注、战后虚脱。
//!
//! 渡劫后续接入说明：渡虚劫第三波"无外援"不禁止渡劫者本人使用全力一击。
//! 后续 plan-tribulation-v1 P5 可把劫雷实体作为 target，复用本模块的
//! `FullPowerAttackIntent` 结算。

use std::collections::HashSet;

use valence::prelude::{
    bevy_ecs, App, Commands, Component, DVec3, Entity, Event, EventReader, EventWriter,
    IntoSystemConfigs, Position, Query, Res, UniqueId, Update,
};

use crate::combat::components::{Lifecycle, StatusEffects, WoundKind, Wounds};
use crate::combat::events::{
    ApplyStatusEffectIntent, AttackIntent, AttackReach, AttackSource, CombatEvent, StatusEffectKind,
};
use crate::combat::realm_gap::{classify_gap, realm_gap_multiplier, realm_index, RealmGapTier};
use crate::combat::status::has_active_status;
use crate::combat::{CombatClock, CombatSystemSet};
use crate::cultivation::components::{Cultivation, Realm};
use crate::cultivation::skill_registry::{CastRejectReason, CastResult, SkillRegistry};
use crate::schema::social::RenownTagV1;
use crate::social::events::SocialRenownDeltaEvent;

pub const FULL_POWER_CHARGE_SKILL_ID: &str = "baomai.full_power_charge";
pub const FULL_POWER_RELEASE_SKILL_ID: &str = "baomai.full_power_release";
pub const FULL_POWER_CHARGE_RATE_PER_TICK: f64 = 50.0;
pub const FULL_POWER_MIN_QI_TO_START: f64 = 100.0;
pub const EXHAUST_TICKS_PER_QI_COMMITTED: u64 = 2;
/// 虚脱共享 modifier：同时作 qi 回复倍率（×0.5）与 defense_power 倍率（×0.5）。
/// 以 `ApplyStatusEffectIntent.magnitude` 承载，消费侧 status.rs / tick.rs 读取。
/// 旧的 `Exhausted` 组件分两字段 `qi_recovery_modifier`/`defense_modifier` 均为 0.5，
/// 转 debuff 建模后统一为一个 magnitude，数值守恒不变。
pub const EXHAUSTED_MODIFIER: f32 = 0.5;
/// 兼容旧名（qi 回复倍率，f64）。值与 `EXHAUSTED_MODIFIER` 一致，供 tick.rs 守恒断言。
pub const EXHAUSTED_QI_RECOVERY_MODIFIER: f64 = EXHAUSTED_MODIFIER as f64;
/// 兼容旧名（防御倍率，f32）。值与 `EXHAUSTED_MODIFIER` 一致。
pub const EXHAUSTED_DEFENSE_MODIFIER: f32 = EXHAUSTED_MODIFIER;
pub const FULL_POWER_REACH: AttackReach = AttackReach {
    base: 8.0,
    step_bonus: 0.0,
    max: 8.0,
};
pub const FULL_POWER_RELEASE_COOLDOWN_TICKS: u64 = 20;
pub const FULL_POWER_RELEASE_ANIM_TICKS: u32 = 8;
pub const FULL_POWER_HIGH_REALM_FAME_DELTA: i32 = 25;

#[derive(Debug, Clone, Component, PartialEq)]
pub struct ChargingState {
    pub slot: u8,
    pub started_at_tick: u64,
    pub qi_committed: f64,
    pub target_qi: f64,
}

#[derive(Debug, Clone, Copy, Component, PartialEq)]
pub struct FullPowerChargeRateOverride {
    pub rate_per_tick: f64,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct ChargeStartedEvent {
    pub caster: Entity,
    pub started_at_tick: u64,
    pub initial_qi: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptTrigger {
    Damage,
    Movement,
    Player,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct ChargeInterruptedEvent {
    pub caster: Entity,
    pub qi_lost: f64,
    pub qi_refunded: f64,
    pub trigger: InterruptTrigger,
    pub at_tick: u64,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct FullPowerAttackIntent {
    pub caster: Entity,
    pub target: Option<Entity>,
    pub qi_released: f64,
    pub at_tick: u64,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct FullPowerReleasedEvent {
    pub caster: Entity,
    pub target: Option<Entity>,
    pub qi_released: f64,
    pub at_tick: u64,
    pub hit_position: Option<[f64; 3]>,
    pub realm_gap_tier: Option<RealmGapTier>,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct FullPowerStrikeKilledEvent {
    pub caster: Entity,
    pub target: Entity,
    pub target_realm: Realm,
    pub at_tick: u64,
}

pub fn register(app: &mut App) {
    app.add_event::<ChargeStartedEvent>();
    app.add_event::<ChargeInterruptedEvent>();
    app.add_event::<FullPowerAttackIntent>();
    app.add_event::<FullPowerReleasedEvent>();
    app.add_event::<FullPowerStrikeKilledEvent>();
    app.add_systems(
        Update,
        (
            charge_tick_system
                .in_set(CombatSystemSet::Intent)
                .after(crate::combat::debug::tick_combat_clock),
            apply_full_power_attack_intent_system.in_set(CombatSystemSet::Intent),
        ),
    );
    app.add_systems(
        Update,
        (
            charge_interrupt_system
                .in_set(CombatSystemSet::Resolve)
                .after(crate::combat::resolve::resolve_attack_intents),
            full_power_kill_detection_system
                .in_set(CombatSystemSet::Emit)
                .after(crate::combat::resolve::resolve_attack_intents),
        ),
    );
}

pub fn register_skills(registry: &mut SkillRegistry) {
    registry.register(FULL_POWER_CHARGE_SKILL_ID, start_charge_fn);
    registry.register(FULL_POWER_RELEASE_SKILL_ID, release_full_power_fn);
}

pub fn start_charge_fn(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    _target: Option<Entity>,
) -> CastResult {
    let now_tick = current_tick(world);
    if world
        .get::<crate::combat::components::SkillBarBindings>(caster)
        .is_some_and(|bindings| bindings.is_on_cooldown(slot, now_tick))
    {
        return rejected(CastRejectReason::OnCooldown);
    }
    if world.get::<ChargingState>(caster).is_some() || is_exhausted(world, caster) {
        return rejected(CastRejectReason::InRecovery);
    }

    let Some(cultivation) = world.get::<Cultivation>(caster) else {
        return rejected(CastRejectReason::RealmTooLow);
    };
    if cultivation.qi_current + f64::EPSILON < FULL_POWER_MIN_QI_TO_START {
        return rejected(CastRejectReason::QiInsufficient);
    }

    let initial_qi = cultivation.qi_current.max(0.0);
    world.entity_mut(caster).insert(ChargingState {
        slot,
        started_at_tick: now_tick,
        qi_committed: 0.0,
        target_qi: initial_qi,
    });
    world.send_event(ChargeStartedEvent {
        caster,
        started_at_tick: now_tick,
        initial_qi,
    });

    CastResult::Started {
        cooldown_ticks: 0,
        anim_duration_ticks: 1,
    }
}

pub fn release_full_power_fn(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
) -> CastResult {
    // 标准注册入口：虚脱时长不缩放（multiplier=1.0）。
    // baomai_v3::cast_full_power_release 走 release_full_power_with_exhaust 传入
    // mastery 派生的缩放系数（flow>1.0 时传 None 跳过虚脱）。
    release_full_power_with_exhaust(world, caster, slot, target, Some(1.0))
}

/// 全力一击释放核心实现。
///
/// `exhaust_multiplier`：
/// - `Some(m)` → 虚脱时长 = `exhausted_duration_ticks(qi_released) * m`（baomai skill_lv 缩放）；
/// - `None`    → 完全跳过虚脱（baomai flow>1.0 的「乘风」免虚脱分支）。
///
/// 虚脱以 `ApplyStatusEffectIntent{Exhausted, magnitude=EXHAUSTED_MODIFIER}` 施加，
/// 由标准 status 生命周期到期/`/reset` 清除——不再插入游离 `Exhausted` 组件。
pub fn release_full_power_with_exhaust(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
    exhaust_multiplier: Option<f64>,
) -> CastResult {
    let now_tick = current_tick(world);
    if world
        .get::<crate::combat::components::SkillBarBindings>(caster)
        .is_some_and(|bindings| bindings.is_on_cooldown(slot, now_tick))
    {
        return rejected(CastRejectReason::OnCooldown);
    }

    let Some(state) = world.get::<ChargingState>(caster).cloned() else {
        return rejected(CastRejectReason::InvalidTarget);
    };
    if state.qi_committed + f64::EPSILON < FULL_POWER_MIN_QI_TO_START {
        return rejected(CastRejectReason::QiInsufficient);
    }

    let qi_released = state.qi_committed.max(0.0);
    world
        .entity_mut(caster)
        .remove::<ChargingState>()
        .remove::<FullPowerChargeRateOverride>();
    if let Some(multiplier) = exhaust_multiplier {
        let base_duration = exhausted_duration_ticks(qi_released);
        let duration = scale_exhaust_duration(base_duration, multiplier);
        if duration > 0 {
            world.send_event(ApplyStatusEffectIntent {
                target: caster,
                kind: StatusEffectKind::Exhausted,
                magnitude: EXHAUSTED_MODIFIER,
                duration_ticks: duration,
                issued_at_tick: now_tick,
            });
        }
    }
    if let Some(mut bindings) = world.get_mut::<crate::combat::components::SkillBarBindings>(caster)
    {
        bindings.set_cooldown(
            slot,
            now_tick.saturating_add(FULL_POWER_RELEASE_COOLDOWN_TICKS),
        );
    }

    let hit_position = target.and_then(|entity| position_array(world, entity));
    let realm_gap_tier = target.and_then(|entity| {
        let caster_realm = world.get::<Cultivation>(caster)?.realm;
        let target_realm = world.get::<Cultivation>(entity)?.realm;
        Some(classify_gap(realm_gap_multiplier(
            caster_realm,
            target_realm,
        )))
    });
    world.send_event(FullPowerAttackIntent {
        caster,
        target,
        qi_released,
        at_tick: now_tick,
    });
    world.send_event(FullPowerReleasedEvent {
        caster,
        target,
        qi_released,
        at_tick: now_tick,
        hit_position,
        realm_gap_tier,
    });

    CastResult::Started {
        cooldown_ticks: FULL_POWER_RELEASE_COOLDOWN_TICKS,
        anim_duration_ticks: FULL_POWER_RELEASE_ANIM_TICKS,
    }
}

pub fn charge_tick_system(
    mut q: Query<(
        &mut ChargingState,
        &mut Cultivation,
        Option<&FullPowerChargeRateOverride>,
    )>,
) {
    for (mut charging, mut cultivation, rate_override) in &mut q {
        let remaining = (charging.target_qi - charging.qi_committed).max(0.0);
        let charge_rate = rate_override
            .map(|override_rate| override_rate.rate_per_tick)
            .unwrap_or(FULL_POWER_CHARGE_RATE_PER_TICK)
            .max(0.0);
        let to_consume = charge_rate
            .min(cultivation.qi_current.max(0.0))
            .min(remaining);
        if to_consume <= f64::EPSILON {
            continue;
        }
        cultivation.qi_current =
            (cultivation.qi_current - to_consume).clamp(0.0, cultivation.qi_max);
        charging.qi_committed += to_consume;
    }
}

pub fn apply_full_power_attack_intent_system(
    mut intents: EventReader<FullPowerAttackIntent>,
    cultivations: Query<&Cultivation>,
    mut attacks: EventWriter<AttackIntent>,
) {
    for intent in intents.read() {
        let Some(target) = intent.target else {
            continue;
        };
        let Ok(caster_cultivation) = cultivations.get(intent.caster) else {
            continue;
        };
        let target_realm = cultivations
            .get(target)
            .map(|cultivation| cultivation.realm)
            .unwrap_or(caster_cultivation.realm);
        let multiplier = realm_gap_multiplier(caster_cultivation.realm, target_realm);
        let qi_invest = (intent.qi_released as f32 * multiplier).max(0.0);
        if qi_invest <= f32::EPSILON {
            continue;
        }
        attacks.send(AttackIntent {
            attacker: intent.caster,
            target: Some(target),
            issued_at_tick: intent.at_tick,
            reach: FULL_POWER_REACH,
            qi_invest,
            wound_kind: WoundKind::Concussion,
            source: AttackSource::FullPower,
            debug_command: None,
        });
    }
}

pub fn charge_interrupt_system(
    clock: Res<CombatClock>,
    mut commands: Commands,
    mut combat_events: EventReader<CombatEvent>,
    charging_q: Query<&ChargingState>,
    mut cultivations: Query<&mut Cultivation>,
    mut interrupted: EventWriter<ChargeInterruptedEvent>,
) {
    let mut interrupted_this_tick = HashSet::new();
    for event in combat_events.read() {
        if !interrupted_this_tick.insert(event.target) {
            continue;
        }
        let Ok(charging) = charging_q.get(event.target) else {
            continue;
        };
        let qi_refunded = charging.qi_committed * 0.6;
        let qi_lost = (charging.qi_committed - qi_refunded).max(0.0);
        if let Ok(mut cultivation) = cultivations.get_mut(event.target) {
            cultivation.qi_current =
                (cultivation.qi_current + qi_refunded).clamp(0.0, cultivation.qi_max);
        }
        commands
            .entity(event.target)
            .remove::<ChargingState>()
            .remove::<FullPowerChargeRateOverride>();
        interrupted.send(ChargeInterruptedEvent {
            caster: event.target,
            qi_lost,
            qi_refunded,
            trigger: InterruptTrigger::Damage,
            at_tick: clock.tick,
        });
    }
}

pub fn full_power_kill_detection_system(
    clock: Res<CombatClock>,
    mut combat_events: EventReader<CombatEvent>,
    wounds_q: Query<&Wounds>,
    cultivations: Query<&Cultivation>,
    lifecycles: Query<&Lifecycle>,
    mut killed: EventWriter<FullPowerStrikeKilledEvent>,
    mut renown_deltas: EventWriter<SocialRenownDeltaEvent>,
) {
    for event in combat_events.read() {
        if event.source != AttackSource::FullPower {
            continue;
        }
        let Ok(wounds) = wounds_q.get(event.target) else {
            continue;
        };
        if wounds.health_current > 0.0 {
            continue;
        }
        let Ok(target_cultivation) = cultivations.get(event.target) else {
            continue;
        };
        if !is_high_realm(target_cultivation.realm) {
            continue;
        }
        killed.send(FullPowerStrikeKilledEvent {
            caster: event.attacker,
            target: event.target,
            target_realm: target_cultivation.realm,
            at_tick: clock.tick,
        });

        if let Ok(lifecycle) = lifecycles.get(event.attacker) {
            renown_deltas.send(SocialRenownDeltaEvent {
                char_id: lifecycle.character_id.clone(),
                fame_delta: FULL_POWER_HIGH_REALM_FAME_DELTA,
                notoriety_delta: 0,
                tags_added: vec![RenownTagV1 {
                    tag: "full_power_high_realm_kill".to_string(),
                    weight: 1.0,
                    last_seen_tick: clock.tick,
                    permanent: true,
                }],
                tick: clock.tick,
                reason: "full_power_strike_high_realm_kill".to_string(),
            });
        }
    }
}

pub fn exhausted_duration_ticks(qi_committed: f64) -> u64 {
    if !qi_committed.is_finite() || qi_committed <= 0.0 {
        return 0;
    }
    (qi_committed.ceil() as u64).saturating_mul(EXHAUST_TICKS_PER_QI_COMMITTED)
}

/// 按 baomai skill_lv 缩放系数缩放虚脱基础时长。
/// 与旧 `Exhausted` 组件 retro 缩放等价：`(duration * m).round().max(1.0)`，
/// 但 base_duration==0（释放真元过少）时保持 0（不施加虚脱）。
fn scale_exhaust_duration(base_duration: u64, multiplier: f64) -> u64 {
    if base_duration == 0 {
        return 0;
    }
    let m = if multiplier.is_finite() {
        multiplier.max(0.0)
    } else {
        1.0
    };
    (base_duration as f64 * m).round().max(1.0) as u64
}

/// 是否处于虚脱 debuff（取代旧的 `world.get::<Exhausted>().is_some()`）。
fn is_exhausted(world: &bevy_ecs::world::World, entity: Entity) -> bool {
    world
        .get::<StatusEffects>(entity)
        .is_some_and(|status| has_active_status(status, StatusEffectKind::Exhausted))
}

fn is_high_realm(realm: Realm) -> bool {
    realm_index(realm) >= realm_index(Realm::Spirit)
}

fn current_tick(world: &bevy_ecs::world::World) -> u64 {
    world
        .get_resource::<CombatClock>()
        .map(|clock| clock.tick)
        .unwrap_or_default()
}

fn position_array(world: &bevy_ecs::world::World, entity: Entity) -> Option<[f64; 3]> {
    let position = world.get::<Position>(entity)?.get();
    Some([position.x, position.y, position.z])
}

pub fn entity_uuid(world: &bevy_ecs::world::World, entity: Entity) -> Option<String> {
    world
        .get::<UniqueId>(entity)
        .map(|unique_id| unique_id.0.to_string())
}

pub fn entity_position(world: &bevy_ecs::world::World, entity: Entity) -> Option<DVec3> {
    world.get::<Position>(entity).map(|position| position.get())
}

fn rejected(reason: CastRejectReason) -> CastResult {
    CastResult::Rejected { reason }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::components::{ActiveStatusEffect, SkillBarBindings, StatusEffects, Wounds};
    use crate::combat::events::CombatEvent;
    use crate::social::events::SocialRenownDeltaEvent;
    use valence::prelude::{App, Events, Update};

    fn app() -> App {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 10 });
        app.add_event::<AttackIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<ChargeStartedEvent>();
        app.add_event::<ChargeInterruptedEvent>();
        app.add_event::<FullPowerAttackIntent>();
        app.add_event::<FullPowerReleasedEvent>();
        app.add_event::<FullPowerStrikeKilledEvent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<SocialRenownDeltaEvent>();
        app
    }

    /// 测试辅助：读取最近一次施加给 caster 的虚脱 ApplyStatusEffectIntent 的时长。
    fn last_exhausted_intent_duration(app: &App, caster: Entity) -> Option<u64> {
        app.world()
            .resource::<Events<ApplyStatusEffectIntent>>()
            .iter_current_update_events()
            .filter(|e| e.target == caster && e.kind == StatusEffectKind::Exhausted)
            .map(|e| e.duration_ticks)
            .last()
    }

    /// 测试辅助：把虚脱 status 直接写入实体的 StatusEffects（模拟 status_effect_apply_tick 后果）。
    fn insert_exhausted_status(app: &mut App, caster: Entity, remaining_ticks: u64) {
        app.world_mut()
            .entity_mut(caster)
            .get_mut::<StatusEffects>()
            .expect("actor should have StatusEffects")
            .active
            .push(ActiveStatusEffect {
                kind: StatusEffectKind::Exhausted,
                magnitude: EXHAUSTED_MODIFIER,
                remaining_ticks,
                source_pill: None,
            });
    }

    fn actor(app: &mut App, realm: Realm, qi_current: f64, qi_max: f64) -> Entity {
        app.world_mut()
            .spawn((
                Cultivation {
                    realm,
                    qi_current,
                    qi_max,
                    ..Default::default()
                },
                SkillBarBindings::default(),
                StatusEffects::default(),
                Wounds::default(),
                Lifecycle {
                    character_id: format!("char:{realm:?}:{qi_max}"),
                    ..Default::default()
                },
            ))
            .id()
    }

    #[test]
    fn start_charge_adds_charging_state_when_qi_sufficient() {
        let mut app = app();
        let caster = actor(&mut app, Realm::Induce, 150.0, 150.0);

        let result = start_charge_fn(app.world_mut(), caster, 0, None);

        assert!(matches!(result, CastResult::Started { .. }));
        let charging = app.world().get::<ChargingState>(caster).unwrap();
        assert_eq!(charging.qi_committed, 0.0);
        assert_eq!(charging.target_qi, 150.0);
        assert_eq!(charging.started_at_tick, 10);
    }

    #[test]
    fn charge_tick_consumes_qi_and_increases_committed() {
        let mut app = app();
        let caster = actor(&mut app, Realm::Induce, 150.0, 150.0);
        app.world_mut().entity_mut(caster).insert(ChargingState {
            slot: 0,
            started_at_tick: 10,
            qi_committed: 0.0,
            target_qi: 150.0,
        });
        app.add_systems(Update, charge_tick_system);

        app.update();

        assert_eq!(
            app.world().get::<Cultivation>(caster).unwrap().qi_current,
            100.0
        );
        assert_eq!(
            app.world()
                .get::<ChargingState>(caster)
                .unwrap()
                .qi_committed,
            50.0
        );
    }

    #[test]
    fn charge_tick_caps_at_target_qi() {
        let mut app = app();
        let caster = actor(&mut app, Realm::Induce, 80.0, 150.0);
        app.world_mut().entity_mut(caster).insert(ChargingState {
            slot: 0,
            started_at_tick: 10,
            qi_committed: 130.0,
            target_qi: 150.0,
        });
        app.add_systems(Update, charge_tick_system);

        app.update();

        assert_eq!(
            app.world().get::<Cultivation>(caster).unwrap().qi_current,
            60.0
        );
        assert_eq!(
            app.world()
                .get::<ChargingState>(caster)
                .unwrap()
                .qi_committed,
            150.0
        );
    }

    /// 释放：发 FullPowerAttackIntent + 以 ApplyStatusEffectIntent 施加虚脱 debuff
    /// （duration = qi_released*2 = 1200），不再插入游离 Exhausted 组件。
    #[test]
    fn release_full_power_emits_attack_intent_and_applies_exhausted_debuff() {
        let mut app = app();
        let caster = actor(&mut app, Realm::Condense, 0.0, 600.0);
        let target = actor(&mut app, Realm::Solidify, 100.0, 2000.0);
        app.world_mut().entity_mut(caster).insert(ChargingState {
            slot: 0,
            started_at_tick: 10,
            qi_committed: 600.0,
            target_qi: 600.0,
        });

        let result = release_full_power_fn(app.world_mut(), caster, 1, Some(target));

        assert!(matches!(result, CastResult::Started { .. }));
        assert!(app.world().get::<ChargingState>(caster).is_none());
        assert_eq!(
            last_exhausted_intent_duration(&app, caster),
            Some(1200),
            "虚脱 debuff 时长应为 qi_released(600)*EXHAUST_TICKS_PER_QI_COMMITTED(2)=1200"
        );
        let intent = app
            .world()
            .resource::<Events<ApplyStatusEffectIntent>>()
            .iter_current_update_events()
            .find(|e| e.target == caster && e.kind == StatusEffectKind::Exhausted)
            .cloned()
            .expect("应施加虚脱 debuff");
        assert_eq!(
            intent.magnitude, EXHAUSTED_MODIFIER,
            "虚脱 magnitude 承载共享 modifier 0.5"
        );
        assert!(app
            .world()
            .resource::<Events<FullPowerAttackIntent>>()
            .iter_current_update_events()
            .any(|event| event.caster == caster && event.target == Some(target)));
    }

    #[test]
    fn full_power_attack_applies_realm_gap_multiplier() {
        let mut app = app();
        let caster = actor(&mut app, Realm::Condense, 0.0, 600.0);
        let target = actor(&mut app, Realm::Solidify, 100.0, 2000.0);
        app.add_systems(Update, apply_full_power_attack_intent_system);
        app.world_mut().send_event(FullPowerAttackIntent {
            caster,
            target: Some(target),
            qi_released: 600.0,
            at_tick: 10,
        });

        app.update();

        let attacks = app.world().resource::<Events<AttackIntent>>();
        let attack = attacks.iter_current_update_events().next().unwrap();
        assert_eq!(attack.source, AttackSource::FullPower);
        assert!((attack.qi_invest - 166.8).abs() < 0.1);
    }

    #[test]
    fn release_with_no_target_still_consumes_qi_and_exhausts() {
        let mut app = app();
        let caster = actor(&mut app, Realm::Condense, 0.0, 600.0);
        app.world_mut().entity_mut(caster).insert(ChargingState {
            slot: 0,
            started_at_tick: 10,
            qi_committed: 600.0,
            target_qi: 600.0,
        });

        let result = release_full_power_fn(app.world_mut(), caster, 1, None);

        assert!(matches!(result, CastResult::Started { .. }));
        assert_eq!(
            last_exhausted_intent_duration(&app, caster),
            Some(1200),
            "无目标释放也应施加虚脱 debuff（消耗真元仍触发虚脱）"
        );
        let released = app.world().resource::<Events<FullPowerReleasedEvent>>();
        assert!(released
            .iter_current_update_events()
            .any(|event| event.target.is_none() && event.qi_released == 600.0));
    }

    #[test]
    fn rejects_invalid_charge_and_release_states() {
        let mut app = app();
        let no_cultivation = app.world_mut().spawn(SkillBarBindings::default()).id();
        assert_eq!(
            start_charge_fn(app.world_mut(), no_cultivation, 0, None),
            rejected(CastRejectReason::RealmTooLow)
        );

        let low_qi = actor(&mut app, Realm::Induce, 99.0, 100.0);
        assert_eq!(
            start_charge_fn(app.world_mut(), low_qi, 0, None),
            rejected(CastRejectReason::QiInsufficient)
        );

        let charging = actor(&mut app, Realm::Induce, 120.0, 120.0);
        app.world_mut().entity_mut(charging).insert(ChargingState {
            slot: 0,
            started_at_tick: 10,
            qi_committed: 10.0,
            target_qi: 120.0,
        });
        assert_eq!(
            start_charge_fn(app.world_mut(), charging, 0, None),
            rejected(CastRejectReason::InRecovery)
        );
        assert_eq!(
            release_full_power_fn(app.world_mut(), charging, 1, None),
            rejected(CastRejectReason::QiInsufficient)
        );

        let exhausted_actor = actor(&mut app, Realm::Induce, 120.0, 120.0);
        insert_exhausted_status(&mut app, exhausted_actor, 200);
        assert_eq!(
            start_charge_fn(app.world_mut(), exhausted_actor, 0, None),
            rejected(CastRejectReason::InRecovery),
            "虚脱 debuff（StatusEffects）期间禁止重新蓄力"
        );

        let idle = actor(&mut app, Realm::Induce, 120.0, 120.0);
        assert_eq!(
            release_full_power_fn(app.world_mut(), idle, 1, None),
            rejected(CastRejectReason::InvalidTarget)
        );
    }

    #[test]
    fn charge_interrupted_by_damage_refunds_60_percent_qi() {
        let mut app = app();
        let attacker = actor(&mut app, Realm::Induce, 100.0, 100.0);
        let caster = actor(&mut app, Realm::Induce, 50.0, 200.0);
        app.world_mut().entity_mut(caster).insert(ChargingState {
            slot: 0,
            started_at_tick: 10,
            qi_committed: 100.0,
            target_qi: 200.0,
        });
        app.add_systems(Update, charge_interrupt_system);
        app.world_mut().send_event(CombatEvent {
            attacker,
            target: caster,
            resolved_at_tick: 10,
            body_part: crate::combat::components::BodyPart::Chest,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: false,
            physical_damage: 0.0,
            damage: 1.0,
            contam_delta: 0.0,
            description: "test hit".to_string(),
            defense_kind: None,
            defense_effectiveness: None,
            defense_contam_reduced: None,
            defense_wound_severity: None,
        });

        app.update();

        assert!(app.world().get::<ChargingState>(caster).is_none());
        assert!(
            !is_exhausted(app.world(), caster),
            "蓄力被打断不应进入虚脱（虚脱仅在成功释放后施加）"
        );
        assert_eq!(
            app.world().get::<Cultivation>(caster).unwrap().qi_current,
            110.0
        );
        let event = app
            .world()
            .resource::<Events<ChargeInterruptedEvent>>()
            .iter_current_update_events()
            .next()
            .unwrap();
        assert_eq!(event.qi_refunded, 60.0);
        assert_eq!(event.qi_lost, 40.0);
    }

    #[test]
    fn charge_interrupted_by_multiple_hits_refunds_once() {
        let mut app = app();
        let attacker = actor(&mut app, Realm::Induce, 100.0, 100.0);
        let caster = actor(&mut app, Realm::Induce, 50.0, 200.0);
        app.world_mut().entity_mut(caster).insert(ChargingState {
            slot: 0,
            started_at_tick: 10,
            qi_committed: 100.0,
            target_qi: 200.0,
        });
        app.add_systems(Update, charge_interrupt_system);
        for _ in 0..2 {
            app.world_mut().send_event(CombatEvent {
                attacker,
                target: caster,
                resolved_at_tick: 10,
                body_part: crate::combat::components::BodyPart::Chest,
                wound_kind: WoundKind::Blunt,
                source: AttackSource::Melee,
                debug_command: false,
                physical_damage: 0.0,
                damage: 1.0,
                contam_delta: 0.0,
                description: "test hit".to_string(),
                defense_kind: None,
                defense_effectiveness: None,
                defense_contam_reduced: None,
                defense_wound_severity: None,
            });
        }

        app.update();

        assert_eq!(
            app.world().get::<Cultivation>(caster).unwrap().qi_current,
            110.0
        );
        let events = app.world().resource::<Events<ChargeInterruptedEvent>>();
        assert_eq!(events.iter_current_update_events().count(), 1);
    }

    /// 端到端状态机：释放 → status_effect_apply_tick 入 StatusEffects（虚脱生效）
    /// → status_effect_tick 时长归零后移除（虚脱解除）。
    /// 取代旧的游离 exhausted_expire_system——到期改由标准 status 生命周期管理。
    #[test]
    fn release_to_exhausted_to_normal_state_transition() {
        use crate::combat::components::STATUS_EFFECT_TICK_INTERVAL_TICKS;
        use crate::combat::status::{status_effect_apply_tick, status_effect_tick};

        let mut app = app();
        // 短时长便于到期：qi_committed=1 → duration=2 ticks。
        let caster = actor(&mut app, Realm::Induce, 0.0, 200.0);
        app.world_mut().entity_mut(caster).insert(ChargingState {
            slot: 0,
            started_at_tick: 10,
            qi_committed: 1.0,
            target_qi: 1.0,
        });
        // qi_committed=1 < FULL_POWER_MIN_QI_TO_START(100) 会被拒；直接施加短时虚脱 status。
        insert_exhausted_status(&mut app, caster, STATUS_EFFECT_TICK_INTERVAL_TICKS);
        assert!(is_exhausted(app.world(), caster), "施加后应处于虚脱态");

        // 推进标准 status tick：remaining 归零 → retain 移除。
        app.add_systems(
            Update,
            (status_effect_apply_tick, status_effect_tick).chain(),
        );
        app.world_mut().resource_mut::<CombatClock>().tick = STATUS_EFFECT_TICK_INTERVAL_TICKS;
        app.update();

        assert!(
            !is_exhausted(app.world(), caster),
            "虚脱 status 到期后应由标准 status 生命周期移除（不再有独立 expire 系统）"
        );
    }

    /// 虚脱 debuff 是否随成功释放真正进入 StatusEffects（走完整 apply 链路）。
    #[test]
    fn release_then_apply_tick_puts_exhausted_into_status_effects() {
        use crate::combat::status::status_effect_apply_tick;

        let mut app = app();
        let caster = actor(&mut app, Realm::Condense, 0.0, 600.0);
        app.world_mut().entity_mut(caster).insert(ChargingState {
            slot: 0,
            started_at_tick: 10,
            qi_committed: 600.0,
            target_qi: 600.0,
        });

        let result = release_full_power_fn(app.world_mut(), caster, 1, None);
        assert!(matches!(result, CastResult::Started { .. }));

        app.add_systems(Update, status_effect_apply_tick);
        app.update();

        assert!(
            is_exhausted(app.world(), caster),
            "release 发的 ApplyStatusEffectIntent 应被 status_effect_apply_tick 落入 StatusEffects"
        );
    }

    #[test]
    fn exhausted_duration_boundaries_match_plan() {
        assert_eq!(exhausted_duration_ticks(50.0), 100);
        assert_eq!(exhausted_duration_ticks(500.0), 1000);
        assert_eq!(exhausted_duration_ticks(2000.0), 4000);
    }

    /// scale_exhaust_duration：baomai skill_lv 缩放与边界。
    #[test]
    fn scale_exhaust_duration_applies_multiplier_and_floors_at_one() {
        // 满级缩放系数 0.7（1 - 1.0*0.30）：1200 * 0.7 = 840。
        assert_eq!(scale_exhaust_duration(1200, 0.7), 840);
        // multiplier=1.0（标准入口）不变。
        assert_eq!(scale_exhaust_duration(1200, 1.0), 1200);
        // base=0（释放真元过少）→ 0，不施加虚脱。
        assert_eq!(scale_exhaust_duration(0, 0.7), 0);
        // 极小非零结果 floor 到 1（永不把存在的虚脱缩到 0）。
        assert_eq!(scale_exhaust_duration(1, 0.0001), 1);
        // 非有限 multiplier 兜底为 1.0。
        assert_eq!(scale_exhaust_duration(100, f64::NAN), 100);
    }

    #[test]
    fn full_power_kill_high_realm_emits_killed_and_renown_events() {
        let mut app = app();
        let caster = actor(&mut app, Realm::Condense, 0.0, 600.0);
        let target = actor(&mut app, Realm::Void, 0.0, 1000.0);
        app.world_mut()
            .get_mut::<Wounds>(target)
            .unwrap()
            .health_current = 0.0;
        app.add_systems(Update, full_power_kill_detection_system);
        app.world_mut().send_event(CombatEvent {
            attacker: caster,
            target,
            resolved_at_tick: 10,
            body_part: crate::combat::components::BodyPart::Chest,
            wound_kind: WoundKind::Concussion,
            source: AttackSource::FullPower,
            debug_command: false,
            physical_damage: 0.0,
            damage: 1000.0,
            contam_delta: 0.0,
            description: "full power".to_string(),
            defense_kind: None,
            defense_effectiveness: None,
            defense_contam_reduced: None,
            defense_wound_severity: None,
        });

        app.update();

        assert!(app
            .world()
            .resource::<Events<FullPowerStrikeKilledEvent>>()
            .iter_current_update_events()
            .any(|event| event.caster == caster && event.target == target));
        assert!(app
            .world()
            .resource::<Events<SocialRenownDeltaEvent>>()
            .iter_current_update_events()
            .any(|event| event.fame_delta >= FULL_POWER_HIGH_REALM_FAME_DELTA));
    }

    #[test]
    fn full_power_kill_low_realm_does_not_emit_killed_event() {
        let mut app = app();
        let caster = actor(&mut app, Realm::Condense, 0.0, 600.0);
        let target = actor(&mut app, Realm::Induce, 0.0, 100.0);
        app.world_mut()
            .get_mut::<Wounds>(target)
            .unwrap()
            .health_current = 0.0;
        app.add_systems(Update, full_power_kill_detection_system);
        app.world_mut().send_event(CombatEvent {
            attacker: caster,
            target,
            resolved_at_tick: 10,
            body_part: crate::combat::components::BodyPart::Chest,
            wound_kind: WoundKind::Concussion,
            source: AttackSource::FullPower,
            debug_command: false,
            physical_damage: 0.0,
            damage: 1000.0,
            contam_delta: 0.0,
            description: "full power".to_string(),
            defense_kind: None,
            defense_effectiveness: None,
            defense_contam_reduced: None,
            defense_wound_severity: None,
        });

        app.update();

        assert!(app
            .world()
            .resource::<Events<FullPowerStrikeKilledEvent>>()
            .iter_current_update_events()
            .next()
            .is_none());
    }

    // --- bao_mai ↔ baomai ID 常数 pin (regression: r2-P3 bughunt fix) ---

    #[test]
    fn skill_id_constants_use_canonical_baomai_prefix() {
        // 期望 "baomai.full_power_charge" / "baomai.full_power_release"（无下划线），
        // 与 combat::baomai_v3::events 中 BAOMAI_FULL_POWER_*_SKILL_ID 以及
        // SkillMeridianDependencies 的 declare 键一致；若用 "bao_mai.*" 则
        // skill registry 注册键与 meridian dep 门控键分叉，实际游戏中经脉门控失效。
        assert_eq!(
            FULL_POWER_CHARGE_SKILL_ID,
            "baomai.full_power_charge",
            "FULL_POWER_CHARGE_SKILL_ID must be 'baomai.full_power_charge' (no underscore in prefix)"
        );
        assert_eq!(
            FULL_POWER_RELEASE_SKILL_ID,
            "baomai.full_power_release",
            "FULL_POWER_RELEASE_SKILL_ID must be 'baomai.full_power_release' (no underscore in prefix)"
        );
    }

    #[test]
    fn skill_ids_match_baomai_v3_events_constants() {
        // 交叉校验：full_power_strike 的常数必须与 baomai_v3::events 中对应常数完全一致，
        // 否则 register_skills 注册的 SkillRegistry 入口与 check_static_deps
        // 使用的 meridian dep 键脱节。
        assert_eq!(
            FULL_POWER_CHARGE_SKILL_ID,
            crate::combat::baomai_v3::events::BAOMAI_FULL_POWER_CHARGE_SKILL_ID,
            "full_power_strike::FULL_POWER_CHARGE_SKILL_ID should equal baomai_v3::BAOMAI_FULL_POWER_CHARGE_SKILL_ID"
        );
        assert_eq!(
            FULL_POWER_RELEASE_SKILL_ID,
            crate::combat::baomai_v3::events::BAOMAI_FULL_POWER_RELEASE_SKILL_ID,
            "full_power_strike::FULL_POWER_RELEASE_SKILL_ID should equal baomai_v3::BAOMAI_FULL_POWER_RELEASE_SKILL_ID"
        );
    }
}
