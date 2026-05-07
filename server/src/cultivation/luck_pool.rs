//! Per-life 运数池（plan-multi-life-v1 §3 Q-ML 设计轴心 / O.4 决策）。
//!
//! 本模块是 [`crate::combat::components::Lifecycle::fortune_remaining`] 的语义门面。
//! plan-multi-life-v1 §0 第 1 条要求："每角色独立 3 次,**不跨角色累计**——
//! 每新角色满运数重置"。combat 端 `Lifecycle::fortune_remaining` 已是 per-character
//! 字段（character_id 绑定，新角色由 [`reset_for_new_life`] 风格逻辑写回 3），
//! 此模块仅暴露明确命名的查询 / 扣减 / 重置 API，让 multi-life 流程的调用点
//! 能从语义层（"运数池"）而非数据层（`fortune_remaining` 字段）写代码。
//!
//! `INITIAL_FORTUNE_PER_LIFE` 取代散落各处的 `3` 字面量。修改此常数同时改变
//! 新角色 / 重生扣减判定的初始值，单一来源（plan §0 决策："每角色独立 3 次"）。

use crate::combat::components::Lifecycle;

/// 每个角色出生时的运数额度（plan-multi-life-v1 §0 O.4 决策）。
///
/// **per-life 语义**：角色终结后开新角色，运数池重置回此值；不跨角色累计。
/// 当前数值与 `combat::components::DEFAULT_FORTUNE_REMAINING` 对齐。
pub const INITIAL_FORTUNE_PER_LIFE: u8 = 3;

/// 查询当前角色剩余运数。
///
/// 返回 0 表示运数耗尽——下次死亡若非自然老死也会触发角色终结。
pub fn current_fortune(lifecycle: &Lifecycle) -> u8 {
    lifecycle.fortune_remaining
}

/// 是否运数已耗尽（== 0）。
pub fn is_exhausted(lifecycle: &Lifecycle) -> bool {
    lifecycle.fortune_remaining == 0
}

/// 扣 1 点运数。
///
/// - 成功扣减：返回 `Some(剩余值)`，剩余值范围 `0..=INITIAL_FORTUNE_PER_LIFE-1`
/// - 已为 0：返回 `None`，不修改 lifecycle，调用方应当转走终结分支
///
/// 该函数只更新运数字段；death_count / state 等由 combat::lifecycle 流程负责。
pub fn spend_fortune(lifecycle: &mut Lifecycle) -> Option<u8> {
    if lifecycle.fortune_remaining == 0 {
        return None;
    }
    lifecycle.fortune_remaining -= 1;
    Some(lifecycle.fortune_remaining)
}

/// 重置运数池为新一世的初始额度（plan-multi-life-v1 §2 流程：
/// "新角色生成: ... 运数 = 3"）。同时清零 death_count，使新角色 LifeRecord
/// 不携带前世死亡计数。
///
/// 调用点：`combat::lifecycle::reset_for_new_character` 与 multi-life
/// `character_select::next_character_spec` 应用环节。
pub fn reset_for_new_life(lifecycle: &mut Lifecycle) {
    lifecycle.fortune_remaining = INITIAL_FORTUNE_PER_LIFE;
    lifecycle.death_count = 0;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::components::{Lifecycle, LifecycleState};

    fn fresh_lifecycle(char_id: &str) -> Lifecycle {
        Lifecycle {
            character_id: char_id.to_string(),
            ..Lifecycle::default()
        }
    }

    #[test]
    fn fresh_lifecycle_starts_with_initial_fortune() {
        let lc = fresh_lifecycle("offline:Alice:gen0");
        assert_eq!(
            current_fortune(&lc),
            INITIAL_FORTUNE_PER_LIFE,
            "新 Lifecycle 默认 fortune_remaining 应等于 INITIAL_FORTUNE_PER_LIFE",
        );
        assert!(
            !is_exhausted(&lc),
            "新角色 fortune={INITIAL_FORTUNE_PER_LIFE} 不应是 exhausted",
        );
    }

    #[test]
    fn spend_decrements_by_one() {
        let mut lc = fresh_lifecycle("offline:Alice:gen0");
        assert_eq!(spend_fortune(&mut lc), Some(2));
        assert_eq!(current_fortune(&lc), 2);
    }

    #[test]
    fn spend_to_zero_then_returns_none() {
        let mut lc = fresh_lifecycle("offline:Alice:gen0");
        assert_eq!(spend_fortune(&mut lc), Some(2));
        assert_eq!(spend_fortune(&mut lc), Some(1));
        assert_eq!(spend_fortune(&mut lc), Some(0));
        assert_eq!(
            spend_fortune(&mut lc),
            None,
            "运数为 0 时再次 spend 应返回 None，不修改 lifecycle",
        );
        assert_eq!(
            current_fortune(&lc),
            0,
            "spend_fortune 在 0 时不应使 fortune 变成下溢值",
        );
        assert!(is_exhausted(&lc), "三次扣减后应进入 exhausted 状态");
    }

    #[test]
    fn is_exhausted_boundary_one_to_zero() {
        let mut lc = fresh_lifecycle("offline:Alice:gen0");
        lc.fortune_remaining = 1;
        assert!(
            !is_exhausted(&lc),
            "fortune=1 不算 exhausted（边界条件 off-by-one 测试）",
        );
        spend_fortune(&mut lc);
        assert!(is_exhausted(&lc), "spend 到 0 后必须 exhausted");
    }

    #[test]
    fn reset_returns_to_initial_and_clears_death_count() {
        let mut lc = fresh_lifecycle("offline:Alice:gen0");
        lc.fortune_remaining = 0;
        lc.death_count = 5;

        reset_for_new_life(&mut lc);

        assert_eq!(
            current_fortune(&lc),
            INITIAL_FORTUNE_PER_LIFE,
            "reset_for_new_life 应把运数恢复到 plan §0 决议的初始值 {INITIAL_FORTUNE_PER_LIFE}",
        );
        assert_eq!(
            lc.death_count, 0,
            "新一世 death_count 必须清零，否则 LifeRecord 会沿用前世计数",
        );
    }

    #[test]
    fn reset_does_not_touch_unrelated_fields() {
        let mut lc = fresh_lifecycle("offline:Alice:gen0");
        lc.fortune_remaining = 1;
        lc.last_death_tick = Some(123);
        lc.last_revive_tick = Some(456);
        lc.state = LifecycleState::AwaitingRevival;

        reset_for_new_life(&mut lc);

        assert_eq!(
            lc.last_death_tick,
            Some(123),
            "reset_for_new_life 仅管 fortune+death_count，last_death_tick 不在范围内",
        );
        assert_eq!(lc.last_revive_tick, Some(456));
        assert_eq!(
            lc.state,
            LifecycleState::AwaitingRevival,
            "state 转换由 combat::lifecycle 负责，luck_pool 不应越界改写",
        );
    }

    #[test]
    fn per_character_isolation() {
        // plan-multi-life-v1 §0 第 1 条："per-life 运数 ... 不跨角色累计"
        // 不同 character_id 的 Lifecycle 独立扣减——每个 Component 实例
        // 持有独立的 fortune_remaining，luck_pool API 操作的是 &mut 引用。
        let mut alice_g0 = fresh_lifecycle("offline:Alice:gen0");
        let alice_g1 = fresh_lifecycle("offline:Alice:gen1");

        spend_fortune(&mut alice_g0);
        spend_fortune(&mut alice_g0);

        assert_eq!(current_fortune(&alice_g0), 1);
        assert_eq!(
            current_fortune(&alice_g1),
            INITIAL_FORTUNE_PER_LIFE,
            "扣 alice_g0 不应影响 alice_g1（per-character 隔离）",
        );
    }

    #[test]
    fn spend_after_reset_works_again() {
        let mut lc = fresh_lifecycle("offline:Alice:gen0");
        spend_fortune(&mut lc);
        spend_fortune(&mut lc);
        spend_fortune(&mut lc);
        assert!(is_exhausted(&lc));

        reset_for_new_life(&mut lc);

        assert_eq!(spend_fortune(&mut lc), Some(2));
        assert_eq!(spend_fortune(&mut lc), Some(1));
        assert_eq!(spend_fortune(&mut lc), Some(0));
        assert_eq!(spend_fortune(&mut lc), None);
    }

    #[test]
    fn initial_fortune_matches_combat_default() {
        // 防回归：luck_pool 的初始值不能跟 combat 默认值漂移。
        // 若 combat::components::DEFAULT_FORTUNE_REMAINING 改了，本测试会撞红
        // 提示同步更新 plan-multi-life-v1 §0 决策与本常数。
        let lc = Lifecycle::default();
        assert_eq!(
            lc.fortune_remaining, INITIAL_FORTUNE_PER_LIFE,
            "Lifecycle::default().fortune_remaining 必须等于 luck_pool::INITIAL_FORTUNE_PER_LIFE，否则两套数值会漂移",
        );
    }
}
