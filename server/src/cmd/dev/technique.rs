use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::parsers::CommandArg;
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{App, Client, EventReader, Query, Res, Update, Username};

use crate::combat::components::{SkillBarBindings, SkillSlot};
use crate::cultivation::known_techniques::{KnownTechnique, KnownTechniques, TechniqueRegistry};
use crate::player::state::{update_player_ui_prefs, PlayerStatePersistence, SkillSlotPersist};

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
            // dev-only: 直接改写已知功法，绕过残卷/师承/顿悟与 qi_physics 路径。
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

type TechniqueCmdItem<'a> = (
    &'a mut KnownTechniques,
    &'a mut Client,
    Option<&'a mut SkillBarBindings>,
    Option<&'a Username>,
);

pub fn handle_technique(
    mut events: EventReader<CommandResultEvent<TechniqueCmd>>,
    registry: Res<TechniqueRegistry>,
    mut players: Query<TechniqueCmdItem<'_>>,
    persistence: Option<Res<PlayerStatePersistence>>,
) {
    let registry = registry.as_ref();
    for event in events.read() {
        let Ok((mut techniques, mut client, skill_bar, username)) = players.get_mut(event.executor)
        else {
            continue;
        };

        match &event.result {
            TechniqueCmd::List => {
                for line in technique_catalog_lines(registry, &techniques) {
                    client.send_chat_message(line);
                }
            }
            TechniqueCmd::Add { id } => {
                if registry.get(id).is_none() {
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
                    let granted = grant_all_techniques(registry, &mut techniques);
                    client.send_chat_message(format!(
                        "[dev] technique give all: added={} activated={} total={}",
                        granted.added,
                        granted.activated,
                        techniques.entries.len()
                    ));
                    continue;
                }
                let Some(definition) = registry.get(id) else {
                    client.send_chat_message(format!(
                        "[dev] technique give rejected: unknown `{id}`; use /technique list"
                    ));
                    continue;
                };
                match grant_technique(&mut techniques, &definition.id) {
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
                let removed = techniques.entries.len() != before;
                let cleared = prune_stale_bindings_after_mutation(
                    &techniques,
                    registry,
                    skill_bar,
                    username,
                    persistence.as_deref(),
                );
                client.send_chat_message(format!(
                    "[dev] technique `{id}` removed={removed} bindings_cleared={cleared}"
                ));
            }
            TechniqueCmd::Proficiency { id, value } => {
                if registry.get(id).is_none() {
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
                if registry.get(id).is_none() {
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
                *techniques = KnownTechniques::dev_default(registry);
                let cleared = prune_stale_bindings_after_mutation(
                    &techniques,
                    registry,
                    skill_bar,
                    username,
                    persistence.as_deref(),
                );
                client.send_chat_message(format!(
                    "[dev] technique reset_all; entries={} bindings_cleared={cleared}",
                    techniques.entries.len()
                ));
            }
        }
    }
}

/// remove/reset_all 后技能栏里指向已不再拥有功法的绑定必须一并清掉：
/// 运行时不清会留死图标（cast 被 ownership 门拒但 HUD 仍显示），持久化
/// 不清则旧偏好仍会保留无效绑定。冷却按 skill_id 全量按 known 集合裁剪，
/// 避免已经解绑的功法仍带着旧冷却，在重新授予后复活。
fn prune_stale_bindings_after_mutation(
    known: &KnownTechniques,
    registry: &TechniqueRegistry,
    skill_bar: Option<impl std::ops::DerefMut<Target = SkillBarBindings>>,
    username: Option<&Username>,
    persistence: Option<&PlayerStatePersistence>,
) -> usize {
    let mut cleared = 0;
    if let Some(mut bindings) = skill_bar {
        if prune_unknown_dash_binding(&mut bindings.dash_skill_id, known, registry) {
            cleared += 1;
        }
        cleared += prune_unknown_skill_slots(&mut bindings, known);
    }
    if let (Some(persistence), Some(username)) = (persistence, username) {
        if let Err(error) = update_player_ui_prefs(persistence, username.0.as_str(), |prefs| {
            prune_unknown_dash_binding(&mut prefs.dash_skill_id, known, registry);
            prune_unknown_persisted_skill_slots(&mut prefs.skill_bar, known);
        }) {
            tracing::warn!(
                "[dev] failed to prune persisted skill bar for `{}` after technique mutation: {error}",
                username.0
            );
        }
    }
    cleared
}

fn prune_unknown_dash_binding(
    dash_skill_id: &mut Option<String>,
    known: &KnownTechniques,
    registry: &TechniqueRegistry,
) -> bool {
    let stale = dash_skill_id.as_deref().is_some_and(|skill_id| {
        !known_contains(known, skill_id)
            || registry
                .get(skill_id)
                .is_none_or(|definition| definition.input_kind() != "dash")
    });
    if stale {
        *dash_skill_id = None;
    }
    stale
}

fn known_contains(known: &KnownTechniques, skill_id: &str) -> bool {
    known.entries.iter().any(|entry| entry.id == skill_id)
}

fn prune_unknown_skill_slots(bindings: &mut SkillBarBindings, known: &KnownTechniques) -> usize {
    let mut cleared = 0;
    for slot in 0..SkillBarBindings::SLOT_COUNT {
        let stale = matches!(
            &bindings.slots[slot],
            SkillSlot::Skill { skill_id } if !known_contains(known, skill_id)
        );
        if stale {
            bindings.set(slot as u8, SkillSlot::Empty);
            cleared += 1;
        }
    }
    bindings
        .cooldowns
        .retain(|skill_id, _| known_contains(known, skill_id));
    cleared
}

fn prune_unknown_persisted_skill_slots(
    skill_bar: &mut [SkillSlotPersist; SkillBarBindings::SLOT_COUNT],
    known: &KnownTechniques,
) -> usize {
    let mut cleared = 0;
    for slot in skill_bar.iter_mut() {
        let stale = matches!(
            slot,
            SkillSlotPersist::Skill { skill_id } if !known_contains(known, skill_id)
        );
        if stale {
            *slot = SkillSlotPersist::Empty;
            cleared += 1;
        }
    }
    cleared
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

fn technique_catalog_lines(
    registry: &TechniqueRegistry,
    techniques: &KnownTechniques,
) -> Vec<String> {
    let mut lines = vec![format!(
        "[dev] technique list: {} definitions; * = known; use /technique give <id|all>",
        registry.len()
    )];
    let mut grade: Option<&str> = None;
    let mut parts = Vec::new();
    for definition in registry.iter() {
        if grade.is_some_and(|current| current != definition.grade.as_str()) {
            flush_catalog_line(&mut lines, grade.unwrap_or("unknown"), &mut parts);
        }
        grade = Some(&definition.grade);
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

fn grant_all_techniques(
    registry: &TechniqueRegistry,
    techniques: &mut KnownTechniques,
) -> TechniqueGrantSummary {
    let mut summary = TechniqueGrantSummary {
        added: 0,
        activated: 0,
    };
    for definition in registry.iter() {
        match grant_technique(techniques, &definition.id) {
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
    use crate::cultivation::known_techniques::TechniqueDefinition;
    use valence::prelude::{Client, Events, Position};
    use valence::protocol::packets::play::GameMessageS2c;
    use valence::testing::{create_mock_client, MockClientHelper};

    const BENG_QUAN: &str = "burst_meridian.beng_quan";
    const NEEDLE: &str = "dugu.shoot_needle";
    const ECHO: &str = "anqi.echo_fractal";

    fn setup_app() -> App {
        let mut app = App::new();
        app.insert_resource(runtime_registry());
        app.add_event::<CommandResultEvent<TechniqueCmd>>();
        app.add_systems(Update, handle_technique);
        app
    }

    /// M03：dev 命令必须消费注入的权威 runtime registry，而不是独立重读默认
    /// catalog。所有命令测试统一安装带 runtime-only 招式的 registry——`runtime.only`
    /// 只存在于注入实例，checked-in catalog 中没有。若命令 handler 改读默认
    /// catalog（或传入别的 registry 实例），`give`/`list`/`add` 会找不到它并撞红。
    fn runtime_registry() -> TechniqueRegistry {
        TechniqueRegistry::load_for_tests_with_definition(TechniqueDefinition {
            id: "runtime.only".to_string(),
            display_name: "运行时专属".to_string(),
            grade: "common".to_string(),
            description: "只存在于注入 registry 的 runtime-only 招式（M03 契约）。".to_string(),
            required_realm: "Awaken".to_string(),
            required_meridians: Vec::new(),
            required_race: crate::body_plan::RaceGateOwned::Any,
            qi_cost: 1.0,
            stamina_cost: 1.0,
            cast_ticks: 10,
            cooldown_ticks: 20,
            range: 3.0,
            icon_texture: "bong-client:textures/gui/items/skill_scroll_runtime_only.png"
                .to_string(),
            category: crate::cultivation::known_techniques::SkillCategory::Attack,
            dispatch: crate::cultivation::known_techniques::TechniqueDispatch::DirectGeneric,
        })
    }

    fn spawn_known_with_helper(
        app: &mut App,
        techniques: KnownTechniques,
    ) -> (valence::prelude::Entity, MockClientHelper) {
        let (mut client_bundle, helper) = create_mock_client("Alice");
        client_bundle.player.position = Position::new([0.0, 0.0, 0.0]);
        let player = app.world_mut().spawn(client_bundle).id();
        app.world_mut().entity_mut(player).insert(techniques);
        (player, helper)
    }

    fn flush_client_packets(app: &mut App) {
        let world = app.world_mut();
        let mut query = world.query::<&mut Client>();
        for mut client in query.iter_mut(world) {
            client
                .flush_packets()
                .expect("mock client packets should flush successfully");
        }
    }

    fn collect_chat_texts(helper: &mut MockClientHelper) -> Vec<String> {
        helper
            .collect_received()
            .0
            .into_iter()
            .filter_map(|frame| {
                frame
                    .decode::<GameMessageS2c>()
                    .ok()
                    .map(|packet| packet.chat.to_string())
            })
            .collect()
    }

    fn spawn_known(app: &mut App, techniques: KnownTechniques) -> valence::prelude::Entity {
        let player = spawn_test_client(app, "Alice", [0.0, 0.0, 0.0]);
        app.world_mut().entity_mut(player).insert(techniques);
        player
    }

    fn default_technique_count() -> usize {
        runtime_registry().len()
    }

    fn bindings_with(slots: &[(usize, SkillSlot)]) -> SkillBarBindings {
        let mut bindings = SkillBarBindings::default();
        for (slot, value) in slots {
            assert!(
                *slot < SkillBarBindings::SLOT_COUNT,
                "test slot {slot} must fit the runtime skill-bar contract"
            );
            bindings.set(*slot as u8, value.clone());
        }
        bindings
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
        let player = spawn_known(&mut app, KnownTechniques::dev_default(&runtime_registry()));

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
        let registry = runtime_registry();
        let lines = technique_catalog_lines(
            &registry,
            &KnownTechniques {
                entries: Vec::new(),
            },
        );
        let joined = lines.join("\n");
        assert!(joined.contains("use /technique give <id|all>"));
        assert!(joined.contains("movement.dash(闪避)"));
        assert!(joined.contains("woliu.vortex"));
        assert!(joined.contains("anqi.echo_fractal"));
        assert_eq!(
            lines[0],
            format!(
                "[dev] technique list: {} definitions; * = known; use /technique give <id|all>",
                registry.len()
            )
        );
    }

    #[test]
    fn technique_add_is_idempotent_and_rejects_unknown_ids() {
        let mut app = setup_app();
        let player = spawn_known(&mut app, KnownTechniques::dev_default(&runtime_registry()));

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
        assert_eq!(
            techniques.entries.len(),
            app.world().resource::<TechniqueRegistry>().len()
        );
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
        let player = spawn_known(&mut app, KnownTechniques::dev_default(&runtime_registry()));

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
        let player = spawn_known(&mut app, KnownTechniques::dev_default(&runtime_registry()));

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
        let player = spawn_known(&mut app, KnownTechniques::dev_default(&runtime_registry()));
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
    fn technique_remove_clears_only_removed_skill_bindings() {
        let mut app = setup_app();
        let player = spawn_known(&mut app, KnownTechniques::dev_default(&runtime_registry()));
        app.world_mut().entity_mut(player).insert(bindings_with(&[
            (
                0,
                SkillSlot::Skill {
                    skill_id: NEEDLE.to_string(),
                },
            ),
            (
                1,
                SkillSlot::Skill {
                    skill_id: ECHO.to_string(),
                },
            ),
        ]));

        send(
            &mut app,
            player,
            TechniqueCmd::Remove {
                id: NEEDLE.to_string(),
            },
        );
        run_update(&mut app);

        let bindings = app.world().get::<SkillBarBindings>(player).unwrap();
        assert_eq!(bindings.slots[0], SkillSlot::Empty);
        assert_eq!(
            bindings.slots[1],
            SkillSlot::Skill {
                skill_id: ECHO.to_string()
            },
            "a still-known skill binding must survive removing another skill"
        );
    }

    #[test]
    fn technique_remove_clears_cooldown_for_pruned_skill_id() {
        let mut app = setup_app();
        let player = spawn_known(&mut app, KnownTechniques::dev_default(&runtime_registry()));
        let mut bindings = bindings_with(&[(
            0,
            SkillSlot::Skill {
                skill_id: NEEDLE.to_string(),
            },
        )]);
        bindings.set_cooldown(NEEDLE, 12_345);
        app.world_mut().entity_mut(player).insert(bindings);

        send(
            &mut app,
            player,
            TechniqueCmd::Remove {
                id: NEEDLE.to_string(),
            },
        );
        run_update(&mut app);

        let bindings = app.world().get::<SkillBarBindings>(player).unwrap();
        assert!(
            !bindings.is_on_cooldown(NEEDLE, 0),
            "removing a skill must remove its skill-id cooldown with the stale binding"
        );
        assert!(!bindings.cooldowns.contains_key(NEEDLE));
    }

    #[test]
    fn technique_remove_clears_cooldown_for_unbound_removed_skill() {
        let mut app = setup_app();
        let player = spawn_known(&mut app, KnownTechniques::dev_default(&runtime_registry()));
        let mut bindings = bindings_with(&[(
            0,
            SkillSlot::Skill {
                skill_id: ECHO.to_string(),
            },
        )]);
        bindings.set_cooldown(NEEDLE, 12_345);
        app.world_mut().entity_mut(player).insert(bindings);

        send(
            &mut app,
            player,
            TechniqueCmd::Remove {
                id: NEEDLE.to_string(),
            },
        );
        run_update(&mut app);

        let bindings = app.world().get::<SkillBarBindings>(player).unwrap();
        assert!(
            !bindings.cooldowns.contains_key(NEEDLE),
            "removing a skill must clear its cooldown even when no slot still references it"
        );
        assert_eq!(
            bindings.slots[0],
            SkillSlot::Skill {
                skill_id: ECHO.to_string()
            }
        );
    }

    #[test]
    fn technique_remove_clears_stale_dash_binding() {
        let mut app = setup_app();
        let player = spawn_known(
            &mut app,
            KnownTechniques {
                entries: vec![KnownTechnique {
                    id: "legacy.dash".to_string(),
                    proficiency: 0.5,
                    active: true,
                }],
            },
        );
        let bindings = SkillBarBindings {
            dash_skill_id: Some("legacy.dash".to_string()),
            ..Default::default()
        };
        app.world_mut().entity_mut(player).insert(bindings);

        send(
            &mut app,
            player,
            TechniqueCmd::Remove {
                id: "legacy.dash".to_string(),
            },
        );
        run_update(&mut app);

        assert_eq!(
            app.world()
                .get::<SkillBarBindings>(player)
                .unwrap()
                .dash_skill_id,
            None,
            "remove must clear the runtime dash binding when its technique is no longer known"
        );
    }

    #[test]
    fn technique_remove_clears_known_non_dash_binding() {
        let mut app = setup_app();
        let player = spawn_known(
            &mut app,
            KnownTechniques {
                entries: vec![
                    KnownTechnique {
                        id: BENG_QUAN.to_string(),
                        proficiency: 0.5,
                        active: true,
                    },
                    KnownTechnique {
                        id: NEEDLE.to_string(),
                        proficiency: 0.5,
                        active: true,
                    },
                ],
            },
        );
        let bindings = SkillBarBindings {
            dash_skill_id: Some(BENG_QUAN.to_string()),
            ..Default::default()
        };
        app.world_mut().entity_mut(player).insert(bindings);

        send(
            &mut app,
            player,
            TechniqueCmd::Remove {
                id: NEEDLE.to_string(),
            },
        );
        run_update(&mut app);

        assert_eq!(
            app.world()
                .get::<SkillBarBindings>(player)
                .unwrap()
                .dash_skill_id,
            None,
            "a known non-dash technique must not survive as a dash binding"
        );
    }

    #[test]
    fn technique_reset_all_prunes_bindings_absent_from_runtime_defaults() {
        let mut app = setup_app();
        let player = spawn_known(
            &mut app,
            KnownTechniques {
                entries: vec![KnownTechnique {
                    id: "legacy.gone".to_string(),
                    proficiency: 0.4,
                    active: true,
                }],
            },
        );
        app.world_mut().entity_mut(player).insert(bindings_with(&[
            (
                0,
                SkillSlot::Skill {
                    skill_id: "legacy.gone".to_string(),
                },
            ),
            (
                1,
                SkillSlot::Skill {
                    skill_id: BENG_QUAN.to_string(),
                },
            ),
        ]));

        send(&mut app, player, TechniqueCmd::ResetAll);
        run_update(&mut app);

        let bindings = app.world().get::<SkillBarBindings>(player).unwrap();
        assert_eq!(bindings.slots[0], SkillSlot::Empty);
        assert_eq!(
            bindings.slots[1],
            SkillSlot::Skill {
                skill_id: BENG_QUAN.to_string()
            },
            "reset_all must retain a binding present in the injected runtime registry"
        );
    }

    #[test]
    fn prune_runtime_slots_clears_every_stale_slot() {
        let known = KnownTechniques {
            entries: Vec::new(),
        };
        let mut bindings = SkillBarBindings::default();
        for slot in 0..SkillBarBindings::SLOT_COUNT {
            bindings.slots[slot] = SkillSlot::Skill {
                skill_id: format!("gone.{slot}"),
            };
        }

        let cleared = prune_unknown_skill_slots(&mut bindings, &known);

        assert_eq!(cleared, SkillBarBindings::SLOT_COUNT);
        assert!(bindings.slots.iter().all(|slot| *slot == SkillSlot::Empty));
    }

    #[test]
    fn prune_persisted_slots_clears_only_stale_skill_kind() {
        let known = KnownTechniques {
            entries: vec![KnownTechnique {
                id: BENG_QUAN.to_string(),
                proficiency: 0.2,
                active: true,
            }],
        };
        let mut skill_bar: [SkillSlotPersist; SkillBarBindings::SLOT_COUNT] = Default::default();
        skill_bar[0] = SkillSlotPersist::Skill {
            skill_id: BENG_QUAN.to_string(),
        };
        skill_bar[1] = SkillSlotPersist::Skill {
            skill_id: "legacy.gone".to_string(),
        };

        let cleared = prune_unknown_persisted_skill_slots(&mut skill_bar, &known);

        assert_eq!(cleared, 1);
        assert_eq!(
            skill_bar[0],
            SkillSlotPersist::Skill {
                skill_id: BENG_QUAN.to_string()
            }
        );
        assert_eq!(skill_bar[1], SkillSlotPersist::Empty);
    }

    #[test]
    fn prune_persisted_slots_preserves_item_bindings() {
        let known = KnownTechniques {
            entries: Vec::new(),
        };
        let mut skill_bar: [SkillSlotPersist; SkillBarBindings::SLOT_COUNT] = Default::default();
        skill_bar[0] = SkillSlotPersist::Item {
            template_id: "healing_herb".to_string(),
        };
        skill_bar[1] = SkillSlotPersist::Skill {
            skill_id: "legacy.gone".to_string(),
        };

        let cleared = prune_unknown_persisted_skill_slots(&mut skill_bar, &known);

        assert_eq!(cleared, 1);
        assert_eq!(
            skill_bar[0],
            SkillSlotPersist::Item {
                template_id: "healing_herb".to_string()
            }
        );
        assert_eq!(skill_bar[1], SkillSlotPersist::Empty);
    }

    #[test]
    fn prune_persisted_dash_binding_requires_known_dash_definition() {
        let registry = runtime_registry();
        let known = KnownTechniques {
            entries: vec![
                KnownTechnique {
                    id: BENG_QUAN.to_string(),
                    proficiency: 0.5,
                    active: true,
                },
                KnownTechnique {
                    id: crate::movement::dash_proficiency::DASH_TECHNIQUE_ID.to_string(),
                    proficiency: 0.5,
                    active: true,
                },
            ],
        };
        let mut prefs = crate::player::state::PlayerUiPrefs {
            dash_skill_id: Some("legacy.dash".to_string()),
            ..Default::default()
        };
        assert!(prune_unknown_dash_binding(
            &mut prefs.dash_skill_id,
            &known,
            &registry
        ));
        assert_eq!(prefs.dash_skill_id, None);

        let mut known_non_dash = Some(BENG_QUAN.to_string());
        assert!(prune_unknown_dash_binding(
            &mut known_non_dash,
            &known,
            &registry
        ));
        assert_eq!(known_non_dash, None);

        let mut known_dash = Some(crate::movement::dash_proficiency::DASH_TECHNIQUE_ID.to_string());
        assert!(!prune_unknown_dash_binding(
            &mut known_dash,
            &known,
            &registry
        ));
        assert_eq!(
            known_dash.as_deref(),
            Some(crate::movement::dash_proficiency::DASH_TECHNIQUE_ID)
        );
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

    #[test]
    fn runtime_only_technique_is_consumed_by_dev_commands() {
        // M03：dev 命令必须消费注入 registry 的 runtime-only 招式——若 handler 改读
        // 默认/静态 catalog，`runtime.only` 会从 list/give/remove 中消失并撞红。
        let mut app = setup_app();
        let (player, mut helper) = spawn_known_with_helper(&mut app, KnownTechniques::default());

        // give all：registry 注入的 runtime-only 招式必须出现在授予集合。
        send(
            &mut app,
            player,
            TechniqueCmd::Give {
                id: "all".to_string(),
            },
        );
        run_update(&mut app);
        let techniques = app.world().get::<KnownTechniques>(player).unwrap();
        assert!(
            techniques
                .entries
                .iter()
                .any(|entry| entry.id == "runtime.only"),
            "give all 必须授予注入 registry 的 runtime-only 招式（M03）"
        );

        // list：catalog 行必须包含 runtime-only 招式（消费注入 registry，非独立重读）。
        send(&mut app, player, TechniqueCmd::List);
        run_update(&mut app);
        flush_client_packets(&mut app);
        let chats = collect_chat_texts(&mut helper);
        assert!(
            chats.iter().any(|line| line.contains("runtime.only")),
            "list 必须通过客户端聊天输出注入 registry 的 runtime-only 招式，实际 {chats:?}"
        );

        // remove：runtime-only 招式可被移除（默认 catalog 不认识它也能移除）。
        send(
            &mut app,
            player,
            TechniqueCmd::Remove {
                id: "runtime.only".to_string(),
            },
        );
        run_update(&mut app);
        let techniques = app.world().get::<KnownTechniques>(player).unwrap();
        assert!(
            techniques
                .entries
                .iter()
                .all(|entry| entry.id != "runtime.only"),
            "remove 必须能移除注入 registry 的 runtime-only 招式（M03）"
        );

        // add：移除后仍必须从注入 registry 重新学会 runtime-only 招式。
        send(
            &mut app,
            player,
            TechniqueCmd::Add {
                id: "runtime.only".to_string(),
            },
        );
        run_update(&mut app);
        let entry = app
            .world()
            .get::<KnownTechniques>(player)
            .unwrap()
            .entries
            .iter()
            .find(|entry| entry.id == "runtime.only")
            .expect("add 必须从注入 registry 学会 runtime-only 招式（M03）");
        assert_eq!(
            entry.proficiency, 0.5,
            "add 的 runtime-only 招式必须使用 dev 初始熟练度"
        );
        assert!(entry.active, "add 的 runtime-only 招式必须默认激活");

        // proficiency：注入 registry 认识 runtime-only 后，才能修改其熟练度。
        send(
            &mut app,
            player,
            TechniqueCmd::Proficiency {
                id: "runtime.only".to_string(),
                value: 0.875,
            },
        );
        run_update(&mut app);
        let entry = app
            .world()
            .get::<KnownTechniques>(player)
            .unwrap()
            .entries
            .iter()
            .find(|entry| entry.id == "runtime.only")
            .unwrap();
        assert_eq!(
            entry.proficiency, 0.875,
            "proficiency 必须修改注入 registry 的 runtime-only 招式"
        );

        // active：同一 runtime-only entry 的激活状态必须可切换。
        send(
            &mut app,
            player,
            TechniqueCmd::Active {
                id: "runtime.only".to_string(),
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
            .find(|entry| entry.id == "runtime.only")
            .unwrap();
        assert!(
            !entry.active,
            "active false 必须作用于注入 registry 的 runtime-only 招式"
        );

        // reset_all：重置集合仍必须来自注入 registry，并保留 runtime-only 条目。
        send(&mut app, player, TechniqueCmd::ResetAll);
        run_update(&mut app);
        let entry = app
            .world()
            .get::<KnownTechniques>(player)
            .unwrap()
            .entries
            .iter()
            .find(|entry| entry.id == "runtime.only")
            .expect("reset_all 必须从注入 registry 恢复 runtime-only 招式（M03）");
        assert_eq!(
            entry.proficiency, 0.5,
            "reset_all 必须恢复 runtime-only 招式的默认熟练度"
        );
        assert!(
            entry.active,
            "reset_all 必须恢复 runtime-only 招式的激活状态"
        );
    }
}
