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
//!
//! 持久化（照抄 `mineral::persistence::ExhaustedMineralsLog` 的 hydrate/dirty/
//! 节流 flush 模式）：启动期从 `data/craft/recipe_unlocks.json` hydrate，
//! unlock/clear 变更标 dirty，`tick_recipe_unlock_flush` 系统按节流窗口
//! （默认 600 tick = 30 秒 @ 20 tps）刷盘。落盘格式：
//! ```json
//! { "version": 1, "by_player": { "offline:Alice": ["craft.example.a"] } }
//! ```

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use valence::prelude::{ResMut, Resource};

use super::events::{InsightTrigger, UnlockEventSource};
use super::recipe::{RecipeId, UnlockSource};

const DEFAULT_UNLOCK_PATH: &str = "data/craft/recipe_unlocks.json";

/// 落盘格式 wrapper — 留 `version` 字段方便后续 schema 演进。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct RecipeUnlockFile {
    pub version: u32,
    pub by_player: HashMap<String, HashSet<RecipeId>>,
}

/// per-player canonical id 的 unlock 集合 + 节流刷盘 — 在 `register` 时插入到
/// ECS resource。
#[derive(Debug)]
pub struct RecipeUnlockState {
    /// `canonical_player_id`（如 `"offline:Alice"`）→ 已解锁配方集合。
    /// 玩家未注册时返回空 set，等同未解锁任何配方。
    by_player: HashMap<String, HashSet<RecipeId>>,
    /// 自上次 flush 以来是否有未落盘的变更。
    dirty: bool,
    /// 距上次 flush 累计 tick 数，用于节流。
    flush_clock: u32,
    /// 节流窗口（tick）。默认 600 = 30 秒 @ 20 tps。
    flush_interval_ticks: u32,
    /// 落盘路径；test override 用 `with_path`。
    file_path: PathBuf,
}

impl Resource for RecipeUnlockState {}

impl Default for RecipeUnlockState {
    fn default() -> Self {
        Self {
            by_player: HashMap::new(),
            dirty: false,
            flush_clock: 0,
            flush_interval_ticks: 600,
            file_path: PathBuf::from(DEFAULT_UNLOCK_PATH),
        }
    }
}

impl RecipeUnlockState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.file_path = path.into();
        self
    }

    pub fn with_flush_interval(mut self, ticks: u32) -> Self {
        self.flush_interval_ticks = ticks;
        self
    }

    /// 是否有未落盘的变更（测试 / flush 系统用）。
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// 已注册玩家数（hydrate 启动日志用）。
    pub fn player_count(&self) -> usize {
        self.by_player.len()
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
        let inserted = self.by_player.entry(player).or_default().insert(recipe);
        if inserted {
            self.dirty = true;
        }
        inserted
    }

    /// 玩家死透重生时调用 — 清空该玩家所有 unlock state。
    pub fn clear_for_player(&mut self, player: &str) {
        if self.by_player.remove(player).is_some() {
            self.dirty = true;
        }
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

    /// 强制刷盘 — 测试 / 关服 hook 用。
    pub fn flush(&mut self) -> Result<(), String> {
        if !self.dirty {
            return Ok(());
        }
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("create dir {} failed: {e}", parent.display()))?;
        }
        let file = RecipeUnlockFile {
            version: 1,
            by_player: self.by_player.clone(),
        };
        let json = serde_json::to_string_pretty(&file)
            .map_err(|e| format!("serialize recipe unlock log failed: {e}"))?;
        fs::write(&self.file_path, json)
            .map_err(|e| format!("write {} failed: {e}", self.file_path.display()))?;
        self.dirty = false;
        self.flush_clock = 0;
        Ok(())
    }

    /// 启动期 hydrator — 从 `path` 读回磁盘 log，还原 in-memory state。
    ///
    /// - 文件不存在：等价 `default()`，静默（首次启动常态）。
    /// - 文件存在但解析失败：warn + 启动一份空 state（避免 corrupt 文件阻塞启动）。
    /// - 成功：`by_player` 预填；`dirty=false` 防止启动立即重写文件。
    pub fn hydrated_from_path(path: impl Into<PathBuf>) -> Self {
        let path: PathBuf = path.into();
        let mut state = Self::default().with_path(path.clone());
        if !path.exists() {
            return state;
        }
        match load_recipe_unlock_log(&path) {
            Ok(file) => {
                state.by_player = file.by_player;
                state.dirty = false;
            }
            Err(err) => {
                tracing::warn!(
                    target: "bong::craft",
                    "failed to load recipe unlock log at {}: {err} — starting fresh",
                    path.display()
                );
            }
        }
        state
    }

    /// 默认路径（`data/craft/recipe_unlocks.json`）hydrator — `register` 启动路径用。
    pub fn hydrated() -> Self {
        Self::hydrated_from_path(DEFAULT_UNLOCK_PATH)
    }
}

/// 启动期 / 测试用 — 读取磁盘 log 重建 in-memory state。
pub fn load_recipe_unlock_log(path: impl AsRef<Path>) -> Result<RecipeUnlockFile, String> {
    let path = path.as_ref();
    let raw =
        fs::read_to_string(path).map_err(|e| format!("read {} failed: {e}", path.display()))?;
    serde_json::from_str(&raw).map_err(|e| format!("parse {} failed: {e}", path.display()))
}

/// system — 按节流窗口把 dirty 的 `RecipeUnlockState` 刷盘。
/// 挂进 `Update`；unlock 三渠道 + 材料发现路径都是直接函数调用（非事件驱动），
/// 所以本系统只负责节流计时 + flush，不读取任何 EventReader。
pub fn tick_recipe_unlock_flush(mut state: ResMut<RecipeUnlockState>) {
    state.flush_clock = state.flush_clock.saturating_add(1);
    if state.flush_clock >= state.flush_interval_ticks && state.dirty {
        if let Err(error) = state.flush() {
            tracing::warn!(
                target: "bong::craft",
                "recipe unlock flush failed: {error}"
            );
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

/// 材料发现解锁结果（plan-craft-material-discovery）。
///
/// 与三渠道 [`UnlockOutcome`] 分开：材料发现是**被动发现路径**，不挂
/// `UnlockEventSource`、不走 wire/agent 的解锁广播（避免为它扩 proto / 客户端
/// tagged union）。调用方据此决定是否刷新配方列表 + 推 narration。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterialUnlockOutcome {
    /// 实际新增解锁 — 调用方应刷新该玩家配方列表并提示
    Newly,
    /// 已解锁，noop
    Already,
    /// 不适用：配方有显式解锁来源（秘传，材料不该泄露），
    /// 或 `acquired_template` 不是该配方原料
    NotApplicable,
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

/// 渠道四：材料发现解锁（plan-craft-material-discovery）。
///
/// 把原先"空 `unlock_sources` = 默认全解锁"改为"**持有任一原料才解锁**"：
///   * **仅对无显式解锁来源的配方生效** —— 残卷/师承/顿悟门控的秘传配方
///     不因玩家手里有某种常见材料而泄露（worldview §九 信息差）。
///   * `acquired_template` 必须是 `recipe.materials` 中任意一项（持有任一即可，
///     不要求集齐全部原料；集齐与否由 `start_craft` 的材料校验另行把关）。
///
/// 调用方：[`crate::network::craft_emit::apply_material_discovery_unlock`] 扫描
/// 玩家背包后调用。本函数只改 `RecipeUnlockState`，不发事件 / 不刷 UI（由调用方负责）。
pub fn unlock_via_material(
    state: &mut RecipeUnlockState,
    player: &str,
    recipe: &super::recipe::CraftRecipe,
    acquired_template: &str,
) -> MaterialUnlockOutcome {
    // 秘传配方（有显式 unlock_sources）不走材料发现路径。
    if !recipe.unlock_sources.is_empty() {
        return MaterialUnlockOutcome::NotApplicable;
    }
    let is_ingredient = recipe
        .materials
        .iter()
        .any(|(template, _)| template == acquired_template);
    if !is_ingredient {
        return MaterialUnlockOutcome::NotApplicable;
    }
    if state.is_unlocked(player, &recipe.id) {
        return MaterialUnlockOutcome::Already;
    }
    state.unlock(player.to_string(), recipe.id.clone());
    MaterialUnlockOutcome::Newly
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

/// plan-craft-material-discovery — 配方查询：哪些配方可由"获得材料 X"被动解锁。
///
/// 只含**无显式解锁来源**、且原料含 `item_template` 的配方（与 [`unlock_via_material`]
/// 的判定一致）。秘传配方即使原料里有 X 也不会出现在这里。
pub fn find_recipes_unlockable_by_material<'a>(
    registry: &'a super::registry::CraftRegistry,
    item_template: &str,
) -> Vec<&'a super::recipe::CraftRecipe> {
    registry
        .iter()
        .filter(|r| {
            r.unlock_sources.is_empty() && r.materials.iter().any(|(t, _)| t == item_template)
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
            station: None,
        }
    }

    /// 无显式解锁来源（材料发现路径）的配方，可指定 id + 原料清单。
    fn empty_source_recipe(id: &str, materials: &[(&str, u32)]) -> CraftRecipe {
        CraftRecipe {
            id: RecipeId::new(id),
            category: CraftCategory::Tool,
            display_name: id.into(),
            materials: materials
                .iter()
                .map(|(t, c)| ((*t).to_string(), *c))
                .collect(),
            qi_cost: 0.0,
            time_ticks: 60,
            output: ("test_out".into(), 1),
            requirements: CraftRequirements::default(),
            unlock_sources: vec![],
            station: None,
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

    // ===== 持久化（照抄 mineral::persistence::ExhaustedMineralsLog 测试模式）=====

    fn unique_tmp_path(name: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("bong-craft-recipe-unlocks-{stamp}-{name}.json"))
    }

    #[test]
    fn new_state_is_not_dirty() {
        let state = RecipeUnlockState::new();
        assert!(
            !state.is_dirty(),
            "freshly constructed state must not be dirty (nothing to flush)"
        );
    }

    #[test]
    fn unlock_marks_dirty_when_actually_new() {
        let mut state = RecipeUnlockState::new();
        assert!(!state.is_dirty());
        state.unlock("offline:Alice", RecipeId::new("a"));
        assert!(
            state.is_dirty(),
            "unlock() inserting a new recipe must mark state dirty so flush picks it up"
        );
    }

    #[test]
    fn repeated_unlock_does_not_redundantly_mark_dirty() {
        let mut state = RecipeUnlockState::new();
        state.unlock("offline:Alice", RecipeId::new("a"));
        state.flush_clock = 0; // irrelevant to this assertion, just documenting isolation
                               // manually clear dirty to simulate "already flushed"
        state.dirty = false;
        // unlocking the same recipe again is a noop — must NOT re-dirty the state,
        // otherwise every duplicate unlock attempt would force an unnecessary disk write.
        let inserted = state.unlock("offline:Alice", RecipeId::new("a"));
        assert!(!inserted, "duplicate unlock must report no insertion");
        assert!(
            !state.is_dirty(),
            "duplicate unlock must not re-mark a clean state dirty"
        );
    }

    #[test]
    fn clear_for_player_marks_dirty_only_when_something_removed() {
        let mut state = RecipeUnlockState::new();
        state.unlock("offline:Alice", RecipeId::new("a"));
        state.dirty = false; // simulate "already flushed"
        state.clear_for_player("offline:Alice");
        assert!(
            state.is_dirty(),
            "clearing a player that actually had unlocks must mark state dirty"
        );

        state.dirty = false;
        state.clear_for_player("offline:Alice"); // already empty — noop
        assert!(
            !state.is_dirty(),
            "clearing an already-empty player must not spuriously mark dirty"
        );
    }

    #[test]
    fn flush_writes_json_and_roundtrips() {
        let path = unique_tmp_path("flush_writes");
        let mut state = RecipeUnlockState::default().with_path(&path);
        state.unlock("offline:Alice", RecipeId::new("craft.example.a"));
        state.flush().expect("flush should succeed");

        let loaded = load_recipe_unlock_log(&path).expect("load should parse");
        assert_eq!(loaded.version, 1);
        assert_eq!(
            loaded
                .by_player
                .get("offline:Alice")
                .map(|s| s.contains(&RecipeId::new("craft.example.a"))),
            Some(true),
            "flushed file must contain the unlocked recipe for offline:Alice"
        );
        assert!(
            !state.is_dirty(),
            "flush must clear the dirty flag on success"
        );

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn flush_no_op_when_clean() {
        let path = unique_tmp_path("flush_clean");
        let mut state = RecipeUnlockState::default().with_path(&path);
        // 没 unlock 过 → 不应写文件
        state.flush().expect("clean flush ok");
        assert!(!path.exists(), "clean flush should not create a file");
    }

    #[test]
    fn hydrated_from_missing_path_returns_empty_state() {
        let path = unique_tmp_path("hydrate_missing");
        assert!(!path.exists());
        let state = RecipeUnlockState::hydrated_from_path(&path);
        assert_eq!(state.player_count(), 0);
        assert!(!state.is_dirty(), "fresh startup state must not be dirty");
        assert_eq!(state.file_path, path);
    }

    #[test]
    fn hydrated_restores_state_from_prior_flush() {
        let path = unique_tmp_path("hydrate_restore");
        // 先 flush 一份到磁盘（模拟上次关服）
        let mut prior = RecipeUnlockState::default().with_path(&path);
        prior.unlock("offline:Alice", RecipeId::new("craft.example.a"));
        prior.unlock("offline:Alice", RecipeId::new("craft.example.b"));
        prior.unlock("offline:Bob", RecipeId::new("craft.example.c"));
        prior.flush().expect("flush should succeed");

        // hydrate 回来
        let restored = RecipeUnlockState::hydrated_from_path(&path);
        assert_eq!(
            restored.unlocked_count("offline:Alice"),
            2,
            "Alice should have both recipes restored after restart"
        );
        assert!(restored.is_unlocked("offline:Alice", &RecipeId::new("craft.example.a")));
        assert!(restored.is_unlocked("offline:Alice", &RecipeId::new("craft.example.b")));
        assert!(
            restored.is_unlocked("offline:Bob", &RecipeId::new("craft.example.c")),
            "multi-player state must round-trip independently — Bob's unlock is separate from Alice's"
        );
        assert!(
            !restored.is_unlocked("offline:Bob", &RecipeId::new("craft.example.a")),
            "Bob must not inherit Alice's unlocks after hydrate — per-player isolation must survive persistence"
        );
        assert!(
            !restored.is_dirty(),
            "hydrated state must not be dirty on startup"
        );

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn hydrated_falls_back_when_file_corrupt() {
        let path = unique_tmp_path("hydrate_corrupt");
        fs::write(&path, "corrupted json {{{").unwrap();
        let state = RecipeUnlockState::hydrated_from_path(&path);
        // 坏文件 → 空 state（warn log 已发，启动不阻塞）
        assert_eq!(state.player_count(), 0);
        assert!(!state.is_dirty());
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn load_recipe_unlock_log_rejects_invalid_json() {
        let path = unique_tmp_path("invalid_json");
        fs::write(&path, "not valid json").unwrap();
        assert!(load_recipe_unlock_log(&path).is_err());
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn multi_player_flush_hydrate_keeps_each_player_isolated() {
        let path = unique_tmp_path("multi_player_isolation");
        let mut state = RecipeUnlockState::default().with_path(&path);
        state.unlock("offline:Alice", RecipeId::new("craft.example.a"));
        state.unlock("offline:Bob", RecipeId::new("craft.example.b"));
        state.unlock("offline:Carol", RecipeId::new("craft.example.c"));
        state.flush().expect("flush should succeed");

        let restored = RecipeUnlockState::hydrated_from_path(&path);
        assert_eq!(restored.player_count(), 3);
        assert!(restored.is_unlocked("offline:Alice", &RecipeId::new("craft.example.a")));
        assert!(!restored.is_unlocked("offline:Alice", &RecipeId::new("craft.example.b")));
        assert!(restored.is_unlocked("offline:Bob", &RecipeId::new("craft.example.b")));
        assert!(!restored.is_unlocked("offline:Bob", &RecipeId::new("craft.example.c")));
        assert!(restored.is_unlocked("offline:Carol", &RecipeId::new("craft.example.c")));
        assert!(!restored.is_unlocked("offline:Carol", &RecipeId::new("craft.example.a")));

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn tick_recipe_unlock_flush_only_writes_after_interval_when_dirty() {
        let path = unique_tmp_path("tick_flush_interval");
        let mut app = valence::prelude::App::new();
        let state = RecipeUnlockState::default()
            .with_path(&path)
            .with_flush_interval(3);
        app.insert_resource(state);
        app.add_systems(valence::prelude::Update, tick_recipe_unlock_flush);

        // dirty=false, interval 未到 → 多次 tick 都不应写文件
        app.update();
        app.update();
        assert!(
            !path.exists(),
            "flush system must not write to disk while state is clean"
        );

        // 标记 dirty，但节流窗口还没到（第 3 次 tick 才到期）
        app.world_mut()
            .resource_mut::<RecipeUnlockState>()
            .unlock("offline:Alice", RecipeId::new("craft.example.a"));
        app.update(); // tick 3 of interval-3 window (clock started counting from app.update() calls above)

        // 继续 tick 直到节流窗口耗尽，确保最终确实落盘
        for _ in 0..5 {
            app.update();
            if path.exists() {
                break;
            }
        }
        assert!(
            path.exists(),
            "flush system must eventually persist dirty state once flush_interval_ticks elapses"
        );
        let loaded = load_recipe_unlock_log(&path).expect("load should parse");
        assert!(loaded
            .by_player
            .get("offline:Alice")
            .map(|s| s.contains(&RecipeId::new("craft.example.a")))
            .unwrap_or(false));

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn tick_recipe_unlock_flush_clears_dirty_after_writing() {
        let path = unique_tmp_path("tick_flush_clears_dirty");
        let mut app = valence::prelude::App::new();
        let state = RecipeUnlockState::default()
            .with_path(&path)
            .with_flush_interval(1);
        app.insert_resource(state);
        app.add_systems(valence::prelude::Update, tick_recipe_unlock_flush);

        app.world_mut()
            .resource_mut::<RecipeUnlockState>()
            .unlock("offline:Alice", RecipeId::new("craft.example.a"));
        app.update();

        let resource = app.world().resource::<RecipeUnlockState>();
        assert!(
            !resource.is_dirty(),
            "after a successful throttled flush the dirty flag must be cleared"
        );

        let _ = fs::remove_file(&path);
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
        assert_eq!(
            matches.len(),
            1,
            "expected one recipe because scroll_herb_knife_iron maps to one example, actual matches={matches:?}"
        );
        assert_eq!(
            matches[0].id.as_str(),
            "craft.example.herb_knife.iron",
            "expected herb knife recipe for scroll_herb_knife_iron, actual id={}",
            matches[0].id.as_str()
        );
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
        assert_eq!(
            matches.len(),
            1,
            "expected one recipe because array_scribe teaches one example, actual matches={matches:?}"
        );
        assert_eq!(
            matches[0].id.as_str(),
            "craft.example.zhenfa_trap.iron",
            "expected zhenfa trap recipe for array_scribe, actual id={}",
            matches[0].id.as_str()
        );
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

    // ── plan-craft-material-discovery — unlock_via_material ──────────

    #[test]
    fn material_unlock_succeeds_when_ingredient_present_and_empty_source() {
        let mut state = RecipeUnlockState::new();
        let recipe = empty_source_recipe("craft.tool.knife", &[("fan_tie", 2)]);
        let outcome = unlock_via_material(&mut state, "offline:Alice", &recipe, "fan_tie");
        assert_eq!(
            outcome,
            MaterialUnlockOutcome::Newly,
            "持有空源配方的原料应触发新解锁"
        );
        assert!(state.is_unlocked("offline:Alice", &recipe.id));
    }

    #[test]
    fn material_unlock_unlocks_via_any_one_of_multiple_ingredients() {
        // 多原料配方：持有其中任一即解锁（不要求集齐）
        let recipe = empty_source_recipe("craft.tool.multi", &[("fan_tie", 2), ("crude_wood", 1)]);
        for trigger in ["fan_tie", "crude_wood"] {
            let mut state = RecipeUnlockState::new();
            assert_eq!(
                unlock_via_material(&mut state, "offline:Alice", &recipe, trigger),
                MaterialUnlockOutcome::Newly,
                "持有原料 `{trigger}` 应足以解锁多原料配方"
            );
        }
    }

    #[test]
    fn material_unlock_not_applicable_for_recipe_with_explicit_source() {
        // 秘传配方（有 Scroll 源）即使持有其原料也不解锁 —— worldview §九 信息差
        let mut state = RecipeUnlockState::new();
        let mut recipe = empty_source_recipe("craft.secret.poison", &[("herb_a", 1)]);
        recipe.unlock_sources = vec![UnlockSource::Scroll {
            item_template: "scroll_secret".into(),
        }];
        let outcome = unlock_via_material(&mut state, "offline:Alice", &recipe, "herb_a");
        assert_eq!(
            outcome,
            MaterialUnlockOutcome::NotApplicable,
            "有显式解锁来源的秘传配方不应被材料发现解锁"
        );
        assert!(!state.is_unlocked("offline:Alice", &recipe.id));
    }

    #[test]
    fn material_unlock_not_applicable_when_template_not_ingredient() {
        let mut state = RecipeUnlockState::new();
        let recipe = empty_source_recipe("craft.tool.knife", &[("fan_tie", 2)]);
        let outcome = unlock_via_material(&mut state, "offline:Alice", &recipe, "zhu_pi");
        assert_eq!(
            outcome,
            MaterialUnlockOutcome::NotApplicable,
            "非该配方原料的物品不应解锁该配方"
        );
        assert!(!state.is_unlocked("offline:Alice", &recipe.id));
    }

    #[test]
    fn material_unlock_already_when_repeated() {
        let mut state = RecipeUnlockState::new();
        let recipe = empty_source_recipe("craft.tool.knife", &[("fan_tie", 2)]);
        unlock_via_material(&mut state, "offline:Alice", &recipe, "fan_tie");
        let again = unlock_via_material(&mut state, "offline:Alice", &recipe, "fan_tie");
        assert_eq!(
            again,
            MaterialUnlockOutcome::Already,
            "已解锁配方重复发现应返回 Already（noop，避免重复刷 UI）"
        );
    }

    #[test]
    fn material_unlock_is_player_scoped() {
        let mut state = RecipeUnlockState::new();
        let recipe = empty_source_recipe("craft.tool.knife", &[("fan_tie", 2)]);
        unlock_via_material(&mut state, "offline:Alice", &recipe, "fan_tie");
        assert!(state.is_unlocked("offline:Alice", &recipe.id));
        assert!(
            !state.is_unlocked("offline:Bob", &recipe.id),
            "材料发现解锁应 per-player，不应泄露给其他玩家"
        );
    }

    // ── find_recipes_unlockable_by_material ──────────────────────────

    #[test]
    fn find_by_material_returns_only_empty_source_with_matching_ingredient() {
        let mut registry = super::super::registry::CraftRegistry::new();
        registry
            .register(empty_source_recipe("craft.tool.a", &[("fan_tie", 1)]))
            .unwrap();
        registry
            .register(empty_source_recipe("craft.tool.b", &[("zhu_pi", 1)]))
            .unwrap();
        // 秘传配方：原料含 fan_tie，但有 Scroll 源 → 不应出现
        let mut secret = empty_source_recipe("craft.secret.c", &[("fan_tie", 1)]);
        secret.unlock_sources = vec![UnlockSource::Scroll {
            item_template: "scroll_c".into(),
        }];
        registry.register(secret).unwrap();

        let matches = find_recipes_unlockable_by_material(&registry, "fan_tie");
        assert_eq!(
            matches.len(),
            1,
            "fan_tie 只应匹配空源的 craft.tool.a，秘传 craft.secret.c 被排除，实际={matches:?}"
        );
        assert_eq!(matches[0].id.as_str(), "craft.tool.a");
    }

    #[test]
    fn find_by_material_returns_empty_when_no_recipe_uses_it() {
        let mut registry = super::super::registry::CraftRegistry::new();
        registry
            .register(empty_source_recipe("craft.tool.a", &[("fan_tie", 1)]))
            .unwrap();
        assert!(find_recipes_unlockable_by_material(&registry, "unobtainium").is_empty());
    }
}
