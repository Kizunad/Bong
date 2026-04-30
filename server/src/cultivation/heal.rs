//! MeridianHealTick（plan §2.1）— 经脉裂痕修复。
//!
//! 静坐（此处简化：非战斗态下默认静坐）时，每 tick 按 zone.spirit_qi 的
//! 1/10 比例推进 `healing_progress`。达到 severity 后裂痕移除并恢复一小
//! 段 integrity。

use valence::prelude::{Position, Query, Res};

use crate::world::events::EVENT_REALM_COLLAPSE;
use crate::world::zone::ZoneRegistry;

use super::components::MeridianSystem;

pub const BASE_HEAL_RATE: f64 = 0.01;
pub const INTEGRITY_PER_CRACK_HEALED: f64 = 0.02;

/// 纯函数：推进一条裂痕愈合。返回 (delta, is_healed)。
pub fn advance_heal(severity: f64, progress: &mut f64, zone_qi: f64) -> (f64, bool) {
    if *progress >= severity {
        return (0.0, true);
    }
    let delta = BASE_HEAL_RATE * zone_qi;
    *progress = (*progress + delta).min(severity);
    let healed = *progress >= severity;
    (delta, healed)
}

pub fn meridian_heal_tick(
    zones: Option<Res<ZoneRegistry>>,
    mut players: Query<(&Position, &mut MeridianSystem)>,
) {
    let Some(zones) = zones else {
        return;
    };
    for (pos, mut meridians) in players.iter_mut() {
        let zone_qi = zones
            .find_zone(crate::world::dimension::DimensionKind::Overworld, pos.0)
            .filter(|zone| {
                !zone
                    .active_events
                    .iter()
                    .any(|event| event == EVENT_REALM_COLLAPSE)
            })
            .map(|z| z.spirit_qi)
            .unwrap_or(0.0);
        if zone_qi <= 0.0 {
            continue;
        }
        for m in meridians.iter_mut() {
            let mut healed_count = 0usize;
            for crack in m.cracks.iter_mut() {
                let (_d, healed) =
                    advance_heal(crack.severity, &mut crack.healing_progress, zone_qi);
                if healed {
                    healed_count += 1;
                }
            }
            m.cracks.retain(|c| c.healing_progress < c.severity);
            if healed_count > 0 {
                m.integrity =
                    (m.integrity + INTEGRITY_PER_CRACK_HEALED * healed_count as f64).min(1.0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::{CrackCause, MeridianId, MeridianSystem};
    use crate::world::zone::ZoneRegistry;
    use valence::prelude::{App, Update};

    #[test]
    fn zero_zone_no_heal() {
        let mut p = 0.0;
        let (d, healed) = advance_heal(0.5, &mut p, 0.0);
        assert_eq!(d, 0.0);
        assert!(!healed);
    }

    #[test]
    fn heal_progresses_and_completes() {
        let mut p = 0.0;
        let sev = 0.05;
        for _ in 0..100 {
            let (_, healed) = advance_heal(sev, &mut p, 1.0);
            if healed {
                break;
            }
        }
        assert!(p >= sev);
    }

    #[test]
    fn collapsed_zone_blocks_meridian_heal_even_with_stale_qi() {
        let mut app = App::new();
        let mut zones = ZoneRegistry::fallback();
        let zone = zones.find_zone_mut("spawn").unwrap();
        zone.spirit_qi = 0.9;
        zone.active_events.push(EVENT_REALM_COLLAPSE.to_string());
        app.insert_resource(zones);
        app.add_systems(Update, meridian_heal_tick);

        let mut meridians = MeridianSystem::default();
        let lung = meridians.get_mut(MeridianId::Lung);
        lung.integrity = 0.5;
        lung.cracks
            .push(crate::cultivation::components::MeridianCrack {
                severity: 0.05,
                healing_progress: 0.0,
                cause: CrackCause::Attack,
                created_at: 0,
            });
        let player = app
            .world_mut()
            .spawn((Position::new([8.0, 66.0, 8.0]), meridians))
            .id();

        app.update();

        let meridians = app.world().entity(player).get::<MeridianSystem>().unwrap();
        let lung = meridians.get(MeridianId::Lung);
        assert_eq!(lung.integrity, 0.5);
        assert_eq!(lung.cracks.len(), 1);
        assert_eq!(lung.cracks[0].healing_progress, 0.0);
    }
}
