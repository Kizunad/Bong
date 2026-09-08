pub mod agent_bridge;
pub mod agent_ui;
pub mod alchemy_bridge;
pub mod alchemy_snapshot_emit;
// plan-skill-av-relink-v1 P3 —— P1 接线 anim_id 共享清单单向同步测试。
#[cfg(test)]
mod anim_wiring_manifest_test;
pub mod animation_trigger;
pub mod anqi_event_bridge;
pub mod anqi_hud_emit;
pub mod anticheat_bridge;
pub mod ascension_quota_emit;
pub mod audio_event_emit;
pub mod audio_trigger;
pub mod baomai_v3_event_bridge;
pub mod baomai_v4_event_bridge;
pub mod burst_event_emit;
pub mod carrier_state_emit;
pub mod cast_emit;
pub mod chat_collector;
pub mod client_request;
pub mod client_request_handler;
pub mod combat_bridge;
pub mod combat_event_emit;
pub mod combat_hud_state_emit;
pub mod command_executor;
pub mod craft_emit;
pub mod craft_event_bridge;
pub mod cultivation_bridge;
pub mod cultivation_detail_emit;
pub mod cultivation_insight_offer_emit;
pub mod defense_window_emit;
pub mod derived_attrs_emit;
pub mod disguise_sync;
pub mod dropped_loot_sync_emit;
pub mod dugu_event_bridge;
pub mod dugu_state_emit;
pub mod dugu_v2_event_bridge;
pub mod event_stream_emit;
pub mod extract_emit;
pub mod false_skin_state_emit;
pub mod forge_bridge;
pub mod forge_snapshot_emit;
pub mod freshness_probe_emit;
pub mod full_power_emit;
pub mod gameplay_vfx;
pub mod gate;
// R6 P1 —— contract-first S2C emit scope/builder；暂不接入 production producer。
pub mod emit;
pub mod identity_panel_emit;
pub mod inventory_event_emit;
pub mod inventory_move_rejected_emit;
pub mod inventory_snapshot_emit;
pub mod knockback_sync_emit;
pub mod meridian_severed_emit;
pub mod mineral_probe_emit;
pub mod mutation_event_publish;
pub mod mutation_visual_emit;
pub mod npc_bubble;
pub mod npc_event_bridge;
pub mod npc_lod_emit;
pub mod npc_metadata;
pub mod npc_mood;
pub mod poi_novice_bridge;
pub mod poison_trait_emit;
pub mod qi_attrition_emit;
pub mod qi_color_observed_emit;
pub mod quickslot_config_emit;
// plan-skill-av-relink-v1 P3 —— quickslot 发射契约测试（Item 槽 icon_texture 恒空串）。
#[cfg(test)]
mod quickslot_config_emit_test;
// plan-race-system-v1 P3c — 种族门元数据表（RaceGateMeta）构建 + join 首帧下发。
pub mod morph_state_emit;
pub mod race_gate_meta_emit;
// plan-devour-rat-model P3 — 噬元鼠出招 → GeckoLib 实体招式动画
pub mod rat_av_trigger;
pub mod rat_phase_bridge;
// plan-devour-rat-model P2 — 噬元鼠吸元档位 S2C CustomPayload（贴图变体 + emissive）
pub mod rat_qi_tier_emit;
pub mod redis_bridge;
pub mod remains_sync_emit;
pub mod resourcepack;
// plan-scroll-reading-v1 P0 — 可阅读残卷阅读屏 S2C `ScrollOpen` 回执发送。
pub mod scroll_open_emit;
pub mod skill_config_emit;
pub mod skill_emit;
pub mod skill_snapshot_emit;
pub mod skill_vfx_wiring;
#[cfg(test)]
mod skill_vfx_wiring_test;
pub mod skillbar_config_emit;
#[cfg(test)]
mod skillbar_config_emit_test;
// plan-daozhan-v1 P1 — 道伥伪装状态 S2C CustomPayload
pub mod daozhan_disguise_emit;
// plan-dying-elder-v1 B1 — 垂死大能遭遇 S2C CustomPayload（HUD 驱动）
pub mod elder_encounter_emit;
// plan-era-state-v1 P3 — 时代天象 S2C CustomPayload
pub mod era_ambiance_emit;
// plan-fauna-mimic-spider-v1 P2 — 拟态蛛伪装状态 S2C CustomPayload
pub mod spider_disguise_emit;
// plan-halfstep-rechallenge-integration-v1 P0：半步化虚重渡触发 HUD S2C
pub mod halfstep_rechallenge_emit;
pub mod heiwushi_av_trigger;
pub mod spirit_treasure_emit;
pub mod status_snapshot_emit;
pub mod sword_bond_state_emit;
pub mod techniques_snapshot_emit;
pub mod treasure_equipped_emit;
pub mod tribulation_broadcast_emit;
pub mod tribulation_heart_demon_offer_emit;
pub mod tribulation_state_emit;
pub mod tsy_container_search_emit;
pub mod tsy_event_bridge;
pub mod tsy_polish;
pub mod tuike_ash_emit;
pub mod tuike_event_bridge;
pub mod unlocks_sync_emit;
pub mod vfx_animation_trigger;
pub mod vfx_event_emit;
pub mod void_erosion_visual_emit;
pub mod weapon_equipped_emit;
pub mod weather_bridge;
pub mod woliu_event_bridge;
pub mod woliu_state_emit;
pub mod wounds_snapshot_emit;
pub mod yidao_state_emit;
pub mod zhenfa_v2_event_bridge;
pub mod zhenmai_v2_event_bridge;
pub mod zone_environment_bridge;
pub mod zone_pressure_bridge;

use std::collections::{HashMap, HashSet, VecDeque};
use std::io;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use agent_bridge::{
    payload_type_label, route_recipient_indices, serialize_server_data_payload, AgentCommand,
    GameEvent, NetworkBridgeResource, PayloadBuildError, RecipientMetadata, RecipientSelector,
    SERVER_DATA_CHANNEL,
};
use big_brain::prelude::{ActionState, Actor};
use chat_collector::{collect_player_chat, ChatCollectorRateLimit, ChatObservationClock};
use command_executor::{execute_agent_commands, CommandExecutorResource};
use redis_bridge::{RedisInbound, RedisOutbound};
use valence::prelude::bevy_ecs::system::SystemParam;
use valence::prelude::{
    bevy_ecs, ident, Added, App, Changed, Client, Commands, DVec3, Entity, EntityKind, EventReader,
    EventWriter, Events, IntoSystemConfigs, Or, Position, PostUpdate, Query, Res, ResMut, Resource,
    Startup, Update, Username, With,
};

use crate::combat::components::{Lifecycle, StatusEffects};
use crate::combat::CombatClock;
use crate::cultivation::color::PracticeLog;
use crate::cultivation::components::{Cultivation, MeridianSystem, QiColor};
use crate::cultivation::insight::InsightQuota;
use crate::cultivation::insight_apply::UnlockedPerceptions;
use crate::cultivation::life_record::LifeRecord;
use crate::cultivation::possession::DuoSheWarningEvent;
use crate::fauna::rat_phase::{collect_rat_density_heatmap, RatDensityHeatmapV1, RatPhase};
use crate::inventory::spirit_treasure::{ActiveSpiritTreasures, SpiritTreasureRegistry};
use crate::npc::brain::{canonical_npc_id, ChaseAction, DashAction, FleeAction, MeleeAttackAction};
use crate::npc::dormant::NpcDormantStore;
use crate::npc::faction::{FactionMembership, FactionStore, Lineage, MissionQueue};
use crate::npc::lifecycle::{NpcArchetype, NpcLifespan};
use crate::npc::patrol::NpcPatrol;
use crate::npc::spawn::{NpcBlackboard, NpcMarker};
use crate::npc::spawn_rat::RatBlackboard;
use crate::npc::war::settle::ZoneSpiritBonusStore;
use crate::npc::war::{
    WarConflictStore, WarParticipateIntent, WarPhaseChanged, ZoneConflictPressure,
};
use crate::persistence::{
    bootstrap_agent_world_model_mirror, persist_agent_world_model_authority_state,
    persist_life_record_death_insight, world_model_snapshot_to_mirror_fields,
    AgentWorldModelCommandRecord, AgentWorldModelDecisionRecord, AgentWorldModelNarrationRecord,
    AgentWorldModelNegDomainEscapeSessionRecord, AgentWorldModelNegDomainEscapeTelemetryRecord,
    AgentWorldModelNegDomainPendingTribulationRecord, AgentWorldModelSnapshotRecord,
    PersistenceSettings, WORLD_MODEL_STATE_KEY,
};
use crate::player::gameplay::PendingGameplayNarrations;
use crate::player::state::{canonical_player_id, PlayerState};
use crate::qi_physics::{build_qi_ledger_hash_fields, summarize_world_qi, WorldQiAccount};
use crate::schema::agent_world_model::{AgentWorldModelEnvelopeV1, AgentWorldModelSnapshotV1};
use crate::schema::common::{
    CommandType, EventKind, NarrationKind, NarrationScope, NarrationStyle, NpcStateKind,
    PlayerTrend,
};
use crate::schema::cultivation::{
    realm_from_string, realm_to_string, CultivationSnapshotV1, LifeRecordSnapshotV1,
    SkillMilestoneSnapshotV1,
};
use crate::schema::season::SeasonChangedV1;
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
use crate::schema::social::{
    FactionMembershipSnapshotV1, PlayerSocialSnapshotV1, RelationshipSnapshotV1, RenownSnapshotV1,
};
use crate::schema::world_state::{
    DiscipleSummaryV1, FactionSummaryV1, LineageSummaryV1, MissionQueueSummaryV1, NpcDigestV1,
    NpcSnapshot, PlayerProfile, SeasonStateV1, WorldStateV1, ZoneSnapshot, ZoneStatusV1,
};
use crate::skill::components::SkillId;
use crate::social::components::{
    Anonymity, FactionMembership as PlayerFactionMembership, Relationships, Renown,
};
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::era::WorldEraState;
use crate::world::events::{ActiveEventsResource, EVENT_REALM_COLLAPSE};
use crate::world::season::{query_season, SeasonChangedEvent, WorldSeasonState};
use crate::world::terrain::TerrainProviders;
use crate::world::tsy_lifecycle::EVENT_TSY_RACE_OUT;
use crate::world::zone::{Zone, ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};

const REDIS_URL_ENV_KEY: &str = "REDIS_URL";
const DEFAULT_REDIS_URL: &str = "redis://127.0.0.1:6379";
const WORLD_STATE_PUBLISH_INTERVAL_TICKS: u64 = 200; // ~10 seconds at 20 TPS
const REDIS_INBOUND_DRAIN_BUDGET: usize = 16;
const DEFAULT_PLAYER_ACTIVE_HOURS: f64 = 0.0;
const DEFAULT_PLAYER_RECENT_KILLS: u32 = 0;
const DEFAULT_PLAYER_RECENT_DEATHS: u32 = 0;
const NARRATION_DEDUPE_WINDOW_SECS: u64 = 15;
const NARRATION_DEDUPE_CAPACITY: usize = 512;
const WORLD_MODEL_RUNTIME_MIRROR_RECONCILE_INTERVAL_TICKS: u64 = 20 * 60 * 5;

type ClientLifeRecordQueryItem<'a> = (
    Option<&'a Username>,
    Option<&'a Lifecycle>,
    &'a mut LifeRecord,
);
type ClientLifeRecordQueryFilter = With<Client>;
type InsightContextQueryItem<'a> = (
    &'a QiColor,
    &'a PracticeLog,
    &'a InsightQuota,
    &'a Cultivation,
);

/// Resource holding the Redis bridge channels
pub struct RedisBridgeResource {
    pub tx_outbound: crossbeam_channel::Sender<RedisOutbound>,
    pub rx_inbound: crossbeam_channel::Receiver<RedisInbound>,
}

impl Resource for RedisBridgeResource {}

#[derive(Clone)]
pub struct RuntimeMirrorRedisConfig {
    client: redis::Client,
    connection: Arc<Mutex<Option<redis::Connection>>>,
}

impl Resource for RuntimeMirrorRedisConfig {}

impl RuntimeMirrorRedisConfig {
    fn new(url: String) -> io::Result<Self> {
        let client = redis::Client::open(url.as_str()).map_err(io::Error::other)?;
        Ok(Self {
            client,
            connection: Arc::new(Mutex::new(None)),
        })
    }
}

/// Tick counter for world state publishing
#[derive(Default)]
pub struct WorldStateTimer {
    ticks: u64,
}

impl Resource for WorldStateTimer {}

/// plan-offscreen-war-v1 P0：守恒 telemetry（`bong:qi/ledger`）独立 tick 计数器。
///
/// 用独立 timer 而非复用 `WorldStateTimer`：`bong:qi/ledger` publish 是 exclusive
/// system（`summarize_world_qi` 需 `&mut World`），与 world_state 普通 system 不在
/// 同一调度阶段，复用计数器会因执行顺序产生相位偏差。两者都按
/// `WORLD_STATE_PUBLISH_INTERVAL_TICKS` 周期，节奏一致但各自计数。
#[derive(Default)]
pub struct QiLedgerTimer {
    ticks: u64,
}

impl Resource for QiLedgerTimer {}

#[derive(Default)]
struct ZoneTransitionTracker {
    last_zone_by_entity: HashMap<Entity, String>,
    last_snapshot_by_entity: HashMap<Entity, ZoneInfoRuntimeSnapshot>,
}

impl Resource for ZoneTransitionTracker {}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ZoneInfoRuntimeSnapshot {
    zone: String,
    spirit_qi_bits: u64,
    danger_level: u8,
    status: ZoneStatusV1,
    active_events: Vec<String>,
}

impl ZoneInfoRuntimeSnapshot {
    fn from_zone(zone: &Zone) -> Self {
        Self {
            zone: zone.name.clone(),
            spirit_qi_bits: zone.spirit_qi.to_bits(),
            danger_level: zone.danger_level,
            status: zone_status(zone),
            active_events: zone.active_events.clone(),
        }
    }

    fn spirit_qi(&self) -> f64 {
        f64::from_bits(self.spirit_qi_bits)
    }
}

#[derive(Default)]
struct NarrationDedupeResource {
    recent_payload_keys: VecDeque<(String, u64)>,
}

impl Resource for NarrationDedupeResource {}

#[derive(Default)]
struct WorldModelMirrorReconcileState {
    ticks_since_last_reconcile: u64,
}

impl Resource for WorldModelMirrorReconcileState {}

impl NarrationDedupeResource {
    fn should_drop(&mut self, payload_key: &str, now_secs: u64) -> bool {
        self.prune(now_secs);

        if self
            .recent_payload_keys
            .iter()
            .any(|(key, _)| key == payload_key)
        {
            return true;
        }

        self.recent_payload_keys
            .push_back((payload_key.to_string(), now_secs));
        while self.recent_payload_keys.len() > NARRATION_DEDUPE_CAPACITY {
            self.recent_payload_keys.pop_front();
        }

        false
    }

    fn prune(&mut self, now_secs: u64) {
        while let Some((_, seen_at_secs)) = self.recent_payload_keys.front() {
            let age_secs = now_secs.saturating_sub(*seen_at_secs);
            if age_secs > NARRATION_DEDUPE_WINDOW_SECS {
                self.recent_payload_keys.pop_front();
                continue;
            }
            break;
        }

        while self.recent_payload_keys.len() > NARRATION_DEDUPE_CAPACITY {
            self.recent_payload_keys.pop_front();
        }
    }
}

pub(crate) fn register_craft_start_runtime_system(app: &mut App) {
    app.add_systems(
        Update,
        craft_emit::apply_craft_start_intents
            .after(client_request_handler::handle_client_request_payloads),
    );
}

/// network 层生产注册入口 = 外部 bridge bootstrap（有副作用）+ 纯 App 装配。
///
/// 拆成两段是为了让接线门禁测试能跑**真正的生产装配路径**：`register_app_wiring` 只做
/// `insert_resource` / `add_systems` / `add_event`，不起线程、不碰 IO，测试可直接调用；
/// 起 Redis bridge 线程那段单独关在 `bootstrap_redis_bridge` 里（PR #1262 review 要求）。
pub(crate) fn register_lingtian_ingress_wiring(app: &mut App) {
    app.init_resource::<client_request_handler::ClientRequestBudget>();
    app.init_resource::<client_request_handler::LingtianPlotIndex>();
    app.add_systems(
        Update,
        client_request_handler::refresh_lingtian_plot_index
            .before(client_request_handler::handle_client_request_payloads),
    );
    app.add_systems(
        Update,
        client_request_handler::cleanup_client_request_budget
            .before(client_request_handler::handle_client_request_payloads)
            .before(crate::player::despawn_disconnected_clients),
    );
    app.add_systems(
        Update,
        client_request_handler::handle_client_request_payloads
            .in_set(crate::lingtian::LingtianRequestIngressSet),
    );
}

pub fn register(app: &mut App) {
    bootstrap_redis_bridge(app);
    register_app_wiring(app);
}

/// **唯一有外部副作用的一段**：起 Redis bridge 线程 + 装入 redis 相关资源。测试不调它。
fn bootstrap_redis_bridge(app: &mut App) {
    let redis_url = redis_url_from_env();
    tracing::info!(
        "[bong][redis] configured redis endpoint: {}",
        redact_redis_url_for_log(redis_url.as_str())
    );
    let (handle, tx_outbound, rx_inbound) = redis_bridge::spawn_redis_bridge(redis_url.as_str());
    std::mem::drop(handle); // detach thread

    app.insert_resource(RedisBridgeResource {
        tx_outbound,
        rx_inbound,
    });
    let runtime_mirror_redis =
        RuntimeMirrorRedisConfig::new(redis_url.clone()).unwrap_or_else(|error| {
            panic!(
                "failed to initialize runtime mirror redis client for {}: {error}",
                redact_redis_url_for_log(redis_url.as_str())
            )
        });
    app.insert_resource(runtime_mirror_redis);
}

/// **纯 App 装配**（只 `insert_resource` / `add_systems` / `add_event`；无线程、无网络 / 文件 IO，
/// 仅 `ResourcePackConfig::from_env` 读几个环境变量且无 panic 路径）——
/// 生产由 `register` 调用，接线门禁测试也调用同一个它。于是「顶层把
/// `audio_trigger::register(app)` 那行删掉」不再是测试照不到的死角。
pub(crate) fn register_app_wiring(app: &mut App) {
    // Legacy mock bridge systems
    app.add_systems(
        Update,
        (send_welcome_payload_on_join, process_bridge_messages),
    );

    app.insert_resource(WorldStateTimer::default());
    app.insert_resource(QiLedgerTimer::default());
    app.insert_resource(ZoneTransitionTracker::default());
    app.insert_resource(ChatCollectorRateLimit::default());
    app.insert_resource(ChatObservationClock::default());
    app.insert_resource(CommandExecutorResource::default());
    app.insert_resource(NarrationDedupeResource::default());
    app.insert_resource(WorldModelMirrorReconcileState::default());
    app.insert_resource(combat_bridge::CombatSummaryAccumulator::default());
    app.insert_resource(resourcepack::ResourcePackConfig::default());
    app.insert_resource(resourcepack::ResourcePackStatusStore::default());

    app.add_systems(
        Startup,
        bootstrap_world_model_runtime_mirror_system
            .after(crate::persistence::PersistenceBootstrapSet),
    );

    app.add_systems(
        Update,
        (
            // plan-sword-path-v2 P2.3 review fix: 必须在 heaven_gate_cast_system 之后跑，
            // 否则化虚一击同 tick 内 publish 会先于盲区注册执行，导致 caster 坐标
            // 在被隐藏前泄露一个发布周期给 agent。
            publish_world_state_to_redis
                .after(crate::sword_path::skill_register::heaven_gate_cast_system),
            collect_player_chat,
            process_redis_inbound,
            reconcile_world_model_runtime_mirror_system.after(process_redis_inbound),
            execute_agent_commands.after(process_redis_inbound),
            enqueue_duo_she_warning_narrations
                .after(crate::cultivation::possession::process_duo_she_requests),
            emit_gameplay_narrations.after(crate::player::gameplay::apply_queued_gameplay_actions),
            emit_player_state_payloads
                .after(crate::player::attach_player_state_to_joined_clients)
                .after(crate::player::gameplay::apply_queued_gameplay_actions)
                .after(crate::social::apply_social_renown_deltas),
            inventory_snapshot_emit::emit_join_inventory_snapshots
                .after(crate::inventory::attach_inventory_to_joined_clients),
            alchemy_snapshot_emit::emit_join_alchemy_snapshots
                .after(crate::inventory::attach_inventory_to_joined_clients),
            skill_snapshot_emit::emit_join_skill_snapshots
                .after(crate::inventory::attach_inventory_to_joined_clients),
            skill_emit::emit_skill_xp_gain_payloads.after(crate::skill::consume_skill_xp_gain),
            skill_emit::emit_skill_lv_up_payloads.after(crate::skill::consume_skill_xp_gain),
            skill_emit::emit_skill_cap_changed_payloads,
            skill_emit::emit_skill_scroll_used_payloads,
            emit_zone_info_on_zone_transition,
            emit_event_alerts_on_major_event_creation.after(execute_agent_commands),
            combat_bridge::publish_combat_realtime_events
                .after(crate::combat::resolve::resolve_attack_intents)
                .in_set(crate::npc::lifecycle::NpcTerminalSystemSet::PostCommit),
            combat_bridge::publish_death_insight_requests
                .after(crate::combat::lifecycle::death_arbiter_tick),
            combat_bridge::publish_combat_summary_on_interval.after(publish_world_state_to_redis),
        ),
    );
    app.add_systems(
        Update,
        combat_bridge::publish_death_cinematic_events
            .after(crate::combat::lifecycle::near_death_tick),
    );
    app.add_systems(
        Update,
        publish_season_changed_events
            .after(crate::world::season::season_tick)
            .after(emit_player_state_payloads),
    );
    // plan-offscreen-war-v1 P0：守恒 telemetry exclusive system，与 world_state 同节奏。
    app.add_systems(Update, publish_qi_ledger_to_redis);
    app.add_systems(
        Update,
        anticheat_bridge::publish_anticheat_violation_events
            .after(crate::combat::anticheat::emit_anticheat_threshold_reports),
    );
    // Spawn producers use deferred Commands in Update. Publish their stable LifeRecord identity
    // only after those commands have been applied at the schedule boundary.
    app.add_systems(PostUpdate, npc_event_bridge::publish_npc_spawn_events);
    app.add_systems(
        Update,
        (
            npc_event_bridge::publish_npc_death_events,
            // plan-offscreen-war-v1 P2：离屏战果 telemetry → bong:npc/combat。
            npc_event_bridge::publish_dormant_combat_events,
            // plan-offscreen-war-v1 P3：战场遗物创建 telemetry → bong:npc/relic。
            // Bevy 0.14 tuple 内系统**顺序不保证执行先后**（默认并行/机会式），故用显式
            // .after()/.before() 锁死「combat → relic → faction」语义先后（CodeRabbit）：战果
            // 先于遗物、遗物先于派系 telemetry，bong:npc/relic 发送时序与注释一致。
            npc_event_bridge::publish_pending_dormant_relic_events
                .after(npc_event_bridge::publish_dormant_combat_events)
                .before(npc_event_bridge::publish_faction_events),
            npc_event_bridge::publish_faction_events
                .after(execute_agent_commands)
                .after(npc_event_bridge::publish_pending_dormant_relic_events),
            // plan-offscreen-war-v1 P5：散修群体消长盘面 telemetry → bong:faction_state。
            // 与 combat/relic/faction telemetry 同节奏 Update 旁路（不另起 timer，§10.1 #3）；
            // 纯只读 census，零真元——排在 faction telemetry 之后，盘面 publish 时序居于事件流末尾。
            npc_event_bridge::publish_faction_state.after(npc_event_bridge::publish_faction_events),
            // plan-faction-expansion-v1 P3：具名势力注册表快照 → bong:named_faction_state。
            npc_event_bridge::publish_named_faction_state
                .after(npc_event_bridge::publish_faction_state),
            npc_event_bridge::publish_named_faction_state_on_lifecycle_events
                .after(npc_event_bridge::publish_named_faction_state),
            // plan-offscreen-war-v1 P6：涌现冲突生命周期（reframe b，纯观测、零真元）。
            // 调度链：combat_events → accumulate → advance_idle → handle_participate → publish_war。
            npc_event_bridge::accumulate_zone_conflict_pressure
                .after(npc_event_bridge::publish_dormant_combat_events)
                .before(npc_event_bridge::publish_faction_state),
            npc_event_bridge::advance_idle_wars
                .after(npc_event_bridge::accumulate_zone_conflict_pressure),
            crate::npc::war::handle_war_participate_intent
                .after(npc_event_bridge::advance_idle_wars),
            // plan-offscreen-war-v1 P9：settle systems（ZoneSpiritBonus + Renown）。
            // 读 WarPhaseChanged 事件沿，排在 advance_idle_wars 之后、publish_faction_war 之前。
            crate::npc::war::settle::apply_war_zone_spirit_bonus
                .after(npc_event_bridge::advance_idle_wars)
                .after(npc_event_bridge::accumulate_zone_conflict_pressure),
            crate::npc::war::settle::award_war_winner_renown
                .after(npc_event_bridge::advance_idle_wars)
                .after(npc_event_bridge::accumulate_zone_conflict_pressure),
            npc_event_bridge::publish_faction_war
                .after(npc_event_bridge::advance_idle_wars)
                .after(crate::npc::war::handle_war_participate_intent)
                .after(crate::npc::war::settle::apply_war_zone_spirit_bonus)
                .after(crate::npc::war::settle::award_war_winner_renown)
                .after(npc_event_bridge::publish_named_faction_state)
                .after(npc_event_bridge::publish_named_faction_state_on_lifecycle_events),
            rat_phase_bridge::publish_rat_phase_events
                .after(crate::fauna::rat_phase::pressure_sensor_tick_system),
            zone_pressure_bridge::publish_zone_pressure_crossed_events
                .after(crate::lingtian::systems::compute_zone_pressure_system),
            // plan-lingtian-weather-v1 §3 / §4.4 — 把 Bevy WeatherLifecycleEvent
            // 转译成 RedisOutbound::WeatherEventUpdate；必须在 weather generator /
            // apply system 之后跑，确保 Bevy events 已就位。
            weather_bridge::publish_weather_lifecycle_events
                .after(crate::lingtian::weather::weather_apply_to_plot_system),
            zone_environment_bridge::mark_zone_environment_dirty_for_new_clients
                .after(crate::world::weather_to_environment::weather_environment_sync_system),
            zone_environment_bridge::zone_environment_broadcast_system
                .after(zone_environment_bridge::mark_zone_environment_dirty_for_new_clients),
        ),
    );
    // plan-era-state-v1 P3 — 时代天象 S2C：监听 EraChangedEvent，按 realm gate 分档发送。
    // 排在 era 系统之后（WorldEraState + EraChangedEvent 由 world::era::register 注册）。
    app.add_systems(
        Update,
        (
            era_ambiance_emit::era_ambiance_on_era_changed_system
                .after(crate::world::era::era_decree_system),
            era_ambiance_emit::era_ambiance_on_join_system,
        ),
    );
    app.add_systems(
        Update,
        techniques_snapshot_emit::emit_join_techniques_snapshot_payloads
            .after(crate::player::attach_player_state_to_joined_clients),
    );
    app.add_systems(
        Update,
        identity_panel_emit::emit_identity_panel_state_payloads
            .after(crate::identity::command::handle_identity_command)
            .after(crate::social::apply_social_renown_deltas),
    );
    app.add_systems(
        Update,
        (
            resourcepack::prompt_resource_pack_on_join.after(send_welcome_payload_on_join),
            resourcepack::record_resource_pack_status,
            resourcepack::cleanup_disconnected_resource_pack_sessions,
        ),
    );
    app.add_systems(
        Update,
        (
            cultivation_bridge::publish_breakthrough_events,
            cultivation_bridge::publish_forge_events,
            cultivation_bridge::publish_cultivation_death_events,
            cultivation_bridge::publish_insight_requests,
            cultivation_bridge::publish_lifespan_events
                .after(crate::cultivation::lifespan::lifespan_aging_tick)
                .after(crate::combat::lifecycle::death_arbiter_tick),
            cultivation_bridge::publish_duo_she_events
                .after(crate::cultivation::possession::process_duo_she_requests),
            cultivation_bridge::publish_aging_events
                .after(crate::cultivation::lifespan::lifespan_aging_tick),
            crate::cultivation::tribulation::publish_tribulation_events,
            tribulation_broadcast_emit::emit_tribulation_broadcast_payloads,
            ascension_quota_emit::emit_ascension_quota_payloads
                .after(crate::cultivation::tribulation::tribulation_wave_system),
            tribulation_heart_demon_offer_emit::emit_heart_demon_offer_payloads,
            anqi_event_bridge::publish_carrier_charged_events,
            anqi_event_bridge::publish_carrier_impact_events,
            anqi_event_bridge::publish_projectile_despawned_events,
            woliu_event_bridge::publish_woliu_backfire_events,
            woliu_event_bridge::publish_projectile_qi_drained_events,
            zhenmai_v2_event_bridge::publish_zhenmai_skill_events,
            dugu_event_bridge::publish_dugu_poison_progress_events
                .after(crate::cultivation::dugu::dugu_poison_tick),
            tuike_event_bridge::publish_tuike_shed_events
                .after(crate::combat::resolve::resolve_attack_intents),
        ),
    );
    // Skill resolvers enqueue BurstMeridianEvent via deferred commands. The explicit
    // dependency makes Bevy insert ApplyDeferred between the request handler and this
    // exclusive bridge, so every accepted cast reaches S2C in the same Update.
    app.add_systems(
        Update,
        burst_event_emit::emit_burst_meridian_events
            .after(client_request_handler::handle_client_request_payloads),
    );
    app.add_systems(Update, dugu_event_bridge::publish_antidote_result_events);
    app.add_systems(
        Update,
        cultivation_bridge::publish_breakthrough_cinematic_events,
    );
    // plan-exploration-probe-return-v1 P0：神识感知矿脉 S2C 回执。
    app.add_systems(Update, mineral_probe_emit::emit_mineral_probe_results);
    // plan-exploration-probe-return-v1 P1：神识感知保鲜 S2C 回执。
    app.add_systems(Update, freshness_probe_emit::emit_freshness_probe_results);
    // plan-exploration-probe-return-v1 P2：修炼顿悟 S2C 回执。
    app.add_systems(
        Update,
        cultivation_insight_offer_emit::emit_cultivation_insight_offers,
    );
    // plan-halfstep-rechallenge-integration-v1 P0/P1：半步化虚重渡触发 HUD + Redis + HIDE S2C。
    app.add_systems(
        Update,
        (
            halfstep_rechallenge_emit::emit_halfstep_rechallenge_trigger.after(
                crate::cultivation::tribulation::dispatch_rechallenge_on_quota_opened_system,
            ),
            halfstep_rechallenge_emit::emit_halfstep_rechallenge_hide_on_settle,
            // plan-halfstep-rechallenge-integration-v1 P1：Redis publish → agent narration。
            halfstep_rechallenge_emit::publish_halfstep_rechallenge_to_redis.after(
                crate::cultivation::tribulation::dispatch_rechallenge_on_quota_opened_system,
            ),
        ),
    );
    app.add_systems(Update, tuike_event_bridge::publish_tuike_v2_skill_events);
    // plan-combat-skill-feedback-bridges-v1 P6：蜕壳灰烬入包 + VFX + Redis（FalseSkinDecayedToAshEvent）
    app.add_systems(Update, tuike_ash_emit::publish_tuike_ash_events);
    app.add_systems(
        Update,
        (
            poison_trait_emit::publish_poison_dose_events
                .after(crate::cultivation::poison_trait::consume_poison_pill_system),
            poison_trait_emit::publish_poison_overdose_events
                .after(crate::cultivation::poison_trait::consume_poison_pill_system),
        ),
    );
    app.add_systems(
        Update,
        poison_trait_emit::emit_poison_trait_state_payloads
            .after(crate::cultivation::poison_trait::poison_toxicity_decay_tick)
            .after(crate::cultivation::poison_trait::digestion_load_decay_tick),
    );
    app.add_systems(
        Update,
        zhenfa_v2_event_bridge::publish_zhenfa_v2_events
            .after(crate::zhenfa::ZhenfaSystemSet::Runtime),
    );
    app.add_systems(
        Update,
        (
            anqi_event_bridge::publish_multi_shot_events,
            anqi_event_bridge::publish_qi_injection_events,
            anqi_event_bridge::publish_echo_fractal_events,
            anqi_event_bridge::publish_container_events,
            woliu_event_bridge::publish_woliu_v2_cast_events,
            woliu_event_bridge::publish_woliu_v2_backfire_events,
            woliu_event_bridge::publish_woliu_v2_turbulence_events,
            // plan-combat-skill-feedback-bridges-v1 P3 — 虚蚀阶段推进 → agent 叙事桥
            woliu_event_bridge::publish_void_erosion_advance_events,
            dugu_v2_event_bridge::publish_dugu_v2_eclipse_events,
            dugu_v2_event_bridge::publish_dugu_v2_penetrate_events,
            dugu_v2_event_bridge::publish_dugu_v2_shroud_events,
            dugu_v2_event_bridge::publish_dugu_v2_self_cure_events,
            dugu_v2_event_bridge::publish_dugu_v2_reverse_events,
            // plan-combat-skill-feedback-bridges-v1 P5：永久真元上限衰减 S2C（不走 Redis）
            dugu_v2_event_bridge::publish_permanent_qi_max_decay_to_client,
            baomai_v3_event_bridge::publish_baomai_v3_skill_events
                .after(client_request_handler::handle_client_request_payloads),
            // plan-combat-skill-feedback-bridges-v1 P0 — 经脉断脉叙事+VFX 桥接
            meridian_severed_emit::publish_meridian_severed_events
                .before(vfx_event_emit::emit_vfx_event_payloads),
            // plan-combat-skill-feedback-bridges-v1 P1 — baomai_v4 反馈整桥
            baomai_v4_event_bridge::publish_scar_circuit_events,
            baomai_v4_event_bridge::push_iron_cocoon_stage_up,
            baomai_v4_event_bridge::emit_crack_reading_payload,
            baomai_v4_event_bridge::emit_resonance_lock_payloads,
        ),
    );
    // plan-combat-skill-feedback-bridges-v1 P2 — 爆脉 v3 残余事件桥（单独 add_systems 避免超长 tuple）
    app.add_systems(
        Update,
        (
            baomai_v3_event_bridge::publish_mountain_shake_event,
            baomai_v3_event_bridge::publish_blood_burn_event,
            baomai_v3_event_bridge::publish_body_transcendence_expired,
            baomai_v3_event_bridge::publish_overload_ripple_event,
        ),
    );
    app.add_systems(
        Update,
        (
            full_power_emit::emit_full_power_charging_state_payloads,
            full_power_emit::emit_full_power_charged_orb_vfx,
            full_power_emit::emit_full_power_charging_clear_payloads,
            full_power_emit::emit_full_power_release_payloads,
            full_power_emit::emit_full_power_exhausted_mist_vfx,
            full_power_emit::emit_full_power_exhausted_mist_refresh_vfx,
        )
            .after(crate::cultivation::full_power_strike::charge_tick_system)
            .after(crate::cultivation::full_power_strike::apply_full_power_attack_intent_system)
            .before(vfx_event_emit::emit_vfx_event_payloads),
    );
    // 逆脉护体的体表逆流纹：buff 存续期内跟着施法者当前位置周期重发（见 burst_meridian
    // §NI_MAI_HU_TI_AURA_REEMIT_INTERVAL_TICKS）。与上面的 full_power 持续态 VFX 同理，
    // 必须排在粒子投递之前才能当帧送达。
    app.add_systems(
        Update,
        crate::cultivation::burst_meridian::ni_mai_hu_ti_aura_vfx_tick
            .before(vfx_event_emit::emit_vfx_event_payloads),
    );
    app.add_systems(
        Update,
        (
            cultivation_detail_emit::emit_cultivation_detail_payloads,
            cultivation_detail_emit::emit_body_plan_layout_payloads,
            race_gate_meta_emit::emit_race_gate_meta_payloads,
            morph_state_emit::emit_morph_state_payloads,
            morph_state_emit::emit_morph_state_delta_payloads,
            qi_color_observed_emit::emit_qi_color_observed_payloads
                .after(client_request_handler::handle_client_request_payloads),
            audio_event_emit::handle_audio_debug_commands,
            audio_event_emit::emit_audio_play_payloads
                .after(audio_event_emit::handle_audio_debug_commands)
                .after(process_redis_inbound)
                .after(emit_event_alerts_on_major_event_creation)
                .after(emit_gameplay_narrations),
            audio_event_emit::emit_audio_stop_payloads,
            vfx_event_emit::handle_vfx_debug_commands,
            vfx_event_emit::emit_vfx_event_payloads
                .after(vfx_event_emit::handle_vfx_debug_commands),
            vfx_event_emit::emit_vanilla_vfx_particles
                .after(vfx_event_emit::handle_vfx_debug_commands),
        ),
    );
    app.add_systems(
        Update,
        (
            // plan-tsy-zone-followup-v1 §2 — TsyEnter/Exit Bevy event → bong:tsy_event
            tsy_event_bridge::publish_tsy_enter_events,
            tsy_event_bridge::publish_tsy_exit_events,
            tsy_event_bridge::publish_tsy_npc_spawned_events
                .after(crate::npc::tsy_hostile::emit_tsy_hostile_spawn_summary),
            tsy_event_bridge::publish_tsy_sentinel_phase_changed_events,
            // plan-agent-ui-data-v1 server fix — TsyZoneActivated → bong:tsy_event
            tsy_event_bridge::publish_tsy_zone_activated_events
                .after(crate::inventory::tsy_loot_spawn::tsy_loot_spawn_on_enter),
            poi_novice_bridge::publish_poi_spawned_events,
            poi_novice_bridge::publish_trespass_events,
            forge_snapshot_emit::emit_join_forge_snapshots
                .after(crate::inventory::attach_inventory_to_joined_clients),
        ),
    );
    app.add_systems(
        Update,
        (
            alchemy_bridge::publish_alchemy_session_end_events
                .after(client_request_handler::handle_client_request_payloads),
            alchemy_bridge::publish_alchemy_insight_events,
            alchemy_snapshot_emit::emit_alchemy_outcome_resolved
                .after(client_request_handler::handle_client_request_payloads),
        ),
    );
    app.add_systems(
        Update,
        cultivation_bridge::publish_rebirth_events
            .after(crate::combat::lifecycle::death_arbiter_tick),
    );
    app.add_systems(
        Update,
        (
            vfx_animation_trigger::emit_attack_animation_triggers
                .after(crate::combat::resolve::resolve_attack_intents),
            vfx_animation_trigger::emit_defense_animation_triggers
                .after(crate::combat::resolve::apply_defense_intents),
            vfx_animation_trigger::emit_hit_recoil_animation_triggers
                .after(crate::combat::resolve::resolve_attack_intents),
            vfx_animation_trigger::emit_breakthrough_animation_triggers
                .after(crate::cultivation::breakthrough::breakthrough_system),
            vfx_animation_trigger::emit_tribulation_animation_triggers
                .after(crate::cultivation::tribulation::start_tribulation_system)
                .after(crate::cultivation::tribulation::tribulation_failure_system),
            vfx_animation_trigger::emit_tribulation_settled_vfx_triggers
                .after(crate::cultivation::tribulation::juebi_settlement_system)
                .after(crate::cultivation::tribulation::tribulation_failure_system),
            vfx_animation_trigger::emit_woliu_v2_visual_triggers,
            vfx_animation_trigger::emit_woliu_v2_visual_stop_triggers,
            vfx_animation_trigger::emit_botany_harvest_visual_triggers,
            vfx_animation_trigger::emit_lingtian_visual_triggers,
            vfx_animation_trigger::emit_baomai_v3_visual_triggers,
            animation_trigger::emit_animation_trigger_components,
            vfx_animation_trigger::emit_tuike_v2_visual_triggers,
            // 必须在 heaven_gate_cast_system 之后跑：天门结算时 emit 的 HeavenGateRelease AV
            // 才能同 tick 播出，否则 charge→release 间有 1 tick(~50ms)肉眼可察抖动。
            vfx_animation_trigger::emit_sword_path_visual_triggers
                .after(crate::sword_path::skill_register::heaven_gate_cast_system),
            // 暗器六招 cast → 动画 + 粒子（纯 cosmetic，复用现有 anim/sprite 资产）。
            vfx_animation_trigger::emit_anqi_visual_triggers,
            // 蛊道两招（凝针 / 灌毒蛊）cast → 动画 + 粒子（纯 cosmetic，复用现有资产）。
            vfx_animation_trigger::emit_dugu_needle_visual_triggers,
            // 蛊道 v2 五招 cast → 粒子（anim/audio 已在 skills.rs 内联 emit；此处补
            // visual.particle_id → SpawnParticle 落地，三张 dugu_* 贴图首次可见）。
            vfx_animation_trigger::emit_dugu_v2_visual_triggers,
            // 绝灵涡流（woliu v1）长驻领域 → 起手动画 + 开涡/存续/反噬粒子（lifecycle 驱动）。
            vfx_animation_trigger::emit_woliu_v1_vortex_visual_triggers,
            // 广播体操练习完成 → guard_raise 伸展姿态 + happy_villager 正反馈粒子
            //（纯 cosmetic，复用现有 anim + vanilla 粒子，无净新资产）。
            vfx_animation_trigger::emit_guangbo_ticao_visual_triggers,
        )
            .before(vfx_event_emit::emit_vfx_event_payloads),
    );
    // plan-skill-av-relink-v1 P1 — 孤儿动画接线 adapter 组（上一组 tuple 已满 20 上限，
    // 独立注册；同样约束在 emit_vfx_event_payloads 之前跑）。
    app.add_systems(
        Update,
        (
            // 激活功法（TechniqueLearnedEvent）→ 流派架势动画（stance_*）。卷轴习得
            // 路走 client_request_handler，after 保证同 tick 播出；导师传授 / 首击
            // 领悟路事件跨 tick 仍被消费（events 双缓冲）。
            vfx_animation_trigger::emit_technique_learned_stance_triggers
                .after(client_request_handler::handle_client_request_payloads),
            // 淬炼按键（TemperingHit）→ forge_hammer 抡锤动画（与 forge 模块
            // FORGE_HAMMER_STRIKE 粒子同源同 tick）。
            vfx_animation_trigger::emit_forge_tempering_animation_triggers
                .after(client_request_handler::handle_client_request_payloads),
        )
            .before(vfx_event_emit::emit_vfx_event_payloads),
    );
    // 全部 audio-trigger 系统的调度**唯一生产真源**在 `audio_trigger::register`——
    // 生产与接线门禁测试共用同一份系统清单，测试里不许再抄一遍（PR #1262 review 要求）。
    audio_trigger::register(app);
    app.add_systems(
        Update,
        (
            // plan-sword-path-complete §B — 黑武士 boss action → VFX + 音效。
            heiwushi_av_trigger::emit_heiwushi_visual_triggers
                .before(vfx_event_emit::emit_vfx_event_payloads),
            heiwushi_av_trigger::emit_heiwushi_audio_triggers,
            npc_metadata::emit_npc_metadata_payloads,
        )
            .after(audio_trigger::tick_audio_dedup_clock)
            .before(audio_event_emit::emit_audio_play_payloads)
            // loop recipe 的收尾（如低血心跳血量回升）也要同帧下发，别拖到下一帧。
            .before(audio_event_emit::emit_audio_stop_payloads),
    );
    app.add_systems(
        Update,
        (
            npc_bubble::emit_npc_reaction_bubbles
                .after(crate::combat::resolve::resolve_attack_intents),
            npc_bubble::emit_npc_bubble_payloads,
            npc_mood::emit_npc_mood_payloads,
            npc_lod_emit::emit_npc_lod_payloads,
            tsy_polish::emit_tsy_boss_health_payloads,
            tsy_polish::emit_tsy_death_vfx_payloads
                .after(crate::combat::lifecycle::death_arbiter_tick),
        ),
    );
    app.add_systems(
        Update,
        crate::cultivation::tribulation::publish_heart_demon_pregen_requests
            .after(crate::cultivation::tribulation::start_tribulation_system),
    );
    app.add_systems(
        Update,
        tribulation_state_emit::emit_tribulation_state_payloads
            .after(crate::cultivation::tribulation::tribulation_wave_system),
    );
    // fix-spec-1901-v2 §4.5 — lingtian C2S 入口排进 `LingtianRequestIngressSet`：
    // 只入队，不读权威位置；post-transfer validator 排在其后（见 lingtian::register
    // 的 chain：ingress → AuthoritativePositionCommitSet → validator）。
    register_lingtian_ingress_wiring(app);
    // plan-scroll-reading-v1 P2 §8.1 #4 — 读卷循环动画死亡/断线兜底清理（模板：
    // combat::shield_block::cleanup_shield_on_{death,disconnect}）。死亡分支需在
    // death_arbiter_tick 之后（DeathEvent 已 emit）；断线分支需在
    // despawn_disconnected_clients 之前（entity 尚未被 despawn，marker 仍可查）。
    app.add_systems(
        Update,
        scroll_open_emit::cleanup_scroll_reading_on_death
            .after(crate::combat::lifecycle::death_arbiter_tick),
    );
    app.add_systems(
        Update,
        scroll_open_emit::cleanup_scroll_reading_on_disconnect
            .before(crate::player::despawn_disconnected_clients),
    );
    app.add_systems(
        Update,
        qi_attrition_emit::emit_qi_attrition_payloads
            .after(client_request_handler::handle_client_request_payloads),
    );
    app.add_systems(
        Update,
        skill_config_emit::emit_skill_config_snapshots
            .after(crate::player::attach_player_state_to_joined_clients)
            .after(client_request_handler::handle_client_request_payloads),
    );
    app.add_systems(
        Update,
        crate::alchemy::apply_alchemy_explode_outcomes
            .after(client_request_handler::handle_client_request_payloads),
    );
    // ── plan-craft-v1 P2/P3：通用手搓 IPC（client_request → intent → session → outcome
    //    + 三渠道解锁 intent → unlock_via_* → RecipeUnlocked）──
    register_craft_start_runtime_system(app);
    app.add_systems(
        Update,
        (
            craft_emit::apply_craft_cancel_intents.after(craft_emit::apply_craft_start_intents),
            craft_emit::apply_unlock_intents
                .after(client_request_handler::handle_client_request_payloads),
            craft_emit::tick_craft_sessions.after(craft_emit::apply_craft_cancel_intents),
            craft_emit::persist_dirty_craft_sessions
                .after(craft_emit::apply_craft_start_intents)
                .after(craft_emit::apply_craft_cancel_intents)
                .after(craft_emit::tick_craft_sessions)
                .before(crate::player::despawn_disconnected_clients),
            craft_emit::emit_craft_session_state.after(craft_emit::persist_dirty_craft_sessions),
            craft_emit::emit_craft_outcome_payloads.after(craft_emit::persist_dirty_craft_sessions),
            craft_emit::emit_recipe_unlocked_payloads.after(craft_emit::apply_unlock_intents),
            craft_emit::emit_recipe_list_on_join
                .after(crate::inventory::attach_inventory_to_joined_clients),
            // plan-craft-material-discovery：持有任一原料被动解锁空源配方 + 重推列表
            craft_emit::apply_material_discovery_unlock
                .after(crate::inventory::attach_inventory_to_joined_clients),
            // P3 server → agent Redis bridge
            craft_event_bridge::publish_craft_completed_to_redis
                .after(craft_emit::persist_dirty_craft_sessions),
            craft_event_bridge::publish_craft_failed_to_redis
                .after(craft_emit::apply_craft_cancel_intents)
                .after(craft_emit::persist_dirty_craft_sessions),
            craft_event_bridge::publish_recipe_unlocked_to_redis
                .after(craft_emit::apply_unlock_intents),
        ),
    );
    // Separate add_systems call to avoid Bevy 0.14 tuple-arity limit.
    app.add_systems(Update, derived_attrs_emit::emit_derived_attrs_sync_payloads);
    app.add_systems(
        Update,
        false_skin_state_emit::emit_false_skin_state_payloads,
    );
    app.add_systems(
        Update,
        false_skin_state_emit::emit_tuike_v2_false_skin_state_payloads
            .after(crate::combat::tuike_v2::tick::sync_false_skin_stack_from_inventory),
    );
    app.add_systems(
        Update,
        yidao_state_emit::emit_yidao_hud_state_payloads
            .after(crate::combat::yidao::complete_yidao_casts),
    );
    app.add_systems(
        Update,
        yidao_state_emit::emit_healer_npc_ai_state_payloads
            .after(crate::combat::yidao::complete_yidao_casts),
    );
    app.add_systems(
        Update,
        crate::combat::yidao::complete_yidao_casts.after(cast_emit::tick_casts_or_interrupt),
    );
    app.add_systems(
        Update,
        (
            combat_hud_state_emit::emit_combat_hud_state_payloads
                .in_set(crate::combat::CombatSystemSet::Emit),
            wounds_snapshot_emit::emit_wounds_snapshot_payloads,
            // After apply_defense_intents writes incoming_window the same tick.
            defense_window_emit::emit_defense_window_payloads
                .after(crate::combat::resolve::apply_defense_intents),
            // Run after attack resolve so damage interrupts are observed same tick.
            cast_emit::tick_casts_or_interrupt
                .after(crate::combat::resolve::resolve_attack_intents)
                .after(audio_trigger::tick_audio_dedup_clock),
            // After cast tick (which sets cooldown) so client sees fresh state same frame.
            quickslot_config_emit::emit_quickslot_config_payloads
                .after(crate::combat::yidao::complete_yidao_casts),
            skillbar_config_emit::emit_skillbar_config_payloads
                .after(crate::combat::yidao::complete_yidao_casts),
            techniques_snapshot_emit::emit_techniques_snapshot_payloads,
            inventory_snapshot_emit::emit_changed_inventory_snapshots
                .after(inventory_event_emit::emit_durability_changed_inventory_events)
                .after(crate::fauna::dying_elder::dying_elder_give_dan_system)
                .after(crate::social::SocialSystemSet::TradeOfferResponse),
            inventory_snapshot_emit::emit_revive_inventory_resyncs,
            skill_snapshot_emit::emit_revive_skill_resyncs,
            inventory_event_emit::emit_dropped_item_inventory_events,
            inventory_event_emit::publish_armor_durability_changed_events
                .after(crate::combat::resolve::resolve_attack_intents),
            inventory_event_emit::emit_durability_changed_inventory_events
                .after(crate::combat::resolve::resolve_attack_intents),
            dropped_loot_sync_emit::emit_join_dropped_loot_syncs,
            dropped_loot_sync_emit::emit_changed_dropped_loot_syncs,
            // Fires on Added (join hydration) + any later mutation.
            unlocks_sync_emit::emit_unlocks_sync_payloads,
            // After resolve so we read freshly-emitted CombatEvents the same tick.
            event_stream_emit::emit_combat_events_to_event_stream
                .after(crate::combat::resolve::resolve_attack_intents),
            // plan-weapon-v1 §8：weapon equipped / broken 推送。放在 sync_weapon 之后
            // 以便 Added/Changed/Removed 能观察到本 tick sync 产生的结果。
            weapon_equipped_emit::emit_weapon_equipped_payloads,
            weapon_equipped_emit::emit_weapon_broken_payloads,
            treasure_equipped_emit::emit_treasure_equipped_payloads,
        ),
    );
    // plan-remains-suite P0：遗骸容器世界同步（独立 add_systems 避免 Bevy 20 元素 tuple 上限）。
    app.add_systems(
        Update,
        (
            remains_sync_emit::emit_join_remains_syncs,
            remains_sync_emit::emit_changed_remains_syncs,
        ),
    );
    // plan-shield-block-v1 P3：盾牌破损推送（独立 add_systems 避免 Bevy 20元素 tuple 上限）。
    app.add_systems(Update, weapon_equipped_emit::emit_shield_broken_payloads);
    // plan-botany-harvest-full-inventory-loss-v1 P1：满包掉地面 event_stream 提示
    // （同样独立 add_systems，紧邻上面 shield-block-v1 P3 先例——L895-932 的 20 元素
    // add_systems 元组已达 Bevy 0.14.2 IntoSystemConfigs tuple 上限，不得再追加）。
    app.add_systems(
        Update,
        event_stream_emit::emit_botany_harvest_overflow_to_event_stream
            .after(crate::botany::harvest::tick_harvest_sessions),
    );
    // plan-gathering-tool-bind-v1 P1：徒手割手 event_stream 提示（同上先例，独立
    // add_systems 避免 tuple 上限）。
    app.add_systems(
        Update,
        event_stream_emit::emit_botany_harvest_wound_to_event_stream
            .after(crate::botany::harvest::tick_harvest_sessions),
    );
    // plan-shield-block-v1 P4：盾格挡命中推送（材质差异化粒子+音效）。
    app.add_systems(
        Update,
        weapon_equipped_emit::emit_shield_block_hit_payloads
            .after(crate::combat::resolve::resolve_attack_intents),
    );
    app.add_systems(Update, status_snapshot_emit::emit_status_snapshot_payloads);
    app.add_systems(
        Update,
        spirit_treasure_emit::emit_spirit_treasure_state_payloads
            .after(crate::inventory::spirit_treasure::sync_spirit_treasures),
    );
    app.add_systems(
        Update,
        combat_event_emit::emit_combat_event_to_client
            .after(crate::combat::resolve::resolve_attack_intents),
    );
    app.add_systems(
        Update,
        knockback_sync_emit::emit_knockback_sync_to_client
            .after(crate::combat::resolve::resolve_attack_intents),
    );
    app.add_systems(Update, woliu_state_emit::emit_vortex_state_payloads);
    app.add_systems(
        Update,
        void_erosion_visual_emit::emit_void_erosion_visual_sync,
    );
    app.add_systems(Update, dugu_state_emit::emit_dugu_poison_state_payloads);
    // plan-combat-skill-feedback-bridges-v1 P4：暗器 HUD S2C（DecoyDeploy/QiInjection/CarrierAbrasion）
    app.add_systems(Update, anqi_hud_emit::emit_anqi_hud_payloads);
    // plan-combat-skill-feedback-bridges-v1 P6：人剑共生 HUD S2C（每秒推送 SwordBondHudState）
    app.add_systems(
        Update,
        sword_bond_state_emit::emit_sword_bond_hud_state_payloads,
    );
    app.add_systems(Update, carrier_state_emit::emit_carrier_state_payloads);
    app.add_systems(
        Update,
        (
            extract_emit::emit_rift_portal_state_payloads,
            extract_emit::emit_rift_portal_removed_payloads
                .after(crate::world::extract_system::despawn_expired_portals)
                .after(crate::world::extract_system::on_tsy_collapse_completed),
            extract_emit::emit_rift_portal_state_payloads_to_joined_clients,
            extract_emit::emit_extract_started_payloads
                .after(crate::world::extract_system::start_extract_request),
            extract_emit::emit_extract_progress_payloads
                .after(crate::world::extract_system::tick_extract_progress),
            extract_emit::emit_extract_completed_payloads
                .after(crate::world::extract_system::tick_extract_progress)
                .before(crate::world::extract_system::handle_extract_completed),
            extract_emit::emit_extract_aborted_payloads
                .after(crate::world::extract_system::tick_extract_progress)
                .after(crate::world::extract_system::cancel_extract_request),
            extract_emit::emit_extract_failed_payloads
                .after(crate::world::extract_system::tick_extract_progress)
                .before(crate::world::extract_system::handle_extract_failed),
            extract_emit::emit_tsy_collapse_portal_state_payloads
                .after(crate::world::extract_system::on_tsy_collapse_started),
            extract_emit::emit_tsy_collapse_started_payloads
                .after(crate::world::extract_system::on_tsy_collapse_started),
        ),
    );
    app.add_systems(
        Update,
        (
            tsy_container_search_emit::emit_container_state_payloads,
            tsy_container_search_emit::emit_container_state_payloads_to_joined_clients,
            tsy_container_search_emit::emit_search_started_payloads
                .after(crate::world::tsy_container_search::start_search_container),
            tsy_container_search_emit::emit_search_progress_payloads
                .after(crate::world::tsy_container_search::tick_search_progress),
            tsy_container_search_emit::emit_search_completed_payloads
                .after(crate::world::tsy_container_search::tick_search_progress),
            tsy_container_search_emit::emit_search_aborted_payloads
                .after(crate::world::tsy_container_search::tick_search_progress)
                .after(crate::world::tsy_container_search::handle_cancel_search),
        ),
    );
    // plan-offscreen-war-v1 P6：涌现冲突 resources + events。
    app.init_resource::<ZoneConflictPressure>();
    app.init_resource::<WarConflictStore>();
    app.add_event::<WarParticipateIntent>();
    app.add_event::<WarPhaseChanged>();
    // plan-offscreen-war-v1 P9：战事结算 resource（ZoneSpiritBonus 倍率表）。
    app.init_resource::<ZoneSpiritBonusStore>();

    // plan-daozhan-v1 P1 — 道伥伪装渲染 S2C payloads
    daozhan_disguise_emit::register(app);
    // plan-dying-elder-v1 B1 — 垂死大能遭遇 HUD S2C payloads
    elder_encounter_emit::register(app);
    // plan-fauna-mimic-spider-v1 P2 — 拟态蛛伪装渲染 S2C payloads
    spider_disguise_emit::register(app);
    // plan-devour-rat-model P2 — 噬元鼠吸元档位 S2C payloads（贴图变体 q0/q1/q2）
    rat_qi_tier_emit::register(app);
    // plan-devour-rat-model P3 — 噬元鼠咬击 → peck/claw/pounce 实体招式动画
    rat_av_trigger::register(app);

    app.init_resource::<cultivation_detail_emit::CultivationDetailEmitState>();
    app.init_resource::<morph_state_emit::MorphStateEmitState>();
    app.init_resource::<morph_state_emit::MorphStateEntityCache>();
    app.init_resource::<client_request_handler::AlchemyMockState>();
    app.init_resource::<audio_event_emit::AudioInstanceIdAllocator>();
    app.init_resource::<audio_trigger::AudioTriggerState>();
    app.init_resource::<npc_metadata::NpcMetadataSyncState>();
    app.init_resource::<npc_bubble::NpcBubbleSyncState>();
    app.init_resource::<npc_mood::NpcMoodSyncState>();
    app.init_resource::<npc_lod_emit::NpcLodEmitState>();
    app.init_resource::<tsy_polish::TsyBossHealthSyncState>();
    app.add_event::<audio_event_emit::PlaySoundRecipeRequest>();
    app.add_event::<audio_event_emit::StopSoundRecipeRequest>();
    app.add_event::<qi_attrition_emit::AttritionAppliedEvent>();
    app.add_event::<qi_color_observed_emit::QiColorInspectRequest>();
    app.add_event::<vfx_event_emit::VfxEventRequest>();
    app.add_event::<vfx_event_emit::VanillaVfxParticleRequest>();
    app.add_event::<crate::combat::weapon::WeaponBroken>();
    app.add_event::<crate::combat::weapon::ShieldBroken>();
    app.add_event::<crate::combat::weapon::ShieldBlockHit>();

    // ─── plan-agent-ui-data-v1 P0：天道 UI-as-Data 会话状态机 ─────────────
    app.add_event::<agent_ui::AgentUiCmdEvent>();
    app.add_event::<agent_ui::AgentUiResponseEvent>();
    app.init_resource::<agent_ui::AgentUiSessionStore>();
    app.init_resource::<agent_ui::CurrentTickResource>();
    app.add_systems(
        Update,
        (
            // increment_current_tick_system 必须在 agent_ui_tick_system /
            // receive_agent_ui_cmd_system 之前运行，使同帧看到一致的 current_tick。
            // MINOR 修复：receive_agent_ui_cmd_system 也加 .after(increment_current_tick_system)，
            // 确保 expire_tick = current_tick + timeout_ticks 与 agent_ui_tick_system 基准一致
            // （否则 cmd_system 看到 tick N-1，ticker 看到 tick N，边界偏 1）。
            agent_ui::increment_current_tick_system,
            agent_ui::receive_agent_ui_cmd_system
                .after(process_redis_inbound)
                .after(agent_ui::increment_current_tick_system),
            agent_ui::agent_ui_tick_system.after(agent_ui::increment_current_tick_system),
            agent_ui::receive_agent_ui_response_system,
            agent_ui::receive_player_disconnect_system,
        ),
    );
}

fn redis_url_from_env() -> String {
    resolve_redis_url(std::env::var(REDIS_URL_ENV_KEY).ok())
}

fn resolve_redis_url(env_value: Option<String>) -> String {
    env_value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_REDIS_URL.to_string())
}

fn redact_redis_url_for_log(redis_url: &str) -> String {
    let Some(scheme_index) = redis_url.find("://") else {
        return "[redacted redis endpoint]".to_string();
    };

    let authority_and_path = &redis_url[(scheme_index + 3)..];
    let authority = authority_and_path
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default();
    let endpoint = authority
        .rsplit_once('@')
        .map(|(_, host)| host)
        .unwrap_or(authority)
        .trim();

    if endpoint.is_empty() {
        "[redacted redis endpoint]".to_string()
    } else {
        endpoint.to_string()
    }
}

/// 额外上下文 SystemParam：将超出 Bevy 16 参数上限的两个可选 Resource 捆绑为一个参数。
///
/// plan-era-state-v1 P2：引入 WorldEraState 后函数参数超出 16 个限制，故使用此包裹体。
#[derive(SystemParam)]
struct WorldStateContextParams<'w> {
    tiandao_blind_zones: Option<Res<'w, crate::sword_path::heaven_gate::TiandaoBlindZoneRegistry>>,
    world_era_state: Option<Res<'w, WorldEraState>>,
    persistence: Option<Res<'w, crate::persistence::PersistenceSettings>>,
}

/// Periodically publish world state snapshot to Redis
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn publish_world_state_to_redis(
    redis: Res<RedisBridgeResource>,
    mut timer: valence::prelude::ResMut<WorldStateTimer>,
    clients: Query<
        (
            Entity,
            &Position,
            &Username,
            Option<&PlayerState>,
            Option<&Cultivation>,
            Option<&Anonymity>,
            Option<&Renown>,
            Option<&Relationships>,
            Option<&PlayerFactionMembership>,
        ),
        With<Client>,
    >,
    zone_registry: Option<Res<ZoneRegistry>>,
    active_events: Option<Res<ActiveEventsResource>>,
    season_state: Option<Res<WorldSeasonState>>,
    faction_store: Option<Res<FactionStore>>,
    npcs: Query<
        (
            Entity,
            &Position,
            &NpcBlackboard,
            &EntityKind,
            Option<&NpcArchetype>,
            Option<&NpcLifespan>,
            Option<&Cultivation>,
            Option<&FactionMembership>,
            Option<&NpcPatrol>,
        ),
        With<NpcMarker>,
    >,
    mut dormant_store: Option<ResMut<NpcDormantStore>>,
    rat_density_q: Query<(&RatBlackboard, &RatPhase), With<NpcMarker>>,
    flee_actions: Query<(&Actor, &ActionState), With<FleeAction>>,
    chase_actions: Query<(&Actor, &ActionState), With<ChaseAction>>,
    melee_actions: Query<(&Actor, &ActionState), With<MeleeAttackAction>>,
    dash_actions: Query<(&Actor, &ActionState), With<DashAction>>,
    cultivation_q: Query<
        (Entity, &Cultivation, &MeridianSystem, &QiColor, &LifeRecord),
        With<Client>,
    >,
    ctx: WorldStateContextParams<'_>,
) {
    timer.ticks += 1;
    if !timer
        .ticks
        .is_multiple_of(WORLD_STATE_PUBLISH_INTERVAL_TICKS)
    {
        return;
    }

    let npc_action_states =
        collect_npc_action_states(&flee_actions, &chase_actions, &melee_actions, &dash_actions);

    let cultivation_by_entity = collect_cultivation_snapshots(&cultivation_q);
    let rat_density_heatmap = collect_rat_density_heatmap(rat_density_q.iter());

    let state = build_world_state_snapshot(
        current_unix_timestamp_secs(),
        timer.ticks,
        &clients,
        zone_registry.as_deref(),
        active_events.as_deref(),
        season_state.as_deref().map(|state| state.current.into()),
        faction_store.as_deref(),
        &npcs,
        &npc_action_states,
        &cultivation_by_entity,
        dormant_store.as_deref(),
        rat_density_heatmap,
        ctx.tiandao_blind_zones.as_deref(),
        ctx.world_era_state.as_deref(),
    );

    let _ = redis.tx_outbound.send(RedisOutbound::WorldState(state));
    // Dormant persistence is dirty-gated and allows only one HASH replacement
    // in flight. A receipt confirms or re-arms that revision before a newer
    // snapshot may be enqueued, so background writes cannot land out of order.
    if let Some(dormant_store) = dormant_store.as_mut() {
        if let Some(settings) = ctx.persistence.as_deref() {
            dormant_store.apply_persistence_receipts_with_settings(settings);
        } else {
            dormant_store.apply_persistence_receipts();
        }
        if let Some(revision) = dormant_store.begin_persistence() {
            if let Some(settings) = ctx.persistence.as_deref() {
                if let Err(error) =
                    dormant_store.bind_unbound_terminal_tombstones(settings, revision)
                {
                    dormant_store.requeue_persistence(revision);
                    tracing::warn!(
                        "[bong][network] failed to bind dormant terminal cleanup revision {revision}: {error}"
                    );
                    return;
                }
            }
            match dormant_store.to_redis_hash_payloads() {
                Ok(entries) => {
                    tracing::debug!(
                        "[bong][network] syncing {} dormant NPC snapshots to Redis HASH",
                        dormant_store.len()
                    );
                    let outbound = RedisOutbound::NpcDormantHash {
                        entries,
                        revision,
                        receipt_tx: dormant_store.persistence_receipt_sender(),
                    };
                    if redis.tx_outbound.send(outbound).is_err() {
                        dormant_store.requeue_persistence(revision);
                    }
                }
                Err(error) => {
                    dormant_store.requeue_persistence(revision);
                    tracing::warn!(
                        "[bong][network] failed to serialize dormant NPC Redis HASH payloads: {error}"
                    );
                }
            }
        }
    }
}

/// plan-offscreen-war-v1 P0：把全服守恒账本周期性发布到 `bong:qi/ledger` HASH。
///
/// 这是 **exclusive system**（取 `&mut World`），因为 `summarize_world_qi` 需要遍历
/// 全 World（zone / player / container / ledger 四类真元分量）才能算出权威
/// `total_observed`。普通参数化 system 无法一次拿全这些来源，故走 exclusive 入口。
///
/// 节奏：与 `publish_world_state_to_redis` 同为 `WORLD_STATE_PUBLISH_INTERVAL_TICKS`，
/// 但用独立 `QiLedgerTimer` 计数（见其 doc-comment）。外部脚本可 `HGETALL bong:qi/ledger`
/// 做守恒断言：守恒总量恒定的真锚点是 `budget_initial_total`（== `DEFAULT_SPIRIT_QI_TOTAL`），
/// 而 `total_observed` 是**已落位**真元（≤ 预算，minimal 世界起服后远小于预算）。
///
/// **只读发布，零真元流动**——不调 `WorldQiAccount::transfer`，不改任何 balance。
fn publish_qi_ledger_to_redis(world: &mut bevy_ecs::world::World) {
    {
        let mut timer = world.resource_mut::<QiLedgerTimer>();
        timer.ticks += 1;
        if !timer
            .ticks
            .is_multiple_of(WORLD_STATE_PUBLISH_INTERVAL_TICKS)
        {
            return;
        }
    }

    // 没有 redis bridge（部分测试 app）时静默跳过。
    if world.get_resource::<RedisBridgeResource>().is_none() {
        return;
    }

    let snapshot = summarize_world_qi(world);
    // `summarize_world_qi` 已容忍缺失 `WorldQiAccount`（ledger_qi=0），发布路径也必须容忍：
    // 缺资源时用空账本算聚合字段（无 per-account 行），绝不 panic。
    let fields = world
        .get_resource::<WorldQiAccount>()
        .map(|accounts| build_qi_ledger_hash_fields(&snapshot, accounts))
        .unwrap_or_else(|| build_qi_ledger_hash_fields(&snapshot, &WorldQiAccount::default()));

    let redis = world.resource::<RedisBridgeResource>();
    let _ = redis.tx_outbound.send(RedisOutbound::QiLedgerHash(fields));
}

fn collect_cultivation_snapshots(
    q: &Query<(Entity, &Cultivation, &MeridianSystem, &QiColor, &LifeRecord), With<Client>>,
) -> HashMap<Entity, (CultivationSnapshotV1, LifeRecordSnapshotV1)> {
    const RECENT_BIO_N: usize = 12;
    const RECENT_SKILL_MILESTONES_N: usize = 6;
    q.iter()
        .map(|(entity, c, m, q, life)| {
            let snap = CultivationSnapshotV1::from_components(c, m, q);
            let skill_milestones = life
                .skill_milestones
                .iter()
                .rev()
                .take(RECENT_SKILL_MILESTONES_N)
                .collect::<Vec<_>>();
            let life_snap = LifeRecordSnapshotV1 {
                recent_biography_summary: life.recent_summary_text(RECENT_BIO_N),
                recent_skill_milestones_summary: life
                    .recent_skill_milestones_summary_text(RECENT_SKILL_MILESTONES_N),
                skill_milestones: skill_milestones
                    .into_iter()
                    .rev()
                    .map(SkillMilestoneSnapshotV1::from_runtime)
                    .collect(),
            };
            (entity, (snap, life_snap))
        })
        .collect()
}

fn publish_season_changed_events(
    redis: Res<RedisBridgeResource>,
    mut events: EventReader<SeasonChangedEvent>,
    season_state: Option<Res<WorldSeasonState>>,
    zone_registry: Option<Res<ZoneRegistry>>,
    clock: Option<Res<CombatClock>>,
    terrain_providers: Option<Res<TerrainProviders>>,
    mut clients: Query<PlayerStateEmitQueryItem<'_>, With<Client>>,
) {
    let zone_registry = effective_zone_registry(zone_registry.as_deref());
    let tick = clock.as_deref().map(|clock| clock.tick).unwrap_or_default();
    let terrain_providers = terrain_providers.as_deref();

    for event in events.read() {
        let state = season_state
            .as_deref()
            .map(|state| state.current)
            .unwrap_or_else(|| query_season("", event.tick));
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::SeasonChanged(SeasonChangedV1::new(
                *event, state,
            )));

        let season_state_for_client: SeasonStateV1 = state.into();
        for (
            entity,
            mut client,
            username,
            position,
            current_dimension,
            player_state,
            cultivation,
            anonymity,
            renown,
            relationships,
            faction_membership,
        ) in &mut clients
        {
            send_player_state_payload_to_client(
                entity,
                &mut client,
                username,
                position,
                current_dimension,
                player_state,
                cultivation,
                anonymity,
                renown,
                relationships,
                faction_membership,
                &zone_registry,
                terrain_providers,
                tick,
                season_state_for_client,
                "season_changed",
            );
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn build_world_state_snapshot(
    ts: u64,
    tick: u64,
    clients: &Query<
        (
            Entity,
            &Position,
            &Username,
            Option<&PlayerState>,
            Option<&Cultivation>,
            Option<&Anonymity>,
            Option<&Renown>,
            Option<&Relationships>,
            Option<&PlayerFactionMembership>,
        ),
        With<Client>,
    >,
    zone_registry: Option<&ZoneRegistry>,
    active_events: Option<&ActiveEventsResource>,
    season_state: Option<SeasonStateV1>,
    faction_store: Option<&FactionStore>,
    npcs: &Query<
        (
            Entity,
            &Position,
            &NpcBlackboard,
            &EntityKind,
            Option<&NpcArchetype>,
            Option<&NpcLifespan>,
            Option<&Cultivation>,
            Option<&FactionMembership>,
            Option<&NpcPatrol>,
        ),
        With<NpcMarker>,
    >,
    npc_action_states: &HashMap<Entity, NpcStateKind>,
    cultivation_by_entity: &HashMap<Entity, (CultivationSnapshotV1, LifeRecordSnapshotV1)>,
    dormant_store: Option<&NpcDormantStore>,
    rat_density_heatmap: RatDensityHeatmapV1,
    tiandao_blind_zones: Option<&crate::sword_path::heaven_gate::TiandaoBlindZoneRegistry>,
    world_era_state: Option<&WorldEraState>,
) -> WorldStateV1 {
    use crate::schema::world_state::{EraStateV1 as IpcEraStateV1, EraTypeV1Wire};

    let zone_registry = effective_zone_registry(zone_registry);
    let (players, player_ids_by_entity, player_counts_by_zone) = collect_player_snapshots(
        tick,
        clients,
        &zone_registry,
        cultivation_by_entity,
        tiandao_blind_zones,
    );

    // plan-era-state-v1 P2：从 WorldEraState Resource 填充 era 字段。
    // Unknown 时代（或无 Resource）不填充（None → skip_serializing_if）。
    let era = world_era_state.and_then(|era_state| {
        if era_state.era == crate::world::era::EraType::Unknown {
            None
        } else {
            Some(IpcEraStateV1 {
                era_type: EraTypeV1Wire::from(era_state.era),
                intensity: era_state.intensity,
                onset_tick: era_state.onset_tick,
            })
        }
    });

    WorldStateV1 {
        v: 1,
        ts,
        tick,
        season_state: season_state.unwrap_or_else(|| query_season("", tick).into()),
        players,
        npcs: collect_npc_snapshots(
            npcs,
            npc_action_states,
            &player_ids_by_entity,
            &zone_registry,
            dormant_store,
        ),
        factions: faction_store.map(collect_faction_summaries),
        rat_density_heatmap,
        zones: collect_zone_snapshots(&zone_registry, &player_counts_by_zone),
        recent_events: active_events
            .map(ActiveEventsResource::recent_events_snapshot)
            .unwrap_or_default(),
        era,
    }
}

#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn build_player_state_payload(
    player_state: &PlayerState,
    cultivation: &Cultivation,
    zone: impl Into<String>,
) -> Result<Vec<u8>, PayloadBuildError> {
    let payload = player_state.server_payload_with_social_and_local_pressure(
        cultivation,
        None,
        zone.into(),
        None,
        None,
        None,
    );
    serialize_server_data_payload(&payload)
}

#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn collect_players_for_world_state<'a, I>(
    clients: I,
    zone_registry: &ZoneRegistry,
) -> (Vec<PlayerProfile>, HashMap<String, u32>)
where
    I: IntoIterator<
        Item = (
            &'a str,
            valence::prelude::Uuid,
            valence::prelude::DVec3,
            Option<&'a PlayerState>,
        ),
    >,
{
    let mut player_counts_by_zone = HashMap::new();
    let mut players = clients
        .into_iter()
        .map(|(name, _uuid, position, player_state)| {
            let zone_name = zone_name_for_position(zone_registry, position);
            let (composite_power, breakdown) = player_state
                .map(|state| {
                    // Test helper only has PlayerState; use default cultivation for scoring.
                    let cultivation = Cultivation::default();
                    (
                        state.normalized().composite_power(&cultivation),
                        state.normalized().power_breakdown(&cultivation),
                    )
                })
                .unwrap_or_else(|| {
                    let default_state = PlayerState::default();
                    let cultivation = Cultivation::default();
                    (
                        default_state.normalized().composite_power(&cultivation),
                        default_state.normalized().power_breakdown(&cultivation),
                    )
                });

            *player_counts_by_zone.entry(zone_name.clone()).or_default() += 1;

            PlayerProfile {
                uuid: canonical_player_id(name),
                name: name.to_string(),
                // Test helper only receives PlayerState, not live Cultivation.
                // Runtime world-state emission uses the real cultivation snapshot path.
                realm: crate::schema::cultivation::realm_to_string(
                    crate::cultivation::components::Realm::Awaken,
                )
                .to_string(),
                composite_power,
                breakdown,
                trend: PlayerTrend::Stable,
                active_hours: DEFAULT_PLAYER_ACTIVE_HOURS,
                zone: zone_name,
                pos: vec3_to_array(position),
                recent_kills: DEFAULT_PLAYER_RECENT_KILLS,
                recent_deaths: DEFAULT_PLAYER_RECENT_DEATHS,
                cultivation: None,
                life_record: None,
                social: None,
            }
        })
        .collect::<Vec<_>>();

    players.sort_by(|left, right| left.uuid.cmp(&right.uuid));

    (players, player_counts_by_zone)
}

fn emit_gameplay_narrations(
    zone_registry: Option<Res<ZoneRegistry>>,
    gameplay_narrations: Option<valence::prelude::ResMut<PendingGameplayNarrations>>,
    mut clients: Query<(Entity, &mut Client, &Username, &Position), With<Client>>,
    audio_events: Option<ResMut<Events<audio_event_emit::PlaySoundRecipeRequest>>>,
) {
    let Some(mut gameplay_narrations) = gameplay_narrations else {
        return;
    };

    let narrations = gameplay_narrations.drain();
    if narrations.is_empty() {
        return;
    }

    let mut audio_events = audio_events;
    process_agent_narrations(
        &mut clients,
        zone_registry.as_deref(),
        audio_events.as_deref_mut(),
        narrations.as_slice(),
    );
}

fn enqueue_duo_she_warning_narrations(
    mut warnings: EventReader<DuoSheWarningEvent>,
    pending_narrations: Option<ResMut<PendingGameplayNarrations>>,
) {
    let Some(mut pending_narrations) = pending_narrations else {
        return;
    };

    for warning in warnings.read() {
        pending_narrations.push_player(
            warning.target_id.as_str(),
            format!(
                "夺舍业记已成：{} 于本劫夺取此身，真名公开。",
                warning.host_id
            ),
            NarrationStyle::SystemWarning,
        );
    }
}

fn effective_zone_registry(zone_registry: Option<&ZoneRegistry>) -> ZoneRegistry {
    match zone_registry {
        Some(zone_registry) if !zone_registry.zones.is_empty() => zone_registry.clone(),
        _ => ZoneRegistry::fallback(),
    }
}

#[allow(clippy::type_complexity)]
fn collect_player_snapshots(
    tick: u64,
    clients: &Query<
        (
            Entity,
            &Position,
            &Username,
            Option<&PlayerState>,
            Option<&Cultivation>,
            Option<&Anonymity>,
            Option<&Renown>,
            Option<&Relationships>,
            Option<&PlayerFactionMembership>,
        ),
        With<Client>,
    >,
    zone_registry: &ZoneRegistry,
    cultivation_by_entity: &HashMap<Entity, (CultivationSnapshotV1, LifeRecordSnapshotV1)>,
    tiandao_blind_zones: Option<&crate::sword_path::heaven_gate::TiandaoBlindZoneRegistry>,
) -> (
    Vec<PlayerProfile>,
    HashMap<Entity, String>,
    HashMap<String, u32>,
) {
    let mut player_ids_by_entity = HashMap::new();
    let mut player_counts_by_zone = HashMap::new();

    let mut players = clients
        .iter()
        .filter(|(_, position, _, _, _, _, _, _, _)| {
            // plan-sword-path-v2 P2.3: 化虚一击后玩家被天道盲区遮蔽 5 min，
            // 此期间不向 agent 推送其 snapshot——agent 看不见、查不到、推演不到。
            // 守 worldview §八 天道感应 + plan §techniques::heaven_gate。
            !matches!(
                tiandao_blind_zones,
                Some(registry) if registry.is_player_hidden(position.get())
            )
        })
        .map(
            |(
                entity,
                position,
                username,
                player_state,
                cultivation,
                anonymity,
                renown,
                relationships,
                faction_membership,
            )| {
                let name = username.0.clone();
                let zone_name = zone_name_for_position(zone_registry, position.get());
                let canonical_id = canonical_player_id(&name);
                let state = player_state.cloned().unwrap_or_default();

                // For player list summary, prefer live Cultivation on the entity when
                // available (tests attach it directly). Fall back to periodic
                // cultivation snapshots, then to defaults.

                // Player list summary uses the best cultivation snapshot we have at
                // the time of building WorldState. Fall back to default cultivation.
                let (realm, composite_power, breakdown) = if let Some(cultivation) = cultivation {
                    (
                        realm_to_string(cultivation.realm).to_string(),
                        state.composite_power(cultivation),
                        state.power_breakdown(cultivation),
                    )
                } else {
                    cultivation_by_entity
                        .get(&entity)
                        .map(|(cultivation_snapshot, _)| {
                            let cultivation = Cultivation {
                                realm: realm_from_string(cultivation_snapshot.realm.as_str()),
                                qi_current: cultivation_snapshot.qi_current,
                                qi_max: cultivation_snapshot.qi_max,
                                ..Cultivation::default()
                            };
                            (
                                cultivation_snapshot.realm.clone(),
                                state.composite_power(&cultivation),
                                state.power_breakdown(&cultivation),
                            )
                        })
                        .unwrap_or_else(|| {
                            let cultivation = Cultivation::default();
                            (
                                realm_to_string(cultivation.realm).to_string(),
                                state.composite_power(&cultivation),
                                state.power_breakdown(&cultivation),
                            )
                        })
                };

                player_ids_by_entity.insert(entity, canonical_id.clone());
                *player_counts_by_zone.entry(zone_name.clone()).or_default() += 1;

                let (cultivation, life_record) = cultivation_by_entity
                    .get(&entity)
                    .cloned()
                    .map(|(c, l)| (Some(c), Some(l)))
                    .unwrap_or((None, None));

                PlayerProfile {
                    uuid: canonical_id,
                    name,
                    realm,
                    composite_power,
                    breakdown,
                    trend: PlayerTrend::Stable,
                    active_hours: DEFAULT_PLAYER_ACTIVE_HOURS,
                    zone: zone_name,
                    pos: vec3_to_array(position.get()),
                    recent_kills: DEFAULT_PLAYER_RECENT_KILLS,
                    recent_deaths: DEFAULT_PLAYER_RECENT_DEATHS,
                    cultivation,
                    life_record,
                    social: build_player_social_snapshot(
                        tick,
                        anonymity,
                        renown,
                        relationships,
                        faction_membership,
                    ),
                }
            },
        )
        .collect::<Vec<_>>();

    players.sort_by(|left, right| left.uuid.cmp(&right.uuid));

    (players, player_ids_by_entity, player_counts_by_zone)
}

fn build_player_social_snapshot(
    tick: u64,
    anonymity: Option<&Anonymity>,
    renown: Option<&Renown>,
    relationships: Option<&Relationships>,
    faction_membership: Option<&PlayerFactionMembership>,
) -> Option<PlayerSocialSnapshotV1> {
    if anonymity.is_none()
        && renown.is_none()
        && relationships.is_none()
        && faction_membership.is_none()
    {
        return None;
    }

    let renown = renown
        .map(|renown| RenownSnapshotV1 {
            fame: renown.fame,
            notoriety: renown.notoriety,
            top_tags: renown.top_tags(tick, 5),
        })
        .unwrap_or_default();
    let relationships = relationships
        .map(|relationships| {
            relationships
                .edges
                .iter()
                .map(|relationship| RelationshipSnapshotV1 {
                    kind: relationship.kind,
                    peer: relationship.peer.clone(),
                    since_tick: relationship.since_tick,
                    metadata: relationship.metadata.clone(),
                })
                .collect()
        })
        .unwrap_or_default();
    let exposed_to_count = anonymity
        .map(|anonymity| anonymity.exposed_to.len().min(u32::MAX as usize) as u32)
        .unwrap_or_default();
    let faction_membership = faction_membership.map(|membership| FactionMembershipSnapshotV1 {
        faction: membership.faction.as_str().to_string(),
        rank: membership.rank,
        loyalty: membership.loyalty,
        betrayal_count: membership.betrayal_count,
        invite_block_until_tick: membership.invite_block_until_tick,
        permanently_refused: membership.permanently_refused,
    });

    Some(PlayerSocialSnapshotV1 {
        renown,
        relationships,
        exposed_to_count,
        faction_membership,
    })
}

#[allow(clippy::type_complexity)]
fn collect_npc_snapshots(
    npcs: &Query<
        (
            Entity,
            &Position,
            &NpcBlackboard,
            &EntityKind,
            Option<&NpcArchetype>,
            Option<&NpcLifespan>,
            Option<&Cultivation>,
            Option<&FactionMembership>,
            Option<&NpcPatrol>,
        ),
        With<NpcMarker>,
    >,
    npc_action_states: &HashMap<Entity, NpcStateKind>,
    player_ids_by_entity: &HashMap<Entity, String>,
    zone_registry: &ZoneRegistry,
    dormant_store: Option<&NpcDormantStore>,
) -> Vec<NpcSnapshot> {
    let mut npc_snapshots = npcs
        .iter()
        .map(
            |(
                entity,
                position,
                blackboard,
                kind,
                archetype,
                lifespan,
                cultivation,
                faction_membership,
                patrol,
            )| {
                NpcSnapshot {
                    id: canonical_npc_id(entity),
                    kind: format!("{kind:?}"),
                    zone: patrol
                        .map(|patrol| patrol.home_zone.clone())
                        .unwrap_or_else(|| zone_name_for_position(zone_registry, position.get())),
                    pos: vec3_to_array(position.get()),
                    state: npc_action_states
                        .get(&entity)
                        .cloned()
                        .unwrap_or(NpcStateKind::Idle),
                    blackboard: build_npc_blackboard(blackboard, player_ids_by_entity),
                    digest: build_npc_digest(
                        archetype.copied(),
                        lifespan.copied(),
                        faction_membership.cloned(),
                        cultivation,
                        Some(position.get()),
                    ),
                }
            },
        )
        .collect::<Vec<_>>();

    if let Some(dormant_store) = dormant_store {
        npc_snapshots.extend(
            dormant_store
                .sorted_snapshots()
                .into_iter()
                .map(|snapshot| {
                    let mut blackboard = HashMap::new();
                    blackboard.insert("dormant".to_string(), serde_json::json!(true));
                    blackboard.insert(
                        "qi_ledger_net".to_string(),
                        serde_json::json!(snapshot.qi_ledger_net),
                    );
                    NpcSnapshot {
                        id: snapshot.char_id.clone(),
                        kind: format!("dormant:{}", snapshot.archetype.as_str()),
                        zone: snapshot.zone_name.clone(),
                        pos: snapshot.position,
                        state: NpcStateKind::Idle,
                        blackboard,
                        digest: Some(NpcDigestV1 {
                            archetype: snapshot.archetype.as_str().to_string(),
                            age_band: age_band_for_ratio(snapshot.lifespan.age_ratio()).to_string(),
                            age_ratio: snapshot.lifespan.age_ratio().clamp(0.0, 1.0),
                            realm: Some(snapshot.realm_label()),
                            faction_id: snapshot.faction_id_label(),
                            position: Some(snapshot.position),
                            disciple: snapshot.faction.clone().map(build_disciple_summary),
                        }),
                    }
                }),
        );
    }

    npc_snapshots.sort_by(|left, right| left.id.cmp(&right.id));

    npc_snapshots
}

fn build_npc_digest(
    archetype: Option<NpcArchetype>,
    lifespan: Option<NpcLifespan>,
    faction_membership: Option<FactionMembership>,
    cultivation: Option<&Cultivation>,
    position: Option<DVec3>,
) -> Option<NpcDigestV1> {
    let archetype = archetype?;
    let lifespan = lifespan?;
    let age_ratio = lifespan.age_ratio().clamp(0.0, 1.0);
    let age_band = age_band_for_ratio(age_ratio);

    Some(NpcDigestV1 {
        archetype: archetype.as_str().to_string(),
        age_band: age_band.to_string(),
        age_ratio,
        realm: cultivation.map(|cultivation| realm_to_string(cultivation.realm).to_string()),
        faction_id: faction_membership
            .as_ref()
            .map(|membership| membership.faction_id),
        position: position.map(vec3_to_array),
        disciple: faction_membership.map(build_disciple_summary),
    })
}

fn age_band_for_ratio(age_ratio: f64) -> &'static str {
    if age_ratio >= 1.0 {
        "expired"
    } else if age_ratio >= 0.8 {
        "elder"
    } else if age_ratio >= 0.4 {
        "adult"
    } else {
        "young"
    }
}

fn collect_faction_summaries(faction_store: &FactionStore) -> Vec<FactionSummaryV1> {
    faction_store
        .iter()
        .map(|faction| FactionSummaryV1 {
            id: faction.id,
            loyalty_bias: faction.loyalty_bias,
            leader_lineage: faction.leader_lineage.as_ref().map(build_lineage_summary),
            mission_queue: build_mission_queue_summary(&faction.mission_queue),
        })
        .collect()
}

fn build_disciple_summary(membership: FactionMembership) -> DiscipleSummaryV1 {
    DiscipleSummaryV1 {
        faction_id: membership.faction_id,
        rank: membership.rank,
        loyalty: membership.reputation.loyalty(),
        lineage: membership.lineage.as_ref().map(build_lineage_summary),
        mission_queue: build_mission_queue_summary(&membership.mission_queue),
    }
}

fn build_lineage_summary(lineage: &Lineage) -> LineageSummaryV1 {
    LineageSummaryV1 {
        master_id: lineage.master_id.clone(),
        disciple_count: lineage.disciple_count(),
    }
}

fn build_mission_queue_summary(mission_queue: &MissionQueue) -> Option<MissionQueueSummaryV1> {
    let pending_count = mission_queue.pending_count();
    if pending_count == 0 {
        None
    } else {
        Some(MissionQueueSummaryV1 {
            pending_count,
            top_mission_id: mission_queue.top_mission_id().map(ToString::to_string),
        })
    }
}

fn collect_zone_snapshots(
    zone_registry: &ZoneRegistry,
    player_counts_by_zone: &HashMap<String, u32>,
) -> Vec<ZoneSnapshot> {
    let mut zones = zone_registry
        .zones
        .iter()
        .map(|zone| ZoneSnapshot {
            name: zone.name.clone(),
            spirit_qi: zone.spirit_qi,
            danger_level: zone.danger_level,
            status: zone_status(zone),
            active_events: zone.active_events.clone(),
            player_count: player_counts_by_zone
                .get(&zone.name)
                .copied()
                .unwrap_or_default(),
        })
        .collect::<Vec<_>>();

    zones.sort_by(|left, right| left.name.cmp(&right.name));

    zones
}

fn zone_status(zone: &Zone) -> ZoneStatusV1 {
    if zone
        .active_events
        .iter()
        .any(|event| event == EVENT_REALM_COLLAPSE)
    {
        ZoneStatusV1::Collapsed
    } else if zone
        .active_events
        .iter()
        .any(|event| event == EVENT_TSY_RACE_OUT)
    {
        ZoneStatusV1::RaceOut
    } else {
        ZoneStatusV1::Normal
    }
}

fn collect_npc_action_states(
    flee_actions: &Query<(&Actor, &ActionState), With<FleeAction>>,
    chase_actions: &Query<(&Actor, &ActionState), With<ChaseAction>>,
    melee_actions: &Query<(&Actor, &ActionState), With<MeleeAttackAction>>,
    dash_actions: &Query<(&Actor, &ActionState), With<DashAction>>,
) -> HashMap<Entity, NpcStateKind> {
    let mut states = HashMap::new();

    // Lower priority first, higher priority overwrites.
    for (Actor(entity), action_state) in chase_actions.iter() {
        if matches!(action_state, ActionState::Executing) {
            states.insert(*entity, NpcStateKind::Patrolling);
        }
    }
    for (Actor(entity), action_state) in flee_actions.iter() {
        if matches!(action_state, ActionState::Executing) {
            states.insert(*entity, NpcStateKind::Fleeing);
        }
    }
    for (Actor(entity), action_state) in dash_actions.iter() {
        if matches!(action_state, ActionState::Executing) {
            states.insert(*entity, NpcStateKind::Attacking);
        }
    }
    for (Actor(entity), action_state) in melee_actions.iter() {
        if matches!(action_state, ActionState::Executing) {
            states.insert(*entity, NpcStateKind::Attacking);
        }
    }

    states
}

fn build_npc_blackboard(
    blackboard: &NpcBlackboard,
    player_ids_by_entity: &HashMap<Entity, String>,
) -> HashMap<String, serde_json::Value> {
    let mut snapshot = HashMap::new();

    if let Some(nearest_player) = blackboard.nearest_player {
        if let Some(player_id) = player_ids_by_entity.get(&nearest_player) {
            snapshot.insert(
                "nearest_player".to_string(),
                serde_json::Value::String(player_id.clone()),
            );
        }
    }

    snapshot
}

fn zone_name_for_position(
    zone_registry: &ZoneRegistry,
    position: valence::prelude::DVec3,
) -> String {
    zone_registry
        .find_zone(crate::world::dimension::DimensionKind::Overworld, position)
        .map(|zone| zone.name.clone())
        .unwrap_or_else(|| DEFAULT_SPAWN_ZONE_NAME.to_string())
}

fn current_unix_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn vec3_to_array(position: valence::prelude::DVec3) -> [f64; 3] {
    [position.x, position.y, position.z]
}

type PlayerStateEmitQueryItem<'a> = (
    Entity,
    &'a mut Client,
    &'a Username,
    &'a Position,
    Option<&'a CurrentDimension>,
    &'a PlayerState,
    &'a Cultivation,
    Option<&'a Anonymity>,
    Option<&'a Renown>,
    Option<&'a Relationships>,
    Option<&'a PlayerFactionMembership>,
);

type PlayerStateEmitQueryFilter = (
    With<Client>,
    Or<(
        Added<PlayerState>,
        Changed<PlayerState>,
        Added<Cultivation>,
        Changed<Cultivation>,
        Added<Anonymity>,
        Changed<Anonymity>,
        Added<Renown>,
        Changed<Renown>,
        Added<Relationships>,
        Changed<Relationships>,
        Added<PlayerFactionMembership>,
        Changed<PlayerFactionMembership>,
    )>,
);

fn emit_player_state_payloads(
    zone_registry: Option<Res<ZoneRegistry>>,
    clock: Option<Res<CombatClock>>,
    terrain_providers: Option<Res<TerrainProviders>>,
    season_state: Option<Res<WorldSeasonState>>,
    mut clients: Query<PlayerStateEmitQueryItem<'_>, PlayerStateEmitQueryFilter>,
) {
    let zone_registry = effective_zone_registry(zone_registry.as_deref());
    let tick = clock.as_deref().map(|clock| clock.tick).unwrap_or_default();
    let terrain_providers = terrain_providers.as_deref();
    let season_state_for_client: SeasonStateV1 = season_state
        .as_deref()
        .map(|state| state.current.into())
        .unwrap_or_else(|| query_season("", tick).into());

    for (
        entity,
        mut client,
        username,
        position,
        current_dimension,
        player_state,
        cultivation,
        anonymity,
        renown,
        relationships,
        faction_membership,
    ) in &mut clients
    {
        send_player_state_payload_to_client(
            entity,
            &mut client,
            username,
            position,
            current_dimension,
            player_state,
            cultivation,
            anonymity,
            renown,
            relationships,
            faction_membership,
            &zone_registry,
            terrain_providers,
            tick,
            season_state_for_client,
            "component_changed",
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn send_player_state_payload_to_client(
    entity: Entity,
    client: &mut Client,
    username: &Username,
    position: &Position,
    current_dimension: Option<&CurrentDimension>,
    player_state: &PlayerState,
    cultivation: &Cultivation,
    anonymity: Option<&Anonymity>,
    renown: Option<&Renown>,
    relationships: Option<&Relationships>,
    faction_membership: Option<&PlayerFactionMembership>,
    zone_registry: &ZoneRegistry,
    terrain_providers: Option<&TerrainProviders>,
    tick: u64,
    season_state_for_client: SeasonStateV1,
    reason: &'static str,
) {
    let zone_name = zone_name_for_position(zone_registry, position.get());
    let social =
        build_player_social_snapshot(tick, anonymity, renown, relationships, faction_membership);
    let local_neg_pressure = local_neg_pressure_at(terrain_providers, current_dimension, position);
    // plan-wire-format-bridge-v1 P3/RC6：此前 zone_spirit_qi 在 proto 里根本不存在，
    // client PlayerStateViewModel.zoneSpiritQiNormalized() 恒为 NaN 归一化默认值。
    let zone_spirit_qi = zone_registry
        .find_zone_by_name(zone_name.as_str())
        .map(|zone| zone.spirit_qi);
    let mut payload = player_state.server_payload_with_social_and_local_pressure(
        cultivation,
        Some(canonical_player_id(username.0.as_str())),
        zone_name,
        social,
        local_neg_pressure,
        zone_spirit_qi,
    );
    if let ServerDataPayloadV1::PlayerState { season_state, .. } = &mut payload.payload {
        *season_state = Some(season_state_for_client);
    }
    let payload_type = payload_type_label(payload.payload_type());
    let payload_bytes = match serialize_server_data_payload(&payload) {
        Ok(payload) => payload,
        Err(error) => {
            log_payload_build_error(payload_type, &error);
            return;
        }
    };

    send_server_data_payload(client, payload_bytes.as_slice());
    // trace：player_state 周期推送 ~每 tick/玩家，INFO 会把整份 server log 刷成
    // 噪声墙（2026-07-06 playtest：10s 挂机 258 行，取证全靠 grep -v）。
    tracing::trace!(
        "[bong][network] sent {} {} payload to client entity {entity:?} for `{}` ({reason})",
        SERVER_DATA_CHANNEL,
        payload_type,
        username.0,
    );
}

fn local_neg_pressure_at(
    terrain_providers: Option<&TerrainProviders>,
    current_dimension: Option<&CurrentDimension>,
    position: &Position,
) -> Option<f32> {
    let providers = terrain_providers?;
    let dimension = current_dimension
        .map(|dimension| dimension.0)
        .unwrap_or(DimensionKind::Overworld);
    let provider = providers.for_dimension(dimension)?;
    let sample = provider.sample(position.0.x.floor() as i32, position.0.z.floor() as i32);
    local_neg_pressure_from_sample(sample.neg_pressure, sample.portal_anchor_sdf)
}

fn local_neg_pressure_from_sample(neg_pressure: f32, portal_anchor_sdf: f32) -> Option<f32> {
    if neg_pressure <= 0.0
        || portal_anchor_sdf > crate::cultivation::neg_pressure::HOTSPOT_RADIUS_BLOCKS
    {
        return None;
    }
    Some(-neg_pressure.clamp(0.0, 1.0))
}

type ZoneInfoClientItem<'a> = (
    Entity,
    &'a mut Client,
    &'a Position,
    Option<&'a Cultivation>,
    Option<&'a UnlockedPerceptions>,
);
type ZoneInfoClientFilter = With<Client>;

fn emit_zone_info_on_zone_transition(
    zone_registry: Option<Res<ZoneRegistry>>,
    mut tracker: valence::prelude::ResMut<ZoneTransitionTracker>,
    mut clients: Query<ZoneInfoClientItem<'_>, ZoneInfoClientFilter>,
) {
    let zone_registry = effective_zone_registry(zone_registry.as_deref());
    let mut live_entities = HashSet::new();

    for (entity, mut client, position, cultivation, perceptions) in &mut clients {
        live_entities.insert(entity);

        let zone_name = zone_name_for_position(&zone_registry, position.get());
        let previous_zone = tracker.last_zone_by_entity.get(&entity).cloned();
        let transitioned = previous_zone
            .as_deref()
            .map(|last_zone| !last_zone.eq_ignore_ascii_case(zone_name.as_str()))
            .unwrap_or(true);

        let Some(zone) = zone_registry.find_zone_by_name(zone_name.as_str()) else {
            tracing::warn!(
                "[bong][network] zone transition for entity {entity:?} resolved unknown zone `{}`",
                zone_name
            );
            tracker.last_zone_by_entity.insert(entity, zone_name);
            tracker.last_snapshot_by_entity.remove(&entity);
            continue;
        };

        let current_snapshot = ZoneInfoRuntimeSnapshot::from_zone(zone);
        let runtime_changed = tracker
            .last_snapshot_by_entity
            .get(&entity)
            .map(|previous_snapshot| previous_snapshot != &current_snapshot)
            .unwrap_or(true);
        if !transitioned && !runtime_changed {
            continue;
        }

        let previous_qi = tracker
            .last_snapshot_by_entity
            .get(&entity)
            .map(|snapshot| snapshot.spirit_qi() as f32)
            .or_else(|| {
                previous_zone
                    .as_deref()
                    .and_then(|name| zone_registry.find_zone_by_name(name))
                    .map(|zone| zone.spirit_qi as f32)
            })
            .unwrap_or(zone.spirit_qi as f32);
        let perception_text = crate::combat::woliu::ambient_qi_perception(
            previous_qi,
            current_snapshot.spirit_qi() as f32,
            has_zone_qi_inspect(cultivation, perceptions),
        );

        let active_events = (!current_snapshot.active_events.is_empty())
            .then(|| current_snapshot.active_events.clone());
        let payload = ServerDataV1::new(ServerDataPayloadV1::ZoneInfo {
            zone: current_snapshot.zone.clone(),
            spirit_qi: current_snapshot.spirit_qi(),
            danger_level: current_snapshot.danger_level,
            status: current_snapshot.status,
            active_events,
            perception_text,
        });
        let payload_type = payload_type_label(payload.payload_type());
        let payload_bytes = match serialize_server_data_payload(&payload) {
            Ok(payload) => payload,
            Err(error) => {
                log_payload_build_error(payload_type, &error);
                continue;
            }
        };

        send_server_data_payload(&mut client, payload_bytes.as_slice());
        tracker.last_zone_by_entity.insert(entity, zone_name);
        tracker
            .last_snapshot_by_entity
            .insert(entity, current_snapshot);
    }

    tracker
        .last_zone_by_entity
        .retain(|entity, _| live_entities.contains(entity));
    tracker
        .last_snapshot_by_entity
        .retain(|entity, _| live_entities.contains(entity));
}

fn has_zone_qi_inspect(
    cultivation: Option<&Cultivation>,
    perceptions: Option<&UnlockedPerceptions>,
) -> bool {
    perceptions.is_some_and(|perceptions| perceptions.set.contains("zone_qi_density"))
        || cultivation.is_some_and(|cultivation| {
            crate::cultivation::realm_vision::planner::realm_rank(cultivation.realm) >= 2
        })
}

fn emit_event_alerts_on_major_event_creation(
    mut active_events: Option<valence::prelude::ResMut<ActiveEventsResource>>,
    mut clients: Query<(Entity, &mut Client), With<Client>>,
    audio_events: Option<ResMut<Events<audio_event_emit::PlaySoundRecipeRequest>>>,
) {
    let Some(active_events) = active_events.as_deref_mut() else {
        return;
    };
    let mut audio_events = audio_events;

    for pending_alert in active_events.drain_major_event_alerts() {
        let message = pending_alert.message.clone().unwrap_or_else(|| {
            major_event_alert_message(
                pending_alert.event_name.as_str(),
                pending_alert.zone_name.as_str(),
                pending_alert.duration_ticks,
            )
        });
        let Some(event_kind) = event_kind_from_name(pending_alert.event_name.as_str()) else {
            tracing::warn!(
                "[bong][network] skipping unsupported major event alert `{}` for zone `{}`",
                pending_alert.event_name,
                pending_alert.zone_name
            );
            continue;
        };

        let payload = ServerDataV1::new(ServerDataPayloadV1::EventAlert {
            event: event_kind,
            message: message.clone(),
            zone: Some(pending_alert.zone_name.clone()),
            duration_ticks: Some(pending_alert.duration_ticks),
        });
        let payload_type = payload_type_label(payload.payload_type());
        let payload_bytes = match serialize_server_data_payload(&payload) {
            Ok(payload) => payload,
            Err(error) => {
                log_payload_build_error(payload_type, &error);
                continue;
            }
        };

        let locust_payload = locust_swarm_warning_payload(&pending_alert, message.as_str());

        for (entity, mut client) in &mut clients {
            send_server_data_payload(&mut client, payload_bytes.as_slice());
            if let Some(locust_payload) = locust_payload.as_ref() {
                client.send_custom_payload(ident!("bong:locust_swarm_warning"), locust_payload);
                if let Some(audio_events) = audio_events.as_deref_mut() {
                    audio_events.send(audio_event_emit::PlaySoundRecipeRequest {
                        recipe_id: "locust_swarm_warning".to_string(),
                        instance_id: 0,
                        pos: None,
                        flag: None,
                        volume_mul: 1.0,
                        pitch_shift: 0.0,
                        recipient: audio_event_emit::AudioRecipient::Single(entity),
                    });
                }
            }
        }
    }
}

fn locust_swarm_warning_payload(
    pending_alert: &crate::world::events::MajorEventAlert,
    message: &str,
) -> Option<Vec<u8>> {
    if pending_alert.event_name != crate::world::events::EVENT_BEAST_TIDE
        || !message.contains("灵蝗潮")
    {
        return None;
    }

    let payload = serde_json::json!({
        "v": 1,
        "type": "locust_swarm_warning",
        "zone": pending_alert.zone_name,
        "message": message,
        "duration_ticks": pending_alert.duration_ticks,
    });
    serde_json::to_vec(&payload).ok()
}

fn event_kind_from_name(event_name: &str) -> Option<EventKind> {
    match event_name {
        crate::world::events::EVENT_THUNDER_TRIBULATION => Some(EventKind::ThunderTribulation),
        crate::world::events::EVENT_BEAST_TIDE => Some(EventKind::BeastTide),
        crate::world::events::EVENT_REALM_COLLAPSE => Some(EventKind::RealmCollapse),
        crate::world::events::EVENT_KARMA_BACKLASH => Some(EventKind::KarmaBacklash),
        crate::world::calamity::EVENT_POISON_MIASMA => Some(EventKind::PoisonMiasma),
        crate::world::calamity::EVENT_MERIDIAN_SEAL => Some(EventKind::MeridianSeal),
        crate::world::calamity::EVENT_DAOXIANG_WAVE => Some(EventKind::DaoxiangWave),
        crate::world::calamity::EVENT_HEAVENLY_FIRE => Some(EventKind::HeavenlyFire),
        crate::world::calamity::EVENT_PRESSURE_INVERT => Some(EventKind::PressureInvert),
        crate::world::calamity::EVENT_ALL_WITHER => Some(EventKind::AllWither),
        _ => None,
    }
}

fn major_event_alert_message(event_name: &str, zone_name: &str, duration_ticks: u64) -> String {
    let event_label = match event_name {
        crate::world::events::EVENT_THUNDER_TRIBULATION => "天劫",
        crate::world::events::EVENT_BEAST_TIDE => "兽潮",
        crate::world::events::EVENT_REALM_COLLAPSE => "域崩",
        crate::world::events::EVENT_KARMA_BACKLASH => "因果反噬",
        crate::world::calamity::EVENT_POISON_MIASMA => "毒瘴",
        crate::world::calamity::EVENT_MERIDIAN_SEAL => "封脉阵",
        crate::world::calamity::EVENT_DAOXIANG_WAVE => "道伥潮",
        crate::world::calamity::EVENT_HEAVENLY_FIRE => "天火",
        crate::world::calamity::EVENT_PRESSURE_INVERT => "灵压倒转",
        crate::world::calamity::EVENT_ALL_WITHER => "万物凋零",
        _ => "异变",
    };

    format!("{event_label}已在区域 {zone_name} 触发，预计持续 {duration_ticks} tick。")
}

/// Process inbound messages from Redis (agent commands + narrations)
#[allow(clippy::too_many_arguments)]
fn process_redis_inbound(
    redis: Res<RedisBridgeResource>,
    zone_registry: Option<Res<ZoneRegistry>>,
    mut clients: Query<(Entity, &mut Client, &Username, &Position), With<Client>>,
    mut spirit_treasure_holders: Query<
        (&ActiveSpiritTreasures, Option<&mut StatusEffects>),
        With<Client>,
    >,
    insight_context: Query<InsightContextQueryItem<'_>>,
    mut command_executor: valence::prelude::ResMut<CommandExecutorResource>,
    mut narration_dedupe: valence::prelude::ResMut<NarrationDedupeResource>,
    mut spirit_treasure_registry: Option<ResMut<SpiritTreasureRegistry>>,
    mut commands: Commands,
    mut insight_offers: EventWriter<crate::cultivation::insight::InsightOffer>,
    mut life_records: Query<ClientLifeRecordQueryItem<'_>, ClientLifeRecordQueryFilter>,
    audio_events: Option<ResMut<Events<audio_event_emit::PlaySoundRecipeRequest>>>,
    persistence_settings: Option<Res<PersistenceSettings>>,
    runtime_mirror_redis: Option<Res<RuntimeMirrorRedisConfig>>,
    mut agent_ui_cmd_tx: EventWriter<agent_ui::AgentUiCmdEvent>,
) {
    let mut audio_events = audio_events;
    let mut drained_messages = 0;

    while drained_messages < REDIS_INBOUND_DRAIN_BUDGET {
        let Ok(msg) = redis.rx_inbound.try_recv() else {
            break;
        };

        drained_messages += 1;

        match msg {
            RedisInbound::AgentCommand(cmd) => {
                let command_count = cmd.commands.len();
                let batch_id = cmd.id.clone();
                let source = cmd.source.clone().unwrap_or_else(|| "unknown".to_string());
                let enqueue_outcome = command_executor.enqueue_batch(cmd);

                if enqueue_outcome.dedupe_drop {
                    tracing::info!(
                        "[bong][network] dedupe_drop batch_id={} source={} type=command_batch target=- result=dropped_duplicate_batch command_count={}",
                        batch_id,
                        source.as_str(),
                        command_count
                    );
                    continue;
                }

                tracing::info!(
                    "[bong][network] command_batch_ingress batch_id={} source={} type=command_batch target=- result=queued command_count={}",
                    batch_id,
                    source.as_str(),
                    command_count
                );
            }
            RedisInbound::AgentNarration(narr) => {
                backfill_skill_milestone_narrations_from_batch(
                    &mut life_records,
                    narr.narrations.as_slice(),
                );
                process_agent_narrations_with_dedupe(
                    &mut clients,
                    zone_registry.as_deref(),
                    &mut narration_dedupe,
                    &mut life_records,
                    audio_events.as_deref_mut(),
                    persistence_settings.as_deref(),
                    narr.narrations.as_slice(),
                );
            }
            RedisInbound::AgentWorldModel(envelope) => {
                process_agent_world_model_envelope(
                    persistence_settings.as_deref(),
                    runtime_mirror_redis.as_deref(),
                    &envelope,
                );
            }
            RedisInbound::InsightOffer(offer) => {
                tracing::info!(
                    "[bong][network] insight_offer_received character_id={} trigger_id={} choices={}",
                    offer.character_id,
                    offer.trigger_id,
                    offer.choices.len()
                );
                let Some((entity, _, _, _)) = clients
                    .iter_mut()
                    .find(|(_, _, name, _)| name.0 == offer.character_id)
                else {
                    tracing::warn!(
                        "[bong][network] insight offer character_id={:?} not connected; dropping",
                        offer.character_id
                    );
                    continue;
                };
                let context = insight_context.get(entity).ok().map(
                    |(qi_color, practice_log, quota, cultivation)| {
                        (qi_color, practice_log, quota, cultivation.realm)
                    },
                );
                if context.is_none() {
                    tracing::warn!(
                        "[bong][network] insight offer character_id={} trigger_id={} has no cultivation context; using default fallback context",
                        offer.character_id,
                        offer.trigger_id
                    );
                }
                let Some(choices) = crate::cultivation::insight_flow::ingest_agent_insight_offer(
                    &offer.trigger_id,
                    &offer.choices,
                    context,
                ) else {
                    continue;
                };
                commands.entity(entity).insert(
                    crate::cultivation::insight_flow::PendingInsightOffer {
                        trigger_id: offer.trigger_id.clone(),
                        choices: choices.clone(),
                    },
                );
                insight_offers.send(crate::cultivation::insight::InsightOffer {
                    entity,
                    trigger_id: offer.trigger_id.clone(),
                    choices,
                });
            }
            RedisInbound::HeartDemonOffer(offer) => {
                tracing::info!(
                    "[bong][network] heart_demon_offer_received trigger_id={} choices={}",
                    offer.trigger_id,
                    offer.choices.len()
                );
                let Some((entity, _, _, _)) = clients.iter_mut().find(|(entity, _, _, _)| {
                    let Some((entity_index, started_tick)) =
                        parse_heart_demon_trigger_id(&offer.trigger_id)
                    else {
                        return false;
                    };
                    entity.index() == entity_index
                        && heart_demon_trigger_id_for_entity(entity.index(), started_tick)
                            == offer.trigger_id
                }) else {
                    tracing::warn!(
                        "[bong][network] heart demon offer trigger_id={:?} has no connected target; dropping",
                        offer.trigger_id
                    );
                    continue;
                };
                commands.entity(entity).insert(
                    crate::cultivation::tribulation::PendingHeartDemonOffer {
                        trigger_id: offer.trigger_id.clone(),
                        payload: offer,
                    },
                );
            }
            RedisInbound::SpiritTreasureDialogue(dialogue) => {
                if let Some(registry) = spirit_treasure_registry.as_deref_mut() {
                    spirit_treasure_emit::process_spirit_treasure_dialogue(
                        dialogue,
                        zone_registry.as_deref(),
                        registry,
                        &mut clients,
                        &mut spirit_treasure_holders,
                    );
                }
            }
            // ─── plan-agent-ui-data-v1 P0：天道 UI 指令 ─────────────────────
            RedisInbound::AgentUiCmd(cmd) => {
                agent_ui_cmd_tx.send(agent_ui::AgentUiCmdEvent(cmd));
            }
        }
    }

    if drained_messages == REDIS_INBOUND_DRAIN_BUDGET {
        tracing::debug!(
            "[bong][network] redis inbound drain hit budget {REDIS_INBOUND_DRAIN_BUDGET}; remaining messages will be handled next tick"
        );
    }
}

fn parse_heart_demon_trigger_id(trigger_id: &str) -> Option<(u32, u64)> {
    let mut parts = trigger_id.split(':');
    let prefix = parts.next()?;
    let entity_index = parts.next()?.parse().ok()?;
    let started_tick = parts.next()?.parse().ok()?;
    if parts.next().is_some() || prefix != "heart_demon" {
        return None;
    }
    Some((entity_index, started_tick))
}

fn heart_demon_trigger_id_for_entity(entity_index: u32, started_tick: u64) -> String {
    format!("heart_demon:{entity_index}:{started_tick}")
}

fn process_agent_world_model_envelope(
    persistence_settings: Option<&PersistenceSettings>,
    runtime_mirror_redis: Option<&RuntimeMirrorRedisConfig>,
    envelope: &AgentWorldModelEnvelopeV1,
) {
    let Some(settings) = persistence_settings else {
        tracing::warn!(
            "[bong][network] dropped agent world-model envelope id={} because PersistenceSettings is unavailable",
            envelope.id
        );
        return;
    };

    let snapshot = agent_world_model_snapshot_from_wire(&envelope.snapshot);
    let source = envelope.source.as_deref().unwrap_or("unknown");

    if let Err(error) =
        persist_agent_world_model_authority_state(settings, envelope.id.as_str(), source, &snapshot)
    {
        tracing::warn!(
            "[bong][network] failed sqlite authority persist for agent world-model id={}: {error}",
            envelope.id
        );
        return;
    }

    let Some(redis_config) = runtime_mirror_redis else {
        tracing::warn!(
            "[bong][network] sqlite authority persist succeeded for id={}, but RuntimeMirrorRedisConfig is unavailable; skipped mirror update",
            envelope.id
        );
        return;
    };

    if let Err(error) = write_world_model_runtime_mirror(redis_config, Some(&snapshot)) {
        tracing::warn!(
            "[bong][network] sqlite authority persist succeeded for id={}, but redis mirror update failed: {error}",
            envelope.id
        );
    }
}

fn bootstrap_world_model_runtime_mirror_system(
    persistence_settings: Option<Res<PersistenceSettings>>,
    runtime_mirror_redis: Option<Res<RuntimeMirrorRedisConfig>>,
) {
    let Some(settings) = persistence_settings.as_deref() else {
        tracing::warn!(
            "[bong][network] skipped world-model runtime mirror bootstrap: PersistenceSettings unavailable"
        );
        return;
    };
    let Some(redis_config) = runtime_mirror_redis.as_deref() else {
        tracing::warn!(
            "[bong][network] skipped world-model runtime mirror bootstrap: RuntimeMirrorRedisConfig unavailable"
        );
        return;
    };

    let snapshot = match bootstrap_agent_world_model_mirror(settings) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            tracing::warn!(
                "[bong][network] failed to load sqlite authority world-model snapshot during startup: {error}"
            );
            return;
        }
    };

    if let Err(error) = write_world_model_runtime_mirror(redis_config, snapshot.as_ref()) {
        tracing::warn!(
            "[bong][network] failed to rebuild runtime world-model mirror from sqlite authority: {error}"
        );
    }
}

fn should_run_world_model_runtime_mirror_reconcile(
    state: &mut WorldModelMirrorReconcileState,
) -> bool {
    state.ticks_since_last_reconcile = state.ticks_since_last_reconcile.saturating_add(1);
    if state.ticks_since_last_reconcile < WORLD_MODEL_RUNTIME_MIRROR_RECONCILE_INTERVAL_TICKS {
        return false;
    }

    state.ticks_since_last_reconcile = 0;
    true
}

fn reconcile_world_model_runtime_mirror_with_writer<F>(
    settings: &PersistenceSettings,
    mut write_mirror: F,
) -> io::Result<()>
where
    F: FnMut(Option<&AgentWorldModelSnapshotRecord>) -> io::Result<()>,
{
    let snapshot = bootstrap_agent_world_model_mirror(settings)?;
    write_mirror(snapshot.as_ref())
}

fn reconcile_world_model_runtime_mirror(
    settings: &PersistenceSettings,
    redis_config: &RuntimeMirrorRedisConfig,
) -> io::Result<()> {
    reconcile_world_model_runtime_mirror_with_writer(settings, |snapshot| {
        write_world_model_runtime_mirror(redis_config, snapshot)
    })
}

fn reconcile_world_model_runtime_mirror_system(
    persistence_settings: Option<Res<PersistenceSettings>>,
    runtime_mirror_redis: Option<Res<RuntimeMirrorRedisConfig>>,
    mut reconcile_state: ResMut<WorldModelMirrorReconcileState>,
) {
    let Some(settings) = persistence_settings.as_deref() else {
        return;
    };
    let Some(redis_config) = runtime_mirror_redis.as_deref() else {
        return;
    };
    if !should_run_world_model_runtime_mirror_reconcile(&mut reconcile_state) {
        return;
    }

    if let Err(error) = reconcile_world_model_runtime_mirror(settings, redis_config) {
        tracing::warn!(
            "[bong][network] failed periodic runtime world-model mirror reconcile from sqlite authority: {error}"
        );
    }
}

fn agent_world_model_snapshot_from_wire(
    snapshot: &AgentWorldModelSnapshotV1,
) -> AgentWorldModelSnapshotRecord {
    let mut last_decisions = std::collections::BTreeMap::new();
    for (agent_name, decision) in &snapshot.last_decisions {
        let commands = decision
            .commands
            .iter()
            .map(|command| AgentWorldModelCommandRecord {
                command_type: command_type_to_wire_value(&command.command_type).to_string(),
                target: command.target.clone(),
                params: command.params.clone().into_iter().collect(),
            })
            .collect::<Vec<_>>();

        let narrations = decision
            .narrations
            .iter()
            .map(|narration| AgentWorldModelNarrationRecord {
                scope: narration_scope_to_wire_value(&narration.scope).to_string(),
                target: narration.target.clone(),
                text: narration.text.clone(),
                style: narration_style_to_wire_value(&narration.style).to_string(),
            })
            .collect::<Vec<_>>();

        last_decisions.insert(
            agent_name.clone(),
            AgentWorldModelDecisionRecord {
                commands,
                narrations,
                reasoning: decision.reasoning.clone(),
            },
        );
    }

    let zone_history = snapshot
        .zone_history
        .iter()
        .map(|(zone_name, history)| {
            let serialized = history
                .iter()
                .map(|entry| serde_json::to_value(entry).unwrap_or(serde_json::Value::Null))
                .collect::<Vec<_>>();
            (zone_name.clone(), serialized)
        })
        .collect::<std::collections::BTreeMap<_, _>>();

    let current_era = snapshot
        .current_era
        .as_ref()
        .and_then(|era| serde_json::to_value(era).ok());

    let neg_domain_pending_tribulations = snapshot
        .neg_domain_pending_tribulations
        .iter()
        .map(|(player_id, pending)| {
            (
                player_id.clone(),
                AgentWorldModelNegDomainPendingTribulationRecord {
                    player_uuid: pending.player_uuid.clone(),
                    player_name: pending.player_name.clone(),
                    zone: pending.zone.clone(),
                    entered_at_tick: pending.entered_at_tick,
                    last_suppressed_tick: pending.last_suppressed_tick,
                    reason: pending.reason.clone(),
                },
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();

    let neg_domain_escape_telemetry = AgentWorldModelNegDomainEscapeTelemetryRecord {
        escape_entry_count: snapshot.neg_domain_escape_telemetry.escape_entry_count,
        post_escape_realm_drop_count: snapshot
            .neg_domain_escape_telemetry
            .post_escape_realm_drop_count,
        successful_tribulation_avoidance_count: snapshot
            .neg_domain_escape_telemetry
            .successful_tribulation_avoidance_count,
        active_escape_session_count: snapshot
            .neg_domain_escape_telemetry
            .active_escape_session_count,
        post_escape_realm_drop_rate: snapshot
            .neg_domain_escape_telemetry
            .post_escape_realm_drop_rate,
    };

    let neg_domain_escape_sessions = snapshot
        .neg_domain_escape_sessions
        .iter()
        .map(|(player_id, session)| {
            (
                player_id.clone(),
                AgentWorldModelNegDomainEscapeSessionRecord {
                    player_uuid: session.player_uuid.clone(),
                    player_name: session.player_name.clone(),
                    zone: session.zone.clone(),
                    entered_at_tick: session.entered_at_tick,
                    entry_realm_rank: session.entry_realm_rank,
                },
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();

    AgentWorldModelSnapshotRecord {
        current_era,
        zone_history,
        last_decisions,
        player_first_seen_tick: snapshot.player_first_seen_tick.clone(),
        neg_domain_pending_tribulations,
        neg_domain_escape_telemetry,
        neg_domain_escape_sessions,
        last_tick: snapshot.last_tick,
        last_state_ts: snapshot.last_state_ts,
    }
}

fn command_type_to_wire_value(command_type: &CommandType) -> &'static str {
    match command_type {
        CommandType::SpawnEvent => "spawn_event",
        CommandType::SpawnNpc => "spawn_npc",
        CommandType::DespawnNpc => "despawn_npc",
        CommandType::FactionEvent => "faction_event",
        CommandType::ModifyZone => "modify_zone",
        CommandType::NpcBehavior => "npc_behavior",
        CommandType::HeartbeatOverride => "heartbeat_override",
    }
}

fn narration_scope_to_wire_value(scope: &NarrationScope) -> &'static str {
    match scope {
        NarrationScope::Broadcast => "broadcast",
        NarrationScope::Zone => "zone",
        NarrationScope::Player => "player",
    }
}

fn narration_style_to_wire_value(style: &NarrationStyle) -> &'static str {
    match style {
        NarrationStyle::SystemWarning => "system_warning",
        NarrationStyle::Perception => "perception",
        NarrationStyle::Narration => "narration",
        NarrationStyle::EraDecree => "era_decree",
        NarrationStyle::PoliticalJianghu => "political_jianghu",
    }
}

fn write_world_model_runtime_mirror(
    redis_config: &RuntimeMirrorRedisConfig,
    snapshot: Option<&AgentWorldModelSnapshotRecord>,
) -> io::Result<()> {
    let mut connection_guard = redis_config.connection.lock().map_err(|error| {
        io::Error::other(format!(
            "runtime mirror redis connection lock poisoned: {error}"
        ))
    })?;
    if connection_guard.is_none() {
        *connection_guard = Some(
            redis_config
                .client
                .get_connection()
                .map_err(io::Error::other)?,
        );
    }
    let connection = connection_guard.as_mut().ok_or_else(|| {
        io::Error::other("runtime mirror redis connection missing after initialization")
    })?;

    if let Some(snapshot) = snapshot {
        let fields = world_model_snapshot_to_mirror_fields(snapshot)?;

        let field_pairs = fields
            .iter()
            .map(|(field, value)| (field.as_str(), value.as_str()))
            .collect::<Vec<_>>();

        let _: usize = redis::cmd("HSET")
            .arg(WORLD_MODEL_STATE_KEY)
            .arg(field_pairs)
            .query(connection)
            .map_err(io::Error::other)?;
    } else {
        let _: usize = redis::cmd("DEL")
            .arg(WORLD_MODEL_STATE_KEY)
            .query(connection)
            .map_err(io::Error::other)?;
    }

    Ok(())
}

fn process_agent_narrations(
    clients: &mut Query<(Entity, &mut Client, &Username, &Position), With<Client>>,
    zone_registry: Option<&ZoneRegistry>,
    mut audio_events: Option<&mut Events<audio_event_emit::PlaySoundRecipeRequest>>,
    narrations: &[crate::schema::narration::Narration],
) {
    for narration in narrations {
        process_single_narration(
            clients,
            zone_registry,
            audio_events.as_deref_mut(),
            narration,
        );
    }
}

fn process_agent_narrations_with_dedupe(
    clients: &mut Query<(Entity, &mut Client, &Username, &Position), With<Client>>,
    zone_registry: Option<&ZoneRegistry>,
    narration_dedupe: &mut NarrationDedupeResource,
    life_records: &mut Query<ClientLifeRecordQueryItem<'_>, ClientLifeRecordQueryFilter>,
    mut audio_events: Option<&mut Events<audio_event_emit::PlaySoundRecipeRequest>>,
    persistence_settings: Option<&PersistenceSettings>,
    narrations: &[crate::schema::narration::Narration],
) {
    for narration in narrations {
        let dedupe_key = narration_dedupe_key(narration);
        if narration_dedupe.should_drop(dedupe_key.as_str(), current_unix_timestamp_secs()) {
            tracing::info!(
                "[bong][network] dedupe_drop batch_id=- source=agent type=narration target={:?} result=dropped_duplicate_payload scope={:?}",
                narration.target,
                narration.scope
            );
            continue;
        }

        archive_death_insight_narration(life_records, persistence_settings, narration);
        process_single_narration(
            clients,
            zone_registry,
            audio_events.as_deref_mut(),
            narration,
        );
    }
}

fn backfill_skill_milestone_narrations_from_batch(
    players: &mut Query<ClientLifeRecordQueryItem<'_>, ClientLifeRecordQueryFilter>,
    narrations: &[crate::schema::narration::Narration],
) {
    for narration in narrations {
        let Some(target) = narration.target.as_deref() else {
            continue;
        };
        let Some((entity, skill, new_lv)) = parse_skill_milestone_narration_target(target) else {
            continue;
        };
        let Ok((_, _, mut life_record)) = players.get_mut(entity) else {
            continue;
        };
        if let Some(milestone) = life_record
            .skill_milestones
            .iter_mut()
            .rev()
            .find(|milestone| milestone.skill == skill && milestone.new_lv == new_lv)
        {
            milestone.narration = narration.text.clone();
        }
    }
}

fn parse_skill_milestone_narration_target(target: &str) -> Option<(Entity, SkillId, u8)> {
    let mut char_bits = None;
    let mut skill = None;
    let mut new_lv = None;

    for part in target
        .split('|')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        if let Some(bits) = part.strip_prefix("char:") {
            char_bits = bits.parse::<u64>().ok();
            continue;
        }
        if let Some(raw_skill) = part.strip_prefix("skill:") {
            skill = match raw_skill {
                "herbalism" => Some(SkillId::Herbalism),
                "alchemy" => Some(SkillId::Alchemy),
                "forging" => Some(SkillId::Forging),
                "combat" => Some(SkillId::Combat),
                "mineral" => Some(SkillId::Mineral),
                "cultivation" => Some(SkillId::Cultivation),
                _ => None,
            };
            continue;
        }
        if let Some(raw_lv) = part.strip_prefix("lv:") {
            new_lv = raw_lv.parse::<u8>().ok();
        }
    }

    Some((Entity::try_from_bits(char_bits?).ok()?, skill?, new_lv?))
}

fn narration_dedupe_key(narration: &crate::schema::narration::Narration) -> String {
    format!(
        "scope={:?}|target={}|style={:?}|kind={:?}|text={}",
        narration.scope,
        narration.target.as_deref().unwrap_or_default(),
        narration.style,
        narration.kind,
        narration.text
    )
}

fn archive_death_insight_narration(
    life_records: &mut Query<ClientLifeRecordQueryItem<'_>, ClientLifeRecordQueryFilter>,
    persistence_settings: Option<&PersistenceSettings>,
    narration: &crate::schema::narration::Narration,
) {
    if narration.kind != Some(NarrationKind::DeathInsight)
        || narration.scope != NarrationScope::Player
    {
        return;
    }

    let Some(target) = narration.target.as_deref() else {
        return;
    };
    let Some((_, _, mut life_record)) =
        life_records
            .iter_mut()
            .find(|(username, lifecycle, record)| {
                death_insight_target_matches_life_record(target, *username, *lifecycle, record)
            })
    else {
        tracing::debug!(
            "[bong][network] death insight narration target {target:?} matched no active LifeRecord"
        );
        return;
    };

    life_record.push_death_insight(
        narration.text.clone(),
        narration_style_to_wire_value(&narration.style),
    );

    if let Some(settings) = persistence_settings {
        if let Err(error) = persist_life_record_death_insight(settings, &life_record) {
            tracing::warn!(
                "[bong][network] failed to persist death insight for {}: {error}",
                life_record.character_id
            );
        }
    }
}

fn death_insight_target_matches_life_record(
    target: &str,
    username: Option<&Username>,
    lifecycle: Option<&Lifecycle>,
    life_record: &LifeRecord,
) -> bool {
    normalize_life_record_target(target).is_some_and(|target_key| {
        normalize_life_record_target(life_record.character_id.as_str()).as_deref()
            == Some(target_key.as_str())
            || lifecycle
                .and_then(|lifecycle| normalize_life_record_target(lifecycle.character_id.as_str()))
                .as_deref()
                == Some(target_key.as_str())
            || username
                .map(|username| canonical_player_id(username.0.as_str()))
                .and_then(|canonical| normalize_life_record_target(canonical.as_str()))
                .as_deref()
                == Some(target_key.as_str())
            || username
                .and_then(|username| normalize_life_record_target(username.0.as_str()))
                .as_deref()
                == Some(target_key.as_str())
    })
}

fn normalize_life_record_target(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let stripped = trimmed.strip_prefix("offline:").unwrap_or(trimmed).trim();
    if stripped.is_empty() {
        return None;
    }
    let username = stripped
        .split_once(':')
        .map_or(stripped, |(username, _)| username)
        .trim();
    if username.is_empty() {
        None
    } else {
        Some(username.to_ascii_lowercase())
    }
}

fn process_single_narration(
    clients: &mut Query<(Entity, &mut Client, &Username, &Position), With<Client>>,
    zone_registry: Option<&ZoneRegistry>,
    mut audio_events: Option<&mut Events<audio_event_emit::PlaySoundRecipeRequest>>,
    narration: &crate::schema::narration::Narration,
) {
    let selector = match narration_selector(narration) {
        Some(selector) => selector,
        None => {
            tracing::warn!(
                "[bong][network] dropped narration with missing/invalid target for scope {:?}",
                narration.scope
            );
            return;
        }
    };

    let routed_targets = collect_routed_targets(clients, zone_registry, &selector);
    if routed_targets.is_empty() {
        tracing::debug!(
            "[bong][network] narration scope {:?} target {:?} matched zero recipients",
            narration.scope,
            narration.target
        );
        return;
    }

    let payload = ServerDataV1::new(ServerDataPayloadV1::Narration {
        narrations: vec![narration.clone()],
    });
    let payload_type = payload_type_label(payload.payload_type());
    let payload_bytes = match serialize_server_data_payload(&payload) {
        Ok(payload) => payload,
        Err(error) => {
            log_payload_build_error(payload_type, &error);
            return;
        }
    };

    for entity in routed_targets.iter().copied() {
        if let Ok((_, mut client, _, _)) = clients.get_mut(entity) {
            if let Some(audio_events) = audio_events.as_deref_mut() {
                audio_events.send(audio_event_emit::PlaySoundRecipeRequest {
                    recipe_id: "narration_cue".to_string(),
                    instance_id: 0,
                    pos: None,
                    flag: None,
                    volume_mul: 1.0,
                    pitch_shift: 0.0,
                    recipient: audio_event_emit::AudioRecipient::Single(entity),
                });
            }
            send_server_data_payload(&mut client, payload_bytes.as_slice());
        }
    }

    tracing::info!(
        "[bong][network] sent {} {} narration payload to {} recipient(s) for scope {:?} target {:?}",
        SERVER_DATA_CHANNEL,
        payload_type,
        routed_targets.len(),
        narration.scope,
        narration.target
    );
}

fn narration_selector(
    narration: &crate::schema::narration::Narration,
) -> Option<RecipientSelector> {
    match narration.scope {
        crate::schema::common::NarrationScope::Broadcast => Some(RecipientSelector::Broadcast),
        crate::schema::common::NarrationScope::Zone => narration
            .target
            .as_deref()
            .map(str::trim)
            .filter(|target| !target.is_empty())
            .map(RecipientSelector::zone),
        crate::schema::common::NarrationScope::Player => narration
            .target
            .as_deref()
            .map(str::trim)
            .filter(|target| !target.is_empty())
            .map(RecipientSelector::player),
    }
}

fn collect_routed_targets(
    clients: &mut Query<(Entity, &mut Client, &Username, &Position), With<Client>>,
    zone_registry: Option<&ZoneRegistry>,
    selector: &RecipientSelector,
) -> Vec<Entity> {
    let zone_registry = effective_zone_registry(zone_registry);

    let recipient_rows = clients
        .iter_mut()
        .map(|(entity, _, username, position)| {
            let computed_zone = Some(zone_name_for_position(&zone_registry, position.get()));

            (
                entity,
                RecipientMetadata {
                    username: Some(username.0.clone()),
                    char_id: Some(format!("char:{}", entity.to_bits())),
                    zone: computed_zone,
                },
            )
        })
        .collect::<Vec<_>>();

    let recipient_metadata = recipient_rows
        .iter()
        .map(|(_, metadata)| metadata.clone())
        .collect::<Vec<_>>();

    let matched_indices = route_recipient_indices(
        selector,
        recipient_metadata.as_slice(),
        Some(&|zone_name, recipient| {
            recipient
                .zone
                .as_deref()
                .is_some_and(|zone| zone.eq_ignore_ascii_case(zone_name))
        }),
    );

    matched_indices
        .into_iter()
        .filter_map(|index| recipient_rows.get(index).map(|(entity, _)| *entity))
        .collect()
}

// ─── Legacy mock bridge systems (unchanged) ──────────────

fn send_welcome_payload_on_join(mut joined_clients: Query<(Entity, &mut Client), Added<Client>>) {
    let payload = ServerDataV1::welcome(crate::schema::server_data::WELCOME_MESSAGE);
    let payload_type = payload_type_label(payload.payload_type());
    let payload_bytes = match serialize_server_data_payload(&payload) {
        Ok(payload) => payload,
        Err(error) => {
            log_payload_build_error(payload_type, &error);
            return;
        }
    };

    for (entity, mut client) in &mut joined_clients {
        send_server_data_payload(&mut client, payload_bytes.as_slice());
        tracing::info!(
            "[bong][network] sent {} {} payload to client entity {entity:?}",
            SERVER_DATA_CHANNEL,
            payload_type,
        );
    }
}

fn process_bridge_messages(bridge: Res<NetworkBridgeResource>, mut clients: Query<&mut Client>) {
    let payload = ServerDataV1::heartbeat(crate::schema::server_data::HEARTBEAT_MESSAGE);
    let payload_type = payload_type_label(payload.payload_type());
    let heartbeat_payload = match serialize_server_data_payload(&payload) {
        Ok(payload) => payload,
        Err(error) => {
            log_payload_build_error(payload_type, &error);
            return;
        }
    };

    drain_bridge_commands(&bridge, || {
        for mut client in &mut clients {
            send_server_data_payload(&mut client, heartbeat_payload.as_slice());
        }
    });
}

pub(crate) fn send_server_data_payload(client: &mut Client, payload: &[u8]) {
    client.send_custom_payload(ident!("bong:server_data"), payload);
}

fn drain_bridge_commands(bridge: &NetworkBridgeResource, mut on_heartbeat: impl FnMut()) -> usize {
    let mut drained_messages = 0;

    while let Ok(command) = bridge.rx_from_agent.try_recv() {
        drained_messages += 1;

        match command {
            AgentCommand::Heartbeat => on_heartbeat(),
        }

        let _ = bridge.tx_to_agent.send(GameEvent::Placeholder);
    }

    drained_messages
}

pub(crate) fn log_payload_build_error(payload_type: &str, error: &PayloadBuildError) {
    match error {
        PayloadBuildError::Json(json_error) => tracing::error!(
            "[bong][network] failed to serialize {payload_type} payload for {}: {json_error}",
            SERVER_DATA_CHANNEL
        ),
        PayloadBuildError::Oversize { size, max } => tracing::error!(
            "[bong][network] {payload_type} payload for {} rejected as oversize: {size} > {max}",
            SERVER_DATA_CHANNEL
        ),
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
