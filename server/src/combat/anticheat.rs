use std::fs;
use std::path::Path;

use serde::Deserialize;
use valence::prelude::{
    bevy_ecs, Component, Entity, Event, EventWriter, Query, Res, Resource, Username,
};

use crate::combat::components::Lifecycle;
use crate::combat::CombatClock;
use crate::player::state::canonical_player_id;
use crate::schema::anticheat::{AntiCheatReportV1, ViolationKindV1};

pub const DEFAULT_ANTICHEAT_CONFIG_PATH: &str = "assets/config/anticheat.toml";

#[derive(Debug, Clone, Component, Default)]
pub struct AntiCheatCounter {
    pub reach_violations: u32,
    pub cooldown_violations: u32,
    pub qi_invest_violations: u32,
    pub last_report_tick: u64,
    pub last_reach_details: String,
    pub last_cooldown_details: String,
    pub last_qi_invest_details: String,
    reach_reported_count: u32,
    cooldown_reported_count: u32,
    qi_invest_reported_count: u32,
    next_report_kind_cursor: usize,
}

impl AntiCheatCounter {
    pub fn record_violation(&mut self, kind: ViolationKindV1, details: impl Into<String>) -> u32 {
        let count = self.increment(kind);
        self.set_details(kind, details.into());
        count
    }

    fn next_reportable_kind(
        &self,
        at_tick: u64,
        config: &AntiCheatConfig,
    ) -> Option<ViolationKindV1> {
        if self.last_report_tick != 0
            && at_tick.saturating_sub(self.last_report_tick) < config.report_cooldown_ticks
        {
            return None;
        }

        let kinds = [
            ViolationKindV1::ReachExceeded,
            ViolationKindV1::CooldownBypassed,
            ViolationKindV1::QiInvestExceeded,
        ];
        let start = self.next_report_kind_cursor % kinds.len();
        (0..kinds.len())
            .map(|offset| kinds[(start + offset) % kinds.len()])
            .find(|kind| {
                self.count_for(*kind) >= config.threshold_for(*kind)
                    && self.count_for(*kind) > self.reported_count_for(*kind)
            })
    }

    fn build_report(
        &mut self,
        entity: Entity,
        char_id: &str,
        at_tick: u64,
        kind: ViolationKindV1,
        config: &AntiCheatConfig,
    ) -> Option<AntiCheatReportV1> {
        let count = self.count_for(kind);
        if count < config.threshold_for(kind) {
            return None;
        }
        if count <= self.reported_count_for(kind) {
            return None;
        }
        if self.last_report_tick != 0
            && at_tick.saturating_sub(self.last_report_tick) < config.report_cooldown_ticks
        {
            return None;
        }

        self.last_report_tick = at_tick;
        self.set_reported_count(kind, count);
        self.next_report_kind_cursor = Self::kind_index(kind).saturating_add(1);
        Some(AntiCheatReportV1::new(
            char_id,
            entity.to_bits(),
            at_tick,
            kind,
            count,
            self.details_for(kind),
        ))
    }

    fn increment(&mut self, kind: ViolationKindV1) -> u32 {
        match kind {
            ViolationKindV1::ReachExceeded => {
                self.reach_violations = self.reach_violations.saturating_add(1);
                self.reach_violations
            }
            ViolationKindV1::CooldownBypassed => {
                self.cooldown_violations = self.cooldown_violations.saturating_add(1);
                self.cooldown_violations
            }
            ViolationKindV1::QiInvestExceeded => {
                self.qi_invest_violations = self.qi_invest_violations.saturating_add(1);
                self.qi_invest_violations
            }
        }
    }

    fn count_for(&self, kind: ViolationKindV1) -> u32 {
        match kind {
            ViolationKindV1::ReachExceeded => self.reach_violations,
            ViolationKindV1::CooldownBypassed => self.cooldown_violations,
            ViolationKindV1::QiInvestExceeded => self.qi_invest_violations,
        }
    }

    fn reported_count_for(&self, kind: ViolationKindV1) -> u32 {
        match kind {
            ViolationKindV1::ReachExceeded => self.reach_reported_count,
            ViolationKindV1::CooldownBypassed => self.cooldown_reported_count,
            ViolationKindV1::QiInvestExceeded => self.qi_invest_reported_count,
        }
    }

    fn set_reported_count(&mut self, kind: ViolationKindV1, count: u32) {
        match kind {
            ViolationKindV1::ReachExceeded => self.reach_reported_count = count,
            ViolationKindV1::CooldownBypassed => self.cooldown_reported_count = count,
            ViolationKindV1::QiInvestExceeded => self.qi_invest_reported_count = count,
        }
    }

    fn set_details(&mut self, kind: ViolationKindV1, details: String) {
        match kind {
            ViolationKindV1::ReachExceeded => self.last_reach_details = details,
            ViolationKindV1::CooldownBypassed => self.last_cooldown_details = details,
            ViolationKindV1::QiInvestExceeded => self.last_qi_invest_details = details,
        }
    }

    fn details_for(&self, kind: ViolationKindV1) -> &str {
        match kind {
            ViolationKindV1::ReachExceeded => self.last_reach_details.as_str(),
            ViolationKindV1::CooldownBypassed => self.last_cooldown_details.as_str(),
            ViolationKindV1::QiInvestExceeded => self.last_qi_invest_details.as_str(),
        }
    }

    fn kind_index(kind: ViolationKindV1) -> usize {
        match kind {
            ViolationKindV1::ReachExceeded => 0,
            ViolationKindV1::CooldownBypassed => 1,
            ViolationKindV1::QiInvestExceeded => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, Resource, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AntiCheatConfig {
    pub reach_threshold: u32,
    pub cooldown_threshold: u32,
    pub qi_invest_threshold: u32,
    pub report_cooldown_ticks: u64,
}

impl Default for AntiCheatConfig {
    fn default() -> Self {
        Self {
            reach_threshold: 10,
            cooldown_threshold: 5,
            qi_invest_threshold: 20,
            report_cooldown_ticks: 1200,
        }
    }
}

impl AntiCheatConfig {
    pub fn threshold_for(&self, kind: ViolationKindV1) -> u32 {
        match kind {
            ViolationKindV1::ReachExceeded => self.reach_threshold,
            ViolationKindV1::CooldownBypassed => self.cooldown_threshold,
            ViolationKindV1::QiInvestExceeded => self.qi_invest_threshold,
        }
        .max(1)
    }

    fn validate(self) -> Result<Self, String> {
        if self.reach_threshold == 0 {
            return Err("reach_threshold must be >= 1".to_string());
        }
        if self.cooldown_threshold == 0 {
            return Err("cooldown_threshold must be >= 1".to_string());
        }
        if self.qi_invest_threshold == 0 {
            return Err("qi_invest_threshold must be >= 1".to_string());
        }
        Ok(self)
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct AntiCheatConfigFile {
    anticheat: AntiCheatConfig,
}

pub fn load_anticheat_config(path: impl AsRef<Path>) -> Result<AntiCheatConfig, String> {
    let path = path.as_ref();
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let parsed: AntiCheatConfigFile = toml::from_str(raw.as_str())
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
    parsed.anticheat.validate()
}

#[derive(Debug, Clone, Event)]
pub struct AntiCheatViolationEvent {
    pub report: AntiCheatReportV1,
}

pub fn log_anticheat_report(report: &AntiCheatReportV1) {
    tracing::error!(
        "[bong][anticheat] char_id={} entity_id={} kind={:?} count={} tick={} details={}",
        report.char_id,
        report.entity_id,
        report.kind,
        report.count,
        report.at_tick,
        report.details
    );
}

pub fn emit_anticheat_threshold_reports(
    clock: Res<CombatClock>,
    anticheat_config: Option<Res<AntiCheatConfig>>,
    mut reports: EventWriter<AntiCheatViolationEvent>,
    mut counters: Query<(
        Entity,
        &mut AntiCheatCounter,
        Option<&Lifecycle>,
        Option<&Username>,
    )>,
) {
    let anticheat_config = anticheat_config.as_deref().copied().unwrap_or_default();

    for (entity, mut counter, lifecycle, username) in &mut counters {
        let Some(kind) = counter.next_reportable_kind(clock.tick, &anticheat_config) else {
            continue;
        };
        let char_id = lifecycle
            .map(|lifecycle| lifecycle.character_id.clone())
            .or_else(|| username.map(|username| canonical_player_id(username.0.as_str())))
            .unwrap_or_else(|| format!("entity:{}", entity.to_bits()));
        let Some(report) = counter.build_report(
            entity,
            char_id.as_str(),
            clock.tick,
            kind,
            &anticheat_config,
        ) else {
            continue;
        };

        log_anticheat_report(&report);
        reports.send(AntiCheatViolationEvent { report });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use valence::prelude::{App, Events, Update};

    #[test]
    fn counter_reports_only_after_threshold_and_cooldown() {
        let config = AntiCheatConfig {
            reach_threshold: 2,
            cooldown_threshold: 2,
            qi_invest_threshold: 2,
            report_cooldown_ticks: 10,
        };
        let entity = Entity::from_raw(42);
        let mut counter = AntiCheatCounter::default();

        counter.record_violation(
            ViolationKindV1::ReachExceeded,
            "reach: target_distance=3.0 server_max=1.3",
        );
        let first = counter.build_report(
            entity,
            "offline:Azure",
            5,
            ViolationKindV1::ReachExceeded,
            &config,
        );
        assert!(first.is_none());

        counter.record_violation(
            ViolationKindV1::ReachExceeded,
            "reach: target_distance=3.0 server_max=1.3",
        );
        let second = counter
            .build_report(
                entity,
                "offline:Azure",
                6,
                ViolationKindV1::ReachExceeded,
                &config,
            )
            .expect("threshold crossing should report");
        assert_eq!(second.kind, ViolationKindV1::ReachExceeded);
        assert_eq!(second.count, 2);
        assert_eq!(second.entity_id, entity.to_bits());

        counter.record_violation(
            ViolationKindV1::ReachExceeded,
            "reach: target_distance=3.0 server_max=1.3",
        );
        let suppressed = counter.build_report(
            entity,
            "offline:Azure",
            10,
            ViolationKindV1::ReachExceeded,
            &config,
        );
        assert!(suppressed.is_none());

        let after_cooldown = counter.build_report(
            entity,
            "offline:Azure",
            16,
            ViolationKindV1::ReachExceeded,
            &config,
        );
        assert!(after_cooldown.is_some());
    }

    #[test]
    fn default_asset_config_loads() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(DEFAULT_ANTICHEAT_CONFIG_PATH);
        let config = load_anticheat_config(path).expect("default anticheat config should load");
        assert_eq!(config.reach_threshold, 10);
        assert_eq!(config.cooldown_threshold, 5);
        assert_eq!(config.qi_invest_threshold, 20);
        assert_eq!(config.report_cooldown_ticks, 1200);
    }

    #[test]
    fn emit_threshold_reports_sends_event_with_lifecycle_character_id() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 42 });
        app.insert_resource(AntiCheatConfig {
            reach_threshold: 2,
            cooldown_threshold: 2,
            qi_invest_threshold: 2,
            report_cooldown_ticks: 10,
        });
        app.add_event::<AntiCheatViolationEvent>();
        app.add_systems(Update, emit_anticheat_threshold_reports);

        let mut counter = AntiCheatCounter::default();
        counter.record_violation(
            ViolationKindV1::ReachExceeded,
            "reach: target_distance=3.0 server_max=1.3",
        );
        counter.record_violation(
            ViolationKindV1::ReachExceeded,
            "reach: target_distance=4.0 server_max=1.3",
        );
        let entity = app
            .world_mut()
            .spawn((
                counter,
                Lifecycle {
                    character_id: "offline:Azure".to_string(),
                    ..Default::default()
                },
            ))
            .id();

        app.update();

        let events = app.world().resource::<Events<AntiCheatViolationEvent>>();
        let emitted: Vec<_> = events.get_reader().read(events).cloned().collect();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].report.char_id, "offline:Azure");
        assert_eq!(emitted[0].report.entity_id, entity.to_bits());
        assert_eq!(emitted[0].report.kind, ViolationKindV1::ReachExceeded);
        assert_eq!(emitted[0].report.count, 2);
        assert_eq!(
            emitted[0].report.details,
            "reach: target_distance=4.0 server_max=1.3"
        );
    }

    #[test]
    fn report_selection_rotates_between_ready_violation_kinds() {
        let config = AntiCheatConfig {
            reach_threshold: 1,
            cooldown_threshold: 1,
            qi_invest_threshold: 1,
            report_cooldown_ticks: 10,
        };
        let entity = Entity::from_raw(42);
        let mut counter = AntiCheatCounter::default();
        counter.record_violation(ViolationKindV1::ReachExceeded, "reach: first");
        counter.record_violation(ViolationKindV1::CooldownBypassed, "cooldown: first");

        let first = counter
            .build_report(
                entity,
                "offline:Azure",
                10,
                ViolationKindV1::ReachExceeded,
                &config,
            )
            .expect("reach should report first by default");
        assert_eq!(first.kind, ViolationKindV1::ReachExceeded);

        counter.record_violation(ViolationKindV1::ReachExceeded, "reach: second");
        assert_eq!(
            counter.next_reportable_kind(20, &config),
            Some(ViolationKindV1::CooldownBypassed),
            "cooldown should not be starved by continuing reach violations"
        );
        let second = counter
            .build_report(
                entity,
                "offline:Azure",
                20,
                ViolationKindV1::CooldownBypassed,
                &config,
            )
            .expect("cooldown should report after rotation");
        assert_eq!(second.kind, ViolationKindV1::CooldownBypassed);
        assert_eq!(second.details, "cooldown: first");
    }
}
