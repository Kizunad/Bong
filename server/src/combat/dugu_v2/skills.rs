use valence::prelude::{bevy_ecs, DVec3, Entity, Event, Events, Position, UniqueId};

use crate::combat::components::{
    BodyPart, CastSource, Casting, SkillBarBindings, Wound, WoundKind, Wounds, TICKS_PER_SECOND,
};
use crate::combat::events::emit_death_event_if_lethal;
use crate::combat::CombatClock;
use crate::cultivation::components::{ColorKind, Cultivation, MeridianId, QiColor, Realm};
use crate::cultivation::dugu::DuguRevealedEvent;
use crate::cultivation::meridian::severed::{
    check_meridian_dependencies, MeridianSeveredPermanent, SkillMeridianDependencies,
};
use crate::cultivation::skill_registry::{CastRejectReason, CastResult, SkillRegistry};
use crate::cultivation::tribulation::{JueBiTriggerEvent, JueBiTriggerSource};
use crate::network::audio_event_emit::{AudioRecipient, PlaySoundRecipeRequest, AUDIO_BROADCAST_RADIUS};
use crate::network::cast_emit::current_unix_millis;
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::qi_physics::reverse_burst_all_marks;
use crate::schema::vfx_event::VfxEventPayloadV1;

use super::events::{
    DuguSelfRevealedEvent, DuguSkillId, DuguSkillVisual, EclipseNeedleEvent, PenetrateChainEvent,
    ReverseTriggeredEvent, SelfCureProgressEvent, ShroudActivatedEvent, TaintTier,
};
use super::physics::{
    defender_resistance, dirty_qi_collision, eclipse_effect, fake_qi_color_for_realm,
    penetrate_spec, reveal_probability, self_cure_gain_percent, shroud_spec, skill_spec,
    SELF_CURE_REVEAL_THRESHOLD_PERCENT, SELF_CURE_SOFT_CAP_PERCENT,
};
use super::state::{DuguState, ReverseAftermathCloud, ShroudActive, TaintMark};

pub const DUGU_ECLIPSE_SKILL_ID: &str = "dugu.eclipse";
pub const DUGU_SELF_CURE_SKILL_ID: &str = "dugu.self_cure";
pub const DUGU_PENETRATE_SKILL_ID: &str = "dugu.penetrate";
pub const DUGU_SHROUD_SKILL_ID: &str = "dugu.shroud";
pub const DUGU_REVERSE_SKILL_ID: &str = "dugu.reverse";
pub const DUGU_REQUIRED_MERIDIANS: [MeridianId; 1] = [MeridianId::Liver];

const TEMP_TAINT_DURATION_TICKS: u64 = 24 * 60 * 60 * TICKS_PER_SECOND;
const SELF_CURE_HOURS_PER_CAST: f32 = 1.0;
const JUEBI_REVERSE_DELAY_TICKS: u64 = 30 * TICKS_PER_SECOND;
const REVERSE_QI_PER_TARGET: f64 = 30.0;

pub fn register_skills(registry: &mut SkillRegistry) {
    registry.register(DUGU_ECLIPSE_SKILL_ID, cast_eclipse);
    registry.register(DUGU_SELF_CURE_SKILL_ID, cast_self_cure);
    registry.register(DUGU_PENETRATE_SKILL_ID, cast_penetrate);
    registry.register(DUGU_SHROUD_SKILL_ID, cast_shroud);
    registry.register(DUGU_REVERSE_SKILL_ID, cast_reverse);
}

pub fn declare_meridian_dependencies(dependencies: &mut SkillMeridianDependencies) {
    for skill_id in [
        DUGU_ECLIPSE_SKILL_ID,
        DUGU_SELF_CURE_SKILL_ID,
        DUGU_PENETRATE_SKILL_ID,
        DUGU_SHROUD_SKILL_ID,
        DUGU_REVERSE_SKILL_ID,
    ] {
        dependencies.declare(skill_id, DUGU_REQUIRED_MERIDIANS.to_vec());
    }
}

pub fn cast_eclipse(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
) -> CastResult {
    resolve_dugu_v2_skill(world, caster, slot, target, DuguSkillId::Eclipse)
}

pub fn cast_self_cure(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
) -> CastResult {
    resolve_dugu_v2_skill(world, caster, slot, target, DuguSkillId::SelfCure)
}

pub fn cast_penetrate(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
) -> CastResult {
    resolve_dugu_v2_skill(world, caster, slot, target, DuguSkillId::Penetrate)
}

pub fn cast_shroud(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
) -> CastResult {
    resolve_dugu_v2_skill(world, caster, slot, target, DuguSkillId::Shroud)
}

pub fn cast_reverse(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
) -> CastResult {
    resolve_dugu_v2_skill(world, caster, slot, target, DuguSkillId::Reverse)
}

pub fn resolve_dugu_v2_skill(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
    skill: DuguSkillId,
) -> CastResult {
    let now_tick = now_tick(world);
    if is_on_cooldown(world, caster, slot, now_tick) {
        return rejected(CastRejectReason::OnCooldown);
    }
    let Some(cultivation) = world.get::<Cultivation>(caster).cloned() else {
        return rejected(CastRejectReason::QiInsufficient);
    };
    let spec = skill_spec(skill);
    if cultivation.qi_current + f64::EPSILON < spec.qi_cost {
        return rejected(CastRejectReason::QiInsufficient);
    }
    if let Err(reason) = check_static_meridian_dependencies(world, caster, skill.as_str()) {
        return rejected(reason);
    }

    let result = match skill {
        DuguSkillId::Eclipse => apply_eclipse(world, caster, target, &cultivation, now_tick),
        DuguSkillId::SelfCure => apply_self_cure(world, caster, &cultivation, now_tick),
        DuguSkillId::Penetrate => apply_penetrate(world, caster, target, &cultivation, now_tick),
        DuguSkillId::Shroud => apply_shroud(world, caster, &cultivation, now_tick),
        DuguSkillId::Reverse => apply_reverse(world, caster, target, &cultivation, now_tick),
    };

    if let Err(reason) = result {
        return rejected(reason);
    }

    if let Some(mut cultivation) = world.get_mut::<Cultivation>(caster) {
        cultivation.qi_current =
            (cultivation.qi_current - spec.qi_cost).clamp(0.0, cultivation.qi_max);
    }
    insert_cast(
        world,
        caster,
        slot,
        skill,
        spec.cooldown_ticks,
        spec.cast_ticks,
        now_tick,
    );
    CastResult::Started {
        cooldown_ticks: spec.cooldown_ticks,
        anim_duration_ticks: spec.cast_ticks,
    }
}

fn apply_eclipse(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    target: Option<Entity>,
    _caster_cultivation: &Cultivation,
    now_tick: u64,
) -> Result<(), CastRejectReason> {
    let Some(target) = target else {
        return Err(CastRejectReason::InvalidTarget);
    };
    let Some(target_cultivation) = world.get::<Cultivation>(target).cloned() else {
        return Err(CastRejectReason::InvalidTarget);
    };
    let self_cure_percent = world
        .get::<DuguState>(caster)
        .map(|state| state.insidious_color_percent)
        .unwrap_or(0.0);
    let effect = eclipse_effect(target_cultivation.realm, self_cure_percent);
    let collision = dirty_qi_collision(
        f64::from(effect.qi_loss.max(1.0)),
        defender_resistance(&target_cultivation),
        distance_between(world, caster, target),
    );

    apply_damage(
        world,
        caster,
        target,
        DuguSkillId::Eclipse,
        effect.hp_loss,
        now_tick,
    );
    let mut mark_to_insert = None;
    if let Some(mut cultivation) = world.get_mut::<Cultivation>(target) {
        cultivation.qi_current = (cultivation.qi_current - f64::from(effect.qi_loss)).max(0.0);
        if effect.tier == TaintTier::Temporary {
            let loss =
                (cultivation.qi_max * f64::from(effect.temporary_qi_max_loss_fraction)) as f32;
            cultivation.qi_max = (cultivation.qi_max - f64::from(loss)).max(0.0);
            mark_to_insert = Some((
                effect.tier,
                collision.effective_hit.max(1.0),
                Some(now_tick.saturating_add(TEMP_TAINT_DURATION_TICKS)),
                loss,
                0.0,
                collision.returned_zone_qi,
            ));
        } else if effect.tier == TaintTier::Permanent {
            mark_to_insert = Some((
                effect.tier,
                collision.effective_hit.max(1.0),
                None,
                0.0,
                effect.permanent_decay_rate_per_min,
                collision.returned_zone_qi,
            ));
        }
    }
    if let Some((tier, intensity, expires_at_tick, temp_loss, decay, returned_zone_qi)) =
        mark_to_insert
    {
        insert_taint_mark(
            world,
            caster,
            target,
            tier,
            intensity,
            expires_at_tick,
            temp_loss,
            decay,
            returned_zone_qi,
            now_tick,
        );
    }

    let reveal_probability = emit_reveal_if_needed(world, caster, target, now_tick);
    send_event_if_present(
        world,
        EclipseNeedleEvent {
            caster,
            target,
            target_realm: target_cultivation.realm,
            tier: effect.tier,
            injected_qi: collision.injected_qi,
            hp_loss: effect.hp_loss,
            qi_loss: effect.qi_loss,
            qi_max_loss: if effect.tier == TaintTier::Temporary {
                target_cultivation.qi_max as f32 * effect.temporary_qi_max_loss_fraction
            } else {
                0.0
            },
            permanent_decay_rate_per_min: effect.permanent_decay_rate_per_min,
            returned_zone_qi: collision.returned_zone_qi,
            reveal_probability,
            tick: now_tick,
            visual: visual_for(DuguSkillId::Eclipse),
        },
    );
    if let Some(pos) = world.get::<Position>(caster).map(|p| p.get()) {
        emit_vfx(world, pos, "bong:poison_mist", "#44AA44", 0.85, 10, 40);
        emit_audio(world, "dugu_needle_hiss", pos);
        emit_anim(world, caster, "bong:dugu_needle_throw");
    }
    Ok(())
}

fn apply_self_cure(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    _cultivation: &Cultivation,
    now_tick: u64,
) -> Result<(), CastRejectReason> {
    let day = now_tick / (24 * 60 * 60 * TICKS_PER_SECOND);
    let mut state = world.get::<DuguState>(caster).cloned().unwrap_or_default();
    state.reset_daily_if_needed(day);
    let gain = self_cure_gain_percent(
        state.insidious_color_percent,
        SELF_CURE_HOURS_PER_CAST,
        state.self_cure_hours_today,
    );
    if gain <= f32::EPSILON {
        return Err(CastRejectReason::InRecovery);
    }
    state.self_cure_hours_today += SELF_CURE_HOURS_PER_CAST;
    state.insidious_color_percent =
        (state.insidious_color_percent + gain).clamp(0.0, SELF_CURE_SOFT_CAP_PERCENT);
    state.morphology_percent = state.morphology_percent.max(state.insidious_color_percent);
    let newly_revealed =
        !state.self_revealed && state.morphology_percent >= SELF_CURE_REVEAL_THRESHOLD_PERCENT;
    state.self_revealed |= newly_revealed;

    let mut color = world.get::<QiColor>(caster).cloned().unwrap_or_default();
    color.main = ColorKind::Insidious;
    color.lock_permanent(ColorKind::Insidious);
    world.entity_mut(caster).insert((state.clone(), color));

    if newly_revealed {
        send_event_if_present(
            world,
            DuguSelfRevealedEvent {
                caster,
                insidious_color_percent: state.insidious_color_percent,
                morphology_percent: state.morphology_percent,
                tick: now_tick,
            },
        );
    }
    send_event_if_present(
        world,
        SelfCureProgressEvent {
            caster,
            hours_used: SELF_CURE_HOURS_PER_CAST,
            daily_hours_after: state.self_cure_hours_today,
            gain_percent: gain,
            insidious_color_percent: state.insidious_color_percent,
            morphology_percent: state.morphology_percent,
            self_revealed: state.self_revealed,
            tick: now_tick,
        },
    );
    if let Some(pos) = world.get::<Position>(caster).map(|p| p.get()) {
        emit_audio(world, "dugu_self_cure_drink", pos);
        emit_anim(world, caster, "bong:dugu_self_cure_pose");
    }
    Ok(())
}

fn apply_penetrate(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    target: Option<Entity>,
    cultivation: &Cultivation,
    now_tick: u64,
) -> Result<(), CastRejectReason> {
    let Some(target) = target else {
        return Err(CastRejectReason::InvalidTarget);
    };
    let Some(mark) = world.get::<TaintMark>(target).cloned() else {
        return Err(CastRejectReason::InvalidTarget);
    };
    let spec = penetrate_spec(cultivation.realm);
    let affected = affected_taint_targets(world, caster, target, spec.radius_blocks, false);
    let affected_count = affected.len().max(1) as u32;
    for entity in affected {
        apply_damage(
            world,
            caster,
            entity,
            DuguSkillId::Penetrate,
            10.0 * spec.multiplier,
            now_tick,
        );
        if let Some(mut target_cultivation) = world.get_mut::<Cultivation>(entity) {
            target_cultivation.qi_current =
                (target_cultivation.qi_current - f64::from(15.0 * spec.multiplier)).max(0.0);
        }
        if spec.extra_permanent_decay_rate_per_min > 0.0 {
            if let Some(mut taint) = world.get_mut::<TaintMark>(entity) {
                taint.tier = TaintTier::Permanent;
                taint.expires_at_tick = None;
                taint.permanent_decay_rate_per_min += spec.extra_permanent_decay_rate_per_min;
            }
        }
    }
    let target_tier = world
        .get::<TaintMark>(target)
        .map(|mark| mark.tier)
        .unwrap_or(mark.tier);
    let reveal_probability = emit_reveal_if_needed(world, caster, target, now_tick);
    send_event_if_present(
        world,
        PenetrateChainEvent {
            caster,
            target,
            taint_tier: target_tier,
            multiplier: spec.multiplier,
            affected_targets: affected_count,
            permanent_decay_rate_per_min: mark.permanent_decay_rate_per_min
                + spec.extra_permanent_decay_rate_per_min,
            reveal_probability,
            tick: now_tick,
            visual: visual_for(DuguSkillId::Penetrate),
        },
    );
    if let Some(pos) = world.get::<Position>(caster).map(|p| p.get()) {
        emit_vfx(world, pos, "bong:poison_mist", "#228B22", 1.0, 16, 60);
        emit_audio(world, "dugu_curse_cackle", pos);
        emit_anim(world, caster, "bong:dugu_pointing_curse");
    }
    Ok(())
}

fn apply_shroud(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    cultivation: &Cultivation,
    now_tick: u64,
) -> Result<(), CastRejectReason> {
    let spec = shroud_spec(cultivation.realm);
    let expires_at_tick = now_tick.saturating_add(spec.duration_ticks);
    world.entity_mut(caster).insert(ShroudActive {
        skill: DuguSkillId::Shroud,
        strength: spec.strength,
        fake_qi_color: fake_qi_color_for_realm(cultivation.realm),
        started_at_tick: now_tick,
        expires_at_tick,
        permanent_until_cancelled: spec.permanent_until_cancelled,
        maintain_qi_per_tick: 0.5 / TICKS_PER_SECOND as f64,
    });
    send_event_if_present(
        world,
        ShroudActivatedEvent {
            caster,
            strength: spec.strength,
            expires_at_tick,
            tick: now_tick,
            visual: visual_for(DuguSkillId::Shroud),
        },
    );
    if let Some(pos) = world.get::<Position>(caster).map(|p| p.get()) {
        emit_audio(world, "dugu_cast", pos);
        emit_anim(world, caster, "bong:dugu_shroud_activate");
    }
    Ok(())
}

fn apply_reverse(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    target: Option<Entity>,
    cultivation: &Cultivation,
    now_tick: u64,
) -> Result<(), CastRejectReason> {
    if cultivation.realm != Realm::Void {
        return Err(CastRejectReason::RealmTooLow);
    }
    let targets = match target {
        Some(target) => affected_taint_targets(world, caster, target, 0.0, true),
        None => all_permanent_taint_targets(world, caster),
    };
    if targets.is_empty() {
        return Err(CastRejectReason::InvalidTarget);
    }
    let extra_qi_cost = REVERSE_QI_PER_TARGET * targets.len() as f64;
    if cultivation.qi_current + f64::EPSILON
        < skill_spec(DuguSkillId::Reverse).qi_cost + extra_qi_cost
    {
        return Err(CastRejectReason::QiInsufficient);
    }
    let intensities: Vec<f64> = targets
        .iter()
        .filter_map(|entity| {
            world
                .get::<TaintMark>(*entity)
                .map(|mark| f64::from(mark.intensity))
        })
        .collect();
    let burst = reverse_burst_all_marks(intensities);
    for entity in &targets {
        apply_damage(
            world,
            caster,
            *entity,
            DuguSkillId::Reverse,
            burst.burst_damage as f32,
            now_tick,
        );
        if let Some(mut cultivation) = world.get_mut::<Cultivation>(*entity) {
            cultivation.qi_current = 0.0;
        }
        world.entity_mut(*entity).remove::<TaintMark>();
    }
    let center = target
        .and_then(|entity| world.get::<Position>(entity).map(|pos| pos.get()))
        .or_else(|| world.get::<Position>(caster).map(|pos| pos.get()))
        .unwrap_or(DVec3::ZERO);
    world.entity_mut(caster).insert(ReverseAftermathCloud {
        caster,
        intensity: 1.0,
        radius_blocks: 10.0,
        expires_at_tick: now_tick.saturating_add(30 * TICKS_PER_SECOND),
    });
    if let Some(mut caster_cultivation) = world.get_mut::<Cultivation>(caster) {
        caster_cultivation.qi_current =
            (caster_cultivation.qi_current - extra_qi_cost).clamp(0.0, caster_cultivation.qi_max);
    }
    let mut state = world.get::<DuguState>(caster).cloned().unwrap_or_default();
    state.insidious_color_percent = (state.insidious_color_percent + 5.0 * targets.len() as f32)
        .clamp(0.0, SELF_CURE_SOFT_CAP_PERCENT);
    state.morphology_percent = state.morphology_percent.max(state.insidious_color_percent);
    state.self_revealed |= state.morphology_percent >= SELF_CURE_REVEAL_THRESHOLD_PERCENT;
    world.entity_mut(caster).insert(state);
    send_event_if_present(
        world,
        ReverseTriggeredEvent {
            caster,
            affected_targets: targets.len() as u32,
            burst_damage: burst.burst_damage as f32,
            returned_zone_qi: burst.returned_zone_qi as f32,
            juebi_delay_ticks: Some(JUEBI_REVERSE_DELAY_TICKS),
            tick: now_tick,
            center,
            visual: visual_for(DuguSkillId::Reverse),
        },
    );
    send_event_if_present(
        world,
        JueBiTriggerEvent {
            entity: caster,
            source: JueBiTriggerSource::DuguReverse,
            delay_ticks: JUEBI_REVERSE_DELAY_TICKS,
            triggered_at_tick: now_tick,
            epicenter: Some([center.x, center.y, center.z]),
        },
    );
    emit_vfx(world, center, "bong:poison_mist", "#006400", 1.0, 24, 80);
    emit_audio(world, "dugu_poison_signature", center);
    emit_anim(world, caster, "bong:dugu_pointing_curse");
    emit_reveal_if_needed(world, caster, target.unwrap_or(caster), now_tick);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn insert_taint_mark(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    target: Entity,
    tier: TaintTier,
    intensity: f32,
    expires_at_tick: Option<u64>,
    temporary_qi_max_loss: f32,
    permanent_decay_rate_per_min: f32,
    returned_zone_qi: f32,
    now_tick: u64,
) {
    world.entity_mut(target).insert(TaintMark {
        caster,
        intensity,
        since_tick: now_tick,
        expires_at_tick,
        tier,
        temporary_qi_max_loss,
        permanent_decay_rate_per_min,
        returned_zone_qi,
    });
}

fn apply_damage(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    target: Entity,
    skill: DuguSkillId,
    hp_loss: f32,
    now_tick: u64,
) {
    if hp_loss <= f32::EPSILON {
        return;
    }
    let inflicted_by = Some(format!("entity:{}", caster.to_bits()));
    let mut death_state = None;
    if let Some(mut wounds) = world.get_mut::<Wounds>(target) {
        let was_alive = wounds.health_current > 0.0;
        let severity = (hp_loss / wounds.health_max.max(1.0)).clamp(0.0, 1.0);
        wounds.health_current = (wounds.health_current - hp_loss).max(0.0);
        wounds.entries.push(Wound {
            location: BodyPart::Chest,
            kind: WoundKind::Pierce,
            severity,
            bleeding_per_sec: 0.0,
            created_at_tick: now_tick,
            inflicted_by,
        });
        death_state = Some((was_alive, wounds.health_current));
    }
    if let Some((was_alive, health_current)) = death_state {
        emit_death_event_if_lethal(
            world,
            was_alive,
            health_current,
            target,
            format!("{}:entity:{}", skill.as_str(), caster.to_bits()),
            Some(caster),
            None,
            now_tick,
        );
    }
}

fn emit_reveal_if_needed(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    target: Entity,
    now_tick: u64,
) -> f32 {
    let Some(caster_cultivation) = world.get::<Cultivation>(caster) else {
        return 0.0;
    };
    let Some(target_cultivation) = world.get::<Cultivation>(target) else {
        return 0.0;
    };
    let shroud = world
        .get::<ShroudActive>(caster)
        .map(|active| active.strength)
        .unwrap_or(0.0);
    let distance = distance_between(world, caster, target) as f32;
    let probability = reveal_probability(
        caster_cultivation.realm,
        shroud,
        distance,
        target_cultivation.realm,
    );
    if probability <= 0.0 {
        return probability;
    }
    let roll = deterministic_roll(caster, target, now_tick);
    if roll > probability {
        return probability;
    }
    let at_position = world
        .get::<Position>(target)
        .map(|pos| {
            let p = pos.get();
            [p.x, p.y, p.z]
        })
        .unwrap_or([0.0, 0.0, 0.0]);
    send_event_if_present(
        world,
        DuguRevealedEvent {
            revealed_player: caster,
            witness: target,
            witness_realm: target_cultivation.realm,
            at_position,
            at_tick: now_tick,
        },
    );
    probability
}

fn affected_taint_targets(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    target: Entity,
    radius_blocks: f32,
    permanent_only: bool,
) -> Vec<Entity> {
    if radius_blocks == 0.0 {
        return world
            .get::<TaintMark>(target)
            .filter(|mark| mark.caster == caster && (!permanent_only || mark.is_permanent()))
            .map(|_| vec![target])
            .unwrap_or_default();
    }
    let origin = world
        .get::<Position>(target)
        .map(|pos| pos.get())
        .unwrap_or(DVec3::ZERO);
    let mut query = world.query::<(Entity, &TaintMark, Option<&Position>)>();
    query
        .iter(world)
        .filter_map(|(entity, mark, position)| {
            if mark.caster != caster || (permanent_only && !mark.is_permanent()) {
                return None;
            }
            if radius_blocks.is_finite() {
                let distance = position
                    .map(|pos| pos.get().distance(origin))
                    .unwrap_or(f64::INFINITY);
                if distance > f64::from(radius_blocks) {
                    return None;
                }
            }
            Some(entity)
        })
        .collect()
}

fn all_permanent_taint_targets(world: &mut bevy_ecs::world::World, caster: Entity) -> Vec<Entity> {
    let mut query = world.query::<(Entity, &TaintMark)>();
    query
        .iter(world)
        .filter_map(|(entity, mark)| {
            (mark.caster == caster && mark.is_permanent()).then_some(entity)
        })
        .collect()
}

fn insert_cast(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    skill: DuguSkillId,
    cooldown_ticks: u64,
    cast_ticks: u32,
    now_tick: u64,
) {
    if let Some(mut bindings) = world.get_mut::<SkillBarBindings>(caster) {
        bindings.set_cooldown(slot, now_tick.saturating_add(cooldown_ticks));
    }
    let start_position = world
        .get::<Position>(caster)
        .map(|position| position.get())
        .unwrap_or(DVec3::ZERO);
    world.entity_mut(caster).insert(Casting {
        source: CastSource::SkillBar,
        slot,
        started_at_tick: now_tick,
        duration_ticks: u64::from(cast_ticks),
        started_at_ms: current_unix_millis(),
        duration_ms: cast_ticks.saturating_mul(50),
        bound_instance_id: None,
        start_position,
        complete_cooldown_ticks: cooldown_ticks,
        skill_id: Some(skill.as_str().to_string()),
        skill_config: None,
    });
}

fn is_on_cooldown(world: &bevy_ecs::world::World, caster: Entity, slot: u8, now_tick: u64) -> bool {
    world
        .get::<SkillBarBindings>(caster)
        .is_some_and(|bindings| bindings.is_on_cooldown(slot, now_tick))
}

fn distance_between(world: &bevy_ecs::world::World, a: Entity, b: Entity) -> f64 {
    match (world.get::<Position>(a), world.get::<Position>(b)) {
        (Some(a), Some(b)) => a.get().distance(b.get()),
        _ => 1.0,
    }
}

fn check_static_meridian_dependencies(
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

fn now_tick(world: &bevy_ecs::world::World) -> u64 {
    world
        .get_resource::<CombatClock>()
        .map(|clock| clock.tick)
        .unwrap_or_default()
}

fn deterministic_roll(caster: Entity, target: Entity, tick: u64) -> f32 {
    let mut x = caster.to_bits() ^ target.to_bits().rotate_left(17) ^ tick.rotate_left(7);
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51afd7ed558ccd);
    x ^= x >> 33;
    (x % 10_000) as f32 / 10_000.0
}

fn send_event_if_present<T: Event>(world: &mut bevy_ecs::world::World, event: T) {
    if let Some(mut events) = world.get_resource_mut::<Events<T>>() {
        events.send(event);
    }
}

fn rejected(reason: CastRejectReason) -> CastResult {
    CastResult::Rejected { reason }
}

pub(super) fn emit_vfx(
    world: &mut bevy_ecs::world::World,
    origin: DVec3,
    event_id: &str,
    color: &str,
    strength: f32,
    count: u16,
    duration_ticks: u16,
) {
    if let Some(mut events) = world.get_resource_mut::<Events<VfxEventRequest>>() {
        events.send(VfxEventRequest::new(
            origin,
            VfxEventPayloadV1::SpawnParticle {
                event_id: event_id.to_string(),
                origin: [origin.x, origin.y + 1.0, origin.z],
                direction: None,
                color: Some(color.to_string()),
                strength: Some(strength.clamp(0.0, 1.0)),
                count: Some(count),
                duration_ticks: Some(duration_ticks),
            },
        ));
    }
}

pub(super) fn emit_audio(world: &mut bevy_ecs::world::World, recipe: &str, origin: DVec3) {
    if let Some(mut events) = world.get_resource_mut::<Events<PlaySoundRecipeRequest>>() {
        events.send(PlaySoundRecipeRequest {
            recipe_id: recipe.to_string(),
            instance_id: 0,
            pos: None,
            flag: None,
            volume_mul: 1.0,
            pitch_shift: 0.0,
            recipient: AudioRecipient::Radius {
                origin,
                radius: AUDIO_BROADCAST_RADIUS,
            },
        });
    }
}

pub(super) fn emit_anim(world: &mut bevy_ecs::world::World, entity: Entity, anim_id: &str) {
    let origin = world
        .get::<Position>(entity)
        .map(|p| p.get())
        .unwrap_or(DVec3::ZERO);
    let unique_id = world.get::<UniqueId>(entity).map(|id| id.0.to_string());
    if let (Some(target_player), Some(mut events)) = (
        unique_id,
        world.get_resource_mut::<Events<VfxEventRequest>>(),
    ) {
        events.send(VfxEventRequest::new(
            origin,
            VfxEventPayloadV1::PlayAnim {
                target_player,
                anim_id: anim_id.to_string(),
                priority: 1200,
                fade_in_ticks: Some(2),
            },
        ));
    }
}

pub fn visual_for(skill: DuguSkillId) -> DuguSkillVisual {
    match skill {
        DuguSkillId::Eclipse => DuguSkillVisual {
            animation_id: "bong:dugu_needle_throw",
            particle_id: "bong:dugu_taint_pulse",
            sound_recipe_id: "dugu_needle_hiss",
            hud_hint: "蚀针",
            icon_texture: "bong:textures/gui/skill/dugu_eclipse.png",
        },
        DuguSkillId::SelfCure => DuguSkillVisual {
            animation_id: "bong:dugu_self_cure_pose",
            particle_id: "bong:dugu_dark_green_mist",
            sound_recipe_id: "dugu_self_cure_drink",
            hud_hint: "自蕴",
            icon_texture: "bong:textures/gui/skill/dugu_self_cure.png",
        },
        DuguSkillId::Penetrate => DuguSkillVisual {
            animation_id: "bong:dugu_needle_throw",
            particle_id: "bong:dugu_taint_pulse",
            sound_recipe_id: "dugu_needle_hiss",
            hud_hint: "侵染",
            icon_texture: "bong:textures/gui/skill/dugu_penetrate.png",
        },
        DuguSkillId::Shroud => DuguSkillVisual {
            animation_id: "bong:dugu_shroud_activate",
            particle_id: "bong:dugu_dark_green_mist",
            sound_recipe_id: "dugu_self_cure_drink",
            hud_hint: "神识遮蔽",
            icon_texture: "bong:textures/gui/skill/dugu_shroud.png",
        },
        DuguSkillId::Reverse => DuguSkillVisual {
            animation_id: "bong:dugu_pointing_curse",
            particle_id: "bong:dugu_reverse_burst",
            sound_recipe_id: "dugu_curse_cackle",
            hud_hint: "倒蚀",
            icon_texture: "bong:textures/gui/skill/dugu_reverse.png",
        },
    }
}
