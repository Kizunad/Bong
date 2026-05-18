//! P4 §5.3 + §8.1 #5 — 暴龙王三阶段 AI（纯 Scorer 链组合）。
//!
//! 阶段切换通过 health_ratio 区间门控，不扩展 big-brain 框架。
//! 阶段一：驱逐（HP > 70%）
//! 阶段二：暴怒（30%-70%）
//! 阶段三：崩溃（< 30% 或炉毁）

use super::boss::{BaolongwangBoss, BossPhase};

// 三阶段 AI Scorer 权重。
// §8.1 #5: 纯 Scorer 链组合，每阶段 Action 评分函数内置 health_ratio 区间门控。

// ── 阶段一：驱逐 ────────────────────────────────────────────

/// 驱逐阶段：远离玩家。
pub fn score_avoid_player(boss: &BaolongwangBoss, distance: f32) -> f32 {
    phase_gate(boss, BossPhase::Expel) * if distance < 20.0 { 0.8 } else { 0.0 }
}

/// 驱逐阶段：丹雾驱逐。
pub fn score_pill_mist_repel(boss: &BaolongwangBoss, distance: f32) -> f32 {
    phase_gate(boss, BossPhase::Expel) * if distance < 8.0 { 0.5 } else { 0.0 }
}

/// 驱逐阶段：尾击反击（近距离）。
pub fn score_tail_swipe(boss: &BaolongwangBoss, distance: f32) -> f32 {
    phase_gate(boss, BossPhase::Expel) * if distance < 3.0 { 0.9 } else { 0.0 }
}

// ── 阶段二：暴怒 ────────────────────────────────────────────

/// 暴怒阶段：近战攻击。
pub fn score_melee_attack(boss: &BaolongwangBoss, distance: f32) -> f32 {
    phase_gate(boss, BossPhase::Rage) * if distance < 5.0 { 0.7 } else { 0.0 }
}

/// 暴怒阶段：角冲撞（中距离）。
pub fn score_horn_charge(boss: &BaolongwangBoss, distance: f32) -> f32 {
    phase_gate(boss, BossPhase::Rage)
        * if (5.0..=15.0).contains(&distance) {
            0.6
        } else {
            0.0
        }
}

/// 暴怒阶段：自服丹（HP < 50% 时优先）。
pub fn score_self_pill(boss: &BaolongwangBoss) -> f32 {
    phase_gate(boss, BossPhase::Rage)
        * if boss.hp_fraction < 0.50 && boss.pill_reserve > 0 {
            0.8
        } else {
            0.0
        }
}

/// 暴怒阶段：投掷毒丹（中远距离）。
pub fn score_pill_bomb_attack(boss: &BaolongwangBoss, distance: f32) -> f32 {
    phase_gate(boss, BossPhase::Rage)
        * if (5.0..=20.0).contains(&distance) && boss.pill_reserve > 0 {
            0.5
        } else {
            0.0
        }
}

// ── 阶段三：崩溃 ────────────────────────────────────────────

/// 崩溃阶段：狂暴攻击（无距离限制，全力输出）。
pub fn score_berserk_attack(boss: &BaolongwangBoss) -> f32 {
    phase_gate(boss, BossPhase::Collapse) * 1.0
}

/// 崩溃阶段：自爆丹药储备（HP < 10%）。
pub fn score_self_detonate(boss: &BaolongwangBoss) -> f32 {
    phase_gate(boss, BossPhase::Collapse) * if boss.hp_fraction < 0.10 { 0.9 } else { 0.0 }
}

// ── 内部辅助 ────────────────────────────────────────────────

/// §8.1 #5: 阶段门控。只有当前 phase 匹配时返回 1.0，否则 0.0。
/// 确保同一时刻只有一个阶段的 Scorer 评分 > 0。
fn phase_gate(boss: &BaolongwangBoss, expected: BossPhase) -> f32 {
    let current = super::boss::determine_phase(boss.hp_fraction, boss.furnace_intact);
    if current == expected {
        1.0
    } else {
        0.0
    }
}

/// 选择最高分的 Action（简化版 Picker）。
pub fn pick_best_action<'a>(scores: &'a [(&'a str, f32)]) -> Option<&'a str> {
    scores
        .iter()
        .filter(|(_, s)| *s > 0.0)
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(name, _)| *name)
}

/// 暴龙王单实例 + 72h 重生。
pub const BOSS_RESPAWN_TICKS: u64 = 72 * 3600 * 20; // 72h × 3600s × 20tps = 5,184,000 ticks
/// 暴龙王活动范围限制（格）。
pub const BOSS_ACTIVITY_RADIUS: f32 = 200.0;
/// 炉毁后倒计时（秒 × tps）。plan §5.3 说 180s，boss.rs 用 120s=2400 ticks。
/// 以 boss.rs 代码为准（§8.1 说按代码现状适配）。
pub const BOSS_COLLAPSE_COUNTDOWN_TICKS: u64 = 2400;

#[cfg(test)]
mod boss_ai_tests {
    use super::*;
    use crate::dandao::boss::BaolongwangBoss;

    fn boss_at(hp: f32, furnace: bool, pills: u32) -> BaolongwangBoss {
        BaolongwangBoss {
            hp_fraction: hp,
            furnace_intact: furnace,
            pill_reserve: pills,
            ..BaolongwangBoss::default()
        }
    }

    // ── 阶段一：驱逐 ──────────────────────────

    #[test]
    fn expel_phase_avoid_player_near() {
        let boss = boss_at(0.9, true, 30);
        assert!(
            score_avoid_player(&boss, 10.0) > 0.0,
            "HP > 70% + 距离 < 20 应远离"
        );
    }

    #[test]
    fn expel_phase_avoid_player_far() {
        let boss = boss_at(0.9, true, 30);
        assert_eq!(score_avoid_player(&boss, 25.0), 0.0, "距离 > 20 不触发远离");
    }

    #[test]
    fn expel_phase_pill_mist() {
        let boss = boss_at(0.9, true, 30);
        assert!(
            score_pill_mist_repel(&boss, 5.0) > 0.0,
            "近距离应释放丹雾驱逐"
        );
        assert_eq!(score_pill_mist_repel(&boss, 15.0), 0.0, "远距离不释放丹雾");
    }

    #[test]
    fn expel_phase_tail_swipe_close() {
        let boss = boss_at(0.9, true, 30);
        assert!(score_tail_swipe(&boss, 2.0) > 0.0, "贴身应尾击反击");
        assert_eq!(score_tail_swipe(&boss, 5.0), 0.0, "距离 > 3 不触发尾击");
    }

    // ── 阶段二：暴怒 ──────────────────────────

    #[test]
    fn rage_phase_melee() {
        let boss = boss_at(0.5, true, 30);
        assert!(score_melee_attack(&boss, 3.0) > 0.0, "暴怒阶段近距离应近战");
        assert_eq!(score_melee_attack(&boss, 10.0), 0.0, "远距离不近战");
    }

    #[test]
    fn rage_phase_horn_charge() {
        let boss = boss_at(0.5, true, 30);
        assert!(score_horn_charge(&boss, 10.0) > 0.0, "中距离应角冲撞");
        assert_eq!(score_horn_charge(&boss, 3.0), 0.0, "太近不角冲撞");
        assert_eq!(score_horn_charge(&boss, 20.0), 0.0, "边界外不角冲撞");
    }

    #[test]
    fn rage_phase_self_pill_low_hp() {
        let boss = boss_at(0.4, true, 10);
        assert!(score_self_pill(&boss) > 0.0, "HP < 50% 且有丹药应自服");
    }

    #[test]
    fn rage_phase_self_pill_high_hp() {
        let boss = boss_at(0.6, true, 10);
        assert_eq!(score_self_pill(&boss), 0.0, "HP >= 50% 不自服");
    }

    #[test]
    fn rage_phase_self_pill_no_reserve() {
        let boss = boss_at(0.4, true, 0);
        assert_eq!(score_self_pill(&boss), 0.0, "无丹药储备不自服");
    }

    #[test]
    fn rage_phase_pill_bomb() {
        let boss = boss_at(0.5, true, 10);
        assert!(score_pill_bomb_attack(&boss, 10.0) > 0.0, "中距离投丹");
        assert_eq!(score_pill_bomb_attack(&boss, 3.0), 0.0, "太近不投丹");
    }

    // ── 阶段三：崩溃 ──────────────────────────

    #[test]
    fn collapse_phase_berserk() {
        let boss = boss_at(0.2, true, 30);
        assert!(score_berserk_attack(&boss) > 0.0, "崩溃阶段应狂暴攻击");
    }

    #[test]
    fn collapse_phase_self_detonate() {
        let boss = boss_at(0.05, true, 30);
        assert!(score_self_detonate(&boss) > 0.0, "HP < 10% 应自爆");
    }

    #[test]
    fn collapse_phase_no_detonate_above_10() {
        let boss = boss_at(0.2, true, 30);
        assert_eq!(score_self_detonate(&boss), 0.0, "HP >= 10% 不自爆");
    }

    #[test]
    fn furnace_destroyed_triggers_collapse() {
        let boss = boss_at(0.8, false, 30);
        assert!(score_berserk_attack(&boss) > 0.0, "炉毁应直接进入崩溃阶段");
        assert_eq!(score_avoid_player(&boss, 5.0), 0.0, "炉毁后不应再驱逐");
    }

    // ── 阶段隔离 ──────────────────────────────

    #[test]
    fn expel_actions_zero_during_rage() {
        let boss = boss_at(0.5, true, 30);
        assert_eq!(score_avoid_player(&boss, 5.0), 0.0);
        assert_eq!(score_pill_mist_repel(&boss, 5.0), 0.0);
        assert_eq!(score_tail_swipe(&boss, 2.0), 0.0);
    }

    #[test]
    fn rage_actions_zero_during_expel() {
        let boss = boss_at(0.9, true, 30);
        assert_eq!(score_melee_attack(&boss, 3.0), 0.0);
        assert_eq!(score_horn_charge(&boss, 10.0), 0.0);
        assert_eq!(score_self_pill(&boss), 0.0);
    }

    #[test]
    fn rage_actions_zero_during_collapse() {
        let boss = boss_at(0.2, true, 30);
        assert_eq!(score_melee_attack(&boss, 3.0), 0.0);
        assert_eq!(score_horn_charge(&boss, 10.0), 0.0);
    }

    // ── pick_best_action ──────────────────────

    #[test]
    fn pick_best_returns_highest() {
        let scores = vec![("avoid", 0.8), ("mist", 0.5), ("tail", 0.9)];
        assert_eq!(pick_best_action(&scores), Some("tail"));
    }

    #[test]
    fn pick_best_returns_none_when_all_zero() {
        let scores = vec![("avoid", 0.0), ("mist", 0.0)];
        assert_eq!(pick_best_action(&scores), None);
    }

    #[test]
    fn pick_best_empty_returns_none() {
        let scores: Vec<(&str, f32)> = vec![];
        assert_eq!(pick_best_action(&scores), None);
    }

    // ── 常量 ──────────────────────────────────

    #[test]
    fn respawn_ticks_is_72h() {
        assert_eq!(
            BOSS_RESPAWN_TICKS,
            72 * 3600 * 20,
            "72h 重生 = 5,184,000 ticks"
        );
    }

    #[test]
    fn activity_radius_is_200() {
        assert!((BOSS_ACTIVITY_RADIUS - 200.0).abs() < f32::EPSILON);
    }

    #[test]
    fn collapse_countdown_matches_boss_module() {
        assert_eq!(
            BOSS_COLLAPSE_COUNTDOWN_TICKS, 2400,
            "与 boss.rs on_furnace_destroyed 一致"
        );
    }

    // ── 阶段边界精确测试 ──────────────────────

    #[test]
    fn phase_boundary_70_percent() {
        // HP = 0.70 should be Expel (> 70% threshold is exclusive in determine_phase)
        let boss = boss_at(0.70, true, 30);
        // determine_phase: hp < 0.70 → Rage, else Expel
        // 0.70 is NOT < 0.70, so Expel
        assert!(score_avoid_player(&boss, 5.0) > 0.0, "HP=0.70 应在驱逐阶段");
        assert_eq!(
            score_melee_attack(&boss, 3.0),
            0.0,
            "HP=0.70 不应在暴怒阶段"
        );
    }

    #[test]
    fn phase_boundary_30_percent() {
        // HP = 0.30 should be Rage (>= 0.30)
        let boss = boss_at(0.30, true, 30);
        // determine_phase: hp < 0.30 → Collapse, 0.30..0.70 → Rage
        // 0.30 is NOT < 0.30, so Rage
        assert!(score_melee_attack(&boss, 3.0) > 0.0, "HP=0.30 应在暴怒阶段");
        assert_eq!(score_berserk_attack(&boss), 0.0, "HP=0.30 不应在崩溃阶段");
    }

    #[test]
    fn phase_boundary_just_below_30() {
        let boss = boss_at(0.29, true, 30);
        assert!(score_berserk_attack(&boss) > 0.0, "HP=0.29 应在崩溃阶段");
    }

    #[test]
    fn phase_boundary_just_below_70() {
        let boss = boss_at(0.69, true, 30);
        assert!(score_melee_attack(&boss, 3.0) > 0.0, "HP=0.69 应在暴怒阶段");
    }
}
