//! plan-tsy-zone-v1 §3.1 — `!tsy-spawn <family_id>` 调试命令实现。
//!
//! 流程：
//! 1. chat_collector 解析 `!tsy-spawn <family_id>` → emit `TsySpawnRequested`
//! 2. 本模块的 `apply_tsy_spawn_requests` 系统消费 event：
//!    a. 从 `server/zones.tsy.json` 读 3 个 subzone（family_id 匹配）
//!    b. 调 `ZoneRegistry::register_runtime_zone` 追加（已存在 → 报错）
//!    c. 在玩家当前 overworld 位置 spawn 一个 Entry `RiftPortal` 实体
//!    d. 在 TSY 内对应 `_shallow` 中心 spawn 一个 Exit `RiftPortal` 实体
//!
//! 正式发布走 plan-tsy-worldgen-v1：本调试命令退化为"强制激活已注册 zone +
//! 传玩家"。P0 阶段是骨架兜底。

use std::collections::HashSet;
use std::path::Path;

use serde::Deserialize;
use valence::prelude::{
    bevy_ecs, App, Commands, DVec3, Entity, Event, EventReader, EventWriter, Position, Query,
    ResMut, Update,
};

use crate::world::dimension::DimensionKind;
use crate::world::tsy::{DimensionAnchor, PortalDirection, RiftPortal};
use crate::world::zone::{Zone, ZoneRegistry};

/// chat_collector → tsy_dev_command 桥事件。
#[derive(Event, Debug, Clone)]
pub struct TsySpawnRequested {
    /// 触发命令的玩家 entity（用于回写聊天反馈）。
    pub player_entity: Entity,
    /// 玩家当前主世界位置（裂缝实体的 Position）。
    pub player_pos: DVec3,
    /// 目标 family_id（必须命中 zones.tsy.json 内的 `<family>_shallow/_mid/_deep`）。
    pub family_id: String,
}

/// 命令处理结果（emit 给 chat 反馈）。
#[derive(Event, Debug, Clone)]
pub struct TsySpawnResult {
    pub player_entity: Entity,
    pub outcome: TsySpawnOutcome,
}

#[derive(Debug, Clone)]
pub enum TsySpawnOutcome {
    Success {
        family_id: String,
        portal_pos: DVec3,
    },
    AlreadySpawned {
        family_id: String,
    },
    UnknownFamily {
        family_id: String,
    },
    BlueprintMissing,
    BlueprintParseError(String),
}

#[derive(Deserialize)]
struct BlueprintRoot {
    zones: Vec<BlueprintZone>,
}

#[derive(Deserialize)]
struct BlueprintZone {
    name: String,
    #[serde(default)]
    dimension: DimensionKind,
    aabb: BlueprintAabb,
    spirit_qi: f64,
    danger_level: u8,
    #[serde(default)]
    active_events: Vec<String>,
    #[serde(default)]
    patrol_anchors: Vec<[f64; 3]>,
    #[serde(default)]
    blocked_tiles: Vec<[i32; 2]>,
}

#[derive(Deserialize)]
struct BlueprintAabb {
    min: [f64; 3],
    max: [f64; 3],
}

fn load_blueprint() -> Result<BlueprintRoot, TsySpawnOutcome> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("zones.tsy.json");
    let text = std::fs::read_to_string(&path).map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            TsySpawnOutcome::BlueprintMissing
        } else {
            TsySpawnOutcome::BlueprintParseError(err.to_string())
        }
    })?;
    serde_json::from_str(&text).map_err(|err| TsySpawnOutcome::BlueprintParseError(err.to_string()))
}

fn pick_family<'a>(
    blueprint: &'a BlueprintRoot,
    family_id: &str,
) -> Option<Vec<&'a BlueprintZone>> {
    let prefix = format!("{family_id}_");
    let matches: Vec<_> = blueprint
        .zones
        .iter()
        .filter(|z| z.name.starts_with(&prefix))
        .collect();
    if matches.len() == 3 {
        Some(matches)
    } else {
        None
    }
}

fn build_zone(z: &BlueprintZone) -> Zone {
    Zone {
        name: z.name.clone(),
        dimension: z.dimension,
        bounds: (
            DVec3::new(z.aabb.min[0], z.aabb.min[1], z.aabb.min[2]),
            DVec3::new(z.aabb.max[0], z.aabb.max[1], z.aabb.max[2]),
        ),
        spirit_qi: z.spirit_qi,
        danger_level: z.danger_level,
        active_events: z.active_events.clone(),
        patrol_anchors: z
            .patrol_anchors
            .iter()
            .map(|a| DVec3::new(a[0], a[1], a[2]))
            .collect(),
        blocked_tiles: z.blocked_tiles.iter().map(|[x, z]| (*x, *z)).collect(),
    }
}

/// 系统：消费 TsySpawnRequested 事件 → 注册 TSY subzone + spawn Entry/Exit portals。
pub fn apply_tsy_spawn_requests(
    mut commands: Commands,
    mut requests: EventReader<TsySpawnRequested>,
    mut results: EventWriter<TsySpawnResult>,
    zones: Option<ResMut<ZoneRegistry>>,
    portals: Query<&RiftPortal>,
) {
    let Some(mut zones) = zones else {
        return;
    };

    // 同 tick 去重：本系统这一遍消费的所有 family_id 缓存到 HashSet。
    // 必须显式去重，因为 `Commands::spawn` 是 deferred —— 同 tick 内 spawn 的
    // RiftPortal 实体在 `apply_deferred` 之前对 `portals.iter()` 不可见。
    // 不加这层会让两个相邻 TsySpawnRequested(同 family) 都通过 already-present
    // 检查，造成重复 portal + 第二条 outcome 撒谎说 Success（codex review P2）。
    let mut handled_in_tick: HashSet<String> = HashSet::new();

    for req in requests.read() {
        // 已存在同 family 的 portal（持久层）或本 tick 已处理过 → 拒绝
        // （plan §3.1 idempotency + codex review P2 同 tick 防重）
        let family_already_present = portals.iter().any(|p| p.family_id == req.family_id);
        let family_handled_this_tick = handled_in_tick.contains(&req.family_id);
        if family_already_present || family_handled_this_tick {
            results.send(TsySpawnResult {
                player_entity: req.player_entity,
                outcome: TsySpawnOutcome::AlreadySpawned {
                    family_id: req.family_id.clone(),
                },
            });
            continue;
        }

        let blueprint = match load_blueprint() {
            Ok(b) => b,
            Err(err) => {
                results.send(TsySpawnResult {
                    player_entity: req.player_entity,
                    outcome: err,
                });
                continue;
            }
        };

        let Some(family_zones) = pick_family(&blueprint, &req.family_id) else {
            results.send(TsySpawnResult {
                player_entity: req.player_entity,
                outcome: TsySpawnOutcome::UnknownFamily {
                    family_id: req.family_id.clone(),
                },
            });
            continue;
        };

        // 按层注册 — 同名已注册时 register_runtime_zone 会拒绝；这里若任一失败，
        // 我们停止并把已注册的留下（部分失败状态由人工 / 重启清理）
        let mut shallow_center: Option<DVec3> = None;
        for raw in &family_zones {
            let zone = build_zone(raw);
            let center = zone.center();
            if zone.name.ends_with("_shallow") {
                shallow_center = Some(center);
            }
            if let Err(err) = zones.register_runtime_zone(zone) {
                tracing::warn!("[bong][tsy_dev] register_runtime_zone failed: {err}");
            }
        }

        let Some(shallow_center) = shallow_center else {
            // family 缺 _shallow 层 → blueprint 错；视作 UnknownFamily
            results.send(TsySpawnResult {
                player_entity: req.player_entity,
                outcome: TsySpawnOutcome::UnknownFamily {
                    family_id: req.family_id.clone(),
                },
            });
            continue;
        };

        // 标记本 tick 已为该 family 触发 spawn —— 任何后续同 family 请求会
        // 在循环顶端 `family_handled_this_tick` 分支拦掉，避免 deferred Commands
        // 让 portals.iter() 看不到刚 spawn 的同 family portal 而误判 not-present。
        handled_in_tick.insert(req.family_id.clone());

        // Entry portal：主世界，玩家当前坐标
        commands.spawn((
            Position::new([req.player_pos.x, req.player_pos.y, req.player_pos.z]),
            RiftPortal {
                family_id: req.family_id.clone(),
                target: DimensionAnchor {
                    dimension: DimensionKind::Tsy,
                    pos: shallow_center,
                },
                trigger_radius: 1.5,
                direction: PortalDirection::Entry,
            },
        ));

        // Exit portal：TSY dim，shallow 中心
        commands.spawn((
            Position::new([shallow_center.x, shallow_center.y, shallow_center.z]),
            RiftPortal {
                family_id: req.family_id.clone(),
                target: DimensionAnchor {
                    dimension: DimensionKind::Overworld,
                    pos: req.player_pos + DVec3::Y,
                },
                trigger_radius: 1.5,
                direction: PortalDirection::Exit,
            },
        ));

        results.send(TsySpawnResult {
            player_entity: req.player_entity,
            outcome: TsySpawnOutcome::Success {
                family_id: req.family_id.clone(),
                portal_pos: req.player_pos,
            },
        });
    }
}

pub fn register(app: &mut App) {
    app.add_event::<TsySpawnRequested>()
        .add_event::<TsySpawnResult>()
        .add_systems(Update, apply_tsy_spawn_requests);
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::{App, Events};

    fn run_with_world(family_id: &str) -> App {
        let mut app = App::new();
        app.insert_resource(ZoneRegistry::fallback());
        app.add_event::<TsySpawnRequested>();
        app.add_event::<TsySpawnResult>();
        app.add_systems(Update, apply_tsy_spawn_requests);

        let player = app.world_mut().spawn(()).id();
        {
            let mut tx = app.world_mut().resource_mut::<Events<TsySpawnRequested>>();
            tx.send(TsySpawnRequested {
                player_entity: player,
                player_pos: DVec3::new(8.0, 64.0, 8.0),
                family_id: family_id.to_string(),
            });
        }
        app.update();
        app
    }

    fn outcomes(app: &App) -> Vec<TsySpawnOutcome> {
        let events = app.world().resource::<Events<TsySpawnResult>>();
        events
            .get_reader()
            .read(events)
            .map(|r| r.outcome.clone())
            .collect()
    }

    #[test]
    fn unknown_family_returns_unknown_family_outcome() {
        let app = run_with_world("tsy_does_not_exist_99");
        let out = outcomes(&app);
        assert_eq!(out.len(), 1);
        assert!(
            matches!(out[0], TsySpawnOutcome::UnknownFamily { .. }),
            "got: {:?}",
            out[0]
        );
    }

    #[test]
    fn known_family_registers_three_subzones_and_spawns_two_portals() {
        // zones.tsy.json 内的 sample family
        let mut app = run_with_world("tsy_lingxu_01");
        let out = outcomes(&app);
        assert_eq!(out.len(), 1);
        assert!(
            matches!(out[0], TsySpawnOutcome::Success { .. }),
            "got: {:?}",
            out[0]
        );

        let registry = app.world().resource::<ZoneRegistry>();
        // fallback (1) + 3 new TSY subzones
        assert_eq!(registry.zones.len(), 4);
        assert!(registry
            .zones
            .iter()
            .any(|z| z.name == "tsy_lingxu_01_shallow"));
        assert!(registry.zones.iter().any(|z| z.name == "tsy_lingxu_01_mid"));
        assert!(registry
            .zones
            .iter()
            .any(|z| z.name == "tsy_lingxu_01_deep"));

        // 两个 RiftPortal 实体（Entry + Exit）
        let mut q = app.world_mut().query::<&RiftPortal>();
        let portals: Vec<_> = q.iter(app.world()).cloned().collect();
        assert_eq!(portals.len(), 2);
        let directions: Vec<PortalDirection> = portals.iter().map(|p| p.direction).collect();
        assert!(directions.contains(&PortalDirection::Entry));
        assert!(directions.contains(&PortalDirection::Exit));
    }

    #[test]
    fn double_spawn_same_family_is_rejected_after_first_succeeds() {
        let mut app = App::new();
        app.insert_resource(ZoneRegistry::fallback());
        app.add_event::<TsySpawnRequested>();
        app.add_event::<TsySpawnResult>();
        app.add_systems(Update, apply_tsy_spawn_requests);

        let player = app.world_mut().spawn(()).id();

        // 第一次：成功
        {
            let mut tx = app.world_mut().resource_mut::<Events<TsySpawnRequested>>();
            tx.send(TsySpawnRequested {
                player_entity: player,
                player_pos: DVec3::new(8.0, 64.0, 8.0),
                family_id: "tsy_lingxu_01".to_string(),
            });
        }
        app.update();

        // 第二次：因为 RiftPortal 已存在 → AlreadySpawned
        {
            let mut tx = app.world_mut().resource_mut::<Events<TsySpawnRequested>>();
            tx.send(TsySpawnRequested {
                player_entity: player,
                player_pos: DVec3::new(8.0, 64.0, 8.0),
                family_id: "tsy_lingxu_01".to_string(),
            });
        }
        app.update();

        let out = outcomes(&app);
        // Both outcomes get accumulated across both ticks (Events 默认 hold 2 ticks)
        // 我们只关心最后一条是 AlreadySpawned。
        assert!(
            out.iter()
                .any(|o| matches!(o, TsySpawnOutcome::AlreadySpawned { .. })),
            "expected one AlreadySpawned outcome, got: {out:?}"
        );
    }

    /// Codex review P2 regression：两条同 family 的 TsySpawnRequested 在同一
    /// system pass 内被消费时，必须只 spawn 一组 portal —— 因为 Commands::spawn
    /// 是 deferred，第二条 request 看不到第一条刚发的 spawn 命令，没有去重保护
    /// 会让 portal 加倍。
    #[test]
    fn same_tick_double_request_for_same_family_dedupes_to_one_spawn() {
        let mut app = App::new();
        app.insert_resource(ZoneRegistry::fallback());
        app.add_event::<TsySpawnRequested>();
        app.add_event::<TsySpawnResult>();
        app.add_systems(Update, apply_tsy_spawn_requests);

        let player = app.world_mut().spawn(()).id();

        // 同一 tick 内连发两条同 family 请求
        {
            let mut tx = app.world_mut().resource_mut::<Events<TsySpawnRequested>>();
            tx.send(TsySpawnRequested {
                player_entity: player,
                player_pos: DVec3::new(1.0, 64.0, 1.0),
                family_id: "tsy_lingxu_01".to_string(),
            });
            tx.send(TsySpawnRequested {
                player_entity: player,
                player_pos: DVec3::new(2.0, 64.0, 2.0), // 不同位置——确认不会 spawn 第二组
                family_id: "tsy_lingxu_01".to_string(),
            });
        }
        app.update();

        // 1) 只 spawn 一对 portal（Entry + Exit），不是两对
        let mut q = app.world_mut().query::<&RiftPortal>();
        let portals: Vec<_> = q.iter(app.world()).cloned().collect();
        assert_eq!(
            portals.len(),
            2,
            "expected exactly 2 portals (1 Entry + 1 Exit), got {}",
            portals.len()
        );

        // 2) outcomes：第一条 Success，第二条 AlreadySpawned
        let out = outcomes(&app);
        assert_eq!(out.len(), 2, "got: {out:?}");
        assert!(
            matches!(out[0], TsySpawnOutcome::Success { .. }),
            "first outcome should be Success, got: {:?}",
            out[0]
        );
        assert!(
            matches!(out[1], TsySpawnOutcome::AlreadySpawned { .. }),
            "second outcome should be AlreadySpawned, got: {:?}",
            out[1]
        );
    }
}
