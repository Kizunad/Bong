use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{App, Client, EventReader, Query, Res, Update};

use crate::world::zone::ZoneRegistry;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZonesCmd {
    Zones,
}

impl Command for ZonesCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        graph
            .root()
            .literal("zones")
            .with_executable(|_| ZonesCmd::Zones);
    }
}

pub fn register(app: &mut App) {
    app.add_command::<ZonesCmd>()
        .add_systems(Update, handle_zones);
}

pub fn zone_names(registry: Option<&ZoneRegistry>) -> String {
    registry
        .cloned()
        .unwrap_or_else(ZoneRegistry::fallback)
        .zones
        .iter()
        .map(|zone| zone.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn handle_zones(
    mut events: EventReader<CommandResultEvent<ZonesCmd>>,
    zone_registry: Option<Res<ZoneRegistry>>,
    mut clients: Query<&mut Client>,
) {
    for event in events.read() {
        let Ok(mut client) = clients.get_mut(event.executor) else {
            continue;
        };
        client.send_chat_message(format!("Zones: {}", zone_names(zone_registry.as_deref())));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::dev::test_support::{run_update, spawn_test_client};
    use valence::prelude::Events;

    #[test]
    fn zone_names_uses_fallback_when_registry_missing() {
        assert_eq!(
            zone_names(None),
            crate::world::zone::DEFAULT_SPAWN_ZONE_NAME
        );
    }

    #[test]
    fn zones_command_handles_missing_executor() {
        let mut app = App::new();
        app.add_event::<CommandResultEvent<ZonesCmd>>();
        app.add_systems(Update, handle_zones);
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<ZonesCmd>>>()
            .send(CommandResultEvent {
                result: ZonesCmd::Zones,
                executor: valence::prelude::Entity::PLACEHOLDER,
                modifiers: Default::default(),
            });

        run_update(&mut app);
    }

    #[test]
    fn zones_command_runs_for_client_with_fallback_registry() {
        let mut app = App::new();
        app.add_event::<CommandResultEvent<ZonesCmd>>();
        app.add_systems(Update, handle_zones);
        let player = spawn_test_client(&mut app, "Alice", [0.0, 0.0, 0.0]);
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<ZonesCmd>>>()
            .send(CommandResultEvent {
                result: ZonesCmd::Zones,
                executor: player,
                modifiers: Default::default(),
            });

        run_update(&mut app);
    }

    #[test]
    fn zone_names_lists_registry_entries_in_order() {
        let registry = ZoneRegistry {
            zones: vec![
                crate::world::zone::Zone {
                    name: "spawn".to_string(),
                    dimension: crate::world::dimension::DimensionKind::Overworld,
                    bounds: (
                        valence::prelude::DVec3::new(0.0, 0.0, 0.0),
                        valence::prelude::DVec3::new(1.0, 1.0, 1.0),
                    ),
                    spirit_qi: 0.9,
                    danger_level: 0,
                    active_events: Vec::new(),
                    patrol_anchors: Vec::new(),
                    blocked_tiles: Vec::new(),
                },
                crate::world::zone::Zone {
                    name: "north_wastes".to_string(),
                    dimension: crate::world::dimension::DimensionKind::Overworld,
                    bounds: (
                        valence::prelude::DVec3::new(2.0, 0.0, 0.0),
                        valence::prelude::DVec3::new(3.0, 1.0, 1.0),
                    ),
                    spirit_qi: -0.4,
                    danger_level: 2,
                    active_events: Vec::new(),
                    patrol_anchors: Vec::new(),
                    blocked_tiles: Vec::new(),
                },
            ],
        };

        assert_eq!(zone_names(Some(&registry)), "spawn, north_wastes");
    }
}
