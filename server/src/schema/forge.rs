//! 炼器（武器）IPC 共享原子（plan-forge-v1 §4 数据契约）。
//!
//! 与 TypeScript `agent/packages/schema/src/forge.ts` 1:1 镜像。
//! `server-data` / `client-request` 模块复用本文件的类型组装具体 payload。

use serde::{Deserialize, Serialize};

use crate::cultivation::components::ColorKind;

/// plan §1.3 四步串行（与 `ForgeSession::current_step` 对齐）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForgeStepV1 {
    Billet,
    Tempering,
    Inscription,
    Consecration,
    Done,
}

/// plan §2 品阶四阶（与 `compute_achieved_tier` 输出对齐）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeaponTierV1 {
    Mundane = 1,
    Artifact = 2,
    Spirit = 3,
    Dao = 4,
}

/// 淬炼击键（J=Light, K=Heavy, L=Fold）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TemperBeatV1 {
    #[serde(rename = "L")]
    Light,
    #[serde(rename = "H")]
    Heavy,
    #[serde(rename = "F")]
    Fold,
}

impl From<crate::forge::blueprint::TemperBeat> for TemperBeatV1 {
    fn from(b: crate::forge::blueprint::TemperBeat) -> Self {
        match b {
            crate::forge::blueprint::TemperBeat::Light => Self::Light,
            crate::forge::blueprint::TemperBeat::Heavy => Self::Heavy,
            crate::forge::blueprint::TemperBeat::Fold => Self::Fold,
        }
    }
}

/// plan §1.3 五结果桶。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForgeOutcomeBucketV1 {
    Perfect,
    Good,
    Flawed,
    Waste,
    Explode,
}

impl From<crate::forge::events::ForgeBucket> for ForgeOutcomeBucketV1 {
    fn from(b: crate::forge::events::ForgeBucket) -> Self {
        match b {
            crate::forge::events::ForgeBucket::Perfect => Self::Perfect,
            crate::forge::events::ForgeBucket::Good => Self::Good,
            crate::forge::events::ForgeBucket::Flawed => Self::Flawed,
            crate::forge::events::ForgeBucket::Waste => Self::Waste,
            crate::forge::events::ForgeBucket::Explode => Self::Explode,
        }
    }
}

// ─── server → client 推送 payload（plan §4 数据契约） ───────────────────

/// 砧信息快照。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeaponForgeStationDataV1 {
    pub station_id: String,
    pub tier: u8,
    pub integrity: f32,
    pub owner_name: String,
    pub has_session: bool,
}

/// 锻造会话实时状态。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForgeSessionDataV1 {
    pub session_id: u64,
    pub blueprint_id: String,
    pub blueprint_name: String,
    pub active: bool,
    pub current_step: ForgeStepV1,
    pub step_index: u32,
    pub achieved_tier: u32,
    pub step_state: ForgeStepStateDataV1,
}

/// 各步实时状态（按 current_step 选择对应 variant）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum ForgeStepStateDataV1 {
    #[serde(rename = "billet")]
    Billet {
        /// 当前投入材料：{material -> count}
        materials_in: Vec<(String, u32)>,
        active_carrier: Option<String>,
        resolved_tier_cap: u32,
    },
    #[serde(rename = "tempering")]
    Tempering {
        /// 待击 pattern 序列
        pattern: Vec<TemperBeatV1>,
        beat_cursor: u32,
        hits: u32,
        misses: u32,
        deviation: u32,
        qi_spent: f64,
    },
    #[serde(rename = "inscription")]
    Inscription {
        filled_slots: u32,
        max_slots: u32,
        failed: bool,
    },
    #[serde(rename = "consecration")]
    Consecration {
        qi_injected: f64,
        qi_required: f64,
        color_imprint: Option<ColorKind>,
    },
    None,
}

/// 锻造结果结算推送。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForgeOutcomeDataV1 {
    pub session_id: u64,
    pub blueprint_id: String,
    pub bucket: ForgeOutcomeBucketV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weapon_item: Option<String>,
    pub quality: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<ColorKind>,
    pub side_effects: Vec<String>,
    pub achieved_tier: u32,
    /// 是否走了残缺匹配路径
    pub flawed_path: bool,
}

/// 已学图谱书快照。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForgeBlueprintBookDataV1 {
    pub learned: Vec<ForgeBlueprintEntryV1>,
    pub current_index: u32,
}

/// 单条已学图谱条目（与 client BlueprintScrollStore 对齐）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForgeBlueprintEntryV1 {
    pub id: String,
    pub display_name: String,
    pub tier_cap: u8,
    pub step_count: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forge_step_serde_snake_case() {
        assert_eq!(
            serde_json::to_string(&ForgeStepV1::Billet).unwrap(),
            "\"billet\""
        );
        assert_eq!(
            serde_json::to_string(&ForgeStepV1::Tempering).unwrap(),
            "\"tempering\""
        );
    }

    #[test]
    fn outcome_bucket_serde_roundtrip() {
        let s = serde_json::to_string(&ForgeOutcomeBucketV1::Explode).unwrap();
        assert_eq!(s, "\"explode\"");
        let back: ForgeOutcomeBucketV1 = serde_json::from_str("\"flawed\"").unwrap();
        assert_eq!(back, ForgeOutcomeBucketV1::Flawed);
    }

    #[test]
    fn forge_step_state_billet_roundtrip() {
        let state = ForgeStepStateDataV1::Billet {
            materials_in: vec![("iron_ingot".into(), 3)],
            active_carrier: None,
            resolved_tier_cap: 1,
        };
        let s = serde_json::to_string(&state).unwrap();
        let back: ForgeStepStateDataV1 = serde_json::from_str(&s).unwrap();
        assert_eq!(back, state);
    }

    #[test]
    fn forge_step_state_tempering_roundtrip() {
        let state = ForgeStepStateDataV1::Tempering {
            pattern: vec![TemperBeatV1::Light, TemperBeatV1::Heavy],
            beat_cursor: 0,
            hits: 0,
            misses: 0,
            deviation: 0,
            qi_spent: 0.0,
        };
        let s = serde_json::to_string(&state).unwrap();
        let back: ForgeStepStateDataV1 = serde_json::from_str(&s).unwrap();
        assert_eq!(back, state);
    }

    #[test]
    fn forge_outcome_data_roundtrip() {
        let outcome = ForgeOutcomeDataV1 {
            session_id: 1,
            blueprint_id: "iron_sword_v0".into(),
            bucket: ForgeOutcomeBucketV1::Perfect,
            weapon_item: Some("iron_sword".into()),
            quality: 1.0,
            color: None,
            side_effects: vec![],
            achieved_tier: 1,
            flawed_path: false,
        };
        let s = serde_json::to_string(&outcome).unwrap();
        let back: ForgeOutcomeDataV1 = serde_json::from_str(&s).unwrap();
        assert_eq!(back, outcome);
    }

    #[test]
    fn forge_station_data_roundtrip() {
        let data = WeaponForgeStationDataV1 {
            station_id: "s1".into(),
            tier: 1,
            integrity: 0.95,
            owner_name: "test".into(),
            has_session: false,
        };
        let s = serde_json::to_string(&data).unwrap();
        let back: WeaponForgeStationDataV1 = serde_json::from_str(&s).unwrap();
        assert_eq!(back, data);
    }

    #[test]
    fn temper_beat_serde() {
        assert_eq!(
            serde_json::to_string(&TemperBeatV1::Light).unwrap(),
            "\"L\""
        );
        assert_eq!(
            serde_json::to_string(&TemperBeatV1::Heavy).unwrap(),
            "\"H\""
        );
        assert_eq!(
            serde_json::to_string(&TemperBeatV1::Fold).unwrap(),
            "\"F\""
        );
        let back: TemperBeatV1 = serde_json::from_str("\"L\"").unwrap();
        assert_eq!(back, TemperBeatV1::Light);
    }

    #[test]
    fn blueprint_entry_roundtrip() {
        let entry = ForgeBlueprintEntryV1 {
            id: "iron_sword_v0".into(),
            display_name: "铁剑（测试）".into(),
            tier_cap: 1,
            step_count: 1,
        };
        let s = serde_json::to_string(&entry).unwrap();
        let back: ForgeBlueprintEntryV1 = serde_json::from_str(&s).unwrap();
        assert_eq!(back, entry);
    }
}
