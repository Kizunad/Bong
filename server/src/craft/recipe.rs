//! plan-craft-v1 §3 数据契约 — Craft 配方层。
//!
//! `CraftRecipe` 是手搓配方的 source-of-truth。每个流派 plan（dugu-v2 /
//! tuike-v2 / zhenfa-v2 / tools-v1）在自己 P0 阶段调 `CraftRegistry::register`
//! 注入。本 plan 内 `mod_default` 注册 5 个示例配方，作为 P1 验收基线。
//!
//! 与 `alchemy::Recipe` 的区别：
//!   * **无火候 / 阶段投料** — 单步投料即起手搓
//!   * **无残缺匹配** — 材料必须严格满足，缺料 reject 而不是降级出炉
//!   * **qi_cost 走 ledger** — `start_craft` 以 ECS `Cultivation` 为玩家真元权威，
//!     先用 `transfer_external_qi_to_ledger` 写入 `Crafting` 审计与 pending 余额，
//!     成功后再等额扣减 `qi_current`；禁止绕过 ledger 直接扣减
//!
//! §5 决策门 #1 = A（保留 6 类）。plan-anqi-v2 追加 Container 类目，
//! 用于箭袋 / 裤袋 / 封灵匣这类非载体但同属流派装备的配方。

use serde::{Deserialize, Serialize};

use crate::cultivation::components::{ColorKind, Realm};

use super::events::InsightTrigger;

/// plan-workbench-recipes-v1 §P2.1 — 制作站类型。
///
/// `None` = 手搓（随时随地），`Some(Workbench)` = 需 3 格内制作台。
/// 未来可扩展 AlchemyBench / ForgeBench 等。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CraftStationKind {
    Workbench,
}

impl CraftStationKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Workbench => "workbench",
        }
    }
}

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
/// 灵眼勘探），plan vN+1 再扩；`poison_trait` 作为 active plan 明确补 `PoisonPowder`。
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
    /// 凡物盔甲（骨甲 / 兽皮甲 / 铁甲 / 铜甲 / 灵布衫 / 残卷缠甲）。
    ArmorCraft,
    /// 容器 / 装具（箭袋、裤袋、封灵匣等）。
    Container,
    /// 毒丹研磨粉末（poison-trait-v1 双层附毒的消耗品路径）。
    PoisonPowder,
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
            Self::ArmorCraft => "armor_craft",
            Self::Container => "container",
            Self::PoisonPowder => "poison_powder",
            Self::Misc => "misc",
        }
    }

    /// UI 左列表分组顺序固定（§5 决策门 #2 = A，按类别分组 + 字母）。
    /// 客户端不应自行打乱该顺序，否则解锁状态视觉跟服务端不一致。
    pub const ALL: [Self; 9] = [
        Self::AnqiCarrier,
        Self::DuguPotion,
        Self::TuikeSkin,
        Self::ZhenfaTrap,
        Self::Tool,
        Self::ArmorCraft,
        Self::Container,
        Self::PoisonPowder,
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
    /// 玩家任一已习得流派技能的有效等级下限（含）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill_lv_min: Option<u8>,
}

/// §3 显式解锁来源。每条配方关联 `Vec<UnlockSource>`，玩家命中任一即解锁。
///
/// **空 `unlock_sources` 语义（plan-craft-material-discovery）**：不再是"默认全
/// 解锁"，而是**材料发现路径** —— 玩家持有该配方任一原料即被动解锁
/// （`unlock::unlock_via_material` + `craft_emit::apply_material_discovery_unlock`）。
/// 因此无显式来源 ≠ 永远学不会：基础凡器/加工链留空源，靠采集到原料解锁；
/// 残卷/师承/顿悟门控的秘传配方才显式列来源（材料发现对它们不生效）。
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
    /// 自身真元投入（一次性，不维持）。**走 external → ledger Crafting reason**。
    pub qi_cost: f64,
    /// in-game tick 推进时间。1 秒 = 20 tick；3 min = 3600 tick。
    /// 玩家 inventory 关闭时不推进（§0 设计轴心，下线暂停）。
    pub time_ticks: u64,
    /// 产出 `(template_id, count)`。count >= 1。
    pub output: (String, u32),
    pub requirements: CraftRequirements,
    /// 显式解锁来源（残卷 / 师承 / 顿悟，命中任一即解锁）。
    /// **空 Vec = 材料发现路径**（持有任一原料即被动解锁，见 `UnlockSource` 文档
    /// 与 `unlock::unlock_via_material`）；非空 = 秘传配方，只能走列出的显式渠道。
    pub unlock_sources: Vec<UnlockSource>,
    /// plan-workbench-recipes-v1 §P2.1 — 制作站约束。
    /// `None` = 手搓（不需要制作台），`Some(Workbench)` = 需 3 格内制作台。
    pub station: Option<CraftStationKind>,
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
        let mut material_templates = std::collections::HashSet::new();
        for (index, (template, count)) in self.materials.iter().enumerate() {
            if !material_templates.insert(template) {
                return Err(RecipeValidationError::DuplicateMaterialTemplate {
                    id: self.id.clone(),
                    template: template.clone(),
                });
            }
            if template.is_empty() {
                return Err(RecipeValidationError::EmptyMaterialTemplate {
                    id: self.id.clone(),
                });
            }
            if *count == 0 {
                return Err(RecipeValidationError::ZeroCount {
                    id: self.id.clone(),
                    index,
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
        // 空 unlock_sources = 材料发现路径（凡器基础加工链等，靠持有原料解锁，
        // 见 unlock::unlock_via_material）；validate 允许为空，不视为错误。
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
        if let Some(skill_lv_min) = self.requirements.skill_lv_min {
            if skill_lv_min > crate::skill::curve::SKILL_MAX_LEVEL {
                return Err(RecipeValidationError::SkillLevelTooHigh {
                    id: self.id.clone(),
                    skill_lv_min,
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
        index: usize,
        template: String,
    },
    DuplicateMaterialTemplate {
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
    /// 历史遗留 variant：早期语义下"空 unlock_sources = 永远学不会"。
    /// plan-craft-material-discovery 起空源 = 材料发现路径（合法），`validate`
    /// 不再构造本错误；保留 variant 仅为 API 稳定，永不返回。
    #[allow(dead_code)]
    NoUnlockSources {
        id: RecipeId,
    },
    SkillLevelTooHigh {
        id: RecipeId,
        skill_lv_min: u8,
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
            Self::ZeroCount {
                id, template, ..
            } => {
                write!(f, "recipe `{id}` material `{template}` count is 0")
            }
            Self::DuplicateMaterialTemplate { id, template } => write!(
                f,
                "recipe `{id}` declares duplicate material template `{template}`"
            ),
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
                "recipe `{id}` has no unlock_sources (legacy; empty now means material-discovery path)"
            ),
            Self::SkillLevelTooHigh { skill_lv_min, .. } => write!(
                f,
                "recipe skill_lv_min {skill_lv_min} exceeds runtime maximum {}",
                crate::skill::curve::SKILL_MAX_LEVEL
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
