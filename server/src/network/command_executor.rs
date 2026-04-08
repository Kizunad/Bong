use std::collections::VecDeque;

use valence::prelude::{ResMut, Resource};

use crate::npc::brain::NpcBehaviorRuntimeConfig;
use crate::schema::agent_command::{AgentCommandV1, Command};
use crate::schema::common::CommandType;
use crate::world::ZoneRegistry;

const MAX_COMMANDS_PER_FRAME: usize = crate::schema::common::MAX_COMMANDS_PER_TICK;
const THUNDER_TRIBULATION_EVENT: &str = "thunder_tribulation";
const PARAM_EVENT: &str = "event";
const PARAM_INTENSITY: &str = "intensity";
const PARAM_DURATION_TICKS: &str = "duration_ticks";
const PARAM_TARGET_PLAYER: &str = "target_player";
const PARAM_SPIRIT_QI_DELTA: &str = "spirit_qi_delta";
const PARAM_DANGER_LEVEL_DELTA: &str = "danger_level_delta";
const PARAM_FLEE_THRESHOLD: &str = "flee_threshold";
const NPC_BEHAVIOR_GLOBAL_TARGET: &str = "global";
const DEFAULT_EVENT_INTENSITY: f64 = 1.0;
const DEFAULT_EVENT_DURATION_TICKS: u64 = 200;
const MAX_EVENT_DURATION_TICKS: u64 = 7_200;

#[derive(Debug, Clone)]
pub struct QueuedAgentCommand {
    pub batch_id: String,
    pub source: Option<String>,
    pub command: Command,
}

#[derive(Debug, Default)]
pub struct CommandExecutorResource {
    queue: VecDeque<QueuedAgentCommand>,
}

impl Resource for CommandExecutorResource {}

impl CommandExecutorResource {
    pub fn enqueue_batch(&mut self, batch: AgentCommandV1) -> usize {
        let AgentCommandV1 {
            id,
            source,
            commands,
            ..
        } = batch;
        let command_count = commands.len();

        for command in commands {
            self.queue.push_back(QueuedAgentCommand {
                batch_id: id.clone(),
                source: source.clone(),
                command,
            });
        }

        command_count
    }

    fn dequeue_one(&mut self) -> Option<QueuedAgentCommand> {
        self.queue.pop_front()
    }

    pub fn pending_count(&self) -> usize {
        self.queue.len()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActiveThunderTribulationEvent {
    pub command_batch_id: String,
    pub source: Option<String>,
    pub zone: String,
    pub intensity: f64,
    pub duration_ticks: u64,
    pub target_player: Option<String>,
}

#[derive(Debug, Default)]
pub struct ActiveWorldEventsResource {
    pub thunder_tribulations: Vec<ActiveThunderTribulationEvent>,
}

impl Resource for ActiveWorldEventsResource {}

pub fn validate_and_enqueue_agent_command_batch(
    executor: &mut CommandExecutorResource,
    batch: AgentCommandV1,
) -> usize {
    if batch.v != 1 {
        tracing::warn!(
            "[bong][network][command_executor] skip command batch {} due to unsupported version v={} (expected v=1)",
            batch.id,
            batch.v
        );
        return 0;
    }

    let AgentCommandV1 {
        id,
        source,
        commands,
        ..
    } = batch;

    let mut accepted_commands = Vec::new();

    for (index, command) in commands.into_iter().enumerate() {
        match validate_command_for_enqueue(&command) {
            Ok(()) => accepted_commands.push(command),
            Err(reason) => {
                tracing::warn!(
                    "[bong][network][command_executor] skip invalid command in batch_id={} index={} type={:?} target={}: {}",
                    id,
                    index,
                    command.command_type,
                    command.target,
                    reason
                );
            }
        }
    }

    if accepted_commands.is_empty() {
        return 0;
    }

    executor.enqueue_batch(AgentCommandV1 {
        v: 1,
        id,
        source,
        commands: accepted_commands,
    })
}

pub fn execute_agent_commands(
    mut executor: ResMut<CommandExecutorResource>,
    mut zone_registry: ResMut<ZoneRegistry>,
    mut active_events: ResMut<ActiveWorldEventsResource>,
    mut npc_behavior_config: ResMut<NpcBehaviorRuntimeConfig>,
) {
    execute_agent_commands_with_limit(
        executor.as_mut(),
        zone_registry.as_mut(),
        active_events.as_mut(),
        npc_behavior_config.as_mut(),
        MAX_COMMANDS_PER_FRAME,
    );
}

fn execute_agent_commands_with_limit(
    executor: &mut CommandExecutorResource,
    zone_registry: &mut ZoneRegistry,
    active_events: &mut ActiveWorldEventsResource,
    npc_behavior_config: &mut NpcBehaviorRuntimeConfig,
    max_commands: usize,
) -> usize {
    let mut executed_count = 0;

    for _ in 0..max_commands {
        let Some(queued) = executor.dequeue_one() else {
            break;
        };

        execute_single_command(queued, zone_registry, active_events, npc_behavior_config);
        executed_count += 1;
    }

    executed_count
}

fn execute_single_command(
    queued: QueuedAgentCommand,
    zone_registry: &mut ZoneRegistry,
    active_events: &mut ActiveWorldEventsResource,
    npc_behavior_config: &mut NpcBehaviorRuntimeConfig,
) {
    match queued.command.command_type {
        CommandType::ModifyZone => handle_modify_zone(queued, zone_registry),
        CommandType::SpawnEvent => handle_spawn_event(queued, zone_registry, active_events),
        CommandType::NpcBehavior => handle_npc_behavior(queued, npc_behavior_config),
    }
}

fn validate_command_for_enqueue(command: &Command) -> Result<(), String> {
    if command.target.trim().is_empty() {
        return Err("target must be a non-empty string".to_string());
    }

    match command.command_type {
        CommandType::ModifyZone => validate_modify_zone_for_enqueue(command),
        CommandType::SpawnEvent => validate_spawn_event_for_enqueue(command),
        CommandType::NpcBehavior => validate_npc_behavior_for_enqueue(command),
    }
}

fn validate_modify_zone_for_enqueue(command: &Command) -> Result<(), String> {
    validate_no_unsupported_params(command, &[PARAM_SPIRIT_QI_DELTA, PARAM_DANGER_LEVEL_DELTA])?;

    let spirit_qi_delta = parse_optional_f64(command, PARAM_SPIRIT_QI_DELTA)
        .map_err(|error| format!("{PARAM_SPIRIT_QI_DELTA} {error}"))?;
    let danger_level_delta = parse_optional_f64(command, PARAM_DANGER_LEVEL_DELTA)
        .map_err(|error| format!("{PARAM_DANGER_LEVEL_DELTA} {error}"))?;

    if spirit_qi_delta.is_none() && danger_level_delta.is_none() {
        return Err("requires at least one supported delta param".to_string());
    }

    if let Some(delta) = spirit_qi_delta {
        if delta.abs() > 1.0 {
            return Err(format!("{PARAM_SPIRIT_QI_DELTA} exceeds ±1.0"));
        }
    }

    if let Some(delta) = danger_level_delta {
        if delta.abs() > 5.0 {
            return Err(format!("{PARAM_DANGER_LEVEL_DELTA} exceeds ±5.0"));
        }
    }

    Ok(())
}

fn validate_spawn_event_for_enqueue(command: &Command) -> Result<(), String> {
    validate_no_unsupported_params(
        command,
        &[
            PARAM_EVENT,
            PARAM_INTENSITY,
            PARAM_DURATION_TICKS,
            PARAM_TARGET_PLAYER,
        ],
    )?;

    let Some(event_name) = command
        .params
        .get(PARAM_EVENT)
        .and_then(serde_json::Value::as_str)
    else {
        return Err(format!("{PARAM_EVENT} must be a string"));
    };

    if event_name != THUNDER_TRIBULATION_EVENT {
        return Err(format!(
            "unsupported event {event_name} (M1 only supports {THUNDER_TRIBULATION_EVENT})"
        ));
    }

    if let Some(intensity) = parse_optional_f64(command, PARAM_INTENSITY)
        .map_err(|error| format!("{PARAM_INTENSITY} {error}"))?
    {
        if !(0.0..=1.0).contains(&intensity) {
            return Err(format!("{PARAM_INTENSITY} must be within [0, 1]"));
        }
    }

    if let Some(duration_ticks) = parse_optional_u64(command, PARAM_DURATION_TICKS)
        .map_err(|error| format!("{PARAM_DURATION_TICKS} {error}"))?
    {
        if duration_ticks == 0 || duration_ticks > MAX_EVENT_DURATION_TICKS {
            return Err(format!(
                "{PARAM_DURATION_TICKS} must be within [1, {MAX_EVENT_DURATION_TICKS}]"
            ));
        }
    }

    if let Some(value) = command.params.get(PARAM_TARGET_PLAYER) {
        if value.as_str().is_none() {
            return Err(format!("{PARAM_TARGET_PLAYER} must be a string"));
        }
    }

    Ok(())
}

fn validate_npc_behavior_for_enqueue(command: &Command) -> Result<(), String> {
    if command.target != NPC_BEHAVIOR_GLOBAL_TARGET {
        return Err(format!("target must be {NPC_BEHAVIOR_GLOBAL_TARGET} in M1"));
    }

    validate_no_unsupported_params(command, &[PARAM_FLEE_THRESHOLD])?;

    let Some(flee_threshold) = parse_optional_f64(command, PARAM_FLEE_THRESHOLD)
        .map_err(|error| format!("{PARAM_FLEE_THRESHOLD} {error}"))?
    else {
        return Err(format!("{PARAM_FLEE_THRESHOLD} is required"));
    };

    if !(0.0..=1.0).contains(&flee_threshold) {
        return Err(format!("{PARAM_FLEE_THRESHOLD} must be within [0, 1]"));
    }

    Ok(())
}

fn validate_no_unsupported_params(
    command: &Command,
    supported_keys: &[&str],
) -> Result<(), String> {
    if let Some(unsupported_key) = command
        .params
        .keys()
        .find(|key| !supported_keys.contains(&key.as_str()))
    {
        return Err(format!("unsupported param key {unsupported_key}"));
    }

    Ok(())
}

fn handle_modify_zone(queued: QueuedAgentCommand, zone_registry: &mut ZoneRegistry) {
    let Some(zone) = zone_registry.find_zone_mut(&queued.command.target) else {
        tracing::warn!(
            "[bong][network][command_executor] skip modify_zone for unknown zone target={} batch_id={}",
            queued.command.target,
            queued.batch_id
        );
        return;
    };

    let spirit_qi_delta = match parse_optional_f64(&queued.command, PARAM_SPIRIT_QI_DELTA) {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(
                "[bong][network][command_executor] skip modify_zone due to invalid {PARAM_SPIRIT_QI_DELTA} for target={} batch_id={}: {error}",
                queued.command.target,
                queued.batch_id
            );
            return;
        }
    };
    let danger_level_delta = match parse_optional_f64(&queued.command, PARAM_DANGER_LEVEL_DELTA) {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(
                "[bong][network][command_executor] skip modify_zone due to invalid {PARAM_DANGER_LEVEL_DELTA} for target={} batch_id={}: {error}",
                queued.command.target,
                queued.batch_id
            );
            return;
        }
    };

    if spirit_qi_delta.is_none() && danger_level_delta.is_none() {
        tracing::warn!(
            "[bong][network][command_executor] skip modify_zone for target={} batch_id={} because no supported params were provided",
            queued.command.target,
            queued.batch_id
        );
        return;
    }

    if let Some(delta) = spirit_qi_delta {
        if delta.abs() > 1.0 {
            tracing::warn!(
                "[bong][network][command_executor] skip modify_zone spirit_qi_delta={} as excessive for target={} batch_id={}",
                delta,
                queued.command.target,
                queued.batch_id
            );
            return;
        }
    }

    if let Some(delta) = danger_level_delta {
        if delta.abs() > 5.0 {
            tracing::warn!(
                "[bong][network][command_executor] skip modify_zone danger_level_delta={} as excessive for target={} batch_id={}",
                delta,
                queued.command.target,
                queued.batch_id
            );
            return;
        }
    }

    if let Some(delta) = spirit_qi_delta {
        zone.spirit_qi = (zone.spirit_qi + delta).clamp(0.0, 1.0);
    }

    if let Some(delta) = danger_level_delta {
        let next = (zone.danger_level as f64 + delta).clamp(0.0, 5.0);
        zone.danger_level = next.round() as u8;
    }

    tracing::info!(
        "[bong][network][command_executor] modify_zone applied for target={} batch_id={} spirit_qi={} danger_level={}",
        zone.name,
        queued.batch_id,
        zone.spirit_qi,
        zone.danger_level
    );
}

fn handle_spawn_event(
    queued: QueuedAgentCommand,
    zone_registry: &mut ZoneRegistry,
    active_events: &mut ActiveWorldEventsResource,
) {
    let Some(zone) = zone_registry.find_zone_mut(&queued.command.target) else {
        tracing::warn!(
            "[bong][network][command_executor] skip spawn_event for unknown zone target={} batch_id={}",
            queued.command.target,
            queued.batch_id
        );
        return;
    };

    let Some(event_name) = queued
        .command
        .params
        .get(PARAM_EVENT)
        .and_then(serde_json::Value::as_str)
    else {
        tracing::warn!(
            "[bong][network][command_executor] skip spawn_event for target={} batch_id={} because param.event is missing",
            queued.command.target,
            queued.batch_id
        );
        return;
    };

    if event_name != THUNDER_TRIBULATION_EVENT {
        tracing::warn!(
            "[bong][network][command_executor] skip spawn_event event={} for target={} batch_id={} (M1 only supports {})",
            event_name,
            queued.command.target,
            queued.batch_id,
            THUNDER_TRIBULATION_EVENT
        );
        return;
    }

    let intensity = match parse_optional_f64(&queued.command, PARAM_INTENSITY) {
        Ok(Some(value)) => value,
        Ok(None) => DEFAULT_EVENT_INTENSITY,
        Err(error) => {
            tracing::warn!(
                "[bong][network][command_executor] skip spawn_event due to invalid {PARAM_INTENSITY} for target={} batch_id={}: {error}",
                queued.command.target,
                queued.batch_id
            );
            return;
        }
    };
    if !(0.0..=1.0).contains(&intensity) {
        tracing::warn!(
            "[bong][network][command_executor] skip spawn_event intensity={} as excessive for target={} batch_id={}",
            intensity,
            queued.command.target,
            queued.batch_id
        );
        return;
    }

    let duration_ticks = match parse_optional_u64(&queued.command, PARAM_DURATION_TICKS) {
        Ok(Some(value)) => value,
        Ok(None) => DEFAULT_EVENT_DURATION_TICKS,
        Err(error) => {
            tracing::warn!(
                "[bong][network][command_executor] skip spawn_event due to invalid {PARAM_DURATION_TICKS} for target={} batch_id={}: {error}",
                queued.command.target,
                queued.batch_id
            );
            return;
        }
    };
    if duration_ticks == 0 || duration_ticks > MAX_EVENT_DURATION_TICKS {
        tracing::warn!(
            "[bong][network][command_executor] skip spawn_event duration_ticks={} as excessive for target={} batch_id={}",
            duration_ticks,
            queued.command.target,
            queued.batch_id
        );
        return;
    }

    let target_player = match queued.command.params.get(PARAM_TARGET_PLAYER) {
        Some(value) => match value.as_str() {
            Some(player) => Some(player.to_string()),
            None => {
                tracing::warn!(
                    "[bong][network][command_executor] skip spawn_event due to invalid {PARAM_TARGET_PLAYER} for target={} batch_id={}",
                    queued.command.target,
                    queued.batch_id
                );
                return;
            }
        },
        None => None,
    };

    if !zone
        .active_events
        .iter()
        .any(|name| name == THUNDER_TRIBULATION_EVENT)
    {
        zone.active_events
            .push(THUNDER_TRIBULATION_EVENT.to_string());
    }

    active_events
        .thunder_tribulations
        .push(ActiveThunderTribulationEvent {
            command_batch_id: queued.batch_id.clone(),
            source: queued.source.clone(),
            zone: queued.command.target.clone(),
            intensity,
            duration_ticks,
            target_player,
        });

    tracing::info!(
        "[bong][network][command_executor] spawn_event thunder_tribulation recorded for target={} batch_id={} duration_ticks={} intensity={}",
        queued.command.target,
        queued.batch_id,
        duration_ticks,
        intensity
    );
}

fn handle_npc_behavior(
    queued: QueuedAgentCommand,
    npc_behavior_config: &mut NpcBehaviorRuntimeConfig,
) {
    if queued.command.target != NPC_BEHAVIOR_GLOBAL_TARGET {
        tracing::warn!(
            "[bong][network][command_executor] skip npc_behavior for unsupported target={} batch_id={} (M1 only supports target={NPC_BEHAVIOR_GLOBAL_TARGET})",
            queued.command.target,
            queued.batch_id
        );
        return;
    }

    let flee_threshold = match parse_optional_f64(&queued.command, PARAM_FLEE_THRESHOLD) {
        Ok(Some(value)) => value,
        Ok(None) => {
            tracing::warn!(
                "[bong][network][command_executor] skip npc_behavior for target={} batch_id={} because param.flee_threshold is missing",
                queued.command.target,
                queued.batch_id
            );
            return;
        }
        Err(error) => {
            tracing::warn!(
                "[bong][network][command_executor] skip npc_behavior due to invalid {PARAM_FLEE_THRESHOLD} for target={} batch_id={}: {error}",
                queued.command.target,
                queued.batch_id
            );
            return;
        }
    };

    if !(0.0..=1.0).contains(&flee_threshold) {
        tracing::warn!(
            "[bong][network][command_executor] skip npc_behavior flee_threshold={} as excessive for target={} batch_id={}",
            flee_threshold,
            queued.command.target,
            queued.batch_id
        );
        return;
    }

    npc_behavior_config.flee_threshold = flee_threshold as f32;

    tracing::info!(
        "[bong][network][command_executor] npc_behavior updated runtime flee_threshold={} for batch_id={}",
        npc_behavior_config.flee_threshold,
        queued.batch_id
    );
}

fn parse_optional_f64(command: &Command, key: &str) -> Result<Option<f64>, &'static str> {
    let Some(value) = command.params.get(key) else {
        return Ok(None);
    };
    let Some(parsed) = value.as_f64() else {
        return Err("must be a finite number");
    };
    if !parsed.is_finite() {
        return Err("must be a finite number");
    }

    Ok(Some(parsed))
}

fn parse_optional_u64(command: &Command, key: &str) -> Result<Option<u64>, &'static str> {
    let Some(value) = command.params.get(key) else {
        return Ok(None);
    };
    let Some(parsed) = value.as_u64() else {
        return Err("must be a non-negative integer");
    };

    Ok(Some(parsed))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::schema::common::CommandType;
    use serde_json::json;

    fn command(
        command_type: CommandType,
        target: &str,
        params: HashMap<String, serde_json::Value>,
    ) -> Command {
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
            source: Some("tests".to_string()),
            commands,
        }
    }

    #[test]
    fn modify_zone_applies_and_clamps_values() {
        let mut executor = CommandExecutorResource::default();
        let mut zone_registry = ZoneRegistry::fallback();
        let mut active_events = ActiveWorldEventsResource::default();
        let mut npc_behavior_config = NpcBehaviorRuntimeConfig::default();

        let mut params = HashMap::new();
        params.insert(PARAM_SPIRIT_QI_DELTA.to_string(), json!(0.4));
        params.insert(PARAM_DANGER_LEVEL_DELTA.to_string(), json!(3));

        executor.enqueue_batch(batch(
            "cmd_modify_zone_clamp",
            vec![command(CommandType::ModifyZone, "spawn", params)],
        ));

        let executed = execute_agent_commands_with_limit(
            &mut executor,
            &mut zone_registry,
            &mut active_events,
            &mut npc_behavior_config,
            1,
        );

        assert_eq!(executed, 1);
        let zone = zone_registry
            .get_zone("spawn")
            .expect("spawn zone should exist in fallback registry");
        assert_eq!(zone.spirit_qi, 1.0);
        assert_eq!(zone.danger_level, 3);
    }

    #[test]
    fn invalid_zone_target_is_skipped() {
        let mut executor = CommandExecutorResource::default();
        let mut zone_registry = ZoneRegistry::fallback();
        let mut active_events = ActiveWorldEventsResource::default();
        let mut npc_behavior_config = NpcBehaviorRuntimeConfig::default();

        let mut params = HashMap::new();
        params.insert(PARAM_SPIRIT_QI_DELTA.to_string(), json!(0.2));

        executor.enqueue_batch(batch(
            "cmd_invalid_target",
            vec![command(CommandType::ModifyZone, "missing_zone", params)],
        ));

        let executed = execute_agent_commands_with_limit(
            &mut executor,
            &mut zone_registry,
            &mut active_events,
            &mut npc_behavior_config,
            1,
        );

        assert_eq!(executed, 1);
        let zone = zone_registry
            .get_zone("spawn")
            .expect("spawn zone should stay unchanged for invalid targets");
        assert_eq!(zone.spirit_qi, 0.9);
        assert_eq!(zone.danger_level, 0);
    }

    #[test]
    fn npc_behavior_updates_runtime_flee_threshold() {
        let mut executor = CommandExecutorResource::default();
        let mut zone_registry = ZoneRegistry::fallback();
        let mut active_events = ActiveWorldEventsResource::default();
        let mut npc_behavior_config = NpcBehaviorRuntimeConfig::default();

        let mut params = HashMap::new();
        params.insert(PARAM_FLEE_THRESHOLD.to_string(), json!(0.75));

        executor.enqueue_batch(batch(
            "cmd_npc_behavior",
            vec![command(CommandType::NpcBehavior, "global", params)],
        ));

        let executed = execute_agent_commands_with_limit(
            &mut executor,
            &mut zone_registry,
            &mut active_events,
            &mut npc_behavior_config,
            1,
        );

        assert_eq!(executed, 1);
        assert!((npc_behavior_config.flee_threshold - 0.75).abs() < f32::EPSILON);
    }

    #[test]
    fn thunder_tribulation_event_is_recorded() {
        let mut executor = CommandExecutorResource::default();
        let mut zone_registry = ZoneRegistry::fallback();
        let mut active_events = ActiveWorldEventsResource::default();
        let mut npc_behavior_config = NpcBehaviorRuntimeConfig::default();

        let mut params = HashMap::new();
        params.insert(PARAM_EVENT.to_string(), json!(THUNDER_TRIBULATION_EVENT));
        params.insert(PARAM_INTENSITY.to_string(), json!(0.7));
        params.insert(PARAM_DURATION_TICKS.to_string(), json!(600));
        params.insert(PARAM_TARGET_PLAYER.to_string(), json!("offline:Steve"));

        executor.enqueue_batch(batch(
            "cmd_spawn_event",
            vec![command(CommandType::SpawnEvent, "spawn", params)],
        ));

        let executed = execute_agent_commands_with_limit(
            &mut executor,
            &mut zone_registry,
            &mut active_events,
            &mut npc_behavior_config,
            1,
        );

        assert_eq!(executed, 1);
        assert_eq!(active_events.thunder_tribulations.len(), 1);
        assert_eq!(active_events.thunder_tribulations[0].zone, "spawn");

        let spawn_zone = zone_registry
            .get_zone("spawn")
            .expect("spawn zone should exist in fallback registry");
        assert!(spawn_zone
            .active_events
            .iter()
            .any(|name| name == THUNDER_TRIBULATION_EVENT));
    }

    #[test]
    fn execute_system_respects_fixed_command_cap() {
        let mut executor = CommandExecutorResource::default();
        let mut zone_registry = ZoneRegistry::fallback();
        let mut active_events = ActiveWorldEventsResource::default();
        let mut npc_behavior_config = NpcBehaviorRuntimeConfig::default();

        let mut params_a = HashMap::new();
        params_a.insert(PARAM_SPIRIT_QI_DELTA.to_string(), json!(-0.1));

        let mut params_b = HashMap::new();
        params_b.insert(PARAM_DANGER_LEVEL_DELTA.to_string(), json!(1));

        executor.enqueue_batch(batch(
            "cmd_budget",
            vec![
                command(CommandType::ModifyZone, "spawn", params_a),
                command(CommandType::ModifyZone, "spawn", params_b),
            ],
        ));

        let executed = execute_agent_commands_with_limit(
            &mut executor,
            &mut zone_registry,
            &mut active_events,
            &mut npc_behavior_config,
            1,
        );

        assert_eq!(executed, 1);
        assert_eq!(executor.pending_count(), 1);
    }

    #[test]
    fn validate_and_enqueue_skips_invalid_commands() {
        let mut executor = CommandExecutorResource::default();

        let mut valid_modify_zone = HashMap::new();
        valid_modify_zone.insert(PARAM_SPIRIT_QI_DELTA.to_string(), json!(-0.1));

        let mut invalid_spawn_event = HashMap::new();
        invalid_spawn_event.insert(PARAM_EVENT.to_string(), json!("beast_tide"));

        let mut invalid_npc_behavior = HashMap::new();
        invalid_npc_behavior.insert(PARAM_FLEE_THRESHOLD.to_string(), json!("fast"));

        let enqueued = validate_and_enqueue_agent_command_batch(
            &mut executor,
            batch(
                "cmd_validate_enqueue",
                vec![
                    command(CommandType::ModifyZone, "spawn", valid_modify_zone),
                    command(CommandType::SpawnEvent, "spawn", invalid_spawn_event),
                    command(CommandType::ModifyZone, "", HashMap::new()),
                    command(
                        CommandType::NpcBehavior,
                        NPC_BEHAVIOR_GLOBAL_TARGET,
                        invalid_npc_behavior,
                    ),
                ],
            ),
        );

        assert_eq!(enqueued, 1);
        assert_eq!(executor.pending_count(), 1);

        let queued = executor
            .dequeue_one()
            .expect("exactly one validated command should be enqueued");
        assert_eq!(queued.command.command_type, CommandType::ModifyZone);
        assert_eq!(queued.command.target, "spawn");
    }

    #[test]
    fn invalid_modify_zone_params_are_skipped_without_panicking() {
        let mut executor = CommandExecutorResource::default();
        let mut zone_registry = ZoneRegistry::fallback();
        let mut active_events = ActiveWorldEventsResource::default();
        let mut npc_behavior_config = NpcBehaviorRuntimeConfig::default();

        let mut params = HashMap::new();
        params.insert(PARAM_SPIRIT_QI_DELTA.to_string(), json!("too_high"));

        executor.enqueue_batch(batch(
            "cmd_invalid_params",
            vec![command(CommandType::ModifyZone, "spawn", params)],
        ));

        let executed = execute_agent_commands_with_limit(
            &mut executor,
            &mut zone_registry,
            &mut active_events,
            &mut npc_behavior_config,
            1,
        );

        assert_eq!(executed, 1);
        let zone = zone_registry
            .get_zone("spawn")
            .expect("spawn zone should exist in fallback registry");
        assert_eq!(zone.spirit_qi, 0.9);
        assert_eq!(zone.danger_level, 0);
    }
}
