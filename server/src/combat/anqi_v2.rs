use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, App, Entity, Event, Resource};

use crate::combat::carrier::{CarrierKind, InjectionKind};
use crate::combat::components::SkillBarBindings;
use crate::combat::CombatClock;
use crate::cultivation::components::{Cultivation, MeridianId, QiColor, Realm};
use crate::cultivation::meridian::severed::{MeridianSeveredPermanent, SkillMeridianDependencies};
use crate::cultivation::skill_registry::{CastRejectReason, CastResult, SkillFn, SkillRegistry};
use crate::qi_physics::{
    abrasion_loss, armor_penetrate, cone_dispersion, density_echo, high_density_inject,
    AbrasionDirection, AnqiContainerKind, ArmorPenetrationOutcome, ConeDispersionShot,
    EchoFractalOutcome, HighDensityInjectionOutcome,
};

pub const ANQI_SINGLE_SNIPE_SKILL_ID: &str = "anqi.single_snipe";
pub const ANQI_MULTI_SHOT_SKILL_ID: &str = "anqi.multi_shot";
pub const ANQI_SOUL_INJECT_SKILL_ID: &str = "anqi.soul_inject";
pub const ANQI_ARMOR_PIERCE_SKILL_ID: &str = "anqi.armor_pierce";
pub const ANQI_ECHO_FRACTAL_SKILL_ID: &str = "anqi.echo_fractal";
pub const CONTAINER_SWITCH_EXPOSURE_TICKS: u64 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnqiSkillId {
    SingleSnipe,
    MultiShot,
    SoulInject,
    ArmorPierce,
    EchoFractal,
}

impl AnqiSkillId {
    pub const ALL: [Self; 5] = [
        Self::SingleSnipe,
        Self::MultiShot,
        Self::SoulInject,
        Self::ArmorPierce,
        Self::EchoFractal,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SingleSnipe => ANQI_SINGLE_SNIPE_SKILL_ID,
            Self::MultiShot => ANQI_MULTI_SHOT_SKILL_ID,
            Self::SoulInject => ANQI_SOUL_INJECT_SKILL_ID,
            Self::ArmorPierce => ANQI_ARMOR_PIERCE_SKILL_ID,
            Self::EchoFractal => ANQI_ECHO_FRACTAL_SKILL_ID,
        }
    }

    pub const fn display_name(self) -> &'static str {
        match self {
            Self::SingleSnipe => "单射狙击",
            Self::MultiShot => "多发齐射",
            Self::SoulInject => "凝魂注射",
            Self::ArmorPierce => "破甲注射",
            Self::EchoFractal => "诱饵分形",
        }
    }

    pub const fn carrier_kind(self) -> CarrierKind {
        match self {
            Self::SingleSnipe => CarrierKind::YibianShougu,
            Self::MultiShot => CarrierKind::LingmuArrow,
            Self::SoulInject => CarrierKind::DyedBone,
            Self::ArmorPierce => CarrierKind::FenglingheBone,
            Self::EchoFractal => CarrierKind::ShangguBone,
        }
    }

    pub const fn injection_kind(self) -> InjectionKind {
        match self {
            Self::SingleSnipe => InjectionKind::Snipe,
            Self::MultiShot => InjectionKind::MultiShot,
            Self::SoulInject => InjectionKind::SoulInject,
            Self::ArmorPierce => InjectionKind::ArmorPierce,
            Self::EchoFractal => InjectionKind::EchoFractal,
        }
    }

    pub const fn min_realm(self) -> Realm {
        match self {
            Self::SingleSnipe | Self::MultiShot => Realm::Awaken,
            Self::SoulInject => Realm::Condense,
            Self::ArmorPierce => Realm::Solidify,
            Self::EchoFractal => Realm::Void,
        }
    }

    pub const fn qi_ratio(self) -> f64 {
        match self {
            Self::SingleSnipe => 0.25,
            Self::MultiShot => 0.40,
            Self::SoulInject => 0.35,
            Self::ArmorPierce => 0.45,
            Self::EchoFractal => 0.60,
        }
    }

    pub const fn cast_ticks_at_mastery_zero(self) -> u32 {
        match self {
            Self::SingleSnipe => 6,
            Self::MultiShot => 30,
            Self::SoulInject => 20,
            Self::ArmorPierce => 40,
            Self::EchoFractal => 60,
        }
    }

    pub const fn cooldown_ticks_at_mastery_zero(self) -> u64 {
        match self {
            Self::SingleSnipe => 60,
            Self::MultiShot => 240,
            Self::SoulInject => 360,
            Self::ArmorPierce => 500,
            Self::EchoFractal => 6_000,
        }
    }
}

#[derive(Debug, Clone, Default, bevy_ecs::component::Component, Serialize, Deserialize)]
pub struct AnqiMastery {
    values: HashMap<AnqiSkillId, f32>,
}

impl AnqiMastery {
    pub fn level(&self, skill: AnqiSkillId) -> u8 {
        self.values
            .get(&skill)
            .copied()
            .unwrap_or_default()
            .clamp(0.0, 100.0)
            .round() as u8
    }

    pub fn grant_cast_xp(&mut self, skill: AnqiSkillId) -> f32 {
        let current = self.values.entry(skill).or_insert(0.0);
        let gain = mastery_gain(*current);
        *current = (*current + gain).clamp(0.0, 100.0);
        *current
    }
}

pub fn mastery_gain(current: f32) -> f32 {
    if current < 50.0 {
        0.5
    } else if current < 80.0 {
        0.2
    } else {
        0.05
    }
}

#[derive(Debug, Clone, Copy, PartialEq, bevy_ecs::component::Component)]
pub struct ContainerSlot {
    pub active: AnqiContainerKind,
    pub switching_until_tick: u64,
}

impl Default for ContainerSlot {
    fn default() -> Self {
        Self {
            active: AnqiContainerKind::HandSlot,
            switching_until_tick: 0,
        }
    }
}

impl ContainerSlot {
    pub fn next_combat_container(self) -> AnqiContainerKind {
        match self.active {
            AnqiContainerKind::HandSlot => AnqiContainerKind::Quiver,
            AnqiContainerKind::Quiver => AnqiContainerKind::PocketPouch,
            AnqiContainerKind::PocketPouch | AnqiContainerKind::Fenglinghe => {
                AnqiContainerKind::HandSlot
            }
        }
    }
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct MultiShotEvent {
    pub caster: Entity,
    pub projectile_count: u8,
    pub carrier_kind: CarrierKind,
    pub shots: Vec<ConeDispersionShot>,
    pub tick: u64,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct QiInjectionEvent {
    pub caster: Entity,
    pub target: Option<Entity>,
    pub skill: AnqiSkillId,
    pub carrier_kind: CarrierKind,
    pub outcome: HighDensityInjectionOutcome,
    pub tick: u64,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct ArmorPierceEvent {
    pub caster: Entity,
    pub target: Option<Entity>,
    pub carrier_kind: CarrierKind,
    pub outcome: ArmorPenetrationOutcome,
    pub tick: u64,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct EchoFractalEvent {
    pub caster: Entity,
    pub carrier_kind: CarrierKind,
    pub outcome: EchoFractalOutcome,
    pub tick: u64,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct CarrierAbrasionEvent {
    pub carrier: Entity,
    pub container: AnqiContainerKind,
    pub direction: AbrasionDirection,
    pub lost_qi: f64,
    pub after_qi: f64,
    pub tick: u64,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct ContainerSwapEvent {
    pub carrier: Entity,
    pub from: AnqiContainerKind,
    pub to: AnqiContainerKind,
    pub switching_until_tick: u64,
    pub tick: u64,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct DecoyDeployEvent {
    pub caster: Entity,
    pub echo_count: u32,
    pub tick: u64,
}

#[derive(Debug, Clone, Resource)]
pub struct AnqiV2PhysicsConfig {
    pub local_void_density: f64,
    pub echo_threshold: f64,
}

impl Default for AnqiV2PhysicsConfig {
    fn default() -> Self {
        Self {
            local_void_density: 9.0,
            echo_threshold: 0.3,
        }
    }
}

pub fn register(app: &mut App) {
    app.init_resource::<AnqiV2PhysicsConfig>();
    app.add_event::<MultiShotEvent>();
    app.add_event::<QiInjectionEvent>();
    app.add_event::<ArmorPierceEvent>();
    app.add_event::<EchoFractalEvent>();
    app.add_event::<CarrierAbrasionEvent>();
    app.add_event::<ContainerSwapEvent>();
    app.add_event::<DecoyDeployEvent>();
}

pub fn switch_container_slot(
    world: &mut bevy_ecs::world::World,
    carrier: Entity,
    to: AnqiContainerKind,
    tick: u64,
) -> Option<ContainerSlot> {
    if !to.allows_combat_swap() {
        return None;
    }

    let mut slot = world
        .get::<ContainerSlot>(carrier)
        .copied()
        .unwrap_or_default();
    if slot.active == to {
        return Some(slot);
    }

    let from = slot.active;
    slot.active = to;
    slot.switching_until_tick = tick.saturating_add(CONTAINER_SWITCH_EXPOSURE_TICKS);
    world.entity_mut(carrier).insert(slot);

    if let Some(mut events) =
        world.get_resource_mut::<bevy_ecs::event::Events<ContainerSwapEvent>>()
    {
        events.send(ContainerSwapEvent {
            carrier,
            from,
            to,
            switching_until_tick: slot.switching_until_tick,
            tick,
        });
    }

    Some(slot)
}

pub fn cycle_container_slot(
    world: &mut bevy_ecs::world::World,
    carrier: Entity,
    tick: u64,
) -> Option<ContainerSlot> {
    let current = world
        .get::<ContainerSlot>(carrier)
        .copied()
        .unwrap_or_default();
    switch_container_slot(world, carrier, current.next_combat_container(), tick)
}

pub fn register_skills(registry: &mut SkillRegistry) {
    for skill in AnqiSkillId::ALL {
        tracing::debug!(
            skill_id = skill.as_str(),
            display_name = skill.display_name(),
            injection_kind = ?skill.injection_kind(),
            "registering anqi-v2 skill"
        );
        registry.register(skill.as_str(), resolver_for(skill));
    }
}

fn resolver_for(skill: AnqiSkillId) -> SkillFn {
    match skill {
        AnqiSkillId::SingleSnipe => resolve_single_snipe,
        AnqiSkillId::MultiShot => resolve_multi_shot,
        AnqiSkillId::SoulInject => resolve_soul_inject,
        AnqiSkillId::ArmorPierce => resolve_armor_pierce,
        AnqiSkillId::EchoFractal => resolve_echo_fractal,
    }
}

pub fn declare_meridian_dependencies(dependencies: &mut SkillMeridianDependencies) {
    dependencies.declare(
        ANQI_SINGLE_SNIPE_SKILL_ID,
        vec![MeridianId::Lung, MeridianId::Heart, MeridianId::Pericardium],
    );
    dependencies.declare(ANQI_MULTI_SHOT_SKILL_ID, vec![MeridianId::Pericardium]);
    dependencies.declare(ANQI_SOUL_INJECT_SKILL_ID, vec![MeridianId::Spleen]);
    dependencies.declare(ANQI_ARMOR_PIERCE_SKILL_ID, vec![MeridianId::LargeIntestine]);
    dependencies.declare(ANQI_ECHO_FRACTAL_SKILL_ID, vec![MeridianId::Du]);
}

fn resolve_single_snipe(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
) -> CastResult {
    resolve_anqi_skill(world, caster, slot, target, AnqiSkillId::SingleSnipe)
}

fn resolve_multi_shot(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
) -> CastResult {
    resolve_anqi_skill(world, caster, slot, target, AnqiSkillId::MultiShot)
}

fn resolve_soul_inject(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
) -> CastResult {
    resolve_anqi_skill(world, caster, slot, target, AnqiSkillId::SoulInject)
}

fn resolve_armor_pierce(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
) -> CastResult {
    resolve_anqi_skill(world, caster, slot, target, AnqiSkillId::ArmorPierce)
}

fn resolve_echo_fractal(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
) -> CastResult {
    resolve_anqi_skill(world, caster, slot, target, AnqiSkillId::EchoFractal)
}

fn resolve_anqi_skill(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
    skill: AnqiSkillId,
) -> CastResult {
    let now_tick = world
        .get_resource::<CombatClock>()
        .map(|clock| clock.tick)
        .unwrap_or_default();
    if world
        .get::<SkillBarBindings>(caster)
        .is_some_and(|bindings| bindings.is_on_cooldown(slot, now_tick))
    {
        return CastResult::Rejected {
            reason: CastRejectReason::OnCooldown,
        };
    }

    let Some(cultivation) = world.get::<Cultivation>(caster).cloned() else {
        return CastResult::Rejected {
            reason: CastRejectReason::QiInsufficient,
        };
    };
    if realm_rank(cultivation.realm) < realm_rank(skill.min_realm()) {
        return CastResult::Rejected {
            reason: CastRejectReason::RealmTooLow,
        };
    }
    if let Some(blocked) = blocked_meridian(skill, world.get::<MeridianSeveredPermanent>(caster)) {
        return CastResult::Rejected {
            reason: CastRejectReason::MeridianSevered(Some(blocked)),
        };
    }
    let container_slot = world
        .get::<ContainerSlot>(caster)
        .copied()
        .unwrap_or_default();
    if container_slot.switching_until_tick > now_tick {
        return CastResult::Rejected {
            reason: CastRejectReason::InRecovery,
        };
    }

    let qi_cost = cultivation.qi_max * skill.qi_ratio();
    if cultivation.qi_current + f64::EPSILON < qi_cost {
        return CastResult::Rejected {
            reason: CastRejectReason::QiInsufficient,
        };
    }

    let mastery = world
        .get::<AnqiMastery>(caster)
        .map(|m| m.level(skill))
        .unwrap_or_default();
    let color_matched = world.get::<QiColor>(caster).is_some_and(|color| {
        // v2 凝实色匹配读 Cultivation/QiColor 现有主色；当前 ColorKind 没有独立档差，
        // 以 main color 存在视作匹配入口，后续 color plan 可细化。
        matches!(color.main, crate::cultivation::components::ColorKind::Solid)
    });

    if let Some(mut cultivation) = world.get_mut::<Cultivation>(caster) {
        cultivation.qi_current = (cultivation.qi_current - qi_cost).clamp(0.0, cultivation.qi_max);
    }
    if world.get::<AnqiMastery>(caster).is_none() {
        world.entity_mut(caster).insert(AnqiMastery::default());
    }
    if let Some(mut mastery) = world.get_mut::<AnqiMastery>(caster) {
        mastery.grant_cast_xp(skill);
    }

    let payload_qi =
        draw_payload_after_abrasion(world, caster, container_slot.active, qi_cost, now_tick);
    emit_skill_event(
        world,
        caster,
        target,
        skill,
        payload_qi,
        cultivation.qi_max,
        cultivation.realm,
        mastery,
        color_matched,
        now_tick,
    );

    let cooldown = cooldown_for(skill, mastery);
    if let Some(mut bindings) = world.get_mut::<SkillBarBindings>(caster) {
        bindings.set_cooldown(slot, now_tick.saturating_add(cooldown));
    }
    CastResult::Started {
        cooldown_ticks: cooldown,
        anim_duration_ticks: cast_ticks_for(skill, mastery),
    }
}

fn draw_payload_after_abrasion(
    world: &mut bevy_ecs::world::World,
    carrier: Entity,
    container: AnqiContainerKind,
    qi_payload: f64,
    tick: u64,
) -> f64 {
    let Ok(outcome) = abrasion_loss(qi_payload, container, AbrasionDirection::Draw) else {
        return qi_payload;
    };
    if container != AnqiContainerKind::HandSlot {
        if let Some(mut events) =
            world.get_resource_mut::<bevy_ecs::event::Events<CarrierAbrasionEvent>>()
        {
            events.send(CarrierAbrasionEvent {
                carrier,
                container,
                direction: AbrasionDirection::Draw,
                lost_qi: outcome.lost_qi,
                after_qi: outcome.after_qi,
                tick,
            });
        }
    }
    outcome.after_qi
}

#[allow(clippy::too_many_arguments)]
fn emit_skill_event(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    target: Option<Entity>,
    skill: AnqiSkillId,
    payload_qi: f64,
    caster_qi_max: f64,
    realm: Realm,
    mastery: u8,
    color_matched: bool,
    tick: u64,
) {
    match skill {
        AnqiSkillId::SingleSnipe => {
            if let Some(mut events) =
                world.get_resource_mut::<bevy_ecs::event::Events<QiInjectionEvent>>()
            {
                if let Ok(outcome) = high_density_inject(payload_qi, caster_qi_max, true, mastery) {
                    events.send(QiInjectionEvent {
                        caster,
                        target,
                        skill,
                        carrier_kind: skill.carrier_kind(),
                        outcome,
                        tick,
                    });
                }
            }
        }
        AnqiSkillId::MultiShot => {
            if let Some(mut events) =
                world.get_resource_mut::<bevy_ecs::event::Events<MultiShotEvent>>()
            {
                if let Ok(shots) = cone_dispersion(5, 60.0, 30.0, mastery) {
                    events.send(MultiShotEvent {
                        caster,
                        projectile_count: 5,
                        carrier_kind: skill.carrier_kind(),
                        shots,
                        tick,
                    });
                }
            }
        }
        AnqiSkillId::SoulInject => {
            if let Some(mut events) =
                world.get_resource_mut::<bevy_ecs::event::Events<QiInjectionEvent>>()
            {
                if let Ok(outcome) =
                    high_density_inject(payload_qi, caster_qi_max, color_matched, mastery)
                {
                    events.send(QiInjectionEvent {
                        caster,
                        target,
                        skill,
                        carrier_kind: skill.carrier_kind(),
                        outcome,
                        tick,
                    });
                }
            }
        }
        AnqiSkillId::ArmorPierce => {
            if let Some(mut events) =
                world.get_resource_mut::<bevy_ecs::event::Events<ArmorPierceEvent>>()
            {
                if let Ok(outcome) = armor_penetrate(
                    payload_qi,
                    0.75,
                    realm,
                    mastery,
                    crate::qi_physics::CarrierGrade::AncientRelic,
                ) {
                    events.send(ArmorPierceEvent {
                        caster,
                        target,
                        carrier_kind: skill.carrier_kind(),
                        outcome,
                        tick,
                    });
                }
            }
        }
        AnqiSkillId::EchoFractal => {
            let config = world
                .get_resource::<AnqiV2PhysicsConfig>()
                .cloned()
                .unwrap_or_default();
            if let Ok(outcome) = density_echo(
                config.local_void_density,
                config.echo_threshold,
                payload_qi,
                mastery,
            ) {
                if let Some(mut events) =
                    world.get_resource_mut::<bevy_ecs::event::Events<EchoFractalEvent>>()
                {
                    events.send(EchoFractalEvent {
                        caster,
                        carrier_kind: skill.carrier_kind(),
                        outcome,
                        tick,
                    });
                }
                if let Some(mut decoys) =
                    world.get_resource_mut::<bevy_ecs::event::Events<DecoyDeployEvent>>()
                {
                    decoys.send(DecoyDeployEvent {
                        caster,
                        echo_count: outcome.echo_count,
                        tick,
                    });
                }
            }
        }
    }
}

pub fn blocked_meridian(
    skill: AnqiSkillId,
    severed: Option<&MeridianSeveredPermanent>,
) -> Option<MeridianId> {
    let severed = severed?;
    match skill {
        AnqiSkillId::SingleSnipe => {
            let hand_yin = [MeridianId::Lung, MeridianId::Heart, MeridianId::Pericardium];
            hand_yin
                .iter()
                .all(|id| severed.is_severed(*id))
                .then_some(MeridianId::Lung)
        }
        AnqiSkillId::MultiShot => severed
            .is_severed(MeridianId::Pericardium)
            .then_some(MeridianId::Pericardium),
        AnqiSkillId::SoulInject => severed
            .is_severed(MeridianId::Spleen)
            .then_some(MeridianId::Spleen),
        AnqiSkillId::ArmorPierce => severed
            .is_severed(MeridianId::LargeIntestine)
            .then_some(MeridianId::LargeIntestine),
        AnqiSkillId::EchoFractal => severed.is_severed(MeridianId::Du).then_some(MeridianId::Du),
    }
}

pub fn cast_ticks_for(skill: AnqiSkillId, mastery: u8) -> u32 {
    let ratio = f64::from(mastery.min(100)) / 100.0;
    let min_ratio = match skill {
        AnqiSkillId::SingleSnipe => 0.33,
        AnqiSkillId::MultiShot => 0.40,
        AnqiSkillId::SoulInject => 0.40,
        AnqiSkillId::ArmorPierce => 0.40,
        AnqiSkillId::EchoFractal => 0.40,
    };
    let factor = 1.0 - (1.0 - min_ratio) * ratio;
    (f64::from(skill.cast_ticks_at_mastery_zero()) * factor).round() as u32
}

pub fn cooldown_for(skill: AnqiSkillId, mastery: u8) -> u64 {
    let ratio = f64::from(mastery.min(100)) / 100.0;
    let min_ratio = match skill {
        AnqiSkillId::SingleSnipe => 0.27,
        AnqiSkillId::MultiShot => 0.33,
        AnqiSkillId::SoulInject => 0.33,
        AnqiSkillId::ArmorPierce => 0.40,
        AnqiSkillId::EchoFractal => 0.40,
    };
    let factor = 1.0 - (1.0 - min_ratio) * ratio;
    (skill.cooldown_ticks_at_mastery_zero() as f64 * factor).round() as u64
}

fn realm_rank(realm: Realm) -> u8 {
    match realm {
        Realm::Awaken => 0,
        Realm::Induce => 1,
        Realm::Condense => 2,
        Realm::Solidify => 3,
        Realm::Spirit => 4,
        Realm::Void => 5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::meridian::severed::{MeridianSeveredPermanent, SeveredSource};

    #[test]
    fn skill_specs_bind_to_distinct_carriers() {
        let carriers: std::collections::HashSet<_> = AnqiSkillId::ALL
            .into_iter()
            .map(AnqiSkillId::carrier_kind)
            .collect();
        assert_eq!(carriers.len(), 5);
        assert_eq!(AnqiSkillId::MultiShot.as_str(), ANQI_MULTI_SHOT_SKILL_ID);
        assert_eq!(AnqiSkillId::SoulInject.display_name(), "凝魂注射");
        assert_eq!(
            AnqiSkillId::ArmorPierce.injection_kind(),
            InjectionKind::ArmorPierce
        );
        assert_eq!(AnqiSkillId::EchoFractal.min_realm(), Realm::Void);
    }

    #[test]
    fn single_snipe_only_fails_when_all_hand_yin_are_severed() {
        let mut severed = MeridianSeveredPermanent::default();
        severed.insert(MeridianId::Lung, SeveredSource::CombatWound, 1);
        assert_eq!(
            blocked_meridian(AnqiSkillId::SingleSnipe, Some(&severed)),
            None
        );
        severed.insert(MeridianId::Heart, SeveredSource::CombatWound, 1);
        severed.insert(MeridianId::Pericardium, SeveredSource::CombatWound, 1);
        assert_eq!(
            blocked_meridian(AnqiSkillId::SingleSnipe, Some(&severed)),
            Some(MeridianId::Lung)
        );
    }

    #[test]
    fn per_skill_meridian_failures_match_plan_table() {
        let mut severed = MeridianSeveredPermanent::default();
        severed.insert(MeridianId::Spleen, SeveredSource::OverloadTear, 1);
        severed.insert(MeridianId::LargeIntestine, SeveredSource::CombatWound, 1);
        severed.insert(MeridianId::Du, SeveredSource::TribulationFail, 1);
        assert_eq!(
            blocked_meridian(AnqiSkillId::SoulInject, Some(&severed)),
            Some(MeridianId::Spleen)
        );
        assert_eq!(
            blocked_meridian(AnqiSkillId::ArmorPierce, Some(&severed)),
            Some(MeridianId::LargeIntestine)
        );
        assert_eq!(
            blocked_meridian(AnqiSkillId::EchoFractal, Some(&severed)),
            Some(MeridianId::Du)
        );
    }

    #[test]
    fn mastery_growth_uses_three_bands() {
        assert_eq!(mastery_gain(0.0), 0.5);
        assert_eq!(mastery_gain(50.0), 0.2);
        assert_eq!(mastery_gain(80.0), 0.05);
    }

    #[test]
    fn mastery_reduces_cast_and_cooldown() {
        assert!(
            cast_ticks_for(AnqiSkillId::EchoFractal, 100)
                < cast_ticks_for(AnqiSkillId::EchoFractal, 0)
        );
        assert!(
            cooldown_for(AnqiSkillId::MultiShot, 100) < cooldown_for(AnqiSkillId::MultiShot, 0)
        );
    }

    #[test]
    fn container_cycle_emits_swap_event_and_exposure_window() {
        let mut world = bevy_ecs::world::World::new();
        world.insert_resource(bevy_ecs::event::Events::<ContainerSwapEvent>::default());
        let carrier = world.spawn(ContainerSlot::default()).id();

        let slot = cycle_container_slot(&mut world, carrier, 120).unwrap();

        assert_eq!(slot.active, AnqiContainerKind::Quiver);
        assert_eq!(
            slot.switching_until_tick,
            120 + CONTAINER_SWITCH_EXPOSURE_TICKS
        );
        let events = world.resource::<bevy_ecs::event::Events<ContainerSwapEvent>>();
        let emitted: Vec<_> = events.get_reader().read(events).cloned().collect();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].from, AnqiContainerKind::HandSlot);
        assert_eq!(emitted[0].to, AnqiContainerKind::Quiver);
    }

    #[test]
    fn switching_to_fenglinghe_is_rejected_in_combat_window() {
        let mut world = bevy_ecs::world::World::new();
        world.insert_resource(bevy_ecs::event::Events::<ContainerSwapEvent>::default());
        let carrier = world.spawn(ContainerSlot::default()).id();

        assert_eq!(
            switch_container_slot(&mut world, carrier, AnqiContainerKind::Fenglinghe, 120),
            None
        );

        assert_eq!(
            world.get::<ContainerSlot>(carrier).copied().unwrap().active,
            AnqiContainerKind::HandSlot
        );
        let events = world.resource::<bevy_ecs::event::Events<ContainerSwapEvent>>();
        assert_eq!(events.get_reader().read(events).count(), 0);
    }

    #[test]
    fn quiver_cast_emits_draw_abrasion_and_uses_taxed_payload() {
        let mut world = bevy_ecs::world::World::new();
        world.insert_resource(CombatClock { tick: 200 });
        world.insert_resource(bevy_ecs::event::Events::<QiInjectionEvent>::default());
        world.insert_resource(bevy_ecs::event::Events::<CarrierAbrasionEvent>::default());
        let caster = world
            .spawn((
                Cultivation {
                    realm: Realm::Awaken,
                    qi_current: 100.0,
                    qi_max: 100.0,
                    ..Default::default()
                },
                SkillBarBindings::default(),
                ContainerSlot {
                    active: AnqiContainerKind::Quiver,
                    switching_until_tick: 0,
                },
            ))
            .id();

        let result = resolve_anqi_skill(&mut world, caster, 0, None, AnqiSkillId::SingleSnipe);

        assert!(matches!(result, CastResult::Started { .. }));
        let abrasions = world.resource::<bevy_ecs::event::Events<CarrierAbrasionEvent>>();
        let abrasion_events: Vec<_> = abrasions.get_reader().read(abrasions).cloned().collect();
        assert_eq!(abrasion_events.len(), 1);
        assert_eq!(abrasion_events[0].container, AnqiContainerKind::Quiver);
        assert_eq!(abrasion_events[0].direction, AbrasionDirection::Draw);
        assert!((abrasion_events[0].lost_qi - 1.25).abs() <= f64::EPSILON);
        assert!((abrasion_events[0].after_qi - 23.75).abs() <= f64::EPSILON);

        let injections = world.resource::<bevy_ecs::event::Events<QiInjectionEvent>>();
        let injection_events: Vec<_> = injections.get_reader().read(injections).cloned().collect();
        assert_eq!(injection_events.len(), 1);
        assert!((injection_events[0].outcome.payload_qi - 23.75).abs() <= f64::EPSILON);
        assert!((injection_events[0].outcome.overload_ratio - 0.2375).abs() <= f64::EPSILON);
    }

    #[test]
    fn switching_window_blocks_release() {
        let mut world = bevy_ecs::world::World::new();
        world.insert_resource(CombatClock { tick: 200 });
        world.insert_resource(bevy_ecs::event::Events::<QiInjectionEvent>::default());
        world.insert_resource(bevy_ecs::event::Events::<CarrierAbrasionEvent>::default());
        let caster = world
            .spawn((
                Cultivation {
                    realm: Realm::Awaken,
                    qi_current: 100.0,
                    qi_max: 100.0,
                    ..Default::default()
                },
                SkillBarBindings::default(),
                ContainerSlot {
                    active: AnqiContainerKind::Quiver,
                    switching_until_tick: 205,
                },
            ))
            .id();

        let result = resolve_anqi_skill(&mut world, caster, 0, None, AnqiSkillId::SingleSnipe);

        assert_eq!(
            result,
            CastResult::Rejected {
                reason: CastRejectReason::InRecovery
            }
        );
        let abrasions = world.resource::<bevy_ecs::event::Events<CarrierAbrasionEvent>>();
        assert_eq!(abrasions.get_reader().read(abrasions).count(), 0);
    }
}
