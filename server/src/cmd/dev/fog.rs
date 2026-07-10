//! dev `/fog` —— 以执行者为中心生成/清除动态雾堤（plan-dense-fog-v1 P1 前置）。
//!
//! 写入 `EnvironmentOverlays`，由 `weather_environment_sync_system` 每 tick 并进
//! zone effects 组装并经 `bong:zone_environment` 广播。density ≥ 0.85
//! （`OPAQUE_FOG_DENSITY_THRESHOLD`）会同时触发 `weather_vision_obscure_system`
//! 的 ViewDistance 压缩。

use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::parsers::CommandArg;
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{App, Client, EventReader, Position, Query, Res, ResMut, Update};

use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::environment_overlay::{EnvironmentOverlays, DEFAULT_FOG_BANK_TINT};
use crate::world::weather_physics::vision::OPAQUE_FOG_DENSITY_THRESHOLD;
use crate::world::zone::ZoneRegistry;

/// 水平半径合法区间（格）。
pub const FOG_RADIUS_MIN: f64 = 1.0;
pub const FOG_RADIUS_MAX: f64 = 512.0;
/// 雾堤竖直跨度：执行者脚下向下 / 向上延伸（格）。
pub const FOG_DEPTH_BELOW: f64 = 24.0;
pub const FOG_HEIGHT_ABOVE: f64 = 48.0;

#[derive(Debug, Clone, PartialEq)]
pub enum FogCmd {
    Spawn {
        radius: f64,
        density: f64,
        duration_ticks: Option<u32>,
    },
    Clear {
        id: String,
    },
    ClearAll,
    List,
}

impl Command for FogCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        let root = graph.root().literal("fog").id();

        let spawn = graph
            .at(root)
            .literal("spawn")
            .argument("radius")
            .with_parser::<f64>()
            .argument("density")
            .with_parser::<f64>()
            .with_executable(|input| FogCmd::Spawn {
                radius: f64::parse_arg(input).unwrap(),
                density: f64::parse_arg(input).unwrap(),
                duration_ticks: None,
            })
            .id();

        graph
            .at(spawn)
            .argument("duration_ticks")
            .with_parser::<u32>()
            .with_executable(|input| FogCmd::Spawn {
                radius: f64::parse_arg(input).unwrap(),
                density: f64::parse_arg(input).unwrap(),
                duration_ticks: Some(u32::parse_arg(input).unwrap()),
            });

        graph
            .at(root)
            .literal("clear")
            .argument("id")
            .with_parser::<String>()
            .with_executable(|input| FogCmd::Clear {
                id: String::parse_arg(input).unwrap(),
            });

        graph
            .at(root)
            .literal("clear_all")
            .with_executable(|_| FogCmd::ClearAll);

        graph
            .at(root)
            .literal("list")
            .with_executable(|_| FogCmd::List);
    }
}

pub fn register(app: &mut App) {
    app.add_command::<FogCmd>().add_systems(Update, handle_fog);
}

pub fn handle_fog(
    mut events: EventReader<CommandResultEvent<FogCmd>>,
    overlays: Option<ResMut<EnvironmentOverlays>>,
    zones: Option<Res<ZoneRegistry>>,
    mut clients: Query<(&mut Client, &Position, Option<&CurrentDimension>)>,
) {
    let Some(mut overlays) = overlays else {
        for event in events.read() {
            if let Ok((mut client, _, _)) = clients.get_mut(event.executor) {
                client.send_chat_message("[dev] fog failed: EnvironmentOverlays missing");
            }
        }
        return;
    };

    for event in events.read() {
        let Ok((mut client, position, dimension)) = clients.get_mut(event.executor) else {
            continue;
        };
        match &event.result {
            FogCmd::Spawn {
                radius,
                density,
                duration_ticks,
            } => {
                if !radius.is_finite() || *radius < FOG_RADIUS_MIN || *radius > FOG_RADIUS_MAX {
                    client.send_chat_message(format!(
                        "[dev] fog rejected: radius must be finite in {FOG_RADIUS_MIN}..={FOG_RADIUS_MAX}, got {radius}"
                    ));
                    continue;
                }
                if !density.is_finite() || *density <= 0.0 {
                    client.send_chat_message(format!(
                        "[dev] fog rejected: density must be finite in (0,1], got {density}"
                    ));
                    continue;
                }
                if *duration_ticks == Some(0) {
                    client.send_chat_message(
                        "[dev] fog rejected: duration_ticks must be > 0 (omit it for a permanent bank)",
                    );
                    continue;
                }
                let density = density.min(1.0);
                let Some(zones) = zones.as_ref() else {
                    client.send_chat_message("[dev] fog failed: ZoneRegistry missing");
                    continue;
                };

                let dimension_kind = dimension
                    .map(|dimension| dimension.0)
                    .unwrap_or(DimensionKind::Overworld);
                let center = position.0;
                let aabb_min = [
                    center.x - radius,
                    center.y - FOG_DEPTH_BELOW,
                    center.z - radius,
                ];
                let aabb_max = [
                    center.x + radius,
                    center.y + FOG_HEIGHT_ABOVE,
                    center.z + radius,
                ];

                let hit_zones: Vec<&str> = zones
                    .zones
                    .iter()
                    .filter(|zone| zone.dimension == dimension_kind)
                    .filter(|zone| {
                        let (zone_min, zone_max) = zone.bounds;
                        (aabb_min[0] <= zone_max.x && aabb_max[0] >= zone_min.x)
                            && (aabb_min[1] <= zone_max.y && aabb_max[1] >= zone_min.y)
                            && (aabb_min[2] <= zone_max.z && aabb_max[2] >= zone_min.z)
                    })
                    .map(|zone| zone.name.as_str())
                    .collect();
                if hit_zones.is_empty() {
                    client.send_chat_message(
                        "[dev] fog rejected: AABB overlaps no zone in this dimension (broadcast is zone-scoped) — move into a zone first (/tpzone)",
                    );
                    continue;
                }
                let hit_zones = hit_zones.join(", ");

                let id = overlays.spawn_fog_bank(
                    dimension_kind.ident_str(),
                    aabb_min,
                    aabb_max,
                    DEFAULT_FOG_BANK_TINT,
                    density as f32,
                    duration_ticks.map(u64::from),
                );
                tracing::warn!(
                    "[dev-cmd] fog bank `{id}` spawned bypassing weather system: r={radius}, density={density:.2}, zones=[{hit_zones}]"
                );
                let duration_label = duration_ticks
                    .map(|ticks| format!("{ticks} ticks"))
                    .unwrap_or_else(|| "permanent".to_string());
                let obscure_hint = if density as f32 >= OPAQUE_FOG_DENSITY_THRESHOLD {
                    " (>=0.85: view distance obscured)"
                } else {
                    ""
                };
                client.send_chat_message(format!(
                    "[dev] fog `{id}` spawned: r={radius}, density={density:.2}, duration={duration_label}, zones=[{hit_zones}]{obscure_hint}"
                ));
            }
            FogCmd::Clear { id } => {
                if overlays.remove_fog_bank(id) {
                    client.send_chat_message(format!("[dev] fog `{id}` cleared"));
                } else {
                    let known = known_ids(&overlays);
                    client.send_chat_message(format!(
                        "[dev] fog `{id}` not found; active: [{known}]"
                    ));
                }
            }
            FogCmd::ClearAll => {
                let count = overlays.clear_fog_banks();
                client.send_chat_message(format!("[dev] fog cleared {count} bank(s)"));
            }
            FogCmd::List => {
                if overlays.fog_banks().is_empty() {
                    client.send_chat_message("[dev] fog: no active banks");
                } else {
                    for bank in overlays.fog_banks() {
                        let remaining = bank
                            .remaining_ticks
                            .map(|ticks| format!("{ticks} ticks"))
                            .unwrap_or_else(|| "permanent".to_string());
                        client.send_chat_message(format!(
                            "[dev] fog `{}`: dim={}, density={:.2}, remaining={remaining}",
                            bank.id, bank.dimension, bank.density
                        ));
                    }
                }
            }
        }
    }
}

fn known_ids(overlays: &EnvironmentOverlays) -> String {
    overlays
        .fog_banks()
        .iter()
        .map(|bank| bank.id.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::dev::test_support::{run_update, spawn_test_client};
    use crate::world::environment::{EnvironmentEffect, ZoneEnvironmentRegistry};
    use valence::prelude::{Events, IntoSystemConfigs};

    fn setup_app() -> App {
        let mut app = App::new();
        app.insert_resource(ZoneRegistry::fallback());
        app.insert_resource(EnvironmentOverlays::default());
        app.insert_resource(ZoneEnvironmentRegistry::new());
        app.add_event::<CommandResultEvent<FogCmd>>();
        app.add_systems(
            Update,
            (
                handle_fog,
                crate::world::weather_to_environment::weather_environment_sync_system,
            )
                .chain(),
        );
        app
    }

    fn send(app: &mut App, player: valence::prelude::Entity, cmd: FogCmd) {
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<FogCmd>>>()
            .send(CommandResultEvent {
                result: cmd,
                executor: player,
                modifiers: Default::default(),
            });
    }

    fn spawn_cmd(radius: f64, density: f64, duration_ticks: Option<u32>) -> FogCmd {
        FogCmd::Spawn {
            radius,
            density,
            duration_ticks,
        }
    }

    fn fallback_zone_center(app: &App) -> [f64; 3] {
        let zones = app.world().resource::<ZoneRegistry>();
        let center = zones.zones[0].center();
        [center.x, center.y, center.z]
    }

    fn registry_fog_densities(app: &App, zone: &str) -> Vec<f32> {
        app.world()
            .resource::<ZoneEnvironmentRegistry>()
            .current(zone)
            .iter()
            .filter_map(|effect| match effect {
                EnvironmentEffect::FogVeil { density, .. } => Some(*density),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn fog_spawn_composes_fog_veil_into_zone_registry() {
        let mut app = setup_app();
        let center = fallback_zone_center(&app);
        let player = spawn_test_client(&mut app, "Alice", center);
        let zone_name = app.world().resource::<ZoneRegistry>().zones[0].name.clone();

        send(&mut app, player, spawn_cmd(32.0, 0.95, None));
        run_update(&mut app);

        let banks = app.world().resource::<EnvironmentOverlays>();
        assert_eq!(banks.fog_banks().len(), 1, "spawn 后应登记 1 片雾堤");
        assert!(
            registry_fog_densities(&app, &zone_name).contains(&0.95),
            "sync 组装后 zone `{zone_name}` 的 effects 应含 density=0.95 的 FogVeil，实际 {:?}",
            registry_fog_densities(&app, &zone_name)
        );
    }

    #[test]
    fn fog_spawn_rejects_invalid_radius() {
        let mut app = setup_app();
        let center = fallback_zone_center(&app);
        let player = spawn_test_client(&mut app, "Alice", center);

        for radius in [0.0, -5.0, 513.0, f64::NAN, f64::INFINITY] {
            send(&mut app, player, spawn_cmd(radius, 0.9, None));
        }
        run_update(&mut app);

        assert!(
            app.world()
                .resource::<EnvironmentOverlays>()
                .fog_banks()
                .is_empty(),
            "非法 radius（越界/非有限）不得登记雾堤"
        );
    }

    #[test]
    fn fog_spawn_rejects_nonpositive_density_and_clamps_overshoot() {
        let mut app = setup_app();
        let center = fallback_zone_center(&app);
        let player = spawn_test_client(&mut app, "Alice", center);

        for density in [0.0, -0.5, f64::NAN] {
            send(&mut app, player, spawn_cmd(16.0, density, None));
        }
        run_update(&mut app);
        assert!(
            app.world()
                .resource::<EnvironmentOverlays>()
                .fog_banks()
                .is_empty(),
            "density <= 0 或非有限值应整条拒绝"
        );

        send(&mut app, player, spawn_cmd(16.0, 5.0, None));
        run_update(&mut app);
        let banks = app.world().resource::<EnvironmentOverlays>();
        assert_eq!(banks.fog_banks().len(), 1);
        assert_eq!(banks.fog_banks()[0].density, 1.0, "density 超 1 应钳到 1.0");
    }

    #[test]
    fn fog_spawn_rejects_zero_duration() {
        let mut app = setup_app();
        let center = fallback_zone_center(&app);
        let player = spawn_test_client(&mut app, "Alice", center);

        send(&mut app, player, spawn_cmd(16.0, 0.9, Some(0)));
        run_update(&mut app);

        assert!(
            app.world()
                .resource::<EnvironmentOverlays>()
                .fog_banks()
                .is_empty(),
            "duration_ticks=0 应拒绝（常驻请省略该参数）"
        );
    }

    #[test]
    fn fog_spawn_outside_any_zone_rejected() {
        let mut app = setup_app();
        let player = spawn_test_client(&mut app, "Alice", [99999.0, 64.0, 99999.0]);

        send(&mut app, player, spawn_cmd(16.0, 0.9, None));
        run_update(&mut app);

        assert!(
            app.world()
                .resource::<EnvironmentOverlays>()
                .fog_banks()
                .is_empty(),
            "AABB 不与任何 zone 相交时应拒绝（广播是 zone-scoped，登记了也发不出去）"
        );
    }

    #[test]
    fn fog_duration_expires_and_leaves_composition() {
        let mut app = setup_app();
        let center = fallback_zone_center(&app);
        let player = spawn_test_client(&mut app, "Alice", center);
        let zone_name = app.world().resource::<ZoneRegistry>().zones[0].name.clone();

        send(&mut app, player, spawn_cmd(32.0, 0.95, Some(2)));
        run_update(&mut app);
        assert!(
            registry_fog_densities(&app, &zone_name).contains(&0.95),
            "duration=2 的雾堤在第 1 tick 应仍在组装结果里"
        );

        run_update(&mut app);
        assert!(
            app.world()
                .resource::<EnvironmentOverlays>()
                .fog_banks()
                .is_empty(),
            "第 2 tick 后雾堤应到期摘除"
        );
        assert!(
            !registry_fog_densities(&app, &zone_name).contains(&0.95),
            "到期后 zone effects 不应再含该 FogVeil"
        );
    }

    #[test]
    fn fog_clear_removes_by_id_and_unknown_id_is_noop() {
        let mut app = setup_app();
        let center = fallback_zone_center(&app);
        let player = spawn_test_client(&mut app, "Alice", center);

        send(&mut app, player, spawn_cmd(32.0, 0.9, None));
        run_update(&mut app);
        let id = app.world().resource::<EnvironmentOverlays>().fog_banks()[0]
            .id
            .clone();

        send(
            &mut app,
            player,
            FogCmd::Clear {
                id: "no_such".into(),
            },
        );
        run_update(&mut app);
        assert_eq!(
            app.world()
                .resource::<EnvironmentOverlays>()
                .fog_banks()
                .len(),
            1,
            "未知 id 不得误删既有雾堤"
        );

        send(&mut app, player, FogCmd::Clear { id });
        run_update(&mut app);
        assert!(app
            .world()
            .resource::<EnvironmentOverlays>()
            .fog_banks()
            .is_empty());
    }

    #[test]
    fn fog_clear_all_removes_everything_and_list_is_safe() {
        let mut app = setup_app();
        let center = fallback_zone_center(&app);
        let player = spawn_test_client(&mut app, "Alice", center);

        send(&mut app, player, spawn_cmd(32.0, 0.9, None));
        send(&mut app, player, spawn_cmd(16.0, 0.5, Some(1200)));
        run_update(&mut app);
        assert_eq!(
            app.world()
                .resource::<EnvironmentOverlays>()
                .fog_banks()
                .len(),
            2
        );

        send(&mut app, player, FogCmd::List);
        send(&mut app, player, FogCmd::ClearAll);
        send(&mut app, player, FogCmd::List);
        run_update(&mut app);
        assert!(app
            .world()
            .resource::<EnvironmentOverlays>()
            .fog_banks()
            .is_empty());
    }
}
