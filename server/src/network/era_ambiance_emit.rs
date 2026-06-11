//! plan-era-state-v1 P3 — 时代天象 S2C CustomPayload（`bong:era_ambiance`）
//!
//! 职责：
//!   - 监听 [`EraChangedEvent`]，按三时代参数组装 [`EraAmbianceS2c`] payload
//!   - **Realm gate（server 强制执行）**：
//!     - `Realm < Solidify`（凝脉-）：完全不发包
//!     - `Realm::Solidify / Condense`：发缩减包（仅 `fog_density_delta * 0.1`，无 sky_tint，无 sound）
//!     - `Realm >= Spirit`（通灵+）：发完整包（sky_tint + fog_density + ambient_sound_id）
//!   - 新玩家连接时（`Added<Client>`）若当前时代非 Unknown，补发当前天象快照
//!   - payload JSON 线协议：`{ "v": 1, "era_type": ..., "sky_tint_hex": ...,
//!     "fog_density_delta": ..., "ambient_sound_id": ..., "transition_ticks": 1200 }`
//!
//! ## 三时代天象规格（design decision §5）
//!
//! | 时代 | sky_tint_hex | fog_density_delta | ambient_sound_id |
//! |------|-------------|-------------------|------------------|
//! | Calamity（灾劫） | #4A1A1A | +0.15 | ambient.weather.thunder |
//! | Change（变化） | #1A3A4A | +0.05 | block.water.ambient |
//! | Deduction（演绎） | #2A2A3A | -0.05 | entity.enderman.ambient |
//! | Unknown | #000000（忽略） | 0.0 | （空/reset） |
//!
//! 过渡时长：`TRANSITION_TICKS = 1200`（60 秒 @20TPS）。
//!
//! 低境玩家 sky_tint / sound 会被 server 侧 realm gate 过滤掉：
//! - `Solidify` 仅收到 fog_density_delta 的 10%，其余字段为 baseline（`sky_tint_hex="#000000"`，无 sound）。
//! - `Condense` 及以下完全不收包。

use serde::{Deserialize, Serialize};
use valence::prelude::{ident, Added, Client, EventReader, Query, Res, With};

use crate::cultivation::components::{Cultivation, Realm};
use crate::network::npc_metadata::realm_rank;
use crate::schema::common::MAX_PAYLOAD_BYTES;
use crate::schema::server_data::ServerDataBuildError;
use crate::world::era::{EraChangedEvent, EraType, WorldEraState};

// ── 常数 ─────────────────────────────────────────────────────────────────────

/// EraAmbiance 渐变过渡时长（client 侧线性插值）。
pub const ERA_TRANSITION_TICKS: u32 = 1200;

/// 低境界（Solidify）收到的 fog_density_delta 衰减系数（10%）。
pub const SOLIDIFY_FOG_ATTENUATION: f64 = 0.1;

/// Realm gate 阈值：>= SPIRIT_RANK 收完整包；>= SOLIDIFY_RANK 收缩减包；< SOLIDIFY_RANK 不收包。
const SPIRIT_REALM_RANK: i32 = 4; // Realm::Spirit
const SOLIDIFY_REALM_RANK: i32 = 3; // Realm::Solidify

// ── 三时代天象参数 ─────────────────────────────────────────────────────────────

/// 灾劫时代天象参数。
pub const CALAMITY_SKY_TINT_HEX: &str = "#4A1A1A";
pub const CALAMITY_FOG_DENSITY_DELTA: f64 = 0.15;
pub const CALAMITY_AMBIENT_SOUND_ID: &str = "ambient.weather.thunder";

/// 变化时代天象参数。
pub const CHANGE_SKY_TINT_HEX: &str = "#1A3A4A";
pub const CHANGE_FOG_DENSITY_DELTA: f64 = 0.05;
pub const CHANGE_AMBIENT_SOUND_ID: &str = "block.water.ambient";

/// 演绎时代天象参数。
pub const DEDUCTION_SKY_TINT_HEX: &str = "#2A2A3A";
pub const DEDUCTION_FOG_DENSITY_DELTA: f64 = -0.05;
pub const DEDUCTION_AMBIENT_SOUND_ID: &str = "entity.enderman.ambient";

// ── Payload ────────────────────────────────────────────────────────────────────

/// S2C payload：时代天象参数。
///
/// Client 收到后以 [`transition_ticks`] 时长线性插值更新 `ZoneAtmosphereProfile`。
/// realm gate 由 server 执行：低境玩家不收此包，server 不会向 `Realm < Solidify` 发送。
///
/// `sky_tint_hex` = `"#000000"` 时 client 不修改天空色调（baseline/reset）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EraAmbianceS2c {
    /// 协议版本，固定为 1。
    pub v: u8,
    /// 时代类型（wire string）。
    pub era_type: String,
    /// 天空色调 hex（`#RRGGBB`）。`"#000000"` = 不修改（reset/baseline）。
    pub sky_tint_hex: String,
    /// 雾密度增量（-1.0..1.0）。
    pub fog_density_delta: f64,
    /// Ambient sound id（空字符串 = 不播放/stop）。
    pub ambient_sound_id: String,
    /// 过渡时长（tick）。
    pub transition_ticks: u32,
}

impl EraAmbianceS2c {
    /// 完整包：通灵+ 玩家收到的全部三个天象参数。
    pub fn full(
        era_type: EraType,
        sky_tint_hex: impl Into<String>,
        fog_density_delta: f64,
        ambient_sound_id: impl Into<String>,
    ) -> Self {
        Self {
            v: 1,
            era_type: era_type.as_wire_str().to_string(),
            sky_tint_hex: sky_tint_hex.into(),
            fog_density_delta,
            ambient_sound_id: ambient_sound_id.into(),
            transition_ticks: ERA_TRANSITION_TICKS,
        }
    }

    /// 缩减包：固元 玩家仅收到 fog_density_delta × SOLIDIFY_FOG_ATTENUATION，无 sky_tint，无 sound。
    pub fn attenuated(era_type: EraType, fog_density_delta: f64) -> Self {
        Self {
            v: 1,
            era_type: era_type.as_wire_str().to_string(),
            sky_tint_hex: "#000000".to_string(),
            fog_density_delta: fog_density_delta * SOLIDIFY_FOG_ATTENUATION,
            ambient_sound_id: String::new(),
            transition_ticks: ERA_TRANSITION_TICKS,
        }
    }

    /// 重置包：时代衰减到 Unknown 时 client 清零天象。
    pub fn reset() -> Self {
        Self {
            v: 1,
            era_type: EraType::Unknown.as_wire_str().to_string(),
            sky_tint_hex: "#000000".to_string(),
            fog_density_delta: 0.0,
            ambient_sound_id: String::new(),
            transition_ticks: ERA_TRANSITION_TICKS,
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

// ── 天象参数查找 ───────────────────────────────────────────────────────────────

/// 根据时代类型返回完整天象三参数（sky_tint_hex, fog_density_delta, ambient_sound_id）。
///
/// Unknown 时代返回 reset 参数。
pub fn era_ambiance_params(era: EraType) -> (&'static str, f64, &'static str) {
    match era {
        EraType::Calamity => (
            CALAMITY_SKY_TINT_HEX,
            CALAMITY_FOG_DENSITY_DELTA,
            CALAMITY_AMBIENT_SOUND_ID,
        ),
        EraType::Change => (
            CHANGE_SKY_TINT_HEX,
            CHANGE_FOG_DENSITY_DELTA,
            CHANGE_AMBIENT_SOUND_ID,
        ),
        EraType::Deduction => (
            DEDUCTION_SKY_TINT_HEX,
            DEDUCTION_FOG_DENSITY_DELTA,
            DEDUCTION_AMBIENT_SOUND_ID,
        ),
        EraType::Unknown => ("#000000", 0.0, ""),
    }
}

// ── Realm gate 分档 ────────────────────────────────────────────────────────────

/// Realm gate 分档：决定该玩家应收到哪种天象包。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RealmGateTier {
    /// Realm < Solidify：不发任何包。
    None,
    /// Realm::Solidify（固元）：发缩减包（仅 fog_density_delta * 0.1）。
    Attenuated,
    /// Realm >= Spirit（通灵+）：发完整包。
    Full,
}

pub fn realm_gate_tier(realm: Realm) -> RealmGateTier {
    let rank = realm_rank(realm);
    if rank >= SPIRIT_REALM_RANK {
        RealmGateTier::Full
    } else if rank >= SOLIDIFY_REALM_RANK {
        RealmGateTier::Attenuated
    } else {
        RealmGateTier::None
    }
}

/// 为给定时代和 realm 分档构建 payload（None = 不发）。
pub fn build_payload_for_tier(era: EraType, tier: RealmGateTier) -> Option<EraAmbianceS2c> {
    match tier {
        RealmGateTier::None => None,
        RealmGateTier::Attenuated => {
            if era == EraType::Unknown {
                // Unknown 时代不需要发缩减包给低境玩家（无天象效果）
                return None;
            }
            let (_, fog_delta, _) = era_ambiance_params(era);
            Some(EraAmbianceS2c::attenuated(era, fog_delta))
        }
        RealmGateTier::Full => {
            if era == EraType::Unknown {
                return Some(EraAmbianceS2c::reset());
            }
            let (sky_tint, fog_delta, sound_id) = era_ambiance_params(era);
            Some(EraAmbianceS2c::full(era, sky_tint, fog_delta, sound_id))
        }
    }
}

// ── 系统：EraChangedEvent → send_custom_payload ────────────────────────────────

/// 监听 [`EraChangedEvent`]，按三时代参数向各境界玩家分档发送 `bong:era_ambiance` payload。
///
/// Realm gate（server 强制执行）：
/// - `Realm < Solidify`（凝脉-）：不发包
/// - `Realm::Solidify`（固元）：发缩减包（fog_density_delta × 0.1，无 sky_tint，无 sound）
/// - `Realm >= Spirit`（通灵+）：发完整包
pub fn era_ambiance_on_era_changed_system(
    mut era_changed: EventReader<EraChangedEvent>,
    mut clients: Query<(&mut Client, Option<&Cultivation>), With<Client>>,
) {
    let events: Vec<EraChangedEvent> = era_changed.read().cloned().collect();
    if events.is_empty() {
        return;
    }

    // 每次时代切换，取最新的一条事件（多条时取 onset_tick 最大的）
    let latest = events
        .iter()
        .max_by_key(|e| e.onset_tick)
        .expect("events non-empty");
    let new_era = latest.new_era;

    for (mut client, cultivation) in &mut clients {
        let realm = cultivation.map(|c| c.realm).unwrap_or(Realm::Awaken);
        let tier = realm_gate_tier(realm);
        let Some(payload) = build_payload_for_tier(new_era, tier) else {
            continue;
        };
        match payload.to_json_bytes_checked() {
            Ok(bytes) => {
                client.send_custom_payload(ident!("bong:era_ambiance"), &bytes);
            }
            Err(error) => {
                tracing::warn!("[bong][era_ambiance] payload oversize: {error:?}");
            }
        }
    }

    tracing::info!(
        "[bong][era_ambiance] era_changed broadcast: new_era={}",
        new_era.as_wire_str()
    );
}

/// 新玩家连接时，若当前时代非 Unknown，补发当前天象快照。
///
/// 确保后加入的玩家也能看到当前时代的天象效果，无需等待下一次 era_decree。
pub fn era_ambiance_on_join_system(
    mut new_clients: Query<(&mut Client, Option<&Cultivation>), Added<Client>>,
    world_era: Option<Res<WorldEraState>>,
) {
    let current_era = world_era
        .as_deref()
        .map(|e| e.era)
        .unwrap_or(EraType::Unknown);

    if current_era == EraType::Unknown {
        // 无时代宣告，无需补发
        return;
    }

    for (mut client, cultivation) in &mut new_clients {
        let realm = cultivation.map(|c| c.realm).unwrap_or(Realm::Awaken);
        let tier = realm_gate_tier(realm);
        let Some(payload) = build_payload_for_tier(current_era, tier) else {
            continue;
        };
        match payload.to_json_bytes_checked() {
            Ok(bytes) => {
                client.send_custom_payload(ident!("bong:era_ambiance"), &bytes);
            }
            Err(error) => {
                tracing::warn!("[bong][era_ambiance] on_join payload oversize: {error:?}");
            }
        }
    }
}

// ── 单测 ─────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── realm_gate_tier 三档 pin 测试 ─────────────────────────────────────────

    #[test]
    fn realm_gate_tier_awaken_is_none() {
        assert_eq!(
            realm_gate_tier(Realm::Awaken),
            RealmGateTier::None,
            "醒灵境界 < 固元，应不收任何天象包（realm gate 硬过滤）"
        );
    }

    #[test]
    fn realm_gate_tier_induce_is_none() {
        assert_eq!(
            realm_gate_tier(Realm::Induce),
            RealmGateTier::None,
            "引气境界 < 固元，应不收任何天象包"
        );
    }

    #[test]
    fn realm_gate_tier_condense_is_none() {
        assert_eq!(
            realm_gate_tier(Realm::Condense),
            RealmGateTier::None,
            "凝脉境界 < 固元，应不收任何天象包"
        );
    }

    #[test]
    fn realm_gate_tier_solidify_is_attenuated() {
        assert_eq!(
            realm_gate_tier(Realm::Solidify),
            RealmGateTier::Attenuated,
            "固元境界应收缩减包（fog_density_delta * 0.1，无 sky_tint，无 sound）"
        );
    }

    #[test]
    fn realm_gate_tier_spirit_is_full() {
        assert_eq!(
            realm_gate_tier(Realm::Spirit),
            RealmGateTier::Full,
            "通灵境界应收完整包（sky_tint + fog + sound）"
        );
    }

    #[test]
    fn realm_gate_tier_void_is_full() {
        assert_eq!(
            realm_gate_tier(Realm::Void),
            RealmGateTier::Full,
            "化虚境界应收完整包"
        );
    }

    // ── 三时代天象参数 pin 测试 ────────────────────────────────────────────────

    #[test]
    fn era_ambiance_params_calamity_pin() {
        let (sky, fog, sound) = era_ambiance_params(EraType::Calamity);
        assert_eq!(
            sky, "#4A1A1A",
            "灾劫时代 sky_tint_hex 必须为 #4A1A1A（设计决议 §5）；实际 = {}",
            sky
        );
        assert!(
            (fog - 0.15).abs() < 1e-9,
            "灾劫时代 fog_density_delta 必须为 0.15；实际 = {}",
            fog
        );
        assert_eq!(
            sound, "ambient.weather.thunder",
            "灾劫时代 ambient_sound_id 必须为 ambient.weather.thunder；实际 = {}",
            sound
        );
    }

    #[test]
    fn era_ambiance_params_change_pin() {
        let (sky, fog, sound) = era_ambiance_params(EraType::Change);
        assert_eq!(
            sky, "#1A3A4A",
            "变化时代 sky_tint_hex 必须为 #1A3A4A；实际 = {}",
            sky
        );
        assert!(
            (fog - 0.05).abs() < 1e-9,
            "变化时代 fog_density_delta 必须为 0.05；实际 = {}",
            fog
        );
        assert_eq!(
            sound, "block.water.ambient",
            "变化时代 ambient_sound_id 必须为 block.water.ambient；实际 = {}",
            sound
        );
    }

    #[test]
    fn era_ambiance_params_deduction_pin() {
        let (sky, fog, sound) = era_ambiance_params(EraType::Deduction);
        assert_eq!(
            sky, "#2A2A3A",
            "演绎时代 sky_tint_hex 必须为 #2A2A3A；实际 = {}",
            sky
        );
        assert!(
            (fog - (-0.05)).abs() < 1e-9,
            "演绎时代 fog_density_delta 必须为 -0.05；实际 = {}",
            fog
        );
        assert_eq!(
            sound, "entity.enderman.ambient",
            "演绎时代 ambient_sound_id 必须为 entity.enderman.ambient；实际 = {}",
            sound
        );
    }

    // ── build_payload_for_tier 行为 ───────────────────────────────────────────

    #[test]
    fn build_payload_none_tier_returns_none() {
        for era in [
            EraType::Calamity,
            EraType::Change,
            EraType::Deduction,
            EraType::Unknown,
        ] {
            assert!(
                build_payload_for_tier(era, RealmGateTier::None).is_none(),
                "None tier 对所有时代应返回 None（不发包）；era = {}",
                era.as_wire_str()
            );
        }
    }

    #[test]
    fn build_payload_attenuated_calamity_fog_is_10_percent() {
        let payload = build_payload_for_tier(EraType::Calamity, RealmGateTier::Attenuated)
            .expect("Attenuated + Calamity 应产生 payload");
        let expected = CALAMITY_FOG_DENSITY_DELTA * SOLIDIFY_FOG_ATTENUATION;
        assert!(
            (payload.fog_density_delta - expected).abs() < 1e-12,
            "固元缩减包：fog_density_delta 应为 Calamity({}) * 0.1 = {}；实际 = {}",
            CALAMITY_FOG_DENSITY_DELTA,
            expected,
            payload.fog_density_delta
        );
        assert_eq!(
            payload.sky_tint_hex, "#000000",
            "固元缩减包：sky_tint_hex 应为 #000000（不修改天空色调）；实际 = {}",
            payload.sky_tint_hex
        );
        assert!(
            payload.ambient_sound_id.is_empty(),
            "固元缩减包：ambient_sound_id 应为空（无音效）；实际 = {}",
            payload.ambient_sound_id
        );
    }

    #[test]
    fn build_payload_full_calamity_contains_all_params() {
        let payload = build_payload_for_tier(EraType::Calamity, RealmGateTier::Full)
            .expect("Full + Calamity 应产生 payload");
        assert_eq!(payload.sky_tint_hex, CALAMITY_SKY_TINT_HEX);
        assert!((payload.fog_density_delta - CALAMITY_FOG_DENSITY_DELTA).abs() < 1e-9);
        assert_eq!(payload.ambient_sound_id, CALAMITY_AMBIENT_SOUND_ID);
        assert_eq!(
            payload.transition_ticks, ERA_TRANSITION_TICKS,
            "过渡时长必须为 1200 tick（60 秒 @20TPS）；实际 = {}",
            payload.transition_ticks
        );
    }

    #[test]
    fn build_payload_full_unknown_returns_reset() {
        let payload = build_payload_for_tier(EraType::Unknown, RealmGateTier::Full)
            .expect("Full + Unknown 应产生 reset payload");
        assert_eq!(payload.era_type, "unknown");
        assert_eq!(payload.sky_tint_hex, "#000000");
        assert!((payload.fog_density_delta).abs() < 1e-9);
        assert!(payload.ambient_sound_id.is_empty());
    }

    #[test]
    fn build_payload_attenuated_unknown_returns_none() {
        // Unknown 时代对 Attenuated 层（固元）也不发包（无天象效果）
        let payload = build_payload_for_tier(EraType::Unknown, RealmGateTier::Attenuated);
        assert!(
            payload.is_none(),
            "Unknown 时代 Attenuated tier 不应发包（无任何天象效果）；实际 = {:?}",
            payload
        );
    }

    // ── EraAmbianceS2c payload wire 格式 ──────────────────────────────────────

    #[test]
    fn era_ambiance_s2c_full_wire_roundtrip() {
        let payload = EraAmbianceS2c::full(
            EraType::Calamity,
            CALAMITY_SKY_TINT_HEX,
            CALAMITY_FOG_DENSITY_DELTA,
            CALAMITY_AMBIENT_SOUND_ID,
        );
        let json = serde_json::to_string(&payload).expect("serialize must succeed");
        let decoded: EraAmbianceS2c =
            serde_json::from_str(&json).expect("deserialize must succeed");
        assert_eq!(payload, decoded, "EraAmbianceS2c 序列化往返必须等价");
    }

    #[test]
    fn era_ambiance_s2c_wire_version_is_1() {
        let payload = EraAmbianceS2c::reset();
        let v: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&payload).unwrap()).unwrap();
        assert_eq!(v["v"], 1, "协议版本必须为 1");
    }

    #[test]
    fn era_ambiance_s2c_transition_ticks_pin() {
        assert_eq!(
            ERA_TRANSITION_TICKS, 1200,
            "过渡时长必须为 1200 tick（60s @20TPS，设计决议 §5）；实际 = {}",
            ERA_TRANSITION_TICKS
        );
        let payload = EraAmbianceS2c::full(EraType::Change, "#1A3A4A", 0.05, "block.water.ambient");
        assert_eq!(
            payload.transition_ticks, 1200,
            "payload.transition_ticks 必须为 1200；实际 = {}",
            payload.transition_ticks
        );
    }

    #[test]
    fn era_ambiance_s2c_to_json_bytes_checked_within_limit() {
        let payload = EraAmbianceS2c::full(
            EraType::Deduction,
            "#2A2A3A",
            -0.05,
            "entity.enderman.ambient",
        );
        let result = payload.to_json_bytes_checked();
        assert!(
            result.is_ok(),
            "EraAmbianceS2c 应在 MAX_PAYLOAD_BYTES 限制内；error = {:?}",
            result.err()
        );
    }

    // ── 新玩家连接补发（on_join）逻辑 ─────────────────────────────────────────

    /// 验证 WorldEraState Unknown 时 build_payload_for_tier 对所有 realm gate 分档均不发
    /// 实质性天象包：Full 档返回 reset（非 None），Attenuated/None 档返回 None。
    ///
    /// 同时覆盖 on_join_system 在 era == Unknown 时直接 return 的实际效果：
    /// build_payload_for_tier(Unknown, Full) 返回 reset payload，
    /// 但该路径只在 era != Unknown 时触发（on_join_system 先判断 era == Unknown → return）。
    #[test]
    fn on_join_unknown_era_no_meaningful_payload_for_any_tier() {
        // Full 档（Spirit+）：Unknown 时返回 reset payload（sky_tint="#000000"，fog=0，无 sound）
        let full_payload = build_payload_for_tier(EraType::Unknown, RealmGateTier::Full).expect(
            "Unknown+Full 应返回 reset payload 而非 None（on_join_system 在 era==Unknown 时 skip）",
        );
        assert_eq!(
            full_payload.sky_tint_hex, "#000000",
            "Unknown 时代 Full 档 reset payload 的 sky_tint_hex 应为 #000000（无天象）；\
             实际 {}",
            full_payload.sky_tint_hex
        );
        assert!(
            (full_payload.fog_density_delta - 0.0).abs() < 1e-9,
            "Unknown 时代 Full 档 reset payload 的 fog_density_delta 应为 0.0；\
             实际 {}",
            full_payload.fog_density_delta
        );
        assert!(
            full_payload.ambient_sound_id.is_empty(),
            "Unknown 时代 Full 档 reset payload 的 ambient_sound_id 应为空字符串（无音效）；\
             实际 {:?}",
            full_payload.ambient_sound_id
        );

        // Attenuated 档（Solidify）：Unknown 时返回 None（无需发缩减包）
        let attenuated = build_payload_for_tier(EraType::Unknown, RealmGateTier::Attenuated);
        assert!(
            attenuated.is_none(),
            "Unknown 时代 Attenuated 档应返回 None（无天象，无需发任何包）；\
             实际 {:?}",
            attenuated
        );

        // None 档（Condense-）：始终返回 None
        let none_tier = build_payload_for_tier(EraType::Unknown, RealmGateTier::None);
        assert!(
            none_tier.is_none(),
            "Unknown 时代 None 档应返回 None（境界不足，不发包）；实际 {:?}",
            none_tier
        );
    }

    /// 验证灾劫时代 Spirit 玩家连接时会产生完整 payload（on_join 路径）。
    #[test]
    fn on_join_calamity_spirit_gets_full_payload() {
        let payload = build_payload_for_tier(EraType::Calamity, realm_gate_tier(Realm::Spirit));
        let payload = payload.expect("Spirit 玩家 + Calamity 应产生完整 payload");
        assert_eq!(payload.sky_tint_hex, CALAMITY_SKY_TINT_HEX);
        assert!(!payload.ambient_sound_id.is_empty());
    }

    /// 验证灾劫时代 Condense（凝脉）玩家连接时不产生 payload。
    #[test]
    fn on_join_calamity_condense_gets_no_payload() {
        let payload = build_payload_for_tier(EraType::Calamity, realm_gate_tier(Realm::Condense));
        assert!(
            payload.is_none(),
            "凝脉境界 < 固元，连接时不应收到时代天象包；实际 = {:?}",
            payload
        );
    }

    // ── 渐变时长一致性 ─────────────────────────────────────────────────────────

    #[test]
    fn all_era_full_payloads_have_same_transition_ticks() {
        for era in [EraType::Calamity, EraType::Change, EraType::Deduction] {
            let payload = build_payload_for_tier(era, RealmGateTier::Full)
                .unwrap_or_else(|| panic!("Full + {} 应产生 payload", era.as_wire_str()));
            assert_eq!(
                payload.transition_ticks,
                ERA_TRANSITION_TICKS,
                "{} 时代 Full payload 过渡时长必须为 {} tick；实际 = {}",
                era.as_wire_str(),
                ERA_TRANSITION_TICKS,
                payload.transition_ticks
            );
        }
    }

    #[test]
    fn all_era_attenuated_payloads_have_same_transition_ticks() {
        for era in [EraType::Calamity, EraType::Change] {
            let payload = build_payload_for_tier(era, RealmGateTier::Attenuated)
                .unwrap_or_else(|| panic!("Attenuated + {} 应产生 payload", era.as_wire_str()));
            assert_eq!(
                payload.transition_ticks,
                ERA_TRANSITION_TICKS,
                "{} 时代 Attenuated payload 过渡时长必须为 {} tick；实际 = {}",
                era.as_wire_str(),
                ERA_TRANSITION_TICKS,
                payload.transition_ticks
            );
        }
    }

    // ── Bevy App 集成：EraChangedEvent → payload 发送路径 ─────────────────────

    #[test]
    fn era_ambiance_on_era_changed_system_no_panic_without_clients() {
        use valence::prelude::{App, Update};

        let mut app = App::new();
        app.add_event::<EraChangedEvent>();
        app.insert_resource(WorldEraState::default());
        app.add_systems(Update, era_ambiance_on_era_changed_system);

        // 发送 EraChangedEvent 但无玩家 client，系统不应 panic
        app.world_mut().send_event(EraChangedEvent {
            old_era: EraType::Unknown,
            new_era: EraType::Calamity,
            onset_tick: 100,
        });

        app.update(); // 不应 panic
    }

    #[test]
    fn era_ambiance_on_join_system_no_panic_without_world_era() {
        use valence::prelude::{App, Update};

        let mut app = App::new();
        // 故意不插入 WorldEraState Resource，验证 Option<Res<...>> 安全路径
        app.add_systems(Update, era_ambiance_on_join_system);

        app.update(); // 不应 panic
    }
}
