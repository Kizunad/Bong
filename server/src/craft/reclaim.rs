//! plan-coffin-tiers-v1 P2 §8.1 #3 — 可放置物回收返还（通用工具）。
//!
//! 当玩家**破坏**或主动**回收**一个由 `CraftRecipe` 合成的可放置物（延寿棺 /
//! 未来的家具 / 工坊台等）时，按配方原料返还部分材料。本模块只负责"返还多少"
//! 的纯计算，**不**碰 inventory / ECS / Commands —— 调用方拿到 `Vec<(template, count)>`
//! 后自行发还（守恒：这是合成投入材料的部分回收，不凭空造物）。
//!
//! 两种模式（§8.1 #3）：
//!   * [`ReclaimMode::Break`]   — 左键攻击破坏，体验同破坏方块：**随机部分**返还
//!     （每种材料数量 ∈ `[0, count]`，模拟暴力破坏的损耗惩罚）。
//!   * [`ReclaimMode::Reclaim`] — G 菜单主动 [回收]：**较全**返还（每种材料 full
//!     `count`，奖励玩家主动回收而非砸毁）。
//!
//! **确定性**：核心 `recipe_reclaim_drops_seeded` 接受显式 `seed`，同 seed 同输入
//! 必出同输出（单测可锁边界）。运行时入口 `recipe_reclaim_drops` 同样接受 `seed`
//! （由调用方按上下文派生，如棺位 + tick 经 `DefaultHasher` 散列，对齐仓内 loot
//! roll 的熵源约定），并过滤掉 Break 模式掷出的 0 数量项。测试一律走 seeded 版本。

use crate::craft::recipe::CraftRecipe;

/// 回收模式（§8.1 #3）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReclaimMode {
    /// 破坏（左键攻击）—— 随机部分返还，数量 ∈ `[0, count]`。
    Break,
    /// 主动回收（G 菜单）—— 较全返还（full count）。
    Reclaim,
}

/// 简易确定性 PRNG（splitmix64）。仅用于回收返还的"随机部分"计算，**不**用于
/// 任何安全 / 守恒敏感场景，目的是让破坏返还有一点抖动且单测可复现。
fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// 给定 `count`（>=1）与 seed，返回 `[0, count]` 闭区间内的均匀随机值。
fn random_partial(count: u32, rng_state: &mut u64) -> u32 {
    // count==0 的材料在配方校验阶段已被拒（CraftRecipe::validate），这里防御性返回 0。
    if count == 0 {
        return 0;
    }
    // [0, count] 闭区间共 count+1 个取值，取模即可（count 远小于 u64 范围，无显著偏置）。
    (splitmix64(rng_state) % u64::from(count + 1)) as u32
}

/// 纯函数核心：按配方原料与模式计算返还清单（确定性，由 `seed` 驱动随机）。
///
/// 返回 `Vec<(template_id, count)>`，与 `CraftRecipe.materials` 同序（保留配方顺序，
/// 便于 e2e 断言）。**Break 模式下 count 可能为 0** —— 调用方需自行跳过 0 数量项
/// （`recipe_reclaim_drops` 运行时入口已过滤）。Reclaim 模式恒等于配方原料 full count。
///
/// 不变量（单测锁定）：
///   * 空配方 → 空 vec
///   * 任意模式 / seed：`Break.count ∈ [0, material.count]`
///   * `Reclaim.count == material.count`（恒 >= 对应 Break，"回收 >= 破坏"）
pub fn recipe_reclaim_drops_seeded(
    recipe: &CraftRecipe,
    mode: ReclaimMode,
    seed: u64,
) -> Vec<(String, u32)> {
    let mut rng_state = seed;
    recipe
        .materials
        .iter()
        .map(|(template, count)| {
            let returned = match mode {
                ReclaimMode::Reclaim => *count,
                ReclaimMode::Break => random_partial(*count, &mut rng_state),
            };
            (template.clone(), returned)
        })
        .collect()
}

/// 运行时入口：用调用方提供的 `seed` 驱动 [`recipe_reclaim_drops_seeded`]，并过滤掉
/// 0 数量项（Break 模式可能掷出 0）。调用方拿到的每项 count 都 >= 1，可直接发还
/// inventory。`seed` 应按上下文派生（如棺位 + tick 散列），保持运行时熵源与仓内
/// loot roll 一致、且不引入新依赖。
pub fn recipe_reclaim_drops(
    recipe: &CraftRecipe,
    mode: ReclaimMode,
    seed: u64,
) -> Vec<(String, u32)> {
    recipe_reclaim_drops_seeded(recipe, mode, seed)
        .into_iter()
        .filter(|(_, count)| *count > 0)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::craft::recipe::{CraftCategory, CraftRecipe, CraftRequirements, RecipeId};

    fn recipe_with(materials: Vec<(&str, u32)>) -> CraftRecipe {
        CraftRecipe {
            id: RecipeId::new("test.reclaim"),
            category: CraftCategory::Misc,
            display_name: "回收测试配方".into(),
            materials: materials
                .into_iter()
                .map(|(t, c)| (t.to_string(), c))
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
    fn empty_recipe_returns_empty_drops_for_both_modes() {
        // 无原料配方（理论边界）→ 空返还，不 panic。
        let recipe = recipe_with(vec![]);
        for mode in [ReclaimMode::Break, ReclaimMode::Reclaim] {
            let drops = recipe_reclaim_drops_seeded(&recipe, mode, 1234);
            assert!(
                drops.is_empty(),
                "空配方 {mode:?} 应返还空 vec，实得 {drops:?}"
            );
        }
    }

    #[test]
    fn reclaim_mode_returns_full_materials() {
        // Reclaim = 较全返还：每种材料 full count，与配方原料逐项相等。
        let recipe = recipe_with(vec![("ling_mu_ban", 6), ("ling_mu_gun", 2)]);
        let drops = recipe_reclaim_drops_seeded(&recipe, ReclaimMode::Reclaim, 999);
        assert_eq!(
            drops,
            vec![
                ("ling_mu_ban".to_string(), 6),
                ("ling_mu_gun".to_string(), 2)
            ],
            "Reclaim 应 full 返还配方原料（含顺序），实得 {drops:?}"
        );
    }

    #[test]
    fn break_mode_each_material_within_zero_to_count() {
        // Break = 随机部分：每种材料返还 ∈ [0, count]，跨多个 seed 全覆盖仍成立。
        let recipe = recipe_with(vec![("a", 1), ("b", 4), ("c", 10)]);
        for seed in 0u64..200 {
            let drops = recipe_reclaim_drops_seeded(&recipe, ReclaimMode::Break, seed);
            assert_eq!(drops.len(), 3, "Break 应保留全部材料项（数量可为 0）");
            for ((template, returned), (_, original)) in drops.iter().zip(recipe.materials.iter()) {
                assert!(
                    *returned <= *original,
                    "Break 返还 {template}={returned} 超出原料上限 {original}（seed={seed}）"
                );
            }
        }
    }

    #[test]
    fn break_count_one_material_yields_zero_or_one() {
        // 边界 count=1：Break 只能掷出 0 或 1，且两种结果在足够多 seed 下都出现。
        let recipe = recipe_with(vec![("solo", 1)]);
        let mut saw_zero = false;
        let mut saw_one = false;
        for seed in 0u64..64 {
            let drops = recipe_reclaim_drops_seeded(&recipe, ReclaimMode::Break, seed);
            let returned = drops[0].1;
            assert!(
                returned <= 1,
                "count=1 的 Break 返还必 <=1，实得 {returned}（seed={seed}）"
            );
            saw_zero |= returned == 0;
            saw_one |= returned == 1;
        }
        assert!(
            saw_zero && saw_one,
            "count=1 的 Break 应在足够多 seed 下既能掷 0 又能掷 1（saw_zero={saw_zero}, saw_one={saw_one}）"
        );
    }

    #[test]
    fn reclaim_is_always_greater_or_equal_to_break_per_material() {
        // 核心契约："主动回收 >= 暴力破坏"。逐材料、跨 seed 恒成立。
        let recipe = recipe_with(vec![("x", 3), ("y", 7), ("z", 2)]);
        let reclaim = recipe_reclaim_drops_seeded(&recipe, ReclaimMode::Reclaim, 0);
        for seed in 0u64..200 {
            let brk = recipe_reclaim_drops_seeded(&recipe, ReclaimMode::Break, seed);
            for ((_, r_count), (_, b_count)) in reclaim.iter().zip(brk.iter()) {
                assert!(
                    *r_count >= *b_count,
                    "Reclaim({r_count}) 必 >= Break({b_count})（seed={seed}）—— 回收奖励不应低于破坏"
                );
            }
        }
    }

    #[test]
    fn seeded_is_deterministic() {
        // 同 seed + 同配方 → 同输出（单测可复现的前提）。
        let recipe = recipe_with(vec![("a", 5), ("b", 9)]);
        let first = recipe_reclaim_drops_seeded(&recipe, ReclaimMode::Break, 42);
        let second = recipe_reclaim_drops_seeded(&recipe, ReclaimMode::Break, 42);
        assert_eq!(first, second, "同 seed 的 Break 返还必须确定性一致");
    }

    #[test]
    fn runtime_entry_filters_zero_counts() {
        // 运行时入口过滤 0 数量项：返回的每项 count 都 >= 1（直接喂 inventory 安全）。
        // 用一堆 count=1 材料，统计上几乎必有若干被掷成 0 → 验证过滤生效（len 可 < 原料数）。
        let recipe = recipe_with(vec![("a", 1), ("b", 1), ("c", 1), ("d", 1)]);
        let mut saw_filtered = false;
        for seed in 0u64..50 {
            let drops = recipe_reclaim_drops(&recipe, ReclaimMode::Break, seed);
            for (_, count) in &drops {
                assert!(*count >= 1, "运行时入口返回项 count 必 >=1，实得 {count}");
            }
            if drops.len() < recipe.materials.len() {
                saw_filtered = true;
            }
        }
        assert!(
            saw_filtered,
            "运行时 Break 入口应能过滤掉 0 数量项（多次抽样后至少一次 len < 原料数）"
        );
    }

    #[test]
    fn runtime_reclaim_returns_all_materials_nonzero() {
        // Reclaim 运行时入口：full 返还，0 过滤不影响（无 0 项），逐项等于原料。
        let recipe = recipe_with(vec![("ling_mu_ban", 6), ("ling_mu_gun", 2)]);
        let drops = recipe_reclaim_drops(&recipe, ReclaimMode::Reclaim, 7);
        assert_eq!(
            drops,
            vec![
                ("ling_mu_ban".to_string(), 6),
                ("ling_mu_gun".to_string(), 2)
            ],
            "Reclaim 运行时入口应 full 返还所有材料"
        );
    }
}
