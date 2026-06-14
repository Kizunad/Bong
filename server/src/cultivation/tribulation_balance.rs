//! plan-tribulation-balance-v1 P0/P1：渡劫平衡监控配置 Resource。
//!
//! `TribulationBalanceConfig` 镜像 tribulation.rs 中的真实代码常数，提供一份只读看板
//! 快照接口，供 `/balance tribulation` dev 命令展示当前平衡参数。字段命名按 plan 文档
//! 保留（锁接口），每字段 default 取真实代码常数，并有 const-引用 pin 测试防漂移。
//!
//! **无副作用**：本 resource 在 P0 阶段仅用于读取展示，不参与任何运行时机制或守恒计算。

use valence::prelude::{bevy_ecs, Resource};

use crate::cultivation::tribulation::{
    compute_void_quota_limit, DEFAULT_VOID_QUOTA_K, DUXU_AOE_DAMAGE_BASE,
    DUXU_HEART_DEMON_OBSESSION_QI_PENALTY_RATIO, DUXU_QI_DRAIN_BASE, JUEBI_INTENSITY_BASE,
};

pub const QUOTA_FILL_TARGET_MIN: f32 = 0.30;
pub const QUOTA_FILL_TARGET_MAX: f32 = 0.70;
pub const QUOTA_BALANCE_SIMULATED_OCCUPIED_SLOTS: u32 = 1;

pub fn quota_fill_ratio_for_limit(occupied_slots: u32, quota_limit: u32) -> Option<f32> {
    if quota_limit == 0 {
        return None;
    }
    Some(occupied_slots as f32 / quota_limit as f32)
}

pub fn quota_fill_ratio_for_budget(
    total_world_qi: f64,
    quota_k: f64,
    occupied_slots: u32,
) -> Option<f32> {
    quota_fill_ratio_for_limit(
        occupied_slots,
        compute_void_quota_limit(total_world_qi, quota_k),
    )
}

pub fn quota_fill_is_in_target_band(fill_ratio: f32) -> bool {
    fill_ratio.is_finite() && (QUOTA_FILL_TARGET_MIN..=QUOTA_FILL_TARGET_MAX).contains(&fill_ratio)
}

/// plan-tribulation-balance-v1 P0/P1：平衡参数快照（只读看板）。
///
/// 字段说明：
/// - `quota_k`：void quota 门槛系数（真实语义 = `DEFAULT_VOID_QUOTA_K`，公式 `floor(world_qi / quota_k)`，
///   无 player_count 维度，无硬上限；plan 文档中的 "quota_per_player/quota_max_hard_cap"
///   是对外命名映射，实际模型见 `compute_void_quota_limit`）
/// - `wave_damage_base`：渡虚劫每波 AOE 伤害基础值（`DUXU_AOE_DAMAGE_BASE`，实际波伤 = base × wave）
/// - `wave_qi_drain_base`：渡虚劫每波真元耗损基础值（`DUXU_QI_DRAIN_BASE`，实际消耗 = base × wave）
/// - `juebi_intensity_base`：绝壁灾难基础强度系数（`JUEBI_INTENSITY_BASE`）
/// - `heart_demon_qi_penalty_ratio`：心魔劫执念惩罚真元上限减损比例（`DUXU_HEART_DEMON_OBSESSION_QI_PENALTY_RATIO`）
#[derive(Debug, Clone, Copy, PartialEq, Resource)]
pub struct TribulationBalanceConfig {
    /// void quota 门槛系数（映射 `DEFAULT_VOID_QUOTA_K`）
    /// 计划文档字段：quota_per_player（语义对齐：quota_k 决定每多少 world_qi 允许一个化虚位）
    pub quota_k: f64,
    /// 渡虚劫每波 AOE 伤害基础值（`DUXU_AOE_DAMAGE_BASE`）
    /// 计划文档字段：wave_1/2/3_intensity（damage 分量）
    pub wave_damage_base: f32,
    /// 渡虚劫每波真元耗损基础值（`DUXU_QI_DRAIN_BASE`）
    /// 计划文档字段：wave_1/2/3_intensity（qi_drain 分量）
    pub wave_qi_drain_base: f64,
    /// 绝壁灾难基础强度系数（`JUEBI_INTENSITY_BASE`）
    pub juebi_intensity_base: f32,
    /// 心魔劫执念惩罚真元上限减损比例（`DUXU_HEART_DEMON_OBSESSION_QI_PENALTY_RATIO`）
    /// 计划文档字段：heart_demon_trigger_ratio（映射执念惩罚比，真实心魔为确定性触发非概率）
    pub heart_demon_qi_penalty_ratio: f64,
}

impl Default for TribulationBalanceConfig {
    fn default() -> Self {
        Self {
            quota_k: DEFAULT_VOID_QUOTA_K,
            wave_damage_base: DUXU_AOE_DAMAGE_BASE,
            wave_qi_drain_base: DUXU_QI_DRAIN_BASE,
            juebi_intensity_base: JUEBI_INTENSITY_BASE,
            heart_demon_qi_penalty_ratio: DUXU_HEART_DEMON_OBSESSION_QI_PENALTY_RATIO,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::tribulation::DEFAULT_VOID_QUOTA_TARGET_SLOTS_AT_FULL_QI;
    use crate::qi_physics::constants::DEFAULT_SPIRIT_QI_TOTAL;

    /// pin 测试：config default 必须精确对齐 tribulation.rs 中的真实常数。
    /// 若 tribulation.rs 常数被改动，本测试立刻撞红，强制同步 config 说明/文档。
    #[test]
    fn config_default_quota_k_matches_real_const() {
        let cfg = TribulationBalanceConfig::default();
        assert_eq!(
            cfg.quota_k,
            DEFAULT_VOID_QUOTA_K,
            "TribulationBalanceConfig.quota_k 必须等于 DEFAULT_VOID_QUOTA_K={DEFAULT_VOID_QUOTA_K}; \
             当前值={} (tribulation.rs 常数漂移时本测试撞红)",
            cfg.quota_k
        );
    }

    #[test]
    fn p1_default_quota_k_targets_two_slots_at_full_world_qi() {
        let limit = compute_void_quota_limit(DEFAULT_SPIRIT_QI_TOTAL, DEFAULT_VOID_QUOTA_K);
        assert_eq!(
            limit, DEFAULT_VOID_QUOTA_TARGET_SLOTS_AT_FULL_QI,
            "DEFAULT_VOID_QUOTA_K={DEFAULT_VOID_QUOTA_K} 应让满灵气预算 \
             DEFAULT_SPIRIT_QI_TOTAL={DEFAULT_SPIRIT_QI_TOTAL} 产生 \
             DEFAULT_VOID_QUOTA_TARGET_SLOTS_AT_FULL_QI={DEFAULT_VOID_QUOTA_TARGET_SLOTS_AT_FULL_QI} 个化虚名额；got {limit}"
        );
    }

    #[test]
    fn p1_default_quota_simulation_one_occupied_is_in_target_band() {
        let fill_ratio = quota_fill_ratio_for_budget(
            DEFAULT_SPIRIT_QI_TOTAL,
            DEFAULT_VOID_QUOTA_K,
            QUOTA_BALANCE_SIMULATED_OCCUPIED_SLOTS,
        )
        .expect("满灵气预算下 quota_limit 应大于 0，才能做 P1 满载率模拟");
        assert!(
            quota_fill_is_in_target_band(fill_ratio),
            "P1 默认模拟应落入目标满载率区间 [{:.0}%, {:.0}%]；\
             got {:.1}%（occupied={}, quota_k={DEFAULT_VOID_QUOTA_K}）",
            QUOTA_FILL_TARGET_MIN * 100.0,
            QUOTA_FILL_TARGET_MAX * 100.0,
            fill_ratio * 100.0,
            QUOTA_BALANCE_SIMULATED_OCCUPIED_SLOTS
        );
    }

    #[test]
    fn p1_quota_fill_ratio_returns_none_when_limit_zero() {
        assert_eq!(
            quota_fill_ratio_for_limit(QUOTA_BALANCE_SIMULATED_OCCUPIED_SLOTS, 0),
            None,
            "quota_limit=0 不能被伪装成 0% 满载；P0 看板负责按 OVER-FULL/空名额显式展示"
        );
    }

    #[test]
    fn p1_quota_fill_ratio_handles_zero_occupied_slots() {
        let fill_ratio = quota_fill_ratio_for_limit(0, DEFAULT_VOID_QUOTA_TARGET_SLOTS_AT_FULL_QI)
            .expect("quota_limit > 0 时应返回 Some(fill_ratio)，即使 occupied_slots=0");
        assert_eq!(
            fill_ratio, 0.0,
            "occupied_slots=0 应产生 0% 填充率；got {fill_ratio}"
        );
    }

    #[test]
    fn p1_quota_fill_ratio_handles_over_quota_state() {
        let fill_ratio = quota_fill_ratio_for_limit(3, 2)
            .expect("quota_limit > 0 时应返回 Some(fill_ratio)，超额占用也应显式暴露");
        assert!(
            fill_ratio > 1.0,
            "occupied_slots > quota_limit 应产生 >100% 填充率；got {fill_ratio}"
        );
        assert_eq!(
            fill_ratio, 1.5,
            "3 occupied / 2 limit 应等于 150%；got {fill_ratio}"
        );
    }

    #[test]
    fn p1_quota_fill_target_band_rejects_underfilled_and_full_states() {
        assert!(
            !quota_fill_is_in_target_band(QUOTA_FILL_TARGET_MIN / 2.0),
            "低于 QUOTA_FILL_TARGET_MIN={QUOTA_FILL_TARGET_MIN} 的名额占用必须被识别为过空"
        );
        assert!(
            !quota_fill_is_in_target_band(1.0),
            "100% 满载必须被识别为过满，不能落入 P1 30%-70% 目标区间"
        );
        assert!(
            quota_fill_is_in_target_band((QUOTA_FILL_TARGET_MIN + QUOTA_FILL_TARGET_MAX) / 2.0),
            "目标区间中点应通过，避免边界判断写反"
        );
    }

    #[test]
    fn p1_quota_fill_target_band_includes_exact_boundaries() {
        assert!(
            quota_fill_is_in_target_band(QUOTA_FILL_TARGET_MIN),
            "精确等于 QUOTA_FILL_TARGET_MIN={QUOTA_FILL_TARGET_MIN} 应被接受"
        );
        assert!(
            quota_fill_is_in_target_band(QUOTA_FILL_TARGET_MAX),
            "精确等于 QUOTA_FILL_TARGET_MAX={QUOTA_FILL_TARGET_MAX} 应被接受"
        );
    }

    #[test]
    fn p1_quota_fill_target_band_rejects_nan_and_infinity() {
        assert!(
            !quota_fill_is_in_target_band(f32::NAN),
            "NaN 必须被 is_finite() 检查拒绝"
        );
        assert!(
            !quota_fill_is_in_target_band(f32::INFINITY),
            "Infinity 必须被 is_finite() 检查拒绝"
        );
    }

    #[test]
    fn p1_quota_model_has_no_hidden_hard_cap() {
        let doubled_budget = DEFAULT_SPIRIT_QI_TOTAL * 2.0;
        let limit = compute_void_quota_limit(doubled_budget, DEFAULT_VOID_QUOTA_K);
        let expected = DEFAULT_VOID_QUOTA_TARGET_SLOTS_AT_FULL_QI * 2;
        assert_eq!(
            limit, expected,
            "真实 quota 模型是 floor(world_qi / quota_k)，不应保留 plan 草稿里的硬上限 3；\
             doubled_budget={doubled_budget} quota_k={DEFAULT_VOID_QUOTA_K} expected={expected} got={limit}"
        );
    }

    #[test]
    fn config_default_wave_damage_base_matches_real_const() {
        let cfg = TribulationBalanceConfig::default();
        assert_eq!(
            cfg.wave_damage_base,
            DUXU_AOE_DAMAGE_BASE,
            "TribulationBalanceConfig.wave_damage_base 必须等于 DUXU_AOE_DAMAGE_BASE={DUXU_AOE_DAMAGE_BASE}; \
             当前值={} (渡虚劫 damage 基础值漂移)",
            cfg.wave_damage_base
        );
    }

    #[test]
    fn config_default_wave_qi_drain_base_matches_real_const() {
        let cfg = TribulationBalanceConfig::default();
        assert_eq!(
            cfg.wave_qi_drain_base,
            DUXU_QI_DRAIN_BASE,
            "TribulationBalanceConfig.wave_qi_drain_base 必须等于 DUXU_QI_DRAIN_BASE={DUXU_QI_DRAIN_BASE}; \
             当前值={} (渡虚劫 qi_drain 基础值漂移)",
            cfg.wave_qi_drain_base
        );
    }

    #[test]
    fn config_default_juebi_intensity_base_matches_real_const() {
        let cfg = TribulationBalanceConfig::default();
        assert_eq!(
            cfg.juebi_intensity_base,
            JUEBI_INTENSITY_BASE,
            "TribulationBalanceConfig.juebi_intensity_base 必须等于 JUEBI_INTENSITY_BASE={JUEBI_INTENSITY_BASE}; \
             当前值={} (绝壁强度系数漂移)",
            cfg.juebi_intensity_base
        );
    }

    #[test]
    fn config_default_heart_demon_penalty_matches_real_const() {
        let cfg = TribulationBalanceConfig::default();
        assert_eq!(
            cfg.heart_demon_qi_penalty_ratio,
            DUXU_HEART_DEMON_OBSESSION_QI_PENALTY_RATIO,
            "TribulationBalanceConfig.heart_demon_qi_penalty_ratio 必须等于 \
             DUXU_HEART_DEMON_OBSESSION_QI_PENALTY_RATIO={DUXU_HEART_DEMON_OBSESSION_QI_PENALTY_RATIO}; \
             当前值={} (心魔惩罚比漂移)",
            cfg.heart_demon_qi_penalty_ratio
        );
    }
}
