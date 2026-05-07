//! plan-lingtian-v1 §5.1 — npc 系统消费 `ZonePressureCrossed{ level: High }`
//! 在该 zone 某个 plot 周围 spawn 3×3 道伥（worldview §八.1 注视规则）。
//!
//! 本模块跨边界订阅 lingtian 事件而不让 lingtian 反向引用 npc — 单向依赖。
//!
//! 道伥实体当前用 zombie archetype 兜底（与 spawn::spawn_zombie_npc_at 同档）。
//! 后续如有专门的"道伥" archetype（异色 / 高伤），换 archetype 即可。

use valence::prelude::{
    App, Commands, DVec3, Entity, EventReader, IntoSystemConfigs, Query, Update, With,
};

use crate::lingtian::pressure::PressureLevel;
use crate::lingtian::{LingtianPlot, ZonePressureCrossed};

use super::spawn::spawn_zombie_npc_at;

/// 触发时在 zone 某 plot 周围 spawn 9 个 zombie 作为 "道伥"。
pub fn spawn_daoshen_on_pressure_high(
    mut events: EventReader<ZonePressureCrossed>,
    plots: Query<&LingtianPlot>,
    layers: Query<Entity, With<crate::world::dimension::OverworldLayer>>,
    mut commands: Commands,
) {
    for e in events.read() {
        if !matches!(e.level, PressureLevel::High) {
            continue;
        }
        let Some(target_plot) = plots.iter().find(|p| p.zone == e.zone) else {
            tracing::warn!(
                "[bong][npc][daoshen] zone `{}` HIGH triggered but no LingtianPlot found for that zone",
                e.zone
            );
            continue;
        };
        let Ok(layer) = layers.get_single() else {
            tracing::warn!("[bong][npc][daoshen] no chunk+entity layer; skip spawn");
            continue;
        };
        let center = DVec3::new(
            target_plot.pos.x as f64,
            target_plot.pos.y as f64,
            target_plot.pos.z as f64,
        );
        // plan §5.1 — 3×3 范围 9 个
        let mut spawned = 0;
        for dx in -1..=1i32 {
            for dz in -1..=1i32 {
                let pos = DVec3::new(center.x + dx as f64, center.y, center.z + dz as f64);
                spawn_zombie_npc_at(&mut commands, layer, &e.zone, pos, pos);
                spawned += 1;
            }
        }
        tracing::warn!(
            "[bong][npc][daoshen] zone `{}` HIGH pressure ({:.3}) → spawned {spawned} 道伥 around plot {:?}",
            e.zone,
            e.raw_pressure,
            target_plot.pos
        );
    }
}

pub fn register(app: &mut App) {
    app.add_systems(Update, spawn_daoshen_on_pressure_high.chain());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lingtian::pressure::PressureLevel;
    use crate::lingtian::LingtianPlot;
    use crate::npc::spawn::NpcMarker;
    use valence::prelude::{App, BlockPos, Update};

    fn make_app() -> App {
        let mut app = App::new();
        app.add_event::<ZonePressureCrossed>();
        app.add_systems(Update, spawn_daoshen_on_pressure_high);
        app
    }

    #[test]
    fn multi_zone_picks_correct_plot() {
        let mut app = make_app();
        let layer = app
            .world_mut()
            .spawn(crate::world::dimension::OverworldLayer)
            .id();
        app.world_mut().spawn(
            LingtianPlot::new(BlockPos::new(100, 64, 100), None).with_zone("zone_a"),
        );
        app.world_mut().spawn(
            LingtianPlot::new(BlockPos::new(900, 64, 900), None).with_zone("zone_b"),
        );
        app.world_mut().send_event(ZonePressureCrossed {
            zone: "zone_b".to_string(),
            level: PressureLevel::High,
            raw_pressure: 1.0,
        });
        app.update();

        let mut positions = Vec::new();
        let world = app.world_mut();
        let mut query = world.query_filtered::<&valence::prelude::Position, With<NpcMarker>>();
        for pos in query.iter(world) {
            positions.push(pos.get());
        }
        assert!(
            !positions.is_empty(),
            "should have spawned daoshen for zone_b"
        );
        for pos in &positions {
            assert!(
                (pos.x - 900.0).abs() < 5.0,
                "daoshen should spawn near zone_b plot (x=900), got x={}",
                pos.x,
            );
        }
        let _ = layer;
    }

    #[test]
    fn single_zone_works_unchanged() {
        let mut app = make_app();
        app.world_mut()
            .spawn(crate::world::dimension::OverworldLayer);
        app.world_mut().spawn(
            LingtianPlot::new(BlockPos::new(50, 64, 50), None).with_zone("spawn"),
        );
        app.world_mut().send_event(ZonePressureCrossed {
            zone: "spawn".to_string(),
            level: PressureLevel::High,
            raw_pressure: 1.0,
        });
        app.update();

        let count = {
            let world = app.world_mut();
            let mut q = world.query_filtered::<Entity, With<NpcMarker>>();
            q.iter(world).count()
        };
        assert_eq!(count, 9, "should spawn 3×3=9 daoshen for single zone");
    }

    #[test]
    fn event_zone_no_match_warns_and_skips() {
        let mut app = make_app();
        app.world_mut()
            .spawn(crate::world::dimension::OverworldLayer);
        app.world_mut().spawn(
            LingtianPlot::new(BlockPos::new(50, 64, 50), None).with_zone("other"),
        );
        app.world_mut().send_event(ZonePressureCrossed {
            zone: "nonexistent".to_string(),
            level: PressureLevel::High,
            raw_pressure: 1.0,
        });
        app.update();

        let count = {
            let world = app.world_mut();
            let mut q = world.query_filtered::<Entity, With<NpcMarker>>();
            q.iter(world).count()
        };
        assert_eq!(
            count, 0,
            "no daoshen should spawn when zone doesn't match any plot"
        );
    }

    #[test]
    fn low_pressure_does_not_trigger() {
        let mut app = make_app();
        app.world_mut()
            .spawn(crate::world::dimension::OverworldLayer);
        app.world_mut().spawn(
            LingtianPlot::new(BlockPos::new(50, 64, 50), None).with_zone("spawn"),
        );
        app.world_mut().send_event(ZonePressureCrossed {
            zone: "spawn".to_string(),
            level: PressureLevel::Low,
            raw_pressure: 0.2,
        });
        app.update();

        let count = {
            let world = app.world_mut();
            let mut q = world.query_filtered::<Entity, With<NpcMarker>>();
            q.iter(world).count()
        };
        assert_eq!(count, 0, "low pressure should not trigger daoshen spawn");
    }
}
