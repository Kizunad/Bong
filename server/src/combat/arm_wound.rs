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
//!
//! ## plan-race-system-v1 P0b —— 主/副手臂身份判定改为 PartConsequence 分发
//!
//! `MAIN_ARM`/`OFF_ARM` 两个 `pub const BodyPart` 因外部既有调用点
//! （`combat/needle.rs`、`combat/anqi_v2.rs`、`combat/resolve.rs` 均以编译期字面量
//! 使用）而保留——Rust `const` 无法承载"运行时查表得到的 BodyPart"，且改这两处的类型
//! 会牵动本 work item 声明范围外的文件。真正泛化的是 [`combined_factor`] 的**内部
//! 分发逻辑**：不再假设"`ArmL`/`ArmR` 这两个 enum 变体就是双臂"，而是查询
//! **调用方按目标实体解析出的** `&BodyPlan` 的 `PartConsequence::Manipulator{main_hand}`
//! 标签（`assets/body_plans/plans/humanoid.json`）——`main_off_arm_consts_match_humanoid_plan_manipulator_consequence`
//! pin 测试锁死 `MAIN_ARM`/`OFF_ARM` 与该数据表一致，数值行为 bit-for-bit 不变。
//!
//! ## plan-race-system-v1 P0 review 修复 —— `combined_factor` 不再固定读 humanoid 单例
//!
//! [`combined_factor`] / [`combined_factor_from_optional`] 现在接受一个 `plan: &BodyPlan`
//! 参数，由**生产调用点**（`combat/resolve.rs` 攻方伤害倍率 / 防方格挡倍率、
//! `combat/anqi_v2.rs::multi_shot_cone_degrees`、`combat/needle.rs::resolve_shoot_needle_intents`）
//! 经 [`crate::body_plan::resolve_body_plan_for_target`] 按**目标实体**（施法者/受击者）
//! 解析后传入——不再无条件绑死 [`crate::body_plan::humanoid_plan_static`]。资源缺失
//! （大量既有单测未插入 `BodyPlanRegistry`/`RaceRegistry`）时 `resolve_body_plan_for_target`
//! 优雅退化到 `humanoid_plan_static()`，humanoid 行为 bit-for-bit 不变；非人形实体
//! （携带非 humanoid `BodyPlan` 的实体）现在会真的按自己的 `PartConsequence` 标签分发，
//! 不再被静默按人形规则误判。
//!
//! ## Severed 行为级后果落地状态（决议 §8.1 #2 原文额外强调的两条）
//!
//! 决议对 Severed 分级除六维倍率外，还点名两条"行为级"后果——本节显式记录两条各自
//! 的落地状态，避免 `main_arm_severed`/`off_arm_severed` 这两个 bool 字段被算出来却
//! 无人读取的静默孤岛（这两个字段本身只是"读点"，真正的后果必须由消费端接住）：
//!
//! - **① 断臂脱手落地：已实现。** `combat/resolve.rs::resolve_attack_intents`
//!   在命中结算后检查 `hit_probe.body_part == MAIN_ARM &&
//!   is_severed(wound_severity_to_grade(wound_severity))`——本次命中让主手臂
//!   （持械侧）直接判定为 Severed 时，读取目标 entity 当前的 `Weapon` component
//!   （若其 `slot == EquipSlot::MainHand`），把对应 instance 经
//!   `inventory::discard_inventory_item_to_dropped_loot` 送入
//!   `DroppedLootRegistry`（世界掉落，走既有掉落物链路，与武器耐久归零脱手同构但不
//!   走"塞回随身容器"分支——断臂后没有手可以把武器放回背包），并 `remove::<Weapon>()`
//!   清空该 entity 的武器 runtime component。副手臂(OFF_ARM) Severed **不**触发此
//!   逻辑（副手臂对应盾/副手件，决议原文"脱手落地"只点名持械侧）。
//! - **② 禁双持：本 PR 暂缓，非静默——原因如下。** 决议原文"无法双手持械"字面上要求
//!   在**装备时**校验：副手臂(OFF_ARM) Severed 时禁止把武器件（`OffHandTypeMismatch`
//!   规则允许的 dagger/fist 类武器）装入 off_hand 槽。但装备校验入口
//!   `inventory::apply_inventory_move` / `inventory::validate_move_semantics` 的签名
//!   只接受 `PlayerInventory` + `ItemRegistry` + 位置参数，**完全不接触 combat 侧的
//!   `Wounds` component**——要让装备校验感知臂伤，需要把 `Entity`/`Wounds` 一路从
//!   network handler 穿透到 inventory 校验层（跨模块签名变更，波及
//!   `client_request_handler` 及所有直接调用 `apply_inventory_move` 的既有测试），
//!   属于本 work item 声明范围外的重装备重构，不在本 PR 干净落地。
//!   `ArmWoundFactors.off_arm_severed` 字段因此暂时**只读不消费**——这是显式记录的
//!   已知缺口而非遗忘：后续 PR 若要补齐，落点应是给 `apply_inventory_move`（或其上游
//!   network handler）加一个可选的"当前臂伤态"输入，命中"目标槽=weapon 类 && 对侧
//!   OFF_ARM already Severed"时拒绝，返回新增的 `InventoryMoveRejectReason` 变体。

use crate::body_plan::{BodyPartId, BodyPlan};
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

/// 给定部位 id，取该部位所有伤口中最严重的分级。镜像
/// `movement::leg_wound::worst_wound_grade` 签名与语义。
///
/// plan-race-system-v1 P0 review r2（BLOCKING-2 收口）—— 参数从 legacy `BodyPart` 迁移
/// 为 `&BodyPartId`：`Wound.location` 本身已经是 `BodyPartId`，直接比较部位 id 不再需要
/// 反压 legacy enum 才能过滤。
pub fn worst_wound_grade(wounds: &Wounds, part_id: &BodyPartId) -> ArmWoundGrade {
    wounds
        .entries
        .iter()
        .filter(|wound| &wound.location == part_id)
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
    /// 决议 §8.1 #2 行为级后果 ①（已实现，见模块顶部文档"Severed 行为级后果落地状态"）：
    /// `combat/resolve.rs::resolve_attack_intents` 命中当拍即读该标志同等条件
    /// （`hit_probe.body_part == MAIN_ARM && is_severed(...)`）触发断臂脱手落地——
    /// 此字段本身不驱动任何逻辑，只是暴露给下游只读判定用（如 UI/日志），真正触发点
    /// 是 resolve.rs 里对命中部位与本次伤势的直接重算，不经由本字段回读。
    pub main_arm_severed: bool,
    /// 决议 §8.1 #2 行为级后果 ②"禁双持"（**本 PR 暂缓，非静默** —— 见模块顶部文档
    /// "Severed 行为级后果落地状态"条目 ② 的完整理由：装备校验入口
    /// `inventory::apply_inventory_move` 不接触 `Wounds`，需要跨模块签名重构才能让
    /// 装备时校验读到这个标志，超出本 work item 声明范围）。当前**零消费**——后续 PR
    /// 补齐前，副手臂断裂不会阻止装备武器件到 off_hand。
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

/// plan-race-system-v1 P0 review r2（BLOCKING-2 收口）—— 按
/// `PartConsequence::Manipulator{main_hand}` 标签查询**调用方传入的** `&BodyPlan`
/// （不再固定读 `humanoid_plan_static`），取该档全部部位中最重的伤势分级（humanoid plan
/// 每档恰好一个部位——`arm_r`=`main_hand:true`/`arm_l`=`main_hand:false`——故本函数对
/// humanoid plan 的结果与旧版直接 `worst_wound_grade(wounds, MAIN_ARM/OFF_ARM)`
/// bit-for-bit 相同；泛化后天然支持非人形 plan 声明多个同档 Manipulator 部位，或把
/// Manipulator 标签挂在完全不同的部位 id 上）。
///
/// 改用 `BodyPlan::parts_matching`（直接产出部位 id）取代
/// `legacy::legacy_body_parts_matching`（产出 legacy `BodyPart`，会在遇到非人形部位 id
/// 时静默跳过——`Wound.location` 已经是 `BodyPartId`，不再需要这层 legacy 反压）。
fn manipulator_worst_grade(wounds: &Wounds, main_hand: bool, plan: &BodyPlan) -> ArmWoundGrade {
    plan.parts_matching(move |consequence| {
        matches!(consequence, crate::body_plan::PartConsequence::Manipulator { main_hand: mh } if *mh == main_hand)
    })
    .map(|part_id| worst_wound_grade(wounds, part_id))
    .max_by_key(|grade| grade_rank(*grade))
    .unwrap_or(ArmWoundGrade::Intact)
}

/// 双臂皆伤取各维度最重值不叠乘（决议 §8.1 #2）：主手臂驱动攻击/冷却/散布/蓄力，
/// 副手臂驱动格挡，cast_ticks 取两臂较重者——六个维度各自独立读取，互不相乘。
///
/// `plan` 必须是**目标实体**经 [`crate::body_plan::resolve_body_plan_for_target`]
/// 解析出的 `BodyPlanPurpose::Intrinsic` 结果——生产调用点不得再传入固定的
/// `humanoid_plan_static()`（见模块顶部"P0 review 修复"文档）。
pub fn combined_factor(wounds: &Wounds, plan: &BodyPlan) -> ArmWoundFactors {
    let main_grade = manipulator_worst_grade(wounds, true, plan);
    let off_grade = manipulator_worst_grade(wounds, false, plan);
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

pub fn combined_factor_from_optional(wounds: Option<&Wounds>, plan: &BodyPlan) -> ArmWoundFactors {
    wounds
        .map(|wounds| combined_factor(wounds, plan))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body_plan::{BodyPartDef, PartConsequence};
    use crate::combat::components::WoundKind;

    // ── plan-race-system-v1 P0b：MAIN_ARM/OFF_ARM 编译期字面量必须与 humanoid.json 的
    // PartConsequence 数据一致——防止未来有人只改 JSON 忘了 JSON 与硬编码 const 已经脱节 ──

    #[test]
    fn main_off_arm_consts_match_humanoid_plan_manipulator_consequence() {
        let plan = crate::body_plan::humanoid_plan_static();
        let main_id = crate::body_plan::legacy_body_part_to_id(MAIN_ARM);
        let off_id = crate::body_plan::legacy_body_part_to_id(OFF_ARM);
        let main_def = plan
            .parts
            .iter()
            .find(|def| def.id == main_id)
            .expect("humanoid.json must declare MAIN_ARM's part id");
        let off_def = plan
            .parts
            .iter()
            .find(|def| def.id == off_id)
            .expect("humanoid.json must declare OFF_ARM's part id");
        assert_eq!(
            main_def.consequence,
            PartConsequence::Manipulator { main_hand: true },
            "MAIN_ARM const ({MAIN_ARM:?}) must be tagged Manipulator{{main_hand:true}} in humanoid.json"
        );
        assert_eq!(
            off_def.consequence,
            PartConsequence::Manipulator { main_hand: false },
            "OFF_ARM const ({OFF_ARM:?}) must be tagged Manipulator{{main_hand:false}} in humanoid.json"
        );
    }

    #[test]
    fn manipulator_worst_grade_ignores_non_manipulator_wounds() {
        // Chest/Leg 伤势不应被 main_hand 分发误纳入——即使严重度拉满。
        let wounds = Wounds {
            entries: vec![
                wound(BodyPart::Chest, 99.0),
                wound(BodyPart::LegL, 99.0),
                wound(BodyPart::LegR, 99.0),
            ],
            ..Default::default()
        };
        let plan = crate::body_plan::humanoid_plan_static();
        assert_eq!(
            manipulator_worst_grade(&wounds, true, plan),
            ArmWoundGrade::Intact
        );
        assert_eq!(
            manipulator_worst_grade(&wounds, false, plan),
            ArmWoundGrade::Intact
        );
    }

    fn wound(location: BodyPart, severity: f32) -> Wound {
        Wound {
            location: crate::body_plan::legacy_body_part_to_id(location),
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
            worst_wound_grade(
                &wounds,
                &crate::body_plan::legacy_body_part_to_id(BodyPart::ArmR)
            ),
            ArmWoundGrade::Fracture
        );
        assert_eq!(
            worst_wound_grade(
                &wounds,
                &crate::body_plan::legacy_body_part_to_id(BodyPart::ArmL)
            ),
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
            worst_wound_grade(
                &wounds,
                &crate::body_plan::legacy_body_part_to_id(BodyPart::ArmL)
            ),
            ArmWoundGrade::Intact
        );
        assert_eq!(
            worst_wound_grade(
                &wounds,
                &crate::body_plan::legacy_body_part_to_id(BodyPart::ArmR)
            ),
            ArmWoundGrade::Intact
        );
    }

    // ── combined_factor：主/副手臂分工 + 双臂皆伤取最重不叠乘 ───────────────────

    #[test]
    fn combined_factor_defaults_to_neutral_when_wounds_empty() {
        let factors = combined_factor(&Wounds::default(), crate::body_plan::humanoid_plan_static());
        assert_eq!(factors, ArmWoundFactors::default());
    }

    #[test]
    fn combined_factor_from_optional_none_is_neutral() {
        assert_eq!(
            combined_factor_from_optional(None, crate::body_plan::humanoid_plan_static()),
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
        let factors = combined_factor(&wounds, crate::body_plan::humanoid_plan_static());

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
        let factors = combined_factor(&wounds, crate::body_plan::humanoid_plan_static());

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
        let factors = combined_factor(&wounds, crate::body_plan::humanoid_plan_static());

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
            combined_factor(&off_severed, crate::body_plan::humanoid_plan_static())
                .cast_ticks_multiplier,
            1.25,
            "副手臂断裂也应触发 cast_ticks 惩罚（结印需要双手，不偏主副手）"
        );

        // Case B：只有主手臂 Severed → cast_ticks 同样触发。
        let main_severed = Wounds {
            entries: vec![wound(BodyPart::ArmR, 75.0)], // Severed on MAIN_ARM
            ..Default::default()
        };
        assert_eq!(
            combined_factor(&main_severed, crate::body_plan::humanoid_plan_static())
                .cast_ticks_multiplier,
            1.25
        );

        // Case C：两臂都只是 Bruise → cast_ticks 不触发。
        let both_bruise = Wounds {
            entries: vec![wound(BodyPart::ArmR, 2.0), wound(BodyPart::ArmL, 2.0)],
            ..Default::default()
        };
        assert_eq!(
            combined_factor(&both_bruise, crate::body_plan::humanoid_plan_static())
                .cast_ticks_multiplier,
            1.0
        );
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
        let factors = combined_factor(&wounds, crate::body_plan::humanoid_plan_static());

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
            let factors = combined_factor(&wounds, crate::body_plan::humanoid_plan_static());
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

    // ── plan-race-system-v1 P0 review 修复：`combined_factor` 必须真的按调用方传入的
    // `plan` 分发，而不是内部悄悄查 `humanoid_plan_static()`（BLOCKING-1）───────────

    /// 合成一个"外星"身体构型：主手臂 `Manipulator{main_hand:true}` 标签挂在 legacy
    /// `LegR` 上而不是 humanoid.json 的 `ArmR`，`ArmR` 本身在这个构型里退化成 `Core`
    /// （不再是操作肢体）。用于证明 [`combined_factor`] 的结果完全来自**调用时传入的
    /// `plan` 参数**，不是从 `humanoid_plan_static()` 硬读、也不是从 `BodyPart::ArmR`
    /// 这个 legacy enum 变体本身推断出"这就是主手臂"。
    fn alien_manipulator_plan() -> BodyPlan {
        BodyPlan {
            id: crate::body_plan::BodyPlanId::new("test_alien_manipulator"),
            display_name: "测试用外星构型".to_string(),
            is_humanoid: false,
            parts: vec![
                BodyPartDef {
                    id: crate::body_plan::legacy_body_part_to_id(BodyPart::ArmR),
                    damage_mul: 1.0,
                    contam_mul: 1.0,
                    bleed_mul: 1.0,
                    consequence: PartConsequence::Core,
                },
                BodyPartDef {
                    id: crate::body_plan::legacy_body_part_to_id(BodyPart::LegR),
                    damage_mul: 1.0,
                    contam_mul: 1.0,
                    bleed_mul: 1.0,
                    consequence: PartConsequence::Manipulator { main_hand: true },
                },
                BodyPartDef {
                    id: crate::body_plan::legacy_body_part_to_id(BodyPart::ArmL),
                    damage_mul: 1.0,
                    contam_mul: 1.0,
                    bleed_mul: 1.0,
                    consequence: PartConsequence::Manipulator { main_hand: false },
                },
            ],
            hit_geometry: crate::body_plan::humanoid_plan_static()
                .hit_geometry
                .clone(),
            equip_slots: vec![],
            meridian_profile: None,
            mutation_slot_mapping: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn combined_factor_reads_manipulator_tag_from_supplied_plan_not_humanoid_static() {
        let alien = alien_manipulator_plan();

        // ArmR Severed：humanoid.json 把 ArmR 标为主手臂，但这个外星构型把 ArmR 标成
        // Core——伤害倍率必须保持中性 1.0，证明不是硬编码"ArmR 就是主手臂"。
        let arm_r_severed = Wounds {
            entries: vec![wound(BodyPart::ArmR, 75.0)],
            ..Default::default()
        };
        let factors_arm_r = combined_factor(&arm_r_severed, &alien);
        assert_eq!(
            factors_arm_r.attack_damage_multiplier, 1.0,
            "外星构型里 ArmR 是 Core 非 Manipulator，ArmR 断裂不应触发主手臂伤害惩罚"
        );
        assert!(
            !factors_arm_r.main_arm_severed,
            "ArmR 在外星构型里不是主手臂，不应被标记为 main_arm_severed"
        );

        // LegR Severed：外星构型把 LegR 标成主手臂 Manipulator{main_hand:true}——
        // 伤害倍率必须真的按 Severed 分级打到 0.40，证明结果来自传入 plan 的数据，
        // 而不是 humanoid_plan_static()（humanoid.json 里 LegR 是 Locomotion，与
        // arm_wound 毫无关系）。
        let leg_r_severed = Wounds {
            entries: vec![wound(BodyPart::LegR, 75.0)],
            ..Default::default()
        };
        let factors_leg_r = combined_factor(&leg_r_severed, &alien);
        assert_eq!(
            factors_leg_r.main_arm_grade,
            ArmWoundGrade::Severed,
            "外星构型的主手臂标签落在 LegR 上，LegR 断裂必须被判定为主手臂 Severed"
        );
        assert_eq!(
            factors_leg_r.attack_damage_multiplier, 0.40,
            "主手臂（本构型里是 LegR）Severed 应打到 0.40 倍攻击伤害"
        );
        assert!(
            factors_leg_r.main_arm_severed,
            "LegR 在外星构型里是主手臂，Severed 后必须标记 main_arm_severed"
        );

        // 同一份伤口喂给 humanoid_plan_static() 时行为必须完全不同（LegR 在 humanoid
        // plan 里只挂 Locomotion，不影响任何臂伤维度）——同一个 Wounds 值、不同 plan
        // 产出不同结果，锁死"结果随 plan 数据变化"这条核心断言，而非源自 humanoid
        // 静态表或 BodyPart 枚举变体本身。
        let factors_leg_r_humanoid =
            combined_factor(&leg_r_severed, crate::body_plan::humanoid_plan_static());
        assert_eq!(
            factors_leg_r_humanoid.attack_damage_multiplier, 1.0,
            "同一伤口在 humanoid plan 下 LegR 不是 Manipulator，不应触发臂伤惩罚——\
             与外星构型的 0.40 形成对照，证明 combined_factor 结果随 plan 数据变化"
        );
        assert!(!factors_leg_r_humanoid.main_arm_severed);
    }
}
