//! plan-supply-coffin-v1 P3.1 — `/supply_coffin` dev 命令族。
//!
//! 全部 dev-only 入口：显式绕过 wall-clock 冷却 + zone 选点（plan §0 设计轴心
//! 3），不允许从生产 gameplay 路径复用。
//!
//! 子命令：
//! - `spawn <grade>` — 在玩家脚下强制 spawn 一个指定 grade 的物资棺
//! - `list` — 把当前 active + cooldown 状态发回给执行者
//! - `reset` — 清空 active + cooldown，让 refresh tick 重新初始化
//! - `cooldown <grade> <secs>` — 临时把指定 grade 的首个冷却推迟到
//!   `now - cooldown_secs + <secs>`（等价于"还剩 secs 秒"）

use bevy_transform::components::{GlobalTransform, Transform};
use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::parsers::{CommandArg, CommandArgParseError, ParseInput};
use valence::command::{AddCommand, Command};
use valence::entity::entity::NoGravity;
use valence::entity::marker::MarkerEntityBundle;
use valence::message::SendMessage;
use valence::prelude::{
    App, Client, Commands, EntityLayerId, EventReader, Look, Position, Query, ResMut, Update,
};
use valence::protocol::packets::play::command_tree_s2c::Parser;

use crate::supply_coffin::refresh::SupplyCoffinMarker;
use crate::supply_coffin::{current_wall_clock_secs, SupplyCoffinGrade, SupplyCoffinRegistry};
use crate::world::entity_model::{BongVisualEntity, BongVisualState};

/// 解析 grade 字符串参数。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SupplyCoffinGradeArg(pub SupplyCoffinGrade);

impl CommandArg for SupplyCoffinGradeArg {
    fn parse_arg(input: &mut ParseInput) -> Result<Self, CommandArgParseError> {
        let raw = String::parse_arg(input)?;
        SupplyCoffinGrade::from_str(raw.as_str())
            .map(Self)
            .ok_or_else(|| CommandArgParseError::InvalidArgument {
                expected: "common|rare|precious".to_string(),
                got: raw,
            })
    }

    fn display() -> Parser {
        String::display()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupplyCoffinCmd {
    Spawn { grade: SupplyCoffinGrade },
    List,
    Reset,
    Cooldown { grade: SupplyCoffinGrade, secs: u64 },
}

impl Command for SupplyCoffinCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        let root = graph.root().literal("supply_coffin").id();

        graph
            .at(root)
            .literal("spawn")
            .argument("grade")
            .with_parser::<SupplyCoffinGradeArg>()
            .with_executable(|input| SupplyCoffinCmd::Spawn {
                grade: SupplyCoffinGradeArg::parse_arg(input).unwrap().0,
            });

        graph
            .at(root)
            .literal("list")
            .with_executable(|_| SupplyCoffinCmd::List);

        graph
            .at(root)
            .literal("reset")
            .with_executable(|_| SupplyCoffinCmd::Reset);

        graph
            .at(root)
            .literal("cooldown")
            .argument("grade")
            .with_parser::<SupplyCoffinGradeArg>()
            .argument("secs")
            .with_parser::<i32>()
            .with_executable(|input| {
                let grade = SupplyCoffinGradeArg::parse_arg(input).unwrap().0;
                let secs = i32::parse_arg(input).unwrap().max(0) as u64;
                SupplyCoffinCmd::Cooldown { grade, secs }
            });
    }
}

pub fn register(app: &mut App) {
    app.add_command::<SupplyCoffinCmd>()
        .add_systems(Update, handle_supply_coffin_cmd);
}

#[allow(clippy::too_many_arguments)]
pub fn handle_supply_coffin_cmd(
    mut commands: Commands,
    mut events: EventReader<CommandResultEvent<SupplyCoffinCmd>>,
    registry: Option<ResMut<SupplyCoffinRegistry>>,
    mut clients: Query<&mut Client>,
    layers: Option<valence::prelude::Res<crate::world::dimension::DimensionLayers>>,
    positions: Query<&Position>,
    despawnable_markers: Query<(valence::prelude::Entity, &SupplyCoffinMarker)>,
) {
    // SupplyCoffinRegistry resource 仅在 supply_coffin::register() 注入。dev cmd
    // 在 cmd::register() 早于运行时 register 的最小化测试 App 里也会被 tick，
    // 用 Option<…> 守门避免 missing-resource panic（参考 dev/kill.rs 同模式）。
    let Some(mut registry) = registry else {
        // 把 reader drain，避免事件堆积导致后续 tick 重复处理。
        for _ in events.read() {}
        return;
    };
    for event in events.read() {
        match event.result {
            SupplyCoffinCmd::Spawn { grade } => {
                let Some(layers) = layers.as_deref() else {
                    reply(
                        &mut clients,
                        event.executor,
                        "[dev] DimensionLayers missing",
                    );
                    continue;
                };
                let Ok(executor_pos) = positions.get(event.executor) else {
                    reply(
                        &mut clients,
                        event.executor,
                        "[dev] executor lacks Position",
                    );
                    continue;
                };
                let pos = executor_pos.get();
                let visual_kind = grade.visual_kind();
                let now = current_wall_clock_secs();
                let entity = commands
                    .spawn((
                        MarkerEntityBundle {
                            kind: visual_kind.entity_kind(),
                            layer: EntityLayerId(layers.overworld),
                            position: Position::new([pos.x, pos.y, pos.z]),
                            entity_no_gravity: NoGravity(true),
                            look: Look::new(0.0, 0.0),
                            ..Default::default()
                        },
                        Transform::from_xyz(pos.x as f32, pos.y as f32, pos.z as f32),
                        GlobalTransform::default(),
                        BongVisualEntity {
                            kind: visual_kind,
                            source: None,
                        },
                        BongVisualState(0),
                        SupplyCoffinMarker { grade },
                    ))
                    .id();
                registry.insert_active(entity, grade, pos, now);
                reply(
                    &mut clients,
                    event.executor,
                    format!(
                        "[dev] spawned {} at ({:.1},{:.1},{:.1}) active={}",
                        grade.as_str(),
                        pos.x,
                        pos.y,
                        pos.z,
                        registry.active_count(grade)
                    ),
                );
            }
            SupplyCoffinCmd::List => {
                let mut lines = vec![format!(
                    "[dev] active={} cooldowns={}",
                    registry.active.len(),
                    registry.cooldowns.len()
                )];
                for grade in SupplyCoffinGrade::ALL {
                    lines.push(format!(
                        "  {}: active={}/{} cd={}",
                        grade.as_str(),
                        registry.active_count(grade),
                        grade.max_active(),
                        registry
                            .cooldowns
                            .iter()
                            .filter(|c| c.grade == grade)
                            .count(),
                    ));
                }
                reply(&mut clients, event.executor, lines.join("\n"));
            }
            SupplyCoffinCmd::Reset => {
                // Despawn all marker entities so they disappear client-side too.
                for (entity, _) in &despawnable_markers {
                    commands.entity(entity).insert(valence::prelude::Despawned);
                }
                let cleared_active = registry.active.len();
                let cleared_cooldowns = registry.cooldowns.len();
                registry.active.clear();
                registry.cooldowns.clear();
                reply(
                    &mut clients,
                    event.executor,
                    format!(
                        "[dev] reset cleared active={} cooldowns={}",
                        cleared_active, cleared_cooldowns
                    ),
                );
            }
            SupplyCoffinCmd::Cooldown { grade, secs } => {
                // Re-target first matching cooldown so its `is_ready(now)` becomes true
                // after `secs` seconds from now: set broken_at = now - cooldown_secs + secs.
                let now = current_wall_clock_secs();
                let target_broken_at = now
                    .saturating_add(secs)
                    .saturating_sub(grade.cooldown_secs());
                if let Some(c) = registry.cooldowns.iter_mut().find(|c| c.grade == grade) {
                    c.broken_at_wall_secs = target_broken_at;
                    reply(
                        &mut clients,
                        event.executor,
                        format!(
                            "[dev] cooldown {} retargeted to ready in {}s",
                            grade.as_str(),
                            secs
                        ),
                    );
                } else {
                    // No cooldown entry for this grade → enqueue one
                    registry
                        .cooldowns
                        .push(crate::supply_coffin::CoffinCooldown {
                            grade,
                            broken_at_wall_secs: target_broken_at,
                        });
                    reply(
                        &mut clients,
                        event.executor,
                        format!(
                            "[dev] cooldown {} enqueued (ready in {}s)",
                            grade.as_str(),
                            secs
                        ),
                    );
                }
            }
        }
    }
}

fn reply(
    clients: &mut Query<&mut Client>,
    executor: valence::prelude::Entity,
    message: impl Into<String>,
) {
    if let Ok(mut client) = clients.get_mut(executor) {
        client.send_chat_message(message.into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::dev::test_support::{run_update, spawn_test_client};
    use crate::supply_coffin::SupplyCoffinRegistry;
    use crate::world::dimension::{DimensionLayers, OverworldLayer};
    use valence::prelude::{App, DVec3, Entity, Events};

    fn setup_app(with_layers: bool) -> App {
        let mut app = App::new();
        app.add_event::<CommandResultEvent<SupplyCoffinCmd>>();
        app.insert_resource(SupplyCoffinRegistry::new(
            (DVec3::new(0.0, 0.0, 0.0), DVec3::new(100.0, 0.0, 100.0)),
            65.0,
            0xCAFE,
        ));
        if with_layers {
            // Spawn dummy entities to act as dimension layers; bind via local
            // bindings to avoid double-mutable-borrow of `app`.
            let overworld = app.world_mut().spawn_empty().id();
            let tsy = app.world_mut().spawn_empty().id();
            app.insert_resource(DimensionLayers { overworld, tsy });
            app.world_mut().spawn(OverworldLayer);
        }
        app.add_systems(Update, handle_supply_coffin_cmd);
        app
    }

    fn send(app: &mut App, executor: Entity, cmd: SupplyCoffinCmd) {
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<SupplyCoffinCmd>>>()
            .send(CommandResultEvent {
                result: cmd,
                executor,
                modifiers: Default::default(),
            });
    }

    #[test]
    fn grade_arg_parses_all_three_variants() {
        assert_eq!(
            SupplyCoffinGradeArg::arg_from_str("common").unwrap().0,
            SupplyCoffinGrade::Common
        );
        assert_eq!(
            SupplyCoffinGradeArg::arg_from_str("rare").unwrap().0,
            SupplyCoffinGrade::Rare
        );
        assert_eq!(
            SupplyCoffinGradeArg::arg_from_str("precious").unwrap().0,
            SupplyCoffinGrade::Precious
        );
    }

    #[test]
    fn grade_arg_rejects_unknown_input() {
        assert!(SupplyCoffinGradeArg::arg_from_str("epic").is_err());
        assert!(SupplyCoffinGradeArg::arg_from_str("Common").is_err());
        assert!(SupplyCoffinGradeArg::arg_from_str("").is_err());
    }

    #[test]
    fn list_reports_empty_state_when_registry_is_empty() {
        let mut app = setup_app(false);
        let player = spawn_test_client(&mut app, "Alice", [0.0, 0.0, 0.0]);
        send(&mut app, player, SupplyCoffinCmd::List);
        run_update(&mut app);
        // 不验证 chat 内容（需 Valence helper）；仅校验 registry 未被破坏。
        let r = app.world().resource::<SupplyCoffinRegistry>();
        assert_eq!(r.active.len(), 0);
        assert_eq!(r.cooldowns.len(), 0);
    }

    #[test]
    fn reset_clears_active_and_cooldowns() {
        let mut app = setup_app(false);
        let player = spawn_test_client(&mut app, "Alice", [0.0, 0.0, 0.0]);

        // 预先放一些状态
        {
            let mut r = app.world_mut().resource_mut::<SupplyCoffinRegistry>();
            r.insert_active(
                Entity::from_raw(99),
                SupplyCoffinGrade::Common,
                DVec3::new(10.0, 65.0, 10.0),
                0,
            );
            r.enqueue_cooldown(SupplyCoffinGrade::Rare, 100);
        }

        send(&mut app, player, SupplyCoffinCmd::Reset);
        run_update(&mut app);

        let r = app.world().resource::<SupplyCoffinRegistry>();
        assert_eq!(r.active.len(), 0, "reset 后 active 必空");
        assert_eq!(r.cooldowns.len(), 0, "reset 后 cooldowns 必空");
    }

    #[test]
    fn cooldown_subcommand_retargets_existing_cooldown_to_finish_in_n_seconds() {
        let mut app = setup_app(false);
        let player = spawn_test_client(&mut app, "Alice", [0.0, 0.0, 0.0]);

        // 预先放 Common 冷却
        {
            let mut r = app.world_mut().resource_mut::<SupplyCoffinRegistry>();
            r.enqueue_cooldown(SupplyCoffinGrade::Common, current_wall_clock_secs());
        }

        send(
            &mut app,
            player,
            SupplyCoffinCmd::Cooldown {
                grade: SupplyCoffinGrade::Common,
                secs: 5,
            },
        );
        run_update(&mut app);

        let r = app.world().resource::<SupplyCoffinRegistry>();
        // is_ready(now+5) 必须 true；is_ready(now+4) 必须 false
        let now = current_wall_clock_secs();
        assert!(
            !r.cooldowns[0].is_ready(now + 4),
            "Cooldown 子命令应让 cooldown 在 5s 后到期，而不是 4s"
        );
        assert!(
            r.cooldowns[0].is_ready(now + 5),
            "Cooldown 子命令必须让 cooldown 恰在 5s 后到期"
        );
    }

    #[test]
    fn cooldown_subcommand_enqueues_when_no_matching_entry_exists() {
        let mut app = setup_app(false);
        let player = spawn_test_client(&mut app, "Alice", [0.0, 0.0, 0.0]);
        send(
            &mut app,
            player,
            SupplyCoffinCmd::Cooldown {
                grade: SupplyCoffinGrade::Precious,
                secs: 10,
            },
        );
        run_update(&mut app);

        let r = app.world().resource::<SupplyCoffinRegistry>();
        assert_eq!(r.cooldowns.len(), 1);
        assert_eq!(r.cooldowns[0].grade, SupplyCoffinGrade::Precious);
    }

    #[test]
    fn spawn_subcommand_inserts_active_at_executor_position() {
        let mut app = setup_app(true);
        let player = spawn_test_client(&mut app, "Alice", [42.0, 65.0, 99.0]);
        send(
            &mut app,
            player,
            SupplyCoffinCmd::Spawn {
                grade: SupplyCoffinGrade::Rare,
            },
        );
        run_update(&mut app);

        let r = app.world().resource::<SupplyCoffinRegistry>();
        assert_eq!(r.active.len(), 1, "spawn 必须插入一个 active");
        let (_, rec) = r.active.iter().next().unwrap();
        assert_eq!(rec.grade, SupplyCoffinGrade::Rare);
        assert!((rec.pos.x - 42.0).abs() < 0.01);
        assert!((rec.pos.y - 65.0).abs() < 0.01);
        assert!((rec.pos.z - 99.0).abs() < 0.01);
    }

    #[test]
    fn spawn_subcommand_noop_when_dimension_layers_missing() {
        let mut app = setup_app(false); // 不插入 DimensionLayers
        let player = spawn_test_client(&mut app, "Alice", [0.0, 0.0, 0.0]);
        send(
            &mut app,
            player,
            SupplyCoffinCmd::Spawn {
                grade: SupplyCoffinGrade::Common,
            },
        );
        run_update(&mut app);

        let r = app.world().resource::<SupplyCoffinRegistry>();
        assert_eq!(
            r.active.len(),
            0,
            "缺 DimensionLayers 时不应插入 active（dev 命令应平稳报错）"
        );
    }
}
