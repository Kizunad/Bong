use valence::prelude::{EventReader, Res};

use super::redis_bridge::RedisOutbound;
use super::RedisBridgeResource;
use crate::npc::dormant::{DormantCombatOutcome, PendingDormantRelicCreated};
use crate::npc::faction::FactionEventNotice;
use crate::npc::lifecycle::{NpcDeathNotice, NpcSpawnNotice};
use crate::npc::movement::GameTick;
use crate::schema::npc::{
    DormantCombatOutcomeV1, FactionEventV1, NpcDeathV1, NpcSpawnedV1, PendingDormantRelicV1,
};

const NPC_EVENT_VERSION: u8 = 1;

pub fn publish_npc_spawn_events(
    redis: Res<RedisBridgeResource>,
    game_tick: Option<Res<GameTick>>,
    mut events: EventReader<NpcSpawnNotice>,
) {
    let at_tick = current_game_tick(game_tick.as_deref());
    for ev in events.read() {
        let wire = NpcSpawnedV1 {
            v: NPC_EVENT_VERSION,
            kind: "npc_spawned".to_string(),
            npc_id: ev.npc_id.clone(),
            archetype: ev.archetype.as_str().to_string(),
            source: ev.source.as_str().to_string(),
            zone: ev.home_zone.clone(),
            pos: [ev.position.x, ev.position.y, ev.position.z],
            initial_age_ticks: ev.initial_age_ticks,
            at_tick,
        };
        if let Err(error) = redis.tx_outbound.send(RedisOutbound::NpcSpawned(wire)) {
            tracing::warn!("[bong][npc_event_bridge] dropped NpcSpawned: {error}");
        }
    }
}

pub fn publish_npc_death_events(
    redis: Res<RedisBridgeResource>,
    game_tick: Option<Res<GameTick>>,
    mut events: EventReader<NpcDeathNotice>,
) {
    let at_tick = current_game_tick(game_tick.as_deref());
    for ev in events.read() {
        let wire = NpcDeathV1 {
            v: NPC_EVENT_VERSION,
            kind: "npc_death".to_string(),
            npc_id: ev.npc_id.clone(),
            archetype: ev.archetype.as_str().to_string(),
            cause: ev.reason.as_str().to_string(),
            faction_id: ev.faction_id.map(|faction| faction.as_str().to_string()),
            life_record_snapshot: ev.life_record_snapshot.clone(),
            age_ticks: ev.age_ticks,
            max_age_ticks: ev.max_age_ticks,
            at_tick,
            from_dormant_combat: ev.from_dormant_combat,
            pos: ev.pos,
        };
        if let Err(error) = redis.tx_outbound.send(RedisOutbound::NpcDeath(wire)) {
            tracing::warn!("[bong][npc_event_bridge] dropped NpcDeath: {error}");
        }
    }
}

/// plan-offscreen-war-v1 P2：把离屏 dormant 互殴战果 `DormantCombatOutcome` 转成
/// `DormantCombatOutcomeV1` 发到 `bong:npc/combat`（纯 telemetry，镜像
/// `publish_npc_death_events`）。**真元流动不经此**——已由 dormant 战死结算的
/// `release_dormant_qi_to_zone` → `ledger.transfer(ReleaseToZone)` 真实记账完成；本系统
/// 只搬运观测字段，外部 e2e 据此把 outcome 与 `bong:npc/death` 对账。
pub fn publish_dormant_combat_events(
    redis: Res<RedisBridgeResource>,
    game_tick: Option<Res<GameTick>>,
    mut events: EventReader<DormantCombatOutcome>,
) {
    let at_tick = current_game_tick(game_tick.as_deref());
    for ev in events.read() {
        let wire = DormantCombatOutcomeV1 {
            v: NPC_EVENT_VERSION,
            kind: "dormant_combat_outcome".to_string(),
            winner: ev.winner.clone(),
            loser: ev.loser.clone(),
            zone: ev.zone.clone(),
            qi_released: ev.qi_released,
            at_tick,
        };
        if let Err(error) = redis
            .tx_outbound
            .send(RedisOutbound::DormantCombatOutcome(wire))
        {
            tracing::warn!("[bong][npc_event_bridge] dropped DormantCombatOutcome: {error}");
        }
    }
}

/// plan-offscreen-war-v1 P3：把克制式战场遗物创建 `PendingDormantRelicCreated` 转成
/// `PendingDormantRelicV1` 发到 `bong:npc/relic`（纯 telemetry，镜像
/// `publish_dormant_combat_events`）。**零真元**——遗物 loot 物化时 spirit_quality=0，持久层
/// 不碰 ledger；本系统只搬运观测字段，让真服 e2e 在不便直接读 sqlite 时仍能 headless 断言
/// "知名战死 → 遗物创建"（§11）。同一 event 也被 persistence 消费落盘 sqlite，两条消费互不干扰。
pub fn publish_pending_dormant_relic_events(
    redis: Res<RedisBridgeResource>,
    game_tick: Option<Res<GameTick>>,
    mut events: EventReader<PendingDormantRelicCreated>,
) {
    let at_tick = current_game_tick(game_tick.as_deref());
    for ev in events.read() {
        let wire = PendingDormantRelicV1 {
            v: NPC_EVENT_VERSION,
            kind: "pending_dormant_relic".to_string(),
            char_id: ev.char_id.clone(),
            zone: ev.zone.clone(),
            pos: ev.position,
            archetype: ev.archetype.as_str().to_string(),
            loot_seed: ev.loot_seed,
            created_tick: ev.created_tick,
            at_tick,
        };
        if let Err(error) = redis
            .tx_outbound
            .send(RedisOutbound::PendingDormantRelic(wire))
        {
            tracing::warn!("[bong][npc_event_bridge] dropped PendingDormantRelic: {error}");
        }
    }
}

pub fn publish_faction_events(
    redis: Res<RedisBridgeResource>,
    game_tick: Option<Res<GameTick>>,
    mut events: EventReader<FactionEventNotice>,
) {
    let at_tick = current_game_tick(game_tick.as_deref());
    for ev in events.read() {
        let wire = FactionEventV1 {
            v: NPC_EVENT_VERSION,
            kind: "faction_event".to_string(),
            faction_id: ev.applied.faction_id.as_str().to_string(),
            event_kind: ev.applied.kind.as_str().to_string(),
            leader_id: ev.applied.leader_id.clone(),
            loyalty_bias: ev.applied.loyalty_bias,
            mission_queue_size: ev.applied.mission_queue_size,
            at_tick,
        };
        if let Err(error) = redis.tx_outbound.send(RedisOutbound::FactionEvent(wire)) {
            tracing::warn!("[bong][npc_event_bridge] dropped FactionEvent: {error}");
        }
    }
}

fn current_game_tick(game_tick: Option<&GameTick>) -> u64 {
    game_tick.map(|tick| u64::from(tick.0)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::redis_bridge::RedisOutbound;
    use crate::npc::faction::{FactionEventApplied, FactionEventKind, FactionId};
    use crate::npc::lifecycle::{NpcArchetype, NpcDeathReason};
    use crossbeam_channel::{unbounded, Receiver};
    use valence::prelude::{App, DVec3, Update};

    fn setup_app() -> (App, Receiver<RedisOutbound>) {
        let mut app = App::new();
        let (tx_outbound, rx_outbound) = unbounded();
        let (_tx_inbound, rx_inbound) = unbounded();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        (app, rx_outbound)
    }

    #[test]
    fn publish_faction_events_uses_dedicated_outbound_variant() {
        let (mut app, rx) = setup_app();
        app.add_event::<FactionEventNotice>();
        app.insert_resource(GameTick(321));
        app.add_systems(Update, publish_faction_events);

        app.world_mut().send_event(FactionEventNotice {
            applied: FactionEventApplied {
                faction_id: FactionId::Attack,
                kind: FactionEventKind::AdjustLoyaltyBias,
                leader_id: None,
                loyalty_bias: 0.7,
                mission_queue_size: 2,
            },
        });
        app.update();

        let outbound = rx.try_recv().expect("expected faction event outbound");
        let RedisOutbound::FactionEvent(payload) = outbound else {
            panic!("expected FactionEvent outbound");
        };
        assert_eq!(payload.faction_id, "attack");
        assert_eq!(payload.event_kind, "adjust_loyalty_bias");
        assert_eq!(payload.at_tick, 321);
    }

    #[test]
    fn publish_spawn_and_death_events_use_game_tick() {
        let (mut app, rx) = setup_app();
        app.add_event::<NpcSpawnNotice>();
        app.add_event::<NpcDeathNotice>();
        app.insert_resource(GameTick(654));
        app.add_systems(Update, (publish_npc_spawn_events, publish_npc_death_events));

        app.world_mut().send_event(NpcSpawnNotice {
            npc_id: "npc_1v1".to_string(),
            archetype: NpcArchetype::Rogue,
            source: crate::npc::lifecycle::NpcSpawnSource::AgentCommand,
            home_zone: "green_cloud_peak".to_string(),
            position: DVec3::new(1.0, 64.0, 2.0),
            initial_age_ticks: 0.0,
        });
        app.world_mut().send_event(NpcDeathNotice {
            npc_id: "npc_2v1".to_string(),
            archetype: NpcArchetype::Commoner,
            reason: NpcDeathReason::Combat,
            faction_id: Some(FactionId::Neutral),
            life_record_snapshot: None,
            age_ticks: 10.0,
            max_age_ticks: 20.0,
            from_dormant_combat: false,
            pos: None,
        });
        app.update();

        let outbounds = [
            rx.try_recv().expect("expected first NPC event outbound"),
            rx.try_recv().expect("expected second NPC event outbound"),
        ];
        assert!(outbounds.iter().any(|outbound| matches!(
            outbound,
            RedisOutbound::NpcSpawned(payload) if payload.at_tick == 654
        )));
        assert!(outbounds.iter().any(|outbound| matches!(
            outbound,
            RedisOutbound::NpcDeath(payload) if payload.at_tick == 654
        )));
    }

    #[test]
    fn publish_pending_dormant_relic_uses_dedicated_outbound_variant() {
        // plan-offscreen-war-v1 P3：PendingDormantRelicCreated event → bong:npc/relic 观测旁路。
        let (mut app, rx) = setup_app();
        app.add_event::<PendingDormantRelicCreated>();
        app.insert_resource(GameTick(777));
        app.add_systems(Update, publish_pending_dormant_relic_events);

        app.world_mut().send_event(PendingDormantRelicCreated {
            char_id: "dormant:fallen:disciple".to_string(),
            zone: "rift_valley".to_string(),
            position: [12.0, 64.0, -8.0],
            archetype: NpcArchetype::Disciple,
            loot_seed: 0xDEAD_BEEF,
            created_tick: 42,
        });
        app.update();

        let outbound = rx.try_recv().expect("expected pending relic outbound");
        let RedisOutbound::PendingDormantRelic(payload) = outbound else {
            panic!("expected PendingDormantRelic outbound, got a different variant");
        };
        assert_eq!(payload.char_id, "dormant:fallen:disciple");
        assert_eq!(payload.zone, "rift_valley");
        assert_eq!(payload.archetype, "disciple");
        assert_eq!(payload.loot_seed, 0xDEAD_BEEF);
        assert_eq!(payload.created_tick, 42);
        assert_eq!(payload.at_tick, 777);
        assert_eq!(payload.kind, "pending_dormant_relic");
    }

    #[test]
    fn publish_pending_dormant_relic_defaults_at_tick_to_zero_without_game_tick() {
        // 边界（CodeRabbit）：缺 GameTick 资源时 at_tick 必须回退 0（current_game_tick 的
        // unwrap_or_default 分支），而非 panic / 读到脏值——否则 telemetry 时间戳错乱。
        let (mut app, rx) = setup_app();
        app.add_event::<PendingDormantRelicCreated>();
        // 故意**不** insert GameTick。
        app.add_systems(Update, publish_pending_dormant_relic_events);

        app.world_mut().send_event(PendingDormantRelicCreated {
            char_id: "dormant:no_tick".to_string(),
            zone: "spawn".to_string(),
            position: [0.0, 64.0, 0.0],
            archetype: NpcArchetype::Disciple,
            loot_seed: 1,
            created_tick: 5,
        });
        app.update();

        let outbound = rx.try_recv().expect("expected pending relic outbound even without GameTick");
        let RedisOutbound::PendingDormantRelic(payload) = outbound else {
            panic!("expected PendingDormantRelic outbound, got a different variant");
        };
        assert_eq!(
            payload.at_tick, 0,
            "without a GameTick resource, at_tick must default to 0 (current_game_tick falls back to u64::default), got {}",
            payload.at_tick
        );
        // created_tick 来自 event 本身、与 GameTick 无关，仍应原样透传。
        assert_eq!(
            payload.created_tick, 5,
            "created_tick comes from the event, not GameTick, and must pass through unchanged; got {}",
            payload.created_tick
        );
    }

    #[test]
    fn publish_pending_dormant_relic_drops_on_closed_channel_without_panic() {
        // 错误分支（CodeRabbit）：outbound channel 已关闭（接收端 drop）时，send 返回 Err，
        // 系统必须 warn + drop 该 event、**不 panic**（否则一个掉线的 Redis 桥会拖垮整个 tick）。
        let mut app = App::new();
        let (tx_outbound, rx_outbound) = unbounded::<RedisOutbound>();
        let (_tx_inbound, rx_inbound) = unbounded();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        // 关闭接收端 → 后续 send 必返回 SendError。
        drop(rx_outbound);
        app.add_event::<PendingDormantRelicCreated>();
        app.insert_resource(GameTick(9));
        app.add_systems(Update, publish_pending_dormant_relic_events);

        app.world_mut().send_event(PendingDormantRelicCreated {
            char_id: "dormant:dropped".to_string(),
            zone: "spawn".to_string(),
            position: [0.0, 64.0, 0.0],
            archetype: NpcArchetype::Disciple,
            loot_seed: 2,
            created_tick: 9,
        });
        // 不 panic 即通过该分支的核心契约（系统吞下 SendError、记 warn 继续）。
        app.update();
        // 二次 update 仍不 panic（确认 channel 关闭是稳定可重入的 drop 路径）。
        app.world_mut().send_event(PendingDormantRelicCreated {
            char_id: "dormant:dropped_again".to_string(),
            zone: "spawn".to_string(),
            position: [0.0, 64.0, 0.0],
            archetype: NpcArchetype::Disciple,
            loot_seed: 3,
            created_tick: 10,
        });
        app.update();
    }
}
