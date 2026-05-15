use valence::entity::Look;
use valence::prelude::{bevy_ecs, DVec3, Entity, Events, Position, ResMut};

use crate::combat::components::{
    BodyPart, SkillBarBindings, Wound, WoundKind, Wounds, TICKS_PER_SECOND,
};
use crate::combat::events::{
    emit_death_event_if_lethal, ApplyStatusEffectIntent, AttackSource, CombatEvent,
    StatusEffectKind,
};
use crate::combat::CombatClock;
use crate::cultivation::components::{
    ColorKind, ContamSource, Contamination, Cultivation, MeridianId, MeridianSystem, Realm,
};
use crate::cultivation::known_techniques::KnownTechniques;
use crate::cultivation::meridian::severed::{
    check_meridian_runtime_integrity, MeridianSeveredPermanent, SkillMeridianDependencies,
};
use crate::cultivation::skill_registry::{CastRejectReason, CastResult, SkillRegistry};
use crate::cultivation::technique_proficiency::woliu_scalars_for_proficiency;
use crate::qi_physics::{QiAccountId, QiTransfer, QiTransferReason};
use crate::skill::components::SkillId;
use crate::skill::events::{SkillXpGain, XpGainSource};
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::zone::ZoneRegistry;

use super::backfire::{
    apply_backfire_to_hand_meridians, backfire_level_for_overflow, forced_backfire,
};
use super::events::{
    BackfireCauseV2, BackfireLevel, EntityDisplacedByVortexPull, TurbulenceFieldSpawned,
    VortexBackfireEventV2, VortexCastEvent, WoliuSkillId, WoliuSkillVisual,
};
use super::physics::{
    contamination_ratio, pull_displacement_blocks, stir_99_1, StirInput, StirOutcome,
};
use super::state::{PassiveVortex, TurbulenceExposure, TurbulenceField, VortexV2State};

pub const WOLIU_HOLD_SKILL_ID: &str = "woliu.hold";
pub const WOLIU_BURST_SKILL_ID: &str = "woliu.burst";
pub const WOLIU_MOUTH_SKILL_ID: &str = "woliu.mouth";
pub const WOLIU_PULL_SKILL_ID: &str = "woliu.pull";
pub const WOLIU_HEART_SKILL_ID: &str = "woliu.heart";
pub const WOLIU_VACUUM_PALM_SKILL_ID: &str = "woliu.vacuum_palm";
pub const WOLIU_VORTEX_SHIELD_SKILL_ID: &str = "woliu.vortex_shield";
pub const WOLIU_VACUUM_LOCK_SKILL_ID: &str = "woliu.vacuum_lock";
pub const WOLIU_VORTEX_RESONANCE_SKILL_ID: &str = "woliu.vortex_resonance";
pub const WOLIU_TURBULENCE_BURST_SKILL_ID: &str = "woliu.turbulence_burst";
pub const WOLIU_V2_REQUIRED_MERIDIANS: [MeridianId; 1] = [MeridianId::Lung];
pub const WOLIU_V3_REQUIRED_MERIDIANS: [MeridianId; 2] = [MeridianId::Lung, MeridianId::Heart];
pub const WOLIU_TURBULENCE_BURST_DAMAGE: f32 = 60.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WoliuSkillSpec {
    pub skill: WoliuSkillId,
    pub field_strength: f32,
    pub lethal_radius: f32,
    pub influence_radius: f32,
    pub turbulence_radius: f32,
    pub startup_qi: f64,
    pub maintain_qi_per_sec: f64,
    pub duration_ticks: u64,
    pub cooldown_ticks: u64,
    pub cast_ticks: u32,
    pub pull_force: f64,
    pub drain_qi_per_sec: f64,
    pub passive_default_enabled: bool,
    pub visual: WoliuSkillVisual,
}

impl WoliuSkillSpec {
    pub fn duration_seconds(self) -> f64 {
        self.duration_ticks as f64 / TICKS_PER_SECOND as f64
    }

    pub fn total_qi_cost(self) -> f64 {
        self.startup_qi + self.maintain_qi_per_sec * self.duration_seconds()
    }

    pub fn total_drained(self) -> f64 {
        let radius_sq = f64::from(self.turbulence_radius.max(0.0)).powi(2);
        let field = f64::from(self.field_strength.max(0.0));
        let duration = self.duration_seconds().max(0.05);
        field * radius_sq * duration + self.drain_qi_per_sec.max(0.0) * duration
    }
}

pub fn register_skills(registry: &mut SkillRegistry) {
    registry.register(WOLIU_HOLD_SKILL_ID, cast_hold);
    registry.register(WOLIU_BURST_SKILL_ID, cast_burst);
    registry.register(WOLIU_MOUTH_SKILL_ID, cast_mouth);
    registry.register(WOLIU_PULL_SKILL_ID, cast_pull);
    registry.register(WOLIU_HEART_SKILL_ID, cast_heart);
    registry.register(WOLIU_VACUUM_PALM_SKILL_ID, cast_vacuum_palm);
    registry.register(WOLIU_VORTEX_SHIELD_SKILL_ID, cast_vortex_shield);
    registry.register(WOLIU_VACUUM_LOCK_SKILL_ID, cast_vacuum_lock);
    registry.register(WOLIU_VORTEX_RESONANCE_SKILL_ID, cast_vortex_resonance);
    registry.register(WOLIU_TURBULENCE_BURST_SKILL_ID, cast_turbulence_burst);
}

pub fn declare_woliu_v2_meridian_dependencies(mut deps: ResMut<SkillMeridianDependencies>) {
    for skill_id in [
        WOLIU_HOLD_SKILL_ID,
        WOLIU_BURST_SKILL_ID,
        WOLIU_MOUTH_SKILL_ID,
        WOLIU_PULL_SKILL_ID,
        WOLIU_HEART_SKILL_ID,
    ] {
        deps.declare(skill_id, WOLIU_V2_REQUIRED_MERIDIANS.to_vec());
    }
    for skill_id in [
        WOLIU_VACUUM_PALM_SKILL_ID,
        WOLIU_VORTEX_SHIELD_SKILL_ID,
        WOLIU_VACUUM_LOCK_SKILL_ID,
        WOLIU_VORTEX_RESONANCE_SKILL_ID,
        WOLIU_TURBULENCE_BURST_SKILL_ID,
    ] {
        deps.declare(skill_id, WOLIU_V3_REQUIRED_MERIDIANS.to_vec());
    }
}

pub fn cast_hold(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
) -> CastResult {
    resolve_woliu_v2_skill(world, caster, slot, target, WoliuSkillId::Hold)
}

pub fn cast_burst(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
) -> CastResult {
    resolve_woliu_v2_skill(world, caster, slot, target, WoliuSkillId::Burst)
}

pub fn cast_mouth(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
) -> CastResult {
    resolve_woliu_v2_skill(world, caster, slot, target, WoliuSkillId::Mouth)
}

pub fn cast_pull(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
) -> CastResult {
    resolve_woliu_v2_skill(world, caster, slot, target, WoliuSkillId::Pull)
}

pub fn cast_heart(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
) -> CastResult {
    resolve_woliu_v2_skill(world, caster, slot, target, WoliuSkillId::Heart)
}

pub fn cast_vacuum_palm(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
) -> CastResult {
    resolve_woliu_v2_skill(world, caster, slot, target, WoliuSkillId::VacuumPalm)
}

pub fn cast_vortex_shield(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
) -> CastResult {
    resolve_woliu_v2_skill(world, caster, slot, target, WoliuSkillId::VortexShield)
}

pub fn cast_vacuum_lock(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
) -> CastResult {
    resolve_woliu_v2_skill(world, caster, slot, target, WoliuSkillId::VacuumLock)
}

pub fn cast_vortex_resonance(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
) -> CastResult {
    resolve_woliu_v2_skill(world, caster, slot, target, WoliuSkillId::VortexResonance)
}

pub fn cast_turbulence_burst(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
) -> CastResult {
    resolve_woliu_v2_skill(world, caster, slot, target, WoliuSkillId::TurbulenceBurst)
}

pub fn resolve_woliu_v2_skill(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
    skill: WoliuSkillId,
) -> CastResult {
    let now_tick = world
        .get_resource::<CombatClock>()
        .map(|clock| clock.tick)
        .unwrap_or_default();
    if world
        .get::<SkillBarBindings>(caster)
        .is_some_and(|bindings| bindings.is_on_cooldown(slot, now_tick))
    {
        return rejected(CastRejectReason::OnCooldown);
    }
    let Some(position) = world.get::<Position>(caster).copied() else {
        return rejected(CastRejectReason::InvalidTarget);
    };
    let Some(cultivation) = world.get::<Cultivation>(caster).cloned() else {
        return rejected(CastRejectReason::QiInsufficient);
    };

    let proficiency = known_woliu_proficiency(world, caster, skill);
    let spec = scale_spec_for_proficiency(skill_spec(skill, cultivation.realm), proficiency);
    let dimension = world
        .get::<CurrentDimension>(caster)
        .map(|dimension| dimension.0)
        .unwrap_or(DimensionKind::Overworld);
    let center = match cast_center(world, position.get(), dimension, target, skill, spec) {
        Ok(center) => center,
        Err(reason) => return rejected(reason),
    };

    let cost = spec.total_qi_cost();
    if cultivation.qi_current + f64::EPSILON < cost {
        return rejected(CastRejectReason::QiInsufficient);
    }
    let meridian_capacity = {
        let Some(meridians) = world.get::<MeridianSystem>(caster) else {
            return rejected(CastRejectReason::InvalidTarget);
        };
        let severed = world.get::<MeridianSeveredPermanent>(caster);
        if let Err(blocking) =
            check_meridian_runtime_integrity(required_meridians_for(skill), meridians, severed)
        {
            return rejected(CastRejectReason::MeridianSevered(Some(blocking)));
        }
        meridians.sum_capacity().max(1.0)
    };

    let zone_context = current_zone_context(
        world.get_resource::<ZoneRegistry>(),
        dimension,
        center,
        spec.turbulence_radius,
    );
    let contamination = contamination_ratio(world.get::<Contamination>(caster), cultivation.qi_max);
    let turbulence_cast_precision = world
        .get::<TurbulenceExposure>(caster)
        .map(|exposure| exposure.cast_precision_multiplier())
        .unwrap_or(1.0);

    let stir = stir_99_1(StirInput {
        total_drained: spec.total_drained()
            * zone_context.env_qi.max(0.0)
            * turbulence_cast_precision,
        realm: cultivation.realm,
        contamination_ratio: contamination,
        meridian_flow_capacity: meridian_capacity,
        dt_seconds: spec.duration_seconds().max(0.05),
    });
    let forced = forced_backfire(skill, dimension, 0.0);
    let overflow_level = backfire_level_for_overflow(
        stir.overflow * woliu_scalars_for_proficiency(proficiency).backfire_multiplier,
        cultivation.qi_max,
    )
    .map(|level| (level, BackfireCauseV2::MeridianOverflow));
    let backfire = forced.or(overflow_level);

    {
        let mut cultivation = world
            .get_mut::<Cultivation>(caster)
            .expect("cultivation was checked above");
        cultivation.qi_current =
            (cultivation.qi_current - cost + stir.actual_absorbed).clamp(0.0, cultivation.qi_max);
    }

    if let Some((level, _)) = backfire {
        if let Some(mut meridians) = world.get_mut::<MeridianSystem>(caster) {
            apply_backfire_to_hand_meridians(&mut meridians, level);
        }
    }
    let target_siphoned_qi = apply_target_siphon(world, caster, target, skill, spec);
    record_stir_contamination(world, caster, stir.contamination_gain, now_tick);

    let cooldown_until_tick = now_tick.saturating_add(spec.cooldown_ticks);
    let active_until_tick = now_tick.saturating_add(spec.duration_ticks);
    if spec.turbulence_radius > 0.0 || skill == WoliuSkillId::Heart {
        world.entity_mut(caster).insert(VortexV2State {
            active_skill_kind: skill,
            heart_passive_enabled: spec.passive_default_enabled,
            lethal_radius: spec.lethal_radius,
            influence_radius: spec.influence_radius,
            turbulence_radius: spec.turbulence_radius,
            turbulence_intensity: turbulence_intensity(&spec, stir),
            backfire_level: backfire.map(|(level, _)| level),
            started_at_tick: now_tick,
            active_until_tick,
            cooldown_until_tick,
        });
    }
    if skill == WoliuSkillId::Heart && cultivation.realm == Realm::Void {
        world.entity_mut(caster).insert(PassiveVortex {
            enabled: spec.passive_default_enabled,
            toggled_at_tick: now_tick,
        });
    }
    if spec.turbulence_radius > 0.0 && stir.rotational_swirl > f64::EPSILON {
        world.entity_mut(caster).insert(TurbulenceField::new(
            caster,
            center,
            spec.turbulence_radius,
            turbulence_intensity(&spec, stir),
            stir.rotational_swirl as f32,
            now_tick,
        ));
    }
    if let Some(mut bindings) = world.get_mut::<SkillBarBindings>(caster) {
        bindings.set_cooldown(slot, cooldown_until_tick);
    }

    emit_cast_events(
        world,
        caster,
        target,
        skill,
        spec,
        stir,
        target_siphoned_qi,
        backfire,
        &zone_context,
        center,
        now_tick,
    );
    apply_v3_runtime_effects(
        world,
        V3RuntimeEffectContext {
            caster,
            target,
            skill,
            spec,
            center,
            dimension,
            now_tick,
        },
    );

    CastResult::Started {
        cooldown_ticks: spec.cooldown_ticks,
        anim_duration_ticks: spec.cast_ticks,
    }
}

#[derive(Debug, Clone, Copy)]
struct V3RuntimeEffectContext {
    caster: Entity,
    target: Option<Entity>,
    skill: WoliuSkillId,
    spec: WoliuSkillSpec,
    center: DVec3,
    dimension: DimensionKind,
    now_tick: u64,
}

fn apply_v3_runtime_effects(world: &mut bevy_ecs::world::World, ctx: V3RuntimeEffectContext) {
    match ctx.skill {
        WoliuSkillId::VortexShield => {
            send_event_if_present(
                world,
                ApplyStatusEffectIntent {
                    target: ctx.caster,
                    kind: StatusEffectKind::DamageReduction,
                    magnitude: 0.6,
                    duration_ticks: ctx.spec.duration_ticks,
                    issued_at_tick: ctx.now_tick,
                },
            );
        }
        WoliuSkillId::VacuumLock => {
            if let Some(target) = ctx.target {
                send_event_if_present(
                    world,
                    ApplyStatusEffectIntent {
                        target,
                        kind: StatusEffectKind::Slowed,
                        magnitude: 0.8,
                        duration_ticks: ctx.spec.duration_ticks,
                        issued_at_tick: ctx.now_tick,
                    },
                );
            }
        }
        WoliuSkillId::VortexResonance => {
            let targets = collect_targets_in_radius(
                world,
                ctx.caster,
                ctx.center,
                ctx.dimension,
                ctx.spec.influence_radius,
            );
            let displacement = vortex_resonance_displacement(targets.len());
            for target in targets {
                if let Some(actual_displacement) = apply_pull_displacement(
                    world,
                    ctx.caster,
                    target,
                    displacement,
                    ctx.spec.influence_radius,
                ) {
                    send_event_if_present(
                        world,
                        EntityDisplacedByVortexPull {
                            caster: ctx.caster,
                            target,
                            displacement_blocks: actual_displacement,
                            tick: ctx.now_tick,
                        },
                    );
                }
            }
        }
        WoliuSkillId::TurbulenceBurst => {
            let targets = collect_targets_in_radius(
                world,
                ctx.caster,
                ctx.center,
                ctx.dimension,
                ctx.spec.influence_radius,
            );
            for target in targets {
                apply_turbulence_burst_target_effects(
                    world,
                    ctx.caster,
                    target,
                    ctx.center,
                    ctx.spec,
                    ctx.now_tick,
                );
            }
            if let (Some(caster_pos), Some(facing)) = (
                world
                    .get::<Position>(ctx.caster)
                    .map(|position| position.get()),
                world
                    .get::<Look>(ctx.caster)
                    .and_then(horizontal_facing_from_look),
            ) {
                let recoil_origin = caster_pos + facing;
                if let Some(actual_displacement) =
                    apply_radial_displacement(world, ctx.caster, recoil_origin, 2.0, true)
                {
                    send_event_if_present(
                        world,
                        EntityDisplacedByVortexPull {
                            caster: ctx.caster,
                            target: ctx.caster,
                            displacement_blocks: actual_displacement,
                            tick: ctx.now_tick,
                        },
                    );
                }
            }
        }
        _ => {}
    }
}

fn collect_targets_in_radius(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    center: DVec3,
    dimension: DimensionKind,
    radius: f32,
) -> Vec<Entity> {
    let radius_sq = f64::from(radius.max(0.0)).powi(2);
    let mut query = world.query::<(
        Entity,
        &Position,
        Option<&CurrentDimension>,
        Option<&Cultivation>,
    )>();
    query
        .iter(world)
        .filter_map(|(entity, position, current_dimension, cultivation)| {
            if entity == caster {
                return None;
            }
            if current_dimension.map(|d| d.0).unwrap_or_default() != dimension {
                return None;
            }
            if !cultivation.is_some_and(|c| c.qi_current > f64::EPSILON) {
                return None;
            }
            if position.get().distance_squared(center) > radius_sq + f64::EPSILON {
                return None;
            }
            Some(entity)
        })
        .collect()
}

fn vortex_resonance_displacement(target_count: usize) -> f32 {
    if target_count == 0 {
        return 0.0;
    }
    (1.0 + 0.2 * target_count as f32).max(1.0)
}

fn apply_turbulence_burst_target_effects(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    target: Entity,
    center: DVec3,
    spec: WoliuSkillSpec,
    now_tick: u64,
) {
    let damage = WOLIU_TURBULENCE_BURST_DAMAGE;
    let mut death_state = None;
    let damaged = if let Some(mut wounds) = world.get_mut::<Wounds>(target) {
        let was_alive = wounds.health_current > 0.0;
        wounds.health_current = (wounds.health_current - damage).clamp(0.0, wounds.health_max);
        wounds.entries.push(Wound {
            location: BodyPart::Chest,
            kind: WoundKind::Concussion,
            severity: damage,
            bleeding_per_sec: 0.0,
            created_at_tick: now_tick,
            inflicted_by: Some(format!("entity:{}", caster.to_bits())),
        });
        death_state = Some((was_alive, wounds.health_current));
        true
    } else {
        false
    };
    if damaged {
        send_event_if_present(
            world,
            CombatEvent {
                attacker: caster,
                target,
                resolved_at_tick: now_tick,
                body_part: BodyPart::Chest,
                wound_kind: WoundKind::Concussion,
                source: AttackSource::BurstMeridian,
                debug_command: false,
                physical_damage: 0.0,
                damage,
                contam_delta: 0.0,
                description: format!(
                    "woliu.turbulence_burst entity:{} -> entity:{} dealt {damage:.1} concussion damage",
                    caster.to_bits(),
                    target.to_bits()
                ),
                defense_kind: None,
                defense_effectiveness: None,
                defense_contam_reduced: None,
                defense_wound_severity: None,
            },
        );
        if let Some((was_alive, health_current)) = death_state {
            emit_death_event_if_lethal(
                world,
                was_alive,
                health_current,
                target,
                format!("woliu.turbulence_burst:entity:{}", caster.to_bits()),
                Some(caster),
                None,
                now_tick,
            );
        }
    }
    send_event_if_present(
        world,
        ApplyStatusEffectIntent {
            target,
            kind: StatusEffectKind::Stunned,
            magnitude: 1.0,
            duration_ticks: TICKS_PER_SECOND,
            issued_at_tick: now_tick,
        },
    );
    if let Some(actual_displacement) =
        apply_radial_displacement(world, target, center, spec.pull_force as f32, true)
    {
        send_event_if_present(
            world,
            EntityDisplacedByVortexPull {
                caster,
                target,
                displacement_blocks: actual_displacement,
                tick: now_tick,
            },
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_cast_events(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    target: Option<Entity>,
    skill: WoliuSkillId,
    spec: WoliuSkillSpec,
    stir: StirOutcome,
    target_siphoned_qi: f64,
    backfire: Option<(BackfireLevel, BackfireCauseV2)>,
    zone_context: &ZoneContext,
    center: DVec3,
    now_tick: u64,
) {
    send_event_if_present(
        world,
        VortexCastEvent {
            caster,
            skill,
            tick: now_tick,
            center,
            lethal_radius: spec.lethal_radius,
            influence_radius: spec.influence_radius,
            turbulence_radius: spec.turbulence_radius,
            absorbed_qi: (stir.actual_absorbed + target_siphoned_qi) as f32,
            swirl_qi: stir.rotational_swirl as f32,
            backfire_level: backfire.map(|(level, _)| level),
            visual: spec.visual,
        },
    );
    if spec.turbulence_radius > 0.0 && stir.rotational_swirl > f64::EPSILON {
        send_event_if_present(
            world,
            TurbulenceFieldSpawned {
                caster,
                skill,
                center,
                radius: spec.turbulence_radius,
                intensity: turbulence_intensity(&spec, stir),
                swirl_qi: stir.rotational_swirl as f32,
                tick: now_tick,
            },
        );
    }
    if let Some((level, cause)) = backfire {
        send_event_if_present(
            world,
            VortexBackfireEventV2 {
                caster,
                skill,
                level,
                cause,
                overflow_qi: stir.overflow as f32,
                tick: now_tick,
            },
        );
    }
    if matches!(skill, WoliuSkillId::Pull | WoliuSkillId::VacuumPalm) {
        if let Some(target) = target {
            let caster_qi = world
                .get::<Cultivation>(caster)
                .map(|c| c.qi_current)
                .unwrap_or(0.0);
            let target_qi = world
                .get::<Cultivation>(target)
                .map(|c| c.qi_current)
                .unwrap_or(0.0);
            let displacement = if skill == WoliuSkillId::VacuumPalm {
                spec.pull_force as f32
            } else {
                pull_displacement_blocks(caster_qi, target_qi, spec.pull_force)
            };
            if let Some(actual_displacement) =
                apply_pull_displacement(world, caster, target, displacement, spec.influence_radius)
            {
                send_event_if_present(
                    world,
                    EntityDisplacedByVortexPull {
                        caster,
                        target,
                        displacement_blocks: actual_displacement,
                        tick: now_tick,
                    },
                );
            }
        }
    }
    for transfer in build_stir_transfers(caster, zone_context, stir) {
        send_event_if_present(world, transfer);
    }
    if let Some(target_siphon) = build_target_siphon_transfer(caster, target, target_siphoned_qi) {
        send_event_if_present(world, target_siphon);
    }
    send_event_if_present(
        world,
        SkillXpGain {
            char_entity: caster,
            skill: SkillId::Combat,
            amount: skill.practice_xp(),
            source: XpGainSource::Action {
                plan_id: "woliu_v2",
                action: skill.as_str(),
            },
        },
    );
}

pub(super) fn apply_target_siphon(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    target: Option<Entity>,
    skill: WoliuSkillId,
    spec: WoliuSkillSpec,
) -> f64 {
    let requested = target_siphon_amount(skill, spec);
    if requested <= f64::EPSILON {
        return 0.0;
    }
    let Some(target) = target else {
        return 0.0;
    };
    if target == caster {
        return 0.0;
    }
    let Some(caster_remaining_qi) = world
        .get::<Cultivation>(caster)
        .map(|cultivation| (cultivation.qi_max - cultivation.qi_current).max(0.0))
    else {
        return 0.0;
    };
    if caster_remaining_qi <= f64::EPSILON {
        return 0.0;
    }
    let drained = {
        let Some(mut target_cultivation) = world.get_mut::<Cultivation>(target) else {
            return 0.0;
        };
        let drained = target_cultivation
            .qi_current
            .min(requested)
            .min(caster_remaining_qi)
            .max(0.0);
        target_cultivation.qi_current = (target_cultivation.qi_current - drained).max(0.0);
        drained
    };

    if let Some(mut caster_cultivation) = world.get_mut::<Cultivation>(caster) {
        caster_cultivation.qi_current =
            (caster_cultivation.qi_current + drained).clamp(0.0, caster_cultivation.qi_max);
    }
    drained
}

fn target_siphon_amount(skill: WoliuSkillId, spec: WoliuSkillSpec) -> f64 {
    match skill {
        WoliuSkillId::VacuumPalm => 15.0,
        WoliuSkillId::VacuumLock => spec.drain_qi_per_sec * spec.duration_seconds(),
        _ => 0.0,
    }
}

fn required_meridians_for(skill: WoliuSkillId) -> &'static [MeridianId] {
    match skill {
        WoliuSkillId::VacuumPalm
        | WoliuSkillId::VortexShield
        | WoliuSkillId::VacuumLock
        | WoliuSkillId::VortexResonance
        | WoliuSkillId::TurbulenceBurst => &WOLIU_V3_REQUIRED_MERIDIANS,
        _ => &WOLIU_V2_REQUIRED_MERIDIANS,
    }
}

fn apply_pull_displacement(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    target: Entity,
    displacement_blocks: f32,
    max_radius: f32,
) -> Option<f32> {
    if !displacement_blocks.is_finite() || displacement_blocks <= f32::EPSILON {
        return None;
    }
    let caster_pos = world
        .get::<Position>(caster)
        .map(|position| position.get())?;
    let caster_dim = world
        .get::<CurrentDimension>(caster)
        .map(|dimension| dimension.0)
        .unwrap_or_default();
    let target_dim = world
        .get::<CurrentDimension>(target)
        .map(|dimension| dimension.0)
        .unwrap_or_default();
    if target_dim != caster_dim {
        return None;
    }
    let mut target_pos = world.get_mut::<Position>(target)?;
    let current = target_pos.get();
    let offset = caster_pos - current;
    let distance = offset.length();
    if !distance.is_finite() || distance <= f64::EPSILON {
        return None;
    }
    if distance > f64::from(max_radius.max(0.0)) + f64::EPSILON {
        return None;
    }
    let step = f64::from(displacement_blocks).min(distance);
    if step <= f64::EPSILON {
        return None;
    }
    target_pos.set(current + offset / distance * step);
    Some(step as f32)
}

fn apply_radial_displacement(
    world: &mut bevy_ecs::world::World,
    target: Entity,
    origin: DVec3,
    displacement_blocks: f32,
    outward: bool,
) -> Option<f32> {
    if !displacement_blocks.is_finite() || displacement_blocks <= f32::EPSILON {
        return None;
    }
    let mut target_pos = world.get_mut::<Position>(target)?;
    let current = target_pos.get();
    let offset = if outward {
        current - origin
    } else {
        origin - current
    };
    let distance = offset.length();
    if !distance.is_finite() || distance <= f64::EPSILON {
        return None;
    }
    let step = if outward {
        f64::from(displacement_blocks)
    } else {
        f64::from(displacement_blocks).min(distance)
    };
    if step <= f64::EPSILON {
        return None;
    }
    target_pos.set(current + offset / distance * step);
    Some(step as f32)
}

fn horizontal_facing_from_look(look: &Look) -> Option<DVec3> {
    let yaw = f64::from(look.yaw).to_radians();
    let facing = DVec3::new(-yaw.sin(), 0.0, yaw.cos());
    facing.is_finite().then_some(facing)
}

fn cast_center(
    world: &bevy_ecs::world::World,
    caster_pos: DVec3,
    caster_dim: DimensionKind,
    target: Option<Entity>,
    skill: WoliuSkillId,
    spec: WoliuSkillSpec,
) -> Result<DVec3, CastRejectReason> {
    match skill {
        WoliuSkillId::Mouth => match target {
            Some(target) => validated_target_position(
                world,
                caster_pos,
                caster_dim,
                target,
                spec.influence_radius,
                false,
            ),
            None => Ok(caster_pos),
        },
        WoliuSkillId::VacuumPalm | WoliuSkillId::VacuumLock => {
            let target = target.ok_or(CastRejectReason::InvalidTarget)?;
            validated_target_position(
                world,
                caster_pos,
                caster_dim,
                target,
                spec.influence_radius,
                true,
            )
        }
        WoliuSkillId::Pull => {
            let target = target.ok_or(CastRejectReason::InvalidTarget)?;
            validated_target_position(
                world,
                caster_pos,
                caster_dim,
                target,
                spec.influence_radius,
                true,
            )?;
            Ok(caster_pos)
        }
        _ => Ok(caster_pos),
    }
}

fn validated_target_position(
    world: &bevy_ecs::world::World,
    caster_pos: DVec3,
    caster_dim: DimensionKind,
    target: Entity,
    max_radius: f32,
    require_qi: bool,
) -> Result<DVec3, CastRejectReason> {
    if require_qi
        && !world
            .get::<Cultivation>(target)
            .is_some_and(|cultivation| cultivation.qi_current > f64::EPSILON)
    {
        return Err(CastRejectReason::InvalidTarget);
    }
    let target_dim = world
        .get::<CurrentDimension>(target)
        .map(|dimension| dimension.0)
        .ok_or(CastRejectReason::InvalidTarget)?;
    if target_dim != caster_dim {
        return Err(CastRejectReason::InvalidTarget);
    }
    let target_pos = world
        .get::<Position>(target)
        .map(|position| position.get())
        .ok_or(CastRejectReason::InvalidTarget)?;
    let distance = caster_pos.distance(target_pos);
    if !distance.is_finite() || distance > f64::from(max_radius.max(0.0)) + f64::EPSILON {
        return Err(CastRejectReason::InvalidTarget);
    }
    Ok(target_pos)
}

fn send_event_if_present<T: valence::prelude::Event>(world: &mut bevy_ecs::world::World, event: T) {
    if let Some(mut events) = world.get_resource_mut::<Events<T>>() {
        events.send(event);
    }
}

fn record_stir_contamination(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    contamination_gain: f64,
    now_tick: u64,
) {
    if !contamination_gain.is_finite() || contamination_gain <= f64::EPSILON {
        return;
    }
    let source = ContamSource {
        amount: contamination_gain,
        color: ColorKind::Intricate,
        meridian_id: Some(MeridianId::Lung),
        attacker_id: None,
        introduced_at: now_tick,
    };
    if let Some(mut contamination) = world.get_mut::<Contamination>(caster) {
        contamination.entries.push(source);
    } else {
        world.entity_mut(caster).insert(Contamination {
            entries: vec![source],
        });
    }
}

#[derive(Debug, Clone)]
struct ZoneContext {
    env_qi: f64,
    source_zone: String,
    swirl_zones: Vec<String>,
}

fn build_stir_transfers(
    caster: Entity,
    zone_context: &ZoneContext,
    stir: StirOutcome,
) -> Vec<QiTransfer> {
    let zone = QiAccountId::zone(zone_context.source_zone.clone());
    let player = QiAccountId::player(format!("entity:{}", caster.to_bits()));
    let mut transfers = Vec::with_capacity(1 + zone_context.swirl_zones.len());
    if let Ok(absorbed) = QiTransfer::new(
        zone.clone(),
        player,
        stir.actual_absorbed,
        QiTransferReason::Channeling,
    ) {
        if absorbed.amount > f64::EPSILON {
            transfers.push(absorbed);
        }
    }
    let swirl_targets = if zone_context.swirl_zones.is_empty() {
        vec![zone_context.source_zone.clone()]
    } else {
        zone_context.swirl_zones.clone()
    };
    let swirl_share = stir.rotational_swirl / swirl_targets.len() as f64;
    for target_zone in swirl_targets {
        if let Ok(swirl) = QiTransfer::new(
            zone.clone(),
            QiAccountId::zone(target_zone),
            swirl_share,
            QiTransferReason::Channeling,
        ) {
            if swirl.amount > f64::EPSILON {
                transfers.push(swirl);
            }
        }
    }
    transfers
}

fn build_target_siphon_transfer(
    caster: Entity,
    target: Option<Entity>,
    amount: f64,
) -> Option<QiTransfer> {
    if amount <= f64::EPSILON {
        return None;
    }
    let target = target?;
    QiTransfer::new(
        QiAccountId::player(format!("entity:{}", target.to_bits())),
        QiAccountId::player(format!("entity:{}", caster.to_bits())),
        amount,
        QiTransferReason::Channeling,
    )
    .ok()
}

fn turbulence_intensity(spec: &WoliuSkillSpec, stir: StirOutcome) -> f32 {
    if spec.turbulence_radius <= 0.0 || stir.rotational_swirl <= 0.0 {
        return 0.0;
    }
    (stir.rotational_swirl / (f64::from(spec.turbulence_radius).powi(2) + 1.0)).clamp(0.05, 1.0)
        as f32
}

fn current_zone_context(
    zones: Option<&ZoneRegistry>,
    dimension: DimensionKind,
    position: DVec3,
    turbulence_radius: f32,
) -> ZoneContext {
    let Some(zones) = zones else {
        return ZoneContext {
            env_qi: 0.9,
            source_zone: "spawn".to_string(),
            swirl_zones: vec!["spawn".to_string()],
        };
    };
    let source = zones.find_zone(dimension, position);
    let source_zone = source
        .map(|zone| zone.name.clone())
        .unwrap_or_else(|| "spawn".to_string());
    let radius = f64::from(turbulence_radius.max(0.0));
    let mut swirl_zones: Vec<String> = zones
        .zones
        .iter()
        .filter(|zone| zone.dimension == dimension)
        .filter(|zone| zone.contains(position) || zone.center().distance(position) <= radius)
        .map(|zone| zone.name.clone())
        .collect();
    if !swirl_zones.iter().any(|zone| zone == &source_zone) {
        swirl_zones.push(source_zone.clone());
    }
    ZoneContext {
        env_qi: source.map(|zone| zone.spirit_qi).unwrap_or(0.9),
        source_zone,
        swirl_zones,
    }
}

fn rejected(reason: CastRejectReason) -> CastResult {
    CastResult::Rejected { reason }
}

pub fn skill_spec(skill: WoliuSkillId, realm: Realm) -> WoliuSkillSpec {
    match skill {
        WoliuSkillId::Hold => hold_spec(realm),
        WoliuSkillId::Burst => burst_spec(realm),
        WoliuSkillId::Mouth => mouth_spec(realm),
        WoliuSkillId::Pull => pull_spec(realm),
        WoliuSkillId::Heart => heart_spec(realm),
        WoliuSkillId::VacuumPalm => vacuum_palm_spec(),
        WoliuSkillId::VortexShield => vortex_shield_spec(),
        WoliuSkillId::VacuumLock => vacuum_lock_spec(),
        WoliuSkillId::VortexResonance => vortex_resonance_spec(),
        WoliuSkillId::TurbulenceBurst => turbulence_burst_spec(),
    }
}

pub fn scale_spec_for_proficiency(mut spec: WoliuSkillSpec, proficiency: f32) -> WoliuSkillSpec {
    let scalars = woliu_scalars_for_proficiency(proficiency);
    let delta_scale = (scalars.vortex_delta / 0.10) as f32;
    spec.field_strength *= delta_scale;
    spec.startup_qi *= scalars.qi_cost_multiplier;
    spec.maintain_qi_per_sec *= scalars.qi_cost_multiplier;
    spec.lethal_radius *= scalars.radius_multiplier;
    spec.influence_radius *= scalars.radius_multiplier;
    spec.turbulence_radius *= scalars.radius_multiplier;
    spec.cast_ticks = ((spec.cast_ticks as f32) * scalars.cast_ticks_multiplier)
        .ceil()
        .max(1.0) as u32;
    spec
}

fn known_woliu_proficiency(
    world: &bevy_ecs::world::World,
    caster: Entity,
    skill: WoliuSkillId,
) -> f32 {
    world
        .get::<KnownTechniques>(caster)
        .and_then(|known| {
            known
                .entries
                .iter()
                .find(|entry| entry.id == skill.as_str())
        })
        .map(|entry| entry.proficiency)
        .unwrap_or(0.5)
        .clamp(0.0, 1.0)
}

fn vacuum_palm_spec() -> WoliuSkillSpec {
    WoliuSkillSpec {
        skill: WoliuSkillId::VacuumPalm,
        field_strength: 0.35,
        lethal_radius: 1.0,
        influence_radius: 8.0,
        turbulence_radius: 1.5,
        startup_qi: 20.0,
        maintain_qi_per_sec: 0.0,
        duration_ticks: TICKS_PER_SECOND * 3 / 2,
        cooldown_ticks: 3 * TICKS_PER_SECOND,
        cast_ticks: 6,
        pull_force: 3.0,
        drain_qi_per_sec: 10.0,
        passive_default_enabled: false,
        visual: visual_for(WoliuSkillId::VacuumPalm),
    }
}

fn vortex_shield_spec() -> WoliuSkillSpec {
    WoliuSkillSpec {
        skill: WoliuSkillId::VortexShield,
        field_strength: 0.45,
        lethal_radius: 0.0,
        influence_radius: 2.0,
        turbulence_radius: 2.0,
        startup_qi: 25.0,
        maintain_qi_per_sec: 5.0,
        duration_ticks: 5 * TICKS_PER_SECOND,
        cooldown_ticks: 12 * TICKS_PER_SECOND,
        cast_ticks: 10,
        pull_force: 1.0,
        drain_qi_per_sec: 1.0,
        passive_default_enabled: false,
        visual: visual_for(WoliuSkillId::VortexShield),
    }
}

fn vacuum_lock_spec() -> WoliuSkillSpec {
    WoliuSkillSpec {
        skill: WoliuSkillId::VacuumLock,
        field_strength: 0.55,
        lethal_radius: 1.5,
        influence_radius: 12.0,
        turbulence_radius: 1.5,
        startup_qi: 35.0,
        maintain_qi_per_sec: 0.0,
        duration_ticks: 3 * TICKS_PER_SECOND,
        cooldown_ticks: 15 * TICKS_PER_SECOND,
        cast_ticks: 8,
        pull_force: 0.0,
        drain_qi_per_sec: 10.0,
        passive_default_enabled: false,
        visual: visual_for(WoliuSkillId::VacuumLock),
    }
}

fn vortex_resonance_spec() -> WoliuSkillSpec {
    WoliuSkillSpec {
        skill: WoliuSkillId::VortexResonance,
        field_strength: 0.65,
        lethal_radius: 0.0,
        influence_radius: 6.0,
        turbulence_radius: 6.0,
        startup_qi: 50.0,
        maintain_qi_per_sec: 0.0,
        duration_ticks: 4 * TICKS_PER_SECOND,
        cooldown_ticks: 20 * TICKS_PER_SECOND,
        cast_ticks: 80,
        pull_force: 1.6,
        drain_qi_per_sec: 3.0,
        passive_default_enabled: false,
        visual: visual_for(WoliuSkillId::VortexResonance),
    }
}

fn turbulence_burst_spec() -> WoliuSkillSpec {
    WoliuSkillSpec {
        skill: WoliuSkillId::TurbulenceBurst,
        field_strength: 0.80,
        lethal_radius: 6.0,
        influence_radius: 6.0,
        turbulence_radius: 6.0,
        startup_qi: 80.0,
        maintain_qi_per_sec: 0.0,
        duration_ticks: 2 * TICKS_PER_SECOND,
        cooldown_ticks: 30 * TICKS_PER_SECOND,
        cast_ticks: 40,
        pull_force: 4.0,
        drain_qi_per_sec: 8.0,
        passive_default_enabled: false,
        visual: visual_for(WoliuSkillId::TurbulenceBurst),
    }
}

fn hold_spec(realm: Realm) -> WoliuSkillSpec {
    let (field_strength, lethal, influence, turbulence, duration, cost) = match realm {
        Realm::Awaken => (0.0, 0.5, 0.5, 0.0, 1, 5.0),
        Realm::Induce => (0.05, 1.0, 1.5, 0.3, 2, 5.0),
        Realm::Condense => (0.25, 1.5, 3.0, 1.0, 5, 6.0),
        Realm::Solidify => (0.45, 2.0, 5.0, 2.0, 8, 7.0),
        Realm::Spirit => (0.65, 3.0, 15.0, 8.0, 12, 9.0),
        Realm::Void => (0.80, 5.0, 40.0, 25.0, 18, 12.0),
    };
    WoliuSkillSpec {
        skill: WoliuSkillId::Hold,
        field_strength,
        lethal_radius: lethal,
        influence_radius: influence,
        turbulence_radius: turbulence,
        startup_qi: 0.0,
        maintain_qi_per_sec: cost,
        duration_ticks: duration * TICKS_PER_SECOND,
        cooldown_ticks: TICKS_PER_SECOND / 2,
        cast_ticks: 1,
        pull_force: 0.0,
        drain_qi_per_sec: field_strength as f64,
        passive_default_enabled: false,
        visual: visual_for(WoliuSkillId::Hold),
    }
}

fn burst_spec(realm: Realm) -> WoliuSkillSpec {
    let (k_drain, window_ms) = match realm {
        Realm::Awaken => (0.0, 200),
        Realm::Induce => (0.10, 200),
        Realm::Condense => (0.25, 200),
        Realm::Solidify => (0.40, 200),
        Realm::Spirit => (0.50, 200),
        Realm::Void => (0.50, 250),
    };
    WoliuSkillSpec {
        skill: WoliuSkillId::Burst,
        field_strength: k_drain,
        lethal_radius: 1.0,
        influence_radius: if realm == Realm::Void { 30.0 } else { 1.0 },
        turbulence_radius: if realm == Realm::Awaken { 0.0 } else { 0.5 },
        startup_qi: 8.0,
        maintain_qi_per_sec: 0.0,
        duration_ticks: ((window_ms as u64) * TICKS_PER_SECOND).div_ceil(1000),
        cooldown_ticks: 5 * TICKS_PER_SECOND,
        cast_ticks: 1,
        pull_force: 0.0,
        drain_qi_per_sec: k_drain as f64,
        passive_default_enabled: false,
        visual: visual_for(WoliuSkillId::Burst),
    }
}

fn mouth_spec(realm: Realm) -> WoliuSkillSpec {
    let (range, drain, duration): (f64, f64, u64) = match realm {
        Realm::Awaken => (0.0, 0.0, 1),
        Realm::Induce => (1.5, 1.0, 2),
        Realm::Condense => (3.0, 2.5, 4),
        Realm::Solidify => (5.0, 4.0, 6),
        Realm::Spirit => (30.0, 5.0, 8),
        Realm::Void => (300.0, 6.0, 10),
    };
    WoliuSkillSpec {
        skill: WoliuSkillId::Mouth,
        field_strength: drain as f32,
        lethal_radius: range.min(5.0) as f32,
        influence_radius: range as f32,
        turbulence_radius: range.min(10.0) as f32,
        startup_qi: 12.0,
        maintain_qi_per_sec: 3.0,
        duration_ticks: duration * TICKS_PER_SECOND,
        cooldown_ticks: 8 * TICKS_PER_SECOND,
        cast_ticks: 6,
        pull_force: 0.0,
        drain_qi_per_sec: drain,
        passive_default_enabled: false,
        visual: visual_for(WoliuSkillId::Mouth),
    }
}

fn pull_spec(realm: Realm) -> WoliuSkillSpec {
    let (radius, pull, turbulence, cooldown) = match realm {
        Realm::Awaken => (0.0, 0.0, 0.0, 30),
        Realm::Induce => (3.0, 1.0, 0.0, 30),
        Realm::Condense => (5.0, 2.5, 1.0, 30),
        Realm::Solidify => (7.0, 4.0, 2.0, 25),
        Realm::Spirit => (30.0, 6.0, 5.0, 20),
        Realm::Void => (100.0, 8.0, 8.0, 15),
    };
    WoliuSkillSpec {
        skill: WoliuSkillId::Pull,
        field_strength: pull as f32,
        lethal_radius: 0.0,
        influence_radius: radius as f32,
        turbulence_radius: turbulence as f32,
        startup_qi: 25.0,
        maintain_qi_per_sec: 0.0,
        duration_ticks: TICKS_PER_SECOND,
        cooldown_ticks: cooldown * TICKS_PER_SECOND,
        cast_ticks: 5,
        pull_force: pull,
        drain_qi_per_sec: 0.0,
        passive_default_enabled: false,
        visual: visual_for(WoliuSkillId::Pull),
    }
}

fn heart_spec(realm: Realm) -> WoliuSkillSpec {
    let (lethal, influence, turbulence, duration, passive) = match realm {
        Realm::Awaken | Realm::Induce => (0.0, 0.0, 0.0, 1, false),
        Realm::Condense => (1.5, 1.5, 1.0, 2, false),
        Realm::Solidify => (2.0, 10.0, 5.0, 4, false),
        Realm::Spirit => (3.0, 30.0, 15.0, 6, false),
        Realm::Void => (5.0, 300.0, 75.0, 30, false),
    };
    WoliuSkillSpec {
        skill: WoliuSkillId::Heart,
        field_strength: if realm == Realm::Void { 0.8 } else { 0.5 },
        lethal_radius: lethal,
        influence_radius: influence,
        turbulence_radius: turbulence,
        startup_qi: 50.0,
        maintain_qi_per_sec: 8.0,
        duration_ticks: duration * TICKS_PER_SECOND,
        cooldown_ticks: 20 * TICKS_PER_SECOND,
        cast_ticks: 10,
        pull_force: 0.0,
        drain_qi_per_sec: if realm == Realm::Void { 8.0 } else { 2.0 },
        passive_default_enabled: passive,
        visual: visual_for(WoliuSkillId::Heart),
    }
}

pub fn visual_for(skill: WoliuSkillId) -> WoliuSkillVisual {
    match skill {
        WoliuSkillId::Hold => WoliuSkillVisual {
            animation_id: "bong:vortex_palm_open",
            particle_id: "bong:vortex_spiral",
            sound_recipe_id: "vortex_low_hum",
            hud_hint: "hold",
            icon_texture: "bong:textures/gui/skill/woliu_hold.png",
        },
        WoliuSkillId::Burst => WoliuSkillVisual {
            animation_id: "bong:vortex_palm_open",
            particle_id: "bong:vortex_spiral",
            sound_recipe_id: "vortex_qi_siphon",
            hud_hint: "burst",
            icon_texture: "bong:textures/gui/skill/woliu_burst.png",
        },
        WoliuSkillId::Mouth => WoliuSkillVisual {
            animation_id: "bong:vortex_palm_open",
            particle_id: "bong:vortex_spiral",
            sound_recipe_id: "vortex_qi_siphon",
            hud_hint: "mouth",
            icon_texture: "bong:textures/gui/skill/woliu_mouth.png",
        },
        WoliuSkillId::Pull => WoliuSkillVisual {
            animation_id: "bong:vortex_spiral_stance",
            particle_id: "bong:vortex_spiral",
            sound_recipe_id: "vortex_qi_siphon",
            hud_hint: "pull",
            icon_texture: "bong:textures/gui/skill/woliu_pull.png",
        },
        WoliuSkillId::Heart => WoliuSkillVisual {
            animation_id: "bong:vortex_spiral_stance",
            particle_id: "bong:vortex_spiral",
            sound_recipe_id: "vortex_low_hum",
            hud_hint: "heart",
            icon_texture: "bong:textures/gui/skill/woliu_heart.png",
        },
        WoliuSkillId::VacuumPalm => WoliuSkillVisual {
            animation_id: "bong:woliu_vacuum_palm",
            particle_id: "bong:woliu_vacuum_palm_spiral",
            sound_recipe_id: "woliu_vacuum_palm",
            hud_hint: "vacuum_palm",
            icon_texture: "bong:textures/gui/skill/woliu_mouth.png",
        },
        WoliuSkillId::VortexShield => WoliuSkillVisual {
            animation_id: "bong:woliu_vortex_shield",
            particle_id: "bong:woliu_vortex_shield_sphere",
            sound_recipe_id: "woliu_vortex_shield",
            hud_hint: "vortex_shield",
            icon_texture: "bong:textures/gui/skill/woliu_hold.png",
        },
        WoliuSkillId::VacuumLock => WoliuSkillVisual {
            animation_id: "bong:woliu_vacuum_lock",
            particle_id: "bong:woliu_vacuum_lock_cage",
            sound_recipe_id: "woliu_vacuum_lock",
            hud_hint: "vacuum_lock",
            icon_texture: "bong:textures/gui/skill/woliu_pull.png",
        },
        WoliuSkillId::VortexResonance => WoliuSkillVisual {
            animation_id: "bong:woliu_vortex_resonance",
            particle_id: "bong:woliu_vortex_resonance_field",
            sound_recipe_id: "woliu_vortex_resonance",
            hud_hint: "vortex_resonance",
            icon_texture: "bong:textures/gui/skill/woliu_heart.png",
        },
        WoliuSkillId::TurbulenceBurst => WoliuSkillVisual {
            animation_id: "bong:woliu_turbulence_burst",
            particle_id: "bong:woliu_turbulence_burst_wave",
            sound_recipe_id: "woliu_turbulence_burst",
            hud_hint: "turbulence_burst",
            icon_texture: "bong:textures/gui/skill/woliu_burst.png",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proficiency_scales_vortex_combat_knobs() {
        let base = skill_spec(WoliuSkillId::Hold, Realm::Condense);
        let novice = scale_spec_for_proficiency(base, 0.0);
        let master = scale_spec_for_proficiency(base, 1.0);

        assert!((novice.field_strength - base.field_strength * 0.8).abs() < 0.0001);
        assert!((master.field_strength - base.field_strength * 1.2).abs() < 0.0001);
        assert!((novice.startup_qi - base.startup_qi * 1.3).abs() < 0.0001);
        assert!((master.startup_qi - base.startup_qi * 0.85).abs() < 0.0001);
        assert!((novice.influence_radius - base.influence_radius * 0.8).abs() < 0.0001);
        assert!((master.influence_radius - base.influence_radius * 1.1).abs() < 0.0001);
        assert_eq!(
            novice.cast_ticks,
            ((base.cast_ticks as f32) * 1.2).ceil() as u32
        );
        assert_eq!(
            master.cast_ticks,
            ((base.cast_ticks as f32) * 0.9).ceil() as u32
        );
    }
}
