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
    /// 累计丹毒总量（client MutationHudPlanner 丹毒条读取此字段）。
    pub cumulative_toxin: f64,
}

impl MutationVisualSyncPayload {
    /// 从 MutationState + cumulative_toxin 构建 payload。
    ///
    /// # 大小写契约（server ↔ client）
    /// - `kind`：保留 Rust Debug 的 CamelCase（如 "GoldenIris"）。
    ///   client `MutationKind.fromServerName` 以 `([a-z])([A-Z])` 正则拆分后 `valueOf`。
    /// - `body_slot`：保留 Rust Debug 的 PascalCase（如 "Head"）。
    ///   client `translateBodySlot` 先 `.toUpperCase()` 再 switch，"Head" → "HEAD" → "头部" ✓。
    pub fn from_state(entity_id: &str, state: &MutationState, cumulative_toxin: f64) -> Self {
        Self {
            entity: entity_id.to_string(),
            stage: state.stage as u8,
            slots: state
                .slots
                .iter()
                .map(|s| MutationVisualSlot {
                    // CamelCase 直接用 Debug fmt，不做任何大小写变换
                    kind: format!("{:?}", s.kind),
                    // PascalCase (e.g. "Head"), client 会 toUpperCase → "HEAD"
                    body_slot: format!("{:?}", s.slot),
                    level: s.level,
                })
                .collect(),
            meridian_penalty: state.meridian_penalty,
            cumulative_toxin,
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
            cumulative_toxin: 0.0,
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
                    kind: "Horns".to_string(),
                    body_slot: "Head".to_string(),
                    level: 2,
                },
                MutationVisualSlot {
                    kind: "Tail".to_string(),
                    body_slot: "Lower".to_string(),
                    level: 1,
                },
            ],
            meridian_penalty: 0.15,
            cumulative_toxin: 300.0,
        };
        let json = serde_json::to_string(&payload).expect("serialize");
        let back: MutationVisualSyncPayload = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(payload, back);
    }

    /// 验证 JSON 中存在 cumulative_toxin 字段（client fallback 0.0 的前提是该 key 实际出现）。
    #[test]
    fn payload_json_contains_cumulative_toxin_key() {
        let payload = MutationVisualSyncPayload {
            entity: "p".to_string(),
            stage: 1,
            slots: vec![],
            meridian_penalty: 0.03,
            cumulative_toxin: 42.5,
        };
        let json = serde_json::to_string(&payload).expect("serialize");
        let val: serde_json::Value = serde_json::from_str(&json).expect("parse");
        assert!(
            val.get("cumulative_toxin").is_some(),
            "JSON 必须包含 cumulative_toxin key，client MutationHudPlanner 丹毒条依赖它"
        );
        assert_eq!(
            val["cumulative_toxin"],
            serde_json::json!(42.5),
            "cumulative_toxin 序列化值应与原始值一致"
        );
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
        let payload = MutationVisualSyncPayload::from_state("test_player", &state, 300.0);
        assert_eq!(payload.entity, "test_player");
        assert_eq!(payload.stage, 3);
        assert_eq!(payload.slots.len(), 1);
        assert_eq!(payload.slots[0].level, 2);
        assert!((payload.meridian_penalty - 0.15).abs() < f64::EPSILON);
        assert!(
            (payload.cumulative_toxin - 300.0).abs() < f64::EPSILON,
            "cumulative_toxin 应从 DandaoStyle 传入并透传"
        );
    }

    #[test]
    fn from_state_empty_slots() {
        let state = MutationState::default();
        let payload = MutationVisualSyncPayload::from_state("empty", &state, 0.0);
        assert_eq!(payload.stage, 0);
        assert!(payload.slots.is_empty());
        assert_eq!(payload.cumulative_toxin, 0.0);
    }

    // --- 大小写契约（server ↔ client round-trip pin 测试）---

    /// MutationKind Debug 输出 CamelCase，client fromServerName 需要此形态。
    #[test]
    fn from_state_kind_camel_case_golden_iris() {
        let state = MutationState {
            stage: MutationStage::Subtle,
            slots: vec![ActiveMutation {
                kind: MutationKind::GoldenIris,
                slot: BodySlot::Head,
                level: 1,
                acquired_tick: 0,
            }],
            meridian_penalty: 0.03,
        };
        let payload = MutationVisualSyncPayload::from_state("p", &state, 30.0);
        assert_eq!(
            payload.slots[0].kind, "GoldenIris",
            "kind 应为 CamelCase 'GoldenIris'，client fromServerName 依赖此格式"
        );
    }

    #[test]
    fn from_state_kind_camel_case_all_variants() {
        use crate::dandao::mutation::BodySlot;

        let cases: &[(MutationKind, BodySlot, &str)] = &[
            (MutationKind::GoldenIris, BodySlot::Head, "GoldenIris"),
            (
                MutationKind::HardenedNails,
                BodySlot::Forearm,
                "HardenedNails",
            ),
            (MutationKind::ToughSkin, BodySlot::Torso, "ToughSkin"),
            (MutationKind::BoneRidge, BodySlot::Head, "BoneRidge"),
            (
                MutationKind::ForearmScales,
                BodySlot::Forearm,
                "ForearmScales",
            ),
            (MutationKind::SpineSpurs, BodySlot::Back, "SpineSpurs"),
            (MutationKind::Horns, BodySlot::Head, "Horns"),
            (MutationKind::Tail, BodySlot::Lower, "Tail"),
            (MutationKind::BackCarapace, BodySlot::Back, "BackCarapace"),
            (MutationKind::ExtraArms, BodySlot::Forearm, "ExtraArms"),
            (MutationKind::BodyEnlarge, BodySlot::Torso, "BodyEnlarge"),
            (MutationKind::BeastFace, BodySlot::Head, "BeastFace"),
        ];

        for (kind, slot, expected_kind_str) in cases {
            let state = MutationState {
                stage: MutationStage::Subtle,
                slots: vec![ActiveMutation {
                    kind: *kind,
                    slot: *slot,
                    level: 1,
                    acquired_tick: 0,
                }],
                meridian_penalty: 0.03,
            };
            let payload = MutationVisualSyncPayload::from_state("p", &state, 0.0);
            assert_eq!(
                payload.slots[0].kind, *expected_kind_str,
                "MutationKind::{kind:?} 应序列化为 '{expected_kind_str}'"
            );
        }
    }

    #[test]
    fn from_state_body_slot_pascal_case_all_variants() {
        use crate::dandao::mutation::BodySlot;

        let cases: &[(BodySlot, &str)] = &[
            (BodySlot::Head, "Head"),
            (BodySlot::Forearm, "Forearm"),
            (BodySlot::Back, "Back"),
            (BodySlot::Torso, "Torso"),
            (BodySlot::Lower, "Lower"),
        ];

        for (slot, expected) in cases {
            let state = MutationState {
                stage: MutationStage::Subtle,
                slots: vec![ActiveMutation {
                    kind: MutationKind::GoldenIris,
                    slot: *slot,
                    level: 1,
                    acquired_tick: 0,
                }],
                meridian_penalty: 0.0,
            };
            let payload = MutationVisualSyncPayload::from_state("p", &state, 0.0);
            assert_eq!(
                payload.slots[0].body_slot, *expected,
                "BodySlot::{slot:?} 应序列化为 '{expected}'，client translateBodySlot 会 toUpperCase"
            );
        }
    }

    /// 验证 JSON 中 kind/body_slot 的字面与 client 期望字面完全吻合（payload 层面契约锁定）。
    #[test]
    fn json_literal_round_trip_contract_golden_iris_head() {
        let state = MutationState {
            stage: MutationStage::Subtle,
            slots: vec![ActiveMutation {
                kind: MutationKind::GoldenIris,
                slot: BodySlot::Head,
                level: 1,
                acquired_tick: 0,
            }],
            meridian_penalty: 0.03,
        };
        let payload = MutationVisualSyncPayload::from_state("p", &state, 35.0);
        let json = serde_json::to_string(&payload).expect("serialize");
        let val: serde_json::Value = serde_json::from_str(&json).expect("parse");
        let slot0 = &val["slots"][0];
        assert_eq!(
            slot0["kind"], "GoldenIris",
            "JSON kind 字面必须为 'GoldenIris'，client MutationKind.fromServerName 依赖此值"
        );
        assert_eq!(
            slot0["body_slot"], "Head",
            "JSON body_slot 字面必须为 'Head'，client translateBodySlot(.toUpperCase()) 依赖此值"
        );
        assert!(
            val["cumulative_toxin"].as_f64().unwrap() > 0.0,
            "cumulative_toxin 应在 JSON 中且非零"
        );
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
