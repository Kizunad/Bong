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
use valence::prelude::bevy_ecs::system::SystemParam;
use valence::prelude::{
    bevy_ecs, App, Client, Commands, EventReader, EventWriter, Position, Query, Res, Update,
    Username, With,
};

use crate::npc::faction::{
    legacy_faction_id_for_named_faction, EmergentGroupId, FactionRelationMatrix, FactionStatus,
    NamedFactionId, NamedFactionLeader, NamedFactionRegistry,
};
use crate::npc::movement::GameTick;
use crate::npc::war::{WarParticipateIntent, WarRole};
use crate::player::state::canonical_player_id;
use crate::social::components::{FactionMembership, FactionReputation};
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::zone::ZoneRegistry;

/// 玩家在线参与涌现冲突的 brigadier 命令。
///
/// - `/faction join <group>`     → Enlist（投靠某匿名群体，兼容旧数字路径）
/// - `/faction join <named_id>`  → 挂靠具名散修势力（plan-faction-expansion-v1 P3）
/// - `/faction mercenary <group>` → Mercenary（临时佣兵）
/// - `/faction intercept`         → Intercept（截胡，双方都打）
/// - `/faction list`              → 显示三势力关系矩阵（plan-faction-expansion-v1 P1）
///
/// Spectate 为默认态，无需命令；Settling/Aftermath 阶段 server 侧在
/// `handle_war_participate_intent` 中拒绝并打 debug log。
#[derive(Debug, Clone, PartialEq)]
pub enum FactionCmd {
    Join {
        target: String,
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
            .argument("target")
            .with_parser::<String>()
            .with_executable(|input| FactionCmd::Join {
                target: String::parse_arg(input).unwrap(),
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

type FactionCommandClientItem<'a> = (
    &'a Username,
    &'a Position,
    Option<&'a CurrentDimension>,
    Option<&'a FactionReputation>,
);

type FactionCommandClientQuery<'w, 's> =
    Query<'w, 's, FactionCommandClientItem<'static>, With<Client>>;

#[derive(SystemParam)]
pub struct FactionWarCommandState<'w, 's> {
    clients: FactionCommandClientQuery<'w, 's>,
    zone_registry: Option<Res<'w, ZoneRegistry>>,
    registry: Option<Res<'w, NamedFactionRegistry>>,
    game_tick: Option<Res<'w, GameTick>>,
    client_q: Query<'w, 's, &'static mut Client>,
}

/// brigadier handler：按当前坐标查 zone，组装并发送 `WarParticipateIntent`。
pub fn handle_faction_war_cmd(
    mut events: EventReader<CommandResultEvent<FactionCmd>>,
    mut commands: Commands,
    mut intents: EventWriter<WarParticipateIntent>,
    mut state: FactionWarCommandState,
) {
    let at_tick = state
        .game_tick
        .as_deref()
        .map(|t| u64::from(t.0))
        .unwrap_or_default();

    let zone_registry = state.zone_registry.as_deref();
    let registry = state.registry.as_deref();

    for event in events.read() {
        let Ok((username, position, maybe_dim, reputation)) = state.clients.get(event.executor)
        else {
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
            FactionCmd::Join { target } => {
                if let Ok(group) = target.parse::<u32>() {
                    (
                        WarRole::Enlist,
                        Some(EmergentGroupId(u16::try_from(group).unwrap_or(u16::MAX))),
                    )
                } else {
                    handle_named_faction_join(
                        event.executor,
                        target,
                        registry,
                        reputation,
                        &mut commands,
                        &mut state.client_q,
                    );
                    continue;
                }
            }
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
        if let Ok(mut client) = state.client_q.get_mut(event.executor) {
            client.send_chat_message("[faction] 参与意图已发送，等待处理……");
        }
    }
}

fn handle_named_faction_join(
    player: valence::prelude::Entity,
    target: &str,
    registry: Option<&NamedFactionRegistry>,
    reputation: Option<&FactionReputation>,
    commands: &mut Commands,
    client_q: &mut Query<&mut Client>,
) {
    let Some(named_faction) = parse_named_faction_target(target) else {
        if let Ok(mut client) = client_q.get_mut(player) {
            client.send_chat_message(format!(
                "[faction] 未知具名势力 `{target}`；可用：qingyun_hunters / cangyuan_merchants / north_waste_drifters"
            ));
        }
        return;
    };

    let Some(faction_entry) = registry.and_then(|registry| registry.get(named_faction)) else {
        if let Ok(mut client) = client_q.get_mut(player) {
            client.send_chat_message("[faction] 具名势力注册表尚未初始化。");
        }
        return;
    };

    if faction_entry.status == FactionStatus::Decayed {
        if let Ok(mut client) = client_q.get_mut(player) {
            client.send_chat_message(format!(
                "[faction] {} 已经消亡，不能再挂靠。",
                faction_entry.display_name
            ));
        }
        return;
    }

    let score = reputation
        .map(|reputation| reputation.score(named_faction))
        .unwrap_or_default();
    if score < 0 {
        if let Ok(mut client) = client_q.get_mut(player) {
            client.send_chat_message(format!(
                "[faction] {} 对你的信誉为 {score}，低于中性，拒绝挂靠。",
                faction_entry.display_name
            ));
        }
        return;
    }

    commands.entity(player).insert(FactionMembership {
        faction: legacy_faction_id_for_named_faction(named_faction),
        named_faction: Some(named_faction),
        rank: 0,
        loyalty: score.max(10),
        betrayal_count: 0,
        invite_block_until_tick: None,
        permanently_refused: false,
    });
    if let Ok(mut client) = client_q.get_mut(player) {
        client.send_chat_message(format!("[faction] 已挂靠 {}。", faction_entry.display_name));
    }
}

fn parse_named_faction_target(value: &str) -> Option<NamedFactionId> {
    NamedFactionId::from_str_name(value).or_else(|| {
        NamedFactionId::all()
            .into_iter()
            .find(|faction| faction.display_name() == value)
    })
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
    use crate::npc::faction::FactionId;
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
        send(
            &mut app,
            player,
            FactionCmd::Join {
                target: "1".to_string(),
            },
        );
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
    fn named_faction_join_inserts_membership_when_reputation_is_neutral() {
        let mut app = setup_app();
        app.insert_resource(NamedFactionRegistry::startup_default());
        let player = spawn_test_client(&mut app, "Alice", [0.0, 64.0, 0.0]);
        app.world_mut()
            .entity_mut(player)
            .insert(FactionReputation::default());
        send(
            &mut app,
            player,
            FactionCmd::Join {
                target: "qingyun_hunters".to_string(),
            },
        );
        run_update(&mut app);

        assert!(
            drain_war_intents(&app).is_empty(),
            "具名势力 join 不应复用匿名 WarParticipateIntent 路径"
        );
        let membership = app.world().get::<FactionMembership>(player).unwrap();
        assert_eq!(
            membership.named_faction,
            Some(NamedFactionId::QingyunHunters)
        );
        assert_eq!(membership.faction, FactionId::Attack);
    }

    #[test]
    fn named_faction_join_rejects_negative_reputation() {
        let mut app = setup_app();
        app.insert_resource(NamedFactionRegistry::startup_default());
        let player = spawn_test_client(&mut app, "Alice", [0.0, 64.0, 0.0]);
        let mut reputation = FactionReputation::default();
        reputation.apply_delta(NamedFactionId::QingyunHunters, -1);
        app.world_mut().entity_mut(player).insert(reputation);
        send(
            &mut app,
            player,
            FactionCmd::Join {
                target: "qingyun_hunters".to_string(),
            },
        );
        run_update(&mut app);

        assert!(
            app.world().get::<FactionMembership>(player).is_none(),
            "低于中性信誉时不能挂靠具名势力"
        );
    }

    #[test]
    fn named_faction_join_rejects_decayed_faction() {
        let mut registry = NamedFactionRegistry::startup_default();
        registry
            .get_mut(NamedFactionId::QingyunHunters)
            .unwrap()
            .set_status(FactionStatus::Decayed);

        let mut app = setup_app();
        app.insert_resource(registry);
        let player = spawn_test_client(&mut app, "Alice", [0.0, 64.0, 0.0]);
        app.world_mut()
            .entity_mut(player)
            .insert(FactionReputation::default());
        send(
            &mut app,
            player,
            FactionCmd::Join {
                target: "qingyun_hunters".to_string(),
            },
        );
        run_update(&mut app);

        assert!(
            app.world().get::<FactionMembership>(player).is_none(),
            "Decayed 势力不能再被玩家挂靠"
        );
    }

    #[test]
    fn named_faction_join_rejects_unknown_target() {
        let mut app = setup_app();
        app.insert_resource(NamedFactionRegistry::startup_default());
        let player = spawn_test_client(&mut app, "Alice", [0.0, 64.0, 0.0]);

        send(
            &mut app,
            player,
            FactionCmd::Join {
                target: "missing_faction".to_string(),
            },
        );
        run_update(&mut app);

        assert!(
            app.world().get::<FactionMembership>(player).is_none(),
            "未知具名势力必须拒绝挂靠，避免写入错误 membership"
        );
    }

    #[test]
    fn named_faction_join_accepts_headless_faction() {
        let mut app = setup_app();
        app.insert_resource(NamedFactionRegistry::startup_default());
        let player = spawn_test_client(&mut app, "Alice", [0.0, 64.0, 0.0]);

        send(
            &mut app,
            player,
            FactionCmd::Join {
                target: "north_waste_drifters".to_string(),
            },
        );
        run_update(&mut app);

        let membership = app.world().get::<FactionMembership>(player).unwrap();
        assert_eq!(
            membership.named_faction,
            Some(NamedFactionId::NorthWasteDrifters),
            "Headless 不是 Decayed，应允许中性玩家挂靠"
        );
        assert_eq!(
            membership.faction,
            FactionId::Neutral,
            "NorthWasteDrifters legacy faction must map to Neutral"
        );
    }

    #[test]
    fn named_faction_join_accepts_missing_reputation_component_as_neutral() {
        let mut app = setup_app();
        app.insert_resource(NamedFactionRegistry::startup_default());
        let player = spawn_test_client(&mut app, "Alice", [0.0, 64.0, 0.0]);

        send(
            &mut app,
            player,
            FactionCmd::Join {
                target: "qingyun_hunters".to_string(),
            },
        );
        run_update(&mut app);

        let membership = app.world().get::<FactionMembership>(player).unwrap();
        assert_eq!(
            membership.named_faction,
            Some(NamedFactionId::QingyunHunters),
            "缺 FactionReputation 组件时 score 应按 0 处理中性准入"
        );
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
