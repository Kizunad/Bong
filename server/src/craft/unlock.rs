//! plan-craft-v1 §3 — `RecipeUnlockState` resource + 三渠道解锁路径。
//!
//! §0 设计轴心 / worldview §九:843：
//!   * 玩家不会因为修了某流派就自动解锁所有相关配方
//!   * **必须**通过残卷 / 师承 / 顿悟三渠道之一获得
//!
//! 设计选择（plan §3 已声明）：
//!   * 不扩 `SkillSet.learned_recipes` 字段，避免污染 skill 模块
//!   * 新建独立 `RecipeUnlockState` resource，per-player 存 HashSet<RecipeId>
//!
//! ⚠️ 重生策略（worldview §十二 / death-lifecycle）：玩家 canonical id 维度
//! 的 unlock state 在角色死透重生后**不迁移**（"经验在玩家脑子里不在角色身上"
//! 是 SkillSet 的语义；本 resource 跟 SkillSet 共进退）。具体清空入口由
//! death-lifecycle plan 负责挂 hook，本 plan 只暴露 `clear_for_player` API。

use std::collections::{HashMap, HashSet};

use valence::prelude::Resource;

use super::events::{InsightTrigger, UnlockEventSource};
use super::recipe::{RecipeId, UnlockSource};

/// per-player canonical id 的 unlock 集合。
#[derive(Debug, Default)]
pub struct RecipeUnlockState {
    /// `canonical_player_id`（如 `"offline:Alice"`）→ 已解锁配方集合。
    /// 玩家未注册时返回空 set，等同未解锁任何配方。
    by_player: HashMap<String, HashSet<RecipeId>>,
}

impl Resource for RecipeUnlockState {}

impl RecipeUnlockState {
    pub fn new() -> Self {
        Self::default()
    }

    /// 查询玩家是否已解锁某配方。未注册玩家恒返回 false。
    pub fn is_unlocked(&self, player: &str, recipe: &RecipeId) -> bool {
        self.by_player
            .get(player)
            .map(|s| s.contains(recipe))
            .unwrap_or(false)
    }

    /// 直接解锁（不区分来源，三渠道辅助函数最终都走这里）。
    /// 返回 true = 实际新增，false = 已解锁的 noop。
    pub fn unlock(&mut self, player: impl Into<String>, recipe: RecipeId) -> bool {
        let player = player.into();
        self.by_player.entry(player).or_default().insert(recipe)
    }

    /// 玩家死透重生时调用 — 清空该玩家所有 unlock state。
    pub fn clear_for_player(&mut self, player: &str) {
        self.by_player.remove(player);
    }

    /// 该玩家已解锁配方数量（UI 统计用）。
    pub fn unlocked_count(&self, player: &str) -> usize {
        self.by_player.get(player).map(|s| s.len()).unwrap_or(0)
    }

    /// 迭代玩家已解锁配方（无序）。
    pub fn unlocked_recipes<'a>(
        &'a self,
        player: &str,
    ) -> Box<dyn Iterator<Item = &'a RecipeId> + 'a> {
        match self.by_player.get(player) {
            Some(set) => Box::new(set.iter()),
            None => Box::new(std::iter::empty()),
        }
    }
}

/// 三渠道解锁尝试结果 — 调用方根据返回值决定是否广播 RecipeUnlockedEvent。
///
/// 当前 API 形态下 `unlock_via_*` 直接接收 `&CraftRecipe`，调用方负责
/// registry 查找。所以本 enum 不含 `UnknownRecipe` variant —— 配方不存在时
/// 调用方在 lookup 阶段就 reject，不会走到本路径。
#[derive(Debug, Clone, PartialEq)]
pub enum UnlockOutcome {
    /// 实际新增解锁 — 调用方应广播 RecipeUnlockedEvent
    Newly { source: UnlockEventSource },
    /// 已解锁，noop（调用方不应重复广播 / 重复扣 cost）
    Already,
    /// 配方对此 source 不开放该路径（如该配方没注册 Mentor 但玩家走了 Mentor 流程）
    SourceMismatch,
}

/// §3 渠道一：残卷解锁。
///
/// 调用方：玩家 use ScrollItem 时由上层（inventory 物品使用 hook）调用。
/// 检查：
///   1. recipe 存在于 registry
///   2. recipe 的 unlock_sources 包含 Scroll variant 且 item_template 匹配
///   3. 玩家未解锁 → 写入 by_player
///
/// 注意：本函数**不**消耗 inventory 残卷（消耗逻辑在 hook 层），只做"是否能解锁"判定。
pub fn unlock_via_scroll(
    state: &mut RecipeUnlockState,
    player: &str,
    recipe: &super::recipe::CraftRecipe,
    scroll_item_template: &str,
) -> UnlockOutcome {
    let id_match = recipe.unlock_sources.iter().any(|src| match src {
        UnlockSource::Scroll { item_template } => item_template == scroll_item_template,
        _ => false,
    });
    if !id_match {
        return UnlockOutcome::SourceMismatch;
    }
    if state.is_unlocked(player, &recipe.id) {
        return UnlockOutcome::Already;
    }
    state.unlock(player.to_string(), recipe.id.clone());
    UnlockOutcome::Newly {
        source: UnlockEventSource::Scroll {
            item_template: scroll_item_template.to_string(),
        },
    }
}

/// §3 渠道二：师承解锁。
///
/// 调用方：NPC dialog 选项触发后，扣完 Renown / qi cost 后调用本函数。
/// 检查同 scroll，但比对 npc_archetype。
pub fn unlock_via_mentor(
    state: &mut RecipeUnlockState,
    player: &str,
    recipe: &super::recipe::CraftRecipe,
    npc_archetype: &str,
) -> UnlockOutcome {
    let archetype_match = recipe.unlock_sources.iter().any(|src| match src {
        UnlockSource::Mentor { npc_archetype: a } => a == npc_archetype,
        _ => false,
    });
    if !archetype_match {
        return UnlockOutcome::SourceMismatch;
    }
    if state.is_unlocked(player, &recipe.id) {
        return UnlockOutcome::Already;
    }
    state.unlock(player.to_string(), recipe.id.clone());
    UnlockOutcome::Newly {
        source: UnlockEventSource::Mentor {
            npc_archetype: npc_archetype.to_string(),
        },
    }
}

/// §3 渠道三：顿悟解锁。
///
/// 调用方：cultivation::BreakthroughEvent / combat::DeathEvent / combat::DefeatEvent
/// 等关键事件触发，agent 给玩家弹"顿悟选项菜单"，玩家选定后调用。
///
/// 注意：本函数不限定"一生一次" — 决策门 #4 = A（死亡清空），
/// 重生后玩家需要再次触发顿悟事件才能重新解锁。
pub fn unlock_via_insight(
    state: &mut RecipeUnlockState,
    player: &str,
    recipe: &super::recipe::CraftRecipe,
    trigger: InsightTrigger,
) -> UnlockOutcome {
    let trigger_match = recipe.unlock_sources.iter().any(|src| match src {
        UnlockSource::Insight { trigger: t } => *t == trigger,
        _ => false,
    });
    if !trigger_match {
        return UnlockOutcome::SourceMismatch;
    }
    if state.is_unlocked(player, &recipe.id) {
        return UnlockOutcome::Already;
    }
    state.unlock(player.to_string(), recipe.id.clone());
    UnlockOutcome::Newly {
        source: UnlockEventSource::Insight { trigger },
    }
}

/// plan-craft-v1 P3 — 配方查询辅助：哪些配方可以由"使用残卷 X"解锁。
///
/// 各 source plan（inventory ItemUse hook）调用本函数把"使用一卷 scroll_X"
/// 转成可解锁的 RecipeId 列表，再 emit `CraftUnlockIntent`。
pub fn find_recipes_unlockable_by_scroll<'a>(
    registry: &'a super::registry::CraftRegistry,
    scroll_item_template: &str,
) -> Vec<&'a super::recipe::CraftRecipe> {
    registry
        .iter()
        .filter(|r| {
            r.unlock_sources.iter().any(|src| match src {
                UnlockSource::Scroll { item_template } => item_template == scroll_item_template,
                _ => false,
            })
        })
        .collect()
}

/// plan-craft-v1 P3 — 配方查询：哪些配方可以由"师承 archetype Y"解锁。
pub fn find_recipes_unlockable_by_mentor<'a>(
    registry: &'a super::registry::CraftRegistry,
    npc_archetype: &str,
) -> Vec<&'a super::recipe::CraftRecipe> {
    registry
        .iter()
        .filter(|r| {
            r.unlock_sources.iter().any(|src| match src {
                UnlockSource::Mentor {
                    npc_archetype: arch,
                } => arch == npc_archetype,
                _ => false,
            })
        })
        .collect()
}

/// plan-craft-v1 P3 — 配方查询：哪些配方可以由"顿悟 trigger Z"解锁。
pub fn find_recipes_unlockable_by_insight(
    registry: &super::registry::CraftRegistry,
    trigger: InsightTrigger,
) -> Vec<&super::recipe::CraftRecipe> {
    registry
        .iter()
        .filter(|r| {
            r.unlock_sources.iter().any(|src| match src {
                UnlockSource::Insight { trigger: t } => *t == trigger,
                _ => false,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::recipe::{CraftCategory, CraftRecipe, CraftRequirements};
    use super::*;

    fn recipe_with_sources(sources: Vec<UnlockSource>) -> CraftRecipe {
        CraftRecipe {
            id: RecipeId::new("craft.example.test"),
            category: CraftCategory::Misc,
            display_name: "测试".into(),
            materials: vec![("herb_a".into(), 1)],
            qi_cost: 1.0,
            time_ticks: 60,
            output: ("test_out".into(), 1),
            requirements: CraftRequirements::default(),
            unlock_sources: sources,
        }
    }

    #[test]
    fn unlock_state_default_player_not_unlocked() {
        let state = RecipeUnlockState::new();
        assert!(!state.is_unlocked("offline:Alice", &RecipeId::new("anything")));
        assert_eq!(state.unlocked_count("offline:Alice"), 0);
    }

    #[test]
    fn unlock_marks_player_specific() {
        let mut state = RecipeUnlockState::new();
        let id = RecipeId::new("a");
        assert!(state.unlock("offline:Alice", id.clone()));
        assert!(state.is_unlocked("offline:Alice", &id));
        // Bob 不受影响
        assert!(!state.is_unlocked("offline:Bob", &id));
    }

    #[test]
    fn unlock_returns_false_when_already_present() {
        let mut state = RecipeUnlockState::new();
        let id = RecipeId::new("a");
        assert!(state.unlock("offline:Alice", id.clone()));
        assert!(!state.unlock("offline:Alice", id.clone()));
    }

    #[test]
    fn clear_for_player_drops_only_that_player() {
        let mut state = RecipeUnlockState::new();
        state.unlock("offline:Alice", RecipeId::new("a"));
        state.unlock("offline:Bob", RecipeId::new("b"));
        state.clear_for_player("offline:Alice");
        assert_eq!(state.unlocked_count("offline:Alice"), 0);
        assert!(state.is_unlocked("offline:Bob", &RecipeId::new("b")));
    }

    #[test]
    fn scroll_unlock_succeeds_when_template_matches() {
        let mut state = RecipeUnlockState::new();
        let recipe = recipe_with_sources(vec![UnlockSource::Scroll {
            item_template: "scroll_eclipse".into(),
        }]);
        let outcome = unlock_via_scroll(&mut state, "offline:Alice", &recipe, "scroll_eclipse");
        match outcome {
            UnlockOutcome::Newly {
                source: UnlockEventSource::Scroll { item_template },
            } => assert_eq!(item_template, "scroll_eclipse"),
            other => panic!("expected Newly Scroll, got {other:?}"),
        }
        assert!(state.is_unlocked("offline:Alice", &recipe.id));
    }

    #[test]
    fn scroll_unlock_already_when_repeated() {
        let mut state = RecipeUnlockState::new();
        let recipe = recipe_with_sources(vec![UnlockSource::Scroll {
            item_template: "scroll_a".into(),
        }]);
        unlock_via_scroll(&mut state, "offline:Alice", &recipe, "scroll_a");
        let again = unlock_via_scroll(&mut state, "offline:Alice", &recipe, "scroll_a");
        assert_eq!(again, UnlockOutcome::Already);
    }

    #[test]
    fn scroll_unlock_source_mismatch_when_template_differs() {
        let mut state = RecipeUnlockState::new();
        let recipe = recipe_with_sources(vec![UnlockSource::Scroll {
            item_template: "scroll_a".into(),
        }]);
        let outcome = unlock_via_scroll(&mut state, "offline:Alice", &recipe, "scroll_wrong");
        assert_eq!(outcome, UnlockOutcome::SourceMismatch);
        assert!(!state.is_unlocked("offline:Alice", &recipe.id));
    }

    #[test]
    fn scroll_unlock_source_mismatch_when_recipe_only_supports_mentor() {
        let mut state = RecipeUnlockState::new();
        let recipe = recipe_with_sources(vec![UnlockSource::Mentor {
            npc_archetype: "poison_master".into(),
        }]);
        let outcome = unlock_via_scroll(&mut state, "offline:Alice", &recipe, "scroll_a");
        assert_eq!(outcome, UnlockOutcome::SourceMismatch);
    }

    #[test]
    fn mentor_unlock_succeeds_when_archetype_matches() {
        let mut state = RecipeUnlockState::new();
        let recipe = recipe_with_sources(vec![UnlockSource::Mentor {
            npc_archetype: "poison_master".into(),
        }]);
        let outcome = unlock_via_mentor(&mut state, "offline:Alice", &recipe, "poison_master");
        assert!(matches!(outcome, UnlockOutcome::Newly { .. }));
        assert!(state.is_unlocked("offline:Alice", &recipe.id));
    }

    #[test]
    fn mentor_unlock_source_mismatch_for_unrelated_archetype() {
        let mut state = RecipeUnlockState::new();
        let recipe = recipe_with_sources(vec![UnlockSource::Mentor {
            npc_archetype: "poison_master".into(),
        }]);
        let outcome = unlock_via_mentor(&mut state, "offline:Alice", &recipe, "blacksmith");
        assert_eq!(outcome, UnlockOutcome::SourceMismatch);
    }

    #[test]
    fn insight_unlock_matches_specific_trigger() {
        let mut state = RecipeUnlockState::new();
        let recipe = recipe_with_sources(vec![UnlockSource::Insight {
            trigger: InsightTrigger::Breakthrough,
        }]);
        let ok = unlock_via_insight(
            &mut state,
            "offline:Alice",
            &recipe,
            InsightTrigger::Breakthrough,
        );
        assert!(matches!(ok, UnlockOutcome::Newly { .. }));

        // 不同 trigger 不应解锁另一个新玩家
        let other = unlock_via_insight(
            &mut state,
            "offline:Bob",
            &recipe,
            InsightTrigger::NearDeath,
        );
        assert_eq!(other, UnlockOutcome::SourceMismatch);
    }

    #[test]
    fn multi_source_recipe_can_unlock_via_either_path() {
        let mut state = RecipeUnlockState::new();
        let recipe = recipe_with_sources(vec![
            UnlockSource::Scroll {
                item_template: "scroll_a".into(),
            },
            UnlockSource::Mentor {
                npc_archetype: "poison_master".into(),
            },
        ]);
        // Alice 走 scroll
        let alice = unlock_via_scroll(&mut state, "offline:Alice", &recipe, "scroll_a");
        assert!(matches!(alice, UnlockOutcome::Newly { .. }));
        // Bob 走 mentor
        let bob = unlock_via_mentor(&mut state, "offline:Bob", &recipe, "poison_master");
        assert!(matches!(bob, UnlockOutcome::Newly { .. }));
    }

    // ── plan-craft-v1 P3 — find_recipes_unlockable_by_* ─────────────

    #[test]
    fn find_by_scroll_returns_only_matching_recipes() {
        let mut registry = super::super::registry::CraftRegistry::new();
        crate::craft::register_examples(&mut registry).unwrap();
        let matches = find_recipes_unlockable_by_scroll(&registry, "scroll_herb_knife_iron");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].id.as_str(), "craft.example.herb_knife.iron");
    }

    #[test]
    fn find_by_scroll_returns_empty_when_template_unknown() {
        let mut registry = super::super::registry::CraftRegistry::new();
        crate::craft::register_examples(&mut registry).unwrap();
        let matches = find_recipes_unlockable_by_scroll(&registry, "scroll_unknown");
        assert!(matches.is_empty());
    }

    #[test]
    fn find_by_mentor_returns_all_recipes_with_matching_archetype() {
        let mut registry = super::super::registry::CraftRegistry::new();
        crate::craft::register_examples(&mut registry).unwrap();
        let matches = find_recipes_unlockable_by_mentor(&registry, "array_scribe");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].id.as_str(), "craft.example.zhenfa_trap.iron");
    }

    #[test]
    fn find_by_insight_matches_only_specific_trigger() {
        let mut registry = super::super::registry::CraftRegistry::new();
        crate::craft::register_examples(&mut registry).unwrap();
        // 伪灵皮 light 注册了 Insight::NearDeath
        let near_death = find_recipes_unlockable_by_insight(&registry, InsightTrigger::NearDeath);
        assert_eq!(near_death.len(), 1);
        assert_eq!(near_death[0].id.as_str(), "craft.example.fake_skin.light");
        // breakthrough 没人注册 → 空
        let bt = find_recipes_unlockable_by_insight(&registry, InsightTrigger::Breakthrough);
        assert!(bt.is_empty());
    }
}
