//! plan-craft-v1 — 通用手搓系统（P0 决策门收口 + P1 server 主体）。
//!
//! 模块结构：
//!   * [`recipe`] — `CraftRecipe` / `CraftCategory` / `CraftRequirements` /
//!     `RecipeId` / `UnlockSource`（数据契约）
//!   * [`registry`] — `CraftRegistry` resource（全局注册表，6 类分组）
//!   * [`session`] — `CraftSession` component + `start/cancel/finalize/tick`
//!     （守恒律走 `qi_physics::ledger::Crafting`）
//!   * [`unlock`] — `RecipeUnlockState` resource + 三渠道（残卷/师承/顿悟）
//!   * [`events`] — `CraftStartedEvent` / `CraftCompletedEvent` /
//!     `CraftFailedEvent` / `RecipeUnlockedEvent` / `InsightTrigger`
//!
//! 跨 plan 钩子（P3 接入，本 plan P0+P1 不动）：
//!   * 流派 plan（dugu-v2 / tuike-v2 / zhenfa-v2 / tools-v1）→ register 自家配方
//!   * agent narration runtime（`craft_runtime.ts`）→ 4 类叙事
//!   * client `CraftTabScreen` UI → P2
//!   * `unlock_via_scroll` ItemUse hook → inventory 物品使用层
//!   * `unlock_via_mentor` NPC dialog 选项 → social plan dialog 引擎
//!   * `unlock_via_insight` BreakthroughEvent / DefeatStrongerEvent 监听 → cultivation/combat plan
//!
//! P0 决策门收口（详见 `docs/finished_plans/plan-craft-v1.md` §5）：
//!   * #1 = A：保留 6 类（AnqiCarrier / DuguPotion / TuikeSkin / ZhenfaTrap / Tool / Misc）
//!   * #2 = A：UI 排序按类别分组 + 类别内字母（`registry::grouped_for_ui`）
//!   * #3 = B：取消任务返还材料 70%（`session::CANCEL_REFUND_RATIO`），qi 不退
//!   * #4 = A：玩家死亡 → cancel + PlayerDied reason
//!   * #5 = A：手搓 tab 不收 vanilla，凡器破例（5 个示例之一）
//!   * #6 = B：requirements 软 gate（UI 灰显 + 服务端硬校验防作弊）

pub mod events;
pub mod recipe;
pub mod registry;
pub mod session;
pub mod unlock;

use valence::prelude::App;

#[allow(unused_imports)]
pub use events::{
    CraftCancelIntent, CraftCompletedEvent, CraftFailedEvent, CraftFailureReason, CraftStartIntent,
    CraftStartedEvent, CraftUnlockIntent, InsightTrigger, RecipeUnlockedEvent, UnlockEventSource,
};
#[allow(unused_imports)]
pub use recipe::{
    CraftCategory, CraftRecipe, CraftRequirements, RecipeId, RecipeValidationError, UnlockSource,
};
#[allow(unused_imports)]
pub use registry::{CraftRegistry, RegistryError};
#[allow(unused_imports)]
pub use session::{
    cancel_craft, consume_materials_from_inventory, count_template_in_inventory, finalize_craft,
    start_craft, tick_session, CancelCraftOutcome, CraftSession, FinalizeCraftOutcome,
    MaterialDeficit, StartCraftDeps, StartCraftError, StartCraftRequest, StartCraftSuccess,
    CANCEL_REFUND_RATIO,
};
#[allow(unused_imports)]
pub use unlock::{
    unlock_via_insight, unlock_via_mentor, unlock_via_scroll, RecipeUnlockState, UnlockOutcome,
};

use crate::cultivation::components::{ColorKind, Realm};

/// 注册 craft 子系统到主 App。
///
/// 当前 P0+P1：
///   * 注册 5 个示例配方到 `CraftRegistry`（流派 plan 接入前的 P1 验收基线）
///   * 注册 4 类事件
///   * 注册 `CraftRegistry` / `RecipeUnlockState` resources
///
/// P2/P3 阶段补：UI tab + agent narration + 三渠道 hook（接 inventory ItemUse / social
/// dialog / cultivation BreakthroughEvent）。
pub fn register(app: &mut App) {
    tracing::info!("[bong][craft] registering craft subsystem (plan-craft-v1 P0+P1)");

    let mut registry = CraftRegistry::new();
    register_examples(&mut registry).unwrap_or_else(|err| {
        panic!("[bong][craft] failed to register example recipes: {err}");
    });
    tracing::info!(
        "[bong][craft] registered {} example recipe(s) (P1 baseline)",
        registry.len()
    );

    app.insert_resource(registry);
    app.insert_resource(RecipeUnlockState::new());

    app.add_event::<CraftStartedEvent>();
    app.add_event::<CraftCompletedEvent>();
    app.add_event::<CraftFailedEvent>();
    app.add_event::<RecipeUnlockedEvent>();
    // P2 client → server intents（被 `network/craft_emit::apply_craft_intents` 系统消费）
    app.add_event::<CraftStartIntent>();
    app.add_event::<CraftCancelIntent>();
    // P3 三渠道解锁 intent —— 由各 source plan emit，被
    // `network/craft_emit::apply_unlock_intents` 系统消费
    app.add_event::<CraftUnlockIntent>();
}

/// P1 验收基线：注册 5 个示例配方覆盖全 6 类（除 Misc 外）。
///
/// 命名约定：`craft.example.<物品>.<档位>` —— `craft.example.*` 命名空间
/// 标识"plan-craft-v1 自带的示例"，流派 plan vN+1 接入时用各自命名空间
/// （`dugu.*` / `tuike.*` / `zhenfa.*` / `tools.*`）。
///
/// 5 个示例分布（plan §2 UI Mockup / plan §1 P1 验收清单）：
///   1. AnqiCarrier — 蚀针（凡铁）
///   2. DuguPotion  — 毒源煎汤（凡毒）
///   3. TuikeSkin   — 伪灵皮（轻档）
///   4. ZhenfaTrap  — 真元诡雷（凡铁）
///   5. Tool        — 采药刀（凡铁）— §5 决策门 #5 凡器破例收录
pub fn register_examples(registry: &mut CraftRegistry) -> Result<(), RegistryError> {
    // 1. 蚀针（凡铁）— AnqiCarrier
    registry.register(CraftRecipe {
        id: RecipeId::new("craft.example.eclipse_needle.iron"),
        category: CraftCategory::AnqiCarrier,
        display_name: "蚀针（凡铁档）".into(),
        materials: vec![
            ("iron_needle".into(), 3),
            ("chi_xui_cao".into(), 1), // 赤髓草（plan-botany / 现有 herbalism 词条）
        ],
        qi_cost: 8.0,
        time_ticks: 3 * 60 * 20, // 3 min in-game
        output: ("eclipse_needle_iron".into(), 3),
        requirements: CraftRequirements {
            realm_min: None, // 不强制 — worldview §五:537 流派由组合涌现
            qi_color_min: Some((ColorKind::Insidious, 0.05)),
            skill_lv_min: None,
        },
        unlock_sources: vec![
            UnlockSource::Scroll {
                item_template: "scroll_eclipse_needle_iron".into(),
            },
            UnlockSource::Mentor {
                npc_archetype: "poison_master".into(),
            },
        ],
    })?;

    // 2. 毒源煎汤（凡毒）— DuguPotion
    registry.register(CraftRecipe {
        id: RecipeId::new("craft.example.poison_decoction.fan"),
        category: CraftCategory::DuguPotion,
        display_name: "毒源煎汤（凡毒）".into(),
        materials: vec![
            ("shao_hou_man".into(), 2), // 烧候蔓
            ("clay_pot".into(), 1),
        ],
        qi_cost: 4.0,
        time_ticks: 90 * 20, // 1.5 min in-game
        output: ("poison_decoction_fan".into(), 1),
        requirements: CraftRequirements::default(),
        unlock_sources: vec![
            UnlockSource::Scroll {
                item_template: "scroll_poison_decoction_fan".into(),
            },
            UnlockSource::Mentor {
                npc_archetype: "poison_master".into(),
            },
        ],
    })?;

    // 3. 伪灵皮（轻档）— TuikeSkin
    registry.register(CraftRecipe {
        id: RecipeId::new("craft.example.fake_skin.light"),
        category: CraftCategory::TuikeSkin,
        display_name: "伪灵皮（轻档）".into(),
        materials: vec![
            ("rabbit_pelt".into(), 4),
            ("yu_yi_zhi".into(), 1), // 鱼衣脂
        ],
        qi_cost: 2.0,
        time_ticks: 2 * 60 * 20, // 2 min in-game
        output: ("fake_skin_light".into(), 1),
        requirements: CraftRequirements {
            realm_min: Some(Realm::Induce), // 引气起步 — 替尸需要灵气过渡
            qi_color_min: None,
            skill_lv_min: None,
        },
        unlock_sources: vec![
            UnlockSource::Scroll {
                item_template: "scroll_fake_skin_light".into(),
            },
            UnlockSource::Insight {
                trigger: InsightTrigger::NearDeath,
            },
        ],
    })?;

    // 4. 真元诡雷（凡铁）— ZhenfaTrap
    registry.register(CraftRecipe {
        id: RecipeId::new("craft.example.zhenfa_trap.iron"),
        category: CraftCategory::ZhenfaTrap,
        display_name: "真元诡雷（凡铁芯）".into(),
        materials: vec![
            ("iron_ingot".into(), 2),
            ("zhenfa_blank_array".into(), 1), // 阵法白纸
        ],
        qi_cost: 6.0,
        time_ticks: 4 * 60 * 20, // 4 min in-game
        output: ("zhenfa_trap_iron".into(), 1),
        requirements: CraftRequirements {
            realm_min: Some(Realm::Induce),
            qi_color_min: None,
            skill_lv_min: None,
        },
        unlock_sources: vec![
            UnlockSource::Scroll {
                item_template: "scroll_zhenfa_trap_iron".into(),
            },
            UnlockSource::Mentor {
                npc_archetype: "array_scribe".into(),
            },
        ],
    })?;

    // 5. 采药刀（凡铁）— Tool（§5 决策门 #5 凡器破例收录手搓 tab）
    registry.register(CraftRecipe {
        id: RecipeId::new("craft.example.herb_knife.iron"),
        category: CraftCategory::Tool,
        display_name: "采药刀（凡铁）".into(),
        materials: vec![("iron_ingot".into(), 1), ("wood_handle".into(), 1)],
        qi_cost: 0.0,        // 凡器不投入真元
        time_ticks: 30 * 20, // 30 sec in-game
        output: ("herb_knife_iron".into(), 1),
        requirements: CraftRequirements::default(),
        unlock_sources: vec![UnlockSource::Scroll {
            item_template: "scroll_herb_knife_iron".into(),
        }],
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_examples_succeeds() {
        let mut registry = CraftRegistry::new();
        register_examples(&mut registry).unwrap();
        assert_eq!(registry.len(), 5);
    }

    #[test]
    fn register_examples_covers_all_categories_except_misc() {
        let mut registry = CraftRegistry::new();
        register_examples(&mut registry).unwrap();
        let categories: std::collections::HashSet<CraftCategory> =
            registry.iter().map(|r| r.category).collect();
        for cat in [
            CraftCategory::AnqiCarrier,
            CraftCategory::DuguPotion,
            CraftCategory::TuikeSkin,
            CraftCategory::ZhenfaTrap,
            CraftCategory::Tool,
        ] {
            assert!(
                categories.contains(&cat),
                "missing example for category {:?}",
                cat
            );
        }
        // 5 个示例不该覆盖 Misc — Misc 是兜底
        assert!(!categories.contains(&CraftCategory::Misc));
    }

    #[test]
    fn register_examples_each_has_unlock_sources() {
        let mut registry = CraftRegistry::new();
        register_examples(&mut registry).unwrap();
        for recipe in registry.iter() {
            assert!(
                !recipe.unlock_sources.is_empty(),
                "example `{}` must have at least one unlock_source",
                recipe.id
            );
        }
    }

    #[test]
    fn register_examples_qi_cost_uses_ledger_safe_finite_values() {
        // 守恒律前置 — 所有示例 qi_cost finite >= 0
        let mut registry = CraftRegistry::new();
        register_examples(&mut registry).unwrap();
        for recipe in registry.iter() {
            assert!(recipe.qi_cost.is_finite());
            assert!(recipe.qi_cost >= 0.0);
        }
    }

    #[test]
    fn register_examples_includes_tool_with_zero_qi_cost() {
        // §5 决策门 #5 凡器破例 — Tool 无真元投入
        let mut registry = CraftRegistry::new();
        register_examples(&mut registry).unwrap();
        let tool = registry
            .by_category(CraftCategory::Tool)
            .next()
            .expect("must have at least one Tool example");
        assert_eq!(tool.qi_cost, 0.0);
    }

    #[test]
    fn register_examples_eclipse_needle_uses_insidious_qi_color_gate() {
        // 蚀针匹配 worldview §六：阴诡色需求
        let mut registry = CraftRegistry::new();
        register_examples(&mut registry).unwrap();
        let recipe = registry
            .get(&RecipeId::new("craft.example.eclipse_needle.iron"))
            .expect("eclipse_needle example must register");
        let (kind, share) = recipe
            .requirements
            .qi_color_min
            .expect("eclipse_needle must have qi_color gate");
        assert_eq!(kind, ColorKind::Insidious);
        assert!(share > 0.0);
    }

    #[test]
    fn register_examples_each_id_starts_with_craft_example_namespace() {
        let mut registry = CraftRegistry::new();
        register_examples(&mut registry).unwrap();
        for recipe in registry.iter() {
            assert!(
                recipe.id.as_str().starts_with("craft.example."),
                "example recipe id `{}` should be in `craft.example.*` namespace",
                recipe.id
            );
        }
    }

    #[test]
    fn register_examples_rejects_double_register() {
        let mut registry = CraftRegistry::new();
        register_examples(&mut registry).unwrap();
        // 第二次 register 必须 reject（duplicate id）
        let err = register_examples(&mut registry).unwrap_err();
        assert!(matches!(err, RegistryError::DuplicateId(_)));
    }
}
