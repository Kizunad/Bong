//! 拟态灰烬蛛伪装状态 S2C CustomPayload — plan-fauna-mimic-spider-v1 P2
//!
//! 两个 channel：
//!   - `bong:spider_disguise_enter`：玩家连接或进入区域时，下发当前所有 Disguised 蛛的
//!     entity_id 列表，供 client 端 SpiderDisguiseHandler.java 切换渲染（ash_block 贴图覆盖）。
//!   - `bong:spider_ambush_trigger`：蛛从 Disguised→Ambush 转换时（SpiderAmbushTriggerEvent），
//!     向范围内玩家广播，client 端切回正常蜘蛛渲染。
//!
//! payload JSON 格式（两者共用相同 schema，type 字段区分）：
//! ```json
//! {
//!   "v": 1,
//!   "type": "spider_disguise_enter" | "spider_ambush_trigger",
//!   "entity_ids": [42, 77, ...]
//! }
//! ```
//!
//! `entity_ids` 为 Valence EntityId（i32），client 通过 MC entity id 查找实体。

use serde::{Deserialize, Serialize};
use valence::entity::EntityId;
use valence::prelude::{ident, Added, Client, EventReader, Position, Query, Res, With, Without};

use crate::cultivation::tick::CultivationClock;
use crate::fauna::mimic_spider::SpiderDisguiseState;
use crate::npc::brain_spider::SpiderAmbushTriggerEvent;
use crate::npc::spawn::NpcMarker;
use crate::schema::common::MAX_PAYLOAD_BYTES;
use crate::schema::server_data::ServerDataBuildError;

/// 伪装蛛广播半径（格）：Ambush 暴起事件广播给此半径内所有玩家。
///
/// 比感知半径（8格）大很多，确保任何可能目击蛛暴起的玩家都收到渲染切换信号。
pub const SPIDER_AMBUSH_BROADCAST_RADIUS: f64 = 64.0;

/// 下发 spider_disguise_enter 的节流间隔（tick）：避免频繁重发。
pub const SPIDER_DISGUISE_SYNC_INTERVAL_TICKS: u64 = 40; // 2秒

/// S2C payload（两个 channel 共用）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpiderDisguiseS2c {
    pub v: u8,
    #[serde(rename = "type")]
    pub ty: String,
    /// Valence 实体 ID（client 侧用于定位 MC entity）
    pub entity_ids: Vec<i32>,
}

impl SpiderDisguiseS2c {
    pub fn disguise_enter(entity_ids: Vec<i32>) -> Self {
        Self {
            v: 1,
            ty: "spider_disguise_enter".to_string(),
            entity_ids,
        }
    }

    pub fn ambush_trigger(entity_ids: Vec<i32>) -> Self {
        Self {
            v: 1,
            ty: "spider_ambush_trigger".to_string(),
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

// ── 系统：玩家首次连接时广播 Disguised 蛛列表 ─────────────────────────────────

/// 新玩家连接时推送当前所有 Disguised 蛛 entity id 列表。
///
/// 走 `Added<Client>` 过滤，只对新连接玩家发送（`SPIDER_DISGUISE_SYNC_INTERVAL_TICKS`
/// 节流由 `periodic_spider_disguise_sync_system` 处理）。
pub fn on_player_join_send_spider_disguise_list(
    mut new_clients: Query<&mut Client, Added<Client>>,
    spiders: Query<(&EntityId, &Position, &SpiderDisguiseState), With<NpcMarker>>,
) {
    // 如无新玩家，fast-return
    if new_clients.is_empty() {
        return;
    }

    // 收集所有 Disguised 蛛的 entity_id
    let disguised_ids: Vec<i32> = spiders
        .iter()
        .filter(|(_, _, state)| **state == SpiderDisguiseState::Disguised)
        .map(|(eid, _, _)| eid.get())
        .collect();

    let payload = SpiderDisguiseS2c::disguise_enter(disguised_ids);
    let Ok(bytes) = payload.to_json_bytes_checked() else {
        tracing::warn!("[bong][spider_disguise] disguise_enter payload oversize, skip");
        return;
    };

    for mut client in &mut new_clients {
        client.send_custom_payload(ident!("bong:spider_disguise_enter"), &bytes);
    }
}

/// 周期性全量同步 Disguised 蛛列表（防止 client 状态漂移）。
///
/// 每 `SPIDER_DISGUISE_SYNC_INTERVAL_TICKS` tick 向所有玩家重发当前 Disguised 蛛列表。
pub fn periodic_spider_disguise_sync_system(
    clock: Res<CultivationClock>,
    mut clients: Query<&mut Client, With<Client>>,
    spiders: Query<(&EntityId, &SpiderDisguiseState), With<NpcMarker>>,
) {
    if clock.tick % SPIDER_DISGUISE_SYNC_INTERVAL_TICKS != 0 {
        return;
    }

    let disguised_ids: Vec<i32> = spiders
        .iter()
        .filter(|(_, state)| **state == SpiderDisguiseState::Disguised)
        .map(|(eid, _)| eid.get())
        .collect();

    let payload = SpiderDisguiseS2c::disguise_enter(disguised_ids);
    let Ok(bytes) = payload.to_json_bytes_checked() else {
        tracing::warn!("[bong][spider_disguise] periodic disguise_enter oversize, skip");
        return;
    };

    for mut client in &mut clients {
        client.send_custom_payload(ident!("bong:spider_disguise_enter"), &bytes);
    }
}

// ── 系统：蛛暴起时广播 ambush_trigger ─────────────────────────────────────────

type AmbushSpiderQuery<'w, 's> =
    Query<'w, 's, (&'static EntityId, &'static Position), (With<NpcMarker>, Without<Client>)>;
type AmbushClientQuery<'w, 's> =
    Query<'w, 's, (&'static mut Client, &'static Position), Without<NpcMarker>>;

/// 监听 `SpiderAmbushTriggerEvent`，向暴起坐标周围 `SPIDER_AMBUSH_BROADCAST_RADIUS` 内玩家
/// 广播 `bong:spider_ambush_trigger`。
///
/// Client 收到后将对应 entity 的渲染从 ash_block 切回正常蜘蛛外观。
pub fn on_spider_ambush_broadcast_system(
    mut ambush_events: EventReader<SpiderAmbushTriggerEvent>,
    spiders: AmbushSpiderQuery<'_, '_>,
    mut clients: AmbushClientQuery<'_, '_>,
) {
    let events: Vec<SpiderAmbushTriggerEvent> = ambush_events.read().cloned().collect();
    if events.is_empty() {
        return;
    }

    for event in &events {
        // 找到对应 spider entity 的 entity_id
        // SpiderAmbushTriggerEvent.spider 是 Entity::raw index，但 EntityId 是 MC 协议 id
        // 我们需要找到与 trigger_pos 最近的 Ambush 蛛的 entity_id
        // 策略：按 trigger_pos 距离找最近蛛（Ambush 已转换）
        let trigger_pos = event.trigger_pos;
        let trigger_pos_arr = [trigger_pos.x, trigger_pos.y, trigger_pos.z];

        let Some((spider_eid, _)) = spiders.iter().min_by(|(_, pa), (_, pb)| {
            let da = dist3(trigger_pos_arr, [pa.get().x, pa.get().y, pa.get().z]);
            let db = dist3(trigger_pos_arr, [pb.get().x, pb.get().y, pb.get().z]);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        }) else {
            continue;
        };

        let payload = SpiderDisguiseS2c::ambush_trigger(vec![spider_eid.get()]);
        let Ok(bytes) = payload.to_json_bytes_checked() else {
            tracing::warn!("[bong][spider_disguise] ambush_trigger payload oversize, skip");
            continue;
        };

        // 广播给周围玩家
        for (mut client, client_pos) in &mut clients {
            let client_pos_arr = {
                let p = client_pos.get();
                [p.x, p.y, p.z]
            };
            if dist3(trigger_pos_arr, client_pos_arr) <= SPIDER_AMBUSH_BROADCAST_RADIUS {
                client.send_custom_payload(ident!("bong:spider_ambush_trigger"), &bytes);
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

pub fn register(app: &mut valence::prelude::App) {
    use valence::prelude::Update;
    app.add_systems(
        Update,
        (
            on_player_join_send_spider_disguise_list,
            periodic_spider_disguise_sync_system,
            on_spider_ambush_broadcast_system,
        ),
    );
}

// ── 测试 ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── schema pin 测试 ────────────────────────────────────────────────────

    #[test]
    fn spider_disguise_s2c_wire_format_pin_enter() {
        let payload = SpiderDisguiseS2c::disguise_enter(vec![42, 77]);
        let json = serde_json::to_string(&payload).expect("serialize must succeed");
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["v"], 1, "版本号必须为 1");
        assert_eq!(v["type"], "spider_disguise_enter", "type wire 名必须稳定");
        let ids: Vec<i32> = serde_json::from_value(v["entity_ids"].clone()).unwrap();
        assert_eq!(ids, vec![42, 77], "entity_ids 应原样序列化");
    }

    #[test]
    fn spider_disguise_s2c_wire_format_pin_ambush() {
        let payload = SpiderDisguiseS2c::ambush_trigger(vec![99]);
        let json = serde_json::to_string(&payload).expect("serialize must succeed");
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["v"], 1, "版本号必须为 1");
        assert_eq!(v["type"], "spider_ambush_trigger", "type wire 名必须稳定");
        let ids: Vec<i32> = serde_json::from_value(v["entity_ids"].clone()).unwrap();
        assert_eq!(ids, vec![99]);
    }

    #[test]
    fn spider_disguise_s2c_empty_ids_serializes_as_empty_array() {
        let payload = SpiderDisguiseS2c::disguise_enter(vec![]);
        let json = serde_json::to_string(&payload).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(
            v["entity_ids"].as_array().is_some_and(|a| a.is_empty()),
            "空蛛列表应序列化为空数组 []，实际 {:?}",
            v["entity_ids"]
        );
    }

    #[test]
    fn spider_disguise_s2c_roundtrip() {
        let original = SpiderDisguiseS2c {
            v: 1,
            ty: "spider_disguise_enter".to_string(),
            entity_ids: vec![1, 2, 3],
        };
        let json = serde_json::to_string(&original).unwrap();
        let decoded: SpiderDisguiseS2c = serde_json::from_str(&json).unwrap();
        assert_eq!(original, decoded, "SpiderDisguiseS2c 序列化往返必须等价");
    }

    #[test]
    fn spider_disguise_s2c_type_field_is_renamed() {
        // 确保 "type" wire name 稳定（serde rename）
        let enter = SpiderDisguiseS2c::disguise_enter(vec![]);
        let json_enter = serde_json::to_string(&enter).unwrap();
        assert!(
            json_enter.contains("\"type\":\"spider_disguise_enter\""),
            "type 字段 wire name 必须为 spider_disguise_enter，实际 {json_enter}"
        );

        let ambush = SpiderDisguiseS2c::ambush_trigger(vec![]);
        let json_ambush = serde_json::to_string(&ambush).unwrap();
        assert!(
            json_ambush.contains("\"type\":\"spider_ambush_trigger\""),
            "type 字段 wire name 必须为 spider_ambush_trigger，实际 {json_ambush}"
        );
    }

    #[test]
    fn broadcast_radius_pin() {
        assert!(
            (SPIDER_AMBUSH_BROADCAST_RADIUS - 64.0).abs() < 1e-9,
            "暴起广播半径应为 64 格（期望 64.0，实际 {SPIDER_AMBUSH_BROADCAST_RADIUS}）"
        );
    }

    #[test]
    fn dist3_correct() {
        assert!(
            (dist3([0.0, 0.0, 0.0], [3.0, 4.0, 0.0]) - 5.0).abs() < 1e-9,
            "dist3 三角函数校验失败"
        );
    }
}
