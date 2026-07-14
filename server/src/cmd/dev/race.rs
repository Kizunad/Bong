//! plan-race-system-v1 P5 / PR-6a — `/race set <id>` dev 命令。
//!
//! **dev-only**：直写玩家种族的入口，但落地方式是走真实两阶段事务
//! （`cultivation::race_change::{precheck_race_change, commit_race_change}`），不是
//! 绕过装备门/经脉迁移/qi 守恒的裸字段赋值——唯一被"绕过"的是境界自然修炼流程
//! （玩家不需要真的换体/渡劫）。仿 `cmd::dev::realm`：brigadier 只做字符串解析，
//! 目标 race id 是否存在的校验放在 handler 里对着 `Res<RaceRegistry>` 动态查（不像
//! `realm.rs` 的 6 境界是编译期闭合枚举，种族是 registry-driven 的开放字符串集合，
//! brigadier `CommandArg::parse` 没有 ECS 资源访问权限，无法在解析阶段完成这一步）。

use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::parsers::{CommandArg, ParseInput};
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{App, Client, Events, Update};

use crate::body_plan::{RaceId, RaceRegistry};
use crate::cultivation::race_change::{commit_race_change, precheck_race_change, RaceChangeRejection};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RaceCmd {
    Set { raw: String },
}

impl Command for RaceCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        graph
            .root()
            .literal("race")
            .literal("set")
            .argument("id")
            .with_parser::<String>()
            .with_executable(|input: &mut ParseInput| RaceCmd::Set {
                raw: String::parse_arg(input)
                    .expect("brigadier should pre-validate race id as a string"),
            });
    }
}

pub fn register(app: &mut App) {
    app.add_command::<RaceCmd>()
        .add_systems(Update, handle_race_cmd);
}

/// exclusive system —— `precheck_race_change` 与 `commit_race_change` 必须在同一次
/// 调用内、不跨 tick 顺序执行（见 `race_change` 模块文档：两阶段之间 world 状态一旦
/// 漂移，precheck 产出的 plan 就不再保证适用）。
pub fn handle_race_cmd(world: &mut valence::prelude::bevy_ecs::world::World) {
    let pending: Vec<CommandResultEvent<RaceCmd>> = {
        let Some(mut events) = world.get_resource_mut::<Events<CommandResultEvent<RaceCmd>>>()
        else {
            return;
        };
        events.drain().collect()
    };
    if pending.is_empty() {
        return;
    }

    let Some(registry) = world.get_resource::<RaceRegistry>().cloned() else {
        for event in pending {
            reply(world, event.executor, "[dev] race set unavailable: RaceRegistry not loaded");
        }
        return;
    };

    for event in pending {
        let RaceCmd::Set { raw } = event.result;
        let target = RaceId::new(raw.trim());
        if registry.get(&target).is_none() {
            reply(
                world,
                event.executor,
                format!("[dev] race set rejected: unknown race id {raw:?}"),
            );
            continue;
        }

        match precheck_race_change(world, event.executor, target.clone(), &registry) {
            Ok(plan) => {
                commit_race_change(world, event.executor, plan);
                tracing::warn!(
                    "[dev-cmd] bypass natural cultivation: entity={:?} race -> {target:?}",
                    event.executor
                );
                reply(
                    world,
                    event.executor,
                    format!("[dev] race set -> {}", target.as_str()),
                );
            }
            Err(rejection) => {
                reply(
                    world,
                    event.executor,
                    format!("[dev] race set rejected: {}", describe_rejection(&rejection)),
                );
            }
        }
    }
}

fn describe_rejection(rejection: &RaceChangeRejection) -> String {
    match rejection {
        RaceChangeRejection::UnknownRace(id) => format!("unknown race {id}"),
        RaceChangeRejection::MissingBodyPlanRegistry => "body plan registry unavailable".to_string(),
        RaceChangeRejection::UnknownBodyPlan(id) => format!("unknown body plan {id}"),
        RaceChangeRejection::MissingCultivation => "missing Cultivation component".to_string(),
        RaceChangeRejection::MissingMeridianSystem => "missing MeridianSystem component".to_string(),
        RaceChangeRejection::MeridianMappingSourceMissing(channel) => {
            format!("meridian mapping source channel {channel} missing from entity")
        }
        RaceChangeRejection::QiTransferPrepareFailed(error) => {
            format!("qi transfer prepare failed: {error}")
        }
    }
}

fn reply(
    world: &mut valence::prelude::bevy_ecs::world::World,
    executor: valence::prelude::Entity,
    message: impl Into<String>,
) {
    if let Some(mut client) = world.get_mut::<Client>(executor) {
        client.send_chat_message(message.into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body_plan::race_registry::RaceEntry;
    use crate::body_plan::registry::BodyPlanRegistry;
    use crate::body_plan::types::{
        BodyPartDef, HeightBand, HeightBandAssignment, HitGeometry, PartConsequence,
        StandingAabbSpec,
    };
    use crate::body_plan::HUMAN_RACE_ID;
    use crate::cultivation::components::{Cultivation, MeridianSystem};
    use crate::cultivation::meridian::severed::MeridianSeveredPermanent;
    use valence::protocol::packets::play::{CommandExecutionC2s, GameMessageS2c};
    use valence::protocol::{Bounded, FixedBitSet, VarInt};
    use valence::testing::{create_mock_client, MockClientHelper};

    fn trivial_plan(id: &str) -> crate::body_plan::BodyPlan {
        crate::body_plan::BodyPlan {
            id: id.into(),
            display_name: id.to_string(),
            is_humanoid: false,
            parts: vec![BodyPartDef {
                id: "core".into(),
                damage_mul: 1.0,
                contam_mul: 1.0,
                bleed_mul: 1.0,
                consequence: PartConsequence::Core,
            }],
            hit_geometry: HitGeometry::HeightBands {
                aabb: StandingAabbSpec {
                    half_width: 0.3,
                    height: 1.8,
                },
                bands: vec![HeightBand {
                    min_rel_y: -1.0,
                    assignment: HeightBandAssignment::Single {
                        part: "core".into(),
                    },
                }],
                lateral_threshold: 0.19,
            },
            equip_slots: vec![],
            meridian_profile: None,
            mutation_slot_mapping: Default::default(),
        }
    }

    fn human_only_registry() -> (BodyPlanRegistry, RaceRegistry) {
        let body_plans =
            BodyPlanRegistry::from_plans(vec![trivial_plan("humanoid")]).expect("valid plans");
        let races = RaceRegistry::from_parts_for_test(
            vec![RaceEntry {
                id: RaceId::new(HUMAN_RACE_ID),
                display_name: "人族".to_string(),
                body_plan_id: "humanoid".into(),
                beast_kinds: vec![],
            }],
            vec![],
            &body_plans,
        )
        .expect("valid registry");
        (body_plans, races)
    }

    fn setup_command_app() -> valence::prelude::App {
        let mut app = valence::prelude::App::new();
        app.add_plugins((
            valence::event_loop::EventLoopPlugin,
            valence::command::manager::CommandPlugin,
        ));
        app.add_event::<crate::qi_physics::QiTransfer>();
        register(&mut app);
        app.finish();
        app.cleanup();
        app.update();
        app
    }

    fn execute_command(
        app: &mut valence::prelude::App,
        helper: &mut MockClientHelper,
        command: &str,
    ) {
        helper.send(&CommandExecutionC2s {
            command: Bounded(command),
            timestamp: 0,
            salt: 0,
            argument_signatures: Vec::new(),
            message_count: VarInt(0),
            acknowledgement: FixedBitSet::default(),
        });
        app.update();
    }

    fn flush_and_collect_chat(
        app: &mut valence::prelude::App,
        helper: &mut MockClientHelper,
    ) -> Vec<String> {
        let world = app.world_mut();
        let mut clients = world.query::<&mut Client>();
        for mut client in clients.iter_mut(world) {
            client
                .flush_packets()
                .expect("mock client packets should flush successfully");
        }
        helper
            .collect_received()
            .0
            .into_iter()
            .filter_map(|frame| {
                frame
                    .decode::<GameMessageS2c>()
                    .ok()
                    .map(|packet| packet.chat.to_legacy_lossy())
            })
            .collect()
    }

    #[test]
    fn race_set_unknown_id_rejected_with_player_visible_feedback() {
        let (body_plans, races) = human_only_registry();
        let mut app = setup_command_app();
        app.insert_resource(body_plans);
        app.insert_resource(races);
        let (bundle, mut helper) = create_mock_client("Alice");
        let player = app.world_mut().spawn(bundle).id();
        app.world_mut().entity_mut(player).insert((
            Cultivation::default(),
            MeridianSystem::default(),
            MeridianSeveredPermanent::default(),
        ));

        execute_command(&mut app, &mut helper, "race set bot_e2e_no_such_race");

        let chats = flush_and_collect_chat(&mut app, &mut helper);
        assert!(
            chats
                .iter()
                .any(|text| text.contains("bot_e2e_no_such_race") && text.contains("unknown race id")),
            "非法 race id 必须返回包含原输入的玩家 chat，实际：{chats:?}"
        );
        assert_eq!(
            app.world().get::<Cultivation>(player).unwrap().race,
            RaceId::new(HUMAN_RACE_ID),
            "非法 race id 不得修改种族"
        );
    }

    #[test]
    fn race_set_missing_registry_rejects_all_pending_with_feedback() {
        let mut app = setup_command_app();
        // 故意不插入 RaceRegistry。
        let (bundle, mut helper) = create_mock_client("Alice");
        let player = app.world_mut().spawn(bundle).id();
        app.world_mut()
            .entity_mut(player)
            .insert((Cultivation::default(), MeridianSystem::default()));

        execute_command(&mut app, &mut helper, "race set whale");

        let chats = flush_and_collect_chat(&mut app, &mut helper);
        assert!(
            chats.iter().any(|text| text.contains("RaceRegistry not loaded")),
            "缺 RaceRegistry 必须有明确反馈，实际：{chats:?}"
        );
    }

    #[test]
    fn race_set_does_not_mutate_race_field_directly_bypassing_race_change() {
        // 回归锁：本命令必须走 precheck/commit 两阶段事务，不是裸字段赋值——用一个会
        // 让 precheck 失败的场景（缺 MeridianSystem）证明：即便目标 race 合法存在，
        // 缺前置组件时整个提交都不应发生。
        let (body_plans, races) = human_only_registry();
        let mut app = setup_command_app();
        app.insert_resource(body_plans);
        app.insert_resource(races);
        let (bundle, mut helper) = create_mock_client("Alice");
        let player = app.world_mut().spawn(bundle).id();
        // 只插入 Cultivation，故意不插入 MeridianSystem。
        app.world_mut().entity_mut(player).insert(Cultivation::default());

        execute_command(&mut app, &mut helper, "race set human");

        let chats = flush_and_collect_chat(&mut app, &mut helper);
        assert!(
            chats
                .iter()
                .any(|text| text.contains("missing MeridianSystem component")),
            "precheck 失败必须反馈明确原因，实际：{chats:?}"
        );
    }

    #[test]
    fn race_cmd_variant_pin() {
        let a = RaceCmd::Set {
            raw: "human".to_string(),
        };
        let b = RaceCmd::Set {
            raw: "human".to_string(),
        };
        assert_eq!(a, b);
    }
}
