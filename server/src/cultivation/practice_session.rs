use valence::prelude::{bevy_ecs, Component, Entity, Event};

use crate::cultivation::technique_proficiency::{
    practice_session_gain, practice_session_qi_cost_per_tick, should_exit_practice_session,
};
use crate::schema::common::NarrationStyle;

#[derive(Debug, Component)]
pub struct PracticeSession {
    pub technique_id: String,
    pub started_at_tick: u64,
    pub total_gain: f32,
}

#[derive(Debug, Event)]
pub struct PracticeSessionStarted {
    pub player: Entity,
    pub technique_id: String,
}

#[derive(Debug, Event)]
pub struct PracticeSessionEnded {
    pub player: Entity,
    pub technique_id: String,
    pub total_gain: f32,
    pub duration_ticks: u64,
}

#[derive(Debug, Component, Default)]
pub(crate) struct PracticeSessionTracker {
    pub(crate) last_technique: Option<String>,
    pub(crate) consecutive_casts: u32,
}

const CONSECUTIVE_CASTS_THRESHOLD: u32 = 3;

pub(crate) fn track_casts_for_practice_entry(
    tracker: &mut PracticeSessionTracker,
    technique_id: &str,
) -> bool {
    if tracker
        .last_technique
        .as_deref()
        .is_some_and(|last| last == technique_id)
    {
        tracker.consecutive_casts += 1;
    } else {
        tracker.last_technique = Some(technique_id.to_string());
        tracker.consecutive_casts = 1;
    }
    tracker.consecutive_casts >= CONSECUTIVE_CASTS_THRESHOLD
}

pub(crate) fn practice_narration_text(duration_ticks: u64) -> &'static str {
    if duration_ticks < 200 {
        "心浮气躁。练了等于没练。"
    } else if duration_ticks <= 1000 {
        "筋骨略记住了几分。不够。"
    } else {
        "筋骨酸痛。但下次出手会快一些。"
    }
}

pub(crate) fn practice_narration_style() -> NarrationStyle {
    NarrationStyle::Perception
}

#[allow(dead_code)]
fn practice_session_tick(
    session: &mut PracticeSession,
    zone_qi: f64,
    current_proficiency: f32,
    color_match: bool,
    meridian_health: f32,
    current_qi: &mut f64,
) -> f32 {
    let cost = practice_session_qi_cost_per_tick();
    *current_qi -= cost;
    let gain = practice_session_gain(zone_qi, current_proficiency, color_match, meridian_health);
    session.total_gain += gain;
    gain
}

#[allow(dead_code)]
fn check_practice_session_exit(
    session: &PracticeSession,
    current_tick: u64,
    moved: bool,
    attacked: bool,
    qi_ratio: f64,
    current_technique: Option<&str>,
) -> Option<PracticeSessionEnded> {
    let different_technique =
        current_technique.is_some_and(|technique| technique != session.technique_id);
    if different_technique || should_exit_practice_session(moved, attacked, qi_ratio) {
        Some(PracticeSessionEnded {
            player: Entity::PLACEHOLDER,
            technique_id: session.technique_id.clone(),
            total_gain: session.total_gain,
            duration_ticks: current_tick.saturating_sub(session.started_at_tick),
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn practice_session_starts_on_3_consecutive_casts() {
        let mut tracker = PracticeSessionTracker::default();
        assert!(
            !track_casts_for_practice_entry(&mut tracker, "sword.cleave"),
            "1st cast should not trigger practice session"
        );
        assert_eq!(tracker.consecutive_casts, 1);
        assert!(
            !track_casts_for_practice_entry(&mut tracker, "sword.cleave"),
            "2nd cast should not trigger practice session"
        );
        assert_eq!(tracker.consecutive_casts, 2);
        assert!(
            track_casts_for_practice_entry(&mut tracker, "sword.cleave"),
            "3rd consecutive same-technique cast should trigger practice session entry"
        );
        assert_eq!(tracker.consecutive_casts, 3);
    }

    #[test]
    fn practice_session_tracker_resets_on_different_technique() {
        let mut tracker = PracticeSessionTracker::default();
        track_casts_for_practice_entry(&mut tracker, "sword.cleave");
        track_casts_for_practice_entry(&mut tracker, "sword.cleave");
        assert_eq!(tracker.consecutive_casts, 2);

        track_casts_for_practice_entry(&mut tracker, "sword.thrust");
        assert_eq!(
            tracker.consecutive_casts, 1,
            "switching technique should reset consecutive count to 1"
        );
        assert_eq!(tracker.last_technique.as_deref(), Some("sword.thrust"));
    }

    #[test]
    fn practice_session_ends_on_movement() {
        assert!(
            should_exit_practice_session(true, false, 1.0),
            "movement (moved=true) should trigger practice session exit"
        );
    }

    #[test]
    fn practice_session_ends_on_combat_hit() {
        assert!(
            should_exit_practice_session(false, true, 1.0),
            "combat hit (attacked=true) should trigger practice session exit"
        );
    }

    #[test]
    fn practice_session_ends_on_low_qi() {
        assert!(
            should_exit_practice_session(false, false, 0.09),
            "qi_ratio=0.09 (below 10% threshold) should trigger practice session exit"
        );
        assert!(
            !should_exit_practice_session(false, false, 0.10),
            "qi_ratio=0.10 (exactly at threshold) should NOT trigger exit"
        );
        assert!(
            !should_exit_practice_session(false, false, 0.50),
            "qi_ratio=0.50 (above threshold) should NOT trigger exit"
        );
    }

    #[test]
    fn practice_session_gain_uses_zone_multiplier() {
        let normal_gain = practice_session_gain(0.5, 0.0, false, 1.0);
        let negative_gain = practice_session_gain(-0.3, 0.0, false, 1.0);
        assert!(
            negative_gain > normal_gain,
            "negative zone_qi (-0.3) should give higher gain ({negative_gain}) than normal zone_qi (0.5) gain ({normal_gain})"
        );
        let ratio = negative_gain / normal_gain;
        assert!(
            (ratio - 1.5).abs() < 1e-5,
            "zone_qi in [-0.5, 0.0) should give 1.5x multiplier, got {ratio}x"
        );
    }

    #[test]
    fn practice_session_gain_zone_qi_negative_2x() {
        let normal_gain = practice_session_gain(0.5, 0.0, false, 1.0);
        let deep_negative_gain = practice_session_gain(-0.6, 0.0, false, 1.0);
        let ratio = deep_negative_gain / normal_gain;
        assert!(
            (ratio - 2.0).abs() < 1e-5,
            "zone_qi < -0.5 should give 2.0x multiplier, got {ratio}x"
        );
    }

    #[test]
    fn practice_session_short_narration_under_200_ticks() {
        assert_eq!(
            practice_narration_text(0),
            "心浮气躁。练了等于没练。",
            "0 ticks (short) should produce impatience narration"
        );
        assert_eq!(
            practice_narration_text(100),
            "心浮气躁。练了等于没练。",
            "100 ticks (short) should produce impatience narration"
        );
        assert_eq!(
            practice_narration_text(199),
            "心浮气躁。练了等于没练。",
            "199 ticks (short boundary) should produce impatience narration"
        );
    }

    #[test]
    fn practice_session_long_narration_over_1000_ticks() {
        assert_eq!(
            practice_narration_text(1001),
            "筋骨酸痛。但下次出手会快一些。",
            "1001 ticks (long) should produce positive narration"
        );
        assert_eq!(
            practice_narration_text(5000),
            "筋骨酸痛。但下次出手会快一些。",
            "5000 ticks (very long) should produce positive narration"
        );
    }

    #[test]
    fn practice_session_medium_narration_200_to_1000_ticks() {
        assert_eq!(
            practice_narration_text(200),
            "筋骨略记住了几分。不够。",
            "200 ticks (medium lower bound) should produce medium narration"
        );
        assert_eq!(
            practice_narration_text(500),
            "筋骨略记住了几分。不够。",
            "500 ticks (medium) should produce medium narration"
        );
        assert_eq!(
            practice_narration_text(1000),
            "筋骨略记住了几分。不够。",
            "1000 ticks (medium upper bound) should produce medium narration"
        );
    }

    #[test]
    fn practice_narration_uses_perception_style() {
        assert_eq!(
            practice_narration_style(),
            NarrationStyle::Perception,
            "practice session narration should use NarrationStyle::Perception"
        );
    }

    #[test]
    fn practice_session_tick_consumes_qi_and_accumulates_gain() {
        let mut session = PracticeSession {
            technique_id: "sword.cleave".to_string(),
            started_at_tick: 0,
            total_gain: 0.0,
        };
        let mut qi = 100.0;
        let gain = practice_session_tick(&mut session, 0.5, 0.0, false, 1.0, &mut qi);
        assert!(
            gain > 0.0,
            "practice session tick should produce positive gain"
        );
        assert!(
            (qi - (100.0 - practice_session_qi_cost_per_tick())).abs() < 1e-9,
            "qi should be reduced by qi_cost_per_tick ({}) per tick, got qi={qi}",
            practice_session_qi_cost_per_tick()
        );
        assert!(
            (session.total_gain - gain).abs() < 1e-9,
            "session.total_gain should accumulate the tick gain"
        );
    }

    #[test]
    fn check_exit_returns_ended_on_different_technique() {
        let session = PracticeSession {
            technique_id: "sword.cleave".to_string(),
            started_at_tick: 100,
            total_gain: 0.05,
        };
        let result =
            check_practice_session_exit(&session, 300, false, false, 1.0, Some("sword.thrust"));
        assert!(
            result.is_some(),
            "casting a different technique should trigger practice session exit"
        );
        let ended = result.unwrap();
        assert_eq!(ended.technique_id, "sword.cleave");
        assert_eq!(ended.duration_ticks, 200);
        assert!((ended.total_gain - 0.05).abs() < 1e-6);
    }

    #[test]
    fn check_exit_returns_none_when_no_exit_condition() {
        let session = PracticeSession {
            technique_id: "sword.cleave".to_string(),
            started_at_tick: 100,
            total_gain: 0.05,
        };
        let result = check_practice_session_exit(&session, 300, false, false, 0.50, None);
        assert!(result.is_none(), "no exit condition met should return None");
    }
}
