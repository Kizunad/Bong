use serde::{de::Error as _, Deserialize, Deserializer, Serialize};
use std::collections::HashMap;

use crate::fauna::rat_phase::RatDensityHeatmapV1;
use crate::npc::faction::{FactionId, FactionRank};
use crate::world::era::EraType;

use super::common::{GameEventType, NpcStateKind, PlayerTrend};
use super::cultivation::{CultivationSnapshotV1, LifeRecordSnapshotV1};
use super::social::PlayerSocialSnapshotV1;

pub type Vec3 = [f64; 3];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SeasonV1 {
    Summer,
    SummerToWinter,
    Winter,
    WinterToSummer,
}

impl From<crate::world::season::Season> for SeasonV1 {
    fn from(value: crate::world::season::Season) -> Self {
        match value {
            crate::world::season::Season::Summer => Self::Summer,
            crate::world::season::Season::SummerToWinter => Self::SummerToWinter,
            crate::world::season::Season::Winter => Self::Winter,
            crate::world::season::Season::WinterToSummer => Self::WinterToSummer,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SeasonStateV1 {
    pub season: SeasonV1,
    pub tick_into_phase: u64,
    pub phase_total_ticks: u64,
    pub year_index: u64,
}

impl From<crate::world::season::SeasonState> for SeasonStateV1 {
    fn from(value: crate::world::season::SeasonState) -> Self {
        Self {
            season: value.season.into(),
            tick_into_phase: value.tick_into_phase,
            phase_total_ticks: value.phase_total_ticks,
            year_index: value.year_index,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ZoneStatusV1 {
    #[default]
    Normal,
    RaceOut,
    Collapsed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerPowerBreakdown {
    pub combat: f64,
    pub wealth: f64,
    pub social: f64,
    pub karma: f64,
    pub territory: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerProfile {
    pub uuid: String,
    pub name: String,
    pub realm: String,
    pub composite_power: f64,
    pub breakdown: PlayerPowerBreakdown,
    pub trend: PlayerTrend,
    pub active_hours: f64,
    pub zone: String,
    pub pos: Vec3,
    pub recent_kills: u32,
    pub recent_deaths: u32,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub cultivation: Option<CultivationSnapshotV1>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub life_record: Option<LifeRecordSnapshotV1>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub social: Option<PlayerSocialSnapshotV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NpcSnapshot {
    pub id: String,
    pub kind: String,
    pub zone: String,
    pub pos: Vec3,
    pub state: NpcStateKind,
    pub blackboard: HashMap<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub digest: Option<NpcDigestV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NpcDigestV1 {
    pub archetype: String,
    pub age_band: String,
    pub age_ratio: f64,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub realm: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub faction_id: Option<FactionId>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub position: Option<Vec3>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub disciple: Option<DiscipleSummaryV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscipleSummaryV1 {
    pub faction_id: FactionId,
    pub rank: FactionRank,
    pub loyalty: f64,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub lineage: Option<LineageSummaryV1>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub mission_queue: Option<MissionQueueSummaryV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LineageSummaryV1 {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub master_id: Option<String>,
    pub disciple_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MissionQueueSummaryV1 {
    pub pending_count: u32,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub top_mission_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactionSummaryV1 {
    pub id: FactionId,
    pub loyalty_bias: f64,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub leader_lineage: Option<LineageSummaryV1>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub mission_queue: Option<MissionQueueSummaryV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZoneSnapshot {
    pub name: String,
    pub spirit_qi: f64,
    pub danger_level: u8,
    #[serde(default)]
    pub status: ZoneStatusV1,
    pub active_events: Vec<String>,
    pub player_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameEvent {
    #[serde(rename = "type")]
    pub event_type: GameEventType,
    pub tick: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub player: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<HashMap<String, serde_json::Value>>,
}

/// plan-era-state-v1 P2：时代状态 IPC 结构体，嵌入 WorldStateV1.era（Optional）。
///
/// TypeBox source of truth: `EraStateV1` in `agent/packages/schema/src/world-state.ts`。
/// era_type="unknown" 时 server 不填此字段（None → 不序列化）。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EraStateV1 {
    /// 时代类型（wire 字符串：calamity / change / deduction / unknown）。
    pub era_type: EraTypeV1Wire,
    /// 强度 [0.0, 1.0]，由 |spirit_qi_delta| / 0.05 推算（clamp 到 1.0）。
    pub intensity: f64,
    /// 时代宣告时的 server tick。
    pub onset_tick: u64,
}

/// 时代类型 wire 枚举（与 TypeBox EraTypeV1 字面量对应）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EraTypeV1Wire {
    Calamity,
    Change,
    Deduction,
    Unknown,
}

impl From<EraType> for EraTypeV1Wire {
    fn from(era: EraType) -> Self {
        match era {
            EraType::Calamity => EraTypeV1Wire::Calamity,
            EraType::Change => EraTypeV1Wire::Change,
            EraType::Deduction => EraTypeV1Wire::Deduction,
            EraType::Unknown => EraTypeV1Wire::Unknown,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorldStateV1 {
    #[serde(deserialize_with = "deserialize_v1_version")]
    pub v: u8,
    pub ts: u64,
    pub tick: u64,
    pub season_state: SeasonStateV1,
    pub players: Vec<PlayerProfile>,
    pub npcs: Vec<NpcSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub factions: Option<Vec<FactionSummaryV1>>,
    pub rat_density_heatmap: RatDensityHeatmapV1,
    pub zones: Vec<ZoneSnapshot>,
    pub recent_events: Vec<GameEvent>,
    /// 当前天道时代状态（plan-era-state-v1 P2）。
    /// era_type=Unknown 时不序列化（None）；非 Unknown 时填充 EraStateV1。
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub era: Option<EraStateV1>,
}

fn deserialize_v1_version<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: Deserializer<'de>,
{
    let version = u8::deserialize(deserializer)?;
    if version == 1 {
        Ok(version)
    } else {
        Err(D::Error::custom(format!(
            "WorldStateV1.v must be 1, got {version}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::channels::{CH_PLAYER_CHAT, CH_WORLD_STATE};
    use serde_json::{json, Value};

    fn sample_world_state_value() -> Value {
        serde_json::from_str(include_str!(
            "../../../agent/packages/schema/samples/world-state.sample.json"
        ))
        .expect("world-state sample should parse into JSON value")
    }

    #[test]
    fn deserialize_world_state_sample() {
        let json = include_str!("../../../agent/packages/schema/samples/world-state.sample.json");
        let state: WorldStateV1 = serde_json::from_str(json)
            .expect("world-state.sample.json should deserialize into WorldStateV1");

        assert_eq!(state.v, 1);
        assert_eq!(state.tick, 84000);
        assert_eq!(state.season_state.season, SeasonV1::Summer);
        assert_eq!(state.players.len(), 2);
        assert_eq!(state.players[0].name, "Steve");
        assert_eq!(state.players[0].pos, [128.5, 66.0, 200.3]);
        let social = state.players[0]
            .social
            .as_ref()
            .expect("world-state sample should carry a social snapshot");
        assert_eq!(social.renown.fame, 2);
        assert_eq!(social.relationships[0].peer, "char:new_player_1");
        assert_eq!(social.exposed_to_count, 1);
        assert_eq!(state.npcs.len(), 1);
        assert_eq!(state.npcs[0].id, "npc_001");
        assert_eq!(
            state.npcs[0]
                .digest
                .as_ref()
                .and_then(|digest| digest.disciple.as_ref())
                .map(|d| d.faction_id),
            Some(FactionId::Neutral)
        );
        assert_eq!(state.factions.as_ref().map(Vec::len), Some(3));
        assert_eq!(
            state
                .rat_density_heatmap
                .zones
                .get("spawn")
                .map(|snapshot| snapshot.gregarious),
            Some(2)
        );
        assert_eq!(state.zones.len(), 2);
        assert_eq!(state.recent_events.len(), 2);
        // plan-era-state-v1 P2：sample 中含 era 字段
        let era = state
            .era
            .expect("world-state.sample.json 应含 era 字段（plan-era-state-v1 P2）");
        assert_eq!(
            era.era_type,
            EraTypeV1Wire::Calamity,
            "sample 中 era_type 应为 calamity"
        );
        assert!(
            (era.intensity - 0.9).abs() < 1e-9,
            "sample 中 era.intensity 应为 0.9；实际 = {}",
            era.intensity
        );
        assert_eq!(era.onset_tick, 80000, "sample 中 era.onset_tick 应为 80000");
        assert_eq!(CH_WORLD_STATE, "bong:world_state");
        assert_eq!(CH_PLAYER_CHAT, "bong:player_chat");
    }

    #[test]
    fn roundtrip_world_state() {
        let json = include_str!("../../../agent/packages/schema/samples/world-state.sample.json");
        let state: WorldStateV1 = serde_json::from_str(json).unwrap();
        let re_json = serde_json::to_string(&state).unwrap();
        let state2: WorldStateV1 = serde_json::from_str(&re_json).unwrap();
        assert_eq!(state.v, state2.v);
        assert_eq!(state.tick, state2.tick);
        assert_eq!(state.players.len(), state2.players.len());
        assert_eq!(
            state.factions.as_ref().map(Vec::len),
            state2.factions.as_ref().map(Vec::len)
        );
    }

    #[test]
    fn deserialize_world_state_sample_rejects_wrong_version() {
        let mut value = sample_world_state_value();
        value["v"] = json!(2);

        assert!(serde_json::from_value::<WorldStateV1>(value).is_err());
    }

    #[test]
    fn deserialize_world_state_sample_rejects_unknown_top_level_field() {
        let mut value = sample_world_state_value();
        value["realm_clock"] = json!(99);

        assert!(serde_json::from_value::<WorldStateV1>(value).is_err());
    }

    #[test]
    fn deserialize_world_state_sample_rejects_unknown_nested_player_field() {
        let mut value = sample_world_state_value();
        value["players"][0]["rogue_power"] = json!(999);

        assert!(serde_json::from_value::<WorldStateV1>(value).is_err());
    }

    #[test]
    fn deserialize_world_state_defaults_missing_zone_status() {
        let mut value = sample_world_state_value();
        value["zones"][0]
            .as_object_mut()
            .expect("sample zone should be an object")
            .remove("status");

        let state: WorldStateV1 = serde_json::from_value(value).expect("deserialize world state");

        assert_eq!(state.zones[0].status, ZoneStatusV1::Normal);
    }

    #[test]
    fn deserialize_world_state_accepts_collapsed_zone_status() {
        let mut value = sample_world_state_value();
        value["zones"][0]["status"] = json!("collapsed");

        let state: WorldStateV1 = serde_json::from_value(value).expect("deserialize world state");

        assert_eq!(state.zones[0].status, ZoneStatusV1::Collapsed);
    }

    #[test]
    fn deserialize_world_state_accepts_race_out_zone_status() {
        let mut value = sample_world_state_value();
        value["zones"][0]["status"] = json!("race_out");

        let state: WorldStateV1 = serde_json::from_value(value).expect("deserialize world state");

        assert_eq!(state.zones[0].status, ZoneStatusV1::RaceOut);
    }

    // ── plan-era-state-v1 P2：EraStateV1 Rust serde 正反 sample 对拍 ──────────

    #[test]
    fn deserialize_era_state_calamity() {
        let json = r#"{"era_type":"calamity","intensity":0.8,"onset_tick":500}"#;
        let era: EraStateV1 =
            serde_json::from_str(json).expect("EraStateV1 calamity 应正常反序列化");
        assert_eq!(
            era.era_type,
            EraTypeV1Wire::Calamity,
            "era_type 应为 Calamity"
        );
        assert!((era.intensity - 0.8).abs() < 1e-9, "intensity 应为 0.8");
        assert_eq!(era.onset_tick, 500, "onset_tick 应为 500");
    }

    #[test]
    fn deserialize_era_state_change() {
        let json = r#"{"era_type":"change","intensity":0.5,"onset_tick":1200}"#;
        let era: EraStateV1 = serde_json::from_str(json).expect("EraStateV1 change 应正常反序列化");
        assert_eq!(era.era_type, EraTypeV1Wire::Change, "era_type 应为 Change");
        assert!((era.intensity - 0.5).abs() < 1e-9, "intensity 应为 0.5");
    }

    #[test]
    fn deserialize_era_state_deduction() {
        let json = r#"{"era_type":"deduction","intensity":0.3,"onset_tick":2400}"#;
        let era: EraStateV1 =
            serde_json::from_str(json).expect("EraStateV1 deduction 应正常反序列化");
        assert_eq!(
            era.era_type,
            EraTypeV1Wire::Deduction,
            "era_type 应为 Deduction"
        );
    }

    #[test]
    fn deserialize_era_state_rejects_extra_field() {
        let json = r#"{"era_type":"calamity","intensity":0.8,"onset_tick":500,"rogue":99}"#;
        assert!(
            serde_json::from_str::<EraStateV1>(json).is_err(),
            "EraStateV1 应拒绝额外字段（deny_unknown_fields）"
        );
    }

    #[test]
    fn deserialize_era_state_rejects_unknown_era_type() {
        let json = r#"{"era_type":"invalid_type","intensity":0.5,"onset_tick":100}"#;
        assert!(
            serde_json::from_str::<EraStateV1>(json).is_err(),
            "EraStateV1 应拒绝未知 era_type 字符串"
        );
    }

    #[test]
    fn era_type_v1_wire_roundtrip_all_variants() {
        for (era_type, wire_str) in [
            (EraTypeV1Wire::Calamity, "\"calamity\""),
            (EraTypeV1Wire::Change, "\"change\""),
            (EraTypeV1Wire::Deduction, "\"deduction\""),
            (EraTypeV1Wire::Unknown, "\"unknown\""),
        ] {
            let serialized = serde_json::to_string(&era_type)
                .unwrap_or_else(|_| panic!("序列化 {:?} 不应失败", era_type));
            assert_eq!(
                serialized, wire_str,
                "EraTypeV1Wire::{:?} wire 字符串应为 {wire_str}",
                era_type
            );
            let deserialized: EraTypeV1Wire = serde_json::from_str(wire_str)
                .unwrap_or_else(|_| panic!("反序列化 {wire_str} 不应失败"));
            assert_eq!(
                deserialized, era_type,
                "反序列化 {wire_str} 应还原为 {:?}",
                era_type
            );
        }
    }

    #[test]
    fn era_type_v1_wire_from_era_type_all_variants() {
        use crate::world::era::EraType;

        assert_eq!(
            EraTypeV1Wire::from(EraType::Calamity),
            EraTypeV1Wire::Calamity
        );
        assert_eq!(EraTypeV1Wire::from(EraType::Change), EraTypeV1Wire::Change);
        assert_eq!(
            EraTypeV1Wire::from(EraType::Deduction),
            EraTypeV1Wire::Deduction
        );
        assert_eq!(
            EraTypeV1Wire::from(EraType::Unknown),
            EraTypeV1Wire::Unknown
        );
    }

    #[test]
    fn world_state_era_field_optional_absent_is_none() {
        // world-state.sample.json に era フィールドがない場合は None になるべき
        // (we'll add era to the sample; this test verifies removal => None)
        let mut value = sample_world_state_value();
        value.as_object_mut().map(|obj| obj.remove("era"));
        let state: WorldStateV1 =
            serde_json::from_value(value).expect("era フィールド不在でも逆シリアル化できるべき");
        assert!(state.era.is_none(), "era フィールド不在 → None であるべき");
    }

    #[test]
    fn world_state_era_field_calamity_roundtrip() {
        let mut value = sample_world_state_value();
        value["era"] = json!({
            "era_type": "calamity",
            "intensity": 0.9,
            "onset_tick": 1000
        });
        let state: WorldStateV1 =
            serde_json::from_value(value).expect("WorldStateV1 含 era=calamity 应反序列化成功");
        let era = state.era.expect("era 字段应存在");
        assert_eq!(
            era.era_type,
            EraTypeV1Wire::Calamity,
            "era_type 应为 Calamity"
        );
        assert!((era.intensity - 0.9).abs() < 1e-9, "intensity 应为 0.9");
        assert_eq!(era.onset_tick, 1000);

        // Roundtrip: serialize back and re-parse
        let re_json = serde_json::to_string(&state).expect("序列化不应失败");
        let state2: WorldStateV1 = serde_json::from_str(&re_json).expect("重新反序列化不应失败");
        let era2 = state2.era.expect("roundtrip 后 era 应存在");
        assert_eq!(era2.era_type, EraTypeV1Wire::Calamity);
        assert!((era2.intensity - 0.9).abs() < 1e-9);
        assert_eq!(era2.onset_tick, 1000);
    }

    #[test]
    fn world_state_era_field_none_not_serialized() {
        // era=None 时序列化产物不应有 "era" key
        let mut value = sample_world_state_value();
        value.as_object_mut().map(|obj| obj.remove("era"));
        let state: WorldStateV1 = serde_json::from_value(value).expect("deserialize ok");
        assert!(state.era.is_none());
        let serialized = serde_json::to_string(&state).expect("serialize ok");
        assert!(
            !serialized.contains("\"era\""),
            "era=None 时序列化产物不应包含 'era' key；actual: {serialized}"
        );
    }

    #[test]
    fn deserialize_era_state_sample_json() {
        // 验证 era-state.sample.json 三时代正样本均能正常反序列化
        let json = include_str!("../../../agent/packages/schema/samples/era-state.sample.json");
        let samples: serde_json::Value =
            serde_json::from_str(json).expect("era-state.sample.json 应为有效 JSON");

        let calamity = &samples["calamity"];
        let era: EraStateV1 = serde_json::from_value(calamity.clone())
            .expect("era-state.sample.json calamity 应反序列化为 EraStateV1");
        assert_eq!(era.era_type, EraTypeV1Wire::Calamity);

        let change = &samples["change"];
        let era: EraStateV1 = serde_json::from_value(change.clone())
            .expect("era-state.sample.json change 应反序列化为 EraStateV1");
        assert_eq!(era.era_type, EraTypeV1Wire::Change);

        let deduction = &samples["deduction"];
        let era: EraStateV1 = serde_json::from_value(deduction.clone())
            .expect("era-state.sample.json deduction 应反序列化为 EraStateV1");
        assert_eq!(era.era_type, EraTypeV1Wire::Deduction);
    }
}
