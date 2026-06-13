//! plan-sou-da-che-v1 P0 — `/riskmap` GM 调试命令。
//!
//! 输出执行者当前所在 zone 的四轴风险数据（灵气/野兽/NPC/玩家分量 + 总 RiskScore）。
//! 使用 `RiskHeatmap` Resource 读取已计算好的四轴数据，避免在命令处理中重新遍历 ECS。

use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{App, Client, EventReader, Position, Query, Res, Update};

use crate::cultivation::components::{Cultivation, Realm};
use crate::world::dimension::DimensionKind;
use crate::world::risk_heatmap::{risk_score, RiskHeatmap};
use crate::world::risk_signals::{RiskSignalMap, RiskSignalProfile};
use crate::world::zone::ZoneRegistry;

#[derive(Debug, Clone, PartialEq)]
pub enum RiskmapCmd {
    Show,
}

impl Command for RiskmapCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        graph
            .root()
            .literal("riskmap")
            .with_executable(|_input| RiskmapCmd::Show);
    }
}

/// 将 zone 名、四轴分量、RiskScore、境界格式化为 `/riskmap` 输出行。
///
/// 格式示例：
/// `[dev] riskmap zone=spawn qi=15.0 fauna=10.0 npc=8.0 player=4.0 RiskScore=37 realm=Spirit`
///
/// 抽为纯函数供单元测试断言可观察契约。
pub fn format_riskmap_line(
    zone_name: &str,
    axes: crate::world::risk_heatmap::RiskAxes,
    score: crate::world::risk_heatmap::RiskScore,
    realm: crate::cultivation::components::Realm,
    signals: Option<&RiskSignalProfile>,
) -> String {
    let mut line = format!(
        "[dev] riskmap zone={zone_name} \
         qi={:.1} fauna={:.1} npc={:.1} player={:.1} \
         RiskScore={} realm={realm:?}",
        axes.qi, axes.fauna, axes.npc, axes.player, score.0
    );
    if let Some(signals) = signals {
        line.push_str(" signals={");
        line.push_str(&signals.summary_tokens());
        line.push('}');
    }
    line
}

pub fn register(app: &mut App) {
    app.add_command::<RiskmapCmd>()
        .add_systems(Update, handle_riskmap);
}

pub fn handle_riskmap(
    mut events: EventReader<CommandResultEvent<RiskmapCmd>>,
    heatmap: Option<Res<RiskHeatmap>>,
    signals: Option<Res<RiskSignalMap>>,
    zones: Option<Res<ZoneRegistry>>,
    mut clients: Query<(&mut Client, &Position, Option<&Cultivation>)>,
) {
    let Some(heatmap) = heatmap else {
        for event in events.read() {
            if let Ok((mut client, _, _)) = clients.get_mut(event.executor) {
                client.send_chat_message(
                    "[dev] riskmap failed: RiskHeatmap resource not initialized",
                );
            }
        }
        return;
    };

    let Some(zones) = zones else {
        for event in events.read() {
            if let Ok((mut client, _, _)) = clients.get_mut(event.executor) {
                client.send_chat_message(
                    "[dev] riskmap failed: ZoneRegistry resource not initialized",
                );
            }
        }
        return;
    };

    for event in events.read() {
        let Ok((mut client, pos, cultivation)) = clients.get_mut(event.executor) else {
            continue;
        };

        let player_pos = pos.0;
        // 使用 ZoneRegistry::find_zone(Overworld, pos) 取最小 AABB zone，
        // 与 update_risk_heatmap 系统的归属逻辑完全一致。
        let zone_name = zones
            .find_zone(DimensionKind::Overworld, player_pos)
            .map(|z| z.name.as_str())
            .unwrap_or("<unknown>");

        let axes = heatmap.axes_for_zone(zone_name);
        let player_realm = cultivation.map(|c| c.realm).unwrap_or(Realm::Condense);
        let score = risk_score(axes, player_realm);
        let signal_profile = signals
            .as_ref()
            .and_then(|signals| signals.profile_for_zone(zone_name));

        client.send_chat_message(format_riskmap_line(
            zone_name,
            axes,
            score,
            player_realm,
            signal_profile,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::{Cultivation, Realm};
    use crate::world::risk_heatmap::{RiskAxes, RiskHeatmap, RiskScore};
    use crate::world::zone::ZoneRegistry;
    use valence::prelude::{App, Events, Position, Update};
    use valence::protocol::packets::play::GameMessageS2c;
    use valence::testing::create_mock_client;

    fn setup_app() -> App {
        let mut app = App::new();
        app.add_event::<CommandResultEvent<RiskmapCmd>>();
        app.add_systems(Update, handle_riskmap);
        app
    }

    fn send_cmd(app: &mut App, player: valence::prelude::Entity) {
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<RiskmapCmd>>>()
            .send(CommandResultEvent {
                result: RiskmapCmd::Show,
                executor: player,
                modifiers: Default::default(),
            });
    }

    /// 创建测试客户端并保留 helper（用于捕获发出的数据包）。
    fn spawn_client_with_helper(
        app: &mut App,
        username: &str,
        position: [f64; 3],
    ) -> (valence::prelude::Entity, valence::testing::MockClientHelper) {
        let (mut bundle, helper) = create_mock_client(username);
        bundle.player.position = Position::new(position);
        let entity = app.world_mut().spawn(bundle).id();
        (entity, helper)
    }

    /// 刷新所有客户端的 ECS 发包缓冲，使数据包进入 MockClientHelper 可读取队列。
    fn flush_packets(app: &mut App) {
        let world = app.world_mut();
        let mut query = world.query::<&mut valence::client::Client>();
        for mut client in query.iter_mut(world) {
            client
                .flush_packets()
                .expect("mock client flush should succeed");
        }
    }

    /// 从 helper 收集所有已发给客户端的 GameMessage 文本。
    fn collect_messages(helper: &mut valence::testing::MockClientHelper) -> Vec<String> {
        helper
            .collect_received()
            .0
            .into_iter()
            .filter_map(|frame| {
                frame
                    .decode::<GameMessageS2c>()
                    .ok()
                    .map(|pkt| pkt.chat.to_legacy_lossy())
            })
            .collect()
    }

    // ── format_riskmap_line 纯函数契约测试 ────────────────────────────────────

    #[test]
    fn format_riskmap_line_contains_zone_name() {
        let axes = RiskAxes {
            qi: 15.0,
            fauna: 10.0,
            npc: 8.0,
            player: 4.0,
        };
        let score = RiskScore(37);
        let line = format_riskmap_line("qingyun_peaks", axes, score, Realm::Spirit, None);
        assert!(
            line.contains("zone=qingyun_peaks"),
            "输出行必须含 zone 名称，实际=`{line}`"
        );
    }

    #[test]
    fn format_riskmap_line_contains_all_four_axes() {
        let axes = RiskAxes {
            qi: 15.0,
            fauna: 10.0,
            npc: 8.0,
            player: 4.0,
        };
        let score = RiskScore(37);
        let line = format_riskmap_line("spawn", axes, score, Realm::Condense, None);
        assert!(
            line.contains("qi=15.0"),
            "输出行必须含 qi 分量，实际=`{line}`"
        );
        assert!(
            line.contains("fauna=10.0"),
            "输出行必须含 fauna 分量，实际=`{line}`"
        );
        assert!(
            line.contains("npc=8.0"),
            "输出行必须含 npc 分量，实际=`{line}`"
        );
        assert!(
            line.contains("player=4.0"),
            "输出行必须含 player 分量，实际=`{line}`"
        );
    }

    #[test]
    fn format_riskmap_line_contains_risk_score() {
        let axes = RiskAxes {
            qi: 15.0,
            fauna: 10.0,
            npc: 8.0,
            player: 4.0,
        };
        let score = RiskScore(37);
        let line = format_riskmap_line("spawn", axes, score, Realm::Condense, None);
        assert!(
            line.contains("RiskScore=37"),
            "输出行必须含 RiskScore 值，实际=`{line}`"
        );
    }

    #[test]
    fn format_riskmap_line_contains_realm() {
        let axes = RiskAxes::default();
        let score = RiskScore(0);
        let spirit_line = format_riskmap_line("spawn", axes, score, Realm::Spirit, None);
        let induce_line = format_riskmap_line("spawn", axes, score, Realm::Induce, None);
        assert!(
            spirit_line.contains("realm=Spirit"),
            "通灵输出行必须含 realm=Spirit，实际=`{spirit_line}`"
        );
        assert!(
            induce_line.contains("realm=Induce"),
            "引气输出行必须含 realm=Induce，实际=`{induce_line}`"
        );
    }

    #[test]
    fn format_riskmap_line_unknown_zone() {
        // <unknown> zone 时格式应正确，不含 zone= 的真实名称
        let line = format_riskmap_line(
            "<unknown>",
            RiskAxes::default(),
            RiskScore(0),
            Realm::Condense,
            None,
        );
        assert!(
            line.contains("zone=<unknown>"),
            "未知 zone 输出行必须含 zone=<unknown>，实际=`{line}`"
        );
    }

    // ── handler ECS 测试：错误路径断言可观察消息 ──────────────────────────────

    #[test]
    fn riskmap_missing_heatmap_resource_sends_error_message() {
        // 错误路径①：RiskHeatmap resource 缺失 → 客户端收到错误文本
        let mut app = setup_app();
        // 无 RiskHeatmap、无 ZoneRegistry
        let (player, mut helper) = spawn_client_with_helper(&mut app, "Tester", [8.0, 66.0, 8.0]);
        send_cmd(&mut app, player);
        app.update();
        flush_packets(&mut app);

        let messages = collect_messages(&mut helper);
        assert!(
            messages
                .iter()
                .any(|m| m.contains("RiskHeatmap resource not initialized")),
            "RiskHeatmap 缺失时客户端应收到错误消息，实际消息列表={messages:?}"
        );
    }

    #[test]
    fn riskmap_missing_zone_registry_sends_error_message() {
        // 错误路径②（CodeRabbit Major）：ZoneRegistry resource 缺失 → 客户端收到错误文本
        let mut app = setup_app();
        // 有 RiskHeatmap，但无 ZoneRegistry
        app.insert_resource(RiskHeatmap::default());
        let (player, mut helper) = spawn_client_with_helper(&mut app, "Tester2", [8.0, 66.0, 8.0]);
        send_cmd(&mut app, player);
        app.update();
        flush_packets(&mut app);

        let messages = collect_messages(&mut helper);
        assert!(
            messages
                .iter()
                .any(|m| m.contains("ZoneRegistry resource not initialized")),
            "ZoneRegistry 缺失时客户端应收到错误消息，实际消息列表={messages:?}"
        );
    }

    // ── handler ECS 测试：正常路径断言可观察消息内容 ─────────────────────────

    #[test]
    fn riskmap_normal_output_contains_zone_axes_score_realm() {
        // 正常路径：验证客户端实际收到含 zone 名/四轴/RiskScore/realm 的消息
        let mut app = setup_app();
        app.insert_resource(ZoneRegistry::fallback());

        let mut heatmap = RiskHeatmap::default();
        let axes = RiskAxes {
            qi: 15.0,
            fauna: 10.0,
            npc: 8.0,
            player: 4.0,
        };
        heatmap.by_zone.insert("spawn".to_string(), axes);
        app.insert_resource(heatmap);

        // fallback spawn zone: [-128, 64, -128] to [128, 80, 128]
        let (player, mut helper) = spawn_client_with_helper(&mut app, "Bob", [8.0, 66.0, 8.0]);
        send_cmd(&mut app, player);
        app.update();
        flush_packets(&mut app);

        let messages = collect_messages(&mut helper);
        let msg = messages
            .iter()
            .find(|m| m.contains("riskmap"))
            .cloned()
            .unwrap_or_else(|| panic!("期望找到含 riskmap 的消息，实际消息列表={messages:?}"));

        assert!(
            msg.contains("zone=spawn"),
            "消息应含 zone=spawn，实际=`{msg}`"
        );
        assert!(msg.contains("qi=15.0"), "消息应含 qi=15.0，实际=`{msg}`");
        assert!(
            msg.contains("fauna=10.0"),
            "消息应含 fauna=10.0，实际=`{msg}`"
        );
        assert!(msg.contains("npc=8.0"), "消息应含 npc=8.0，实际=`{msg}`");
        assert!(
            msg.contains("player=4.0"),
            "消息应含 player=4.0，实际=`{msg}`"
        );
        assert!(
            msg.contains("RiskScore="),
            "消息应含 RiskScore=，实际=`{msg}`"
        );
        assert!(
            msg.contains("realm=Condense"),
            "无 Cultivation 时 realm 应为 Condense，实际=`{msg}`"
        );
    }

    #[test]
    fn riskmap_normal_output_includes_p1_environment_signal_tokens_when_available() {
        let mut app = setup_app();
        app.insert_resource(ZoneRegistry::fallback());

        let mut heatmap = RiskHeatmap::default();
        heatmap.by_zone.insert(
            "spawn".to_string(),
            RiskAxes {
                qi: 30.0,
                fauna: 25.0,
                npc: 0.0,
                player: 0.0,
            },
        );
        app.insert_resource(heatmap);

        let mut signal_map = RiskSignalMap::default();
        signal_map.by_zone.insert(
            "spawn".to_string(),
            RiskSignalProfile {
                tier: crate::world::risk_signals::RiskSignalTier::Dangerous,
                flora: crate::world::risk_signals::FloraRiskSignal::LushBright,
                audio: crate::world::risk_signals::AudioRiskSignal::RatScreechAndBranches,
                particle: crate::world::risk_signals::ParticleRiskSignal::DarkSpores,
                npc_behavior: crate::world::risk_signals::NpcBehaviorRiskSignal::FaunaFleeLine,
            },
        );
        app.insert_resource(signal_map);

        let (player, mut helper) =
            spawn_client_with_helper(&mut app, "SignalTester", [8.0, 66.0, 8.0]);
        send_cmd(&mut app, player);
        app.update();
        flush_packets(&mut app);

        let messages = collect_messages(&mut helper);
        let msg = messages
            .iter()
            .find(|m| m.contains("riskmap"))
            .cloned()
            .unwrap_or_else(|| panic!("期望收到 riskmap 消息，实际={messages:?}"));

        assert!(
            msg.contains("signals={tier=dangerous"),
            "P1 signal summary 应随 riskmap 输出，实际=`{msg}`"
        );
        assert!(
            msg.contains("flora=lush_bright"),
            "P1 植被信号应可见，实际=`{msg}`"
        );
        assert!(
            msg.contains("particle=bong:risk_dark_spores"),
            "P1 粒子信号 event_id 应可见，实际=`{msg}`"
        );
        assert!(
            msg.contains("npc=fauna_flee_line"),
            "P1 NPC 行为信号应可见，实际=`{msg}`"
        );
    }

    #[test]
    fn riskmap_outputs_correct_realm_in_message() {
        // 有 Cultivation(Spirit) 时，消息 realm 字段应为 Spirit
        let mut app = setup_app();
        app.insert_resource(ZoneRegistry::fallback());

        let mut heatmap = RiskHeatmap::default();
        heatmap
            .by_zone
            .insert("spawn".to_string(), RiskAxes::default());
        app.insert_resource(heatmap);

        let (player, mut helper) =
            spawn_client_with_helper(&mut app, "CultivatorA", [8.0, 66.0, 8.0]);
        app.world_mut().entity_mut(player).insert(Cultivation {
            realm: Realm::Spirit,
            ..Default::default()
        });
        send_cmd(&mut app, player);
        app.update();
        flush_packets(&mut app);

        let messages = collect_messages(&mut helper);
        let msg = messages
            .iter()
            .find(|m| m.contains("riskmap"))
            .cloned()
            .unwrap_or_else(|| panic!("期望收到 riskmap 消息，实际={messages:?}"));

        assert!(
            msg.contains("realm=Spirit"),
            "Spirit 境界玩家的消息应含 realm=Spirit，实际=`{msg}`"
        );
    }

    #[test]
    fn riskmap_unknown_zone_reports_unknown_without_panic() {
        // 空 registry，玩家不在任何 zone → 消息含 zone=<unknown>
        let mut app = setup_app();
        let registry = ZoneRegistry { zones: vec![] };
        app.insert_resource(registry);
        app.insert_resource(RiskHeatmap::default());

        let (player, mut helper) =
            spawn_client_with_helper(&mut app, "Wanderer", [9999.0, 0.0, 9999.0]);
        send_cmd(&mut app, player);
        app.update();
        flush_packets(&mut app);

        let messages = collect_messages(&mut helper);
        let msg = messages
            .iter()
            .find(|m| m.contains("riskmap"))
            .cloned()
            .unwrap_or_else(|| panic!("期望收到 riskmap 消息，实际={messages:?}"));

        assert!(
            msg.contains("zone=<unknown>"),
            "无 zone 时消息应含 zone=<unknown>，实际=`{msg}`"
        );
    }

    #[test]
    fn riskmap_score_zero_for_empty_axes() {
        // 验证 risk_score 对全零轴返回 0（契约级）
        use crate::world::risk_heatmap::risk_score;
        let axes = RiskAxes::default();
        let score = risk_score(axes, Realm::Condense);
        assert_eq!(score.0, 0, "全零轴凝脉应得 0 分，实际={}", score.0);
    }
}
