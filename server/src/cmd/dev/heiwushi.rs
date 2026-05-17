use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{
    App, Client, Commands, DVec3, Entity, EntityLayerId, EventReader, Position, Query, Res, Update,
    With,
};

use crate::fauna::visual::HEIWUSHI_ENTITY_KIND;
use crate::npc::spawn::spawn_zombie_npc_at;
use crate::world::dimension::{DimensionKind, OverworldLayer};
use crate::world::zone::{ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeiwushiCmd {
    Summon,
}

impl Command for HeiwushiCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        graph
            .root()
            .literal("summon")
            .literal("heiwushi")
            .with_executable(|_| Self::Summon);
    }
}

pub fn register(app: &mut App) {
    app.add_command::<HeiwushiCmd>()
        .add_systems(Update, handle_summon_heiwushi);
}

fn handle_summon_heiwushi(
    mut commands: Commands,
    mut events: EventReader<CommandResultEvent<HeiwushiCmd>>,
    mut players: Query<(&Position, Option<&EntityLayerId>, &mut Client)>,
    layers: Query<Entity, With<OverworldLayer>>,
    zones: Option<Res<ZoneRegistry>>,
) {
    for event in events.read() {
        let Ok((position, player_layer, mut client)) = players.get_mut(event.executor) else {
            continue;
        };

        let Some(layer) = player_layer.map(|l| l.0).or_else(|| layers.iter().next()) else {
            client.send_chat_message("/summon heiwushi failed: no active layer.");
            continue;
        };

        let pos = position.get();
        let spawn_pos = DVec3::new(pos.x + 3.0, pos.y, pos.z + 3.0);

        let zone_name = zones
            .as_ref()
            .and_then(|z| z.find_zone(DimensionKind::Overworld, spawn_pos))
            .map(|z| z.name.clone())
            .unwrap_or_else(|| DEFAULT_SPAWN_ZONE_NAME.to_string());

        let entity = spawn_zombie_npc_at(
            &mut commands,
            layer,
            &zone_name,
            spawn_pos,
            spawn_pos,
        );

        // Override entity kind to Heiwushi model
        commands.entity(entity).insert(HEIWUSHI_ENTITY_KIND);

        client.send_chat_message(format!(
            "§6[黑武士] §f已召唤 ({:.0}, {:.0}, {:.0})",
            spawn_pos.x, spawn_pos.y, spawn_pos.z
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::dev::test_support::{run_update, spawn_test_client};
    use crate::npc::spawn::NpcMarker;
    use valence::prelude::{EntityKind, Events};

    #[test]
    fn summon_heiwushi_spawns_entity_with_correct_kind() {
        let mut app = App::new();
        app.add_event::<CommandResultEvent<HeiwushiCmd>>();
        app.add_systems(Update, handle_summon_heiwushi);
        let layer = app.world_mut().spawn(OverworldLayer).id();
        let player = spawn_test_client(&mut app, "Alice", [100.0, 70.0, 100.0]);
        app.world_mut()
            .entity_mut(player)
            .insert(EntityLayerId(layer));

        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<HeiwushiCmd>>>()
            .send(CommandResultEvent {
                result: HeiwushiCmd::Summon,
                executor: player,
                modifiers: Default::default(),
            });

        run_update(&mut app);

        let mut query = app
            .world_mut()
            .query_filtered::<&EntityKind, With<NpcMarker>>();
        let spawned: Vec<_> = query.iter(app.world()).collect();
        assert_eq!(spawned.len(), 1, "Should spawn exactly one heiwushi NPC");
        assert_eq!(*spawned[0], HEIWUSHI_ENTITY_KIND);
    }

    #[test]
    fn summon_heiwushi_without_executor_does_not_panic() {
        let mut app = App::new();
        app.add_event::<CommandResultEvent<HeiwushiCmd>>();
        app.add_systems(Update, handle_summon_heiwushi);

        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<HeiwushiCmd>>>()
            .send(CommandResultEvent {
                result: HeiwushiCmd::Summon,
                executor: Entity::PLACEHOLDER,
                modifiers: Default::default(),
            });

        run_update(&mut app);

        let mut query = app
            .world_mut()
            .query_filtered::<&EntityKind, With<NpcMarker>>();
        assert_eq!(query.iter(app.world()).count(), 0);
    }
}
