//! P3 §4.1 — MutationVisualSyncPayload。
//!
//! CustomPayload `bong:mutation_visual` 定义。
//! server → client 同步变异 slot 列表。
//! **不实现 client GeckoLib 渲染**（那是 Blockbench 手工工作）。
//! 只实现 schema + server emit + payload 序列化。

use serde::{Deserialize, Serialize};

use super::components::MutationStage;
use super::mutation::{ActiveMutation, MutationKind, MutationState};

/// CustomPayload channel ID。
pub const MUTATION_VISUAL_CHANNEL: &str = "bong:mutation_visual";

/// 单个变异 slot 的视觉同步数据。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MutationVisualSlot {
    pub kind: String,
    pub body_slot: String,
    pub level: u8,
}

/// 变异视觉同步 payload（server → client）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MutationVisualSyncPayload {
    /// Entity UUID 或 player name。
    pub entity: String,
    /// 当前变异阶段。
    pub stage: u8,
    /// 所有已激活的变异 slot。
    pub slots: Vec<MutationVisualSlot>,
    /// 经脉惩罚百分比。
    pub meridian_penalty: f64,
}

impl MutationVisualSyncPayload {
    /// 从 MutationState 构建 payload。
    pub fn from_state(entity_id: &str, state: &MutationState) -> Self {
        Self {
            entity: entity_id.to_string(),
            stage: state.stage as u8,
            slots: state
                .slots
                .iter()
                .map(|s| MutationVisualSlot {
                    kind: format!("{:?}", s.kind).to_ascii_lowercase(),
                    body_slot: format!("{:?}", s.slot).to_ascii_lowercase(),
                    level: s.level,
                })
                .collect(),
            meridian_penalty: state.meridian_penalty,
        }
    }
}

/// P3 §4.2 — HUD schema placeholder。
/// 丹道 HUD 面板数据，按需显示（DandaoStyle 存在时才显示）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DandaoHudPanelData {
    /// 变异阶段 (0-4)。
    pub mutation_stage: u8,
    /// 最大阶段。
    pub max_stage: u8,
    /// 累计丹毒。
    pub cumulative_toxin: f64,
    /// 到下一阶段需要的丹毒。
    pub toxin_to_next: f64,
    /// 经脉惩罚百分比。
    pub meridian_penalty_pct: f64,
    /// 已获变异列表。
    pub acquired_mutations: Vec<HudMutationEntry>,
    /// 下一阶段可选变异（若未到最大阶段）。
    pub next_stage_choices: Vec<String>,
}

/// HUD 中的变异条目。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HudMutationEntry {
    pub name: String,
    pub body_slot: String,
    pub level: u8,
    pub effect_desc: String,
}

impl DandaoHudPanelData {
    /// 从 MutationState + DandaoStyle 构建 HUD 面板数据。
    pub fn build(
        mutation_stage: u8,
        cumulative_toxin: f64,
        meridian_penalty: f64,
        slots: &[ActiveMutation],
    ) -> Self {
        use super::components::MUTATION_STAGE_THRESHOLDS;

        let toxin_to_next = if mutation_stage < 4 {
            MUTATION_STAGE_THRESHOLDS[mutation_stage as usize] - cumulative_toxin
        } else {
            0.0
        }
        .max(0.0);

        let next_stage = MutationStage::from(mutation_stage + 1);
        let next_choices: Vec<String> = MutationKind::choices_for_stage(next_stage)
            .iter()
            .map(|k| format!("{:?}", k))
            .collect();

        Self {
            mutation_stage,
            max_stage: 4,
            cumulative_toxin,
            toxin_to_next,
            meridian_penalty_pct: meridian_penalty * 100.0,
            acquired_mutations: slots
                .iter()
                .map(|s| HudMutationEntry {
                    name: format!("{:?}", s.kind),
                    body_slot: format!("{:?}", s.slot),
                    level: s.level,
                    effect_desc: format!("{:?}", s.kind.effect()),
                })
                .collect(),
            next_stage_choices: next_choices,
        }
    }
}

#[cfg(test)]
mod visual_sync_tests {
    use super::*;
    use crate::dandao::components::MutationStage;
    use crate::dandao::mutation::{ActiveMutation, BodySlot, MutationKind, MutationState};

    #[test]
    fn channel_id_correct() {
        assert_eq!(MUTATION_VISUAL_CHANNEL, "bong:mutation_visual");
    }

    #[test]
    fn payload_serde_roundtrip_empty() {
        let payload = MutationVisualSyncPayload {
            entity: "player_test".to_string(),
            stage: 0,
            slots: vec![],
            meridian_penalty: 0.0,
        };
        let json = serde_json::to_string(&payload).expect("serialize");
        let back: MutationVisualSyncPayload = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(payload, back);
    }

    #[test]
    fn payload_serde_roundtrip_with_slots() {
        let payload = MutationVisualSyncPayload {
            entity: "player_123".to_string(),
            stage: 3,
            slots: vec![
                MutationVisualSlot {
                    kind: "horns".to_string(),
                    body_slot: "head".to_string(),
                    level: 2,
                },
                MutationVisualSlot {
                    kind: "tail".to_string(),
                    body_slot: "lower".to_string(),
                    level: 1,
                },
            ],
            meridian_penalty: 0.15,
        };
        let json = serde_json::to_string(&payload).expect("serialize");
        let back: MutationVisualSyncPayload = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(payload, back);
    }

    #[test]
    fn from_state_converts_correctly() {
        let state = MutationState {
            stage: MutationStage::Heavy,
            slots: vec![ActiveMutation {
                kind: MutationKind::Horns,
                slot: BodySlot::Head,
                level: 2,
                acquired_tick: 100,
            }],
            meridian_penalty: 0.15,
        };
        let payload = MutationVisualSyncPayload::from_state("test_player", &state);
        assert_eq!(payload.entity, "test_player");
        assert_eq!(payload.stage, 3);
        assert_eq!(payload.slots.len(), 1);
        assert_eq!(payload.slots[0].level, 2);
        assert!((payload.meridian_penalty - 0.15).abs() < f64::EPSILON);
    }

    #[test]
    fn from_state_empty_slots() {
        let state = MutationState::default();
        let payload = MutationVisualSyncPayload::from_state("empty", &state);
        assert_eq!(payload.stage, 0);
        assert!(payload.slots.is_empty());
    }

    // --- HUD panel tests ---

    #[test]
    fn hud_panel_serde_roundtrip() {
        let panel = DandaoHudPanelData {
            mutation_stage: 2,
            max_stage: 4,
            cumulative_toxin: 150.0,
            toxin_to_next: 100.0,
            meridian_penalty_pct: 8.0,
            acquired_mutations: vec![HudMutationEntry {
                name: "GoldenIris".to_string(),
                body_slot: "Head".to_string(),
                level: 1,
                effect_desc: "vision boost".to_string(),
            }],
            next_stage_choices: vec![
                "Horns".to_string(),
                "Tail".to_string(),
                "BackCarapace".to_string(),
            ],
        };
        let json = serde_json::to_string(&panel).expect("serialize");
        let back: DandaoHudPanelData = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(panel, back);
    }

    #[test]
    fn hud_panel_build_stage_0() {
        let panel = DandaoHudPanelData::build(0, 10.0, 0.0, &[]);
        assert_eq!(panel.mutation_stage, 0);
        assert!(panel.toxin_to_next > 0.0, "阶段 0 应有下一阈值距离");
        assert!(panel.acquired_mutations.is_empty());
        assert_eq!(panel.next_stage_choices.len(), 3, "阶段 1 应有 3 个选择");
    }

    #[test]
    fn hud_panel_build_stage_4_no_next() {
        let panel = DandaoHudPanelData::build(4, 600.0, 0.20, &[]);
        assert_eq!(panel.mutation_stage, 4);
        assert!(
            (panel.toxin_to_next - 0.0).abs() < f64::EPSILON,
            "阶段 4 无下一阶段"
        );
        assert!((panel.meridian_penalty_pct - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn hud_panel_toxin_to_next_correct() {
        use crate::dandao::components::MUTATION_STAGE_THRESHOLDS;
        let toxin = 50.0; // 在阶段 1 (30-100)
        let panel = DandaoHudPanelData::build(1, toxin, 0.03, &[]);
        let expected = MUTATION_STAGE_THRESHOLDS[1] - toxin;
        assert!(
            (panel.toxin_to_next - expected).abs() < f64::EPSILON,
            "toxin_to_next 应为 {expected}, got {}",
            panel.toxin_to_next
        );
    }
}
