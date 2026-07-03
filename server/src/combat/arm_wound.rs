//! plan-combat-hit-location-v1 P1 — 臂伤消费端：命中部位真实化后，ArmL/ArmR 终于有
//! gameplay 后果（决议 §8.1 #2）。结构照抄 `movement/leg_wound.rs:13-65`
//! （`WoundGrade` 枚举 + `wound_severity_to_grade` + `worst_wound_grade` 模式），
//! 但臂伤是多维度惩罚（攻击/冷却/格挡/散布/蓄力/施法），不像腿伤只有单一"速度"维度。
//!
//! ## 主副手臂划分（决议 §8.1 #2）
//!
//! 本仓库当前没有玩家惯用手（handedness）组件——`MainHand`/`OffHand` 只是装备槽概念
//! （武器持握 vs 盾牌持握），不等同于解剖学左右臂。P1 采用约定：
//! **`ArmR` = 主手臂（持械侧）**，**`ArmL` = 副手臂（持盾/副手侧）**，对应现实世界
//! 右利手默认与 vanilla MC `main_hand` 默认为右手一致。此约定与经脉/护甲等系统的
//! `ArmL`/`ArmR` 命名保持不变，只是新增"主/副"语义标签，不改变 `BodyPart` 枚举本身。
//!
//! ## 六维伤势表（决议 §8.1 #2）
//!
//! | 分级 | 攻击伤害 | 冷却 | 格挡减伤 | 散布角 | 蓄力时长 | 施法 cast_ticks |
//! |------|---------|------|---------|--------|---------|----------------|
//! | Bruise    | ×0.95 | ×1.0  | ×1.0 | ×1.0 | ×1.0 | ×1.0 |
//! | Abrasion  | ×0.90 | ×1.0  | ×1.0 | ×1.0 | ×1.0 | ×1.0 |
//! | Laceration| ×0.80 | ×1.10 | ×0.80 (−20%) | ×1.0 | ×1.0 | ×1.0 |
//! | Fracture  | ×0.60 | ×1.25 | ×0.60 (−40%) | ×1.50 (+50%) | ×1.30 (+30%) | ×1.0 |
//! | Severed   | ×0.40 | ×1.25 | ×0.60 (−40%) | ×1.50 (+50%) | ×1.30 (+30%) | ×1.25 (+25%) |
//!
//! 决议表原文只显式给出 Laceration/Fracture 两档的冷却/格挡/散布/蓄力数值，Severed 行
//! 只额外强调"脱手落地 + 无法双手持械 + cast_ticks+25%"。工程决策：伤势严重度单调
//! 不减（与 `leg_wound_to_speed` 的 Severed=0.0 严格劣于 Fracture=0.4 同构），
//! Severed 在冷却/格挡/散布/蓄力四维上**延续 Fracture 数值作为下限**而非无中生数的
//! 新数字——武器已脱手时这四维本就不再是主要惩罚来源（脱手本身已是更强的惩罚）。
//!
//! ## 主副手臂取值规则（决议 §8.1 #2）
//!
//! - 攻击伤害 / 冷却 / 散布 / 蓄力惩罚：读**主手臂**（持械侧）伤势分级。
//! - 格挡惩罚：读**副手臂**（持盾侧）伤势分级。
//! - 施法 cast_ticks 惩罚（"结印手受损"）：施法需要双手结印，读**两臂中较重**的分级
//!   （`worst_of_grades`），不偏主副手。
//! - 双臂皆伤：**各维度独立取值，不叠乘**——不会出现"主手 Fracture 攻击 ×0.60 且副手
//!   也 Fracture 再乘一次格挡 ×0.60 变成总惩罚翻倍"的情况，因为每个维度只读它对应
//!   的那只手，互不影响。
//! - 拒绝路线：不做"臂伤禁招"硬门。经脉断（`MeridianSeveredPermanent`）是招式不可用；
//!   肉伤（本模块）是招式可用但打折——两者边界清晰分离。
//!
//! ## 本阶段实际接线范围
//!
//! plan §8.1 决议原文的"落点"只明确点名三处消费端：
//! `combat/resolve.rs`（攻击伤害）、`combat/shield_block.rs` + `combat/zhenmai_v2.rs`
//! （格挡减伤）、`combat/anqi_v2.rs` + `combat/needle.rs`（散布角）。冷却 / 蓄力时长 /
//! cast_ticks 三维的天然宿主系统（`combat/player_attack.rs` 攻击冷却门、
//! `cultivation/full_power_strike.rs` 蓄力速率、各技能模块散落的 cast 时长构造点）
//! 不在本 work item 声明的文件范围内，本阶段只把这三维实现为完整测试覆盖的纯函数
//! （决议表数值已锁定、可随时被下游系统读取），暂不改动上述宿主文件，避免单个 P1
//! 阶段同时牵动过多不相关模块导致测试面失焦。

use crate::combat::components::{BodyPart, Wound, Wounds};

/// 与 `movement::leg_wound::LegWoundGrade` 同构的五级分级（决议 §8.1 #2）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArmWoundGrade {
    Intact,
    Bruise,
    Abrasion,
    Laceration,
    Fracture,
    Severed,
}

/// 决议 §8.1 #2：主手臂 = 持械侧（约定 `ArmR`，本模块顶部文档注释详述理由）。
pub const MAIN_ARM: BodyPart = BodyPart::ArmR;
/// 决议 §8.1 #2：副手臂 = 持盾/副手侧（约定 `ArmL`）。
pub const OFF_ARM: BodyPart = BodyPart::ArmL;

/// 伤势严重度 → 分级映射。数值边界与 `movement::leg_wound::wound_severity_to_grade`
/// 完全一致（决议 §8.1 #2 要求"结构完全照抄 movement/leg_wound.rs:13-65"）。
pub fn wound_severity_to_grade(severity: f32) -> ArmWoundGrade {
    if severity >= 70.0 {
        ArmWoundGrade::Severed
    } else if severity >= 35.0 {
        ArmWoundGrade::Fracture
    } else if severity >= 15.0 {
        ArmWoundGrade::Laceration
    } else if severity >= 5.0 {
        ArmWoundGrade::Abrasion
    } else if severity > 0.0 {
        ArmWoundGrade::Bruise
    } else {
        ArmWoundGrade::Intact
    }
}

const fn grade_rank(grade: ArmWoundGrade) -> u8 {
    match grade {
        ArmWoundGrade::Intact => 0,
        ArmWoundGrade::Bruise => 1,
        ArmWoundGrade::Abrasion => 2,
        ArmWoundGrade::Laceration => 3,
        ArmWoundGrade::Fracture => 4,
        ArmWoundGrade::Severed => 5,
    }
}

fn wound_grade(wound: &Wound) -> ArmWoundGrade {
    wound_severity_to_grade(wound.severity)
}

/// 给定 body part（`ArmL`/`ArmR`），取该侧所有伤口中最严重的分级。
/// 镜像 `movement::leg_wound::worst_wound_grade` 签名与语义。
pub fn worst_wound_grade(wounds: &Wounds, part: BodyPart) -> ArmWoundGrade {
    wounds
        .entries
        .iter()
        .filter(|wound| wound.location == part)
        .map(wound_grade)
        .max_by_key(|grade| grade_rank(*grade))
        .unwrap_or(ArmWoundGrade::Intact)
}

fn worst_of_grades(a: ArmWoundGrade, b: ArmWoundGrade) -> ArmWoundGrade {
    if grade_rank(a) >= grade_rank(b) {
        a
    } else {
        b
    }
}

/// 攻击伤害倍率（决议 §8.1 #2 表首列）：Bruise 0.95 / Abrasion 0.90 / Laceration 0.80 /
/// Fracture 0.60 / Severed 0.40。读主手臂（持械侧）分级。
pub fn attack_damage_multiplier(grade: ArmWoundGrade) -> f32 {
    match grade {
        ArmWoundGrade::Intact => 1.0,
        ArmWoundGrade::Bruise => 0.95,
        ArmWoundGrade::Abrasion => 0.90,
        ArmWoundGrade::Laceration => 0.80,
        ArmWoundGrade::Fracture => 0.60,
        ArmWoundGrade::Severed => 0.40,
    }
}

/// 攻击冷却倍率（决议 §8.1 #2：Laceration +10% / Fracture +25%）。>1.0 = 冷却更长。
/// Severed 延续 Fracture 数值（见模块顶部文档注释"单调不减"说明）。读主手臂分级。
pub fn attack_cooldown_multiplier(grade: ArmWoundGrade) -> f32 {
    match grade {
        ArmWoundGrade::Intact | ArmWoundGrade::Bruise | ArmWoundGrade::Abrasion => 1.0,
        ArmWoundGrade::Laceration => 1.10,
        ArmWoundGrade::Fracture | ArmWoundGrade::Severed => 1.25,
    }
}

/// 格挡/招架减伤效果倍率（决议 §8.1 #2：Laceration −20% / Fracture −40%）。
/// <1.0 = 格挡效果打折（乘在原 block_ratio 上，越低格挡削减的伤害越少）。
/// Severed 延续 Fracture 数值。读副手臂（持盾侧）分级。
pub fn block_multiplier(grade: ArmWoundGrade) -> f32 {
    match grade {
        ArmWoundGrade::Intact | ArmWoundGrade::Bruise | ArmWoundGrade::Abrasion => 1.0,
        ArmWoundGrade::Laceration => 0.80,
        ArmWoundGrade::Fracture | ArmWoundGrade::Severed => 0.60,
    }
}

/// 投射类（凝针/暗器）散布角倍率（决议 §8.1 #2：Fracture +50%）。>1.0 = 散布更宽。
/// Severed 延续 Fracture 数值。读主手臂分级。
pub fn spread_multiplier(grade: ArmWoundGrade) -> f32 {
    match grade {
        ArmWoundGrade::Intact
        | ArmWoundGrade::Bruise
        | ArmWoundGrade::Abrasion
        | ArmWoundGrade::Laceration => 1.0,
        ArmWoundGrade::Fracture | ArmWoundGrade::Severed => 1.50,
    }
}

/// 蓄力类（全力一击）蓄力时长倍率（决议 §8.1 #2：Fracture +30%）。>1.0 = 蓄力更久。
/// Severed 延续 Fracture 数值。读主手臂分级。
pub fn charge_duration_multiplier(grade: ArmWoundGrade) -> f32 {
    match grade {
        ArmWoundGrade::Intact
        | ArmWoundGrade::Bruise
        | ArmWoundGrade::Abrasion
        | ArmWoundGrade::Laceration => 1.0,
        ArmWoundGrade::Fracture | ArmWoundGrade::Severed => 1.30,
    }
}

/// 施法 cast_ticks 倍率（决议 §8.1 #2："结印手受损"，仅 Severed +25%）。
/// 读两臂中较重的分级（施法需双手结印，不偏主副手）。
pub fn cast_ticks_multiplier(grade: ArmWoundGrade) -> f32 {
    match grade {
        ArmWoundGrade::Severed => 1.25,
        _ => 1.0,
    }
}

pub fn is_severed(grade: ArmWoundGrade) -> bool {
    grade == ArmWoundGrade::Severed
}

/// 六维臂伤惩罚因子聚合（决议 §8.1 #2）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArmWoundFactors {
    pub attack_damage_multiplier: f32,
    pub attack_cooldown_multiplier: f32,
    pub block_multiplier: f32,
    pub spread_multiplier: f32,
    pub charge_duration_multiplier: f32,
    pub cast_ticks_multiplier: f32,
    pub main_arm_grade: ArmWoundGrade,
    pub off_arm_grade: ArmWoundGrade,
    pub main_arm_severed: bool,
    pub off_arm_severed: bool,
}

impl Default for ArmWoundFactors {
    fn default() -> Self {
        Self {
            attack_damage_multiplier: 1.0,
            attack_cooldown_multiplier: 1.0,
            block_multiplier: 1.0,
            spread_multiplier: 1.0,
            charge_duration_multiplier: 1.0,
            cast_ticks_multiplier: 1.0,
            main_arm_grade: ArmWoundGrade::Intact,
            off_arm_grade: ArmWoundGrade::Intact,
            main_arm_severed: false,
            off_arm_severed: false,
        }
    }
}

/// 双臂皆伤取各维度最重值不叠乘（决议 §8.1 #2）：主手臂驱动攻击/冷却/散布/蓄力，
/// 副手臂驱动格挡，cast_ticks 取两臂较重者——六个维度各自独立读取，互不相乘。
pub fn combined_factor(wounds: &Wounds) -> ArmWoundFactors {
    let main_grade = worst_wound_grade(wounds, MAIN_ARM);
    let off_grade = worst_wound_grade(wounds, OFF_ARM);
    let cast_grade = worst_of_grades(main_grade, off_grade);

    ArmWoundFactors {
        attack_damage_multiplier: attack_damage_multiplier(main_grade),
        attack_cooldown_multiplier: attack_cooldown_multiplier(main_grade),
        block_multiplier: block_multiplier(off_grade),
        spread_multiplier: spread_multiplier(main_grade),
        charge_duration_multiplier: charge_duration_multiplier(main_grade),
        cast_ticks_multiplier: cast_ticks_multiplier(cast_grade),
        main_arm_grade: main_grade,
        off_arm_grade: off_grade,
        main_arm_severed: is_severed(main_grade),
        off_arm_severed: is_severed(off_grade),
    }
}

pub fn combined_factor_from_optional(wounds: Option<&Wounds>) -> ArmWoundFactors {
    wounds.map(combined_factor).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::components::WoundKind;

    fn wound(location: BodyPart, severity: f32) -> Wound {
        Wound {
            location,
            kind: WoundKind::Blunt,
            severity,
            bleeding_per_sec: 0.0,
            created_at_tick: 0,
            inflicted_by: None,
        }
    }

    // ── 分级边界：与 leg_wound 同构的五级阈值（0/5/15/35/70）─────────────────────

    #[test]
    fn wound_severity_to_grade_boundary_table() {
        assert_eq!(wound_severity_to_grade(0.0), ArmWoundGrade::Intact);
        assert_eq!(wound_severity_to_grade(-5.0), ArmWoundGrade::Intact);
        assert_eq!(wound_severity_to_grade(0.01), ArmWoundGrade::Bruise);
        assert_eq!(wound_severity_to_grade(4.99), ArmWoundGrade::Bruise);
        assert_eq!(wound_severity_to_grade(5.0), ArmWoundGrade::Abrasion);
        assert_eq!(wound_severity_to_grade(14.99), ArmWoundGrade::Abrasion);
        assert_eq!(wound_severity_to_grade(15.0), ArmWoundGrade::Laceration);
        assert_eq!(wound_severity_to_grade(34.99), ArmWoundGrade::Laceration);
        assert_eq!(wound_severity_to_grade(35.0), ArmWoundGrade::Fracture);
        assert_eq!(wound_severity_to_grade(69.99), ArmWoundGrade::Fracture);
        assert_eq!(wound_severity_to_grade(70.0), ArmWoundGrade::Severed);
        assert_eq!(wound_severity_to_grade(1000.0), ArmWoundGrade::Severed);
    }

    // ── 六维惩罚映射表专属 case（决议 §8.1 #2 表逐格核验）──────────────────────

    #[test]
    fn attack_damage_multiplier_matches_decision_table() {
        assert_eq!(attack_damage_multiplier(ArmWoundGrade::Intact), 1.0);
        assert_eq!(attack_damage_multiplier(ArmWoundGrade::Bruise), 0.95);
        assert_eq!(attack_damage_multiplier(ArmWoundGrade::Abrasion), 0.90);
        assert_eq!(attack_damage_multiplier(ArmWoundGrade::Laceration), 0.80);
        assert_eq!(attack_damage_multiplier(ArmWoundGrade::Fracture), 0.60);
        assert_eq!(attack_damage_multiplier(ArmWoundGrade::Severed), 0.40);
    }

    #[test]
    fn attack_cooldown_multiplier_matches_decision_table() {
        assert_eq!(attack_cooldown_multiplier(ArmWoundGrade::Intact), 1.0);
        assert_eq!(attack_cooldown_multiplier(ArmWoundGrade::Bruise), 1.0);
        assert_eq!(attack_cooldown_multiplier(ArmWoundGrade::Abrasion), 1.0);
        assert_eq!(attack_cooldown_multiplier(ArmWoundGrade::Laceration), 1.10);
        assert_eq!(attack_cooldown_multiplier(ArmWoundGrade::Fracture), 1.25);
        assert_eq!(
            attack_cooldown_multiplier(ArmWoundGrade::Severed),
            1.25,
            "Severed 延续 Fracture 冷却惩罚（决议表未显式给出新数字，单调不减）"
        );
    }

    #[test]
    fn block_multiplier_matches_decision_table() {
        assert_eq!(block_multiplier(ArmWoundGrade::Intact), 1.0);
        assert_eq!(block_multiplier(ArmWoundGrade::Bruise), 1.0);
        assert_eq!(block_multiplier(ArmWoundGrade::Abrasion), 1.0);
        assert_eq!(block_multiplier(ArmWoundGrade::Laceration), 0.80);
        assert_eq!(block_multiplier(ArmWoundGrade::Fracture), 0.60);
        assert_eq!(
            block_multiplier(ArmWoundGrade::Severed),
            0.60,
            "Severed 延续 Fracture 格挡惩罚"
        );
    }

    #[test]
    fn spread_multiplier_matches_decision_table() {
        assert_eq!(spread_multiplier(ArmWoundGrade::Intact), 1.0);
        assert_eq!(spread_multiplier(ArmWoundGrade::Bruise), 1.0);
        assert_eq!(spread_multiplier(ArmWoundGrade::Abrasion), 1.0);
        assert_eq!(spread_multiplier(ArmWoundGrade::Laceration), 1.0);
        assert_eq!(spread_multiplier(ArmWoundGrade::Fracture), 1.50);
        assert_eq!(
            spread_multiplier(ArmWoundGrade::Severed),
            1.50,
            "Severed 延续 Fracture 散布惩罚"
        );
    }

    #[test]
    fn charge_duration_multiplier_matches_decision_table() {
        assert_eq!(charge_duration_multiplier(ArmWoundGrade::Intact), 1.0);
        assert_eq!(charge_duration_multiplier(ArmWoundGrade::Bruise), 1.0);
        assert_eq!(charge_duration_multiplier(ArmWoundGrade::Abrasion), 1.0);
        assert_eq!(charge_duration_multiplier(ArmWoundGrade::Laceration), 1.0);
        assert_eq!(charge_duration_multiplier(ArmWoundGrade::Fracture), 1.30);
        assert_eq!(
            charge_duration_multiplier(ArmWoundGrade::Severed),
            1.30,
            "Severed 延续 Fracture 蓄力惩罚"
        );
    }

    #[test]
    fn cast_ticks_multiplier_only_severed_triggers() {
        assert_eq!(cast_ticks_multiplier(ArmWoundGrade::Intact), 1.0);
        assert_eq!(cast_ticks_multiplier(ArmWoundGrade::Bruise), 1.0);
        assert_eq!(cast_ticks_multiplier(ArmWoundGrade::Abrasion), 1.0);
        assert_eq!(cast_ticks_multiplier(ArmWoundGrade::Laceration), 1.0);
        assert_eq!(
            cast_ticks_multiplier(ArmWoundGrade::Fracture),
            1.0,
            "cast_ticks 惩罚只在 Severed 触发（结印手受损专属于断臂），Fracture 不应提前生效"
        );
        assert_eq!(cast_ticks_multiplier(ArmWoundGrade::Severed), 1.25);
    }

    #[test]
    fn is_severed_only_true_for_severed_grade() {
        assert!(!is_severed(ArmWoundGrade::Intact));
        assert!(!is_severed(ArmWoundGrade::Bruise));
        assert!(!is_severed(ArmWoundGrade::Abrasion));
        assert!(!is_severed(ArmWoundGrade::Laceration));
        assert!(!is_severed(ArmWoundGrade::Fracture));
        assert!(is_severed(ArmWoundGrade::Severed));
    }

    // ── worst_wound_grade：按 body part 过滤 + 取最重 ───────────────────────────

    #[test]
    fn worst_wound_grade_filters_by_body_part_and_takes_max() {
        let wounds = Wounds {
            entries: vec![
                wound(BodyPart::ArmR, 3.0),  // Bruise
                wound(BodyPart::ArmR, 40.0), // Fracture — 应被取中
                wound(BodyPart::ArmL, 90.0), // 不同部位，不应影响 ArmR 结果
                wound(BodyPart::Chest, 99.0),
            ],
            ..Default::default()
        };

        assert_eq!(
            worst_wound_grade(&wounds, BodyPart::ArmR),
            ArmWoundGrade::Fracture
        );
        assert_eq!(
            worst_wound_grade(&wounds, BodyPart::ArmL),
            ArmWoundGrade::Severed
        );
    }

    #[test]
    fn worst_wound_grade_defaults_to_intact_when_no_matching_wounds() {
        let wounds = Wounds {
            entries: vec![wound(BodyPart::Chest, 99.0)],
            ..Default::default()
        };

        assert_eq!(
            worst_wound_grade(&wounds, BodyPart::ArmL),
            ArmWoundGrade::Intact
        );
        assert_eq!(
            worst_wound_grade(&wounds, BodyPart::ArmR),
            ArmWoundGrade::Intact
        );
    }

    // ── combined_factor：主/副手臂分工 + 双臂皆伤取最重不叠乘 ───────────────────

    #[test]
    fn combined_factor_defaults_to_neutral_when_wounds_empty() {
        let factors = combined_factor(&Wounds::default());
        assert_eq!(factors, ArmWoundFactors::default());
    }

    #[test]
    fn combined_factor_from_optional_none_is_neutral() {
        assert_eq!(
            combined_factor_from_optional(None),
            ArmWoundFactors::default()
        );
    }

    #[test]
    fn combined_factor_main_arm_drives_attack_cooldown_spread_charge() {
        // 决议 §8.1 #2：主手臂（ArmR）Fracture 驱动攻击/冷却/散布/蓄力四维；
        // 副手臂（ArmL）Intact（无伤）不影响这四维。
        let wounds = Wounds {
            entries: vec![wound(BodyPart::ArmR, 40.0)], // Fracture on MAIN_ARM
            ..Default::default()
        };
        let factors = combined_factor(&wounds);

        assert_eq!(factors.main_arm_grade, ArmWoundGrade::Fracture);
        assert_eq!(factors.off_arm_grade, ArmWoundGrade::Intact);
        assert_eq!(factors.attack_damage_multiplier, 0.60);
        assert_eq!(factors.attack_cooldown_multiplier, 1.25);
        assert_eq!(factors.spread_multiplier, 1.50);
        assert_eq!(factors.charge_duration_multiplier, 1.30);
        // 副手臂完好 → 格挡不受影响。
        assert_eq!(
            factors.block_multiplier, 1.0,
            "副手臂(ArmL)完好，主手臂(ArmR)受伤不应影响格挡维度（各维度独立读取）"
        );
    }

    #[test]
    fn combined_factor_off_arm_drives_block_only() {
        // 决议 §8.1 #2：副手臂（ArmL）Fracture 只驱动格挡维度；主手臂完好，
        // 攻击/冷却/散布/蓄力四维应保持中性 1.0（不被副手臂伤势波及）。
        let wounds = Wounds {
            entries: vec![wound(BodyPart::ArmL, 40.0)], // Fracture on OFF_ARM
            ..Default::default()
        };
        let factors = combined_factor(&wounds);

        assert_eq!(factors.block_multiplier, 0.60);
        assert_eq!(
            factors.attack_damage_multiplier, 1.0,
            "主手臂(ArmR)完好，副手臂(ArmL)受伤不应影响攻击伤害维度"
        );
        assert_eq!(factors.attack_cooldown_multiplier, 1.0);
        assert_eq!(factors.spread_multiplier, 1.0);
        assert_eq!(factors.charge_duration_multiplier, 1.0);
    }

    #[test]
    fn combined_factor_both_arms_wounded_takes_worst_per_dimension_not_multiplied() {
        // 决议 §8.1 #2 核心断言："双臂皆伤取各维度最重值不叠乘"——
        // 主手臂 Fracture(0.60) + 副手臂 Fracture(0.60) 时，攻击倍率必须仍是 0.60
        // （只读主手臂），不能变成 0.60*0.60=0.36（叠乘）。格挡倍率同理只读副手臂。
        let wounds = Wounds {
            entries: vec![
                wound(BodyPart::ArmR, 40.0), // Fracture
                wound(BodyPart::ArmL, 40.0), // Fracture
            ],
            ..Default::default()
        };
        let factors = combined_factor(&wounds);

        assert_eq!(
            factors.attack_damage_multiplier, 0.60,
            "双臂皆 Fracture 时攻击倍率应仍是主手臂单独的 0.60，不叠乘为 0.36"
        );
        assert_eq!(
            factors.block_multiplier, 0.60,
            "双臂皆 Fracture 时格挡倍率应仍是副手臂单独的 0.60，不叠乘为 0.36"
        );
    }

    #[test]
    fn combined_factor_cast_ticks_reads_worst_of_both_arms() {
        // cast_ticks（结印手受损）读两臂中较重者，不偏主副手。
        // Case A：只有副手臂 Severed → cast_ticks 仍应触发（不是只看主手臂）。
        let off_severed = Wounds {
            entries: vec![wound(BodyPart::ArmL, 75.0)], // Severed on OFF_ARM
            ..Default::default()
        };
        assert_eq!(
            combined_factor(&off_severed).cast_ticks_multiplier,
            1.25,
            "副手臂断裂也应触发 cast_ticks 惩罚（结印需要双手，不偏主副手）"
        );

        // Case B：只有主手臂 Severed → cast_ticks 同样触发。
        let main_severed = Wounds {
            entries: vec![wound(BodyPart::ArmR, 75.0)], // Severed on MAIN_ARM
            ..Default::default()
        };
        assert_eq!(combined_factor(&main_severed).cast_ticks_multiplier, 1.25);

        // Case C：两臂都只是 Bruise → cast_ticks 不触发。
        let both_bruise = Wounds {
            entries: vec![wound(BodyPart::ArmR, 2.0), wound(BodyPart::ArmL, 2.0)],
            ..Default::default()
        };
        assert_eq!(combined_factor(&both_bruise).cast_ticks_multiplier, 1.0);
    }

    #[test]
    fn combined_factor_severed_flags_track_each_arm_independently() {
        let wounds = Wounds {
            entries: vec![
                wound(BodyPart::ArmR, 75.0), // Severed on MAIN_ARM
                wound(BodyPart::ArmL, 10.0), // Abrasion on OFF_ARM (not severed)
            ],
            ..Default::default()
        };
        let factors = combined_factor(&wounds);

        assert!(factors.main_arm_severed);
        assert!(!factors.off_arm_severed);
    }

    #[test]
    fn combined_factor_all_five_grades_via_main_arm_severity_sweep() {
        // 五级分级 × 主手臂维度全覆盖：确保每一级都能被正确产出（不止 boundary 两端）。
        let cases = [
            (2.0, ArmWoundGrade::Bruise, 0.95),
            (10.0, ArmWoundGrade::Abrasion, 0.90),
            (20.0, ArmWoundGrade::Laceration, 0.80),
            (50.0, ArmWoundGrade::Fracture, 0.60),
            (80.0, ArmWoundGrade::Severed, 0.40),
        ];
        for (severity, expected_grade, expected_damage_mul) in cases {
            let wounds = Wounds {
                entries: vec![wound(BodyPart::ArmR, severity)],
                ..Default::default()
            };
            let factors = combined_factor(&wounds);
            assert_eq!(
                factors.main_arm_grade, expected_grade,
                "severity={severity} 应映射到 {expected_grade:?}"
            );
            assert_eq!(
                factors.attack_damage_multiplier, expected_damage_mul,
                "severity={severity} ({expected_grade:?}) 的攻击倍率应为 {expected_damage_mul}"
            );
        }
    }
}
