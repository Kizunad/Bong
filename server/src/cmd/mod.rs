pub mod dev;
pub mod gameplay;
pub mod ping;
pub mod registry_pin;

use valence::prelude::App;

pub fn register(app: &mut App) {
    let _pinned_command_names = registry_pin::COMMAND_NAMES;
    ping::register(app);
    dev::register(app);
    gameplay::register(app);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::events::DebugCombatCommand;
    use crate::npc::scenario::PendingScenario;
    use crate::player::gameplay::GameplayActionQueue;
    use crate::world::tsy_dev_command::TsySpawnRequested;
    use valence::command::CommandRegistry;
    use valence::prelude::App;
    use valence::protocol::packets::play::command_tree_s2c::{
        CommandTreeS2c, NodeData, Parser, StringArg,
    };

    fn setup_registry_app() -> App {
        let mut app = App::new();
        app.add_plugins(valence::command::manager::CommandPlugin);
        app.add_event::<DebugCombatCommand>();
        app.add_event::<TsySpawnRequested>();
        app.insert_resource(PendingScenario::default());
        app.insert_resource(GameplayActionQueue::default());
        register(&mut app);
        app.finish();
        app.cleanup();
        app.update();
        app
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
            Parser::Float { .. } => "float".to_string(),
            Parser::Double { .. } => "double".to_string(),
            Parser::String(StringArg::SingleWord) => "string".to_string(),
            Parser::String(StringArg::QuotablePhrase) => "phrase".to_string(),
            Parser::String(StringArg::GreedyPhrase) => "greedy".to_string(),
            other => format!("{other:?}"),
        }
    }
}
