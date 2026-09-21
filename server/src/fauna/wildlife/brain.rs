//! BigBrain 只选择行为；物种战术负责目标、站位和调用已注册技能。

use big_brain::prelude::*;
use valence::client::ClientMarker;
use valence::prelude::*;

use crate::combat::components::{Stamina, Wounds};
use crate::combat::events::{AttackSource, CombatEvent};
use crate::combat::CombatSystemSet;
use crate::cultivation::components::Cultivation;
use crate::cultivation::skill_registry::{CastResult, SkillRegistry};
use crate::fauna::components::{BeastKind, FaunaTag};
use crate::fauna::mundane::MundaneFaunaSpecies;
use crate::npc::hunger::Hunger;
use crate::npc::movement::GameTick;
use crate::npc::navigator::Navigator;

use super::config::WildlifeDefinition;
use super::skills::{self, WildlifeCast, WildlifeSkill};

#[derive(Clone, Debug, Component)]
pub struct WildlifeBrain {
    pub kind: BeastKind,
    pub home: DVec3,
    pub herd: u64,
    pub definition: WildlifeDefinition,
    pub target: Option<Entity>,
    pub threat_until: u64,
    pub rally_until: u64,
    pub recover_until: u64,
    pub last_attack_hit: bool,
    pub last_skill: Option<WildlifeSkill>,
    pub airborne: bool,
    pub flight_ready_at: u64,
    pub flight_goal: DVec3,
    pub next_idle_at: u64,
}

impl WildlifeBrain {
    pub fn new(kind: BeastKind, home: DVec3, herd: u64, definition: WildlifeDefinition) -> Self {
        Self {
            kind,
            home,
            herd,
            definition,
            target: None,
            threat_until: 0,
            rally_until: 0,
            recover_until: 0,
            last_attack_hit: false,
            last_skill: None,
            airborne: false,
            flight_ready_at: 0,
            flight_goal: home,
            next_idle_at: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Component, PartialEq, Eq)]
enum Behavior {
    Retreat,
    Fight,
    Home,
    Rest,
    Wander,
}

impl ScorerBuilder for Behavior {
    fn build(&self, commands: &mut Commands, entity: Entity, _: Entity) {
        commands.entity(entity).insert(*self);
    }
    fn label(&self) -> Option<&str> {
        Some("WildlifeNeed")
    }
}

impl ActionBuilder for Behavior {
    fn build(&self, commands: &mut Commands, entity: Entity, _: Entity) {
        commands.entity(entity).insert(*self);
    }
    fn label(&self) -> Option<&str> {
        Some("WildlifeBehavior")
    }
}

pub fn wildlife_thinker() -> ThinkerBuilder {
    Thinker::build()
        .label("Wildlife")
        .picker(FirstToScore { threshold: 0.05 })
        .when(Behavior::Retreat, Behavior::Retreat)
        .when(Behavior::Fight, Behavior::Fight)
        .when(Behavior::Home, Behavior::Home)
        .when(Behavior::Rest, Behavior::Rest)
        .when(Behavior::Wander, Behavior::Wander)
}

/// 感知只在同层、可见、有效生命目标中选取。马不会主动攻击路人。
fn sense(world: &mut bevy_ecs::world::World) {
    let tick = skills::now(world);
    if !tick.is_multiple_of(10) {
        return;
    }
    let actors: Vec<_> = world
        .query::<(Entity, &WildlifeBrain, &Position)>()
        .iter(world)
        .map(|(id, brain, position)| (id, brain.clone(), position.get()))
        .collect();
    let candidates: Vec<_> = world
        .query::<(
            Entity,
            &Position,
            &Wounds,
            Option<&FaunaTag>,
            Option<&ClientMarker>,
            Option<&MundaneFaunaSpecies>,
        )>()
        .iter(world)
        .filter(|(_, _, _, fauna, client, mundane)| {
            fauna.is_some() || client.is_some() || mundane.is_some()
        })
        .map(|(id, p, wounds, fauna, _, _)| {
            (
                id,
                p.get(),
                wounds.health_current / wounds.health_max.max(1.0),
                fauna.map(|tag| tag.beast_kind),
            )
        })
        .collect();
    for (entity, mut brain, position) in actors {
        if !skills::alive(world, entity) {
            continue;
        }
        brain.target = brain.target.filter(|target| {
            tick < brain.threat_until
                && skills::valid_target(world, entity, *target)
                && world.get::<Position>(*target).is_some_and(|p| {
                    p.get().distance(position) <= 36.0 && p.get().distance(brain.home) <= 64.0
                })
        });
        if brain.target.is_none()
            && brain.kind != BeastKind::Horse
            && position.distance(brain.home) < 40.0
        {
            let hungry = world
                .get::<Hunger>(entity)
                .is_some_and(|h| h.hunger_pressure() > 0.35);
            brain.target = candidates
                .iter()
                .filter(|(id, p, health, kind)| {
                    let distance = position.distance(*p);
                    *kind != Some(brain.kind)
                        && skills::valid_target(world, entity, *id)
                        && distance < 20.0
                        && skills::visible(world, entity, *id)
                        && match brain.kind {
                            BeastKind::DainuLion => distance < 7.0 || hungry,
                            BeastKind::FuyuVulture => {
                                *health < 0.55
                                    && (distance < 10.0 || hungry)
                                    && world
                                        .get::<Cultivation>(*id)
                                        .is_some_and(|c| c.realm.rank() <= 2)
                            }
                            _ => false,
                        }
                })
                .min_by(|a, b| {
                    position
                        .distance_squared(a.1)
                        .total_cmp(&position.distance_squared(b.1))
                })
                .map(|entry| entry.0);
            if brain.target.is_some() {
                brain.threat_until = tick + 300;
            }
        }
        world.entity_mut(entity).insert(brain);
    }
}

fn scores(
    actors: Query<(&WildlifeBrain, &Position, &Wounds, &Stamina)>,
    mut scorers: Query<(&Actor, &Behavior, &mut Score)>,
) {
    for (Actor(entity), behavior, mut score) in &mut scorers {
        let value = actors
            .get(*entity)
            .map(|(brain, p, wounds, stamina)| match behavior {
                Behavior::Retreat => {
                    if brain.target.is_some()
                        && (wounds.health_current
                            < wounds.health_max * brain.definition.retreat_health
                            || stamina.current < stamina.max * 0.15)
                    {
                        1.0
                    } else {
                        0.0
                    }
                }
                Behavior::Fight => {
                    if brain.target.is_some() {
                        0.9
                    } else {
                        0.0
                    }
                }
                Behavior::Home => {
                    if p.get().distance(brain.home) > 16.0 {
                        0.7
                    } else {
                        0.0
                    }
                }
                Behavior::Rest => {
                    if stamina.current < stamina.max * 0.6 {
                        0.6
                    } else {
                        0.0
                    }
                }
                Behavior::Wander => 0.1,
            })
            .unwrap_or(0.0);
        score.set(value);
    }
}

fn actions(world: &mut bevy_ecs::world::World) {
    let entries: Vec<_> = world
        .query::<(Entity, &Actor, &Behavior, &ActionState)>()
        .iter(world)
        .map(|(id, actor, behavior, state)| (id, actor.0, *behavior, state.clone()))
        .collect();
    for (action, actor, behavior, state) in entries {
        match state {
            ActionState::Requested => {
                world.entity_mut(action).insert(ActionState::Executing);
            }
            ActionState::Executing => {
                if skills::interrupted(world, actor) {
                    stop(world, actor);
                    world.entity_mut(action).insert(ActionState::Failure);
                    continue;
                }
                let Some(mut brain) = world.get::<WildlifeBrain>(actor).cloned() else {
                    world.entity_mut(action).insert(ActionState::Failure);
                    continue;
                };
                let Some(position) = world.get::<Position>(actor).map(|p| p.get()) else {
                    world.entity_mut(action).insert(ActionState::Failure);
                    continue;
                };
                act(world, actor, &mut brain, position, behavior);
                world.entity_mut(actor).insert(brain);
            }
            ActionState::Cancelled => {
                stop(world, actor);
                world.entity_mut(action).insert(ActionState::Failure);
            }
            _ => {}
        }
    }
}

fn stop(world: &mut bevy_ecs::world::World, entity: Entity) {
    if let Some(mut nav) = world.get_mut::<Navigator>(entity) {
        nav.stop();
    }
}

fn goal(
    world: &mut bevy_ecs::world::World,
    entity: Entity,
    brain: &mut WildlifeBrain,
    point: DVec3,
    speed: f64,
) {
    if brain.airborne {
        brain.flight_goal = point;
    } else if let Some(mut nav) = world.get_mut::<Navigator>(entity) {
        nav.set_goal(point, speed);
    }
}

fn cast(world: &mut bevy_ecs::world::World, entity: Entity, target: Entity, id: &str) -> bool {
    let callback = world.resource::<SkillRegistry>().lookup(id);
    callback.is_some_and(|skill| {
        matches!(
            skill(world, entity, 0, Some(target)),
            CastResult::Started { .. }
        )
    })
}

fn act(
    world: &mut bevy_ecs::world::World,
    entity: Entity,
    brain: &mut WildlifeBrain,
    position: DVec3,
    behavior: Behavior,
) {
    let tick = skills::now(world);
    if world.get::<WildlifeCast>(entity).is_some() {
        stop(world, entity);
        return;
    }
    let target = brain
        .target
        .filter(|target| skills::valid_target(world, entity, *target))
        .and_then(|target| world.get::<Position>(target).map(|p| (target, p.get())));
    if let Some((target, target_pos)) = target {
        let away = (position - target_pos).with_y(0.0).normalize_or_zero();
        if behavior == Behavior::Retreat {
            goal(
                world,
                entity,
                brain,
                position + away * 14.0,
                brain.definition.speed * 1.5,
            );
            return;
        }
        if behavior == Behavior::Fight {
            match brain.kind {
                BeastKind::DainuLion => {
                    if tick < brain.recover_until {
                        stop(world, entity);
                        return;
                    }
                    let followup =
                        brain.last_skill == Some(WildlifeSkill::Pounce) && brain.last_attack_hit;
                    let id = if followup {
                        &brain.definition.followup_skill
                    } else {
                        &brain.definition.engage_skill
                    };
                    if cast(world, entity, target, id) {
                        brain.last_attack_hit = false;
                        stop(world, entity);
                    } else if position.distance(target_pos) > 2.0 {
                        goal(world, entity, brain, target_pos, brain.definition.speed);
                    } else {
                        stop(world, entity);
                    }
                }
                BeastKind::FuyuVulture => {
                    if !brain.airborne {
                        brain.airborne = true;
                        brain.flight_ready_at = tick + 11;
                        skills::animate(world, entity, "animation.bong.fuyu_vulture.unfold", 11);
                    }
                    // 先爬升侧绕，冷却结束后从外圈俯冲；俯冲方向在起手锁定。
                    let angle = (tick as f64 / 55.0) + (entity.index() % 7) as f64;
                    let orbit =
                        target_pos + DVec3::new(angle.cos() * 10.0, 5.0, angle.sin() * 10.0);
                    brain.flight_goal = orbit;
                    if tick >= brain.flight_ready_at
                        && tick >= brain.recover_until
                        && position.y > target_pos.y + 3.0
                        && position.distance(target_pos) > 6.0
                    {
                        cast(world, entity, target, &brain.definition.engage_skill);
                    }
                    stop(world, entity);
                }
                BeastKind::Horse => {
                    let allies = world
                        .query::<(Entity, &WildlifeBrain, &Position)>()
                        .iter(world)
                        .filter(|(other, b, p)| {
                            *other != entity
                                && b.kind == BeastKind::Horse
                                && b.herd == brain.herd
                                && b.target == Some(target)
                                && p.get().distance(position) < 20.0
                                && skills::valid_target(world, entity, *other)
                        })
                        .count();
                    if tick < brain.recover_until {
                        goal(
                            world,
                            entity,
                            brain,
                            position + away * 9.0,
                            brain.definition.speed,
                        );
                    } else if allies > 0 {
                        let lane = (entity.index() % 3) as f64 - 1.0;
                        let side = DVec3::new(-away.z, 0.0, away.x) * lane * 2.5;
                        if tick >= brain.rally_until
                            && cast(world, entity, target, &brain.definition.engage_skill)
                        {
                            stop(world, entity);
                        } else {
                            goal(
                                world,
                                entity,
                                brain,
                                target_pos + away * 7.0 + side,
                                brain.definition.speed,
                            );
                        }
                    } else if !cast(world, entity, target, &brain.definition.followup_skill) {
                        goal(
                            world,
                            entity,
                            brain,
                            position + away * 12.0,
                            brain.definition.speed * 1.3,
                        );
                    }
                }
                _ => {}
            }
            return;
        }
    }
    if brain.airborne {
        brain.flight_goal = brain.home;
        if position.distance(brain.home) < 1.0 {
            brain.airborne = false;
            skills::animate(world, entity, "animation.bong.fuyu_vulture_flight.land", 24);
        }
        return;
    }
    match behavior {
        Behavior::Home => goal(
            world,
            entity,
            brain,
            brain.home,
            brain.definition.speed * 0.7,
        ),
        Behavior::Rest => stop(world, entity),
        _ if tick >= brain.next_idle_at => {
            brain.next_idle_at = tick + 160 + u64::from(entity.index() % 60);
            if (tick / 160 + u64::from(entity.index())).is_multiple_of(3) {
                stop(world, entity);
                let animation = match brain.kind {
                    BeastKind::Horse => "animation.bong.horse.graze",
                    BeastKind::FuyuVulture => "animation.bong.fuyu_vulture.peck",
                    _ => "animation.bong.dainu_lion.idle",
                };
                skills::animate(world, entity, animation, 80);
                if brain.kind == BeastKind::Horse {
                    let grass = world
                        .get::<EntityLayerId>(entity)
                        .and_then(|id| world.get::<ChunkLayer>(id.0))
                        .and_then(|layer| {
                            layer.block(BlockPos::new(
                                position.x.floor() as i32,
                                position.y.floor() as i32 - 1,
                                position.z.floor() as i32,
                            ))
                        })
                        .is_some_and(|block| block.state == BlockState::GRASS_BLOCK);
                    if grass {
                        if let Some(mut hunger) = world.get_mut::<Hunger>(entity) {
                            hunger.replenish(0.2);
                        }
                    }
                }
            } else {
                let angle = (tick as f64 + entity.index() as f64 * 17.0) * 0.13;
                goal(
                    world,
                    entity,
                    brain,
                    brain.home + DVec3::new(angle.cos() * 10.0, 0.0, angle.sin() * 10.0),
                    brain.definition.speed * 0.5,
                );
            }
        }
        _ => {}
    }
}

/// 使用结算回执确认扑击命中；被伤害的同群马共享仇恨与集结时刻。
pub(super) fn combat_feedback(
    mut events: EventReader<CombatEvent>,
    tick: Res<GameTick>,
    mut actors: Query<(
        Entity,
        &mut WildlifeBrain,
        &Position,
        &EntityLayerId,
        Option<&mut WildlifeCast>,
    )>,
) {
    for event in events.read() {
        if event.physical_damage + event.damage <= 0.0 {
            continue;
        }
        if let Ok((_, _, _, _, Some(mut cast))) = actors.get_mut(event.attacker) {
            if event.source == AttackSource::NpcMelee
                && cast.target == event.target
                && cast.submitted_at.is_some()
            {
                cast.hit = true;
            }
        }
        let Ok((_, brain, position, layer, _)) = actors.get(event.target) else {
            continue;
        };
        let (herd, kind, position, layer) = (brain.herd, brain.kind, position.get(), layer.0);
        for (id, mut brain, p, l, _) in &mut actors {
            if id != event.attacker
                && (id == event.target
                    || (kind == BeastKind::Horse
                        && brain.kind == kind
                        && brain.herd == herd
                        && l.0 == layer
                        && position.distance(p.get()) < 24.0))
            {
                if brain.target != Some(event.attacker) {
                    brain.rally_until = u64::from(tick.0) + 30;
                }
                brain.target = Some(event.attacker);
                brain.threat_until = u64::from(tick.0) + 400;
            }
        }
    }
}

fn feed_after_hunt(
    mut events: EventReader<crate::npc::lifecycle::NpcTerminalSettlementSucceeded>,
    mut hunters: Query<(&WildlifeBrain, &mut Hunger)>,
) {
    for event in events.read() {
        if let Some(attacker) = event.attacker {
            if let Ok((brain, mut hunger)) = hunters.get_mut(attacker) {
                if brain.kind != BeastKind::Horse {
                    hunger.replenish(0.6);
                }
            }
        }
    }
}

pub fn register(app: &mut App) {
    app.add_event::<crate::npc::lifecycle::NpcTerminalSettlementSucceeded>();
    app.add_systems(PreUpdate, sense.before(BigBrainSet::Scorers));
    app.add_systems(PreUpdate, scores.in_set(BigBrainSet::Scorers));
    app.add_systems(PreUpdate, actions.in_set(BigBrainSet::Actions));
    app.add_systems(Update, combat_feedback.in_set(CombatSystemSet::Emit));
    app.add_systems(
        Update,
        feed_after_hunt.in_set(crate::npc::lifecycle::NpcTerminalSystemSet::PostCommit),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::components::Lifecycle;
    use bevy_ecs::world::World;

    fn execute_action(world: &mut World, actor: Entity, behavior: Option<Behavior>) -> ActionState {
        let action = world
            .spawn((
                Actor(actor),
                ActionState::Executing,
                behavior.unwrap_or(Behavior::Wander),
            ))
            .id();
        actions(world);
        world
            .get::<ActionState>(action)
            .cloned()
            .expect("wildlife action must retain its state component")
    }

    #[test]
    fn interrupted_or_incomplete_executing_actions_fail_and_normal_actions_continue() {
        let mut interrupted_world = World::new();
        let interrupted_actor = interrupted_world.spawn_empty().id();
        assert_eq!(
            execute_action(&mut interrupted_world, interrupted_actor, None),
            ActionState::Failure,
            "an interrupted wildlife actor must leave Executing so BigBrain can replan"
        );

        let mut missing_brain_world = World::new();
        let missing_brain_actor = missing_brain_world.spawn(Lifecycle::default()).id();
        assert_eq!(
            execute_action(&mut missing_brain_world, missing_brain_actor, None),
            ActionState::Failure,
            "an actor without WildlifeBrain must not strand its action in Executing"
        );

        let mut missing_position_world = World::new();
        let definition = crate::fauna::wildlife::config::WildlifeCatalog::load()
            .get(BeastKind::Horse)
            .clone();
        let missing_position_actor = missing_position_world
            .spawn((
                Lifecycle::default(),
                WildlifeBrain::new(BeastKind::Horse, DVec3::ZERO, 0, definition),
            ))
            .id();
        assert_eq!(
            execute_action(&mut missing_position_world, missing_position_actor, None),
            ActionState::Failure,
            "an actor without Position must not strand its action in Executing"
        );

        let mut normal_world = World::new();
        let definition = crate::fauna::wildlife::config::WildlifeCatalog::load()
            .get(BeastKind::Horse)
            .clone();
        let normal_actor = normal_world
            .spawn((
                Lifecycle::default(),
                Position::new([0.0, 0.0, 0.0]),
                WildlifeBrain::new(BeastKind::Horse, DVec3::ZERO, 0, definition),
            ))
            .id();
        assert_eq!(
            execute_action(&mut normal_world, normal_actor, Some(Behavior::Wander)),
            ActionState::Executing,
            "a complete continuous wildlife action must remain Executing after act()"
        );
    }
}
