//! plan-skill-v1 §2 / §6 XP 曲线 + effect 插值。
//!
//! - `xp_to_next(lv)` = `100 * (lv+1)^2`，Lv.9→10 时 10_000。Lv.10 为硬上限：返回 `u32::MAX`。
//! - `add_xp` 累加并跨级，返回本次跨过的**新 Lv 列表**（非结束后 lv 值）。
//! - `interp` 对 plan §6 关键 Lv 线性插值；下/上界外钳制到端点（下界外取 pts[0].1，上界外取
//!   pts.last().1，与原 plan §6.0 示意保持一致 —— 未指定端点外行为，这里保守钳制）。
//! - `effective_lv(real_lv, cap)` = `min(real, cap)`（plan §4 cap 压制）。

use super::components::{SkillEntry, Tick};

/// plan §2.1 硬上限 Lv.10。
pub const SKILL_MAX_LEVEL: u8 = 10;

/// plan §2.1 `100 * (lv+1)^2`。Lv.10 返回 `u32::MAX` 作为 hard-cap 标记：任何 xp 都追不上。
pub fn xp_to_next(lv: u8) -> u32 {
    if lv >= SKILL_MAX_LEVEL {
        return u32::MAX;
    }
    let next = (u32::from(lv)) + 1;
    100u32.saturating_mul(next.saturating_mul(next))
}

/// plan §6.0 线性插值：pts 按 lv 升序给出关键端点；端点之外钳制到最近端点。
///
/// 示例断言：`interp(4, &[(3,8.0),(5,6.0)]) == 7.0`（plan §6.0）。
pub fn interp(lv: u8, pts: &[(u8, f32)]) -> f32 {
    debug_assert!(!pts.is_empty(), "interp: pts must not be empty");
    if pts.is_empty() {
        return 0.0;
    }

    // 下界外：返回第一个端点值。plan §6.0 未明确，这里选择钳制到端点，避免 Lv.0 默认放大第一段斜率。
    let first = pts[0];
    if lv <= first.0 {
        return first.1;
    }
    // 上界外：返回最后一个端点值。
    let last = *pts.last().expect("checked non-empty above");
    if lv >= last.0 {
        return last.1;
    }

    for w in pts.windows(2) {
        let (l0, v0) = w[0];
        let (l1, v1) = w[1];
        if lv >= l0 && lv <= l1 {
            let span = f32::from(l1 - l0);
            let t = f32::from(lv - l0) / span;
            return v0 + (v1 - v0) * t;
        }
    }
    last.1
}

/// plan §4 cap 压制：`effective_lv = min(real_lv, cap)`。
pub fn effective_lv(real_lv: u8, cap: u8) -> u8 {
    real_lv.min(cap)
}

/// plan §2.1 累加 XP 并在到达阈值时升级。返回**本次跨过的新 Lv 列表**（空 = 没升）。
///
/// `now` 用来更新 `last_action_at`（UI 最近 +XP 窗口用）。`recent_repeat_count` 由调用方根据
/// 动作是否重复自行维护 —— 本函数不管（多样性奖励策略在 plan §3.1，属于 Event 阶段，不属于曲线层）。
pub fn add_xp(entry: &mut SkillEntry, amount: u32, now: Tick) -> Vec<u8> {
    let mut leveled: Vec<u8> = Vec::new();
    if amount == 0 {
        entry.last_action_at = now;
        return leveled;
    }

    entry.total_xp = entry.total_xp.saturating_add(u64::from(amount));
    entry.last_action_at = now;

    let mut remaining = amount;
    while remaining > 0 {
        if entry.lv >= SKILL_MAX_LEVEL {
            // Lv.10 硬封顶：剩余 XP 不累积到当前 lv 的 xp 桶（xp_to_next 返回 u32::MAX，
            // 直接吞掉也行；但留 xp=0 更简洁，避免 Lv.10 面板显示一堆没用的 XP）。
            entry.xp = 0;
            break;
        }
        let to_next = xp_to_next(entry.lv);
        let space = to_next.saturating_sub(entry.xp);
        if remaining < space {
            entry.xp = entry.xp.saturating_add(remaining);
            remaining = 0;
        } else {
            remaining = remaining.saturating_sub(space);
            entry.lv = entry.lv.saturating_add(1);
            entry.xp = 0;
            leveled.push(entry.lv);
        }
    }
    leveled
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skill::components::SkillEntry;

    /// plan §2.1 Lv.0-9 XP-to-next 精确数值。
    #[test]
    fn xp_to_next_matches_plan_table() {
        let expected = [100, 400, 900, 1600, 2500, 3600, 4900, 6400, 8100, 10_000];
        for (lv, exp) in expected.iter().enumerate() {
            assert_eq!(
                xp_to_next(lv as u8),
                *exp,
                "xp_to_next({lv}) should equal {exp}"
            );
        }
    }

    /// plan §2.1 Lv.10 为硬上限：返回 u32::MAX，再灌都追不上。
    #[test]
    fn xp_to_next_lv10_is_hard_cap() {
        assert_eq!(xp_to_next(10), u32::MAX);
        assert_eq!(xp_to_next(200), u32::MAX);
    }

    /// plan §2.1 累计表：Lv.0 → 5 共需 100+400+900+1600+2500 = 5_500；
    /// Lv.0 → 6 需累积 9_100；灌 11_500 必经 5→6 并在 Lv.6 内留 2_400 xp，返回 [1,2,3,4,5,6]。
    #[test]
    fn add_xp_crosses_multiple_levels_and_reports_each() {
        let mut entry = SkillEntry::default();
        let levels = add_xp(&mut entry, 11_500, 42);
        assert_eq!(levels, vec![1, 2, 3, 4, 5, 6]);
        assert_eq!(entry.lv, 6);
        // 5_500 + 3_600 = 9_100 花到 Lv.6；余 2_400 入 xp 桶。
        assert_eq!(entry.xp, 2_400);
        assert_eq!(entry.total_xp, 11_500);
        assert_eq!(entry.last_action_at, 42);
    }

    /// plan §2.1 Lv.9 → 10 跨过后继续灌不再涨。
    #[test]
    fn add_xp_saturates_at_max_level() {
        let mut entry = SkillEntry {
            lv: 9,
            xp: 0,
            total_xp: 0,
            last_action_at: 0,
            recent_repeat_count: 0,
        };
        // Lv.9 → 10 需要 10_000 XP。先灌刚好跨过。
        let first = add_xp(&mut entry, 10_000, 1);
        assert_eq!(first, vec![10]);
        assert_eq!(entry.lv, 10);
        assert_eq!(entry.xp, 0);

        // 再灌 50_000：不应升级、xp 桶保持 0（Lv.10 不累积）。
        let second = add_xp(&mut entry, 50_000, 2);
        assert!(
            second.is_empty(),
            "no further level-ups once Lv.{SKILL_MAX_LEVEL} reached",
        );
        assert_eq!(entry.lv, SKILL_MAX_LEVEL);
        assert_eq!(entry.xp, 0);
        // total_xp 仍然累计（统计口径）
        assert_eq!(entry.total_xp, 60_000);
    }

    /// plan §6.0 示例：`interp(4, [(3, 8.0), (5, 6.0)]) == 7.0`。
    #[test]
    fn interp_matches_plan_example() {
        let pts = [(3u8, 8.0f32), (5, 6.0)];
        assert!((interp(4, &pts) - 7.0).abs() < 1e-6);
    }

    /// 下界外：返回下界端点值（钳制），避免 Lv.0/1 超出 `[3,5]` 范围外发散。
    #[test]
    fn interp_below_first_point_clamps_to_first() {
        let pts = [(3u8, 8.0f32), (5, 6.0)];
        // plan §6.0 未明确端点外行为，选钳制端点值：Lv.1 → 8.0。
        assert!((interp(1, &pts) - 8.0).abs() < 1e-6);
    }

    /// 上界外：钳制到最后端点值。
    #[test]
    fn interp_above_last_point_clamps_to_last() {
        let pts = [(3u8, 8.0f32), (5, 6.0)];
        assert!((interp(9, &pts) - 6.0).abs() < 1e-6);
    }

    /// plan §4 cap 压制：effective_lv(7, 5) = 5。
    #[test]
    fn effective_lv_caps_real_above_cap() {
        assert_eq!(effective_lv(7, 5), 5);
        assert_eq!(effective_lv(3, 5), 3);
        assert_eq!(effective_lv(10, 10), 10);
    }
}
