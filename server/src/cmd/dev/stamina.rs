use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::parsers::CommandArg;
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{App, Client, Entity, EventReader, EventWriter, Query, Update};

use crate::combat::events::{DebugCombatCommand, DebugCombatCommandKind};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StaminaCmd {
    Set { value: f32 },
}

impl Command for StaminaCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        graph
            .root()
            .literal("stamina")
            .literal("set")
            .argument("value")
            .with_parser::<f32>()
            .with_executable(|input| StaminaCmd::Set {
                value: f32::parse_arg(input).unwrap(),
            });
    }
}

pub fn register(app: &mut App) {
    app.add_command::<StaminaCmd>()
        .add_systems(Update, handle_stamina);
}

pub fn queue_stamina_command(target: Entity, value: f32, tx: &mut EventWriter<DebugCombatCommand>) {
    tx.send(DebugCombatCommand {
        target,
        kind: DebugCombatCommandKind::SetStamina(value),
    });
}

pub fn handle_stamina(
    mut events: EventReader<CommandResultEvent<StaminaCmd>>,
    mut debug_combat_tx: EventWriter<DebugCombatCommand>,
    mut clients: Query<&mut Client>,
) {
    for event in events.read() {
        let StaminaCmd::Set { value } = event.result;
        queue_stamina_command(event.executor, value, &mut debug_combat_tx);
        if let Ok(mut client) = clients.get_mut(event.executor) {
            client.send_chat_message(format!("Queued /stamina set {value:.1}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::dev::test_support::{run_update, spawn_test_client};
    use valence::prelude::Events;

    #[test]
    fn stamina_value_parser_accepts_float() {
        assert_eq!(f32::arg_from_str("10.5").unwrap(), 10.5);
    }

    #[test]
    fn stamina_value_parser_rejects_non_number() {
        assert!(f32::arg_from_str("many").is_err());
    }

    #[test]
    fn stamina_command_emits_debug_event() {
        let mut app = App::new();
        app.add_event::<DebugCombatCommand>();
        app.add_event::<CommandResultEvent<StaminaCmd>>();
        app.add_systems(Update, handle_stamina);
        let player = spawn_test_client(&mut app, "Alice", [0.0, 0.0, 0.0]);
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<StaminaCmd>>>()
            .send(CommandResultEvent {
                result: StaminaCmd::Set { value: 10.0 },
                executor: player,
                modifiers: Default::default(),
            });

        run_update(&mut app);

        let events = app.world().resource::<Events<DebugCombatCommand>>();
        let mut reader = events.get_reader();
        let collected = reader.read(events).cloned().collect::<Vec<_>>();
        assert!(matches!(
            collected.as_slice(),
            [DebugCombatCommand {
                kind: DebugCombatCommandKind::SetStamina(n),
                ..
            }] if (*n - 10.0).abs() < f32::EPSILON
        ));
    }

    #[test]
    fn stamina_command_emits_even_without_client_component() {
        let mut app = App::new();
        app.add_event::<DebugCombatCommand>();
        app.add_event::<CommandResultEvent<StaminaCmd>>();
        app.add_systems(Update, handle_stamina);
        let entity = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<StaminaCmd>>>()
            .send(CommandResultEvent {
                result: StaminaCmd::Set { value: 5.0 },
                executor: entity,
                modifiers: Default::default(),
            });

        run_update(&mut app);

        let events = app.world().resource::<Events<DebugCombatCommand>>();
        let mut reader = events.get_reader();
        assert_eq!(reader.read(events).count(), 1);
    }
}
