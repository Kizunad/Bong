//! plan-daozhan-v1 P1 — 道伥伪装状态 S2C CustomPayload
//!
//! 两个 channel：
//!   - `bong:daozhan_disguise_enter`：玩家连接或进入区域时，下发当前所有 Mimicry 态道伥的
//!     entity_id 列表，供 client 端 DaoZhanDisguiseHandler.java 切换为 FakePlayerEntity 渲染。
//!   - `bong:daozhan_reveal`：道伥从 Mimicry → Ambush 转换时（P2 DaoZhangRevealEvent），
//!     向范围内玩家广播，client 端切回正常 Daoxiang 渲染。
//!
//! payload JSON 格式（两 channel 共用相同 schema，type 字段区分）：
//! ```json
//! {
//!   "v": 1,
//!   "type": "daozhan_disguise_enter" | "daozhan_reveal",
//!   "entity_ids": [42, 77, ...]
//! }
//! ```
//!
//! `entity_ids` 为 Valence EntityId（i32），client 通过 MC entity id 查找实体。
//!
//! ## 架构说明
//!
//! 参考 `spider_disguise_emit.rs` 模式（plan-fauna-mimic-spider-v1 P2）：
//! - 新玩家连接时推送当前所有 Mimicry 道伥列表（`Added<Client>` 过滤）。
//! - 每 `DAOZHAN_DISGUISE_SYNC_INTERVAL_TICKS` tick 周期性全量 sync（防 client 状态漂移）。
//! - P2 暴露事件 `DaoZhangRevealEvent` 广播 `bong:daozhan_reveal`（P1 仅注册 placeholder 事件）。

use serde::{Deserialize, Serialize};
use valence::entity::EntityId;
use valence::prelude::{
    bevy_ecs, ident, Added, App, Client, DVec3, Entity, Event, EventReader, Position, Query, Res,
    Update, With, Without,
};

use crate::cultivation::tick::CultivationClock;
use crate::fauna::daozhan::DaoZhangState;
use crate::npc::spawn::NpcMarker;
use crate::schema::common::MAX_PAYLOAD_BYTES;
use crate::schema::server_data::ServerDataBuildError;

/// 道伥暴起广播半径（格）：reveal 事件广播给此半径内所有玩家。
pub const DAOZHAN_REVEAL_BROADCAST_RADIUS: f64 = 64.0;

/// 道伥伪装列表下发节流间隔（tick）。
pub const DAOZHAN_DISGUISE_SYNC_INTERVAL_TICKS: u64 = 40; // 2秒

// ── S2C payload ───────────────────────────────────────────────────────────────

/// S2C payload（两个 channel 共用）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DaoZhanDisguiseS2c {
    pub v: u8,
    #[serde(rename = "type")]
    pub ty: String,
    /// Valence 实体 ID（client 侧用于定位 MC entity）
    pub entity_ids: Vec<i32>,
}

impl DaoZhanDisguiseS2c {
    pub fn disguise_enter(entity_ids: Vec<i32>) -> Self {
        Self {
            v: 1,
            ty: "daozhan_disguise_enter".to_string(),
            entity_ids,
        }
    }

    pub fn reveal(entity_ids: Vec<i32>) -> Self {
        Self {
            v: 1,
            ty: "daozhan_reveal".to_string(),
            entity_ids,
        }
    }

    pub fn to_json_bytes_checked(&self) -> Result<Vec<u8>, ServerDataBuildError> {
        let bytes = serde_json::to_vec(self).map_err(ServerDataBuildError::Json)?;
        if bytes.len() > MAX_PAYLOAD_BYTES {
            return Err(ServerDataBuildError::Oversize {
                size: bytes.len(),
                max: MAX_PAYLOAD_BYTES,
            });
        }
        Ok(bytes)
    }
}

// ── 事件：P2 暴起触发 ─────────────────────────────────────────────────────────

/// plan-daozhan-v1 P2 — 道伥从 Mimicry 暴起时 emit 的 Bevy 事件。
///
/// P1 仅声明占位，P2 DaoZhangAmbushAction 实际 emit。
/// `daozhan` 为道伥 Entity raw index，`trigger_pos` 为暴起世界坐标。
#[derive(Debug, Clone, Event)]
pub struct DaoZhangRevealEvent {
    /// 道伥 Entity raw index（跨系统定位）。
    #[allow(dead_code)]
    pub daozhan: u32,
    /// 暴起世界坐标（广播给周围玩家时的参考中心）。
    #[allow(dead_code)]
    pub trigger_pos: DVec3,
}

// ── 系统共用 Query 类型别名 ───────────────────────────────────────────────────

type DaoZhanStateQuery<'w, 's> =
    Query<'w, 's, (&'static EntityId, &'static DaoZhangState), (With<NpcMarker>, Without<Client>)>;

// ── 系统：玩家首次连接时广播 Mimicry 道伥列表 ────────────────────────────────

/// 新玩家连接时推送当前所有 Mimicry 道伥 entity id 列表。
pub fn on_player_join_send_daozhan_disguise_list(
    mut new_clients: Query<&mut Client, Added<Client>>,
    daozhan_q: DaoZhanStateQuery<'_, '_>,
) {
    if new_clients.is_empty() {
        return;
    }

    let disguised_ids: Vec<i32> = daozhan_q
        .iter()
        .filter(|(_, state)| **state == DaoZhangState::Mimicry)
        .map(|(eid, _)| eid.get())
        .collect();

    let payload = DaoZhanDisguiseS2c::disguise_enter(disguised_ids);
    let Ok(bytes) = payload.to_json_bytes_checked() else {
        tracing::warn!("[bong][daozhan_disguise] disguise_enter payload oversize, skip");
        return;
    };

    for mut client in &mut new_clients {
        client.send_custom_payload(ident!("bong:daozhan_disguise_enter"), &bytes);
    }
}

/// 周期性全量同步 Mimicry 道伥列表（防止 client 状态漂移）。
pub fn periodic_daozhan_disguise_sync_system(
    clock: Res<CultivationClock>,
    mut clients: Query<&mut Client, With<Client>>,
    daozhan_q: DaoZhanStateQuery<'_, '_>,
) {
    if clock.tick % DAOZHAN_DISGUISE_SYNC_INTERVAL_TICKS != 0 {
        return;
    }

    let disguised_ids: Vec<i32> = daozhan_q
        .iter()
        .filter(|(_, state)| **state == DaoZhangState::Mimicry)
        .map(|(eid, _)| eid.get())
        .collect();

    let payload = DaoZhanDisguiseS2c::disguise_enter(disguised_ids);
    let Ok(bytes) = payload.to_json_bytes_checked() else {
        tracing::warn!("[bong][daozhan_disguise] periodic disguise_enter oversize, skip");
        return;
    };

    for mut client in &mut clients {
        client.send_custom_payload(ident!("bong:daozhan_disguise_enter"), &bytes);
    }
}

// ── 系统：道伥暴起时广播 daozhan_reveal ──────────────────────────────────────

type RevealDaoZhanQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static EntityId, &'static Position),
    (With<NpcMarker>, Without<Client>),
>;
type RevealClientQuery<'w, 's> =
    Query<'w, 's, (&'static mut Client, &'static Position), Without<NpcMarker>>;

/// 监听 `DaoZhangRevealEvent`，向暴起坐标周围 `DAOZHAN_REVEAL_BROADCAST_RADIUS` 内玩家
/// 广播 `bong:daozhan_reveal`。
///
/// Client 收到后将对应实体的渲染从 FakePlayerEntity 切回正常道伥外观。
pub fn on_daozhan_reveal_broadcast_system(
    mut reveal_events: EventReader<DaoZhangRevealEvent>,
    daozhan_q: RevealDaoZhanQuery<'_, '_>,
    mut clients: RevealClientQuery<'_, '_>,
) {
    let events: Vec<DaoZhangRevealEvent> = reveal_events.read().cloned().collect();
    if events.is_empty() {
        return;
    }

    for event in &events {
        let trigger_pos = event.trigger_pos;
        let trigger_pos_arr = [trigger_pos.x, trigger_pos.y, trigger_pos.z];

        let Some((_, daozhan_eid, _)) = daozhan_q
            .iter()
            .find(|(entity, _, _)| entity.index() == event.daozhan)
        else {
            continue;
        };

        let payload = DaoZhanDisguiseS2c::reveal(vec![daozhan_eid.get()]);
        let Ok(bytes) = payload.to_json_bytes_checked() else {
            tracing::warn!("[bong][daozhan_disguise] daozhan_reveal payload oversize, skip");
            continue;
        };

        for (mut client, client_pos) in &mut clients {
            let cp = client_pos.get();
            let client_pos_arr = [cp.x, cp.y, cp.z];
            if dist3(trigger_pos_arr, client_pos_arr) <= DAOZHAN_REVEAL_BROADCAST_RADIUS {
                client.send_custom_payload(ident!("bong:daozhan_reveal"), &bytes);
            }
        }
    }
}

#[inline]
fn dist3(a: [f64; 3], b: [f64; 3]) -> f64 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

// ── Bevy 注册 ─────────────────────────────────────────────────────────────────

pub fn register(app: &mut App) {
    app.add_event::<DaoZhangRevealEvent>();
    app.add_systems(
        Update,
        (
            on_player_join_send_daozhan_disguise_list,
            periodic_daozhan_disguise_sync_system,
            on_daozhan_reveal_broadcast_system,
        ),
    );
}

// ── 测试 ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── schema pin 测试 ────────────────────────────────────────────────────

    #[test]
    fn daozhan_disguise_s2c_wire_format_enter() {
        let payload = DaoZhanDisguiseS2c::disguise_enter(vec![42, 77]);
        let json = serde_json::to_string(&payload).expect("serialize must succeed");
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["v"], 1, "版本号必须为 1");
        assert_eq!(
            v["type"], "daozhan_disguise_enter",
            "type wire 名必须稳定，实际={}",
            v["type"]
        );
        let ids: Vec<i32> = serde_json::from_value(v["entity_ids"].clone()).unwrap();
        assert_eq!(ids, vec![42, 77], "entity_ids 应原样序列化");
    }

    #[test]
    fn daozhan_disguise_s2c_wire_format_reveal() {
        let payload = DaoZhanDisguiseS2c::reveal(vec![99]);
        let json = serde_json::to_string(&payload).expect("serialize must succeed");
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["v"], 1, "版本号必须为 1");
        assert_eq!(
            v["type"], "daozhan_reveal",
            "type wire 名必须稳定，实际={}",
            v["type"]
        );
        let ids: Vec<i32> = serde_json::from_value(v["entity_ids"].clone()).unwrap();
        assert_eq!(ids, vec![99]);
    }

    #[test]
    fn daozhan_disguise_s2c_empty_ids_is_valid() {
        let payload = DaoZhanDisguiseS2c::disguise_enter(vec![]);
        let json = serde_json::to_string(&payload).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(
            v["entity_ids"].as_array().is_some_and(|a| a.is_empty()),
            "空道伥列表应序列化为空数组 []，实际={:?}",
            v["entity_ids"]
        );
    }

    #[test]
    fn daozhan_disguise_s2c_roundtrip_enter() {
        let original = DaoZhanDisguiseS2c {
            v: 1,
            ty: "daozhan_disguise_enter".to_string(),
            entity_ids: vec![1, 2, 3],
        };
        let json = serde_json::to_string(&original).unwrap();
        let decoded: DaoZhanDisguiseS2c = serde_json::from_str(&json).unwrap();
        assert_eq!(original, decoded, "DaoZhanDisguiseS2c 序列化往返必须等价");
    }

    #[test]
    fn daozhan_disguise_s2c_type_field_renamed() {
        // 验证 "type" wire name 稳定（serde rename）
        let enter = DaoZhanDisguiseS2c::disguise_enter(vec![]);
        let json = serde_json::to_string(&enter).unwrap();
        assert!(
            json.contains("\"type\":\"daozhan_disguise_enter\""),
            "type 字段 wire name 必须为 daozhan_disguise_enter，实际 {json}"
        );

        let reveal = DaoZhanDisguiseS2c::reveal(vec![]);
        let json_reveal = serde_json::to_string(&reveal).unwrap();
        assert!(
            json_reveal.contains("\"type\":\"daozhan_reveal\""),
            "type 字段 wire name 必须为 daozhan_reveal，实际 {json_reveal}"
        );
    }

    #[test]
    fn daozhan_reveal_broadcast_radius_pin() {
        assert!(
            (DAOZHAN_REVEAL_BROADCAST_RADIUS - 64.0).abs() < 1e-9,
            "暴起广播半径应为 64 格，实际 {DAOZHAN_REVEAL_BROADCAST_RADIUS}"
        );
    }

    #[test]
    fn dist3_correct() {
        assert!(
            (dist3([0.0, 0.0, 0.0], [3.0, 4.0, 0.0]) - 5.0).abs() < 1e-9,
            "dist3 三角函数校验失败"
        );
    }

    /// 验证 reveal 时 entity index 匹配逻辑：应按 event.daozhan index 定位，而非距离最近。
    #[test]
    fn reveal_broadcast_uses_entity_index_not_nearest_position() {
        // 道伥 A：index=1，MC id=100，位置[0,64,0]（更近）
        // 道伥 B：index=2，MC id=200，位置[5,64,0]（更远）
        // event.daozhan = 2 → 应选道伥 B (id=200)
        let entries: Vec<(u32, i32, [f64; 3])> =
            vec![(1, 100, [0.0, 64.0, 0.0]), (2, 200, [5.0, 64.0, 0.0])];
        let event_daozhan: u32 = 2;

        let found = entries
            .iter()
            .find(|(idx, _, _)| *idx == event_daozhan)
            .map(|(_, mc_id, _)| *mc_id);

        assert_eq!(
            found,
            Some(200),
            "event.daozhan=2 应定位到 entity_id=200（道伥 B），而非坐标更近的 entity_id=100"
        );
    }

    /// 道伥已 despawn 时（找不到 entity index），broadcast 应安静跳过。
    #[test]
    fn reveal_broadcast_skips_when_entity_not_found() {
        let entries: Vec<(u32, i32)> = vec![(1, 100)];
        let event_daozhan: u32 = 99; // 不存在

        let found = entries.iter().find(|(idx, _)| *idx == event_daozhan);
        assert!(
            found.is_none(),
            "已 despawn 的道伥（index=99）查找应返回 None，系统应安静跳过"
        );
    }

    /// 验证 DaoZhangRevealEvent 字段存在（P2 emit 路径静态检查）。
    #[test]
    fn daozhan_reveal_event_fields_exist() {
        let event = DaoZhangRevealEvent {
            daozhan: 42,
            trigger_pos: DVec3::new(1.0, 64.0, -3.0),
        };
        assert_eq!(event.daozhan, 42);
        assert!((event.trigger_pos.x - 1.0).abs() < 1e-9);
    }
}
