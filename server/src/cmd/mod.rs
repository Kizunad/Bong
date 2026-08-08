pub mod completions;
pub mod dev;
pub mod gameplay;
pub mod ping;
pub mod registry_pin;

use valence::prelude::{App, ConnectionMode, NetworkSettings, PostStartup};
use valence::EventLoopPreUpdate;

pub fn register(app: &mut App) {
    register_for_dev_mode(app, dev::dev_mode_enabled());
}

fn register_for_dev_mode(app: &mut App, dev_mode_enabled: bool) {
    let _pinned_command_names = registry_pin::COMMAND_NAMES;
    ping::register(app);
    dev::register_for_dev_mode(app, dev_mode_enabled);
    gameplay::register(app);
    // Tab 补全：全部 add_command 完成后（PostStartup）标记 AskServer 节点，
    // 运行期在事件循环里应答客户端补全请求。
    app.add_systems(PostStartup, completions::mark_ask_server_arguments);
    app.add_systems(EventLoopPreUpdate, completions::answer_command_completions);
}

/// 测试用：注册全部命令的最小 App（completions / registry_pin 测试共用）。
pub fn test_command_app() -> App {
    test_command_app_with_connection_mode(ConnectionMode::Offline)
}

/// 测试用：使用生产 command registration，并保留可驱动 packet input 的 event loop。
/// `connection_mode` 必须在 registration 前插入，因为 operator permission 在注册时快照它。
pub fn test_command_app_with_connection_mode(connection_mode: ConnectionMode) -> App {
    test_command_app_for_dev_mode(connection_mode, true)
}

fn test_command_app_for_dev_mode(connection_mode: ConnectionMode, dev_mode_enabled: bool) -> App {
    use crate::combat::events::DebugCombatCommand;
    use crate::cultivation::tribulation::StartDuXuRequest;
    use crate::fauna::rat_phase::RatPhaseChangeEvent;
    use crate::npc::scenario::PendingScenario;
    use crate::npc::war::WarParticipateIntent;
    use crate::player::gameplay::GameplayActionQueue;
    use crate::shader::ShaderStatePayload;
    use crate::world::tsy_dev_command::TsySpawnRequested;

    let mut app = App::new();
    app.insert_resource(NetworkSettings {
        connection_mode,
        ..Default::default()
    });
    app.add_plugins((
        valence::event_loop::EventLoopPlugin,
        valence::command::manager::CommandPlugin,
    ));
    app.add_event::<DebugCombatCommand>();
    app.add_event::<RatPhaseChangeEvent>();
    app.add_event::<TsySpawnRequested>();
    app.add_event::<StartDuXuRequest>();
    // plan-offscreen-war-v1 P6：/faction 命令系统用 EventWriter<WarParticipateIntent>
    app.add_event::<WarParticipateIntent>();
    app.insert_resource(PendingScenario::default());
    app.insert_resource(GameplayActionQueue::default());
    app.insert_resource(ShaderStatePayload::default());
    register_for_dev_mode(&mut app, dev_mode_enabled);
    crate::identity::command::register(&mut app);
    app.finish();
    app.cleanup();
    app.update();
    app
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::command::CommandRegistry;
    use valence::protocol::packets::play::command_tree_s2c::{
        CommandTreeS2c, NodeData, Parser, StringArg,
    };

    fn setup_registry_app() -> App {
        test_command_app()
    }

    #[test]
    fn production_command_registry_does_not_expose_dev_only_spawn_fixtures() {
        let app = test_command_app_for_dev_mode(ConnectionMode::Offline, false);
        let registry = app.world().resource::<CommandRegistry>();
        let roots = registry
            .graph
            .graph
            .neighbors(registry.graph.root)
            .filter_map(|node| match &registry.graph.graph[node].data {
                NodeData::Literal { name } => Some(name.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();

        for dev_only in ["ambient_spawn", "botany_spawn"] {
            assert!(
                !roots.contains(&dev_only),
                "production command tree must not expose /{dev_only} when BONG_DEV_MODE is disabled; roots={roots:?}"
            );
        }
    }

    #[test]
    fn command_registry_contains_pinned_root_literals() {
        let app = setup_registry_app();
        let registry = app.world().resource::<CommandRegistry>();
        let mut roots = registry
            .graph
            .graph
            .neighbors(registry.graph.root)
            .filter_map(|node| match &registry.graph.graph[node].data {
                valence::protocol::packets::play::command_tree_s2c::NodeData::Literal { name } => {
                    Some(name.as_str())
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        roots.sort_unstable();
        roots.dedup();

        assert_eq!(
            roots,
            registry_pin::COMMAND_NAMES,
            "brigadier root literal fixture changed; update registry_pin intentionally"
        );
    }

    #[test]
    fn command_registry_marks_every_pinned_root_reachable() {
        let app = setup_registry_app();
        let registry = app.world().resource::<CommandRegistry>();
        let literals = registry
            .graph
            .graph
            .node_weights()
            .filter_map(|node| match &node.data {
                valence::protocol::packets::play::command_tree_s2c::NodeData::Literal { name } => {
                    Some(name.as_str())
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        for command in registry_pin::COMMAND_NAMES {
            assert!(
                literals.contains(command),
                "expected command tree to contain /{command}, got {literals:?}"
            );
        }
    }

    #[test]
    fn command_registry_matches_frozen_executable_paths() {
        let app = setup_registry_app();
        let registry = app.world().resource::<CommandRegistry>();

        assert_eq!(
            executable_paths(registry),
            registry_pin::COMMAND_TREE_PATHS,
            "brigadier executable command tree changed; update registry_pin intentionally"
        );
    }

    #[test]
    fn command_tree_packet_contains_pinned_root_literals() {
        let app = setup_registry_app();
        let registry = app.world().resource::<CommandRegistry>();
        let packet = CommandTreeS2c::from(registry.graph.clone());
        let root = &packet.commands[packet.root_index.0 as usize];
        let mut roots = root
            .children
            .iter()
            .filter_map(|child| match &packet.commands[child.0 as usize].data {
                NodeData::Literal { name } => Some(name.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        roots.sort_unstable();
        roots.dedup();

        assert_eq!(
            roots,
            registry_pin::COMMAND_NAMES,
            "wire CommandTreeS2c root literals changed; update registry_pin intentionally"
        );
    }

    fn executable_paths(registry: &CommandRegistry) -> Vec<String> {
        let mut paths = Vec::new();
        let mut stack = vec![(registry.graph.root, Vec::<String>::new())];

        while let Some((node, path)) = stack.pop() {
            let node_data = &registry.graph.graph[node];
            if node_data.executable && !path.is_empty() {
                paths.push(path.join(" "));
            }

            let mut children = registry.graph.graph.neighbors(node).collect::<Vec<_>>();
            children.sort_by_key(|child| child.index());
            for child in children.into_iter().rev() {
                let mut child_path = path.clone();
                match &registry.graph.graph[child].data {
                    NodeData::Root => {}
                    NodeData::Literal { name } => child_path.push(name.clone()),
                    NodeData::Argument { name, parser, .. } => {
                        child_path.push(format!("<{}:{}>", name, parser_label(parser)));
                    }
                }
                stack.push((child, child_path));
            }
        }

        paths.sort_unstable();
        paths
    }

    fn parser_label(parser: &Parser) -> String {
        match parser {
            Parser::Bool => "bool".to_string(),
            Parser::Float { .. } => "float".to_string(),
            Parser::Double { .. } => "double".to_string(),
            Parser::Integer { .. } => "integer".to_string(),
            Parser::String(StringArg::SingleWord) => "string".to_string(),
            Parser::String(StringArg::QuotablePhrase) => "phrase".to_string(),
            Parser::String(StringArg::GreedyPhrase) => "greedy".to_string(),
            other => format!("{other:?}"),
        }
    }
}
