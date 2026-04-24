//! plan-mineral-v1 §M6 — KarmaFlagIntent → GameEvent::MineralKarmaFlag bridge。
//!
//! 监听 `KarmaFlagIntent`，把品阶 ≥ 3 矿块的劫气标记转发给 agent_bridge
//! 的 GameEvent 通道。当前 agent 端仅订阅 stub；本桥保证 emit 链路就位，
//! 即便 agent 未消费也不报错（crossbeam send 失败仅日志告警）。

use valence::prelude::{EventReader, Query, Res};

use crate::network::agent_bridge::{GameEvent, NetworkBridgeResource};

use super::events::KarmaFlagIntent;

/// 玩家 entity → username 反查。当前 server 用 valence::client::Username 组件
/// 存玩家用户名。bridge 在 emit 时按 Entity 查名字；查不到则用 "<unknown>" 占位。
type ClientUsernameQuery<'world, 'state> =
    Query<'world, 'state, &'static valence::prelude::Username>;

pub fn forward_karma_flag_to_agent(
    mut karma: EventReader<KarmaFlagIntent>,
    bridge: Res<NetworkBridgeResource>,
    clients: ClientUsernameQuery,
) {
    for intent in karma.read() {
        let player_username = clients
            .get(intent.player)
            .map(|name| name.0.clone())
            .unwrap_or_else(|_| "<unknown>".to_string());

        let event = GameEvent::MineralKarmaFlag {
            player_username,
            mineral_id: intent.mineral_id.as_str().to_string(),
            position: [intent.position.x, intent.position.y, intent.position.z],
            probability: intent.probability,
        };

        if let Err(error) = bridge.tx_to_agent.send(event) {
            tracing::warn!(
                target: "bong::mineral",
                "karma flag forward to agent bridge failed: {error}"
            );
        }
    }
}
