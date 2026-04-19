//! 炼丹 IPC 共享原子（plan-alchemy-v1 §4 数据契约）。
//!
//! 与 TypeScript `agent/packages/schema/src/alchemy.ts` 1:1 镜像。
//! `server-data` / `client-request` 模块复用本文件的类型组装具体 payload。

use serde::{Deserialize, Serialize};

use crate::cultivation::components::ColorKind;

/// plan §1.3 五结果桶（与 `crate::alchemy::outcome::OutcomeBucket` 不同 — 此为线上序列化形式）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlchemyOutcomeBucketV1 {
    Perfect,
    Good,
    Flawed,
    Waste,
    Explode,
}

impl From<crate::alchemy::outcome::OutcomeBucket> for AlchemyOutcomeBucketV1 {
    fn from(bucket: crate::alchemy::outcome::OutcomeBucket) -> Self {
        use crate::alchemy::outcome::OutcomeBucket as B;
        match bucket {
            B::Perfect => Self::Perfect,
            B::Good => Self::Good,
            B::Flawed => Self::Flawed,
            B::Waste => Self::Waste,
            B::Explode => Self::Explode,
        }
    }
}

/// plan §1.3 玩家干预 — discriminated union by `kind`。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AlchemyInterventionV1 {
    AdjustTemp { temp: f64 },
    InjectQi { qi: f64 },
    AutoProfile { profile_id: String },
}

impl From<AlchemyInterventionV1> for crate::alchemy::Intervention {
    fn from(value: AlchemyInterventionV1) -> Self {
        match value {
            AlchemyInterventionV1::AdjustTemp { temp } => Self::AdjustTemp(temp),
            AlchemyInterventionV1::InjectQi { qi } => Self::InjectQi(qi),
            AlchemyInterventionV1::AutoProfile { profile_id } => Self::AutoProfile(profile_id),
        }
    }
}

impl From<&crate::alchemy::Intervention> for AlchemyInterventionV1 {
    fn from(value: &crate::alchemy::Intervention) -> Self {
        match value {
            crate::alchemy::Intervention::AdjustTemp(t) => Self::AdjustTemp { temp: *t },
            crate::alchemy::Intervention::InjectQi(q) => Self::InjectQi { qi: *q },
            crate::alchemy::Intervention::AutoProfile(id) => Self::AutoProfile {
                profile_id: id.clone(),
            },
        }
    }
}

/// 单条已学方子（与 client `RecipeScrollStore.RecipeEntry` 对齐）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlchemyRecipeEntryV1 {
    pub id: String,
    pub display_name: String,
    pub body_text: String,
    pub author: String,
    pub era: String,
    pub max_known: u32,
}

/// 中途投料阶段提示（plan §1.3）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlchemyStageHintV1 {
    pub at_tick: u32,
    pub window: u32,
    pub summary: String,
    pub completed: bool,
    pub missed: bool,
}

/// 丹毒色快照（plan §2 — 复用 `ColorKind`）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlchemyContaminationLevelV1 {
    pub color: ColorKind,
    pub current: f64,
    pub max: f64,
    pub ok: bool,
}

// ─── server → client 推送 payload（plan §4） ────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlchemyFurnaceDataV1 {
    pub furnace_id: String,
    pub tier: u8,
    pub integrity: f64,
    pub integrity_max: f64,
    pub owner_name: String,
    pub has_session: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlchemySessionDataV1 {
    pub recipe_id: Option<String>,
    pub active: bool,
    pub elapsed_ticks: u32,
    pub target_ticks: u32,
    pub temp_current: f64,
    pub temp_target: f64,
    pub temp_band: f64,
    pub qi_injected: f64,
    pub qi_target: f64,
    pub status_label: String,
    pub stages: Vec<AlchemyStageHintV1>,
    pub interventions_recent: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlchemyOutcomeForecastDataV1 {
    pub perfect_pct: f64,
    pub good_pct: f64,
    pub flawed_pct: f64,
    pub waste_pct: f64,
    pub explode_pct: f64,
    pub perfect_note: String,
    pub good_note: String,
    pub flawed_note: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlchemyOutcomeResolvedDataV1 {
    pub bucket: AlchemyOutcomeBucketV1,
    pub recipe_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pill: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub toxin_amount: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub toxin_color: Option<ColorKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qi_gain: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub side_effect_tag: Option<String>,
    pub flawed_path: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub damage: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meridian_crack: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlchemyRecipeBookDataV1 {
    pub learned: Vec<AlchemyRecipeEntryV1>,
    pub current_index: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlchemyContaminationDataV1 {
    pub levels: Vec<AlchemyContaminationLevelV1>,
    pub metabolism_note: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outcome_bucket_serde_snake_case() {
        let s = serde_json::to_string(&AlchemyOutcomeBucketV1::Perfect).unwrap();
        assert_eq!(s, "\"perfect\"");
        let back: AlchemyOutcomeBucketV1 = serde_json::from_str("\"explode\"").unwrap();
        assert_eq!(back, AlchemyOutcomeBucketV1::Explode);
    }

    #[test]
    fn intervention_inject_qi_roundtrip() {
        let v = AlchemyInterventionV1::InjectQi { qi: 1.5 };
        let s = serde_json::to_string(&v).unwrap();
        assert_eq!(s, r#"{"kind":"inject_qi","qi":1.5}"#);
        let back: AlchemyInterventionV1 = serde_json::from_str(&s).unwrap();
        assert_eq!(back, v);
    }

    #[test]
    fn intervention_adjust_temp_roundtrip() {
        let v = AlchemyInterventionV1::AdjustTemp { temp: 0.6 };
        let s = serde_json::to_string(&v).unwrap();
        let back: AlchemyInterventionV1 = serde_json::from_str(&s).unwrap();
        assert_eq!(back, v);
    }

    #[test]
    fn intervention_auto_profile_roundtrip() {
        let v = AlchemyInterventionV1::AutoProfile {
            profile_id: "kai_mai_safe".into(),
        };
        let s = serde_json::to_string(&v).unwrap();
        let back: AlchemyInterventionV1 = serde_json::from_str(&s).unwrap();
        assert_eq!(back, v);
    }

    #[test]
    fn intervention_into_alchemy_module_type() {
        let v = AlchemyInterventionV1::InjectQi { qi: 2.0 };
        let module: crate::alchemy::Intervention = v.into();
        assert!(
            matches!(module, crate::alchemy::Intervention::InjectQi(q) if (q - 2.0).abs() < 1e-9)
        );
    }

    #[test]
    fn outcome_bucket_from_module_enum() {
        let b: AlchemyOutcomeBucketV1 = crate::alchemy::outcome::OutcomeBucket::Flawed.into();
        assert_eq!(b, AlchemyOutcomeBucketV1::Flawed);
    }

    #[test]
    fn recipe_entry_roundtrip() {
        let entry = AlchemyRecipeEntryV1 {
            id: "kai_mai_pill_v0".into(),
            display_name: "开脉丹方".into(),
            body_text: "...".into(),
            author: "散修 刘三".into(),
            era: "末法 十二年".into(),
            max_known: 8,
        };
        let s = serde_json::to_string(&entry).unwrap();
        let back: AlchemyRecipeEntryV1 = serde_json::from_str(&s).unwrap();
        assert_eq!(back, entry);
    }

    #[test]
    fn stage_hint_roundtrip() {
        let h = AlchemyStageHintV1 {
            at_tick: 80,
            window: 20,
            summary: "shou_gu × 1".into(),
            completed: false,
            missed: false,
        };
        let s = serde_json::to_string(&h).unwrap();
        let back: AlchemyStageHintV1 = serde_json::from_str(&s).unwrap();
        assert_eq!(back, h);
    }

    #[test]
    fn contamination_level_roundtrip() {
        let c = AlchemyContaminationLevelV1 {
            color: ColorKind::Mellow,
            current: 0.18,
            max: 0.6,
            ok: true,
        };
        let s = serde_json::to_string(&c).unwrap();
        let back: AlchemyContaminationLevelV1 = serde_json::from_str(&s).unwrap();
        assert_eq!(back, c);
    }
}
