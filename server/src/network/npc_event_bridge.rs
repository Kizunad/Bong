use valence::prelude::{EventReader, EventWriter, Res, ResMut};

use super::redis_bridge::RedisOutbound;
use super::RedisBridgeResource;
use crate::npc::dormant::census::{assign_status, compute_faction_census, LastFactionCensus};
use crate::npc::dormant::{
    effective_group, DormantCombatOutcome, NpcDormantStore, PendingDormantRelicCreated,
};
use crate::npc::faction::{
    FactionEventNotice, FactionRelationMatrix, FactionStore, NamedFactionDecayEvent,
    NamedFactionId, NamedFactionLeaderDownEvent, NamedFactionRegistry,
};
use crate::npc::lifecycle::{NpcDeathNotice, NpcSpawnNotice};
use crate::npc::movement::GameTick;
use crate::npc::war::{WarConflictStore, WarPhaseChanged, ZoneConflictPressure};
use crate::schema::cultivation::realm_to_string;
use crate::schema::npc::{
    DormantCombatOutcomeV1, FactionEventV1, FactionRelationEntryV1, FactionStateV1,
    FactionWarEventV1, NamedFactionEntryV1, NamedFactionStateV1, NpcDeathV1, NpcSpawnedV1,
    PendingDormantRelicV1,
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

/// plan-offscreen-war-v1 P5：周期性把散修群体消长盘面发到 `bong:faction_state`（纯 telemetry，
/// reframe b）。读 dormant store + faction store → [`compute_faction_census`] 算每个涌现群体的
/// 人口 / 众数 zone / 涌现强者 → [`assign_status`] 用上轮人口（[`LastFactionCensus`]）判消长 →
/// 每组组装一条 [`FactionStateV1`] publish → 更新 `LastFactionCensus` 供下轮对比。
///
/// **节奏**：挂在与 `publish_dormant_combat_events` 同一 `Update` 调度里（不另起 timer，§10.1 #3
/// 同节奏旁路），每帧跑——census 是 store 的廉价只读快照，不依赖 dormant batch tick 间隔。
///
/// **守恒红线（§10.1 #5）**：全程只读 store + faction store，绝不碰 `WorldQiAccount` / ledger；
/// 强者陨落（最高境界活体已被 P2 结算移出 store）只让本系统读到更少人口 + 更低强者境界 → 发出
/// status=Waning 的盘面，真元守恒回灌仍唯一走 P2 的 `release_dormant_qi_to_zone`。绝不学
/// abstract_combat 那种 emit-only QiTransfer 蒸发——本系统不 emit 任何真元流动。
pub fn publish_faction_state(
    redis: Res<RedisBridgeResource>,
    game_tick: Option<Res<GameTick>>,
    store: Res<NpcDormantStore>,
    faction_store: Res<FactionStore>,
    mut last_census: ResMut<LastFactionCensus>,
) {
    let at_tick = current_game_tick(game_tick.as_deref());
    let census = compute_faction_census(&store, &faction_store);

    // 先用上轮历史判每组消长 + publish，再整体替换 LastFactionCensus（避免边算边覆盖串味）。
    let mut next_history = std::collections::HashMap::with_capacity(census.len());
    for entry in &census {
        let status = assign_status(entry.group_id, entry.population, &last_census);
        let wire = FactionStateV1 {
            v: NPC_EVENT_VERSION,
            kind: "faction_state".to_string(),
            group_id: entry.group_id.0,
            region_descriptor: entry.region_descriptor.clone(),
            population: entry.population,
            status,
            dominant_zone: entry.dominant_zone.clone(),
            strongest_realm: realm_to_string(entry.strongest_realm).to_string(),
            strongest_char_id: entry.strongest_char_id.clone(),
            at_tick,
        };
        if let Err(error) = redis.tx_outbound.send(RedisOutbound::FactionState(wire)) {
            tracing::warn!("[bong][npc_event_bridge] dropped FactionState: {error}");
        }
        next_history.insert(entry.group_id, entry.population);
    }
    // 整体替换：消失的群体（本轮无 entry）也从历史里清掉，下次重新涌现时按「新涌现」判 Rising。
    last_census.0 = next_history;
}

/// plan-faction-expansion-v1 P3：发布具名势力注册表/关系矩阵快照到
/// `bong:named_faction_state`。这是纯观测 telemetry，不触真元账户。
pub fn publish_named_faction_state(
    redis: Res<RedisBridgeResource>,
    game_tick: Option<Res<GameTick>>,
    registry: Option<Res<NamedFactionRegistry>>,
    relation_matrix: Option<Res<FactionRelationMatrix>>,
) {
    let Some(registry) = registry.as_deref() else {
        return;
    };
    let Some(relation_matrix) = relation_matrix.as_deref() else {
        return;
    };
    let at_tick = current_game_tick(game_tick.as_deref());
    publish_named_faction_snapshot(&redis, at_tick, registry, relation_matrix);
}

/// plan-faction-expansion-v1 P3：领袖陨落/势力消亡事件触发一次显式快照刷新。
pub fn publish_named_faction_state_on_lifecycle_events(
    redis: Res<RedisBridgeResource>,
    game_tick: Option<Res<GameTick>>,
    registry: Option<Res<NamedFactionRegistry>>,
    relation_matrix: Option<Res<FactionRelationMatrix>>,
    mut leader_down_events: EventReader<NamedFactionLeaderDownEvent>,
    mut decay_events: EventReader<NamedFactionDecayEvent>,
) {
    let Some(registry) = registry.as_deref() else {
        return;
    };
    let Some(relation_matrix) = relation_matrix.as_deref() else {
        return;
    };
    let transition_count = leader_down_events.read().count() + decay_events.read().count();
    if transition_count == 0 {
        return;
    }
    let at_tick = current_game_tick(game_tick.as_deref());
    publish_named_faction_snapshot(&redis, at_tick, registry, relation_matrix);
}

fn publish_named_faction_snapshot(
    redis: &RedisBridgeResource,
    at_tick: u64,
    registry: &NamedFactionRegistry,
    relation_matrix: &FactionRelationMatrix,
) {
    let named_factions = registry
        .iter()
        .map(|faction| NamedFactionEntryV1 {
            id: faction.id.as_str().to_string(),
            display_name: faction.display_name.clone(),
            zone_anchor: faction.zone_anchor.clone(),
            current_npc_count: faction.current_npc_count,
            status: faction.status,
            is_active: faction.is_active,
        })
        .collect();

    let all = NamedFactionId::all();
    let mut relation_entries = Vec::new();
    for i in 0..all.len() {
        for j in (i + 1)..all.len() {
            relation_entries.push(FactionRelationEntryV1 {
                a: all[i].as_str().to_string(),
                b: all[j].as_str().to_string(),
                hostile: relation_matrix.are_hostile(all[i], all[j]),
            });
        }
    }

    let wire = NamedFactionStateV1 {
        v: NPC_EVENT_VERSION,
        kind: "named_faction_state".to_string(),
        named_factions,
        relation_matrix: relation_entries,
        at_tick,
    };
    if let Err(error) = redis
        .tx_outbound
        .send(RedisOutbound::NamedFactionState(wire))
    {
        tracing::warn!("[bong][npc_event_bridge] dropped NamedFactionState: {error}");
    }
}

/// plan-offscreen-war-v1 P6：把离屏战果压力累积到 `ZoneConflictPressure`，并在越阈时创建/升级
/// `WarConflictStore` 里的涌现冲突。**纯计数**（`pressure += 1.0`/条），切断真元语义关联；
/// 真元流动仍唯一走 P2 `release_dormant_qi_to_zone`。
///
/// - 每条 `DormantCombatOutcome` 读 zone + winner/loser char_id → 查 dormant store 反查
///   `effective_group`（有则取，无则跳过 group 累积但仍更新 pressure/casualties）。
/// - 越阈且 ≥2 group 后调 `WarConflictStore::escalate_or_create`，每次相变 emit `WarPhaseChanged`。
pub fn accumulate_zone_conflict_pressure(
    mut combat_events: EventReader<DormantCombatOutcome>,
    dormant_store: Res<NpcDormantStore>,
    faction_store: Res<FactionStore>,
    mut pressure: ResMut<ZoneConflictPressure>,
    mut war_store: ResMut<WarConflictStore>,
    mut phase_changed: EventWriter<WarPhaseChanged>,
    game_tick: Option<Res<GameTick>>,
) {
    let now = current_game_tick(game_tick.as_deref());

    for ev in combat_events.read() {
        let entry = pressure.entry(&ev.zone);
        // 反查 winner/loser 的 effective_group（dormant store + faction store 联查）。
        // 查不到的 char_id 不阻止 pressure 累积，只是该群体不被计入 groups。
        let winner_group = dormant_store
            .snapshots
            .get(ev.winner.as_str())
            .and_then(|snap| effective_group(snap, &faction_store));
        let loser_group = dormant_store
            .snapshots
            .get(ev.loser.as_str())
            .and_then(|snap| effective_group(snap, &faction_store));

        match (winner_group, loser_group) {
            (Some(w), Some(l)) => entry.accumulate(w, l, now),
            (Some(w), None) => {
                // winner 侧有 group，loser 侧无：累积 pressure/casualties + wins_by_group[w]+1
                entry.pressure += 1.0;
                entry.casualties += 1;
                entry.last_tick = now;
                *entry.wins_by_group.entry(w).or_insert(0) += 1;
                if !entry.groups.contains(&w) {
                    entry.groups.push(w);
                    entry.groups.sort();
                }
            }
            (None, Some(l)) => {
                // loser 侧有 group，winner 侧无：累积 pressure/casualties，不计 wins
                entry.pressure += 1.0;
                entry.casualties += 1;
                entry.last_tick = now;
                if !entry.groups.contains(&l) {
                    entry.groups.push(l);
                    entry.groups.sort();
                }
            }
            (None, None) => {
                // 两侧都无 group（纯 NPC，无 faction）：仍累积计数
                entry.pressure += 1.0;
                entry.casualties += 1;
                entry.last_tick = now;
            }
        }

        // 每次 outcome 后检查该 zone 是否需要升级 war
        let entry_snapshot = pressure.get(&ev.zone).cloned().unwrap_or_default();
        for (war_id, new_phase, war_snapshot) in
            war_store.escalate_or_create(&ev.zone, &entry_snapshot, now)
        {
            let counts = war_snapshot.role_counts();
            phase_changed.send(WarPhaseChanged {
                war_id,
                zone: war_snapshot.zone.clone(),
                region_descriptor: war_snapshot.region_descriptor.clone(),
                phase: new_phase,
                groups: war_snapshot.groups.clone(),
                outcome: war_snapshot.outcome.clone(),
                player_role_counts: counts,
                war_snapshot_player_roles: war_snapshot.player_roles.clone(),
                at_tick: now,
            });
        }
    }
}

/// plan-offscreen-war-v1 P6：tick 推进 Settling war 的冷却（无 outcome 驱动也要走）。
/// 遍历所有 Settling war，到冷却时升 Aftermath + 清 zone_active + emit。
pub fn advance_idle_wars(
    mut war_store: ResMut<WarConflictStore>,
    mut phase_changed: EventWriter<WarPhaseChanged>,
    game_tick: Option<Res<GameTick>>,
) {
    let now = current_game_tick(game_tick.as_deref());
    for (war_id, new_phase, war_snapshot) in war_store.advance_settling_wars(now) {
        let counts = war_snapshot.role_counts();
        phase_changed.send(WarPhaseChanged {
            war_id,
            zone: war_snapshot.zone.clone(),
            region_descriptor: war_snapshot.region_descriptor.clone(),
            phase: new_phase,
            groups: war_snapshot.groups.clone(),
            outcome: war_snapshot.outcome.clone(),
            player_role_counts: counts,
            war_snapshot_player_roles: war_snapshot.player_roles.clone(),
            at_tick: now,
        });
    }
}

/// plan-offscreen-war-v1 P6：把 `WarPhaseChanged` 转为 `FactionWarEventV1` publish 到
/// `bong:faction/war`（纯观测、零真元）。
pub fn publish_faction_war(
    redis: Res<RedisBridgeResource>,
    game_tick: Option<Res<GameTick>>,
    mut events: EventReader<WarPhaseChanged>,
) {
    let at_tick = current_game_tick(game_tick.as_deref());
    for evt in events.read() {
        let counts = evt.player_role_counts;
        let wire = FactionWarEventV1 {
            v: 1,
            kind: "faction_war_event".to_string(),
            war_id: evt.war_id.0,
            zone: evt.zone.clone(),
            region_descriptor: evt.region_descriptor.clone(),
            phase: evt.phase,
            groups: evt.groups.iter().map(|g| g.0).collect(),
            enlist_count: counts.enlist,
            mercenary_count: counts.mercenary,
            intercept_count: counts.intercept,
            spectate_count: counts.spectate,
            winner_group: evt.outcome.as_ref().map(|o| o.winner_group.0),
            loser_group: evt.outcome.as_ref().map(|o| o.loser_group.0),
            total_casualties: evt.outcome.as_ref().map(|o| o.total_casualties),
            at_tick,
        };
        if let Err(error) = redis.tx_outbound.send(RedisOutbound::FactionWar(wire)) {
            tracing::warn!("[bong][npc_event_bridge] dropped FactionWarEventV1: {error}");
        }
    }
}

fn current_game_tick(game_tick: Option<&GameTick>) -> u64 {
    game_tick.map(|tick| u64::from(tick.0)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::{Cultivation, Realm};
    use crate::network::redis_bridge::RedisOutbound;
    use crate::npc::dormant::{DormantBehaviorIntent, NpcDormantSnapshot};
    use crate::npc::faction::{
        EmergentGroupId, FactionEventApplied, FactionEventKind, FactionId, FactionRelationMatrix,
        FactionStatus, GroupStatus, NamedFactionId, NamedFactionRegistry,
    };
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

        let outbound = rx
            .try_recv()
            .expect("expected pending relic outbound even without GameTick");
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

    // ───────────────────────── publish_faction_state (P5) ─────────────────────────

    /// 造一个最小 dormant 快照：char_id / zone / realm / 显式 emergent_group（与 census 测试同源）。
    fn dormant_snapshot(
        char_id: &str,
        zone: &str,
        realm: Realm,
        group: EmergentGroupId,
    ) -> NpcDormantSnapshot {
        let cultivation = Cultivation {
            realm,
            ..Default::default()
        };
        NpcDormantSnapshot {
            char_id: char_id.to_string(),
            archetype: NpcArchetype::Rogue,
            dimension: crate::world::dimension::DimensionKind::Overworld,
            zone_name: zone.to_string(),
            position: [0.0, 64.0, 0.0],
            schedule_seed: None,
            cultivation: cultivation.clone(),
            meridian_system: Default::default(),
            meridian_severed: Default::default(),
            contamination: Default::default(),
            lifespan: crate::npc::lifecycle::NpcLifespan::new(0.0, 100_000.0),
            shared_lifespan: crate::cultivation::lifespan::LifespanComponent::for_realm(realm),
            lifespan_extension_ledger: Default::default(),
            death_registry: crate::cultivation::lifespan::DeathRegistry::new(char_id.to_string()),
            life_record: crate::cultivation::life_record::LifeRecord::new(char_id.to_string()),
            faction: None,
            emergent_group: Some(group),
            patrol: None,
            loot_table: None,
            guardian_relic: None,
            tsy_hostile: None,
            intent: DormantBehaviorIntent::Cultivate {
                zone: zone.to_string(),
            },
            dormant_since_tick: 0,
            last_dormant_tick_processed: 0,
            initial_qi: cultivation.qi_current,
            qi_ledger_net: 0.0,
            combat_dead_pending_release: false,
        }
    }

    fn setup_faction_state_app(
        snaps: Vec<NpcDormantSnapshot>,
        tick: u64,
    ) -> (App, Receiver<RedisOutbound>) {
        let (mut app, rx) = setup_app();
        let mut store = NpcDormantStore::default();
        for snap in snaps {
            store.snapshots.insert(snap.char_id.clone(), snap);
        }
        store.rebuild_indexes();
        app.insert_resource(store);
        app.insert_resource(FactionStore::default());
        app.init_resource::<LastFactionCensus>();
        app.insert_resource(GameTick(tick as u32));
        app.add_systems(Update, publish_faction_state);
        (app, rx)
    }

    fn drain_faction_states(rx: &Receiver<RedisOutbound>) -> Vec<FactionStateV1> {
        let mut out = Vec::new();
        while let Ok(outbound) = rx.try_recv() {
            if let RedisOutbound::FactionState(payload) = outbound {
                out.push(payload);
            }
        }
        out.sort_by_key(|p| p.group_id);
        out
    }

    fn drain_named_faction_states(rx: &Receiver<RedisOutbound>) -> Vec<NamedFactionStateV1> {
        let mut out = Vec::new();
        while let Ok(outbound) = rx.try_recv() {
            if let RedisOutbound::NamedFactionState(payload) = outbound {
                out.push(payload);
            }
        }
        out
    }

    #[test]
    fn publish_named_faction_state_includes_decayed_is_active_false() {
        let (mut app, rx) = setup_app();
        let mut registry = NamedFactionRegistry::startup_default();
        registry
            .get_mut(NamedFactionId::CangyuanMerchants)
            .unwrap()
            .set_status(FactionStatus::Decayed);
        app.insert_resource(registry);
        app.insert_resource(FactionRelationMatrix::startup_default());
        app.insert_resource(GameTick(77));
        app.add_systems(Update, publish_named_faction_state);

        app.update();

        let states = drain_named_faction_states(&rx);
        assert_eq!(states.len(), 1);
        let cangyuan = states[0]
            .named_factions
            .iter()
            .find(|entry| entry.id == "cangyuan_merchants")
            .expect("cangyuan entry must be present");
        assert_eq!(cangyuan.status, FactionStatus::Decayed);
        assert!(
            !cangyuan.is_active,
            "Decayed 势力的 Redis 快照必须显式 is_active=false"
        );
        assert_eq!(states[0].relation_matrix.len(), 3);
        assert_eq!(states[0].at_tick, 77);
    }

    #[test]
    fn publish_named_faction_state_skips_when_required_resources_missing() {
        let (mut missing_registry_app, missing_registry_rx) = setup_app();
        missing_registry_app.insert_resource(FactionRelationMatrix::startup_default());
        missing_registry_app.add_systems(Update, publish_named_faction_state);
        missing_registry_app.update();
        assert!(
            drain_named_faction_states(&missing_registry_rx).is_empty(),
            "missing NamedFactionRegistry must skip publishing instead of emitting partial state"
        );

        let (mut missing_matrix_app, missing_matrix_rx) = setup_app();
        missing_matrix_app.insert_resource(NamedFactionRegistry::startup_default());
        missing_matrix_app.add_systems(Update, publish_named_faction_state);
        missing_matrix_app.update();
        assert!(
            drain_named_faction_states(&missing_matrix_rx).is_empty(),
            "missing FactionRelationMatrix must skip publishing instead of emitting partial state"
        );
    }

    #[test]
    fn publish_named_faction_state_on_lifecycle_events_skips_when_no_events() {
        let (mut app, rx) = setup_app();
        app.add_event::<NamedFactionLeaderDownEvent>();
        app.add_event::<NamedFactionDecayEvent>();
        app.insert_resource(NamedFactionRegistry::startup_default());
        app.insert_resource(FactionRelationMatrix::startup_default());
        app.add_systems(Update, publish_named_faction_state_on_lifecycle_events);

        app.update();

        assert!(
            drain_named_faction_states(&rx).is_empty(),
            "zero lifecycle events must not publish a named faction snapshot"
        );
    }

    #[test]
    fn publish_named_faction_state_on_lifecycle_events_skips_without_required_resources() {
        let (mut missing_registry_app, missing_registry_rx) = setup_app();
        missing_registry_app.add_event::<NamedFactionLeaderDownEvent>();
        missing_registry_app.add_event::<NamedFactionDecayEvent>();
        missing_registry_app.insert_resource(FactionRelationMatrix::startup_default());
        missing_registry_app.add_systems(Update, publish_named_faction_state_on_lifecycle_events);
        missing_registry_app
            .world_mut()
            .send_event(NamedFactionLeaderDownEvent {
                faction: NamedFactionId::QingyunHunters,
                zone: "qingyun_peaks".to_string(),
            });
        missing_registry_app.update();
        assert!(
            drain_named_faction_states(&missing_registry_rx).is_empty(),
            "missing NamedFactionRegistry must skip lifecycle-triggered snapshot without consuming a partial state"
        );

        let (mut missing_matrix_app, missing_matrix_rx) = setup_app();
        missing_matrix_app.add_event::<NamedFactionLeaderDownEvent>();
        missing_matrix_app.add_event::<NamedFactionDecayEvent>();
        missing_matrix_app.insert_resource(NamedFactionRegistry::startup_default());
        missing_matrix_app.add_systems(Update, publish_named_faction_state_on_lifecycle_events);
        missing_matrix_app
            .world_mut()
            .send_event(NamedFactionDecayEvent {
                faction: NamedFactionId::CangyuanMerchants,
                final_zone: "blood_valley".to_string(),
                last_npc_count: 2,
            });
        missing_matrix_app.update();
        assert!(
            drain_named_faction_states(&missing_matrix_rx).is_empty(),
            "missing FactionRelationMatrix must skip lifecycle-triggered snapshot without emitting partial state"
        );
    }

    #[test]
    fn publish_named_faction_state_on_lifecycle_events_publishes_for_leader_down_only() {
        let (mut app, rx) = setup_app();
        app.add_event::<NamedFactionLeaderDownEvent>();
        app.add_event::<NamedFactionDecayEvent>();
        app.insert_resource(NamedFactionRegistry::startup_default());
        app.insert_resource(FactionRelationMatrix::startup_default());
        app.insert_resource(GameTick(42));
        app.add_systems(Update, publish_named_faction_state_on_lifecycle_events);

        app.world_mut().send_event(NamedFactionLeaderDownEvent {
            faction: NamedFactionId::QingyunHunters,
            zone: "qingyun_peaks".to_string(),
        });
        app.update();

        let states = drain_named_faction_states(&rx);
        assert_eq!(
            states.len(),
            1,
            "leader_down event must trigger exactly one named faction snapshot, actual {}",
            states.len()
        );
        assert_eq!(
            states[0].at_tick, 42,
            "lifecycle-triggered snapshot must use current GameTick, actual {}",
            states[0].at_tick
        );
    }

    #[test]
    fn publish_named_faction_state_on_lifecycle_events_publishes_for_decay_only() {
        let (mut app, rx) = setup_app();
        app.add_event::<NamedFactionLeaderDownEvent>();
        app.add_event::<NamedFactionDecayEvent>();
        app.insert_resource(NamedFactionRegistry::startup_default());
        app.insert_resource(FactionRelationMatrix::startup_default());
        app.insert_resource(GameTick(43));
        app.add_systems(Update, publish_named_faction_state_on_lifecycle_events);

        app.world_mut().send_event(NamedFactionDecayEvent {
            faction: NamedFactionId::CangyuanMerchants,
            final_zone: "blood_valley".to_string(),
            last_npc_count: 2,
        });
        app.update();

        let states = drain_named_faction_states(&rx);
        assert_eq!(
            states.len(),
            1,
            "decay event must trigger exactly one named faction snapshot, actual {}",
            states.len()
        );
        assert_eq!(
            states[0].at_tick, 43,
            "decay-triggered snapshot must use current GameTick, actual {}",
            states[0].at_tick
        );
    }

    #[test]
    fn publish_named_faction_state_on_lifecycle_events_defaults_tick_when_game_tick_missing() {
        let (mut app, rx) = setup_app();
        app.add_event::<NamedFactionLeaderDownEvent>();
        app.add_event::<NamedFactionDecayEvent>();
        app.insert_resource(NamedFactionRegistry::startup_default());
        app.insert_resource(FactionRelationMatrix::startup_default());
        app.add_systems(Update, publish_named_faction_state_on_lifecycle_events);

        app.world_mut().send_event(NamedFactionLeaderDownEvent {
            faction: NamedFactionId::NorthWasteDrifters,
            zone: "north_wastes".to_string(),
        });
        app.update();

        let states = drain_named_faction_states(&rx);
        assert_eq!(
            states.len(),
            1,
            "lifecycle event without GameTick must still publish one snapshot, actual {}",
            states.len()
        );
        assert_eq!(
            states[0].at_tick, 0,
            "missing GameTick must serialize lifecycle snapshot at_tick=0, actual {}",
            states[0].at_tick
        );
    }

    #[test]
    fn publish_named_faction_state_on_lifecycle_events_coalesces_both_event_types() {
        let (mut app, rx) = setup_app();
        app.add_event::<NamedFactionLeaderDownEvent>();
        app.add_event::<NamedFactionDecayEvent>();
        app.insert_resource(NamedFactionRegistry::startup_default());
        app.insert_resource(FactionRelationMatrix::startup_default());
        app.insert_resource(GameTick(99));
        app.add_systems(Update, publish_named_faction_state_on_lifecycle_events);

        app.world_mut().send_event(NamedFactionLeaderDownEvent {
            faction: NamedFactionId::QingyunHunters,
            zone: "qingyun_peaks".to_string(),
        });
        app.world_mut().send_event(NamedFactionDecayEvent {
            faction: NamedFactionId::CangyuanMerchants,
            final_zone: "blood_valley".to_string(),
            last_npc_count: 2,
        });
        app.update();

        let states = drain_named_faction_states(&rx);
        assert_eq!(
            states.len(),
            1,
            "leader_down + decay in one update must coalesce into one snapshot, actual {}",
            states.len()
        );
        assert_eq!(
            states[0].at_tick, 99,
            "coalesced snapshot must keep current tick, actual {}",
            states[0].at_tick
        );

        app.update();
        assert!(
            drain_named_faction_states(&rx).is_empty(),
            "consumed lifecycle events must not publish again on the next update"
        );
    }

    #[test]
    fn publish_named_faction_state_defaults_tick_and_serializes_statuses_and_relations() {
        let (mut app, rx) = setup_app();
        let mut registry = NamedFactionRegistry::startup_default();
        registry
            .get_mut(NamedFactionId::QingyunHunters)
            .unwrap()
            .current_npc_count = 2;
        registry
            .get_mut(NamedFactionId::CangyuanMerchants)
            .unwrap()
            .set_status(FactionStatus::Decayed);
        registry
            .get_mut(NamedFactionId::NorthWasteDrifters)
            .unwrap()
            .set_status(FactionStatus::Headless);
        let mut relation_matrix = FactionRelationMatrix::startup_default();
        relation_matrix.set(
            NamedFactionId::CangyuanMerchants,
            NamedFactionId::NorthWasteDrifters,
            crate::npc::faction::FactionRelation::Pact,
        );
        app.insert_resource(registry);
        app.insert_resource(relation_matrix);
        app.add_systems(Update, publish_named_faction_state);

        app.update();

        let states = drain_named_faction_states(&rx);
        assert_eq!(
            states.len(),
            1,
            "expected one NamedFactionStateV1 snapshot when resources are complete, actual {}",
            states.len()
        );
        let state = &states[0];
        assert_eq!(
            state.at_tick, 0,
            "missing GameTick must default named faction snapshot at_tick to 0, actual {}",
            state.at_tick
        );
        let qingyun = state
            .named_factions
            .iter()
            .find(|entry| entry.id == "qingyun_hunters")
            .expect("qingyun entry must exist");
        assert_eq!(
            (qingyun.status, qingyun.is_active),
            (FactionStatus::Active, true),
            "Active faction must serialize is_active=true, actual {:?}/{}",
            qingyun.status,
            qingyun.is_active
        );
        let north = state
            .named_factions
            .iter()
            .find(|entry| entry.id == "north_waste_drifters")
            .expect("north entry must exist");
        assert_eq!(
            (north.status, north.is_active),
            (FactionStatus::Headless, true),
            "Headless faction must serialize is_active=true, actual {:?}/{}",
            north.status,
            north.is_active
        );
        assert!(
            state
                .relation_matrix
                .iter()
                .any(|entry| entry.a == "qingyun_hunters"
                    && entry.b == "north_waste_drifters"
                    && entry.hostile),
            "QingyunHunters ↔ NorthWasteDrifters default hostile=true relation must publish"
        );
        assert!(
            state
                .relation_matrix
                .iter()
                .any(|entry| entry.a == "cangyuan_merchants"
                    && entry.b == "north_waste_drifters"
                    && !entry.hostile),
            "Pact/Neutral relations must publish hostile=false"
        );
    }

    #[test]
    fn publish_faction_state_defaults_at_tick_to_zero_without_game_tick() {
        // 边界（CodeRabbit）：缺 GameTick 资源时 at_tick 必须回退 0（current_game_tick 的
        // unwrap_or_default 分支），而非 panic / 脏值——与同文件其他 telemetry publisher 同契约。
        let (mut app, rx) = setup_app();
        let mut store = NpcDormantStore::default();
        store.snapshots.insert(
            "dormant:no_tick".to_string(),
            dormant_snapshot(
                "dormant:no_tick",
                "spawn",
                Realm::Awaken,
                EmergentGroupId(0),
            ),
        );
        store.rebuild_indexes();
        app.insert_resource(store);
        app.insert_resource(FactionStore::default());
        app.init_resource::<LastFactionCensus>();
        // 故意**不** insert GameTick。
        app.add_systems(Update, publish_faction_state);
        app.update();

        let states = drain_faction_states(&rx);
        assert_eq!(
            states.len(),
            1,
            "single living group must publish exactly one FactionStateV1, got {}",
            states.len()
        );
        assert_eq!(
            states[0].at_tick, 0,
            "without a GameTick resource, at_tick must default to 0 (current_game_tick falls back to u64::default), got {}",
            states[0].at_tick
        );
    }

    #[test]
    fn publish_faction_state_drops_on_closed_channel_without_panic() {
        // 错误分支（CodeRabbit）：outbound channel 关闭（接收端 drop）时 send 返回 SendError，
        // 系统必须 warn + 吞掉、**不 panic**（一个掉线的 Redis 桥不能拖垮整个 tick）。
        let mut app = App::new();
        let (tx_outbound, rx_outbound) = unbounded::<RedisOutbound>();
        let (_tx_inbound, rx_inbound) = unbounded();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        // 关闭接收端 → 后续 send 必返回 SendError。
        drop(rx_outbound);
        let mut store = NpcDormantStore::default();
        store.snapshots.insert(
            "dormant:dropped".to_string(),
            dormant_snapshot(
                "dormant:dropped",
                "spawn",
                Realm::Awaken,
                EmergentGroupId(0),
            ),
        );
        store.rebuild_indexes();
        app.insert_resource(store);
        app.insert_resource(FactionStore::default());
        app.init_resource::<LastFactionCensus>();
        app.insert_resource(GameTick(9));
        app.add_systems(Update, publish_faction_state);
        // 不 panic 即通过该分支核心契约；二次 update 确认稳定可重入。
        app.update();
        app.update();
    }

    #[test]
    fn publish_faction_state_emits_one_payload_per_group() {
        // happy 链路：store→system→publish。两组分别 publish 一条 FactionStateV1，
        // population / dominant_zone / region_descriptor / strongest_realm / status 全正确。
        let (mut app, rx) = setup_faction_state_app(
            vec![
                dormant_snapshot(
                    "dormant:rogue:0",
                    "spawn",
                    Realm::Awaken,
                    EmergentGroupId(0),
                ),
                dormant_snapshot(
                    "dormant:rogue:1",
                    "spawn",
                    Realm::Solidify,
                    EmergentGroupId(0),
                ),
                dormant_snapshot(
                    "dormant:rogue:2",
                    "rift_valley",
                    Realm::Induce,
                    EmergentGroupId(1),
                ),
            ],
            555,
        );
        app.update();

        let states = drain_faction_states(&rx);
        assert_eq!(
            states.len(),
            2,
            "two distinct emergent groups must each publish one FactionStateV1, got {}",
            states.len()
        );

        let g0 = &states[0];
        assert_eq!(g0.group_id, 0, "first payload must be group 0 (sorted)");
        assert_eq!(g0.kind, "faction_state", "kind tag must be faction_state");
        assert_eq!(
            g0.population, 2,
            "group 0 has 2 living members, expected population 2, got {}",
            g0.population
        );
        assert_eq!(
            g0.dominant_zone, "spawn",
            "group 0 members are both in spawn, dominant_zone must be spawn, got {}",
            g0.dominant_zone
        );
        assert_eq!(
            g0.region_descriptor, "spawn一带散修",
            "region_descriptor must be the anonymous \"{{zone}}一带散修\" descriptor, got {}",
            g0.region_descriptor
        );
        assert_eq!(
            g0.strongest_realm, "Solidify",
            "group 0's emergent strongest is the Solidify rogue, strongest_realm must be realm_to_string(Solidify)=\"Solidify\", got {}",
            g0.strongest_realm
        );
        assert_eq!(
            g0.strongest_char_id, "dormant:rogue:1",
            "strongest_char_id must point at the Solidify member, got {}",
            g0.strongest_char_id
        );
        assert_eq!(
            g0.status,
            GroupStatus::Rising,
            "with empty history (first publish tick), group 0 must be Rising, got {:?}",
            g0.status
        );
        assert_eq!(g0.at_tick, 555, "at_tick must come from GameTick(555)");

        assert_eq!(states[1].group_id, 1, "second payload must be group 1");
        assert_eq!(states[1].population, 1, "group 1 has 1 member");
    }

    #[test]
    fn publish_faction_state_updates_last_census_for_next_tick_status() {
        // 状态转换 over two ticks：第一轮（空历史）→ Rising 并写回 LastFactionCensus；
        // 第二轮人口不变 → Stable（证明历史确实被写回并用于下轮判定）。
        let (mut app, rx) = setup_faction_state_app(
            vec![
                dormant_snapshot(
                    "dormant:rogue:0",
                    "spawn",
                    Realm::Awaken,
                    EmergentGroupId(0),
                ),
                dormant_snapshot(
                    "dormant:rogue:1",
                    "spawn",
                    Realm::Awaken,
                    EmergentGroupId(0),
                ),
            ],
            10,
        );

        // 第一轮：Rising。
        app.update();
        let first = drain_faction_states(&rx);
        assert_eq!(first.len(), 1);
        assert_eq!(
            first[0].status,
            GroupStatus::Rising,
            "first tick (empty history) must be Rising, got {:?}",
            first[0].status
        );
        // 历史已写回：population 2。
        assert_eq!(
            app.world().resource::<LastFactionCensus>().0.get(&EmergentGroupId(0)),
            Some(&2),
            "after the first publish, LastFactionCensus must record group 0's population as 2 for next-tick comparison"
        );

        // 第二轮：人口不变 → Stable（依赖上一轮写回的历史）。
        app.update();
        let second = drain_faction_states(&rx);
        assert_eq!(second.len(), 1);
        assert_eq!(
            second[0].status,
            GroupStatus::Stable,
            "second tick with unchanged population (2→2) must be Stable, proving last-census history was written back and consulted, got {:?}",
            second[0].status
        );
    }

    #[test]
    fn publish_faction_state_strongest_falls_emits_waning() {
        // 强者陨落→式微 e2e：第一轮 1 Solidify 强者 + 2 Awaken（population 3, strongest Solidify,
        // Rising）。移除强者后第二轮 → population 2 < 3 → Waning，且 strongest_realm 降到 Awaken。
        let (mut app, rx) = setup_faction_state_app(
            vec![
                dormant_snapshot(
                    "dormant:rogue:0",
                    "spawn",
                    Realm::Solidify,
                    EmergentGroupId(0),
                ),
                dormant_snapshot(
                    "dormant:rogue:1",
                    "spawn",
                    Realm::Awaken,
                    EmergentGroupId(0),
                ),
                dormant_snapshot(
                    "dormant:rogue:2",
                    "spawn",
                    Realm::Awaken,
                    EmergentGroupId(0),
                ),
            ],
            20,
        );

        app.update();
        let before = drain_faction_states(&rx);
        assert_eq!(before.len(), 1);
        assert_eq!(before[0].population, 3, "before: 3 living members");
        assert_eq!(
            before[0].strongest_realm, "Solidify",
            "before: emergent strongest is Solidify"
        );
        assert_eq!(
            before[0].status,
            GroupStatus::Rising,
            "before: first tick Rising"
        );

        // 强者陨落：把 Solidify 强者移出 store（守恒：真元释放归 P2，本系统不碰 ledger）。
        {
            let mut store = app.world_mut().resource_mut::<NpcDormantStore>();
            store.snapshots.remove("dormant:rogue:0");
            store.rebuild_indexes();
        }

        app.update();
        let after = drain_faction_states(&rx);
        assert_eq!(after.len(), 1, "group still has 2 survivors");
        assert_eq!(
            after[0].population, 2,
            "after the strongest falls: population must drop 3→2, got {}",
            after[0].population
        );
        assert_eq!(
            after[0].strongest_realm, "Awaken",
            "after the strongest falls: strongest_realm must drop Solidify→Awaken (only Awaken survivors), got {}",
            after[0].strongest_realm
        );
        assert_eq!(
            after[0].status,
            GroupStatus::Waning,
            "after the strongest falls (population 3→2): status must be Waning, got {:?}",
            after[0].status
        );
    }

    #[test]
    fn publish_faction_state_empty_store_publishes_nothing() {
        // 边界：空 store → 不 publish 任何 FactionStateV1，LastFactionCensus 保持空。
        let (mut app, rx) = setup_faction_state_app(vec![], 1);
        app.update();
        let states = drain_faction_states(&rx);
        assert!(
            states.is_empty(),
            "an empty dormant store must publish no FactionStateV1, got {states:?}"
        );
        assert!(
            app.world().resource::<LastFactionCensus>().0.is_empty(),
            "with no groups, LastFactionCensus must remain empty"
        );
    }
}
