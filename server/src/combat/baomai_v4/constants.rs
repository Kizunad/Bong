//! plan-baomai-v4 gameplay 常数。
//!
//! 仅放 gameplay 参数；涉及真元/经脉物理的常数在 `qi_physics::constants` 中。
//! P3-P5 常数前置声明，后续 PR 消费——允许暂时 dead_code。
#![allow(dead_code)]
// Inner attribute valid: this file is a module root (constants.rs as `mod constants`).

// ── 活茧（Iron Cocoon）──
/// 四阶活茧的过载次数阈值：[茧皮, 茧骨, 茧肉, 茧灵]。
pub const IRON_COCOON_THRESHOLDS: [u32; 4] = [50, 120, 250, 500];

// ── 互噬（Resonance Lock）──
/// 共振锁定触发距离（blocks）。
pub const RESONANCE_LOCK_RANGE: f64 = 2.0;
/// 每次互中的共振计量基础增量。
pub const RESONANCE_METER_HIT_BASE: f64 = 0.12;
/// 共振锁定持续时间（ticks，20 tps x 3s）。
pub const RESONANCE_LOCK_DURATION_TICKS: u64 = 60;
/// 共振触发最低过载次数门槛。
pub const RESONANCE_SCAR_THRESHOLD: u32 = 40;

// ── 裂读（Crack Reading）──
/// 每次过载增加的裂读成功率。
pub const CRACK_READING_RATE_PER_OVERLOAD: f64 = 0.004;
/// 裂读成功率上限。
pub const CRACK_READING_RATE_CAP: f64 = 0.50;
/// 裂读结果在 HUD 上的持续时间（ticks）。
pub const CRACK_READING_DISPLAY_TICKS: u64 = 60;
/// 深读窗口时间（ticks）——连续命中同一目标时触发深读。
pub const CRACK_READING_DEEP_WINDOW_TICKS: u64 = 60;

// ── 疤纹回路（Scar Circuit）──
/// 形成疤纹回路最低过载次数。
pub const SCAR_CIRCUIT_MIN_OVERLOADS: u32 = 5;
/// 疤纹回路检查周期（ticks）。
pub const SCAR_CIRCUIT_CHECK_INTERVAL_TICKS: u64 = 40;

// ── 死脉甲（Dead Meridian Armor）──
/// 主动绝脉最低过载次数。
pub const VOLUNTARY_SEVER_MIN_OVERLOADS: u32 = 80;
/// 主动绝脉最低 mastery 等级。
pub const VOLUNTARY_SEVER_MIN_MASTERY: f32 = 60.0;
/// 主动绝脉目标经脉最高 integrity。
pub const VOLUNTARY_SEVER_MAX_INTEGRITY: f64 = 0.70;
/// 主动绝脉引导时间（ticks）。
pub const VOLUNTARY_SEVER_CAST_TICKS: u64 = 100;
/// 主动绝脉战斗冷却期（ticks）——3s 内无 CombatEvent。
pub const VOLUNTARY_SEVER_COMBAT_COOLDOWN_TICKS: u64 = 60;

// ── 疤纹回路被动效果数值 ──
/// 虎口回路：崩拳伤害转 contamination 的比例。
pub const TIGER_MOUTH_CONTAM_RATIO: f64 = 0.08;
/// 三阳合流：近战 reach 加成（blocks）。
pub const TRIPLE_YANG_REACH_BONUS: f64 = 0.3;
/// 心肺短路：焚血期间 qi regen 倍率。
pub const HEART_LUNG_QI_REGEN_MULTIPLIER: f64 = 1.15;
/// 肝肾交汇：contamination 排毒速率倍率。
pub const LIVER_KIDNEY_CONTAM_PURGE_MULTIPLIER: f64 = 1.10;
/// 脾肾固本：安全区自愈速率倍率。
pub const SPLEEN_KIDNEY_HEALING_MULTIPLIER: f64 = 1.20;
/// 任督桥：全力充能速率倍率。
pub const REN_DU_CHARGE_RATE_MULTIPLIER: f64 = 1.25;
/// 任督桥：充能被打断时 Ren/Du integrity 惩罚。
pub const REN_DU_INTERRUPT_INTEGRITY_PENALTY: f64 = 0.03;

// ── 活茧被动效果数值 ──
/// 茧皮：BRUISE 伤害阈值倍率。
pub const TOUGH_SKIN_BRUISE_THRESHOLD_MULTIPLIER: f64 = 1.25;
/// 茧骨：FRACTURE 降级为 LACERATION 的概率。
pub const IRON_BONE_FRACTURE_DOWNGRADE_CHANCE: f64 = 0.20;
/// 茧灵：有活跃回路的经脉 flow_rate 加成倍率。
pub const SCAR_FORGED_FLOW_RATE_BONUS: f64 = 1.05;
