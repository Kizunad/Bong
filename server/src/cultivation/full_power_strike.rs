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

use crate::combat::components::{Lifecycle, WoundKind, Wounds};
use crate::combat::events::{AttackIntent, AttackReach, AttackSource, CombatEvent};
use crate::combat::realm_gap::{classify_gap, realm_gap_multiplier, realm_index, RealmGapTier};
use crate::combat::{CombatClock, CombatSystemSet};
use crate::cultivation::components::{Cultivation, Realm};
use crate::cultivation::skill_registry::{CastRejectReason, CastResult, SkillRegistry};
use crate::schema::social::RenownTagV1;
use crate::social::events::SocialRenownDeltaEvent;

pub const FULL_POWER_CHARGE_SKILL_ID: &str = "bao_mai.full_power_charge";
pub const FULL_POWER_RELEASE_SKILL_ID: &str = "bao_mai.full_power_release";
pub const FULL_POWER_CHARGE_RATE_PER_TICK: f64 = 50.0;
pub const FULL_POWER_MIN_QI_TO_START: f64 = 100.0;
pub const EXHAUST_TICKS_PER_QI_COMMITTED: u64 = 2;
pub const EXHAUSTED_QI_RECOVERY_MODIFIER: f64 = 0.5;
pub const EXHAUSTED_DEFENSE_MODIFIER: f32 = 0.5;
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

#[derive(Debug, Clone, Component, PartialEq)]
pub struct Exhausted {
    pub started_at_tick: u64,
    pub recovery_at_tick: u64,
    pub qi_recovery_modifier: f64,
    pub defense_modifier: f32,
}

impl Exhausted {
    pub fn from_committed_qi(now_tick: u64, qi_committed: f64) -> Self {
        let duration = exhausted_duration_ticks(qi_committed);
        Self {
            started_at_tick: now_tick,
            recovery_at_tick: now_tick.saturating_add(duration),
            qi_recovery_modifier: EXHAUSTED_QI_RECOVERY_MODIFIER,
            defense_modifier: EXHAUSTED_DEFENSE_MODIFIER,
        }
    }
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

#[derive(Debug, Clone, Event, PartialEq)]
pub struct ExhaustedExpiredEvent {
    pub entity: Entity,
    pub at_tick: u64,
}

pub fn register(app: &mut App) {
    app.add_event::<ChargeStartedEvent>();
    app.add_event::<ChargeInterruptedEvent>();
    app.add_event::<FullPowerAttackIntent>();
    app.add_event::<FullPowerReleasedEvent>();
    app.add_event::<FullPowerStrikeKilledEvent>();
    app.add_event::<ExhaustedExpiredEvent>();
    app.add_systems(
        Update,
        (
            charge_tick_system
                .in_set(CombatSystemSet::Intent)
                .after(crate::combat::debug::tick_combat_clock),
            apply_full_power_attack_intent_system.in_set(CombatSystemSet::Intent),
            exhausted_expire_system.in_set(CombatSystemSet::Physics),
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
    if world.get::<ChargingState>(caster).is_some() || world.get::<Exhausted>(caster).is_some() {
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
    let exhausted = Exhausted::from_committed_qi(now_tick, qi_released);
    world
        .entity_mut(caster)
        .remove::<ChargingState>()
        .remove::<FullPowerChargeRateOverride>();
    world.entity_mut(caster).insert(exhausted);
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

pub fn exhausted_expire_system(
    clock: Res<CombatClock>,
    mut commands: Commands,
    exhausted_q: Query<(Entity, &Exhausted)>,
    mut expired: EventWriter<ExhaustedExpiredEvent>,
) {
    for (entity, exhausted) in &exhausted_q {
        if exhausted.recovery_at_tick > clock.tick {
            continue;
        }
        commands.entity(entity).remove::<Exhausted>();
        expired.send(ExhaustedExpiredEvent {
            entity,
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
    use crate::combat::components::{SkillBarBindings, Wounds};
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
        app.add_event::<ExhaustedExpiredEvent>();
        app.add_event::<SocialRenownDeltaEvent>();
        app
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

    #[test]
    fn release_full_power_emits_attack_intent_and_adds_exhausted() {
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
        let exhausted = app.world().get::<Exhausted>(caster).unwrap();
        assert_eq!(exhausted.recovery_at_tick, 1210);
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
        assert!(app.world().get::<Exhausted>(caster).is_some());
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
        app.world_mut()
            .entity_mut(exhausted_actor)
            .insert(Exhausted::from_committed_qi(10, 100.0));
        assert_eq!(
            start_charge_fn(app.world_mut(), exhausted_actor, 0, None),
            rejected(CastRejectReason::InRecovery)
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
        assert!(app.world().get::<Exhausted>(caster).is_none());
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

    #[test]
    fn release_to_exhausted_to_normal_state_transition() {
        let mut app = app();
        let caster = actor(&mut app, Realm::Induce, 0.0, 200.0);
        app.world_mut()
            .entity_mut(caster)
            .insert(Exhausted::from_committed_qi(10, 50.0));
        app.add_systems(Update, exhausted_expire_system);
        app.world_mut().resource_mut::<CombatClock>().tick = 110;

        app.update();

        assert!(app.world().get::<Exhausted>(caster).is_none());
        assert!(app
            .world()
            .resource::<Events<ExhaustedExpiredEvent>>()
            .iter_current_update_events()
            .any(|event| event.entity == caster));
    }

    #[test]
    fn exhausted_duration_boundaries_match_plan() {
        assert_eq!(exhausted_duration_ticks(50.0), 100);
        assert_eq!(exhausted_duration_ticks(500.0), 1000);
        assert_eq!(exhausted_duration_ticks(2000.0), 4000);
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
}
