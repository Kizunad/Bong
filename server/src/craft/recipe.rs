//! plan-craft-v1 §3 数据契约 — Craft 配方层。
//!
//! `CraftRecipe` 是手搓配方的 source-of-truth。每个流派 plan（dugu-v2 /
//! tuike-v2 / zhenfa-v2 / tools-v1）在自己 P0 阶段调 `CraftRegistry::register`
//! 注入。本 plan 内 `mod_default` 注册 5 个示例配方，作为 P1 验收基线。
//!
//! 与 `alchemy::Recipe` 的区别：
//!   * **无火候 / 阶段投料** — 单步投料即起手搓
//!   * **无残缺匹配** — 材料必须严格满足，缺料 reject 而不是降级出炉
//!   * **qi_cost 走 ledger** — `start_craft` 内 `WorldQiAccount::transfer`
//!     与 `Crafting` reason，禁止 plan 内 `cultivation.qi_current -= cost`
//!
//! §5 决策门 #1 = A（保留 6 类）。plan-anqi-v2 追加 Container 类目，
//! 用于箭袋 / 裤袋 / 封灵匣这类非载体但同属流派装备的配方。

use serde::{Deserialize, Serialize};

use crate::cultivation::components::{ColorKind, Realm};

use super::events::InsightTrigger;

/// 配方唯一 ID。命名约定：`<流派>.<物品>.<档位>`，如 `dugu.eclipse_needle.iron`。
/// 各流派 plan 内统一 prefix，避免与本 plan 内的 `craft.example.*` 示例 ID 冲突。
#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RecipeId(pub String);

impl RecipeId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for RecipeId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for RecipeId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl std::fmt::Display for RecipeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// §5 决策门 #1 = A。流派/物品大类，UI 左列表分组依据。
///
/// 后续若新流派/系统要加类别（如 BaomaiSpecial 体修自损增益、SpiritEyeEquipment
/// 灵眼勘探），plan vN+1 再扩，**禁止本 plan P1 内私下加 variant**。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CraftCategory {
    /// 暗器载体（蚀针 / 骨刺等，dugu / anqi 流派接入）
    AnqiCarrier,
    /// 煎汤 / 自蕴（毒源煎汤 / 自蕴丹胚等，dugu 自蕴档案）
    DuguPotion,
    /// 伪皮 / 替尸（伪灵皮，tuike 流派）
    TuikeSkin,
    /// 阵法预埋件（真元诡雷 / 阵旗），zhenfa 流派
    ZhenfaTrap,
    /// 凡器（采药刀 / 刮刀 / 镰刀），tools 流派
    Tool,
    /// 容器 / 装具（箭袋、裤袋、封灵匣等）。
    Container,
    /// 兜底类别。新流派 plan 应明确选 5 类之一，避免堆 Misc
    Misc,
}

impl CraftCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AnqiCarrier => "anqi_carrier",
            Self::DuguPotion => "dugu_potion",
            Self::TuikeSkin => "tuike_skin",
            Self::ZhenfaTrap => "zhenfa_trap",
            Self::Tool => "tool",
            Self::Container => "container",
            Self::Misc => "misc",
        }
    }

    /// UI 左列表分组顺序固定（§5 决策门 #2 = A，按类别分组 + 字母）。
    /// 客户端不应自行打乱该顺序，否则解锁状态视觉跟服务端不一致。
    pub const ALL: [Self; 7] = [
        Self::AnqiCarrier,
        Self::DuguPotion,
        Self::TuikeSkin,
        Self::ZhenfaTrap,
        Self::Tool,
        Self::Container,
        Self::Misc,
    ];
}

/// §3 配方门槛。所有字段 None = 不强制 gate；§5 决策门 #6 = B（软 gate）—
/// 不满足时 UI [开始手搓] 灰显并提示原因，**不**从列表里隐藏。`start_craft`
/// 内强制校验，前端展示是辅助，但服务端必须独立判定（防作弊）。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CraftRequirements {
    /// 境界下限（含）。例：醒灵起步 → `Some(Realm::Awaken)`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub realm_min: Option<Realm>,
    /// 真元色门槛（kind, min_share）。`min_share ∈ [0,1]` 表示该色权重最低占比。
    /// 当前用 main color 命中即视为满足（secondary 不参与）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qi_color_min: Option<(ColorKind, f32)>,
    /// 流派技能等级下限（含）。后续 plan-skill-v2 接入后由 SkillSet::lv 校验。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill_lv_min: Option<u8>,
}

/// §3 解锁来源。每条配方关联 `Vec<UnlockSource>`，玩家命中任一即解锁。
///
/// **强 invariant**：每条配方 `unlock_sources.is_empty() == false` —
/// 没有解锁来源等于"永远学不会"，应通过 §5 决策门 #1 改类别或者
/// 直接默认 unlocked（用空 Vec + 显式默认 unlocked flag 表示，本 plan 暂未实装）。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum UnlockSource {
    /// 残卷掉落（worldview §十）。玩家 use 该 item 即触发 unlock_via_scroll。
    Scroll { item_template: String },
    /// 师承 NPC dialog（worldview §十一）。npc_archetype 是 NPC 的 archetype id
    /// （如 "poison_master"），而非具体 entity id —— 多个同 archetype NPC 都可教。
    Mentor { npc_archetype: String },
    /// 顿悟事件（worldview §六:658）。trigger 命中后由 agent / server 弹选项菜单，
    /// 玩家选定后写 RecipeUnlockedEvent。
    Insight { trigger: InsightTrigger },
}

/// §3 完整配方。clone-cheap（材料 / 解锁源 vec 按需拷贝），可放进 `CraftRegistry`
/// 的 HashMap 内 owned。
#[derive(Debug, Clone, PartialEq)]
pub struct CraftRecipe {
    pub id: RecipeId,
    pub category: CraftCategory,
    /// 显示名（中文 / UI 用）。本 plan 内简短，流派 plan 注册时用正典命名。
    pub display_name: String,
    /// 材料清单：`(template_id, count)`。`template_id` 与 `inventory::ItemInstance.template_id`
    /// 对齐；count >= 1。重复的 template 不建议（应聚合到一条 entry）。
    pub materials: Vec<(String, u32)>,
    /// 自身真元投入（一次性，不维持）。**走 ledger Crafting reason**。
    pub qi_cost: f64,
    /// in-game tick 推进时间。1 秒 = 20 tick；3 min = 3600 tick。
    /// 玩家 inventory 关闭时不推进（§0 设计轴心，下线暂停）。
    pub time_ticks: u64,
    /// 产出 `(template_id, count)`。count >= 1。
    pub output: (String, u32),
    pub requirements: CraftRequirements,
    /// 解锁来源（残卷 / 师承 / 顿悟 任一）。注册时 invariant：非空。
    pub unlock_sources: Vec<UnlockSource>,
}

impl CraftRecipe {
    /// `register` 时调用；返回 Err 则注册失败（CraftRegistry::register 转发）。
    pub fn validate(&self) -> Result<(), RecipeValidationError> {
        if self.id.as_str().is_empty() {
            return Err(RecipeValidationError::EmptyId);
        }
        if self.materials.is_empty() {
            return Err(RecipeValidationError::NoMaterials {
                id: self.id.clone(),
            });
        }
        for (template, count) in &self.materials {
            if template.is_empty() {
                return Err(RecipeValidationError::EmptyMaterialTemplate {
                    id: self.id.clone(),
                });
            }
            if *count == 0 {
                return Err(RecipeValidationError::ZeroCount {
                    id: self.id.clone(),
                    template: template.clone(),
                });
            }
        }
        if self.output.0.is_empty() {
            return Err(RecipeValidationError::EmptyOutputTemplate {
                id: self.id.clone(),
            });
        }
        if self.output.1 == 0 {
            return Err(RecipeValidationError::ZeroOutputCount {
                id: self.id.clone(),
            });
        }
        if !self.qi_cost.is_finite() || self.qi_cost < 0.0 {
            return Err(RecipeValidationError::InvalidQiCost {
                id: self.id.clone(),
                qi_cost: self.qi_cost,
            });
        }
        if self.time_ticks == 0 {
            return Err(RecipeValidationError::ZeroTimeTicks {
                id: self.id.clone(),
            });
        }
        if self.unlock_sources.is_empty() {
            return Err(RecipeValidationError::NoUnlockSources {
                id: self.id.clone(),
            });
        }
        // qi_color_min share 范围 [0.0, 1.0]，finite
        if let Some((kind, share)) = self.requirements.qi_color_min {
            if !share.is_finite() || !(0.0..=1.0).contains(&share) {
                return Err(RecipeValidationError::InvalidQiColorMinShare {
                    id: self.id.clone(),
                    color: kind,
                    share,
                });
            }
        }
        // unlock_sources 内每个 string payload 非空（避免"永远无法匹配"的源）
        for src in &self.unlock_sources {
            match src {
                UnlockSource::Scroll { item_template } if item_template.is_empty() => {
                    return Err(RecipeValidationError::EmptyUnlockSourceTemplate {
                        id: self.id.clone(),
                        kind: "scroll",
                    });
                }
                UnlockSource::Mentor { npc_archetype } if npc_archetype.is_empty() => {
                    return Err(RecipeValidationError::EmptyUnlockSourceTemplate {
                        id: self.id.clone(),
                        kind: "mentor",
                    });
                }
                _ => {}
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RecipeValidationError {
    EmptyId,
    NoMaterials {
        id: RecipeId,
    },
    EmptyMaterialTemplate {
        id: RecipeId,
    },
    ZeroCount {
        id: RecipeId,
        template: String,
    },
    EmptyOutputTemplate {
        id: RecipeId,
    },
    ZeroOutputCount {
        id: RecipeId,
    },
    InvalidQiCost {
        id: RecipeId,
        qi_cost: f64,
    },
    ZeroTimeTicks {
        id: RecipeId,
    },
    NoUnlockSources {
        id: RecipeId,
    },
    InvalidQiColorMinShare {
        id: RecipeId,
        color: ColorKind,
        share: f32,
    },
    EmptyUnlockSourceTemplate {
        id: RecipeId,
        kind: &'static str,
    },
}

impl std::fmt::Display for RecipeValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyId => write!(f, "recipe id is empty"),
            Self::NoMaterials { id } => write!(f, "recipe `{id}` has no materials"),
            Self::EmptyMaterialTemplate { id } => {
                write!(f, "recipe `{id}` has empty material template_id")
            }
            Self::ZeroCount { id, template } => {
                write!(f, "recipe `{id}` material `{template}` count is 0")
            }
            Self::EmptyOutputTemplate { id } => {
                write!(f, "recipe `{id}` output template_id is empty")
            }
            Self::ZeroOutputCount { id } => write!(f, "recipe `{id}` output count is 0"),
            Self::InvalidQiCost { id, qi_cost } => {
                write!(
                    f,
                    "recipe `{id}` qi_cost {qi_cost} is not finite or negative"
                )
            }
            Self::ZeroTimeTicks { id } => write!(f, "recipe `{id}` time_ticks is 0"),
            Self::NoUnlockSources { id } => write!(
                f,
                "recipe `{id}` has no unlock_sources (would be permanently unlearnable)"
            ),
            Self::InvalidQiColorMinShare { id, color, share } => write!(
                f,
                "recipe `{id}` qi_color_min share for {color:?} is {share} (must be finite and in [0.0, 1.0])"
            ),
            Self::EmptyUnlockSourceTemplate { id, kind } => write!(
                f,
                "recipe `{id}` has empty {kind} unlock_source payload"
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok_recipe() -> CraftRecipe {
        CraftRecipe {
            id: RecipeId::new("craft.example.test"),
            category: CraftCategory::Misc,
            display_name: "测试配方".into(),
            materials: vec![("herb_a".into(), 2)],
            qi_cost: 5.0,
            time_ticks: 60,
            output: (("test_pill".into()), 1),
            requirements: CraftRequirements::default(),
            unlock_sources: vec![UnlockSource::Scroll {
                item_template: "scroll_test".into(),
            }],
        }
    }

    #[test]
    fn recipe_id_roundtrip_serde() {
        let id = RecipeId::new("dugu.eclipse_needle.iron");
        let s = serde_json::to_string(&id).unwrap();
        assert_eq!(s, "\"dugu.eclipse_needle.iron\"");
        let back: RecipeId = serde_json::from_str(&s).unwrap();
        assert_eq!(back, id);
    }

    #[test]
    fn category_str_stable_and_all_unique() {
        let strs: Vec<&str> = CraftCategory::ALL.iter().map(|c| c.as_str()).collect();
        // 类目必须各不相同 — UI 分组依赖 str id 做 key
        let mut sorted = strs.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            CraftCategory::ALL.len(),
            "expected unique category str ids"
        );
        // 顺序固定（不能因为 enum order 改变破坏 UI 分组顺序）
        assert_eq!(
            strs,
            [
                "anqi_carrier",
                "dugu_potion",
                "tuike_skin",
                "zhenfa_trap",
                "tool",
                "container",
                "misc"
            ]
        );
    }

    #[test]
    fn requirements_default_is_no_gate() {
        let req = CraftRequirements::default();
        assert!(req.realm_min.is_none());
        assert!(req.qi_color_min.is_none());
        assert!(req.skill_lv_min.is_none());
    }

    #[test]
    fn validate_accepts_well_formed_recipe() {
        assert!(ok_recipe().validate().is_ok());
    }

    #[test]
    fn validate_rejects_empty_id() {
        let mut r = ok_recipe();
        r.id = RecipeId::new("");
        assert_eq!(r.validate(), Err(RecipeValidationError::EmptyId));
    }

    #[test]
    fn validate_rejects_no_materials() {
        let mut r = ok_recipe();
        r.materials.clear();
        assert!(matches!(
            r.validate(),
            Err(RecipeValidationError::NoMaterials { .. })
        ));
    }

    #[test]
    fn validate_rejects_empty_material_template() {
        let mut r = ok_recipe();
        r.materials = vec![("".into(), 1)];
        assert!(matches!(
            r.validate(),
            Err(RecipeValidationError::EmptyMaterialTemplate { .. })
        ));
    }

    #[test]
    fn validate_rejects_zero_count_material() {
        let mut r = ok_recipe();
        r.materials = vec![("herb_a".into(), 0)];
        assert!(matches!(
            r.validate(),
            Err(RecipeValidationError::ZeroCount { .. })
        ));
    }

    #[test]
    fn validate_rejects_empty_output_template() {
        let mut r = ok_recipe();
        r.output = ("".into(), 1);
        assert!(matches!(
            r.validate(),
            Err(RecipeValidationError::EmptyOutputTemplate { .. })
        ));
    }

    #[test]
    fn validate_rejects_zero_output_count() {
        let mut r = ok_recipe();
        r.output = ("test_pill".into(), 0);
        assert!(matches!(
            r.validate(),
            Err(RecipeValidationError::ZeroOutputCount { .. })
        ));
    }

    #[test]
    fn validate_rejects_negative_qi_cost() {
        let mut r = ok_recipe();
        r.qi_cost = -1.0;
        assert!(matches!(
            r.validate(),
            Err(RecipeValidationError::InvalidQiCost { .. })
        ));
    }

    #[test]
    fn validate_rejects_nan_qi_cost() {
        let mut r = ok_recipe();
        r.qi_cost = f64::NAN;
        assert!(matches!(
            r.validate(),
            Err(RecipeValidationError::InvalidQiCost { .. })
        ));
    }

    #[test]
    fn validate_rejects_zero_time_ticks() {
        let mut r = ok_recipe();
        r.time_ticks = 0;
        assert!(matches!(
            r.validate(),
            Err(RecipeValidationError::ZeroTimeTicks { .. })
        ));
    }

    #[test]
    fn validate_rejects_no_unlock_sources() {
        let mut r = ok_recipe();
        r.unlock_sources.clear();
        assert!(matches!(
            r.validate(),
            Err(RecipeValidationError::NoUnlockSources { .. })
        ));
    }

    #[test]
    fn validate_accepts_zero_qi_cost() {
        // qi_cost = 0 是合法的（凡器手搓，无真元投入）
        let mut r = ok_recipe();
        r.qi_cost = 0.0;
        assert!(r.validate().is_ok());
    }

    #[test]
    fn validate_rejects_qi_color_min_share_above_one() {
        let mut r = ok_recipe();
        r.requirements.qi_color_min = Some((ColorKind::Insidious, 1.5));
        assert!(matches!(
            r.validate(),
            Err(RecipeValidationError::InvalidQiColorMinShare { share, .. }) if (share - 1.5).abs() < 1e-6
        ));
    }

    #[test]
    fn validate_rejects_qi_color_min_share_negative() {
        let mut r = ok_recipe();
        r.requirements.qi_color_min = Some((ColorKind::Insidious, -0.1));
        assert!(matches!(
            r.validate(),
            Err(RecipeValidationError::InvalidQiColorMinShare { share, .. }) if share < 0.0
        ));
    }

    #[test]
    fn validate_rejects_qi_color_min_share_nan() {
        let mut r = ok_recipe();
        r.requirements.qi_color_min = Some((ColorKind::Insidious, f32::NAN));
        assert!(matches!(
            r.validate(),
            Err(RecipeValidationError::InvalidQiColorMinShare { share, .. }) if share.is_nan()
        ));
    }

    #[test]
    fn validate_accepts_qi_color_min_share_at_bounds() {
        let mut r = ok_recipe();
        // 0.0 边界
        r.requirements.qi_color_min = Some((ColorKind::Insidious, 0.0));
        assert!(r.validate().is_ok());
        // 1.0 边界
        r.requirements.qi_color_min = Some((ColorKind::Insidious, 1.0));
        assert!(r.validate().is_ok());
    }

    #[test]
    fn validate_rejects_empty_scroll_unlock_payload() {
        let mut r = ok_recipe();
        r.unlock_sources = vec![UnlockSource::Scroll {
            item_template: String::new(),
        }];
        assert!(matches!(
            r.validate(),
            Err(RecipeValidationError::EmptyUnlockSourceTemplate { kind: "scroll", .. })
        ));
    }

    #[test]
    fn validate_rejects_empty_mentor_unlock_payload() {
        let mut r = ok_recipe();
        r.unlock_sources = vec![UnlockSource::Mentor {
            npc_archetype: String::new(),
        }];
        assert!(matches!(
            r.validate(),
            Err(RecipeValidationError::EmptyUnlockSourceTemplate { kind: "mentor", .. })
        ));
    }
}
