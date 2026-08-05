use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::parsers::CommandArg;
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{
    App, Client, DVec3, EventReader, IntoSystemConfigs, Position, Query, Res, Update,
};

use crate::world::poi_novice::PoiNoviceRegistry;
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
    InspectNovice,
    Teleport { zone: String, poi: String },
}

impl Command for TppoiCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        graph
            .root()
            .literal("tppoi")
            .literal("novice")
            .with_executable(|_| TppoiCmd::InspectNovice);
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
    app.add_command::<TppoiCmd>().add_systems(
        Update,
        // fix-spec-1901-v2 §4.2 — 直接写 `Position`，纳入统一移动 commit set。
        handle_tppoi.in_set(crate::world::movement_commit::AuthoritativePositionCommitSet),
    );
}

pub fn handle_tppoi(
    mut events: EventReader<CommandResultEvent<TppoiCmd>>,
    zone_registry: Option<Res<ZoneRegistry>>,
    novice_registry: Option<Res<PoiNoviceRegistry>>,
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
        let Ok((mut position, mut client)) = players.get_mut(event.executor) else {
            continue;
        };
        match &event.result {
            TppoiCmd::InspectNovice => {
                for line in novice_registry_chat_lines(novice_registry.as_deref()) {
                    client.send_chat_message(line);
                }
            }
            TppoiCmd::Teleport { zone, poi } => {
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
    }
}

pub fn novice_registry_chat_lines(registry: Option<&PoiNoviceRegistry>) -> Vec<String> {
    let Some(registry) = registry else {
        return vec!["[dev] novice_poi registry missing".to_string()];
    };
    let mut sites = registry.sites().iter().collect::<Vec<_>>();
    sites.sort_by(|left, right| left.id.cmp(&right.id));

    let mut kind_counts = std::collections::BTreeMap::<&str, usize>::new();
    for site in &sites {
        *kind_counts.entry(site.kind.as_str()).or_default() += 1;
    }
    let kinds = if kind_counts.is_empty() {
        "none".to_string()
    } else {
        kind_counts
            .into_iter()
            .map(|(kind, count)| format!("{kind}:{count}"))
            .collect::<Vec<_>>()
            .join(",")
    };
    let mut lines = vec![format!(
        "[dev] novice_poi registry count={} kinds={kinds}",
        sites.len()
    )];
    for site in sites {
        lines.push(format!(
            "[dev] novice_poi {} pos={:.0},{:.0},{:.0} selection={}",
            site.kind.as_str(),
            site.pos_xyz[0],
            site.pos_xyz[1],
            site.pos_xyz[2],
            site.selection_strategy
        ));
    }
    lines
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
    use crate::world::poi_novice::{PoiNoviceKind, PoiNoviceSite};
    use valence::prelude::Events;

    fn setup_app_with_dan_zong() -> App {
        let mut app = App::new();
        app.add_event::<CommandResultEvent<TppoiCmd>>();
        app.add_systems(Update, handle_tppoi);
        app.insert_resource(ZoneRegistry {
            spatial_revision: 0,
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
                qi_equilibrium: 0.0,
                qi_inflow_per_min: 0.0,
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
    fn novice_registry_chat_lines_distinguish_missing_empty_and_loaded() {
        assert_eq!(
            novice_registry_chat_lines(None),
            vec!["[dev] novice_poi registry missing".to_string()]
        );

        let mut registry = PoiNoviceRegistry::default();
        assert_eq!(
            novice_registry_chat_lines(Some(&registry)),
            vec!["[dev] novice_poi registry count=0 kinds=none".to_string()]
        );

        registry.replace_all(vec![PoiNoviceSite {
            id: "spawn:forge_station:test".to_string(),
            kind: PoiNoviceKind::ForgeStation,
            zone: "spawn".to_string(),
            name: "破败炼器台".to_string(),
            pos_xyz: [224.0, 71.0, -240.0],
            selection_strategy: "strict_radius_1500".to_string(),
            qi_affinity: 0.15,
            danger_bias: 0,
            tags: vec!["poi_novice".to_string()],
        }]);
        assert_eq!(
            novice_registry_chat_lines(Some(&registry)),
            vec![
                "[dev] novice_poi registry count=1 kinds=forge_station:1".to_string(),
                "[dev] novice_poi forge_station pos=224,71,-240 selection=strict_radius_1500"
                    .to_string(),
            ]
        );
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
