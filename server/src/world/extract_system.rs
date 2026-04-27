//! plan-tsy-extract-v1 — TSY 定点撤离倒计时、中断与 race-out portal 切换。

use valence::prelude::{
    bevy_ecs, App, Commands, Component, Entity, EntityLayerId, Event, EventReader, EventWriter,
    IntoSystemConfigs, Position, Query, Res, SystemSet, Update,
};

use crate::combat::components::{CombatState, Wounds};
use crate::combat::events::DeathEvent;
use crate::combat::{CombatClock, CombatSystemSet};
use crate::cultivation::components::Cultivation;
use crate::world::dimension::{DimensionKind, DimensionLayers};
use crate::world::dimension_transfer::{DimensionTransferRequest, DimensionTransferSet};
use crate::world::rift_portal::PORTAL_INTERACT_RADIUS;
use crate::world::tsy::{RiftKind, RiftPortal, TickWindow, TsyPresence};
use crate::world::tsy_lifecycle::{
    TsyCollapseCompleted, TsyCollapseStarted, TsyLifecycle, TsyZoneStateRegistry,
    COLLAPSE_DURATION_TICKS,
};
use crate::world::zone::ZoneRegistry;

pub const EXTRACT_MOVE_ABORT_RADIUS: f64 = 0.5;
pub const EXTRACT_PROGRESS_BROADCAST_INTERVAL_TICKS: u64 = 5;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TsyExtractSet;

#[derive(Component, Debug, Clone)]
pub struct ExtractProgress {
    pub portal: Entity,
    pub required_ticks: u32,
    pub elapsed_ticks: u32,
    pub started_at_tick: u64,
    pub started_pos: [f64; 3],
    pub wound_count_at_start: usize,
}

#[derive(Event, Debug, Clone)]
pub struct StartExtractRequest {
    pub player: Entity,
    pub portal: Entity,
}

#[derive(Event, Debug, Clone)]
#[allow(dead_code)] // 后续 schema bridge 会消费完整结果字段；当前 server 测试只匹配 reason。
pub enum StartExtractResult {
    Started {
        player: Entity,
        portal: Entity,
        required_ticks: u32,
    },
    Rejected {
        player: Entity,
        portal: Entity,
        reason: ExtractRejectionReason,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtractRejectionReason {
    OutOfRange,
    AlreadyBusy,
    InCombat,
    NotInTsy,
    PortalExpired,
    CannotExit,
    PortalCollapsed,
}

#[derive(Event, Debug, Clone)]
pub struct CancelExtractRequest {
    pub player: Entity,
}

#[derive(Event, Debug, Clone)]
#[allow(dead_code)] // 后续 schema bridge 会消费 portal 字段；当前完成系统只需要 player。
pub struct ExtractCompleted {
    pub player: Entity,
    pub portal: Entity,
    pub family_id: String,
    pub portal_kind: RiftKind,
    pub exit_world_pos: [f64; 3],
}

#[derive(Event, Debug, Clone, Copy)]
#[allow(dead_code)] // 后续 schema bridge 会消费 abort 详情；当前本地测试读取 reason。
pub struct ExtractAborted {
    pub player: Entity,
    pub portal: Entity,
    pub reason: ExtractAbortReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtractAbortReason {
    Moved,
    Combat,
    Damaged,
    Cancelled,
    PortalExpired,
}

#[derive(Event, Debug, Clone, Copy)]
#[allow(dead_code)] // 后续 schema bridge 会消费失败 portal；当前死亡路径只需要 player/reason。
pub struct ExtractFailed {
    pub player: Entity,
    pub portal: Entity,
    pub reason: ExtractFailureReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtractFailureReason {
    SpiritQiDrained,
}

#[derive(Event, Debug, Clone)]
#[allow(dead_code)] // 后续 HUD/IPC bridge 会消费进度 pulse；server 先产出事件。
pub struct ExtractProgressPulse {
    pub player: Entity,
    pub portal: Entity,
    pub elapsed_ticks: u32,
    pub required_ticks: u32,
}

fn is_in_combat(state: &CombatState, now_tick: u64) -> bool {
    state
        .in_combat_until_tick
        .is_some_and(|until_tick| now_tick < until_tick)
}

fn started_pos(pos: &Position) -> [f64; 3] {
    [pos.0.x, pos.0.y, pos.0.z]
}

fn distance_from_started(pos: &Position, started: [f64; 3]) -> f64 {
    pos.0.distance(valence::prelude::DVec3::new(
        started[0], started[1], started[2],
    ))
}

#[allow(clippy::type_complexity)]
pub fn start_extract_request(
    mut events: EventReader<StartExtractRequest>,
    mut results: EventWriter<StartExtractResult>,
    portals: Query<(&RiftPortal, &Position)>,
    players: Query<(
        &Position,
        Option<&TsyPresence>,
        &CombatState,
        &Wounds,
        Option<&ExtractProgress>,
    )>,
    mut commands: Commands,
    clock: Res<CombatClock>,
    lifecycle_registry: Option<Res<TsyZoneStateRegistry>>,
) {
    for req in events.read() {
        let Ok((portal, portal_pos)) = portals.get(req.portal) else {
            continue;
        };
        let Ok((player_pos, presence, combat, wounds, existing_progress)) = players.get(req.player)
        else {
            continue;
        };
        let Some(presence) = presence else {
            results.send(StartExtractResult::Rejected {
                player: req.player,
                portal: req.portal,
                reason: ExtractRejectionReason::NotInTsy,
            });
            continue;
        };

        let rejection = if existing_progress.is_some() {
            Some(ExtractRejectionReason::AlreadyBusy)
        } else if is_in_combat(combat, clock.tick) {
            Some(ExtractRejectionReason::InCombat)
        } else if player_pos.0.distance(portal_pos.0) > PORTAL_INTERACT_RADIUS {
            Some(ExtractRejectionReason::OutOfRange)
        } else if presence.family_id != portal.family_id || !portal.kind.allows_exit() {
            Some(ExtractRejectionReason::CannotExit)
        } else if portal
            .activation_window
            .is_some_and(|win| clock.tick < win.start_at_tick || clock.tick > win.end_at_tick)
        {
            Some(ExtractRejectionReason::PortalExpired)
        } else if lifecycle_registry.as_ref().is_some_and(|registry| {
            registry
                .by_family
                .get(&portal.family_id)
                .is_some_and(|state| state.lifecycle == TsyLifecycle::Dead)
        }) {
            Some(ExtractRejectionReason::PortalCollapsed)
        } else {
            None
        };

        if let Some(reason) = rejection {
            results.send(StartExtractResult::Rejected {
                player: req.player,
                portal: req.portal,
                reason,
            });
            continue;
        }

        let required_ticks = portal.current_extract_ticks;
        commands.entity(req.player).insert(ExtractProgress {
            portal: req.portal,
            required_ticks,
            elapsed_ticks: 0,
            started_at_tick: clock.tick,
            started_pos: started_pos(player_pos),
            wound_count_at_start: wounds.entries.len(),
        });
        results.send(StartExtractResult::Started {
            player: req.player,
            portal: req.portal,
            required_ticks,
        });
    }
}

pub fn cancel_extract_request(
    mut events: EventReader<CancelExtractRequest>,
    mut commands: Commands,
    players: Query<&ExtractProgress>,
    mut aborted: EventWriter<ExtractAborted>,
) {
    for req in events.read() {
        let Ok(progress) = players.get(req.player) else {
            continue;
        };
        aborted.send(ExtractAborted {
            player: req.player,
            portal: progress.portal,
            reason: ExtractAbortReason::Cancelled,
        });
        commands.entity(req.player).remove::<ExtractProgress>();
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn tick_extract_progress(
    mut players: Query<(
        Entity,
        &Position,
        &CombatState,
        &Wounds,
        &Cultivation,
        Option<&TsyPresence>,
        &mut ExtractProgress,
    )>,
    portals: Query<&RiftPortal>,
    mut commands: Commands,
    clock: Res<CombatClock>,
    mut complete_events: EventWriter<ExtractCompleted>,
    mut abort_events: EventWriter<ExtractAborted>,
    mut fail_events: EventWriter<ExtractFailed>,
    mut pulse_events: EventWriter<ExtractProgressPulse>,
) {
    for (player, pos, combat, wounds, cultivation, presence, mut progress) in &mut players {
        if cultivation.qi_current <= 0.0 {
            fail_events.send(ExtractFailed {
                player,
                portal: progress.portal,
                reason: ExtractFailureReason::SpiritQiDrained,
            });
            commands.entity(player).remove::<ExtractProgress>();
            continue;
        }

        if portals.get(progress.portal).is_err() {
            abort_events.send(ExtractAborted {
                player,
                portal: progress.portal,
                reason: ExtractAbortReason::PortalExpired,
            });
            commands.entity(player).remove::<ExtractProgress>();
            continue;
        }

        if distance_from_started(pos, progress.started_pos) > EXTRACT_MOVE_ABORT_RADIUS {
            abort_events.send(ExtractAborted {
                player,
                portal: progress.portal,
                reason: ExtractAbortReason::Moved,
            });
            commands.entity(player).remove::<ExtractProgress>();
            continue;
        }

        if is_in_combat(combat, clock.tick) {
            abort_events.send(ExtractAborted {
                player,
                portal: progress.portal,
                reason: ExtractAbortReason::Combat,
            });
            commands.entity(player).remove::<ExtractProgress>();
            continue;
        }

        if wounds.entries.len() > progress.wound_count_at_start {
            abort_events.send(ExtractAborted {
                player,
                portal: progress.portal,
                reason: ExtractAbortReason::Damaged,
            });
            commands.entity(player).remove::<ExtractProgress>();
            continue;
        }

        progress.elapsed_ticks = progress.elapsed_ticks.saturating_add(1);
        if clock
            .tick
            .saturating_sub(progress.started_at_tick)
            .is_multiple_of(EXTRACT_PROGRESS_BROADCAST_INTERVAL_TICKS)
        {
            pulse_events.send(ExtractProgressPulse {
                player,
                portal: progress.portal,
                elapsed_ticks: progress.elapsed_ticks,
                required_ticks: progress.required_ticks,
            });
        }

        if progress.elapsed_ticks >= progress.required_ticks {
            let Ok(portal) = portals.get(progress.portal) else {
                continue;
            };
            complete_events.send(ExtractCompleted {
                player,
                portal: progress.portal,
                family_id: portal.family_id.clone(),
                portal_kind: portal.kind,
                exit_world_pos: presence
                    .map(|presence| {
                        [
                            presence.return_to.pos.x,
                            presence.return_to.pos.y,
                            presence.return_to.pos.z,
                        ]
                    })
                    .unwrap_or([pos.0.x, pos.0.y, pos.0.z]),
            });
            commands.entity(player).remove::<ExtractProgress>();
        }
    }
}

pub fn handle_extract_completed(
    mut events: EventReader<ExtractCompleted>,
    mut commands: Commands,
    presences: Query<&TsyPresence>,
    mut dim_transfer: EventWriter<DimensionTransferRequest>,
) {
    for event in events.read() {
        let Ok(presence) = presences.get(event.player) else {
            continue;
        };
        dim_transfer.send(DimensionTransferRequest {
            entity: event.player,
            target: presence.return_to.dimension,
            target_pos: presence.return_to.pos,
        });
        commands.entity(event.player).remove::<TsyPresence>();
    }
}

pub fn handle_extract_failed(
    mut events: EventReader<ExtractFailed>,
    mut deaths: EventWriter<DeathEvent>,
    clock: Res<CombatClock>,
) {
    for event in events.read() {
        let cause = match event.reason {
            ExtractFailureReason::SpiritQiDrained => "tsy_extract_failed:spirit_qi_drained",
        };
        deaths.send(DeathEvent {
            target: event.player,
            cause: cause.to_string(),
            attacker: None,
            attacker_player_id: None,
            at_tick: clock.tick,
        });
    }
}

pub fn on_tsy_collapse_started(
    mut events: EventReader<TsyCollapseStarted>,
    mut portals: Query<(Entity, &mut RiftPortal)>,
    mut extracting: Query<&mut ExtractProgress>,
    mut commands: Commands,
    zones: Res<ZoneRegistry>,
    layers: Option<Res<DimensionLayers>>,
    clock: Res<CombatClock>,
) {
    let Some(layers) = layers else {
        for _ in events.read() {}
        return;
    };

    for event in events.read() {
        for (_entity, mut portal) in &mut portals {
            if portal.family_id == event.family_id {
                portal.current_extract_ticks = RiftKind::CollapseTear.base_extract_ticks();
            }
        }

        for mut progress in &mut extracting {
            if let Ok((_entity, portal)) = portals.get(progress.portal) {
                if portal.family_id == event.family_id {
                    progress.required_ticks = RiftKind::CollapseTear.base_extract_ticks();
                    progress.elapsed_ticks = 0;
                    progress.started_at_tick = clock.tick;
                }
            }
        }

        spawn_collapse_tears(
            &mut commands,
            &zones,
            layers.tsy,
            &event.family_id,
            event.at_tick,
        );
    }
}

fn spawn_collapse_tears(
    commands: &mut Commands,
    zones: &ZoneRegistry,
    tsy_layer: Entity,
    family_id: &str,
    now_tick: u64,
) {
    let family_zones: Vec<_> = zones
        .zones
        .iter()
        .filter(|zone| zone.tsy_family_id().as_deref() == Some(family_id))
        .collect();
    if family_zones.is_empty() {
        return;
    }

    let count = 3 + deterministic_seed(family_id, now_tick) as usize % 3;
    for idx in 0..count {
        let zone = family_zones[idx % family_zones.len()];
        let pos = deterministic_point_in_zone(zone, family_id, now_tick, idx as u64);
        commands.spawn((
            Position(pos),
            EntityLayerId(tsy_layer),
            RiftPortal {
                family_id: family_id.to_string(),
                target: crate::world::tsy::DimensionAnchor {
                    dimension: DimensionKind::Overworld,
                    pos: valence::prelude::DVec3::ZERO,
                },
                trigger_radius: PORTAL_INTERACT_RADIUS,
                direction: crate::world::tsy::PortalDirection::Exit,
                kind: RiftKind::CollapseTear,
                current_extract_ticks: RiftKind::CollapseTear.base_extract_ticks(),
                activation_window: Some(TickWindow {
                    start_at_tick: now_tick,
                    end_at_tick: now_tick.saturating_add(COLLAPSE_DURATION_TICKS),
                }),
            },
        ));
    }
}

fn deterministic_seed(family_id: &str, now_tick: u64) -> u64 {
    let mut seed = now_tick ^ 0xA076_1D64_78BD_642F;
    for byte in family_id.as_bytes() {
        seed ^= u64::from(*byte);
        seed = seed.wrapping_mul(0xE703_7ED1_A0B4_28DB).rotate_left(17);
    }
    seed
}

fn unit_from_seed(seed: u64) -> f64 {
    (seed % 10_000) as f64 / 10_000.0
}

fn deterministic_point_in_zone(
    zone: &crate::world::zone::Zone,
    family_id: &str,
    now_tick: u64,
    salt: u64,
) -> valence::prelude::DVec3 {
    let (min, max) = zone.bounds;
    let s1 = deterministic_seed(family_id, now_tick ^ salt.wrapping_mul(31));
    let s2 = deterministic_seed(family_id, now_tick ^ salt.wrapping_mul(131));
    let s3 = deterministic_seed(family_id, now_tick ^ salt.wrapping_mul(521));
    valence::prelude::DVec3::new(
        min.x + (max.x - min.x) * unit_from_seed(s1),
        min.y + (max.y - min.y) * unit_from_seed(s2),
        min.z + (max.z - min.z) * unit_from_seed(s3),
    )
}

pub fn on_tsy_collapse_completed(
    mut events: EventReader<TsyCollapseCompleted>,
    portals: Query<(Entity, &RiftPortal)>,
    players_in_tsy: Query<(Entity, &TsyPresence)>,
    mut deaths: EventWriter<DeathEvent>,
    mut commands: Commands,
    clock: Res<CombatClock>,
) {
    for event in events.read() {
        for (portal_entity, portal) in &portals {
            if portal.family_id == event.family_id {
                commands.entity(portal_entity).despawn();
            }
        }

        for (player, presence) in &players_in_tsy {
            if presence.family_id == event.family_id {
                deaths.send(DeathEvent {
                    target: player,
                    cause: "tsy_collapsed".to_string(),
                    attacker: None,
                    attacker_player_id: None,
                    at_tick: clock.tick,
                });
            }
        }
    }
}

pub fn despawn_expired_portals(
    portals: Query<(Entity, &RiftPortal)>,
    mut commands: Commands,
    clock: Res<CombatClock>,
) {
    for (entity, portal) in &portals {
        if portal
            .activation_window
            .is_some_and(|win| clock.tick > win.end_at_tick)
        {
            commands.entity(entity).despawn();
        }
    }
}

pub fn register(app: &mut App) {
    app.add_event::<StartExtractRequest>()
        .add_event::<StartExtractResult>()
        .add_event::<CancelExtractRequest>()
        .add_event::<ExtractCompleted>()
        .add_event::<ExtractAborted>()
        .add_event::<ExtractFailed>()
        .add_event::<ExtractProgressPulse>()
        .add_systems(
            Update,
            (
                start_extract_request,
                cancel_extract_request,
                on_tsy_collapse_started,
                despawn_expired_portals,
                tick_extract_progress,
                handle_extract_completed,
                handle_extract_failed,
                on_tsy_collapse_completed,
            )
                .chain()
                .in_set(TsyExtractSet)
                .after(CombatSystemSet::Physics)
                .before(DimensionTransferSet),
        );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::dimension::CurrentDimension;
    use crate::world::tsy::{DimensionAnchor, PortalDirection};
    use crate::world::zone::{Zone, ZoneRegistry};
    use valence::prelude::{App, DVec3, Events, IntoSystemConfigs, Update};

    fn app_with_extract_system<M>(system: impl IntoSystemConfigs<M>) -> App {
        let mut app = App::new();
        app.insert_resource(CombatClock::default());
        app.add_event::<StartExtractRequest>();
        app.add_event::<StartExtractResult>();
        app.add_event::<ExtractCompleted>();
        app.add_event::<ExtractAborted>();
        app.add_event::<ExtractFailed>();
        app.add_event::<ExtractProgressPulse>();
        app.add_event::<DimensionTransferRequest>();
        app.add_event::<DeathEvent>();
        app.add_systems(Update, system);
        app
    }

    fn portal(family_id: &str, kind: RiftKind, pos: DVec3) -> (Position, RiftPortal) {
        (
            Position(pos),
            RiftPortal::exit(
                family_id.to_string(),
                DimensionAnchor {
                    dimension: DimensionKind::Overworld,
                    pos: DVec3::ZERO,
                },
                PORTAL_INTERACT_RADIUS,
                kind,
            ),
        )
    }

    fn presence(family_id: &str) -> TsyPresence {
        TsyPresence {
            family_id: family_id.to_string(),
            entered_at_tick: 0,
            entry_inventory_snapshot: Vec::new(),
            return_to: DimensionAnchor {
                dimension: DimensionKind::Overworld,
                pos: DVec3::new(8.0, 65.0, 9.0),
            },
        }
    }

    fn spawn_player(app: &mut App, pos: DVec3, family_id: Option<&str>) -> Entity {
        let entity = app
            .world_mut()
            .spawn((
                Position(pos),
                CombatState::default(),
                Wounds::default(),
                Cultivation {
                    qi_current: 10.0,
                    qi_max: 10.0,
                    ..Default::default()
                },
                CurrentDimension(DimensionKind::Tsy),
            ))
            .id();
        if let Some(family_id) = family_id {
            app.world_mut()
                .entity_mut(entity)
                .insert(presence(family_id));
        }
        entity
    }

    #[test]
    fn start_extract_rejects_out_of_range() {
        let mut app = app_with_extract_system(start_extract_request);
        let portal = app
            .world_mut()
            .spawn(portal("tsy_lingxu_01", RiftKind::MainRift, DVec3::ZERO))
            .id();
        let player = spawn_player(&mut app, DVec3::new(10.0, 0.0, 0.0), Some("tsy_lingxu_01"));
        app.world_mut()
            .resource_mut::<Events<StartExtractRequest>>()
            .send(StartExtractRequest { player, portal });
        app.update();
        let results = app.world().resource::<Events<StartExtractResult>>();
        let collected: Vec<_> = results.get_reader().read(results).cloned().collect();
        assert!(matches!(
            collected.first(),
            Some(StartExtractResult::Rejected {
                reason: ExtractRejectionReason::OutOfRange,
                ..
            })
        ));
    }

    #[test]
    fn tick_extract_progress_aborts_when_moved() {
        let mut app = app_with_extract_system(tick_extract_progress);
        let portal = app
            .world_mut()
            .spawn(portal("tsy_lingxu_01", RiftKind::MainRift, DVec3::ZERO))
            .id();
        let player = spawn_player(&mut app, DVec3::new(1.0, 0.0, 0.0), Some("tsy_lingxu_01"));
        app.world_mut().entity_mut(player).insert(ExtractProgress {
            portal,
            required_ticks: 10,
            elapsed_ticks: 0,
            started_at_tick: 0,
            started_pos: [0.0, 0.0, 0.0],
            wound_count_at_start: 0,
        });
        app.update();
        let events = app.world().resource::<Events<ExtractAborted>>();
        let collected: Vec<_> = events.get_reader().read(events).cloned().collect();
        assert!(matches!(
            collected.first(),
            Some(ExtractAborted {
                reason: ExtractAbortReason::Moved,
                ..
            })
        ));
    }

    #[test]
    fn extract_completed_sends_dimension_transfer_and_removes_presence() {
        let mut app = app_with_extract_system(handle_extract_completed);
        let player = app
            .world_mut()
            .spawn((Position::new([0.0, 0.0, 0.0]), presence("tsy_lingxu_01")))
            .id();
        let portal = app.world_mut().spawn(()).id();
        app.world_mut()
            .resource_mut::<Events<ExtractCompleted>>()
            .send(ExtractCompleted {
                player,
                portal,
                family_id: "tsy_lingxu_01".to_string(),
                portal_kind: RiftKind::MainRift,
                exit_world_pos: [0.0, 0.0, 0.0],
            });
        app.update();
        assert!(app.world().entity(player).get::<TsyPresence>().is_none());
        let dim_events = app.world().resource::<Events<DimensionTransferRequest>>();
        let collected: Vec<_> = dim_events.get_reader().read(dim_events).cloned().collect();
        assert_eq!(collected.len(), 1);
        assert_eq!(collected[0].target, DimensionKind::Overworld);
    }

    #[test]
    fn collapse_started_compresses_portals_and_spawns_tears() {
        let mut app = app_with_extract_system(on_tsy_collapse_started);
        let tsy_layer = app.world_mut().spawn(()).id();
        let overworld = app.world_mut().spawn(()).id();
        app.insert_resource(DimensionLayers {
            overworld,
            tsy: tsy_layer,
        });
        app.insert_resource(ZoneRegistry {
            zones: vec![Zone {
                name: "tsy_lingxu_01_shallow".to_string(),
                dimension: DimensionKind::Tsy,
                bounds: (DVec3::ZERO, DVec3::new(10.0, 10.0, 10.0)),
                spirit_qi: -0.4,
                danger_level: 4,
                active_events: vec![],
                patrol_anchors: vec![],
                blocked_tiles: vec![],
            }],
        });
        let existing = app
            .world_mut()
            .spawn(portal("tsy_lingxu_01", RiftKind::DeepRift, DVec3::ZERO))
            .id();
        app.add_event::<TsyCollapseStarted>();
        app.world_mut()
            .resource_mut::<Events<TsyCollapseStarted>>()
            .send(TsyCollapseStarted {
                family_id: "tsy_lingxu_01".to_string(),
                at_tick: 10,
            });
        app.update();
        assert_eq!(
            app.world()
                .entity(existing)
                .get::<RiftPortal>()
                .unwrap()
                .current_extract_ticks,
            60
        );
        let mut q = app.world_mut().query::<&RiftPortal>();
        let tears = q
            .iter(app.world())
            .filter(|portal| portal.kind == RiftKind::CollapseTear)
            .count();
        assert!((3..=5).contains(&tears));
    }

    #[test]
    fn collapse_completed_despawns_portals_and_kills_remaining_players() {
        let mut app = app_with_extract_system(on_tsy_collapse_completed);
        let portal = app
            .world_mut()
            .spawn(portal("tsy_lingxu_01", RiftKind::MainRift, DVec3::ZERO))
            .id();
        let player = app.world_mut().spawn(presence("tsy_lingxu_01")).id();
        app.add_event::<TsyCollapseCompleted>();
        app.world_mut()
            .resource_mut::<Events<TsyCollapseCompleted>>()
            .send(TsyCollapseCompleted {
                family_id: "tsy_lingxu_01".to_string(),
                at_tick: 100,
            });
        app.update();
        assert!(app.world().get_entity(portal).is_none());
        let deaths = app.world().resource::<Events<DeathEvent>>();
        let collected: Vec<_> = deaths.get_reader().read(deaths).cloned().collect();
        assert_eq!(collected.len(), 1);
        assert_eq!(collected[0].target, player);
        assert_eq!(collected[0].cause, "tsy_collapsed");
    }

    #[test]
    fn deep_rift_is_exit_only() {
        let deep = RiftPortal::exit(
            "tsy_lingxu_01".to_string(),
            DimensionAnchor {
                dimension: DimensionKind::Overworld,
                pos: DVec3::ZERO,
            },
            PORTAL_INTERACT_RADIUS,
            RiftKind::DeepRift,
        );
        assert_eq!(deep.direction, PortalDirection::Exit);
        assert!(!deep.kind.allows_entry());
        assert!(deep.kind.allows_exit());
    }
}
