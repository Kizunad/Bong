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
pub mod breakthrough_cinematic;
pub mod burst_meridian;
pub mod character_lifecycle;
pub mod character_select;
pub mod color;
pub mod color_affinity;
pub mod color_bonus;
pub mod components;
pub mod composure;
pub mod contamination;
pub mod dead_zone;
pub mod death_hooks;
pub mod dugu;
pub mod epitaph;
pub mod first_hit_dash;
pub mod forging;
pub mod full_power_strike;
pub mod generic_talent;
pub mod heal;
pub mod insight;
pub mod insight_apply;
pub mod insight_fallback;
pub mod insight_flavor;
pub mod insight_flow;
pub mod karma;
pub mod known_techniques;
pub mod life_record;
pub mod lifespan;
pub mod luck_pool;
pub mod meridian;
pub mod meridian_open;
// plan-race-system-v1 P1 对抗审查 M2 —— 非人合成构型全链测试。
pub mod neg_pressure;
pub mod negative_zone;
#[cfg(test)]
mod non_humanoid_meridian_synthetic_chain_test;
pub mod overload;
pub mod perception;
pub mod poison_trait;
pub mod possession;
pub mod practice_session;
pub mod qi_field;
pub mod qi_zero_decay;
pub mod race_change;
pub mod realm_taint;
pub mod realm_vision;
pub mod skill_registry;
pub mod special_talent;
pub mod spiritual_sense;
pub mod style_modifier;
// plan-skill-anim-fidelity-v1 P0 —— technique cast_ticks 快照单向同步测试。
#[cfg(test)]
mod technique_cast_ticks_snapshot_test;
// plan-skill-av-relink-v1 P3 —— technique 图标快照单向同步 + 映射约束测试。
#[cfg(test)]
mod technique_icon_snapshot_test;
pub mod technique_mentor;
pub mod technique_observe;
pub mod technique_proficiency;
pub mod technique_scroll;
pub mod tick;
pub mod topology;
pub mod tribulation;
// plan-tribulation-balance-v1 P0：平衡监控配置 Resource
pub mod tribulation_balance;
pub mod void;

use valence::entity::entity::Flags;
use valence::prelude::bevy_ecs::system::SystemParam;
use valence::prelude::{
    bevy_ecs, Added, App, Client, Commands, Component, Entity, EntityLayerId, EventReader,
    EventWriter, Events, IntoSystemConfigs, Or, Position, Query, Res, ResMut, Update, Username,
    VisibleChunkLayer, VisibleEntityLayers, With, Without,
};

use self::breakthrough::{
    breakthrough_system, rapid_breakthrough_karma_mark_system, BreakthroughOutcome,
    BreakthroughRequest,
};
use self::breakthrough_cinematic::{
    breakthrough_cinematic_phase_tick, interrupt_breakthrough_cinematic_on_hit,
    start_breakthrough_cinematic_on_outcome, BreakthroughCinematicAgentEvent,
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
use self::known_techniques::{KnownTechniques, KnownTechniquesLoadFailed};
use self::life_record::{BiographyEntry, LifeRecord};
use self::lifespan::{
    lifespan_aging_tick, process_lifespan_extension_intents, sync_frailty_status_effects,
    AgingEventEmitted, DeathRegistry, LifespanCapTable, LifespanComponent, LifespanEventEmitted,
    LifespanExtensionIntent, LifespanExtensionLedger,
};
use self::meridian::severed::{
    apply_severed_event_system, meridian_severed_detection_tick, MeridianSeveredEvent,
    MeridianSeveredPermanent, SkillMeridianDependencies,
};
use self::meridian_open::{meridian_open_tick, MeridianOpenedEvent};
use self::neg_pressure::tick_neg_pressure;
use self::negative_zone::negative_zone_siphon_tick;
use self::overload::{
    apply_meridian_crack_events, apply_meridian_overload_events, overload_detection_tick,
    MeridianCrackEvent, MeridianOverloadEvent,
};
use self::perception::passive_qi_color_scan_system;
use self::poison_trait::{
    apply_poison_overdose_costs, consume_poison_pill_system, digestion_load_decay_tick,
    poison_toxicity_decay_tick, ConsumePoisonPillIntent, DigestionLoad, DigestionOverloadEvent,
    PoisonDoseEvent, PoisonOverdoseEvent, PoisonPowderConsumedEvent, PoisonToxicity,
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
use self::technique_proficiency::{track_woliu_proficiency_from_casts, TechniqueMasteredEvent};
use self::technique_scroll::{TechniqueLearnedEvent, TechniqueScrollReadEvent};
use self::tick::{
    prune_cultivation_session_practice_accumulator, qi_regen_and_zone_drain_tick, CultivationClock,
    CultivationSessionPracticeAccumulator,
};
use self::tribulation::{
    abort_du_xu_on_client_removed, dispatch_rechallenge_on_quota_opened_system,
    emit_tribulation_boundary_vfx_system, heart_demon_choice_system, heart_demon_timeout_system,
    juebi_phase_effect_system, juebi_settlement_system, juebi_terrain_seed_system,
    juebi_terrain_tick_system, juebi_zone_aftershock_system, record_tribulation_interceptor_system,
    schedule_juebi_triggers_system, start_du_xu_request_system, start_due_juebi_triggers_system,
    start_tribulation_system, track_quota_full_duration_system, track_tribulation_metrics_system,
    tribulation_aoe_system, tribulation_escape_boundary_system, tribulation_failure_system,
    tribulation_intercept_death_system, tribulation_omen_cloud_block_overlay_system,
    tribulation_phase_tick_system, tribulation_wave_system, AscensionQuotaOccupied,
    AscensionQuotaOpened, HalfStepRechallengeQueue, HalfStepRechallengeTriggerEvent,
    HeartDemonChoiceSubmitted, InitiateXuhuaTribulation, JueBiRuntimeContext, JueBiTerrainOverlay,
    JueBiTriggerEvent, JueBiTriggerSource, JueBiTriggeredEvent, JueBiZoneAftershocks,
    PendingJueBiTriggers, QuotaFullTracker, StartDuXuRequest, TribulationAnnounce,
    TribulationFailed, TribulationFled, TribulationLocked, TribulationMetrics,
    TribulationOmenCloudBlocks, TribulationOriginDimension, TribulationSettled, TribulationState,
    TribulationWaveCleared,
};
use crate::body_plan::RaceId;
use crate::coffin::{clear_player_coffin_runtime, CoffinRegistry, CoffinStateChanged};
use crate::combat::CombatClock;
use crate::cultivation::components::Realm;
use crate::nourishment::Nourishment;
use crate::npc::possession::DuoSheIntentForwardSet;
use crate::persistence::{
    load_active_tribulation, load_player_cultivation_bundle, persist_new_character_transition,
    release_ascension_quota_slot, NewCharacterPersistenceBundle, PersistenceSettings,
    PlayerCultivationBundle,
};
use crate::player::state::{
    canonical_player_id, load_current_character_id, load_player_lifecycle_slice,
    player_character_id, PlayerState, PlayerStatePersistence,
};
#[cfg(test)]
use crate::qi_physics::{pending_inflow_account, QiAccountId};
use crate::qi_physics::{QiTransfer, WorldQiAccount};
use crate::skill::events::SkillCapChanged;
use crate::tribulation::scorch_record::{
    record_tribulation_scorch_system, TribulationScorchRecords,
};
use crate::world::dimension::{
    publish_overworld_runtime, CurrentDimension, DimensionKind, DimensionLayers,
    OverworldVisibilityPolicy,
};
use crate::world::karma::{karma_weight_decay_tick, void_realm_karma_pressure_tick};
use crate::world::spawn_tutorial::{TutorialState, TutorialTelemetry};
use crate::world::zone::ZoneRegistry;

pub fn register(app: &mut App) {
    tracing::info!("[bong][cultivation] registering cultivation systems (plan P1–P5)");
    let mut skill_meridian_dependencies = SkillMeridianDependencies::default();
    crate::combat::zhenmai_v2::declare_meridian_dependencies(&mut skill_meridian_dependencies);
    crate::combat::anqi_v2::declare_meridian_dependencies(&mut skill_meridian_dependencies);
    crate::combat::dugu_v2::declare_meridian_dependencies(&mut skill_meridian_dependencies);
    crate::combat::tuike_v2::declare_meridian_dependencies(&mut skill_meridian_dependencies);
    crate::combat::sword_basics::declare_meridian_dependencies(&mut skill_meridian_dependencies);
    // plan-shield-block-v1 P4：盾牌格挡不依赖任何经脉（凡人物理防御）。
    crate::combat::shield_block::declare_meridian_dependencies(&mut skill_meridian_dependencies);
    crate::sword_path::skill_register::declare_meridian_dependencies(
        &mut skill_meridian_dependencies,
    );
    crate::movement::dash_proficiency::declare_dash_meridian_dependencies(
        &mut skill_meridian_dependencies,
    );
    crate::npc::npc_skill::declare_npc_skill_meridian_deps(&mut skill_meridian_dependencies);
    // GAP-1 fix: woliu.vortex 依赖 Lung（手太阴肺经），resolver 同步加 check gate。
    crate::combat::woliu::declare_meridian_dependencies(&mut skill_meridian_dependencies);
    // GAP-2 fix: burst_meridian.beng_quan 依赖手三阳（LargeIntestine/SmallIntestine/TripleEnergizer）。
    crate::cultivation::burst_meridian::declare_meridian_dependencies(
        &mut skill_meridian_dependencies,
    );
    // GAP-3 fix: yidao 五招补入审计表（功能门已在 resolver 内部实现，此处补完整性声明）。
    crate::combat::yidao::declare_meridian_dependencies(&mut skill_meridian_dependencies);
    // GAP-4 fix: dandao 三招补入审计表（功能门已在 resolver 内部实现，此处补完整性声明）。
    crate::dandao::declare_meridian_dependencies(&mut skill_meridian_dependencies);
    // dugu 两招无经脉前置，显式声明空 deps 以满足审计完整性不变量。
    crate::cultivation::dugu::declare_meridian_dependencies(&mut skill_meridian_dependencies);
    // plan-race-system-v1 P4：morph.yixing 无经脉前置表条目（专属 form_anchors_open
    // 门在别处判定），显式声明空 deps 以满足审计完整性不变量。
    crate::body_plan::morph::declare_meridian_dependencies(&mut skill_meridian_dependencies);

    // plan-race-system-v1 P1b：`MeridianTopology` 不再是全局单例 Resource——拓扑数据
    // 按实体解析出的 BodyPlan 现场派生（见 `body_plan::resolve_meridian_topology_for_target`）。
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
    app.init_resource::<TribulationScorchRecords>();
    app.insert_resource(self::tribulation::VoidQuotaConfig::from_env());
    // plan-halfstep-buff-v1 P0/P3：渡虚劫遥测 + quota 满时长追踪 + 重渡 FIFO 队列
    app.init_resource::<TribulationMetrics>();
    app.init_resource::<QuotaFullTracker>();
    app.init_resource::<HalfStepRechallengeQueue>();
    app.add_event::<HalfStepRechallengeTriggerEvent>();
    // plan-tribulation-balance-v1 P0：平衡监控配置 Resource（只读看板，初始值镜像 tribulation.rs 常数）
    app.init_resource::<tribulation_balance::TribulationBalanceConfig>();
    app.insert_resource(SpiritualSensePushState::default());
    realm_taint::register(app);
    void::register(app);
    full_power_strike::register(app);
    // plan-life-record-epitaph-v1 P0：碑刻生成系统
    epitaph::register(app);

    // 事件（plan §3/§4/§5 全家桶）
    app.add_event::<BreakthroughRequest>();
    app.add_event::<BreakthroughOutcome>();
    app.add_event::<BreakthroughCinematicAgentEvent>();
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
    app.add_event::<TechniqueScrollReadEvent>();
    app.add_event::<TechniqueLearnedEvent>();
    app.add_event::<TechniqueMasteredEvent>();
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
    app.add_event::<MeridianOpenedEvent>();
    app.add_event::<CultivationSessionPracticeEvent>();
    app.add_event::<InfuseDuguPoisonIntent>();
    app.add_event::<DuguObfuscationDisruptedEvent>();
    app.add_event::<DuguPoisonProgressEvent>();
    app.add_event::<SelfAntidoteIntent>();
    app.add_event::<AntidoteResultEvent>();
    app.add_event::<ConsumePoisonPillIntent>();
    app.add_event::<PoisonDoseEvent>();
    app.add_event::<PoisonOverdoseEvent>();
    app.add_event::<DigestionOverloadEvent>();
    app.add_event::<PoisonPowderConsumedEvent>();

    app.add_systems(
        Update,
        qi_regen_and_zone_drain_tick
            .after(crate::combat::status::attribute_aggregate_tick)
            .after(crate::combat::baomai_v4::scar_circuit::scar_circuit_derive_system)
            .after(crate::combat::body_conditioning::body_conditioning_aggregate),
    );
    // Bevy IntoSystemConfigs 最多 20 个元素；拆两组。
    app.add_systems(
        Update,
        (
            attach_cultivation_to_joined_clients
                .after(crate::player::attach_player_state_to_joined_clients)
                // plan-remains-suite：转世门在本系统内会强制覆写 PlayerInventory
                // （新角色发默认 loadout），必须排在 inventory 的 join-attach 之后，
                // 否则两个系统对同一 tick 内 Commands 的插入顺序不确定，可能被
                // `attach_inventory_to_joined_clients` 的默认背包在同一 sync point
                // 后再次覆盖回去。
                .after(crate::inventory::attach_inventory_to_joined_clients),
            // 核心 tick 后续：打通 → 事务；回气/扣 zone 已在上方单独注册。
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
            on_player_revived.after(crate::combat::lifecycle::emit_player_revived_completions),
            on_player_terminated.after(crate::combat::lifecycle::handle_revival_action_intents),
            // plan §11-5 业力
            karma_weight_decay_tick.after(qi_regen_and_zone_drain_tick),
            void_realm_karma_pressure_tick.after(karma_weight_decay_tick),
        ),
    );
    app.add_systems(
        Update,
        (
            start_breakthrough_cinematic_on_outcome.after(breakthrough_system),
            breakthrough_cinematic_phase_tick.after(start_breakthrough_cinematic_on_outcome),
            interrupt_breakthrough_cinematic_on_hit
                .after(crate::combat::resolve::resolve_attack_intents),
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
    app.add_systems(Update, track_woliu_proficiency_from_casts);
    // plan-color-v1 P4: 被动神视感知 — Spirit/Void 境界玩家对范围内目标被动扫描 QiColor
    app.add_systems(
        Update,
        passive_qi_color_scan_system.after(qi_color_evolution_tick),
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
            start_due_juebi_triggers_system
                .after(schedule_juebi_triggers_system)
                .after(attach_cultivation_to_joined_clients)
                .after(crate::combat::lifecycle::handle_revival_action_intents),
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
            record_tribulation_scorch_system
                .after(juebi_settlement_system)
                .after(tribulation_failure_system)
                .after(tribulation_escape_boundary_system)
                .after(tribulation_intercept_death_system),
        ),
    );
    // plan-halfstep-buff-v1 P0/P3：渡虚劫遥测累计 + quota 满时长追踪 + 重渡派发
    app.add_systems(
        Update,
        (
            track_tribulation_metrics_system.after(juebi_settlement_system),
            track_quota_full_duration_system,
            dispatch_rechallenge_on_quota_opened_system
                .after(track_quota_full_duration_system)
                .after(attach_cultivation_to_joined_clients)
                .after(crate::combat::lifecycle::handle_revival_action_intents),
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
            consume_poison_pill_system.after(lifespan_aging_tick),
            apply_poison_overdose_costs
                .after(consume_poison_pill_system)
                .before(apply_meridian_crack_events),
            poison_toxicity_decay_tick.after(consume_poison_pill_system),
            digestion_load_decay_tick.after(consume_poison_pill_system),
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

/// Explicit admission for a client whose cultivation hydration failed before runtime publication.
///
/// The marker keeps transient SQLite/loadout failures retryable without treating every later
/// `Without<Cultivation>` transition (notably deliberate termination cleanup) as a fresh join.
#[derive(Debug, Component)]
pub(crate) struct CultivationAttachRetry;

type CultivationAttachFilter = (
    With<Client>,
    Without<Cultivation>,
    Or<(
        Added<Client>,
        Added<CurrentDimension>,
        With<CultivationAttachRetry>,
    )>,
);
type CultivationAttachQueryItem<'a> = (
    Entity,
    &'a Username,
    Option<&'a mut PlayerState>,
    Option<&'a LifespanComponent>,
    Option<&'a mut EntityLayerId>,
    Option<&'a mut VisibleChunkLayer>,
    Option<&'a mut VisibleEntityLayers>,
    Option<&'a mut Position>,
    Option<&'a mut CurrentDimension>,
    Option<&'a mut Flags>,
);

fn parse_persisted_tribulation_dimension(value: &str) -> Option<DimensionKind> {
    match value {
        "minecraft:overworld" | "overworld" => Some(DimensionKind::Overworld),
        "bong:tsy" | "tsy" => Some(DimensionKind::Tsy),
        _ => None,
    }
}

#[derive(SystemParam)]
pub(crate) struct CultivationAttachAuthorities<'w> {
    zones: Option<ResMut<'w, ZoneRegistry>>,
    qi_account: Option<ResMut<'w, WorldQiAccount>>,
    qi_transfers: Option<ResMut<'w, Events<QiTransfer>>>,
    // plan-race-system-v1 P0 —— 持久化 `Cultivation.race` 拒载执行点：`Option<Res<...>>`
    // 同 `body_plan::register()` 恒装载的既有约定（大量既有测试未插入该资源，缺失时
    // 无法校验，退回本函数原有"信任解码结果"行为，不是新的宽松分支）。
    race_registry: Option<Res<'w, crate::body_plan::RaceRegistry>>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn attach_cultivation_to_joined_clients(
    mut commands: Commands,
    settings: Res<PersistenceSettings>,
    player_persistence: Option<Res<PlayerStatePersistence>>,
    default_loadout: Option<Res<crate::inventory::DefaultLoadout>>,
    item_registry: Option<Res<crate::inventory::ItemRegistry>>,
    mut inventory_allocator: Option<ResMut<crate::inventory::InventoryInstanceIdAllocator>>,
    mut pending_narrations: Option<ResMut<crate::player::gameplay::PendingGameplayNarrations>>,
    clock: Option<Res<CombatClock>>,
    mut tutorial_telemetry: Option<ResMut<TutorialTelemetry>>,
    mut coffin_registry: Option<ResMut<CoffinRegistry>>,
    dimension_layers: Option<Res<DimensionLayers>>,
    mut coffin_state_events: Option<ResMut<Events<CoffinStateChanged>>>,
    mut pending_juebi_triggers: Option<ResMut<PendingJueBiTriggers>>,
    mut halfstep_queue: Option<ResMut<HalfStepRechallengeQueue>>,
    mut authorities: CultivationAttachAuthorities,
    mut joined_clients: Query<CultivationAttachQueryItem<'_>, CultivationAttachFilter>,
) {
    for (
        entity,
        username,
        mut player_state,
        restored_lifespan,
        layer_id,
        visible_chunk_layer,
        visible_entity_layers,
        mut position,
        current_dimension,
        mut flags,
    ) in &mut joined_clients
    {
        let persisted_bundle = match load_player_cultivation_bundle(&settings, username.0.as_str())
        {
            Ok(value) => value,
            Err(error) => {
                tracing::warn!(
                    "[bong][cultivation] failed to load persisted cultivation bundle for `{}`: {error}; deferring hydration instead of publishing defaults",
                    username.0,
                );
                commands.entity(entity).insert(CultivationAttachRetry);
                continue;
            }
        };

        let persisted_lifecycle = match player_persistence.as_deref() {
            Some(persistence) => {
                let current_combat_clock_tick = clock.as_deref().map_or(0, |clock| clock.tick);
                match load_player_lifecycle_slice(
                    persistence,
                    username.0.as_str(),
                    current_combat_clock_tick,
                ) {
                    Ok(value) => value,
                    Err(error) => {
                        tracing::warn!(
                            "[bong][cultivation] failed to load persisted lifecycle for `{}`: {error}; deferring cultivation hydration instead of risking terminated-character fallback",
                            username.0,
                        );
                        commands.entity(entity).insert(CultivationAttachRetry);
                        continue;
                    }
                }
            }
            None => None,
        };
        let expected_character_id = match player_persistence.as_deref() {
            Some(persistence) => {
                match load_current_character_id(persistence, username.0.as_str()) {
                    Ok(Some(current_char_id)) => {
                        player_character_id(username.0.as_str(), current_char_id.as_str())
                    }
                    Ok(None) => canonical_player_id(username.0.as_str()),
                    Err(error) => {
                        tracing::warn!(
                            "[bong][cultivation] failed to load current character identity for `{}`: {error}; deferring cultivation hydration instead of comparing lifecycle against an unknown identity",
                            username.0,
                        );
                        commands.entity(entity).insert(CultivationAttachRetry);
                        continue;
                    }
                }
            }
            None => canonical_player_id(username.0.as_str()),
        };
        let persisted_life_record = match persisted_bundle.as_ref() {
            Some(bundle) => match bundle.get("life_record") {
                Some(value) => match serde_json::from_value::<LifeRecord>(value.clone()) {
                    Ok(record) => Some(record),
                    Err(error) => {
                        tracing::warn!(
                            "[bong][cultivation] refusing cultivation hydration for `{}`: persisted life_record cannot be decoded: {error}; leaving the durable identity untouched",
                            username.0,
                        );
                        commands.entity(entity).insert(CultivationAttachRetry);
                        continue;
                    }
                },
                None => {
                    tracing::warn!(
                        "[bong][cultivation] refusing cultivation hydration for `{}`: persisted bundle is missing life_record; leaving the durable identity untouched",
                        username.0,
                    );
                    commands.entity(entity).insert(CultivationAttachRetry);
                    continue;
                }
            },
            None => None,
        };
        if persisted_life_record
            .as_ref()
            .is_some_and(|life_record| life_record.character_id != expected_character_id)
        {
            tracing::warn!(
                "[bong][cultivation] refusing cultivation hydration for `{}`: persisted life_record belongs to a different character than `{}`; leaving both identities untouched",
                username.0,
                expected_character_id,
            );
            commands.entity(entity).insert(CultivationAttachRetry);
            continue;
        }
        let life_record_declares_terminated =
            persisted_life_record.as_ref().is_some_and(|record| {
                matches!(
                    record.biography.last(),
                    Some(BiographyEntry::Terminated { .. })
                )
            });
        let persisted_current_lifecycle = persisted_lifecycle
            .as_ref()
            .filter(|lifecycle| lifecycle.character_id == expected_character_id);
        let lifecycle_declares_current_terminated =
            persisted_current_lifecycle.is_some_and(|lifecycle| {
                lifecycle.state == crate::combat::components::LifecycleState::Terminated
            });
        // LifeRecord keeps pre-lifecycle-slice saves compatible. When a current-identity Lifecycle
        // exists, both durable records must agree in both directions before old qi can be settled.
        if persisted_life_record.is_some()
            && persisted_current_lifecycle.is_some()
            && life_record_declares_terminated != lifecycle_declares_current_terminated
        {
            tracing::warn!(
                "[bong][cultivation] refusing cultivation hydration for `{}`: life_record and durable lifecycle disagree about termination; leaving the old identity untouched",
                username.0,
            );
            commands.entity(entity).insert(CultivationAttachRetry);
            continue;
        }
        let persisted_terminated =
            life_record_declares_terminated || lifecycle_declares_current_terminated;
        if lifecycle_declares_current_terminated && persisted_bundle.is_none() {
            tracing::warn!(
                "[bong][cultivation] refusing terminated-character reincarnation for `{}`: durable cultivation bundle is missing; leaving the old identity untouched",
                username.0,
            );
            commands.entity(entity).insert(CultivationAttachRetry);
            continue;
        }

        // plan-race-system-v1 P0 review r4（bughunt major-2 收口）—— 未知 race id 必须
        // 拒载**整份** bundle，不只是 `cultivation` 这一个 slice：此前的实现只在下面
        // `cultivation` 字段的解码分支里做校验，一旦拒绝也只把 `cultivation` 变量留在
        // `Cultivation::default()`，但 `meridians`/`qi_color`/`karma`/`practice_log`/
        // `contamination`/`life_record`/`insight_quota`/`unlocked_perceptions`/
        // `insight_modifiers`/`meridian_severed`/`poison_toxicity`/`digestion_load`/
        // `nourishment` 这 14 个 sibling slice 全部照常从同一份
        // 不可信 bundle 里正常水合——一份来自
        // 不兼容部署（race 在当前 RaceRegistry 里根本不存在）的存档，其经脉拓扑/
        // 真元/毒性等字段同样不可信，"只挡 race 字段本身"等于开了个后门：醒灵后
        // 立刻嵌合体全通经脉、满毒素抗性表白跳突破。这里在任何字段解码之前先窥探
        // 原始 JSON 里的 `cultivation.race`，一旦命中未知 race 就把整份 bundle 提前
        // 归零成 `None`，让下面所有 slice 的 hydration 分支统一走"无持久化数据"的
        // 默认路径（缺失 race 字段的旧存档 `cultivation.race` 键本身不存在，
        // `as_str()` 拿不到值，不受影响，仍走既有 legacy 迁移路径）。终结角色例外：
        // fallback 会把旧 qi 伪装成零并允许后续覆盖，必须保留 durable bundle 等待修复。
        let unknown_persisted_race = persisted_bundle.as_ref().and_then(|bundle| {
            authorities.race_registry.as_deref().and_then(|registry| {
                bundle
                    .get("cultivation")
                    .and_then(|cultivation_value| cultivation_value.get("race"))
                    .and_then(|race_value| race_value.as_str())
                    .map(|race_str| registry.get(&RaceId::new(race_str)).is_none())
            })
        });
        if unknown_persisted_race == Some(true) && persisted_terminated {
            tracing::warn!(
                "[bong][cultivation] refusing terminated-character reincarnation for `{}`: persisted race id is unknown; leaving the old identity untouched",
                username.0,
            );
            commands.entity(entity).insert(CultivationAttachRetry);
            continue;
        }
        let persisted_bundle = if unknown_persisted_race == Some(true) {
            tracing::warn!(
                "[bong][cultivation] rejecting entire persisted cultivation bundle for `{}`: \
                 unknown race id in persisted `cultivation.race` is not found in \
                 RaceRegistry — falling back to default state for every slice \
                 (cultivation/meridians/qi_color/karma/practice_log/contamination/\
                 life_record/insight_quota/unlocked_perceptions/insight_modifiers/\
                 meridian_severed/poison_toxicity/digestion_load/nourishment/\
                 ), not just the \
                 `cultivation` field",
                username.0,
            );
            None
        } else {
            persisted_bundle
        };

        // plan-race-system-v1 P1a —— bundle 内嵌版本号（`persist_player_cultivation_bundle`
        // 的 `"v"` 字段，与全局 `CURRENT_SCHEMA_VERSION`/`CURRENT_USER_VERSION` 是两套
        // 独立版本号，只管 `cultivation_json` blob 自身的形态演进）。缺失该字段的旧存档
        // 视为 v1（`MeridianSystem`/`MeridianSeveredPermanent` 的 `MeridianId` PascalCase
        // 枚举名 channel id 形态）；`legacy_meridian_bundle::CURRENT_BUNDLE_VERSION`（本次
        // 提升到 2）起 channel id 换轨为 humanoid.json 声明的 snake_case
        // `MeridianChannelId`——两种形态字段名/嵌套结构完全相同，差异只在 id 字符串本身。
        let bundle_version = persisted_bundle
            .as_ref()
            .and_then(|bundle| bundle.get("v"))
            .and_then(|v| v.as_i64())
            .unwrap_or(1);

        // A terminated character is the only join path that can replace the durable identity and
        // settle the old cultivation bundle. Its persisted slices therefore must be decoded as a
        // unit before any default value can make the old qi look like exact zero. Ordinary live
        // hydration keeps the legacy best-effort behavior below for backwards compatibility.
        if persisted_terminated {
            if let Some(bundle) = persisted_bundle.as_ref() {
                if let Err(error) = validate_terminated_persisted_bundle(bundle, bundle_version) {
                    tracing::warn!(
                        "[bong][cultivation] refusing terminated-character reincarnation for `{}`: persisted cultivation bundle is incomplete: {error}; leaving the old identity untouched",
                        username.0,
                    );
                    commands.entity(entity).insert(CultivationAttachRetry);
                    continue;
                }
            }
        }

        let mut cultivation = Cultivation::default();
        let mut meridians = MeridianSystem::default();
        let mut qi_color = QiColor::default();
        let mut karma = Karma::default();
        let mut practice_log = PracticeLog::default();
        let mut contamination = Contamination::default();
        let mut canonical_id = expected_character_id.clone();
        let mut life_record = LifeRecord::new(canonical_id.clone());
        let mut insight_quota = InsightQuota::default();
        let mut unlocked_perceptions = UnlockedPerceptions::default();
        let mut insight_modifiers = InsightModifiers::new();
        let mut nourishment = Nourishment::spawn_default();

        if let Some(persisted_bundle) = persisted_bundle.as_ref() {
            // Best-effort hydration; schema is versioned and may evolve.
            if let Some(value) = persisted_bundle.get("cultivation") {
                // plan-race-system-v1 P0 review r4（bughunt major-2 收口）—— 未知
                // race id 的整份 bundle 拒载已经在上面按原始 JSON 提前判定并把
                // `persisted_bundle` 归零成 `None`（见该处注释），本分支只在 race
                // 已知（或缺失、经 `#[serde(default = "default_race_id")]` 落
                // "human"）时才会执行，不再需要重复校验 `decoded.race`。
                match serde_json::from_value::<Cultivation>(value.clone()) {
                    Ok(decoded) => cultivation = decoded,
                    Err(error) => {
                        warn_cultivation_decode(username.0.as_str(), "cultivation", error)
                    }
                }
            }
            if let Some(value) = persisted_bundle.get("meridians") {
                match legacy_meridian_bundle::decode_meridian_system(value.clone(), bundle_version)
                {
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
            if let Some(decoded) = persisted_life_record.as_ref() {
                life_record = decoded.clone();
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
            if let Some(value) = persisted_bundle.get("nourishment") {
                match serde_json::from_value::<Nourishment>(value.clone()) {
                    Ok(decoded) => nourishment = decoded,
                    Err(error) => {
                        warn_cultivation_decode(username.0.as_str(), "nourishment", error)
                    }
                }
            }
        } else if player_state.is_some() {
            tracing::debug!(
                "[bong][cultivation] no persisted cultivation bundle for `{}`; using defaults",
                username.0,
            );
        }

        // plan-remains-suite P0：join 转世门。只在本地 staged 新身份；真正持久化必须和
        // fresh player slices、life record、cultivation+nourishment 在一个 IMMEDIATE transaction
        // 内完成，成功后才允许把新角色组件插入 ECS。
        let previous_character_id = canonical_id.clone();
        let mut staged_old_qi_release = None;
        let reincarnation = if matches!(
            life_record.biography.last(),
            Some(BiographyEntry::Terminated { .. })
        ) {
            if player_persistence.is_none()
                || default_loadout.is_none()
                || item_registry.is_none()
                || inventory_allocator.is_none()
                || dimension_layers.is_none()
                || pending_juebi_triggers.is_none()
                || halfstep_queue.is_none()
                || (cultivation.qi_current > 0.0
                    && (authorities.zones.is_none() || authorities.qi_account.is_none()))
            {
                tracing::warn!(
                    "[bong][cultivation] `{}` joined with a terminated character but atomic reincarnation or world-layer resources are incomplete; leaving the terminated record untouched",
                    username.0,
                );
                None
            } else {
                let old_qi_release = match crate::combat::lifecycle::stage_lifecycle_qi_release(
                    entity,
                    cultivation.qi_current.max(0.0),
                    previous_character_id.as_str(),
                    current_dimension
                        .as_deref()
                        .zip(position.as_deref())
                        .map(|(dimension, position)| (dimension.0, position.get().to_array())),
                    authorities.zones.as_deref(),
                    authorities.qi_account.as_deref(),
                ) {
                    Ok(staged) => staged,
                    Err(error) => {
                        tracing::warn!(
                            "[bong][cultivation] `{}` joined with a terminated character but old-life qi release cannot be staged atomically: {error}; leaving the terminated record untouched",
                            username.0,
                        );
                        commands.entity(entity).insert(CultivationAttachRetry);
                        continue;
                    }
                };
                staged_old_qi_release = old_qi_release;
                let bundle = self::character_select::prepare_new_character(username.0.as_str());
                canonical_id = bundle.next_character_id.clone();
                cultivation = Cultivation::default();
                meridians = MeridianSystem::default();
                qi_color = QiColor::default();
                karma = Karma::default();
                practice_log = PracticeLog::default();
                contamination = Contamination::default();
                life_record = LifeRecord::new(canonical_id.clone());
                insight_quota = InsightQuota::default();
                unlocked_perceptions = UnlockedPerceptions::default();
                insight_modifiers = InsightModifiers::new();
                Some(bundle)
            }
        } else {
            None
        };

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
                match legacy_meridian_bundle::decode_meridian_severed(value.clone(), bundle_version)
                {
                    Ok(decoded) => severed_permanent = decoded,
                    Err(error) => {
                        warn_cultivation_decode(username.0.as_str(), "meridian_severed", error)
                    }
                }
            }
        }

        let mut poison_toxicity = PoisonToxicity::default();
        let mut digestion_load = DigestionLoad::for_realm(cultivation.realm);
        if let Some(persisted_bundle) = persisted_bundle.as_ref() {
            if let Some(value) = persisted_bundle.get("poison_toxicity") {
                match serde_json::from_value::<PoisonToxicity>(value.clone()) {
                    Ok(decoded) => poison_toxicity = decoded,
                    Err(error) => {
                        warn_cultivation_decode(username.0.as_str(), "poison_toxicity", error)
                    }
                }
            }
            if let Some(value) = persisted_bundle.get("digestion_load") {
                match serde_json::from_value::<DigestionLoad>(value.clone()) {
                    Ok(decoded) => {
                        digestion_load = decoded;
                        digestion_load.capacity = digestion_load
                            .capacity
                            .max(DigestionLoad::for_realm(cultivation.realm).capacity);
                    }
                    Err(error) => {
                        warn_cultivation_decode(username.0.as_str(), "digestion_load", error)
                    }
                }
            }
        }

        let is_reincarnating = reincarnation.is_some();
        if is_reincarnating {
            // 旧身体的永久损伤、中毒进度、消化负担与饥渴都不能附体到新生命。
            severed_permanent = MeridianSeveredPermanent::default();
            poison_toxicity = PoisonToxicity::default();
            digestion_load = DigestionLoad::for_realm(cultivation.realm);
            nourishment.reset_to_spawn();
        }

        let fresh_reincarnation_runtime = if let Some(bundle) = reincarnation.as_ref() {
            let (
                Some(player_persistence),
                Some(default_loadout),
                Some(item_registry),
                Some(allocator),
            ) = (
                player_persistence.as_deref(),
                default_loadout.as_deref(),
                item_registry.as_deref(),
                inventory_allocator.as_deref_mut(),
            )
            else {
                unreachable!("atomic reincarnation resources were checked before staging");
            };
            let mut staged_allocator = allocator.clone();
            let fresh_inventory = match crate::inventory::instantiate_inventory_from_loadout(
                &default_loadout.0,
                &mut staged_allocator,
                item_registry,
            ) {
                Ok(inventory) => inventory,
                Err(error) => {
                    tracing::warn!(
                        "[bong][cultivation] refusing join reincarnation for `{}`: default loadout failed: {error}",
                        username.0,
                    );
                    commands.entity(entity).insert(CultivationAttachRetry);
                    continue;
                }
            };
            let fresh_player_state = PlayerState::default();
            let fresh_skill_set = crate::skill::components::SkillSet::default();
            let fresh_lifespan = LifespanComponent::new(bundle.spec.lifespan_cap);
            let combat_clock_tick = clock.as_deref().map(|clock| clock.tick).unwrap_or_default();
            let fresh_tutorial_state = TutorialState::new(combat_clock_tick);
            let fresh_lifecycle = crate::combat::components::Lifecycle {
                character_id: bundle.next_character_id.clone(),
                last_revive_tick: Some(combat_clock_tick),
                ..Default::default()
            };
            if let Err(error) = persist_new_character_transition(
                &settings,
                player_persistence,
                username.0.as_str(),
                NewCharacterPersistenceBundle {
                    previous_character_id: previous_character_id.as_str(),
                    current_char_id: bundle.current_char_id.as_str(),
                    state: &fresh_player_state,
                    position: bundle.spec.spawn_pos,
                    inventory: Some(&fresh_inventory),
                    lifespan: &fresh_lifespan,
                    skill_set: &fresh_skill_set,
                    lifecycle: &fresh_lifecycle,
                    combat_clock_tick,
                    cultivation: PlayerCultivationBundle {
                        cultivation: &cultivation,
                        meridians: &meridians,
                        qi_color: &qi_color,
                        karma: &karma,
                        contamination: &contamination,
                        life_record: &life_record,
                        practice_log: &practice_log,
                        insight_quota: &insight_quota,
                        unlocked_perceptions: &unlocked_perceptions,
                        insight_modifiers: &insight_modifiers,
                        tutorial_state: Some(&fresh_tutorial_state),
                        meridian_severed: &severed_permanent,
                        poison_toxicity: Some(&poison_toxicity),
                        digestion_load: Some(&digestion_load),
                        nourishment: &nourishment,
                    },
                    zone_runtime: staged_old_qi_release
                        .as_ref()
                        .and_then(|release| release.zone_runtime.as_ref()),
                    qi_ledger: staged_old_qi_release
                        .as_ref()
                        .map(|release| &release.qi_account),
                },
            ) {
                tracing::warn!(
                    "[bong][cultivation] failed atomic join reincarnation for `{}`: {error}; leaving the terminated character untouched",
                    username.0,
                );
                commands.entity(entity).insert(CultivationAttachRetry);
                continue;
            }
            if let Some(staged_release) = staged_old_qi_release.take() {
                *authorities
                    .zones
                    .as_deref_mut()
                    .expect("staged join reincarnation qi release requires ZoneRegistry") =
                    staged_release.zones;
                *authorities
                    .qi_account
                    .as_deref_mut()
                    .expect("staged join reincarnation qi release requires WorldQiAccount") =
                    staged_release.qi_account;
                if let Some(events) = authorities.qi_transfers.as_deref_mut() {
                    for transfer in staged_release.transfers {
                        events.send(transfer);
                    }
                } else if !staged_release.transfers.is_empty() {
                    tracing::warn!(
                        "[bong][cultivation] committed join reincarnation qi release for `{}` without QiTransfer event resource",
                        username.0,
                    );
                }
            }
            *allocator = staged_allocator;
            if let Some(telemetry) = tutorial_telemetry.as_deref_mut() {
                telemetry.started = telemetry.started.saturating_add(1);
            }
            Some((
                fresh_player_state,
                fresh_inventory,
                fresh_skill_set,
                fresh_lifespan,
                fresh_tutorial_state,
                fresh_lifecycle,
            ))
        } else {
            None
        };

        // plan-race-system-v1 P5/PR-6c —— `IntrinsicRace` 是本体种族的真源，join 首帧
        // 必须与刚水合出来的 `Cultivation.race` 同步落地，否则任何只查 `IntrinsicRace`
        // 组件（不回落 `Cultivation`）的消费点在玩家从未经历过 `RaceChange`（6a 只在
        // 换种族事务里才 insert 本组件）时会读到组件缺失——这是本 PR 关闭的孤岛
        // （recon 标定的最大孤岛：`IntrinsicRace` 定义了零处 insert）。
        let intrinsic_race = crate::body_plan::IntrinsicRace(cultivation.race.clone());
        let mut entity_commands = commands.entity(entity);
        entity_commands.remove::<CultivationAttachRetry>();
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
        entity_commands.insert((
            poison_toxicity,
            digestion_load,
            nourishment,
            crate::nourishment::tick::NourishmentActivityWindow::default(),
            intrinsic_race,
        ));
        // 普通登录仅在持久化寿元缺失时补默认值；转世的新寿元由下方 fresh runtime
        // bundle 在事务成功后覆盖，避免同一 flush 内重复插入 LifespanComponent。
        if restored_lifespan.is_none() && !is_reincarnating {
            entity_commands.insert(default_lifespan.clone());
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

        if let (
            Some(bundle),
            Some((
                fresh_player_state,
                fresh_inventory,
                fresh_skill_set,
                fresh_lifespan,
                fresh_tutorial_state,
                fresh_lifecycle,
            )),
        ) = (reincarnation, fresh_reincarnation_runtime)
        {
            if let Some(player_state) = player_state.as_deref_mut() {
                *player_state = fresh_player_state;
            } else {
                entity_commands.insert(fresh_player_state);
            }
            entity_commands.insert((
                fresh_inventory,
                fresh_skill_set,
                fresh_lifespan,
                fresh_tutorial_state,
                fresh_lifecycle,
                KnownTechniques::default(),
            ));
            entity_commands.remove::<KnownTechniquesLoadFailed>();

            let target_position = Position::new(bundle.spec.spawn_pos);
            if let Some(position) = position.as_deref_mut() {
                position.set(bundle.spec.spawn_pos);
            } else {
                entity_commands.insert(target_position);
            }
            if let Some(flags) = flags.as_deref_mut() {
                flags.set_invisible(false);
            }

            let layers = dimension_layers
                .as_deref()
                .expect("atomic reincarnation checked DimensionLayers before commit");
            drop(entity_commands);
            publish_overworld_runtime(
                entity,
                &mut commands,
                layer_id,
                visible_chunk_layer,
                visible_entity_layers,
                current_dimension,
                layers,
                OverworldVisibilityPolicy::PreserveUnrelatedLayers,
            );

            pending_juebi_triggers
                .as_deref_mut()
                .expect("atomic reincarnation checked PendingJueBiTriggers before commit")
                .cancel_for_character(entity, previous_character_id.as_str());
            halfstep_queue
                .as_deref_mut()
                .expect("atomic reincarnation checked HalfStepRechallengeQueue before commit")
                .remove_character(entity, previous_character_id.as_str());
            commands
                .entity(entity)
                .remove::<crate::craft::CraftSession>()
                .remove::<crate::network::craft_emit::CraftSessionPersistenceDirty>()
                .insert(crate::network::craft_emit::CraftSessionStateDirty);

            let was_in_coffin =
                clear_player_coffin_runtime(entity, &mut commands, coffin_registry.as_deref_mut());
            if was_in_coffin {
                if let Some(events) = coffin_state_events.as_deref_mut() {
                    events.send(CoffinStateChanged {
                        player: entity,
                        grade: None,
                    });
                }
            }

            if let Some(pending_narrations) = pending_narrations.as_deref_mut() {
                pending_narrations.push_player(
                    username.0.as_str(),
                    "前尘已尽，一缕残魂自醒灵境重新苏醒——今生与前身再无瓜葛。",
                    crate::schema::common::NarrationStyle::Narration,
                );
            }

            tracing::info!(
                "[bong][cultivation] atomically reincarnated `{}` and attached fresh cultivation bundle to {entity:?}",
                username.0,
            );
        } else {
            tracing::info!("[bong][cultivation] attached full cultivation bundle to {entity:?}");
        }
    }
}

fn warn_cultivation_decode(username: &str, slice: &str, error: serde_json::Error) {
    tracing::warn!(
        "[bong][cultivation] failed to decode persisted {slice} slice for `{username}`: {error}"
    );
}

/// Validate every non-optional slice before replacing a terminated character's durable identity.
///
/// Ordinary hydration below deliberately preserves the historical best-effort behavior for live
/// characters. Terminated hydration is different: its first successful transaction replaces the
/// old character and settles its qi, so a decode fallback here would turn an unreadable positive
/// `qi_current` into zero and make the old balance unaccountable.
fn validate_terminated_persisted_bundle(
    bundle: &serde_json::Value,
    bundle_version: i64,
) -> Result<(), String> {
    macro_rules! require_decode {
        ($key:literal, $ty:ty) => {{
            let value = bundle
                .get($key)
                .ok_or_else(|| format!("missing required `{}` slice", $key))?;
            serde_json::from_value::<$ty>(value.clone())
                .map_err(|error| format!("failed to decode `{}`: {error}", $key))?;
        }};
    }

    if let Some(version) = bundle.get("v") {
        if version.as_i64().is_none() {
            return Err("bundle `v` must be an integer when present".to_string());
        }
    }

    require_decode!("cultivation", Cultivation);
    legacy_meridian_bundle::decode_meridian_system(
        bundle
            .get("meridians")
            .ok_or_else(|| "missing required `meridians` slice".to_string())?
            .clone(),
        bundle_version,
    )
    .map_err(|error| format!("failed to decode `meridians`: {error}"))?;
    require_decode!("qi_color", QiColor);
    require_decode!("karma", Karma);
    require_decode!("contamination", Contamination);
    require_decode!("life_record", LifeRecord);
    require_decode!("practice_log", PracticeLog);
    require_decode!("insight_quota", InsightQuota);
    require_decode!("unlocked_perceptions", UnlockedPerceptions);
    require_decode!("insight_modifiers", InsightModifiers);
    legacy_meridian_bundle::decode_meridian_severed(
        bundle
            .get("meridian_severed")
            .ok_or_else(|| "missing required `meridian_severed` slice".to_string())?
            .clone(),
        bundle_version,
    )
    .map_err(|error| format!("failed to decode `meridian_severed`: {error}"))?;

    macro_rules! decode_optional_if_present {
        ($key:literal, $ty:ty) => {
            if let Some(value) = bundle.get($key).filter(|value| !value.is_null()) {
                serde_json::from_value::<$ty>(value.clone())
                    .map_err(|error| format!("failed to decode `{}`: {error}", $key))?;
            }
        };
    }
    decode_optional_if_present!("tutorial_state", TutorialState);
    decode_optional_if_present!("poison_toxicity", PoisonToxicity);
    decode_optional_if_present!("digestion_load", DigestionLoad);
    decode_optional_if_present!("nourishment", Nourishment);

    Ok(())
}

/// plan-race-system-v1 P1a —— `cultivation_json` bundle 里 `meridians` / `meridian_severed`
/// 两个子字段的旧存档显式迁移。
///
/// **为什么需要迁移而不能直接 `serde_json::from_value`**：`MeridianSystem`/
/// `MeridianSeveredPermanent` 的容器形状（`regular`/`extraordinary` 两个 Vec 字段名、
/// `severed_meridians`/`severed_at`/`dead_meridians` 三个字段名）迁移前后完全不变——
/// 变的只是"经脉 channel id 用什么字符串表示"：v1（bump 前）用 `MeridianId` 闭合枚举
/// 的 serde 默认表示（unit variant 名，PascalCase，如 `"Lung"`）；v2 起换成
/// `body_plan::MeridianProfile`（`humanoid.json`）声明的规范 snake_case
/// `MeridianChannelId`（如 `"lung"`）。`MeridianChannelId` 是 `#[serde(transparent)]`
/// 包裹的裸字符串，对**任意**字符串都能"成功"反序列化——这意味着如果不做迁移，
/// 旧存档的 `"id":"Lung"` 会被静默解析成 `MeridianChannelId("Lung")`（大小写、内容都
/// 对不上 humanoid.json 的 `"lung"`），后续 `MeridianSystem::get`/`get_mut` 找不到这条
/// 经脉直接 panic——不是"解析失败"而是"解析成功但语义损坏"，比崩溃更危险，必须
/// 显式迁移拦下。
pub(crate) mod legacy_meridian_bundle {
    use std::collections::{HashMap, HashSet};

    use serde::Deserialize;

    use super::components::{Meridian, MeridianCrack, MeridianId, MeridianSystem};
    use super::meridian::severed::{MeridianSeveredPermanent, SeveredRecord};

    /// bundle 内嵌版本号阈值——`>= CURRENT_BUNDLE_VERSION` 走新形态直接反序列化，
    /// 更旧的（含缺失 `"v"` 字段、隐式视为 1）先按 legacy 形态解码再逐条转换 channel id。
    /// `persistence::persist_player_cultivation_bundle` 写入 bundle 时的 `"v"` 字段必须
    /// 引用同一常量（`crate::cultivation::legacy_meridian_bundle::CURRENT_BUNDLE_VERSION`），
    /// 不允许两处各自维护一份数字。
    pub(crate) const CURRENT_BUNDLE_VERSION: i64 = 2;

    #[derive(Debug, Deserialize)]
    struct LegacyMeridian {
        id: MeridianId,
        opened: bool,
        open_progress: f64,
        flow_rate: f64,
        flow_capacity: f64,
        rate_tier: u8,
        capacity_tier: u8,
        throughput_current: f64,
        integrity: f64,
        #[serde(default)]
        cracks: Vec<MeridianCrack>,
        opened_at: u64,
    }

    impl From<LegacyMeridian> for Meridian {
        fn from(legacy: LegacyMeridian) -> Self {
            Meridian {
                id: legacy.id.channel_id(),
                opened: legacy.opened,
                open_progress: legacy.open_progress,
                flow_rate: legacy.flow_rate,
                flow_capacity: legacy.flow_capacity,
                rate_tier: legacy.rate_tier,
                capacity_tier: legacy.capacity_tier,
                throughput_current: legacy.throughput_current,
                integrity: legacy.integrity,
                cracks: legacy.cracks,
                opened_at: legacy.opened_at,
            }
        }
    }

    #[derive(Debug, Deserialize)]
    struct LegacyMeridianSystem {
        regular: Vec<LegacyMeridian>,
        extraordinary: Vec<LegacyMeridian>,
    }

    /// 解码 `meridians` bundle 子字段。`bundle_version >= CURRENT_BUNDLE_VERSION` 时
    /// 直接按当前 `MeridianSystem` 形态解析（新存档，channel id 已是 snake_case）；
    /// 否则先按 v1 legacy 形态（`MeridianId` PascalCase 枚举名）解析再迁移。
    pub fn decode_meridian_system(
        value: serde_json::Value,
        bundle_version: i64,
    ) -> Result<MeridianSystem, serde_json::Error> {
        if bundle_version >= CURRENT_BUNDLE_VERSION {
            return serde_json::from_value(value);
        }
        let legacy: LegacyMeridianSystem = serde_json::from_value(value)?;
        Ok(MeridianSystem {
            regular: legacy.regular.into_iter().map(Meridian::from).collect(),
            extraordinary: legacy
                .extraordinary
                .into_iter()
                .map(Meridian::from)
                .collect(),
        })
    }

    #[derive(Debug, Default, Deserialize)]
    struct LegacyMeridianSeveredPermanent {
        #[serde(default)]
        severed_meridians: HashSet<MeridianId>,
        #[serde(default)]
        severed_at: HashMap<MeridianId, SeveredRecord>,
        #[serde(default)]
        dead_meridians: HashSet<MeridianId>,
    }

    /// 解码 `meridian_severed` bundle 子字段，语义同 [`decode_meridian_system`]——
    /// **未映射通道不删除不洗白**（§8.1 #9 决议）：v1 存档里出现的 SEVERED/死脉记录
    /// 在本函数内 100% 覆盖 humanoid 20 条经脉（`MeridianId::channel_id` 是全函数，
    /// 无法产出"未映射"的 legacy 条目），迁移后逐条转换、一个不丢；`meridian_mapping`
    /// 式"部分不可逆映射"只在 P5 `RaceChange`（种族切换）场景出现，不属于本函数处理的
    /// "同一构型内 id 表示法换代"范畴。
    pub fn decode_meridian_severed(
        value: serde_json::Value,
        bundle_version: i64,
    ) -> Result<MeridianSeveredPermanent, serde_json::Error> {
        if bundle_version >= CURRENT_BUNDLE_VERSION {
            return serde_json::from_value(value);
        }
        let legacy: LegacyMeridianSeveredPermanent = serde_json::from_value(value)?;
        Ok(MeridianSeveredPermanent {
            severed_meridians: legacy
                .severed_meridians
                .into_iter()
                .map(MeridianId::channel_id)
                .collect(),
            severed_at: legacy
                .severed_at
                .into_iter()
                .map(|(id, record)| (id.channel_id(), record))
                .collect(),
            dead_meridians: legacy
                .dead_meridians
                .into_iter()
                .map(MeridianId::channel_id)
                .collect(),
            // plan-race-system-v1 P5/PR-6a — 休眠登记是 RaceChange 换种族才产生的新
            // 状态，legacy v1 存档（早于本机制）没有对应字段，恒空迁移。
            dormant_meridians: HashMap::new(),
        })
    }

    /// 全闭合的 v1 legacy `meridians` JSON 样本（`MeridianId` PascalCase 枚举名
    /// channel id）——供**本模块之外**（`cultivation::mod` 的持久化 e2e 测试）构造
    /// "早于 P1 的旧存档" fixture 时复用，避免那类测试意外用上当前形态的
    /// `MeridianSystem::default()` 掩盖迁移分支未被真正走通的问题。
    #[cfg(test)]
    pub(crate) fn v1_all_closed_meridian_system_sample() -> serde_json::Value {
        fn entry(id: MeridianId) -> serde_json::Value {
            serde_json::json!({
                "id": format!("{id:?}"),
                "opened": false,
                "open_progress": 0.0,
                "flow_rate": 1.0,
                "flow_capacity": 10.0,
                "rate_tier": 0,
                "capacity_tier": 0,
                "throughput_current": 0.0,
                "integrity": 1.0,
                "cracks": [],
                "opened_at": 0,
            })
        }
        let regular: Vec<serde_json::Value> =
            MeridianId::REGULAR.iter().copied().map(entry).collect();
        let extraordinary: Vec<serde_json::Value> = MeridianId::EXTRAORDINARY
            .iter()
            .copied()
            .map(entry)
            .collect();
        serde_json::json!({ "regular": regular, "extraordinary": extraordinary })
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::cultivation::components::{CrackCause, MeridianChannelId};
        use crate::cultivation::meridian::severed::SeveredSource;

        /// 真实 v1 旧存档样本（`MeridianId` PascalCase 枚举名 channel id，
        /// `regular`/`extraordinary` 定长 12+8）——不是只测新形状的合成样本。
        fn v1_meridian_system_sample() -> serde_json::Value {
            let mut regular = Vec::new();
            for id in MeridianId::REGULAR {
                regular.push(serde_json::json!({
                    "id": format!("{id:?}"),
                    "opened": id == MeridianId::Lung,
                    "open_progress": if id == MeridianId::Lung { 1.0 } else { 0.3 },
                    "flow_rate": 1.0,
                    "flow_capacity": 10.0,
                    "rate_tier": 0,
                    "capacity_tier": 0,
                    "throughput_current": 0.0,
                    "integrity": 1.0,
                    "cracks": [],
                    "opened_at": if id == MeridianId::Lung { 42 } else { 0 },
                }));
            }
            let mut extraordinary = Vec::new();
            for id in MeridianId::EXTRAORDINARY {
                extraordinary.push(serde_json::json!({
                    "id": format!("{id:?}"),
                    "opened": id == MeridianId::Ren,
                    "open_progress": if id == MeridianId::Ren { 1.0 } else { 0.0 },
                    "flow_rate": 1.0,
                    "flow_capacity": 10.0,
                    "rate_tier": if id == MeridianId::Ren { 2 } else { 0 },
                    "capacity_tier": 0,
                    "throughput_current": 0.0,
                    "integrity": if id == MeridianId::Ren { 0.6 } else { 1.0 },
                    "cracks": [],
                    "opened_at": if id == MeridianId::Ren { 7 } else { 0 },
                }));
            }
            serde_json::json!({ "regular": regular, "extraordinary": extraordinary })
        }

        #[test]
        fn decode_meridian_system_migrates_v1_pascal_case_ids_to_snake_case_channel_ids() {
            let sample = v1_meridian_system_sample();
            let decoded = decode_meridian_system(sample, 1).expect("v1 sample should migrate");

            assert_eq!(decoded.regular.len(), 12);
            assert_eq!(decoded.extraordinary.len(), 8);

            let lung = decoded
                .regular
                .iter()
                .find(|m| m.id == MeridianChannelId::new("lung"))
                .expect("Lung must migrate to channel id \"lung\"");
            assert!(lung.opened, "Lung 逐脉状态（opened）必须在迁移中原样保留");
            assert_eq!(lung.opened_at, 42, "Lung opened_at 必须原样保留");

            let ren = decoded
                .extraordinary
                .iter()
                .find(|m| m.id == MeridianChannelId::new("ren"))
                .expect("Ren must migrate to channel id \"ren\"");
            assert!(ren.opened);
            assert_eq!(ren.rate_tier, 2, "Ren rate_tier 必须原样保留");
            assert_eq!(ren.integrity, 0.6, "Ren integrity 必须原样保留");
            assert_eq!(ren.opened_at, 7);

            // 逐脉状态对拍：除 Lung/Ren 外全部保持"未开、integrity=1.0"的样本基线。
            for m in decoded.regular.iter().chain(decoded.extraordinary.iter()) {
                if m.id == MeridianChannelId::new("lung") || m.id == MeridianChannelId::new("ren") {
                    continue;
                }
                assert!(
                    !m.opened,
                    "channel {} 样本基线未开，迁移不应把它变成 opened=true",
                    m.id
                );
                assert_eq!(
                    m.integrity, 1.0,
                    "channel {} integrity 样本基线应保持 1.0",
                    m.id
                );
            }
        }

        #[test]
        fn decode_meridian_system_v2_bundle_parses_directly_without_migration() {
            let fresh = MeridianSystem::default();
            let json = serde_json::to_value(&fresh).expect("serialize fresh MeridianSystem");
            let decoded = decode_meridian_system(json, CURRENT_BUNDLE_VERSION)
                .expect("v2 bundle should parse directly");
            assert_eq!(decoded.regular.len(), fresh.regular.len());
            assert_eq!(decoded.extraordinary.len(), fresh.extraordinary.len());
            assert_eq!(decoded.regular[0].id, fresh.regular[0].id);
        }

        #[test]
        fn decode_meridian_system_missing_version_defaults_to_legacy_path() {
            // bundle_version 参数由调用方从 `"v"` 字段推导，缺失时上游约定 unwrap_or(1)
            // ——本测试直接传 1 模拟"旧存档完全没有 v 字段"的路径,不经调用方那层。
            let sample = v1_meridian_system_sample();
            assert!(decode_meridian_system(sample, 1).is_ok());
        }

        #[test]
        fn decode_meridian_severed_migrates_v1_pascal_case_ids() {
            let sample = serde_json::json!({
                "severed_meridians": ["Lung", "Heart"],
                "severed_at": {
                    "Lung": { "at_tick": 100, "source": "CombatWound" },
                    "Heart": { "at_tick": 200, "source": "TribulationFail" },
                },
                "dead_meridians": ["Lung"],
            });
            let decoded =
                decode_meridian_severed(sample, 1).expect("v1 severed sample should migrate");

            assert!(decoded.is_severed(MeridianChannelId::new("lung")));
            assert!(decoded.is_severed(MeridianChannelId::new("heart")));
            assert!(
                !decoded.is_severed(MeridianChannelId::new("kidney")),
                "未在旧样本中出现的经脉不应被迁移函数意外标记为 SEVERED"
            );
            assert!(
                decoded.is_dead(MeridianChannelId::new("lung")),
                "Lung 的 dead 标记必须在迁移中保留"
            );
            assert!(
                !decoded.is_dead(MeridianChannelId::new("heart")),
                "Heart 只 SEVERED 不 dead，迁移不应误将其升级为死脉"
            );

            let lung_record = decoded
                .record_for(MeridianChannelId::new("lung"))
                .expect("Lung severed_at record must migrate");
            assert_eq!(lung_record.at_tick, 100);
            assert_eq!(lung_record.source, SeveredSource::CombatWound);

            let heart_record = decoded
                .record_for(MeridianChannelId::new("heart"))
                .expect("Heart severed_at record must migrate");
            assert_eq!(heart_record.at_tick, 200);
            assert_eq!(heart_record.source, SeveredSource::TribulationFail);
        }

        #[test]
        fn decode_meridian_severed_v2_bundle_parses_directly_without_migration() {
            let mut permanent = MeridianSeveredPermanent::default();
            permanent.insert(
                MeridianId::Kidney.channel_id(),
                SeveredSource::OverloadTear,
                55,
            );
            let json = serde_json::to_value(&permanent).expect("serialize");
            let decoded = decode_meridian_severed(json, CURRENT_BUNDLE_VERSION)
                .expect("v2 bundle should parse directly");
            assert_eq!(decoded, permanent);
        }

        #[test]
        fn decode_meridian_severed_empty_v1_sample_is_valid() {
            let sample = serde_json::json!({
                "severed_meridians": [],
                "severed_at": {},
                "dead_meridians": [],
            });
            let decoded =
                decode_meridian_severed(sample, 1).expect("empty v1 sample should migrate");
            assert_eq!(decoded.severed_count(), 0);
        }

        /// plan-race-system-v1 P1 对抗审查 MINOR ③：`LegacyMeridian` 的标量字段（除
        /// `cracks` 外）均无 `#[serde(default)]`——缺失任一必填标量字段的 v1 存档条目
        /// 必须被拒绝而不是静默补零/静默丢弃该经脉（那会悄悄伪造一条"从未存在过"的
        /// 经脉状态）。本用例逐个删掉 `opened_at`/`flow_rate` 验证两者都触发拒绝。
        #[test]
        fn decode_meridian_system_rejects_legacy_entry_missing_required_scalar_field() {
            // "opened_at" 字段缺失（其余标量字段齐全）。
            let entry_missing_opened_at = serde_json::json!({
                "id": "Lung",
                "opened": false,
                "open_progress": 0.0,
                "flow_rate": 1.0,
                "flow_capacity": 10.0,
                "rate_tier": 0,
                "capacity_tier": 0,
                "throughput_current": 0.0,
                "integrity": 1.0,
                "cracks": [],
            });
            let broken = serde_json::json!({
                "regular": [entry_missing_opened_at],
                "extraordinary": [],
            });
            assert!(
                decode_meridian_system(broken, 1).is_err(),
                "缺 opened_at 的 legacy meridian 条目必须被拒绝，不能静默补 0"
            );

            // "flow_rate" 字段缺失（其余标量字段齐全）。
            let entry_missing_flow_rate = serde_json::json!({
                "id": "Lung",
                "opened": false,
                "open_progress": 0.0,
                "flow_capacity": 10.0,
                "rate_tier": 0,
                "capacity_tier": 0,
                "throughput_current": 0.0,
                "integrity": 1.0,
                "cracks": [],
                "opened_at": 0,
            });
            let broken2 = serde_json::json!({
                "regular": [entry_missing_flow_rate],
                "extraordinary": [],
            });
            assert!(
                decode_meridian_system(broken2, 1).is_err(),
                "缺 flow_rate 的 legacy meridian 条目必须被拒绝，不能静默补 0"
            );
        }

        #[test]
        fn decode_meridian_system_rejects_malformed_legacy_json() {
            let broken = serde_json::json!({ "regular": "not an array" });
            assert!(decode_meridian_system(broken, 1).is_err());
        }

        /// `CrackCause` 走 legacy `Meridian.cracks` 字段——确认迁移路径下 crack 列表
        /// （含 cause 枚举）本身也被正确保留，不只是顶层标量字段。
        #[test]
        fn decode_meridian_system_preserves_cracks_through_migration() {
            let mut regular = Vec::new();
            for id in MeridianId::REGULAR {
                let cracks = if id == MeridianId::Lung {
                    serde_json::json!([{
                        "severity": 0.4,
                        "healing_progress": 0.1,
                        "cause": "Overload",
                        "created_at": 10,
                    }])
                } else {
                    serde_json::json!([])
                };
                regular.push(serde_json::json!({
                    "id": format!("{id:?}"),
                    "opened": false,
                    "open_progress": 0.0,
                    "flow_rate": 1.0,
                    "flow_capacity": 10.0,
                    "rate_tier": 0,
                    "capacity_tier": 0,
                    "throughput_current": 0.0,
                    "integrity": 1.0,
                    "cracks": cracks,
                    "opened_at": 0,
                }));
            }
            let extraordinary: Vec<serde_json::Value> = MeridianId::EXTRAORDINARY
                .iter()
                .map(|id| {
                    serde_json::json!({
                        "id": format!("{id:?}"),
                        "opened": false,
                        "open_progress": 0.0,
                        "flow_rate": 1.0,
                        "flow_capacity": 10.0,
                        "rate_tier": 0,
                        "capacity_tier": 0,
                        "throughput_current": 0.0,
                        "integrity": 1.0,
                        "cracks": [],
                        "opened_at": 0,
                    })
                })
                .collect();
            let sample = serde_json::json!({ "regular": regular, "extraordinary": extraordinary });

            let decoded = decode_meridian_system(sample, 1).expect("sample should migrate");
            let lung = decoded
                .regular
                .iter()
                .find(|m| m.id == MeridianChannelId::new("lung"))
                .unwrap();
            assert_eq!(lung.cracks.len(), 1);
            assert_eq!(lung.cracks[0].severity, 0.4);
            assert_eq!(lung.cracks[0].cause, CrackCause::Overload);
        }
    }
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

    use crate::body_plan::{RaceId, RaceRegistry};
    use crate::coffin::{
        CoffinComponent, CoffinEntity, CoffinGrade, CoffinRegistry, CoffinStateChanged,
    };
    use crate::combat::components::Lifecycle;
    use crate::cultivation::components::{ColorKind, ContamSource};
    use crate::cultivation::lifespan::{DeathRegistry, LifespanCapTable, LifespanComponent};
    use crate::persistence::{
        load_active_tribulation, load_ascension_quota, persist_active_tribulation,
        ActiveTribulationRecord, PersistenceSettings,
    };
    use crate::player::state::canonical_player_id;
    use crate::player::state::PlayerState;
    use crate::skill::events::SkillCapChanged;
    use crate::world::dimension::{CurrentDimension, DimensionKind, DimensionLayers};
    use crate::world::spawn_tutorial::{TutorialHook, TutorialState, TutorialTelemetry};
    use std::collections::HashMap;
    use valence::prelude::{
        App, BlockPos, EntityLayerId, Events, IntoSystemConfigs, Position, Update,
        VisibleChunkLayer, VisibleEntityLayers,
    };
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

    fn insert_reincarnation_cleanup_resources(app: &mut App) {
        app.insert_resource(PendingJueBiTriggers::default());
        app.insert_resource(HalfStepRechallengeQueue::default());
    }

    fn reincarnation_hydration_app(
        settings: &PersistenceSettings,
        player_persistence: &crate::player::state::PlayerStatePersistence,
    ) -> App {
        let mut app = App::new();
        insert_test_dimension_layers(&mut app);
        app.insert_resource(settings.clone());
        app.insert_resource(player_persistence.clone());
        let (default_loadout, item_registry, allocator) = inventory_test_resources();
        app.insert_resource(default_loadout);
        app.insert_resource(item_registry);
        app.insert_resource(allocator);
        let mut zones = ZoneRegistry::fallback();
        zones
            .find_zone_mut(crate::world::zone::DEFAULT_SPAWN_ZONE_NAME)
            .expect("fallback registry must include spawn")
            .spirit_qi = 1.0;
        app.insert_resource(zones);
        app.insert_resource(WorldQiAccount::default());
        app.add_event::<QiTransfer>();
        insert_reincarnation_cleanup_resources(&mut app);
        app.add_systems(
            Update,
            (
                attach_cultivation_to_joined_clients,
                crate::combat::attach_combat_bundle_to_joined_clients
                    .after(attach_cultivation_to_joined_clients),
            ),
        );
        app
    }

    fn persisted_test_craft_session(username: &str) -> crate::craft::CraftSession {
        crate::craft::CraftSession {
            recipe_id: crate::craft::RecipeId::new("craft.test.previous_life"),
            started_at_tick: 10,
            remaining_ticks: 37,
            total_ticks: 40,
            owner_player_id: canonical_player_id(username),
            qi_paid: 0.0,
            quantity_total: 3,
            completed_count: 1,
        }
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

    /// plan-race-system-v1 P5/PR-6c —— `IntrinsicRace` 曾经零处 insert（6a 只在
    /// `RaceChange` 换种族事务里才写它）。首次 join（无持久化 bundle,新角色）也必须
    /// 在同一帧拿到 `IntrinsicRace`,且必须与同批插入的 `Cultivation.race` 一致——
    /// 这是本 PR 关闭的孤岛核心断言。
    #[test]
    fn joined_client_receives_intrinsic_race_matching_cultivation_race_on_first_join() {
        let mut app = App::new();
        app.insert_resource(PersistenceSettings::default());
        app.add_systems(Update, attach_cultivation_to_joined_clients);

        let (client_bundle, _helper) = create_mock_client("FreshJoiner");
        let entity = app.world_mut().spawn(client_bundle).id();

        app.update();

        let cultivation = app
            .world()
            .get::<crate::cultivation::components::Cultivation>(entity)
            .expect("joined client should receive Cultivation");
        let intrinsic_race = app
            .world()
            .get::<crate::body_plan::IntrinsicRace>(entity)
            .expect(
                "joined client should receive IntrinsicRace on first join (no RaceChange needed)",
            );

        assert_eq!(
            intrinsic_race.0, cultivation.race,
            "IntrinsicRace must mirror the freshly-hydrated Cultivation.race"
        );
        assert_eq!(
            intrinsic_race.0,
            RaceId::new(crate::body_plan::HUMAN_RACE_ID),
            "brand-new character with no persisted bundle must default to the human race"
        );
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

    // ─── plan-remains-suite P0：join 转世门 ──────────────────────────────────

    fn player_state_persistence_for(
        settings: &PersistenceSettings,
        temp_root: &std::path::Path,
    ) -> crate::player::state::PlayerStatePersistence {
        crate::player::state::PlayerStatePersistence::with_db_path(
            temp_root.join("data"),
            settings.db_path(),
        )
    }

    fn insert_test_dimension_layers(app: &mut App) -> DimensionLayers {
        let layers = DimensionLayers {
            overworld: app.world_mut().spawn_empty().id(),
            tsy: app.world_mut().spawn_empty().id(),
        };
        app.insert_resource(layers);
        layers
    }

    fn inventory_test_resources() -> (
        crate::inventory::DefaultLoadout,
        crate::inventory::ItemRegistry,
        crate::inventory::InventoryInstanceIdAllocator,
    ) {
        let item_registry =
            crate::inventory::load_item_registry().expect("item registry should load");
        let default_loadout = crate::inventory::load_default_loadout(&item_registry)
            .expect("default loadout should load");
        (
            crate::inventory::DefaultLoadout(default_loadout),
            item_registry,
            crate::inventory::InventoryInstanceIdAllocator::default(),
        )
    }

    fn terminated_life_record(character_id: &str) -> LifeRecord {
        let mut record = LifeRecord::new(character_id.to_string());
        record.push(BiographyEntry::NearDeath {
            cause: "old_test_wound".to_string(),
            tick: 40,
        });
        record.push(BiographyEntry::Terminated {
            cause: "natural_end".to_string(),
            tick: 50,
        });
        record
    }

    fn coffin_runtime_for_player(
        player: Entity,
        lower: BlockPos,
        grade: CoffinGrade,
    ) -> CoffinRegistry {
        let coffin = CoffinEntity {
            lower,
            upper: BlockPos::new(lower.x + 1, lower.y, lower.z),
            occupied_by: Some(player),
            placed_at_tick: 20,
            grade,
            marker_entity: None,
        };
        CoffinRegistry {
            coffins: HashMap::from([(coffin.lower, coffin), (coffin.upper, coffin)]),
            player_in_coffin: HashMap::from([(player, lower)]),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn seed_cultivation_bundle_with_tutorial_and_qi(
        settings: &PersistenceSettings,
        username: &str,
        realm: Realm,
        qi_current: f64,
        life_record: &LifeRecord,
        tutorial_state: Option<&TutorialState>,
    ) {
        crate::persistence::persist_player_cultivation_bundle(
            settings,
            username,
            &Cultivation {
                realm,
                qi_current,
                ..Default::default()
            },
            &MeridianSystem::default(),
            &QiColor::default(),
            &Karma::default(),
            &Contamination::default(),
            life_record,
            &PracticeLog::default(),
            &InsightQuota::default(),
            &UnlockedPerceptions::default(),
            &InsightModifiers::new(),
            tutorial_state,
            &MeridianSeveredPermanent::default(),
            None,
            None,
        )
        .expect("seeding cultivation bundle should succeed");
    }

    #[allow(clippy::too_many_arguments)]
    fn seed_cultivation_bundle_with_tutorial(
        settings: &PersistenceSettings,
        username: &str,
        realm: Realm,
        life_record: &LifeRecord,
        tutorial_state: Option<&TutorialState>,
    ) {
        seed_cultivation_bundle_with_tutorial_and_qi(
            settings,
            username,
            realm,
            0.0,
            life_record,
            tutorial_state,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn seed_cultivation_bundle(
        settings: &PersistenceSettings,
        username: &str,
        realm: Realm,
        life_record: &LifeRecord,
    ) {
        seed_cultivation_bundle_with_tutorial(settings, username, realm, life_record, None);
    }

    #[test]
    fn join_with_terminated_character_reincarnates_exactly_once() {
        let (settings, root) = temp_persistence_settings("reincarnate-join");
        let player_persistence = player_state_persistence_for(&settings, &root);

        // 预置：「Azure」的当前角色已终结——唯一轮换入口（终结屏「开启新生」）从未被点过。
        let old_raw_id =
            crate::player::state::rotate_current_character_id(&player_persistence, "Azure")
                .expect("seeding current_char_id should succeed");
        let old_canonical_id = crate::player::state::player_character_id("Azure", &old_raw_id);
        let terminated_record = terminated_life_record(&old_canonical_id);
        let persisted_nourishment = Nourishment {
            satiety: 19.0,
            hydration: 31.0,
        };
        crate::persistence::persist_player_cultivation_bundle_with_nourishment(
            &settings,
            "Azure",
            &Cultivation {
                realm: Realm::Spirit,
                ..Default::default()
            },
            &MeridianSystem::default(),
            &QiColor::default(),
            &Karma::default(),
            &Contamination::default(),
            &terminated_record,
            &PracticeLog::default(),
            &InsightQuota::default(),
            &UnlockedPerceptions::default(),
            &InsightModifiers::new(),
            None,
            &MeridianSeveredPermanent::default(),
            None,
            None,
            Some(&persisted_nourishment),
        )
        .expect("seeding terminated cultivation bundle with nourishment should succeed");
        let seeded_bundle = crate::persistence::load_player_cultivation_bundle(&settings, "Azure")
            .expect("seeded bundle reload should succeed")
            .expect("seeded bundle should exist before join");
        assert_eq!(
            serde_json::from_value::<Nourishment>(seeded_bundle["nourishment"].clone())
                .expect("seeded nourishment should decode"),
            persisted_nourishment,
            "SQLite must contain the non-default nourishment axes before the real join attach"
        );
        assert!(
            seeded_bundle.get("nourishment_activity_window").is_none(),
            "session-only activity must never be seeded into the cultivation bundle"
        );
        let old_player_state = PlayerState {
            karma: 0.4,
            inventory_score: 0.8,
        };
        crate::player::state::save_player_core_slice(
            &player_persistence,
            "Azure",
            &old_player_state,
        )
        .expect("seeding the terminated character's player core should succeed");
        // 老角色寿元耗尽（Spirit cap 满）——复现"ECS 本 session 挂着旧值又立刻老死"的坑：
        // 转世门必须无条件覆写它，不能指望这份 exhausted 值自然被替换掉。
        let exhausted_lifespan = LifespanComponent {
            born_at_tick: 0,
            years_lived: LifespanCapTable::SPIRIT as f64,
            cap_by_realm: LifespanCapTable::SPIRIT,
            offline_pause_tick: None,
        };
        crate::player::state::save_player_lifespan_slice(
            &player_persistence,
            "Azure",
            &exhausted_lifespan,
        )
        .expect("seeding exhausted lifespan should succeed");
        let old_craft_session = persisted_test_craft_session("Azure");
        crate::player::state::save_player_inventory_and_craft_session_slices(
            &player_persistence,
            "Azure",
            None,
            Some(&old_craft_session),
        )
        .expect("seeding the terminated character's craft session should succeed");

        let mut app = App::new();
        insert_test_dimension_layers(&mut app);
        app.insert_resource(settings.clone());
        app.insert_resource(player_persistence.clone());
        let (default_loadout, item_registry, allocator) = inventory_test_resources();
        app.insert_resource(default_loadout);
        app.insert_resource(item_registry);
        app.insert_resource(allocator);
        app.insert_resource(crate::player::gameplay::PendingGameplayNarrations::default());
        insert_reincarnation_cleanup_resources(&mut app);
        app.insert_resource(CombatClock { tick: 50 });
        app.add_event::<AscensionQuotaOpened>();
        app.add_event::<HalfStepRechallengeTriggerEvent>();
        app.add_systems(
            Update,
            (
                attach_cultivation_to_joined_clients,
                dispatch_rechallenge_on_quota_opened_system
                    .after(attach_cultivation_to_joined_clients),
            ),
        );

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                old_player_state.clone(),
                // 生产链路里 `attach_player_state_to_joined_clients` 会先把这份 exhausted
                // lifespan 挂上；这里手动模拟那一步，复现"restored_lifespan 已经 Some"
                // 的真实时序，而不是让测试绕过这条风险路径。
                exhausted_lifespan.clone(),
                old_craft_session.clone(),
                crate::network::craft_emit::CraftSessionPersistenceDirty,
                crate::network::craft_emit::CraftSessionStateDirty,
            ))
            .id();
        app.world_mut()
            .resource_mut::<PendingJueBiTriggers>()
            .schedule_for_character(
                &JueBiTriggerEvent {
                    entity,
                    character_id: Some(old_canonical_id.clone()),
                    source: JueBiTriggerSource::VoidQuotaExceeded,
                    triggered_at_tick: 10,
                    delay_ticks: 200,
                    epicenter: None,
                },
                old_canonical_id.clone(),
            );
        app.world_mut()
            .resource_mut::<HalfStepRechallengeQueue>()
            .enqueue(crate::cultivation::tribulation::HalfStepRechallengeEntry {
                char_id: old_canonical_id.clone(),
                entity,
                entered_at: 10,
                rechallenge_window_until: 10_000,
                is_dormant: false,
                buff_applied: true,
            });
        app.world_mut()
            .send_event(AscensionQuotaOpened { occupied_slots: 3 });

        app.update();

        assert_eq!(
            app.world()
                .get::<PlayerState>(entity)
                .expect("reincarnated client should retain PlayerState"),
            &PlayerState::default(),
            "join-time reincarnation must replace the terminated character's runtime player core before autosave can publish it again"
        );
        let reincarnated_nourishment = *app
            .world()
            .get::<Nourishment>(entity)
            .expect("reincarnated client should have Nourishment");
        assert_eq!(
            reincarnated_nourishment,
            Nourishment::spawn_default(),
            "the real join-time reincarnation attach must discard both nourishment axes from the terminated body"
        );
        assert_eq!(
            (
                reincarnated_nourishment.satiety,
                reincarnated_nourishment.hydration,
            ),
            (80.0, 80.0),
            "join-time reincarnation must attach the new body's exact 80/80 nourishment spawn state"
        );
        assert_eq!(
            *app.world()
                .get::<crate::nourishment::tick::NourishmentActivityWindow>(entity)
                .expect("reincarnated client should have a fresh nourishment activity window"),
            crate::nourishment::tick::NourishmentActivityWindow::default(),
            "join-time reincarnation must start a fresh session activity window"
        );
        assert!(
            app.world()
                .get::<crate::craft::CraftSession>(entity)
                .is_none(),
            "join-time reincarnation must remove the terminated character's active craft session"
        );
        assert!(
            app.world()
                .get::<crate::network::craft_emit::CraftSessionPersistenceDirty>(entity)
                .is_none(),
            "join-time reincarnation must clear stale craft persistence dirty state before a later flush can restore the old session"
        );
        assert!(
            app.world()
                .get::<crate::network::craft_emit::CraftSessionStateDirty>(entity)
                .is_some(),
            "join-time reincarnation must request an inactive craft-session payload so the client drops old progress UI"
        );
        assert!(
            app.world().resource::<PendingJueBiTriggers>().is_empty(),
            "join-time reincarnation must cancel delayed JueBi work owned by the terminated character"
        );
        assert!(
            app.world()
                .resource::<HalfStepRechallengeQueue>()
                .is_empty(),
            "join-time reincarnation must remove the terminated character from the HalfStep FIFO"
        );
        assert_eq!(
            app.world_mut()
                .resource_mut::<Events<HalfStepRechallengeTriggerEvent>>()
                .drain()
                .count(),
            0,
            "a quota opening in the reincarnation frame must not dispatch a HalfStep retry for the terminated identity"
        );

        let cultivation = app
            .world()
            .get::<Cultivation>(entity)
            .expect("reincarnated client should have Cultivation");
        assert_eq!(
            cultivation.realm,
            Realm::Awaken,
            "转世后应回到醒灵境，不能继承终结前的 Spirit 境界"
        );

        let life_record = app
            .world()
            .get::<LifeRecord>(entity)
            .expect("reincarnated client should have LifeRecord");
        assert_ne!(
            life_record.character_id, old_canonical_id,
            "转世必须换发新的 character_id"
        );
        assert!(
            life_record.biography.is_empty(),
            "新角色的生平卷应从空白开始，不应带着旧角色的 Terminated 记录；实际={:?}",
            life_record.biography
        );

        let death_registry = app
            .world()
            .get::<DeathRegistry>(entity)
            .expect("reincarnated client should have DeathRegistry");
        assert_eq!(
            death_registry.char_id, life_record.character_id,
            "DeathRegistry 必须绑定新 char_id，不能停留在旧角色上"
        );

        let lifespan = app
            .world()
            .get::<LifespanComponent>(entity)
            .expect("reincarnated client should have LifespanComponent");
        assert_eq!(
            lifespan.cap_by_realm,
            LifespanCapTable::AWAKEN,
            "转世后寿元 cap 必须回到 AWAKEN"
        );
        assert_eq!(
            lifespan.years_lived, 0.0,
            "转世后寿元必须清零——不能带着旧角色耗尽的 years_lived 上线，\
             否则下一次 lifespan tick 会立刻把刚转世的新角色又判定老死（死循环复现）"
        );

        let new_raw_id =
            crate::player::state::load_current_character_id(&player_persistence, "Azure")
                .expect("load current_char_id should succeed")
                .expect("current_char_id should be set after reincarnation");
        assert_ne!(
            new_raw_id, old_raw_id,
            "player_core.current_char_id 必须完成一次轮换"
        );

        assert!(
            app.world()
                .get::<crate::inventory::PlayerInventory>(entity)
                .is_some(),
            "转世应发一份全新默认 loadout 背包"
        );

        assert!(
            app.world_mut()
                .query::<&crate::inventory::RemainsContainer>()
                .iter(app.world())
                .next()
                .is_none(),
            "join 转世门不应重放死亡链——不该生成任何遗骸容器"
        );

        let narrations = app
            .world_mut()
            .resource_mut::<crate::player::gameplay::PendingGameplayNarrations>()
            .drain();
        assert_eq!(narrations.len(), 1, "转世应给玩家恰好一条提示 narration");
        assert_eq!(narrations[0].target.as_deref(), Some("Azure"));

        // 幂等性前提：落盘的 player core 与 cultivation bundle 都必须属于新角色。
        // 生产 autosave 直接写 ECS PlayerState；这里用同一持久化 helper 再写一次，证明旧值
        // 不会在事务成功后的下一次 core flush 中反向污染新角色行。
        let persisted_player =
            crate::player::state::load_player_slices(&player_persistence, "Azure");
        assert_eq!(
            persisted_player.state,
            PlayerState::default(),
            "join-time reincarnation transaction must immediately replace the terminated character's durable player core"
        );
        assert!(
            persisted_player.craft_session.is_none(),
            "join-time reincarnation transaction must delete the terminated character's durable craft session"
        );
        let runtime_player_state = app
            .world()
            .get::<PlayerState>(entity)
            .expect("reincarnated client should retain PlayerState")
            .clone();
        crate::player::state::save_player_core_slice(
            &player_persistence,
            "Azure",
            &runtime_player_state,
        )
        .expect("simulated production core autosave should succeed");
        let runtime_inventory = app
            .world()
            .get::<crate::inventory::PlayerInventory>(entity)
            .expect("reincarnated client should have a fresh inventory")
            .clone();
        crate::player::state::save_player_inventory_and_craft_session_slices(
            &player_persistence,
            "Azure",
            Some(&runtime_inventory),
            app.world().get::<crate::craft::CraftSession>(entity),
        )
        .expect("simulated production inventory/session flush should succeed");
        let persisted_after_flush =
            crate::player::state::load_player_slices(&player_persistence, "Azure");
        assert_eq!(
            persisted_after_flush.state,
            PlayerState::default(),
            "the first core autosave after reincarnation must not restore the terminated character's karma or inventory score"
        );
        assert!(
            persisted_after_flush.craft_session.is_none(),
            "the first inventory/session flush after reincarnation must not restore the terminated character's craft session"
        );

        // 幂等性前提：落盘的 cultivation bundle 也必须是新的（不是 Terminated），
        // 否则玩家断线重连时"下一次 join"仍会再次误判终结。
        let persisted = crate::persistence::load_player_cultivation_bundle(&settings, "Azure")
            .expect("reload should succeed")
            .expect("bundle should exist after reincarnation");
        let persisted_life_record: LifeRecord =
            serde_json::from_value(persisted["life_record"].clone())
                .expect("persisted life_record should decode");
        assert!(
            persisted_life_record.biography.is_empty(),
            "落盘的 life_record 也必须是新角色的空白生平，否则重连会再次触发转世"
        );
        let persisted_reincarnated_nourishment =
            serde_json::from_value::<Nourishment>(persisted["nourishment"].clone())
                .expect("persisted reincarnated nourishment should decode");
        assert_eq!(
            persisted_reincarnated_nourishment,
            Nourishment::spawn_default(),
            "join-time reincarnation must immediately overwrite SQLite with the new body's nourishment"
        );
        assert_eq!(
            (
                persisted_reincarnated_nourishment.satiety,
                persisted_reincarnated_nourishment.hydration,
            ),
            (80.0, 80.0),
            "the immediate SQLite overwrite must contain exact 80/80 nourishment axes"
        );
        assert!(
            persisted.get("nourishment_activity_window").is_none(),
            "the immediate SQLite overwrite must not serialize session-only activity"
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn join_reincarnation_settles_positive_old_life_qi_before_identity_rotation() {
        let (settings, root) = temp_persistence_settings("reincarnate-join-positive-qi");
        let player_persistence = player_state_persistence_for(&settings, &root);
        let username = "QiBearingJoin";
        let old_raw_id =
            crate::player::state::rotate_current_character_id(&player_persistence, username)
                .expect("seeding current_char_id should succeed");
        let old_canonical_id =
            crate::player::state::player_character_id(username, old_raw_id.as_str());
        let old_qi = 7.0;
        seed_cultivation_bundle_with_tutorial_and_qi(
            &settings,
            username,
            Realm::Spirit,
            old_qi,
            &terminated_life_record(old_canonical_id.as_str()),
            None,
        );

        let mut app = App::new();
        insert_test_dimension_layers(&mut app);
        app.insert_resource(settings.clone());
        app.insert_resource(player_persistence.clone());
        let (default_loadout, item_registry, allocator) = inventory_test_resources();
        app.insert_resource(default_loadout);
        app.insert_resource(item_registry);
        app.insert_resource(allocator);
        app.insert_resource(ZoneRegistry::fallback());
        app.insert_resource(WorldQiAccount::default());
        app.add_event::<QiTransfer>();
        insert_reincarnation_cleanup_resources(&mut app);
        app.add_systems(Update, attach_cultivation_to_joined_clients);

        let spawn_zone_before = app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name(crate::world::zone::DEFAULT_SPAWN_ZONE_NAME)
            .expect("fallback registry must include spawn")
            .spirit_qi;
        let (mut client_bundle, _helper) = create_mock_client(username);
        client_bundle.player.position = Position::new([0.0, 66.0, 0.0]);
        let entity = app
            .world_mut()
            .spawn((client_bundle, CurrentDimension(DimensionKind::Overworld)))
            .id();

        app.update();

        let fresh_character_id = {
            let lifecycle = app
                .world()
                .get::<Lifecycle>(entity)
                .expect("join reincarnation must publish the fresh lifecycle");
            assert_ne!(
                lifecycle.character_id, old_canonical_id,
                "the new identity must rotate only after old-life qi is accounted"
            );
            lifecycle.character_id.clone()
        };
        assert_eq!(
            app.world()
                .get::<Cultivation>(entity)
                .expect("fresh cultivation must be attached")
                .qi_current,
            0.0
        );
        let spawn_zone_after = app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name(crate::world::zone::DEFAULT_SPAWN_ZONE_NAME)
            .expect("runtime spawn zone must remain available")
            .spirit_qi;
        let zone_qi_delta = (spawn_zone_after - spawn_zone_before)
            * crate::qi_physics::constants::QI_ZONE_UNIT_CAPACITY;
        let pending_qi = app
            .world()
            .resource::<WorldQiAccount>()
            .balance(&pending_inflow_account());
        let tolerance = old_qi * f64::EPSILON * 8.0;
        assert!(
            ((zone_qi_delta + pending_qi) - old_qi).abs() <= tolerance,
            "old-life qi must equal zone + durable pending inflow before identity rotation: old={old_qi}, zone={zone_qi_delta}, pending={pending_qi}"
        );
        let ledger = app.world().resource::<WorldQiAccount>();
        assert!(
            !ledger.transfers().is_empty(),
            "positive old-life qi must produce at least one release audit"
        );
        assert!(
            ledger
                .transfers()
                .iter()
                .all(|transfer| transfer.from == QiAccountId::player(old_canonical_id.clone())),
            "every split release audit must remain owned by the terminated identity"
        );
        assert!(
            (ledger
                .transfers()
                .iter()
                .map(|transfer| transfer.amount)
                .sum::<f64>()
                - old_qi)
                .abs()
                <= tolerance,
            "zone acceptance plus pending overflow audits must preserve all old-life qi"
        );
        let emitted: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<QiTransfer>>()
            .drain()
            .collect();
        assert!(
            !emitted.is_empty(),
            "positive old-life qi must emit its release audits"
        );
        assert!(emitted
            .iter()
            .all(|transfer| transfer.from == QiAccountId::player(old_canonical_id.clone())));
        assert!(
            (emitted.iter().map(|transfer| transfer.amount).sum::<f64>() - old_qi).abs()
                <= tolerance
        );

        let persisted_bundle =
            crate::persistence::load_player_cultivation_bundle(&settings, username)
                .expect("fresh cultivation bundle should reload")
                .expect("join reincarnation must commit a fresh bundle");
        assert_eq!(
            serde_json::from_value::<Cultivation>(persisted_bundle["cultivation"].clone())
                .expect("persisted cultivation should decode")
                .qi_current,
            0.0
        );
        let persisted_lifecycle =
            crate::player::state::load_player_lifecycle_slice(&player_persistence, username, 0)
                .expect("fresh lifecycle should reload")
                .expect("join reincarnation must commit its lifecycle slice");
        assert_eq!(
            persisted_lifecycle.state,
            crate::combat::components::LifecycleState::Alive
        );
        assert_eq!(persisted_lifecycle.character_id, fresh_character_id);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn terminated_bundle_validation_rejects_every_unreadable_slice() {
        let life_record = terminated_life_record("offline:BundleValidation");
        let valid = serde_json::json!({
            "v": legacy_meridian_bundle::CURRENT_BUNDLE_VERSION,
            "cultivation": Cultivation::default(),
            "meridians": MeridianSystem::default(),
            "qi_color": QiColor::default(),
            "karma": Karma::default(),
            "contamination": Contamination::default(),
            "life_record": life_record,
            "practice_log": PracticeLog::default(),
            "insight_quota": InsightQuota::default(),
            "unlocked_perceptions": UnlockedPerceptions::default(),
            "insight_modifiers": InsightModifiers::new(),
            "tutorial_state": null,
            "meridian_severed": MeridianSeveredPermanent::default(),
            "poison_toxicity": null,
            "digestion_load": null,
            "nourishment": null,
        });
        assert!(validate_terminated_persisted_bundle(
            &valid,
            legacy_meridian_bundle::CURRENT_BUNDLE_VERSION
        )
        .is_ok());

        let required = [
            "cultivation",
            "meridians",
            "qi_color",
            "karma",
            "contamination",
            "life_record",
            "practice_log",
            "insight_quota",
            "unlocked_perceptions",
            "insight_modifiers",
            "meridian_severed",
        ];
        for key in required {
            let mut missing = valid.clone();
            missing
                .as_object_mut()
                .expect("bundle fixture must be an object")
                .remove(key);
            assert!(
                validate_terminated_persisted_bundle(
                    &missing,
                    legacy_meridian_bundle::CURRENT_BUNDLE_VERSION
                )
                .is_err(),
                "required slice `{key}` must not fall back when missing"
            );

            let mut malformed = valid.clone();
            malformed[key] = serde_json::json!("unreadable");
            assert!(
                validate_terminated_persisted_bundle(
                    &malformed,
                    legacy_meridian_bundle::CURRENT_BUNDLE_VERSION
                )
                .is_err(),
                "required slice `{key}` must not fall back when malformed"
            );
        }

        for key in [
            "tutorial_state",
            "poison_toxicity",
            "digestion_load",
            "nourishment",
        ] {
            let mut malformed = valid.clone();
            malformed[key] = serde_json::json!("unreadable");
            assert!(
                validate_terminated_persisted_bundle(
                    &malformed,
                    legacy_meridian_bundle::CURRENT_BUNDLE_VERSION
                )
                .is_err(),
                "present optional slice `{key}` must not be silently discarded when malformed"
            );
        }

        let mut invalid_version = valid;
        invalid_version["v"] = serde_json::json!("current");
        assert!(
            validate_terminated_persisted_bundle(&invalid_version, 1).is_err(),
            "a present non-integer bundle version must fail closed"
        );
    }

    #[test]
    fn join_reincarnation_rejects_terminated_life_record_from_stale_identity() {
        let (settings, root) = temp_persistence_settings("reincarnate-stale-life-record");
        let player_persistence = player_state_persistence_for(&settings, &root);
        let username = "StaleLifeRecordJoin";
        let current_raw_id =
            crate::player::state::rotate_current_character_id(&player_persistence, username)
                .expect("seeding current_char_id should succeed");
        let current_character_id =
            crate::player::state::player_character_id(username, current_raw_id.as_str());
        let stale_character_id = crate::player::state::player_character_id(username, "stale-life");
        let old_qi = crate::qi_physics::constants::QI_EPSILON / 2.0;
        seed_cultivation_bundle_with_tutorial_and_qi(
            &settings,
            username,
            Realm::Spirit,
            old_qi,
            &terminated_life_record(stale_character_id.as_str()),
            None,
        );
        let persisted_before =
            crate::persistence::load_player_cultivation_bundle(&settings, username)
                .expect("stale bundle should load")
                .expect("stale bundle should exist");

        let mut app = reincarnation_hydration_app(&settings, &player_persistence);
        let (mut client_bundle, _helper) = create_mock_client(username);
        client_bundle.player.position = Position::new([0.0, 66.0, 0.0]);
        let entity = app
            .world_mut()
            .spawn((client_bundle, CurrentDimension(DimensionKind::Overworld)))
            .id();

        app.update();

        assert!(app.world().get::<Cultivation>(entity).is_none());
        assert!(app.world().get::<Lifecycle>(entity).is_none());
        assert!(app.world().get::<CultivationAttachRetry>(entity).is_some());
        assert_eq!(
            crate::player::state::load_current_character_id(&player_persistence, username)
                .expect("current identity should reload"),
            Some(current_raw_id),
            "a stale terminated LifeRecord must not rotate the current identity"
        );
        assert_eq!(
            current_character_id,
            crate::player::state::player_character_id(
                username,
                crate::player::state::load_current_character_id(&player_persistence, username)
                    .unwrap()
                    .unwrap()
                    .as_str()
            )
        );
        assert_eq!(
            crate::persistence::load_player_cultivation_bundle(&settings, username)
                .expect("stale bundle should remain readable")
                .expect("stale bundle should remain durable"),
            persisted_before
        );
        assert_eq!(
            app.world()
                .resource::<WorldQiAccount>()
                .balance(&pending_inflow_account()),
            0.0
        );
        assert_eq!(app.world().resource::<Events<QiTransfer>>().len(), 0);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn join_hydration_rejects_current_lifecycle_terminated_life_record_alive_conflict() {
        let (settings, root) = temp_persistence_settings("reincarnate-reverse-conflict");
        let player_persistence = player_state_persistence_for(&settings, &root);
        let username = "ReverseTerminationConflict";
        let current_raw_id =
            crate::player::state::rotate_current_character_id(&player_persistence, username)
                .expect("seeding current_char_id should succeed");
        let current_character_id =
            crate::player::state::player_character_id(username, current_raw_id.as_str());
        let old_qi = crate::qi_physics::constants::QI_EPSILON / 2.0;
        seed_cultivation_bundle_with_tutorial_and_qi(
            &settings,
            username,
            Realm::Spirit,
            old_qi,
            &LifeRecord::new(current_character_id.clone()),
            None,
        );
        crate::player::state::save_player_lifecycle_slice(
            &player_persistence,
            username,
            &crate::combat::components::Lifecycle {
                state: crate::combat::components::LifecycleState::Terminated,
                character_id: current_character_id,
                ..Default::default()
            },
            50,
        )
        .expect("terminated lifecycle should persist");
        let persisted_before =
            crate::persistence::load_player_cultivation_bundle(&settings, username)
                .expect("live bundle should load")
                .expect("live bundle should exist");

        let mut app = reincarnation_hydration_app(&settings, &player_persistence);
        let (mut client_bundle, _helper) = create_mock_client(username);
        client_bundle.player.position = Position::new([0.0, 66.0, 0.0]);
        let entity = app
            .world_mut()
            .spawn((client_bundle, CurrentDimension(DimensionKind::Overworld)))
            .id();

        app.update();

        assert!(app.world().get::<Cultivation>(entity).is_none());
        assert!(app.world().get::<Lifecycle>(entity).is_none());
        assert!(app.world().get::<CultivationAttachRetry>(entity).is_some());
        assert_eq!(
            crate::player::state::load_current_character_id(&player_persistence, username)
                .expect("current identity should reload"),
            Some(current_raw_id),
            "a reverse termination conflict must preserve the current identity"
        );
        assert_eq!(
            crate::persistence::load_player_cultivation_bundle(&settings, username)
                .expect("conflicting bundle should remain readable")
                .expect("conflicting bundle should remain durable"),
            persisted_before
        );
        assert_eq!(
            app.world()
                .resource::<WorldQiAccount>()
                .balance(&pending_inflow_account()),
            0.0
        );
        assert_eq!(app.world().resource::<Events<QiTransfer>>().len(), 0);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn join_hydration_rejects_current_lifecycle_alive_life_record_terminated_conflict() {
        let (settings, root) = temp_persistence_settings("reincarnate-forward-conflict");
        let player_persistence = player_state_persistence_for(&settings, &root);
        let username = "ForwardTerminationConflict";
        let current_raw_id =
            crate::player::state::rotate_current_character_id(&player_persistence, username)
                .expect("seeding current_char_id should succeed");
        let current_character_id =
            crate::player::state::player_character_id(username, current_raw_id.as_str());
        let old_qi = crate::qi_physics::constants::QI_EPSILON / 2.0;
        seed_cultivation_bundle_with_tutorial_and_qi(
            &settings,
            username,
            Realm::Spirit,
            old_qi,
            &terminated_life_record(current_character_id.as_str()),
            None,
        );
        crate::player::state::save_player_lifecycle_slice(
            &player_persistence,
            username,
            &crate::combat::components::Lifecycle {
                state: crate::combat::components::LifecycleState::Alive,
                character_id: current_character_id,
                ..Default::default()
            },
            50,
        )
        .expect("alive lifecycle should persist");
        let persisted_before =
            crate::persistence::load_player_cultivation_bundle(&settings, username)
                .expect("terminated bundle should load")
                .expect("terminated bundle should exist");

        let mut app = reincarnation_hydration_app(&settings, &player_persistence);
        let (mut client_bundle, _helper) = create_mock_client(username);
        client_bundle.player.position = Position::new([0.0, 66.0, 0.0]);
        let entity = app
            .world_mut()
            .spawn((client_bundle, CurrentDimension(DimensionKind::Overworld)))
            .id();

        app.update();

        assert!(app.world().get::<Cultivation>(entity).is_none());
        assert!(app.world().get::<Lifecycle>(entity).is_none());
        assert!(app.world().get::<CultivationAttachRetry>(entity).is_some());
        assert_eq!(
            crate::player::state::load_current_character_id(&player_persistence, username)
                .expect("current identity should reload"),
            Some(current_raw_id),
            "a forward termination conflict must preserve the current identity"
        );
        assert_eq!(
            crate::persistence::load_player_cultivation_bundle(&settings, username)
                .expect("conflicting bundle should remain readable")
                .expect("conflicting bundle should remain durable"),
            persisted_before
        );
        assert_eq!(
            app.world()
                .resource::<WorldQiAccount>()
                .balance(&pending_inflow_account()),
            0.0
        );
        assert_eq!(app.world().resource::<Events<QiTransfer>>().len(), 0);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn join_reincarnation_malformed_life_record_uses_durable_lifecycle_and_preserves_qi() {
        let (settings, root) = temp_persistence_settings("reincarnate-life-record-retry");
        let player_persistence = player_state_persistence_for(&settings, &root);
        let username = "MalformedLifeRecordJoin";
        let old_raw_id =
            crate::player::state::rotate_current_character_id(&player_persistence, username)
                .expect("seeding current_char_id should succeed");
        let old_canonical_id =
            crate::player::state::player_character_id(username, old_raw_id.as_str());
        let old_qi = crate::qi_physics::constants::QI_EPSILON / 2.0;
        let terminated_record = terminated_life_record(old_canonical_id.as_str());
        seed_cultivation_bundle_with_tutorial_and_qi(
            &settings,
            username,
            Realm::Spirit,
            old_qi,
            &terminated_record,
            None,
        );
        crate::player::state::save_player_lifecycle_slice(
            &player_persistence,
            username,
            &crate::combat::components::Lifecycle {
                state: crate::combat::components::LifecycleState::Terminated,
                character_id: old_canonical_id.clone(),
                ..Default::default()
            },
            50,
        )
        .expect("terminated lifecycle should persist");
        let valid_bundle = crate::persistence::load_player_cultivation_bundle(&settings, username)
            .expect("seeded bundle should load")
            .expect("seeded bundle should exist");
        let mut malformed_bundle = valid_bundle.clone();
        malformed_bundle["life_record"] = serde_json::json!("unreadable");
        let connection = rusqlite::Connection::open(settings.db_path())
            .expect("cultivation database should open");
        connection
            .execute(
                "UPDATE player_cultivation SET cultivation_json = ?1 WHERE username = ?2",
                rusqlite::params![malformed_bundle.to_string(), username],
            )
            .expect("malformed life_record should be written");
        drop(connection);

        let mut app = App::new();
        insert_test_dimension_layers(&mut app);
        app.insert_resource(settings.clone());
        app.insert_resource(player_persistence.clone());
        let (default_loadout, item_registry, allocator) = inventory_test_resources();
        app.insert_resource(default_loadout);
        app.insert_resource(item_registry);
        app.insert_resource(allocator);
        let mut zones = ZoneRegistry::fallback();
        zones
            .find_zone_mut(crate::world::zone::DEFAULT_SPAWN_ZONE_NAME)
            .expect("fallback registry must include spawn")
            .spirit_qi = 1.0;
        app.insert_resource(zones);
        app.insert_resource(WorldQiAccount::default());
        app.add_event::<QiTransfer>();
        insert_reincarnation_cleanup_resources(&mut app);
        app.add_systems(
            Update,
            (
                attach_cultivation_to_joined_clients,
                crate::combat::attach_combat_bundle_to_joined_clients
                    .after(attach_cultivation_to_joined_clients),
            ),
        );

        let (mut client_bundle, _helper) = create_mock_client(username);
        client_bundle.player.position = Position::new([0.0, 66.0, 0.0]);
        let entity = app
            .world_mut()
            .spawn((client_bundle, CurrentDimension(DimensionKind::Overworld)))
            .id();

        app.update();

        assert!(app.world().get::<Cultivation>(entity).is_none());
        assert!(app.world().get::<Lifecycle>(entity).is_none());
        assert!(app.world().get::<CultivationAttachRetry>(entity).is_some());
        assert_eq!(
            crate::player::state::load_current_character_id(&player_persistence, username)
                .expect("current_char_id should reload"),
            Some(old_raw_id.clone()),
            "malformed life_record must not rotate the durable terminated identity"
        );
        assert_eq!(
            crate::persistence::load_player_cultivation_bundle(&settings, username)
                .expect("malformed bundle should remain readable as JSON")
                .expect("malformed bundle should remain durable"),
            malformed_bundle,
            "durable lifecycle classification must reject the malformed bundle without overwriting old qi"
        );
        assert_eq!(
            app.world()
                .resource::<WorldQiAccount>()
                .balance(&pending_inflow_account()),
            0.0
        );
        assert_eq!(app.world().resource::<Events<QiTransfer>>().len(), 0);

        let connection = rusqlite::Connection::open(settings.db_path())
            .expect("cultivation database should reopen");
        connection
            .execute(
                "UPDATE player_cultivation SET cultivation_json = ?1 WHERE username = ?2",
                rusqlite::params![valid_bundle.to_string(), username],
            )
            .expect("repairing life_record should succeed");
        drop(connection);

        app.update();

        assert_eq!(
            app.world().get::<Cultivation>(entity).unwrap().qi_current,
            0.0
        );
        assert!(app.world().get::<CultivationAttachRetry>(entity).is_none());
        assert!(app.world().get::<Lifecycle>(entity).is_some());
        assert!(
            crate::player::state::load_current_character_id(&player_persistence, username)
                .expect("fresh current_char_id should reload")
                .is_some_and(|id| id != old_raw_id)
        );
        assert_eq!(
            app.world()
                .resource::<WorldQiAccount>()
                .balance(&pending_inflow_account()),
            old_qi
        );
        assert_eq!(
            crate::persistence::load_pending_inflow_balance(&settings)
                .expect("pending inflow should reload"),
            old_qi
        );
        let emitted: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<QiTransfer>>()
            .drain()
            .collect();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].from, QiAccountId::player(old_canonical_id));
        assert_eq!(emitted[0].to, pending_inflow_account());
        assert_eq!(emitted[0].amount, old_qi);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn join_reincarnation_decode_failure_preserves_positive_qi_then_retries_atomically() {
        let (settings, root) = temp_persistence_settings("reincarnate-join-decode-retry");
        let player_persistence = player_state_persistence_for(&settings, &root);
        let username = "MalformedQiJoin";
        let old_raw_id =
            crate::player::state::rotate_current_character_id(&player_persistence, username)
                .expect("seeding current_char_id should succeed");
        let old_canonical_id =
            crate::player::state::player_character_id(username, old_raw_id.as_str());
        let old_qi = crate::qi_physics::constants::QI_EPSILON / 2.0;
        seed_cultivation_bundle_with_tutorial_and_qi(
            &settings,
            username,
            Realm::Spirit,
            old_qi,
            &terminated_life_record(old_canonical_id.as_str()),
            None,
        );
        let valid_bundle = crate::persistence::load_player_cultivation_bundle(&settings, username)
            .expect("seeded bundle should load")
            .expect("seeded bundle should exist");
        let mut malformed_bundle = valid_bundle.clone();
        malformed_bundle["meridians"] = serde_json::json!({ "regular": "unreadable" });
        let connection = rusqlite::Connection::open(settings.db_path())
            .expect("cultivation database should open");
        connection
            .execute(
                "UPDATE player_cultivation SET cultivation_json = ?1 WHERE username = ?2",
                rusqlite::params![malformed_bundle.to_string(), username],
            )
            .expect("malformed terminated bundle should be written");
        drop(connection);

        let mut app = App::new();
        insert_test_dimension_layers(&mut app);
        app.insert_resource(settings.clone());
        app.insert_resource(player_persistence.clone());
        let (default_loadout, item_registry, allocator) = inventory_test_resources();
        app.insert_resource(default_loadout);
        app.insert_resource(item_registry);
        app.insert_resource(allocator);
        let mut zones = ZoneRegistry::fallback();
        zones
            .find_zone_mut(crate::world::zone::DEFAULT_SPAWN_ZONE_NAME)
            .expect("fallback registry must include spawn")
            .spirit_qi = 1.0;
        app.insert_resource(zones);
        app.insert_resource(WorldQiAccount::default());
        app.add_event::<QiTransfer>();
        insert_reincarnation_cleanup_resources(&mut app);
        app.add_systems(
            Update,
            (
                attach_cultivation_to_joined_clients,
                crate::combat::attach_combat_bundle_to_joined_clients
                    .after(attach_cultivation_to_joined_clients),
            ),
        );

        let (mut client_bundle, _helper) = create_mock_client(username);
        client_bundle.player.position = Position::new([0.0, 66.0, 0.0]);
        let entity = app
            .world_mut()
            .spawn((client_bundle, CurrentDimension(DimensionKind::Overworld)))
            .id();

        app.update();

        assert!(
            app.world().get::<Cultivation>(entity).is_none(),
            "an unreadable terminated bundle must not publish default cultivation over positive old qi"
        );
        assert!(
            app.world().get::<Lifecycle>(entity).is_none(),
            "the production combat attach must not expose Terminated while old cultivation and qi are unreadable"
        );
        assert!(
            app.world()
                .get::<crate::combat::components::Wounds>(entity)
                .is_none(),
            "a rejected cultivation hydration must keep the entire combat bundle unpublished"
        );
        assert!(
            app.world().get::<CultivationAttachRetry>(entity).is_some(),
            "a rejected terminated hydration must stay explicitly retryable"
        );
        assert_eq!(
            crate::player::state::load_current_character_id(&player_persistence, username)
                .expect("current_char_id should reload"),
            Some(old_raw_id.clone()),
            "decode rejection must preserve the durable old identity"
        );
        assert_eq!(
            crate::persistence::load_player_cultivation_bundle(&settings, username)
                .expect("malformed bundle should remain readable as JSON")
                .expect("malformed bundle should remain durable"),
            malformed_bundle,
            "decode rejection must not overwrite any durable old-life slice"
        );
        assert_eq!(
            app.world()
                .resource::<WorldQiAccount>()
                .balance(&pending_inflow_account()),
            0.0,
            "decode rejection must not publish staged pending inflow"
        );
        assert_eq!(app.world().resource::<Events<QiTransfer>>().len(), 0);
        assert_eq!(
            crate::persistence::load_pending_inflow_balance(&settings)
                .expect("durable pending balance should remain readable"),
            0.0,
            "decode rejection must not persist any partial qi settlement"
        );

        let connection = rusqlite::Connection::open(settings.db_path())
            .expect("cultivation database should reopen");
        connection
            .execute(
                "UPDATE player_cultivation SET cultivation_json = ?1 WHERE username = ?2",
                rusqlite::params![valid_bundle.to_string(), username],
            )
            .expect("repairing the terminated bundle should succeed");
        drop(connection);

        app.update();

        let fresh_lifecycle = app
            .world()
            .get::<Lifecycle>(entity)
            .expect("the repaired retry must publish a fresh lifecycle");
        assert_ne!(fresh_lifecycle.character_id, old_canonical_id);
        assert_eq!(
            app.world().get::<Cultivation>(entity).unwrap().qi_current,
            0.0,
            "the fresh life must start after settling all old qi"
        );
        assert!(app.world().get::<CultivationAttachRetry>(entity).is_none());
        assert!(
            app.world()
                .get::<crate::combat::components::Wounds>(entity)
                .is_some(),
            "the repaired retry must publish the combat bundle after cultivation succeeds"
        );
        assert!(
            crate::player::state::load_current_character_id(&player_persistence, username)
                .expect("fresh current_char_id should reload")
                .is_some_and(|id| id != old_raw_id),
            "only the successful retry may rotate the durable identity"
        );
        assert_eq!(
            app.world()
                .resource::<WorldQiAccount>()
                .balance(&pending_inflow_account()),
            old_qi,
            "a full zone must retain even sub-epsilon old qi in pending inflow"
        );
        assert_eq!(
            crate::persistence::load_pending_inflow_balance(&settings)
                .expect("committed pending balance should reload"),
            old_qi,
            "the retry transaction must durably conserve sub-epsilon old qi"
        );
        let emitted: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<QiTransfer>>()
            .drain()
            .collect();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].from, QiAccountId::player(old_canonical_id));
        assert_eq!(emitted[0].to, pending_inflow_account());
        assert_eq!(emitted[0].amount, old_qi);

        let persisted_fresh =
            crate::persistence::load_player_cultivation_bundle(&settings, username)
                .expect("fresh bundle should reload")
                .expect("successful retry must persist a fresh bundle");
        assert_eq!(
            serde_json::from_value::<Cultivation>(persisted_fresh["cultivation"].clone())
                .expect("fresh cultivation should decode")
                .qi_current,
            0.0
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn production_join_reincarnation_replaces_old_tutorial_before_join_hydration() {
        let (settings, root) = temp_persistence_settings("reincarnate-join-tutorial-order");
        let player_persistence = player_state_persistence_for(&settings, &root);
        let username = "TutorialReborn";
        let old_raw_id =
            crate::player::state::rotate_current_character_id(&player_persistence, username)
                .expect("seeding current_char_id should succeed");
        let old_canonical_id =
            crate::player::state::player_character_id(username, old_raw_id.as_str());
        let mut old_tutorial = TutorialState::new(777);
        old_tutorial.trigger(TutorialHook::CoffinOpened);
        old_tutorial.completed_at_tick = Some(999);
        seed_cultivation_bundle_with_tutorial(
            &settings,
            username,
            Realm::Spirit,
            &terminated_life_record(&old_canonical_id),
            Some(&old_tutorial),
        );

        let mut app = App::new();
        insert_test_dimension_layers(&mut app);
        app.insert_resource(settings.clone());
        app.insert_resource(player_persistence);
        let (default_loadout, item_registry, allocator) = inventory_test_resources();
        app.insert_resource(default_loadout);
        app.insert_resource(item_registry);
        app.insert_resource(allocator);
        app.insert_resource(TutorialTelemetry::default());
        insert_reincarnation_cleanup_resources(&mut app);
        app.add_systems(
            Update,
            (
                attach_cultivation_to_joined_clients,
                crate::world::spawn_tutorial::attach_tutorial_state_to_joined_clients
                    .after(attach_cultivation_to_joined_clients),
            ),
        );

        let (client_bundle, _helper) = create_mock_client(username);
        let entity = app.world_mut().spawn(client_bundle).id();
        app.update();

        let tutorial = app
            .world()
            .get::<TutorialState>(entity)
            .expect("join reincarnation must attach a fresh tutorial state in the commit frame");
        assert_eq!(tutorial, &TutorialState::new(0));
        assert_ne!(tutorial, &old_tutorial);
        assert_eq!(
            app.world().resource::<TutorialTelemetry>().started,
            1,
            "join reincarnation must count exactly one fresh tutorial start before hydration skips the entity"
        );

        let persisted = crate::persistence::load_player_cultivation_bundle(&settings, username)
            .expect("reincarnated bundle should reload")
            .expect("reincarnated bundle must exist");
        assert_eq!(
            serde_json::from_value::<TutorialState>(persisted["tutorial_state"].clone())
                .expect("fresh tutorial state must persist"),
            TutorialState::new(0)
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn production_join_reincarnation_clears_tsy_and_coffin_runtime_after_commit() {
        let (settings, root) = temp_persistence_settings("reincarnate-join-runtime-cleanup");
        let player_persistence = player_state_persistence_for(&settings, &root);
        let username = "CoffinJoin";
        let old_raw_id =
            crate::player::state::rotate_current_character_id(&player_persistence, username)
                .expect("seeding current_char_id should succeed");
        let old_canonical_id =
            crate::player::state::player_character_id(username, old_raw_id.as_str());
        seed_cultivation_bundle(
            &settings,
            username,
            Realm::Spirit,
            &terminated_life_record(&old_canonical_id),
        );

        let old_lifespan = LifespanComponent {
            born_at_tick: 10,
            years_lived: LifespanCapTable::SPIRIT as f64,
            cap_by_realm: LifespanCapTable::SPIRIT,
            offline_pause_tick: None,
        };
        let coffin_position = [18.5, 73.05, -21.5];
        crate::player::state::save_player_slices_with_coffin(
            &player_persistence,
            username,
            &PlayerState::default(),
            coffin_position,
            DimensionKind::Tsy,
            None,
            Some(&old_lifespan),
            &crate::skill::components::SkillSet::default(),
            Some(CoffinGrade::Jade),
            None,
        )
        .expect("seeding TSY coffin slices should succeed");

        let mut app = App::new();
        let overworld_layer = app.world_mut().spawn_empty().id();
        let tsy_layer = app.world_mut().spawn_empty().id();
        app.insert_resource(settings.clone());
        app.insert_resource(player_persistence.clone());
        app.insert_resource(DimensionLayers {
            overworld: overworld_layer,
            tsy: tsy_layer,
        });
        let (default_loadout, item_registry, allocator) = inventory_test_resources();
        app.insert_resource(default_loadout);
        app.insert_resource(item_registry);
        app.insert_resource(allocator);
        app.insert_resource(CoffinRegistry::default());
        insert_reincarnation_cleanup_resources(&mut app);
        app.add_event::<CoffinStateChanged>();
        app.add_systems(
            Update,
            (
                crate::player::attach_player_state_to_joined_clients,
                crate::inventory::attach_inventory_to_joined_clients
                    .after(crate::player::attach_player_state_to_joined_clients),
                attach_cultivation_to_joined_clients
                    .after(crate::player::attach_player_state_to_joined_clients)
                    .after(crate::inventory::attach_inventory_to_joined_clients),
            ),
        );

        let (mut client_bundle, _helper) = create_mock_client(username);
        client_bundle.player.layer.0 = tsy_layer;
        client_bundle.visible_chunk_layer.0 = tsy_layer;
        client_bundle.visible_entity_layers.0.clear();
        client_bundle.visible_entity_layers.0.insert(tsy_layer);
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut()
            .entity_mut(entity)
            .remove::<EntityLayerId>()
            .remove::<VisibleChunkLayer>();
        assert!(app.world().get::<EntityLayerId>(entity).is_none());
        assert!(app.world().get::<VisibleChunkLayer>(entity).is_none());
        let coffin_lower = crate::coffin::coffin_lower_from_player_position(coffin_position);
        app.insert_resource(coffin_runtime_for_player(
            entity,
            coffin_lower,
            CoffinGrade::Jade,
        ));

        app.update();

        let world = app.world();
        let player = world.entity(entity);
        let position = player
            .get::<Position>()
            .expect("reincarnated player should retain Position")
            .get();
        assert_ne!(
            position,
            valence::prelude::DVec3::from_array(coffin_position),
            "successful join reincarnation must not leave the new body pinned at the old coffin"
        );
        assert_eq!(
            player.get::<CurrentDimension>().copied(),
            Some(CurrentDimension(DimensionKind::Overworld)),
            "successful join reincarnation must move runtime dimension to Overworld"
        );
        assert_eq!(
            player.get::<EntityLayerId>().unwrap().0,
            overworld_layer,
            "successful join reincarnation must move the entity replication layer to Overworld"
        );
        assert_eq!(
            player.get::<VisibleChunkLayer>().unwrap().0,
            overworld_layer,
            "successful join reincarnation must move visible chunks to Overworld"
        );
        let visible_entities = &player.get::<VisibleEntityLayers>().unwrap().0;
        assert!(visible_entities.contains(&overworld_layer));
        assert!(!visible_entities.contains(&tsy_layer));
        assert!(
            !player
                .get::<valence::entity::entity::Flags>()
                .expect("mock client should carry entity flags")
                .invisible(),
            "successful join reincarnation must make the new body visible after leaving the old coffin"
        );
        assert!(
            player.get::<CoffinComponent>().is_none(),
            "successful join reincarnation must remove the inherited CoffinComponent"
        );
        {
            let registry = app.world().resource::<CoffinRegistry>();
            assert!(!registry.player_in_coffin.contains_key(&entity));
            assert_eq!(
                registry
                    .lookup(coffin_lower)
                    .and_then(|coffin| coffin.occupied_by),
                None,
                "successful join reincarnation must release the old coffin occupancy"
            );
        }
        let state_events: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<CoffinStateChanged>>()
            .drain()
            .collect();
        assert_eq!(state_events.len(), 1);
        assert_eq!(state_events[0].player, entity);
        assert_eq!(state_events[0].grade, None);

        let persisted = crate::player::state::load_player_slices(&player_persistence, username);
        assert_eq!(persisted.last_dimension, DimensionKind::Overworld);
        assert!(!persisted.in_coffin);
        assert_eq!(persisted.coffin_grade, None);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn join_reincarnation_without_dimension_layers_rejects_before_commit() {
        let (settings, root) = temp_persistence_settings("reincarnate-join-no-layers");
        let player_persistence = player_state_persistence_for(&settings, &root);
        let username = "LayerlessJoin";
        let old_raw_id =
            crate::player::state::rotate_current_character_id(&player_persistence, username)
                .expect("seeding current_char_id should succeed");
        let old_canonical_id =
            crate::player::state::player_character_id(username, old_raw_id.as_str());
        seed_cultivation_bundle(
            &settings,
            username,
            Realm::Spirit,
            &terminated_life_record(&old_canonical_id),
        );

        let mut app = App::new();
        app.insert_resource(settings.clone());
        app.insert_resource(player_persistence.clone());
        let (default_loadout, item_registry, allocator) = inventory_test_resources();
        app.insert_resource(default_loadout);
        app.insert_resource(item_registry);
        app.insert_resource(allocator);
        app.add_systems(Update, attach_cultivation_to_joined_clients);
        let (client_bundle, _helper) = create_mock_client(username);
        let entity = app.world_mut().spawn(client_bundle).id();

        app.update();

        assert_eq!(
            crate::player::state::load_current_character_id(&player_persistence, username)
                .expect("current_char_id should reload"),
            Some(old_raw_id),
            "missing DimensionLayers must reject before rotating the durable identity"
        );
        let life_record = app
            .world()
            .get::<LifeRecord>(entity)
            .expect("the terminated record should remain hydrated");
        assert!(matches!(
            life_record.biography.last(),
            Some(BiographyEntry::Terminated { .. })
        ));
        assert!(
            app.world()
                .get::<crate::inventory::PlayerInventory>(entity)
                .is_none(),
            "missing DimensionLayers must reject before publishing fresh inventory runtime"
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn join_reincarnation_precommit_failure_rolls_back_then_retries_next_tick() {
        let (settings, root) = temp_persistence_settings("reincarnate-join-precommit");
        let player_persistence = player_state_persistence_for(&settings, &root);
        let username = "AtomicJoin";
        let old_raw_id =
            crate::player::state::rotate_current_character_id(&player_persistence, username)
                .expect("seeding current_char_id should succeed");
        let old_canonical_id =
            crate::player::state::player_character_id(username, old_raw_id.as_str());
        let terminated_record = terminated_life_record(&old_canonical_id);
        let persisted_nourishment = Nourishment {
            satiety: 17.0,
            hydration: 29.0,
        };
        let old_tutorial = TutorialState::new(600);
        crate::persistence::persist_player_cultivation_bundle_with_nourishment(
            &settings,
            username,
            &Cultivation {
                realm: Realm::Spirit,
                ..Default::default()
            },
            &MeridianSystem::default(),
            &QiColor::default(),
            &Karma::default(),
            &Contamination::default(),
            &terminated_record,
            &PracticeLog::default(),
            &InsightQuota::default(),
            &UnlockedPerceptions::default(),
            &InsightModifiers::new(),
            Some(&old_tutorial),
            &MeridianSeveredPermanent::default(),
            None,
            None,
            Some(&persisted_nourishment),
        )
        .expect("seeding terminated bundle should succeed");
        let persisted_bundle_before =
            crate::persistence::load_player_cultivation_bundle(&settings, username)
                .expect("seeded bundle should load")
                .expect("seeded bundle should exist");

        let old_state = PlayerState {
            karma: 0.35,
            inventory_score: 0.65,
        };
        let old_position = [18.0, 73.0, -22.0];
        let old_lifespan = LifespanComponent {
            born_at_tick: 10,
            years_lived: LifespanCapTable::SPIRIT as f64,
            cap_by_realm: LifespanCapTable::SPIRIT,
            offline_pause_tick: None,
        };
        crate::player::state::save_player_slices_with_coffin(
            &player_persistence,
            username,
            &old_state,
            old_position,
            DimensionKind::Tsy,
            None,
            Some(&old_lifespan),
            &crate::skill::components::SkillSet::default(),
            Some(CoffinGrade::Jade),
            None,
        )
        .expect("seeding prior player slices should succeed");
        let old_craft_session = persisted_test_craft_session(username);
        crate::player::state::save_player_inventory_and_craft_session_slices(
            &player_persistence,
            username,
            None,
            Some(&old_craft_session),
        )
        .expect("seeding prior craft session should succeed");
        let shrine_before = [4.0, 66.0, 9.0];
        crate::player::state::save_player_shrine_anchor_slice(
            &player_persistence,
            username,
            Some(shrine_before),
        )
        .expect("seeding shrine should succeed");

        let mut app = App::new();
        insert_test_dimension_layers(&mut app);
        app.insert_resource(settings.clone());
        app.insert_resource(player_persistence.clone());
        let (default_loadout, item_registry, allocator) = inventory_test_resources();
        app.insert_resource(default_loadout);
        app.insert_resource(item_registry);
        app.insert_resource(allocator);
        app.insert_resource(crate::player::gameplay::PendingGameplayNarrations::default());
        app.insert_resource(CoffinRegistry::default());
        app.insert_resource(TutorialTelemetry::default());
        insert_reincarnation_cleanup_resources(&mut app);
        app.add_event::<CoffinStateChanged>();
        app.add_systems(Update, attach_cultivation_to_joined_clients);

        let (mut client_bundle, _helper) = create_mock_client(username);
        let coffin_lower = crate::coffin::coffin_lower_from_player_position(old_position);
        client_bundle.player.position = Position::new(old_position);
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                old_state.clone(),
                old_lifespan.clone(),
                old_tutorial.clone(),
                CurrentDimension(DimensionKind::Tsy),
                CoffinComponent {
                    entered_at_tick: 20,
                    coffin_lower,
                    grade: CoffinGrade::Jade,
                },
                old_craft_session.clone(),
                crate::network::craft_emit::CraftSessionPersistenceDirty,
                crate::network::craft_emit::CraftSessionStateDirty,
            ))
            .id();
        app.insert_resource(coffin_runtime_for_player(
            entity,
            coffin_lower,
            CoffinGrade::Jade,
        ));
        app.world_mut()
            .resource_mut::<PendingJueBiTriggers>()
            .schedule_for_character(
                &JueBiTriggerEvent {
                    entity,
                    character_id: Some(old_canonical_id.clone()),
                    source: JueBiTriggerSource::VoidQuotaExceeded,
                    triggered_at_tick: 10,
                    delay_ticks: 200,
                    epicenter: None,
                },
                old_canonical_id.clone(),
            );
        app.world_mut()
            .resource_mut::<HalfStepRechallengeQueue>()
            .enqueue(crate::cultivation::tribulation::HalfStepRechallengeEntry {
                char_id: old_canonical_id.clone(),
                entity,
                entered_at: 10,
                rechallenge_window_until: 10_000,
                is_dormant: false,
                buff_applied: true,
            });
        let allocator_before = format!(
            "{:?}",
            app.world()
                .resource::<crate::inventory::InventoryInstanceIdAllocator>()
        );
        let _failpoint = crate::persistence::arm_fail_before_commit(settings.db_path());

        app.update();

        assert!(
            app.world().get::<Cultivation>(entity).is_none(),
            "failed join reincarnation must not attach a staged fresh cultivation state"
        );
        assert_eq!(
            app.world().get::<TutorialState>(entity),
            Some(&old_tutorial),
            "failed join reincarnation must preserve the prior tutorial state"
        );
        assert_eq!(
            app.world().resource::<TutorialTelemetry>().started,
            0,
            "failed join reincarnation must not count a tutorial that never committed"
        );
        assert!(
            app.world()
                .get::<crate::inventory::PlayerInventory>(entity)
                .is_none(),
            "failed join reincarnation must not attach a staged fresh inventory"
        );
        assert_eq!(
            *app.world().get::<PlayerState>(entity).unwrap(),
            old_state,
            "failed join reincarnation must preserve the restored player state"
        );
        assert_eq!(
            app.world().get::<crate::craft::CraftSession>(entity),
            Some(&old_craft_session),
            "failed join reincarnation must preserve the terminated character's active craft session"
        );
        assert!(
            app.world()
                .get::<crate::network::craft_emit::CraftSessionPersistenceDirty>(entity)
                .is_some(),
            "failed join reincarnation must preserve pending craft persistence work"
        );
        assert!(
            !app.world().resource::<PendingJueBiTriggers>().is_empty(),
            "failed join reincarnation must preserve delayed JueBi work for the unchanged character"
        );
        assert_eq!(
            app.world().resource::<HalfStepRechallengeQueue>().len(),
            1,
            "failed join reincarnation must preserve the unchanged character's HalfStep FIFO entry"
        );
        assert_eq!(
            app.world().get::<LifespanComponent>(entity).unwrap(),
            &old_lifespan,
            "failed join reincarnation must preserve the exhausted prior lifespan"
        );
        assert_eq!(
            app.world().get::<CurrentDimension>(entity).copied(),
            Some(CurrentDimension(DimensionKind::Tsy)),
            "failed join reincarnation must preserve the old runtime dimension"
        );
        assert_eq!(
            app.world().get::<Position>(entity).unwrap().get(),
            valence::prelude::DVec3::from_array(old_position),
            "failed join reincarnation must preserve the old runtime position"
        );
        assert_eq!(
            app.world().get::<CoffinComponent>(entity),
            Some(&CoffinComponent {
                entered_at_tick: 20,
                coffin_lower,
                grade: CoffinGrade::Jade,
            }),
            "failed join reincarnation must preserve the inherited CoffinComponent"
        );
        {
            let registry = app.world().resource::<CoffinRegistry>();
            assert_eq!(registry.player_in_coffin.get(&entity), Some(&coffin_lower));
            assert_eq!(
                registry
                    .lookup(coffin_lower)
                    .and_then(|coffin| coffin.occupied_by),
                Some(entity),
                "failed join reincarnation must preserve old coffin occupancy"
            );
        }
        assert!(
            app.world_mut()
                .resource_mut::<Events<CoffinStateChanged>>()
                .drain()
                .next()
                .is_none(),
            "failed join reincarnation must not emit a coffin leave event"
        );
        assert_eq!(
            format!(
                "{:?}",
                app.world()
                    .resource::<crate::inventory::InventoryInstanceIdAllocator>()
            ),
            allocator_before,
            "failed join reincarnation must not consume inventory instance ids"
        );
        assert!(
            app.world_mut()
                .resource_mut::<crate::player::gameplay::PendingGameplayNarrations>()
                .drain()
                .is_empty(),
            "failed join reincarnation must not announce a new life that did not commit"
        );

        assert_eq!(
            crate::player::state::load_current_character_id(&player_persistence, username)
                .expect("current_char_id should reload")
                .expect("prior current_char_id should remain"),
            old_raw_id,
            "failed join reincarnation must roll back player_core.current_char_id"
        );
        let persisted_player_after =
            crate::player::state::load_player_slices(&player_persistence, username);
        assert_eq!(persisted_player_after.state, old_state);
        assert_eq!(
            persisted_player_after.craft_session.as_ref(),
            Some(&old_craft_session),
            "failed join reincarnation must roll back durable craft-session deletion"
        );
        assert_eq!(persisted_player_after.position, old_position);
        assert_eq!(persisted_player_after.last_dimension, DimensionKind::Tsy);
        assert!(persisted_player_after.in_coffin);
        assert_eq!(persisted_player_after.coffin_grade, Some(CoffinGrade::Jade));
        assert_eq!(
            persisted_player_after
                .lifespan
                .expect("prior lifespan should remain"),
            old_lifespan
        );
        assert_eq!(
            crate::player::state::load_player_shrine_anchor_slice(&player_persistence, username)
                .expect("shrine should reload"),
            Some(shrine_before),
            "failed join reincarnation must roll back shrine clearing"
        );
        assert_eq!(
            crate::persistence::load_player_cultivation_bundle(&settings, username)
                .expect("cultivation bundle should reload")
                .expect("terminated bundle should remain"),
            persisted_bundle_before,
            "failed join reincarnation must roll back cultivation and nourishment replacement"
        );
        let connection = rusqlite::Connection::open(settings.db_path())
            .expect("sqlite should reopen after rollback");
        let life_record_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM life_records", [], |row| row.get(0))
            .expect("life_records count should query");
        assert_eq!(
            life_record_count, 0,
            "failed join reincarnation must not leak a fresh life_records row"
        );
        drop(connection);

        // The failpoint is one-shot. The same online entity must be admitted again through the
        // explicit retry marker, without requiring a new `Added<Client>`/`Added<CurrentDimension>`
        // edge; unrelated later `Without<Cultivation>` transitions must remain excluded.
        app.update();

        assert!(
            app.world().get::<Cultivation>(entity).is_some(),
            "the next tick must retry and attach cultivation after the transient commit failure"
        );
        let fresh_lifecycle = app
            .world()
            .get::<crate::combat::components::Lifecycle>(entity)
            .expect("successful retry must publish the fresh lifecycle");
        assert_eq!(
            fresh_lifecycle.state,
            crate::combat::components::LifecycleState::Alive
        );
        assert_ne!(
            fresh_lifecycle.character_id, old_canonical_id,
            "successful retry must rotate away from the terminated identity"
        );
        assert_eq!(
            app.world().resource::<TutorialTelemetry>().started,
            1,
            "only the committed retry may count the fresh tutorial"
        );
        assert!(
            app.world().get::<CultivationAttachRetry>(entity).is_none(),
            "successful retry must clear its one-shot hydration admission marker"
        );
        assert!(
            app.world().get::<CoffinComponent>(entity).is_none(),
            "successful retry must clear the inherited coffin runtime"
        );
        assert!(app.world().resource::<PendingJueBiTriggers>().is_empty());
        assert_eq!(app.world().resource::<HalfStepRechallengeQueue>().len(), 0);
        assert_ne!(
            format!(
                "{:?}",
                app.world()
                    .resource::<crate::inventory::InventoryInstanceIdAllocator>()
            ),
            allocator_before,
            "only the committed retry may consume fresh inventory instance ids"
        );
        assert_ne!(
            crate::player::state::load_current_character_id(&player_persistence, username)
                .expect("fresh current_char_id should reload")
                .expect("successful retry must persist a current_char_id"),
            old_raw_id,
            "successful retry must publish the rotated durable identity"
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn existing_client_without_cultivation_or_retry_marker_is_not_rehydrated() {
        let (settings, root) = temp_persistence_settings("attach-retry-admission");
        seed_cultivation_bundle(
            &settings,
            "TerminatedOnline",
            Realm::Spirit,
            &terminated_life_record("offline:TerminatedOnline"),
        );

        let mut app = App::new();
        app.insert_resource(settings);
        app.add_systems(Update, attach_cultivation_to_joined_clients);
        let (client_bundle, _helper) = create_mock_client("TerminatedOnline");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.update();
        assert!(app.world().get::<Cultivation>(entity).is_some());

        app.world_mut().entity_mut(entity).remove::<Cultivation>();
        app.update();

        assert!(
            app.world().get::<Cultivation>(entity).is_none(),
            "a deliberate later removal must not be mistaken for a join or transient hydration retry"
        );
        assert!(app.world().get::<CultivationAttachRetry>(entity).is_none());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn join_after_reincarnation_does_not_rotate_again() {
        let (settings, root) = temp_persistence_settings("reincarnate-idempotent");
        let player_persistence = player_state_persistence_for(&settings, &root);

        let old_raw_id =
            crate::player::state::rotate_current_character_id(&player_persistence, "Azure")
                .expect("seed rotate should succeed");
        let old_canonical_id = crate::player::state::player_character_id("Azure", &old_raw_id);
        seed_cultivation_bundle(
            &settings,
            "Azure",
            Realm::Awaken,
            &terminated_life_record(&old_canonical_id),
        );

        // 第一次 join：应触发转世。
        let mut app1 = App::new();
        insert_test_dimension_layers(&mut app1);
        app1.insert_resource(settings.clone());
        app1.insert_resource(player_persistence.clone());
        {
            let (default_loadout, item_registry, allocator) = inventory_test_resources();
            app1.insert_resource(default_loadout);
            app1.insert_resource(item_registry);
            app1.insert_resource(allocator);
        }
        insert_reincarnation_cleanup_resources(&mut app1);
        app1.add_systems(Update, attach_cultivation_to_joined_clients);
        let (client_bundle, _helper) = create_mock_client("Azure");
        app1.world_mut().spawn(client_bundle);
        app1.update();

        let raw_id_after_first_join =
            crate::player::state::load_current_character_id(&player_persistence, "Azure")
                .expect("load should succeed")
                .expect("current_char_id should exist");
        assert_ne!(
            raw_id_after_first_join, old_raw_id,
            "第一次 join 应完成一次轮换"
        );

        // 第二次 join（模拟重连）：全新 App / 全新 entity，复用同一份持久化文件。
        let mut app2 = App::new();
        app2.insert_resource(settings.clone());
        app2.insert_resource(player_persistence.clone());
        {
            let (default_loadout, item_registry, allocator) = inventory_test_resources();
            app2.insert_resource(default_loadout);
            app2.insert_resource(item_registry);
            app2.insert_resource(allocator);
        }
        app2.add_systems(Update, attach_cultivation_to_joined_clients);
        let (client_bundle2, _helper2) = create_mock_client("Azure");
        let entity2 = app2.world_mut().spawn(client_bundle2).id();
        app2.update();

        let raw_id_after_second_join =
            crate::player::state::load_current_character_id(&player_persistence, "Azure")
                .expect("load should succeed")
                .expect("current_char_id should exist");
        assert_eq!(
            raw_id_after_second_join, raw_id_after_first_join,
            "已经转世过的角色再次 join 不应再次轮换 current_char_id（幂等）"
        );

        let life_record2 = app2
            .world()
            .get::<LifeRecord>(entity2)
            .expect("second join should still attach a LifeRecord");
        assert!(
            life_record2.biography.is_empty(),
            "第二次 join 读到的应仍是转世后的空白生平，不应再被判定终结"
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn join_with_non_terminated_character_is_unaffected_by_reincarnation_gate() {
        let (settings, root) = temp_persistence_settings("reincarnate-not-terminated");
        let player_persistence = player_state_persistence_for(&settings, &root);

        let raw_id =
            crate::player::state::rotate_current_character_id(&player_persistence, "Azure")
                .expect("seed rotate should succeed");
        let canonical_id = crate::player::state::player_character_id("Azure", &raw_id);

        let mut life_record = LifeRecord::new(canonical_id.clone());
        // 关键：只有 NearDeath，没有 Terminated —— 角色仍然"活着"。
        life_record.push(BiographyEntry::NearDeath {
            cause: "close_call".to_string(),
            tick: 10,
        });
        seed_cultivation_bundle(&settings, "Azure", Realm::Spirit, &life_record);

        let mut app = App::new();
        insert_test_dimension_layers(&mut app);
        app.insert_resource(settings.clone());
        app.insert_resource(player_persistence.clone());
        let (default_loadout, item_registry, allocator) = inventory_test_resources();
        app.insert_resource(default_loadout);
        app.insert_resource(item_registry);
        app.insert_resource(allocator);
        app.add_systems(Update, attach_cultivation_to_joined_clients);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.update();

        let cultivation = app
            .world()
            .get::<Cultivation>(entity)
            .expect("client should have Cultivation");
        assert_eq!(
            cultivation.realm,
            Realm::Spirit,
            "未终结角色 join 不应触发转世门，境界应保持持久化的原值"
        );

        let life_record_after = app
            .world()
            .get::<LifeRecord>(entity)
            .expect("client should have LifeRecord");
        assert_eq!(
            life_record_after.character_id, canonical_id,
            "未终结角色的 character_id 不应被轮换"
        );
        assert_eq!(
            life_record_after.biography.len(),
            1,
            "未终结角色的生平卷应原样水合，不应被清空"
        );

        let new_raw_id =
            crate::player::state::load_current_character_id(&player_persistence, "Azure")
                .expect("load should succeed")
                .expect("current_char_id should exist");
        assert_eq!(
            new_raw_id, raw_id,
            "未终结角色 join 不应触碰 player_core.current_char_id"
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn join_with_terminated_character_and_no_player_persistence_skips_gate_gracefully() {
        let (settings, root) = temp_persistence_settings("reincarnate-no-persistence");

        seed_cultivation_bundle(
            &settings,
            "Ghost",
            Realm::Awaken,
            &terminated_life_record("offline:Ghost"),
        );

        let mut app = App::new();
        app.insert_resource(settings.clone());
        // 故意不插入 PlayerStatePersistence —— 复现"无 persistence 资源"场景
        // （轮换需要写 player_core，没有这个资源就没法安全轮换）。
        app.add_systems(Update, attach_cultivation_to_joined_clients);

        let (client_bundle, _helper) = create_mock_client("Ghost");
        let entity = app.world_mut().spawn(client_bundle).id();

        // 不应 panic：既没有 PlayerStatePersistence 可写，也不能就地丢弃玩家数据。
        app.update();

        let life_record = app
            .world()
            .get::<LifeRecord>(entity)
            .expect("client should still receive a LifeRecord even without PlayerStatePersistence");
        assert!(
            matches!(
                life_record.biography.last(),
                Some(BiographyEntry::Terminated { .. })
            ),
            "无 PlayerStatePersistence 时应优雅跳过转世门（沿用旧记录），而不是 panic 或静默丢数据"
        );

        let _ = std::fs::remove_dir_all(root);
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
                    spawn_anchor_damaged: false,
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

    // ─── plan-race-system-v1 P0：持久化 RaceId 拒载执行点 ──────────────────────

    /// 只含 "human" 一条种族的最小 `RaceRegistry` 测试夹具——`body_plan_id` 复用真实
    /// `humanoid_plan_static()` 的克隆，不另起一份几何数据（保持与生产 registry
    /// bit-for-bit 一致，只是脱离磁盘 glob 加载）。
    fn race_registry_with_only_human() -> RaceRegistry {
        use crate::body_plan::race_registry::RaceEntry;

        let body_plans = crate::body_plan::BodyPlanRegistry::from_plans(vec![
            crate::body_plan::humanoid_plan_static().clone(),
        ])
        .expect("humanoid plan clone must validate as a standalone registry");
        RaceRegistry::from_parts_for_test(
            vec![RaceEntry {
                id: RaceId::new(crate::body_plan::HUMAN_RACE_ID),
                display_name: "人族".to_string(),
                body_plan_id: crate::body_plan::HUMANOID_BODY_PLAN_ID.into(),
                beast_kinds: vec![],
            }],
            vec![],
            &body_plans,
        )
        .expect("human-only race registry fixture must validate")
    }

    #[allow(clippy::too_many_arguments)]
    fn seed_cultivation_bundle_with_race(
        settings: &PersistenceSettings,
        username: &str,
        realm: Realm,
        race: RaceId,
        life_record: &LifeRecord,
    ) {
        crate::persistence::persist_player_cultivation_bundle(
            settings,
            username,
            &Cultivation {
                realm,
                race,
                ..Default::default()
            },
            &MeridianSystem::default(),
            &QiColor::default(),
            &Karma::default(),
            &Contamination::default(),
            life_record,
            &PracticeLog::default(),
            &InsightQuota::default(),
            &UnlockedPerceptions::default(),
            &InsightModifiers::new(),
            None,
            &MeridianSeveredPermanent::default(),
            None,
            None,
        )
        .expect("seeding cultivation bundle with a custom race should succeed");
    }

    fn seed_raw_cultivation_bundle(
        settings: &PersistenceSettings,
        username: &str,
        bundle: serde_json::Value,
    ) {
        let cultivation_json =
            serde_json::to_string(&bundle).expect("hand-built cultivation bundle must serialize");
        let connection = rusqlite::Connection::open(settings.db_path())
            .expect("open sqlite connection to seed a raw cultivation bundle");
        connection
            .execute(
                "
                INSERT INTO player_cultivation (
                    username,
                    cultivation_json,
                    schema_version,
                    last_updated_wall
                ) VALUES (?1, ?2, 1, 0)
                ON CONFLICT(username) DO UPDATE SET
                    cultivation_json = excluded.cultivation_json,
                    schema_version = excluded.schema_version,
                    last_updated_wall = excluded.last_updated_wall
                ",
                rusqlite::params![username, cultivation_json],
            )
            .expect("insert raw cultivation bundle");
    }

    #[test]
    fn joined_clients_hydrate_nourishment_and_discard_legacy_activity_json() {
        struct Case {
            username: &'static str,
            nourishment: Option<serde_json::Value>,
            legacy_activity: Option<serde_json::Value>,
            expected_nourishment: Nourishment,
        }

        let cases = [
            Case {
                username: "NourishMissing",
                nourishment: None,
                legacy_activity: None,
                expected_nourishment: Nourishment::spawn_default(),
            },
            Case {
                username: "NourishAxisMissing",
                nourishment: Some(serde_json::json!({"satiety": 31.0})),
                legacy_activity: Some(serde_json::json!({
                    "idle_ticks": 40,
                    "move_ticks": 50,
                    "dash_ticks": 60
                })),
                expected_nourishment: Nourishment {
                    satiety: 31.0,
                    hydration: Nourishment::spawn_default().hydration,
                },
            },
            Case {
                username: "NourishBadTypes",
                nourishment: Some(serde_json::json!({
                    "satiety": "bad",
                    "hydration": 44.0
                })),
                legacy_activity: Some(serde_json::json!({
                    "idle_ticks": "bad",
                    "move_ticks": 61,
                    "dash_ticks": false
                })),
                expected_nourishment: Nourishment {
                    satiety: Nourishment::spawn_default().satiety,
                    hydration: 44.0,
                },
            },
            Case {
                username: "NourishClamped",
                nourishment: Some(serde_json::json!({
                    "satiety": -9.0,
                    "hydration": 999.0
                })),
                legacy_activity: Some(serde_json::json!({
                    "idle_ticks": u32::MAX,
                    "move_ticks": u32::MAX,
                    "dash_ticks": u32::MAX
                })),
                expected_nourishment: Nourishment {
                    satiety: crate::nourishment::NOURISH_MIN_VALUE,
                    hydration: crate::nourishment::NOURISH_MAX_VALUE,
                },
            },
        ];

        let (settings, root) = temp_persistence_settings("nourishment-hydration-table");
        let mut app = App::new();
        app.insert_resource(settings.clone());
        app.add_systems(Update, attach_cultivation_to_joined_clients);

        let mut entities = Vec::new();
        let mut client_helpers = Vec::new();
        for case in &cases {
            let mut bundle = serde_json::Map::new();
            if let Some(nourishment) = &case.nourishment {
                bundle.insert("nourishment".to_string(), nourishment.clone());
            }
            if let Some(legacy_activity) = &case.legacy_activity {
                bundle.insert(
                    "nourishment_activity_window".to_string(),
                    legacy_activity.clone(),
                );
            }
            seed_raw_cultivation_bundle(
                &settings,
                case.username,
                serde_json::Value::Object(bundle),
            );

            let (client_bundle, helper) = create_mock_client(case.username);
            entities.push(app.world_mut().spawn(client_bundle).id());
            client_helpers.push(helper);
        }

        app.update();

        for (case, entity) in cases.iter().zip(entities) {
            assert_eq!(
                *app.world()
                    .get::<Nourishment>(entity)
                    .expect("joined client should receive nourishment"),
                case.expected_nourishment,
                "{} should hydrate each nourishment axis independently",
                case.username
            );
            assert_eq!(
                *app.world()
                    .get::<crate::nourishment::tick::NourishmentActivityWindow>(entity)
                    .expect("joined client should receive a fresh activity window"),
                crate::nourishment::tick::NourishmentActivityWindow::default(),
                "{} must discard any legacy persisted activity and begin a session-local window",
                case.username
            );
        }

        let _ = std::fs::remove_dir_all(root);
    }

    /// 手写 SQL 插入模拟"race 字段加入前"的旧存档形状：`persist_player_cultivation_bundle`
    /// 恒序列化完整 `Cultivation`（`race` 字段总在场），无法产出缺 race 字段的 bundle，
    /// 所以这里绕开它直接拼一份不含 "race" key 的 JSON 落库。
    fn seed_raw_cultivation_bundle_without_race_field(
        settings: &PersistenceSettings,
        username: &str,
        realm: Realm,
    ) {
        let mut cultivation_value = serde_json::to_value(Cultivation {
            realm,
            ..Default::default()
        })
        .expect("Cultivation must serialize to JSON");
        cultivation_value
            .as_object_mut()
            .expect("Cultivation serializes to a JSON object")
            .remove("race");

        let bundle = serde_json::json!({
            "v": 1,
            "cultivation": cultivation_value,
            // plan-race-system-v1 P1a：本 fixture 显式标 "v":1（race 字段加入前的旧
            // 存档），`meridians` 必须同样是真实 v1 legacy 形状（`MeridianId`
            // PascalCase 枚举名 channel id）而不是 `MeridianSystem::default()`（那是
            // *当前* snake_case 形态）——否则会静默触发本模块的 legacy 迁移分支解析
            // 失败又静默 fallback 回默认值，恰好凑出同一个值掩盖了问题，而不是真的
            // 走通迁移路径。
            "meridians": legacy_meridian_bundle::v1_all_closed_meridian_system_sample(),
            "qi_color": QiColor::default(),
            "karma": Karma::default(),
            "contamination": Contamination::default(),
            "life_record": LifeRecord::new(canonical_player_id(username)),
            "practice_log": PracticeLog::default(),
            "insight_quota": InsightQuota::default(),
            "unlocked_perceptions": UnlockedPerceptions::default(),
            "insight_modifiers": InsightModifiers::new(),
        });
        let cultivation_json =
            serde_json::to_string(&bundle).expect("hand-built legacy bundle must serialize");

        let connection = rusqlite::Connection::open(settings.db_path())
            .expect("open sqlite connection to seed a raw legacy bundle");
        connection
            .execute(
                "
                INSERT INTO player_cultivation (
                    username,
                    cultivation_json,
                    schema_version,
                    last_updated_wall
                ) VALUES (?1, ?2, 1, 0)
                ON CONFLICT(username) DO UPDATE SET
                    cultivation_json = excluded.cultivation_json,
                    schema_version = excluded.schema_version,
                    last_updated_wall = excluded.last_updated_wall
                ",
                rusqlite::params![username, cultivation_json],
            )
            .expect("insert raw legacy cultivation bundle");
    }

    #[test]
    fn joined_clients_reject_persisted_bundle_with_unknown_race() {
        let (settings, root) = temp_persistence_settings("reject-unknown-race");
        seed_cultivation_bundle_with_race(
            &settings,
            "Ghoul",
            Realm::Solidify,
            RaceId::new("nonexistent"),
            &LifeRecord::new(canonical_player_id("Ghoul")),
        );

        let mut app = App::new();
        app.insert_resource(settings);
        app.insert_resource(race_registry_with_only_human());
        app.add_systems(Update, attach_cultivation_to_joined_clients);

        let (client_bundle, _helper) = create_mock_client("Ghoul");
        let entity = app.world_mut().spawn(client_bundle).id();

        app.update();

        let cultivation = app
            .world()
            .get::<Cultivation>(entity)
            .expect("cultivation must still attach (reject into error state, not skip attach)");
        assert_eq!(
            cultivation.race,
            RaceId::new(crate::body_plan::HUMAN_RACE_ID),
            "an unknown persisted race must never end up on the live component — the safe \
             default fallback is the only acceptable outcome, not a silent pass-through of \
             `nonexistent`"
        );
        assert_eq!(
            cultivation.realm,
            Realm::Awaken,
            "unknown race must reject the *entire* persisted cultivation bundle (realm=Solidify \
             was persisted alongside it but must NOT survive) — proving this is bundle-level \
             corrupted-path handling, not a narrow 'only overwrite the race field' patch that \
             would silently keep the rest of an untrusted bundle"
        );

        let _ = std::fs::remove_dir_all(root);
    }

    /// plan-race-system-v1 bughunt major-2：未知 race 的 bundle 里 sibling slice 全部
    /// 塞进明显偏离默认值的数据（经脉全开/带毒/带污染/带洞察额度……），用来证明拒载
    /// 覆盖的不只是 `Cultivation` 一个组件——旧实现只回退 `cultivation`，其余 14 个
    /// slice 会原样水合，相当于"醒灵即刻全通经脉 + 满毒素抗性表"的白嫖突破后门。
    #[test]
    fn joined_clients_reject_persisted_bundle_with_unknown_race_resets_every_sibling_slice() {
        let (settings, root) =
            temp_persistence_settings("reject-unknown-race-full-bundle-rollback");
        let username = "ChimeraGhoul";

        let mut poisoned_meridians = MeridianSystem::default();
        for meridian in poisoned_meridians
            .regular
            .iter_mut()
            .chain(poisoned_meridians.extraordinary.iter_mut())
        {
            meridian.opened = true;
            meridian.open_progress = 1.0;
        }
        let poisoned_qi_color = QiColor {
            is_chaotic: true,
            is_hunyuan: true,
            ..Default::default()
        };
        let poisoned_karma = Karma { weight: 999.0 };
        let poisoned_contamination = Contamination {
            entries: vec![ContamSource {
                amount: 50.0,
                color: ColorKind::Sharp,
                meridian_id: None,
                attacker_id: Some("intruder".to_string()),
                introduced_at: 1,
            }],
        };
        let mut poisoned_practice_log = PracticeLog::default();
        poisoned_practice_log
            .weights
            .insert(ColorKind::Sharp, 12345.0);
        let mut poisoned_insight_quota = InsightQuota {
            used_this_realm: 9,
            ..Default::default()
        };
        poisoned_insight_quota
            .fired_triggers
            .push("stolen_trigger".to_string());
        let mut poisoned_unlocked_perceptions = UnlockedPerceptions::default();
        poisoned_unlocked_perceptions
            .set
            .insert("stolen_perception".to_string());
        let poisoned_insight_modifiers = InsightModifiers {
            qi_regen_mul: 7.0,
            ..InsightModifiers::new()
        };
        let poisoned_severed = MeridianSeveredPermanent::default();
        let poisoned_poison_toxicity = PoisonToxicity {
            level: 88.0,
            source_history: Vec::new(),
            last_dose_tick: 1,
            last_decay_tick: 0,
            ..Default::default()
        };
        let poisoned_digestion_load = DigestionLoad {
            current: 500.0,
            capacity: 1.0,
            decay_rate: 0.0,
            digest_lock_until_tick: None,
            ..Default::default()
        };
        let poisoned_nourishment = Nourishment {
            satiety: 3.0,
            hydration: 4.0,
        };

        crate::persistence::persist_player_cultivation_bundle_with_nourishment(
            &settings,
            username,
            &Cultivation {
                realm: Realm::Solidify,
                race: RaceId::new("nonexistent"),
                ..Default::default()
            },
            &poisoned_meridians,
            &poisoned_qi_color,
            &poisoned_karma,
            &poisoned_contamination,
            &LifeRecord::new(canonical_player_id(username)),
            &poisoned_practice_log,
            &poisoned_insight_quota,
            &poisoned_unlocked_perceptions,
            &poisoned_insight_modifiers,
            None,
            &poisoned_severed,
            Some(&poisoned_poison_toxicity),
            Some(&poisoned_digestion_load),
            Some(&poisoned_nourishment),
        )
        .expect("seeding a poisoned unknown-race bundle should succeed");

        let mut app = App::new();
        app.insert_resource(settings);
        app.insert_resource(race_registry_with_only_human());
        app.add_systems(Update, attach_cultivation_to_joined_clients);

        let (client_bundle, _helper) = create_mock_client(username);
        let entity = app.world_mut().spawn(client_bundle).id();

        app.update();

        let world = app.world();
        assert_eq!(
            world.get::<Cultivation>(entity).unwrap().race,
            RaceId::new(crate::body_plan::HUMAN_RACE_ID),
            "cultivation slice must reset"
        );
        let live_meridians = world
            .get::<MeridianSystem>(entity)
            .expect("meridians must still attach");
        assert!(
            live_meridians
                .regular
                .iter()
                .chain(live_meridians.extraordinary.iter())
                .all(|m| !m.opened),
            "meridians sibling slice must reset to the closed default, not inherit the \
             poisoned all-opened bundle from the rejected unknown-race save — a chimera that \
             never validated a race would otherwise wake up with every meridian already open"
        );
        let live_qi_color = world.get::<QiColor>(entity).unwrap();
        assert!(
            !live_qi_color.is_chaotic && !live_qi_color.is_hunyuan,
            "qi_color sibling slice must reset to default (is_chaotic=false, is_hunyuan=false), \
             not inherit the poisoned is_chaotic=true/is_hunyuan=true, 实测 {live_qi_color:?}"
        );
        assert_eq!(
            world.get::<Karma>(entity).unwrap().weight,
            0.0,
            "karma sibling slice must reset to default (weight=0.0), not the poisoned 999.0"
        );
        assert!(
            world
                .get::<Contamination>(entity)
                .unwrap()
                .entries
                .is_empty(),
            "contamination sibling slice must reset to empty, not inherit the poisoned entry"
        );
        assert!(
            world.get::<PracticeLog>(entity).unwrap().weights.is_empty(),
            "practice_log sibling slice must reset to default, not inherit the poisoned weights"
        );
        let live_insight_quota = world.get::<InsightQuota>(entity).unwrap();
        assert_eq!(
            live_insight_quota.used_this_realm, 0,
            "insight_quota sibling slice must reset to default"
        );
        assert!(
            live_insight_quota.fired_triggers.is_empty(),
            "insight_quota.fired_triggers must reset to empty, not inherit the stolen trigger"
        );
        assert!(
            world
                .get::<UnlockedPerceptions>(entity)
                .unwrap()
                .set
                .is_empty(),
            "unlocked_perceptions sibling slice must reset to empty, not inherit the stolen \
             perception"
        );
        assert_eq!(
            world.get::<InsightModifiers>(entity).unwrap().qi_regen_mul,
            InsightModifiers::new().qi_regen_mul,
            "insight_modifiers sibling slice must reset to default, not the poisoned 7.0x"
        );
        let live_poison_toxicity = world
            .get::<PoisonToxicity>(entity)
            .expect("poison_toxicity must still attach");
        assert_eq!(
            live_poison_toxicity.level, 0.0,
            "poison_toxicity sibling slice must reset to default (level=0.0), not the poisoned \
             88.0"
        );
        let live_digestion_load = world
            .get::<DigestionLoad>(entity)
            .expect("digestion_load must still attach");
        assert_ne!(
            live_digestion_load.current, 500.0,
            "digestion_load sibling slice must reset to the realm default, not inherit the \
             poisoned current=500.0"
        );
        assert_eq!(
            *world
                .get::<Nourishment>(entity)
                .expect("nourishment must still attach"),
            Nourishment::spawn_default(),
            "nourishment sibling slice must reset to the spawn default, not inherit 3/4"
        );
        assert_eq!(
            *world
                .get::<crate::nourishment::tick::NourishmentActivityWindow>(entity)
                .expect("nourishment activity window must still attach"),
            crate::nourishment::tick::NourishmentActivityWindow::default(),
            "activity window must attach fresh rather than inherit any persisted state"
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn joined_clients_default_race_for_legacy_bundle_missing_race_field() {
        let (settings, root) = temp_persistence_settings("legacy-missing-race");
        seed_raw_cultivation_bundle_without_race_field(&settings, "OldTimer", Realm::Solidify);

        let mut app = App::new();
        app.insert_resource(settings);
        app.insert_resource(race_registry_with_only_human());
        app.add_systems(Update, attach_cultivation_to_joined_clients);

        let (client_bundle, _helper) = create_mock_client("OldTimer");
        let entity = app.world_mut().spawn(client_bundle).id();

        app.update();

        let cultivation = app
            .world()
            .get::<Cultivation>(entity)
            .expect("cultivation must attach");
        assert_eq!(
            cultivation.race,
            RaceId::new(crate::body_plan::HUMAN_RACE_ID),
            "a legacy bundle with no \"race\" key at all must fall back to \
             #[serde(default = \"default_race_id\")] = \"human\", exactly like any other \
             pre-P0 archived save"
        );
        assert_eq!(
            cultivation.realm,
            Realm::Solidify,
            "unlike the unknown-race case, a merely *missing* race field is not corruption — \
             the rest of the bundle (realm=Solidify) must survive intact, proving the reject \
             path is keyed on 'race id present but unresolvable', not on 'race field \
             untouched by the persisted payload'"
        );

        let _ = std::fs::remove_dir_all(root);
    }
}
