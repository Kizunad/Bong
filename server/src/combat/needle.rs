use serde::{Deserialize, Serialize};
use valence::prelude::{
    bevy_ecs, Commands, Component, DVec3, Entity, Event, EventReader, EventWriter, Position, Query,
    Res,
};

use crate::combat::components::{Lifecycle, LifecycleState, Stamina, WoundKind};
use crate::combat::events::{AttackIntent, AttackReach, AttackSource};
use crate::combat::CombatClock;
use crate::cultivation::components::{Cultivation, QiColor, Realm};

type NeedleActorQueryItem<'a> = (
    &'a mut Cultivation,
    &'a mut Stamina,
    Option<&'a Lifecycle>,
    Option<&'a Position>,
    Option<&'a QiColor>,
);

pub const QI_NEEDLE_SKILL_ID: &str = "dugu.shoot_needle";
pub const QI_NEEDLE_QI_COST: f64 = 1.0;
pub const QI_NEEDLE_STAMINA_COST: f32 = 2.0;
pub const QI_NEEDLE_SPEED_BLOCKS_PER_SEC: f32 = 90.0;
pub const QI_NEEDLE_MAX_DISTANCE_BLOCKS: f32 = 50.0;
pub const QI_NEEDLE_HITBOX_INFLATION: f32 = 0.6;
pub const QI_NEEDLE_COOLDOWN_TICKS: u64 = 12;

pub const QI_NEEDLE_REACH: AttackReach = AttackReach {
    base: QI_NEEDLE_MAX_DISTANCE_BLOCKS,
    step_bonus: 0.0,
    max: QI_NEEDLE_MAX_DISTANCE_BLOCKS,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntentSource {
    Client,
    SkillBar,
    Test,
}

#[derive(Debug, Clone, Event, Serialize, Deserialize)]
pub struct ShootNeedleIntent {
    pub shooter: Entity,
    pub target: Option<Entity>,
    pub dir_unit: [f64; 3],
    pub source: IntentSource,
}

#[derive(Debug, Clone, Component, Serialize, Deserialize)]
pub struct QiNeedle {
    pub shooter: Entity,
    pub qi_payload: f64,
    pub qi_color: String,
    pub infused_dugu: bool,
    pub spawn_pos: [f64; 3],
    pub velocity: [f64; 3],
    pub max_distance: f32,
    pub hitbox_inflation: f32,
    pub spawned_at_tick: u64,
}

#[derive(Debug, Clone, Event, PartialEq, Serialize, Deserialize)]
pub struct QiNeedleChargedEvent {
    pub shooter: Entity,
    pub target: Option<Entity>,
    pub tick: u64,
}

pub fn resolve_shoot_needle_intents(
    mut commands: Commands,
    clock: Res<CombatClock>,
    mut intents: EventReader<ShootNeedleIntent>,
    mut actors: Query<NeedleActorQueryItem<'_>>,
    mut attacks: EventWriter<AttackIntent>,
    mut charged_events: EventWriter<QiNeedleChargedEvent>,
) {
    for intent in intents.read() {
        let Ok((mut cultivation, mut stamina, lifecycle, position, qi_color)) =
            actors.get_mut(intent.shooter)
        else {
            continue;
        };
        if !can_shoot_needle(&cultivation, &stamina, lifecycle) {
            continue;
        }

        cultivation.qi_current =
            (cultivation.qi_current - QI_NEEDLE_QI_COST).clamp(0.0, cultivation.qi_max);
        stamina.current = (stamina.current - QI_NEEDLE_STAMINA_COST).clamp(0.0, stamina.max);

        let dir = normalized_dir(intent.dir_unit);
        let spawn_pos = position
            .map(|position| position.get() + dir * 0.3)
            .unwrap_or(DVec3::ZERO);
        commands.spawn(QiNeedle {
            shooter: intent.shooter,
            qi_payload: QI_NEEDLE_QI_COST,
            qi_color: qi_color_label(qi_color),
            infused_dugu: false,
            spawn_pos: [spawn_pos.x, spawn_pos.y, spawn_pos.z],
            velocity: [
                dir.x * f64::from(QI_NEEDLE_SPEED_BLOCKS_PER_SEC),
                dir.y * f64::from(QI_NEEDLE_SPEED_BLOCKS_PER_SEC),
                dir.z * f64::from(QI_NEEDLE_SPEED_BLOCKS_PER_SEC),
            ],
            max_distance: QI_NEEDLE_MAX_DISTANCE_BLOCKS,
            hitbox_inflation: QI_NEEDLE_HITBOX_INFLATION,
            spawned_at_tick: clock.tick,
        });
        if let Some(target) = intent.target {
            attacks.send(AttackIntent {
                attacker: intent.shooter,
                target: Some(target),
                issued_at_tick: clock.tick,
                reach: QI_NEEDLE_REACH,
                qi_invest: QI_NEEDLE_QI_COST as f32,
                wound_kind: WoundKind::Pierce,
                source: AttackSource::QiNeedle,
                debug_command: None,
            });
        }
        charged_events.send(QiNeedleChargedEvent {
            shooter: intent.shooter,
            target: intent.target,
            tick: clock.tick,
        });
    }
}

pub fn despawn_expired_qi_needles(
    mut commands: Commands,
    clock: Res<CombatClock>,
    needles: Query<(Entity, &QiNeedle)>,
) {
    let max_age_ticks =
        ((QI_NEEDLE_MAX_DISTANCE_BLOCKS / QI_NEEDLE_SPEED_BLOCKS_PER_SEC) * 20.0).ceil() as u64 + 1;
    for (entity, needle) in &needles {
        if clock.tick.saturating_sub(needle.spawned_at_tick) > max_age_ticks {
            commands.entity(entity).despawn();
        }
    }
}

pub fn can_shoot_needle(
    cultivation: &Cultivation,
    stamina: &Stamina,
    lifecycle: Option<&Lifecycle>,
) -> bool {
    realm_rank(cultivation.realm) >= realm_rank(Realm::Induce)
        && cultivation.qi_current + f64::EPSILON >= QI_NEEDLE_QI_COST
        && stamina.current + f32::EPSILON >= QI_NEEDLE_STAMINA_COST
        && lifecycle.is_none_or(|lifecycle| lifecycle.state == LifecycleState::Alive)
}

pub fn normalized_dir(dir: [f64; 3]) -> DVec3 {
    let raw = DVec3::new(dir[0], dir[1], dir[2]);
    if raw.length_squared() <= f64::EPSILON {
        DVec3::new(0.0, 0.0, 1.0)
    } else {
        raw.normalize()
    }
}

fn qi_color_label(qi_color: Option<&QiColor>) -> String {
    qi_color
        .map(|qi_color| format!("{:?}", qi_color.main))
        .unwrap_or_else(|| "Mellow".to_string())
}

pub fn realm_rank(realm: Realm) -> u8 {
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

    use valence::prelude::{App, Events, Update};

    fn actor(realm: Realm, qi_current: f64, stamina_current: f32) -> (Cultivation, Stamina) {
        (
            Cultivation {
                realm,
                qi_current,
                qi_max: 100.0,
                ..Cultivation::default()
            },
            Stamina {
                current: stamina_current,
                ..Stamina::default()
            },
        )
    }

    #[test]
    fn shoot_needle_rejects_awaken_or_empty_pools() {
        let (cultivation, stamina) = actor(Realm::Awaken, 10.0, 10.0);
        assert!(!can_shoot_needle(&cultivation, &stamina, None));

        let (cultivation, stamina) = actor(Realm::Induce, 0.5, 10.0);
        assert!(!can_shoot_needle(&cultivation, &stamina, None));

        let (cultivation, stamina) = actor(Realm::Induce, 10.0, 1.0);
        assert!(!can_shoot_needle(&cultivation, &stamina, None));
    }

    #[test]
    fn shoot_needle_consumes_qi_and_emits_attack_for_target() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 9 });
        app.add_event::<ShootNeedleIntent>();
        app.add_event::<AttackIntent>();
        app.add_event::<QiNeedleChargedEvent>();
        app.add_systems(Update, resolve_shoot_needle_intents);

        let (cultivation, stamina) = actor(Realm::Induce, 10.0, 10.0);
        let shooter = app
            .world_mut()
            .spawn((cultivation, stamina, Lifecycle::default()))
            .id();
        let target = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(ShootNeedleIntent {
            shooter,
            target: Some(target),
            dir_unit: [0.0, 0.0, 2.0],
            source: IntentSource::Test,
        });

        app.update();

        let cultivation = app.world().get::<Cultivation>(shooter).unwrap();
        let stamina = app.world().get::<Stamina>(shooter).unwrap();
        assert_eq!(cultivation.qi_current, 9.0);
        assert_eq!(stamina.current, 8.0);
        let attack_events = app.world().resource::<Events<AttackIntent>>();
        assert!(attack_events
            .get_reader()
            .read(attack_events)
            .any(|event| event.source == AttackSource::QiNeedle && event.target == Some(target)));
        let needle_count = {
            let world = app.world_mut();
            let mut query = world.query::<&QiNeedle>();
            query.iter(world).count()
        };
        assert_eq!(needle_count, 1);
    }
}
