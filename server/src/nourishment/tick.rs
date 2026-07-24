use serde::{Deserialize, Deserializer, Serialize};
use valence::prelude::{bevy_ecs, Client, Component, OldPosition, Position, Query, With};

use crate::cultivation::components::Cultivation;
use crate::movement::{MovementAction, MovementState};

use super::{
    nourishment_loss_multiplier, Nourishment, NOURISH_DASH_ACTIVITY_MULTIPLIER,
    NOURISH_HYDRATION_LOSS_PER_MIN, NOURISH_IDLE_ACTIVITY_MULTIPLIER,
    NOURISH_MOVEMENT_EPSILON_BLOCKS, NOURISH_MOVE_ACTIVITY_MULTIPLIER,
    NOURISH_SATIETY_LOSS_PER_MIN, NOURISH_SWEEP_INTERVAL_TICKS, NOURISH_TICKS_PER_MINUTE,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Component)]
pub struct NourishmentActivityWindow {
    pub idle_ticks: u32,
    pub move_ticks: u32,
    pub dash_ticks: u32,
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

    pub fn total_ticks(self) -> u32 {
        self.idle_ticks
            .saturating_add(self.move_ticks)
            .saturating_add(self.dash_ticks)
    }

    pub fn is_complete(self) -> bool {
        self.total_ticks() >= NOURISH_SWEEP_INTERVAL_TICKS
    }

    pub fn weighted_activity_ticks(self) -> f32 {
        self.idle_ticks as f32 * NOURISH_IDLE_ACTIVITY_MULTIPLIER
            + self.move_ticks as f32 * NOURISH_MOVE_ACTIVITY_MULTIPLIER
            + self.dash_ticks as f32 * NOURISH_DASH_ACTIVITY_MULTIPLIER
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NourishmentActivity {
    Idle,
    Moving,
    Dashing,
}

pub fn classify_activity(
    position: &Position,
    old_position: &OldPosition,
    movement_state: &MovementState,
) -> NourishmentActivity {
    if movement_state.action == MovementAction::Dashing {
        return NourishmentActivity::Dashing;
    }

    let position = position.get();
    let old_position = old_position.get();
    let delta_x = position.x - old_position.x;
    let delta_z = position.z - old_position.z;
    if delta_x.hypot(delta_z) > NOURISH_MOVEMENT_EPSILON_BLOCKS {
        NourishmentActivity::Moving
    } else {
        NourishmentActivity::Idle
    }
}

pub fn sweep_losses(window: NourishmentActivityWindow, realm_multiplier: f32) -> (f32, f32) {
    let weighted_minutes = window.weighted_activity_ticks() / NOURISH_TICKS_PER_MINUTE;
    let realm_multiplier = if realm_multiplier.is_finite() {
        realm_multiplier.max(0.0)
    } else {
        1.0
    };
    (
        NOURISH_SATIETY_LOSS_PER_MIN * weighted_minutes * realm_multiplier,
        NOURISH_HYDRATION_LOSS_PER_MIN * weighted_minutes * realm_multiplier,
    )
}

type NourishmentTickQueryItem<'a> = (
    &'a mut Nourishment,
    &'a mut NourishmentActivityWindow,
    &'a Cultivation,
    &'a Position,
    &'a OldPosition,
    &'a MovementState,
);

pub fn tick_nourishment(mut players: Query<NourishmentTickQueryItem<'_>, With<Client>>) {
    for (
        mut nourishment,
        mut activity_window,
        cultivation,
        position,
        old_position,
        movement_state,
    ) in &mut players
    {
        activity_window.record(classify_activity(position, old_position, movement_state));
        if !activity_window.is_complete() {
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
    use valence::prelude::{App, Update};
    use valence::testing::create_mock_client;

    #[test]
    fn activity_classification_prioritizes_dash_then_horizontal_motion() {
        let position = Position::new([1.0, 64.0, 0.0]);
        let old = OldPosition::new([0.0, 10.0, 0.0]);
        assert_eq!(
            classify_activity(&position, &old, &MovementState::default()),
            NourishmentActivity::Moving,
            "vertical movement must be ignored while horizontal movement counts"
        );

        let position = Position::new([0.05, 90.0, 0.0]);
        let old = OldPosition::new([0.0, 10.0, 0.0]);
        assert_eq!(
            classify_activity(&position, &old, &MovementState::default()),
            NourishmentActivity::Idle,
            "the exact epsilon boundary is idle"
        );

        let movement = MovementState {
            action: MovementAction::Dashing,
            ..Default::default()
        };
        assert_eq!(
            classify_activity(
                &Position::new([0.0, 64.0, 0.0]),
                &OldPosition::new([0.0, 64.0, 0.0]),
                &movement,
            ),
            NourishmentActivity::Dashing
        );
    }

    #[test]
    fn activity_window_accumulates_each_tick_and_resets_without_carryover() {
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
        assert!(window.is_complete());
        assert!((window.weighted_activity_ticks() - 387.5).abs() < f32::EPSILON);

        window.reset();
        assert_eq!(window, NourishmentActivityWindow::default());
        assert!(!window.is_complete());
    }

    #[test]
    fn activity_window_json_round_trip_preserves_a_legal_mixed_snapshot() {
        let expected = NourishmentActivityWindow {
            idle_ticks: 74,
            move_ticks: 75,
            dash_ticks: 50,
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
                serde_json::json!({
                    "idle_ticks": null,
                    "move_ticks": 50,
                    "dash_ticks": 25
                }),
                NourishmentActivityWindow {
                    idle_ticks: 0,
                    move_ticks: 50,
                    dash_ticks: 25,
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
                    "dash_ticks": 12
                }),
                NourishmentActivityWindow {
                    idle_ticks: 0,
                    move_ticks: 0,
                    dash_ticks: 12,
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
            let (satiety, hydration) = sweep_losses(window, 1.0);
            let minutes = NOURISH_SWEEP_INTERVAL_TICKS as f32 / NOURISH_TICKS_PER_MINUTE;
            assert!((satiety - NOURISH_SATIETY_LOSS_PER_MIN * minutes * multiplier).abs() < 1e-6);
            assert!(
                (hydration - NOURISH_HYDRATION_LOSS_PER_MIN * minutes * multiplier).abs() < 1e-6
            );
        }
    }

    #[test]
    fn mixed_window_uses_tick_weighted_average_not_peak_activity() {
        let window = NourishmentActivityWindow {
            idle_ticks: 100,
            move_ticks: 50,
            dash_ticks: 50,
        };
        let (satiety, hydration) = sweep_losses(window, 0.75);
        let weighted_minutes = 325.0 / NOURISH_TICKS_PER_MINUTE;
        assert!((satiety - NOURISH_SATIETY_LOSS_PER_MIN * weighted_minutes * 0.75).abs() < 1e-6);
        assert!(
            (hydration - NOURISH_HYDRATION_LOSS_PER_MIN * weighted_minutes * 0.75).abs() < 1e-6
        );
    }

    #[test]
    fn production_tick_waits_for_full_window_then_applies_loss_and_resets() {
        let mut app = App::new();
        app.add_systems(Update, tick_nourishment);
        let (mut client_bundle, _helper) = create_mock_client("Azure");
        client_bundle.player.position = Position::new([0.0, 64.0, 0.0]);
        client_bundle.player.old_position = OldPosition::new([0.0, 64.0, 0.0]);
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

        for _ in 0..NOURISH_SWEEP_INTERVAL_TICKS - 1 {
            app.update();
        }
        assert_eq!(
            *app.world().get::<Nourishment>(entity).unwrap(),
            Nourishment::spawn_default(),
            "no partial window may deduct nourishment"
        );

        app.update();
        let nourishment = app.world().get::<Nourishment>(entity).unwrap();
        let expected_minutes = NOURISH_SWEEP_INTERVAL_TICKS as f32 / NOURISH_TICKS_PER_MINUTE;
        assert!(
            (nourishment.satiety
                - (NOURISH_SPAWN_VALUE - NOURISH_SATIETY_LOSS_PER_MIN * expected_minutes))
                .abs()
                < 1e-6
        );
        assert!(
            (nourishment.hydration
                - (NOURISH_SPAWN_VALUE - NOURISH_HYDRATION_LOSS_PER_MIN * expected_minutes))
                .abs()
                < 1e-6
        );
        assert_eq!(
            *app.world()
                .get::<NourishmentActivityWindow>(entity)
                .unwrap(),
            NourishmentActivityWindow::default()
        );
    }
}
