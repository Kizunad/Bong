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
}
