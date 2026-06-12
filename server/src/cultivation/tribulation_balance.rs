//! plan-tribulation-balance-v1 P0：渡劫平衡监控配置 Resource。
//!
//! `TribulationBalanceConfig` 镜像 tribulation.rs 中的真实代码常数，提供一份只读看板
//! 快照接口，供 `/balance tribulation` dev 命令展示当前平衡参数。字段命名按 plan 文档
//! 保留（锁接口），每字段 default 取真实代码常数，并有 const-引用 pin 测试防漂移。
//!
//! **无副作用**：本 resource 在 P0 阶段仅用于读取展示，不参与任何运行时机制或守恒计算。

use valence::prelude::{bevy_ecs, Resource};

use crate::cultivation::tribulation::{
    DEFAULT_VOID_QUOTA_K, DUXU_AOE_DAMAGE_BASE, DUXU_HEART_DEMON_OBSESSION_QI_PENALTY_RATIO,
    DUXU_QI_DRAIN_BASE, JUEBI_INTENSITY_BASE,
};

/// plan-tribulation-balance-v1 P0：平衡参数快照（只读看板）。
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
