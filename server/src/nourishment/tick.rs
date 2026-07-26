use std::collections::HashMap;

use valence::movement::MovementEvent;
use valence::prelude::{
    bevy_ecs, Client, Commands, Component, Entity, EventReader, Query, Res, ResMut, Resource, With,
    Without,
};

use crate::combat::CombatClock;
use crate::cultivation::components::Cultivation;
use crate::movement::{MovementAction, MovementState};

use super::{
    nourishment_loss_multiplier, Nourishment, NOURISH_DASH_ACTIVITY_MULTIPLIER,
    NOURISH_HYDRATION_LOSS_PER_MIN, NOURISH_IDLE_ACTIVITY_MULTIPLIER,
    NOURISH_MOVEMENT_EPSILON_BLOCKS, NOURISH_MOVE_ACTIVITY_MULTIPLIER,
    NOURISH_SATIETY_LOSS_PER_MIN, NOURISH_SWEEP_INTERVAL_TICKS, NOURISH_TICKS_PER_MINUTE,
};

/// Per-session activity observed since the previous global nourishment sweep.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Component)]
pub struct NourishmentActivityWindow {
    last_observed_tick: Option<u64>,
    had_qualifying_movement: bool,
    had_dash: bool,
}

impl NourishmentActivityWindow {
    fn observe(&mut self, tick: u64, qualifying_movement: bool, dashing: bool) {
        if self
            .last_observed_tick
            .is_some_and(|last_tick| tick <= last_tick)
        {
            return;
        }
        self.last_observed_tick = Some(tick);
        self.had_qualifying_movement |= qualifying_movement;
        self.had_dash |= dashing;
    }

    fn activity_multiplier(self) -> f32 {
        if self.had_dash {
            NOURISH_DASH_ACTIVITY_MULTIPLIER
        } else if self.had_qualifying_movement {
            NOURISH_MOVE_ACTIVITY_MULTIPLIER
        } else {
            NOURISH_IDLE_ACTIVITY_MULTIPLIER
        }
    }

    fn clear_flags_after_sweep(&mut self) {
        self.had_qualifying_movement = false;
        self.had_dash = false;
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    #[cfg(test)]
    pub(crate) fn observed_flags(self) -> (bool, bool) {
        (self.had_qualifying_movement, self.had_dash)
    }
}

/// Global, non-persistent claim for the 200-tick CombatClock sweep boundary.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Resource)]
pub struct NourishmentSweepGate {
    last_processed_tick: Option<u64>,
}

impl NourishmentSweepGate {
    fn claim_boundary(&mut self, tick: u64) -> bool {
        if !tick.is_multiple_of(u64::from(NOURISH_SWEEP_INTERVAL_TICKS))
            || self
                .last_processed_tick
                .is_some_and(|last_tick| tick <= last_tick)
        {
            return false;
        }
        self.last_processed_tick = Some(tick);
        true
    }
}

pub fn attach_activity_window(
    mut commands: Commands,
    players: Query<Entity, (With<Client>, Without<NourishmentActivityWindow>)>,
) {
    for entity in &players {
        commands
            .entity(entity)
            .insert(NourishmentActivityWindow::default());
    }
}

fn horizontal_distance(event: &MovementEvent) -> f64 {
    let delta_x = event.position.x - event.old_position.x;
    let delta_z = event.position.z - event.old_position.z;
    delta_x.hypot(delta_z)
}

/// Samples each client's events and accepted movement state at most once per monotonic CombatClock
/// tick. Event segments are summed per entity so fragmented and reverse travel both count.
pub fn sample_activity(
    clock: Res<CombatClock>,
    mut events: EventReader<MovementEvent>,
    mut players: Query<
        (
            Entity,
            &mut NourishmentActivityWindow,
            Option<&MovementState>,
        ),
        With<Client>,
    >,
) {
    let mut horizontal_distances = HashMap::<Entity, f64>::new();
    for event in events.read() {
        *horizontal_distances.entry(event.client).or_default() += horizontal_distance(event);
    }

    for (entity, mut activity, movement_state) in &mut players {
        let qualifying_movement = horizontal_distances
            .get(&entity)
            .is_some_and(|distance| *distance > NOURISH_MOVEMENT_EPSILON_BLOCKS);
        let dashing = movement_state.is_some_and(|state| state.action == MovementAction::Dashing);
        activity.observe(clock.tick, qualifying_movement, dashing);
    }
}

pub fn sweep_losses(window: NourishmentActivityWindow, realm_multiplier: f32) -> (f32, f32) {
    let sweep_minutes = NOURISH_SWEEP_INTERVAL_TICKS as f32 / NOURISH_TICKS_PER_MINUTE;
    let realm_multiplier = if realm_multiplier.is_finite() {
        realm_multiplier.max(0.0)
    } else {
        1.0
    };
    let activity_multiplier = window.activity_multiplier();
    (
        NOURISH_SATIETY_LOSS_PER_MIN * sweep_minutes * activity_multiplier * realm_multiplier,
        NOURISH_HYDRATION_LOSS_PER_MIN * sweep_minutes * activity_multiplier * realm_multiplier,
    )
}

pub fn tick_nourishment(
    clock: Res<CombatClock>,
    mut gate: ResMut<NourishmentSweepGate>,
    mut players: Query<
        (
            &mut Nourishment,
            &mut NourishmentActivityWindow,
            &Cultivation,
        ),
        With<Client>,
    >,
) {
    if !gate.claim_boundary(clock.tick) {
        return;
    }

    for (mut nourishment, mut activity_window, cultivation) in &mut players {
        let (satiety_loss, hydration_loss) = sweep_losses(
            *activity_window,
            nourishment_loss_multiplier(cultivation.realm),
        );
        nourishment.apply_loss(satiety_loss, hydration_loss);
        activity_window.clear_flags_after_sweep();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::Realm;
    use crate::nourishment::NOURISH_SPAWN_VALUE;
    use valence::prelude::{App, DVec3, IntoSystemConfigs, Look, Update};
    use valence::testing::create_mock_client;

    fn movement_event(entity: Entity, old: [f64; 3], new: [f64; 3]) -> MovementEvent {
        MovementEvent {
            client: entity,
            position: DVec3::from_array(new),
            old_position: DVec3::from_array(old),
            look: Look::default(),
            old_look: Look::default(),
            on_ground: true,
            old_on_ground: true,
        }
    }

    fn app_at(tick: u64) -> App {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick });
        app.insert_resource(NourishmentSweepGate::default());
        app.add_event::<MovementEvent>();
        app.add_systems(Update, (sample_activity, tick_nourishment).chain());
        app
    }

    fn player(app: &mut App, name: &str) -> Entity {
        app.world_mut()
            .spawn((
                create_mock_client(name).0,
                Cultivation {
                    realm: Realm::Awaken,
                    ..Default::default()
                },
                MovementState::default(),
                Nourishment::spawn_default(),
                NourishmentActivityWindow::default(),
            ))
            .id()
    }

    fn assert_unchanged(app: &App, entity: Entity, message: &str) {
        assert_eq!(
            *app.world().get::<Nourishment>(entity).unwrap(),
            Nourishment::spawn_default(),
            "{message}"
        );
    }

    #[test]
    fn global_sweep_boundaries_are_199_200_201_and_tick_zero() {
        for (tick, loses) in [(199, false), (200, true), (201, false), (0, true)] {
            let mut app = app_at(tick);
            let entity = player(&mut app, "Boundary");
            app.update();
            assert_eq!(
                app.world().get::<Nourishment>(entity).unwrap().satiety < NOURISH_SPAWN_VALUE,
                loses,
                "CombatClock tick {tick} must use its natural global boundary semantics"
            );
        }
    }

    #[test]
    fn boundary_claim_is_global_exactly_once_and_rejects_regression() {
        let mut app = app_at(200);
        let first = player(&mut app, "First");
        let second = player(&mut app, "Second");
        app.update();
        let after_first = *app.world().get::<Nourishment>(first).unwrap();
        assert!(after_first.satiety < NOURISH_SPAWN_VALUE);
        assert_eq!(
            *app.world().get::<Nourishment>(second).unwrap(),
            after_first
        );
        app.update();
        assert_eq!(*app.world().get::<Nourishment>(first).unwrap(), after_first);
        app.world_mut().resource_mut::<CombatClock>().tick = 199;
        app.update();
        app.world_mut().resource_mut::<CombatClock>().tick = 200;
        app.update();
        assert_eq!(
            *app.world().get::<Nourishment>(first).unwrap(),
            after_first,
            "200 -> 199 -> 200 must not replay an already claimed global boundary"
        );
    }

    #[test]
    fn skipped_boundary_is_not_a_debt_and_late_joiner_participates() {
        let mut app = app_at(199);
        let entity = player(&mut app, "LateJoin");
        app.update();
        assert_unchanged(&app, entity, "tick 199 is not a boundary");
        app.world_mut().resource_mut::<CombatClock>().tick = 201;
        app.update();
        assert_unchanged(&app, entity, "skipping 200 must not collect a debt at 201");
        app.world_mut().resource_mut::<CombatClock>().tick = 400;
        app.update();
        assert!(app.world().get::<Nourishment>(entity).unwrap().satiety < NOURISH_SPAWN_VALUE);
    }

    #[test]
    fn late_joiner_at_global_199_participates_in_the_very_next_200_sweep() {
        // 全局相位契约：boundary 只由 CombatClock 判定，不存在任何"个人已存活 200
        // tick"门槛。一个在全局 tick 199（边界前一 tick）才加入的玩家，必须在紧接着
        // 的 tick 200 就跟所有人一起结算，即便它自己只存在了 1 个 tick。
        let mut app = app_at(199);
        let entity = player(&mut app, "JustJoined");
        app.update();
        assert_unchanged(
            &app,
            entity,
            "joining exactly at tick 199 must not itself trigger a settlement",
        );
        app.world_mut().resource_mut::<CombatClock>().tick = 200;
        app.update();
        assert!(
            app.world().get::<Nourishment>(entity).unwrap().satiety < NOURISH_SPAWN_VALUE,
            "a player with only 1 personal tick of existence must still be swept on the \
             very next global 200 boundary"
        );
    }

    #[test]
    fn boundary_activity_is_sampled_before_sweep_and_then_reset() {
        let mut app = app_at(200);
        let entity = player(&mut app, "BoundaryActivity");
        app.world_mut()
            .send_event(movement_event(entity, [0.0, 0.0, 0.0], [0.051, 0.0, 0.0]));
        app.update();
        let (moving_loss, _) = sweep_losses(
            NourishmentActivityWindow {
                had_qualifying_movement: true,
                ..Default::default()
            },
            1.0,
        );
        assert!(
            (app.world().get::<Nourishment>(entity).unwrap().satiety - (80.0 - moving_loss)).abs()
                < 1e-6
        );
        assert_eq!(
            app.world()
                .get::<NourishmentActivityWindow>(entity)
                .unwrap()
                .observed_flags(),
            (false, false),
            "settling must clear session flags"
        );
    }

    #[test]
    fn sampling_aggregates_strict_non_negative_horizontal_segments() {
        let mut app = app_at(42);
        let entity = player(&mut app, "Fragments");
        for (old, new) in [
            ([0.0, 0.0, 0.0], [0.03, 0.0, 0.0]),
            ([0.03, 0.0, 0.0], [0.06, 0.0, 0.0]),
            ([0.06, 0.0, 0.0], [0.03, 0.0, 0.0]),
            ([0.03, 0.0, 0.0], [0.03, 4.0, 0.0]),
        ] {
            app.world_mut().send_event(movement_event(entity, old, new));
        }
        app.update();
        assert_eq!(
            app.world()
                .get::<NourishmentActivityWindow>(entity)
                .unwrap()
                .observed_flags(),
            (true, false),
            "two 0.03 fragments and reverse travel must accumulate above epsilon; vertical travel alone must not"
        );
    }

    #[test]
    fn exactly_epsilon_vertical_and_zero_do_not_qualify() {
        for (old, new) in [
            ([0.0, 0.0, 0.0], [0.05, 0.0, 0.0]),
            ([0.0, 0.0, 0.0], [0.0, 3.0, 0.0]),
            ([0.0, 0.0, 0.0], [0.0, 0.0, 0.0]),
        ] {
            let mut app = app_at(42);
            let entity = player(&mut app, "Threshold");
            app.world_mut().send_event(movement_event(entity, old, new));
            app.update();
            assert_eq!(
                app.world()
                    .get::<NourishmentActivityWindow>(entity)
                    .unwrap()
                    .observed_flags(),
                (false, false)
            );
        }
    }

    #[test]
    fn dash_has_priority_and_duplicate_or_regressed_ticks_cannot_pollute() {
        let mut app = app_at(42);
        let entity = player(&mut app, "Dash");
        app.world_mut()
            .entity_mut(entity)
            .get_mut::<MovementState>()
            .unwrap()
            .action = MovementAction::Dashing;
        app.world_mut()
            .send_event(movement_event(entity, [0.0; 3], [0.06, 0.0, 0.0]));
        app.update();
        assert_eq!(
            app.world()
                .get::<NourishmentActivityWindow>(entity)
                .unwrap()
                .activity_multiplier(),
            NOURISH_DASH_ACTIVITY_MULTIPLIER
        );
        app.world_mut()
            .entity_mut(entity)
            .get_mut::<MovementState>()
            .unwrap()
            .action = MovementAction::None;
        app.world_mut().resource_mut::<CombatClock>().tick = 41;
        app.world_mut()
            .send_event(movement_event(entity, [0.0; 3], [0.06, 0.0, 0.0]));
        app.update();
        assert_eq!(
            app.world()
                .get::<NourishmentActivityWindow>(entity)
                .unwrap()
                .observed_flags(),
            (true, true)
        );
    }
}
