use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{App, Client, EventReader, Position, Query, Res, Update};

use crate::world::terrain::{TerrainProvider, TerrainProviders};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopCmd {
    Top,
}

impl Command for TopCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        graph.root().literal("top").with_executable(|_| TopCmd::Top);
    }
}

pub fn register(app: &mut App) {
    app.add_command::<TopCmd>().add_systems(Update, handle_top);
}

pub fn target_top_y(current: valence::prelude::DVec3, terrain: Option<&TerrainProvider>) -> f64 {
    if let Some(terrain) = terrain {
        let sample = terrain.sample(current.x.floor() as i32, current.z.floor() as i32);
        let surface_y = sample.height.round() as f64;
        let water_y = if sample.water_level >= 0.0 {
            sample.water_level.round() as f64
        } else {
            surface_y
        };
        surface_y.max(water_y) + 3.0
    } else {
        current.y + 24.0
    }
}

pub fn handle_top(
    mut events: EventReader<CommandResultEvent<TopCmd>>,
    providers: Option<Res<TerrainProviders>>,
    mut players: Query<(&mut Position, &mut Client)>,
) {
    for event in events.read() {
        let Ok((mut position, mut client)) = players.get_mut(event.executor) else {
            continue;
        };
        let current = position.get();
        let target_y = target_top_y(current, providers.as_deref().map(|p| &p.overworld));
        position.set([current.x, target_y, current.z]);
        client.send_chat_message(format!("Teleported to top at Y={target_y:.0}."));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::dev::test_support::{run_update, spawn_test_client};
    use valence::prelude::{DVec3, Events, Position};

    fn setup_app() -> App {
        let mut app = App::new();
        app.add_event::<CommandResultEvent<TopCmd>>();
        app.add_systems(Update, handle_top);
        app
    }

    #[test]
    fn top_without_terrain_moves_up_twenty_four_blocks() {
        let mut app = setup_app();
        let player = spawn_test_client(&mut app, "Alice", [8.0, 66.0, 8.0]);
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<TopCmd>>>()
            .send(CommandResultEvent {
                result: TopCmd::Top,
                executor: player,
                modifiers: Default::default(),
            });

        run_update(&mut app);

        let position = app.world().get::<Position>(player).unwrap();
        assert_eq!(position.get().to_array(), [8.0, 90.0, 8.0]);
    }

    #[test]
    fn target_top_y_fallback_handles_negative_y() {
        let y = target_top_y(DVec3::new(1.0, -12.0, 1.0), None);

        assert_eq!(y, 12.0);
    }
}
