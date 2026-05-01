//! OverloadDetectionTick（plan §2.1）— 检测经脉 throughput 超限 → 添加裂痕。
//!
//! `throughput_current` 由战斗端在施法/爆发时写入。本 tick：
//!   * `throughput_current > flow_rate × OVERLOAD_RATIO` → 按超限比产生裂痕
//!   * `qi_max_frozen += severity × FREEZE_FACTOR`
//!
//! 本 plan 只负责检测 + 损伤演化，不感知伤害来源。

use valence::prelude::{bevy_ecs, Entity, Event, EventReader, EventWriter, Query, Res};

use super::components::{CrackCause, Cultivation, MeridianCrack, MeridianId, MeridianSystem};
use super::tick::CultivationClock;

pub const OVERLOAD_RATIO: f64 = 1.5;
pub const CRACK_SEVERITY_COEF: f64 = 0.3;
pub const FREEZE_FACTOR: f64 = 5.0;

#[derive(Debug, Clone, Event)]
pub struct MeridianOverloadEvent {
    pub entity: Entity,
    pub severity: f64,
}

#[derive(Debug, Clone, Event)]
pub struct MeridianCrackEvent {
    pub target: Entity,
    pub severity: f64,
    pub cause: CrackCause,
    pub created_at: u64,
}

/// 纯函数：检测一条经脉是否过载，返回应添加的裂痕 severity（0 表示无）。
pub fn overload_severity(throughput: f64, flow_rate: f64) -> f64 {
    if flow_rate <= 0.0 {
        return 0.0;
    }
    let ratio = throughput / flow_rate;
    if ratio <= OVERLOAD_RATIO {
        return 0.0;
    }
    (ratio - 1.0) * CRACK_SEVERITY_COEF
}

pub fn overload_detection_tick(
    clock: Res<CultivationClock>,
    mut overload_events: EventWriter<MeridianOverloadEvent>,
    mut players: Query<(Entity, &mut Cultivation, &mut MeridianSystem)>,
) {
    let now = clock.tick;
    for (entity, mut cultivation, mut meridians) in players.iter_mut() {
        let mut freeze_add = 0.0;
        let mut max_severity: f64 = 0.0;
        for m in meridians.iter_mut() {
            let sev = overload_severity(m.throughput_current, m.flow_rate);
            if sev > 0.0 {
                max_severity = max_severity.max(sev);
                m.cracks.push(MeridianCrack {
                    severity: sev,
                    healing_progress: 0.0,
                    cause: CrackCause::Overload,
                    created_at: now,
                });
                m.integrity = (m.integrity - sev * 0.1).max(0.0);
                freeze_add += sev * FREEZE_FACTOR;
            }
            // tick 末清空瞬时流量——战斗端负责每帧写入
            m.throughput_current = 0.0;
        }
        if freeze_add > 0.0 {
            let frozen = cultivation.qi_max_frozen.unwrap_or(0.0) + freeze_add;
            cultivation.qi_max_frozen = Some(frozen.min(cultivation.qi_max * 0.5));
            overload_events.send(MeridianOverloadEvent {
                entity,
                severity: max_severity,
            });
        }
    }
}

pub fn apply_meridian_crack_events(
    mut events: EventReader<MeridianCrackEvent>,
    mut meridian_systems: Query<&mut MeridianSystem>,
) {
    for event in events.read() {
        let Ok(mut meridians) = meridian_systems.get_mut(event.target) else {
            tracing::warn!(
                "[bong][cultivation] dropped meridian crack event for {:?}: missing MeridianSystem",
                event.target
            );
            continue;
        };
        apply_meridian_crack_to_system(
            &mut meridians,
            event.severity,
            event.cause,
            event.created_at,
        );
    }
}

pub fn apply_meridian_crack_to_system(
    meridians: &mut MeridianSystem,
    severity: f64,
    cause: CrackCause,
    created_at: u64,
) -> Option<MeridianId> {
    if !severity.is_finite() || severity <= 0.0 {
        return None;
    }
    let severity = severity.clamp(0.0, 1.0);
    let target_id = meridians
        .iter()
        .filter(|meridian| meridian.opened)
        .max_by_key(|meridian| meridian.opened_at)
        .map(|meridian| meridian.id)
        .unwrap_or(MeridianId::Lung);
    let target = meridians.get_mut(target_id);
    target.cracks.push(MeridianCrack {
        severity,
        healing_progress: 0.0,
        cause,
        created_at,
    });
    target.integrity = (target.integrity - severity * 0.1).max(0.0);
    Some(target_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_severity_below_threshold() {
        assert_eq!(overload_severity(1.0, 1.0), 0.0);
        assert_eq!(overload_severity(1.4, 1.0), 0.0);
        assert_eq!(overload_severity(1.5, 1.0), 0.0);
    }

    #[test]
    fn severity_grows_with_overload() {
        let s1 = overload_severity(1.6, 1.0);
        let s2 = overload_severity(2.5, 1.0);
        assert!(s1 > 0.0 && s2 > s1);
        assert!((s2 - 1.5 * CRACK_SEVERITY_COEF).abs() < 1e-9);
    }

    #[test]
    fn zero_flow_rate_safe() {
        assert_eq!(overload_severity(10.0, 0.0), 0.0);
    }

    #[test]
    fn applies_crack_to_latest_opened_meridian() {
        let mut meridians = MeridianSystem::default();
        meridians.get_mut(MeridianId::Lung).opened = true;
        meridians.get_mut(MeridianId::Lung).opened_at = 10;
        meridians.get_mut(MeridianId::Heart).opened = true;
        meridians.get_mut(MeridianId::Heart).opened_at = 20;

        let hit = apply_meridian_crack_to_system(&mut meridians, 0.25, CrackCause::Backfire, 77);

        assert_eq!(hit, Some(MeridianId::Heart));
        let heart = meridians.get(MeridianId::Heart);
        assert_eq!(heart.cracks.len(), 1);
        assert_eq!(heart.cracks[0].cause, CrackCause::Backfire);
        assert!((heart.cracks[0].severity - 0.25).abs() < 1e-9);
        assert!((heart.integrity - 0.975).abs() < 1e-9);
    }

    #[test]
    fn crack_application_falls_back_to_lung_without_opened_meridians() {
        let mut meridians = MeridianSystem::default();

        let hit = apply_meridian_crack_to_system(&mut meridians, 0.1, CrackCause::Backfire, 3);

        assert_eq!(hit, Some(MeridianId::Lung));
        assert_eq!(meridians.get(MeridianId::Lung).cracks.len(), 1);
    }
}
