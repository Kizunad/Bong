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
use valence::prelude::{AppExit, EventReader, ResMut, Resource};

use super::events::{InsightTrigger, UnlockEventSource};
use super::recipe::{RecipeId, UnlockSource};

const DEFAULT_UNLOCK_PATH: &str = "data/craft/recipe_unlocks.json";

/// 基线常显配方 — 不经任何解锁渠道，对所有玩家恒可见、恒可做，死亡重生也不清。
///
/// 目前仅制作台自身（`craft.tool.workbench`）：
/// 它是整棵 workbench 配方树的物理入口。若连它也走材料发现，玩家没摸过
/// spirit_wood / iron_ingot / shu_gu 之前根本不知道"制作台"这条路存在，
/// 入口配方被入口自身的原料锁死。秘传配方（残卷/师承/顿悟）不属于此列。
pub const BASELINE_RECIPES: &[&str] = &["craft.tool.workbench"];

/// 某配方是否属于基线常显集合（[`BASELINE_RECIPES`]）。
pub fn is_baseline_recipe(recipe: &RecipeId) -> bool {
    BASELINE_RECIPES.contains(&recipe.as_str())
}

/// 落盘 schema 版本 — writer（[`RecipeUnlockState::flush`]）与 loader
/// （[`load_recipe_unlock_log`]）必须共用同一常量，避免两处字面量漂移。
/// 递增时需要在 loader 里补迁移逻辑，而不是放宽校验。
const RECIPE_UNLOCK_VERSION: u32 = 1;

/// 落盘格式 wrapper — 留 `version` 字段方便后续 schema 演进。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecipeUnlockFile {
    pub version: u32,
    pub by_player: HashMap<String, HashSet<RecipeId>>,
}

impl Default for RecipeUnlockFile {
    fn default() -> Self {
        Self {
            version: RECIPE_UNLOCK_VERSION,
            by_player: HashMap::new(),
        }
    }
}

/// per-player canonical id 的 unlock 集合 + 节流刷盘 — 在 `register` 时插入到
/// ECS resource。
#[derive(Debug)]
pub struct RecipeUnlockState {
    /// `canonical_player_id`（如 `"offline:Alice"`）→ 已解锁配方集合。
    /// 玩家未注册时返回空 set，等同未解锁任何配方。
    by_player: HashMap<String, HashSet<RecipeId>>,
    /// 本帧已排队、尚未由 `apply_unlock_intents` 提交的残卷解锁，防止同一卷轴被重复消费。
    pending_scroll_unlocks: HashSet<(String, RecipeId)>,
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
            pending_scroll_unlocks: HashSet::new(),
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

    /// 查询玩家是否已解锁某配方。基线配方恒 true；未注册玩家恒返回 false。
    ///
    /// 基线豁免在此单点生效：`start_craft` 门、`build_recipe_list_payload`
    /// 下发过滤、`apply_material_discovery_unlock` 的已解锁短路（避免把基线
    /// 配方写进 `by_player` / 落盘）全部经由本函数，无需各调用点自查。
    pub fn is_unlocked(&self, player: &str, recipe: &RecipeId) -> bool {
        if is_baseline_recipe(recipe) {
            return true;
        }
        self.by_player
            .get(player)
            .map(|s| s.contains(recipe))
            .unwrap_or(false)
    }

    /// 预留本帧将由残卷提交的解锁，防止队列中的重复请求重复扣物品。
    pub fn reserve_scroll_unlock(&mut self, player: &str, recipe: &RecipeId) -> bool {
        if self.is_unlocked(player, recipe) {
            return false;
        }
        self.pending_scroll_unlocks
            .insert((player.to_owned(), recipe.clone()))
    }

    pub fn release_scroll_unlock_reservation(&mut self, player: &str, recipe: &RecipeId) {
        self.pending_scroll_unlocks
            .remove(&(player.to_owned(), recipe.clone()));
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
        self.pending_scroll_unlocks
            .retain(|(pending_player, _)| pending_player != player);
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
    ///
    /// 原子落盘：先写同目录 `.tmp` 临时文件，成功后 `rename` 到最终路径。
    /// `rename` 在同一文件系统内是原子操作，避免写入中途失败/中断时把
    /// `self.file_path` 留成截断 JSON（下次启动会走 corrupt fallback 丢已解锁进度）。
    pub fn flush(&mut self) -> Result<(), String> {
        if !self.dirty {
            return Ok(());
        }
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("create dir {} failed: {e}", parent.display()))?;
        }
        let file = RecipeUnlockFile {
            version: RECIPE_UNLOCK_VERSION,
            by_player: self.by_player.clone(),
        };
        let json = serde_json::to_string_pretty(&file)
            .map_err(|e| format!("serialize recipe unlock log failed: {e}"))?;
        let tmp_path = self.file_path.with_extension("tmp");
        fs::write(&tmp_path, json)
            .map_err(|e| format!("write {} failed: {e}", tmp_path.display()))?;
        fs::rename(&tmp_path, &self.file_path).map_err(|e| {
            format!(
                "rename {} to {} failed: {e}",
                tmp_path.display(),
                self.file_path.display()
            )
        })?;
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
///
/// 拒绝 `version` 与 [`RECIPE_UNLOCK_VERSION`] 不一致的文件（而不是静默接受
/// 任意版本并直接恢复 `by_player`），像其他 schema loader
/// （如 `mineral::persistence::load_exhausted_log` 的落盘约定）一样把版本漂移当错误处理。
pub fn load_recipe_unlock_log(path: impl AsRef<Path>) -> Result<RecipeUnlockFile, String> {
    let path = path.as_ref();
    let raw =
        fs::read_to_string(path).map_err(|e| format!("read {} failed: {e}", path.display()))?;
    let file: RecipeUnlockFile =
        serde_json::from_str(&raw).map_err(|e| format!("parse {} failed: {e}", path.display()))?;
    if file.version != RECIPE_UNLOCK_VERSION {
        return Err(format!(
            "unsupported recipe unlock log version {} at {} (expected {RECIPE_UNLOCK_VERSION})",
            file.version,
            path.display()
        ));
    }
    Ok(file)
}

/// system — 按节流窗口把 dirty 的 `RecipeUnlockState` 刷盘。
/// 挂进 `Update`；unlock 三渠道 + 材料发现路径都是直接函数调用（非事件驱动），
/// 所以本系统只负责节流计时 + flush，不读取任何 EventReader。
pub fn tick_recipe_unlock_flush(mut state: ResMut<RecipeUnlockState>) {
    state.flush_clock = state.flush_clock.saturating_add(1);
    if state.flush_clock >= state.flush_interval_ticks && state.dirty {
        // 无论 flush 成功与否都先清零计时器：`flush()` 失败时不会清 `dirty`，
        // 如果不重置 `flush_clock`，下一帧阈值依旧满足 → 每个 Update 都重试写盘
        // + warn，磁盘/权限故障时会刷爆日志。重置后退避到下一个节流窗口再重试。
        state.flush_clock = 0;
        if let Err(error) = state.flush() {
            tracing::warn!(
                target: "bong::craft",
                "recipe unlock flush failed: {error}"
            );
        }
    }
}

/// 关服 system — 仅在本帧收到 [`AppExit`] 时强制刷盘。
///
/// 挂进 `Last`，与运行期 `Update` 的 600 tick 节流互补。具体 dirty no-op、
/// 原子 tmp + rename、失败后保留 dirty/旧文件语义统一由 [`RecipeUnlockState::flush`]
/// 提供，关服路径不复制持久化策略。
pub fn flush_recipe_unlocks_on_shutdown(
    mut state: ResMut<RecipeUnlockState>,
    mut app_exit: EventReader<AppExit>,
) {
    if app_exit.read().next().is_none() {
        return;
    }

    if let Err(error) = state.flush() {
        tracing::warn!(
            target: "bong::craft",
            "recipe unlock shutdown flush failed: {error}"
        );
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
#[path = "unlock_tests.rs"]
mod tests;
