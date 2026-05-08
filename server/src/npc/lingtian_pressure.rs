//! plan-lingtian-v1 §5.1 — npc 系统消费 `ZonePressureCrossed{ level: High }`
//! 在该 zone 某个 plot 周围 spawn 3×3 道伥（worldview §八.1 注视规则）。
//!
//! 本模块跨边界订阅 lingtian 事件而不让 lingtian 反向引用 npc — 单向依赖。
//!
//! 道伥实体当前用 zombie archetype 兜底（与 spawn::spawn_zombie_npc_at 同档）。
//! 后续如有专门的"道伥" archetype（异色 / 高伤），换 archetype 即可。

use valence::prelude::{App, Commands, DVec3, Entity, EventReader, Query, Res, Update, With};

use crate::lingtian::pressure::PressureLevel;
use crate::lingtian::{LingtianPlot, ZonePressureCrossed, DEFAULT_ZONE};
use crate::world::terrain::TerrainProviders;

use super::spawn::{snap_spawn_y_to_surface, spawn_zombie_npc_at};

/// 触发时在 zone 某 plot 周围 spawn 9 个 zombie 作为 "道伥"。
pub fn spawn_daoshen_on_pressure_high(
    mut events: EventReader<ZonePressureCrossed>,
    plots: Query<&LingtianPlot>,
    layers: Query<Entity, With<crate::world::dimension::OverworldLayer>>,
    providers: Option<Res<TerrainProviders>>,
    mut commands: Commands,
) {
    let terrain = providers.as_deref().map(|p| &p.overworld);
    for e in events.read() {
        if !matches!(e.level, PressureLevel::High) {
            continue;
        }
        // Pressure events currently always carry `DEFAULT_ZONE` ("default")
        // because `compute_zone_pressure_system` is single-zone (lingtian
        // §1.3 — full multi-zone pressure is a separate plan). But
        // `auto_set_plot_zone` back-fills `LingtianPlot.zone` from
        // `ZoneRegistry` real names (e.g. "spawn", "qingyun"), so a strict
        // equality match would *never* hit a plot in production once registry
        // names diverge from "default". Until pressure becomes per-zone, fall
        // back to ANY plot when the event uses the default key.
        let target_plot = if e.zone == DEFAULT_ZONE {
            plots
                .iter()
                .find(|p| p.zone == e.zone)
                .or_else(|| plots.iter().next())
        } else {
            plots.iter().find(|p| p.zone == e.zone)
        };
        let Some(target_plot) = target_plot else {
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
                let raw = DVec3::new(center.x + dx as f64, center.y, center.z + dz as f64);
                // Snap each spawn point to the actual surface — the plot's
                // recorded Y can drift from the live terrain (terrain edits,
                // chunk regen), and floating daoshen would never engage the
                // farm.
                let pos = snap_spawn_y_to_surface(raw, terrain);
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
    app.add_systems(Update, spawn_daoshen_on_pressure_high);
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
        app.world_mut()
            .spawn(LingtianPlot::new(BlockPos::new(100, 64, 100), None).with_zone("zone_a"));
        app.world_mut()
            .spawn(LingtianPlot::new(BlockPos::new(900, 64, 900), None).with_zone("zone_b"));
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
        app.world_mut()
            .spawn(LingtianPlot::new(BlockPos::new(50, 64, 50), None).with_zone("spawn"));
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
        app.world_mut()
            .spawn(LingtianPlot::new(BlockPos::new(50, 64, 50), None).with_zone("other"));
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
        app.world_mut()
            .spawn(LingtianPlot::new(BlockPos::new(50, 64, 50), None).with_zone("spawn"));
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

    // -- P1.B: zone-key compatibility -----------------------------------------
    //
    // `compute_zone_pressure_system` currently always emits ZonePressureCrossed
    // with `zone = DEFAULT_ZONE` ("default"), but `auto_set_plot_zone` fills
    // each plot's zone from ZoneRegistry (e.g. "spawn"). A strict equality
    // match would silently drop daoshen spawning in any non-trivial setup.
    // Codex P1.B regression: confirm the DEFAULT_ZONE event still finds a
    // plot whose zone has been registry-filled to a real name.

    #[test]
    fn default_zone_event_falls_back_to_any_plot() {
        let mut app = make_app();
        app.world_mut()
            .spawn(crate::world::dimension::OverworldLayer);
        // Plot's zone has been filled from ZoneRegistry — name diverges from
        // the "default" the pressure event will carry.
        app.world_mut()
            .spawn(LingtianPlot::new(BlockPos::new(50, 64, 50), None).with_zone("spawn"));
        app.world_mut().send_event(ZonePressureCrossed {
            zone: DEFAULT_ZONE.to_string(),
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
            count, 9,
            "DEFAULT_ZONE event must fall back to any available plot \
             (single-zone pressure compat) — got {count} daoshen"
        );
    }

    #[test]
    fn default_zone_event_with_no_plots_warns_and_skips() {
        let mut app = make_app();
        app.world_mut()
            .spawn(crate::world::dimension::OverworldLayer);
        // No plots at all.
        app.world_mut().send_event(ZonePressureCrossed {
            zone: DEFAULT_ZONE.to_string(),
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
            "no plots → no daoshen, even with DEFAULT_ZONE event"
        );
    }

    #[test]
    fn named_zone_event_still_strict_match() {
        // Future-proofing: when pressure becomes per-zone (named events),
        // the strict-match path must NOT silently fall back to a different
        // zone's plot — that would spawn daoshen in the wrong place.
        let mut app = make_app();
        app.world_mut()
            .spawn(crate::world::dimension::OverworldLayer);
        app.world_mut()
            .spawn(LingtianPlot::new(BlockPos::new(50, 64, 50), None).with_zone("zone_a"));
        app.world_mut()
            .spawn(LingtianPlot::new(BlockPos::new(900, 64, 900), None).with_zone("zone_b"));
        app.world_mut().send_event(ZonePressureCrossed {
            zone: "nonexistent_zone".to_string(),
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
            "named-zone event with no matching plot must NOT fall back to a stranger plot"
        );
    }
}
