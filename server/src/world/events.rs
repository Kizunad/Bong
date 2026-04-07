use serde_json::Value;
use valence::prelude::{App, ResMut, Resource, Update};

use super::zone::ZoneRegistry;
use crate::schema::agent_command::Command;
use crate::schema::world_state::GameEvent;

pub const EVENT_THUNDER_TRIBULATION: &str = "thunder_tribulation";
pub const EVENT_BEAST_TIDE: &str = "beast_tide";

const DEFAULT_EVENT_DURATION_TICKS: u64 = 200;
const MIN_EVENT_DURATION_TICKS: u64 = 1;
const RECENT_GAME_EVENTS_LIMIT: usize = 16;

#[derive(Debug, Clone, PartialEq)]
pub struct ActiveEvent {
    pub event_name: String,
    pub zone_name: String,
    pub elapsed_ticks: u64,
    pub duration_ticks: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MajorEventAlert {
    pub event_name: String,
    pub zone_name: String,
    pub duration_ticks: u64,
}

impl ActiveEvent {
    fn from_spawn_command(command: &Command) -> Option<Self> {
        let event_name = command.params.get("event")?.as_str()?;
        if !matches!(event_name, EVENT_THUNDER_TRIBULATION | EVENT_BEAST_TIDE) {
            return None;
        }

        let duration_ticks = value_to_u64(command.params.get("duration_ticks"))
            .unwrap_or(DEFAULT_EVENT_DURATION_TICKS)
            .max(MIN_EVENT_DURATION_TICKS);

        Some(Self {
            event_name: event_name.to_string(),
            zone_name: command.target.clone(),
            elapsed_ticks: 0,
            duration_ticks,
        })
    }

    fn is_expired(&self) -> bool {
        self.elapsed_ticks >= self.duration_ticks
    }
}

#[derive(Default)]
pub struct ActiveEventsResource {
    active_events: Vec<ActiveEvent>,
    pending_major_alerts: Vec<MajorEventAlert>,
    recent_game_events: Vec<GameEvent>,
}

impl Resource for ActiveEventsResource {}

impl ActiveEventsResource {
    pub fn enqueue_from_spawn_command(
        &mut self,
        command: &Command,
        zone_registry: Option<&mut ZoneRegistry>,
    ) {
        let Some(event) = ActiveEvent::from_spawn_command(command) else {
            let event_name = command
                .params
                .get("event")
                .and_then(Value::as_str)
                .unwrap_or("<missing>");

            tracing::warn!(
                "[bong][world] spawn_event `{event_name}` is not implemented in M1 scheduler"
            );
            return;
        };

        let Some(zone_registry) = zone_registry else {
            tracing::warn!(
                "[bong][world] cannot enqueue {} for `{}` because ZoneRegistry resource is missing",
                event.event_name,
                event.zone_name
            );
            return;
        };

        let Some(zone) = zone_registry.find_zone_mut(event.zone_name.as_str()) else {
            tracing::warn!(
                "[bong][world] {} target zone `{}` was not found",
                event.event_name,
                event.zone_name
            );
            return;
        };

        if !zone
            .active_events
            .iter()
            .any(|name| name == &event.event_name)
        {
            zone.active_events.push(event.event_name.clone());
        }

        tracing::info!(
            "[bong][world] scheduled {} for zone `{}` (duration_ticks={})",
            event.event_name,
            event.zone_name,
            event.duration_ticks
        );

        self.pending_major_alerts.push(MajorEventAlert {
            event_name: event.event_name.clone(),
            zone_name: event.zone_name.clone(),
            duration_ticks: event.duration_ticks,
        });

        self.active_events.push(event);
    }

    pub fn drain_major_event_alerts(&mut self) -> Vec<MajorEventAlert> {
        std::mem::take(&mut self.pending_major_alerts)
    }

    pub fn record_recent_event(&mut self, event: GameEvent) {
        self.recent_game_events.push(event);

        if self.recent_game_events.len() > RECENT_GAME_EVENTS_LIMIT {
            let overflow = self.recent_game_events.len() - RECENT_GAME_EVENTS_LIMIT;
            self.recent_game_events.drain(0..overflow);
        }
    }

    pub fn recent_events_snapshot(&self) -> Vec<GameEvent> {
        self.recent_game_events.clone()
    }

    pub fn tick(&mut self, zone_registry: Option<&mut ZoneRegistry>) {
        for event in &mut self.active_events {
            event.elapsed_ticks = event.elapsed_ticks.saturating_add(1);
        }

        let Some(zone_registry) = zone_registry else {
            return;
        };

        self.active_events.retain(|event| {
            if !event.is_expired() {
                return true;
            }

            if let Some(zone) = zone_registry.find_zone_mut(event.zone_name.as_str()) {
                zone.active_events.retain(|name| name != &event.event_name);
            }

            tracing::info!(
                "[bong][world] expired {} for zone `{}` after {} ticks",
                event.event_name,
                event.zone_name,
                event.elapsed_ticks
            );

            false
        });
    }

    #[cfg(test)]
    pub fn contains(&self, zone_name: &str, event_name: &str) -> bool {
        self.active_events
            .iter()
            .any(|event| event.zone_name == zone_name && event.event_name == event_name)
    }

    #[cfg(test)]
    pub fn count_by_zone_and_event(&self, zone_name: &str, event_name: &str) -> usize {
        self.active_events
            .iter()
            .filter(|event| event.zone_name == zone_name && event.event_name == event_name)
            .count()
    }

    #[cfg(test)]
    pub fn elapsed_for_first(&self, zone_name: &str, event_name: &str) -> Option<u64> {
        self.active_events
            .iter()
            .find(|event| event.zone_name == zone_name && event.event_name == event_name)
            .map(|event| event.elapsed_ticks)
    }
}

pub fn register(app: &mut App) {
    tracing::info!("[bong][world] registering active events scheduler");
    app.insert_resource(ActiveEventsResource::default());
    app.add_systems(Update, tick_active_events);
}

fn tick_active_events(
    mut active_events: ResMut<ActiveEventsResource>,
    mut zone_registry: Option<ResMut<ZoneRegistry>>,
) {
    active_events.tick(zone_registry.as_deref_mut());
}

fn value_to_u64(value: Option<&Value>) -> Option<u64> {
    let value = value?;

    if let Some(v) = value.as_u64() {
        return Some(v);
    }

    let v = value.as_i64()?;
    if v < 0 {
        return None;
    }

    Some(v as u64)
}

#[cfg(test)]
mod events_tests {
    use std::collections::HashMap;

    use serde_json::json;
    use valence::prelude::{App, DVec3, Update};

    use super::{
        tick_active_events, ActiveEventsResource, EVENT_BEAST_TIDE, EVENT_THUNDER_TRIBULATION,
    };
    use crate::network::command_executor::{execute_agent_commands, CommandExecutorResource};
    use crate::schema::agent_command::AgentCommandV1;
    use crate::schema::agent_command::Command;
    use crate::schema::common::CommandType;
    use crate::world::zone::ZoneRegistry;

    fn spawn_event_command(target: &str, event: &str, duration_ticks: u64) -> Command {
        let mut params = HashMap::new();
        params.insert("event".to_string(), json!(event));
        params.insert("duration_ticks".to_string(), json!(duration_ticks));

        Command {
            command_type: CommandType::SpawnEvent,
            target: target.to_string(),
            params,
        }
    }

    fn setup_events_app() -> App {
        let mut app = App::new();
        app.insert_resource(ZoneRegistry::fallback());
        app.insert_resource(ActiveEventsResource::default());
        app.add_systems(Update, tick_active_events);
        app
    }

    fn batch(id: &str, commands: Vec<Command>) -> AgentCommandV1 {
        AgentCommandV1 {
            v: 1,
            id: id.to_string(),
            source: Some("calamity".to_string()),
            commands,
        }
    }

    #[test]
    fn thunder_event_ticks_until_expiry() {
        let mut app = setup_events_app();

        {
            let world = app.world_mut();
            let command = spawn_event_command("spawn", EVENT_THUNDER_TRIBULATION, 2);
            world.resource_scope(|world, mut zones: valence::prelude::Mut<ZoneRegistry>| {
                let mut events = world.resource_mut::<ActiveEventsResource>();
                events.enqueue_from_spawn_command(&command, Some(&mut zones));
            });
        }

        {
            let world = app.world();
            let zone = world
                .resource::<ZoneRegistry>()
                .find_zone(DVec3::new(8.0, 66.0, 8.0))
                .expect("spawn zone should exist");
            assert!(zone
                .active_events
                .iter()
                .any(|event| event == EVENT_THUNDER_TRIBULATION));
            assert!(world
                .resource::<ActiveEventsResource>()
                .contains("spawn", EVENT_THUNDER_TRIBULATION));
        }

        app.update();
        {
            let events = app.world().resource::<ActiveEventsResource>();
            assert_eq!(
                events.elapsed_for_first("spawn", EVENT_THUNDER_TRIBULATION),
                Some(1)
            );
        }

        app.update();
        {
            let world = app.world();
            let zone = world
                .resource::<ZoneRegistry>()
                .find_zone(DVec3::new(8.0, 66.0, 8.0))
                .expect("spawn zone should exist");
            assert!(
                !zone
                    .active_events
                    .iter()
                    .any(|event| event == EVENT_THUNDER_TRIBULATION),
                "thunder event should be removed from zone after expiry"
            );
            assert!(
                !world
                    .resource::<ActiveEventsResource>()
                    .contains("spawn", EVENT_THUNDER_TRIBULATION),
                "thunder event should be removed from scheduler after expiry"
            );
        }
    }

    #[test]
    fn beast_tide_event_spawns_and_cleans_up() {
        let mut app = setup_events_app();

        {
            let world = app.world_mut();
            let command = spawn_event_command("spawn", EVENT_BEAST_TIDE, 1);
            world.resource_scope(|world, mut zones: valence::prelude::Mut<ZoneRegistry>| {
                let mut events = world.resource_mut::<ActiveEventsResource>();
                events.enqueue_from_spawn_command(&command, Some(&mut zones));
            });
        }

        {
            let world = app.world();
            let zone = world
                .resource::<ZoneRegistry>()
                .find_zone(DVec3::new(8.0, 66.0, 8.0))
                .expect("spawn zone should exist");
            assert!(zone
                .active_events
                .iter()
                .any(|event| event == EVENT_BEAST_TIDE));
            assert!(world
                .resource::<ActiveEventsResource>()
                .contains("spawn", EVENT_BEAST_TIDE));
        }

        app.update();
        {
            let world = app.world();
            let zone = world
                .resource::<ZoneRegistry>()
                .find_zone(DVec3::new(8.0, 66.0, 8.0))
                .expect("spawn zone should exist");
            assert!(
                !zone
                    .active_events
                    .iter()
                    .any(|event| event == EVENT_BEAST_TIDE),
                "beast_tide should be removed from zone when duration elapses"
            );
            assert!(
                !world
                    .resource::<ActiveEventsResource>()
                    .contains("spawn", EVENT_BEAST_TIDE),
                "beast_tide should be removed from scheduler when duration elapses"
            );
        }
    }

    #[test]
    fn spawn_event_only_enters_scheduler_once() {
        let mut app = App::new();
        app.insert_resource(CommandExecutorResource::default());
        app.insert_resource(ZoneRegistry::fallback());
        app.insert_resource(ActiveEventsResource::default());
        app.add_systems(Update, execute_agent_commands);

        let command = spawn_event_command("spawn", EVENT_THUNDER_TRIBULATION, 3);
        {
            let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
            executor.enqueue_batch(batch("cmd_event_once", vec![command]));
        }

        app.update();

        let world = app.world();
        let events = world.resource::<ActiveEventsResource>();
        let zone = world
            .resource::<ZoneRegistry>()
            .find_zone(DVec3::new(8.0, 66.0, 8.0))
            .expect("spawn zone should exist");

        assert_eq!(
            events.count_by_zone_and_event("spawn", EVENT_THUNDER_TRIBULATION),
            1,
            "spawn_event should only register one scheduled thunder event"
        );
        assert_eq!(
            zone.active_events
                .iter()
                .filter(|name| name.as_str() == EVENT_THUNDER_TRIBULATION)
                .count(),
            1,
            "spawn zone should expose thunder exactly once in stable active_events"
        );
    }
}
