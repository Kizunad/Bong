use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use serde::Deserialize;

const DEFAULT_EVENT_RHYTHM_JSON: &str = include_str!("../../assets/world/event_rhythm.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RhythmEventKind {
    PseudoVein,
    BeastTide,
    TideSkyOmen,
    RealmCollapse,
    TribulationBroadcast,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayerLoopPhase {
    HomeOrganizing,
    OutboundSearch,
    DeepGathering,
    ReturnTrip,
    SafeShelter,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct EventTiming {
    pub lead_ticks: u64,
    pub min_duration_ticks: u64,
    pub max_duration_ticks: u64,
    pub frequency_multiplier: f64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct EventRhythmRule {
    pub event: RhythmEventKind,
    pub display_name: String,
    pub trigger_conditions: Vec<String>,
    pub preferred_phase: PlayerLoopPhase,
    pub insertion_point: String,
    pub emotional_effects: Vec<String>,
    pub default_timing: EventTiming,
    #[serde(default)]
    pub phase_timing: HashMap<PlayerLoopPhase, EventTiming>,
    #[serde(default)]
    pub narration_samples: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct EventRhythmConfig {
    pub version: u32,
    pub rules: Vec<EventRhythmRule>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EventTimingDecision<'a> {
    pub event: RhythmEventKind,
    pub phase: PlayerLoopPhase,
    pub preferred_phase: PlayerLoopPhase,
    pub is_preferred_phase: bool,
    pub timing: EventTiming,
    pub insertion_point: &'a str,
    pub emotional_effects: &'a [String],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlayerLoopPhaseEvidence {
    pub player_count: usize,
    pub safe_zone_players: usize,
    pub deep_zone_players: usize,
    pub return_route_players: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EventRhythmConfigError {
    Json(String),
    UnsupportedVersion(u32),
    MissingRules,
    DuplicateEvent(RhythmEventKind),
    MissingRequiredEvent(RhythmEventKind),
    InvalidRule {
        event: RhythmEventKind,
        reason: &'static str,
    },
}

impl EventRhythmConfig {
    pub fn rule(&self, event: RhythmEventKind) -> Option<&EventRhythmRule> {
        self.rules.iter().find(|rule| rule.event == event)
    }

    pub fn validate(&self) -> Result<(), EventRhythmConfigError> {
        if self.version != 1 {
            return Err(EventRhythmConfigError::UnsupportedVersion(self.version));
        }
        if self.rules.is_empty() {
            return Err(EventRhythmConfigError::MissingRules);
        }

        let mut seen = HashSet::new();
        for rule in &self.rules {
            if !seen.insert(rule.event) {
                return Err(EventRhythmConfigError::DuplicateEvent(rule.event));
            }
            validate_rule(rule)?;
        }

        for required in required_events() {
            if !seen.contains(required) {
                return Err(EventRhythmConfigError::MissingRequiredEvent(*required));
            }
        }

        Ok(())
    }
}

pub fn default_event_rhythm() -> &'static EventRhythmConfig {
    static CONFIG: OnceLock<EventRhythmConfig> = OnceLock::new();
    CONFIG.get_or_init(|| {
        parse_event_rhythm(DEFAULT_EVENT_RHYTHM_JSON)
            .expect("server/assets/world/event_rhythm.json must remain valid")
    })
}

pub fn parse_event_rhythm(text: &str) -> Result<EventRhythmConfig, EventRhythmConfigError> {
    let config: EventRhythmConfig = serde_json::from_str(text)
        .map_err(|error| EventRhythmConfigError::Json(error.to_string()))?;
    config.validate()?;
    Ok(config)
}

pub fn event_trigger_timing_by_player_loop_phase(
    config: &EventRhythmConfig,
    event: RhythmEventKind,
    phase: PlayerLoopPhase,
) -> Option<EventTimingDecision<'_>> {
    let rule = config.rule(event)?;
    let timing = rule
        .phase_timing
        .get(&phase)
        .copied()
        .unwrap_or(rule.default_timing);
    Some(EventTimingDecision {
        event,
        phase,
        preferred_phase: rule.preferred_phase,
        is_preferred_phase: phase == rule.preferred_phase,
        timing,
        insertion_point: rule.insertion_point.as_str(),
        emotional_effects: rule.emotional_effects.as_slice(),
    })
}

pub fn infer_player_loop_phase(evidence: PlayerLoopPhaseEvidence) -> PlayerLoopPhase {
    if evidence.player_count == 0 {
        return PlayerLoopPhase::SafeShelter;
    }
    if evidence.safe_zone_players == evidence.player_count {
        return PlayerLoopPhase::HomeOrganizing;
    }
    if evidence.deep_zone_players > 0 {
        return PlayerLoopPhase::DeepGathering;
    }
    if evidence.return_route_players > 0 {
        return PlayerLoopPhase::ReturnTrip;
    }
    PlayerLoopPhase::OutboundSearch
}

fn validate_rule(rule: &EventRhythmRule) -> Result<(), EventRhythmConfigError> {
    if rule.display_name.trim().is_empty() {
        return invalid(rule.event, "display_name must not be empty");
    }
    if rule.trigger_conditions.is_empty() {
        return invalid(rule.event, "trigger_conditions must not be empty");
    }
    if rule.insertion_point.trim().is_empty() {
        return invalid(rule.event, "insertion_point must not be empty");
    }
    if rule.emotional_effects.is_empty() {
        return invalid(rule.event, "emotional_effects must not be empty");
    }
    if rule.narration_samples.len() < 2 {
        return invalid(
            rule.event,
            "narration_samples must contain at least two lines",
        );
    }
    validate_timing(rule.event, rule.default_timing)?;
    let preferred_timing = rule.phase_timing.get(&rule.preferred_phase);
    if preferred_timing.is_none() {
        return invalid(
            rule.event,
            "preferred_phase must have explicit phase_timing",
        );
    }
    for timing in rule.phase_timing.values().copied() {
        validate_timing(rule.event, timing)?;
    }
    Ok(())
}

fn validate_timing(
    event: RhythmEventKind,
    timing: EventTiming,
) -> Result<(), EventRhythmConfigError> {
    if timing.lead_ticks == 0 {
        return invalid(event, "lead_ticks must be positive");
    }
    if timing.min_duration_ticks == 0 || timing.max_duration_ticks < timing.min_duration_ticks {
        return invalid(event, "duration range must be positive and ordered");
    }
    if !timing.frequency_multiplier.is_finite() || timing.frequency_multiplier <= 0.0 {
        return invalid(event, "frequency_multiplier must be positive and finite");
    }
    Ok(())
}

fn invalid<T>(event: RhythmEventKind, reason: &'static str) -> Result<T, EventRhythmConfigError> {
    Err(EventRhythmConfigError::InvalidRule { event, reason })
}

fn required_events() -> &'static [RhythmEventKind] {
    &[
        RhythmEventKind::PseudoVein,
        RhythmEventKind::BeastTide,
        RhythmEventKind::TideSkyOmen,
        RhythmEventKind::RealmCollapse,
        RhythmEventKind::TribulationBroadcast,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_asset_parses_and_pins_required_world_events() {
        let config = default_event_rhythm();

        for event in required_events() {
            assert!(
                config.rule(*event).is_some(),
                "event_rhythm.json must declare required P4 world event {event:?}"
            );
        }
    }

    #[test]
    fn preferred_phases_match_search_extract_loop_roles() {
        let config = default_event_rhythm();

        assert_eq!(
            config
                .rule(RhythmEventKind::PseudoVein)
                .unwrap()
                .preferred_phase,
            PlayerLoopPhase::ReturnTrip,
            "伪灵脉应优先插在回程路上形成绕路诱惑"
        );
        assert_eq!(
            config
                .rule(RhythmEventKind::BeastTide)
                .unwrap()
                .preferred_phase,
            PlayerLoopPhase::DeepGathering,
            "兽潮应优先插在深处采集阶段形成恐慌和机会"
        );
        assert_eq!(
            config
                .rule(RhythmEventKind::TideSkyOmen)
                .unwrap()
                .preferred_phase,
            PlayerLoopPhase::HomeOrganizing,
            "汐转期天象应优先插在回家整理阶段影响下一趟路线"
        );
        assert_eq!(
            config
                .rule(RhythmEventKind::RealmCollapse)
                .unwrap()
                .preferred_phase,
            PlayerLoopPhase::SafeShelter,
            "域崩应优先在安全区广播，避免把罕见失去事件做成贴脸惩罚"
        );
        assert_eq!(
            config
                .rule(RhythmEventKind::TribulationBroadcast)
                .unwrap()
                .preferred_phase,
            PlayerLoopPhase::OutboundSearch,
            "天劫广播应优先影响下一趟出门目标选择"
        );
    }

    #[test]
    fn preferred_phase_uses_shorter_lead_than_default() {
        let config = default_event_rhythm();

        for event in required_events() {
            let rule = config.rule(*event).unwrap();
            let preferred =
                event_trigger_timing_by_player_loop_phase(config, *event, rule.preferred_phase)
                    .unwrap();

            assert!(
                preferred.is_preferred_phase,
                "preferred decision must mark {event:?} as preferred phase"
            );
            assert!(
                preferred.timing.lead_ticks <= rule.default_timing.lead_ticks,
                "preferred phase should not delay {event:?}: preferred={} default={}",
                preferred.timing.lead_ticks,
                rule.default_timing.lead_ticks
            );
            assert!(
                preferred.timing.frequency_multiplier >= 1.0,
                "preferred phase should not make {event:?} rarer"
            );
        }
    }

    #[test]
    fn invalid_json_and_bad_contracts_are_rejected() {
        assert!(matches!(
            parse_event_rhythm("{"),
            Err(EventRhythmConfigError::Json(_))
        ));

        let mut config = default_event_rhythm().clone();
        config.version = 2;
        assert_eq!(
            config.validate(),
            Err(EventRhythmConfigError::UnsupportedVersion(2)),
            "unsupported event rhythm config versions must fail closed"
        );

        let mut config = default_event_rhythm().clone();
        config.rules.push(config.rules[0].clone());
        assert_eq!(
            config.validate(),
            Err(EventRhythmConfigError::DuplicateEvent(
                RhythmEventKind::PseudoVein
            )),
            "duplicate event rules should be rejected to avoid ambiguous timing"
        );
    }

    #[test]
    fn player_loop_phase_inference_prefers_observable_loop_state() {
        assert_eq!(
            infer_player_loop_phase(PlayerLoopPhaseEvidence::default()),
            PlayerLoopPhase::SafeShelter
        );
        assert_eq!(
            infer_player_loop_phase(PlayerLoopPhaseEvidence {
                player_count: 2,
                safe_zone_players: 2,
                ..Default::default()
            }),
            PlayerLoopPhase::HomeOrganizing
        );
        assert_eq!(
            infer_player_loop_phase(PlayerLoopPhaseEvidence {
                player_count: 2,
                deep_zone_players: 1,
                return_route_players: 1,
                ..Default::default()
            }),
            PlayerLoopPhase::DeepGathering,
            "deep gathering is the highest-pressure observable phase and should win over return hints"
        );
        assert_eq!(
            infer_player_loop_phase(PlayerLoopPhaseEvidence {
                player_count: 1,
                return_route_players: 1,
                ..Default::default()
            }),
            PlayerLoopPhase::ReturnTrip
        );
        assert_eq!(
            infer_player_loop_phase(PlayerLoopPhaseEvidence {
                player_count: 1,
                ..Default::default()
            }),
            PlayerLoopPhase::OutboundSearch
        );
    }
}
