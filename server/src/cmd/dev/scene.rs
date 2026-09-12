//! 固定名称的实机测试场景。仅在 BONG_TEST_ENV 开启时注册，复用真实状态及 HUD 同步链路。

use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::parsers::CommandArg;
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{
    App, Client, EventReader, IntoSystemConfigs, Position, Query, Res, Resource, Update, Username,
};

use super::DevCommandPermissions;
use crate::combat::components::{
    ActiveStatusEffect, Lifecycle, LifecycleState, StatusEffects, TICKS_PER_SECOND,
};
use crate::combat::events::StatusEffectKind;
use crate::combat::{status, CombatSystemSet};
use crate::gathering::tools::GatheringTargetKind;
use crate::gathering::GatheringSystemSet;
use crate::network::status_snapshot_emit::emit_status_snapshot_payloads;

mod gathering;

pub(crate) struct SceneDefinition {
    pub name: &'static str,
    pub description: &'static str,
    content: SceneContent,
}

enum SceneContent {
    StatusEffects(&'static [(StatusEffectKind, f32, u64)]),
    Gathering {
        target: GatheringTargetKind,
        target_name: &'static str,
    },
}

// 每个场景明确指定效果、强度和秒数；组合不从 enum 或分类自动生成。
pub(crate) const SCENES: &[SceneDefinition] = &[
    SceneDefinition {
        name: "test_status_effect_animation_1",
        description: "流血、僵直、丹毒、疾行：逐个入场、越框侵染、最后 5 秒闪烁和退场",
        content: SceneContent::StatusEffects(&[
            (StatusEffectKind::Bleeding, 0.1, 12),
            (StatusEffectKind::Stunned, 1.0, 7),
            (StatusEffectKind::ContaminationBoost, 0.1, 25),
            (StatusEffectKind::SpeedBoost, 0.2, 28),
        ]),
    },
    SceneDefinition {
        name: "test_debuff_effect_combo_1",
        description: "流血、迟缓、易伤、真元停滞、虚脱，观察负面图标与分批到期",
        content: SceneContent::StatusEffects(&[
            (StatusEffectKind::Bleeding, 0.1, 120),
            (StatusEffectKind::Slowed, 0.35, 120),
            (StatusEffectKind::DamageVulnerability, 0.25, 90),
            (StatusEffectKind::QiRegenPaused, 1.0, 60),
            (StatusEffectKind::Exhausted, 0.5, 30),
        ]),
    },
    SceneDefinition {
        name: "test_buff_effect_combo_1",
        description: "减伤、疾行、回力、养伤、修炼加速，观察增益图标与分批到期",
        content: SceneContent::StatusEffects(&[
            (StatusEffectKind::DamageReduction, 0.25, 120),
            (StatusEffectKind::SpeedBoost, 0.2, 120),
            (StatusEffectKind::StaminaRecovBoost, 0.25, 90),
            (StatusEffectKind::HealthRegenBoost, 0.25, 60),
            (StatusEffectKind::CultivationAcceleration, 0.5, 30),
        ]),
    },
    SceneDefinition {
        name: "test_status_effect_mixed_1",
        description: "流血、控制、负面和增益混排，观察状态栏优先级与控制解除",
        content: SceneContent::StatusEffects(&[
            (StatusEffectKind::SpeedBoost, 0.2, 120),
            (StatusEffectKind::Slowed, 0.35, 90),
            (StatusEffectKind::Stunned, 1.0, 15),
            (StatusEffectKind::Bleeding, 0.1, 120),
            (StatusEffectKind::DamageReduction, 0.25, 120),
            (StatusEffectKind::QiRegenPaused, 1.0, 60),
            (StatusEffectKind::Disoriented, 1.0, 30),
            (StatusEffectKind::StaminaRecovBoost, 0.25, 90),
        ]),
    },
    SceneDefinition {
        name: "test_status_effect_overflow_1",
        description: "12 种状态，观察顶部 8 格筛选和 Inspect 状态列表",
        content: SceneContent::StatusEffects(&[
            (StatusEffectKind::SpeedBoost, 0.2, 120),
            (StatusEffectKind::DamageReduction, 0.25, 120),
            (StatusEffectKind::StaminaRecovBoost, 0.25, 120),
            (StatusEffectKind::HealthRegenBoost, 0.25, 120),
            (StatusEffectKind::Bleeding, 0.1, 120),
            (StatusEffectKind::Stunned, 1.0, 15),
            (StatusEffectKind::Disoriented, 1.0, 30),
            (StatusEffectKind::Slowed, 0.35, 90),
            (StatusEffectKind::QiRegenPaused, 1.0, 60),
            (StatusEffectKind::DamageVulnerability, 0.25, 90),
            (StatusEffectKind::Exhausted, 0.5, 30),
            (StatusEffectKind::CultivationAcceleration, 0.5, 120),
        ]),
    },
    SceneDefinition {
        name: "test_gathering_herb_1",
        description: "手持铁锄采药：8 秒进度、自然完成，移动或受击中断",
        content: SceneContent::Gathering {
            target: GatheringTargetKind::Herb,
            target_name: "凝脉草",
        },
    },
    SceneDefinition {
        name: "test_gathering_ore_1",
        description: "手持铁镐采矿：8 秒进度、自然完成，移动或受击中断",
        content: SceneContent::Gathering {
            target: GatheringTargetKind::Ore,
            target_name: "凡铁矿",
        },
    },
    SceneDefinition {
        name: "test_gathering_wood_1",
        description: "手持铁斧伐木：8 秒进度、自然完成，移动或受击中断",
        content: SceneContent::Gathering {
            target: GatheringTargetKind::Wood,
            target_name: "粗木",
        },
    },
];

#[derive(Debug, Clone)]
pub enum SceneCmd {
    List,
    Clear,
    Run { name: String },
}

impl Command for SceneCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        let root = graph
            .root()
            .literal("scene")
            .with_executable(|_| Self::List)
            .id();
        graph
            .at(root)
            .literal("list")
            .with_executable(|_| Self::List);
        graph
            .at(root)
            .literal("clear")
            .with_executable(|_| Self::Clear);
        graph
            .at(root)
            .argument("name")
            .with_parser::<String>()
            .with_executable(|input| Self::Run {
                name: String::parse_arg(input).unwrap(),
            });
    }
}

pub(crate) struct TestSceneAccess;

impl Resource for TestSceneAccess {}

pub(super) fn register(app: &mut App, test_env: bool) {
    if !test_env {
        return;
    }
    app.insert_resource(TestSceneAccess)
        .init_resource::<gathering::GatheringSceneState>()
        .add_command::<SceneCmd>()
        .add_systems(
            Update,
            handle_scene
                .in_set(CombatSystemSet::Intent)
                .after(status::status_effect_apply_tick)
                .before(GatheringSystemSet::Produce)
                .before(emit_status_snapshot_payloads),
        )
        .add_systems(
            Update,
            gathering::cleanup_scenes
                .after(handle_scene)
                .before(GatheringSystemSet::Produce),
        )
        .add_systems(
            Update,
            gathering::sync_scene_tools
                .after(GatheringSystemSet::Emit)
                .after(crate::network::weapon_equipped_emit::emit_weapon_equipped_payloads),
        );
}

type ScenePlayer<'a> = (
    &'a Username,
    &'a mut Client,
    Option<&'a mut StatusEffects>,
    Option<&'a Lifecycle>,
    Option<&'a Position>,
);

fn handle_scene(
    mut events: EventReader<CommandResultEvent<SceneCmd>>,
    permissions: Res<DevCommandPermissions>,
    mut players: Query<ScenePlayer<'_>>,
    mut gathering: gathering::GatheringSceneContext<'_>,
) {
    for event in events.read() {
        let Ok((username, mut client, statuses, lifecycle, position)) =
            players.get_mut(event.executor)
        else {
            continue;
        };
        if !permissions.is_operator(&username.0) {
            client.send_chat_message("[scene] 需要管理员权限。");
            continue;
        }
        let scene = match &event.result {
            SceneCmd::List => {
                client.send_chat_message(
                    "[scene] 固定测试场景：切换会替换自身状态效果并结束上一段测试采集；/scene clear 清理。",
                );
                for scene in SCENES {
                    client.send_chat_message(format!("{} : {}", scene.name, scene.description));
                }
                continue;
            }
            SceneCmd::Clear => None,
            SceneCmd::Run { name } => {
                let Some(scene) = SCENES.iter().find(|scene| scene.name == name) else {
                    client.send_chat_message(format!(
                        "[scene] 未知场景 {name}，使用 /scene list 查看。"
                    ));
                    continue;
                };
                if !lifecycle.is_some_and(|lifecycle| lifecycle.state == LifecycleState::Alive) {
                    client.send_chat_message(
                        "[scene] 当前角色尚未就绪或正在等待复活，不能加载场景。",
                    );
                    continue;
                }
                Some(scene)
            }
        };
        let Some(mut statuses) = statuses else {
            client.send_chat_message("[scene] 当前角色的状态组件尚未就绪。");
            continue;
        };

        if let Some(SceneDefinition {
            name,
            content:
                SceneContent::Gathering {
                    target,
                    target_name,
                },
            ..
        }) = scene
        {
            let Some(position) = position else {
                client.send_chat_message("[scene] 当前角色的位置尚未就绪。");
                continue;
            };
            match gathering.start(event.executor, *target, target_name, position) {
                Ok(()) => {
                    statuses.active.clear();
                    client.send_chat_message(format!(
                        "[scene] 已加载 {name}：站定 8 秒完成，移动或受击中断；/scene clear 停止。仅演示采集反馈，不产出物品。"
                    ));
                }
                Err(message) => client.send_chat_message(format!("[scene] {message}")),
            }
            continue;
        }

        gathering.clear(event.executor);
        // 直接替换 dev 夹具，随后由正式系统执行属性聚合、倒计时和完整快照同步。
        statuses.active = match scene.map(|scene| &scene.content) {
            Some(SceneContent::StatusEffects(effects)) => effects
                .iter()
                .map(|(kind, magnitude, seconds)| ActiveStatusEffect {
                    kind: kind.clone(),
                    magnitude: *magnitude,
                    remaining_ticks: seconds * TICKS_PER_SECOND,
                    source_pill: None,
                })
                .collect(),
            _ => Vec::new(),
        };
        match scene {
            Some(scene) => client.send_chat_message(format!(
                "[scene] 已加载 {}：{}。效果按预设时长到期，可用 /scene clear 清空。",
                scene.name, scene.description
            )),
            None => client.send_chat_message("[scene] 已清空状态效果并结束当前角色的测试采集。"),
        }
    }
}

#[cfg(test)]
mod tests;
