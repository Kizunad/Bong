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
    EmergentGroupId, FactionEventApplied, FactionEventCommand, FactionEventError, FactionEventKind,
    FactionEventNotice, FactionId, FactionRank, FactionStore,
};
use crate::npc::lifecycle::{NpcArchetype, NpcRegistry, NpcSpawnNotice, NpcSpawnSource};
use crate::npc::spawn::{
    snap_spawn_y_to_surface, spawn_beast_npc_at, spawn_commoner_npc_at, spawn_disciple_npc_at,
    spawn_notice, spawn_relic_guard_npc_at, spawn_rogue_npc_at, spawn_zombie_npc_at, NpcMarker,
    NpcSkinSpawnContext,
};
use crate::npc::territory::Territory;
use crate::npc::tsy_hostile::{
    spawn_tsy_daoxiang_at, spawn_tsy_fuya_at, spawn_tsy_skull_fiend_at, spawn_tsy_zhinian_at,
};
use crate::npc::war::{WarParticipateIntent, WarRole};
use crate::qi_physics::ledger::{QiTransfer, WorldQiAccount};
use crate::schema::agent_command::{AgentCommandV1, Command};
use crate::schema::common::{CommandType, GameEventType, MAX_COMMANDS_PER_TICK};
use crate::schema::pseudo_vein::PseudoVeinSeasonV1;
use crate::skin::{NpcSkinFallbackPolicy, SkinPool};
use crate::world::calamity::{CalamityArsenal, TiandaoPower};
use crate::world::era::EraDecreeIntent;
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
    // plan-zone-qi-economy-v1 P3 §8.1 决议 #3 — 灵潮注入从独立待分配池真实借出，
    // agent-command 驱动的 spawn_event{event=pseudo_vein} 路径需要 ledger 访问权。
    qi_ledger: Option<ResMut<'w, WorldQiAccount>>,
}

struct SpawnEventCommandResources<'a> {
    zone_registry: Option<&'a mut ZoneRegistry>,
    active_events: Option<&'a mut ActiveEventsResource>,
    tiandao_power: Option<&'a mut TiandaoPower>,
    calamity_arsenal: Option<&'a CalamityArsenal>,
    karma_weights: Option<&'a KarmaWeightStore>,
    qi_heatmap: Option<&'a QiDensityHeatmap>,
    qi_ledger: Option<&'a mut WorldQiAccount>,
}

/// 合并 agent command 执行上下文，避免 Bevy 0.14 顶层 SystemParam 16 上限。
#[derive(SystemParam)]
pub struct CommandExecutionParams<'w> {
    heartbeat: Option<ResMut<'w, WorldHeartbeat>>,
    karma_weights: Option<Res<'w, KarmaWeightStore>>,
    qi_heatmap: Option<Res<'w, QiDensityHeatmap>>,
    clock: Option<Res<'w, CultivationClock>>,
    terrain_providers: Option<Res<'w, TerrainProviders>>,
    technique_registry: Res<'w, crate::cultivation::known_techniques::TechniqueRegistry>,
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
    // plan-offscreen-war-v1 P6：headless 玩家参与涌现冲突（路径 B，汇聚到与 brigadier 同一 handler）。
    mut war_intents: EventWriter<WarParticipateIntent>,
    // plan-era-state-v1 B1：modify_zone{target="全局"} 携带 era_name → EraDecreeIntent。
    mut era_decree_intents: EventWriter<EraDecreeIntent>,
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
                &mut world_resources.qi_ledger,
                &mut npc_registry,
                &mut skin_pool,
                &mut faction_store,
                &mut npc_behavior,
                &mut params.heartbeat,
                &params.technique_registry,
                params.karma_weights.as_deref(),
                params.qi_heatmap.as_deref(),
                params.clock.as_deref().map(|clock| clock.tick),
                terrain,
                &mut npc_spawn_notices,
                &mut faction_notices,
                &mut qi_transfers,
                &mut war_intents,
                &mut era_decree_intents,
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
    qi_ledger: &mut Option<ResMut<WorldQiAccount>>,
    npc_registry: &mut Option<ResMut<NpcRegistry>>,
    skin_pool: &mut Option<ResMut<SkinPool>>,
    faction_store: &mut Option<ResMut<FactionStore>>,
    npc_behavior: &mut Option<ResMut<NpcBehaviorConfig>>,
    heartbeat: &mut Option<ResMut<WorldHeartbeat>>,
    technique_registry: &crate::cultivation::known_techniques::TechniqueRegistry,
    karma_weights: Option<&KarmaWeightStore>,
    qi_heatmap: Option<&QiDensityHeatmap>,
    tick: Option<u64>,
    terrain: Option<&TerrainProvider>,
    npc_spawn_notices: &mut EventWriter<NpcSpawnNotice>,
    faction_notices: &mut EventWriter<FactionEventNotice>,
    qi_transfers: &mut EventWriter<QiTransfer>,
    // plan-offscreen-war-v1 P6：headless 路径 B 注入的战争参与 intent。
    war_intents: &mut EventWriter<WarParticipateIntent>,
    // plan-era-state-v1 B1：生产路径 modify_zone{target="全局"} → EraDecreeIntent。
    era_decree_intents: &mut EventWriter<EraDecreeIntent>,
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
        CommandType::ModifyZone => {
            execute_modify_zone(command, zone_registry, era_decree_intents, tick)
        }
        CommandType::SpawnNpc => execute_spawn_npc(
            command,
            commands,
            technique_registry,
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
        CommandType::FactionEvent => execute_faction_event(
            command,
            faction_store,
            active_events,
            faction_notices,
            war_intents,
        ),
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
                qi_ledger: qi_ledger.as_deref_mut(),
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
    // plan-offscreen-war-v1 P6：路径 B headless 注入出口（`war_participate=true` 分支）。
    war_intents: &mut EventWriter<WarParticipateIntent>,
) -> &'static str {
    // plan-offscreen-war-v1 P6：前置分支——`war_participate=true` 时走涌现冲突参与路径（路径 B），
    // 不走原 faction 逻辑。约定：target == zone_name，params 含 player_id/role/group。
    if command
        .params
        .get("war_participate")
        .and_then(Value::as_bool)
        == Some(true)
    {
        return execute_war_participate_headless(command, war_intents);
    }

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

/// plan-offscreen-war-v1 P6：路径 B headless 参与入口（`execute_faction_event` 前置分支）。
///
/// params 约定：
/// - `"war_participate": true`（触发此分支）
/// - `"player_id": "offline:e2e_merc"`
/// - `"role": "mercenary"` / `"enlist"` / `"intercept"` / `"spectate"`
/// - `"group": 2`（Enlist/Mercenary 必填；Intercept/Spectate 忽略）
///
/// target == zone_name。完全 headless（无 Client entity）；错误直接返回 label。
fn execute_war_participate_headless(
    command: &Command,
    war_intents: &mut EventWriter<WarParticipateIntent>,
) -> &'static str {
    let zone = command.target.clone();

    let Some(player_id) = command
        .params
        .get("player_id")
        .and_then(Value::as_str)
        .map(str::to_string)
    else {
        tracing::warn!(
            "[bong][network][war] war_participate missing player_id for zone={}",
            zone
        );
        return "rejected_missing_player_id";
    };

    let role_str = command
        .params
        .get("role")
        .and_then(Value::as_str)
        .unwrap_or("spectate");

    let role =
        match role_str {
            "enlist" => WarRole::Enlist,
            "mercenary" => WarRole::Mercenary,
            "intercept" => WarRole::Intercept,
            "spectate" => WarRole::Spectate,
            unknown => {
                tracing::warn!(
                "[bong][network][war] war_participate unknown role \"{}\" for zone={} player_id={}",
                unknown, zone, player_id
            );
                return "rejected_invalid_war_role";
            }
        };

    let allied_group = command
        .params
        .get("group")
        .and_then(Value::as_u64)
        .and_then(|g| u16::try_from(g).ok())
        .map(EmergentGroupId);

    war_intents.send(WarParticipateIntent {
        player_id,
        zone,
        role,
        allied_group,
        at_tick: 0, // headless 无 GameTick 访问权，0 是合理默认（handle_war_participate_intent 不依赖此值做业务判断）
    });

    "ok"
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
    technique_registry: &crate::cultivation::known_techniques::TechniqueRegistry,
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
        "daoxiang" => NpcArchetype::Daoxiang,
        "zhinian" => NpcArchetype::Zhinian,
        "fuya" => NpcArchetype::Fuya,
        "skull_fiend" => NpcArchetype::SkullFiend,
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
                    technique_registry,
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
                    technique_registry,
                    NpcSkinSpawnContext::new(
                        skin_pool.as_deref_mut(),
                        NpcSkinFallbackPolicy::AllowFallback,
                    ),
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
                    technique_registry,
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
            NpcArchetype::Daoxiang => {
                let family_id = command
                    .params
                    .get("tsy_family_id")
                    .and_then(Value::as_str)
                    .unwrap_or(zone.name.as_str());
                let entity = spawn_tsy_daoxiang_at(
                    commands,
                    technique_registry,
                    layer,
                    family_id,
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
                    initial_age_ticks,
                ));
            }
            NpcArchetype::Zhinian => {
                let family_id = command
                    .params
                    .get("tsy_family_id")
                    .and_then(Value::as_str)
                    .unwrap_or(zone.name.as_str());
                let entity = spawn_tsy_zhinian_at(
                    commands,
                    technique_registry,
                    layer,
                    family_id,
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
                    initial_age_ticks,
                ));
            }
            NpcArchetype::Fuya => {
                let family_id = command
                    .params
                    .get("tsy_family_id")
                    .and_then(Value::as_str)
                    .unwrap_or(zone.name.as_str());
                let entity = spawn_tsy_fuya_at(
                    commands,
                    layer,
                    family_id,
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
                    initial_age_ticks,
                ));
            }
            NpcArchetype::SkullFiend => {
                let family_id = command
                    .params
                    .get("tsy_family_id")
                    .and_then(Value::as_str)
                    .unwrap_or(zone.name.as_str());
                let entity = spawn_tsy_skull_fiend_at(
                    commands,
                    layer,
                    family_id,
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
                    initial_age_ticks,
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
        qi_ledger,
    } = resources;

    if event_name(command) == Some(EVENT_PSEUDO_VEIN) {
        return execute_spawn_pseudo_vein(
            command,
            commands,
            zone_registry,
            qi_ledger,
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

#[allow(clippy::too_many_arguments)]
fn execute_spawn_pseudo_vein(
    command: &Command,
    commands: &mut Commands,
    zone_registry: Option<&mut ZoneRegistry>,
    qi_ledger: Option<&mut WorldQiAccount>,
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
    let injected_qi = match qi_ledger {
        Some(ledger) => {
            if let Some(transfer) = inject_zone_for_pseudo_vein(zone, ledger) {
                let amount = transfer.amount;
                qi_transfers.send(transfer);
                amount
            } else {
                0.0
            }
        }
        None => {
            tracing::warn!(
                "[bong][network] cannot borrow pending-inflow qi for pseudo_vein zone `{}` \
                 because WorldQiAccount is missing; spawning with zero injection",
                zone.name
            );
            0.0
        }
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
    era_decree_intents: &mut EventWriter<EraDecreeIntent>,
    tick: Option<u64>,
) -> &'static str {
    // plan-era-state-v1 B1 — 若 target=="全局" 且 params 携带 era_name，走时代宣告路径。
    // agent era.md skill 总是将 era_decree 作为 modify_zone{target="全局"} 发出（viability 决议 §8#1）。
    if command.target == "全局" {
        if let Some(era_name) = command.params.get("era_name").and_then(|v| v.as_str()) {
            let spirit_qi_delta = param_as_f64(&command.params, "spirit_qi_delta").unwrap_or(0.0);
            let danger_level_delta =
                optional_param_as_i64(&command.params, "danger_level_delta").unwrap_or(0);
            let current_tick = tick.unwrap_or(0);

            tracing::info!(
                "[bong][era] era_decree_from_agent: era_name={} spirit_qi_delta={} danger_level_delta={} tick={}",
                era_name,
                spirit_qi_delta,
                danger_level_delta,
                current_tick,
            );

            era_decree_intents.send(EraDecreeIntent {
                era_name: era_name.to_string(),
                spirit_qi_delta,
                danger_level_delta,
                tick: current_tick,
            });
            return "ok_era_decree";
        }
        // 全局 target without era_name — reject with clear reason
        tracing::warn!(
            "[bong][network] modify_zone target `全局` missing `era_name` param; \
             全局 target only valid for era_decree commands"
        );
        return "rejected_global_missing_era_name";
    }

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
#[path = "command_executor_tests.rs"]
mod command_executor_tests;
