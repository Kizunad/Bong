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
    /// plan-tsy-raceout-v1 §4 Q-RC4：CollapseTear 单 portal 同时只许 1 人。
    /// 第二个玩家"撞墙"必须找下一个裂口；增加 race-out chicken-game 紧迫感。
    PortalOccupied,
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

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
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
    portal_occupants: Query<&ExtractProgress>,
    mut commands: Commands,
    clock: Res<CombatClock>,
    lifecycle_registry: Option<Res<TsyZoneStateRegistry>>,
) {
    // plan-tsy-raceout-v1 §4 Q-RC4：CollapseTear 单 portal 1 人。
    //
    // `Commands::insert` 是 deferred（system 结束才 ApplyDeferred），所以 `portal_occupants`
    // 只能看到上一 tick 的 ExtractProgress。同一 tick 内若有两个 StartExtractRequest 命中
    // 同一 CollapseTear，**两个都会通过 `portal_occupants.iter()` 检查并被 Started**——直接
    // 违反 Q-RC4 单 portal 单 player 的契约（多 client 包同帧到达时常发生）。
    //
    // 修复：本 system 内部用 `admitted_collapse_tears` 做 reservation，每次 admit 后立刻
    // 标记，下一次 occupied 检查同时看 set + portal_occupants。
    use std::collections::HashSet;
    let mut admitted_collapse_tears: HashSet<Entity> = HashSet::new();

    for req in events.read() {
        let Ok((portal, portal_pos)) = portals.get(req.portal) else {
            results.send(StartExtractResult::Rejected {
                player: req.player,
                portal: req.portal,
                reason: ExtractRejectionReason::PortalExpired,
            });
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

        let collapse_tear_occupied = portal.kind == RiftKind::CollapseTear
            && (portal_occupants
                .iter()
                .any(|progress| progress.portal == req.portal)
                || admitted_collapse_tears.contains(&req.portal));

        let rejection = if existing_progress.is_some() {
            Some(ExtractRejectionReason::AlreadyBusy)
        } else if is_in_combat(combat, clock.tick) {
            Some(ExtractRejectionReason::InCombat)
        } else if player_pos.0.distance(portal_pos.0) > portal.trigger_radius {
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
        } else if collapse_tear_occupied {
            Some(ExtractRejectionReason::PortalOccupied)
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
        if portal.kind == RiftKind::CollapseTear {
            admitted_collapse_tears.insert(req.portal);
        }
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
    zones: Option<Res<ZoneRegistry>>,
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
        let ticks_since_start = clock.tick.saturating_sub(progress.started_at_tick);
        if ticks_since_start > 0
            && ticks_since_start.is_multiple_of(EXTRACT_PROGRESS_BROADCAST_INTERVAL_TICKS)
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
            let exit_world_pos =
                extract_exit_world_pos(portal, presence, pos, zones.as_deref(), player, clock.tick);
            complete_events.send(ExtractCompleted {
                player,
                portal: progress.portal,
                family_id: portal.family_id.clone(),
                portal_kind: portal.kind,
                exit_world_pos,
            });
            commands.entity(player).remove::<ExtractProgress>();
        }
    }
}

fn extract_exit_world_pos(
    portal: &RiftPortal,
    presence: Option<&TsyPresence>,
    player_pos: &Position,
    zones: Option<&ZoneRegistry>,
    player: Entity,
    now_tick: u64,
) -> [f64; 3] {
    if portal.kind == RiftKind::CollapseTear {
        if let Some(pos) = collapse_rift_world_exit_pos(zones, &portal.family_id, player, now_tick)
        {
            return dvec3_to_array(pos);
        }
    }

    presence
        .map(|presence| dvec3_to_array(presence.return_to.pos))
        .unwrap_or_else(|| dvec3_to_array(player_pos.0))
}

fn collapse_rift_world_exit_pos(
    zones: Option<&ZoneRegistry>,
    family_id: &str,
    player: Entity,
    now_tick: u64,
) -> Option<valence::prelude::DVec3> {
    let zones = zones?;
    let candidates: Vec<_> = zones
        .zones
        .iter()
        .filter(|zone| zone.dimension == DimensionKind::Overworld)
        .filter(|zone| zone.name.starts_with("rift_mouth_"))
        .collect();
    if candidates.is_empty() {
        return None;
    }

    let seed = deterministic_seed(family_id, now_tick ^ player.to_bits());
    let zone = candidates[seed as usize % candidates.len()];
    Some(zone.patrol_target(0))
}

fn dvec3_to_array(pos: valence::prelude::DVec3) -> [f64; 3] {
    [pos.x, pos.y, pos.z]
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
        let (target, target_pos) = match event.portal_kind {
            RiftKind::CollapseTear => (
                DimensionKind::Overworld,
                valence::prelude::DVec3::new(
                    event.exit_world_pos[0],
                    event.exit_world_pos[1],
                    event.exit_world_pos[2],
                ),
            ),
            _ => (presence.return_to.dimension, presence.return_to.pos),
        };
        dim_transfer.send(DimensionTransferRequest {
            entity: event.player,
            target,
            target_pos,
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

    fn portal_with_radius(
        family_id: &str,
        kind: RiftKind,
        pos: DVec3,
        radius: f64,
    ) -> (Position, RiftPortal) {
        (
            Position(pos),
            RiftPortal::exit(
                family_id.to_string(),
                DimensionAnchor {
                    dimension: DimensionKind::Overworld,
                    pos: DVec3::ZERO,
                },
                radius,
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
    fn start_extract_uses_portal_trigger_radius() {
        let mut app = app_with_extract_system(start_extract_request);
        let portal = app
            .world_mut()
            .spawn(portal_with_radius(
                "tsy_lingxu_01",
                RiftKind::MainRift,
                DVec3::ZERO,
                1.5,
            ))
            .id();
        let player = spawn_player(&mut app, DVec3::new(1.7, 0.0, 0.0), Some("tsy_lingxu_01"));

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
    fn start_extract_rejects_missing_portal_as_expired() {
        let mut app = app_with_extract_system(start_extract_request);
        let missing_portal = app.world_mut().spawn(()).id();
        app.world_mut().entity_mut(missing_portal).despawn();
        let player = spawn_player(&mut app, DVec3::ZERO, Some("tsy_lingxu_01"));

        app.world_mut()
            .resource_mut::<Events<StartExtractRequest>>()
            .send(StartExtractRequest {
                player,
                portal: missing_portal,
            });
        app.update();

        let results = app.world().resource::<Events<StartExtractResult>>();
        let collected: Vec<_> = results.get_reader().read(results).cloned().collect();
        assert!(matches!(
            collected.first(),
            Some(StartExtractResult::Rejected {
                reason: ExtractRejectionReason::PortalExpired,
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
        assert_eq!(collected[0].target_pos, DVec3::new(8.0, 65.0, 9.0));
    }

    #[test]
    fn collapse_extract_completed_uses_event_exit_world_pos() {
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
                portal_kind: RiftKind::CollapseTear,
                exit_world_pos: [111.0, 74.0, -222.0],
            });
        app.update();

        let dim_events = app.world().resource::<Events<DimensionTransferRequest>>();
        let collected: Vec<_> = dim_events.get_reader().read(dim_events).cloned().collect();
        assert_eq!(collected.len(), 1);
        assert_eq!(collected[0].target, DimensionKind::Overworld);
        assert_eq!(collected[0].target_pos, DVec3::new(111.0, 74.0, -222.0));
    }

    #[test]
    fn collapse_extract_progress_targets_rift_mouth_zone() {
        let mut app = app_with_extract_system(tick_extract_progress);
        app.insert_resource(ZoneRegistry {
            zones: vec![Zone {
                name: "rift_mouth_north_001".to_string(),
                dimension: DimensionKind::Overworld,
                bounds: (
                    DVec3::new(-650.0, 50.0, -8650.0),
                    DVec3::new(-350.0, 100.0, -8350.0),
                ),
                spirit_qi: 0.05,
                danger_level: 5,
                active_events: vec!["rift_mouth_entry".to_string()],
                patrol_anchors: vec![DVec3::new(-500.0, 74.0, -8500.0)],
                blocked_tiles: vec![],
            }],
        });
        let portal = app
            .world_mut()
            .spawn(portal("tsy_lingxu_01", RiftKind::CollapseTear, DVec3::ZERO))
            .id();
        let player = spawn_player(&mut app, DVec3::ZERO, Some("tsy_lingxu_01"));
        app.world_mut().entity_mut(player).insert(ExtractProgress {
            portal,
            required_ticks: 1,
            elapsed_ticks: 0,
            started_at_tick: 0,
            started_pos: [0.0, 0.0, 0.0],
            wound_count_at_start: 0,
        });

        app.update();

        let events = app.world().resource::<Events<ExtractCompleted>>();
        let collected: Vec<_> = events.get_reader().read(events).cloned().collect();
        assert_eq!(collected.len(), 1);
        assert_eq!(collected[0].portal_kind, RiftKind::CollapseTear);
        assert_eq!(collected[0].exit_world_pos, [-500.0, 74.0, -8500.0]);
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

    /// plan-tsy-raceout-v1 §4 Q-RC4：CollapseTear 同时只许 1 人撤；
    /// 第二个玩家请求同一 portal 必须收到 `PortalOccupied`，触发"撞墙换下一个裂口"UX。
    #[test]
    fn collapse_tear_rejects_second_player_with_portal_occupied() {
        let mut app = app_with_extract_system(start_extract_request);
        let collapse_tear = app
            .world_mut()
            .spawn(portal("tsy_lingxu_01", RiftKind::CollapseTear, DVec3::ZERO))
            .id();
        let first = spawn_player(&mut app, DVec3::ZERO, Some("tsy_lingxu_01"));
        let second = spawn_player(&mut app, DVec3::ZERO, Some("tsy_lingxu_01"));

        app.world_mut()
            .resource_mut::<Events<StartExtractRequest>>()
            .send(StartExtractRequest {
                player: first,
                portal: collapse_tear,
            });
        app.update();
        // 清掉首轮结果，单看第二个玩家的反馈
        app.world_mut()
            .resource_mut::<Events<StartExtractResult>>()
            .clear();
        assert!(
            app.world().entity(first).get::<ExtractProgress>().is_some(),
            "首位玩家应已挂上 ExtractProgress 锁定该 CollapseTear"
        );

        app.world_mut()
            .resource_mut::<Events<StartExtractRequest>>()
            .send(StartExtractRequest {
                player: second,
                portal: collapse_tear,
            });
        app.update();

        let results = app.world().resource::<Events<StartExtractResult>>();
        let collected: Vec<_> = results.get_reader().read(results).cloned().collect();
        let reasons: Vec<_> = collected
            .iter()
            .filter_map(|r| match r {
                StartExtractResult::Rejected {
                    player,
                    reason,
                    portal,
                } if *player == second && *portal == collapse_tear => Some(*reason),
                _ => None,
            })
            .collect();
        assert_eq!(
            reasons,
            vec![ExtractRejectionReason::PortalOccupied],
            "第二位玩家应被 PortalOccupied 拒绝（CollapseTear 单 portal 单 player），实际：{:?}",
            collected
        );
    }

    /// plan-tsy-raceout-v1 §4 Q-RC4：MainRift / DeepRift 不受单 portal 单 player 限制；
    /// 标准撤离允许多人同时在同一裂缝撤离（worldview "搜打撤" 语义）。
    #[test]
    fn main_rift_allows_concurrent_extracts_unlike_collapse_tear() {
        let mut app = app_with_extract_system(start_extract_request);
        let main_rift = app
            .world_mut()
            .spawn(portal("tsy_lingxu_01", RiftKind::MainRift, DVec3::ZERO))
            .id();
        let first = spawn_player(&mut app, DVec3::ZERO, Some("tsy_lingxu_01"));
        let second = spawn_player(&mut app, DVec3::ZERO, Some("tsy_lingxu_01"));

        app.world_mut()
            .resource_mut::<Events<StartExtractRequest>>()
            .send(StartExtractRequest {
                player: first,
                portal: main_rift,
            });
        app.update();
        app.world_mut()
            .resource_mut::<Events<StartExtractResult>>()
            .clear();

        app.world_mut()
            .resource_mut::<Events<StartExtractRequest>>()
            .send(StartExtractRequest {
                player: second,
                portal: main_rift,
            });
        app.update();

        let results = app.world().resource::<Events<StartExtractResult>>();
        let collected: Vec<_> = results.get_reader().read(results).cloned().collect();
        let started_count = collected
            .iter()
            .filter(
                |r| matches!(r, StartExtractResult::Started { player, .. } if *player == second),
            )
            .count();
        assert_eq!(
            started_count, 1,
            "MainRift 不限单 portal 单 player，第二位玩家应正常 Started，实际：{:?}",
            collected
        );
    }

    /// plan-tsy-raceout-v1 §4 Q-RC4 — Codex review #151 P1 修复回归：
    /// `Commands::insert` 是 deferred；同一 tick 两个 StartExtractRequest 命中
    /// 同一 CollapseTear 时，`portal_occupants.iter()` 只看到上一 tick 的状态。
    /// 没有 in-loop reservation 会让两个 player 同帧都被 Started，违反 Q-RC4。
    /// 本测试 send 两个 event 在 update 前 → 一次 update 内消费两个 event →
    /// 第二个必须收到 PortalOccupied。
    #[test]
    fn collapse_tear_in_loop_reservation_prevents_same_tick_double_admit() {
        let mut app = app_with_extract_system(start_extract_request);
        let tear = app
            .world_mut()
            .spawn(portal("tsy_lingxu_01", RiftKind::CollapseTear, DVec3::ZERO))
            .id();
        let first = spawn_player(&mut app, DVec3::ZERO, Some("tsy_lingxu_01"));
        let second = spawn_player(&mut app, DVec3::ZERO, Some("tsy_lingxu_01"));

        // 同一 update 内 send 两个 event（模拟同 tick 两个 client 包到达）
        {
            let mut req_writer = app
                .world_mut()
                .resource_mut::<Events<StartExtractRequest>>();
            req_writer.send(StartExtractRequest {
                player: first,
                portal: tear,
            });
            req_writer.send(StartExtractRequest {
                player: second,
                portal: tear,
            });
        }
        app.update();

        let results = app.world().resource::<Events<StartExtractResult>>();
        let collected: Vec<_> = results.get_reader().read(results).cloned().collect();
        let started: Vec<Entity> = collected
            .iter()
            .filter_map(|r| match r {
                StartExtractResult::Started { player, .. } => Some(*player),
                _ => None,
            })
            .collect();
        let rejected_with_occupied: Vec<Entity> = collected
            .iter()
            .filter_map(|r| match r {
                StartExtractResult::Rejected {
                    player,
                    reason: ExtractRejectionReason::PortalOccupied,
                    ..
                } => Some(*player),
                _ => None,
            })
            .collect();
        assert_eq!(
            started.len(),
            1,
            "同 tick 双请求只应有 1 个 Started（in-loop reservation 锁住第二个），实际 started={:?} all={:?}",
            started,
            collected
        );
        assert_eq!(
            rejected_with_occupied.len(),
            1,
            "同 tick 双请求第二个应被 PortalOccupied 拒绝，实际 rejected_occupied={:?} all={:?}",
            rejected_with_occupied,
            collected
        );
    }

    /// plan-tsy-raceout-v1 §4 Q-RC4：CollapseTear 多 portal 时各自独立计数；
    /// 同 family 下两个 CollapseTear，两位玩家各占一个应都成功。
    #[test]
    fn collapse_tear_independent_portals_allow_parallel_extracts() {
        let mut app = app_with_extract_system(start_extract_request);
        let tear_a = app
            .world_mut()
            .spawn(portal("tsy_lingxu_01", RiftKind::CollapseTear, DVec3::ZERO))
            .id();
        let tear_b = app
            .world_mut()
            .spawn(portal(
                "tsy_lingxu_01",
                RiftKind::CollapseTear,
                DVec3::new(20.0, 0.0, 0.0),
            ))
            .id();
        let first = spawn_player(&mut app, DVec3::ZERO, Some("tsy_lingxu_01"));
        let second = spawn_player(&mut app, DVec3::new(20.0, 0.0, 0.0), Some("tsy_lingxu_01"));

        for (player, portal_entity) in [(first, tear_a), (second, tear_b)] {
            app.world_mut()
                .resource_mut::<Events<StartExtractRequest>>()
                .send(StartExtractRequest {
                    player,
                    portal: portal_entity,
                });
        }
        app.update();

        let results = app.world().resource::<Events<StartExtractResult>>();
        let collected: Vec<_> = results.get_reader().read(results).cloned().collect();
        let started_players: Vec<_> = collected
            .iter()
            .filter_map(|r| match r {
                StartExtractResult::Started { player, .. } => Some(*player),
                _ => None,
            })
            .collect();
        assert!(
            started_players.contains(&first) && started_players.contains(&second),
            "两位玩家在不同 CollapseTear 上应都 Started，实际：{:?}",
            collected
        );
    }

    /// plan-tsy-raceout-v1 §4 Q-RC4：同一 CollapseTear 上首位玩家撤离结束（ExtractProgress 移除）后，
    /// 第二个玩家应能重新使用该 portal。锁是"在撤"而不是"曾撤"。
    #[test]
    fn collapse_tear_unlocks_after_first_player_completes() {
        let mut app = app_with_extract_system(start_extract_request);
        let tear = app
            .world_mut()
            .spawn(portal("tsy_lingxu_01", RiftKind::CollapseTear, DVec3::ZERO))
            .id();
        let first = spawn_player(&mut app, DVec3::ZERO, Some("tsy_lingxu_01"));
        let second = spawn_player(&mut app, DVec3::ZERO, Some("tsy_lingxu_01"));

        app.world_mut()
            .resource_mut::<Events<StartExtractRequest>>()
            .send(StartExtractRequest {
                player: first,
                portal: tear,
            });
        app.update();
        // 模拟首位完成撤离 — 移除 ExtractProgress
        app.world_mut()
            .entity_mut(first)
            .remove::<ExtractProgress>();
        app.world_mut()
            .resource_mut::<Events<StartExtractResult>>()
            .clear();

        app.world_mut()
            .resource_mut::<Events<StartExtractRequest>>()
            .send(StartExtractRequest {
                player: second,
                portal: tear,
            });
        app.update();

        let results = app.world().resource::<Events<StartExtractResult>>();
        let collected: Vec<_> = results.get_reader().read(results).cloned().collect();
        assert!(
            collected.iter().any(
                |r| matches!(r, StartExtractResult::Started { player, .. } if *player == second)
            ),
            "首位撤完后 portal 应解锁，第二位 Started 实际：{:?}",
            collected
        );
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
