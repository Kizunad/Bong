use valence::prelude::{bevy_ecs, DVec3, Entity, Position};

use crate::combat::components::{BodyPart, Lifecycle, SkillBarBindings, Wound, WoundKind, Wounds};
use crate::combat::events::{
    ApplyStatusEffectIntent, AttackIntent, AttackReach, AttackSource, StatusEffectKind, FIST_REACH,
};
use crate::combat::CombatClock;
use crate::cultivation::color::{record_style_practice, PracticeLog};
use crate::cultivation::components::{
    ColorKind, ContamSource, Contamination, CrackCause, Cultivation, MeridianCrack, MeridianId,
    MeridianSystem, Realm,
};
use crate::cultivation::full_power_strike;
use crate::cultivation::meridian::severed::{
    check_meridian_dependencies, MeridianSeveredEvent, MeridianSeveredPermanent, SeveredSource,
    SkillMeridianDependencies,
};
use crate::cultivation::skill_registry::{CastRejectReason, CastResult, SkillRegistry};
use crate::cultivation::tribulation::{JueBiTriggerEvent, JueBiTriggerSource};
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::qi_physics::{
    aoe_ground_wave, blood_burn_conversion, body_transcendence, QiAccountId, QiTransfer,
    QiTransferReason,
};
use crate::schema::vfx_event::VfxEventPayloadV1;
use crate::skill::components::SkillId;
use crate::skill::events::{SkillXpGain, XpGainSource};

use super::events::{
    BaomaiSkillEvent, BaomaiSkillId, BloodBurnEvent, DispersedQiEvent, MountainShakeEvent,
    OverloadMeridianRippleEvent, BAOMAI_BENG_QUAN_SKILL_ID, BAOMAI_BLOOD_BURN_SKILL_ID,
    BAOMAI_DISPERSE_SKILL_ID, BAOMAI_FULL_POWER_CHARGE_SKILL_ID,
    BAOMAI_FULL_POWER_RELEASE_SKILL_ID, BAOMAI_MOUNTAIN_SHAKE_SKILL_ID,
};
use super::physics::{
    beng_quan_cooldown_ticks, beng_quan_overload_multiplier, beng_quan_qi_cost, blood_burn_profile,
    disperse_profile, full_power_charge_rate_per_tick, full_power_exhausted_duration_multiplier,
    mountain_shake_profile, overload_severity, skill_qi_multiplier,
};
use super::state::{
    BaomaiMastery, BloodBurnActive, BodyTranscendence, DisperseCastHistory, MeridianRippleScar,
};

const HAND_YANG: [MeridianId; 3] = [
    MeridianId::LargeIntestine,
    MeridianId::SmallIntestine,
    MeridianId::TripleEnergizer,
];
const FULL_POWER_DEPS: [MeridianId; 5] = [
    MeridianId::Ren,
    MeridianId::Du,
    MeridianId::LargeIntestine,
    MeridianId::SmallIntestine,
    MeridianId::TripleEnergizer,
];
const MOUNTAIN_SHAKE_DEPS: [MeridianId; 6] = [
    MeridianId::Stomach,
    MeridianId::Bladder,
    MeridianId::Gallbladder,
    MeridianId::LargeIntestine,
    MeridianId::SmallIntestine,
    MeridianId::TripleEnergizer,
];
const BLOOD_BURN_DEPS: [MeridianId; 3] = [MeridianId::Liver, MeridianId::Ren, MeridianId::Du];
const DISPERSE_JUEBI_WINDOW_TICKS: u64 = 30 * 24 * 60 * 60 * 20;

pub fn register_skills(registry: &mut SkillRegistry) {
    registry.register(BaomaiSkillId::BengQuan.as_str(), cast_beng_quan);
    registry.register(
        BaomaiSkillId::FullPowerCharge.as_str(),
        cast_full_power_charge,
    );
    registry.register(
        BaomaiSkillId::FullPowerRelease.as_str(),
        cast_full_power_release,
    );
    registry.register(BaomaiSkillId::MountainShake.as_str(), cast_mountain_shake);
    registry.register(BaomaiSkillId::BloodBurn.as_str(), cast_blood_burn);
    registry.register(BaomaiSkillId::Disperse.as_str(), cast_disperse);
}

pub fn declare_meridian_dependencies(dependencies: &mut SkillMeridianDependencies) {
    dependencies.declare(BAOMAI_BENG_QUAN_SKILL_ID, HAND_YANG.to_vec());
    dependencies.declare(BAOMAI_FULL_POWER_CHARGE_SKILL_ID, FULL_POWER_DEPS.to_vec());
    dependencies.declare(BAOMAI_FULL_POWER_RELEASE_SKILL_ID, FULL_POWER_DEPS.to_vec());
    dependencies.declare(BAOMAI_MOUNTAIN_SHAKE_SKILL_ID, MOUNTAIN_SHAKE_DEPS.to_vec());
    dependencies.declare(BAOMAI_BLOOD_BURN_SKILL_ID, BLOOD_BURN_DEPS.to_vec());
    dependencies.declare(BAOMAI_DISPERSE_SKILL_ID, MeridianId::ALL.to_vec());
}

pub fn cast_beng_quan(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
) -> CastResult {
    let Some(target) = target else {
        return rejected(CastRejectReason::InvalidTarget);
    };
    let now_tick = current_tick(world);
    if is_slot_on_cooldown(world, caster, slot, now_tick) {
        return rejected(CastRejectReason::OnCooldown);
    }
    let Some((caster_pos, target_pos)) = caster_target_positions(world, caster, target) else {
        return rejected(CastRejectReason::InvalidTarget);
    };
    if caster_pos.distance(target_pos) > f64::from(FIST_REACH.max) + f64::EPSILON {
        return rejected(CastRejectReason::InvalidTarget);
    }
    let Some(cultivation) = world.get::<Cultivation>(caster).cloned() else {
        return rejected(CastRejectReason::RealmTooLow);
    };
    if cultivation.realm == Realm::Awaken {
        return rejected(CastRejectReason::RealmTooLow);
    }
    let Some(meridians) = world.get::<MeridianSystem>(caster) else {
        return rejected(CastRejectReason::MERIDIAN_SEVERED);
    };
    let severed = world.get::<MeridianSeveredPermanent>(caster);
    let hand_severed = HAND_YANG.iter().any(|id| {
        severed.is_some_and(|s| s.is_severed(*id)) || meridians.get(*id).integrity <= f64::EPSILON
    });
    let mastery = mastery_level(world, caster, BaomaiSkillId::BengQuan);
    let blood = active_blood_multiplier(world, caster, now_tick);
    let flow = active_flow_multiplier(world, caster, now_tick);
    let heavy = heavy_color_multiplier(world, caster);
    let dependency_multiplier = if hand_severed { 0.5 } else { 1.0 };
    let base_cost = beng_quan_qi_cost(cultivation.qi_max);
    let qi_invested = base_cost
        * beng_quan_overload_multiplier(cultivation.realm)
        * skill_qi_multiplier(blood, flow, heavy)
        * dependency_multiplier;
    if cultivation.qi_current + f64::EPSILON < base_cost {
        return rejected(CastRejectReason::QiInsufficient);
    }
    spend_qi(world, caster, base_cost);
    set_slot_cooldown(
        world,
        caster,
        slot,
        now_tick.saturating_add(beng_quan_cooldown_ticks(mastery)),
    );
    world.send_event(AttackIntent {
        attacker: caster,
        target: Some(target),
        issued_at_tick: now_tick,
        reach: FIST_REACH,
        qi_invest: qi_invested as f32,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::BurstMeridian,
        debug_command: None,
    });
    emit_skill_event(
        world,
        BaomaiSkillEvent {
            skill: BaomaiSkillId::BengQuan,
            caster,
            target: Some(target),
            tick: now_tick,
            qi_invested,
            damage: qi_invested as f32,
            radius_blocks: None,
            blood_multiplier: blood,
            flow_rate_multiplier: flow,
            meridian_dependencies: HAND_YANG.to_vec(),
        },
    );
    record_overload(
        world,
        caster,
        BaomaiSkillId::BengQuan,
        &HAND_YANG,
        flow,
        now_tick,
    );
    record_qi_transfer(world, caster, BaomaiSkillId::BengQuan, base_cost);
    record_practice(world, caster, BaomaiSkillId::BengQuan);
    emit_particle(
        world,
        caster_pos,
        "bong:meridian_ripple_scar",
        "#D8A03A",
        0.9,
        8,
    );
    CastResult::Started {
        cooldown_ticks: beng_quan_cooldown_ticks(mastery),
        anim_duration_ticks: 8,
    }
}

pub fn cast_full_power_charge(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
) -> CastResult {
    if let Err(reason) = check_static_deps(world, caster, BAOMAI_FULL_POWER_CHARGE_SKILL_ID) {
        return rejected(reason);
    }
    let result = full_power_strike::start_charge_fn(world, caster, slot, target);
    if matches!(result, CastResult::Started { .. }) {
        let mastery = mastery_level(world, caster, BaomaiSkillId::FullPowerCharge);
        world
            .entity_mut(caster)
            .insert(full_power_strike::FullPowerChargeRateOverride {
                rate_per_tick: full_power_charge_rate_per_tick(mastery),
            });
        emit_skill_event(
            world,
            BaomaiSkillEvent {
                skill: BaomaiSkillId::FullPowerCharge,
                caster,
                target,
                tick: current_tick(world),
                qi_invested: 0.0,
                damage: 0.0,
                radius_blocks: None,
                blood_multiplier: active_blood_multiplier(world, caster, current_tick(world)),
                flow_rate_multiplier: active_flow_multiplier(world, caster, current_tick(world)),
                meridian_dependencies: FULL_POWER_DEPS.to_vec(),
            },
        );
        record_practice(world, caster, BaomaiSkillId::FullPowerCharge);
    }
    result
}

pub fn cast_full_power_release(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
) -> CastResult {
    if let Err(reason) = check_static_deps(world, caster, BAOMAI_FULL_POWER_RELEASE_SKILL_ID) {
        return rejected(reason);
    }
    let now_tick = current_tick(world);
    let flow = active_flow_multiplier(world, caster, now_tick);
    let mastery = mastery_level(world, caster, BaomaiSkillId::FullPowerRelease);
    let result = full_power_strike::release_full_power_fn(world, caster, slot, target);
    if matches!(result, CastResult::Started { .. }) {
        if flow > 1.0 {
            world
                .entity_mut(caster)
                .remove::<full_power_strike::Exhausted>();
        } else if let Some(mut exhausted) = world.get_mut::<full_power_strike::Exhausted>(caster) {
            let duration = exhausted
                .recovery_at_tick
                .saturating_sub(exhausted.started_at_tick);
            let scaled = (duration as f64 * full_power_exhausted_duration_multiplier(mastery))
                .round()
                .max(1.0) as u64;
            exhausted.recovery_at_tick = exhausted.started_at_tick.saturating_add(scaled);
        }
        record_overload(
            world,
            caster,
            BaomaiSkillId::FullPowerRelease,
            &FULL_POWER_DEPS,
            flow,
            now_tick,
        );
        emit_skill_event(
            world,
            BaomaiSkillEvent {
                skill: BaomaiSkillId::FullPowerRelease,
                caster,
                target,
                tick: now_tick,
                qi_invested: 0.0,
                damage: 0.0,
                radius_blocks: None,
                blood_multiplier: active_blood_multiplier(world, caster, now_tick),
                flow_rate_multiplier: flow,
                meridian_dependencies: FULL_POWER_DEPS.to_vec(),
            },
        );
        record_practice(world, caster, BaomaiSkillId::FullPowerRelease);
    }
    result
}

pub fn cast_mountain_shake(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    _target: Option<Entity>,
) -> CastResult {
    let now_tick = current_tick(world);
    if is_slot_on_cooldown(world, caster, slot, now_tick) {
        return rejected(CastRejectReason::OnCooldown);
    }
    if let Err(reason) = check_static_deps(world, caster, BAOMAI_MOUNTAIN_SHAKE_SKILL_ID) {
        return rejected(reason);
    }
    let Some(cultivation) = world.get::<Cultivation>(caster).cloned() else {
        return rejected(CastRejectReason::RealmTooLow);
    };
    let Some(position) = world.get::<Position>(caster).map(|p| p.get()) else {
        return rejected(CastRejectReason::InvalidTarget);
    };
    let mastery = mastery_level(world, caster, BaomaiSkillId::MountainShake);
    let profile = mountain_shake_profile(cultivation.realm, mastery);
    let outcome =
        match aoe_ground_wave(profile.qi_cost, profile.radius_blocks, profile.shock_damage) {
            Ok(outcome) => outcome,
            Err(_) => return rejected(CastRejectReason::InvalidTarget),
        };
    if cultivation.qi_current + f64::EPSILON < outcome.qi_spent {
        return rejected(CastRejectReason::QiInsufficient);
    }
    spend_qi(world, caster, outcome.qi_spent);
    set_slot_cooldown(
        world,
        caster,
        slot,
        now_tick.saturating_add(profile.cooldown_ticks),
    );
    let targets = targets_in_radius(world, caster, position, outcome.radius_blocks);
    for target in &targets {
        world.send_event(AttackIntent {
            attacker: caster,
            target: Some(*target),
            issued_at_tick: now_tick,
            reach: AttackReach {
                base: outcome.radius_blocks,
                step_bonus: 0.0,
                max: outcome.radius_blocks,
            },
            qi_invest: outcome.shock_damage,
            wound_kind: WoundKind::Concussion,
            source: AttackSource::BurstMeridian,
            debug_command: None,
        });
        world.send_event(ApplyStatusEffectIntent {
            target: *target,
            kind: StatusEffectKind::Stunned,
            magnitude: 1.0,
            duration_ticks: outcome.stagger_ticks,
            issued_at_tick: now_tick,
        });
    }
    emit_skill_event(
        world,
        BaomaiSkillEvent {
            skill: BaomaiSkillId::MountainShake,
            caster,
            target: None,
            tick: now_tick,
            qi_invested: outcome.qi_spent,
            damage: outcome.shock_damage,
            radius_blocks: Some(outcome.radius_blocks),
            blood_multiplier: active_blood_multiplier(world, caster, now_tick),
            flow_rate_multiplier: active_flow_multiplier(world, caster, now_tick),
            meridian_dependencies: MOUNTAIN_SHAKE_DEPS.to_vec(),
        },
    );
    world.send_event(MountainShakeEvent {
        caster,
        affected: targets,
        tick: now_tick,
        qi_spent: outcome.qi_spent,
        radius_blocks: outcome.radius_blocks,
        shock_damage: outcome.shock_damage,
    });
    record_overload(
        world,
        caster,
        BaomaiSkillId::MountainShake,
        &MOUNTAIN_SHAKE_DEPS,
        active_flow_multiplier(world, caster, now_tick),
        now_tick,
    );
    record_qi_transfer(
        world,
        caster,
        BaomaiSkillId::MountainShake,
        outcome.qi_spent,
    );
    record_practice(world, caster, BaomaiSkillId::MountainShake);
    emit_particle(world, position, "bong:ground_wave_dust", "#A8885A", 1.0, 28);
    CastResult::Started {
        cooldown_ticks: profile.cooldown_ticks,
        anim_duration_ticks: profile.cast_ticks,
    }
}

pub fn cast_blood_burn(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    _target: Option<Entity>,
) -> CastResult {
    let now_tick = current_tick(world);
    if is_slot_on_cooldown(world, caster, slot, now_tick) {
        return rejected(CastRejectReason::OnCooldown);
    }
    if let Err(reason) = check_static_deps(world, caster, BAOMAI_BLOOD_BURN_SKILL_ID) {
        return rejected(reason);
    }
    let Some(cultivation) = world.get::<Cultivation>(caster).cloned() else {
        return rejected(CastRejectReason::RealmTooLow);
    };
    let mastery = mastery_level(world, caster, BaomaiSkillId::BloodBurn);
    let profile = blood_burn_profile(cultivation.realm, mastery);
    let Some(wounds) = world.get::<Wounds>(caster).cloned() else {
        return rejected(CastRejectReason::InvalidTarget);
    };
    if wounds.health_current + f32::EPSILON < profile.hp_burn {
        return rejected(CastRejectReason::InvalidTarget);
    }
    let outcome = match blood_burn_conversion(
        wounds.health_current,
        profile.hp_burn,
        profile.qi_multiplier,
        profile.duration_ticks,
    ) {
        Ok(outcome) => outcome,
        Err(_) => return rejected(CastRejectReason::InvalidTarget),
    };
    if let Some(mut wounds) = world.get_mut::<Wounds>(caster) {
        let severity = (outcome.hp_burned / wounds.health_max.max(1.0)).clamp(0.0, 1.0);
        wounds.health_current = (wounds.health_current - outcome.hp_burned).max(0.0);
        wounds.entries.push(Wound {
            location: BodyPart::ArmL,
            kind: WoundKind::Cut,
            severity,
            bleeding_per_sec: 0.0,
            created_at_tick: now_tick,
            inflicted_by: Some("baomai:blood_burn".to_string()),
        });
    }
    if outcome.ends_in_near_death {
        if let Some(mut lifecycle) = world.get_mut::<Lifecycle>(caster) {
            lifecycle.enter_near_death(now_tick);
        }
        apply_blood_burn_contamination(world, caster, now_tick);
    } else {
        world.entity_mut(caster).insert(BloodBurnActive {
            started_at_tick: now_tick,
            active_until_tick: now_tick.saturating_add(outcome.duration_ticks),
            hp_burned: outcome.hp_burned,
            qi_multiplier: outcome.qi_multiplier,
            cooldown_until_tick: now_tick.saturating_add(profile.cooldown_ticks),
        });
    }
    set_slot_cooldown(
        world,
        caster,
        slot,
        now_tick.saturating_add(profile.cooldown_ticks),
    );
    world.send_event(BloodBurnEvent {
        caster,
        tick: now_tick,
        hp_burned: outcome.hp_burned,
        qi_multiplier: outcome.qi_multiplier,
        active_until_tick: now_tick.saturating_add(outcome.duration_ticks),
        ended_in_near_death: outcome.ends_in_near_death,
    });
    record_practice(world, caster, BaomaiSkillId::BloodBurn);
    if let Some(position) = world.get::<Position>(caster).map(|p| p.get()) {
        emit_particle(
            world,
            position,
            "bong:blood_burn_crimson",
            "#C0182B",
            1.0,
            16,
        );
    }
    CastResult::Started {
        cooldown_ticks: profile.cooldown_ticks,
        anim_duration_ticks: 10,
    }
}

pub fn cast_disperse(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    _target: Option<Entity>,
) -> CastResult {
    let now_tick = current_tick(world);
    if is_slot_on_cooldown(world, caster, slot, now_tick) {
        return rejected(CastRejectReason::OnCooldown);
    }
    let Some(cultivation) = world.get::<Cultivation>(caster).cloned() else {
        return rejected(CastRejectReason::RealmTooLow);
    };
    let mastery = mastery_level(world, caster, BaomaiSkillId::Disperse);
    let profile = disperse_profile(cultivation.realm, mastery);
    if let Err(reason) = check_static_deps(world, caster, BAOMAI_DISPERSE_SKILL_ID) {
        return rejected(reason);
    }
    let outcome = match body_transcendence(
        cultivation.qi_max,
        profile.qi_max_loss_ratio,
        profile.flow_rate_multiplier,
        profile.duration_ticks,
    ) {
        Ok(outcome) => outcome,
        Err(_) => return rejected(CastRejectReason::InvalidTarget),
    };
    apply_qi_max_loss(world, caster, cultivation.qi_max, profile.qi_max_loss_ratio);
    if profile.has_transcendence {
        apply_transcendence_window(
            world,
            caster,
            now_tick,
            outcome.flow_rate_multiplier,
            outcome,
        );
        let recent_count = {
            if world.get::<DisperseCastHistory>(caster).is_none() {
                world
                    .entity_mut(caster)
                    .insert(DisperseCastHistory::default());
            }
            world
                .get_mut::<DisperseCastHistory>(caster)
                .map(|mut history| {
                    history.record_and_count_recent(now_tick, DISPERSE_JUEBI_WINDOW_TICKS)
                })
                .unwrap_or(0)
        };
        if cultivation.realm == Realm::Void && recent_count >= 3 {
            world.send_event(JueBiTriggerEvent {
                entity: caster,
                source: JueBiTriggerSource::BaomaiDisperse,
                delay_ticks: 0,
                triggered_at_tick: now_tick,
                epicenter: world.get::<Position>(caster).map(|p| {
                    let pos = p.get();
                    [pos.x, pos.y, pos.z]
                }),
            });
        }
    }
    set_slot_cooldown(
        world,
        caster,
        slot,
        now_tick.saturating_add(profile.duration_ticks.max(20)),
    );
    world.send_event(DispersedQiEvent {
        caster,
        tick: now_tick,
        qi_max_before: outcome.qi_max_before,
        qi_max_after: outcome.qi_max_after,
        flow_rate_multiplier: outcome.flow_rate_multiplier,
        active_until_tick: profile
            .has_transcendence
            .then_some(now_tick.saturating_add(profile.duration_ticks)),
        failed_reason: (!profile.has_transcendence).then_some("凡躯不应".to_string()),
    });
    record_practice(world, caster, BaomaiSkillId::Disperse);
    if profile.has_transcendence {
        if let Some(position) = world.get::<Position>(caster).map(|p| p.get()) {
            emit_particle(
                world,
                position,
                "bong:body_transcendence_pillar",
                "#F5D36A",
                1.0,
                32,
            );
        }
    }
    CastResult::Started {
        cooldown_ticks: profile.duration_ticks.max(20),
        anim_duration_ticks: profile.duration_ticks.min(u64::from(u32::MAX)) as u32,
    }
}

fn apply_blood_burn_contamination(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    now_tick: u64,
) {
    let Some(cultivation) = world.get::<Cultivation>(caster).cloned() else {
        return;
    };
    if let Some(mut contamination) = world.get_mut::<Contamination>(caster) {
        contamination.entries.push(ContamSource {
            amount: cultivation.qi_max.max(0.0) * 0.05,
            color: ColorKind::Violent,
            meridian_id: None,
            attacker_id: Some("baomai:blood_burn".to_string()),
            introduced_at: now_tick,
        });
    }
}

fn current_tick(world: &bevy_ecs::world::World) -> u64 {
    world
        .get_resource::<CombatClock>()
        .map(|clock| clock.tick)
        .unwrap_or_default()
}

fn is_slot_on_cooldown(
    world: &bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    now_tick: u64,
) -> bool {
    world
        .get::<SkillBarBindings>(caster)
        .is_some_and(|bindings| bindings.is_on_cooldown(slot, now_tick))
}

fn set_slot_cooldown(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    until_tick: u64,
) {
    if let Some(mut bindings) = world.get_mut::<SkillBarBindings>(caster) {
        bindings.set_cooldown(slot, until_tick);
    }
}

fn check_static_deps(
    world: &bevy_ecs::world::World,
    caster: Entity,
    skill_id: &str,
) -> Result<(), CastRejectReason> {
    let Some(table) = world.get_resource::<SkillMeridianDependencies>() else {
        return Ok(());
    };
    check_meridian_dependencies(
        table.lookup(skill_id),
        world.get::<MeridianSeveredPermanent>(caster),
    )
    .map_err(|id| CastRejectReason::MeridianSevered(Some(id)))
}

fn mastery_level(world: &bevy_ecs::world::World, caster: Entity, skill: BaomaiSkillId) -> u8 {
    world
        .get::<BaomaiMastery>(caster)
        .map(|mastery| mastery.level(skill))
        .unwrap_or_default()
}

fn active_blood_multiplier(world: &bevy_ecs::world::World, caster: Entity, tick: u64) -> f32 {
    world
        .get::<BloodBurnActive>(caster)
        .filter(|active| active.is_active_at(tick))
        .map(|active| active.qi_multiplier)
        .unwrap_or(1.0)
}

fn active_flow_multiplier(world: &bevy_ecs::world::World, caster: Entity, tick: u64) -> f64 {
    world
        .get::<BodyTranscendence>(caster)
        .filter(|active| active.is_active_at(tick))
        .map(|active| active.flow_rate_multiplier)
        .unwrap_or(1.0)
}

fn heavy_color_multiplier(world: &bevy_ecs::world::World, caster: Entity) -> f64 {
    let Some(log) = world.get::<PracticeLog>(caster) else {
        return 1.0;
    };
    let total = log.total();
    if total <= f64::EPSILON {
        return 1.0;
    }
    let heavy_ratio = log.weights.get(&ColorKind::Heavy).copied().unwrap_or(0.0) / total;
    if heavy_ratio >= 0.30 {
        1.05
    } else {
        1.0
    }
}

fn spend_qi(world: &mut bevy_ecs::world::World, caster: Entity, amount: f64) {
    if amount <= f64::EPSILON {
        return;
    }
    if let Some(mut cultivation) = world.get_mut::<Cultivation>(caster) {
        cultivation.qi_current = (cultivation.qi_current - amount).clamp(0.0, cultivation.qi_max);
    }
}

fn apply_qi_max_loss(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    qi_max_before: f64,
    loss_ratio: f64,
) {
    if let Some(mut cultivation) = world.get_mut::<Cultivation>(caster) {
        cultivation.qi_max = (qi_max_before * (1.0 - loss_ratio.clamp(0.0, 1.0))).max(0.0);
        cultivation.qi_current = cultivation.qi_current.clamp(0.0, cultivation.qi_max);
    }
}

fn apply_transcendence_window(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    now_tick: u64,
    flow_multiplier: f64,
    outcome: crate::qi_physics::BodyTranscendenceOutcome,
) {
    let original_flow_rates = if let Some(mut meridians) = world.get_mut::<MeridianSystem>(caster) {
        let mut original = Vec::with_capacity(MeridianId::ALL.len());
        for id in MeridianId::ALL {
            let meridian = meridians.get_mut(id);
            original.push((id, meridian.flow_rate));
            meridian.flow_rate *= flow_multiplier;
        }
        original
    } else {
        Vec::new()
    };
    if let Some(mut bindings) = world.get_mut::<SkillBarBindings>(caster) {
        bindings.cooldown_until_tick = [0; SkillBarBindings::SLOT_COUNT];
    }
    world.entity_mut(caster).insert(BodyTranscendence {
        started_at_tick: now_tick,
        active_until_tick: now_tick.saturating_add(outcome.duration_ticks),
        flow_rate_multiplier: outcome.flow_rate_multiplier,
        qi_max_lost: outcome.qi_max_lost,
        cooldowns_reset: true,
        overload_tear_suppressed: true,
        original_flow_rates,
    });
}

fn record_practice(world: &mut bevy_ecs::world::World, caster: Entity, skill: BaomaiSkillId) {
    if let Some(mut log) = world.get_mut::<PracticeLog>(caster) {
        record_style_practice(&mut log, ColorKind::Heavy);
    }
    if let Some(mut mastery) = world.get_mut::<BaomaiMastery>(caster) {
        mastery.grant_cast_xp(skill);
    }
    world.send_event(SkillXpGain {
        char_entity: caster,
        skill: SkillId::Combat,
        amount: skill.practice_xp(),
        source: XpGainSource::Action {
            plan_id: "baomai_v3",
            action: skill.wire_kind(),
        },
    });
}

fn record_qi_transfer(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    skill: BaomaiSkillId,
    amount: f64,
) {
    if amount <= f64::EPSILON {
        return;
    }
    if let Ok(transfer) = QiTransfer::new(
        QiAccountId::player(format!("entity:{}", caster.to_bits())),
        QiAccountId::overflow(format!("baomai_v3:{}", skill.wire_kind())),
        amount,
        QiTransferReason::Collision,
    ) {
        world.send_event(transfer);
    }
}

fn record_overload(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    skill: BaomaiSkillId,
    deps: &[MeridianId],
    flow_multiplier: f64,
    tick: u64,
) {
    let severity = overload_severity(skill, flow_multiplier);
    if severity <= f64::EPSILON {
        return;
    }
    let total_severity = {
        if world.get::<MeridianRippleScar>(caster).is_none() {
            world
                .entity_mut(caster)
                .insert(MeridianRippleScar::default());
        }
        world
            .get_mut::<MeridianRippleScar>(caster)
            .map(|mut scar| {
                scar.severity = (scar.severity + severity).clamp(0.0, 1.0);
                scar.accumulated_overloads = scar.accumulated_overloads.saturating_add(1);
                scar.last_updated_tick = tick;
                scar.severity
            })
            .unwrap_or(severity)
    };
    let mut newly_severed = Vec::new();
    if let Some(mut meridians) = world.get_mut::<MeridianSystem>(caster) {
        for id in deps {
            let meridian = meridians.get_mut(*id);
            meridian.integrity = (meridian.integrity - severity).clamp(0.0, 1.0);
            meridian.cracks.push(MeridianCrack {
                severity,
                healing_progress: 0.0,
                cause: CrackCause::Overload,
                created_at: tick,
            });
            if meridian.integrity <= f64::EPSILON {
                newly_severed.push(*id);
            }
        }
    }
    for id in newly_severed {
        world.send_event(MeridianSeveredEvent {
            entity: caster,
            meridian_id: id,
            source: SeveredSource::OverloadTear,
            at_tick: tick,
        });
    }
    world.send_event(OverloadMeridianRippleEvent {
        caster,
        tick,
        skill,
        severity_delta: severity,
        total_severity,
        meridian_ids: deps.to_vec(),
    });
}

fn caster_target_positions(
    world: &bevy_ecs::world::World,
    caster: Entity,
    target: Entity,
) -> Option<(DVec3, DVec3)> {
    Some((
        world.get::<Position>(caster)?.get(),
        world.get::<Position>(target)?.get(),
    ))
}

fn targets_in_radius(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    center: DVec3,
    radius: f32,
) -> Vec<Entity> {
    let radius = f64::from(radius.max(0.0));
    let mut q = world.query::<(Entity, &Position)>();
    q.iter(world)
        .filter_map(|(entity, position)| {
            (entity != caster && position.get().distance(center) <= radius + f64::EPSILON)
                .then_some(entity)
        })
        .collect()
}

fn emit_skill_event(world: &mut bevy_ecs::world::World, event: BaomaiSkillEvent) {
    world.send_event(event);
}

fn emit_particle(
    world: &mut bevy_ecs::world::World,
    origin: DVec3,
    event_id: &str,
    color: &str,
    strength: f32,
    count: u16,
) {
    world.send_event(VfxEventRequest::new(
        origin,
        VfxEventPayloadV1::SpawnParticle {
            event_id: event_id.to_string(),
            origin: [origin.x, origin.y + 1.0, origin.z],
            direction: None,
            color: Some(color.to_string()),
            strength: Some(strength),
            count: Some(count),
            duration_ticks: Some(12),
        },
    ));
}

fn rejected(reason: CastRejectReason) -> CastResult {
    CastResult::Rejected { reason }
}
