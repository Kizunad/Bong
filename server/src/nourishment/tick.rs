use std::collections::HashMap;

use serde::{Deserialize, Deserializer, Serialize};
use valence::movement::MovementEvent;
use valence::prelude::{
    bevy_ecs, Client, Commands, Component, Entity, EventReader, Query, Res, With, Without,
};

use crate::combat::CombatClock;
use crate::cultivation::components::Cultivation;
use crate::movement::{MovementAction, MovementState};

use super::{
    nourishment_loss_multiplier, Nourishment, NOURISH_DASH_ACTIVITY_MULTIPLIER,
    NOURISH_HYDRATION_LOSS_PER_MIN, NOURISH_IDLE_ACTIVITY_MULTIPLIER,
    NOURISH_MOVEMENT_EPSILON_BLOCKS, NOURISH_MOVEMENT_LEASE_TICKS,
    NOURISH_MOVE_ACTIVITY_MULTIPLIER, NOURISH_SATIETY_LOSS_PER_MIN, NOURISH_SWEEP_INTERVAL_TICKS,
    NOURISH_TICKS_PER_MINUTE,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Component)]
pub struct NourishmentActivityWindow {
    pub idle_ticks: u32,
    pub move_ticks: u32,
    pub dash_ticks: u32,
    pub had_qualifying_movement: bool,
}

impl<'de> Deserialize<'de> for NourishmentActivityWindow {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        fn persisted_ticks(value: Option<&serde_json::Value>) -> u32 {
            value
                .and_then(serde_json::Value::as_u64)
                .and_then(|value| u32::try_from(value).ok())
                .unwrap_or_default()
        }

        fn persisted_movement(value: Option<&serde_json::Value>) -> bool {
            value
                .and_then(serde_json::Value::as_bool)
                .unwrap_or_default()
        }

        let Some(value) = Option::<serde_json::Value>::deserialize(deserializer)? else {
            return Ok(Self::default());
        };
        let Some(persisted) = value.as_object() else {
            return Ok(Self::default());
        };
        let window = Self {
            idle_ticks: persisted_ticks(persisted.get("idle_ticks")),
            move_ticks: persisted_ticks(persisted.get("move_ticks")),
            dash_ticks: persisted_ticks(persisted.get("dash_ticks")),
            had_qualifying_movement: persisted_movement(persisted.get("had_qualifying_movement")),
        };

        if window.total_ticks() < NOURISH_SWEEP_INTERVAL_TICKS {
            Ok(window)
        } else {
            Ok(Self::default())
        }
    }
}

impl NourishmentActivityWindow {
    pub fn record(&mut self, activity: NourishmentActivity) {
        match activity {
            NourishmentActivity::Idle => self.idle_ticks = self.idle_ticks.saturating_add(1),
            NourishmentActivity::Moving => self.move_ticks = self.move_ticks.saturating_add(1),
            NourishmentActivity::Dashing => self.dash_ticks = self.dash_ticks.saturating_add(1),
        }
    }

    pub fn record_qualifying_movement(&mut self) {
        self.had_qualifying_movement = true;
    }

    pub fn total_ticks(self) -> u32 {
        self.idle_ticks
            .saturating_add(self.move_ticks)
            .saturating_add(self.dash_ticks)
    }

    pub fn activity_multiplier(self) -> f32 {
        if self.dash_ticks > 0 {
            NOURISH_DASH_ACTIVITY_MULTIPLIER
        } else if self.had_qualifying_movement {
            NOURISH_MOVE_ACTIVITY_MULTIPLIER
        } else {
            NOURISH_IDLE_ACTIVITY_MULTIPLIER
        }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Component)]
pub struct NourishmentSweepCursor {
    last_processed_tick: Option<u64>,
}

impl NourishmentSweepCursor {
    fn claim_tick(&mut self, now_tick: u64) -> bool {
        if self
            .last_processed_tick
            .is_some_and(|last_processed_tick| now_tick <= last_processed_tick)
        {
            return false;
        }
        self.last_processed_tick = Some(now_tick);
        true
    }
}

pub fn attach_sweep_cursor(
    mut commands: Commands,
    players: Query<Entity, (With<Client>, Without<NourishmentSweepCursor>)>,
) {
    for entity in &players {
        commands
            .entity(entity)
            .insert(NourishmentSweepCursor::default());
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Component)]
pub struct NourishmentMovementTracker {
    last_moved_at_tick: Option<u64>,
}

impl NourishmentMovementTracker {
    fn record_movement(&mut self, now_tick: u64) {
        self.last_moved_at_tick = Some(now_tick);
    }

    fn elapsed_ticks(self, now_tick: u64) -> Option<u64> {
        self.last_moved_at_tick
            .map(|last_moved_at_tick| now_tick.saturating_sub(last_moved_at_tick))
    }

    fn is_moving(self, now_tick: u64) -> bool {
        self.elapsed_ticks(now_tick)
            .is_some_and(|elapsed_ticks| elapsed_ticks < NOURISH_MOVEMENT_LEASE_TICKS)
    }

    #[cfg(test)]
    pub(super) fn last_moved_at_tick(self) -> Option<u64> {
        self.last_moved_at_tick
    }
}

pub fn attach_movement_tracker(
    mut commands: Commands,
    players: Query<Entity, (With<Client>, Without<NourishmentMovementTracker>)>,
) {
    for entity in &players {
        commands
            .entity(entity)
            .insert(NourishmentMovementTracker::default());
    }
}

fn horizontal_distance(event: &MovementEvent) -> f64 {
    let delta_x = event.position.x - event.old_position.x;
    let delta_z = event.position.z - event.old_position.z;
    delta_x.hypot(delta_z)
}

#[cfg(test)]
fn has_qualifying_horizontal_movement(event: &MovementEvent) -> bool {
    horizontal_distance(event) > NOURISH_MOVEMENT_EPSILON_BLOCKS
}

pub fn record_movement_events(
    clock: Res<CombatClock>,
    mut events: EventReader<MovementEvent>,
    mut trackers: Query<&mut NourishmentMovementTracker, With<Client>>,
    mut activity_windows: Query<&mut NourishmentActivityWindow, With<Client>>,
) {
    let mut horizontal_distances = HashMap::<Entity, f64>::new();
    for event in events.read() {
        *horizontal_distances.entry(event.client).or_default() += horizontal_distance(event);
    }

    for (entity, distance) in horizontal_distances {
        if distance > NOURISH_MOVEMENT_EPSILON_BLOCKS {
            if let Ok(mut tracker) = trackers.get_mut(entity) {
                tracker.record_movement(clock.tick);
            }
            if let Ok(mut activity_window) = activity_windows.get_mut(entity) {
                activity_window.record_qualifying_movement();
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NourishmentActivity {
    Idle,
    Moving,
    Dashing,
}

pub fn classify_activity(
    tracker: &NourishmentMovementTracker,
    movement_state: &MovementState,
    now_tick: u64,
) -> NourishmentActivity {
    if movement_state.action == MovementAction::Dashing {
        return NourishmentActivity::Dashing;
    }

    if tracker.is_moving(now_tick) {
        NourishmentActivity::Moving
    } else {
        NourishmentActivity::Idle
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

type NourishmentTickQueryItem<'a> = (
    &'a mut Nourishment,
    &'a mut NourishmentActivityWindow,
    &'a mut NourishmentSweepCursor,
    &'a Cultivation,
    Option<&'a NourishmentMovementTracker>,
    &'a MovementState,
);

pub fn tick_nourishment(
    clock: Res<CombatClock>,
    mut players: Query<NourishmentTickQueryItem<'_>, With<Client>>,
) {
    for (
        mut nourishment,
        mut activity_window,
        mut sweep_cursor,
        cultivation,
        tracker,
        movement_state,
    ) in &mut players
    {
        if !sweep_cursor.claim_tick(clock.tick) {
            continue;
        }

        let activity = tracker.map_or(NourishmentActivity::Idle, |tracker| {
            classify_activity(tracker, movement_state, clock.tick)
        });
        activity_window.record(activity);
        if activity_window.total_ticks() < NOURISH_SWEEP_INTERVAL_TICKS {
            continue;
        }

        let (satiety_loss, hydration_loss) = sweep_losses(
            *activity_window,
            nourishment_loss_multiplier(cultivation.realm),
        );
        nourishment.apply_loss(satiety_loss, hydration_loss);
        activity_window.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::Realm;
    use crate::nourishment::NOURISH_SPAWN_VALUE;
    use valence::prelude::{App, DVec3, Entity, Look, Update};
    use valence::testing::create_mock_client;

    fn movement_event(client: Entity, old: [f64; 3], new: [f64; 3]) -> MovementEvent {
        MovementEvent {
            client,
            position: DVec3::from_array(new),
            old_position: DVec3::from_array(old),
            look: Look::default(),
            old_look: Look::default(),
            on_ground: true,
            old_on_ground: true,
        }
    }

    fn event_is_qualifying(old: [f64; 3], new: [f64; 3]) -> bool {
        has_qualifying_horizontal_movement(&movement_event(Entity::PLACEHOLDER, old, new))
    }

    #[test]
    fn horizontal_movement_threshold_is_strictly_above_epsilon() {
        assert!(
            !event_is_qualifying([0.0, 64.0, 0.0], [0.049, 64.0, 0.0]),
            "horizontal movement below epsilon must remain idle"
        );
        assert!(
            !event_is_qualifying(
                [0.0, 64.0, 0.0],
                [NOURISH_MOVEMENT_EPSILON_BLOCKS, 64.0, 0.0],
            ),
            "horizontal movement exactly at epsilon must remain idle"
        );
        assert!(
            event_is_qualifying([0.0, 64.0, 0.0], [0.051, 64.0, 0.0]),
            "horizontal movement above epsilon must refresh activity"
        );
    }

    #[test]
    fn vertical_and_zero_horizontal_events_do_not_qualify() {
        assert!(
            !event_is_qualifying([2.0, 10.0, -4.0], [2.0, 90.0, -4.0]),
            "pure vertical movement must not refresh horizontal activity"
        );
        assert!(
            !event_is_qualifying([2.0, 10.0, -4.0], [2.0, 10.0, -4.0]),
            "look/on-ground packets with zero position delta must not refresh activity"
        );
    }

    #[test]
    fn movement_lease_pins_freshness_boundaries_and_saturates() {
        let mut tracker = NourishmentMovementTracker::default();
        assert_eq!(
            classify_activity(&tracker, &MovementState::default(), u64::MAX),
            NourishmentActivity::Idle,
            "a tracker with no qualifying event must be idle"
        );

        tracker.record_movement(100);
        for (now_tick, expected, label) in [
            (100, NourishmentActivity::Moving, "age 0"),
            (119, NourishmentActivity::Moving, "age 19"),
            (120, NourishmentActivity::Idle, "age 20"),
            (50, NourishmentActivity::Moving, "clock regression"),
        ] {
            assert_eq!(
                classify_activity(&tracker, &MovementState::default(), now_tick),
                expected,
                "{label} must obey the 20-tick freshness lease using saturating subtraction"
            );
        }

        tracker.record_movement(120);
        assert_eq!(
            classify_activity(&tracker, &MovementState::default(), 139),
            NourishmentActivity::Moving,
            "a later qualifying event must refresh the lease"
        );
        assert_eq!(
            classify_activity(&tracker, &MovementState::default(), 140),
            NourishmentActivity::Idle,
            "the refreshed lease must still expire exactly at age 20"
        );
    }

    #[test]
    fn dash_overrides_absent_or_stale_movement() {
        let movement = MovementState {
            action: MovementAction::Dashing,
            ..Default::default()
        };
        assert_eq!(
            classify_activity(&NourishmentMovementTracker::default(), &movement, 500),
            NourishmentActivity::Dashing,
            "dash must win even when no movement event was ever observed"
        );

        let tracker = NourishmentMovementTracker {
            last_moved_at_tick: Some(1),
        };
        assert_eq!(
            classify_activity(&tracker, &movement, 500),
            NourishmentActivity::Dashing,
            "dash must win over an expired movement lease"
        );
    }

    #[test]
    fn session_tracker_attaches_default_before_nourishment_tick() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 7 });
        app.add_event::<MovementEvent>();
        crate::nourishment::register(&mut app);
        let entity = app
            .world_mut()
            .spawn(create_mock_client("FreshSession").0)
            .id();

        app.update();

        assert_eq!(
            *app.world()
                .get::<NourishmentMovementTracker>(entity)
                .expect("production register must attach the session tracker"),
            NourishmentMovementTracker::default(),
            "a new session tracker must not inherit movement from persistence"
        );
    }

    #[test]
    fn same_tick_zero_delta_event_cannot_erase_qualifying_movement() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 42 });
        app.add_event::<MovementEvent>();
        app.add_systems(Update, record_movement_events);
        let entity = app
            .world_mut()
            .spawn((
                NourishmentMovementTracker::default(),
                NourishmentActivityWindow::default(),
                create_mock_client("MultiPacket").0,
            ))
            .id();

        app.world_mut()
            .send_event(movement_event(entity, [0.0, 64.0, 0.0], [0.051, 64.0, 0.0]));
        app.world_mut().send_event(movement_event(
            entity,
            [0.051, 64.0, 0.0],
            [0.051, 80.0, 0.0],
        ));
        app.update();

        assert_eq!(
            app.world()
                .get::<NourishmentMovementTracker>(entity)
                .expect("tracker should remain attached")
                .last_moved_at_tick,
            Some(42),
            "any qualifying event in a tick must win over later zero-horizontal events"
        );
        assert!(
            app.world()
                .get::<NourishmentActivityWindow>(entity)
                .expect("activity window should remain attached")
                .had_qualifying_movement,
            "any qualifying event in a tick must mark that entity's current activity window"
        );
    }

    #[test]
    fn same_tick_movement_uses_sum_of_non_negative_horizontal_distances() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 42 });
        app.add_event::<MovementEvent>();
        app.add_systems(Update, record_movement_events);
        let entity = app
            .world_mut()
            .spawn((
                NourishmentMovementTracker::default(),
                NourishmentActivityWindow::default(),
                create_mock_client("FragmentedMove").0,
            ))
            .id();

        app.world_mut()
            .send_event(movement_event(entity, [0.0, 64.0, 0.0], [0.03, 64.0, 0.0]));
        app.world_mut()
            .send_event(movement_event(entity, [0.03, 64.0, 0.0], [0.06, 64.0, 0.0]));
        app.update();

        assert_eq!(
            app.world()
                .get::<NourishmentMovementTracker>(entity)
                .expect("tracker should remain attached")
                .last_moved_at_tick,
            Some(42),
            "two same-tick 0.03-block fragments must aggregate past the 0.05 threshold"
        );
        assert!(
            app.world()
                .get::<NourishmentActivityWindow>(entity)
                .expect("activity window should remain attached")
                .had_qualifying_movement,
            "aggregated same-tick fragments beyond epsilon must mark the current activity window"
        );

        app.world_mut().resource_mut::<CombatClock>().tick = 43;
        app.world_mut()
            .send_event(movement_event(entity, [0.0, 64.0, 0.0], [0.06, 64.0, 0.0]));
        app.world_mut()
            .send_event(movement_event(entity, [0.06, 64.0, 0.0], [0.0, 64.0, 0.0]));
        app.update();

        assert_eq!(
            app.world()
                .get::<NourishmentMovementTracker>(entity)
                .expect("tracker should remain attached")
                .last_moved_at_tick,
            Some(43),
            "opposite same-tick movement must add traveled distance instead of cancelling"
        );
    }

    #[test]
    fn activity_window_multiplier_requires_real_movement_in_its_own_window() {
        let stale_lease_window = NourishmentActivityWindow {
            move_ticks: NOURISH_MOVEMENT_LEASE_TICKS as u32 - 1,
            ..Default::default()
        };
        assert_eq!(
            stale_lease_window.activity_multiplier(),
            NOURISH_IDLE_ACTIVITY_MULTIPLIER,
            "lease-derived Moving ticks without a new qualifying event must not elevate a fresh sweep"
        );

        let mut qualifying_window = stale_lease_window;
        qualifying_window.record_qualifying_movement();
        assert_eq!(
            qualifying_window.activity_multiplier(),
            NOURISH_MOVE_ACTIVITY_MULTIPLIER,
            "a qualifying horizontal event in this sweep must elevate the entire sweep"
        );
    }

    #[test]
    fn activity_window_reset_clears_qualifying_movement() {
        let mut window = NourishmentActivityWindow {
            had_qualifying_movement: true,
            ..Default::default()
        };
        window.reset();
        assert!(
            !window.had_qualifying_movement,
            "a formal activity-window reset must not inherit movement from the prior lifecycle"
        );
    }
    #[test]
    fn activity_window_records_ticks_and_selects_the_peak_window_activity() {
        let mut window = NourishmentActivityWindow::default();
        for _ in 0..50 {
            window.record(NourishmentActivity::Idle);
        }
        for _ in 0..75 {
            window.record(NourishmentActivity::Moving);
        }
        for _ in 0..75 {
            window.record(NourishmentActivity::Dashing);
        }

        assert_eq!(window.total_ticks(), NOURISH_SWEEP_INTERVAL_TICKS);
        assert_eq!(
            window.activity_multiplier(),
            NOURISH_DASH_ACTIVITY_MULTIPLIER,
            "a dash anywhere in a sweep must select the full-window dash multiplier"
        );

        window.reset();
        assert_eq!(window, NourishmentActivityWindow::default());
    }

    #[test]
    fn activity_window_json_round_trip_preserves_a_legal_mixed_snapshot() {
        let expected = NourishmentActivityWindow {
            idle_ticks: 74,
            move_ticks: 75,
            dash_ticks: 50,
            had_qualifying_movement: true,
        };

        let encoded = serde_json::to_value(expected).expect("activity window should serialize");
        let decoded = serde_json::from_value::<NourishmentActivityWindow>(encoded)
            .expect("legal activity window should deserialize");

        assert_eq!(decoded, expected);
        assert_eq!(decoded.total_ticks(), NOURISH_SWEEP_INTERVAL_TICKS - 1);
    }

    #[test]
    fn activity_window_json_defaults_missing_null_and_invalid_fields_independently() {
        let cases = [
            (
                serde_json::json!({}),
                NourishmentActivityWindow::default(),
                "missing fields",
            ),
            (
                serde_json::json!({"had_qualifying_movement": null}),
                NourishmentActivityWindow::default(),
                "null movement field",
            ),
            (
                serde_json::json!({
                    "idle_ticks": null,
                    "move_ticks": 50,
                    "dash_ticks": 25,
                    "had_qualifying_movement": true
                }),
                NourishmentActivityWindow {
                    idle_ticks: 0,
                    move_ticks: 50,
                    dash_ticks: 25,
                    had_qualifying_movement: true,
                },
                "null field",
            ),
            (
                serde_json::json!({
                    "idle_ticks": "bad",
                    "move_ticks": -1,
                    "dash_ticks": 4_294_967_296_u64
                }),
                NourishmentActivityWindow::default(),
                "wrong type, negative value, and u32 overflow",
            ),
            (
                serde_json::json!({
                    "idle_ticks": 10.5,
                    "move_ticks": true,
                    "dash_ticks": 12,
                    "had_qualifying_movement": "bad"
                }),
                NourishmentActivityWindow {
                    idle_ticks: 0,
                    move_ticks: 0,
                    dash_ticks: 12,
                    had_qualifying_movement: false,
                },
                "fractional and boolean fields",
            ),
        ];

        for (value, expected, label) in cases {
            let decoded = serde_json::from_value::<NourishmentActivityWindow>(value)
                .unwrap_or_else(|error| panic!("{label} should fail safe: {error}"));
            assert_eq!(
                decoded, expected,
                "{label} should default only invalid fields"
            );
        }
    }

    #[test]
    fn activity_window_json_discards_complete_or_oversized_snapshots() {
        let cases = [
            serde_json::json!({"idle_ticks": 200, "move_ticks": 0, "dash_ticks": 0}),
            serde_json::json!({"idle_ticks": 199, "move_ticks": 1, "dash_ticks": 0}),
            serde_json::json!({
                "idle_ticks": u32::MAX,
                "move_ticks": u32::MAX,
                "dash_ticks": u32::MAX
            }),
        ];

        for value in cases {
            let decoded = serde_json::from_value::<NourishmentActivityWindow>(value)
                .expect("numeric activity window should deserialize safely");
            assert_eq!(
                decoded,
                NourishmentActivityWindow::default(),
                "a persisted complete window must not cause an oversized reconnect sweep"
            );
        }

        for value in [
            serde_json::Value::Null,
            serde_json::json!("bad"),
            serde_json::json!([1, 2, 3]),
            serde_json::json!(true),
        ] {
            assert_eq!(
                serde_json::from_value::<NourishmentActivityWindow>(value)
                    .expect("non-object activity window should fail safe"),
                NourishmentActivityWindow::default()
            );
        }
    }

    #[test]
    fn pure_idle_move_and_dash_windows_apply_exact_multipliers() {
        let cases = [
            (NourishmentActivity::Idle, 1.0),
            (NourishmentActivity::Moving, 1.5),
            (NourishmentActivity::Dashing, 3.0),
        ];
        for (activity, multiplier) in cases {
            let mut window = NourishmentActivityWindow::default();
            for _ in 0..NOURISH_SWEEP_INTERVAL_TICKS {
                window.record(activity);
            }
            if activity == NourishmentActivity::Moving {
                window.record_qualifying_movement();
            }
            let (satiety, hydration) = sweep_losses(window, 1.0);
            let minutes = NOURISH_SWEEP_INTERVAL_TICKS as f32 / NOURISH_TICKS_PER_MINUTE;
            assert!((satiety - NOURISH_SATIETY_LOSS_PER_MIN * minutes * multiplier).abs() < 1e-6);
            assert!(
                (hydration - NOURISH_HYDRATION_LOSS_PER_MIN * minutes * multiplier).abs() < 1e-6
            );
        }
    }

    #[test]
    fn mixed_window_uses_peak_activity_with_dash_priority() {
        let cases = [
            (
                NourishmentActivityWindow {
                    idle_ticks: 199,
                    move_ticks: 1,
                    dash_ticks: 0,
                    had_qualifying_movement: true,
                },
                NOURISH_MOVE_ACTIVITY_MULTIPLIER,
                "one qualifying movement tick must elevate the entire sweep",
            ),
            (
                NourishmentActivityWindow {
                    idle_ticks: 199,
                    move_ticks: 0,
                    dash_ticks: 1,
                    had_qualifying_movement: false,
                },
                NOURISH_DASH_ACTIVITY_MULTIPLIER,
                "one dash tick must elevate the entire sweep to dash",
            ),
            (
                NourishmentActivityWindow {
                    idle_ticks: 100,
                    move_ticks: 99,
                    dash_ticks: 1,
                    had_qualifying_movement: false,
                },
                NOURISH_DASH_ACTIVITY_MULTIPLIER,
                "dash must win over moving in a mixed sweep",
            ),
        ];
        let minutes = NOURISH_SWEEP_INTERVAL_TICKS as f32 / NOURISH_TICKS_PER_MINUTE;

        for (window, multiplier, label) in cases {
            let (satiety, hydration) = sweep_losses(window, 0.75);
            assert!(
                (satiety - NOURISH_SATIETY_LOSS_PER_MIN * minutes * multiplier * 0.75).abs() < 1e-6,
                "{label}"
            );
            assert!(
                (hydration - NOURISH_HYDRATION_LOSS_PER_MIN * minutes * multiplier * 0.75).abs()
                    < 1e-6,
                "{label}"
            );
        }
    }

    fn nourishment_test_app(
        name: &str,
        activity_window: NourishmentActivityWindow,
    ) -> (App, Entity) {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 1 });
        app.add_systems(Update, tick_nourishment);
        let (client_bundle, _helper) = create_mock_client(name);
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                Cultivation {
                    realm: Realm::Awaken,
                    ..Default::default()
                },
                MovementState::default(),
                NourishmentMovementTracker::default(),
                NourishmentSweepCursor::default(),
                Nourishment::spawn_default(),
                activity_window,
            ))
            .id();
        (app, entity)
    }

    fn advance_distinct_ticks(app: &mut App, start_tick: u64, count: u32) {
        for offset in 0..count {
            app.world_mut().resource_mut::<CombatClock>().tick = start_tick + u64::from(offset);
            app.update();
        }
    }

    fn assert_spawn_nourishment(app: &App, entity: Entity, reason: &str) {
        assert_eq!(
            *app.world().get::<Nourishment>(entity).unwrap(),
            Nourishment::spawn_default(),
            "{reason}"
        );
    }

    #[test]
    fn qualifying_movement_on_a_sweep_boundary_does_not_elevate_the_next_window() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 1 });
        crate::nourishment::register(&mut app);
        let (client_bundle, _helper) = create_mock_client("WindowBoundary");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                Cultivation {
                    realm: Realm::Awaken,
                    ..Default::default()
                },
                MovementState::default(),
                Nourishment::spawn_default(),
                NourishmentActivityWindow::default(),
            ))
            .id();

        for tick in 1..=NOURISH_SWEEP_INTERVAL_TICKS {
            app.world_mut().resource_mut::<CombatClock>().tick = u64::from(tick);
            if tick == NOURISH_SWEEP_INTERVAL_TICKS {
                app.world_mut().send_event(movement_event(
                    entity,
                    [0.0, 64.0, 0.0],
                    [NOURISH_MOVEMENT_EPSILON_BLOCKS + 0.001, 64.0, 0.0],
                ));
            }
            app.update();
        }

        let (moving_satiety_loss, moving_hydration_loss) = sweep_losses(
            NourishmentActivityWindow {
                had_qualifying_movement: true,
                ..Default::default()
            },
            1.0,
        );
        let after_first = *app
            .world()
            .get::<Nourishment>(entity)
            .expect("first personal window should settle nourishment");
        assert!(
            (after_first.satiety - (NOURISH_SPAWN_VALUE - moving_satiety_loss)).abs() < 1e-6
                && (after_first.hydration - (NOURISH_SPAWN_VALUE - moving_hydration_loss)).abs()
                    < 1e-6,
            "a qualifying event on the 200th tick must elevate the window it belongs to"
        );

        for tick in (NOURISH_SWEEP_INTERVAL_TICKS + 1)..=(NOURISH_SWEEP_INTERVAL_TICKS * 2) {
            app.world_mut().resource_mut::<CombatClock>().tick = u64::from(tick);
            app.update();
        }

        let (idle_satiety_loss, idle_hydration_loss) =
            sweep_losses(NourishmentActivityWindow::default(), 1.0);
        let after_second = *app
            .world()
            .get::<Nourishment>(entity)
            .expect("second personal window should settle nourishment");
        assert!(
            (after_second.satiety - (after_first.satiety - idle_satiety_loss)).abs() < 1e-6
                && (after_second.hydration - (after_first.hydration - idle_hydration_loss)).abs()
                    < 1e-6,
            "the 20-tick movement lease may classify early ticks as Moving but must not elevate a window without a new event"
        );

        for tick in (NOURISH_SWEEP_INTERVAL_TICKS * 2 + 1)..=(NOURISH_SWEEP_INTERVAL_TICKS * 3) {
            app.world_mut().resource_mut::<CombatClock>().tick = u64::from(tick);
            if tick == NOURISH_SWEEP_INTERVAL_TICKS * 2 + 1 {
                app.world_mut().send_event(movement_event(
                    entity,
                    [0.0, 64.0, 0.0],
                    [NOURISH_MOVEMENT_EPSILON_BLOCKS + 0.001, 64.0, 0.0],
                ));
            }
            app.update();
        }

        let after_third = *app
            .world()
            .get::<Nourishment>(entity)
            .expect("third personal window should settle nourishment");
        assert!(
            (after_third.satiety - (after_second.satiety - moving_satiety_loss)).abs() < 1e-6
                && (after_third.hydration - (after_second.hydration - moving_hydration_loss)).abs()
                    < 1e-6,
            "a new qualifying event in the following window must restore the moving multiplier"
        );
    }
    #[test]
    fn new_player_at_global_tick_199_accumulates_a_full_personal_window_before_loss() {
        let (mut app, entity) =
            nourishment_test_app("LateJoin", NourishmentActivityWindow::default());
        advance_distinct_ticks(&mut app, 199, NOURISH_SWEEP_INTERVAL_TICKS - 1);
        assert_spawn_nourishment(
            &app,
            entity,
            "a player joining near a global phase boundary must not settle early",
        );
        assert_eq!(
            app.world()
                .get::<NourishmentActivityWindow>(entity)
                .unwrap()
                .total_ticks(),
            NOURISH_SWEEP_INTERVAL_TICKS - 1,
            "the new player must own all 199 accumulated ticks",
        );

        app.world_mut().resource_mut::<CombatClock>().tick = 398;
        app.update();
        assert!(
            app.world().get::<Nourishment>(entity).unwrap().satiety < NOURISH_SPAWN_VALUE,
            "the 200th distinct tick for this player must settle its personal window",
        );
    }

    #[test]
    fn reset_at_global_boundary_starts_a_fresh_personal_window() {
        let (mut app, entity) = nourishment_test_app(
            "Revived",
            NourishmentActivityWindow {
                idle_ticks: NOURISH_SWEEP_INTERVAL_TICKS - 1,
                ..Default::default()
            },
        );
        app.world_mut()
            .entity_mut(entity)
            .get_mut::<NourishmentActivityWindow>()
            .unwrap()
            .reset();
        app.world_mut().resource_mut::<CombatClock>().tick = 200;
        app.update();

        assert_spawn_nourishment(
            &app,
            entity,
            "a formal lifecycle reset on a global boundary must not settle old activity",
        );
        assert_eq!(
            app.world()
                .get::<NourishmentActivityWindow>(entity)
                .unwrap()
                .total_ticks(),
            1,
            "the first tick after reset must begin a new personal window",
        );
    }

    #[test]
    fn reconnect_with_199_ticks_settles_after_one_new_distinct_tick() {
        let (mut app, entity) = nourishment_test_app(
            "Reconnect199",
            NourishmentActivityWindow {
                idle_ticks: 100,
                move_ticks: 60,
                dash_ticks: 39,
                had_qualifying_movement: false,
            },
        );
        app.world_mut().resource_mut::<CombatClock>().tick = 777;
        app.update();

        assert!(
            app.world().get::<Nourishment>(entity).unwrap().satiety < NOURISH_SPAWN_VALUE,
            "a restored legal 199-tick window must settle after one newly experienced tick",
        );
        assert_eq!(
            *app.world()
                .get::<NourishmentActivityWindow>(entity)
                .unwrap(),
            NourishmentActivityWindow::default(),
            "a settled restored window must reset",
        );
    }

    #[test]
    fn reconnect_with_100_ticks_requires_100_new_distinct_ticks() {
        let (mut app, entity) = nourishment_test_app(
            "Reconnect100",
            NourishmentActivityWindow {
                idle_ticks: 100,
                ..Default::default()
            },
        );
        advance_distinct_ticks(&mut app, 1_000, 99);
        assert_spawn_nourishment(
            &app,
            entity,
            "a restored 100-tick window must not settle after only 99 new ticks",
        );
        app.world_mut().resource_mut::<CombatClock>().tick = 1_099;
        app.update();
        assert!(
            app.world().get::<Nourishment>(entity).unwrap().satiety < NOURISH_SPAWN_VALUE,
            "a restored 100-tick window must settle on its 100th new distinct tick",
        );
    }

    #[test]
    fn duplicate_and_regressed_clock_ticks_do_not_duplicate_counting() {
        let (mut app, entity) =
            nourishment_test_app("Deduplicate", NourishmentActivityWindow::default());
        app.world_mut().resource_mut::<CombatClock>().tick = 42;
        app.update();
        app.update();
        assert_eq!(
            app.world()
                .get::<NourishmentActivityWindow>(entity)
                .unwrap()
                .total_ticks(),
            1,
            "duplicate Update calls at one CombatClock tick must count once",
        );

        app.world_mut().resource_mut::<CombatClock>().tick = 41;
        app.update();
        app.world_mut().resource_mut::<CombatClock>().tick = 42;
        app.update();
        assert_eq!(
            app.world()
                .get::<NourishmentActivityWindow>(entity)
                .unwrap()
                .total_ticks(),
            1,
            "clock regression must not let an already observed runtime tick count twice",
        );

        app.world_mut().resource_mut::<CombatClock>().tick = 43;
        app.update();
        assert_eq!(
            app.world()
                .get::<NourishmentActivityWindow>(entity)
                .unwrap()
                .total_ticks(),
            2,
            "the first tick after a regression catches up must continue the personal window",
        );
    }
}
