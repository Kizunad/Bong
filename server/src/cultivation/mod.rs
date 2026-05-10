//! 修仙系统 — plan-cultivation-v1 完整切片（server 侧 P1–P5）。
//!
//! 子模块：
//!   * components       — 状态定义（Cultivation / MeridianSystem / QiColor / Karma / Contamination）
//!   * topology         — 20 经邻接表 Resource
//!   * tick             — QiRegenTick + ZoneQiDrainTick（零和合并实现）
//!   * meridian_open    — MeridianOpenTick（含 MeridianTarget Component）
//!   * breakthrough     — 5 阶升境事务
//!   * tribulation      — 化虚渡劫状态机（Spirit→Void）
//!   * forging          — rate / capacity 独立锻造
//!   * composure        — 心境缓慢回升
//!   * qi_zero_decay    — 爆脉降境 + LIFO 经脉封闭
//!   * color            — QiColorEvolutionTick
//!   * contamination    — 异种真元排异（10:15）
//!   * overload         — 超量流量 → 裂痕
//!   * heal             — 裂痕愈合
//!   * negative_zone    — 负灵域反吸
//!   * death_hooks      — 死亡触发 & 重生惩罚 & 终结清理
//!   * lifespan         — 寿元 / 死亡登记 / 重生概率纯模型
//!   * life_record      — 修炼生平卷
//!   * karma            — 业力极慢衰减
//!   * insight / insight_fallback / insight_apply — 顿悟系统
//!
//! 跨仓库 TODO：
//!   * 客户端 inspect UI + 目标选择对话框（plan §7）
//!   * agent LLM runtime（InsightRequest → InsightOffer 桥）
//!   * 战斗 plan：消费 CultivationDeathTrigger / throughput 写入，并在渡劫波次失败时发送 TribulationFailed

pub mod breakthrough;
pub mod burst_meridian;
pub mod character_lifecycle;
pub mod character_select;
pub mod color;
pub mod components;
pub mod composure;
pub mod contamination;
pub mod dead_zone;
pub mod death_hooks;
pub mod dugu;
pub mod forging;
pub mod full_power_strike;
pub mod heal;
pub mod insight;
pub mod insight_apply;
pub mod insight_fallback;
pub mod insight_flow;
pub mod karma;
pub mod known_techniques;
pub mod life_record;
pub mod lifespan;
pub mod luck_pool;
pub mod meridian;
pub mod meridian_open;
pub mod neg_pressure;
pub mod negative_zone;
pub mod overload;
pub mod possession;
pub mod qi_field;
pub mod qi_zero_decay;
pub mod realm_taint;
pub mod realm_vision;
pub mod skill_registry;
pub mod spiritual_sense;
pub mod style_modifier;
pub mod tick;
pub mod topology;
pub mod tribulation;
pub mod void;

use valence::prelude::{
    Added, App, Client, Commands, Entity, EventReader, EventWriter, IntoSystemConfigs, Or, Query,
    Res, Update, Username, Without,
};

use self::breakthrough::{
    breakthrough_system, rapid_breakthrough_karma_mark_system, BreakthroughOutcome,
    BreakthroughRequest,
};
use self::color::{
    qi_color_evolution_tick, record_cultivation_session_practice_events,
    CultivationSessionPracticeEvent, PracticeLog,
};
use self::components::{Contamination, Cultivation, Karma, MeridianSystem, QiColor};
use self::composure::composure_tick;
use self::contamination::contamination_tick;
use self::dead_zone::{dead_zone_silent_qi_loss_tick, DeadZoneTickHandler};
use self::death_hooks::{
    on_player_revived, on_player_terminated, CultivationDeathTrigger, PlayerRevived,
    PlayerTerminated,
};
use self::dugu::{
    dugu_poison_ambient_vfx_tick, dugu_poison_tick, expire_dugu_state,
    on_attack_resolved_dugu_handler, resolve_infuse_dugu_poison_intents,
    resolve_self_antidote_intent, AntidoteResultEvent, DuguObfuscationDisruptedEvent,
    DuguPoisonProgressEvent, DuguPractice, InfuseDuguPoisonIntent, SelfAntidoteIntent,
};
use self::forging::{forging_system, ForgeOutcome, ForgeRequest};
use self::heal::meridian_heal_tick;
use self::insight::{
    InsightChosen, InsightOffer, InsightQuota, InsightRequest, InsightTriggerRegistry,
};
use self::insight_apply::{InsightModifiers, UnlockedPerceptions};
use self::insight_flow::{
    apply_insight_chosen, insight_trigger_on_breakthrough, insight_trigger_on_forge,
    insight_trigger_on_wind_candle, process_insight_request,
};
use self::karma::karma_decay_tick;
use self::life_record::LifeRecord;
use self::lifespan::{
    lifespan_aging_tick, process_lifespan_extension_intents, sync_frailty_status_effects,
    AgingEventEmitted, DeathRegistry, LifespanCapTable, LifespanComponent, LifespanEventEmitted,
    LifespanExtensionIntent, LifespanExtensionLedger,
};
use self::meridian::severed::{
    apply_severed_event_system, meridian_severed_detection_tick, MeridianSeveredEvent,
    MeridianSeveredPermanent, SkillMeridianDependencies,
};
use self::meridian_open::meridian_open_tick;
use self::neg_pressure::tick_neg_pressure;
use self::negative_zone::negative_zone_siphon_tick;
use self::overload::{
    apply_meridian_crack_events, apply_meridian_overload_events, overload_detection_tick,
    MeridianCrackEvent, MeridianOverloadEvent,
};
use self::possession::{
    process_duo_she_requests, process_life_core_requests, DuoSheCooldowns, DuoSheEventEmitted,
    DuoSheRequestEvent, DuoSheWarningEvent, UseLifeCoreEvent,
};
use self::qi_zero_decay::{qi_zero_decay_tick, RealmRegressed};
use self::realm_vision::push::{
    push_initial_realm_vision, push_realm_vision_on_breakthrough, push_realm_vision_on_revive,
};
use self::realm_vision::view_distance_ramp::view_distance_ramp_system;
use self::spiritual_sense::push::{
    cleanup_spiritual_sense_push_state, push_spiritual_sense_targets, SpiritualSensePushState,
};
use self::tick::{
    prune_cultivation_session_practice_accumulator, qi_regen_and_zone_drain_tick, CultivationClock,
    CultivationSessionPracticeAccumulator,
};
use self::topology::MeridianTopology;
use self::tribulation::{
    abort_du_xu_on_client_removed, emit_tribulation_boundary_vfx_system, heart_demon_choice_system,
    heart_demon_timeout_system, juebi_phase_effect_system, juebi_settlement_system,
    juebi_terrain_seed_system, juebi_terrain_tick_system, juebi_zone_aftershock_system,
    record_tribulation_interceptor_system, schedule_juebi_triggers_system,
    start_du_xu_request_system, start_due_juebi_triggers_system, start_tribulation_system,
    tribulation_aoe_system, tribulation_escape_boundary_system, tribulation_failure_system,
    tribulation_intercept_death_system, tribulation_omen_cloud_block_overlay_system,
    tribulation_phase_tick_system, tribulation_wave_system, AscensionQuotaOccupied,
    AscensionQuotaOpened, HeartDemonChoiceSubmitted, InitiateXuhuaTribulation, JueBiRuntimeContext,
    JueBiTerrainOverlay, JueBiTriggerEvent, JueBiTriggerSource, JueBiTriggeredEvent,
    JueBiZoneAftershocks, PendingJueBiTriggers, StartDuXuRequest, TribulationAnnounce,
    TribulationFailed, TribulationFled, TribulationLocked, TribulationOmenCloudBlocks,
    TribulationOriginDimension, TribulationSettled, TribulationState, TribulationWaveCleared,
};
use crate::cultivation::components::Realm;
use crate::npc::possession::DuoSheIntentForwardSet;
use crate::persistence::{
    load_active_tribulation, load_player_cultivation_bundle, release_ascension_quota_slot,
    PersistenceSettings,
};
use crate::player::state::{
    canonical_player_id, load_current_character_id, player_character_id, PlayerState,
    PlayerStatePersistence,
};
use crate::skill::events::SkillCapChanged;
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::karma::{karma_weight_decay_tick, void_realm_karma_pressure_tick};

pub fn register(app: &mut App) {
    tracing::info!("[bong][cultivation] registering cultivation systems (plan P1–P5)");
    let mut skill_meridian_dependencies = SkillMeridianDependencies::default();
    crate::combat::zhenmai_v2::declare_meridian_dependencies(&mut skill_meridian_dependencies);
    crate::combat::anqi_v2::declare_meridian_dependencies(&mut skill_meridian_dependencies);

    app.insert_resource(MeridianTopology::standard());
    app.insert_resource(CultivationClock::default());
    app.init_resource::<CultivationSessionPracticeAccumulator>();
    app.insert_resource(DeadZoneTickHandler::default());
    app.insert_resource(skill_registry::init_registry());
    app.insert_resource(skill_meridian_dependencies);
    app.insert_resource(InsightTriggerRegistry::with_defaults());
    app.insert_resource(DuoSheCooldowns::default());
    app.insert_resource(TribulationOmenCloudBlocks::default());
    app.insert_resource(PendingJueBiTriggers::default());
    app.insert_resource(self::tribulation::JueBiNullFields::default());
    app.insert_resource(JueBiTerrainOverlay::default());
    app.insert_resource(JueBiZoneAftershocks::default());
    app.insert_resource(self::tribulation::VoidQuotaConfig::from_env());
    app.insert_resource(SpiritualSensePushState::default());
    realm_taint::register(app);
    void::register(app);
    full_power_strike::register(app);

    // 事件（plan §3/§4/§5 全家桶）
    app.add_event::<BreakthroughRequest>();
    app.add_event::<BreakthroughOutcome>();
    app.add_event::<ForgeRequest>();
    app.add_event::<ForgeOutcome>();
    app.add_event::<RealmRegressed>();
    app.add_event::<CultivationDeathTrigger>();
    app.add_event::<PlayerRevived>();
    app.add_event::<PlayerTerminated>();
    app.add_event::<LifespanEventEmitted>();
    app.add_event::<AgingEventEmitted>();
    app.add_event::<LifespanExtensionIntent>();
    app.add_event::<DuoSheRequestEvent>();
    app.add_event::<DuoSheEventEmitted>();
    app.add_event::<DuoSheWarningEvent>();
    app.add_event::<UseLifeCoreEvent>();
    app.add_event::<InitiateXuhuaTribulation>();
    app.add_event::<StartDuXuRequest>();
    app.add_event::<TribulationAnnounce>();
    app.add_event::<TribulationLocked>();
    app.add_event::<TribulationWaveCleared>();
    app.add_event::<TribulationFailed>();
    app.add_event::<TribulationFled>();
    app.add_event::<TribulationSettled>();
    app.add_event::<JueBiTriggerEvent>();
    app.add_event::<JueBiTriggeredEvent>();
    app.add_event::<AscensionQuotaOpened>();
    app.add_event::<AscensionQuotaOccupied>();
    app.add_event::<HeartDemonChoiceSubmitted>();
    app.add_event::<InsightRequest>();
    app.add_event::<InsightOffer>();
    app.add_event::<InsightChosen>();
    app.add_event::<MeridianOverloadEvent>();
    app.add_event::<MeridianCrackEvent>();
    app.add_event::<burst_meridian::BurstMeridianEvent>();
    app.add_event::<MeridianSeveredEvent>();
    app.add_event::<CultivationSessionPracticeEvent>();
    app.add_event::<InfuseDuguPoisonIntent>();
    app.add_event::<DuguObfuscationDisruptedEvent>();
    app.add_event::<DuguPoisonProgressEvent>();
    app.add_event::<SelfAntidoteIntent>();
    app.add_event::<AntidoteResultEvent>();

    // Bevy IntoSystemConfigs 最多 20 个元素；拆两组。
    app.add_systems(
        Update,
        (
            attach_cultivation_to_joined_clients
                .after(crate::player::attach_player_state_to_joined_clients),
            // 核心 tick：回气/扣 zone → 打通 → 事务
            qi_regen_and_zone_drain_tick,
            lifespan_aging_tick.after(qi_regen_and_zone_drain_tick),
            meridian_open_tick.after(qi_regen_and_zone_drain_tick),
            breakthrough_system.after(meridian_open_tick),
            rapid_breakthrough_karma_mark_system.after(breakthrough_system),
            forging_system.after(breakthrough_system),
            // 稳态演化
            qi_color_evolution_tick,
            composure_tick,
            dead_zone_silent_qi_loss_tick.after(qi_regen_and_zone_drain_tick),
            qi_zero_decay_tick.after(dead_zone_silent_qi_loss_tick),
            emit_skill_caps_on_realm_regressed.after(qi_zero_decay_tick),
            // plan §2.1 损伤/净化链
            overload_detection_tick.after(meridian_open_tick),
            apply_meridian_crack_events.after(overload_detection_tick),
            contamination_tick.after(qi_regen_and_zone_drain_tick),
            negative_zone_siphon_tick.after(qi_regen_and_zone_drain_tick),
            // plan §4 死亡/重生钩子
            on_player_revived,
            on_player_terminated,
            // plan §11-5 业力
            karma_weight_decay_tick.after(qi_regen_and_zone_drain_tick),
            void_realm_karma_pressure_tick.after(karma_weight_decay_tick),
        ),
    );
    // plan-meridian-severed-v1 §1 P1：detection（cracks → integrity ≤ ε → emit
    // SEVERED event）+ apply（event → write component）。两步顺序保证同 tick 内
    // detection 写入 event，apply 后续读取并落 component；独立 add_systems 避开
    // 上面 tuple 超 Bevy 20 元素上限。
    //
    // codex P1（PR #157 review）：apply 必须 .after 所有 SEVERED 发射方，否则
    // tribulation 失败/逃跑路径与 despawn_disconnected_clients 在同 tick 触发时
    // 可能丢 SEVERED event（事件队列在玩家被 despawn 后才被消费，event 落到
    // missing entity 直接 drop）。所有当前 emitter（detection / 三 tribulation
    // 系统）显式 .after 锁定。未来新 emitter 接入时也必须加这条 ordering edge。
    app.add_systems(
        Update,
        (
            meridian_severed_detection_tick,
            apply_severed_event_system
                .after(meridian_severed_detection_tick)
                .after(tribulation_failure_system)
                .after(abort_du_xu_on_client_removed)
                .after(tribulation_escape_boundary_system),
        ),
    );
    app.add_systems(
        Update,
        record_cultivation_session_practice_events
            .after(qi_regen_and_zone_drain_tick)
            .before(qi_color_evolution_tick),
    );
    app.add_systems(
        Update,
        prune_cultivation_session_practice_accumulator.after(qi_regen_and_zone_drain_tick),
    );
    app.add_systems(
        Update,
        tick_neg_pressure.after(qi_regen_and_zone_drain_tick),
    );
    app.add_systems(
        Update,
        (
            // plan §3.2 渡劫：单独分组，避免 Bevy 0.14 tuple arity 上限。
            start_du_xu_request_system,
            schedule_juebi_triggers_system,
            start_due_juebi_triggers_system.after(schedule_juebi_triggers_system),
            start_tribulation_system.after(start_du_xu_request_system),
            tribulation_phase_tick_system
                .after(start_tribulation_system)
                .after(start_due_juebi_triggers_system),
            tribulation_omen_cloud_block_overlay_system.after(start_tribulation_system),
            emit_tribulation_boundary_vfx_system.after(tribulation_phase_tick_system),
            juebi_terrain_seed_system.after(emit_tribulation_boundary_vfx_system),
            juebi_terrain_tick_system.after(juebi_terrain_seed_system),
            tribulation_aoe_system.after(juebi_terrain_tick_system),
            juebi_phase_effect_system.after(tribulation_aoe_system),
            juebi_zone_aftershock_system.after(juebi_phase_effect_system),
            heart_demon_choice_system.after(juebi_zone_aftershock_system),
        ),
    );
    app.add_systems(
        Update,
        (
            heart_demon_timeout_system.after(heart_demon_choice_system),
            tribulation_failure_system.after(heart_demon_timeout_system),
            abort_du_xu_on_client_removed
                .after(tribulation_failure_system)
                .before(crate::player::despawn_disconnected_clients),
            tribulation_escape_boundary_system.after(abort_du_xu_on_client_removed),
            record_tribulation_interceptor_system
                .after(crate::combat::lifecycle::sync_combat_state_from_events),
            tribulation_wave_system.after(tribulation_escape_boundary_system),
            juebi_settlement_system.after(tribulation_wave_system),
            tribulation_intercept_death_system
                .after(crate::combat::lifecycle::death_arbiter_tick)
                .before(crate::inventory::apply_death_drop_on_revive),
        ),
    );
    app.add_systems(
        Update,
        (
            apply_meridian_overload_events.after(overload_detection_tick),
            meridian_heal_tick
                .after(apply_meridian_crack_events)
                .after(apply_meridian_overload_events),
        ),
    );
    app.add_systems(
        Update,
        (
            resolve_infuse_dugu_poison_intents,
            expire_dugu_state,
            on_attack_resolved_dugu_handler.after(crate::combat::resolve::resolve_attack_intents),
            dugu_poison_tick,
            dugu_poison_ambient_vfx_tick,
            resolve_self_antidote_intent,
            // plan-perception-v1.1 §4.1 server authoritative realm vision.
            push_initial_realm_vision.after(attach_cultivation_to_joined_clients),
            push_realm_vision_on_breakthrough.after(breakthrough_system),
            push_realm_vision_on_revive.after(on_player_revived),
            view_distance_ramp_system,
            push_spiritual_sense_targets.after(qi_regen_and_zone_drain_tick),
            cleanup_spiritual_sense_push_state,
            // plan §11-5 业力
            karma_decay_tick,
        ),
    );
    app.add_systems(
        Update,
        (
            process_lifespan_extension_intents.after(lifespan_aging_tick),
            sync_frailty_status_effects.after(process_lifespan_extension_intents),
            process_duo_she_requests
                .after(lifespan_aging_tick)
                .after(DuoSheIntentForwardSet),
            process_life_core_requests.after(process_duo_she_requests),
        ),
    );
    app.add_systems(
        Update,
        (
            // plan §5.4 / §5.5 顿悟流水线
            insight_trigger_on_breakthrough.after(breakthrough_system),
            insight_trigger_on_forge.after(forging_system),
            process_insight_request
                .after(insight_trigger_on_breakthrough)
                .after(insight_trigger_on_forge)
                .after(insight_trigger_on_wind_candle),
            insight_trigger_on_wind_candle.after(lifespan_aging_tick),
            apply_insight_chosen.after(process_insight_request),
        ),
    );
}

type CultivationAttachFilter = (
    Or<(Added<Client>, Added<CurrentDimension>)>,
    Without<Cultivation>,
);
type CultivationAttachQueryItem<'a> = (
    Entity,
    &'a Username,
    Option<&'a PlayerState>,
    Option<&'a LifespanComponent>,
);

fn parse_persisted_tribulation_dimension(value: &str) -> Option<DimensionKind> {
    match value {
        "minecraft:overworld" | "overworld" => Some(DimensionKind::Overworld),
        "bong:tsy" | "tsy" => Some(DimensionKind::Tsy),
        _ => None,
    }
}

fn attach_cultivation_to_joined_clients(
    mut commands: Commands,
    settings: Res<PersistenceSettings>,
    player_persistence: Option<Res<PlayerStatePersistence>>,
    joined_clients: Query<CultivationAttachQueryItem<'_>, CultivationAttachFilter>,
) {
    for (entity, username, player_state, restored_lifespan) in &joined_clients {
        let persisted_bundle = match load_player_cultivation_bundle(&settings, username.0.as_str())
        {
            Ok(value) => value,
            Err(error) => {
                tracing::warn!(
                    "[bong][cultivation] failed to load persisted cultivation bundle for `{}`: {error}",
                    username.0,
                );
                None
            }
        };

        let mut cultivation = Cultivation::default();
        let mut meridians = MeridianSystem::default();
        let mut qi_color = QiColor::default();
        let mut karma = Karma::default();
        let mut practice_log = PracticeLog::default();
        let mut contamination = Contamination::default();
        let canonical_id = player_persistence
            .as_deref()
            .and_then(|persistence| {
                load_current_character_id(persistence, username.0.as_str())
                    .ok()
                    .flatten()
            })
            .map(|current_char_id| player_character_id(username.0.as_str(), &current_char_id))
            .unwrap_or_else(|| canonical_player_id(username.0.as_str()));
        let mut life_record = LifeRecord::new(canonical_id.clone());
        let mut insight_quota = InsightQuota::default();
        let mut unlocked_perceptions = UnlockedPerceptions::default();
        let mut insight_modifiers = InsightModifiers::new();

        if let Some(persisted_bundle) = persisted_bundle.as_ref() {
            // Best-effort hydration; schema is versioned and may evolve.
            if let Some(value) = persisted_bundle.get("cultivation") {
                match serde_json::from_value::<Cultivation>(value.clone()) {
                    Ok(decoded) => cultivation = decoded,
                    Err(error) => {
                        warn_cultivation_decode(username.0.as_str(), "cultivation", error)
                    }
                }
            }
            if let Some(value) = persisted_bundle.get("meridians") {
                match serde_json::from_value::<MeridianSystem>(value.clone()) {
                    Ok(decoded) => meridians = decoded,
                    Err(error) => warn_cultivation_decode(username.0.as_str(), "meridians", error),
                }
            }
            if let Some(value) = persisted_bundle.get("qi_color") {
                match serde_json::from_value::<QiColor>(value.clone()) {
                    Ok(decoded) => qi_color = decoded,
                    Err(error) => warn_cultivation_decode(username.0.as_str(), "qi_color", error),
                }
            }
            if let Some(value) = persisted_bundle.get("karma") {
                match serde_json::from_value::<Karma>(value.clone()) {
                    Ok(decoded) => karma = decoded,
                    Err(error) => warn_cultivation_decode(username.0.as_str(), "karma", error),
                }
            }
            if let Some(value) = persisted_bundle.get("practice_log") {
                match serde_json::from_value::<PracticeLog>(value.clone()) {
                    Ok(decoded) => practice_log = decoded,
                    Err(error) => {
                        warn_cultivation_decode(username.0.as_str(), "practice_log", error)
                    }
                }
            }
            if let Some(value) = persisted_bundle.get("contamination") {
                match serde_json::from_value::<Contamination>(value.clone()) {
                    Ok(decoded) => contamination = decoded,
                    Err(error) => {
                        warn_cultivation_decode(username.0.as_str(), "contamination", error)
                    }
                }
            }
            if let Some(value) = persisted_bundle.get("life_record") {
                match serde_json::from_value::<LifeRecord>(value.clone()) {
                    Ok(decoded) => life_record = decoded,
                    Err(error) => {
                        warn_cultivation_decode(username.0.as_str(), "life_record", error)
                    }
                }
            }
            if let Some(value) = persisted_bundle.get("insight_quota") {
                match serde_json::from_value::<InsightQuota>(value.clone()) {
                    Ok(decoded) => insight_quota = decoded,
                    Err(error) => {
                        warn_cultivation_decode(username.0.as_str(), "insight_quota", error)
                    }
                }
            }
            if let Some(value) = persisted_bundle.get("unlocked_perceptions") {
                match serde_json::from_value::<UnlockedPerceptions>(value.clone()) {
                    Ok(decoded) => unlocked_perceptions = decoded,
                    Err(error) => {
                        warn_cultivation_decode(username.0.as_str(), "unlocked_perceptions", error)
                    }
                }
            }
            if let Some(value) = persisted_bundle.get("insight_modifiers") {
                match serde_json::from_value::<InsightModifiers>(value.clone()) {
                    Ok(decoded) => insight_modifiers = decoded,
                    Err(error) => {
                        warn_cultivation_decode(username.0.as_str(), "insight_modifiers", error)
                    }
                }
            }
        } else if player_state.is_some() {
            tracing::debug!(
                "[bong][cultivation] no persisted cultivation bundle for `{}`; using defaults",
                username.0,
            );
        }

        let active_tribulation = match load_active_tribulation(&settings, canonical_id.as_str()) {
            Ok(record) => record,
            Err(error) => {
                tracing::warn!(
                    "[bong][cultivation] failed to load active tribulation for {}: {error}",
                    canonical_id,
                );
                None
            }
        };
        let restored_origin_dimension = active_tribulation.as_ref().and_then(|record| {
            record
                .origin_dimension
                .as_deref()
                .and_then(|origin_dimension| {
                    parse_persisted_tribulation_dimension(origin_dimension).or_else(|| {
                        tracing::warn!(
                            "[bong][cultivation] unknown persisted tribulation origin dimension `{}` for char_id={} kind={}",
                            origin_dimension,
                            record.char_id,
                            record.kind,
                        );
                        None
                    })
                })
        });
        let restored_tribulation = active_tribulation.as_ref().map(|record| {
            TribulationState::restored_for_kind(
                record.kind.as_str(),
                record
                    .wave_current
                    .saturating_add(1)
                    .min(record.waves_total),
                record.waves_total,
                record.started_tick,
                record.epicenter,
            )
        });
        let restored_juebi_runtime = active_tribulation
            .as_ref()
            .filter(|record| record.kind == "jue_bi")
            .map(|record| {
                let source = JueBiTriggerSource::from_wire_name(record.source.as_str())
                    .unwrap_or_else(|| {
                        tracing::warn!(
                            "[bong][cultivation] unknown JueBi trigger source `{}` for active tribulation char_id={} kind={}; falling back to void_quota_exceeded",
                            record.source,
                            record.char_id,
                            record.kind,
                        );
                        JueBiTriggerSource::VoidQuotaExceeded
                    });
                JueBiRuntimeContext {
                    source,
                    intensity: if record.intensity > 0.0 {
                        record.intensity
                    } else {
                        tribulation::JUEBI_INTENSITY_BASE
                    },
                }
            });
        if active_tribulation
            .as_ref()
            .is_some_and(|record| record.kind == "du_xu")
        {
            cultivation.realm = Realm::Spirit;
        }
        let default_lifespan =
            LifespanComponent::new(LifespanCapTable::for_realm(cultivation.realm));

        let mut severed_permanent = MeridianSeveredPermanent::default();
        if let Some(persisted_bundle) = persisted_bundle.as_ref() {
            if let Some(value) = persisted_bundle.get("meridian_severed") {
                match serde_json::from_value::<MeridianSeveredPermanent>(value.clone()) {
                    Ok(decoded) => severed_permanent = decoded,
                    Err(error) => {
                        warn_cultivation_decode(username.0.as_str(), "meridian_severed", error)
                    }
                }
            }
        }

        let mut entity_commands = commands.entity(entity);
        entity_commands.insert((
            cultivation,
            meridians,
            qi_color,
            karma,
            practice_log,
            contamination,
            life_record,
            DeathRegistry::new(canonical_id.clone()),
            LifespanExtensionLedger::default(),
            insight_quota,
            unlocked_perceptions,
            insight_modifiers,
            DuguPractice::default(),
            severed_permanent,
        ));
        if restored_lifespan.is_none() {
            entity_commands.insert(default_lifespan);
        }
        if let Some(restored_tribulation) = restored_tribulation {
            entity_commands.insert(restored_tribulation);
        }
        if let Some(restored_origin_dimension) = restored_origin_dimension {
            entity_commands.insert(TribulationOriginDimension(restored_origin_dimension));
        }
        if let Some(restored_juebi_runtime) = restored_juebi_runtime {
            entity_commands.insert(restored_juebi_runtime);
        }
        tracing::info!("[bong][cultivation] attached full cultivation bundle to {entity:?}");
    }
}

fn warn_cultivation_decode(username: &str, slice: &str, error: serde_json::Error) {
    tracing::warn!(
        "[bong][cultivation] failed to decode persisted {slice} slice for `{username}`: {error}"
    );
}

fn emit_skill_caps_on_realm_regressed(
    settings: Res<PersistenceSettings>,
    mut regressed: EventReader<RealmRegressed>,
    mut quota_opened: EventWriter<AscensionQuotaOpened>,
    mut skill_cap_events: EventWriter<SkillCapChanged>,
) {
    for event in regressed.read() {
        if event.from == Realm::Void && event.to != Realm::Void {
            match release_ascension_quota_slot(&settings) {
                Ok(release) if release.opened_slot => {
                    quota_opened.send(AscensionQuotaOpened {
                        occupied_slots: release.quota.occupied_slots,
                    });
                }
                Ok(_) => {}
                Err(error) => {
                    tracing::warn!(
                        "[bong][cultivation] failed to release ascension quota after realm regression for {:?}: {error}",
                        event.entity,
                    );
                }
            }
        }
        let new_cap = breakthrough::skill_cap_for_realm(event.to);
        for skill in crate::skill::components::SkillId::ALL {
            skill_cap_events.send(SkillCapChanged {
                char_entity: event.entity,
                skill,
                new_cap,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::combat::components::Lifecycle;
    use crate::cultivation::lifespan::{DeathRegistry, LifespanCapTable, LifespanComponent};
    use crate::persistence::{
        load_active_tribulation, load_ascension_quota, persist_active_tribulation,
        ActiveTribulationRecord, PersistenceSettings,
    };
    use crate::player::state::canonical_player_id;
    use crate::player::state::PlayerState;
    use crate::skill::events::SkillCapChanged;
    use crate::world::dimension::DimensionKind;
    use valence::prelude::App;
    use valence::testing::create_mock_client;

    fn temp_persistence_settings(test_name: &str) -> (PersistenceSettings, std::path::PathBuf) {
        let temp_root = std::env::temp_dir().join(format!(
            "bong-cultivation-{test_name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos(),
        ));
        let db_path = temp_root.join("data").join("bong.db");
        let deceased_dir = temp_root
            .join("library-web")
            .join("public")
            .join("deceased");
        let settings = PersistenceSettings::with_paths(&db_path, &deceased_dir, "cultivation-test");
        crate::persistence::bootstrap_sqlite(settings.db_path(), settings.server_run_id())
            .expect("bootstrap should succeed");
        (settings, temp_root)
    }

    #[test]
    fn joined_clients_receive_canonical_player_character_id() {
        let mut app = App::new();
        app.insert_resource(PersistenceSettings::default());
        app.add_systems(Update, attach_cultivation_to_joined_clients);

        let (client_bundle, _helper) = create_mock_client("Alice");
        let entity = app.world_mut().spawn(client_bundle).id();

        app.update();

        let life_record = app
            .world()
            .get::<LifeRecord>(entity)
            .expect("joined client should receive a LifeRecord");
        let death_registry = app
            .world()
            .get::<DeathRegistry>(entity)
            .expect("joined client should receive a DeathRegistry");
        let lifespan = app
            .world()
            .get::<LifespanComponent>(entity)
            .expect("joined client should receive a LifespanComponent");

        assert_eq!(life_record.character_id, canonical_player_id("Alice"));
        assert_eq!(death_registry.char_id, canonical_player_id("Alice"));
        assert_eq!(lifespan.cap_by_realm, LifespanCapTable::AWAKEN);
    }

    #[test]
    fn joined_client_defaults_to_awaken_lifespan_cap() {
        let mut app = App::new();
        app.insert_resource(PersistenceSettings::default());
        app.add_systems(Update, attach_cultivation_to_joined_clients);

        let (client_bundle, _helper) = create_mock_client("Novice");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                PlayerState {
                    karma: 0.0,
                    inventory_score: 0.0,
                },
            ))
            .id();

        app.update();

        let lifespan = app
            .world()
            .get::<LifespanComponent>(entity)
            .expect("joined client should receive a LifespanComponent");

        assert_eq!(lifespan.cap_by_realm, LifespanCapTable::AWAKEN);
    }

    #[test]
    fn joined_clients_keep_restored_lifespan_component() {
        let mut app = App::new();
        app.insert_resource(PersistenceSettings::default());
        app.add_systems(Update, attach_cultivation_to_joined_clients);

        let restored_lifespan = LifespanComponent {
            born_at_tick: 120,
            years_lived: 42.0,
            cap_by_realm: LifespanCapTable::SPIRIT,
            offline_pause_tick: Some(30),
        };
        let (client_bundle, _helper) = create_mock_client("Persisted");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                PlayerState {
                    karma: 0.0,
                    inventory_score: 0.0,
                },
                restored_lifespan.clone(),
            ))
            .id();

        app.update();

        let lifespan = app
            .world()
            .get::<LifespanComponent>(entity)
            .expect("joined client should keep a LifespanComponent");

        assert_eq!(lifespan, &restored_lifespan);
    }

    #[test]
    fn joined_clients_restore_active_tribulation_from_persistence() {
        let temp_root = std::env::temp_dir().join(format!(
            "bong-cultivation-tribulation-restore-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos(),
        ));
        let db_path = temp_root.join("data").join("bong.db");
        let deceased_dir = temp_root
            .join("library-web")
            .join("public")
            .join("deceased");
        let settings = PersistenceSettings::with_paths(&db_path, &deceased_dir, "cultivation-test");
        crate::persistence::bootstrap_sqlite(settings.db_path(), settings.server_run_id())
            .expect("bootstrap should succeed");
        persist_active_tribulation(
            &settings,
            &ActiveTribulationRecord {
                char_id: canonical_player_id("Alice"),
                kind: "du_xu".to_string(),
                source: String::new(),
                origin_dimension: Some("minecraft:overworld".to_string()),
                wave_current: 2,
                waves_total: 5,
                started_tick: 1440,
                epicenter: [0.0, 64.0, 0.0],
                intensity: 0.0,
            },
        )
        .expect("active tribulation should persist");

        let mut app = App::new();
        app.insert_resource(settings);
        app.add_systems(Update, attach_cultivation_to_joined_clients);

        let (client_bundle, _helper) = create_mock_client("Alice");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                PlayerState {
                    karma: 0.0,
                    inventory_score: 0.0,
                },
            ))
            .id();

        app.update();

        let cultivation = app
            .world()
            .get::<Cultivation>(entity)
            .expect("cultivation should attach");
        let tribulation = app
            .world()
            .get::<TribulationState>(entity)
            .expect("tribulation should restore");
        assert_eq!(cultivation.realm, Realm::Spirit);
        assert_eq!(tribulation.wave_current, 3);
        assert_eq!(tribulation.waves_total, 5);
        assert_eq!(tribulation.started_tick, 1440);
        let origin = app
            .world()
            .get::<TribulationOriginDimension>(entity)
            .expect("tribulation origin dimension should restore");
        assert_eq!(origin.0, DimensionKind::Overworld);

        let _ = std::fs::remove_dir_all(temp_root);
    }

    #[test]
    fn joined_clients_restore_persisted_tribulation_origin_dimension() {
        let temp_root = std::env::temp_dir().join(format!(
            "bong-cultivation-tribulation-restore-dim-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos(),
        ));
        let db_path = temp_root.join("data").join("bong.db");
        let deceased_dir = temp_root
            .join("library-web")
            .join("public")
            .join("deceased");
        let settings = PersistenceSettings::with_paths(&db_path, &deceased_dir, "cultivation-test");
        crate::persistence::bootstrap_sqlite(settings.db_path(), settings.server_run_id())
            .expect("bootstrap should succeed");
        persist_active_tribulation(
            &settings,
            &ActiveTribulationRecord {
                char_id: canonical_player_id("Azure"),
                kind: "du_xu".to_string(),
                source: String::new(),
                origin_dimension: Some("minecraft:overworld".to_string()),
                wave_current: 2,
                waves_total: 5,
                started_tick: 1440,
                epicenter: [0.0, 64.0, 0.0],
                intensity: 0.0,
            },
        )
        .expect("active tribulation should persist");

        let mut app = App::new();
        app.insert_resource(settings);
        app.add_systems(Update, attach_cultivation_to_joined_clients);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                CurrentDimension(DimensionKind::Tsy),
                PlayerState {
                    karma: 0.0,
                    inventory_score: 0.0,
                },
            ))
            .id();

        app.update();

        let origin = app
            .world()
            .get::<TribulationOriginDimension>(entity)
            .expect("tribulation origin dimension should restore");
        assert_eq!(origin.0, DimensionKind::Overworld);

        let _ = std::fs::remove_dir_all(temp_root);
    }

    #[test]
    fn joined_clients_do_not_bind_missing_tribulation_origin_to_current_dimension() {
        let temp_root = std::env::temp_dir().join(format!(
            "bong-cultivation-tribulation-restore-no-dim-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos(),
        ));
        let db_path = temp_root.join("data").join("bong.db");
        let deceased_dir = temp_root
            .join("library-web")
            .join("public")
            .join("deceased");
        let settings = PersistenceSettings::with_paths(&db_path, &deceased_dir, "cultivation-test");
        crate::persistence::bootstrap_sqlite(settings.db_path(), settings.server_run_id())
            .expect("bootstrap should succeed");
        persist_active_tribulation(
            &settings,
            &ActiveTribulationRecord {
                char_id: canonical_player_id("Azure"),
                kind: "du_xu".to_string(),
                source: String::new(),
                origin_dimension: None,
                wave_current: 2,
                waves_total: 5,
                started_tick: 1440,
                epicenter: [0.0, 64.0, 0.0],
                intensity: 0.0,
            },
        )
        .expect("legacy active tribulation should persist without origin dimension");

        let mut app = App::new();
        app.insert_resource(settings);
        app.add_systems(Update, attach_cultivation_to_joined_clients);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                CurrentDimension(DimensionKind::Tsy),
                PlayerState {
                    karma: 0.0,
                    inventory_score: 0.0,
                },
            ))
            .id();

        app.update();

        assert!(
            app.world().get::<TribulationOriginDimension>(entity).is_none(),
            "legacy rows without origin_dimension should defer origin binding instead of using current dimension"
        );

        let _ = std::fs::remove_dir_all(temp_root);
    }

    #[test]
    fn joined_clients_restore_juebi_active_tribulation_kind() {
        let temp_root = std::env::temp_dir().join(format!(
            "bong-cultivation-juebi-restore-kind-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos(),
        ));
        let db_path = temp_root.join("data").join("bong.db");
        let deceased_dir = temp_root
            .join("library-web")
            .join("public")
            .join("deceased");
        let settings = PersistenceSettings::with_paths(&db_path, &deceased_dir, "cultivation-test");
        crate::persistence::bootstrap_sqlite(settings.db_path(), settings.server_run_id())
            .expect("bootstrap should succeed");
        persist_active_tribulation(
            &settings,
            &ActiveTribulationRecord {
                char_id: canonical_player_id("Azure"),
                kind: "jue_bi".to_string(),
                source: "void_action_explode_zone".to_string(),
                origin_dimension: Some("bong:tsy".to_string()),
                wave_current: 1,
                waves_total: 3,
                started_tick: 2880,
                epicenter: [12.0, 66.0, -3.0],
                intensity: 1.6,
            },
        )
        .expect("active JueBi should persist");

        let mut app = App::new();
        app.insert_resource(settings);
        app.add_systems(Update, attach_cultivation_to_joined_clients);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                PlayerState {
                    karma: 0.0,
                    inventory_score: 0.0,
                },
            ))
            .id();

        app.update();

        let cultivation = app
            .world()
            .get::<Cultivation>(entity)
            .expect("cultivation should attach");
        let tribulation = app
            .world()
            .get::<TribulationState>(entity)
            .expect("JueBi should restore");
        assert_eq!(cultivation.realm, Realm::Awaken);
        assert_eq!(tribulation.kind, tribulation::TribulationKind::JueBi);
        assert_eq!(tribulation.epicenter, [12.0, 66.0, -3.0]);
        let origin = app
            .world()
            .get::<TribulationOriginDimension>(entity)
            .expect("JueBi origin dimension should restore");
        assert_eq!(origin.0, DimensionKind::Tsy);
        let runtime = app
            .world()
            .get::<JueBiRuntimeContext>(entity)
            .expect("JueBi runtime context should restore");
        assert_eq!(runtime.source, JueBiTriggerSource::VoidActionExplodeZone);
        assert_eq!(runtime.intensity, 1.6);

        let _ = std::fs::remove_dir_all(temp_root);
    }

    #[test]
    fn joined_clients_cap_restored_auto_pass_wave_at_total_waves() {
        let temp_root = std::env::temp_dir().join(format!(
            "bong-cultivation-tribulation-restore-cap-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos(),
        ));
        let db_path = temp_root.join("data").join("bong.db");
        let deceased_dir = temp_root
            .join("library-web")
            .join("public")
            .join("deceased");
        let settings = PersistenceSettings::with_paths(&db_path, &deceased_dir, "cultivation-test");
        crate::persistence::bootstrap_sqlite(settings.db_path(), settings.server_run_id())
            .expect("bootstrap should succeed");
        persist_active_tribulation(
            &settings,
            &ActiveTribulationRecord {
                char_id: canonical_player_id("Azure"),
                kind: "du_xu".to_string(),
                source: String::new(),
                origin_dimension: Some("minecraft:overworld".to_string()),
                wave_current: 5,
                waves_total: 5,
                started_tick: 1888,
                epicenter: [0.0, 64.0, 0.0],
                intensity: 0.0,
            },
        )
        .expect("active tribulation should persist");

        let mut app = App::new();
        app.insert_resource(settings);
        app.add_systems(Update, attach_cultivation_to_joined_clients);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                PlayerState {
                    karma: 0.0,
                    inventory_score: 0.0,
                },
            ))
            .id();

        app.update();

        let tribulation = app
            .world()
            .get::<TribulationState>(entity)
            .expect("tribulation should restore");
        assert_eq!(tribulation.wave_current, 5);
        assert_eq!(tribulation.waves_total, 5);
        assert_eq!(tribulation.started_tick, 1888);

        let _ = std::fs::remove_dir_all(temp_root);
    }

    #[test]
    fn restored_tribulation_completion_clears_active_row_and_awards_quota() {
        let temp_root = std::env::temp_dir().join(format!(
            "bong-cultivation-tribulation-restore-complete-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos(),
        ));
        let db_path = temp_root.join("data").join("bong.db");
        let deceased_dir = temp_root
            .join("library-web")
            .join("public")
            .join("deceased");
        let settings = PersistenceSettings::with_paths(&db_path, &deceased_dir, "cultivation-test");
        crate::persistence::bootstrap_sqlite(settings.db_path(), settings.server_run_id())
            .expect("bootstrap should succeed");
        persist_active_tribulation(
            &settings,
            &ActiveTribulationRecord {
                char_id: canonical_player_id("Azure"),
                kind: "du_xu".to_string(),
                source: String::new(),
                origin_dimension: Some("minecraft:overworld".to_string()),
                wave_current: 4,
                waves_total: 5,
                started_tick: 2880,
                epicenter: [0.0, 64.0, 0.0],
                intensity: 0.0,
            },
        )
        .expect("active tribulation should persist");

        let mut app = App::new();
        app.insert_resource(settings.clone());
        app.add_event::<tribulation::TribulationWaveCleared>();
        app.add_event::<tribulation::TribulationSettled>();
        app.add_event::<tribulation::JueBiTriggeredEvent>();
        app.add_event::<tribulation::AscensionQuotaOccupied>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<crate::skill::events::SkillCapChanged>();
        app.add_systems(
            Update,
            (
                attach_cultivation_to_joined_clients,
                tribulation::tribulation_wave_system,
            ),
        );

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                PlayerState {
                    karma: 0.0,
                    inventory_score: 0.0,
                },
                Lifecycle {
                    character_id: canonical_player_id("Azure"),
                    death_count: 0,
                    fortune_remaining: 1,
                    last_death_tick: None,
                    last_revive_tick: None,
                    spawn_anchor: None,
                    near_death_deadline_tick: None,
                    awaiting_decision: None,
                    revival_decision_deadline_tick: None,
                    weakened_until_tick: None,
                    state: crate::combat::components::LifecycleState::Alive,
                },
            ))
            .id();

        app.update();

        let restored = app
            .world()
            .get::<tribulation::TribulationState>(entity)
            .expect("tribulation should restore");
        assert_eq!(restored.wave_current, 5);
        assert_eq!(restored.waves_total, 5);

        app.world_mut()
            .resource_mut::<valence::prelude::Events<tribulation::TribulationWaveCleared>>()
            .send(tribulation::TribulationWaveCleared { entity, wave: 5 });

        app.update();

        let cultivation = app
            .world()
            .get::<Cultivation>(entity)
            .expect("cultivation should still be attached");
        assert_eq!(cultivation.realm, Realm::Void);
        assert!(
            app.world()
                .get::<tribulation::TribulationState>(entity)
                .is_none(),
            "tribulation state should be removed after ascension"
        );

        let active = load_active_tribulation(&settings, canonical_player_id("Azure").as_str())
            .expect("active tribulation query should succeed");
        assert!(active.is_none(), "active tribulation row should be cleared");

        let quota = load_ascension_quota(&settings).expect("quota load should succeed");
        assert_eq!(quota.occupied_slots, 1);

        let _ = std::fs::remove_dir_all(temp_root);
    }

    #[test]
    fn realm_regressed_emits_cap_changed_for_all_skills() {
        let mut app = App::new();
        app.insert_resource(PersistenceSettings::default());
        app.add_event::<RealmRegressed>();
        app.add_event::<AscensionQuotaOpened>();
        app.add_event::<SkillCapChanged>();
        app.add_systems(Update, emit_skill_caps_on_realm_regressed);

        let entity = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(RealmRegressed {
            entity,
            from: Realm::Spirit,
            to: Realm::Solidify,
            closed_meridians: 2,
        });
        app.update();

        let caps: Vec<_> = app
            .world_mut()
            .resource_mut::<valence::prelude::Events<SkillCapChanged>>()
            .drain()
            .collect();
        assert_eq!(caps.len(), crate::skill::components::SkillId::ALL.len());
        assert!(caps.iter().all(|e| e.new_cap == 8));
    }

    #[test]
    fn void_realm_regression_releases_ascension_quota() {
        let (settings, root) = temp_persistence_settings("void-regression-release-quota");
        persist_active_tribulation(
            &settings,
            &ActiveTribulationRecord {
                char_id: canonical_player_id("Azure"),
                kind: "du_xu".to_string(),
                source: String::new(),
                origin_dimension: Some("minecraft:overworld".to_string()),
                wave_current: 3,
                waves_total: 3,
                started_tick: 10,
                epicenter: [0.0, 64.0, 0.0],
                intensity: 0.0,
            },
        )
        .expect("active tribulation should persist before quota setup");
        crate::persistence::complete_tribulation_ascension(
            &settings,
            canonical_player_id("Azure").as_str(),
        )
        .expect("quota setup should succeed");

        let mut app = App::new();
        app.insert_resource(settings.clone());
        app.add_event::<RealmRegressed>();
        app.add_event::<AscensionQuotaOpened>();
        app.add_event::<SkillCapChanged>();
        app.add_systems(Update, emit_skill_caps_on_realm_regressed);

        let entity = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(RealmRegressed {
            entity,
            from: Realm::Void,
            to: Realm::Spirit,
            closed_meridians: 8,
        });

        app.update();

        let quota = load_ascension_quota(&settings).expect("quota load should succeed");
        assert_eq!(quota.occupied_slots, 0);
        let quota_events: Vec<_> = app
            .world_mut()
            .resource_mut::<valence::prelude::Events<AscensionQuotaOpened>>()
            .drain()
            .collect();
        assert_eq!(quota_events.len(), 1);
        assert_eq!(quota_events[0].occupied_slots, 0);

        let _ = std::fs::remove_dir_all(root);
    }
}
