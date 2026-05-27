use big_brain::prelude::{ActionBuilder, ActionState, Actor};
use valence::client::ClientMarker;
use valence::prelude::{
    bevy_ecs, Commands, Component, DVec3, Entity, EntityKind, EventWriter, Position, Query, Res,
    ResMut, With, Without,
};

use crate::cultivation::breakthrough::{
    breakthrough_qi_cost, try_breakthrough, BreakthroughError, BreakthroughSuccess, RollSource,
    XorshiftRoll,
};
use crate::cultivation::components::{recover_current_qi, Cultivation, MeridianSystem, Realm};
use crate::cultivation::meridian_open::MeridianTarget;
use crate::cultivation::topology::MeridianTopology;
use crate::cultivation::tribulation::InitiateXuhuaTribulation;
use crate::npc::hunger::{Hunger, HungerConfig};
use crate::npc::lifecycle::{NpcRetireRequest, PendingRetirement};
use crate::npc::navigator::Navigator;
use crate::npc::patrol::NpcPatrol;
use crate::npc::schedule::{
    nearest_poi_for_activity, rest_tick, NpcDailySchedule, NpcHomeBase, ScheduleActivity,
    DAILY_POI_SEARCH_RADIUS,
};
use crate::npc::spawn::{NpcBlackboard, NpcMarker};
use crate::npc::tribulation::{AscensionQuotaStore, NpcTribulationPacing};
use crate::world::poi_novice::PoiNoviceRegistry;
use crate::world::zone::ZoneRegistry;

use super::scorers_cultivation::{next_realm, pick_next_meridian_to_open};
use super::{
    CultivateState, GoToPoiAction, GoToPoiState, RestState, SeclusionState, StallState,
    WanderState, CULTIVATE_DRIFT_RADIUS, CULTIVATE_DRIFT_SPEED, CULTIVATE_MAX_TICKS,
    CULTIVATE_MIN_ZONE_QI, FLEE_CULTIVATOR_SPEED_FACTOR, FLEE_CULTIVATOR_SUCCESS_DISTANCE,
    FLEE_CULTIVATOR_WAYPOINT_DISTANCE, GO_TO_POI_ARRIVAL_DISTANCE, NPC_TRIBULATION_WAVES_DEFAULT,
    REST_MAX_TICKS, REST_RECOVERY_RATE_PER_TICK, RETURN_HOME_ARRIVAL_DISTANCE,
    ROGUE_BREAKTHROUGH_MATERIAL_BONUS, SECLUSION_CYCLE_TICKS, STALL_MAX_TICKS, STALL_MIN_TICKS,
    WANDER_ARRIVAL_DISTANCE, WANDER_MAX_RADIUS, WANDER_MAX_TICKS, WANDER_MIN_RADIUS,
    WANDER_SPEED_FACTOR,
};

use crate::cultivation::tick::CultivationClock;
use crate::npc::movement::GameTick;

// ---------------------------------------------------------------------------
// RetireAction
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Component)]
pub struct RetireAction;

impl ActionBuilder for RetireAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("RetireAction")
    }
}

pub(crate) fn retire_action_system(
    mut commands: Commands,
    npcs: Query<
        (
            Option<&PendingRetirement>,
            &crate::npc::lifecycle::NpcLifespan,
        ),
        With<NpcMarker>,
    >,
    mut actions: Query<(&Actor, &mut ActionState), With<RetireAction>>,
) {
    for (Actor(actor), mut state) in &mut actions {
        let Ok((pending_retirement, lifespan)) = npcs.get(*actor) else {
            *state = ActionState::Failure;
            continue;
        };

        match *state {
            ActionState::Requested => {
                if pending_retirement.is_none() {
                    commands.entity(*actor).insert(PendingRetirement);
                }
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                if pending_retirement.is_some() || lifespan.is_expired() {
                    continue;
                }
                *state = ActionState::Success;
            }
            ActionState::Cancelled => {
                commands.entity(*actor).remove::<PendingRetirement>();
                *state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

pub(crate) fn emit_retire_request_on_pending_added(
    query: Query<Entity, (bevy_ecs::query::Added<PendingRetirement>, With<NpcMarker>)>,
    mut requests: EventWriter<NpcRetireRequest>,
) {
    for entity in &query {
        requests.send(NpcRetireRequest { entity });
    }
}

// ---------------------------------------------------------------------------
// FleeCultivatorAction
// ---------------------------------------------------------------------------

/// Flee away from nearest cultivator (runs farther than regular FleeAction).
#[derive(Clone, Copy, Debug, Component)]
pub struct FleeCultivatorAction;

impl ActionBuilder for FleeCultivatorAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("FleeCultivatorAction")
    }
}

type NpcFleeQueryItem<'a> = (
    &'a Position,
    &'a NpcBlackboard,
    &'a NpcPatrol,
    &'a mut Navigator,
);
type NpcGoalQueryFilter = (With<NpcMarker>, With<EntityKind>, Without<ClientMarker>);

pub(crate) fn flee_cultivator_action_system(
    mut npcs: Query<NpcFleeQueryItem<'_>, NpcGoalQueryFilter>,
    mut actions: Query<(&Actor, &mut ActionState), With<FleeCultivatorAction>>,
    zone_registry: Option<Res<ZoneRegistry>>,
) {
    for (Actor(actor), mut state) in &mut actions {
        let Ok((npc_position, blackboard, patrol, mut navigator)) = npcs.get_mut(*actor) else {
            *state = ActionState::Failure;
            continue;
        };

        match *state {
            ActionState::Requested => {
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                if blackboard.player_distance > FLEE_CULTIVATOR_SUCCESS_DISTANCE as f32 {
                    navigator.stop();
                    *state = ActionState::Success;
                    continue;
                }

                let Some(target_pos) = blackboard.target_position else {
                    navigator.stop();
                    *state = ActionState::Failure;
                    continue;
                };

                let flee_target = compute_cultivator_flee_target(
                    npc_position.get(),
                    target_pos,
                    zone_registry.as_deref(),
                    &patrol.home_zone,
                );
                navigator.set_goal(flee_target, FLEE_CULTIVATOR_SPEED_FACTOR);
            }
            ActionState::Cancelled => {
                navigator.stop();
                *state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

fn compute_cultivator_flee_target(
    npc_pos: DVec3,
    player_pos: DVec3,
    zone_registry: Option<&ZoneRegistry>,
    home_zone: &str,
) -> DVec3 {
    let mut flee_dir = npc_pos - player_pos;
    flee_dir.y = 0.0;

    let dir = if flee_dir.length_squared() <= f64::EPSILON {
        DVec3::new(1.0, 0.0, 0.0)
    } else {
        flee_dir.normalize()
    };

    let target = npc_pos + dir * FLEE_CULTIVATOR_WAYPOINT_DISTANCE;

    if let Some(zone) = zone_registry.and_then(|r| r.find_zone_by_name(home_zone)) {
        let (min, max) = zone.bounds;
        DVec3::new(
            target.x.clamp(min.x, max.x),
            target.y,
            target.z.clamp(min.z, max.z),
        )
    } else {
        target
    }
}

// ---------------------------------------------------------------------------
// FarmAction
// ---------------------------------------------------------------------------

/// In-place farming: stop Navigator, replenish Hunger per tick, Success when full.
#[derive(Clone, Copy, Debug, Component)]
pub struct FarmAction;

impl ActionBuilder for FarmAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("FarmAction")
    }
}

pub(crate) fn farm_action_system(
    mut npcs: Query<(&mut Navigator, &mut Hunger), With<NpcMarker>>,
    mut actions: Query<(&Actor, &mut ActionState), With<FarmAction>>,
    hunger_config: Option<Res<HungerConfig>>,
) {
    let restore = hunger_config
        .as_deref()
        .map(|c| c.farm_restore_per_tick)
        .unwrap_or(0.0);

    for (Actor(actor), mut state) in &mut actions {
        let Ok((mut navigator, mut hunger)) = npcs.get_mut(*actor) else {
            *state = ActionState::Failure;
            continue;
        };

        match *state {
            ActionState::Requested => {
                navigator.stop();
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                hunger.replenish(restore);
                if hunger.value >= 0.99 {
                    *state = ActionState::Success;
                }
            }
            ActionState::Cancelled => {
                *state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

// ---------------------------------------------------------------------------
// WanderAction
// ---------------------------------------------------------------------------

/// Random wander, Success on arrival or timeout.
#[derive(Clone, Copy, Debug, Component)]
pub struct WanderAction;

impl ActionBuilder for WanderAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("WanderAction")
    }
}

pub(crate) fn wander_action_system(
    mut npcs: Query<(&Position, &NpcPatrol, &mut Navigator, &mut WanderState), With<NpcMarker>>,
    mut actions: Query<(&Actor, &mut ActionState), With<WanderAction>>,
    game_tick: Option<Res<GameTick>>,
    zone_registry: Option<Res<ZoneRegistry>>,
) {
    let tick = game_tick.as_deref().map(|t| t.0).unwrap_or(0);

    for (Actor(actor), mut state) in &mut actions {
        let Ok((position, patrol, mut navigator, mut wander)) = npcs.get_mut(*actor) else {
            *state = ActionState::Failure;
            continue;
        };

        match *state {
            ActionState::Requested => {
                let home = zone_registry
                    .as_deref()
                    .and_then(|r| r.find_zone_by_name(&patrol.home_zone));
                let target = wander_target_for(position.get(), actor.index(), tick, home);
                navigator.set_goal(target, WANDER_SPEED_FACTOR);
                wander.destination = Some(target);
                wander.elapsed_ticks = 0;
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                wander.elapsed_ticks = wander.elapsed_ticks.saturating_add(1);
                let arrived = wander
                    .destination
                    .map(|dest| position.get().distance(dest) <= WANDER_ARRIVAL_DISTANCE)
                    .unwrap_or(true);
                if arrived || wander.elapsed_ticks >= WANDER_MAX_TICKS {
                    navigator.stop();
                    wander.destination = None;
                    *state = ActionState::Success;
                }
            }
            ActionState::Cancelled => {
                navigator.stop();
                wander.destination = None;
                *state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

// ---------------------------------------------------------------------------
// GoToPoiAction system
// ---------------------------------------------------------------------------

impl ActionBuilder for GoToPoiAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(self.clone());
    }

    fn label(&self) -> Option<&str> {
        Some("GoToPoiAction")
    }
}

fn poi_arrival_required_ticks(activity: ScheduleActivity) -> u32 {
    match activity {
        ScheduleActivity::Forage => 1,
        ScheduleActivity::Cultivate => CULTIVATE_MAX_TICKS,
        ScheduleActivity::Trade => STALL_MIN_TICKS,
        ScheduleActivity::Rest => REST_MAX_TICKS,
        ScheduleActivity::Patrol | ScheduleActivity::Socialize | ScheduleActivity::Wander => 1,
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn go_to_poi_action_system(
    mut npcs: Query<
        (
            &Position,
            &NpcPatrol,
            &mut Navigator,
            &mut GoToPoiState,
            Option<&NpcDailySchedule>,
            Option<&mut Hunger>,
            Option<&mut Cultivation>,
        ),
        With<NpcMarker>,
    >,
    mut actions: Query<(&Actor, &GoToPoiAction, &mut ActionState)>,
    clock: Option<Res<CultivationClock>>,
    game_tick: Option<Res<GameTick>>,
    pois: Option<Res<PoiNoviceRegistry>>,
    zone_registry: Option<Res<ZoneRegistry>>,
) {
    let clock_tick = clock.as_deref().map(|clock| clock.tick).unwrap_or_default();
    let game_tick = game_tick.as_deref().map(|tick| tick.0).unwrap_or_default();

    for (Actor(actor), action, mut state) in &mut actions {
        let Ok((position, patrol, mut navigator, mut poi_state, schedule, hunger, cultivation)) =
            npcs.get_mut(*actor)
        else {
            *state = ActionState::Failure;
            continue;
        };

        match *state {
            ActionState::Requested => {
                let activity = action
                    .arrive_action
                    .or_else(|| {
                        schedule.map(|schedule| schedule.activity_for(clock_tick, schedule.seed))
                    })
                    .unwrap_or(ScheduleActivity::Wander);
                let target = action
                    .target_poi
                    .as_deref()
                    .and_then(|id| pois.as_deref().and_then(|pois| pois.by_id(id)))
                    .map(|site| (Some(site.id.clone()), site.position_vec(), false))
                    .or_else(|| {
                        nearest_poi_for_activity(
                            pois.as_deref(),
                            position.get(),
                            activity,
                            DAILY_POI_SEARCH_RADIUS,
                        )
                        .map(|pos| (None, pos, false))
                    });
                let (target_poi, destination, fallback_wander) = target.unwrap_or_else(|| {
                    let home = zone_registry
                        .as_deref()
                        .and_then(|zones| zones.find_zone_by_name(&patrol.home_zone));
                    (
                        None,
                        wander_target_for(position.get(), actor.index(), game_tick, home),
                        true,
                    )
                });

                navigator.set_goal(destination, WANDER_SPEED_FACTOR);
                poi_state.target_poi = target_poi;
                poi_state.destination = Some(destination);
                poi_state.arrive_action = Some(activity);
                poi_state.elapsed_ticks = 0;
                poi_state.arrival_ticks = 0;
                poi_state.fallback_wander = fallback_wander;
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                poi_state.elapsed_ticks = poi_state.elapsed_ticks.saturating_add(1);
                let arrived = poi_state
                    .destination
                    .map(|dest| position.get().distance(dest) <= GO_TO_POI_ARRIVAL_DISTANCE)
                    .unwrap_or(true);

                if arrived && !poi_state.fallback_wander {
                    let activity = poi_state.arrive_action.unwrap_or(ScheduleActivity::Wander);
                    if poi_state.arrival_ticks == 0 {
                        let mut hunger = hunger;
                        let mut cultivation = cultivation;
                        finish_poi_arrival(
                            activity,
                            hunger.as_deref_mut(),
                            cultivation.as_deref_mut(),
                        );
                        navigator.stop();
                        poi_state.destination = None;
                        poi_state.target_poi = None;
                    }
                    poi_state.arrival_ticks = poi_state.arrival_ticks.saturating_add(1);
                    if poi_state.arrival_ticks >= poi_arrival_required_ticks(activity) {
                        poi_state.arrival_ticks = 0;
                        *state = ActionState::Success;
                    }
                    continue;
                }

                if arrived || poi_state.elapsed_ticks >= action.timeout_ticks {
                    navigator.stop();
                    poi_state.destination = None;
                    poi_state.target_poi = None;
                    poi_state.arrival_ticks = 0;
                    *state = ActionState::Success;
                }
            }
            ActionState::Cancelled => {
                navigator.stop();
                poi_state.destination = None;
                poi_state.target_poi = None;
                poi_state.arrival_ticks = 0;
                *state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

fn finish_poi_arrival(
    activity: ScheduleActivity,
    hunger: Option<&mut Hunger>,
    cultivation: Option<&mut Cultivation>,
) {
    match activity {
        ScheduleActivity::Forage => {
            if let Some(hunger) = hunger {
                hunger.replenish(0.1);
            }
        }
        ScheduleActivity::Cultivate => {
            if let Some(cultivation) = cultivation {
                recover_current_qi(cultivation, cultivation.qi_max * 0.02);
            }
        }
        ScheduleActivity::Trade
        | ScheduleActivity::Patrol
        | ScheduleActivity::Rest
        | ScheduleActivity::Socialize
        | ScheduleActivity::Wander => {}
    }
}

// ---------------------------------------------------------------------------
// StallAction
// ---------------------------------------------------------------------------

/// Wait at trade spot for player proximity.
#[derive(Clone, Copy, Debug, Component)]
pub struct StallAction;

impl ActionBuilder for StallAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("StallAction")
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn stall_action_system(
    players: Query<&Position, With<ClientMarker>>,
    mut npcs: Query<(&Position, &mut Navigator, &mut StallState), With<NpcMarker>>,
    mut actions: Query<(&Actor, &mut ActionState), With<StallAction>>,
    pois: Option<Res<PoiNoviceRegistry>>,
) {
    for (Actor(actor), mut state) in &mut actions {
        let Ok((position, mut navigator, mut stall)) = npcs.get_mut(*actor) else {
            *state = ActionState::Failure;
            continue;
        };

        match *state {
            ActionState::Requested => {
                stall.elapsed_ticks = 0;
                let current = position.get();
                let trade_destination = nearest_poi_for_activity(
                    pois.as_deref(),
                    current,
                    ScheduleActivity::Trade,
                    DAILY_POI_SEARCH_RADIUS,
                );
                let Some(destination) = trade_destination else {
                    navigator.stop();
                    stall.destination = None;
                    stall.facing_target = None;
                    *state = ActionState::Failure;
                    continue;
                };
                if current.distance(destination) > GO_TO_POI_ARRIVAL_DISTANCE {
                    navigator.set_goal(destination, WANDER_SPEED_FACTOR);
                    stall.destination = Some(destination);
                    stall.facing_target = Some(stall_facing_target(current, destination));
                } else {
                    navigator.stop();
                    stall.destination = None;
                    stall.facing_target = Some(stall_facing_target(current, destination));
                }
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                if let Some(destination) = stall.destination {
                    let current = position.get();
                    if current.distance(destination) > GO_TO_POI_ARRIVAL_DISTANCE {
                        stall.facing_target = Some(stall_facing_target(current, destination));
                        continue;
                    }
                    navigator.stop();
                    stall.destination = None;
                    stall.elapsed_ticks = 0;
                    stall.facing_target = Some(stall_facing_target(current, destination));
                }
                stall.elapsed_ticks = stall.elapsed_ticks.saturating_add(1);
                let player_near = players
                    .iter()
                    .any(|player| position.get().distance(player.get()) <= 8.0);
                if (player_near && stall.elapsed_ticks >= STALL_MIN_TICKS)
                    || stall.elapsed_ticks >= STALL_MAX_TICKS
                {
                    *state = ActionState::Success;
                }
            }
            ActionState::Cancelled => {
                stall.elapsed_ticks = 0;
                stall.destination = None;
                navigator.stop();
                *state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

pub(crate) fn stall_facing_target(position: DVec3, path_hint: DVec3) -> DVec3 {
    if position.distance(path_hint) <= f64::EPSILON {
        DVec3::new(position.x + 1.0, position.y, position.z)
    } else {
        DVec3::new(path_hint.x, position.y, path_hint.z)
    }
}

// ---------------------------------------------------------------------------
// ReturnHomeAction + RestAction
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Component)]
pub struct ReturnHomeAction;

impl ActionBuilder for ReturnHomeAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("ReturnHomeAction")
    }
}

/// Rest at HomeBase, recover hunger / qi.
#[derive(Clone, Copy, Debug, Component)]
pub struct RestAction;

impl ActionBuilder for RestAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("RestAction")
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn return_home_action_system(
    mut npcs: Query<
        (
            &Position,
            &mut Navigator,
            &NpcHomeBase,
            Option<&mut Hunger>,
            Option<&mut Cultivation>,
            &mut RestState,
        ),
        With<NpcMarker>,
    >,
    mut actions: Query<(&Actor, &mut ActionState), With<ReturnHomeAction>>,
) {
    for (Actor(actor), mut state) in &mut actions {
        let Ok((position, mut navigator, home, hunger, cultivation, mut rest)) =
            npcs.get_mut(*actor)
        else {
            *state = ActionState::Failure;
            continue;
        };
        let home_pos = home.center();

        match *state {
            ActionState::Requested => {
                rest.elapsed_ticks = 0;
                navigator.set_goal(home_pos, WANDER_SPEED_FACTOR);
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                if position.get().distance(home_pos) <= RETURN_HOME_ARRIVAL_DISTANCE {
                    navigator.stop();
                    rest.elapsed_ticks = rest.elapsed_ticks.saturating_add(1);
                    let mut hunger = hunger;
                    let mut cultivation = cultivation;
                    rest_tick(
                        hunger.as_deref_mut(),
                        cultivation.as_deref_mut(),
                        home.quality,
                        REST_RECOVERY_RATE_PER_TICK,
                    );
                    if rest.elapsed_ticks >= REST_MAX_TICKS {
                        *state = ActionState::Success;
                    }
                } else {
                    navigator.set_goal(home_pos, WANDER_SPEED_FACTOR);
                }
            }
            ActionState::Cancelled => {
                navigator.stop();
                rest.elapsed_ticks = 0;
                *state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn rest_action_system(
    mut npcs: Query<
        (
            &mut Navigator,
            &NpcHomeBase,
            Option<&mut Hunger>,
            Option<&mut Cultivation>,
            &mut RestState,
        ),
        With<NpcMarker>,
    >,
    mut actions: Query<(&Actor, &mut ActionState), With<RestAction>>,
) {
    for (Actor(actor), mut state) in &mut actions {
        let Ok((mut navigator, home, hunger, cultivation, mut rest)) = npcs.get_mut(*actor) else {
            *state = ActionState::Failure;
            continue;
        };

        match *state {
            ActionState::Requested => {
                navigator.stop();
                rest.elapsed_ticks = 0;
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                rest.elapsed_ticks = rest.elapsed_ticks.saturating_add(1);
                let mut hunger = hunger;
                let mut cultivation = cultivation;
                rest_tick(
                    hunger.as_deref_mut(),
                    cultivation.as_deref_mut(),
                    home.quality,
                    REST_RECOVERY_RATE_PER_TICK,
                );
                if rest.elapsed_ticks >= REST_MAX_TICKS {
                    *state = ActionState::Success;
                }
            }
            ActionState::Cancelled => {
                rest.elapsed_ticks = 0;
                *state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

// ---------------------------------------------------------------------------
// CultivateAction
// ---------------------------------------------------------------------------

/// Sit-and-cultivate: attach MeridianTarget + stop Navigator, inline breakthrough at threshold.
#[derive(Clone, Copy, Debug, Component)]
pub struct CultivateAction;

impl ActionBuilder for CultivateAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("CultivateAction")
    }
}

type CultivateNpcQueryItem<'a> = (
    &'a Position,
    &'a mut Navigator,
    &'a mut Cultivation,
    &'a mut MeridianSystem,
    &'a NpcPatrol,
    &'a mut CultivateState,
    Option<&'a MeridianTarget>,
);

/// Persistent RNG state for CultivateAction's breakthrough rolls.
#[derive(Default)]
pub(crate) struct CultivateRngState(Option<u64>);

#[allow(clippy::type_complexity)]
pub(crate) fn cultivate_action_system(
    mut commands: Commands,
    mut npcs: Query<CultivateNpcQueryItem<'_>, With<NpcMarker>>,
    mut actions: Query<(&Actor, &mut ActionState), With<CultivateAction>>,
    zone_registry: Option<Res<ZoneRegistry>>,
    topology: Option<Res<MeridianTopology>>,
    mut rng_state: valence::prelude::Local<CultivateRngState>,
) {
    let zone_qi_for = |zone_name: &str| -> f64 {
        zone_registry
            .as_deref()
            .and_then(|r| r.find_zone_by_name(zone_name))
            .map(|z| z.spirit_qi)
            .unwrap_or(0.0)
    };

    let mut roll = XorshiftRoll(rng_state.0.unwrap_or(0x51_2f_c8_4d_1a_49_08_0b));

    for (Actor(actor), mut state) in &mut actions {
        let Ok((
            position,
            mut navigator,
            mut cultivation,
            mut meridians,
            patrol,
            mut cultivate,
            existing_target,
        )) = npcs.get_mut(*actor)
        else {
            *state = ActionState::Failure;
            continue;
        };

        match *state {
            ActionState::Requested => {
                cultivate.elapsed_ticks = 0;

                if matches!(cultivation.realm, Realm::Void) {
                    *state = ActionState::Success;
                    continue;
                }

                if zone_qi_for(patrol.home_zone.as_str()) < CULTIVATE_MIN_ZONE_QI {
                    *state = ActionState::Failure;
                    continue;
                }

                if existing_target.is_none() {
                    if let Some(topology) = topology.as_deref() {
                        if let Some(next_m) = pick_next_meridian_to_open(&meridians, topology) {
                            commands.entity(*actor).insert(MeridianTarget(next_m));
                        }
                    }
                }
                let drift = cultivate_drift_target(position.get(), &mut roll);
                navigator.set_goal(drift, CULTIVATE_DRIFT_SPEED);
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                cultivate.elapsed_ticks = cultivate.elapsed_ticks.saturating_add(1);

                if navigator.is_idle() {
                    let drift = cultivate_drift_target(position.get(), &mut roll);
                    navigator.set_goal(drift, CULTIVATE_DRIFT_SPEED);
                }

                let need_retarget = existing_target
                    .map(|t| meridians.get(t.0).opened)
                    .unwrap_or(true);
                if need_retarget {
                    if let Some(topology) = topology.as_deref() {
                        if let Some(next_m) = pick_next_meridian_to_open(&meridians, topology) {
                            commands.entity(*actor).insert(MeridianTarget(next_m));
                        }
                    }
                }

                if let Some(next) = next_realm(cultivation.realm) {
                    let have = meridians.opened_count();
                    let need = next.required_meridians();
                    let qi_need = breakthrough_qi_cost(next);
                    if have >= need && cultivation.qi_current >= qi_need {
                        match try_breakthrough(
                            &mut cultivation,
                            &mut meridians,
                            ROGUE_BREAKTHROUGH_MATERIAL_BONUS,
                            &mut roll,
                        ) {
                            Ok(BreakthroughSuccess { to, .. }) => {
                                tracing::info!(
                                    "[bong][npc] rogue breakthrough actor={:?} to={:?}",
                                    actor,
                                    to
                                );
                                commands.entity(*actor).remove::<MeridianTarget>();
                                navigator.stop();
                                *state = ActionState::Success;
                                continue;
                            }
                            Err(BreakthroughError::RolledFailure { .. }) => {
                                navigator.stop();
                                *state = ActionState::Failure;
                                continue;
                            }
                            Err(_) => {}
                        }
                    }
                }

                if cultivate.elapsed_ticks >= CULTIVATE_MAX_TICKS {
                    navigator.stop();
                    *state = ActionState::Success;
                }
            }
            ActionState::Cancelled => {
                navigator.stop();
                commands.entity(*actor).remove::<MeridianTarget>();
                cultivate.elapsed_ticks = 0;
                *state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }

    rng_state.0 = Some(roll.0);
}

// ---------------------------------------------------------------------------
// StartDuXuAction
// ---------------------------------------------------------------------------

/// Initiate tribulation: reserve quota + send `InitiateXuhuaTribulation`, watch until success/fail.
#[derive(Clone, Copy, Debug, Component)]
pub struct StartDuXuAction;

impl ActionBuilder for StartDuXuAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("StartDuXuAction")
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn start_duxu_action_system(
    mut commands: Commands,
    mut quota: Option<ResMut<AscensionQuotaStore>>,
    mut initiate: EventWriter<InitiateXuhuaTribulation>,
    clock: Option<Res<CultivationClock>>,
    npcs: Query<
        (
            &Cultivation,
            Option<&crate::cultivation::tribulation::TribulationState>,
        ),
        With<NpcMarker>,
    >,
    mut actions: Query<(&Actor, &mut ActionState), With<StartDuXuAction>>,
) {
    for (Actor(actor), mut state) in &mut actions {
        let Ok((cultivation, in_tribulation)) = npcs.get(*actor) else {
            *state = ActionState::Failure;
            continue;
        };

        match *state {
            ActionState::Requested => {
                if in_tribulation.is_some() {
                    *state = ActionState::Executing;
                    continue;
                }
                if !matches!(cultivation.realm, Realm::Spirit) {
                    *state = ActionState::Failure;
                    continue;
                }
                let reserved = quota
                    .as_deref_mut()
                    .map(|q| q.try_reserve(*actor))
                    .unwrap_or(false);
                if !reserved {
                    tracing::info!(
                        "[bong][npc] tribulation slot exhausted or store missing, actor={:?}",
                        actor
                    );
                    *state = ActionState::Failure;
                    continue;
                }

                let started_tick = clock.as_deref().map(|c| c.tick).unwrap_or(0);
                initiate.send(InitiateXuhuaTribulation {
                    entity: *actor,
                    waves_total: NPC_TRIBULATION_WAVES_DEFAULT,
                    started_tick,
                });
                commands
                    .entity(*actor)
                    .insert(NpcTribulationPacing::default());
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                if matches!(cultivation.realm, Realm::Void) {
                    commands.entity(*actor).remove::<NpcTribulationPacing>();
                    *state = ActionState::Success;
                } else if in_tribulation.is_none() {
                    if let Some(q) = quota.as_deref_mut() {
                        q.release(*actor);
                    }
                    commands.entity(*actor).remove::<NpcTribulationPacing>();
                    *state = ActionState::Failure;
                }
            }
            ActionState::Cancelled => {
                if let Some(q) = quota.as_deref_mut() {
                    q.release(*actor);
                }
                commands.entity(*actor).remove::<NpcTribulationPacing>();
                *state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

// ---------------------------------------------------------------------------
// SeclusionAction
// ---------------------------------------------------------------------------

/// Post-Void NPC seclusion behavior: stop Navigator, idle until timer expires.
#[derive(Clone, Copy, Debug, Component)]
pub struct SeclusionAction;

impl ActionBuilder for SeclusionAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("SeclusionAction")
    }
}

pub(crate) fn seclusion_action_system(
    mut commands: Commands,
    mut npcs: Query<(&mut Navigator, Option<&mut SeclusionState>), With<NpcMarker>>,
    mut actions: Query<(&Actor, &mut ActionState), With<SeclusionAction>>,
) {
    for (Actor(actor), mut state) in &mut actions {
        let Ok((mut navigator, existing)) = npcs.get_mut(*actor) else {
            *state = ActionState::Failure;
            continue;
        };

        match *state {
            ActionState::Requested => {
                navigator.stop();
                if existing.is_none() {
                    commands.entity(*actor).insert(SeclusionState::default());
                }
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                let elapsed = match existing {
                    Some(mut s) => {
                        s.elapsed_ticks = s.elapsed_ticks.saturating_add(1);
                        s.elapsed_ticks
                    }
                    None => 0,
                };
                if elapsed >= SECLUSION_CYCLE_TICKS {
                    commands.entity(*actor).remove::<SeclusionState>();
                    *state = ActionState::Success;
                }
            }
            ActionState::Cancelled => {
                commands.entity(*actor).remove::<SeclusionState>();
                *state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Deterministic pseudo-random wander target based on (entity.index, game_tick).
pub(crate) fn wander_target_for(
    npc_pos: DVec3,
    actor_index: u32,
    game_tick: u32,
    home_zone: Option<&crate::world::zone::Zone>,
) -> DVec3 {
    let seed = (actor_index as u64)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add((game_tick as u64).wrapping_mul(0x94D0_49BB_1331_11EB));
    let angle = ((seed >> 16) % 3600) as f64 / 3600.0 * std::f64::consts::TAU;
    let radius_range = WANDER_MAX_RADIUS - WANDER_MIN_RADIUS;
    let radius = WANDER_MIN_RADIUS + ((seed >> 32) % 1000) as f64 / 1000.0 * radius_range;

    let target = DVec3::new(
        npc_pos.x + angle.cos() * radius,
        npc_pos.y,
        npc_pos.z + angle.sin() * radius,
    );

    if let Some(zone) = home_zone {
        let (min, max) = zone.bounds;
        DVec3::new(
            target.x.clamp(min.x, max.x),
            target.y,
            target.z.clamp(min.z, max.z),
        )
    } else {
        target
    }
}

fn cultivate_drift_target(origin: DVec3, roll: &mut XorshiftRoll) -> DVec3 {
    let angle = roll.roll_unit() * std::f64::consts::TAU;
    let radius = 2.0 + roll.roll_unit() * CULTIVATE_DRIFT_RADIUS;
    DVec3::new(
        origin.x + angle.cos() * radius,
        origin.y,
        origin.z + angle.sin() * radius,
    )
}
