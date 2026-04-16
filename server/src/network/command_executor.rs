use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;
use valence::prelude::{Commands, Despawned, Entity, Query, ResMut, Resource, With, Without};

use crate::npc::brain::{canonical_npc_id, NpcBehaviorConfig};
use crate::npc::faction::{
    FactionEventApplied, FactionEventCommand, FactionEventError, FactionEventKind, FactionId,
    FactionStore,
};
use crate::npc::lifecycle::{NpcArchetype, NpcRegistry};
use crate::npc::spawn::{
    spawn_commoner_npc_at, spawn_rogue_npc_at, spawn_zombie_npc_at, NpcMarker,
};
use crate::schema::agent_command::{AgentCommandV1, Command};
use crate::schema::common::{CommandType, GameEventType, MAX_COMMANDS_PER_TICK};
use crate::world::events::ActiveEventsResource;
use crate::world::zone::ZoneRegistry;

const ZONE_SPIRIT_QI_MIN: f64 = -1.0;
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

type LayerQuery<'w, 's> = Query<
    'w,
    's,
    Entity,
    (
        With<valence::prelude::ChunkLayer>,
        With<valence::prelude::EntityLayer>,
    ),
>;

type LiveNpcQuery<'w, 's> = Query<'w, 's, Entity, (With<NpcMarker>, Without<Despawned>)>;

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

#[allow(clippy::too_many_arguments)]
pub fn execute_agent_commands(
    mut commands: Commands,
    mut executor: ResMut<CommandExecutorResource>,
    mut zone_registry: Option<ResMut<ZoneRegistry>>,
    mut active_events: Option<ResMut<ActiveEventsResource>>,
    mut npc_registry: Option<ResMut<NpcRegistry>>,
    mut faction_store: Option<ResMut<FactionStore>>,
    mut npc_behavior: Option<ResMut<NpcBehaviorConfig>>,
    layers: LayerQuery<'_, '_>,
    npc_entities: LiveNpcQuery<'_, '_>,
) {
    let mut remaining_budget = MAX_COMMANDS_PER_TICK;
    let mut pending_despawn_targets = HashSet::new();

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
                &mut commands,
                &mut zone_registry,
                &mut active_events,
                &mut npc_registry,
                &mut faction_store,
                &mut npc_behavior,
                &layers,
                &npc_entities,
                &mut pending_despawn_targets,
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

#[allow(clippy::too_many_arguments)]
fn execute_single_command(
    command: &Command,
    batch_id: &str,
    source: Option<&str>,
    commands: &mut Commands,
    zone_registry: &mut Option<ResMut<ZoneRegistry>>,
    active_events: &mut Option<ResMut<ActiveEventsResource>>,
    npc_registry: &mut Option<ResMut<NpcRegistry>>,
    faction_store: &mut Option<ResMut<FactionStore>>,
    npc_behavior: &mut Option<ResMut<NpcBehaviorConfig>>,
    layers: &LayerQuery<'_, '_>,
    npc_entities: &LiveNpcQuery<'_, '_>,
    pending_despawn_targets: &mut HashSet<String>,
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
        CommandType::SpawnNpc => {
            execute_spawn_npc(command, commands, zone_registry, npc_registry, layers)
        }
        CommandType::DespawnNpc => {
            execute_despawn_npc(command, commands, npc_entities, pending_despawn_targets)
        }
        CommandType::FactionEvent => execute_faction_event(command, faction_store, active_events),
        CommandType::NpcBehavior => {
            execute_npc_behavior(command, npc_behavior, npc_entities, pending_despawn_targets)
        }
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
        CommandType::SpawnNpc => "spawn_npc",
        CommandType::DespawnNpc => "despawn_npc",
        CommandType::FactionEvent => "faction_event",
        CommandType::NpcBehavior => "npc_behavior",
        CommandType::SpawnEvent => "spawn_event",
    }
}

fn execute_faction_event(
    command: &Command,
    faction_store: &mut Option<ResMut<FactionStore>>,
    active_events: &mut Option<ResMut<ActiveEventsResource>>,
) -> &'static str {
    let Some(faction_store) = faction_store.as_deref_mut() else {
        tracing::warn!(
            "[bong][network] cannot execute faction_event for `{}` because FactionStore resource is missing",
            command.target
        );
        return "rejected_missing_faction_store";
    };

    let Some(event_command) = parse_faction_event_command(command) else {
        tracing::warn!(
            "[bong][network] faction_event target `{}` has invalid faction params",
            command.target
        );
        return "rejected_invalid_faction_event";
    };

    match faction_store.apply_event(event_command) {
        Ok(applied) => {
            if let Some(active_events) = active_events.as_deref_mut() {
                active_events.record_recent_event(build_faction_recent_event(applied));
            }
            "ok"
        }
        Err(error) => match error {
            FactionEventError::UnknownFaction(_) => "rejected_unknown_faction",
            FactionEventError::MissingSubjectId
            | FactionEventError::MissingMissionId
            | FactionEventError::MissingLoyaltyDelta => "rejected_invalid_faction_event",
        },
    }
}

fn execute_despawn_npc(
    command: &Command,
    commands: &mut Commands,
    npc_entities: &LiveNpcQuery<'_, '_>,
    pending_despawn_targets: &mut HashSet<String>,
) -> &'static str {
    let Some((entity, target_id)) = resolve_live_npc_target(
        command.target.as_str(),
        npc_entities,
        pending_despawn_targets,
    ) else {
        return reject_unknown_or_invalid_npc_target(command.target.as_str(), "despawn_npc");
    };

    commands.entity(entity).insert(Despawned);
    pending_despawn_targets.insert(target_id);
    "ok"
}

fn execute_spawn_npc(
    command: &Command,
    commands: &mut Commands,
    zone_registry: &mut Option<ResMut<ZoneRegistry>>,
    npc_registry: &mut Option<ResMut<NpcRegistry>>,
    layers: &LayerQuery<'_, '_>,
) -> &'static str {
    let Some(archetype) = command.params.get("archetype").and_then(Value::as_str) else {
        tracing::warn!(
            "[bong][network] spawn_npc target `{}` missing/invalid `archetype`",
            command.target
        );
        return "rejected_invalid_spawn_params";
    };

    let archetype = match archetype {
        "zombie" => NpcArchetype::Zombie,
        "commoner" => NpcArchetype::Commoner,
        "rogue" => NpcArchetype::Rogue,
        _ => {
            tracing::warn!(
                "[bong][network] spawn_npc target `{}` uses unsupported archetype `{}`",
                command.target,
                archetype
            );
            return "rejected_unsupported_archetype";
        }
    };

    let Some(zone_registry) = zone_registry.as_deref_mut() else {
        tracing::warn!(
            "[bong][network] cannot execute spawn_npc for `{}` because ZoneRegistry resource is missing",
            command.target
        );
        return "rejected_missing_zone_registry";
    };

    let Some(zone) = zone_registry
        .find_zone_by_name(command.target.as_str())
        .cloned()
    else {
        tracing::warn!(
            "[bong][network] spawn_npc target `{}` does not match any known zone",
            command.target
        );
        return "rejected_unknown_zone";
    };

    let Some(layer) = layers.iter().next() else {
        tracing::warn!(
            "[bong][network] spawn_npc target `{}` cannot resolve an entity layer",
            command.target
        );
        return "rejected_missing_entity_layer";
    };

    let Some(registry) = npc_registry.as_deref_mut() else {
        tracing::warn!(
            "[bong][network] cannot execute spawn_npc for `{}` because NpcRegistry resource is missing",
            command.target
        );
        return "rejected_missing_npc_registry";
    };

    if registry.reserve_spawn_batch(1) == 0 {
        tracing::info!(
            "[bong][network] spawn_npc target `{}` rejected because npc registry budget is exhausted",
            command.target
        );
        return "rejected_spawn_budget_exhausted";
    }

    let spawn_position = zone
        .patrol_anchors
        .first()
        .copied()
        .unwrap_or_else(|| zone.center());
    let patrol_target = zone.center();

    let initial_age_ticks = command
        .params
        .get("initial_age_ticks")
        .and_then(Value::as_f64)
        .unwrap_or(0.0)
        .max(0.0);

    match archetype {
        NpcArchetype::Zombie => {
            spawn_zombie_npc_at(
                commands,
                layer,
                zone.name.as_str(),
                spawn_position,
                patrol_target,
            );
            "ok"
        }
        NpcArchetype::Commoner => {
            spawn_commoner_npc_at(
                commands,
                layer,
                zone.name.as_str(),
                spawn_position,
                patrol_target,
                initial_age_ticks,
            );
            "ok"
        }
        NpcArchetype::Rogue => {
            spawn_rogue_npc_at(
                commands,
                layer,
                zone.name.as_str(),
                spawn_position,
                patrol_target,
                initial_age_ticks,
            );
            "ok"
        }
        _ => "rejected_unsupported_archetype",
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
    npc_entities: &LiveNpcQuery<'_, '_>,
    pending_despawn_targets: &HashSet<String>,
) -> &'static str {
    let Some(flee_threshold) = param_as_f64(&command.params, "flee_threshold") else {
        tracing::warn!(
            "[bong][network] npc_behavior target `{}` missing/invalid `flee_threshold`",
            command.target
        );
        return "rejected_invalid_flee_threshold";
    };

    let flee_threshold = flee_threshold.clamp(0.0, 1.0) as f32;

    let Some(target_id) = resolve_live_npc_canonical_id(
        command.target.as_str(),
        npc_entities,
        pending_despawn_targets,
    ) else {
        return reject_unknown_or_invalid_npc_target(command.target.as_str(), "npc_behavior");
    };

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
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs(),
        Err(err) => {
            tracing::warn!(
                "[bong][network] system clock before unix epoch; fallback timestamp=0s error={err}"
            );
            0
        }
    }
}

fn parse_npc_id(target: &str) -> Option<String> {
    let suffix = target.strip_prefix("npc_")?;
    let (index, generation) = suffix.split_once('v')?;
    let index = index.parse::<u32>().ok()?;
    let generation = generation.parse::<u32>().ok()?;
    let canonical_id = format!("npc_{index}v{generation}");

    (canonical_id == target).then_some(canonical_id)
}

fn parse_faction_event_command(command: &Command) -> Option<FactionEventCommand> {
    let kind = command.params.get("kind").and_then(Value::as_str)?;
    let faction_id = command.params.get("faction_id").and_then(Value::as_str)?;

    Some(FactionEventCommand {
        faction_id: FactionId::from_str_name(faction_id)?,
        kind: FactionEventKind::from_str_name(kind)?,
        subject_id: command
            .params
            .get("subject_id")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        mission_id: command
            .params
            .get("mission_id")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        loyalty_delta: command.params.get("loyalty_delta").and_then(Value::as_f64),
    })
}

fn build_faction_recent_event(
    applied: FactionEventApplied,
) -> crate::schema::world_state::GameEvent {
    let mut details = HashMap::new();
    details.insert(
        "faction_id".to_string(),
        Value::String(applied.faction_id.as_str().to_string()),
    );
    details.insert(
        "kind".to_string(),
        Value::String(applied.kind.as_str().to_string()),
    );
    details.insert(
        "loyalty_bias".to_string(),
        Value::from(applied.loyalty_bias),
    );
    details.insert(
        "mission_queue_size".to_string(),
        Value::from(applied.mission_queue_size as u64),
    );
    if let Some(leader_id) = applied.leader_id {
        details.insert("leader_id".to_string(), Value::String(leader_id));
    }

    crate::schema::world_state::GameEvent {
        event_type: GameEventType::EventTriggered,
        tick: 0,
        player: None,
        target: Some(format!("faction:{}", applied.faction_id.as_str())),
        zone: None,
        details: Some(details),
    }
}

fn resolve_live_npc_canonical_id(
    target: &str,
    npc_entities: &LiveNpcQuery<'_, '_>,
    pending_despawn_targets: &HashSet<String>,
) -> Option<String> {
    let target_id = parse_npc_id(target)?;
    if pending_despawn_targets.contains(&target_id) {
        return None;
    }
    npc_entities
        .iter()
        .find(|entity| canonical_npc_id(*entity) == target_id)
        .map(canonical_npc_id)
}

fn resolve_live_npc_target(
    target: &str,
    npc_entities: &LiveNpcQuery<'_, '_>,
    pending_despawn_targets: &HashSet<String>,
) -> Option<(Entity, String)> {
    let target_id = parse_npc_id(target)?;
    if pending_despawn_targets.contains(&target_id) {
        return None;
    }
    npc_entities
        .iter()
        .find(|entity| canonical_npc_id(*entity) == target_id)
        .map(|entity| (entity, target_id))
}

fn reject_unknown_or_invalid_npc_target(target: &str, command_type: &str) -> &'static str {
    if parse_npc_id(target).is_none() {
        tracing::warn!(
            "[bong][network] {command_type} target `{}` is not a canonical npc id (`npc_{{index}}v{{generation}}`)",
            target
        );
        "rejected_invalid_npc_target"
    } else {
        tracing::warn!(
            "[bong][network] {command_type} target `{}` does not map to a live NPC",
            target
        );
        "rejected_unknown_npc"
    }
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
    use valence::prelude::{App, DVec3, EntityKind, Position, Update};
    use valence::testing::ScenarioSingleClient;

    use crate::npc::brain::{canonical_npc_id, NpcBehaviorConfig, DEFAULT_FLEE_THRESHOLD};
    use crate::npc::faction::FactionStore;
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
        let scenario = ScenarioSingleClient::new();
        let mut app = scenario.app;
        app.insert_resource(CommandExecutorResource::default());
        app.insert_resource(ZoneRegistry::fallback());
        app.insert_resource(ActiveEventsResource::default());
        app.insert_resource(NpcBehaviorConfig::default());
        app.insert_resource(NpcRegistry::default());
        app.insert_resource(FactionStore::default());
        app.add_systems(Update, execute_agent_commands);
        app
    }

    #[test]
    fn faction_event_updates_store_and_records_recent_event() {
        let mut app = setup_executor_app();

        let mut params = HashMap::new();
        params.insert("kind".to_string(), json!("enqueue_mission"));
        params.insert("faction_id".to_string(), json!("neutral"));
        params.insert("mission_id".to_string(), json!("mission:hold_spawn_gate"));

        {
            let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
            let outcome = executor.enqueue_batch(batch(
                "cmd_faction_event_ok",
                vec![command(CommandType::FactionEvent, "neutral", params)],
            ));
            assert!(outcome.accepted);
            assert!(!outcome.dedupe_drop);
        }

        app.update();

        let store = app.world().resource::<FactionStore>();
        let neutral = store
            .iter()
            .find(|faction| faction.id == FactionId::Neutral)
            .expect("neutral faction should exist");
        assert_eq!(neutral.mission_queue.pending_count(), 1);
        assert_eq!(
            neutral.mission_queue.top_mission_id(),
            Some("mission:hold_spawn_gate")
        );

        let events = app.world().resource::<ActiveEventsResource>();
        let recent = events.recent_events_snapshot();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].event_type, GameEventType::EventTriggered);
        assert_eq!(recent[0].target.as_deref(), Some("faction:neutral"));
        assert_eq!(
            recent[0]
                .details
                .as_ref()
                .and_then(|details| details.get("kind"))
                .and_then(Value::as_str),
            Some("enqueue_mission")
        );
    }

    #[test]
    fn faction_event_rejects_invalid_or_unknown_faction_inputs() {
        let mut app = setup_executor_app();

        let mut commands = Vec::new();

        let mut invalid_kind = HashMap::new();
        invalid_kind.insert("kind".to_string(), json!("invent_new_faction_law"));
        invalid_kind.insert("faction_id".to_string(), json!("neutral"));
        commands.push(command(CommandType::FactionEvent, "neutral", invalid_kind));

        let mut unknown_faction = HashMap::new();
        unknown_faction.insert("kind".to_string(), json!("enqueue_mission"));
        unknown_faction.insert("faction_id".to_string(), json!("sky"));
        unknown_faction.insert("mission_id".to_string(), json!("mission:unknown"));
        commands.push(command(CommandType::FactionEvent, "sky", unknown_faction));

        let mut missing_payload = HashMap::new();
        missing_payload.insert("kind".to_string(), json!("adjust_loyalty_bias"));
        missing_payload.insert("faction_id".to_string(), json!("neutral"));
        commands.push(command(
            CommandType::FactionEvent,
            "neutral",
            missing_payload,
        ));

        {
            let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
            let outcome = executor.enqueue_batch(batch("cmd_faction_event_rejects", commands));
            assert!(outcome.accepted);
            assert!(!outcome.dedupe_drop);
        }

        app.update();

        let store = app.world().resource::<FactionStore>();
        let neutral = store
            .iter()
            .find(|faction| faction.id == FactionId::Neutral)
            .expect("neutral faction should exist");
        assert_eq!(neutral.mission_queue.pending_count(), 0);
        assert!((neutral.loyalty_bias - 0.5).abs() < 1e-9);

        let events = app.world().resource::<ActiveEventsResource>();
        assert!(events.recent_events_snapshot().is_empty());
    }

    #[test]
    fn despawn_npc_marks_live_target_as_despawned() {
        let mut app = setup_executor_app();

        let npc = app.world_mut().spawn(NpcMarker).id();
        let npc_id = canonical_npc_id(npc);

        {
            let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
            let outcome = executor.enqueue_batch(batch(
                "cmd_despawn_npc_ok",
                vec![command(
                    CommandType::DespawnNpc,
                    npc_id.as_str(),
                    HashMap::new(),
                )],
            ));
            assert!(outcome.accepted);
            assert!(!outcome.dedupe_drop);
        }

        app.update();

        let live_npcs = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, (With<NpcMarker>, Without<Despawned>)>();
            query.iter(world).collect::<Vec<_>>()
        };
        assert!(live_npcs.is_empty());
    }

    #[test]
    fn despawn_npc_rejects_invalid_or_unknown_targets() {
        let mut app = setup_executor_app();
        let live_npc = app.world_mut().spawn(NpcMarker).id();
        let live_npc_id = canonical_npc_id(live_npc);
        let missing_npc_id = format!("npc_{}v1", live_npc.index() + 99_999);

        {
            let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
            let outcome = executor.enqueue_batch(batch(
                "cmd_despawn_npc_rejects",
                vec![
                    command(CommandType::DespawnNpc, "npc_123", HashMap::new()),
                    command(
                        CommandType::DespawnNpc,
                        missing_npc_id.as_str(),
                        HashMap::new(),
                    ),
                ],
            ));
            assert!(outcome.accepted);
            assert!(!outcome.dedupe_drop);
        }

        app.update();

        let live_npcs_after_rejects = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, (With<NpcMarker>, Without<Despawned>)>();
            query.iter(world).collect::<Vec<_>>()
        };
        assert_eq!(live_npcs_after_rejects, vec![live_npc]);

        let mut behavior_params = HashMap::new();
        behavior_params.insert("flee_threshold".to_string(), json!(0.2));

        {
            let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
            let outcome = executor.enqueue_batch(batch(
                "cmd_despawn_npc_then_behavior",
                vec![
                    command(
                        CommandType::DespawnNpc,
                        live_npc_id.as_str(),
                        HashMap::new(),
                    ),
                    command(
                        CommandType::NpcBehavior,
                        live_npc_id.as_str(),
                        behavior_params,
                    ),
                ],
            ));
            assert!(outcome.accepted);
            assert!(!outcome.dedupe_drop);
        }

        app.update();

        let live_npcs_after_despawn = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, (With<NpcMarker>, Without<Despawned>)>();
            query.iter(world).collect::<Vec<_>>()
        };
        assert!(live_npcs_after_despawn.is_empty());

        let behavior = app.world().resource::<NpcBehaviorConfig>();
        assert_eq!(
            behavior.threshold_for_npc_id(live_npc_id.as_str()),
            DEFAULT_FLEE_THRESHOLD
        );
    }

    #[test]
    fn spawn_npc_creates_zombie_in_requested_zone() {
        let mut app = setup_executor_app();

        let mut params = HashMap::new();
        params.insert("archetype".to_string(), json!("zombie"));

        {
            let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
            let outcome = executor.enqueue_batch(batch(
                "cmd_spawn_npc_ok",
                vec![command(CommandType::SpawnNpc, "spawn", params)],
            ));
            assert!(outcome.accepted);
            assert!(!outcome.dedupe_drop);
        }

        app.update();

        let npcs = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
            query.iter(world).collect::<Vec<_>>()
        };
        assert_eq!(npcs.len(), 1);

        let npc = npcs[0];
        let npc_archetype = app
            .world()
            .get::<NpcArchetype>(npc)
            .expect("spawned npc should have archetype");
        assert_eq!(*npc_archetype, NpcArchetype::Zombie);

        let spawn_zone = app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name("spawn")
            .expect("spawn zone should exist")
            .clone();

        let patrol = app
            .world()
            .get::<crate::npc::patrol::NpcPatrol>(npc)
            .expect("spawned npc should have patrol state");
        assert_eq!(patrol.home_zone, "spawn");
        assert!(patrol.current_target.distance_squared(spawn_zone.center()) < 1e-9);

        let position = app
            .world()
            .get::<Position>(npc)
            .expect("spawned npc should have position");
        assert!(spawn_zone.contains(position.get()));
        let expected_spawn = spawn_zone
            .patrol_anchors
            .first()
            .copied()
            .unwrap_or_else(|| spawn_zone.center());
        assert!(position.get().distance_squared(expected_spawn) < 1e-9);

        let kind = app
            .world()
            .get::<EntityKind>(npc)
            .expect("spawned npc should have entity kind");
        assert_eq!(*kind, EntityKind::ZOMBIE);

        let registry = app.world().resource::<NpcRegistry>();
        assert_eq!(registry.live_npc_count, 1);
    }

    #[test]
    fn spawn_npc_creates_commoner_when_archetype_param_is_commoner() {
        let mut app = setup_executor_app();

        let mut params = HashMap::new();
        params.insert("archetype".to_string(), json!("commoner"));
        params.insert("initial_age_ticks".to_string(), json!(42.0));

        {
            let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
            let outcome = executor.enqueue_batch(batch(
                "cmd_spawn_npc_commoner",
                vec![command(CommandType::SpawnNpc, "spawn", params)],
            ));
            assert!(outcome.accepted);
        }

        app.update();

        let npcs = {
            let world = app.world_mut();
            let mut query =
                world.query_filtered::<(Entity, &NpcArchetype, &EntityKind), With<NpcMarker>>();
            query
                .iter(world)
                .map(|(e, a, k)| (e, *a, *k))
                .collect::<Vec<_>>()
        };
        assert_eq!(npcs.len(), 1);
        let (entity, archetype, kind) = npcs[0];
        assert_eq!(archetype, NpcArchetype::Commoner);
        assert_eq!(kind, EntityKind::VILLAGER);

        let lifespan = app
            .world()
            .get::<crate::npc::lifecycle::NpcLifespan>(entity)
            .expect("commoner should include lifespan");
        assert_eq!(lifespan.age_ticks, 42.0);

        let hunger = app
            .world()
            .get::<crate::npc::hunger::Hunger>(entity)
            .expect("commoner should include Hunger component");
        assert_eq!(hunger.value, 1.0);
    }

    #[test]
    fn spawn_npc_creates_rogue_when_archetype_param_is_rogue() {
        let mut app = setup_executor_app();

        let mut params = HashMap::new();
        params.insert("archetype".to_string(), json!("rogue"));
        params.insert("initial_age_ticks".to_string(), json!(5000.0));

        {
            let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
            let outcome = executor.enqueue_batch(batch(
                "cmd_spawn_npc_rogue",
                vec![command(CommandType::SpawnNpc, "spawn", params)],
            ));
            assert!(outcome.accepted);
        }

        app.update();

        let npcs = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<(Entity, &NpcArchetype), With<NpcMarker>>();
            query.iter(world).map(|(e, a)| (e, *a)).collect::<Vec<_>>()
        };
        assert_eq!(npcs.len(), 1);
        assert_eq!(npcs[0].1, NpcArchetype::Rogue);

        let lifespan = app
            .world()
            .get::<crate::npc::lifecycle::NpcLifespan>(npcs[0].0)
            .unwrap();
        assert_eq!(lifespan.age_ticks, 5000.0);
    }

    #[test]
    fn spawn_npc_rejects_unknown_zone_unsupported_archetype_and_exhausted_budget() {
        let mut app = setup_executor_app();

        let mut commands = Vec::new();

        let mut bad_zone = HashMap::new();
        bad_zone.insert("archetype".to_string(), json!("zombie"));
        commands.push(command(CommandType::SpawnNpc, "missing_zone", bad_zone));

        let mut bad_archetype = HashMap::new();
        bad_archetype.insert("archetype".to_string(), json!("beast"));
        commands.push(command(CommandType::SpawnNpc, "spawn", bad_archetype));

        {
            let mut registry = app.world_mut().resource_mut::<NpcRegistry>();
            registry.live_npc_count = registry.max_npc_count;
            registry.spawn_paused = true;
        }

        let mut exhausted_budget = HashMap::new();
        exhausted_budget.insert("archetype".to_string(), json!("zombie"));
        commands.push(command(CommandType::SpawnNpc, "spawn", exhausted_budget));

        {
            let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
            let outcome = executor.enqueue_batch(batch("cmd_spawn_npc_rejects", commands));
            assert!(outcome.accepted);
            assert!(!outcome.dedupe_drop);
        }

        app.update();

        let npcs = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
            query.iter(world).collect::<Vec<_>>()
        };
        assert!(npcs.is_empty());
    }

    #[test]
    fn spawn_npc_rejects_missing_archetype_param() {
        let mut app = setup_executor_app();

        {
            let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
            let outcome = executor.enqueue_batch(batch(
                "cmd_spawn_npc_missing_archetype",
                vec![command(CommandType::SpawnNpc, "spawn", HashMap::new())],
            ));
            assert!(outcome.accepted);
            assert!(!outcome.dedupe_drop);
        }

        app.update();

        let npcs = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
            query.iter(world).collect::<Vec<_>>()
        };
        assert!(npcs.is_empty());
    }

    #[test]
    fn clamps_modify_zone_to_negative_and_positive_bounds() {
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

        assert_eq!(spawn_zone.spirit_qi, -1.0);
        assert_eq!(spawn_zone.danger_level, 5);

        let mut params = HashMap::new();
        params.insert("spirit_qi_delta".to_string(), json!(3.0));

        {
            let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
            let outcome = executor.enqueue_batch(batch(
                "cmd_modify_zone_cap_upper",
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

        assert_eq!(spawn_zone.spirit_qi, 1.0);
    }

    #[test]
    fn modify_zone_preserves_negative_one_without_clamping_back_to_zero() {
        let mut app = setup_executor_app();

        let mut lower_to_negative_bound_params = HashMap::new();
        lower_to_negative_bound_params.insert("spirit_qi_delta".to_string(), json!(-10.0));

        {
            let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
            let outcome = executor.enqueue_batch(batch(
                "cmd_modify_zone_reach_negative_one",
                vec![command(
                    CommandType::ModifyZone,
                    "spawn",
                    lower_to_negative_bound_params,
                )],
            ));
            assert!(outcome.accepted);
            assert!(!outcome.dedupe_drop);
        }

        app.update();

        let zone_registry = app.world().resource::<ZoneRegistry>();
        let spawn_zone = zone_registry
            .find_zone(DVec3::new(8.0, 66.0, 8.0))
            .expect("spawn zone should still exist");

        assert_eq!(spawn_zone.spirit_qi, -1.0);

        let mut still_negative_params = HashMap::new();
        still_negative_params.insert("spirit_qi_delta".to_string(), json!(-0.25));

        {
            let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
            let outcome = executor.enqueue_batch(batch(
                "cmd_modify_zone_stay_negative",
                vec![command(
                    CommandType::ModifyZone,
                    "spawn",
                    still_negative_params,
                )],
            ));
            assert!(outcome.accepted);
            assert!(!outcome.dedupe_drop);
        }

        app.update();

        let zone_registry = app.world().resource::<ZoneRegistry>();
        let spawn_zone = zone_registry
            .find_zone(DVec3::new(8.0, 66.0, 8.0))
            .expect("spawn zone should still exist");

        assert_eq!(spawn_zone.spirit_qi, -1.0);
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

        let mut bad_spawn_npc_params = HashMap::new();
        bad_spawn_npc_params.insert("archetype".to_string(), json!("zombie"));
        commands.push(command(
            CommandType::SpawnNpc,
            "unknown_zone",
            bad_spawn_npc_params,
        ));

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

        commands.push(command(
            CommandType::DespawnNpc,
            "npc_999999v1",
            HashMap::new(),
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

        let npc_count = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
            query.iter(world).count()
        };
        assert_eq!(npc_count, 0);
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
