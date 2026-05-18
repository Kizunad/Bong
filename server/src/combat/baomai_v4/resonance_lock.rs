//! plan-baomai-v4 §6 P5 — 互噬（Resonance Lock）。
//!
//! 两个重度疤纹体修贴身搏杀时，双方裂纹经脉的真元振动频率在物理接触中产生
//! 共振放大——共振使裂纹扩展速率急剧上升，形成双向"互噬"。
//!
//! ## 核心流程
//!
//! 1. `resonance_meter_system`: 读 CombatEvent → 判定双方 ScarHistory 门槛 →
//!    计量增长 / 衰减 → meter >= 1.0 时触发锁定。
//! 2. `resonance_lock_tick_system`: 锁定期间每 tick drain 已受损经脉 integrity →
//!    检查距离脱离 / 到期 / 死亡 → emit ResonanceLockEndEvent。
//! 3. `resonance_cascade_system`: 锁定期间一方经脉 SEVERED → 对方同 MeridianId
//!    integrity *= 0.5。

use valence::prelude::{bevy_ecs, Component, Entity, EventReader, EventWriter, Query, Res, With};

use super::constants::{
    RESONANCE_LOCK_DURATION_TICKS, RESONANCE_LOCK_RANGE, RESONANCE_METER_HIT_BASE,
    RESONANCE_SCAR_THRESHOLD,
};
use super::events::{LockEndReason, ResonanceLockEndEvent, ResonanceLockEvent};
use super::scar_history::ScarHistory;
use crate::combat::components::{DerivedAttrs, Lifecycle, LifecycleState};
use crate::combat::events::CombatEvent;
use crate::combat::CombatClock;
use crate::cultivation::components::MeridianSystem;
use crate::cultivation::meridian::severed::MeridianSeveredEvent;
use crate::qi_physics::constants::{
    RESONANCE_LOCK_INTEGRITY_DRAIN, RESONANCE_RETREAT_INTEGRITY_PENALTY, SCAR_CIRCUIT_INTEGRITY_MAX,
};

/// 共振计量器——追踪与特定对手的互击累积。
///
/// 当双方都是重度疤纹体修（total_overloads >= RESONANCE_SCAR_THRESHOLD）且
/// 在近距离内互相命中时，meter 累积增长。达到 1.0 时触发 ResonanceLocked。
#[derive(Component, Debug, Clone)]
pub struct ResonanceMeter {
    /// 当前计量 0.0..=1.0。
    pub value: f64,
    /// 共振对手。
    pub partner: Option<Entity>,
    /// 上次计量增长 tick。
    pub last_hit_tick: u64,
}

impl Default for ResonanceMeter {
    fn default() -> Self {
        Self {
            value: 0.0,
            partner: None,
            last_hit_tick: 0,
        }
    }
}

/// 共振锁定状态——双方进入互噬后挂载此 component。
///
/// 锁定期间：经脉 integrity drain + rho_override = 0.0 + 移速 -30%。
#[derive(Component, Debug, Clone)]
pub struct ResonanceLocked {
    pub partner: Entity,
    pub started_at: u64,
    pub ends_at: u64,
}

/// 衰减无战斗窗口（5s = 100 ticks）。
const RESONANCE_DECAY_IDLE_TICKS: u64 = 100;

/// 衰减速率（per tick）。
const RESONANCE_DECAY_PER_TICK: f64 = 0.02;

/// 互击检测窗口（B 在 A 命中 B 的前 20 tick 内也命中了 A）。
const MUTUAL_HIT_WINDOW_TICKS: u64 = 20;

/// 移速惩罚倍率（-30% = 0.70）。
pub const RESONANCE_LOCK_MOVE_SPEED_MULTIPLIER: f64 = 0.70;

// ────────────────────────────────────────────────────────────
// §6.3 计量公式：纯函数（可单元测试）
// ────────────────────────────────────────────────────────────

/// 计算双方 scar 相似度：min/max overloads 之比。
pub fn scar_similarity(overloads_a: u32, overloads_b: u32) -> f64 {
    let lo = overloads_a.min(overloads_b) as f64;
    let hi = overloads_a.max(overloads_b) as f64;
    if hi == 0.0 {
        return 0.0;
    }
    lo / hi
}

/// 计算单次互击的 meter 增量。
pub fn resonance_increment(overloads_a: u32, overloads_b: u32) -> f64 {
    RESONANCE_METER_HIT_BASE * scar_similarity(overloads_a, overloads_b)
}

/// 判断双方是否都满足共振前置条件。
#[allow(dead_code)]
pub fn both_meet_scar_threshold(history_a: &ScarHistory, history_b: &ScarHistory) -> bool {
    history_a.total_overloads >= RESONANCE_SCAR_THRESHOLD
        && history_b.total_overloads >= RESONANCE_SCAR_THRESHOLD
}

// ────────────────────────────────────────────────────────────
// §6.3 meter system
// ────────────────────────────────────────────────────────────

/// 读 CombatEvent 判定互击 → 累积 meter → meter >= 1.0 触发锁定。
///
/// 调度在 `CombatSystemSet::Emit`（需要读 CombatEvent resolve 结果）。
pub fn resonance_meter_system(
    clock: Res<CombatClock>,
    mut combat_events: EventReader<CombatEvent>,
    mut query: Query<(&mut ResonanceMeter, &ScarHistory), With<ScarHistory>>,
    locked_query: Query<&ResonanceLocked>,
    mut lock_events: EventWriter<ResonanceLockEvent>,
    mut commands: valence::prelude::Commands,
) {
    let now = clock.tick;

    // Collect all combat events into a vec so we can iterate multiple times.
    let events: Vec<_> = combat_events.read().cloned().collect();

    // Build a map of "who last hit whom at what tick" from this frame's events.
    let mut last_attack_tick: std::collections::HashMap<(Entity, Entity), u64> =
        std::collections::HashMap::new();
    for ev in &events {
        last_attack_tick
            .entry((ev.attacker, ev.target))
            .and_modify(|t| *t = (*t).max(ev.resolved_at_tick))
            .or_insert(ev.resolved_at_tick);
    }

    // For each event: A attacks B. Check if B also attacked A within MUTUAL_HIT_WINDOW_TICKS.
    for ev in &events {
        let attacker = ev.attacker;
        let target = ev.target;
        if attacker == target {
            continue;
        }

        // Check mutual hit: B attacked A recently?
        let b_hit_a = query
            .get(target)
            .ok()
            .and_then(|(meter, _)| {
                if meter.partner == Some(attacker)
                    && now.saturating_sub(meter.last_hit_tick) <= MUTUAL_HIT_WINDOW_TICKS
                {
                    Some(true)
                } else {
                    // Also check if B hit A in this same frame's events.
                    last_attack_tick
                        .get(&(target, attacker))
                        .filter(|&&t| now.saturating_sub(t) <= MUTUAL_HIT_WINDOW_TICKS)
                        .map(|_| true)
                }
            })
            .unwrap_or(false);

        if !b_hit_a {
            // Record that A hit B on A's meter (for future mutual-hit checks) but don't increment.
            if let Ok((mut meter_a, _)) = query.get_mut(attacker) {
                meter_a.partner = Some(target);
                meter_a.last_hit_tick = now;
            }
            continue;
        }

        // Both are hitting each other. Check scar thresholds.
        let (overloads_a, overloads_b) = {
            let hist_a = query.get(attacker).map(|(_, h)| h.total_overloads);
            let hist_b = query.get(target).map(|(_, h)| h.total_overloads);
            match (hist_a, hist_b) {
                (Ok(a), Ok(b)) => (a, b),
                _ => continue,
            }
        };

        if overloads_a < RESONANCE_SCAR_THRESHOLD || overloads_b < RESONANCE_SCAR_THRESHOLD {
            // Still record the hit for future checks.
            if let Ok((mut meter_a, _)) = query.get_mut(attacker) {
                meter_a.partner = Some(target);
                meter_a.last_hit_tick = now;
            }
            continue;
        }

        let increment = resonance_increment(overloads_a, overloads_b);

        // Increment both meters.
        let mut trigger = false;
        if let Ok((mut meter_a, _)) = query.get_mut(attacker) {
            meter_a.partner = Some(target);
            meter_a.last_hit_tick = now;
            meter_a.value = (meter_a.value + increment).min(1.0);
            if meter_a.value >= 1.0 {
                trigger = true;
            }
        }
        if let Ok((mut meter_b, _)) = query.get_mut(target) {
            meter_b.partner = Some(attacker);
            meter_b.last_hit_tick = now;
            meter_b.value = (meter_b.value + increment).min(1.0);
            if meter_b.value >= 1.0 {
                trigger = true;
            }
        }

        // If either meter hit 1.0 and neither is already locked, trigger lock.
        if trigger {
            let a_locked = locked_query.get(attacker).is_ok();
            let b_locked = locked_query.get(target).is_ok();
            if !a_locked && !b_locked {
                let ends_at = now + RESONANCE_LOCK_DURATION_TICKS;
                commands.entity(attacker).insert(ResonanceLocked {
                    partner: target,
                    started_at: now,
                    ends_at,
                });
                commands.entity(target).insert(ResonanceLocked {
                    partner: attacker,
                    started_at: now,
                    ends_at,
                });

                // Reset meters.
                if let Ok((mut meter_a, _)) = query.get_mut(attacker) {
                    meter_a.value = 0.0;
                    meter_a.partner = None;
                }
                if let Ok((mut meter_b, _)) = query.get_mut(target) {
                    meter_b.value = 0.0;
                    meter_b.partner = None;
                }

                lock_events.send(ResonanceLockEvent {
                    fighter_a: attacker,
                    fighter_b: target,
                    started_at: now,
                    ends_at,
                });
            }
        }
    }

    // §6.3 衰减：5s 无互击 meter 以 0.02/tick 衰减至 0。
    for (mut meter, _) in query.iter_mut() {
        if meter.value > 0.0 && now.saturating_sub(meter.last_hit_tick) > RESONANCE_DECAY_IDLE_TICKS
        {
            meter.value = (meter.value - RESONANCE_DECAY_PER_TICK).max(0.0);
            if meter.value == 0.0 {
                meter.partner = None;
            }
        }
    }
}

// ────────────────────────────────────────────────────────────
// §6.4 lock tick system
// ────────────────────────────────────────────────────────────

/// 锁定期间：drain integrity + 检查到期/距离/死亡。
///
/// 调度在 `CombatSystemSet::Physics`（每 tick 跑一次）。
pub fn resonance_lock_tick_system(
    clock: Res<CombatClock>,
    mut query: Query<(
        Entity,
        &ResonanceLocked,
        &mut MeridianSystem,
        Option<&Lifecycle>,
        &mut DerivedAttrs,
    )>,
    positions: Query<&valence::prelude::Position>,
    mut end_events: EventWriter<ResonanceLockEndEvent>,
    mut commands: valence::prelude::Commands,
) {
    let now = clock.tick;

    // Collect lock pairs to process (avoid double-processing A-B and B-A).
    let mut processed_pairs: std::collections::HashSet<(Entity, Entity)> =
        std::collections::HashSet::new();

    // First pass: collect all locked entities' info.
    let locked_entities: Vec<(Entity, Entity, u64, u64)> = query
        .iter()
        .map(|(e, lock, _, _, _)| (e, lock.partner, lock.started_at, lock.ends_at))
        .collect();

    for (entity_a, entity_b, _started_at, ends_at) in &locked_entities {
        let pair = if *entity_a < *entity_b {
            (*entity_a, *entity_b)
        } else {
            (*entity_b, *entity_a)
        };
        if processed_pairs.contains(&pair) {
            continue;
        }
        processed_pairs.insert(pair);

        // Check termination conditions.
        let mut end_reason: Option<LockEndReason> = None;

        // §6.5: Natural expiry.
        if now >= *ends_at {
            end_reason = Some(LockEndReason::Expired);
        }

        // §6.5: One side dies.
        if end_reason.is_none() {
            let a_dead = query
                .get(*entity_a)
                .ok()
                .and_then(|(_, _, _, lc, _)| lc)
                .map(|lc| lc.state != LifecycleState::Alive)
                .unwrap_or(true);
            let b_dead = query
                .get(*entity_b)
                .ok()
                .and_then(|(_, _, _, lc, _)| lc)
                .map(|lc| lc.state != LifecycleState::Alive)
                .unwrap_or(true);
            if a_dead {
                end_reason = Some(LockEndReason::Death(*entity_a));
            } else if b_dead {
                end_reason = Some(LockEndReason::Death(*entity_b));
            }
        }

        // §6.5: Distance retreat.
        if end_reason.is_none() {
            let pos_a = positions.get(*entity_a).ok().map(|p| p.0);
            let pos_b = positions.get(*entity_b).ok().map(|p| p.0);
            if let (Some(pa), Some(pb)) = (pos_a, pos_b) {
                let dist = pa.distance(pb);
                if dist > RESONANCE_LOCK_RANGE {
                    // Determine who retreated: the one who moved further from center.
                    // Simple heuristic: we treat both as retreating if distance exceeded.
                    // The plan says "先脱离方" — but in a per-tick check we can't tell who
                    // moved first. Use a convention: the entity with the larger displacement
                    // from the midpoint is the "retreater". If equal, entity_a arbitrarily.
                    let mid = (pa + pb) * 0.5;
                    let dist_a = pa.distance(mid);
                    let dist_b = pb.distance(mid);
                    let retreater = if dist_a >= dist_b {
                        *entity_a
                    } else {
                        *entity_b
                    };
                    end_reason = Some(LockEndReason::Retreated(retreater));
                }
            }
        }

        if let Some(reason) = end_reason {
            // §6.5: Apply retreat penalty to the retreater.
            if let LockEndReason::Retreated(retreater) = reason {
                if let Ok((_, _, mut ms, _, _)) = query.get_mut(retreater) {
                    apply_retreat_penalty(&mut ms);
                }
            }

            // Remove ResonanceLocked from both.
            commands.entity(*entity_a).remove::<ResonanceLocked>();
            commands.entity(*entity_b).remove::<ResonanceLocked>();

            // Reset move speed multiplier.
            if let Ok((_, _, _, _, mut attrs_a)) = query.get_mut(*entity_a) {
                // Only reset if it was the resonance penalty that lowered it.
                // We use a simple approach: reset to 1.0 (other systems will re-apply their
                // modifiers on the next tick).
                attrs_a.move_speed_multiplier = 1.0;
            }
            if let Ok((_, _, _, _, mut attrs_b)) = query.get_mut(*entity_b) {
                attrs_b.move_speed_multiplier = 1.0;
            }

            end_events.send(ResonanceLockEndEvent {
                fighter_a: *entity_a,
                fighter_b: *entity_b,
                reason,
                tick: now,
            });
            continue;
        }

        // §6.4.1: Drain integrity of damaged meridians.
        for entity in [*entity_a, *entity_b] {
            if let Ok((_, _, mut ms, _, mut attrs)) = query.get_mut(entity) {
                drain_damaged_meridians(&mut ms);
                // §6.4.4: Apply move speed penalty.
                attrs.move_speed_multiplier = RESONANCE_LOCK_MOVE_SPEED_MULTIPLIER as f32;
            }
        }
    }
}

// ────────────────────────────────────────────────────────────
// §6.4.3 Cascade system
// ────────────────────────────────────────────────────────────

/// 锁定期间一方经脉 SEVERED → 对方同 MeridianId integrity *= 0.5。
///
/// 调度在 `CombatSystemSet::Resolve`（在 MeridianSeveredEvent 写入后）。
pub fn resonance_cascade_system(
    mut severed_events: EventReader<MeridianSeveredEvent>,
    query: Query<(Entity, &ResonanceLocked)>,
    mut meridian_query: Query<&mut MeridianSystem>,
) {
    for ev in severed_events.read() {
        // Is the severed entity currently locked?
        let partner = query
            .iter()
            .find(|(e, _)| *e == ev.entity)
            .map(|(_, lock)| lock.partner);

        if let Some(partner_entity) = partner {
            // Apply cascade: partner's same MeridianId integrity *= 0.5.
            if let Ok(mut partner_ms) = meridian_query.get_mut(partner_entity) {
                let m = partner_ms.get_mut(ev.meridian_id);
                m.integrity *= 0.5;
            }
        }
    }
}

// ────────────────────────────────────────────────────────────
// Helper functions
// ────────────────────────────────────────────────────────────

/// §6.4.1: Drain already-damaged meridians during resonance lock.
///
/// "Already damaged" = integrity < SCAR_CIRCUIT_INTEGRITY_MAX (0.85).
pub fn drain_damaged_meridians(ms: &mut MeridianSystem) {
    for m in ms.iter_mut() {
        if m.opened && m.integrity < SCAR_CIRCUIT_INTEGRITY_MAX {
            m.integrity = (m.integrity - RESONANCE_LOCK_INTEGRITY_DRAIN).max(0.0);
        }
    }
}

/// §6.5: Retreat penalty — all damaged meridians lose RESONANCE_RETREAT_INTEGRITY_PENALTY.
pub fn apply_retreat_penalty(ms: &mut MeridianSystem) {
    for m in ms.iter_mut() {
        if m.opened && m.integrity < SCAR_CIRCUIT_INTEGRITY_MAX {
            m.integrity = (m.integrity - RESONANCE_RETREAT_INTEGRITY_PENALTY).max(0.0);
        }
    }
}

/// §6.4.2: Check if entity has ResonanceLocked — used by qi_collision callers to
/// decide rho_override.
///
/// Returns the rho_override value if the entity is locked with the given partner,
/// or None if not locked.
#[allow(dead_code)]
pub fn resonance_rho_override(
    _entity: Entity,
    partner: Entity,
    locked: Option<&ResonanceLocked>,
) -> Option<f64> {
    locked
        .filter(|l| l.partner == partner)
        .map(|_| crate::qi_physics::constants::RESONANCE_LOCK_RHO_OVERRIDE)
}
