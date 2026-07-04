use valence::prelude::{Added, Client, Entity, Local, Query, Username, With};

use crate::inventory::{dropped_loot_snapshot, DroppedLootEntry, DroppedLootRegistry};
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::inventory_snapshot_emit::item_view_from_instance;
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::server_data::{DroppedLootEntryV1, ServerDataPayloadV1, ServerDataV1};

type JoinedDropSyncClient<'a> = (Entity, &'a Username, &'a mut Client);
type JoinedDropSyncClientFilter = (With<Client>, Added<Client>);

/// bughunt 实证：`Res<DroppedLootRegistry>::is_changed()` 不可信作为「掉落物真的变了」
/// 的判据——很多和掉落物毫无关系的系统（典型案例：`world::events::tick_active_events`，
/// 每 tick 无条件跑，把 `Option<ResMut<DroppedLootRegistry>>` `.as_deref_mut()` 传给
/// `GameEventState::tick`）只要触碰一下 `ResMut` 的 `DerefMut` 就会把这个 Resource 标脏，
/// 哪怕本 tick 一个掉落物都没变。于是旧版 `emit_changed_dropped_loot_syncs` 每 tick
/// （~20Hz）都误判"changed"，给每个在线 client 重发一次全量快照，把带宽和日志都刷爆
/// （实证：玩家离线后仍在发，因为判据从来就没跟"内容是否真变化"挂钩）。
///
/// 修法：不再信任 Bevy 的变更检测，自己对内容做真 diff（与 `carrier_state_emit` /
/// `woliu_state_emit` 等既有 emit 系统同款套路：`Local` 缓存上次真正广播过的快照，
/// 逐 tick 比较）。
#[derive(Default)]
pub struct DroppedLootSyncCache {
    last_broadcast: Option<Vec<DroppedLootEntry>>,
}

pub fn send_dropped_loot_sync_to_client(
    entity: Entity,
    client: &mut Client,
    registry: &DroppedLootRegistry,
) {
    let drops = dropped_loot_snapshot(registry)
        .into_iter()
        .map(to_wire_entry)
        .collect::<Vec<_>>();

    let payload = ServerDataV1::new(ServerDataPayloadV1::DroppedLootSync(drops));
    let payload_type = payload_type_label(payload.payload_type());
    let payload_bytes = match serialize_server_data_payload(&payload) {
        Ok(bytes) => bytes,
        Err(error) => {
            log_payload_build_error(payload_type, &error);
            return;
        }
    };
    send_server_data_payload(client, payload_bytes.as_slice());
    // per-send 粒度降级为 debug——真正值得 INFO 的是"join 时发了一次"/"内容真变化时广播了
    // 一次"这两个稀有事件（在各自调用点单独打一条 INFO），而不是这里每次发送都打一条。
    tracing::debug!(
        "[bong][network] sent {} {} payload to client entity {:?}",
        SERVER_DATA_CHANNEL,
        payload_type,
        entity,
    );
}

#[allow(clippy::type_complexity)]
pub fn emit_join_dropped_loot_syncs(
    registry: valence::prelude::Res<DroppedLootRegistry>,
    mut clients: Query<JoinedDropSyncClient<'_>, JoinedDropSyncClientFilter>,
) {
    for (entity, username, mut client) in &mut clients {
        send_dropped_loot_sync_to_client(entity, &mut client, &registry);
        tracing::info!(
            "[bong][network] sent join-time dropped_loot_sync snapshot to {:?} (`{}`)",
            entity,
            username.0
        );
    }
}

/// Dropped loot is world-visible. Whenever the registry's *actual content* changes,
/// push a fresh snapshot to all already-connected clients (join-time sync is handled
/// separately by [`emit_join_dropped_loot_syncs`]).
///
/// Deliberately does **not** use `Res::is_changed()` — see [`DroppedLootSyncCache`] doc
/// for why that change-detection signal is contaminated by unrelated systems and would
/// fire every tick regardless of whether any drop actually changed.
pub fn emit_changed_dropped_loot_syncs(
    registry: valence::prelude::Res<DroppedLootRegistry>,
    mut cache: Local<DroppedLootSyncCache>,
    mut clients: Query<JoinedDropSyncClient<'_>, With<Client>>,
) {
    let snapshot = dropped_loot_snapshot(&registry);

    // 本系统第一次跑（还没有基线可比）：只默默记基线，不广播。任何在这一 tick 就已连接的
    // client 早就会被 `emit_join_dropped_loot_syncs`（Added<Client>）单独喂过一次全量快照，
    // 这里再发一遍是纯重复——否则每个玩家 join 都会收到两条内容一样的 dropped_loot_sync
    // （一条来自 join 路径，一条来自"从 None 变成当前内容"的假 diff）。
    let Some(last_broadcast) = cache.last_broadcast.as_ref() else {
        cache.last_broadcast = Some(snapshot);
        return;
    };

    if last_broadcast == &snapshot {
        return;
    }
    let entry_count = snapshot.len();
    cache.last_broadcast = Some(snapshot);

    let mut broadcast_to = 0usize;
    for (entity, _username, mut client) in &mut clients {
        send_dropped_loot_sync_to_client(entity, &mut client, &registry);
        broadcast_to += 1;
    }
    tracing::info!(
        "[bong][network] dropped_loot_sync content changed ({entry_count} entries) — broadcast to {broadcast_to} connected clients",
    );
}

fn to_wire_entry(entry: DroppedLootEntry) -> DroppedLootEntryV1 {
    DroppedLootEntryV1 {
        instance_id: entry.instance_id,
        source_container_id: entry.source_container_id,
        source_row: u64::from(entry.source_row),
        source_col: u64::from(entry.source_col),
        world_pos: entry.world_pos,
        item: item_view_from_instance(&entry.item),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::inventory::{ItemInstance, ItemRarity};
    use crate::world::dimension::DimensionKind;
    use valence::prelude::{App, Position, Update};
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::{create_mock_client, MockClientHelper};

    fn setup_app() -> App {
        let mut app = App::new();
        app.insert_resource(DroppedLootRegistry::default());
        app.add_systems(
            Update,
            (
                emit_join_dropped_loot_syncs,
                emit_changed_dropped_loot_syncs,
            ),
        );
        app
    }

    fn spawn_client_actor(app: &mut App, username: &str) -> (Entity, MockClientHelper) {
        let (mut client_bundle, helper) = create_mock_client(username);
        client_bundle.player.position = Position::new([8.0, 66.0, 8.0]);
        let entity = app.world_mut().spawn(client_bundle).id();
        (entity, helper)
    }

    fn flush_client_packets(app: &mut App) {
        let world = app.world_mut();
        let mut query = world.query::<&mut Client>();
        for mut client in query.iter_mut(world) {
            client
                .flush_packets()
                .expect("mock client packets should flush successfully");
        }
    }

    /// 收集本次 `collect_received()` 里所有 `bong:server_data` 的 DroppedLootSync payload
    /// （每个都是一次真实的网络发送——测试通过数它们的数量来锁"发没发/发几次"）。
    fn collect_dropped_loot_syncs(helper: &mut MockClientHelper) -> Vec<Vec<DroppedLootEntryV1>> {
        let mut payloads = Vec::new();
        for frame in helper.collect_received().0 {
            let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                continue;
            };
            if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                continue;
            }
            let payload: ServerDataV1 = serde_json::from_slice(packet.data.0 .0)
                .expect("server_data payload should decode");
            if let ServerDataPayloadV1::DroppedLootSync(entries) = payload.payload {
                payloads.push(entries);
            }
        }
        payloads
    }

    fn make_entry(instance_id: u64) -> DroppedLootEntry {
        DroppedLootEntry {
            instance_id,
            source_container_id: "test_fixture".to_string(),
            source_row: 0,
            source_col: 0,
            world_pos: [1.0, 64.0, 1.0],
            dimension: DimensionKind::Overworld,
            item: ItemInstance {
                instance_id,
                template_id: "starter_talisman".to_string(),
                display_name: "启程护符".to_string(),
                grid_w: 1,
                grid_h: 1,
                weight: 0.2,
                rarity: ItemRarity::Common,
                description: "fixture".to_string(),
                stack_count: 1,
                spirit_quality: 0.5,
                durability: 1.0,
                freshness: None,
                mineral_id: None,
                charges: None,
                forge_quality: None,
                forge_color: None,
                forge_side_effects: Vec::new(),
                forge_achieved_tier: None,
                alchemy: None,
                lingering_owner_qi: None,
            },
        }
    }

    #[test]
    fn join_sends_full_snapshot_exactly_once() {
        // Registry 里已经有一条掉落物；client 加入时应该拿到含它的全量快照——且只这一条，
        // 不应该因为 emit_join + emit_changed 两个系统都在跑而重复发两条一样的内容。
        let mut app = setup_app();
        {
            let mut registry = app.world_mut().resource_mut::<DroppedLootRegistry>();
            registry.entries.insert(1, make_entry(1));
        }
        let (_entity, mut helper) = spawn_client_actor(&mut app, "Joiner");

        app.update();
        flush_client_packets(&mut app);

        let syncs = collect_dropped_loot_syncs(&mut helper);
        assert_eq!(
            syncs.len(),
            1,
            "期望 join 时恰好收到一条 dropped_loot_sync（emit_join 的 Added<Client> 只在这一 tick 命中一次，\
             emit_changed 应该识别出这就是刚广播过的同一份快照而不重复发）；实际收到 {} 条",
            syncs.len()
        );
        assert_eq!(
            syncs[0].len(),
            1,
            "期望这条快照里恰好含 1 个 entry（registry 里预置的那条）；实际 {} 个",
            syncs[0].len()
        );
        assert_eq!(syncs[0][0].instance_id, 1);
    }

    #[test]
    fn unchanged_registry_does_not_resend_across_ticks() {
        // 这是 Bug 3 的核心回归：registry 内容完全没变的情况下，连续多个 tick 都不应该
        // 重复收到 dropped_loot_sync——修复前 `Res::is_changed()` 会被无关系统的
        // `.as_deref_mut()` 污染成每 tick 都 true，导致这里每 tick 都重发一次全量快照。
        let mut app = setup_app();
        let (_entity, mut helper) = spawn_client_actor(&mut app, "Idler");

        // 第一个 tick：join-time 快照（空 registry），之后清空已收帧。
        app.update();
        flush_client_packets(&mut app);
        let _ = helper.collect_received();

        // 后续 5 个 tick 完全没有触碰 registry。
        for _ in 0..5 {
            app.update();
            flush_client_packets(&mut app);
        }

        let syncs = collect_dropped_loot_syncs(&mut helper);
        assert_eq!(
            syncs.len(),
            0,
            "期望内容不变的连续 tick 不重发 dropped_loot_sync 因为 emit_changed 现在按内容真 diff \
             而非 Bevy 的 `is_changed()`（后者会被其他系统对 ResMut 的无条件 deref_mut 污染）；\
             实际收到 {} 条",
            syncs.len()
        );
    }

    #[test]
    fn entry_added_then_removed_each_broadcast_exactly_once() {
        // 新增 → 恰好一次广播；移除 → 恰好再一次广播；期间任何"没变化"的 tick 都不应该多发。
        let mut app = setup_app();
        let (_entity, mut helper) = spawn_client_actor(&mut app, "Watcher");

        // Tick 1：join-time 快照 + 首次"是否有内容"评估，先清空不关心。
        app.update();
        flush_client_packets(&mut app);
        let _ = helper.collect_received();

        // 新增一条掉落物。
        {
            let mut registry = app.world_mut().resource_mut::<DroppedLootRegistry>();
            registry.entries.insert(42, make_entry(42));
        }
        app.update();
        flush_client_packets(&mut app);
        let after_add = collect_dropped_loot_syncs(&mut helper);
        assert_eq!(
            after_add.len(),
            1,
            "期望新增一条掉落物后恰好广播一次；实际 {} 次",
            after_add.len()
        );
        assert_eq!(after_add[0].len(), 1);
        assert_eq!(after_add[0][0].instance_id, 42);

        // 紧接着两个 tick 完全没变化——不应该再收到任何 sync。
        app.update();
        flush_client_packets(&mut app);
        app.update();
        flush_client_packets(&mut app);
        let idle_after_add = collect_dropped_loot_syncs(&mut helper);
        assert_eq!(
            idle_after_add.len(),
            0,
            "期望新增之后、下一次真正变化之前的 tick 不重发；实际 {} 条",
            idle_after_add.len()
        );

        // 移除该条掉落物。
        {
            let mut registry = app.world_mut().resource_mut::<DroppedLootRegistry>();
            registry.entries.remove(&42);
        }
        app.update();
        flush_client_packets(&mut app);
        let after_remove = collect_dropped_loot_syncs(&mut helper);
        assert_eq!(
            after_remove.len(),
            1,
            "期望移除该条掉落物后恰好再广播一次（空快照）；实际 {} 次",
            after_remove.len()
        );
        assert!(
            after_remove[0].is_empty(),
            "期望移除后广播的快照不含任何 entry；实际 {:?}",
            after_remove[0]
        );
    }
}
