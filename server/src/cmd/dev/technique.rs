use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::parsers::CommandArg;
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{App, Client, EventReader, Query, Update};

use crate::cultivation::known_techniques::{
    technique_definition, KnownTechnique, KnownTechniques, TECHNIQUE_DEFINITIONS,
};

#[derive(Debug, Clone, PartialEq)]
pub enum TechniqueCmd {
    List,
    Add { id: String },
    Give { id: String },
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
            .literal("give")
            .argument("id")
            .with_parser::<String>()
            .with_executable(|input| TechniqueCmd::Give {
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
                for line in technique_catalog_lines(&techniques) {
                    client.send_chat_message(line);
                }
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
            TechniqueCmd::Give { id } => {
                if id == "all" {
                    let granted = grant_all_techniques(&mut techniques);
                    client.send_chat_message(format!(
                        "[dev] technique give all: added={} activated={} total={}",
                        granted.added,
                        granted.activated,
                        techniques.entries.len()
                    ));
                    continue;
                }
                let Some(definition) = technique_definition(id) else {
                    client.send_chat_message(format!(
                        "[dev] technique give rejected: unknown `{id}`; use /technique list"
                    ));
                    continue;
                };
                match grant_technique(&mut techniques, definition.id) {
                    TechniqueGrantResult::Added => client.send_chat_message(format!(
                        "[dev] technique give `{}` ({}) added",
                        definition.id, definition.display_name
                    )),
                    TechniqueGrantResult::Activated => client.send_chat_message(format!(
                        "[dev] technique give `{}` ({}) activated",
                        definition.id, definition.display_name
                    )),
                    TechniqueGrantResult::AlreadyKnown => client.send_chat_message(format!(
                        "[dev] technique give `{}` ({}) already known",
                        definition.id, definition.display_name
                    )),
                }
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
                *techniques = KnownTechniques::dev_default();
                client.send_chat_message(format!(
                    "[dev] technique reset_all; entries={}",
                    techniques.entries.len()
                ));
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TechniqueGrantResult {
    Added,
    Activated,
    AlreadyKnown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TechniqueGrantSummary {
    added: usize,
    activated: usize,
}

fn technique_catalog_lines(techniques: &KnownTechniques) -> Vec<String> {
    let mut lines = vec![format!(
        "[dev] technique list: {} definitions; * = known; use /technique give <id|all>",
        TECHNIQUE_DEFINITIONS.len()
    )];
    let mut grade: Option<&str> = None;
    let mut parts = Vec::new();
    for definition in TECHNIQUE_DEFINITIONS {
        if grade.is_some_and(|current| current != definition.grade) {
            flush_catalog_line(&mut lines, grade.unwrap_or("unknown"), &mut parts);
        }
        grade = Some(definition.grade);
        let known = techniques
            .entries
            .iter()
            .find(|entry| entry.id == definition.id);
        let marker = known.map_or("", |_| "*");
        let suffix = known
            .map(|entry| format!(" p={:.2} active={}", entry.proficiency, entry.active))
            .unwrap_or_default();
        parts.push(format!(
            "{marker}{}({}){suffix}",
            definition.id, definition.display_name
        ));
    }
    if let Some(grade) = grade {
        flush_catalog_line(&mut lines, grade, &mut parts);
    }
    lines
}

fn flush_catalog_line(lines: &mut Vec<String>, grade: &str, parts: &mut Vec<String>) {
    if !parts.is_empty() {
        lines.push(format!("[dev] technique {grade}: {}", parts.join(", ")));
        parts.clear();
    }
}

fn grant_all_techniques(techniques: &mut KnownTechniques) -> TechniqueGrantSummary {
    let mut summary = TechniqueGrantSummary {
        added: 0,
        activated: 0,
    };
    for definition in TECHNIQUE_DEFINITIONS {
        match grant_technique(techniques, definition.id) {
            TechniqueGrantResult::Added => summary.added += 1,
            TechniqueGrantResult::Activated => summary.activated += 1,
            TechniqueGrantResult::AlreadyKnown => {}
        }
    }
    summary
}

fn grant_technique(techniques: &mut KnownTechniques, id: &str) -> TechniqueGrantResult {
    if let Some(entry) = techniques.entries.iter_mut().find(|entry| entry.id == id) {
        if entry.active {
            return TechniqueGrantResult::AlreadyKnown;
        }
        entry.active = true;
        return TechniqueGrantResult::Activated;
    }
    techniques.entries.push(KnownTechnique {
        id: id.to_string(),
        proficiency: 0.5,
        active: true,
    });
    TechniqueGrantResult::Added
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
        KnownTechniques::dev_default().entries.len()
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
        let player = spawn_known(&mut app, KnownTechniques::dev_default());

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
    fn technique_catalog_exposes_all_defined_ids() {
        let lines = technique_catalog_lines(&KnownTechniques {
            entries: Vec::new(),
        });
        let joined = lines.join("\n");
        assert!(joined.contains("use /technique give <id|all>"));
        assert!(joined.contains("movement.dash(闪避)"));
        assert!(joined.contains("woliu.vortex"));
        assert!(joined.contains("anqi.echo_fractal"));
        assert_eq!(
            lines[0],
            format!(
                "[dev] technique list: {} definitions; * = known; use /technique give <id|all>",
                TECHNIQUE_DEFINITIONS.len()
            )
        );
    }

    #[test]
    fn technique_add_is_idempotent_and_rejects_unknown_ids() {
        let mut app = setup_app();
        let player = spawn_known(&mut app, KnownTechniques::dev_default());

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
    fn technique_give_adds_from_empty_and_reactivates_existing() {
        let mut app = setup_app();
        let player = spawn_known(
            &mut app,
            KnownTechniques {
                entries: vec![KnownTechnique {
                    id: ECHO.to_string(),
                    proficiency: 0.75,
                    active: false,
                }],
            },
        );

        send(
            &mut app,
            player,
            TechniqueCmd::Give {
                id: NEEDLE.to_string(),
            },
        );
        send(
            &mut app,
            player,
            TechniqueCmd::Give {
                id: ECHO.to_string(),
            },
        );
        send(
            &mut app,
            player,
            TechniqueCmd::Give {
                id: "missing.technique".to_string(),
            },
        );
        run_update(&mut app);

        let techniques = app.world().get::<KnownTechniques>(player).unwrap();
        assert_eq!(techniques.entries.len(), 2);
        assert!(techniques
            .entries
            .iter()
            .any(|entry| entry.id == NEEDLE && entry.active));
        let echo = techniques
            .entries
            .iter()
            .find(|entry| entry.id == ECHO)
            .unwrap();
        assert_eq!(echo.proficiency, 0.75);
        assert!(echo.active);
        assert!(techniques
            .entries
            .iter()
            .all(|entry| entry.id != "missing.technique"));
    }

    #[test]
    fn technique_give_all_adds_missing_without_resetting_proficiency() {
        let mut app = setup_app();
        let player = spawn_known(
            &mut app,
            KnownTechniques {
                entries: vec![KnownTechnique {
                    id: BENG_QUAN.to_string(),
                    proficiency: 0.9,
                    active: false,
                }],
            },
        );

        send(
            &mut app,
            player,
            TechniqueCmd::Give {
                id: "all".to_string(),
            },
        );
        run_update(&mut app);

        let techniques = app.world().get::<KnownTechniques>(player).unwrap();
        assert_eq!(techniques.entries.len(), TECHNIQUE_DEFINITIONS.len());
        let beng = techniques
            .entries
            .iter()
            .find(|entry| entry.id == BENG_QUAN)
            .unwrap();
        assert_eq!(beng.proficiency, 0.9);
        assert!(beng.active);
    }

    #[test]
    fn technique_remove_allows_unknown_but_removes_existing() {
        let mut app = setup_app();
        let player = spawn_known(&mut app, KnownTechniques::dev_default());

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
        let player = spawn_known(&mut app, KnownTechniques::dev_default());

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
        let player = spawn_known(&mut app, KnownTechniques::dev_default());
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

    #[test]
    fn technique_add_woliu_works_from_empty_default() {
        let mut app = setup_app();
        let player = spawn_known(&mut app, KnownTechniques::default());

        send(
            &mut app,
            player,
            TechniqueCmd::Add {
                id: "woliu.vortex".to_string(),
            },
        );
        run_update(&mut app);

        let techniques = app.world().get::<KnownTechniques>(player).unwrap();
        assert_eq!(techniques.entries.len(), 1);
        assert!(techniques
            .entries
            .iter()
            .any(|entry| entry.id == "woliu.vortex" && entry.active));
    }
}
