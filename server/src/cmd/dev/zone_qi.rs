use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::parsers::CommandArg;
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{App, Client, EventReader, Position, Query, ResMut, Update};

use crate::qi_physics::constants::QI_ZONE_UNIT_CAPACITY;
use crate::world::dimension::CurrentDimension;
use crate::world::zone::ZoneRegistry;

#[derive(Debug, Clone, PartialEq)]
pub enum ZoneQiCmd {
    Set { name: String, value: f64 },
    /// 回显执行者当前所在 zone 的权威 spirit_qi（只读探针）。
    GetCurrent,
}

impl Command for ZoneQiCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        graph
            .root()
            .literal("zone_qi")
            .literal("set")
            .argument("name")
            .with_parser::<String>()
            .argument("value")
            .with_parser::<f64>()
            .with_executable(|input| ZoneQiCmd::Set {
                name: String::parse_arg(input).unwrap(),
                value: f64::parse_arg(input).unwrap(),
            });
        graph
            .root()
            .literal("zone_qi")
            .literal("get")
            .with_executable(|_| ZoneQiCmd::GetCurrent);
    }
}

pub fn register(app: &mut App) {
    app.add_command::<ZoneQiCmd>()
        .add_systems(Update, handle_zone_qi);
}

pub fn handle_zone_qi(
    mut events: EventReader<CommandResultEvent<ZoneQiCmd>>,
    zones: Option<ResMut<ZoneRegistry>>,
    mut clients: Query<&mut Client>,
    locations: Query<(Option<&Position>, Option<&CurrentDimension>)>,
) {
    let Some(mut zones) = zones else {
        for event in events.read() {
            if let Ok(mut client) = clients.get_mut(event.executor) {
                client.send_chat_message("[dev] zone_qi failed: ZoneRegistry missing");
            }
        }
        return;
    };

    for event in events.read() {
        match &event.result {
            ZoneQiCmd::Set { name, value } => {
                let Ok(mut client) = clients.get_mut(event.executor) else {
                    continue;
                };
                if !value.is_finite() {
                    client.send_chat_message("[dev] zone_qi rejected: value must be finite");
                    continue;
                }
                if let Some(zone) = zones.find_zone_mut(name) {
                    let before = zone.spirit_qi;
                    zone.spirit_qi = *value;
                    tracing::warn!(
                        "[dev-cmd] bypass ledger and zone qi tick: zone `{}` {:.3} -> {:.3}",
                        name,
                        before,
                        value
                    );
                    client.send_chat_message(format!(
                        "[dev] zone_qi `{name}` {:.2} -> {:.2}",
                        before, value
                    ));
                } else {
                    let hints = zones
                        .zones
                        .iter()
                        .take(10)
                        .map(|zone| zone.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    client.send_chat_message(format!("[dev] unknown zone `{name}`; known: {hints}"));
                }
            }
            ZoneQiCmd::GetCurrent => {
                let Ok((position, dimension)) = locations.get(event.executor) else {
                    continue;
                };
                let Some(position) = position else {
                    tracing::warn!(
                        "[bong][cmd] zone_qi get rejected: executor {:?} has no Position",
                        event.executor
                    );
                    continue;
                };
                // 无 CurrentDimension 时退化为 Overworld（spawn/常规世界默认）。
                let dimension = dimension
                    .map(|d| d.0)
                    .unwrap_or(crate::world::dimension::DimensionKind::Overworld);
                let Some(zone) = zones.find_zone(dimension, position.0) else {
                    if let Ok(mut client) = clients.get_mut(event.executor) {
                        client.send_chat_message(
                            "[dev] zone_qi get: no zone at executor position",
                        );
                    }
                    continue;
                };
                let zone_total = zone.spirit_qi * QI_ZONE_UNIT_CAPACITY;
                if let Ok(mut client) = clients.get_mut(event.executor) {
                    client.send_chat_message(format!(
                        "[dev] zone_qi {} spirit_qi={:.6} zone_total={:.6}",
                        zone.name, zone.spirit_qi, zone_total
                    ));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::dev::test_support::{run_update, spawn_test_client};
    use crate::qi_physics::QiTransfer;
    use crate::world::dimension::DimensionKind;
    use valence::prelude::Events;
    use valence::protocol::packets::play::GameMessageS2c;
    use valence::testing::{create_mock_client, MockClientHelper};

    fn collected_chat(app: &mut App, helper: &mut MockClientHelper) -> Vec<String> {
        let world = app.world_mut();
        let mut clients = world.query::<&mut Client>();
        for mut client in clients.iter_mut(world) {
            client
                .flush_packets()
                .expect("mock client packets should flush successfully");
        }
        helper
            .collect_received()
            .0
            .into_iter()
            .filter_map(|frame| {
                frame
                    .decode::<GameMessageS2c>()
                    .ok()
                    .map(|packet| packet.chat.to_legacy_lossy())
            })
            .collect()
    }

    fn setup_app() -> App {
        let mut app = App::new();
        app.insert_resource(ZoneRegistry::fallback());
        app.add_event::<CommandResultEvent<ZoneQiCmd>>();
        app.add_event::<QiTransfer>();
        app.add_systems(Update, handle_zone_qi);
        app
    }

    fn send(app: &mut App, player: valence::prelude::Entity, name: &str, value: f64) {
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<ZoneQiCmd>>>()
            .send(CommandResultEvent {
                result: ZoneQiCmd::Set {
                    name: name.to_string(),
                    value,
                },
                executor: player,
                modifiers: Default::default(),
            });
    }

    #[test]
    fn zone_qi_set_updates_spawn_and_allows_negative_values() {
        let mut app = setup_app();
        let player = spawn_test_client(&mut app, "Alice", [0.0, 0.0, 0.0]);

        send(&mut app, player, "spawn", 0.8);
        run_update(&mut app);
        assert_eq!(
            app.world()
                .resource::<ZoneRegistry>()
                .find_zone_by_name("spawn")
                .unwrap()
                .spirit_qi,
            0.8
        );

        send(&mut app, player, "spawn", -0.3);
        run_update(&mut app);
        assert_eq!(
            app.world()
                .resource::<ZoneRegistry>()
                .find_zone_by_name("spawn")
                .unwrap()
                .spirit_qi,
            -0.3
        );
    }

    #[test]
    fn zone_qi_unknown_zone_does_not_mutate_existing_zones() {
        let mut app = setup_app();
        let player = spawn_test_client(&mut app, "Alice", [0.0, 0.0, 0.0]);

        send(&mut app, player, "missing", 0.5);
        run_update(&mut app);

        assert_eq!(
            app.world()
                .resource::<ZoneRegistry>()
                .find_zone_by_name("spawn")
                .unwrap()
                .spirit_qi,
            0.9
        );
    }

    #[test]
    fn zone_qi_set_does_not_emit_qi_transfer() {
        let mut app = setup_app();
        let player = spawn_test_client(&mut app, "Alice", [0.0, 0.0, 0.0]);

        send(&mut app, player, "spawn", 1.0);
        run_update(&mut app);

        assert_eq!(app.world().resource::<Events<QiTransfer>>().len(), 0);
    }

    #[test]
    fn zone_qi_get_echoes_executor_current_zone_spirit_qi() {
        // central-review 2012 #2 回归：forge 正路径的「扣真元走 zone ledger」契约
        // 需要可观测的 zone 目的地读数。`zone_qi get` 是只读探针——解析执行者
        // 当前所在 zone（Position + CurrentDimension），回显其权威 spirit_qi 与
        // 换算后的 zone_total（spirit_qi × QI_ZONE_UNIT_CAPACITY）。场景用它在
        // forge 前后各读一次、断言增量 == qi_cost / QI_ZONE_UNIT_CAPACITY。
        let mut app = setup_app();
        let (client_bundle, mut helper) = create_mock_client("Alice");
        let player = app.world_mut().spawn(client_bundle).id();
        app.world_mut()
            .entity_mut(player)
            .insert(Position::new([8.0, 64.0, 8.0]));
        app.world_mut()
            .entity_mut(player)
            .insert(CurrentDimension(DimensionKind::Overworld));

        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<ZoneQiCmd>>>()
            .send(CommandResultEvent {
                result: ZoneQiCmd::GetCurrent,
                executor: player,
                modifiers: Default::default(),
            });
        run_update(&mut app);

        let chats = collected_chat(&mut app, &mut helper);
        assert!(
            chats.iter().any(|chat| {
                chat.contains("zone_qi spawn")
                    && chat.contains("spirit_qi=0.900000")
                    && chat.contains("zone_total=45.000000")
            }),
            "zone_qi get 应回显 spawn zone 的权威 spirit_qi（fallback 0.9 → zone_total 45），实际 {chats:?}"
        );
    }

    #[test]
    fn zone_qi_get_resolves_position_inside_spawn_bounds() {
        // fallback spawn zone bounds 是 [-128.., 64..80, -128..]；站在其内时
        // GetCurrent 必须解析到 spawn，而不是「no zone at executor position」。
        let mut app = setup_app();
        let (client_bundle, mut helper) = create_mock_client("Bob");
        let player = app.world_mut().spawn(client_bundle).id();
        app.world_mut()
            .entity_mut(player)
            .insert(Position::new([0.0, 64.0, 0.0]));
        app.world_mut()
            .entity_mut(player)
            .insert(CurrentDimension(DimensionKind::Overworld));

        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<ZoneQiCmd>>>()
            .send(CommandResultEvent {
                result: ZoneQiCmd::GetCurrent,
                executor: player,
                modifiers: Default::default(),
            });
        run_update(&mut app);

        let chats = collected_chat(&mut app, &mut helper);
        assert!(
            chats.iter().any(|chat| chat.contains("zone_qi spawn")),
            "zone_qi get 应解析到 spawn zone，实际 {chats:?}"
        );
    }

    #[test]
    fn zone_qi_get_falls_back_to_overworld_without_current_dimension() {
        // GetCurrent 文档化契约：执行者缺 CurrentDimension 时退化为 Overworld
        // （spawn/常规世界默认，zone_qi.rs:107-109 的 unwrap_or）。旧测试全部显式
        // insert CurrentDimension(Overworld)，从未触达缺失组件状态——错误实现
        // 「无 CurrentDimension 就 skip/拒绝读取」也能全过。必须覆盖：仅 Position
        // 无 CurrentDimension 的执行者仍解析到 spawn zone（fallback Overworld）。
        let mut app = setup_app();
        let (client_bundle, mut helper) = create_mock_client("Bob");
        let player = app.world_mut().spawn(client_bundle).id();
        app.world_mut()
            .entity_mut(player)
            .insert(Position::new([8.0, 64.0, 8.0]));
        // 注意：故意**不** insert CurrentDimension——验证 Overworld 退化路径。

        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<ZoneQiCmd>>>()
            .send(CommandResultEvent {
                result: ZoneQiCmd::GetCurrent,
                executor: player,
                modifiers: Default::default(),
            });
        run_update(&mut app);

        let chats = collected_chat(&mut app, &mut helper);
        assert!(
            chats.iter().any(|chat| {
                chat.contains("zone_qi spawn")
                    && chat.contains("spirit_qi=0.900000")
            }),
            "无 CurrentDimension 的执行者应退化为 Overworld 并解析到 spawn zone（spirit_qi=0.9），实际 {chats:?}"
        );
    }
}
