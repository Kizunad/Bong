use std::collections::HashMap;

use valence::prelude::{
    bevy_ecs, Component, Despawned, Entity, EventReader, EventWriter, ParamSet, Query, Res, ResMut,
    Resource, Without,
};

use crate::combat::components::{Lifecycle, LifecycleState};
use crate::cultivation::components::{Cultivation, Karma, Realm};
use crate::cultivation::life_record::{BiographyEntry, LifeRecord};
use crate::cultivation::lifespan::{LifespanComponent, LifespanExtensionIntent};
use crate::cultivation::tick::CultivationClock;
use crate::inventory::{
    consume_item_instance_once, inventory_item_by_instance_borrow, ItemEffect, ItemRegistry,
    PlayerInventory,
};
use crate::npc::brain::canonical_npc_id;
use crate::npc::spawn::NpcMarker;
use crate::player::state::PlayerState;
use crate::schema::death_lifecycle::DuoSheEventV1;

pub const DUO_SHE_KARMA_DELTA: f64 = 100.0;
pub const DUO_SHE_QI_MAX_FACTOR: f64 = 0.80;
pub const DUO_SHE_TAG: &str = "tag=夺舍者";
pub const DUO_SHE_COOLDOWN_TICKS: u64 = 20 * 60 * 30;

#[derive(Debug, Clone, bevy_ecs::event::Event, PartialEq, Eq)]
pub struct DuoSheRequestEvent {
    pub host: Entity,
    pub target_id: String,
}

#[derive(Debug, Clone, bevy_ecs::event::Event, PartialEq)]
pub struct DuoSheEventEmitted {
    pub payload: DuoSheEventV1,
}

#[derive(Debug, Clone, bevy_ecs::event::Event, PartialEq, Eq)]
pub struct DuoSheWarningEvent {
    pub host_id: String,
    pub target_id: String,
    pub at_tick: u64,
}

#[derive(Debug, Default, Resource)]
pub struct DuoSheCooldowns {
    last_by_host: HashMap<Entity, u64>,
}

impl DuoSheCooldowns {
    pub fn is_ready(&self, host: Entity, now_tick: u64) -> bool {
        self.last_by_host
            .get(&host)
            .is_none_or(|last_tick| now_tick.saturating_sub(*last_tick) >= DUO_SHE_COOLDOWN_TICKS)
    }

    pub fn mark(&mut self, host: Entity, now_tick: u64) {
        self.last_by_host.insert(host, now_tick);
    }
}

#[derive(Debug, Clone, Copy, Component, PartialEq, Eq)]
pub struct DuoSheMarked;

#[derive(Debug, Clone, Copy, Component, PartialEq, Eq)]
pub struct PossessedVictim;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuoSheTargetEligibility {
    Eligible,
    Ineligible,
}

type DuoSheHostItem<'a> = (
    &'a mut Cultivation,
    Option<&'a mut Karma>,
    Option<&'a mut PlayerState>,
    &'a mut LifespanComponent,
    Option<&'a mut LifeRecord>,
    Option<&'a mut Lifecycle>,
);

type DuoSheTargetReadItem<'a> = (
    Entity,
    Option<&'a NpcMarker>,
    Option<&'a Cultivation>,
    Option<&'a PlayerState>,
    Option<&'a LifespanComponent>,
    Option<&'a LifeRecord>,
    Option<&'a Lifecycle>,
);

type DuoSheTargetWriteItem<'a> = (Option<&'a mut LifeRecord>, Option<&'a mut Lifecycle>);

pub fn duo_she_target_eligibility(
    player_state: Option<&PlayerState>,
    cultivation: Option<&Cultivation>,
    is_npc: bool,
) -> DuoSheTargetEligibility {
    if player_state.is_some_and(|state| state.realm == "mortal") {
        return DuoSheTargetEligibility::Eligible;
    }
    if cultivation.is_some_and(|cultivation| cultivation.realm == Realm::Awaken) {
        return DuoSheTargetEligibility::Eligible;
    }
    if is_npc && player_state.is_none() && cultivation.is_none() {
        return DuoSheTargetEligibility::Eligible;
    }
    DuoSheTargetEligibility::Ineligible
}

#[allow(clippy::type_complexity)]
pub fn process_duo_she_requests(
    clock: Res<CultivationClock>,
    mut cooldowns: ResMut<DuoSheCooldowns>,
    mut requests: EventReader<DuoSheRequestEvent>,
    mut emitted: EventWriter<DuoSheEventEmitted>,
    mut warnings: EventWriter<DuoSheWarningEvent>,
    mut actors: ParamSet<(
        Query<DuoSheHostItem<'_>, Without<Despawned>>,
        Query<DuoSheTargetReadItem<'_>, Without<Despawned>>,
        Query<DuoSheTargetWriteItem<'_>, Without<Despawned>>,
    )>,
    mut commands: valence::prelude::Commands,
) {
    for request in requests.read() {
        if !cooldowns.is_ready(request.host, clock.tick) {
            continue;
        }
        let Some(snapshot) = ({
            let targets = actors.p1();
            resolve_target_snapshot(request.target_id.as_str(), &targets)
        }) else {
            continue;
        };
        if snapshot.entity == request.host {
            continue;
        }

        if snapshot.eligibility != DuoSheTargetEligibility::Eligible {
            continue;
        }
        let target_age = snapshot.target_age;
        let (host_id, host_prev_age) = {
            let mut hosts = actors.p0();
            let Ok((
                mut host_cultivation,
                host_karma,
                host_player_state,
                mut host_lifespan,
                host_life_record,
                host_lifecycle,
            )) = hosts.get_mut(request.host)
            else {
                continue;
            };

            let host_id = host_identifier(
                host_life_record.as_deref(),
                host_lifecycle.as_deref(),
                request.host,
            );
            let host_prev_age = host_lifespan.years_lived;

            host_lifespan.years_lived = target_age.min(host_lifespan.cap_by_realm as f64);
            host_cultivation.qi_max = (host_cultivation.qi_max * DUO_SHE_QI_MAX_FACTOR).max(1.0);
            host_cultivation.qi_current = host_cultivation.qi_current.min(host_cultivation.qi_max);
            if let Some(mut host_karma) = host_karma {
                host_karma.weight += DUO_SHE_KARMA_DELTA;
            }
            if let Some(mut player_state) = host_player_state {
                player_state.karma = 1.0;
                player_state.spirit_qi_max =
                    (player_state.spirit_qi_max * DUO_SHE_QI_MAX_FACTOR).max(1.0);
                player_state.spirit_qi = player_state.spirit_qi.min(player_state.spirit_qi_max);
            }
            if let Some(mut host_life_record) = host_life_record {
                host_life_record.push(BiographyEntry::DuoShePerformed {
                    target_id: snapshot.target_id.clone(),
                    host_prev_age,
                    target_age,
                    tick: clock.tick,
                });
                host_life_record.push(BiographyEntry::InsightTaken {
                    trigger: DUO_SHE_TAG.to_string(),
                    choice: snapshot.target_id.clone(),
                    tick: clock.tick,
                });
            }
            if let Some(mut lifecycle) = host_lifecycle {
                lifecycle.state = LifecycleState::Alive;
            }
            (host_id, host_prev_age)
        };

        {
            let mut targets = actors.p2();
            let Ok((target_life_record, target_lifecycle)) = targets.get_mut(snapshot.entity)
            else {
                continue;
            };
            if let Some(mut target_life_record) = target_life_record {
                target_life_record.push(BiographyEntry::PossessedBy {
                    host_id: host_id.clone(),
                    tick: clock.tick,
                });
                target_life_record.push(BiographyEntry::Terminated {
                    cause: format!("duoshe_by:{host_id}"),
                    tick: clock.tick,
                });
            }
            if let Some(mut lifecycle) = target_lifecycle {
                lifecycle.state = LifecycleState::Terminated;
                lifecycle.last_death_tick = Some(clock.tick);
            }
        }

        commands.entity(request.host).insert(DuoSheMarked);
        commands
            .entity(snapshot.entity)
            .insert((PossessedVictim, Despawned));
        cooldowns.mark(request.host, clock.tick);
        warnings.send(DuoSheWarningEvent {
            host_id: host_id.clone(),
            target_id: snapshot.target_id.clone(),
            at_tick: clock.tick,
        });
        emitted.send(DuoSheEventEmitted {
            payload: DuoSheEventV1 {
                v: 1,
                host_id,
                target_id: snapshot.target_id,
                at_tick: clock.tick,
                karma_delta: DUO_SHE_KARMA_DELTA,
                host_prev_age,
                target_age,
            },
        });
    }
}

pub fn process_life_core_requests(
    item_registry: Res<ItemRegistry>,
    mut requests: EventReader<UseLifeCoreEvent>,
    mut inventories: Query<&mut PlayerInventory>,
    mut extension_tx: EventWriter<LifespanExtensionIntent>,
) {
    for request in requests.read() {
        let Ok(mut inventory) = inventories.get_mut(request.entity) else {
            continue;
        };
        let Some(item) = inventory_item_by_instance_borrow(&inventory, request.instance_id) else {
            continue;
        };
        let Some(ItemEffect::LifespanExtension { years, source }) = item_registry
            .get(item.template_id.as_str())
            .and_then(|template| template.effect.as_ref())
        else {
            continue;
        };
        if source != "collapse_core" {
            continue;
        }
        let years = *years;
        let source = source.clone();
        if consume_item_instance_once(&mut inventory, request.instance_id).is_err() {
            continue;
        }
        extension_tx.send(LifespanExtensionIntent {
            entity: request.entity,
            requested_years: years,
            source,
        });
    }
}

#[derive(Debug, Clone, bevy_ecs::event::Event, PartialEq, Eq)]
pub struct UseLifeCoreEvent {
    pub entity: Entity,
    pub instance_id: u64,
}

#[derive(Debug, Clone, PartialEq)]
struct DuoSheTargetSnapshot {
    entity: Entity,
    target_id: String,
    target_age: f64,
    eligibility: DuoSheTargetEligibility,
}

fn resolve_target_snapshot(
    target_id: &str,
    targets: &Query<DuoSheTargetReadItem<'_>, Without<Despawned>>,
) -> Option<DuoSheTargetSnapshot> {
    for (entity, npc_marker, cultivation, player_state, lifespan, life_record, lifecycle) in
        targets.iter()
    {
        if life_record.is_some_and(|record| record.character_id == target_id)
            || lifecycle.is_some_and(|lifecycle| lifecycle.character_id == target_id)
            || canonical_npc_id(entity) == target_id
        {
            return Some(DuoSheTargetSnapshot {
                entity,
                target_id: target_identifier(entity, life_record, lifecycle),
                target_age: lifespan
                    .map(|lifespan| lifespan.years_lived)
                    .unwrap_or(18.0),
                eligibility: duo_she_target_eligibility(
                    player_state,
                    cultivation,
                    npc_marker.is_some(),
                ),
            });
        }
    }
    None
}

fn host_identifier(
    life_record: Option<&LifeRecord>,
    lifecycle: Option<&Lifecycle>,
    entity: Entity,
) -> String {
    life_record
        .map(|record| record.character_id.clone())
        .or_else(|| lifecycle.map(|lifecycle| lifecycle.character_id.clone()))
        .unwrap_or_else(|| format!("entity:{entity:?}"))
}

fn target_identifier(
    entity: Entity,
    life_record: Option<&LifeRecord>,
    lifecycle: Option<&Lifecycle>,
) -> String {
    life_record
        .map(|record| record.character_id.clone())
        .or_else(|| lifecycle.map(|lifecycle| lifecycle.character_id.clone()))
        .unwrap_or_else(|| canonical_npc_id(entity))
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::{App, Events, Update};

    #[test]
    fn eligibility_allows_mortal_and_awaken_only() {
        let mortal = PlayerState::default();
        assert_eq!(
            duo_she_target_eligibility(Some(&mortal), None, false),
            DuoSheTargetEligibility::Eligible
        );

        let induce = Cultivation {
            realm: Realm::Induce,
            ..Default::default()
        };
        assert_eq!(
            duo_she_target_eligibility(None, Some(&induce), false),
            DuoSheTargetEligibility::Ineligible
        );
    }

    #[test]
    fn process_duo_she_marks_host_and_terminates_target() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 123 });
        app.insert_resource(DuoSheCooldowns::default());
        app.add_event::<DuoSheRequestEvent>();
        app.add_event::<DuoSheEventEmitted>();
        app.add_event::<DuoSheWarningEvent>();
        app.add_systems(Update, process_duo_she_requests);

        let mut host_lifespan = LifespanComponent::new(120);
        host_lifespan.years_lived = 77.0;
        let host = app
            .world_mut()
            .spawn((
                Cultivation::default(),
                Karma::default(),
                PlayerState::default(),
                host_lifespan,
                LifeRecord::new("offline:Host"),
                Lifecycle::default(),
            ))
            .id();
        let mut target_lifespan = LifespanComponent::new(80);
        target_lifespan.years_lived = 18.0;
        let target = app
            .world_mut()
            .spawn((
                Cultivation::default(),
                target_lifespan,
                LifeRecord::new("offline:Target"),
                Lifecycle::default(),
            ))
            .id();
        app.world_mut()
            .resource_mut::<Events<DuoSheRequestEvent>>()
            .send(DuoSheRequestEvent {
                host,
                target_id: "offline:Target".to_string(),
            });

        app.update();

        let host_entity = app.world().entity(host);
        let host_lifespan = host_entity.get::<LifespanComponent>().unwrap();
        assert_eq!(host_lifespan.years_lived, 18.0);
        assert!(host_entity.get::<DuoSheMarked>().is_some());
        let host_cultivation = host_entity.get::<Cultivation>().unwrap();
        assert!((host_cultivation.qi_max - 8.0).abs() < 1e-9);
        let target_entity = app.world().entity(target);
        assert!(target_entity.get::<PossessedVictim>().is_some());
        assert!(target_entity.get::<Despawned>().is_some());
        assert_eq!(
            app.world().resource::<Events<DuoSheEventEmitted>>().len(),
            1
        );
        assert_eq!(
            app.world().resource::<Events<DuoSheWarningEvent>>().len(),
            1
        );
    }
}
