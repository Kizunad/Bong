use std::collections::{HashMap, VecDeque};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;
use valence::prelude::{Entity, Query, ResMut, Resource, With};

use crate::npc::brain::{canonical_npc_id, NpcBehaviorConfig};
use crate::npc::spawn::NpcMarker;
use crate::schema::agent_command::{AgentCommandV1, Command};
use crate::schema::common::{CommandType, MAX_COMMANDS_PER_TICK};
use crate::world::events::ActiveEventsResource;
use crate::world::zone::ZoneRegistry;

const ZONE_SPIRIT_QI_MIN: f64 = 0.0;
const ZONE_SPIRIT_QI_MAX: f64 = 1.0;
const ZONE_DANGER_LEVEL_MIN: i64 = 0;
const ZONE_DANGER_LEVEL_MAX: i64 = 5;
const COMMAND_BATCH_DEDUPE_WINDOW_SECS: u64 = 30;
const COMMAND_BATCH_DEDUPE_CAPACITY: usize = 256;

#[derive(Debug, Clone, PartialEq)]
pub struct BatchEnqueueOutcome {
    pub accepted: bool,
    pub dedupe_drop: bool,
}

impl BatchEnqueueOutcome {
    fn accepted() -> Self {
        Self {
            accepted: true,
            dedupe_drop: false,
        }
    }

    fn dedupe_dropped() -> Self {
        Self {
            accepted: false,
            dedupe_drop: true,
        }
    }
}

#[derive(Default)]
pub struct CommandExecutorResource {
    pending_batches: VecDeque<AgentCommandV1>,
    recently_seen_batch_ids: VecDeque<(String, u64)>,
}

impl Resource for CommandExecutorResource {}

impl CommandExecutorResource {
    pub fn enqueue_batch(&mut self, batch: AgentCommandV1) -> BatchEnqueueOutcome {
        let now_secs = current_unix_timestamp_secs();
        self.prune_seen_batch_ids(now_secs);

        if self
            .recently_seen_batch_ids
            .iter()
            .any(|(batch_id, _)| batch_id == &batch.id)
        {
            return BatchEnqueueOutcome::dedupe_dropped();
        }

        self.remember_batch_id(batch.id.as_str(), now_secs);
        self.pending_batches.push_back(batch);
        BatchEnqueueOutcome::accepted()
    }

    fn prune_seen_batch_ids(&mut self, now_secs: u64) {
        while let Some((_, seen_at_secs)) = self.recently_seen_batch_ids.front() {
            let age_secs = now_secs.saturating_sub(*seen_at_secs);
            if age_secs > COMMAND_BATCH_DEDUPE_WINDOW_SECS {
                self.recently_seen_batch_ids.pop_front();
                continue;
            }
            break;
        }

        while self.recently_seen_batch_ids.len() > COMMAND_BATCH_DEDUPE_CAPACITY {
            self.recently_seen_batch_ids.pop_front();
        }
    }

    fn remember_batch_id(&mut self, batch_id: &str, now_secs: u64) {
        self.recently_seen_batch_ids
            .push_back((batch_id.to_string(), now_secs));
        while self.recently_seen_batch_ids.len() > COMMAND_BATCH_DEDUPE_CAPACITY {
            self.recently_seen_batch_ids.pop_front();
        }
    }

    #[cfg(test)]
    fn pending_command_count(&self) -> usize {
        self.pending_batches
            .iter()
            .map(|batch| batch.commands.len())
            .sum()
    }

    #[cfg(test)]
    fn dedupe_cache_len(&self) -> usize {
        self.recently_seen_batch_ids.len()
    }
}

pub fn execute_agent_commands(
    mut executor: ResMut<CommandExecutorResource>,
    mut zone_registry: Option<ResMut<ZoneRegistry>>,
    mut active_events: Option<ResMut<ActiveEventsResource>>,
    mut npc_behavior: Option<ResMut<NpcBehaviorConfig>>,
    npc_entities: Query<Entity, With<NpcMarker>>,
) {
    let mut remaining_budget = MAX_COMMANDS_PER_TICK;

    while remaining_budget > 0 {
        let Some(mut batch) = executor.pending_batches.pop_front() else {
            break;
        };

        let batch_id = batch.id.clone();
        let batch_source = batch.source.clone();

        let mut consumed = 0usize;
        while consumed < batch.commands.len() && remaining_budget > 0 {
            execute_single_command(
                &batch.commands[consumed],
                batch_id.as_str(),
                batch_source.as_deref(),
                &mut zone_registry,
                &mut active_events,
                &mut npc_behavior,
                &npc_entities,
            );
            consumed += 1;
            remaining_budget -= 1;
        }

        if consumed < batch.commands.len() {
            batch.commands.drain(0..consumed);
            executor.pending_batches.push_front(batch);
            break;
        }
    }

    if remaining_budget == 0 && !executor.pending_batches.is_empty() {
        tracing::debug!(
            "[bong][network] command executor hit budget {MAX_COMMANDS_PER_TICK}; remaining commands will continue next tick"
        );
    }
}

fn execute_single_command(
    command: &Command,
    batch_id: &str,
    source: Option<&str>,
    zone_registry: &mut Option<ResMut<ZoneRegistry>>,
    active_events: &mut Option<ResMut<ActiveEventsResource>>,
    npc_behavior: &mut Option<ResMut<NpcBehaviorConfig>>,
    npc_entities: &Query<Entity, With<NpcMarker>>,
) {
    let command_type = command_type_label(&command.command_type);

    tracing::info!(
        "[bong][network] command_anchor stage=begin batch_id={} source={} type={} target={} result=pending",
        batch_id,
        source.unwrap_or("unknown"),
        command_type,
        command.target
    );

    let result = match command.command_type {
        CommandType::ModifyZone => execute_modify_zone(command, zone_registry),
        CommandType::NpcBehavior => execute_npc_behavior(command, npc_behavior, npc_entities),
        CommandType::SpawnEvent => execute_spawn_event(command, zone_registry, active_events),
    };

    tracing::info!(
        "[bong][network] command_anchor stage=end batch_id={} source={} type={} target={} result={}",
        batch_id,
        source.unwrap_or("unknown"),
        command_type,
        command.target,
        result
    );
}

fn command_type_label(command_type: &CommandType) -> &'static str {
    match command_type {
        CommandType::ModifyZone => "modify_zone",
        CommandType::NpcBehavior => "npc_behavior",
        CommandType::SpawnEvent => "spawn_event",
    }
}

fn execute_spawn_event(
    command: &Command,
    zone_registry: &mut Option<ResMut<ZoneRegistry>>,
    active_events: &mut Option<ResMut<ActiveEventsResource>>,
) -> &'static str {
    let Some(active_events) = active_events.as_deref_mut() else {
        tracing::warn!(
            "[bong][network] cannot enqueue spawn_event for `{}` because ActiveEventsResource is missing",
            command.target
        );
        return "rejected_missing_active_events";
    };

    if active_events.enqueue_from_spawn_command(command, zone_registry.as_deref_mut()) {
        "ok"
    } else {
        "rejected_spawn_event"
    }
}

fn execute_modify_zone(
    command: &Command,
    zone_registry: &mut Option<ResMut<ZoneRegistry>>,
) -> &'static str {
    let Some(zone_registry) = zone_registry.as_deref_mut() else {
        tracing::warn!(
            "[bong][network] cannot execute modify_zone for `{}` because ZoneRegistry resource is missing",
            command.target
        );
        return "rejected_missing_zone_registry";
    };

    let Some(zone) = zone_registry.find_zone_mut(command.target.as_str()) else {
        tracing::warn!(
            "[bong][network] modify_zone target `{}` does not match any known zone",
            command.target
        );
        return "rejected_unknown_zone";
    };

    let Some(spirit_qi_delta) = param_as_f64(&command.params, "spirit_qi_delta") else {
        tracing::warn!(
            "[bong][network] modify_zone target `{}` missing/invalid `spirit_qi_delta`",
            command.target
        );
        return "rejected_invalid_spirit_qi_delta";
    };

    zone.spirit_qi =
        (zone.spirit_qi + spirit_qi_delta).clamp(ZONE_SPIRIT_QI_MIN, ZONE_SPIRIT_QI_MAX);

    match optional_param_as_i64(&command.params, "danger_level_delta") {
        Some(delta) => {
            zone.danger_level = ((zone.danger_level as i64 + delta)
                .clamp(ZONE_DANGER_LEVEL_MIN, ZONE_DANGER_LEVEL_MAX))
                as u8;
        }
        None if command.params.contains_key("danger_level_delta") => {
            tracing::warn!(
                "[bong][network] modify_zone target `{}` has non-integer `danger_level_delta`, ignoring field",
                command.target
            );
        }
        None => {}
    }

    "ok"
}

fn execute_npc_behavior(
    command: &Command,
    npc_behavior: &mut Option<ResMut<NpcBehaviorConfig>>,
    npc_entities: &Query<Entity, With<NpcMarker>>,
) -> &'static str {
    let Some(flee_threshold) = param_as_f64(&command.params, "flee_threshold") else {
        tracing::warn!(
            "[bong][network] npc_behavior target `{}` missing/invalid `flee_threshold`",
            command.target
        );
        return "rejected_invalid_flee_threshold";
    };

    let flee_threshold = flee_threshold.clamp(0.0, 1.0) as f32;

    let Some(target_id) = parse_npc_id(command.target.as_str()) else {
        tracing::warn!(
            "[bong][network] npc_behavior target `{}` is not a canonical npc id (`npc_{{index}}v{{generation}}`)",
            command.target
        );
        return "rejected_invalid_npc_target";
    };

    let target_exists = npc_entities
        .iter()
        .any(|entity| canonical_npc_id(entity) == target_id);
    if !target_exists {
        tracing::warn!(
            "[bong][network] npc_behavior target `{}` does not map to a live NPC",
            command.target
        );
        return "rejected_unknown_npc";
    }

    apply_flee_threshold(npc_behavior, flee_threshold, target_id.as_str())
}

fn apply_flee_threshold(
    npc_behavior: &mut Option<ResMut<NpcBehaviorConfig>>,
    flee_threshold: f32,
    target: &str,
) -> &'static str {
    let Some(config) = npc_behavior.as_deref_mut() else {
        tracing::warn!(
            "[bong][network] cannot apply npc_behavior for `{target}` because NpcBehaviorConfig resource is missing"
        );
        return "rejected_missing_npc_behavior_config";
    };

    config.set_threshold_for_npc_id(target, flee_threshold);
    "ok"
}

fn current_unix_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_secs()
}

fn parse_npc_id(target: &str) -> Option<String> {
    let suffix = target.strip_prefix("npc_")?;
    let (index, generation) = suffix.split_once('v')?;
    let index = index.parse::<u32>().ok()?;
    let generation = generation.parse::<u32>().ok()?;
    let canonical_id = format!("npc_{index}v{generation}");

    (canonical_id == target).then_some(canonical_id)
}

fn param_as_f64(params: &HashMap<String, Value>, key: &str) -> Option<f64> {
    params.get(key).and_then(Value::as_f64)
}

fn optional_param_as_i64(params: &HashMap<String, Value>, key: &str) -> Option<i64> {
    let value = params.get(key)?;
    value_to_i64(Some(value))
}

fn value_to_i64(value: Option<&Value>) -> Option<i64> {
    let value = value?;

    if let Some(v) = value.as_i64() {
        return Some(v);
    }

    if let Some(v) = value.as_u64() {
        return i64::try_from(v).ok();
    }

    let v = value.as_f64()?;
    if !v.is_finite() {
        return None;
    }

    let rounded = v.round();
    if (v - rounded).abs() > f64::EPSILON {
        return None;
    }

    if rounded < i64::MIN as f64 || rounded > i64::MAX as f64 {
        return None;
    }

    Some(rounded as i64)
}

#[cfg(test)]
mod command_executor_tests {
    use super::*;
    use std::collections::HashMap;

    use serde_json::json;
    use valence::prelude::{App, DVec3, Update};

    use crate::npc::brain::{canonical_npc_id, NpcBehaviorConfig, DEFAULT_FLEE_THRESHOLD};
    use crate::schema::agent_command::Command;
    use crate::world::events::{ActiveEventsResource, EVENT_THUNDER_TRIBULATION};

    fn command(command_type: CommandType, target: &str, params: HashMap<String, Value>) -> Command {
        Command {
            command_type,
            target: target.to_string(),
            params,
        }
    }

    fn batch(id: &str, commands: Vec<Command>) -> AgentCommandV1 {
        AgentCommandV1 {
            v: 1,
            id: id.to_string(),
            source: Some("calamity".to_string()),
            commands,
        }
    }

    fn setup_executor_app() -> App {
        let mut app = App::new();
        app.insert_resource(CommandExecutorResource::default());
        app.insert_resource(ZoneRegistry::fallback());
        app.insert_resource(ActiveEventsResource::default());
        app.insert_resource(NpcBehaviorConfig::default());
        app.add_systems(Update, execute_agent_commands);
        app
    }

    #[test]
    fn applies_modify_zone() {
        let mut app = setup_executor_app();

        let mut params = HashMap::new();
        params.insert("spirit_qi_delta".to_string(), json!(-2.0));
        params.insert("danger_level_delta".to_string(), json!(99));

        {
            let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
            let outcome = executor.enqueue_batch(batch(
                "cmd_modify_zone",
                vec![command(CommandType::ModifyZone, "spawn", params)],
            ));
            assert!(outcome.accepted);
            assert!(!outcome.dedupe_drop);
        }

        app.update();

        let zone_registry = app.world().resource::<ZoneRegistry>();
        let spawn_zone = zone_registry
            .find_zone(DVec3::new(8.0, 66.0, 8.0))
            .expect("spawn zone should still exist");

        assert_eq!(spawn_zone.spirit_qi, 0.0);
        assert_eq!(spawn_zone.danger_level, 5);
    }

    #[test]
    fn caps_commands_per_tick() {
        let mut app = setup_executor_app();

        let commands = (0..(MAX_COMMANDS_PER_TICK + 1))
            .map(|_| {
                let mut params = HashMap::new();
                params.insert("spirit_qi_delta".to_string(), json!(-0.01));
                command(CommandType::ModifyZone, "spawn", params)
            })
            .collect::<Vec<_>>();

        {
            let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
            let outcome = executor.enqueue_batch(batch("cmd_budget", commands));
            assert!(outcome.accepted);
            assert!(!outcome.dedupe_drop);
        }

        app.update();

        {
            let zone_registry = app.world().resource::<ZoneRegistry>();
            let spawn_zone = zone_registry
                .find_zone(DVec3::new(8.0, 66.0, 8.0))
                .expect("spawn zone should still exist");
            let expected = 0.9 - (MAX_COMMANDS_PER_TICK as f64 * 0.01);
            assert!((spawn_zone.spirit_qi - expected).abs() < 1e-9);
        }

        {
            let executor = app.world().resource::<CommandExecutorResource>();
            assert_eq!(executor.pending_command_count(), 1);
        }

        app.update();

        {
            let zone_registry = app.world().resource::<ZoneRegistry>();
            let spawn_zone = zone_registry
                .find_zone(DVec3::new(8.0, 66.0, 8.0))
                .expect("spawn zone should still exist");
            let expected = 0.9 - ((MAX_COMMANDS_PER_TICK + 1) as f64 * 0.01);
            assert!((spawn_zone.spirit_qi - expected).abs() < 1e-9);
        }

        {
            let executor = app.world().resource::<CommandExecutorResource>();
            assert_eq!(executor.pending_command_count(), 0);
        }
    }

    #[test]
    fn updates_flee_threshold_only_for_generation_aware_canonical_target() {
        let mut app = setup_executor_app();
        let npc_a = app.world_mut().spawn(NpcMarker).id();
        let npc_b = app.world_mut().spawn(NpcMarker).id();
        let npc_a_id = canonical_npc_id(npc_a);
        let npc_b_id = canonical_npc_id(npc_b);

        let mut bare_index_params = HashMap::new();
        bare_index_params.insert("flee_threshold".to_string(), json!(0.2));

        {
            let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
            let outcome = executor.enqueue_batch(batch(
                "cmd_npc_behavior",
                vec![command(
                    CommandType::NpcBehavior,
                    format!("npc_{}", npc_a.index()).as_str(),
                    bare_index_params,
                )],
            ));
            assert!(outcome.accepted);
            assert!(!outcome.dedupe_drop);
        }

        app.update();

        {
            let behavior = app.world().resource::<NpcBehaviorConfig>();
            assert_eq!(behavior.threshold_for_npc(npc_a), DEFAULT_FLEE_THRESHOLD);
            assert_eq!(
                behavior.threshold_for_npc_id(npc_a_id.as_str()),
                DEFAULT_FLEE_THRESHOLD
            );
            assert_eq!(
                behavior.threshold_for_npc_id(npc_b_id.as_str()),
                DEFAULT_FLEE_THRESHOLD
            );
        }

        let mut canonical_params = HashMap::new();
        canonical_params.insert("flee_threshold".to_string(), json!(0.2));

        {
            let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
            let outcome = executor.enqueue_batch(batch(
                "cmd_npc_behavior_canonical",
                vec![command(
                    CommandType::NpcBehavior,
                    npc_a_id.as_str(),
                    canonical_params,
                )],
            ));
            assert!(outcome.accepted);
            assert!(!outcome.dedupe_drop);
        }

        app.update();

        let behavior = app.world().resource::<NpcBehaviorConfig>();
        assert!((behavior.threshold_for_npc(npc_a) - 0.2).abs() < 1e-6);
        assert_eq!(behavior.threshold_for_npc_id(npc_a_id.as_str()), 0.2);
        assert_eq!(
            behavior.threshold_for_npc_id(npc_b_id.as_str()),
            DEFAULT_FLEE_THRESHOLD
        );
        assert_eq!(behavior.default_flee_threshold, DEFAULT_FLEE_THRESHOLD);
    }

    #[test]
    fn rejects_unknown_targets() {
        let mut app = setup_executor_app();

        let mut commands = Vec::new();

        let mut bad_zone_params = HashMap::new();
        bad_zone_params.insert("spirit_qi_delta".to_string(), json!(0.1));
        commands.push(command(
            CommandType::ModifyZone,
            "unknown_zone",
            bad_zone_params,
        ));

        let mut bad_npc_params = HashMap::new();
        bad_npc_params.insert("flee_threshold".to_string(), json!(0.1));
        commands.push(command(
            CommandType::NpcBehavior,
            "npc_999999v1",
            bad_npc_params,
        ));

        let mut bad_event_params = HashMap::new();
        bad_event_params.insert("event".to_string(), json!(EVENT_THUNDER_TRIBULATION));
        bad_event_params.insert("intensity".to_string(), json!(0.8));
        bad_event_params.insert("duration_ticks".to_string(), json!(120));
        commands.push(command(
            CommandType::SpawnEvent,
            "missing_zone",
            bad_event_params,
        ));

        let mut unsupported_event_params = HashMap::new();
        unsupported_event_params.insert("event".to_string(), json!("realm_collapse"));
        unsupported_event_params.insert("intensity".to_string(), json!(0.3));
        commands.push(command(
            CommandType::SpawnEvent,
            "spawn",
            unsupported_event_params,
        ));

        let mut bad_params = HashMap::new();
        bad_params.insert("spirit_qi_delta".to_string(), json!("bad-number"));
        commands.push(command(CommandType::ModifyZone, "spawn", bad_params));

        {
            let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
            let outcome = executor.enqueue_batch(batch("cmd_reject_unknown", commands));
            assert!(outcome.accepted);
            assert!(!outcome.dedupe_drop);
        }

        app.update();

        let zone_registry = app.world().resource::<ZoneRegistry>();
        let spawn_zone = zone_registry
            .find_zone(DVec3::new(8.0, 66.0, 8.0))
            .expect("spawn zone should still exist");

        assert_eq!(spawn_zone.spirit_qi, 0.9);
        assert_eq!(spawn_zone.danger_level, 0);
        assert!(spawn_zone.active_events.is_empty());

        let behavior = app.world().resource::<NpcBehaviorConfig>();
        assert_eq!(behavior.default_flee_threshold, DEFAULT_FLEE_THRESHOLD);
        assert_eq!(
            behavior.threshold_for_npc_id("npc_999999v1"),
            DEFAULT_FLEE_THRESHOLD
        );
    }

    #[test]
    fn duplicate_batch_id_is_dropped_before_queueing() {
        let mut app = setup_executor_app();

        let mut params = HashMap::new();
        params.insert("spirit_qi_delta".to_string(), json!(-0.1));

        let duplicate_id = "cmd_dedupe_me";

        {
            let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
            let first = executor.enqueue_batch(batch(
                duplicate_id,
                vec![command(CommandType::ModifyZone, "spawn", params.clone())],
            ));
            let second = executor.enqueue_batch(batch(
                duplicate_id,
                vec![command(CommandType::ModifyZone, "spawn", params)],
            ));

            assert!(first.accepted);
            assert!(!first.dedupe_drop);
            assert!(!second.accepted);
            assert!(second.dedupe_drop);
            assert_eq!(executor.pending_command_count(), 1);
            assert_eq!(executor.dedupe_cache_len(), 1);
        }

        app.update();

        let zone_registry = app.world().resource::<ZoneRegistry>();
        let spawn_zone = zone_registry
            .find_zone(DVec3::new(8.0, 66.0, 8.0))
            .expect("spawn zone should still exist");
        assert!((spawn_zone.spirit_qi - 0.8).abs() < 1e-9);
    }
}
