//! plan-remains-suite P0 — 世界内遗骸容器（`inventory::RemainsContainer`）同步。
//!
//! 照 `dropped_loot_sync_emit.rs` 的形状：join 时全量快照一次 + 内容真变化时
//! 广播；**不信任 Bevy 的 `is_changed()`**（PR #859 实证：无关系统对 `ResMut`/
//! 任意 mutable 访问的触碰会把变更检测污染成"每 tick 都 changed"），而是自己
//! 对内容做真 diff（`Local` 缓存上次真正广播过的快照，逐 tick 比较）。
//!
//! 与 dropped_loot 的关键差异：dropped_loot 的数据源是一个 Resource-owned
//! `HashMap`（`DroppedLootRegistry`），遗骸的数据源是散落世界各处的 ECS 实体
//! （`RemainsContainer` + `UniqueId` + `Position` + `CurrentDimension`），所以
//! 快照直接从 Query 构建，而不是从某个 Resource 读。已经被搬空 despawn
//! （`insert(Despawned)`）的遗骸实体必须被 `Without<Despawned>` 过滤掉，否则
//! 客户端会继续显示一具事实上已经不存在的遗骸。

use valence::prelude::{
    Added, Client, Despawned, Entity, Local, Position, Query, UniqueId, Username, With, Without,
};

use crate::inventory::{RemainsContainer, REMAINS_DISPLAY_NAME};
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::server_data::{RemainsEntryV1, ServerDataPayloadV1, ServerDataV1};
use crate::world::dimension::CurrentDimension;

type JoinedRemainsSyncClient<'a> = (Entity, &'a Username, &'a mut Client);
type JoinedRemainsSyncClientFilter = (With<Client>, Added<Client>);

type RemainsSnapshotQueryItem<'a> = (
    &'a UniqueId,
    &'a RemainsContainer,
    &'a Position,
    Option<&'a CurrentDimension>,
);
type RemainsSnapshotQueryFilter = Without<Despawned>;

/// 见模块文档：不能信任 `Res::is_changed()`，必须对内容做真 diff。
#[derive(Default)]
pub struct RemainsSyncCache {
    last_broadcast: Option<Vec<RemainsEntryV1>>,
}

fn remains_snapshot(
    query: &Query<RemainsSnapshotQueryItem<'_>, RemainsSnapshotQueryFilter>,
) -> Vec<RemainsEntryV1> {
    let mut entries = query
        .iter()
        .map(|(uuid, remains, position, dimension)| {
            let p = position.get();
            RemainsEntryV1 {
                remains_id: uuid.0.to_string(),
                world_pos: [p.x, p.y, p.z],
                dimension: dimension
                    .map(|d| d.0.ident_str().to_string())
                    .unwrap_or_else(|| {
                        crate::world::dimension::DimensionKind::Overworld
                            .ident_str()
                            .to_string()
                    }),
                display_name: REMAINS_DISPLAY_NAME.to_string(),
                item_count: remains.items.len() as u64,
                bone_coins: remains.bone_coins,
            }
        })
        .collect::<Vec<_>>();
    // Deterministic ordering avoids client-side churn (remains_id is a UUID string,
    // so lexicographic order is the only stable order available).
    entries.sort_by(|a, b| a.remains_id.cmp(&b.remains_id));
    entries
}

fn send_remains_sync_to_client(entity: Entity, client: &mut Client, snapshot: Vec<RemainsEntryV1>) {
    let payload = ServerDataV1::new(ServerDataPayloadV1::RemainsSync(snapshot));
    let payload_type = payload_type_label(payload.payload_type());
    let payload_bytes = match serialize_server_data_payload(&payload) {
        Ok(bytes) => bytes,
        Err(error) => {
            log_payload_build_error(payload_type, &error);
            return;
        }
    };
    send_server_data_payload(client, payload_bytes.as_slice());
    tracing::debug!(
        "[bong][network] sent {} {} payload to client entity {:?}",
        SERVER_DATA_CHANNEL,
        payload_type,
        entity,
    );
}

pub fn emit_join_remains_syncs(
    remains_q: Query<RemainsSnapshotQueryItem<'_>, RemainsSnapshotQueryFilter>,
    mut clients: Query<JoinedRemainsSyncClient<'_>, JoinedRemainsSyncClientFilter>,
) {
    if clients.iter().next().is_none() {
        return;
    }
    let snapshot = remains_snapshot(&remains_q);
    for (entity, username, mut client) in &mut clients {
        send_remains_sync_to_client(entity, &mut client, snapshot.clone());
        tracing::info!(
            "[bong][network] sent join-time remains_sync snapshot ({} entries) to {:?} (`{}`)",
            snapshot.len(),
            entity,
            username.0
        );
    }
}

/// 遗骸是世界可见的；内容真变化（新增/被搬空 despawn/部分转移）时推给所有在线 client。
/// 不使用 `Changed<RemainsContainer>` —— 那只能感知"哪些还活着的实体的组件变了"，
/// 感知不到"整具遗骸被 despawn 掉"这种数量减少的情况；内容 diff 天然覆盖两种变化。
pub fn emit_changed_remains_syncs(
    remains_q: Query<RemainsSnapshotQueryItem<'_>, RemainsSnapshotQueryFilter>,
    mut cache: Local<RemainsSyncCache>,
    mut clients: Query<JoinedRemainsSyncClient<'_>, With<Client>>,
) {
    let snapshot = remains_snapshot(&remains_q);

    // 本系统第一次跑：只记基线，不广播——早已连接的 client 会被 `emit_join_remains_syncs`
    // （Added<Client>）单独喂过一次全量快照，这里再发一遍是纯重复。
    let Some(last_broadcast) = cache.last_broadcast.as_ref() else {
        cache.last_broadcast = Some(snapshot);
        return;
    };

    if last_broadcast == &snapshot {
        return;
    }
    let entry_count = snapshot.len();
    cache.last_broadcast = Some(snapshot.clone());

    let mut broadcast_to = 0usize;
    for (entity, _username, mut client) in &mut clients {
        send_remains_sync_to_client(entity, &mut client, snapshot.clone());
        broadcast_to += 1;
    }
    tracing::info!(
        "[bong][network] remains_sync content changed ({entry_count} entries) — broadcast to {broadcast_to} connected clients",
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::inventory::{ItemInstance, ItemRarity, RemainsItemRecord};
    use valence::prelude::{App, DVec3, Entity as ValenceEntity, EntityLayerId, Update, World};
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::{create_mock_client, MockClientHelper};

    fn setup_app() -> App {
        let mut app = App::new();
        app.add_systems(
            Update,
            (emit_join_remains_syncs, emit_changed_remains_syncs),
        );
        app
    }

    fn spawn_client_actor(app: &mut App, username: &str) -> (ValenceEntity, MockClientHelper) {
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

    fn collect_remains_syncs(helper: &mut MockClientHelper) -> Vec<Vec<RemainsEntryV1>> {
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
            if let ServerDataPayloadV1::RemainsSync(entries) = payload.payload {
                payloads.push(entries);
            }
        }
        payloads
    }

    fn spawn_remains(
        world: &mut World,
        layer: ValenceEntity,
        pos: [f64; 3],
        item_count: usize,
        bone_coins: u64,
    ) -> ValenceEntity {
        spawn_remains_with_uuid(
            world,
            layer,
            UniqueId::default(),
            pos,
            item_count,
            bone_coins,
        )
    }

    fn spawn_remains_with_uuid(
        world: &mut World,
        layer: ValenceEntity,
        uuid: UniqueId,
        pos: [f64; 3],
        item_count: usize,
        bone_coins: u64,
    ) -> ValenceEntity {
        let items = (0..item_count)
            .map(|idx| RemainsItemRecord {
                source_container_id: "test_fixture".to_string(),
                source_row: 0,
                source_col: idx as u8,
                item: ItemInstance {
                    instance_id: idx as u64,
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
            })
            .collect();
        let entry_entity = world.spawn_empty().id();
        world
            .spawn((
                EntityLayerId(layer),
                uuid,
                Position::new(DVec3::new(pos[0], pos[1], pos[2])),
                CurrentDimension(crate::world::dimension::DimensionKind::Overworld),
                RemainsContainer {
                    items,
                    bone_coins,
                    player_list_entry: entry_entity,
                },
            ))
            .id()
    }

    #[test]
    fn join_sends_full_snapshot_exactly_once() {
        let mut app = setup_app();
        let (_entity, mut helper) = spawn_client_actor(&mut app, "Joiner");
        let layer = app.world_mut().spawn_empty().id();
        spawn_remains(app.world_mut(), layer, [4.0, 64.0, 4.0], 2, 7);

        app.update();
        flush_client_packets(&mut app);

        let syncs = collect_remains_syncs(&mut helper);
        assert_eq!(
            syncs.len(),
            1,
            "期望 join 时恰好收到一条 remains_sync（emit_join 的 Added<Client> 只在这一 tick \
             命中一次，emit_changed 应该识别出这就是刚广播过的同一份快照而不重复发）；实际收到 {} 条",
            syncs.len()
        );
        assert_eq!(
            syncs[0].len(),
            1,
            "期望这条快照里恰好含 1 具遗骸；实际 {} 具",
            syncs[0].len()
        );
        assert_eq!(syncs[0][0].item_count, 2);
        assert_eq!(syncs[0][0].bone_coins, 7);
        assert_eq!(syncs[0][0].display_name, REMAINS_DISPLAY_NAME);
        assert_eq!(syncs[0][0].dimension, "minecraft:overworld");
    }

    #[test]
    fn unchanged_registry_does_not_resend_across_ticks() {
        let mut app = setup_app();
        let (_entity, mut helper) = spawn_client_actor(&mut app, "Idler");

        app.update();
        flush_client_packets(&mut app);
        let _ = helper.collect_received();

        for _ in 0..5 {
            app.update();
            flush_client_packets(&mut app);
        }

        let syncs = collect_remains_syncs(&mut helper);
        assert_eq!(
            syncs.len(),
            0,
            "期望内容不变的连续 tick 不重发 remains_sync；实际收到 {} 条",
            syncs.len()
        );
    }

    #[test]
    fn entry_added_then_despawned_each_broadcast_exactly_once() {
        let mut app = setup_app();
        let (_entity, mut helper) = spawn_client_actor(&mut app, "Watcher");
        let layer = app.world_mut().spawn_empty().id();

        app.update();
        flush_client_packets(&mut app);
        let _ = helper.collect_received();

        let remains_entity = spawn_remains(app.world_mut(), layer, [1.0, 64.0, 1.0], 1, 0);
        app.update();
        flush_client_packets(&mut app);
        let after_add = collect_remains_syncs(&mut helper);
        assert_eq!(
            after_add.len(),
            1,
            "期望新增一具遗骸后恰好广播一次；实际 {} 次",
            after_add.len()
        );
        assert_eq!(after_add[0].len(), 1);

        app.update();
        flush_client_packets(&mut app);
        let idle_after_add = collect_remains_syncs(&mut helper);
        assert_eq!(
            idle_after_add.len(),
            0,
            "期望新增之后、下一次真正变化之前的 tick 不重发；实际 {} 条",
            idle_after_add.len()
        );

        app.world_mut().entity_mut(remains_entity).insert(Despawned);
        app.update();
        flush_client_packets(&mut app);
        let after_despawn = collect_remains_syncs(&mut helper);
        assert_eq!(
            after_despawn.len(),
            1,
            "期望 despawn 后恰好再广播一次（空快照）；实际 {} 次",
            after_despawn.len()
        );
        assert!(
            after_despawn[0].is_empty(),
            "期望 despawn 后广播的快照不含任何 entry；实际 {:?}",
            after_despawn[0]
        );
    }

    #[test]
    fn partial_loot_broadcasts_updated_snapshot() {
        let mut app = setup_app();
        let (_entity, mut helper) = spawn_client_actor(&mut app, "PartialWatcher");
        let layer = app.world_mut().spawn_empty().id();
        let remains_entity = spawn_remains(app.world_mut(), layer, [1.0, 64.0, 1.0], 2, 7);

        app.update();
        flush_client_packets(&mut app);
        let _ = helper.collect_received();

        {
            let mut remains = app
                .world_mut()
                .get_mut::<RemainsContainer>(remains_entity)
                .expect("fixture remains should exist");
            remains.items.pop();
            remains.bone_coins = 3;
        }
        app.update();
        flush_client_packets(&mut app);

        let syncs = collect_remains_syncs(&mut helper);
        assert_eq!(
            syncs.len(),
            1,
            "部分拾取后 RemainsContainer 仍存活但内容已变，应恰好广播一次；实际 {} 次",
            syncs.len()
        );
        assert_eq!(
            syncs[0][0].item_count, 1,
            "部分拾取后 item_count 应更新为 1"
        );
        assert_eq!(
            syncs[0][0].bone_coins, 3,
            "部分拾取后 bone_coins 应更新为 3"
        );
    }

    #[test]
    fn snapshot_orders_multiple_remains_by_id() {
        let mut app = setup_app();
        let (_entity, mut helper) = spawn_client_actor(&mut app, "SortWatcher");
        let layer = app.world_mut().spawn_empty().id();
        let later = uuid::Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb").unwrap();
        let earlier = uuid::Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();
        spawn_remains_with_uuid(
            app.world_mut(),
            layer,
            UniqueId(later),
            [4.0, 64.0, 4.0],
            1,
            0,
        );
        spawn_remains_with_uuid(
            app.world_mut(),
            layer,
            UniqueId(earlier),
            [1.0, 64.0, 1.0],
            1,
            0,
        );

        app.update();
        flush_client_packets(&mut app);

        let syncs = collect_remains_syncs(&mut helper);
        assert_eq!(syncs.len(), 1, "join 快照应只发送一次");
        assert_eq!(
            syncs[0]
                .iter()
                .map(|entry| entry.remains_id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
                "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb",
            ],
            "remains_snapshot 必须按 remains_id 字典序稳定排序，避免客户端抖动"
        );
    }

    #[test]
    fn content_change_broadcasts_to_all_connected_clients() {
        let mut app = setup_app();
        let (_first, mut first_helper) = spawn_client_actor(&mut app, "WatcherA");
        let (_second, mut second_helper) = spawn_client_actor(&mut app, "WatcherB");
        let layer = app.world_mut().spawn_empty().id();

        app.update();
        flush_client_packets(&mut app);
        let _ = first_helper.collect_received();
        let _ = second_helper.collect_received();

        spawn_remains(app.world_mut(), layer, [1.0, 64.0, 1.0], 1, 0);
        app.update();
        flush_client_packets(&mut app);

        let first_syncs = collect_remains_syncs(&mut first_helper);
        let second_syncs = collect_remains_syncs(&mut second_helper);
        assert_eq!(
            first_syncs.len(),
            1,
            "内容变化后第一个在线 client 应收到一次 remains_sync；实际 {} 次",
            first_syncs.len()
        );
        assert_eq!(
            second_syncs.len(),
            1,
            "内容变化后第二个在线 client 应收到一次 remains_sync；实际 {} 次",
            second_syncs.len()
        );
        assert_eq!(first_syncs[0].len(), 1);
        assert_eq!(second_syncs[0].len(), 1);
    }
}
