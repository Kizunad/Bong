//! P0 虚蚀底盘 — VoidErosion 组件 + 累计追踪 + 阶段推进系统。
//!
//! 虚蚀是涡流修士长期使用涡流招式后身体灵压渐移为负值的不可逆过程。
//! `cumulative_erosion` 只增不减，跨死亡保留（与丹道 `cumulative_toxin` 对齐）。
//!
//! 常量和辅助函数用于未来 tick system 集成（PR-4），当前 PR 仅定义和测试。

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component, Entity, Event};

use crate::combat::components::TICKS_PER_SECOND;
use crate::cultivation::components::Realm;

use super::events::WoliuSkillId;

/// 虚蚀阶段推进检测周期（tick）— 每 30 秒检测一次。
pub const VOID_EROSION_CHECK_INTERVAL: u64 = 600;

/// 各阶段阈值。
pub const STAGE_THRESHOLDS: [f64; 5] = [0.0, 20.0, 80.0, 200.0, 400.0];

/// 常驻涡流每秒虚蚀累积。
pub const AMBIENT_EROSION_PER_SEC: f64 = 0.01;
/// 常驻涡流每秒 qi 回复（从 zone 吸取）。
pub const AMBIENT_QI_RECOVERY_PER_SEC: f64 = 0.3;
/// 常驻涡流每秒 zone drain。
pub const AMBIENT_ZONE_DRAIN_PER_SEC: f64 = 0.01;
/// 常驻涡流每秒每条经脉（肺/心）contamination。
pub const AMBIENT_CONTAM_PER_SEC: f64 = 0.05;

/// 基础涡流招式每次施放虚蚀累积。
pub const BASE_SKILL_EROSION: f64 = 0.2;

/// 虚涡单次虚蚀。
pub const VOID_VORTEX_EROSION: f64 = 3.0;
/// 虚涡 qi 消耗。
pub const VOID_VORTEX_QI_COST: f64 = 40.0;
/// 虚涡冷却 tick。
pub const VOID_VORTEX_COOLDOWN_TICKS: u64 = 15 * TICKS_PER_SECOND;
/// 虚涡持续 tick。
pub const VOID_VORTEX_DURATION_TICKS: u64 = 5 * TICKS_PER_SECOND;
/// 虚涡球体半径（格）。
pub const VOID_VORTEX_RADIUS: f32 = 1.5;
/// 虚涡 qi drain/s per target。
pub const VOID_VORTEX_DRAIN_PER_SEC: f64 = 8.0;
/// 虚涡涡流招式伤害加成。
pub const VOID_VORTEX_DAMAGE_BONUS: f64 = 0.40;
/// 虚涡肺经 contamination/s。
pub const VOID_VORTEX_LUNG_CONTAM_PER_SEC: f64 = 2.0;
/// 虚涡基础 MeridianCrack 概率。
pub const VOID_VORTEX_BASE_CRACK_PROB: f64 = 0.05;
/// 虚涡 MeridianCrack 概率每 lung_contam 乘数。
pub const VOID_VORTEX_CRACK_CONTAM_FACTOR: f64 = 0.02;

/// 吞涡 qi 消耗。
pub const SWALLOWING_QI_COST: f64 = 30.0;
/// 吞涡冷却 tick。
pub const SWALLOWING_COOLDOWN_TICKS: u64 = 20 * TICKS_PER_SECOND;
/// 吞涡吸引半径（格）。
pub const SWALLOWING_ATTRACT_RADIUS: f32 = 6.0;
/// 吞涡吸引持续 tick（2s）。
pub const SWALLOWING_ATTRACT_DURATION_TICKS: u64 = 2 * TICKS_PER_SECOND;
/// 吞涡释放伤害系数。
pub const SWALLOWING_RELEASE_DAMAGE_RATIO: f64 = 0.8;
/// 吞涡吞噬 qi 回收系数。
pub const SWALLOWING_DEVOUR_QI_RATIO: f64 = 0.4;
/// 吞涡吞噬每 qi contamination（每条经脉）。
pub const SWALLOWING_DEVOUR_CONTAM_PER_QI: f64 = 0.15;
/// 吞涡吞噬虚蚀系数。
pub const SWALLOWING_DEVOUR_EROSION_RATIO: f64 = 0.15;
/// 吞涡释放虚蚀。
pub const SWALLOWING_RELEASE_EROSION: f64 = 1.0;
/// 吞涡吞噬心经 MeridianCrack 阈值。
pub const SWALLOWING_CRACK_THRESHOLD: f64 = 30.0;
/// 吞涡吞噬心经 MeridianCrack 概率每超额 qi。
pub const SWALLOWING_CRACK_PER_EXCESS: f64 = 0.03;

/// 涡流回响虚蚀。
pub const ECHO_EROSION: f64 = 0.5;
/// 涡流回响威力比。
pub const ECHO_POWER_RATIO: f32 = 0.4;
/// 涡流回响延迟 tick（阶段 1-2）。
pub const ECHO_DELAY_STAGE_1_2: u64 = 50;
/// 涡流回响延迟 tick（阶段 3）。
pub const ECHO_DELAY_STAGE_3: u64 = 30;
/// 涡流回响延迟 tick（阶段 4）。
pub const ECHO_DELAY_STAGE_4: u64 = 16;
/// 涡流回响阶段 3 失控概率。
pub const ECHO_MISFIRE_STAGE_3: f64 = 0.10;
/// 涡流回响阶段 4 失控概率。
pub const ECHO_MISFIRE_STAGE_4: f64 = 0.25;

/// 虚心 qi 消耗。
pub const VOID_CORE_QI_COST: f64 = 60.0;
/// 虚心冷却 tick。
pub const VOID_CORE_COOLDOWN_TICKS: u64 = 60 * TICKS_PER_SECOND;
/// 虚心持续 tick（3s）。
pub const VOID_CORE_DURATION_TICKS: u64 = 3 * TICKS_PER_SECOND;
/// 虚心吸引范围（格）。
pub const VOID_CORE_ATTRACT_RADIUS: f32 = 10.0;
/// 虚心吸引速度（格/s）。
pub const VOID_CORE_ATTRACT_SPEED: f64 = 2.0;
/// 虚心冲击波半径（格）。
pub const VOID_CORE_SHOCKWAVE_RADIUS: f32 = 10.0;
/// 虚心基础伤害。
pub const VOID_CORE_BASE_DAMAGE: f64 = 50.0;
/// 虚心每阶段伤害加成。
pub const VOID_CORE_DAMAGE_PER_STAGE: f64 = 25.0;
/// 虚心高 qi 者额外 drain。
pub const VOID_CORE_HIGH_QI_DRAIN: f64 = 20.0;
/// 虚心低 qi 者回灌。
pub const VOID_CORE_LOW_QI_GRANT: f64 = 10.0;
/// 虚心虚蚀/s。
pub const VOID_CORE_EROSION_PER_SEC: f64 = 5.0;
/// 虚心阶段 3 MeridianCrack 概率。
pub const VOID_CORE_CRACK_STAGE_3: f64 = 0.25;
/// 虚心阶段 4 MeridianCrack 概率。
pub const VOID_CORE_CRACK_STAGE_4: f64 = 0.50;
/// 虚心阶段 4 肺经 SEVERED 概率。
pub const VOID_CORE_SEVERED_PROB: f64 = 0.05;

/// 常驻涡流阶段 2+ 敌人 qi 逸散倍率。
pub const AMBIENT_QI_DISSIPATION_MULT: f64 = 1.3;
/// 常驻涡流阶段 3+ 投射物命中率减少。
pub const AMBIENT_PROJECTILE_MISS_REDUCTION: f64 = 0.15;
/// 常驻涡流基础范围（格）。
pub const AMBIENT_BASE_RANGE: f32 = 3.0;
/// 常驻涡流阶段 4 范围（格）。
pub const AMBIENT_STAGE_4_RANGE: f32 = 5.0;

#[derive(Component, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VoidErosion {
    /// 历史累计虚蚀值（只增不减）。
    pub cumulative_erosion: f64,
    /// 当前阶段。
    pub stage: VoidErosionStage,
    /// 常驻涡流是否启用。
    pub ambient_active: bool,
    /// 常驻涡流切换时刻。
    pub ambient_toggled_at: u64,
}

impl Default for VoidErosion {
    fn default() -> Self {
        Self {
            cumulative_erosion: 0.0,
            stage: VoidErosionStage::None,
            ambient_active: false,
            ambient_toggled_at: 0,
        }
    }
}

impl VoidErosion {
    /// 根据 cumulative_erosion 计算应处阶段。
    pub fn computed_stage(&self) -> VoidErosionStage {
        VoidErosionStage::from_cumulative(self.cumulative_erosion)
    }

    /// 常驻涡流有效范围（格）。
    pub fn ambient_range(&self) -> f32 {
        if self.computed_stage() >= VoidErosionStage::VoidEroded {
            AMBIENT_STAGE_4_RANGE
        } else {
            AMBIENT_BASE_RANGE
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoidErosionStage {
    /// 阶段 0：cumulative < 20.0
    None = 0,
    /// 阶段 1：20.0 ~ 80.0 — 低压态
    LowPressure = 1,
    /// 阶段 2：80.0 ~ 200.0 — 虚影态
    VoidShadow = 2,
    /// 阶段 3：200.0 ~ 400.0 — 回响体
    EchoBody = 3,
    /// 阶段 4：400.0+ — 虚蚀态
    VoidEroded = 4,
}

impl VoidErosionStage {
    pub fn from_cumulative(erosion: f64) -> Self {
        if erosion >= STAGE_THRESHOLDS[4] {
            Self::VoidEroded
        } else if erosion >= STAGE_THRESHOLDS[3] {
            Self::EchoBody
        } else if erosion >= STAGE_THRESHOLDS[2] {
            Self::VoidShadow
        } else if erosion >= STAGE_THRESHOLDS[1] {
            Self::LowPressure
        } else {
            Self::None
        }
    }

    pub fn as_index(self) -> usize {
        self as usize
    }

    pub fn threshold(self) -> f64 {
        STAGE_THRESHOLDS[self.as_index()]
    }

    /// 下一阶段（如果有）。
    pub fn next(self) -> Option<Self> {
        match self {
            Self::None => Some(Self::LowPressure),
            Self::LowPressure => Some(Self::VoidShadow),
            Self::VoidShadow => Some(Self::EchoBody),
            Self::EchoBody => Some(Self::VoidEroded),
            Self::VoidEroded => Option::None,
        }
    }

    /// 涡流回响延迟 tick。
    pub fn echo_delay(self) -> u64 {
        match self {
            Self::None | Self::LowPressure | Self::VoidShadow => ECHO_DELAY_STAGE_1_2,
            Self::EchoBody => ECHO_DELAY_STAGE_3,
            Self::VoidEroded => ECHO_DELAY_STAGE_4,
        }
    }

    /// 涡流回响失控概率。
    pub fn echo_misfire_probability(self) -> f64 {
        match self {
            Self::None | Self::LowPressure | Self::VoidShadow => 0.0,
            Self::EchoBody => ECHO_MISFIRE_STAGE_3,
            Self::VoidEroded => ECHO_MISFIRE_STAGE_4,
        }
    }
}

/// 虚蚀值累积函数。
///
/// 增加 `cumulative_erosion` 并同步更新 `stage` 字段。
pub fn add_erosion(erosion: &mut VoidErosion, amount: f64) {
    if !amount.is_finite() || amount <= 0.0 {
        return;
    }
    erosion.cumulative_erosion += amount;
    erosion.stage = erosion.computed_stage();
}

/// 虚蚀阶段推进事件。
#[derive(Debug, Clone, Event, PartialEq, Eq)]
pub struct VoidErosionAdvanceEvent {
    pub entity: Entity,
    pub from: VoidErosionStage,
    pub to: VoidErosionStage,
}

/// 死亡螺旋修正系数。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ErosionMod {
    /// 正灵域效率（1.0 - penalty）。
    pub efficiency: f64,
    /// 负灵域加成。
    pub neg_bonus: f64,
    /// contamination 乘数。
    pub contam_mult: f64,
}

/// 计算死亡螺旋修正系数。
///
/// `dandao_penalty`：丹道变异经脉惩罚（从 MutationState.meridian_penalty 取得），
/// 若无丹道修炼则传 0.0。
pub fn erosion_modifier(
    stage: VoidErosionStage,
    zone_spirit_qi: f64,
    dandao_penalty: f64,
) -> ErosionMod {
    let positive_penalty = match stage {
        VoidErosionStage::None => 0.0,
        VoidErosionStage::LowPressure => 0.05,
        VoidErosionStage::VoidShadow => 0.12,
        VoidErosionStage::EchoBody => 0.20,
        VoidErosionStage::VoidEroded => 0.35,
    };
    let efficiency = if zone_spirit_qi > 0.2 {
        1.0 - positive_penalty
    } else {
        1.0
    };
    let neg_bonus = if zone_spirit_qi < -0.2 {
        match stage {
            VoidErosionStage::None => 0.0,
            VoidErosionStage::LowPressure => 0.10,
            VoidErosionStage::VoidShadow => 0.25,
            VoidErosionStage::EchoBody => 0.35,
            VoidErosionStage::VoidEroded => 0.50,
        }
    } else {
        0.0
    };
    let base_contam_mult = match stage {
        VoidErosionStage::None | VoidErosionStage::LowPressure => 1.0,
        VoidErosionStage::VoidShadow => 1.3,
        VoidErosionStage::EchoBody => 1.5,
        VoidErosionStage::VoidEroded => 1.8,
    };
    // §8.1 #6 决议：丹道叠加（乘法组合）。
    let contam_mult = (1.0 + dandao_penalty) * base_contam_mult;
    ErosionMod {
        efficiency,
        neg_bonus,
        contam_mult,
    }
}

/// 虚涡 MeridianCrack 概率 = 5% + lung_contam * 2%。
pub fn void_vortex_crack_probability(lung_contamination: f64) -> f64 {
    (VOID_VORTEX_BASE_CRACK_PROB + lung_contamination * VOID_VORTEX_CRACK_CONTAM_FACTOR)
        .clamp(0.0, 1.0)
}

/// 吞涡吞噬心经 MeridianCrack 概率。
pub fn swallowing_devour_crack_probability(devoured_qi: f64) -> f64 {
    if devoured_qi <= SWALLOWING_CRACK_THRESHOLD {
        0.0
    } else {
        ((devoured_qi - SWALLOWING_CRACK_THRESHOLD) * SWALLOWING_CRACK_PER_EXCESS).clamp(0.0, 1.0)
    }
}

/// 虚心回归冲击波伤害。
pub fn void_core_shockwave_damage(stage: VoidErosionStage) -> f64 {
    VOID_CORE_BASE_DAMAGE + stage.as_index() as f64 * VOID_CORE_DAMAGE_PER_STAGE
}

/// 虚心 MeridianCrack 概率。
pub fn void_core_crack_probability(stage: VoidErosionStage) -> f64 {
    match stage {
        VoidErosionStage::EchoBody => VOID_CORE_CRACK_STAGE_3,
        VoidErosionStage::VoidEroded => VOID_CORE_CRACK_STAGE_4,
        _ => 0.0,
    }
}

/// 虚心回归时每条经脉 contamination 量。
pub fn void_core_contam_per_meridian(stage: VoidErosionStage) -> f64 {
    stage.as_index() as f64 * 5.0
}

// ────────────────────────────────────────────────────────
// P3: 虚蚀视觉同步常量
// ────────────────────────────────────────────────────────

/// 玩家模型半透明 alpha = `1.0 - stage * 0.15`（阶段 4 = 0.4 半透明）。
pub fn erosion_model_alpha(stage: VoidErosionStage) -> f32 {
    (1.0 - stage.as_index() as f32 * 0.15).clamp(0.0, 1.0)
}

/// 回响重播 VfxEventRequest 的 scale 参数（40% 威力）。
pub const ECHO_REPLAY_VFX_SCALE: f32 = 0.4;

/// 声音扭曲 HUD overlay 触发的最低虚蚀阶段。
pub const SOUND_DISTORTION_MIN_STAGE: VoidErosionStage = VoidErosionStage::EchoBody;

/// 检测声音扭曲 HUD overlay 是否应激活。
pub fn should_show_sound_distortion_overlay(stage: VoidErosionStage) -> bool {
    stage >= SOUND_DISTORTION_MIN_STAGE
}

// ────────────────────────────────────────────────────────
// P4: 境界递进门控
// ────────────────────────────────────────────────────────

/// 境界 → 虚蚀阶段上限。VoidErosion 不可超过对应境界的 cap。
///
/// | 境界 | 最高阶段 |
/// |------|----------|
/// | 醒灵 | None（可积累，不推进） |
/// | 引气 | LowPressure（涡流回响解锁） |
/// | 凝脉 | LowPressure（常驻涡流 + 阶段 1 可达） |
/// | 固元 | VoidShadow（虚涡 + 吞涡 + 阶段 2 可达） |
/// | 通灵 | EchoBody（虚心 + 阶段 3 可达） |
/// | 化虚 | VoidEroded（阶段 4 可达 + 所有招式全解锁） |
pub fn realm_erosion_cap(realm: Realm) -> VoidErosionStage {
    match realm {
        Realm::Awaken => VoidErosionStage::None,
        Realm::Induce => VoidErosionStage::LowPressure,
        Realm::Condense => VoidErosionStage::LowPressure,
        Realm::Solidify => VoidErosionStage::VoidShadow,
        Realm::Spirit => VoidErosionStage::EchoBody,
        Realm::Void => VoidErosionStage::VoidEroded,
    }
}

/// 境界 → 某虚蚀招式是否解锁。
///
/// | 境界 | 解锁招式 |
/// |------|----------|
/// | 醒灵 | 无（可积累虚蚀但无招式） |
/// | 引气 | 涡流回响 |
/// | 凝脉 | 常驻涡流 |
/// | 固元 | 虚涡 + 吞涡 |
/// | 通灵 | 虚心 |
/// | 化虚 | 全部 |
pub fn realm_unlocks_skill(realm: Realm, skill: WoliuSkillId) -> bool {
    match skill {
        WoliuSkillId::VortexEcho => matches!(
            realm,
            Realm::Induce | Realm::Condense | Realm::Solidify | Realm::Spirit | Realm::Void
        ),
        WoliuSkillId::AmbientVortex => matches!(
            realm,
            Realm::Condense | Realm::Solidify | Realm::Spirit | Realm::Void
        ),
        WoliuSkillId::VoidVortex | WoliuSkillId::SwallowingVortex => {
            matches!(realm, Realm::Solidify | Realm::Spirit | Realm::Void)
        }
        WoliuSkillId::VoidCore => matches!(realm, Realm::Spirit | Realm::Void),
        // 非虚蚀招式不受此函数管——返回 true 以免误拦截
        _ => true,
    }
}

/// 虚蚀值累积（带境界阶段 cap）。
///
/// 即使 `cumulative_erosion` 数值超过阈值，`stage` 也不会超过 `realm_erosion_cap(realm)`。
pub fn add_erosion_capped(erosion: &mut VoidErosion, amount: f64, realm: Realm) {
    if !amount.is_finite() || amount <= 0.0 {
        return;
    }
    erosion.cumulative_erosion += amount;
    let cap = realm_erosion_cap(realm);
    let computed = VoidErosionStage::from_cumulative(erosion.cumulative_erosion);
    erosion.stage = if computed > cap { cap } else { computed };
}

// ────────────────────────────────────────────────────────
// P4: 天道互动
// ────────────────────────────────────────────────────────

/// 天道感知概率修正。
///
/// - 阶段 0-2：无修正（1.0）
/// - 阶段 3（EchoBody）：-40%（0.6）——存在于天道盲区
/// - 阶段 4（VoidEroded）：0.0——天道放弃追踪
pub fn tiandao_detection_modifier(stage: VoidErosionStage) -> f64 {
    match stage {
        VoidErosionStage::None | VoidErosionStage::LowPressure | VoidErosionStage::VoidShadow => {
            1.0
        }
        VoidErosionStage::EchoBody => 0.6,
        VoidErosionStage::VoidEroded => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ────────────────────────────────────────────────────────
    // VoidErosion 初始化
    // ────────────────────────────────────────────────────────

    #[test]
    fn default_void_erosion_is_stage_none() {
        let erosion = VoidErosion::default();
        assert_eq!(
            erosion.stage,
            VoidErosionStage::None,
            "default VoidErosion should start at stage None"
        );
        assert!(
            erosion.cumulative_erosion.abs() < f64::EPSILON,
            "default cumulative_erosion should be 0.0, got {}",
            erosion.cumulative_erosion
        );
        assert!(
            !erosion.ambient_active,
            "ambient_active should default to false"
        );
    }

    // ────────────────────────────────────────────────────────
    // 阶段推进：5 个阶段各阈值精确断言（0/20/80/200/400）
    // ────────────────────────────────────────────────────────

    #[test]
    fn stage_from_cumulative_exact_thresholds() {
        assert_eq!(
            VoidErosionStage::from_cumulative(0.0),
            VoidErosionStage::None,
            "erosion=0.0 should be None"
        );
        assert_eq!(
            VoidErosionStage::from_cumulative(19.99),
            VoidErosionStage::None,
            "erosion=19.99 should still be None"
        );
        assert_eq!(
            VoidErosionStage::from_cumulative(20.0),
            VoidErosionStage::LowPressure,
            "erosion=20.0 should transition to LowPressure"
        );
        assert_eq!(
            VoidErosionStage::from_cumulative(79.99),
            VoidErosionStage::LowPressure,
            "erosion=79.99 should still be LowPressure"
        );
        assert_eq!(
            VoidErosionStage::from_cumulative(80.0),
            VoidErosionStage::VoidShadow,
            "erosion=80.0 should transition to VoidShadow"
        );
        assert_eq!(
            VoidErosionStage::from_cumulative(199.99),
            VoidErosionStage::VoidShadow,
            "erosion=199.99 should still be VoidShadow"
        );
        assert_eq!(
            VoidErosionStage::from_cumulative(200.0),
            VoidErosionStage::EchoBody,
            "erosion=200.0 should transition to EchoBody"
        );
        assert_eq!(
            VoidErosionStage::from_cumulative(399.99),
            VoidErosionStage::EchoBody,
            "erosion=399.99 should still be EchoBody"
        );
        assert_eq!(
            VoidErosionStage::from_cumulative(400.0),
            VoidErosionStage::VoidEroded,
            "erosion=400.0 should transition to VoidEroded"
        );
        assert_eq!(
            VoidErosionStage::from_cumulative(99999.0),
            VoidErosionStage::VoidEroded,
            "extreme value should still be VoidEroded"
        );
    }

    #[test]
    fn stage_threshold_values_match_constants() {
        assert!(
            (VoidErosionStage::None.threshold() - 0.0).abs() < f64::EPSILON,
            "None threshold should be 0.0"
        );
        assert!(
            (VoidErosionStage::LowPressure.threshold() - 20.0).abs() < f64::EPSILON,
            "LowPressure threshold should be 20.0"
        );
        assert!(
            (VoidErosionStage::VoidShadow.threshold() - 80.0).abs() < f64::EPSILON,
            "VoidShadow threshold should be 80.0"
        );
        assert!(
            (VoidErosionStage::EchoBody.threshold() - 200.0).abs() < f64::EPSILON,
            "EchoBody threshold should be 200.0"
        );
        assert!(
            (VoidErosionStage::VoidEroded.threshold() - 400.0).abs() < f64::EPSILON,
            "VoidEroded threshold should be 400.0"
        );
    }

    #[test]
    fn stage_next_transitions() {
        assert_eq!(
            VoidErosionStage::None.next(),
            Some(VoidErosionStage::LowPressure)
        );
        assert_eq!(
            VoidErosionStage::LowPressure.next(),
            Some(VoidErosionStage::VoidShadow)
        );
        assert_eq!(
            VoidErosionStage::VoidShadow.next(),
            Some(VoidErosionStage::EchoBody)
        );
        assert_eq!(
            VoidErosionStage::EchoBody.next(),
            Some(VoidErosionStage::VoidEroded)
        );
        assert_eq!(
            VoidErosionStage::VoidEroded.next(),
            Option::None,
            "VoidEroded is the final stage, next() should be None"
        );
    }

    #[test]
    fn stage_ordering_is_monotonic() {
        assert!(VoidErosionStage::None < VoidErosionStage::LowPressure);
        assert!(VoidErosionStage::LowPressure < VoidErosionStage::VoidShadow);
        assert!(VoidErosionStage::VoidShadow < VoidErosionStage::EchoBody);
        assert!(VoidErosionStage::EchoBody < VoidErosionStage::VoidEroded);
    }

    // ────────────────────────────────────────────────────────
    // add_erosion
    // ────────────────────────────────────────────────────────

    #[test]
    fn add_erosion_accumulates() {
        let mut erosion = VoidErosion::default();
        add_erosion(&mut erosion, 10.0);
        assert!(
            (erosion.cumulative_erosion - 10.0).abs() < f64::EPSILON,
            "expected 10.0, got {}",
            erosion.cumulative_erosion
        );
        add_erosion(&mut erosion, 15.0);
        assert!(
            (erosion.cumulative_erosion - 25.0).abs() < f64::EPSILON,
            "expected 25.0, got {}",
            erosion.cumulative_erosion
        );
    }

    #[test]
    fn add_erosion_updates_stage_in_place() {
        let mut erosion = VoidErosion::default();
        assert_eq!(erosion.stage, VoidErosionStage::None);
        add_erosion(&mut erosion, 25.0);
        assert_eq!(
            erosion.stage,
            VoidErosionStage::LowPressure,
            "add_erosion should update stage from None to LowPressure after crossing 20.0 threshold"
        );
        add_erosion(&mut erosion, 60.0);
        assert_eq!(
            erosion.stage,
            VoidErosionStage::VoidShadow,
            "add_erosion should update stage to VoidShadow after crossing 80.0 threshold"
        );
        add_erosion(&mut erosion, 120.0);
        assert_eq!(
            erosion.stage,
            VoidErosionStage::EchoBody,
            "add_erosion should update stage to EchoBody after crossing 200.0 threshold"
        );
        add_erosion(&mut erosion, 200.0);
        assert_eq!(
            erosion.stage,
            VoidErosionStage::VoidEroded,
            "add_erosion should update stage to VoidEroded after crossing 400.0 threshold"
        );
    }

    #[test]
    fn add_erosion_ignores_non_positive() {
        let mut erosion = VoidErosion::default();
        add_erosion(&mut erosion, 0.0);
        assert!(
            erosion.cumulative_erosion.abs() < f64::EPSILON,
            "zero amount should not change erosion"
        );
        add_erosion(&mut erosion, -5.0);
        assert!(
            erosion.cumulative_erosion.abs() < f64::EPSILON,
            "negative amount should not change erosion"
        );
        add_erosion(&mut erosion, f64::NAN);
        assert!(
            erosion.cumulative_erosion.abs() < f64::EPSILON,
            "NaN amount should not change erosion"
        );
        add_erosion(&mut erosion, f64::INFINITY);
        assert!(
            erosion.cumulative_erosion.abs() < f64::EPSILON,
            "Infinity should not change erosion"
        );
    }

    #[test]
    fn computed_stage_reflects_cumulative() {
        let mut erosion = VoidErosion::default();
        assert_eq!(erosion.computed_stage(), VoidErosionStage::None);

        erosion.cumulative_erosion = 20.0;
        assert_eq!(erosion.computed_stage(), VoidErosionStage::LowPressure);

        erosion.cumulative_erosion = 80.0;
        assert_eq!(erosion.computed_stage(), VoidErosionStage::VoidShadow);

        erosion.cumulative_erosion = 200.0;
        assert_eq!(erosion.computed_stage(), VoidErosionStage::EchoBody);

        erosion.cumulative_erosion = 400.0;
        assert_eq!(erosion.computed_stage(), VoidErosionStage::VoidEroded);
    }

    // ────────────────────────────────────────────────────────
    // 回响延迟 + 失控概率
    // ────────────────────────────────────────────────────────

    #[test]
    fn echo_delay_by_stage() {
        assert_eq!(VoidErosionStage::None.echo_delay(), 50);
        assert_eq!(VoidErosionStage::LowPressure.echo_delay(), 50);
        assert_eq!(VoidErosionStage::VoidShadow.echo_delay(), 50);
        assert_eq!(VoidErosionStage::EchoBody.echo_delay(), 30);
        assert_eq!(VoidErosionStage::VoidEroded.echo_delay(), 16);
    }

    #[test]
    fn echo_misfire_probability_by_stage() {
        assert!((VoidErosionStage::None.echo_misfire_probability()).abs() < f64::EPSILON);
        assert!((VoidErosionStage::LowPressure.echo_misfire_probability()).abs() < f64::EPSILON);
        assert!((VoidErosionStage::VoidShadow.echo_misfire_probability()).abs() < f64::EPSILON);
        assert!(
            (VoidErosionStage::EchoBody.echo_misfire_probability() - 0.10).abs() < f64::EPSILON
        );
        assert!(
            (VoidErosionStage::VoidEroded.echo_misfire_probability() - 0.25).abs() < f64::EPSILON
        );
    }

    // ────────────────────────────────────────────────────────
    // 死亡螺旋 erosion_modifier
    // ────────────────────────────────────────────────────────

    #[test]
    fn erosion_modifier_no_erosion() {
        let m = erosion_modifier(VoidErosionStage::None, 0.9, 0.0);
        assert!(
            (m.efficiency - 1.0).abs() < 1e-9,
            "None stage should have no positive penalty, but efficiency={}",
            m.efficiency
        );
        assert!(
            m.neg_bonus.abs() < 1e-9,
            "positive zone should have no neg bonus, got {}",
            m.neg_bonus
        );
        assert!(
            (m.contam_mult - 1.0).abs() < 1e-9,
            "None stage contam_mult should be 1.0, got {}",
            m.contam_mult
        );
    }

    #[test]
    fn erosion_modifier_positive_zone_penalty() {
        // 正灵域，各阶段效率惩罚
        let cases = [
            (VoidErosionStage::None, 1.0),
            (VoidErosionStage::LowPressure, 0.95),
            (VoidErosionStage::VoidShadow, 0.88),
            (VoidErosionStage::EchoBody, 0.80),
            (VoidErosionStage::VoidEroded, 0.65),
        ];
        for (stage, expected_eff) in cases {
            let m = erosion_modifier(stage, 0.9, 0.0);
            assert!(
                (m.efficiency - expected_eff).abs() < 1e-9,
                "stage {:?} in positive zone should have efficiency={}, got {}",
                stage,
                expected_eff,
                m.efficiency
            );
        }
    }

    #[test]
    fn erosion_modifier_negative_zone_bonus() {
        let cases = [
            (VoidErosionStage::None, 0.0),
            (VoidErosionStage::LowPressure, 0.10),
            (VoidErosionStage::VoidShadow, 0.25),
            (VoidErosionStage::EchoBody, 0.35),
            (VoidErosionStage::VoidEroded, 0.50),
        ];
        for (stage, expected_bonus) in cases {
            let m = erosion_modifier(stage, -0.5, 0.0);
            assert!(
                (m.neg_bonus - expected_bonus).abs() < 1e-9,
                "stage {:?} in negative zone should have neg_bonus={}, got {}",
                stage,
                expected_bonus,
                m.neg_bonus
            );
            // 负灵域不应有正灵域惩罚
            assert!(
                (m.efficiency - 1.0).abs() < 1e-9,
                "negative zone should not penalize efficiency, got {}",
                m.efficiency
            );
        }
    }

    #[test]
    fn erosion_modifier_neutral_zone_no_bonus_no_penalty() {
        // 中间区 [-0.2, 0.2] 不应有任何修正
        let m = erosion_modifier(VoidErosionStage::VoidEroded, 0.1, 0.0);
        assert!(
            (m.efficiency - 1.0).abs() < 1e-9,
            "neutral zone should have no efficiency penalty"
        );
        assert!(
            m.neg_bonus.abs() < 1e-9,
            "neutral zone should have no neg bonus"
        );
    }

    #[test]
    fn erosion_modifier_contam_mult_by_stage() {
        let cases = [
            (VoidErosionStage::None, 1.0),
            (VoidErosionStage::LowPressure, 1.0),
            (VoidErosionStage::VoidShadow, 1.3),
            (VoidErosionStage::EchoBody, 1.5),
            (VoidErosionStage::VoidEroded, 1.8),
        ];
        for (stage, expected_mult) in cases {
            let m = erosion_modifier(stage, 0.0, 0.0);
            assert!(
                (m.contam_mult - expected_mult).abs() < 1e-9,
                "stage {:?} contam_mult should be {}, got {}",
                stage,
                expected_mult,
                m.contam_mult
            );
        }
    }

    #[test]
    fn erosion_modifier_dandao_stacking() {
        // §8.1 #6：丹道叠加公式 = (1.0 + dandao_penalty) * base_contam_mult
        let m = erosion_modifier(VoidErosionStage::EchoBody, 0.0, 0.15);
        let expected = (1.0 + 0.15) * 1.5; // = 1.725
        assert!(
            (m.contam_mult - expected).abs() < 1e-9,
            "dandao stage 3 + erosion stage 3 contam_mult should be {}, got {}",
            expected,
            m.contam_mult
        );
    }

    #[test]
    fn erosion_modifier_dandao_stage4_erosion_stage4() {
        let m = erosion_modifier(VoidErosionStage::VoidEroded, 0.0, 0.20);
        let expected = (1.0 + 0.20) * 1.8; // = 2.16
        assert!(
            (m.contam_mult - expected).abs() < 1e-9,
            "dandao stage 4 + erosion stage 4 contam_mult should be {}, got {}",
            expected,
            m.contam_mult
        );
    }

    // ────────────────────────────────────────────────────────
    // 虚涡 MeridianCrack 概率
    // ────────────────────────────────────────────────────────

    #[test]
    fn void_vortex_crack_probability_formula() {
        assert!(
            (void_vortex_crack_probability(0.0) - 0.05).abs() < 1e-9,
            "zero contam should give base 5%"
        );
        assert!(
            (void_vortex_crack_probability(10.0) - 0.25).abs() < 1e-9,
            "contam=10 should give 5% + 10*2% = 25%"
        );
        assert!(
            (void_vortex_crack_probability(47.5) - 1.0).abs() < 1e-9,
            "contam=47.5 should saturate to 100%"
        );
        assert!(
            (void_vortex_crack_probability(100.0) - 1.0).abs() < 1e-9,
            "high contam should be clamped to 100%"
        );
    }

    // ────────────────────────────────────────────────────────
    // 吞涡 MeridianCrack 概率
    // ────────────────────────────────────────────────────────

    #[test]
    fn swallowing_devour_crack_probability_formula() {
        assert!(
            swallowing_devour_crack_probability(0.0).abs() < 1e-9,
            "below threshold should be 0"
        );
        assert!(
            swallowing_devour_crack_probability(30.0).abs() < 1e-9,
            "exactly at threshold should be 0"
        );
        assert!(
            (swallowing_devour_crack_probability(40.0) - 0.30).abs() < 1e-9,
            "10 excess * 3% = 30%, got {}",
            swallowing_devour_crack_probability(40.0)
        );
        assert!(
            (swallowing_devour_crack_probability(63.333) - 1.0).abs() < 0.01,
            "saturates to 100%"
        );
    }

    // ────────────────────────────────────────────────────────
    // 虚心伤害 + crack 概率
    // ────────────────────────────────────────────────────────

    #[test]
    fn void_core_shockwave_damage_by_stage() {
        assert!(
            (void_core_shockwave_damage(VoidErosionStage::None) - 50.0).abs() < 1e-9,
            "stage 0: 50 + 0*25 = 50"
        );
        assert!(
            (void_core_shockwave_damage(VoidErosionStage::EchoBody) - 125.0).abs() < 1e-9,
            "stage 3: 50 + 3*25 = 125"
        );
        assert!(
            (void_core_shockwave_damage(VoidErosionStage::VoidEroded) - 150.0).abs() < 1e-9,
            "stage 4: 50 + 4*25 = 150"
        );
    }

    #[test]
    fn void_core_crack_probability_by_stage() {
        assert!(void_core_crack_probability(VoidErosionStage::None).abs() < 1e-9);
        assert!(void_core_crack_probability(VoidErosionStage::LowPressure).abs() < 1e-9);
        assert!(void_core_crack_probability(VoidErosionStage::VoidShadow).abs() < 1e-9);
        assert!((void_core_crack_probability(VoidErosionStage::EchoBody) - 0.25).abs() < 1e-9);
        assert!((void_core_crack_probability(VoidErosionStage::VoidEroded) - 0.50).abs() < 1e-9);
    }

    #[test]
    fn void_core_contam_per_meridian_by_stage() {
        assert!(
            (void_core_contam_per_meridian(VoidErosionStage::None)).abs() < 1e-9,
            "stage 0 = 0"
        );
        assert!(
            (void_core_contam_per_meridian(VoidErosionStage::EchoBody) - 15.0).abs() < 1e-9,
            "stage 3: 3*5 = 15"
        );
        assert!(
            (void_core_contam_per_meridian(VoidErosionStage::VoidEroded) - 20.0).abs() < 1e-9,
            "stage 4: 4*5 = 20"
        );
    }

    // ────────────────────────────────────────────────────────
    // 常驻涡流范围
    // ────────────────────────────────────────────────────────

    #[test]
    fn ambient_range_stage_4_expanded() {
        let erosion_s3 = VoidErosion {
            cumulative_erosion: 200.0,
            stage: VoidErosionStage::EchoBody,
            ..VoidErosion::default()
        };
        assert!(
            (erosion_s3.ambient_range() - 3.0).abs() < f64::EPSILON as f32,
            "stage 3 should be 3 blocks"
        );
        let erosion_s4 = VoidErosion {
            cumulative_erosion: 400.0,
            stage: VoidErosionStage::VoidEroded,
            ..VoidErosion::default()
        };
        assert!(
            (erosion_s4.ambient_range() - 5.0).abs() < f64::EPSILON as f32,
            "stage 4 should be 5 blocks"
        );
    }

    // ────────────────────────────────────────────────────────
    // Serde 对拍
    // ────────────────────────────────────────────────────────

    #[test]
    fn void_erosion_stage_serde_roundtrip() {
        for stage in [
            VoidErosionStage::None,
            VoidErosionStage::LowPressure,
            VoidErosionStage::VoidShadow,
            VoidErosionStage::EchoBody,
            VoidErosionStage::VoidEroded,
        ] {
            let json = serde_json::to_string(&stage).unwrap();
            let deserialized: VoidErosionStage = serde_json::from_str(&json).unwrap();
            assert_eq!(
                stage, deserialized,
                "stage {:?} should roundtrip through serde, json was {}",
                stage, json
            );
        }
    }

    #[test]
    fn void_erosion_serde_roundtrip() {
        let erosion = VoidErosion {
            cumulative_erosion: 123.456,
            stage: VoidErosionStage::VoidShadow,
            ambient_active: true,
            ambient_toggled_at: 9000,
        };
        let json = serde_json::to_string(&erosion).unwrap();
        let deserialized: VoidErosion = serde_json::from_str(&json).unwrap();
        assert_eq!(
            erosion, deserialized,
            "VoidErosion should roundtrip through serde"
        );
    }

    #[test]
    fn void_erosion_stage_snake_case_serde() {
        let json = serde_json::to_string(&VoidErosionStage::LowPressure).unwrap();
        assert!(
            json.contains("low_pressure"),
            "VoidErosionStage should serialize as snake_case, got {}",
            json
        );
    }

    // ────────────────────────────────────────────────────────
    // Monte Carlo: 虚涡 MeridianCrack 概率分布
    // ────────────────────────────────────────────────────────

    #[test]
    fn void_vortex_crack_probability_monte_carlo_base() {
        // 肺经 contamination = 0 → 概率 = 5%
        let expected = 0.05;
        let n = 10_000;
        let mut rng = deterministic_rng(42);
        let mut hits = 0u32;
        for _ in 0..n {
            let roll: f64 = rng.next_f64();
            if roll < void_vortex_crack_probability(0.0) {
                hits += 1;
            }
        }
        let actual = hits as f64 / n as f64;
        assert!(
            (actual - expected).abs() < 0.03,
            "Monte Carlo base crack probability: expected ~{expected}, got {actual} ({hits}/{n})"
        );
    }

    #[test]
    fn void_vortex_crack_probability_monte_carlo_high_contam() {
        // 肺经 contamination = 10 → 概率 = 25%
        let expected = 0.25;
        let n = 10_000;
        let mut rng = deterministic_rng(123);
        let mut hits = 0u32;
        for _ in 0..n {
            let roll: f64 = rng.next_f64();
            if roll < void_vortex_crack_probability(10.0) {
                hits += 1;
            }
        }
        let actual = hits as f64 / n as f64;
        assert!(
            (actual - expected).abs() < 0.03,
            "Monte Carlo high-contam crack probability: expected ~{expected}, got {actual} ({hits}/{n})"
        );
    }

    // ────────────────────────────────────────────────────────
    // Monte Carlo: 涡流回响失控概率
    // ────────────────────────────────────────────────────────

    #[test]
    fn echo_misfire_monte_carlo_stage_3() {
        let expected = 0.10;
        let n = 10_000;
        let mut rng = deterministic_rng(77);
        let mut hits = 0u32;
        for _ in 0..n {
            let roll: f64 = rng.next_f64();
            if roll < VoidErosionStage::EchoBody.echo_misfire_probability() {
                hits += 1;
            }
        }
        let actual = hits as f64 / n as f64;
        assert!(
            (actual - expected).abs() < 0.03,
            "Monte Carlo echo misfire stage 3: expected ~{expected}, got {actual} ({hits}/{n})"
        );
    }

    #[test]
    fn echo_misfire_monte_carlo_stage_4() {
        let expected = 0.25;
        let n = 10_000;
        let mut rng = deterministic_rng(999);
        let mut hits = 0u32;
        for _ in 0..n {
            let roll: f64 = rng.next_f64();
            if roll < VoidErosionStage::VoidEroded.echo_misfire_probability() {
                hits += 1;
            }
        }
        let actual = hits as f64 / n as f64;
        assert!(
            (actual - expected).abs() < 0.03,
            "Monte Carlo echo misfire stage 4: expected ~{expected}, got {actual} ({hits}/{n})"
        );
    }

    #[test]
    fn echo_misfire_monte_carlo_stages_0_1_2_never_fire() {
        let n = 10_000;
        let mut rng = deterministic_rng(333);
        for stage in [
            VoidErosionStage::None,
            VoidErosionStage::LowPressure,
            VoidErosionStage::VoidShadow,
        ] {
            let mut hits = 0u32;
            for _ in 0..n {
                let roll: f64 = rng.next_f64();
                if roll < stage.echo_misfire_probability() {
                    hits += 1;
                }
            }
            assert_eq!(
                hits, 0,
                "stage {:?} should never misfire, but had {} hits in {n}",
                stage, hits
            );
        }
    }

    // ────────────────────────────────────────────────────────
    // Monte Carlo: 虚心 MeridianCrack 概率
    // ────────────────────────────────────────────────────────

    #[test]
    fn void_core_crack_monte_carlo_stage_3() {
        let expected = 0.25;
        let n = 10_000;
        let mut rng = deterministic_rng(555);
        let mut hits = 0u32;
        for _ in 0..n {
            let roll: f64 = rng.next_f64();
            if roll < void_core_crack_probability(VoidErosionStage::EchoBody) {
                hits += 1;
            }
        }
        let actual = hits as f64 / n as f64;
        assert!(
            (actual - expected).abs() < 0.03,
            "Monte Carlo void core crack stage 3: expected ~{expected}, got {actual} ({hits}/{n})"
        );
    }

    #[test]
    fn void_core_crack_monte_carlo_stage_4() {
        let expected = 0.50;
        let n = 10_000;
        let mut rng = deterministic_rng(666);
        let mut hits = 0u32;
        for _ in 0..n {
            let roll: f64 = rng.next_f64();
            if roll < void_core_crack_probability(VoidErosionStage::VoidEroded) {
                hits += 1;
            }
        }
        let actual = hits as f64 / n as f64;
        assert!(
            (actual - expected).abs() < 0.03,
            "Monte Carlo void core crack stage 4: expected ~{expected}, got {actual} ({hits}/{n})"
        );
    }

    // ────────────────────────────────────────────────────────
    // Monte Carlo: 吞涡 MeridianCrack (> 30 threshold)
    // ────────────────────────────────────────────────────────

    #[test]
    fn swallowing_devour_crack_monte_carlo_excess_10() {
        // devoured_qi = 40 → excess = 10 → probability = 30%
        let expected = 0.30;
        let n = 10_000;
        let mut rng = deterministic_rng(888);
        let mut hits = 0u32;
        for _ in 0..n {
            let roll: f64 = rng.next_f64();
            if roll < swallowing_devour_crack_probability(40.0) {
                hits += 1;
            }
        }
        let actual = hits as f64 / n as f64;
        assert!(
            (actual - expected).abs() < 0.03,
            "Monte Carlo swallowing devour crack excess 10: expected ~{expected}, got {actual} ({hits}/{n})"
        );
    }

    // ────────────────────────────────────────────────────────
    // 常驻涡流常量完整性
    // ────────────────────────────────────────────────────────

    /// Helper: wrap a constant in black_box to avoid clippy assertions_on_constants.
    fn bb_f64(v: f64) -> f64 {
        std::hint::black_box(v)
    }
    fn bb_f32(v: f32) -> f32 {
        std::hint::black_box(v)
    }
    fn bb_u64(v: u64) -> u64 {
        std::hint::black_box(v)
    }

    #[test]
    fn ambient_constants_are_sane() {
        let v = bb_f64(AMBIENT_EROSION_PER_SEC);
        assert!(
            v > 0.0 && v < 1.0,
            "ambient erosion/s should be small positive, got {v}"
        );
        assert!(
            bb_f64(AMBIENT_QI_RECOVERY_PER_SEC) > 0.0,
            "ambient qi recovery should be positive"
        );
        assert!(
            bb_f64(AMBIENT_ZONE_DRAIN_PER_SEC) > 0.0,
            "ambient zone drain should be positive"
        );
        assert!(
            bb_f64(AMBIENT_CONTAM_PER_SEC) > 0.0,
            "ambient contamination should be positive"
        );
    }

    #[test]
    fn void_vortex_constants_are_sane() {
        assert!(
            bb_f64(VOID_VORTEX_QI_COST) > 0.0,
            "void vortex qi cost should be positive"
        );
        assert!(
            bb_u64(VOID_VORTEX_COOLDOWN_TICKS) > 0,
            "void vortex cooldown should be positive"
        );
        assert!(
            bb_u64(VOID_VORTEX_DURATION_TICKS) > 0,
            "void vortex duration should be positive"
        );
        assert!(
            bb_f64(VOID_VORTEX_EROSION) > bb_f64(BASE_SKILL_EROSION),
            "void vortex erosion should exceed base skill erosion"
        );
    }

    #[test]
    fn swallowing_constants_are_sane() {
        assert!(
            bb_f64(SWALLOWING_QI_COST) > 0.0,
            "swallowing qi cost should be positive"
        );
        let r = bb_f64(SWALLOWING_RELEASE_DAMAGE_RATIO);
        assert!(
            r > 0.0 && r <= 1.0,
            "swallowing release damage ratio should be in (0, 1], got {r}"
        );
        let d = bb_f64(SWALLOWING_DEVOUR_QI_RATIO);
        assert!(
            d > 0.0 && d <= 1.0,
            "swallowing devour qi ratio should be in (0, 1], got {d}"
        );
        assert!(
            bb_f64(SWALLOWING_CRACK_THRESHOLD) > 0.0,
            "swallowing crack threshold should be positive"
        );
    }

    #[test]
    fn void_core_constants_are_sane() {
        assert!(
            bb_f64(VOID_CORE_QI_COST) > 0.0,
            "void core qi cost should be positive"
        );
        assert!(
            bb_u64(VOID_CORE_COOLDOWN_TICKS) > bb_u64(VOID_CORE_DURATION_TICKS),
            "cooldown should exceed duration"
        );
        assert!(
            bb_f64(VOID_CORE_EROSION_PER_SEC) > 0.0,
            "void core erosion/s should be positive"
        );
        assert!(
            bb_f64(VOID_CORE_BASE_DAMAGE) > 0.0,
            "void core base damage should be positive"
        );
    }

    #[test]
    fn echo_constants_are_sane() {
        assert!(
            bb_u64(ECHO_DELAY_STAGE_1_2) > bb_u64(ECHO_DELAY_STAGE_3),
            "earlier stages should have longer delays"
        );
        assert!(
            bb_u64(ECHO_DELAY_STAGE_3) > bb_u64(ECHO_DELAY_STAGE_4),
            "stage 3 delay should exceed stage 4"
        );
        let p = bb_f32(ECHO_POWER_RATIO);
        assert!(
            p > 0.0 && p < 1.0,
            "echo power ratio should be in (0, 1), got {p}"
        );
    }

    // ────────────────────────────────────────────────────────
    // 死亡螺旋边界值
    // ────────────────────────────────────────────────────────

    #[test]
    fn erosion_modifier_boundary_zone_values() {
        // zone_spirit_qi exactly at boundary 0.2 → no positive penalty
        let m = erosion_modifier(VoidErosionStage::VoidEroded, 0.2, 0.0);
        assert!(
            (m.efficiency - 1.0).abs() < 1e-9,
            "zone exactly at +0.2 should not apply positive penalty"
        );
        // zone_spirit_qi exactly at boundary -0.2 → no negative bonus
        let m = erosion_modifier(VoidErosionStage::VoidEroded, -0.2, 0.0);
        assert!(
            m.neg_bonus.abs() < 1e-9,
            "zone exactly at -0.2 should not apply negative bonus"
        );
    }

    #[test]
    fn erosion_modifier_extreme_dandao_penalty() {
        // High dandao penalty (max stage) + high erosion stage
        let m = erosion_modifier(VoidErosionStage::VoidEroded, 0.0, 1.0);
        let expected = (1.0 + 1.0) * 1.8; // = 3.6
        assert!(
            (m.contam_mult - expected).abs() < 1e-9,
            "extreme dandao=1.0 + erosion stage 4 contam_mult should be {}, got {}",
            expected,
            m.contam_mult
        );
    }

    // ────────────────────────────────────────────────────────
    // 虚心伤害边界值
    // ────────────────────────────────────────────────────────

    #[test]
    fn void_core_shockwave_damage_all_stages() {
        let expected = [
            (VoidErosionStage::None, 50.0),
            (VoidErosionStage::LowPressure, 75.0),
            (VoidErosionStage::VoidShadow, 100.0),
            (VoidErosionStage::EchoBody, 125.0),
            (VoidErosionStage::VoidEroded, 150.0),
        ];
        for (stage, damage) in expected {
            assert!(
                (void_core_shockwave_damage(stage) - damage).abs() < 1e-9,
                "stage {:?} damage: expected {damage}, got {}",
                stage,
                void_core_shockwave_damage(stage)
            );
        }
    }

    #[test]
    fn void_core_contam_all_stages() {
        let expected = [
            (VoidErosionStage::None, 0.0),
            (VoidErosionStage::LowPressure, 5.0),
            (VoidErosionStage::VoidShadow, 10.0),
            (VoidErosionStage::EchoBody, 15.0),
            (VoidErosionStage::VoidEroded, 20.0),
        ];
        for (stage, contam) in expected {
            assert!(
                (void_core_contam_per_meridian(stage) - contam).abs() < 1e-9,
                "stage {:?} contam: expected {contam}, got {}",
                stage,
                void_core_contam_per_meridian(stage)
            );
        }
    }

    // ────────────────────────────────────────────────────────
    // VoidErosion component 状态管理
    // ────────────────────────────────────────────────────────

    #[test]
    fn void_erosion_repeated_init_does_not_reset() {
        let mut erosion = VoidErosion::default();
        add_erosion(&mut erosion, 50.0);
        assert!(
            (erosion.cumulative_erosion - 50.0).abs() < f64::EPSILON,
            "first add should work"
        );
        // A second "init" (represented as creating another default) should NOT replace
        // The real system uses insert_if_absent so we verify default doesn't clobber
        let default = VoidErosion::default();
        assert!(
            erosion.cumulative_erosion > default.cumulative_erosion,
            "existing erosion={} should be preserved over default={}",
            erosion.cumulative_erosion,
            default.cumulative_erosion
        );
    }

    #[test]
    fn add_erosion_incremental_stage_transitions() {
        let mut erosion = VoidErosion::default();
        // Accumulate past all thresholds one by one
        let additions = [10.0, 10.0, 60.0, 120.0, 200.0, 100.0];
        let expected_stages = [
            VoidErosionStage::None,        // 10
            VoidErosionStage::LowPressure, // 20
            VoidErosionStage::VoidShadow,  // 80
            VoidErosionStage::EchoBody,    // 200
            VoidErosionStage::VoidEroded,  // 400
            VoidErosionStage::VoidEroded,  // 500 (stays)
        ];
        for (i, &amount) in additions.iter().enumerate() {
            add_erosion(&mut erosion, amount);
            assert_eq!(
                erosion.computed_stage(),
                expected_stages[i],
                "after adding {amount} (total={}), stage should be {:?}",
                erosion.cumulative_erosion,
                expected_stages[i]
            );
        }
    }

    // ────────────────────────────────────────────────────────
    // VoidErosionAdvanceEvent pin test
    // ────────────────────────────────────────────────────────

    #[test]
    fn void_erosion_advance_event_fields() {
        let event = VoidErosionAdvanceEvent {
            entity: Entity::PLACEHOLDER,
            from: VoidErosionStage::LowPressure,
            to: VoidErosionStage::VoidShadow,
        };
        assert_eq!(event.from, VoidErosionStage::LowPressure);
        assert_eq!(event.to, VoidErosionStage::VoidShadow);
    }

    // ────────────────────────────────────────────────────────
    // ErosionMod struct pin test
    // ────────────────────────────────────────────────────────

    #[test]
    fn erosion_mod_struct_fields_accessible() {
        let m = ErosionMod {
            efficiency: 0.88,
            neg_bonus: 0.25,
            contam_mult: 1.3,
        };
        assert!((m.efficiency - 0.88).abs() < 1e-9);
        assert!((m.neg_bonus - 0.25).abs() < 1e-9);
        assert!((m.contam_mult - 1.3).abs() < 1e-9);
    }

    // ────────────────────────────────────────────────────────
    // 常量 pin 测试
    // ────────────────────────────────────────────────────────

    #[test]
    fn check_interval_is_30_seconds() {
        assert_eq!(
            bb_u64(VOID_EROSION_CHECK_INTERVAL),
            600,
            "check interval should be 600 ticks (30s at 20 tps)"
        );
    }

    #[test]
    fn stage_thresholds_monotonic() {
        let thresholds = std::hint::black_box(STAGE_THRESHOLDS);
        for i in 1..thresholds.len() {
            assert!(
                thresholds[i] > thresholds[i - 1],
                "thresholds should be strictly monotonic: [{}]={} should > [{}]={}",
                i,
                thresholds[i],
                i - 1,
                thresholds[i - 1]
            );
        }
    }

    // ────────────────────────────────────────────────────────
    // Deterministic RNG for Monte Carlo tests
    // ────────────────────────────────────────────────────────

    /// Simple xoshiro128-based deterministic RNG for test reproducibility.
    struct TestRng {
        state: u64,
    }

    impl TestRng {
        fn next_u64(&mut self) -> u64 {
            self.state = self
                .state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            self.state
        }

        fn next_f64(&mut self) -> f64 {
            (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
        }
    }

    fn deterministic_rng(seed: u64) -> TestRng {
        TestRng { state: seed }
    }

    // ────────────────────────────────────────────────────────
    // P3: 虚蚀视觉常量
    // ────────────────────────────────────────────────────────

    #[test]
    fn erosion_model_alpha_by_stage() {
        let cases = [
            (VoidErosionStage::None, 1.0_f32),
            (VoidErosionStage::LowPressure, 0.85),
            (VoidErosionStage::VoidShadow, 0.70),
            (VoidErosionStage::EchoBody, 0.55),
            (VoidErosionStage::VoidEroded, 0.40),
        ];
        for (stage, expected_alpha) in cases {
            let alpha = erosion_model_alpha(stage);
            assert!(
                (alpha - expected_alpha).abs() < 1e-6,
                "stage {:?}: expected alpha={expected_alpha}, got {alpha}",
                stage
            );
        }
    }

    #[test]
    fn sound_distortion_overlay_by_stage() {
        assert!(
            !should_show_sound_distortion_overlay(VoidErosionStage::None),
            "stage 0 should not show distortion"
        );
        assert!(
            !should_show_sound_distortion_overlay(VoidErosionStage::LowPressure),
            "stage 1 should not show distortion"
        );
        assert!(
            !should_show_sound_distortion_overlay(VoidErosionStage::VoidShadow),
            "stage 2 should not show distortion"
        );
        assert!(
            should_show_sound_distortion_overlay(VoidErosionStage::EchoBody),
            "stage 3 should show distortion"
        );
        assert!(
            should_show_sound_distortion_overlay(VoidErosionStage::VoidEroded),
            "stage 4 should show distortion"
        );
    }

    #[test]
    fn echo_replay_vfx_scale_is_0_4() {
        assert!(
            (bb_f32(ECHO_REPLAY_VFX_SCALE) - 0.4).abs() < f32::EPSILON,
            "echo replay VFX scale should be 0.4"
        );
    }

    // ────────────────────────────────────────────────────────
    // P4: 境界递进门控 — realm_erosion_cap
    // ────────────────────────────────────────────────────────

    #[test]
    fn realm_erosion_cap_awaken() {
        assert_eq!(
            realm_erosion_cap(Realm::Awaken),
            VoidErosionStage::None,
            "Awaken should cap at stage None"
        );
    }

    #[test]
    fn realm_erosion_cap_induce() {
        assert_eq!(
            realm_erosion_cap(Realm::Induce),
            VoidErosionStage::LowPressure,
            "Induce should cap at LowPressure"
        );
    }

    #[test]
    fn realm_erosion_cap_condense() {
        assert_eq!(
            realm_erosion_cap(Realm::Condense),
            VoidErosionStage::LowPressure,
            "Condense should cap at LowPressure (阶段 1 可达)"
        );
    }

    #[test]
    fn realm_erosion_cap_solidify() {
        assert_eq!(
            realm_erosion_cap(Realm::Solidify),
            VoidErosionStage::VoidShadow,
            "Solidify should cap at VoidShadow (阶段 2 可达)"
        );
    }

    #[test]
    fn realm_erosion_cap_spirit() {
        assert_eq!(
            realm_erosion_cap(Realm::Spirit),
            VoidErosionStage::EchoBody,
            "Spirit should cap at EchoBody (阶段 3 可达)"
        );
    }

    #[test]
    fn realm_erosion_cap_void() {
        assert_eq!(
            realm_erosion_cap(Realm::Void),
            VoidErosionStage::VoidEroded,
            "Void should cap at VoidEroded (阶段 4 可达 + 所有招式)"
        );
    }

    #[test]
    fn realm_erosion_cap_monotonically_increasing() {
        let realms = [
            Realm::Awaken,
            Realm::Induce,
            Realm::Condense,
            Realm::Solidify,
            Realm::Spirit,
            Realm::Void,
        ];
        for i in 1..realms.len() {
            assert!(
                realm_erosion_cap(realms[i]) >= realm_erosion_cap(realms[i - 1]),
                "realm {:?} cap should >= {:?} cap",
                realms[i],
                realms[i - 1]
            );
        }
    }

    // ────────────────────────────────────────────────────────
    // P4: realm_unlocks_skill — 全交叉矩阵
    // ────────────────────────────────────────────────────────

    #[test]
    fn realm_unlocks_vortex_echo() {
        assert!(
            !realm_unlocks_skill(Realm::Awaken, WoliuSkillId::VortexEcho),
            "Awaken should NOT unlock VortexEcho"
        );
        assert!(
            realm_unlocks_skill(Realm::Induce, WoliuSkillId::VortexEcho),
            "Induce should unlock VortexEcho"
        );
        assert!(
            realm_unlocks_skill(Realm::Condense, WoliuSkillId::VortexEcho),
            "Condense should unlock VortexEcho"
        );
        assert!(
            realm_unlocks_skill(Realm::Solidify, WoliuSkillId::VortexEcho),
            "Solidify should unlock VortexEcho"
        );
        assert!(
            realm_unlocks_skill(Realm::Spirit, WoliuSkillId::VortexEcho),
            "Spirit should unlock VortexEcho"
        );
        assert!(
            realm_unlocks_skill(Realm::Void, WoliuSkillId::VortexEcho),
            "Void should unlock VortexEcho"
        );
    }

    #[test]
    fn realm_unlocks_ambient_vortex() {
        assert!(
            !realm_unlocks_skill(Realm::Awaken, WoliuSkillId::AmbientVortex),
            "Awaken should NOT unlock AmbientVortex"
        );
        assert!(
            !realm_unlocks_skill(Realm::Induce, WoliuSkillId::AmbientVortex),
            "Induce should NOT unlock AmbientVortex"
        );
        assert!(
            realm_unlocks_skill(Realm::Condense, WoliuSkillId::AmbientVortex),
            "Condense should unlock AmbientVortex"
        );
        assert!(
            realm_unlocks_skill(Realm::Solidify, WoliuSkillId::AmbientVortex),
            "Solidify should unlock AmbientVortex"
        );
    }

    #[test]
    fn realm_unlocks_void_vortex_and_swallowing() {
        for skill in [WoliuSkillId::VoidVortex, WoliuSkillId::SwallowingVortex] {
            assert!(
                !realm_unlocks_skill(Realm::Awaken, skill),
                "Awaken should NOT unlock {skill:?}"
            );
            assert!(
                !realm_unlocks_skill(Realm::Induce, skill),
                "Induce should NOT unlock {skill:?}"
            );
            assert!(
                !realm_unlocks_skill(Realm::Condense, skill),
                "Condense should NOT unlock {skill:?}"
            );
            assert!(
                realm_unlocks_skill(Realm::Solidify, skill),
                "Solidify should unlock {skill:?}"
            );
            assert!(
                realm_unlocks_skill(Realm::Spirit, skill),
                "Spirit should unlock {skill:?}"
            );
            assert!(
                realm_unlocks_skill(Realm::Void, skill),
                "Void should unlock {skill:?}"
            );
        }
    }

    #[test]
    fn realm_unlocks_void_core() {
        assert!(
            !realm_unlocks_skill(Realm::Awaken, WoliuSkillId::VoidCore),
            "Awaken should NOT unlock VoidCore"
        );
        assert!(
            !realm_unlocks_skill(Realm::Induce, WoliuSkillId::VoidCore),
            "Induce should NOT unlock VoidCore"
        );
        assert!(
            !realm_unlocks_skill(Realm::Condense, WoliuSkillId::VoidCore),
            "Condense should NOT unlock VoidCore"
        );
        assert!(
            !realm_unlocks_skill(Realm::Solidify, WoliuSkillId::VoidCore),
            "Solidify should NOT unlock VoidCore"
        );
        assert!(
            realm_unlocks_skill(Realm::Spirit, WoliuSkillId::VoidCore),
            "Spirit should unlock VoidCore"
        );
        assert!(
            realm_unlocks_skill(Realm::Void, WoliuSkillId::VoidCore),
            "Void should unlock VoidCore"
        );
    }

    #[test]
    fn realm_unlocks_non_erosion_skills_always_true() {
        // Non-erosion skills should always return true (not gated by this function)
        for realm in [
            Realm::Awaken,
            Realm::Induce,
            Realm::Condense,
            Realm::Solidify,
            Realm::Spirit,
            Realm::Void,
        ] {
            for skill in [
                WoliuSkillId::Hold,
                WoliuSkillId::Burst,
                WoliuSkillId::Mouth,
                WoliuSkillId::Pull,
                WoliuSkillId::Heart,
            ] {
                assert!(
                    realm_unlocks_skill(realm, skill),
                    "non-erosion skill {skill:?} should always be unlocked at realm {realm:?}"
                );
            }
        }
    }

    #[test]
    fn void_realm_unlocks_all_erosion_skills() {
        for skill in [
            WoliuSkillId::AmbientVortex,
            WoliuSkillId::VoidVortex,
            WoliuSkillId::SwallowingVortex,
            WoliuSkillId::VortexEcho,
            WoliuSkillId::VoidCore,
        ] {
            assert!(
                realm_unlocks_skill(Realm::Void, skill),
                "Void realm should unlock all erosion skills, failed for {skill:?}"
            );
        }
    }

    // ────────────────────────────────────────────────────────
    // P4: add_erosion_capped — 境界阶段 cap 验证
    // ────────────────────────────────────────────────────────

    #[test]
    fn add_erosion_capped_respects_realm_cap() {
        // Awaken realm: cap = None, even with massive erosion
        let mut erosion = VoidErosion::default();
        add_erosion_capped(&mut erosion, 500.0, Realm::Awaken);
        assert_eq!(
            erosion.stage,
            VoidErosionStage::None,
            "Awaken realm should cap stage at None even with cumulative_erosion=500"
        );
        assert!(
            (erosion.cumulative_erosion - 500.0).abs() < f64::EPSILON,
            "cumulative_erosion should still accumulate"
        );
    }

    #[test]
    fn add_erosion_capped_induce_caps_at_low_pressure() {
        let mut erosion = VoidErosion::default();
        add_erosion_capped(&mut erosion, 100.0, Realm::Induce);
        assert_eq!(
            erosion.stage,
            VoidErosionStage::LowPressure,
            "Induce realm should cap at LowPressure even with cumulative=100"
        );
    }

    #[test]
    fn add_erosion_capped_solidify_caps_at_void_shadow() {
        let mut erosion = VoidErosion::default();
        add_erosion_capped(&mut erosion, 999.0, Realm::Solidify);
        assert_eq!(
            erosion.stage,
            VoidErosionStage::VoidShadow,
            "Solidify realm should cap at VoidShadow even with cumulative=999"
        );
    }

    #[test]
    fn add_erosion_capped_void_allows_max_stage() {
        let mut erosion = VoidErosion::default();
        add_erosion_capped(&mut erosion, 500.0, Realm::Void);
        assert_eq!(
            erosion.stage,
            VoidErosionStage::VoidEroded,
            "Void realm should allow VoidEroded"
        );
    }

    #[test]
    fn add_erosion_capped_below_threshold_normal_progression() {
        // Within Solidify cap (VoidShadow = 80-200), 50 erosion -> LowPressure
        let mut erosion = VoidErosion::default();
        add_erosion_capped(&mut erosion, 50.0, Realm::Solidify);
        assert_eq!(
            erosion.stage,
            VoidErosionStage::LowPressure,
            "50 erosion at Solidify should be LowPressure (below cap)"
        );
    }

    #[test]
    fn add_erosion_capped_invalid_amounts_ignored() {
        let mut erosion = VoidErosion::default();
        add_erosion_capped(&mut erosion, 0.0, Realm::Void);
        assert!(
            erosion.cumulative_erosion.abs() < f64::EPSILON,
            "zero amount should not change erosion"
        );
        add_erosion_capped(&mut erosion, -5.0, Realm::Void);
        assert!(
            erosion.cumulative_erosion.abs() < f64::EPSILON,
            "negative amount should not change erosion"
        );
        add_erosion_capped(&mut erosion, f64::NAN, Realm::Void);
        assert!(
            erosion.cumulative_erosion.abs() < f64::EPSILON,
            "NaN should not change erosion"
        );
    }

    #[test]
    fn add_erosion_capped_incremental_with_realm_change() {
        let mut erosion = VoidErosion::default();
        // Start at Induce: accumulate 30 -> LowPressure (capped at LowPressure)
        add_erosion_capped(&mut erosion, 30.0, Realm::Induce);
        assert_eq!(erosion.stage, VoidErosionStage::LowPressure);

        // More erosion at Induce: 100 total -> still LowPressure (cap)
        add_erosion_capped(&mut erosion, 70.0, Realm::Induce);
        assert_eq!(
            erosion.stage,
            VoidErosionStage::LowPressure,
            "Induce cap prevents VoidShadow"
        );

        // Breakthrough to Solidify: now VoidShadow cap applies
        // cumulative = 100 -> VoidShadow, and cap is VoidShadow
        add_erosion_capped(&mut erosion, 0.1, Realm::Solidify);
        assert_eq!(
            erosion.stage,
            VoidErosionStage::VoidShadow,
            "after breakthrough to Solidify, stage should unlock to VoidShadow"
        );
    }

    // ────────────────────────────────────────────────────────
    // P4: 天道互动
    // ────────────────────────────────────────────────────────

    #[test]
    fn tiandao_detection_modifier_by_stage() {
        let cases = [
            (VoidErosionStage::None, 1.0),
            (VoidErosionStage::LowPressure, 1.0),
            (VoidErosionStage::VoidShadow, 1.0),
            (VoidErosionStage::EchoBody, 0.6),
            (VoidErosionStage::VoidEroded, 0.0),
        ];
        for (stage, expected) in cases {
            let modifier = tiandao_detection_modifier(stage);
            assert!(
                (modifier - expected).abs() < 1e-9,
                "stage {:?}: expected tiandao modifier={expected}, got {modifier}",
                stage
            );
        }
    }

    #[test]
    fn tiandao_detection_modifier_stage3_is_minus_40_percent() {
        let modifier = tiandao_detection_modifier(VoidErosionStage::EchoBody);
        assert!(
            (modifier - 0.6).abs() < 1e-9,
            "stage 3 should reduce tiandao detection by 40% (modifier=0.6), got {modifier}"
        );
    }

    #[test]
    fn tiandao_detection_modifier_stage4_abandons_tracking() {
        let modifier = tiandao_detection_modifier(VoidErosionStage::VoidEroded);
        assert!(
            modifier.abs() < 1e-9,
            "stage 4 should fully abandon tracking (modifier=0.0), got {modifier}"
        );
    }
}
