//! plan-baomai-v4 §7 — 事件类型汇总（P0-P4 部分）。

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Entity, Event};

use super::crack_reading::CrackReadingResult;
use super::iron_cocoon::IronCocoonStage;
use super::scar_circuit::ScarCircuitKind;
use crate::cultivation::components::MeridianId;

/// 疤纹回路形成事件。
///
/// 消费方：agent narration（ScarCircuitFormed → player perception scope）。
#[derive(Debug, Clone, Event)]
#[allow(dead_code)]
pub struct ScarCircuitFormedEvent {
    pub entity: Entity,
    pub circuit: ScarCircuitKind,
    pub tick: u64,
}

/// 疤纹回路断裂原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CircuitBreakReason {
    /// 经脉修复超过 INTEGRITY_MAX。
    Healed,
    /// 经脉损伤低于 INTEGRITY_MIN。
    Deepened,
    /// 经脉被永久 SEVERED。
    Severed,
    /// 经脉被关闭（opened = false）。
    Closed,
}

/// 疤纹回路断裂事件。
///
/// 消费方：agent narration（ScarCircuitBroken → player perception scope）。
#[derive(Debug, Clone, Event)]
#[allow(dead_code)]
pub struct ScarCircuitBrokenEvent {
    pub entity: Entity,
    pub circuit: ScarCircuitKind,
    pub reason: CircuitBreakReason,
    pub tick: u64,
}

/// 活茧阶段提升事件。
///
/// 消费方：agent narration + client event_flow（IronCocoonStageUp → player perception scope）。
#[derive(Debug, Clone, Event)]
#[allow(dead_code)]
pub struct IronCocoonStageUpEvent {
    pub entity: Entity,
    pub from: IronCocoonStage,
    pub to: IronCocoonStage,
    pub total_overloads: u32,
    pub tick: u64,
}

/// 裂读结果事件（plan-baomai-v4 §5.3）。
///
/// 消费方：client HUD overlay（`bong:crack_reading` CustomPayload）。
#[derive(Debug, Clone, Event)]
#[allow(dead_code)]
pub struct CrackReadingResultEvent {
    pub reader: Entity,
    pub target: Entity,
    pub result: CrackReadingResult,
    pub tick: u64,
}

/// 主动绝脉事件（plan-baomai-v4 §4.2）。
///
/// 消费方：`MeridianSeveredPermanent` 写入 + `DeadMeridianArmor` 更新 + agent narration。
#[derive(Debug, Clone, Event)]
#[allow(dead_code)]
pub struct VoluntarySeverEvent {
    pub entity: Entity,
    pub meridian: MeridianId,
    pub tick: u64,
}

/// 共振锁定结束原因（plan-baomai-v4 §6.5）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LockEndReason {
    /// 60 tick 自然到期。
    Expired,
    /// 一方先移出 RESONANCE_LOCK_RANGE。Entity 是先脱离者。
    Retreated(Entity),
    /// 一方死亡。Entity 是死亡者。
    Death(Entity),
}

/// 共振锁定开始事件（plan-baomai-v4 §6.2/§7）。
///
/// 消费方：client VFX + HUD meter（`bong:resonance_lock` CustomPayload）+ agent narration。
#[derive(Debug, Clone, Event)]
#[allow(dead_code)]
pub struct ResonanceLockEvent {
    pub fighter_a: Entity,
    pub fighter_b: Entity,
    pub started_at: u64,
    pub ends_at: u64,
}

/// 共振锁定结束事件（plan-baomai-v4 §6.5/§7）。
///
/// 消费方：client VFX + HUD meter + agent narration。
#[derive(Debug, Clone, Event)]
#[allow(dead_code)]
pub struct ResonanceLockEndEvent {
    pub fighter_a: Entity,
    pub fighter_b: Entity,
    pub reason: LockEndReason,
    pub tick: u64,
}
