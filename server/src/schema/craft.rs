//! plan-craft-v1 §3 数据契约 — IPC schema 5 sample（server 侧 v1）。
//!
//! 与 TypeScript `agent/packages/schema/src/craft.ts` 1:1 镜像（P3 阶段双端
//! 对齐）。本 plan P0+P1 只声明 Rust 类型 + serde roundtrip 测试，sample
//! json 文件由 P3 双端对拍时落地。
//!
//! Schema 命名约定（plan §3）：
//!   * `CraftStartReqV1` — client → server，玩家点 [开始手搓] 按钮发起
//!   * `CraftSessionStateV1` — server → client，进度 / 剩余时间推送
//!   * `CraftOutcomeV1` — server → agent，结算后产出广播（narration 输入）
//!   * `RecipeUnlockedV1` — server → agent，三渠道触发解锁（narration 输入）
//!   * `RecipeListV1` — server → client，inventory 打开时拉一次配方表

use serde::{Deserialize, Serialize};

use crate::cultivation::components::{ColorKind, Realm};

/// `craft.recipe.id` 线上格式 — 字符串透明化（与 `crate::craft::RecipeId` 等价）。
pub type RecipeIdV1 = String;

/// 配方分组（与 `crate::craft::CraftCategory` 1:1）。
/// snake_case 上线，与 client / agent 都对齐。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CraftCategoryV1 {
    AnqiCarrier,
    DuguPotion,
    TuikeSkin,
    ZhenfaTrap,
    Tool,
    ArmorCraft,
    Container,
    PoisonPowder,
    Misc,
}

impl From<crate::craft::CraftCategory> for CraftCategoryV1 {
    fn from(c: crate::craft::CraftCategory) -> Self {
        match c {
            crate::craft::CraftCategory::AnqiCarrier => Self::AnqiCarrier,
            crate::craft::CraftCategory::DuguPotion => Self::DuguPotion,
            crate::craft::CraftCategory::TuikeSkin => Self::TuikeSkin,
            crate::craft::CraftCategory::ZhenfaTrap => Self::ZhenfaTrap,
            crate::craft::CraftCategory::Tool => Self::Tool,
            crate::craft::CraftCategory::ArmorCraft => Self::ArmorCraft,
            crate::craft::CraftCategory::Container => Self::Container,
            crate::craft::CraftCategory::PoisonPowder => Self::PoisonPowder,
            crate::craft::CraftCategory::Misc => Self::Misc,
        }
    }
}

/// 取消 / 失败原因（与 `events::CraftFailureReason` 1:1）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CraftFailureReasonV1 {
    PlayerCancelled,
    PlayerDied,
    InternalError,
}

impl From<crate::craft::CraftFailureReason> for CraftFailureReasonV1 {
    fn from(r: crate::craft::CraftFailureReason) -> Self {
        match r {
            crate::craft::CraftFailureReason::PlayerCancelled => Self::PlayerCancelled,
            crate::craft::CraftFailureReason::PlayerDied => Self::PlayerDied,
            crate::craft::CraftFailureReason::InternalError => Self::InternalError,
        }
    }
}

/// 顿悟触发源（与 `events::InsightTrigger` 1:1）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InsightTriggerV1 {
    Breakthrough,
    NearDeath,
    DefeatStronger,
}

impl From<crate::craft::InsightTrigger> for InsightTriggerV1 {
    fn from(t: crate::craft::InsightTrigger) -> Self {
        match t {
            crate::craft::InsightTrigger::Breakthrough => Self::Breakthrough,
            crate::craft::InsightTrigger::NearDeath => Self::NearDeath,
            crate::craft::InsightTrigger::DefeatStronger => Self::DefeatStronger,
        }
    }
}

/// 解锁来源 union — discriminated by `kind`。与 client `RecipeUnlockToastPlanner` /
/// agent `craft_runtime` 共享。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UnlockEventSourceV1 {
    Scroll { item_template: String },
    Mentor { npc_archetype: String },
    Insight { trigger: InsightTriggerV1 },
}

impl From<crate::craft::UnlockEventSource> for UnlockEventSourceV1 {
    fn from(s: crate::craft::UnlockEventSource) -> Self {
        match s {
            crate::craft::UnlockEventSource::Scroll { item_template } => {
                Self::Scroll { item_template }
            }
            crate::craft::UnlockEventSource::Mentor { npc_archetype } => {
                Self::Mentor { npc_archetype }
            }
            crate::craft::UnlockEventSource::Insight { trigger } => Self::Insight {
                trigger: trigger.into(),
            },
        }
    }
}

/// `requirements` 序列化形式 — Optional 三字段都映射 None = 字段省略。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CraftRequirementsV1 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub realm_min: Option<Realm>,
    /// (color_kind, min_share)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qi_color_min: Option<(ColorKind, f32)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill_lv_min: Option<u8>,
}

/// `RecipeListV1` 单条 entry — UI 左列表渲染的最小集合。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CraftRecipeEntryV1 {
    pub id: RecipeIdV1,
    pub category: CraftCategoryV1,
    pub display_name: String,
    /// `(template_id, count)` 列表
    pub materials: Vec<(String, u32)>,
    pub qi_cost: f64,
    pub time_ticks: u64,
    pub output: (String, u32),
    pub requirements: CraftRequirementsV1,
    pub unlocked: bool,
}

// ─── 5 sample types ────────────────────────────────────────────────────────

/// client → server：玩家点 [开始手搓]。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CraftStartReqV1 {
    pub v: u8,
    pub player_id: String,
    pub recipe_id: RecipeIdV1,
    pub quantity: u32,
    pub ts: u64,
}

/// server → client：当前任务进度（每 N tick 推一次）。
/// `active=false` 表示玩家无 session（用于关闭进度条）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CraftSessionStateV1 {
    pub v: u8,
    pub player_id: String,
    pub active: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipe_id: Option<RecipeIdV1>,
    pub elapsed_ticks: u64,
    pub total_ticks: u64,
    pub completed_count: u32,
    pub total_count: u32,
    pub ts: u64,
}

/// 出炉结果 union — 成功 / 失败两 variant。`kind` 区分。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CraftOutcomeV1 {
    Completed {
        v: u8,
        player_id: String,
        recipe_id: RecipeIdV1,
        output_template: String,
        output_count: u32,
        completed_at_tick: u64,
        ts: u64,
    },
    Failed {
        v: u8,
        player_id: String,
        recipe_id: RecipeIdV1,
        reason: CraftFailureReasonV1,
        material_returned: u32,
        qi_refunded: f64,
        ts: u64,
    },
}

/// 三渠道解锁广播 — agent narration 4 类之一（首学 / 师承 / 顿悟）的 trigger。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeUnlockedV1 {
    pub v: u8,
    pub player_id: String,
    pub recipe_id: RecipeIdV1,
    pub source: UnlockEventSourceV1,
    pub unlocked_at_tick: u64,
    pub ts: u64,
}

/// inventory 打开时 server 推一次配方全表（按 grouped_for_ui 排序）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeListV1 {
    pub v: u8,
    pub player_id: String,
    pub recipes: Vec<CraftRecipeEntryV1>,
    pub ts: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_serde_snake_case() {
        let s = serde_json::to_string(&CraftCategoryV1::AnqiCarrier).unwrap();
        assert_eq!(s, "\"anqi_carrier\"");
        let back: CraftCategoryV1 = serde_json::from_str("\"dugu_potion\"").unwrap();
        assert_eq!(back, CraftCategoryV1::DuguPotion);
    }

    #[test]
    fn category_from_module_enum_covers_all_variants() {
        for cat in crate::craft::CraftCategory::ALL {
            // From + Into 必须对所有 variant 都不 panic
            let _v1: CraftCategoryV1 = cat.into();
        }
    }

    #[test]
    fn failure_reason_serde_roundtrip() {
        for r in [
            CraftFailureReasonV1::PlayerCancelled,
            CraftFailureReasonV1::PlayerDied,
            CraftFailureReasonV1::InternalError,
        ] {
            let s = serde_json::to_string(&r).unwrap();
            let back: CraftFailureReasonV1 = serde_json::from_str(&s).unwrap();
            assert_eq!(back, r);
        }
    }

    #[test]
    fn insight_trigger_serde_roundtrip() {
        for t in [
            InsightTriggerV1::Breakthrough,
            InsightTriggerV1::NearDeath,
            InsightTriggerV1::DefeatStronger,
        ] {
            let s = serde_json::to_string(&t).unwrap();
            let back: InsightTriggerV1 = serde_json::from_str(&s).unwrap();
            assert_eq!(back, t);
        }
    }

    #[test]
    fn unlock_source_scroll_roundtrip() {
        let s = UnlockEventSourceV1::Scroll {
            item_template: "scroll_eclipse".into(),
        };
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"kind\":\"scroll\""));
        let back: UnlockEventSourceV1 = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn unlock_source_mentor_roundtrip() {
        let s = UnlockEventSourceV1::Mentor {
            npc_archetype: "poison_master".into(),
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: UnlockEventSourceV1 = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn unlock_source_insight_roundtrip() {
        let s = UnlockEventSourceV1::Insight {
            trigger: InsightTriggerV1::Breakthrough,
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: UnlockEventSourceV1 = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn requirements_optional_omitted_when_none() {
        let r = CraftRequirementsV1::default();
        let json = serde_json::to_string(&r).unwrap();
        // 全 None 时不应包含字段名
        assert!(!json.contains("realm_min"));
        assert!(!json.contains("qi_color_min"));
        assert!(!json.contains("skill_lv_min"));
    }

    #[test]
    fn requirements_with_qi_color_roundtrip() {
        let r = CraftRequirementsV1 {
            realm_min: Some(Realm::Awaken),
            qi_color_min: Some((ColorKind::Insidious, 0.05)),
            skill_lv_min: Some(2),
        };
        let json = serde_json::to_string(&r).unwrap();
        let back: CraftRequirementsV1 = serde_json::from_str(&json).unwrap();
        assert_eq!(back, r);
    }

    #[test]
    fn craft_start_req_roundtrip() {
        let v = CraftStartReqV1 {
            v: 1,
            player_id: "offline:Alice".into(),
            recipe_id: "craft.example.eclipse_needle.iron".into(),
            quantity: 3,
            ts: 1234567,
        };
        let s = serde_json::to_string(&v).unwrap();
        let back: CraftStartReqV1 = serde_json::from_str(&s).unwrap();
        assert_eq!(back, v);
    }

    #[test]
    fn craft_session_state_active_roundtrip() {
        let v = CraftSessionStateV1 {
            v: 1,
            player_id: "offline:Alice".into(),
            active: true,
            recipe_id: Some("craft.example.eclipse_needle.iron".into()),
            elapsed_ticks: 30,
            total_ticks: 100,
            completed_count: 1,
            total_count: 3,
            ts: 1234567,
        };
        let s = serde_json::to_string(&v).unwrap();
        let back: CraftSessionStateV1 = serde_json::from_str(&s).unwrap();
        assert_eq!(back, v);
    }

    #[test]
    fn craft_session_state_inactive_omits_recipe_id() {
        let v = CraftSessionStateV1 {
            v: 1,
            player_id: "offline:Alice".into(),
            active: false,
            recipe_id: None,
            elapsed_ticks: 0,
            total_ticks: 0,
            completed_count: 0,
            total_count: 0,
            ts: 1234567,
        };
        let s = serde_json::to_string(&v).unwrap();
        assert!(!s.contains("recipe_id"));
        let back: CraftSessionStateV1 = serde_json::from_str(&s).unwrap();
        assert_eq!(back, v);
    }

    #[test]
    fn craft_outcome_completed_roundtrip() {
        let v = CraftOutcomeV1::Completed {
            v: 1,
            player_id: "offline:Alice".into(),
            recipe_id: "craft.example.eclipse_needle.iron".into(),
            output_template: "eclipse_needle_iron".into(),
            output_count: 3,
            completed_at_tick: 5000,
            ts: 1234567,
        };
        let s = serde_json::to_string(&v).unwrap();
        assert!(s.contains("\"kind\":\"completed\""));
        let back: CraftOutcomeV1 = serde_json::from_str(&s).unwrap();
        assert_eq!(back, v);
    }

    #[test]
    fn craft_outcome_failed_roundtrip() {
        let v = CraftOutcomeV1::Failed {
            v: 1,
            player_id: "offline:Alice".into(),
            recipe_id: "craft.example.eclipse_needle.iron".into(),
            reason: CraftFailureReasonV1::PlayerCancelled,
            material_returned: 3,
            qi_refunded: 0.0,
            ts: 1234567,
        };
        let s = serde_json::to_string(&v).unwrap();
        assert!(s.contains("\"kind\":\"failed\""));
        let back: CraftOutcomeV1 = serde_json::from_str(&s).unwrap();
        assert_eq!(back, v);
    }

    #[test]
    fn recipe_unlocked_roundtrip() {
        let v = RecipeUnlockedV1 {
            v: 1,
            player_id: "offline:Alice".into(),
            recipe_id: "craft.example.fake_skin.light".into(),
            source: UnlockEventSourceV1::Insight {
                trigger: InsightTriggerV1::NearDeath,
            },
            unlocked_at_tick: 10000,
            ts: 1234567,
        };
        let s = serde_json::to_string(&v).unwrap();
        let back: RecipeUnlockedV1 = serde_json::from_str(&s).unwrap();
        assert_eq!(back, v);
    }

    #[test]
    fn recipe_list_roundtrip_with_one_entry() {
        let v = RecipeListV1 {
            v: 1,
            player_id: "offline:Alice".into(),
            recipes: vec![CraftRecipeEntryV1 {
                id: "craft.example.herb_knife.iron".into(),
                category: CraftCategoryV1::Tool,
                display_name: "采药刀（凡铁）".into(),
                materials: vec![("iron_ingot".into(), 1), ("wood_handle".into(), 1)],
                qi_cost: 0.0,
                time_ticks: 600,
                output: ("herb_knife_iron".into(), 1),
                requirements: CraftRequirementsV1::default(),
                unlocked: false,
            }],
            ts: 1234567,
        };
        let s = serde_json::to_string(&v).unwrap();
        let back: RecipeListV1 = serde_json::from_str(&s).unwrap();
        assert_eq!(back, v);
    }

    #[test]
    fn schema_v1_versioning_field_present_on_all_payloads() {
        // 守卫：5 sample 的 v 字段都不省略，确保版本可识别（避免 v=0/缺省混淆）
        let start = CraftStartReqV1 {
            v: 1,
            player_id: "x".into(),
            recipe_id: "y".into(),
            quantity: 1,
            ts: 0,
        };
        assert!(serde_json::to_string(&start).unwrap().contains("\"v\":1"));

        let state = CraftSessionStateV1 {
            v: 1,
            player_id: "x".into(),
            active: false,
            recipe_id: None,
            elapsed_ticks: 0,
            total_ticks: 0,
            completed_count: 0,
            total_count: 0,
            ts: 0,
        };
        assert!(serde_json::to_string(&state).unwrap().contains("\"v\":1"));

        let unlocked = RecipeUnlockedV1 {
            v: 1,
            player_id: "x".into(),
            recipe_id: "y".into(),
            source: UnlockEventSourceV1::Scroll {
                item_template: "z".into(),
            },
            unlocked_at_tick: 0,
            ts: 0,
        };
        assert!(serde_json::to_string(&unlocked)
            .unwrap()
            .contains("\"v\":1"));

        let list = RecipeListV1 {
            v: 1,
            player_id: "x".into(),
            recipes: vec![],
            ts: 0,
        };
        assert!(serde_json::to_string(&list).unwrap().contains("\"v\":1"));
    }
}
