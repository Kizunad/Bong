use valence::prelude::{bevy_ecs, DVec3, Entity, Events, Position, ResMut};

use crate::combat::components::{SkillBarBindings, TICKS_PER_SECOND};
use crate::combat::CombatClock;
use crate::cultivation::components::{
    Contamination, Cultivation, MeridianId, MeridianSystem, Realm,
};
use crate::cultivation::meridian::severed::{
    check_meridian_runtime_integrity, MeridianSeveredPermanent, SkillMeridianDependencies,
};
use crate::cultivation::skill_registry::{CastRejectReason, CastResult, SkillRegistry};
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
pub const WOLIU_REQUIRED_MERIDIANS: [MeridianId; 1] = [MeridianId::Lung];

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
}

pub fn declare_woliu_v2_meridian_dependencies(mut deps: ResMut<SkillMeridianDependencies>) {
    for skill_id in [
        WOLIU_HOLD_SKILL_ID,
        WOLIU_BURST_SKILL_ID,
        WOLIU_MOUTH_SKILL_ID,
        WOLIU_PULL_SKILL_ID,
        WOLIU_HEART_SKILL_ID,
    ] {
        deps.declare(skill_id, WOLIU_REQUIRED_MERIDIANS.to_vec());
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

    let spec = skill_spec(skill, cultivation.realm);
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
            check_meridian_runtime_integrity(&WOLIU_REQUIRED_MERIDIANS, meridians, severed)
        {
            return rejected(CastRejectReason::MeridianSevered(Some(blocking)));
        }
        meridians.sum_capacity().max(1.0)
    };

    let dimension = world
        .get::<CurrentDimension>(caster)
        .map(|dimension| dimension.0)
        .unwrap_or(DimensionKind::Overworld);
    let zone_context = current_zone_context(
        world.get_resource::<ZoneRegistry>(),
        dimension,
        position.get(),
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
    let overflow_level = backfire_level_for_overflow(stir.overflow, cultivation.qi_max)
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

    let center = position.get();
    let cooldown_until_tick = now_tick.saturating_add(spec.cooldown_ticks);
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
        backfire,
        &zone_context,
        center,
        now_tick,
    );

    CastResult::Started {
        cooldown_ticks: spec.cooldown_ticks,
        anim_duration_ticks: spec.cast_ticks,
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
            absorbed_qi: stir.actual_absorbed as f32,
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
    if skill == WoliuSkillId::Pull {
        if let Some(target) = target {
            let caster_qi = world
                .get::<Cultivation>(caster)
                .map(|c| c.qi_current)
                .unwrap_or(0.0);
            let target_qi = world
                .get::<Cultivation>(target)
                .map(|c| c.qi_current)
                .unwrap_or(0.0);
            let displacement = pull_displacement_blocks(caster_qi, target_qi, spec.pull_force);
            send_event_if_present(
                world,
                EntityDisplacedByVortexPull {
                    caster,
                    target,
                    displacement_blocks: displacement,
                    tick: now_tick,
                },
            );
        }
    }
    for transfer in build_stir_transfers(caster, zone_context, stir) {
        send_event_if_present(world, transfer);
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

fn send_event_if_present<T: valence::prelude::Event>(world: &mut bevy_ecs::world::World, event: T) {
    if let Some(mut events) = world.get_resource_mut::<Events<T>>() {
        events.send(event);
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
    }
}
