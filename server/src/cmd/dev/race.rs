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
use crate::cultivation::race_change::{
    commit_race_change, precheck_race_change, RaceChangeRejection,
};

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
            reply(
                world,
                event.executor,
                "[dev] race set unavailable: RaceRegistry not loaded",
            );
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
                    format!(
                        "[dev] race set rejected: {}",
                        describe_rejection(&rejection)
                    ),
                );
            }
        }
    }
}

fn describe_rejection(rejection: &RaceChangeRejection) -> String {
    match rejection {
        RaceChangeRejection::UnknownRace(id) => format!("unknown race {id}"),
        RaceChangeRejection::MissingBodyPlanRegistry => {
            "body plan registry unavailable".to_string()
        }
        RaceChangeRejection::UnknownBodyPlan(id) => format!("unknown body plan {id}"),
        RaceChangeRejection::MissingCultivation => "missing Cultivation component".to_string(),
        RaceChangeRejection::MissingMeridianSystem => {
            "missing MeridianSystem component".to_string()
        }
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
    use crate::body_plan::race_registry::{MeridianMappingDef, RaceEntry};
    use crate::body_plan::registry::BodyPlanRegistry;
    use crate::body_plan::types::{
        BodyPartDef, ChannelDef, HeightBand, HeightBandAssignment, HitGeometry, MeridianFamily,
        PartConsequence, RealmMeridianReq, StandingAabbSpec,
    };
    use crate::body_plan::{IntrinsicRace, HUMAN_RACE_ID};
    use crate::cultivation::components::{
        Cultivation, Meridian, MeridianChannelId, MeridianSystem,
    };
    use crate::cultivation::meridian::severed::MeridianSeveredPermanent;
    use crate::inventory::{DroppedLootRegistry, ItemRegistry, PlayerInventory};
    use crate::qi_physics::QiTransfer;
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

    /// 与 `plan_with_meridian_channels`（此文件顶层其它两个测试模块中同名 fixture）
    /// 目的相同：命令级测试需要一个自带 `meridian_profile` 的 body plan，用来验证
    /// `/race set` 走真实入口时经脉迁移/休眠真的发生了。
    fn trivial_plan_with_meridian(id: &str, channel_ids: &[&str]) -> crate::body_plan::BodyPlan {
        use crate::body_plan::types::MeridianProfile;
        let mut plan = trivial_plan(id);
        plan.meridian_profile = Some(MeridianProfile {
            channels: channel_ids
                .iter()
                .map(|cid| ChannelDef {
                    id: MeridianChannelId::new(*cid),
                    family: MeridianFamily::Regular,
                    body_part: None,
                    roles: vec![],
                })
                .collect(),
            topology_edges: vec![],
            realm_requirements: [RealmMeridianReq::default(); 6],
            dugu_injection: vec![],
        });
        plan
    }

    /// 命令级 happy-path / 拒绝测试共用 fixture：human(lung, heart) --[lung->fin]-->
    /// whale(fin, tail)。`heart` 无映射来源 → 换种后应进休眠登记；`tail` 无映射目标
    /// → 换种后应是全新未打通默认值。与 `cultivation::race_change` 单测里的同构
    /// fixture故意保持一致口径（那边测纯函数，这里测真实命令入口，二者独立锁）。
    fn human_whale_registry_with_partial_mapping() -> (BodyPlanRegistry, RaceRegistry) {
        let mut human_plan = trivial_plan_with_meridian("humanoid", &["lung", "heart"]);
        human_plan.is_humanoid = true;
        let whale_plan = trivial_plan_with_meridian("whale", &["fin", "tail"]);
        let body_plans =
            BodyPlanRegistry::from_plans(vec![human_plan, whale_plan]).expect("valid plans");
        let races = RaceRegistry::from_parts_for_test_with_meridian_mappings(
            vec![
                RaceEntry {
                    id: RaceId::new(HUMAN_RACE_ID),
                    display_name: "人族".to_string(),
                    body_plan_id: "humanoid".into(),
                    beast_kinds: vec![],
                },
                RaceEntry {
                    id: RaceId::new("whale"),
                    display_name: "飞鲸".to_string(),
                    body_plan_id: "whale".into(),
                    beast_kinds: vec![],
                },
            ],
            vec![],
            vec![MeridianMappingDef {
                from: RaceId::new(HUMAN_RACE_ID),
                to: RaceId::new("whale"),
                entries: vec![(
                    MeridianChannelId::new("lung"),
                    MeridianChannelId::new("fin"),
                )],
            }],
            &body_plans,
        )
        .expect("valid registry with meridian mapping");
        (body_plans, races)
    }

    /// 空库存 fixture——用于拒绝测试断言"库存零变更"（无装备/无容器，任何写入都
    /// 会在断言里现形），也用于 happy-path 测试排除装备门重扫的干扰噪音。
    fn empty_player_inventory() -> PlayerInventory {
        PlayerInventory {
            revision: crate::inventory::InventoryRevision(0),
            containers: Vec::new(),
            equipped: std::collections::HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 99.0,
            triggered_treasures: Vec::new(),
        }
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
                .any(|text| text.contains("bot_e2e_no_such_race")
                    && text.contains("unknown race id")),
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
            chats
                .iter()
                .any(|text| text.contains("RaceRegistry not loaded")),
            "缺 RaceRegistry 必须有明确反馈，实际：{chats:?}"
        );
    }

    /// 命令级 happy-path：真实命令入口（`CommandExecutionC2s`）走完 `/race set whale`
    /// 全链路提交，断言"命令解析→precheck→commit"这条链**真的**发生了，而不是被前置
    /// 拒绝分支覆盖住的假绿（review A/B/C/D 共识 MAJOR：此前全部命令级测试都走的是
    /// 拒绝分支，删掉 `commit_race_change` 调用现有测试仍然全绿）。
    #[test]
    fn race_set_whale_happy_path_commits_full_transition_with_success_feedback() {
        let (body_plans, races) = human_whale_registry_with_partial_mapping();
        let mut app = setup_command_app();
        app.insert_resource(body_plans);
        app.insert_resource(races);
        app.insert_resource(ItemRegistry::from_map(Default::default()));
        app.insert_resource(DroppedLootRegistry::default());

        let (bundle, mut helper) = create_mock_client("Alice");
        let player = app.world_mut().spawn(bundle).id();

        let cultivation = Cultivation {
            qi_current: 5.0,
            qi_max: 100.0,
            race: RaceId::new(HUMAN_RACE_ID),
            ..Default::default()
        };
        let mut meridians = MeridianSystem {
            regular: vec![
                Meridian::new(MeridianChannelId::new("lung")),
                Meridian::new(MeridianChannelId::new("heart")),
            ],
            extraordinary: vec![],
        };
        meridians.get_mut(MeridianChannelId::new("lung")).opened = true;
        meridians.get_mut(MeridianChannelId::new("lung")).integrity = 0.8;

        app.world_mut().entity_mut(player).insert((
            cultivation,
            meridians,
            MeridianSeveredPermanent::default(),
            empty_player_inventory(),
        ));

        execute_command(&mut app, &mut helper, "race set whale");

        let chats = flush_and_collect_chat(&mut app, &mut helper);
        assert!(
            chats.iter().any(|text| text.contains("race set -> whale")),
            "命令级成功切换必须有玩家可见反馈，实际：{chats:?}"
        );

        let cultivation = app
            .world()
            .get::<Cultivation>(player)
            .expect("player must retain Cultivation after commit");
        assert_eq!(
            cultivation.race,
            RaceId::new("whale"),
            "Cultivation.race 必须切换到目标种族"
        );
        // 新构型只有 fin(opened, flow_capacity=10.0 默认值) 一条经脉打通，tail 未打通
        // 不计容量——qi_max 必须精确重算为 10.0(基线) + 10.0(fin) = 20.0。
        assert_eq!(cultivation.qi_max, 20.0, "qi_max 必须按新经脉系统精确重算");
        assert_eq!(
            cultivation.qi_current, 5.0,
            "qi_current(5.0) 未超新 qi_max(20.0)，应原样保留、不触发超额释放"
        );

        let intrinsic = app
            .world()
            .get::<IntrinsicRace>(player)
            .expect("commit must insert/replace IntrinsicRace");
        assert_eq!(intrinsic.0, RaceId::new("whale"));

        let new_meridians = app
            .world()
            .get::<MeridianSystem>(player)
            .expect("player must retain MeridianSystem after commit");
        assert!(
            new_meridians.contains(MeridianChannelId::new("fin")),
            "新经脉系统必须来自 whale body_plan 的 profile"
        );
        let fin = new_meridians.get(MeridianChannelId::new("fin"));
        assert!(
            fin.opened,
            "lung(opened) 经 meridian_mapping 映射到 fin，打通状态必须原样迁移"
        );
        assert_eq!(
            fin.integrity, 0.8,
            "迁移必须带着原 integrity 一起走，不是全新默认值"
        );
        let tail = new_meridians.get(MeridianChannelId::new("tail"));
        assert!(!tail.opened, "tail 无映射来源，必须是全新未打通默认值");
        assert!(
            !new_meridians.contains(MeridianChannelId::new("lung")),
            "旧种族 channel id 不应残留在新 MeridianSystem 里"
        );

        let severed = app
            .world()
            .get::<MeridianSeveredPermanent>(player)
            .expect("commit must retain MeridianSeveredPermanent");
        assert!(
            severed.is_dormant(MeridianChannelId::new("heart")),
            "heart 无映射来源，必须进入休眠登记（而非静默丢弃）"
        );
    }

    #[test]
    fn race_set_rejects_with_zero_side_effects_when_target_differs_from_source_race() {
        // review A/B/C/D 共识 MAJOR：human→human 的旧拒绝测试无法判别 commit 到底有没
        // 有发生（初态==目标态，字段值巧合相同）。改为 human→whale（不同的合法目标
        // race），precheck 命中 MissingMeridianSystem 拒绝分支，逐项断言 race/经脉/
        // 休眠登记/库存/qi/ledger/掉落实体全部零变更——commit_race_change 一旦被误触
        // 发，这些断言里至少有一条会撞红。
        let (body_plans, races) = human_whale_registry_with_partial_mapping();
        let mut app = setup_command_app();
        app.insert_resource(body_plans);
        app.insert_resource(races);
        app.insert_resource(ItemRegistry::from_map(Default::default()));
        app.insert_resource(DroppedLootRegistry::default());

        let (bundle, mut helper) = create_mock_client("Alice");
        let player = app.world_mut().spawn(bundle).id();
        let cultivation = Cultivation {
            qi_current: 5.0,
            qi_max: 100.0,
            race: RaceId::new(HUMAN_RACE_ID),
            ..Default::default()
        };
        // 只插入 Cultivation + PlayerInventory，故意不插入 MeridianSystem。
        app.world_mut()
            .entity_mut(player)
            .insert((cultivation, empty_player_inventory()));

        execute_command(&mut app, &mut helper, "race set whale");

        let chats = flush_and_collect_chat(&mut app, &mut helper);
        assert!(
            chats
                .iter()
                .any(|text| text.contains("missing MeridianSystem component")),
            "precheck 失败必须反馈明确原因，实际：{chats:?}"
        );

        let after_cultivation = app
            .world()
            .get::<Cultivation>(player)
            .expect("Cultivation must still be present, untouched");
        assert_eq!(
            after_cultivation.race,
            RaceId::new(HUMAN_RACE_ID),
            "race 字段绝不能改变（目标 whale != 初态 human，能真正判别 commit 有没有发生）"
        );
        assert_eq!(after_cultivation.qi_current, 5.0, "qi_current 不得改变");
        assert_eq!(after_cultivation.qi_max, 100.0, "qi_max 不得改变");

        assert!(
            app.world().get::<IntrinsicRace>(player).is_none(),
            "precheck 失败不得写 IntrinsicRace"
        );
        assert!(
            app.world()
                .get::<MeridianSeveredPermanent>(player)
                .is_none(),
            "precheck 失败不得插入 MeridianSeveredPermanent（经脉休眠登记零变更）"
        );

        let inventory_after = app
            .world()
            .get::<PlayerInventory>(player)
            .expect("inventory must still be present, untouched");
        assert_eq!(
            inventory_after.equipped.len(),
            0,
            "库存装备槽不得变化（precheck 失败绝不触发装备门重扫）"
        );
        assert_eq!(inventory_after.containers.len(), 0, "背包容器不得变化");

        let dropped = app
            .world()
            .get_resource::<DroppedLootRegistry>()
            .expect("DroppedLootRegistry resource must still exist");
        assert!(dropped.entries.is_empty(), "不得产生任何掉落实体");

        let qi_events = app.world().resource::<Events<QiTransfer>>();
        assert_eq!(
            qi_events.len(),
            0,
            "ledger 零变更：precheck 失败绝不得 emit 任何 QiTransfer 审计事件"
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
