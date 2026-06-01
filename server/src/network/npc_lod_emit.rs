//! 远视野低 LOD NPC 渲染数据广播（`bong:npc_lod`）。
//!
//! plan-offscreen-war-v1 §10.1 P7 PR-11。
//!
//! ## 职责
//!
//! 每 `NPC_LOD_MID_SYNC_INTERVAL_TICKS`（80 tick ≈ 4 秒）扫描所有
//! `NpcLodTier::Mid` 的 live ECS NPC，向视野内玩家发送低 LOD 简化渲染数据。
//!
//! ## 守恒红线
//!
//! `bong:npc_lod` packet **只含位置 + archetype + realm + hp_ratio**，
//! 是 server→client **只读渲染数据**：
//! - 零真元流动（不触碰 `qi_current` / `WorldQiAccount` / `qi_physics::ledger`）
//! - 不触碰 `NpcDormantStore`（hydrate/dehydrate 路径与 LOD tier 完全正交）
//! - 不影响 Near 档 NPC 的 Valence 原生 entity sync 路径
//!
//! ## 渲染范围约束
//!
//! - Near（0..80 格）：Valence 原生 entity sync + `bong:npc_metadata`，不走此路径
//! - Mid（80..256 格）：发此 packet，客户端做简化皮肤 overlay
//! - Far/Dormant：超出玩家可见范围，不发

use serde::{Deserialize, Serialize};
use valence::client::ClientMarker;
use valence::entity::EntityId;
use valence::prelude::{
    bevy_ecs, ident, Client, DVec3, Despawned, Position, Query, ResMut, Resource, With, Without,
};

use crate::combat::components::{Lifecycle, LifecycleState, Wounds};
use crate::cultivation::components::{Cultivation, Realm};
use crate::network::npc_metadata::realm_label;
use crate::npc::lifecycle::NpcArchetype;
use crate::npc::lod::NpcLodTier;
use crate::npc::spawn::NpcMarker;
use crate::schema::common::MAX_PAYLOAD_BYTES;
use crate::schema::server_data::ServerDataBuildError;

/// Mid 档低 LOD 广播间隔：80 tick ≈ 4 秒（在 20TPS 下）。
///
/// 远视野渲染数据更新频率远低于近程（20 tick = 1秒），
/// 4 秒/次在感知上不可察觉——Mid NPC 已是 Valence entity，
/// 位置由原生 entity move packet 实时同步；此 packet 只补 overlay 数据。
pub const NPC_LOD_MID_SYNC_INTERVAL_TICKS: u64 = 80;

/// 发送半径：只向视野内（≤256 格，与 mid_radius 对齐）的玩家发此 packet。
///
/// 超出 256 格的玩家已看不到 Mid NPC，发送无意义。
pub const NPC_LOD_SEND_RADIUS: f64 = 256.0;

/// `bong:npc_lod` S2C payload。
///
/// 字段极简——仅供远视野低多边形渲染 / 简化皮肤 overlay 用：
/// - `entity_id`：与 Valence entity id 对齐，客户端 `NpcLodStore` 的键
/// - `archetype`：形体 / 皮肤选择（snake_case，对应 `NpcArchetype::as_str()`）
/// - `realm`：境界标签（中文，对应 `realm_label()`），供 distant label 显示
/// - `hp_ratio`：血量比率 [0.0, 1.0]，供血量条 overlay
/// - `x`, `y`, `z`：NPC 当前坐标（冗余发送，方便客户端本地 LOD 距离判断）
///
/// **无 qi / ledger 字段**，守恒红线隔离。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NpcLodS2c {
    pub v: u8,
    #[serde(rename = "type")]
    pub ty: String,
    pub entity_id: i32,
    pub archetype: String,
    pub realm: String,
    pub hp_ratio: f32,
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl NpcLodS2c {
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

/// LOD 广播系统状态资源（tick 计数 + 幂等保护）。
#[derive(Debug, Default, Resource)]
pub struct NpcLodEmitState {
    tick: u64,
}

type ClientLodItem<'a> = (&'a mut Client, &'a Position);
type NpcLodItem<'a> = (
    &'a EntityId,
    &'a Position,
    &'a NpcLodTier,
    &'a NpcArchetype,
    Option<&'a Cultivation>,
    Option<&'a Wounds>,
    Option<&'a Lifecycle>,
);

/// 低 LOD NPC 渲染数据广播系统。
///
/// 每 `NPC_LOD_MID_SYNC_INTERVAL_TICKS`（80 tick）执行一次：
/// 1. 扫描所有 `NpcLodTier::Mid` 且非 Terminated 的 live ECS NPC
/// 2. 对每个在 `NPC_LOD_SEND_RADIUS`（256格）范围内的玩家发送 `bong:npc_lod` packet
///
/// **守恒保证**：不读写任何 qi / ledger / DormantStore 字段。
#[allow(clippy::type_complexity)]
pub fn emit_npc_lod_payloads(
    mut state: ResMut<NpcLodEmitState>,
    mut clients: Query<ClientLodItem<'_>, With<ClientMarker>>,
    npcs: Query<NpcLodItem<'_>, (With<NpcMarker>, Without<Despawned>)>,
) {
    state.tick = state.tick.saturating_add(1);
    if !state.tick.is_multiple_of(NPC_LOD_MID_SYNC_INTERVAL_TICKS) {
        return;
    }

    let radius_sq = NPC_LOD_SEND_RADIUS * NPC_LOD_SEND_RADIUS;

    // 预先构建 Mid NPC 列表（过滤 Terminated），避免嵌套 Query 借用冲突。
    let mid_npcs: Vec<(
        i32,
        DVec3,
        &NpcArchetype,
        Option<&Cultivation>,
        Option<&Wounds>,
    )> = npcs
        .iter()
        .filter_map(
            |(entity_id, position, tier, archetype, cultivation, wounds, lifecycle)| {
                // 只处理 Mid 档 NPC
                if *tier != NpcLodTier::Mid {
                    return None;
                }
                // 跳过已死亡 NPC
                if lifecycle.is_some_and(|lc| lc.state == LifecycleState::Terminated) {
                    return None;
                }
                Some((
                    entity_id.get(),
                    position.get(),
                    archetype,
                    cultivation,
                    wounds,
                ))
            },
        )
        .collect();

    if mid_npcs.is_empty() {
        return;
    }

    for (mut client, client_position) in &mut clients {
        let client_pos = client_position.get();
        for (entity_id, npc_pos, archetype, cultivation, wounds) in &mid_npcs {
            if client_pos.distance_squared(*npc_pos) > radius_sq {
                continue;
            }

            let realm = cultivation.map(|c| c.realm).unwrap_or(Realm::Awaken);
            let hp_ratio = wounds
                .map(|w| {
                    if w.health_max > 0.0 {
                        (w.health_current / w.health_max).clamp(0.0, 1.0)
                    } else {
                        0.0
                    }
                })
                .unwrap_or(1.0);

            let payload = NpcLodS2c {
                v: 1,
                ty: "npc_lod".to_string(),
                entity_id: *entity_id,
                archetype: archetype.as_str().to_string(),
                realm: realm_label(realm).to_string(),
                hp_ratio,
                x: npc_pos.x,
                y: npc_pos.y,
                z: npc_pos.z,
            };

            let bytes = match payload.to_json_bytes_checked() {
                Ok(b) => b,
                Err(err) => {
                    tracing::warn!(
                        "[bong][npc_lod] dropping lod packet entity_id={}: {err:?}",
                        entity_id
                    );
                    continue;
                }
            };
            client.send_custom_payload(ident!("bong:npc_lod"), &bytes);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── NpcLodS2c 序列化契约测试 ────────────────────────────────────────────

    /// happy path：v=1, type="npc_lod"，全字段正确序列化。
    #[test]
    fn npc_lod_s2c_serializes_all_fields() {
        let payload = NpcLodS2c {
            v: 1,
            ty: "npc_lod".to_string(),
            entity_id: 42,
            archetype: "rogue".to_string(),
            realm: "引气".to_string(),
            hp_ratio: 0.75,
            x: 100.5,
            y: 64.0,
            z: -200.0,
        };
        let bytes = payload
            .to_json_bytes_checked()
            .expect("期望序列化成功，但返回错误");
        let parsed: serde_json::Value =
            serde_json::from_slice(&bytes).expect("期望 JSON 可解析，但失败");

        assert_eq!(parsed["v"], 1, "期望 v=1（协议版本），实际={}", parsed["v"]);
        assert_eq!(
            parsed["type"], "npc_lod",
            "期望 type=npc_lod，实际={}",
            parsed["type"]
        );
        assert_eq!(
            parsed["entity_id"], 42,
            "期望 entity_id=42，实际={}",
            parsed["entity_id"]
        );
        assert_eq!(
            parsed["archetype"], "rogue",
            "期望 archetype=rogue，实际={}",
            parsed["archetype"]
        );
        assert_eq!(
            parsed["realm"], "引气",
            "期望 realm=引气，实际={}",
            parsed["realm"]
        );
        assert!(
            (parsed["hp_ratio"].as_f64().unwrap() - 0.75).abs() < 1e-4,
            "期望 hp_ratio≈0.75，实际={}",
            parsed["hp_ratio"]
        );
        assert!(
            (parsed["x"].as_f64().unwrap() - 100.5).abs() < 1e-6,
            "期望 x≈100.5，实际={}",
            parsed["x"]
        );
    }

    /// archetype 字段必须是 snake_case 字符串（NpcArchetype::as_str() 的规范输出）。
    #[test]
    fn npc_lod_s2c_archetype_is_snake_case() {
        let cases = [
            ("zombie", "zombie"),
            ("commoner", "commoner"),
            ("guardian_relic", "guardian_relic"),
            ("skull_fiend", "skull_fiend"),
        ];
        for (input, expected) in cases {
            let payload = NpcLodS2c {
                v: 1,
                ty: "npc_lod".to_string(),
                entity_id: 1,
                archetype: input.to_string(),
                realm: "醒灵".to_string(),
                hp_ratio: 1.0,
                x: 0.0,
                y: 0.0,
                z: 0.0,
            };
            let bytes = payload.to_json_bytes_checked().unwrap();
            let parsed: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(
                parsed["archetype"], expected,
                "期望 archetype={expected}（snake_case），实际={}",
                parsed["archetype"]
            );
        }
    }

    /// hp_ratio 边界值：0.0（死亡），1.0（满血）。
    #[test]
    fn npc_lod_s2c_hp_ratio_boundary_values() {
        for &hp_ratio in &[0.0_f32, 1.0_f32] {
            let payload = NpcLodS2c {
                v: 1,
                ty: "npc_lod".to_string(),
                entity_id: 1,
                archetype: "zombie".to_string(),
                realm: "醒灵".to_string(),
                hp_ratio,
                x: 0.0,
                y: 0.0,
                z: 0.0,
            };
            let bytes = payload
                .to_json_bytes_checked()
                .unwrap_or_else(|e| panic!("hp_ratio={hp_ratio} 序列化失败：{e:?}"));
            let parsed: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            let got = parsed["hp_ratio"].as_f64().unwrap() as f32;
            assert!(
                (got - hp_ratio).abs() < 1e-4,
                "期望 hp_ratio={hp_ratio}，实际={got}"
            );
        }
    }

    /// payload 体积 < MAX_PAYLOAD_BYTES（常见字段不超限）。
    #[test]
    fn npc_lod_s2c_within_max_payload_bytes() {
        let payload = NpcLodS2c {
            v: 1,
            ty: "npc_lod".to_string(),
            entity_id: i32::MAX,
            archetype: "guardian_relic".to_string(),
            realm: "化虚".to_string(),
            hp_ratio: 0.123,
            x: -9999.999,
            y: 255.0,
            z: 9999.999,
        };
        let bytes = payload
            .to_json_bytes_checked()
            .expect("期望序列化成功，实际超出 MAX_PAYLOAD_BYTES 或序列化失败");
        assert!(
            bytes.len() < MAX_PAYLOAD_BYTES,
            "期望 payload < {} bytes 因为字段极简，实际 {} bytes",
            MAX_PAYLOAD_BYTES,
            bytes.len()
        );
    }

    /// to_json_bytes_checked 无法超出限制（构造一个超大负载验证 Oversize 路径）。
    #[test]
    fn npc_lod_s2c_rejects_oversize_payload() {
        // 构造一个超大 realm 字符串（MAX+1 bytes 肯定超限）
        let huge_realm = "X".repeat(MAX_PAYLOAD_BYTES + 1);
        let payload = NpcLodS2c {
            v: 1,
            ty: "npc_lod".to_string(),
            entity_id: 1,
            archetype: "zombie".to_string(),
            realm: huge_realm,
            hp_ratio: 1.0,
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        let result = payload.to_json_bytes_checked();
        assert!(
            matches!(result, Err(ServerDataBuildError::Oversize { .. })),
            "期望 Oversize 错误（因为 realm 超大），实际={result:?}"
        );
    }

    /// 反序列化（客户端解包方向）：JSON → NpcLodS2c 全字段正确。
    #[test]
    fn npc_lod_s2c_roundtrip_deserialization() {
        let json = r#"{"v":1,"type":"npc_lod","entity_id":99,"archetype":"beast","realm":"凝脉","hp_ratio":0.5,"x":50.0,"y":70.0,"z":-30.0}"#;
        let parsed: NpcLodS2c = serde_json::from_str(json).expect("期望反序列化成功，但失败");
        assert_eq!(parsed.v, 1);
        assert_eq!(parsed.ty, "npc_lod");
        assert_eq!(parsed.entity_id, 99);
        assert_eq!(parsed.archetype, "beast");
        assert_eq!(parsed.realm, "凝脉");
        assert!((parsed.hp_ratio - 0.5).abs() < 1e-4);
        assert!((parsed.x - 50.0).abs() < 1e-6);
        assert!((parsed.z - (-30.0)).abs() < 1e-6);
    }

    /// 无 qi 字段——守恒红线：序列化输出不包含 qi_current / qi_ratio / ledger 等字段。
    #[test]
    fn npc_lod_s2c_contains_no_qi_fields() {
        let payload = NpcLodS2c {
            v: 1,
            ty: "npc_lod".to_string(),
            entity_id: 1,
            archetype: "rogue".to_string(),
            realm: "引气".to_string(),
            hp_ratio: 0.8,
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        let bytes = payload.to_json_bytes_checked().unwrap();
        let json_str = std::str::from_utf8(&bytes).unwrap();
        for forbidden in &["qi_current", "qi_ratio", "qi_max", "ledger", "transfer"] {
            assert!(
                !json_str.contains(forbidden),
                "期望 npc_lod payload 不含 {forbidden}（守恒红线：零真元），实际 JSON={json_str}"
            );
        }
    }

    /// NpcLodS2c 所有 realm 变体都能正确序列化（正反 sample 覆盖 6 境界）。
    #[test]
    fn npc_lod_s2c_realm_all_variants() {
        // 对应 realm_label() 的 6 个输出
        let realm_labels = ["醒灵", "引气", "凝脉", "固元", "通灵", "化虚"];
        for &label in &realm_labels {
            let payload = NpcLodS2c {
                v: 1,
                ty: "npc_lod".to_string(),
                entity_id: 1,
                archetype: "zombie".to_string(),
                realm: label.to_string(),
                hp_ratio: 1.0,
                x: 0.0,
                y: 0.0,
                z: 0.0,
            };
            let bytes = payload
                .to_json_bytes_checked()
                .unwrap_or_else(|e| panic!("realm={label} 序列化失败：{e:?}"));
            let parsed: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(
                parsed["realm"], label,
                "期望 realm={label}，实际={}",
                parsed["realm"]
            );
        }
    }

    /// 常量契约锁定：间隔 80 tick，发送半径 256.0。
    #[test]
    fn npc_lod_emit_constants_are_locked() {
        assert_eq!(
            NPC_LOD_MID_SYNC_INTERVAL_TICKS, 80,
            "期望 Mid LOD 广播间隔=80 tick（4秒@20TPS），实际={}",
            NPC_LOD_MID_SYNC_INTERVAL_TICKS
        );
        assert!(
            (NPC_LOD_SEND_RADIUS - 256.0).abs() < 1e-6,
            "期望发送半径=256.0（与 mid_radius 对齐），实际={}",
            NPC_LOD_SEND_RADIUS
        );
    }

    /// NpcLodEmitState 初始 tick=0。
    #[test]
    fn npc_lod_emit_state_default_tick_is_zero() {
        let state = NpcLodEmitState::default();
        assert_eq!(state.tick, 0, "期望初始 tick=0，实际={}", state.tick);
    }
}
