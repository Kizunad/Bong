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

/// 30+ zone 挤一行在聊天栏不可读（2026-07-06 playtest）——按每行 4 个分批。
/// 首行为 `Zones (N):` 总数头，随后每行至多 4 个名字。
pub fn zone_chat_lines(names: &str) -> Vec<String> {
    let all: Vec<&str> = names.split(", ").filter(|name| !name.is_empty()).collect();
    let mut lines = vec![format!("Zones ({}):", all.len())];
    for chunk in all.chunks(4) {
        lines.push(format!("  {}", chunk.join(", ")));
    }
    lines
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
        for line in zone_chat_lines(&zone_names(zone_registry.as_deref())) {
            client.send_chat_message(line);
        }
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
    fn zone_chat_lines_boundaries_empty_exact_chunk_and_off_by_one() {
        // 空输入：只有总数头（0），不产出空名字行。
        assert_eq!(
            zone_chat_lines(""),
            vec!["Zones (0):".to_string()],
            "空 zone 列表应只有 `Zones (0):` 头行（split 产生的空串必须被过滤）"
        );

        // 恰好 4 个 = 单块：头 + 1 行。
        let four = zone_chat_lines("a, b, c, d");
        assert_eq!(
            four,
            vec!["Zones (4):".to_string(), "  a, b, c, d".to_string()],
            "4 个 zone 应恰好占 1 个分块行（每行 4 个的边界）"
        );

        // 5 个 = off-by-one 跨块：头 + 2 行，第二行只有溢出的 1 个。
        let five = zone_chat_lines("a, b, c, d, e");
        assert_eq!(
            five,
            vec![
                "Zones (5):".to_string(),
                "  a, b, c, d".to_string(),
                "  e".to_string(),
            ],
            "5 个 zone 应跨 2 个分块行（4+1），off-by-one 不得丢名字"
        );

        // 单个：头 + 1 行。
        assert_eq!(
            zone_chat_lines("solo"),
            vec!["Zones (1):".to_string(), "  solo".to_string()]
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
                    qi_equilibrium: 0.0,
                    qi_inflow_per_min: 0.0,
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
                    qi_equilibrium: 0.0,
                    qi_inflow_per_min: 0.0,
                },
            ],
        };

        assert_eq!(zone_names(Some(&registry)), "spawn, north_wastes");
    }
}
