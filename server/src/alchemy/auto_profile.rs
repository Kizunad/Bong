//! plan-alchemy-v2 P3 — AutoProfile 自动炼丹与炉体独立 qi 储量。

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component, Entity, Event, EventReader, Query};

use super::furnace::AlchemyFurnace;
use super::quality::auto_profile_quality_cap;
use super::session::Intervention;

#[derive(Debug, Clone, Component, Serialize, Deserialize, PartialEq)]
pub struct AlchemyAutoProfile {
    pub profile_id: String,
    pub fire_curve: Vec<(f32, f32)>,
    pub qi_feed_rate: f32,
    pub max_sessions: u32,
    #[serde(default)]
    pub sessions_completed: u32,
}

#[derive(Debug, Clone, Copy, Component, Serialize, Deserialize, PartialEq)]
pub struct FurnaceQiReserve {
    pub current: f32,
    pub capacity: f32,
    pub injection_rate_cap: f32,
}

#[derive(Debug, Clone, Copy, Event, PartialEq)]
pub struct InjectQiIntent {
    pub furnace_entity: Entity,
    pub amount_per_sec: f32,
}

impl FurnaceQiReserve {
    pub fn inject(&mut self, requested_per_sec: f32) -> f32 {
        if !requested_per_sec.is_finite() || requested_per_sec <= 0.0 {
            return 0.0;
        }
        let accepted = requested_per_sec.min(self.injection_rate_cap).max(0.0);
        let before = self.current;
        self.current = (self.current + accepted).min(self.capacity.max(0.0));
        self.current - before
    }

    pub fn drain(&mut self, amount: f32) -> bool {
        if !amount.is_finite() || amount <= 0.0 {
            return true;
        }
        if self.current + f32::EPSILON < amount {
            self.current = 0.0;
            return false;
        }
        self.current -= amount;
        true
    }
}

pub fn inject_qi_to_furnace_reserve(
    mut intents: EventReader<InjectQiIntent>,
    mut reserves: Query<&mut FurnaceQiReserve>,
) {
    for intent in intents.read() {
        let Ok(mut reserve) = reserves.get_mut(intent.furnace_entity) else {
            continue;
        };
        reserve.inject(intent.amount_per_sec);
    }
}

pub fn tick_auto_profiles(
    mut furnaces: Query<(
        &mut AlchemyFurnace,
        &mut FurnaceQiReserve,
        &mut AlchemyAutoProfile,
    )>,
) {
    for (mut furnace, mut reserve, profile) in &mut furnaces {
        if profile.sessions_completed >= profile.max_sessions {
            continue;
        }
        let Some(session) = furnace.session.as_mut() else {
            continue;
        };
        if session.finished {
            continue;
        }
        let Some(temp) = fire_curve_temperature(&profile.fire_curve, session.elapsed_ticks) else {
            continue;
        };
        let qi_per_tick = (profile.qi_feed_rate / 20.0).max(0.0);
        if !reserve.drain(qi_per_tick) {
            tracing::info!(
                "[bong][alchemy][auto] profile `{}` stopped: furnace qi reserve empty",
                profile.profile_id
            );
            continue;
        }
        session.apply_intervention(Intervention::AdjustTemp(f64::from(temp)));
        session.apply_intervention(Intervention::InjectQi(f64::from(qi_per_tick)));
    }
}

pub fn fire_curve_temperature(curve: &[(f32, f32)], elapsed_ticks: u32) -> Option<f32> {
    if curve.is_empty() {
        return None;
    }
    let pct = (elapsed_ticks as f32 / 20.0).fract().clamp(0.0, 1.0);
    let mut sorted = curve.to_vec();
    sorted.sort_by(|a, b| a.0.total_cmp(&b.0));
    let first = sorted[0];
    if pct <= first.0 {
        return Some(first.1.clamp(0.0, 1.0));
    }
    for pair in sorted.windows(2) {
        let (left_pct, left_temp) = pair[0];
        let (right_pct, right_temp) = pair[1];
        if pct <= right_pct {
            let span = (right_pct - left_pct).max(f32::EPSILON);
            let t = ((pct - left_pct) / span).clamp(0.0, 1.0);
            return Some((left_temp + (right_temp - left_temp) * t).clamp(0.0, 1.0));
        }
    }
    sorted.last().map(|(_, temp)| temp.clamp(0.0, 1.0))
}

pub fn auto_profile_quality_limit(best_manual_quality: f64) -> f64 {
    auto_profile_quality_cap(best_manual_quality)
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::App;

    #[test]
    fn inject_qi_intent_caps_by_reserve_rate_and_capacity() {
        let mut app = App::new();
        app.add_event::<InjectQiIntent>();
        app.add_systems(valence::prelude::Update, inject_qi_to_furnace_reserve);
        let furnace = app
            .world_mut()
            .spawn(FurnaceQiReserve {
                current: 9.0,
                capacity: 10.0,
                injection_rate_cap: 3.0,
            })
            .id();

        app.world_mut().send_event(InjectQiIntent {
            furnace_entity: furnace,
            amount_per_sec: 9.0,
        });
        app.update();

        let reserve = app.world().get::<FurnaceQiReserve>(furnace).unwrap();
        assert_eq!(reserve.current, 10.0);
    }

    #[test]
    fn fire_curve_interpolates_between_points() {
        let temp =
            fire_curve_temperature(&[(0.0, 0.2), (0.5, 0.8), (1.0, 0.4)], 5).expect("temperature");

        assert!((temp - 0.5).abs() < 1e-6);
    }

    #[test]
    fn auto_profile_drains_furnace_reserve_without_touching_player_qi() {
        let mut furnace = AlchemyFurnace::new(1);
        furnace
            .start_session(crate::alchemy::session::AlchemySession::new(
                "hui_yuan_pill_v0".to_string(),
                "offline:Azure".to_string(),
            ))
            .unwrap();
        let mut app = App::new();
        app.add_systems(valence::prelude::Update, tick_auto_profiles);
        let entity = app
            .world_mut()
            .spawn((
                furnace,
                FurnaceQiReserve {
                    current: 10.0,
                    capacity: 20.0,
                    injection_rate_cap: 5.0,
                },
                AlchemyAutoProfile {
                    profile_id: "safe".to_string(),
                    fire_curve: vec![(0.0, 0.4), (1.0, 0.6)],
                    qi_feed_rate: 20.0,
                    max_sessions: 1,
                    sessions_completed: 0,
                },
            ))
            .id();

        app.update();

        let (furnace, reserve, _) = app
            .world_mut()
            .query::<(&AlchemyFurnace, &FurnaceQiReserve, &AlchemyAutoProfile)>()
            .get(app.world(), entity)
            .unwrap();
        assert!(reserve.current < 10.0);
        assert!(furnace
            .session
            .as_ref()
            .is_some_and(|session| session.qi_injected > 0.0));
    }

    #[test]
    fn auto_profile_quality_limit_keeps_balance_cap() {
        assert!((auto_profile_quality_limit(0.9) - 0.765).abs() < 1e-9);
    }
}
