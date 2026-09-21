//! 技能只负责门控、成本与一次攻击生命周期；战术由 brain 决定，伤害由 combat 结算。

use valence::entity::EntityId;
use valence::prelude::{
    bevy_ecs, Component, DVec3, Despawned, Entity, EntityLayerId, Events, Position,
};

use crate::combat::components::{Lifecycle, LifecycleState, Stamina, StatusEffects, WoundKind};
use crate::combat::events::{AttackIntent, AttackReach, AttackSource, StatusEffectKind};
use crate::cultivation::components::{ActorQiIdentity, ActorQiKind, Cultivation, MeridianSystem};
use crate::cultivation::known_techniques::{
    KnownTechniques, TechniqueDefinition, TechniqueRegistry,
};
use crate::cultivation::life_record::LifeRecord;
use crate::cultivation::meridian::severed::{
    check_skill_channels, MeridianSeveredPermanent, SkillMeridianDependencies,
};
use crate::cultivation::skill_registry::{CastRejectReason, CastResult, SkillRegistry};
use crate::network::audio_event_emit::PlaySoundRecipeRequest;
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::npc::movement::GameTick;
use crate::npc::technique::NpcCooldownMap;
use crate::qi_physics::ledger::{QiTransfer, QiTransferReason, WorldQiAccount};
use crate::schema::vfx_event::VfxEventPayloadV1;
use crate::world::zone::ZoneRegistry;

use super::brain::WildlifeBrain;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WildlifeSkill {
    Pounce,
    Rend,
    Dive,
    Trample,
    Kick,
}

impl WildlifeSkill {
    pub fn id(self) -> &'static str {
        match self {
            Self::Pounce => "lion.pounce",
            Self::Rend => "lion.rend",
            Self::Dive => "vulture.dive",
            Self::Trample => "horse.trample",
            Self::Kick => "horse.kick",
        }
    }

    pub fn animation(self) -> &'static str {
        match self {
            Self::Pounce => "animation.bong.dainu_lion.pounce",
            Self::Rend => "animation.bong.dainu_lion.bite",
            Self::Dive => "animation.bong.fuyu_vulture_flight.dive",
            Self::Trample => "animation.bong.horse.gallop",
            Self::Kick => "animation.bong.horse.kick",
        }
    }

    /// 每个兽类招式的独立粒子路由；客户端 `BeastSkillVfxPlayer` 按这些 id 分化形态。
    pub fn particle_event_id(self) -> &'static str {
        match self {
            Self::Pounce => "bong:fauna_lion_pounce",
            Self::Rend => "bong:fauna_lion_rend",
            Self::Dive => "bong:fauna_vulture_dive",
            Self::Trample => "bong:fauna_horse_trample",
            Self::Kick => "bong:fauna_horse_kick",
        }
    }

    pub fn active_ticks(self) -> u64 {
        match self {
            Self::Pounce => 12,
            Self::Rend | Self::Kick => 1,
            Self::Dive | Self::Trample => 24,
        }
    }

    pub fn damage_multiplier(self) -> f32 {
        match self {
            Self::Pounce => 1.5,
            Self::Rend => 1.0,
            Self::Dive => 1.3,
            Self::Trample => 1.6,
            Self::Kick => 1.2,
        }
    }
}

#[derive(Clone, Debug, Component)]
pub struct WildlifeCast {
    pub skill: WildlifeSkill,
    pub target: Entity,
    pub hit_at: u64,
    pub active_until: u64,
    pub finish_at: u64,
    pub direction: DVec3,
    pub hit: bool,
    pub submitted_at: Option<u64>,
    pub last_tick: Option<u64>,
}

pub fn now(world: &bevy_ecs::world::World) -> u64 {
    world
        .get_resource::<GameTick>()
        .map(|tick| u64::from(tick.0))
        .unwrap_or(0)
}

pub fn alive(world: &bevy_ecs::world::World, entity: Entity) -> bool {
    world.get::<Despawned>(entity).is_none()
        && world
            .get::<Lifecycle>(entity)
            .is_some_and(|life| life.state == LifecycleState::Alive)
}

pub fn interrupted(world: &bevy_ecs::world::World, entity: Entity) -> bool {
    !alive(world, entity)
        || world
            .get::<crate::npc::movement::MovementController>(entity)
            .is_some_and(|controller| {
                matches!(
                    controller.mode,
                    crate::npc::movement::MovementMode::Override(_)
                )
            })
        || world.get::<StatusEffects>(entity).is_some_and(|effects| {
            effects.active.iter().any(|effect| {
                effect.remaining_ticks > 0
                    && matches!(
                        effect.kind,
                        StatusEffectKind::Stunned
                            | StatusEffectKind::Immobilized
                            | StatusEffectKind::Staggered
                    )
            })
        })
}

pub fn channels_ready(world: &bevy_ecs::world::World, entity: Entity, id: &str) -> bool {
    world
        .get_resource::<TechniqueRegistry>()
        .and_then(|registry| registry.get(id))
        .zip(world.get::<MeridianSystem>(entity))
        .is_some_and(|(definition, meridians)| {
            check_skill_channels(
                &definition.required_meridians,
                meridians,
                world.get::<MeridianSeveredPermanent>(entity),
            )
            .is_ok()
        })
}

/// 技能与感知共用视线检查，未加载区块不允许发起攻击。
pub fn visible(world: &bevy_ecs::world::World, caster: Entity, target: Entity) -> bool {
    use valence::prelude::{BlockPos, ChunkLayer};
    let Some(layer) = world
        .get::<EntityLayerId>(caster)
        .and_then(|id| world.get::<ChunkLayer>(id.0))
    else {
        return false;
    };
    let Some((from, to)) = world
        .get::<Position>(caster)
        .zip(world.get::<Position>(target))
    else {
        return false;
    };
    let start = from.get() + DVec3::Y;
    let delta = to.get() + DVec3::Y - start;
    let steps = (delta.length() / 0.25).ceil().max(1.0) as usize;
    (0..=steps).all(|step| {
        let p = start + delta * (step as f64 / steps as f64);
        layer
            .block(BlockPos::new(
                p.x.floor() as i32,
                p.y.floor() as i32,
                p.z.floor() as i32,
            ))
            .is_some_and(|block| block.state.is_air())
    })
}

pub fn valid_target(world: &bevy_ecs::world::World, caster: Entity, target: Entity) -> bool {
    caster != target
        && alive(world, target)
        && !world
            .get::<valence::prelude::GameMode>(target)
            .is_some_and(|mode| {
                matches!(
                    mode,
                    valence::prelude::GameMode::Creative | valence::prelude::GameMode::Spectator
                )
            })
        && world
            .get::<EntityLayerId>(caster)
            .zip(world.get::<EntityLayerId>(target))
            .is_some_and(|(a, b)| a.0 == b.0)
}

pub fn can_cast(
    world: &bevy_ecs::world::World,
    caster: Entity,
    target: Entity,
    id: &str,
) -> Result<TechniqueDefinition, CastRejectReason> {
    let definition = world
        .get_resource::<TechniqueRegistry>()
        .and_then(|registry| registry.get(id))
        .cloned()
        .ok_or(CastRejectReason::TechniqueInactive)?;
    if interrupted(world, caster) || world.get::<WildlifeCast>(caster).is_some() {
        return Err(CastRejectReason::InRecovery);
    }
    if !valid_target(world, caster, target) {
        return Err(CastRejectReason::InvalidTarget);
    }
    let known = world
        .get::<KnownTechniques>(caster)
        .ok_or(CastRejectReason::TechniqueInactive)?;
    if !known
        .entries
        .iter()
        .any(|entry| entry.id == id && entry.active)
    {
        return Err(CastRejectReason::TechniqueInactive);
    }
    let cultivation = world
        .get::<Cultivation>(caster)
        .ok_or(CastRejectReason::RealmTooLow)?;
    if cultivation.realm.rank() < definition.required_realm_value().rank() {
        return Err(CastRejectReason::RealmTooLow);
    }
    if !definition.required_race.allows(&cultivation.race, false) {
        return Err(CastRejectReason::RaceMismatch);
    }
    let meridians = world
        .get::<MeridianSystem>(caster)
        .ok_or(CastRejectReason::MERIDIAN_SEVERED)?;
    check_skill_channels(
        &definition.required_meridians,
        meridians,
        world.get::<MeridianSeveredPermanent>(caster),
    )
    .map_err(|_| CastRejectReason::MERIDIAN_SEVERED)?;
    if world
        .get_resource::<NpcCooldownMap>()
        .is_some_and(|cooldowns| cooldowns.is_on_cooldown(caster, id, now(world)))
    {
        return Err(CastRejectReason::OnCooldown);
    }
    if cultivation.qi_current < definition.qi_cost {
        return Err(CastRejectReason::QiInsufficient);
    }
    if !world
        .get::<Stamina>(caster)
        .is_some_and(|stamina| stamina.current >= definition.stamina_cost)
    {
        return Err(CastRejectReason::InRecovery);
    }
    let positions = world
        .get::<Position>(caster)
        .zip(world.get::<Position>(target));
    if !positions
        .is_some_and(|(from, to)| from.get().distance(to.get()) <= f64::from(definition.range))
        || !visible(world, caster, target)
    {
        return Err(CastRejectReason::InvalidTarget);
    }
    Ok(definition)
}

fn spend_qi(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    amount: f64,
) -> Result<(), CastRejectReason> {
    if amount == 0.0 {
        return Ok(());
    }
    let mut cultivation = world
        .get::<Cultivation>(caster)
        .cloned()
        .ok_or(CastRejectReason::QiInsufficient)?;
    let identity = world
        .get::<LifeRecord>(caster)
        .and_then(|record| ActorQiIdentity::from_life_record(record, ActorQiKind::Npc).ok())
        .ok_or(CastRejectReason::QiInsufficient)?;
    let mut ledger = world
        .get_resource::<WorldQiAccount>()
        .cloned()
        .ok_or(CastRejectReason::QiInsufficient)?;
    let position = world
        .get::<Position>(caster)
        .ok_or(CastRejectReason::InvalidTarget)?
        .get();
    let mut zone = world
        .get_resource::<ZoneRegistry>()
        .and_then(|zones| {
            zones.find_zone(crate::world::dimension::DimensionKind::Overworld, position)
        })
        .cloned();
    let outcome = cultivation
        .release_to_zone(
            zone.as_mut(),
            &mut ledger,
            &identity,
            amount,
            QiTransferReason::ReleaseToZone,
        )
        .map_err(|_| CastRejectReason::QiInsufficient)?;
    if let Some(zone) = zone {
        let name = zone.name.clone();
        *world
            .resource_mut::<ZoneRegistry>()
            .find_zone_mut(&name)
            .expect("已验证 zone") = zone;
    }
    world.entity_mut(caster).insert(cultivation);
    *world.resource_mut::<WorldQiAccount>() = ledger;
    if let Some(mut events) = world.get_resource_mut::<Events<QiTransfer>>() {
        events.send_batch(outcome.transfers);
    }
    Ok(())
}

fn start(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    target: Option<Entity>,
    skill: WildlifeSkill,
) -> CastResult {
    let result = (|| {
        let target = target.ok_or(CastRejectReason::InvalidTarget)?;
        let definition = can_cast(world, caster, target, skill.id())?;
        let position = world.get::<Position>(caster).expect("已检查位置").get();
        let target_position = world.get::<Position>(target).expect("已检查目标位置").get();
        let direction = (target_position - position).normalize_or_zero();
        spend_qi(world, caster, definition.qi_cost)?;
        let tick = now(world);
        let mut stamina = world.get_mut::<Stamina>(caster).expect("已检查体力");
        stamina.current -= definition.stamina_cost;
        stamina.last_drain_tick = Some(tick);
        let hit_at = tick + u64::from(definition.cast_ticks);
        let active_until = hit_at + skill.active_ticks();
        world.entity_mut(caster).insert(WildlifeCast {
            skill,
            target,
            hit_at,
            active_until,
            finish_at: active_until + 10,
            direction,
            hit: false,
            submitted_at: None,
            last_tick: None,
        });
        world.resource_mut::<NpcCooldownMap>().set(
            caster,
            skill.id(),
            tick + u64::from(definition.cooldown_ticks),
        );
        animate(
            world,
            caster,
            skill.animation(),
            (definition.cast_ticks + skill.active_ticks() as u32).min(200) as u16,
        );
        let recipe = skill.id().replace('.', "_");
        if let Some(mut events) = world.get_resource_mut::<Events<PlaySoundRecipeRequest>>() {
            events.send(crate::fauna::experience::play_audio(
                &recipe, position, 1.0, 0.0,
            ));
        }
        Ok(CastResult::Started {
            cooldown_ticks: u64::from(definition.cooldown_ticks),
            anim_duration_ticks: definition.cast_ticks,
        })
    })();
    result.unwrap_or_else(|reason| CastResult::Rejected { reason })
}

pub fn animate(world: &mut bevy_ecs::world::World, entity: Entity, animation: &str, ticks: u16) {
    let Some((id, position)) = world
        .get::<EntityId>(entity)
        .zip(world.get::<Position>(entity))
        .map(|(id, position)| (id.get(), position.get()))
    else {
        return;
    };
    if let Some(mut events) = world.get_resource_mut::<Events<VfxEventRequest>>() {
        events.send(VfxEventRequest::new(
            position,
            VfxEventPayloadV1::PlayEntityAnim {
                entity_id: id,
                anim: animation.to_string(),
                duration_ticks: ticks,
            },
        ));
    }
}

fn emit_skill_particle(
    world: &mut bevy_ecs::world::World,
    skill: WildlifeSkill,
    position: DVec3,
    direction: DVec3,
) {
    let (color, strength, count, duration_ticks) = match skill {
        // 低位上扬的尘环：扑击起势时脚下腾尘。
        WildlifeSkill::Pounce => ("#C3A57A", 0.80, 12, 18),
        // 撕咬的红色飞屑：用单独 event_id 走 Point 血屑路线。
        WildlifeSkill::Rend => ("#A83232", 0.90, 10, 16),
        // 俯冲的冷色气流：方向由 caster→target 传给 Ribbon。
        WildlifeSkill::Dive => ("#A8D8E8", 0.75, 8, 14),
        // 蹄踏的土色贴地冲击环。
        WildlifeSkill::Trample => ("#8A6A44", 0.95, 12, 20),
        // 后踢的暖色定向冲击束。
        WildlifeSkill::Kick => ("#E0B060", 0.80, 8, 12),
    };
    let Some(mut events) = world.get_resource_mut::<Events<VfxEventRequest>>() else {
        return;
    };
    events.send(VfxEventRequest::new(
        position,
        VfxEventPayloadV1::SpawnParticle {
            event_id: skill.particle_event_id().to_string(),
            origin: [position.x, position.y, position.z],
            direction: Some([direction.x, direction.y, direction.z]),
            color: Some(color.to_string()),
            strength: Some(strength),
            count: Some(count),
            duration_ticks: Some(duration_ticks),
        },
    ));
}

pub fn tick_casts(world: &mut bevy_ecs::world::World) {
    let tick = now(world);
    let casts: Vec<_> = world
        .query::<(Entity, &WildlifeCast)>()
        .iter(world)
        .map(|(entity, cast)| (entity, cast.clone()))
        .collect();
    for (entity, mut cast) in casts {
        if interrupted(world, entity)
            || !valid_target(world, entity, cast.target)
            || !channels_ready(world, entity, cast.skill.id())
            || tick >= cast.finish_at
        {
            world.entity_mut(entity).remove::<WildlifeCast>();
            if let Some(mut brain) = world.get_mut::<WildlifeBrain>(entity) {
                brain.last_attack_hit = cast.hit;
                brain.last_skill = Some(cast.skill);
                brain.recover_until = tick
                    + match cast.skill {
                        WildlifeSkill::Pounce if cast.hit => 4,
                        WildlifeSkill::Dive => 80,
                        WildlifeSkill::Trample => 60,
                        _ => 20,
                    };
            }
            continue;
        }
        if cast.last_tick == Some(tick) {
            continue;
        }
        cast.last_tick = Some(tick);
        if tick >= cast.hit_at && tick <= cast.active_until && cast.submitted_at.is_none() {
            let from = world.get::<Position>(entity).expect("攻击者位置").get();
            let to = world.get::<Position>(cast.target).expect("目标位置").get();
            let reach = if cast.skill == WildlifeSkill::Trample {
                2.8
            } else {
                2.6
            };
            if from.distance(to) <= reach && visible(world, entity, cast.target) {
                world
                    .resource_mut::<Events<AttackIntent>>()
                    .send(AttackIntent {
                        attacker: entity,
                        target: Some(cast.target),
                        issued_at_tick: tick,
                        reach: AttackReach::new(reach as f32, 0.0),
                        qi_invest: 0.0,
                        wound_kind: if matches!(
                            cast.skill,
                            WildlifeSkill::Trample | WildlifeSkill::Kick
                        ) {
                            WoundKind::Blunt
                        } else {
                            WoundKind::Pierce
                        },
                        source: AttackSource::NpcMelee,
                        debug_command: None,
                    });
                cast.submitted_at = Some(tick);
                emit_skill_particle(world, cast.skill, from, cast.direction);
            }
        }
        world.entity_mut(entity).insert(cast);
    }
}

pub fn register_skills(registry: &mut SkillRegistry) {
    registry.register("lion.pounce", |world, caster, _, target| {
        start(world, caster, target, WildlifeSkill::Pounce)
    });
    registry.register("lion.rend", |world, caster, _, target| {
        start(world, caster, target, WildlifeSkill::Rend)
    });
    registry.register("vulture.dive", |world, caster, _, target| {
        start(world, caster, target, WildlifeSkill::Dive)
    });
    registry.register("horse.trample", |world, caster, _, target| {
        start(world, caster, target, WildlifeSkill::Trample)
    });
    registry.register("horse.kick", |world, caster, _, target| {
        start(world, caster, target, WildlifeSkill::Kick)
    });
}

pub fn declare_dependencies(deps: &mut SkillMeridianDependencies) {
    deps.declare_channels("lion.pounce", vec!["lion_stride".into()]);
    deps.declare_channels("lion.rend", vec!["lion_jaw".into()]);
    deps.declare_channels("vulture.dive", vec!["vulture_wing".into()]);
    deps.declare_channels("horse.trample", vec!["horse_stride".into()]);
    deps.declare_channels("horse.kick", vec!["horse_stride".into()]);
}

#[cfg(test)]
mod tests {
    use super::WildlifeSkill;

    #[test]
    fn beast_skills_have_distinct_particle_routes() {
        let skills = [
            WildlifeSkill::Pounce,
            WildlifeSkill::Rend,
            WildlifeSkill::Dive,
            WildlifeSkill::Trample,
            WildlifeSkill::Kick,
        ];
        let ids: Vec<_> = skills
            .into_iter()
            .map(WildlifeSkill::particle_event_id)
            .collect();

        assert_eq!(ids.len(), 5);
        let unique: std::collections::HashSet<_> = ids.iter().copied().collect();
        assert_eq!(unique.len(), ids.len(), "五招必须各自拥有独立粒子 event_id");
        assert!(ids.iter().all(|id| *id != "bong:fauna_spawn_dust"));
    }
}
