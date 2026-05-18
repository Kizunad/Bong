//! plan-baomai-v4 §7 — 事件类型汇总（P0-P2 部分）。

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Entity, Event};

use super::iron_cocoon::IronCocoonStage;
use super::scar_circuit::ScarCircuitKind;

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
