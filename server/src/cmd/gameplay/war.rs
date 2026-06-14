//! plan-offscreen-war-v1 P6：`/faction join|mercenary|intercept` — 真玩家在线参与
//! 涌现区域冲突的 brigadier 入口。
//!
//! plan-faction-expansion-v1 P1 扩展：`/faction list` — 显示三势力关系矩阵到聊天栏。
//!
//! **路径 A**（brigadier）：Client 按当前坐标查 zone → 组装 `WarParticipateIntent` → 发
//! 到 `handle_war_participate_intent` system 统一处理。与路径 B（headless FactionEvent
//! 注入）汇聚同一 intent 队列，行为完全一致。
//!
//! reframe b 硬约束：无具名宗门，玩家投靠的是匿名区域群体（裸 EmergentGroupId）。
use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::parsers::CommandArg;
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{
    App, Client, EventReader, EventWriter, Position, Query, Res, Update, Username, With,
};

use crate::npc::faction::{
    EmergentGroupId, FactionRelationMatrix, NamedFactionId, NamedFactionLeader,
    NamedFactionRegistry,
};
use crate::npc::movement::GameTick;
use crate::npc::war::{WarParticipateIntent, WarRole};
use crate::player::state::canonical_player_id;
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::zone::ZoneRegistry;

/// 玩家在线参与涌现冲突的 brigadier 命令。
///
/// - `/faction join <group>`     → Enlist（投靠某匿名群体）
/// - `/faction mercenary <group>` → Mercenary（临时佣兵）
/// - `/faction intercept`         → Intercept（截胡，双方都打）
/// - `/faction list`              → 显示三势力关系矩阵（plan-faction-expansion-v1 P1）
///
/// Spectate 为默认态，无需命令；Settling/Aftermath 阶段 server 侧在
/// `handle_war_participate_intent` 中拒绝并打 debug log。
#[derive(Debug, Clone, PartialEq)]
pub enum FactionCmd {
    Join {
        group: u32,
    },
    Mercenary {
        group: u32,
    },
    Intercept,
    /// plan-faction-expansion-v1 P1：显示三势力关系矩阵到聊天栏。
    List,
}

impl Command for FactionCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        let faction = graph.root().literal("faction").id();

        graph
            .at(faction)
            .literal("join")
            .argument("group")
            .with_parser::<u32>()
            .with_executable(|input| FactionCmd::Join {
                group: u32::parse_arg(input).unwrap(),
            });

        graph
            .at(faction)
            .literal("mercenary")
            .argument("group")
            .with_parser::<u32>()
            .with_executable(|input| FactionCmd::Mercenary {
                group: u32::parse_arg(input).unwrap(),
            });

        graph
            .at(faction)
            .literal("intercept")
            .with_executable(|_| FactionCmd::Intercept);

        // plan-faction-expansion-v1 P1。
        graph
            .at(faction)
            .literal("list")
            .with_executable(|_| FactionCmd::List);
    }
}

pub fn register(app: &mut App) {
    app.add_command::<FactionCmd>()
        .add_systems(Update, handle_faction_war_cmd)
        // plan-faction-expansion-v1 P1：/faction list handler。
        .add_systems(Update, handle_faction_list_cmd);
}

/// brigadier handler：按当前坐标查 zone，组装并发送 `WarParticipateIntent`。
pub fn handle_faction_war_cmd(
    mut events: EventReader<CommandResultEvent<FactionCmd>>,
    clients: Query<(&Username, &Position, Option<&CurrentDimension>), With<Client>>,
    zone_registry: Option<Res<ZoneRegistry>>,
    mut intents: EventWriter<WarParticipateIntent>,
    game_tick: Option<Res<GameTick>>,
    mut client_q: Query<&mut Client>,
) {
    let at_tick = game_tick
        .as_deref()
        .map(|t| u64::from(t.0))
        .unwrap_or_default();

    let zone_registry = zone_registry.as_deref();

    for event in events.read() {
        let Ok((username, position, maybe_dim)) = clients.get(event.executor) else {
            continue;
        };

        let dim = maybe_dim.map(|d| d.0).unwrap_or(DimensionKind::Overworld);

        // 查当前 zone
        let zone = if let Some(reg) = zone_registry {
            reg.find_zone(dim, position.get())
                .map(|z| z.name.clone())
                .unwrap_or_else(|| "unknown".to_string())
        } else {
            "unknown".to_string()
        };

        let player_id = canonical_player_id(username.0.as_str());

        // plan-faction-expansion-v1 P1：/faction list 由 handle_faction_list_cmd 处理，
        // 不发 WarParticipateIntent，这里 skip。
        if event.result == FactionCmd::List {
            continue;
        }

        let (role, allied_group) = match &event.result {
            FactionCmd::Join { group } => (
                WarRole::Enlist,
                Some(EmergentGroupId(u16::try_from(*group).unwrap_or(u16::MAX))),
            ),
            FactionCmd::Mercenary { group } => (
                WarRole::Mercenary,
                Some(EmergentGroupId(u16::try_from(*group).unwrap_or(u16::MAX))),
            ),
            FactionCmd::Intercept => (WarRole::Intercept, None),
            // List is handled above with `continue`; this arm is unreachable but satisfies exhaustiveness.
            FactionCmd::List => continue,
        };

        intents.send(WarParticipateIntent {
            player_id,
            zone,
            role,
            allied_group,
            at_tick,
        });

        // 向玩家回一条提示（具体结果由 telemetry 反映；intent 可能在下一帧被拒绝）
        if let Ok(mut client) = client_q.get_mut(event.executor) {
            client.send_chat_message("[faction] 参与意图已发送，等待处理……");
        }
    }
}

/// plan-faction-expansion-v1 P1：`/faction list` brigadier handler。
///
/// 读 `NamedFactionRegistry` + `FactionRelationMatrix`，格式化输出三势力名称 +
/// 当前关系矩阵到聊天栏。输出格式（中文，对齐正典势力名称）：
/// ```text
/// [势力] 三势力关系矩阵：
///   青云猎盟 ↔ 沧渊商会: 中立
///   青云猎盟 ↔ 北荒漂流者: 敌对
///   沧渊商会 ↔ 北荒漂流者: 中立
/// ```
pub fn handle_faction_list_cmd(
    mut events: EventReader<CommandResultEvent<FactionCmd>>,
    registry: Option<Res<NamedFactionRegistry>>,
    relation_matrix: Option<Res<FactionRelationMatrix>>,
    leaders: Query<&NamedFactionLeader>,
    mut client_q: Query<&mut Client>,
) {
    for event in events.read() {
        if event.result != FactionCmd::List {
            continue;
        }
        let Ok(mut client) = client_q.get_mut(event.executor) else {
            continue;
        };

        let Some(registry) = registry.as_deref() else {
            client.send_chat_message("[势力] 注册表尚未初始化。");
            continue;
        };
        let Some(matrix) = relation_matrix.as_deref() else {
            client.send_chat_message("[势力] 关系矩阵尚未初始化。");
            continue;
        };

        client.send_chat_message("[势力] 三势力关系矩阵：");
        for faction in registry.iter() {
            let leader_alive = leaders.iter().any(|leader| leader.faction == faction.id);
            let leader_status = if leader_alive {
                "领袖存活"
            } else if faction.status == crate::npc::faction::FactionStatus::Headless {
                "无头"
            } else {
                "未刷新"
            };
            client.send_chat_message(format!(
                "  {} [{}] zone={} npc={} {}",
                faction.display_name,
                faction.status.as_str(),
                faction.zone_anchor,
                faction.current_npc_count,
                leader_status
            ));
        }

        // 三对关系（按 NamedFactionId::all() 顺序遍历所有唯一对）。
        let all = NamedFactionId::all();
        for i in 0..all.len() {
            for j in (i + 1)..all.len() {
                let a = all[i];
                let b = all[j];
                let a_name = registry
                    .get(a)
                    .map(|f| f.display_name.as_str())
                    .unwrap_or(a.display_name());
                let b_name = registry
                    .get(b)
                    .map(|f| f.display_name.as_str())
                    .unwrap_or(b.display_name());
                let relation = matrix.get(a, b);
                let relation_str = match relation {
                    crate::npc::faction::FactionRelation::Hostile => "敌对",
                    crate::npc::faction::FactionRelation::Neutral => "中立",
                    crate::npc::faction::FactionRelation::Pact => "盟约",
                };
                client.send_chat_message(format!("  {a_name} ↔ {b_name}: {relation_str}"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::dev::test_support::{run_update, spawn_test_client};
    use crate::npc::war::{WarConflictStore, WarParticipateIntent, WarRole, ZoneConflictPressure};
    use valence::prelude::{App, Events, Update};

    fn setup_app() -> App {
        let mut app = App::new();
        app.add_event::<CommandResultEvent<FactionCmd>>();
        app.add_event::<WarParticipateIntent>();
        app.init_resource::<WarConflictStore>();
        app.init_resource::<ZoneConflictPressure>();
        app.add_systems(Update, handle_faction_war_cmd);
        app
    }

    fn send(app: &mut App, executor: valence::prelude::Entity, result: FactionCmd) {
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<FactionCmd>>>()
            .send(CommandResultEvent {
                result,
                executor,
                modifiers: Default::default(),
            });
    }

    fn drain_war_intents(app: &App) -> Vec<WarParticipateIntent> {
        app.world()
            .resource::<Events<WarParticipateIntent>>()
            .iter_current_update_events()
            .cloned()
            .collect()
    }

    #[test]
    fn faction_join_emits_enlist_intent() {
        let mut app = setup_app();
        let player = spawn_test_client(&mut app, "Alice", [0.0, 64.0, 0.0]);
        send(&mut app, player, FactionCmd::Join { group: 1 });
        run_update(&mut app);

        let intents = drain_war_intents(&app);
        assert_eq!(
            intents.len(),
            1,
            "期望 Join 命令发出 1 条 WarParticipateIntent，实际 {}",
            intents.len()
        );
        let intent = &intents[0];
        assert_eq!(
            intent.role,
            WarRole::Enlist,
            "期望 role=Enlist，实际 {:?}",
            intent.role
        );
        assert_eq!(
            intent.allied_group,
            Some(EmergentGroupId(1)),
            "期望 allied_group=Some(1)，实际 {:?}",
            intent.allied_group
        );
        assert_eq!(intent.player_id, "offline:Alice");
    }

    #[test]
    fn faction_mercenary_emits_mercenary_intent() {
        let mut app = setup_app();
        let player = spawn_test_client(&mut app, "Bob", [0.0, 64.0, 0.0]);
        send(&mut app, player, FactionCmd::Mercenary { group: 2 });
        run_update(&mut app);

        let intents = drain_war_intents(&app);
        assert_eq!(intents.len(), 1);
        let intent = &intents[0];
        assert_eq!(
            intent.role,
            WarRole::Mercenary,
            "期望 role=Mercenary，实际 {:?}",
            intent.role
        );
        assert_eq!(intent.allied_group, Some(EmergentGroupId(2)));
    }

    #[test]
    fn faction_intercept_emits_intercept_intent_no_group() {
        let mut app = setup_app();
        let player = spawn_test_client(&mut app, "Carol", [0.0, 64.0, 0.0]);
        send(&mut app, player, FactionCmd::Intercept);
        run_update(&mut app);

        let intents = drain_war_intents(&app);
        assert_eq!(intents.len(), 1);
        let intent = &intents[0];
        assert_eq!(
            intent.role,
            WarRole::Intercept,
            "期望 role=Intercept，实际 {:?}",
            intent.role
        );
        assert_eq!(
            intent.allied_group, None,
            "期望 Intercept 的 allied_group=None，实际 {:?}",
            intent.allied_group
        );
    }

    #[test]
    fn invalid_executor_does_not_panic() {
        // executor entity 不存在于 clients query 时应静默 continue，不 panic
        let mut app = setup_app();
        let dummy = app.world_mut().spawn_empty().id();
        send(&mut app, dummy, FactionCmd::Intercept);
        run_update(&mut app); // 不应 panic

        let intents = drain_war_intents(&app);
        assert_eq!(intents.len(), 0, "无效 executor 不应发出 intent");
    }
}
