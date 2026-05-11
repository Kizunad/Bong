use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::parsers::CommandArg;
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{App, Client, EventReader, Query, Update};

use crate::cultivation::known_techniques::{technique_definition, KnownTechnique, KnownTechniques};

#[derive(Debug, Clone, PartialEq)]
pub enum TechniqueCmd {
    List,
    Add { id: String },
    Remove { id: String },
    Proficiency { id: String, value: f64 },
    Active { id: String, value: bool },
    ResetAll,
}

impl Command for TechniqueCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        let technique = graph.root().literal("technique").id();

        graph
            .at(technique)
            .literal("list")
            .with_executable(|_| TechniqueCmd::List);

        graph
            .at(technique)
            .literal("add")
            .argument("id")
            .with_parser::<String>()
            .with_executable(|input| TechniqueCmd::Add {
                id: String::parse_arg(input).unwrap(),
            });

        graph
            .at(technique)
            .literal("remove")
            .argument("id")
            .with_parser::<String>()
            .with_executable(|input| TechniqueCmd::Remove {
                id: String::parse_arg(input).unwrap(),
            });

        graph
            .at(technique)
            .literal("proficiency")
            .argument("id")
            .with_parser::<String>()
            .argument("value")
            .with_parser::<f64>()
            .with_executable(|input| TechniqueCmd::Proficiency {
                id: String::parse_arg(input).unwrap(),
                value: f64::parse_arg(input).unwrap(),
            });

        graph
            .at(technique)
            .literal("active")
            .argument("id")
            .with_parser::<String>()
            .argument("value")
            .with_parser::<bool>()
            .with_executable(|input| TechniqueCmd::Active {
                id: String::parse_arg(input).unwrap(),
                value: bool::parse_arg(input).unwrap(),
            });

        graph
            .at(technique)
            .literal("reset_all")
            .with_executable(|_| TechniqueCmd::ResetAll);
    }
}

pub fn register(app: &mut App) {
    app.add_command::<TechniqueCmd>()
        .add_systems(Update, handle_technique);
}

pub fn handle_technique(
    mut events: EventReader<CommandResultEvent<TechniqueCmd>>,
    mut players: Query<(&mut KnownTechniques, &mut Client)>,
) {
    for event in events.read() {
        let Ok((mut techniques, mut client)) = players.get_mut(event.executor) else {
            continue;
        };

        match &event.result {
            TechniqueCmd::List => {
                let body = techniques
                    .entries
                    .iter()
                    .map(|entry| {
                        format!(
                            "{} p={:.2} active={}",
                            entry.id, entry.proficiency, entry.active
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                client.send_chat_message(format!("[dev] techniques: {body}"));
            }
            TechniqueCmd::Add { id } => {
                if technique_definition(id).is_none() {
                    client
                        .send_chat_message(format!("[dev] technique add rejected: unknown `{id}`"));
                    continue;
                }
                if techniques.entries.iter().any(|entry| entry.id == *id) {
                    client.send_chat_message(format!("[dev] technique `{id}` already known"));
                    continue;
                }
                techniques.entries.push(KnownTechnique {
                    id: id.clone(),
                    proficiency: 0.5,
                    active: true,
                });
                client.send_chat_message(format!("[dev] technique `{id}` added"));
            }
            TechniqueCmd::Remove { id } => {
                let before = techniques.entries.len();
                techniques.entries.retain(|entry| entry.id != *id);
                client.send_chat_message(format!(
                    "[dev] technique `{id}` removed={}",
                    techniques.entries.len() != before
                ));
            }
            TechniqueCmd::Proficiency { id, value } => {
                if technique_definition(id).is_none() {
                    client.send_chat_message(format!(
                        "[dev] technique proficiency rejected: unknown `{id}`"
                    ));
                    continue;
                }
                let Some(entry) = techniques.entries.iter_mut().find(|entry| entry.id == *id)
                else {
                    client.send_chat_message(format!("[dev] technique `{id}` missing"));
                    continue;
                };
                if !value.is_finite() {
                    client.send_chat_message(format!(
                        "[dev] technique proficiency rejected: value must be finite for `{id}`"
                    ));
                    continue;
                }
                entry.proficiency = value.clamp(0.0, 1.0) as f32;
                client.send_chat_message(format!(
                    "[dev] technique `{id}` proficiency={:.2}",
                    entry.proficiency
                ));
            }
            TechniqueCmd::Active { id, value } => {
                if technique_definition(id).is_none() {
                    client.send_chat_message(format!(
                        "[dev] technique active rejected: unknown `{id}`"
                    ));
                    continue;
                }
                let Some(entry) = techniques.entries.iter_mut().find(|entry| entry.id == *id)
                else {
                    client.send_chat_message(format!("[dev] technique `{id}` missing"));
                    continue;
                };
                entry.active = *value;
                client.send_chat_message(format!("[dev] technique `{id}` active={value}"));
            }
            TechniqueCmd::ResetAll => {
                *techniques = KnownTechniques::default();
                client.send_chat_message(format!(
                    "[dev] technique reset_all; entries={}",
                    techniques.entries.len()
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::dev::test_support::{run_update, spawn_test_client};
    use valence::prelude::Events;

    const BENG_QUAN: &str = "burst_meridian.beng_quan";
    const NEEDLE: &str = "dugu.shoot_needle";
    const ECHO: &str = "anqi.echo_fractal";

    fn setup_app() -> App {
        let mut app = App::new();
        app.add_event::<CommandResultEvent<TechniqueCmd>>();
        app.add_systems(Update, handle_technique);
        app
    }

    fn spawn_known(app: &mut App, techniques: KnownTechniques) -> valence::prelude::Entity {
        let player = spawn_test_client(app, "Alice", [0.0, 0.0, 0.0]);
        app.world_mut().entity_mut(player).insert(techniques);
        player
    }

    fn default_technique_count() -> usize {
        KnownTechniques::default().entries.len()
    }

    fn send(app: &mut App, player: valence::prelude::Entity, result: TechniqueCmd) {
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<TechniqueCmd>>>()
            .send(CommandResultEvent {
                result,
                executor: player,
                modifiers: Default::default(),
            });
    }

    #[test]
    fn technique_list_keeps_default_entries() {
        let mut app = setup_app();
        let player = spawn_known(&mut app, KnownTechniques::default());

        send(&mut app, player, TechniqueCmd::List);
        run_update(&mut app);

        assert_eq!(
            app.world()
                .get::<KnownTechniques>(player)
                .unwrap()
                .entries
                .len(),
            default_technique_count()
        );
    }

    #[test]
    fn technique_add_is_idempotent_and_rejects_unknown_ids() {
        let mut app = setup_app();
        let player = spawn_known(&mut app, KnownTechniques::default());

        send(
            &mut app,
            player,
            TechniqueCmd::Add {
                id: BENG_QUAN.to_string(),
            },
        );
        send(
            &mut app,
            player,
            TechniqueCmd::Add {
                id: "foo.bar".to_string(),
            },
        );
        run_update(&mut app);

        let techniques = app.world().get::<KnownTechniques>(player).unwrap();
        assert_eq!(techniques.entries.len(), default_technique_count());
        assert!(techniques.entries.iter().all(|entry| entry.id != "foo.bar"));
    }

    #[test]
    fn technique_remove_allows_unknown_but_removes_existing() {
        let mut app = setup_app();
        let player = spawn_known(&mut app, KnownTechniques::default());

        send(
            &mut app,
            player,
            TechniqueCmd::Remove {
                id: NEEDLE.to_string(),
            },
        );
        send(
            &mut app,
            player,
            TechniqueCmd::Remove {
                id: "legacy.unknown".to_string(),
            },
        );
        run_update(&mut app);

        let techniques = app.world().get::<KnownTechniques>(player).unwrap();
        assert_eq!(techniques.entries.len(), default_technique_count() - 1);
        assert!(techniques.entries.iter().all(|entry| entry.id != NEEDLE));
    }

    #[test]
    fn technique_proficiency_clamps_and_active_flag_mutates() {
        let mut app = setup_app();
        let player = spawn_known(&mut app, KnownTechniques::default());

        send(
            &mut app,
            player,
            TechniqueCmd::Proficiency {
                id: ECHO.to_string(),
                value: 1.5,
            },
        );
        send(
            &mut app,
            player,
            TechniqueCmd::Active {
                id: ECHO.to_string(),
                value: false,
            },
        );
        run_update(&mut app);

        let entry = app
            .world()
            .get::<KnownTechniques>(player)
            .unwrap()
            .entries
            .iter()
            .find(|entry| entry.id == ECHO)
            .unwrap();
        assert_eq!(entry.proficiency, 1.0);
        assert!(!entry.active);
    }

    #[test]
    fn technique_proficiency_rejects_non_finite_values() {
        let mut app = setup_app();
        let player = spawn_known(&mut app, KnownTechniques::default());
        let before = app
            .world()
            .get::<KnownTechniques>(player)
            .unwrap()
            .entries
            .iter()
            .find(|entry| entry.id == ECHO)
            .unwrap()
            .proficiency;

        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            send(
                &mut app,
                player,
                TechniqueCmd::Proficiency {
                    id: ECHO.to_string(),
                    value,
                },
            );
        }
        run_update(&mut app);

        let entry = app
            .world()
            .get::<KnownTechniques>(player)
            .unwrap()
            .entries
            .iter()
            .find(|entry| entry.id == ECHO)
            .unwrap();
        assert_eq!(entry.proficiency, before);
        assert!(entry.proficiency.is_finite());
    }

    #[test]
    fn technique_reset_all_restores_default_set() {
        let mut app = setup_app();
        let player = spawn_known(
            &mut app,
            KnownTechniques {
                entries: vec![KnownTechnique {
                    id: BENG_QUAN.to_string(),
                    proficiency: 0.1,
                    active: false,
                }],
            },
        );

        send(&mut app, player, TechniqueCmd::ResetAll);
        run_update(&mut app);

        let techniques = app.world().get::<KnownTechniques>(player).unwrap();
        assert_eq!(techniques.entries.len(), default_technique_count());
        assert!(techniques
            .entries
            .iter()
            .any(|entry| entry.id == BENG_QUAN && entry.active));
    }
}
