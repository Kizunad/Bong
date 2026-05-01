//! plan-mineral-v1 §M6 — KarmaFlagIntent → GameEvent::MineralKarmaFlag bridge。
//!
//! 监听 `KarmaFlagIntent`，把品阶 ≥ 3 矿块的劫气标记转发给 agent_bridge
//! 的 GameEvent 通道。当前 agent 端仅订阅 stub；本桥保证 emit 链路就位，
//! 即便 agent 未消费也不报错（crossbeam send 失败仅日志告警）。

use valence::prelude::{EventReader, Position, Query, Res, ResMut};

use crate::network::agent_bridge::{GameEvent, NetworkBridgeResource};
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::karma::{KarmaWeightStore, QiDensityHeatmap};
use crate::world::zone::ZoneRegistry;

use super::events::KarmaFlagIntent;
use super::persistence::MineralTickClock;

/// 玩家 entity → username 反查。当前 server 用 valence::client::Username 组件
/// 存玩家用户名。bridge 在 emit 时按 Entity 查名字；查不到则用 "<unknown>" 占位。
type ClientUsernameQuery<'world, 'state> =
    Query<'world, 'state, &'static valence::prelude::Username>;

type ClientKarmaContextQuery<'world, 'state> = Query<
    'world,
    'state,
    (
        &'static valence::prelude::Username,
        Option<&'static Position>,
        Option<&'static CurrentDimension>,
    ),
>;

pub fn record_karma_flag_weights(
    mut karma: EventReader<KarmaFlagIntent>,
    mut weights: Option<ResMut<KarmaWeightStore>>,
    mut heatmap: Option<ResMut<QiDensityHeatmap>>,
    clients: ClientKarmaContextQuery,
    zones: Option<Res<ZoneRegistry>>,
    clock: Option<Res<MineralTickClock>>,
) {
    let (Some(weights), Some(heatmap)) = (weights.as_deref_mut(), heatmap.as_deref_mut()) else {
        return;
    };
    let tick = clock.as_deref().map(|clock| clock.tick).unwrap_or_default();

    for intent in karma.read() {
        let Ok((username, position, dimension)) = clients.get(intent.player) else {
            continue;
        };
        let dimension = dimension
            .map(|current| current.0)
            .unwrap_or(DimensionKind::Overworld);
        let zone_name = zones.as_deref().and_then(|registry| {
            position
                .and_then(|pos| registry.find_zone(dimension, pos.get()))
                .map(|zone| zone.name.clone())
        });

        weights.mark_player(
            username.0.clone(),
            zone_name,
            intent.position,
            intent.probability,
            tick,
        );
        heatmap.add_heat(dimension, intent.position, intent.probability);
    }
}

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

#[cfg(test)]
mod tests {
    use crossbeam_channel::unbounded;
    use valence::prelude::{App, BlockPos, Position, Update, Username};

    use super::*;
    use crate::mineral::types::MineralId;

    #[test]
    fn karma_flag_records_internal_weight_and_heatmap() {
        let mut app = App::new();
        app.insert_resource(KarmaWeightStore::default());
        app.insert_resource(QiDensityHeatmap::default());
        app.insert_resource(MineralTickClock { tick: 42 });
        app.add_event::<KarmaFlagIntent>();
        app.add_systems(Update, record_karma_flag_weights);

        let player = app
            .world_mut()
            .spawn((
                Username("Azure".to_string()),
                Position::new([8.0, 66.0, 8.0]),
            ))
            .id();
        app.world_mut().send_event(KarmaFlagIntent {
            player,
            mineral_id: MineralId::SuiTie,
            position: BlockPos::new(31, 64, -1),
            probability: 0.15,
        });

        app.update();

        let weights = app.world().resource::<KarmaWeightStore>();
        let entry = weights
            .entry_for_player("Azure")
            .expect("karma flag should create player weight");
        assert_eq!(entry.weight, 0.15);
        assert_eq!(entry.last_tick, 42);
        assert_eq!(entry.last_position, [31, 64, -1]);

        let heatmap = app.world().resource::<QiDensityHeatmap>();
        assert_eq!(
            heatmap.heat_at(DimensionKind::Overworld, BlockPos::new(20, 64, -8)),
            0.15
        );
    }

    #[test]
    fn karma_flag_forwarding_keeps_agent_bridge_event() {
        let mut app = App::new();
        let (tx_to_agent, rx_from_game) = unbounded();
        let (_tx_to_game, rx_from_agent) = unbounded();
        app.insert_resource(NetworkBridgeResource {
            tx_to_agent,
            rx_from_agent,
        });
        app.add_event::<KarmaFlagIntent>();
        app.add_systems(Update, forward_karma_flag_to_agent);

        let player = app.world_mut().spawn(Username("Azure".to_string())).id();
        app.world_mut().send_event(KarmaFlagIntent {
            player,
            mineral_id: MineralId::KuJin,
            position: BlockPos::new(1, 64, 2),
            probability: 0.30,
        });

        app.update();

        let event = rx_from_game
            .try_recv()
            .expect("agent bridge should receive mineral karma event");
        match event {
            GameEvent::MineralKarmaFlag {
                player_username,
                mineral_id,
                position,
                probability,
            } => {
                assert_eq!(player_username, "Azure");
                assert_eq!(mineral_id, "ku_jin");
                assert_eq!(position, [1, 64, 2]);
                assert_eq!(probability, 0.30);
            }
            other => panic!("unexpected game event: {other:?}"),
        }
    }
}
