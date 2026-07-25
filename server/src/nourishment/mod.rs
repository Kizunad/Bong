use serde::{Deserialize, Deserializer, Serialize};
use valence::prelude::{apply_deferred, bevy_ecs, App, Component, IntoSystemConfigs, Update};

use crate::cultivation::components::Realm;

pub mod tick;

pub const NOURISH_MIN_VALUE: f32 = 0.0;
pub const NOURISH_MAX_VALUE: f32 = 120.0;
pub const NOURISH_SPAWN_VALUE: f32 = 80.0;
pub const NOURISH_OVERFULL_THRESHOLD: f32 = 100.0;
pub const NOURISH_COMFORT_THRESHOLD: f32 = 60.0;
pub const NOURISH_WEAK_THRESHOLD: f32 = 40.0;
pub const NOURISH_SAPPED_THRESHOLD: f32 = 20.0;
pub const NOURISH_SATIETY_LOSS_PER_MIN: f32 = 0.8;
pub const NOURISH_HYDRATION_LOSS_PER_MIN: f32 = 1.2;
pub const NOURISH_SWEEP_INTERVAL_TICKS: u32 = 200;
pub const NOURISH_TICKS_PER_MINUTE: f32 = (crate::combat::components::TICKS_PER_SECOND * 60) as f32;
pub const NOURISH_IDLE_ACTIVITY_MULTIPLIER: f32 = 1.0;
pub const NOURISH_MOVE_ACTIVITY_MULTIPLIER: f32 = 1.5;
pub const NOURISH_DASH_ACTIVITY_MULTIPLIER: f32 = 3.0;
pub const NOURISH_MOVEMENT_EPSILON_BLOCKS: f64 = 0.05;
pub const NOURISH_MOVEMENT_LEASE_TICKS: u64 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NourishmentAxis {
    Satiety,
    Hydration,
}

impl NourishmentAxis {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "satiety" => Some(Self::Satiety),
            "hydration" => Some(Self::Hydration),
            _ => None,
        }
    }

    pub fn wire_name(self) -> &'static str {
        match self {
            Self::Satiety => "satiety",
            Self::Hydration => "hydration",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NourishmentValueError {
    NonFinite,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Component)]
pub struct Nourishment {
    pub satiety: f32,
    pub hydration: f32,
}

impl<'de> Deserialize<'de> for Nourishment {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct PersistedNourishment {
            #[serde(default)]
            satiety: Option<serde_json::Value>,
            #[serde(default)]
            hydration: Option<serde_json::Value>,
        }

        fn persisted_axis(value: Option<serde_json::Value>) -> f32 {
            value
                .and_then(|value| value.as_f64())
                .map(|value| value as f32)
                .and_then(|value| normalize_value(value).ok())
                .unwrap_or(NOURISH_SPAWN_VALUE)
        }

        let persisted = PersistedNourishment::deserialize(deserializer)?;
        Ok(Self {
            satiety: persisted_axis(persisted.satiety),
            hydration: persisted_axis(persisted.hydration),
        })
    }
}

impl Default for Nourishment {
    fn default() -> Self {
        Self::spawn_default()
    }
}

impl Nourishment {
    pub const fn spawn_default() -> Self {
        Self {
            satiety: NOURISH_SPAWN_VALUE,
            hydration: NOURISH_SPAWN_VALUE,
        }
    }

    pub fn try_new(satiety: f32, hydration: f32) -> Result<Self, NourishmentValueError> {
        Ok(Self {
            satiety: normalize_value(satiety)?,
            hydration: normalize_value(hydration)?,
        })
    }

    pub fn sanitized_or_default(mut self) -> Self {
        self.satiety = normalize_value(self.satiety).unwrap_or(NOURISH_SPAWN_VALUE);
        self.hydration = normalize_value(self.hydration).unwrap_or(NOURISH_SPAWN_VALUE);
        self
    }

    pub fn value(self, axis: NourishmentAxis) -> f32 {
        match axis {
            NourishmentAxis::Satiety => self.satiety,
            NourishmentAxis::Hydration => self.hydration,
        }
    }

    pub fn try_set(
        &mut self,
        axis: NourishmentAxis,
        value: f32,
    ) -> Result<f32, NourishmentValueError> {
        let value = normalize_value(value)?;
        match axis {
            NourishmentAxis::Satiety => self.satiety = value,
            NourishmentAxis::Hydration => self.hydration = value,
        }
        Ok(value)
    }

    pub fn apply_loss(&mut self, satiety_loss: f32, hydration_loss: f32) {
        let satiety_loss = finite_non_negative(satiety_loss);
        let hydration_loss = finite_non_negative(hydration_loss);
        self.satiety = (self.satiety - satiety_loss).clamp(NOURISH_MIN_VALUE, NOURISH_MAX_VALUE);
        self.hydration =
            (self.hydration - hydration_loss).clamp(NOURISH_MIN_VALUE, NOURISH_MAX_VALUE);
    }

    pub fn reset_to_spawn(&mut self) {
        *self = Self::spawn_default();
    }
}

pub fn normalize_value(value: f32) -> Result<f32, NourishmentValueError> {
    if !value.is_finite() {
        return Err(NourishmentValueError::NonFinite);
    }
    Ok(value.clamp(NOURISH_MIN_VALUE, NOURISH_MAX_VALUE))
}

fn finite_non_negative(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NourishBand {
    Overfull,
    Comfort,
    Weak,
    Sapped,
    Critical,
}

pub fn band_of(value: f32) -> NourishBand {
    let Ok(value) = normalize_value(value) else {
        return NourishBand::Critical;
    };
    if value > NOURISH_OVERFULL_THRESHOLD {
        NourishBand::Overfull
    } else if value > NOURISH_COMFORT_THRESHOLD {
        NourishBand::Comfort
    } else if value > NOURISH_WEAK_THRESHOLD {
        NourishBand::Weak
    } else if value > NOURISH_SAPPED_THRESHOLD {
        NourishBand::Sapped
    } else {
        NourishBand::Critical
    }
}

pub fn nourishment_loss_multiplier(realm: Realm) -> f32 {
    match realm {
        Realm::Awaken => 1.00,
        Realm::Induce => 0.95,
        Realm::Condense => 0.90,
        Realm::Solidify => 0.85,
        Realm::Spirit => 0.80,
        Realm::Void => 0.75,
    }
}

pub fn register(app: &mut App) {
    app.add_event::<valence::movement::MovementEvent>();
    app.add_systems(
        Update,
        (
            tick::attach_movement_tracker,
            tick::attach_sweep_cursor,
            apply_deferred,
            tick::record_movement_events,
            tick::tick_nourishment,
        )
            .chain()
            .after(crate::movement::tick_movement_actions)
            .after(crate::movement::handle_movement_action_intents),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticks_per_minute_is_derived_from_combat_clock_rate() {
        assert_eq!(
            NOURISH_TICKS_PER_MINUTE,
            (crate::combat::components::TICKS_PER_SECOND * 60) as f32,
            "nourishment minute duration must stay pinned to the combat clock rate",
        );
    }

    #[test]
    fn band_boundaries_pin_every_interval_edge() {
        let cases = [
            (120.0, NourishBand::Overfull),
            (100.01, NourishBand::Overfull),
            (100.0, NourishBand::Comfort),
            (60.01, NourishBand::Comfort),
            (60.0, NourishBand::Weak),
            (40.01, NourishBand::Weak),
            (40.0, NourishBand::Sapped),
            (20.01, NourishBand::Sapped),
            (20.0, NourishBand::Critical),
            (0.0, NourishBand::Critical),
        ];

        for (value, expected) in cases {
            assert_eq!(
                band_of(value),
                expected,
                "value {value} should map to {expected:?}"
            );
        }
    }

    #[test]
    fn band_of_clamps_finite_out_of_range_and_fails_closed_for_non_finite() {
        assert_eq!(band_of(-1.0), NourishBand::Critical);
        assert_eq!(band_of(121.0), NourishBand::Overfull);
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(
                band_of(value),
                NourishBand::Critical,
                "non-finite values must fail closed instead of granting a beneficial band"
            );
        }
    }

    #[test]
    fn construction_and_setters_clamp_finite_values() {
        let mut nourishment = Nourishment::try_new(-5.0, 150.0).unwrap();
        assert_eq!(nourishment.satiety, NOURISH_MIN_VALUE);
        assert_eq!(nourishment.hydration, NOURISH_MAX_VALUE);

        assert_eq!(
            nourishment
                .try_set(NourishmentAxis::Satiety, 121.0)
                .unwrap(),
            NOURISH_MAX_VALUE
        );
        assert_eq!(
            nourishment
                .try_set(NourishmentAxis::Hydration, -1.0)
                .unwrap(),
            NOURISH_MIN_VALUE
        );
    }

    #[test]
    fn construction_and_setters_reject_non_finite_without_mutation() {
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(
                Nourishment::try_new(value, NOURISH_SPAWN_VALUE),
                Err(NourishmentValueError::NonFinite)
            );
        }

        let mut nourishment = Nourishment::spawn_default();
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(
                nourishment.try_set(NourishmentAxis::Satiety, value),
                Err(NourishmentValueError::NonFinite)
            );
            assert_eq!(nourishment, Nourishment::spawn_default());
        }
    }

    #[test]
    fn sanitized_persisted_values_clamp_or_restore_invalid_axes_independently() {
        assert_eq!(
            Nourishment {
                satiety: 160.0,
                hydration: -3.0,
            }
            .sanitized_or_default(),
            Nourishment {
                satiety: NOURISH_MAX_VALUE,
                hydration: NOURISH_MIN_VALUE,
            }
        );
        assert_eq!(
            Nourishment {
                satiety: f32::NAN,
                hydration: 45.0,
            }
            .sanitized_or_default(),
            Nourishment {
                satiety: NOURISH_SPAWN_VALUE,
                hydration: 45.0,
            }
        );
    }

    #[test]
    fn apply_loss_clamps_at_zero_and_ignores_invalid_or_negative_losses() {
        let mut nourishment = Nourishment {
            satiety: 1.0,
            hydration: 2.0,
        };
        nourishment.apply_loss(5.0, 1.5);
        assert_eq!(nourishment.satiety, NOURISH_MIN_VALUE);
        assert_eq!(nourishment.hydration, 0.5);

        nourishment.apply_loss(-10.0, f32::NAN);
        assert_eq!(nourishment.satiety, NOURISH_MIN_VALUE);
        assert_eq!(nourishment.hydration, 0.5);
    }

    #[test]
    fn every_realm_has_the_locked_loss_multiplier() {
        let cases = [
            (Realm::Awaken, 1.00),
            (Realm::Induce, 0.95),
            (Realm::Condense, 0.90),
            (Realm::Solidify, 0.85),
            (Realm::Spirit, 0.80),
            (Realm::Void, 0.75),
        ];
        for (realm, expected) in cases {
            assert!((nourishment_loss_multiplier(realm) - expected).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn production_schedule_consumes_movement_event_and_attaches_default_tracker_same_update() {
        use valence::movement::MovementEvent;
        use valence::prelude::{DVec3, Look};

        let mut app = valence::prelude::App::new();
        app.insert_resource(crate::combat::CombatClock { tick: 31 });
        app.add_event::<MovementEvent>();
        crate::nourishment::register(&mut app);

        let (client_bundle, _helper) = valence::testing::create_mock_client("WalkNourishment");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                crate::movement::MovementState::default(),
                crate::cultivation::components::Cultivation::default(),
                Nourishment::spawn_default(),
                tick::NourishmentActivityWindow::default(),
            ))
            .id();

        app.world_mut().send_event(MovementEvent {
            client: entity,
            position: DVec3::new(0.051, 64.0, 0.0),
            old_position: DVec3::new(0.0, 64.0, 0.0),
            look: Look::default(),
            old_look: Look::default(),
            on_ground: true,
            old_on_ground: true,
        });
        app.update();

        assert_eq!(
            *app.world()
                .get::<tick::NourishmentActivityWindow>(entity)
                .expect("production nourishment register should retain an activity window"),
            tick::NourishmentActivityWindow {
                idle_ticks: 0,
                move_ticks: 1,
                dash_ticks: 0,
            },
            "a qualifying MovementEvent must count as movement in the same production update"
        );
        assert_eq!(
            app.world()
                .get::<tick::NourishmentMovementTracker>(entity)
                .expect(
                    "production register must attach the session tracker before consuming events"
                )
                .last_moved_at_tick(),
            Some(31),
            "the attached default tracker must be visible through deferred Commands and refreshed"
        );
    }

    #[test]
    fn production_schedule_counts_dash_intent_as_dash_on_its_starting_tick() {
        let mut app = valence::prelude::App::new();
        app.insert_resource(crate::combat::CombatClock { tick: 17 });
        app.add_event::<crate::network::vfx_event_emit::VfxEventRequest>();
        app.add_event::<crate::network::audio_event_emit::PlaySoundRecipeRequest>();
        app.add_event::<valence::movement::MovementEvent>();
        crate::movement::register(&mut app);
        crate::nourishment::register(&mut app);

        let (mut client_bundle, _helper) = valence::testing::create_mock_client("DashNourishment");
        client_bundle.player.position = valence::prelude::Position::new([0.0, 64.0, 0.0]);
        client_bundle.player.old_position = valence::prelude::OldPosition::new([0.0, 64.0, 0.0]);
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                crate::movement::MovementState::default(),
                crate::combat::components::Stamina::default(),
                crate::cultivation::components::Cultivation::default(),
                Nourishment::spawn_default(),
                tick::NourishmentActivityWindow::default(),
            ))
            .id();

        app.world_mut()
            .send_event(crate::movement::MovementActionIntent {
                entity,
                action: crate::movement::MovementAction::Dashing,
                yaw_degrees: Some(90.0),
            });
        app.update();

        assert_eq!(
            app.world()
                .get::<crate::movement::MovementState>(entity)
                .expect("production movement register should retain movement state")
                .action,
            crate::movement::MovementAction::Dashing,
            "the real movement intent handler must accept the dash on this update"
        );
        assert_eq!(
            *app.world()
                .get::<tick::NourishmentActivityWindow>(entity)
                .expect("production nourishment register should retain an activity window"),
            tick::NourishmentActivityWindow {
                idle_ticks: 0,
                move_ticks: 0,
                dash_ticks: 1,
            },
            "nourishment must run after both movement systems so the dash starting tick counts as dash"
        );
    }

    #[test]
    fn axis_parser_accepts_only_canonical_names() {
        assert_eq!(
            NourishmentAxis::parse("satiety"),
            Some(NourishmentAxis::Satiety)
        );
        assert_eq!(
            NourishmentAxis::parse("hydration"),
            Some(NourishmentAxis::Hydration)
        );
        for invalid in ["", "food", "water", "Satiety", "hydration "] {
            assert_eq!(NourishmentAxis::parse(invalid), None);
        }
    }
}
