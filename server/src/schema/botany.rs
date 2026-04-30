use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BotanyHarvestModeV1 {
    Manual,
    Auto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BotanyHarvestPhaseV1 {
    Pending,
    InProgress,
    Completed,
    Interrupted,
    Trampled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BotanyHarvestProgressV1 {
    pub session_id: String,
    pub target_id: String,
    pub target_name: String,
    pub plant_kind: String,
    pub mode: BotanyHarvestModeV1,
    pub progress: f64,
    pub auto_selectable: bool,
    pub request_pending: bool,
    pub interrupted: bool,
    pub completed: bool,
    pub detail: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hazard_hints: Vec<String>,
    /// plan §1.3 投影锚定：目标植物世界坐标（可选），client 侧 world→screen 投影定位浮窗。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_pos: Option<[f64; 3]>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BotanyModelOverlayV1 {
    None,
    Emissive,
    DualPhase,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BotanyPlantV2RenderProfileV1 {
    pub plant_id: String,
    pub base_mesh_ref: String,
    pub tint_rgb: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tint_rgb_secondary: Option<u32>,
    pub model_overlay: BotanyModelOverlayV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BotanySkillV1 {
    pub level: u64,
    pub xp: u64,
    pub xp_to_next_level: u64,
    pub auto_unlock_level: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BotanyHarvestRequestV1 {
    pub session_id: String,
    pub mode: BotanyHarvestModeV1,
}

// plan §7 生态快照：server → agent / 运维观测用，每 N tick 聚合发布。

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BotanyVariantV1 {
    None,
    Thunder,
    Tainted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BotanyPlantCountEntryV1 {
    pub kind: String,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BotanyVariantCountEntryV1 {
    pub variant: BotanyVariantV1,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BotanyZoneEcologyV1 {
    pub zone: String,
    pub spirit_qi: f64,
    pub plant_counts: Vec<BotanyPlantCountEntryV1>,
    pub variant_counts: Vec<BotanyVariantCountEntryV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BotanyEcologySnapshotV1 {
    pub v: u8,
    pub tick: u64,
    pub zones: Vec<BotanyZoneEcologyV1>,
}

impl BotanyEcologySnapshotV1 {
    pub fn new(tick: u64, zones: Vec<BotanyZoneEcologyV1>) -> Self {
        Self { v: 1, tick, zones }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn botany_harvest_progress_roundtrip() {
        let payload = BotanyHarvestProgressV1 {
            session_id: "session-botany-01".to_string(),
            target_id: "plant-1".to_string(),
            target_name: "开脉草".to_string(),
            plant_kind: "ning_mai_cao".to_string(),
            mode: BotanyHarvestModeV1::Manual,
            progress: 0.5,
            auto_selectable: true,
            request_pending: false,
            interrupted: false,
            completed: false,
            detail: "晨露未散".to_string(),
            hazard_hints: vec!["靠近 -0.4 真元/s 叠加".to_string()],
            target_pos: Some([10.5, 64.0, 10.5]),
        };

        let json = serde_json::to_string(&payload).unwrap();
        let back: BotanyHarvestProgressV1 = serde_json::from_str(&json).unwrap();
        assert_eq!(back, payload);
    }

    #[test]
    fn botany_harvest_progress_target_pos_is_optional() {
        let payload = BotanyHarvestProgressV1 {
            session_id: "session-botany-02".to_string(),
            target_id: "plant-2".to_string(),
            target_name: "赤髓草".to_string(),
            plant_kind: "chi_sui_cao".to_string(),
            mode: BotanyHarvestModeV1::Auto,
            progress: 1.0,
            auto_selectable: true,
            request_pending: false,
            interrupted: false,
            completed: true,
            detail: String::new(),
            hazard_hints: Vec::new(),
            target_pos: None,
        };

        let json = serde_json::to_string(&payload).unwrap();
        assert!(
            !json.contains("target_pos"),
            "None target_pos should be omitted from wire payload, got {json}"
        );
        let back: BotanyHarvestProgressV1 = serde_json::from_str(&json).unwrap();
        assert_eq!(back, payload);
    }

    #[test]
    fn botany_ecology_snapshot_roundtrip() {
        let payload = BotanyEcologySnapshotV1::new(
            1200,
            vec![BotanyZoneEcologyV1 {
                zone: "spawn".to_string(),
                spirit_qi: 0.45,
                plant_counts: vec![BotanyPlantCountEntryV1 {
                    kind: "ci_she_hao".to_string(),
                    count: 4,
                }],
                variant_counts: vec![
                    BotanyVariantCountEntryV1 {
                        variant: BotanyVariantV1::None,
                        count: 3,
                    },
                    BotanyVariantCountEntryV1 {
                        variant: BotanyVariantV1::Thunder,
                        count: 1,
                    },
                ],
            }],
        );

        let json = serde_json::to_string(&payload).unwrap();
        let back: BotanyEcologySnapshotV1 = serde_json::from_str(&json).unwrap();
        assert_eq!(back, payload);
        assert_eq!(back.v, 1);
    }

    #[test]
    fn botany_skill_roundtrip() {
        let payload = BotanySkillV1 {
            level: 3,
            xp: 240,
            xp_to_next_level: 400,
            auto_unlock_level: 3,
        };

        let json = serde_json::to_string(&payload).unwrap();
        let back: BotanySkillV1 = serde_json::from_str(&json).unwrap();
        assert_eq!(back, payload);
    }

    #[test]
    fn botany_v2_render_profile_roundtrip() {
        let payload = BotanyPlantV2RenderProfileV1 {
            plant_id: "ying_yuan_gu".to_string(),
            base_mesh_ref: "red_mushroom".to_string(),
            tint_rgb: 0xFFA040,
            tint_rgb_secondary: None,
            model_overlay: BotanyModelOverlayV1::Emissive,
        };

        let json = serde_json::to_string(&payload).unwrap();
        let back: BotanyPlantV2RenderProfileV1 = serde_json::from_str(&json).unwrap();
        assert_eq!(back, payload);
    }
}
