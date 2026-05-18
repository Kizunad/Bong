//! plan-baomai-v4 §2 — 疤纹回路（Scar Circuit）。
//!
//! 当两条相邻正经都处于 MICRO_TEAR 状态（integrity ∈ [SCAR_CIRCUIT_INTEGRITY_MIN,
//! SCAR_CIRCUIT_INTEGRITY_MAX]），裂缝外溢的微量真元形成次级通路。

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component, Entity, EventWriter, Query, Res};

use crate::combat::baomai_v4::constants::{
    SCAR_CIRCUIT_CHECK_INTERVAL_TICKS, SCAR_CIRCUIT_MIN_OVERLOADS,
};
use crate::combat::baomai_v4::events::{
    CircuitBreakReason, ScarCircuitBrokenEvent, ScarCircuitFormedEvent,
};
use crate::combat::baomai_v4::scar_history::ScarHistory;
use crate::combat::components::DerivedAttrs;
use crate::combat::CombatClock;
use crate::cultivation::components::{MeridianId, MeridianSystem};
use crate::cultivation::meridian::severed::MeridianSeveredPermanent;
use crate::qi_physics::constants::{SCAR_CIRCUIT_INTEGRITY_MAX, SCAR_CIRCUIT_INTEGRITY_MIN};

use super::constants::{
    HEART_LUNG_QI_REGEN_MULTIPLIER, LIVER_KIDNEY_CONTAM_PURGE_MULTIPLIER,
    SPLEEN_KIDNEY_HEALING_MULTIPLIER, TRIPLE_YANG_REACH_BONUS,
};

/// 7 种疤纹回路——每种由一对邻接经脉组成。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ScarCircuitKind {
    /// 虎口回路 LI↔SI（右臂）
    TigerMouth,
    /// 三阳合流 SI↔TE（右臂）
    TripleYang,
    /// 心肺短路 LU↔HT（左臂）
    HeartLung,
    /// 肝肾交汇 KI↔LR（左腿）
    LiverKidney,
    /// 胆胃通路 ST↔GB（右腿）
    GallStomach,
    /// 脾肾固本 SP↔KI（左腿）
    SpleenKidney,
    /// 任督桥 Ren↔Du（躯干）
    RenDuBridge,
}

impl ScarCircuitKind {
    /// 组成该回路的两条经脉。
    pub fn meridian_pair(&self) -> (MeridianId, MeridianId) {
        match self {
            Self::TigerMouth => (MeridianId::LargeIntestine, MeridianId::SmallIntestine),
            Self::TripleYang => (MeridianId::SmallIntestine, MeridianId::TripleEnergizer),
            Self::HeartLung => (MeridianId::Lung, MeridianId::Heart),
            Self::LiverKidney => (MeridianId::Kidney, MeridianId::Liver),
            Self::GallStomach => (MeridianId::Stomach, MeridianId::Gallbladder),
            Self::SpleenKidney => (MeridianId::Spleen, MeridianId::Kidney),
            Self::RenDuBridge => (MeridianId::Ren, MeridianId::Du),
        }
    }

    /// 所有回路种类。
    pub const ALL: [ScarCircuitKind; 7] = [
        Self::TigerMouth,
        Self::TripleYang,
        Self::HeartLung,
        Self::LiverKidney,
        Self::GallStomach,
        Self::SpleenKidney,
        Self::RenDuBridge,
    ];

    /// 检查某条经脉是否属于此回路。
    #[allow(dead_code)]
    pub fn involves(&self, id: MeridianId) -> bool {
        let (a, b) = self.meridian_pair();
        a == id || b == id
    }
}

/// 活跃疤纹回路集合。
#[derive(Component, Debug, Clone, Default, Serialize, Deserialize)]
pub struct ActiveScarCircuits {
    pub circuits: HashSet<ScarCircuitKind>,
    pub formed_at: HashMap<ScarCircuitKind, u64>,
}

impl ActiveScarCircuits {
    pub fn is_active(&self, kind: ScarCircuitKind) -> bool {
        self.circuits.contains(&kind)
    }

    #[allow(dead_code)]
    pub fn active_count(&self) -> usize {
        self.circuits.len()
    }
}

/// 检查经脉是否在回路形成区间内：opened && integrity in [MIN, MAX] && not severed。
fn meridian_in_circuit_range(
    ms: &MeridianSystem,
    severed: Option<&MeridianSeveredPermanent>,
    id: MeridianId,
) -> bool {
    let m = ms.get(id);
    if !m.opened {
        return false;
    }
    if m.integrity < SCAR_CIRCUIT_INTEGRITY_MIN || m.integrity > SCAR_CIRCUIT_INTEGRITY_MAX {
        return false;
    }
    if let Some(s) = severed {
        if s.is_severed(id) {
            return false;
        }
    }
    true
}

/// 判断回路断裂原因。
fn break_reason(
    ms: &MeridianSystem,
    severed: Option<&MeridianSeveredPermanent>,
    id: MeridianId,
) -> Option<CircuitBreakReason> {
    if let Some(s) = severed {
        if s.is_severed(id) {
            return Some(CircuitBreakReason::Severed);
        }
    }
    let m = ms.get(id);
    if !m.opened {
        return Some(CircuitBreakReason::Closed);
    }
    if m.integrity > SCAR_CIRCUIT_INTEGRITY_MAX {
        Some(CircuitBreakReason::Healed)
    } else if m.integrity < SCAR_CIRCUIT_INTEGRITY_MIN {
        Some(CircuitBreakReason::Deepened)
    } else {
        None
    }
}

/// 40-tick 周期检查疤纹回路形成/断裂。
pub fn scar_circuit_check_system(
    clock: Res<CombatClock>,
    mut query: Query<(
        Entity,
        &mut ActiveScarCircuits,
        &ScarHistory,
        &MeridianSystem,
        Option<&MeridianSeveredPermanent>,
    )>,
    mut formed_events: EventWriter<ScarCircuitFormedEvent>,
    mut broken_events: EventWriter<ScarCircuitBrokenEvent>,
) {
    if clock.tick % SCAR_CIRCUIT_CHECK_INTERVAL_TICKS != 0 {
        return;
    }

    for (entity, mut circuits, history, ms, severed) in query.iter_mut() {
        // Check for new formations.
        for kind in ScarCircuitKind::ALL {
            if circuits.is_active(kind) {
                continue;
            }
            if history.total_overloads < SCAR_CIRCUIT_MIN_OVERLOADS {
                continue;
            }
            let (a, b) = kind.meridian_pair();
            if meridian_in_circuit_range(ms, severed, a)
                && meridian_in_circuit_range(ms, severed, b)
            {
                circuits.circuits.insert(kind);
                circuits.formed_at.insert(kind, clock.tick);
                formed_events.send(ScarCircuitFormedEvent {
                    entity,
                    circuit: kind,
                    tick: clock.tick,
                });
            }
        }

        // Check for breakages.
        let active: Vec<ScarCircuitKind> = circuits.circuits.iter().copied().collect();
        for kind in active {
            let (a, b) = kind.meridian_pair();
            let reason_a = break_reason(ms, severed, a);
            let reason_b = break_reason(ms, severed, b);
            // If either meridian is out of range, break the circuit.
            if let Some(reason) = reason_a.or(reason_b) {
                circuits.circuits.remove(&kind);
                circuits.formed_at.remove(&kind);
                broken_events.send(ScarCircuitBrokenEvent {
                    entity,
                    circuit: kind,
                    reason,
                    tick: clock.tick,
                });
            }
        }
    }
}

/// 读 `ActiveScarCircuits` 写 `DerivedAttrs` 新增字段。
///
/// 仅处理通过 DerivedAttrs 传递的被动效果（reach_bonus, qi_regen_multiplier,
/// contam_purge_multiplier, healing_rate_multiplier）。虎口/胆胃/任督桥的效果
/// 直接在各自技能施放函数中检查 `ActiveScarCircuits`。
pub fn scar_circuit_derive_system(mut query: Query<(&ActiveScarCircuits, &mut DerivedAttrs)>) {
    for (circuits, mut attrs) in query.iter_mut() {
        // Reset circuit-derived fields to neutral defaults.
        attrs.reach_bonus = 0.0;
        attrs.qi_regen_multiplier = 1.0;
        attrs.contam_purge_multiplier = 1.0;
        attrs.healing_rate_multiplier = 1.0;

        if circuits.is_active(ScarCircuitKind::TripleYang) {
            attrs.reach_bonus = TRIPLE_YANG_REACH_BONUS;
        }
        if circuits.is_active(ScarCircuitKind::HeartLung) {
            attrs.qi_regen_multiplier = HEART_LUNG_QI_REGEN_MULTIPLIER;
        }
        if circuits.is_active(ScarCircuitKind::LiverKidney) {
            attrs.contam_purge_multiplier = LIVER_KIDNEY_CONTAM_PURGE_MULTIPLIER;
        }
        if circuits.is_active(ScarCircuitKind::SpleenKidney) {
            attrs.healing_rate_multiplier = SPLEEN_KIDNEY_HEALING_MULTIPLIER;
        }
    }
}
