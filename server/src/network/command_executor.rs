use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;
use valence::prelude::bevy_ecs::system::SystemParam;
use valence::prelude::{
    bevy_ecs, BlockPos, Commands, Despawned, Entity, EventWriter, Query, Res, ResMut, Resource,
    With, Without,
};

use crate::cultivation::components::Realm;
use crate::cultivation::tick::CultivationClock;
use crate::npc::brain::{canonical_npc_id, NpcBehaviorConfig};
use crate::npc::faction::{
    FactionEventApplied, FactionEventCommand, FactionEventError, FactionEventKind,
    FactionEventNotice, FactionId, FactionRank, FactionStore,
};
use crate::npc::lifecycle::{NpcArchetype, NpcRegistry, NpcSpawnNotice, NpcSpawnSource};
use crate::npc::spawn::{
    snap_spawn_y_to_surface, spawn_beast_npc_at, spawn_commoner_npc_at, spawn_disciple_npc_at,
    spawn_notice, spawn_relic_guard_npc_at, spawn_rogue_npc_at, spawn_zombie_npc_at, NpcMarker,
    NpcSkinSpawnContext,
};
use crate::npc::territory::Territory;
use crate::qi_physics::ledger::QiTransfer;
use crate::schema::agent_command::{AgentCommandV1, Command};
use crate::schema::common::{CommandType, GameEventType, MAX_COMMANDS_PER_TICK};
use crate::schema::pseudo_vein::PseudoVeinSeasonV1;
use crate::skin::{NpcSkinFallbackPolicy, SkinPool};
use crate::world::calamity::{CalamityArsenal, TiandaoPower};
use crate::world::events::ActiveEventsResource;
use crate::world::heartbeat::{apply_heartbeat_override_command, WorldHeartbeat};
use crate::world::karma::{KarmaWeightStore, QiDensityHeatmap};
use crate::world::pseudo_vein_runtime::{inject_zone_for_pseudo_vein, PseudoVeinRuntime};
use crate::world::season::{query_season, Season};
use crate::world::terrain::{TerrainProvider, TerrainProviders};
use crate::world::zone::ZoneRegistry;

const ZONE_SPIRIT_QI_MIN: f64 = -1.0;
const ZONE_SPIRIT_QI_MAX: f64 = 1.0;
const ZONE_DANGER_LEVEL_MIN: i64 = 0;
const ZONE_DANGER_LEVEL_MAX: i64 = 5;
const COMMAND_BATCH_DEDUPE_WINDOW_SECS: u64 = 30;
const COMMAND_BATCH_DEDUPE_CAPACITY: usize = 256;
const EVENT_PSEUDO_VEIN: &str = "pseudo_vein";

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

#[derive(SystemParam)]
pub(crate) struct CommandExecutorWorldResources<'w> {
    zone_registry: Option<ResMut<'w, ZoneRegistry>>,
    active_events: Option<ResMut<'w, ActiveEventsResource>>,
    tiandao_power: Option<ResMut<'w, TiandaoPower>>,
    calamity_arsenal: Option<Res<'w, CalamityArsenal>>,
}

struct SpawnEventCommandResources<'a> {
    zone_registry: Option<&'a mut ZoneRegistry>,
    active_events: Option<&'a mut ActiveEventsResource>,
    tiandao_power: Option<&'a mut TiandaoPower>,
    calamity_arsenal: Option<&'a CalamityArsenal>,
    karma_weights: Option<&'a KarmaWeightStore>,
    qi_heatmap: Option<&'a QiDensityHeatmap>,
}

/// 合并 agent command 执行上下文，避免 Bevy 0.14 顶层 SystemParam 16 上限。
#[derive(SystemParam)]
pub struct CommandExecutionParams<'w> {
    heartbeat: Option<ResMut<'w, WorldHeartbeat>>,
    karma_weights: Option<Res<'w, KarmaWeightStore>>,
    qi_heatmap: Option<Res<'w, QiDensityHeatmap>>,
    clock: Option<Res<'w, CultivationClock>>,
    terrain_providers: Option<Res<'w, TerrainProviders>>,
}

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
    mut world_resources: CommandExecutorWorldResources,
    mut npc_registry: Option<ResMut<NpcRegistry>>,
    mut skin_pool: Option<ResMut<SkinPool>>,
    mut faction_store: Option<ResMut<FactionStore>>,
    mut npc_behavior: Option<ResMut<NpcBehaviorConfig>>,
    mut params: CommandExecutionParams,
    mut npc_spawn_notices: EventWriter<NpcSpawnNotice>,
    mut faction_notices: EventWriter<FactionEventNotice>,
    mut qi_transfers: EventWriter<QiTransfer>,
    layers: LayerQuery<'_, '_>,
    npc_entities: LiveNpcQuery<'_, '_>,
    pseudo_vein_runtimes: Query<&PseudoVeinRuntime>,
) {
    let mut remaining_budget = MAX_COMMANDS_PER_TICK;
    let mut pending_despawn_targets = HashSet::new();
    let mut pending_pseudo_vein_zones = HashSet::new();
    let terrain = params.terrain_providers.as_deref().map(|p| &p.overworld);

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
                &mut world_resources.zone_registry,
                &mut world_resources.active_events,
                &mut world_resources.tiandao_power,
                world_resources.calamity_arsenal.as_deref(),
                &mut npc_registry,
                &mut skin_pool,
                &mut faction_store,
                &mut npc_behavior,
                &mut params.heartbeat,
                params.karma_weights.as_deref(),
                params.qi_heatmap.as_deref(),
                params.clock.as_deref().map(|clock| clock.tick),
                terrain,
                &mut npc_spawn_notices,
                &mut faction_notices,
                &mut qi_transfers,
                &layers,
                &npc_entities,
                &pseudo_vein_runtimes,
                &mut pending_pseudo_vein_zones,
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
    tiandao_power: &mut Option<ResMut<TiandaoPower>>,
    calamity_arsenal: Option<&CalamityArsenal>,
    npc_registry: &mut Option<ResMut<NpcRegistry>>,
    skin_pool: &mut Option<ResMut<SkinPool>>,
    faction_store: &mut Option<ResMut<FactionStore>>,
    npc_behavior: &mut Option<ResMut<NpcBehaviorConfig>>,
    heartbeat: &mut Option<ResMut<WorldHeartbeat>>,
    karma_weights: Option<&KarmaWeightStore>,
    qi_heatmap: Option<&QiDensityHeatmap>,
    tick: Option<u64>,
    terrain: Option<&TerrainProvider>,
    npc_spawn_notices: &mut EventWriter<NpcSpawnNotice>,
    faction_notices: &mut EventWriter<FactionEventNotice>,
    qi_transfers: &mut EventWriter<QiTransfer>,
    layers: &LayerQuery<'_, '_>,
    npc_entities: &LiveNpcQuery<'_, '_>,
    pseudo_vein_runtimes: &Query<&PseudoVeinRuntime>,
    pending_pseudo_vein_zones: &mut HashSet<String>,
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
        CommandType::SpawnNpc => execute_spawn_npc(
            command,
            commands,
            zone_registry,
            npc_registry,
            skin_pool,
            npc_spawn_notices,
            layers,
            terrain,
        ),
        CommandType::DespawnNpc => {
            execute_despawn_npc(command, commands, npc_entities, pending_despawn_targets)
        }
        CommandType::FactionEvent => {
            execute_faction_event(command, faction_store, active_events, faction_notices)
        }
        CommandType::NpcBehavior => {
            execute_npc_behavior(command, npc_behavior, npc_entities, pending_despawn_targets)
        }
        CommandType::HeartbeatOverride => execute_heartbeat_override(command, heartbeat, tick),
        CommandType::SpawnEvent => execute_spawn_event(
            command,
            commands,
            SpawnEventCommandResources {
                zone_registry: zone_registry.as_deref_mut(),
                active_events: active_events.as_deref_mut(),
                tiandao_power: tiandao_power.as_deref_mut(),
                calamity_arsenal,
                karma_weights,
                qi_heatmap,
            },
            tick,
            pseudo_vein_runtimes,
            qi_transfers,
            pending_pseudo_vein_zones,
        ),
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
        CommandType::HeartbeatOverride => "heartbeat_override",
        CommandType::SpawnEvent => "spawn_event",
    }
}

fn execute_heartbeat_override(
    command: &Command,
    heartbeat: &mut Option<ResMut<WorldHeartbeat>>,
    tick: Option<u64>,
) -> &'static str {
    let current_tick = tick.unwrap_or_else(|| {
        heartbeat
            .as_deref()
            .map(|heartbeat| {
                heartbeat
                    .last_eval_tick
                    .saturating_add(heartbeat.eval_interval_ticks)
            })
            .unwrap_or_default()
    });
    match apply_heartbeat_override_command(
        heartbeat.as_deref_mut().map(|heartbeat| &mut *heartbeat),
        command,
        current_tick,
    ) {
        Ok(()) => "ok",
        Err(error) => error.result_label(),
    }
}

fn execute_faction_event(
    command: &Command,
    faction_store: &mut Option<ResMut<FactionStore>>,
    active_events: &mut Option<ResMut<ActiveEventsResource>>,
    faction_notices: &mut EventWriter<FactionEventNotice>,
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
                active_events.record_recent_event(build_faction_recent_event(applied.clone()));
            }
            faction_notices.send(FactionEventNotice { applied });
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

#[allow(clippy::too_many_arguments)]
fn execute_spawn_npc(
    command: &Command,
    commands: &mut Commands,
    zone_registry: &mut Option<ResMut<ZoneRegistry>>,
    npc_registry: &mut Option<ResMut<NpcRegistry>>,
    skin_pool: &mut Option<ResMut<SkinPool>>,
    npc_spawn_notices: &mut EventWriter<NpcSpawnNotice>,
    layers: &LayerQuery<'_, '_>,
    terrain: Option<&TerrainProvider>,
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
        "beast" => NpcArchetype::Beast,
        "disciple" => NpcArchetype::Disciple,
        "guardian_relic" => NpcArchetype::GuardianRelic,
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

    let Some(requested_count) = parse_spawn_count(&command.params) else {
        tracing::warn!(
            "[bong][network] spawn_npc target `{}` has invalid `count` (expected integer in [1, {MAX_COMMANDS_PER_TICK}])",
            command.target
        );
        return "rejected_invalid_spawn_count";
    };

    let reserved_count = registry.reserve_zone_batch(zone.name.as_str(), requested_count);
    if reserved_count == 0 {
        tracing::info!(
            "[bong][network] spawn_npc target `{}` rejected because npc registry budget is exhausted",
            command.target
        );
        return "rejected_spawn_budget_exhausted";
    }
    if reserved_count < requested_count {
        tracing::info!(
            "[bong][network] spawn_npc target `{}` clamped by npc registry: requested={} reserved={}",
            command.target,
            requested_count,
            reserved_count
        );
    }

    let raw_spawn_position = zone
        .patrol_anchors
        .first()
        .copied()
        .unwrap_or_else(|| zone.center());
    // Snap to actual terrain surface — zone bounds and patrol anchors are
    // hand-authored and can drift from regenerated terrain. Without this,
    // agent-issued spawns can drop NPCs into the air or below ground.
    let spawn_position = snap_spawn_y_to_surface(raw_spawn_position, terrain);
    let patrol_target = zone.center();

    let initial_age_ticks = command
        .params
        .get("initial_age_ticks")
        .and_then(Value::as_f64)
        .unwrap_or(0.0)
        .max(0.0);

    for _ in 0..reserved_count {
        match archetype {
            NpcArchetype::Zombie => {
                let entity = spawn_zombie_npc_at(
                    commands,
                    layer,
                    zone.name.as_str(),
                    spawn_position,
                    patrol_target,
                );
                npc_spawn_notices.send(spawn_notice(
                    entity,
                    archetype,
                    NpcSpawnSource::AgentCommand,
                    zone.name.as_str(),
                    spawn_position,
                    0.0,
                ));
            }
            NpcArchetype::Commoner => {
                let entity = spawn_commoner_npc_at(
                    commands,
                    NpcSkinSpawnContext::new(
                        skin_pool.as_deref_mut(),
                        NpcSkinFallbackPolicy::AllowFallback,
                    ),
                    layer,
                    zone.name.as_str(),
                    spawn_position,
                    patrol_target,
                    Realm::Awaken,
                    initial_age_ticks,
                );
                npc_spawn_notices.send(spawn_notice(
                    entity,
                    archetype,
                    NpcSpawnSource::AgentCommand,
                    zone.name.as_str(),
                    spawn_position,
                    initial_age_ticks,
                ));
            }
            NpcArchetype::Rogue => {
                let entity = spawn_rogue_npc_at(
                    commands,
                    NpcSkinSpawnContext::new(
                        skin_pool.as_deref_mut(),
                        NpcSkinFallbackPolicy::AllowFallback,
                    ),
                    layer,
                    zone.name.as_str(),
                    spawn_position,
                    patrol_target,
                    Realm::Awaken,
                    initial_age_ticks,
                );
                npc_spawn_notices.send(spawn_notice(
                    entity,
                    archetype,
                    NpcSpawnSource::AgentCommand,
                    zone.name.as_str(),
                    spawn_position,
                    initial_age_ticks,
                ));
            }
            NpcArchetype::Beast => {
                let radius = command
                    .params
                    .get("territory_radius")
                    .and_then(Value::as_f64)
                    .unwrap_or(24.0)
                    .max(1.0);
                let entity = spawn_beast_npc_at(
                    commands,
                    layer,
                    zone.name.as_str(),
                    spawn_position,
                    Territory::new(spawn_position, radius),
                    initial_age_ticks,
                );
                npc_spawn_notices.send(spawn_notice(
                    entity,
                    archetype,
                    NpcSpawnSource::AgentCommand,
                    zone.name.as_str(),
                    spawn_position,
                    initial_age_ticks,
                ));
            }
            NpcArchetype::Disciple => {
                let faction_id = command
                    .params
                    .get("faction_id")
                    .and_then(Value::as_str)
                    .and_then(FactionId::from_str_name)
                    .unwrap_or(FactionId::Neutral);
                let entity = spawn_disciple_npc_at(
                    commands,
                    layer,
                    zone.name.as_str(),
                    spawn_position,
                    patrol_target,
                    faction_id,
                    FactionRank::Disciple,
                    Realm::Awaken,
                    command
                        .params
                        .get("master_id")
                        .and_then(Value::as_str)
                        .map(ToString::to_string),
                    initial_age_ticks,
                );
                npc_spawn_notices.send(spawn_notice(
                    entity,
                    archetype,
                    NpcSpawnSource::AgentCommand,
                    zone.name.as_str(),
                    spawn_position,
                    initial_age_ticks,
                ));
            }
            NpcArchetype::GuardianRelic => {
                let radius = command
                    .params
                    .get("alarm_radius")
                    .and_then(Value::as_f64)
                    .unwrap_or(crate::npc::relic::GUARDIAN_ALARM_RADIUS_DEFAULT)
                    .max(1.0);
                let relic_id = command
                    .params
                    .get("relic_id")
                    .and_then(Value::as_str)
                    .unwrap_or("agent_relic");
                let trial_template_id = command
                    .params
                    .get("trial_template_id")
                    .and_then(Value::as_str)
                    .unwrap_or("agent_trial");
                let entity = spawn_relic_guard_npc_at(
                    commands,
                    layer,
                    zone.name.as_str(),
                    spawn_position,
                    radius,
                    relic_id,
                    trial_template_id,
                );
                npc_spawn_notices.send(spawn_notice(
                    entity,
                    archetype,
                    NpcSpawnSource::AgentCommand,
                    zone.name.as_str(),
                    spawn_position,
                    0.0,
                ));
            }
            _ => unreachable!("archetype match above rejects unsupported variants"),
        }
    }

    "ok"
}

fn execute_spawn_event(
    command: &Command,
    commands: &mut Commands,
    resources: SpawnEventCommandResources<'_>,
    tick: Option<u64>,
    pseudo_vein_runtimes: &Query<&PseudoVeinRuntime>,
    qi_transfers: &mut EventWriter<QiTransfer>,
    pending_pseudo_vein_zones: &mut HashSet<String>,
) -> &'static str {
    let SpawnEventCommandResources {
        zone_registry,
        active_events,
        tiandao_power,
        calamity_arsenal,
        karma_weights,
        qi_heatmap,
    } = resources;

    if event_name(command) == Some(EVENT_PSEUDO_VEIN) {
        return execute_spawn_pseudo_vein(
            command,
            commands,
            zone_registry,
            tick,
            pseudo_vein_runtimes,
            qi_transfers,
            pending_pseudo_vein_zones,
        );
    }

    let Some(active_events) = active_events else {
        tracing::warn!(
            "[bong][network] cannot enqueue spawn_event for `{}` because ActiveEventsResource is missing",
            command.target
        );
        return "rejected_missing_active_events";
    };

    let tick = tick.unwrap_or_default();
    let season = query_season("", tick).season;
    if active_events.enqueue_from_spawn_command_with_karma_power_and_season_at_tick(
        command,
        zone_registry,
        karma_weights,
        qi_heatmap,
        season,
        tick,
        tiandao_power,
        calamity_arsenal,
    ) {
        "ok"
    } else {
        "rejected_spawn_event"
    }
}

fn execute_spawn_pseudo_vein(
    command: &Command,
    commands: &mut Commands,
    zone_registry: Option<&mut ZoneRegistry>,
    tick: Option<u64>,
    pseudo_vein_runtimes: &Query<&PseudoVeinRuntime>,
    qi_transfers: &mut EventWriter<QiTransfer>,
    pending_pseudo_vein_zones: &mut HashSet<String>,
) -> &'static str {
    let Some(zone_registry) = zone_registry else {
        tracing::warn!(
            "[bong][network] cannot spawn pseudo_vein for `{}` because ZoneRegistry is missing",
            command.target
        );
        return "rejected_missing_zone_registry";
    };
    let Some(zone) = zone_registry.find_zone_mut(command.target.as_str()) else {
        tracing::warn!(
            "[bong][network] pseudo_vein target zone `{}` was not found",
            command.target
        );
        return "rejected_unknown_zone";
    };
    if pending_pseudo_vein_zones.contains(zone.name.as_str())
        || pseudo_vein_runtimes
            .iter()
            .any(|runtime| runtime.zone_id == zone.name)
    {
        tracing::info!(
            "[bong][network] pseudo_vein target zone `{}` already has active runtime",
            zone.name
        );
        return "rejected_duplicate_pseudo_vein";
    }

    let now = tick.unwrap_or_default();
    let center = zone.center();
    let injected_qi = if let Some(transfer) = inject_zone_for_pseudo_vein(zone) {
        let amount = transfer.amount;
        qi_transfers.send(transfer);
        amount
    } else {
        0.0
    };
    pending_pseudo_vein_zones.insert(zone.name.clone());
    let mut runtime = PseudoVeinRuntime::new(
        zone.name.clone(),
        BlockPos::new(
            center.x.round() as i32,
            center.y.round() as i32,
            center.z.round() as i32,
        ),
        now,
        pseudo_vein_season_from_world(query_season(command.target.as_str(), now).season),
    );
    runtime.injected_qi = injected_qi;
    commands.spawn(runtime);
    "ok"
}

fn event_name(command: &Command) -> Option<&str> {
    command
        .params
        .get("event")
        .and_then(serde_json::Value::as_str)
}

fn pseudo_vein_season_from_world(season: Season) -> PseudoVeinSeasonV1 {
    match season {
        Season::Summer => PseudoVeinSeasonV1::Summer,
        Season::SummerToWinter => PseudoVeinSeasonV1::SummerToWinter,
        Season::Winter => PseudoVeinSeasonV1::Winter,
        Season::WinterToSummer => PseudoVeinSeasonV1::WinterToSummer,
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

fn parse_spawn_count(params: &HashMap<String, Value>) -> Option<usize> {
    let Some(value) = params.get("count") else {
        return Some(1);
    };
    let count = value_to_i64(Some(value))?;
    if count < 1 || count > MAX_COMMANDS_PER_TICK as i64 {
        return None;
    }
    Some(count as usize)
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
    use valence::prelude::{App, BlockPos, DVec3, EntityKind, Events, Position, Update};
    use valence::testing::ScenarioSingleClient;

    use crate::npc::brain::{canonical_npc_id, NpcBehaviorConfig, DEFAULT_FLEE_THRESHOLD};
    use crate::npc::faction::FactionStore;
    use crate::qi_physics::ledger::{QiAccountId, QiTransferReason};
    use crate::schema::agent_command::Command;
    use crate::world::events::{
        ActiveEventsResource, EVENT_KARMA_BACKLASH, EVENT_THUNDER_TRIBULATION,
    };
    use crate::world::heartbeat::{HeartbeatEventKind, HeartbeatOverrideError};
    use crate::world::karma::{
        TARGETED_CALAMITY_BASE_PROBABILITY, TARGETED_CALAMITY_MAX_PROBABILITY,
    };
    use crate::world::pseudo_vein_runtime::{PseudoVeinPhase, PseudoVeinRuntime};

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
        app.insert_resource(KarmaWeightStore::default());
        app.insert_resource(QiDensityHeatmap::default());
        app.add_event::<NpcSpawnNotice>();
        app.add_event::<FactionEventNotice>();
        app.add_event::<QiTransfer>();
        app.add_systems(Update, execute_agent_commands);
        app
    }

    #[test]
    fn spawn_event_pseudo_vein_creates_runtime_component() {
        let mut app = setup_executor_app();
        let mut params = HashMap::new();
        params.insert("event".to_string(), json!("pseudo_vein"));
        params.insert("intensity".to_string(), json!(0.7));

        {
            let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
            let outcome = executor.enqueue_batch(batch(
                "cmd_pseudo_vein_runtime",
                vec![command(CommandType::SpawnEvent, "spawn", params)],
            ));
            assert!(outcome.accepted);
            assert!(!outcome.dedupe_drop);
        }

        app.update();

        let mut query = app.world_mut().query::<&PseudoVeinRuntime>();
        let runtimes = query.iter(app.world()).collect::<Vec<_>>();
        assert_eq!(runtimes.len(), 1);
        assert_eq!(runtimes[0].zone_id, "spawn");
        assert_eq!(runtimes[0].phase, PseudoVeinPhase::Rising);
    }

    #[test]
    fn spawn_event_pseudo_vein_rejects_unknown_zone() {
        let mut app = setup_executor_app();
        let mut params = HashMap::new();
        params.insert("event".to_string(), json!("pseudo_vein"));
        params.insert("intensity".to_string(), json!(0.7));

        {
            let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
            let outcome = executor.enqueue_batch(batch(
                "cmd_pseudo_vein_unknown_zone",
                vec![command(CommandType::SpawnEvent, "missing_zone", params)],
            ));
            assert!(outcome.accepted);
            assert!(!outcome.dedupe_drop);
        }

        app.update();

        let mut query = app.world_mut().query::<&PseudoVeinRuntime>();
        let runtimes = query.iter(app.world()).collect::<Vec<_>>();
        assert!(
            runtimes.is_empty(),
            "pseudo_vein spawn_event should not create runtime for unknown zone"
        );
    }

    #[test]
    fn spawn_event_pseudo_vein_injects_zone_qi_transfer() {
        let mut app = setup_executor_app();
        app.world_mut()
            .resource_mut::<ZoneRegistry>()
            .find_zone_mut("spawn")
            .expect("fallback registry should contain spawn")
            .spirit_qi = 0.1;
        let mut params = HashMap::new();
        params.insert("event".to_string(), json!("pseudo_vein"));

        {
            let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
            let outcome = executor.enqueue_batch(batch(
                "cmd_pseudo_vein_injects_qi",
                vec![command(CommandType::SpawnEvent, "spawn", params)],
            ));
            assert!(outcome.accepted);
            assert!(!outcome.dedupe_drop);
        }

        app.update();

        let zone_qi = app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name("spawn")
            .expect("spawn zone should remain registered")
            .spirit_qi;
        assert_eq!(zone_qi, 0.6);
        let transfers = app
            .world()
            .resource::<Events<QiTransfer>>()
            .iter_current_update_events()
            .collect::<Vec<_>>();
        assert_eq!(transfers.len(), 1);
        assert_eq!(transfers[0].from, QiAccountId::tiandao());
        assert_eq!(transfers[0].to, QiAccountId::zone("spawn"));
        assert_eq!(transfers[0].amount, 0.5);
        assert_eq!(transfers[0].reason, QiTransferReason::ReleaseToZone);
    }

    #[test]
    fn spawn_event_pseudo_vein_is_idempotent_for_active_zone() {
        let mut app = setup_executor_app();
        app.world_mut()
            .resource_mut::<ZoneRegistry>()
            .find_zone_mut("spawn")
            .expect("fallback registry should contain spawn")
            .spirit_qi = 0.1;
        let mut params = HashMap::new();
        params.insert("event".to_string(), json!("pseudo_vein"));

        {
            let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
            let outcome = executor.enqueue_batch(batch(
                "cmd_pseudo_vein_once",
                vec![command(CommandType::SpawnEvent, "spawn", params.clone())],
            ));
            assert!(outcome.accepted);
        }
        app.update();

        {
            let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
            let outcome = executor.enqueue_batch(batch(
                "cmd_pseudo_vein_retry",
                vec![command(CommandType::SpawnEvent, "spawn", params)],
            ));
            assert!(outcome.accepted);
        }
        app.update();

        let mut query = app.world_mut().query::<&PseudoVeinRuntime>();
        let runtimes = query.iter(app.world()).collect::<Vec<_>>();
        assert_eq!(runtimes.len(), 1);
        assert_eq!(runtimes[0].zone_id, "spawn");
        assert_eq!(runtimes[0].injected_qi, 0.5);
    }

    #[test]
    fn spawn_event_pseudo_vein_rejects_same_batch_duplicate() {
        let mut app = setup_executor_app();
        app.world_mut()
            .resource_mut::<ZoneRegistry>()
            .find_zone_mut("spawn")
            .expect("fallback registry should contain spawn")
            .spirit_qi = 0.1;
        let mut params = HashMap::new();
        params.insert("event".to_string(), json!("pseudo_vein"));

        {
            let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
            let outcome = executor.enqueue_batch(batch(
                "cmd_pseudo_vein_same_batch_duplicate",
                vec![
                    command(CommandType::SpawnEvent, "spawn", params.clone()),
                    command(CommandType::SpawnEvent, "spawn", params),
                ],
            ));
            assert!(outcome.accepted);
        }

        app.update();

        let mut query = app.world_mut().query::<&PseudoVeinRuntime>();
        let runtimes = query.iter(app.world()).collect::<Vec<_>>();
        assert_eq!(runtimes.len(), 1);
        assert_eq!(runtimes[0].zone_id, "spawn");
        assert_eq!(runtimes[0].injected_qi, 0.5);
        let transfers = app
            .world()
            .resource::<Events<QiTransfer>>()
            .iter_current_update_events()
            .collect::<Vec<_>>();
        assert_eq!(
            transfers.len(),
            1,
            "same-batch duplicate should not emit a second pseudo vein injection"
        );
    }

    #[test]
    fn spawn_event_applies_hidden_karma_probability_weighting() {
        let mut app = setup_executor_app();
        app.world_mut()
            .resource_mut::<KarmaWeightStore>()
            .mark_player(
                "Azure",
                Some("spawn".to_string()),
                BlockPos::new(8, 66, 8),
                1.0,
                99,
            );

        let mut params = HashMap::new();
        params.insert("event".to_string(), json!(EVENT_KARMA_BACKLASH));
        params.insert("intensity".to_string(), json!(0.2));
        params.insert("duration_ticks".to_string(), json!(3));

        {
            let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
            let outcome = executor.enqueue_batch(batch(
                "cmd_karma_backlash_weighted",
                vec![command(CommandType::SpawnEvent, "spawn", params)],
            ));
            assert!(outcome.accepted);
            assert!(!outcome.dedupe_drop);
        }

        app.update();

        let events = app.world().resource::<ActiveEventsResource>();
        let recent = events.recent_events_snapshot();
        let marker = recent
            .iter()
            .find(|event| event.target.as_deref() == Some(EVENT_KARMA_BACKLASH))
            .expect("karma backlash should record hidden marker");
        let details = marker.details.as_ref().expect("hidden marker details");
        assert_eq!(
            details.get("command_intensity").and_then(Value::as_f64),
            Some(0.2)
        );
        assert_eq!(
            details.get("karma_weight").and_then(Value::as_f64),
            Some(1.0)
        );
        assert_eq!(
            details.get("base_probability").and_then(Value::as_f64),
            Some(f64::from(TARGETED_CALAMITY_BASE_PROBABILITY))
        );
        assert_eq!(
            details.get("effective_probability").and_then(Value::as_f64),
            Some(f64::from(TARGETED_CALAMITY_MAX_PROBABILITY))
        );
    }

    #[test]
    fn heartbeat_override_applies_via_executor_with_clock_fallback() {
        let mut app = setup_executor_app();
        let mut heartbeat = WorldHeartbeat::default();
        heartbeat.last_eval_tick = 10_000;
        heartbeat.eval_interval_ticks = 200;
        app.world_mut().insert_resource(heartbeat);

        let mut params = HashMap::new();
        params.insert("action".to_string(), json!("accelerate"));
        params.insert("event_type".to_string(), json!("beast_tide"));
        params.insert("target_zone".to_string(), json!("spawn"));
        params.insert("duration_ticks".to_string(), json!(600));
        params.insert("intensity_override".to_string(), json!(0.25));

        {
            let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
            let outcome = executor.enqueue_batch(batch(
                "cmd_heartbeat_override_ok",
                vec![command(CommandType::HeartbeatOverride, "spawn", params)],
            ));
            assert!(outcome.accepted);
            assert!(!outcome.dedupe_drop);
        }

        app.update();

        let heartbeat = app.world().resource::<WorldHeartbeat>();
        let override_ = heartbeat
            .override_for(HeartbeatEventKind::BeastTide, "spawn")
            .expect("heartbeat override should be stored");
        assert_eq!(
            override_.expires_at_tick, 10_800,
            "missing CultivationClock should fall back to last_eval_tick + eval_interval_ticks"
        );
        assert_eq!(
            override_.intensity_override,
            Some(0.25),
            "accelerate override should preserve configured intensity"
        );
    }

    #[test]
    fn heartbeat_override_returns_missing_heartbeat_without_resource() {
        let mut heartbeat = None;
        let command = command(
            CommandType::HeartbeatOverride,
            "spawn",
            HashMap::from([
                ("action".to_string(), json!("accelerate")),
                ("event_type".to_string(), json!("beast_tide")),
            ]),
        );

        let result = execute_heartbeat_override(&command, &mut heartbeat, Some(1_000));
        assert_eq!(
            result,
            HeartbeatOverrideError::MissingHeartbeat.result_label(),
            "missing heartbeat resource should reject the command"
        );
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
        let notices = app
            .world()
            .resource::<valence::prelude::Events<FactionEventNotice>>();
        assert_eq!(notices.len(), 1);
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
    fn spawn_npc_count_spawns_batch_and_clamps_to_remaining_budget() {
        let mut app = setup_executor_app();

        let mut params = HashMap::new();
        params.insert("archetype".to_string(), json!("rogue"));
        params.insert("count".to_string(), json!(3));

        {
            let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
            let outcome = executor.enqueue_batch(batch(
                "cmd_spawn_npc_count",
                vec![command(CommandType::SpawnNpc, "spawn", params)],
            ));
            assert!(outcome.accepted);
        }

        app.update();

        let rogue_count = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<&NpcArchetype, With<NpcMarker>>();
            query
                .iter(world)
                .filter(|archetype| **archetype == NpcArchetype::Rogue)
                .count()
        };
        assert_eq!(rogue_count, 3);
        assert_eq!(app.world().resource::<NpcRegistry>().live_npc_count, 3);

        {
            let mut registry = app.world_mut().resource_mut::<NpcRegistry>();
            registry.live_npc_count = registry.max_npc_count - 1;
            registry.spawn_paused = false;
        }
        let mut clamped_params = HashMap::new();
        clamped_params.insert("archetype".to_string(), json!("rogue"));
        clamped_params.insert("count".to_string(), json!(5));

        {
            let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
            let outcome = executor.enqueue_batch(batch(
                "cmd_spawn_npc_count_clamped",
                vec![command(CommandType::SpawnNpc, "spawn", clamped_params)],
            ));
            assert!(outcome.accepted);
        }

        app.update();

        let all_rogues = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<&NpcArchetype, With<NpcMarker>>();
            query
                .iter(world)
                .filter(|archetype| **archetype == NpcArchetype::Rogue)
                .count()
        };
        assert_eq!(
            all_rogues, 4,
            "second batch should reserve only one remaining slot"
        );
        assert_eq!(
            app.world().resource::<NpcRegistry>().live_npc_count,
            app.world().resource::<NpcRegistry>().max_npc_count
        );
    }

    #[test]
    fn spawn_npc_rejects_unknown_zone_unsupported_archetype_and_exhausted_budget() {
        let mut app = setup_executor_app();

        let mut commands = Vec::new();

        let mut bad_zone = HashMap::new();
        bad_zone.insert("archetype".to_string(), json!("zombie"));
        commands.push(command(CommandType::SpawnNpc, "missing_zone", bad_zone));

        let mut bad_archetype = HashMap::new();
        bad_archetype.insert("archetype".to_string(), json!("daoxiang"));
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
            .find_zone(
                crate::world::dimension::DimensionKind::Overworld,
                DVec3::new(8.0, 66.0, 8.0),
            )
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
            .find_zone(
                crate::world::dimension::DimensionKind::Overworld,
                DVec3::new(8.0, 66.0, 8.0),
            )
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
            .find_zone(
                crate::world::dimension::DimensionKind::Overworld,
                DVec3::new(8.0, 66.0, 8.0),
            )
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
            .find_zone(
                crate::world::dimension::DimensionKind::Overworld,
                DVec3::new(8.0, 66.0, 8.0),
            )
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
                .find_zone(
                    crate::world::dimension::DimensionKind::Overworld,
                    DVec3::new(8.0, 66.0, 8.0),
                )
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
                .find_zone(
                    crate::world::dimension::DimensionKind::Overworld,
                    DVec3::new(8.0, 66.0, 8.0),
                )
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
        unsupported_event_params.insert("event".to_string(), json!("unknown_calamity"));
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
            .find_zone(
                crate::world::dimension::DimensionKind::Overworld,
                DVec3::new(8.0, 66.0, 8.0),
            )
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
            .find_zone(
                crate::world::dimension::DimensionKind::Overworld,
                DVec3::new(8.0, 66.0, 8.0),
            )
            .expect("spawn zone should still exist");
        assert!((spawn_zone.spirit_qi - 0.8).abs() < 1e-9);
    }
}
