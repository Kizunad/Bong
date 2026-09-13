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
    /// 当前已 cap（境界门控）阶段，由 `add_erosion_capped` 维护。
    pub stage: VoidErosionStage,
    /// `void_erosion_check_system` 最近一次已上报的阶段。
    /// 用于检测 `stage` 是否推进（而非 uncapped computed_stage），
    /// 保证 check 不绕境界上限、事件准确反映实际 capped stage 推进。
    /// 跨死亡保留语义同 `stage`。
    pub last_reported_stage: VoidErosionStage,
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
            last_reported_stage: VoidErosionStage::None,
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
        WoliuSkillId::Hold
        | WoliuSkillId::Burst
        | WoliuSkillId::Mouth
        | WoliuSkillId::Pull
        | WoliuSkillId::Heart
        | WoliuSkillId::VacuumPalm
        | WoliuSkillId::VortexShield
        | WoliuSkillId::VacuumLock
        | WoliuSkillId::VortexResonance
        | WoliuSkillId::TurbulenceBurst => true,
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

// ────────────────────────────────────────────────────────
// plan-combat-skill-feedback-bridges-v1 P3 — void_erosion_check_system
// ────────────────────────────────────────────────────────

use valence::prelude::{EventWriter, Query, Res};

use crate::combat::CombatClock;

/// 每 600 tick（30s）检测虚蚀阶段推进，emit `VoidErosionAdvanceEvent`。
///
/// 比较**当前 capped `stage`**（由 `add_erosion_capped` 维护）与
/// `last_reported_stage`——两者相等则跳过，否则 emit 并更新 `last_reported_stage`。
///
/// 设计要点：
/// - 不再调用 uncapped `computed_stage()` 重算（修复 major1：不绕境界上限）。
/// - `stage` 已在 `add_erosion_capped` 被 cap，check 只需看它是否推进过
///   `last_reported_stage`（修复 blocker：能检测到真实施法路径的 capped stage 推进）。
/// - 同帧跨多阶时 `to` 取当前 `stage`（最终档），不分拆多次 emit。
pub fn void_erosion_check_system(
    clock: Res<CombatClock>,
    mut erosion_query: Query<(Entity, &mut VoidErosion)>,
    mut advance_events: EventWriter<VoidErosionAdvanceEvent>,
) {
    if !clock.tick.is_multiple_of(VOID_EROSION_CHECK_INTERVAL) {
        return;
    }
    for (entity, mut erosion) in &mut erosion_query {
        if erosion.stage > erosion.last_reported_stage {
            let from = erosion.last_reported_stage;
            let to = erosion.stage;
            erosion.last_reported_stage = to;
            advance_events.send(VoidErosionAdvanceEvent { entity, from, to });
        }
    }
}
