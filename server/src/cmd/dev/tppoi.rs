use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::parsers::CommandArg;
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{App, Client, DVec3, EventReader, Position, Query, Res, Update};

use crate::world::zone::{Zone, ZoneRegistry};

const DAN_ZONG_ZONE: &str = "dan_zong_yi_yuan";
const DAN_ZONG_LAYOUT_POI_Y: f64 = 82.0;
const TP_SAFE_Y_OFFSET: f64 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq)]
struct PoiOffset {
    id: &'static str,
    offset: [f64; 3],
}

const DAN_ZONG_POI_OFFSETS: &[PoiOffset] = &[
    PoiOffset {
        id: "great_hall",
        offset: [0.0, 0.0, 0.0],
    },
    PoiOffset {
        id: "master_sarcophagus",
        offset: [0.0, -4.0, 8.0],
    },
    PoiOffset {
        id: "poison_spring_main",
        offset: [0.0, -1.0, 96.0],
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TppoiCmd {
    Teleport { zone: String, poi: String },
}

impl Command for TppoiCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        graph
            .root()
            .literal("tppoi")
            .argument("zone")
            .with_parser::<String>()
            .argument("poi")
            .with_parser::<String>()
            .with_executable(|input| TppoiCmd::Teleport {
                zone: String::parse_arg(input).unwrap(),
                poi: String::parse_arg(input).unwrap(),
            });
    }
}

pub fn register(app: &mut App) {
    app.add_command::<TppoiCmd>()
        .add_systems(Update, handle_tppoi);
}

pub fn handle_tppoi(
    mut events: EventReader<CommandResultEvent<TppoiCmd>>,
    zone_registry: Option<Res<ZoneRegistry>>,
    mut players: Query<(&mut Position, &mut Client)>,
) {
    let fallback_registry;
    let zones = if let Some(registry) = zone_registry.as_deref() {
        registry
    } else {
        fallback_registry = ZoneRegistry::fallback();
        &fallback_registry
    };

    for event in events.read() {
        let TppoiCmd::Teleport { zone, poi } = &event.result;
        let Ok((mut position, mut client)) = players.get_mut(event.executor) else {
            continue;
        };
        let Some(zone_def) = zones.find_zone_by_name(zone.as_str()) else {
            client.send_chat_message("Unknown zone.");
            continue;
        };
        let Some(target) = poi_position(zone_def, poi.as_str()) else {
            client.send_chat_message("Unknown POI.");
            continue;
        };
        position.set([target.x, target.y, target.z]);
        client.send_chat_message(format!("Teleported to POI `{zone}/{poi}`."));
    }
}

fn poi_position(zone: &Zone, poi: &str) -> Option<DVec3> {
    if zone.name != DAN_ZONG_ZONE {
        return None;
    }
    let offset = DAN_ZONG_POI_OFFSETS
        .iter()
        .find(|offset| offset.id == poi)?;
    let center = zone.center();
    Some(DVec3::new(
        center.x + offset.offset[0],
        DAN_ZONG_LAYOUT_POI_Y + offset.offset[1] + TP_SAFE_Y_OFFSET,
        center.z + offset.offset[2],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::dev::test_support::{run_update, spawn_test_client};
    use crate::world::dimension::DimensionKind;
    use valence::prelude::Events;

    fn setup_app_with_dan_zong() -> App {
        let mut app = App::new();
        app.add_event::<CommandResultEvent<TppoiCmd>>();
        app.add_systems(Update, handle_tppoi);
        app.insert_resource(ZoneRegistry {
            zones: vec![Zone {
                name: DAN_ZONG_ZONE.to_string(),
                dimension: DimensionKind::Overworld,
                bounds: (
                    DVec3::new(-2400.0, -16.0, 3200.0),
                    DVec3::new(-800.0, 240.0, 4800.0),
                ),
                spirit_qi: 0.4,
                danger_level: 4,
                active_events: Vec::new(),
                patrol_anchors: Vec::new(),
                blocked_tiles: Vec::new(),
            }],
        });
        app
    }

    fn send(app: &mut App, executor: valence::prelude::Entity, zone: &str, poi: &str) {
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<TppoiCmd>>>()
            .send(CommandResultEvent {
                result: TppoiCmd::Teleport {
                    zone: zone.to_string(),
                    poi: poi.to_string(),
                },
                executor,
                modifiers: Default::default(),
            });
    }

    #[test]
    fn tppoi_great_hall_uses_dan_zong_layout_anchor() {
        let mut app = setup_app_with_dan_zong();
        let player = spawn_test_client(&mut app, "Alice", [1.0, 2.0, 3.0]);
        send(&mut app, player, DAN_ZONG_ZONE, "great_hall");

        run_update(&mut app);

        let position = app.world().get::<Position>(player).unwrap().get();
        assert_eq!(position.to_array(), [-1600.0, 84.0, 4000.0]);
    }

    #[test]
    fn tppoi_master_sarcophagus_and_poison_spring_use_layout_offsets() {
        let mut app = setup_app_with_dan_zong();
        let player = spawn_test_client(&mut app, "Alice", [1.0, 2.0, 3.0]);
        send(&mut app, player, DAN_ZONG_ZONE, "master_sarcophagus");
        send(&mut app, player, DAN_ZONG_ZONE, "poison_spring_main");

        run_update(&mut app);

        let position = app.world().get::<Position>(player).unwrap().get();
        assert_eq!(position.to_array(), [-1600.0, 83.0, 4096.0]);
    }

    #[test]
    fn tppoi_unknown_zone_does_not_move_player() {
        let mut app = setup_app_with_dan_zong();
        let player = spawn_test_client(&mut app, "Alice", [1.0, 2.0, 3.0]);
        send(&mut app, player, "missing", "great_hall");

        run_update(&mut app);

        let position = app.world().get::<Position>(player).unwrap().get();
        assert_eq!(position.to_array(), [1.0, 2.0, 3.0]);
    }

    #[test]
    fn tppoi_unknown_poi_does_not_move_player() {
        let mut app = setup_app_with_dan_zong();
        let player = spawn_test_client(&mut app, "Alice", [1.0, 2.0, 3.0]);
        send(&mut app, player, DAN_ZONG_ZONE, "missing");

        run_update(&mut app);

        let position = app.world().get::<Position>(player).unwrap().get();
        assert_eq!(position.to_array(), [1.0, 2.0, 3.0]);
    }

    #[test]
    fn tppoi_missing_executor_is_noop() {
        let mut app = setup_app_with_dan_zong();
        send(
            &mut app,
            valence::prelude::Entity::PLACEHOLDER,
            DAN_ZONG_ZONE,
            "great_hall",
        );

        run_update(&mut app);
    }
}
