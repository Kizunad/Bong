//! plan-dandao-path-v1 P4 — 暴龙王 BOSS。
//!
//! 三阶段战斗 AI（驱逐/暴怒/崩溃）+ 炉生命线 + 丹药储备系统。
//! AI 走 big-brain Utility Scorer→Action 模式，阶段切换通过 HP% 判定。

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component};

/// 暴龙王战斗阶段。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BossPhase {
    /// HP > 70%: 不想打架，远离 + 偶尔丹雾驱逐
    Expel,
    /// HP 30%-70%: 暴怒主动攻击
    Rage,
    /// HP < 30% 或炉被摧毁: 狂暴崩溃
    Collapse,
}

/// 暴龙王 BOSS 组件 — 挂在 boss entity 上。
#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct BaolongwangBoss {
    pub phase: BossPhase,
    /// 存活年数（影响 defense_power）
    pub age_years: u32,
    /// 携带丹药储备数量（战斗中消耗）
    pub pill_reserve: u32,
    /// 变异阶段（固定 4 = 兽化）
    pub mutation_stage: u8,
    /// 炉是否完好
    pub furnace_intact: bool,
    /// 炉被摧毁后的倒计时 tick（None = 炉完好）
    pub collapse_countdown: Option<u64>,
    /// HP 百分比缓存（由外部系统写入）
    pub hp_fraction: f32,
    /// 是否曾进入 Rage 阶段（horn 掉落前置条件）
    pub has_entered_rage: bool,
}

impl Default for BaolongwangBoss {
    fn default() -> Self {
        Self {
            phase: BossPhase::Expel,
            age_years: 5000,
            pill_reserve: 30,
            mutation_stage: 4,
            furnace_intact: true,
            collapse_countdown: None,
            hp_fraction: 1.0,
            has_entered_rage: false,
        }
    }
}

/// defense_power 计算（plan §5.2）。
/// 满档 age_bracket=4 → defense_power=0.35（受伤仅 35%）。
/// 崩溃阶段退化到 0.6。
pub fn boss_defense_power(boss: &BaolongwangBoss) -> f32 {
    let age_bracket = (boss.age_years / 1000).min(4) as f32;
    let base = 0.15 + 0.05 * age_bracket;
    if boss.phase == BossPhase::Collapse {
        // 壳在碎，防御退化
        let decay = match boss.collapse_countdown {
            Some(remaining) => {
                let elapsed_fraction = 1.0 - (remaining as f32 / 2400.0);
                elapsed_fraction * 0.25
            }
            None => 0.0,
        };
        (base + decay).min(0.6)
    } else {
        base
    }
}

/// 阶段判定逻辑（纯函数）。
pub fn determine_phase(hp_fraction: f32, furnace_intact: bool) -> BossPhase {
    if !furnace_intact || hp_fraction < 0.30 {
        BossPhase::Collapse
    } else if hp_fraction < 0.70 {
        BossPhase::Rage
    } else {
        BossPhase::Expel
    }
}

/// 炉摧毁后每 tick 推进倒计时。返回 true = 寿元耗尽，BOSS 死亡。
pub fn tick_collapse_countdown(boss: &mut BaolongwangBoss) -> bool {
    if let Some(ref mut remaining) = boss.collapse_countdown {
        if *remaining == 0 {
            return true;
        }
        *remaining = remaining.saturating_sub(1);
        false
    } else {
        false
    }
}

/// 炉被摧毁时调用。幂等：重复调用不重置倒计时。
pub fn on_furnace_destroyed(boss: &mut BaolongwangBoss) {
    if !boss.furnace_intact {
        return;
    }
    boss.furnace_intact = false;
    boss.collapse_countdown = Some(2400); // 120s × 20 tps
    boss.phase = BossPhase::Collapse;
}

/// BOSS 自服丹（暴怒阶段 AI 调用）。消耗 pill_reserve，返回是否成功。
pub fn boss_self_pill(boss: &mut BaolongwangBoss) -> bool {
    if boss.pill_reserve > 0 {
        boss.pill_reserve -= 1;
        true
    } else {
        false
    }
}

/// BOSS 掉落物模板 ID。
pub const LOOT_BOSS_CORE: &str = "dandao.baolongwang_core";
pub const LOOT_ANCIENT_RECIPE: &str = "dandao.ancient_recipe_fragment";
pub const LOOT_BOSS_HORN: &str = "dandao.baolongwang_horn";
pub const LOOT_BOSS_SCALE: &str = "dandao.baolongwang_scale";
pub const LOOT_FURNACE_REMNANT: &str = "dandao.catalyst_furnace_remnant";
pub const LOOT_XU_YUAN_DAN: &str = "dandao.xu_yuan_dan";

/// 掉落物计算（deterministic seed）。
pub fn compute_loot(boss: &BaolongwangBoss, seed: u64) -> Vec<(&'static str, u32)> {
    let mut loot = Vec::new();

    // 100% drops
    loot.push((LOOT_BOSS_CORE, 1));
    loot.push((LOOT_ANCIENT_RECIPE, 3));

    // 50% horn — only if boss entered Rage phase (horn attacks were possible)
    if boss.has_entered_rage && seed % 100 < 50 {
        loot.push((LOOT_BOSS_HORN, 1));
    }

    // 80% scales ×3-8
    if seed % 100 < 80 {
        let count = 3 + ((seed / 100) % 6) as u32;
        loot.push((LOOT_BOSS_SCALE, count));
    }

    // 100% furnace remnant if furnace was destroyed
    if !boss.furnace_intact {
        loot.push((LOOT_FURNACE_REMNANT, 1));
    }

    // 70% xu_yuan_dan ×5-10
    if seed % 100 < 70 {
        let count = 5 + ((seed / 1000) % 6) as u32;
        loot.push((LOOT_XU_YUAN_DAN, count));
    }

    loot
}

#[cfg(test)]
mod boss_tests {
    use super::*;

    #[test]
    fn default_boss_is_expel_phase() {
        let boss = BaolongwangBoss::default();
        assert_eq!(boss.phase, BossPhase::Expel);
        assert_eq!(boss.pill_reserve, 30);
        assert!(boss.furnace_intact);
        assert_eq!(boss.mutation_stage, 4);
    }

    #[test]
    fn defense_power_at_5000_years_is_035() {
        let boss = BaolongwangBoss::default();
        let dp = boss_defense_power(&boss);
        assert!((dp - 0.35).abs() < f32::EPSILON, "5000 年 defense_power 应为 0.35, got {dp}");
    }

    #[test]
    fn defense_power_at_1000_years() {
        let boss = BaolongwangBoss {
            age_years: 1000,
            ..BaolongwangBoss::default()
        };
        let dp = boss_defense_power(&boss);
        assert!((dp - 0.20).abs() < f32::EPSILON, "1000 年 defense_power 应为 0.20, got {dp}");
    }

    #[test]
    fn defense_power_collapse_degrades() {
        let boss = BaolongwangBoss {
            phase: BossPhase::Collapse,
            collapse_countdown: Some(1200),
            ..BaolongwangBoss::default()
        };
        let dp = boss_defense_power(&boss);
        assert!(dp > 0.35, "崩溃阶段 defense 应退化（大于正常 0.35）: got {dp}");
        assert!(dp <= 0.6, "defense 不超过 0.6: got {dp}");
    }

    #[test]
    fn determine_phase_transitions() {
        assert_eq!(determine_phase(1.0, true), BossPhase::Expel);
        assert_eq!(determine_phase(0.71, true), BossPhase::Expel);
        assert_eq!(determine_phase(0.69, true), BossPhase::Rage);
        assert_eq!(determine_phase(0.31, true), BossPhase::Rage);
        assert_eq!(determine_phase(0.29, true), BossPhase::Collapse);
        assert_eq!(determine_phase(0.5, false), BossPhase::Collapse, "炉毁 = 直接崩溃");
    }

    #[test]
    fn on_furnace_destroyed_starts_countdown() {
        let mut boss = BaolongwangBoss::default();
        on_furnace_destroyed(&mut boss);
        assert!(!boss.furnace_intact);
        assert_eq!(boss.collapse_countdown, Some(2400));
        assert_eq!(boss.phase, BossPhase::Collapse);
    }

    #[test]
    fn tick_collapse_countdown_reaches_zero() {
        let mut boss = BaolongwangBoss::default();
        on_furnace_destroyed(&mut boss);
        for _ in 0..2400 {
            assert!(!tick_collapse_countdown(&mut boss), "倒计时未到 0 不应死");
        }
        assert!(tick_collapse_countdown(&mut boss), "倒计时到 0 应触发死亡");
    }

    #[test]
    fn boss_self_pill_depletes_reserve() {
        let mut boss = BaolongwangBoss::default();
        assert_eq!(boss.pill_reserve, 30);
        for i in 0..30 {
            assert!(boss_self_pill(&mut boss), "第 {} 颗应成功", i + 1);
        }
        assert!(!boss_self_pill(&mut boss), "储备耗尽后应失败");
        assert_eq!(boss.pill_reserve, 0);
    }

    #[test]
    fn compute_loot_always_includes_core_and_recipe() {
        let boss = BaolongwangBoss::default();
        for seed in 0..100 {
            let loot = compute_loot(&boss, seed);
            let ids: Vec<&str> = loot.iter().map(|(id, _)| *id).collect();
            assert!(ids.contains(&LOOT_BOSS_CORE), "seed={seed} 缺少 boss core");
            assert!(ids.contains(&LOOT_ANCIENT_RECIPE), "seed={seed} 缺少 ancient recipe");
        }
    }

    #[test]
    fn compute_loot_furnace_remnant_only_if_destroyed() {
        let mut boss = BaolongwangBoss::default();
        let loot_intact = compute_loot(&boss, 42);
        assert!(!loot_intact.iter().any(|(id, _)| *id == LOOT_FURNACE_REMNANT));

        on_furnace_destroyed(&mut boss);
        let loot_destroyed = compute_loot(&boss, 42);
        assert!(loot_destroyed.iter().any(|(id, _)| *id == LOOT_FURNACE_REMNANT));
    }

    #[test]
    fn compute_loot_horn_probability_roughly_50_percent() {
        let boss = BaolongwangBoss {
            has_entered_rage: true,
            ..BaolongwangBoss::default()
        };
        let count = (0..10000u64)
            .filter(|&seed| compute_loot(&boss, seed).iter().any(|(id, _)| *id == LOOT_BOSS_HORN))
            .count();
        assert!(
            (4500..5500).contains(&count),
            "角掉率应约 50%, 10000 次中掉了 {count} 次"
        );
    }

    #[test]
    fn compute_loot_horn_never_drops_without_rage() {
        let boss = BaolongwangBoss::default();
        assert!(!boss.has_entered_rage);
        let count = (0..100u64)
            .filter(|&seed| compute_loot(&boss, seed).iter().any(|(id, _)| *id == LOOT_BOSS_HORN))
            .count();
        assert_eq!(count, 0, "未进入 Rage 阶段不应掉角");
    }

    #[test]
    fn compute_loot_scales_count_range() {
        let boss = BaolongwangBoss::default();
        for seed in 0..100 {
            let loot = compute_loot(&boss, seed);
            if let Some((_, count)) = loot.iter().find(|(id, _)| *id == LOOT_BOSS_SCALE) {
                assert!(
                    (3..=8).contains(count),
                    "seed={seed} 鳞片数量应在 3-8, got {count}"
                );
            }
        }
    }

    #[test]
    fn boss_serde_roundtrip() {
        let boss = BaolongwangBoss {
            phase: BossPhase::Rage,
            pill_reserve: 15,
            ..BaolongwangBoss::default()
        };
        let json = serde_json::to_string(&boss).expect("serialize");
        let back: BaolongwangBoss = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(boss.phase, back.phase);
        assert_eq!(boss.pill_reserve, back.pill_reserve);
        assert_eq!(boss.age_years, back.age_years);
    }
}
