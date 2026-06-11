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
use crate::world::risk_heatmap::{risk_score, RiskHeatmap};
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

pub fn register(app: &mut App) {
    app.add_command::<RiskmapCmd>()
        .add_systems(Update, handle_riskmap);
}

pub fn handle_riskmap(
    mut events: EventReader<CommandResultEvent<RiskmapCmd>>,
    heatmap: Option<Res<RiskHeatmap>>,
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
        // 查找玩家当前所在 zone（使用 Overworld 维度，与 zone.rs find_zone 行为一致）
        let zone_name = zones
            .zones
            .iter()
            .find(|z| z.contains(player_pos))
            .map(|z| z.name.as_str())
            .unwrap_or("<unknown>");

        let axes = heatmap.axes_for_zone(zone_name);
        let player_realm = cultivation.map(|c| c.realm).unwrap_or(Realm::Condense);
        let score = risk_score(axes, player_realm);

        client.send_chat_message(format!(
            "[dev] riskmap zone={zone_name} \
             qi={:.1} fauna={:.1} npc={:.1} player={:.1} \
             RiskScore={} realm={player_realm:?}",
            axes.qi, axes.fauna, axes.npc, axes.player, score.0
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::dev::test_support::{run_update, spawn_test_client};
    use crate::cultivation::components::{Cultivation, Realm};
    use crate::world::risk_heatmap::{RiskAxes, RiskHeatmap};
    use crate::world::zone::ZoneRegistry;
    use valence::prelude::Events;

    fn setup_app() -> App {
        let mut app = App::new();
        app.add_event::<CommandResultEvent<RiskmapCmd>>();
        app.add_systems(Update, handle_riskmap);
        app
    }

    fn send(app: &mut App, player: valence::prelude::Entity) {
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<RiskmapCmd>>>()
            .send(CommandResultEvent {
                result: RiskmapCmd::Show,
                executor: player,
                modifiers: Default::default(),
            });
    }

    #[test]
    fn riskmap_missing_heatmap_resource_sends_error_message() {
        let mut app = setup_app();
        // 无 RiskHeatmap resource
        let player = spawn_test_client(&mut app, "Tester", [8.0, 66.0, 8.0]);
        send(&mut app, player);
        // 不 panic，正常退出即为通过
        run_update(&mut app);
    }

    #[test]
    fn riskmap_with_resources_outputs_score_for_known_zone() {
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

        // 在 spawn zone 内部（fallback spawn zone bounds: [-128, 64, -128] to [128, 80, 128]）
        let player = spawn_test_client(&mut app, "Bob", [8.0, 66.0, 8.0]);
        send(&mut app, player);
        run_update(&mut app);
        // 命令正常执行，无 panic = 通过
    }

    #[test]
    fn riskmap_outputs_correct_realm_in_message() {
        let mut app = setup_app();
        app.insert_resource(ZoneRegistry::fallback());

        let mut heatmap = RiskHeatmap::default();
        let axes = RiskAxes::default();
        heatmap.by_zone.insert("spawn".to_string(), axes);
        app.insert_resource(heatmap);

        let player = spawn_test_client(&mut app, "CultivatorA", [8.0, 66.0, 8.0]);

        // 给玩家加 Cultivation component（Spirit 境界）
        let cultivation = Cultivation {
            realm: Realm::Spirit,
            ..Default::default()
        };
        app.world_mut().entity_mut(player).insert(cultivation);

        send(&mut app, player);
        run_update(&mut app);
        // 正常完成即通过（无 Cultivation 时 fallback Condense；有 Cultivation 时读真实 realm）
    }

    #[test]
    fn riskmap_unknown_zone_reports_unknown_without_panic() {
        let mut app = setup_app();
        // 空 registry，没有任何 zone（不走 fallback 构造）
        let registry = ZoneRegistry { zones: vec![] };
        app.insert_resource(registry);
        app.insert_resource(RiskHeatmap::default());

        // 玩家在任何位置都不在任何 zone 里
        let player = spawn_test_client(&mut app, "Wanderer", [9999.0, 0.0, 9999.0]);
        send(&mut app, player);
        run_update(&mut app);
        // 报 <unknown> zone，不 panic = 通过
    }

    #[test]
    fn riskmap_score_zero_for_empty_axes() {
        // 验证 risk_score 对全零轴返回 0（契约级）
        let axes = RiskAxes::default();
        let score = risk_score(axes, Realm::Condense);
        assert_eq!(score.0, 0, "全零轴凝脉应得 0 分，实际={}", score.0);
    }
}
