//! 客户端 → 服务端 `bong:client_request` 通道处理（plan-cultivation-v1 §P1 剩余）。
//!
//! Fabric 客户端通过 Minecraft CustomPayload 发送 `ClientRequestV1` JSON；
//! 本系统读取 Valence `CustomPayloadEvent`，按 channel 过滤 → 反序列化
//! → 发射对应 Bevy 事件：
//!   - SetMeridianTarget → 插入/更新 `MeridianTarget` Component
//!   - BreakthroughRequest → emit `BreakthroughRequest` Bevy event
//!   - ForgeRequest → emit `ForgeRequest` Bevy event

use std::collections::{HashMap, HashSet};

#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};

use bevy_ecs::system::SystemParam;
use valence::custom_payload::CustomPayloadEvent;
use valence::message::SendMessage;
use valence::prelude::{
    bevy_ecs, BlockPos, ChunkLayer, Client, Commands, DVec3, Entity, EntityLayerId, EntityManager,
    EventReader, EventWriter, Events, Position, Query, RemovedComponents, Res, ResMut, Resource,
    UniqueId, Username, With, Without,
};

use crate::alchemy::residue::{residue_alchemy_data, residue_kind_for_recyclable_outcome};
use crate::alchemy::{
    learned::LearnResult, AlchemyFurnace, AlchemySession, Intervention, LearnedRecipes,
    PlaceFurnaceRequest, RecipeRegistry, MIN_ZONE_QI_TO_ALCHEMY,
};
use crate::botany::components::HarvestSessionStore;
use crate::botany::harvest::request_harvest_mode;
use crate::coffin::{CoffinEnterRequest, CoffinLeaveRequest, CoffinPlaceRequest};
use crate::combat::anqi_v2::{cycle_container_slot, switch_container_slot};
use crate::combat::carrier::{CarrierSlot, ChargeCarrierIntent, ThrowCarrierIntent};
use crate::combat::components::{
    CastSource, Casting, Lifecycle, LifecycleState, QuickSlotBindings, SkillBarBindings, SkillSlot,
    Stamina, Wounds,
};
use crate::combat::events::{ApplyStatusEffectIntent, DefenseIntent, StatusEffectKind};
use crate::combat::foreign_qi_resistance::foreign_qi_resistance_for_use;
use crate::combat::needle::IntentSource;
use crate::combat::tuike::{can_equip_false_skin, false_skin_kind_for_item, FalseSkinForgeRequest};
use crate::combat::CombatClock;
use crate::craft::workbench::workbench_block_pos;
use crate::craft::WorkbenchBlock;
use crate::cultivation::breakthrough::BreakthroughRequest;
use crate::cultivation::components::{
    recover_current_qi, Cultivation, MeridianChannelId, MeridianId,
};
use crate::cultivation::dugu::SelfAntidoteIntent;
use crate::cultivation::forging::ForgeRequest;
use crate::cultivation::insight::{InsightChosen, InsightRequest};
use crate::cultivation::known_techniques::{
    KnownTechniques, TechniqueDefinition, TechniqueRegistry,
};
use crate::cultivation::lifespan::LifespanExtensionIntent;
use crate::cultivation::meridian::severed::{
    check_player_skill_meridian_gate, MeridianSeveredPermanent, SkillMeridianDependencies,
};
use crate::cultivation::meridian_open::MeridianTarget;
use crate::cultivation::poison_trait::{ConsumePoisonPillIntent, PoisonPillKind};
use crate::cultivation::possession::{DuoSheRequestEvent, UseLifeCoreEvent};
use crate::cultivation::skill_registry::{CastRejectReason, CastResult, SkillRegistry};
use crate::cultivation::technique_scroll::{TechniqueLearnedEvent, TechniqueScrollReadEvent};
use crate::cultivation::tribulation::{HeartDemonChoiceSubmitted, StartDuXuRequest};
use crate::cultivation::void::actions::VoidActionIntent;
use crate::fauna::dying_elder::DyingElderState;
use crate::forge::blueprint::BlueprintRegistry;
use crate::forge::events::{
    ConsecrationInject, InscriptionScrollSubmit, StartForgeRequest, StepAdvance, TemperingHit,
};
use crate::forge::learned::LearnedBlueprints;
use crate::forge::session::{ForgeSessionId, ForgeSessions, ForgeStep};
use crate::forge::station::{PlaceForgeStationRequest, WeaponForgeStation};
use crate::inventory::{
    add_item_to_player_inventory_with_alchemy, apply_inventory_move_with_race,
    apply_item_spiritual_wear, consume_item_instance_once, discard_inventory_item_to_dropped_loot,
    fully_repair_weapon_instance, inventory_instance_container_attrition_exempt,
    inventory_item_by_instance_borrow, inventory_item_by_instance_mut,
    inventory_location_attrition_exempt, pickup_dropped_loot_instance, DroppedLootRegistry,
    InventoryDurabilityChangedEvent, InventoryInstanceIdAllocator, InventoryMoveOutcome,
    InventoryMoveRejectReason, ItemInstance, PlayerInventory,
};
use crate::inventory::{
    AlchemyItemData, ItemCategory, ItemEffect, ItemRegistry,
    DEFAULT_CAST_DURATION_MS as TEMPLATE_DEFAULT_CAST_MS,
    DEFAULT_COOLDOWN_MS as TEMPLATE_DEFAULT_COOLDOWN_MS,
};
use crate::lingtian::requests::PendingLingtianRequest;
use crate::lingtian::session::{ReplenishSource, SessionMode};
use crate::lingtian::LingtianPlot;
use crate::mineral::probe::is_probe_target_in_range;
use crate::mineral::MineralProbeIntent;
use crate::movement::{MovementAction, MovementActionIntent};
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::alchemy_bridge::alchemy_session_id;
use crate::network::alchemy_snapshot_emit;
use crate::network::audio_event_emit::{AudioRecipient, PlaySoundRecipeRequest};
use crate::network::cast_emit::{
    apply_item_effect, current_unix_millis, push_cast_sync, CAST_INTERRUPT_COOLDOWN_TICKS,
};
use crate::network::client_request::{combat, forge, inventory, npc, production, scroll};
use crate::network::client_request::{social, world};
use crate::network::gate::budget::BudgetStore;
use crate::network::gate::{GateContext, GateDenialReason};
use crate::shelflife::probe::FreshnessProbeIntent;
// dropped_loot_sync is emitted by dropped_loot_sync_emit.
#[cfg(test)]
use crate::identity::PlayerIdentities;
use crate::network::inventory_move_rejected_emit::emit_inventory_move_rejected;
use crate::network::inventory_snapshot_emit::send_inventory_snapshot_to_client;
use crate::network::qi_attrition_emit::{
    emit_attrition_applied_if_lost, item_abs_qi_for_attrition, AttritionAppliedEvent,
};
use crate::network::qi_color_observed_emit::QiColorInspectRequest;
use crate::network::quickslot_config_emit::{
    build_quickslot_config, current_unix_millis_for_quickslot, send_quickslot_config_to_client,
};
use crate::network::send_server_data_payload;
use crate::network::skill_config_emit::send_skill_config_snapshot_to_client;
use crate::network::{
    gameplay_vfx, redis_bridge::RedisOutbound, vfx_event_emit::VfxEventRequest, RedisBridgeResource,
};
#[cfg(test)]
use crate::npc::faction::FactionMembership;
use crate::npc::lifecycle::NpcArchetype;
use crate::npc::spawn::NpcMarker;
use crate::persistence::ZoneRuntimeRecord;
use crate::player::gameplay::{GameplayActionQueue, GameplayTick};
use crate::player::state::{
    canonical_player_id, save_player_inventory_and_delete_dropped_loot, update_player_ui_prefs,
    PlayerState, PlayerStatePersistence,
};
use crate::qi_physics::attrition::{apply_attrition_checked, is_attrition_exempt};
use crate::qi_physics::constants::QI_TARGETED_ITEM_WEAR_WEIGHT_THRESHOLD;
use crate::qi_physics::ledger::AttritionOpKind;
use crate::qi_physics::qi_targeted_item_wear_fraction;
use crate::qi_physics::AnqiContainerKind;
use crate::schema::alchemy::{AlchemyInterventionResultV1, AlchemySessionStartV1};
use crate::schema::client_request::{ClientRequestV1, SkillBarBindingV1};
use crate::schema::combat_hud::{CastOutcomeV1, CastPhaseV1, CastSyncV1};
use crate::schema::common::EventKind;
use crate::schema::inventory::{
    ContainerIdV1, EquipSlotV1, EquipStateV1, InventoryEventV1, InventoryLocationV1,
};
use crate::schema::server_data::{PillBuffStatusV1, ServerDataPayloadV1, ServerDataV1};
use crate::schema::social::GuardianKindV1;
use crate::shelflife::{
    age_peak_check_with_season, container_storage_multiplier, spoil_check_with_season,
    AgeBonusRoll, AgePeakCheck, ContainerFreshnessBehavior, DecayProfileRegistry,
    SpoilCheckOutcome, SpoilConsumeWarning, SpoilSeverity,
};
use crate::skill::components::SkillSet;
use crate::skill::config::{
    handle_config_intent, skill_config_snapshot_for_cast, validate_skill_config,
    SkillConfigRejectReason, SkillConfigSchemas, SkillConfigSnapshot, SkillConfigStore,
};
use crate::skill::events::{SkillScrollUsed, SkillXpGain};
#[cfg(test)]
use crate::social::components::{FactionReputation, FactionReputationTier};
use crate::social::events::{
    SpiritNicheActivateGuardianRequest, SpiritNicheCoordinateRevealRequest,
    SpiritNichePlaceRequest, SpiritNicheRepairRequest, SpiritNicheRevealSource,
};
use crate::world::block_place::BlockPlaceRequest;
use crate::world::dimension::{CurrentDimension, DimensionKind, DimensionLayers};
use crate::world::events::EVENT_REALM_COLLAPSE;
use crate::world::extract_system::{
    CancelExtractRequest as CancelExtractRequestEvent,
    StartExtractRequest as StartExtractRequestEvent,
};
use crate::world::karma::KarmaWeightStore;
use crate::world::season::{query_season, WorldSeasonState};
use crate::world::spawn_tutorial::CoffinOpenRequest;
use crate::world::tsy_container_search::{
    CancelSearchRequest as CancelSearchRequestEvent, StartSearchRequest as StartSearchRequestEvent,
};
use crate::world::tsy_lifecycle::TsyZoneStateRegistry;
use crate::world::zone::{ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};
#[path = "client_request/session.rs"]
mod session;

// NPC 请求域实现位于编译期 typed route；保留参数类型作为顶层 system seam。
pub(crate) use crate::network::client_request::npc::NpcEngagementRequestParams;

// 这些 helper re-export 仅供现有 NPC 行为测试复用，生产路由不依赖它们。
#[cfg(test)]
pub(crate) use crate::network::client_request::npc::{
    is_rarity_refused_at_low_rep, npc_trade_catalog_entry, reputation_to_player_score_for_npc_zone,
    NpcEngagementTarget,
};

/// per-client alchemy mock 状态，让 client→server 操作（翻页/学方）有可观察的回响。
/// 真实数据流（ECS 接入后）会替换掉本 resource。
#[derive(Default, Resource, Debug)]
pub struct AlchemyMockState {
    /// player_id → current recipe-book index
    pub recipe_index: HashMap<String, i32>,
}

type DyingElderTargetQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static crate::fauna::dying_elder::DyingElderState,
        &'static NpcArchetype,
    ),
    (With<NpcMarker>, Without<Client>),
>;

/// 把 cast / quickslot 相关查询打包，避免 `handle_client_request_payloads`
/// 顶部参数 tuple 超出 Bevy 0.14 SystemParam 16-tuple 上限。
#[derive(SystemParam)]
pub struct CombatRequestParams<'w, 's> {
    pub casting_q: Query<'w, 's, &'static Casting>,
    pub bindings_q: Query<'w, 's, &'static mut QuickSlotBindings>,
    pub skillbar_bindings_q: Query<'w, 's, &'static mut SkillBarBindings>,
    pub positions: Query<'w, 's, &'static valence::prelude::Position>,
    pub dimensions: Query<'w, 's, &'static CurrentDimension>,
    pub dying_elder_targets: DyingElderTargetQuery<'w, 's>,
    pub unique_ids: Query<'w, 's, &'static UniqueId>,
    pub skill_registry: Option<Res<'w, SkillRegistry>>,
    pub technique_registry: Res<'w, TechniqueRegistry>,
    pub skill_config_store: Option<ResMut<'w, SkillConfigStore>>,
    pub skill_config_schemas: Option<Res<'w, SkillConfigSchemas>>,
    pub entity_manager: Option<Res<'w, EntityManager>>,
    pub item_registry: Res<'w, ItemRegistry>,
    pub decay_profiles: Option<Res<'w, DecayProfileRegistry>>,
    pub buff_tx: EventWriter<'w, ApplyStatusEffectIntent>,
    pub insight_request_tx: Option<ResMut<'w, Events<InsightRequest>>>,
    pub false_skin_forge_tx: Option<ResMut<'w, Events<FalseSkinForgeRequest>>>,
    pub start_extract_tx: Option<ResMut<'w, Events<StartExtractRequestEvent>>>,
    pub cancel_extract_tx: Option<ResMut<'w, Events<CancelExtractRequestEvent>>>,
    pub start_search_tx: Option<ResMut<'w, Events<StartSearchRequestEvent>>>,
    pub cancel_search_tx: Option<ResMut<'w, Events<CancelSearchRequestEvent>>>,
    pub meridians: Query<'w, 's, &'static mut crate::cultivation::components::MeridianSystem>,
    pub contaminations: Query<'w, 's, &'static mut crate::cultivation::components::Contamination>,
    pub wounds: Query<'w, 's, &'static mut Wounds>,
    pub staminas: Query<'w, 's, &'static mut Stamina>,
    pub spoil_warnings: Option<ResMut<'w, Events<SpoilConsumeWarning>>>,
    pub age_bonus_rolls: Option<ResMut<'w, Events<AgeBonusRoll>>>,
    pub season_state: Option<Res<'w, WorldSeasonState>>,
    pub poison_pill_tx: Option<ResMut<'w, Events<ConsumePoisonPillIntent>>>,
    pub pill_intake_tx: Option<ResMut<'w, Events<crate::dandao::toxin_tracker::PillIntakeTracked>>>,
    pub ext_containers:
        Query<'w, 's, &'static mut crate::inventory::external_container::ExternalContainer>,
    /// plan-bug-qc-p1 §skill-cast P0：玩家 skill-bar cast 前的经脉门控。
    /// SkillMeridianDependencies Resource 按 skill_id → deps 表声明；Optional 兼容无 Resource 的测试场景。
    pub skill_meridian_deps: Option<Res<'w, SkillMeridianDependencies>>,
    /// plan-bug-qc-p1 §skill-cast P0：玩家永久断脉状态，供 cast 前 SEVERED 门控。
    pub player_severed: Query<'w, 's, Option<&'static MeridianSeveredPermanent>>,
    /// plan-scroll-reading-v1 P2：读卷中标记（真相源），供 `ScrollReadClosed` 分支查询以
    /// 决定是否需要发 `StopAnim` + 移除 marker。
    pub scroll_reading_q: Query<'w, 's, &'static crate::network::scroll_open_emit::ScrollReading>,
    /// plan-race-system-v1 P3a —— 施放门 race gate（`handle_skill_bar_cast` 拥有门后、
    /// 经脉门前判定，见该函数内插入点）。`Option` 与其余 registry 同规则。
    pub cultivations: Query<'w, 's, &'static Cultivation>,
    pub body_plans: Option<Res<'w, crate::body_plan::BodyPlanRegistry>>,
    pub race_registry: Option<Res<'w, crate::body_plan::RaceRegistry>>,
}

#[derive(SystemParam)]
pub struct DroppedLootRequestParams<'w, 's> {
    pub registry: ResMut<'w, DroppedLootRegistry>,
    pub positions: Query<'w, 's, &'static valence::prelude::Position>,
    /// plan-remains-suite P0 — 遗骸 G 键统一交互，转发给 `inventory::handle_remains_loot_intents`
    /// 独立 system（该 system 需要的 `(Entity, &UniqueId, &mut RemainsContainer, ...)` 查询
    /// 形状与本巨型 match 函数已有的 `Query<&mut PlayerInventory>` 放在同一 system 里会
    /// 产生 query 别名冲突，故走 event 中转，与 `pickup_dropped_item` 的直接处理不同）。
    pub remains_loot_tx: EventWriter<'w, crate::inventory::RemainsLootIntent>,
}

/// plan-lingtian-v1 §1.2-§1.7 + fix-spec-1901-v2 §4.1 — 6 类 intent 的 ingress
/// 队列写入包，避开 SystemParam 16 上限。
///
/// v2 起 producer 不再读取 `Position` / `CurrentDimension`，也不再直接写
/// `Start*Request` event：只把已解析请求 push 进 `PendingLingtianRequests`，
/// 由 `LingtianPostTransferValidationSet` 的唯一 validator 在权威移动写入后
/// dispatch（terrain / environment 的 chunk 读取也移到那里）。
#[derive(SystemParam)]
pub struct LingtianRequestParams<'w> {
    pub pending: ResMut<'w, crate::lingtian::requests::PendingLingtianRequests>,
}

/// Runtime owner of the C2S ingress budget.  The pure token and aggregation
/// accounting remains in [`BudgetStore`]; this wrapper only binds it to the
/// lifetime of a connected ECS client and forgets state when a role generation
/// changes.
#[derive(Debug, Default, Resource)]
pub struct ClientRequestBudget {
    pub store: BudgetStore<Entity>,
    character_ids: HashMap<Entity, String>,
}

/// O(1) lookup surface for authoritative lingtian plot positions.  The
/// snapshot is refreshed once per update; individual C2S requests do not
/// rescan every plot.
#[derive(Debug, Default, Resource)]
pub struct LingtianPlotIndex {
    positions: HashSet<BlockPos>,
}

impl LingtianPlotIndex {
    fn contains(&self, position: &BlockPos) -> bool {
        self.positions.contains(position)
    }
}

pub fn refresh_lingtian_plot_index(
    mut index: ResMut<LingtianPlotIndex>,
    plots: Query<&LingtianPlot>,
) {
    index.positions.clear();
    index.positions.extend(plots.iter().map(|plot| plot.pos));
}

impl ClientRequestBudget {
    fn prepare_client(&mut self, client: Entity, character_id: Option<&str>) -> bool {
        let current = character_id.unwrap_or("<unbound>");
        if let Some(previous) = self.character_ids.get_mut(&client) {
            if previous != current {
                self.store.cleanup(&client);
                current.clone_into(previous);
            }
            return true;
        }

        if self.character_ids.len() >= self.store.max_clients() {
            return false;
        }

        // A bucket may have been seeded through the pure store API before
        // lifecycle metadata was observed. Treat it as an unknown role
        // generation and discard it before binding the current character.
        self.store.cleanup(&client);
        self.character_ids.insert(client, current.to_owned());
        true
    }

    fn cleanup_client(&mut self, client: Entity) {
        self.store.cleanup(&client);
        self.character_ids.remove(&client);
    }

    fn retain_active<I>(&mut self, active_clients: I)
    where
        I: IntoIterator<Item = (Entity, Option<String>)>,
    {
        let active: Vec<_> = active_clients.into_iter().collect();
        let active_entities: HashSet<_> = active.iter().map(|(entity, _)| *entity).collect();
        self.store.retain_active(active_entities.iter().copied());
        self.character_ids
            .retain(|entity, _| active_entities.contains(entity));
        for (entity, character_id) in active {
            if self.character_ids.contains_key(&entity) || self.store.contains_client(&entity) {
                self.prepare_client(entity, character_id.as_deref());
            }
        }
    }
}

/// Drop ingress state before disconnected clients are despawned.  The same
/// pass also notices a changed `Lifecycle.character_id` and starts the new
/// role generation with a fresh bucket.
pub fn cleanup_client_request_budget(
    mut budget: ResMut<ClientRequestBudget>,
    mut disconnected: RemovedComponents<Client>,
    clients: Query<(Entity, Option<&Lifecycle>), With<Client>>,
) {
    for client in disconnected.read() {
        budget.cleanup_client(client);
    }
    budget.retain_active(clients.iter().map(|(entity, lifecycle)| {
        (
            entity,
            lifecycle.map(|lifecycle| lifecycle.character_id.clone()),
        )
    }));
}

type ClientRequestGateTarget<'a> = (
    &'a Position,
    Option<&'a CurrentDimension>,
    Option<&'a EntityLayerId>,
    Option<&'a WorkbenchBlock>,
    Option<&'a DyingElderState>,
);

/// Authority facts needed by the live gate adapters.  This query is
/// deliberately read-only; the external-container mutation query remains in
/// `CombatRequestParams` and is only borrowed after this barrier succeeds.
#[derive(SystemParam)]
pub struct ClientRequestIngressParams<'w, 's> {
    pub combat_clock: Res<'w, CombatClock>,
    pub budget: Option<ResMut<'w, ClientRequestBudget>>,
    pub lifecycles: Query<'w, 's, Option<&'static Lifecycle>>,
    pub gate_targets: Query<'w, 's, ClientRequestGateTarget<'static>>,
    pub lingtian_plot_index: Option<Res<'w, LingtianPlotIndex>>,
    pub chunk_layers:
        Query<'w, 's, &'static ChunkLayer, With<crate::world::dimension::OverworldLayer>>,
    pub dimension_layers: Option<Res<'w, DimensionLayers>>,
}

/// 合并 alchemy 相关 Resource/Query，避开 `handle_client_request_payloads`
/// 顶部参数的 16-tuple Bevy 0.14 SystemParam 上限。
#[derive(SystemParam)]
pub struct AlchemyRequestParams<'w, 's> {
    pub state: ResMut<'w, AlchemyMockState>,
    pub furnaces: Query<'w, 's, (Entity, &'static mut AlchemyFurnace)>,
    pub learned: Query<'w, 's, &'static mut LearnedRecipes>,
    pub recipe_registry: Res<'w, RecipeRegistry>,
    pub learn_fragment_tx: Option<ResMut<'w, Events<crate::alchemy::LearnRecipeFragmentIntent>>>,
    pub place_furnace_tx: Option<ResMut<'w, Events<PlaceFurnaceRequest>>>,
    pub outcome_tx: Option<ResMut<'w, Events<crate::alchemy::AlchemyOutcomeEvent>>>,
    pub item_registry: Res<'w, ItemRegistry>,
    pub instance_allocator: Option<ResMut<'w, InventoryInstanceIdAllocator>>,
    pub redis: Option<Res<'w, RedisBridgeResource>>,
    /// plan-qi-handling-attrition-v1 P0/P1：升级为 ResMut，兼顾 alchemy read-only 和磨损写权限。
    pub zones: Option<ResMut<'w, ZoneRegistry>>,
    /// plan-qi-handling-attrition-v1 P3：死坍缩渊 family 禁止磨损结算。
    pub tsy_lifecycle: Option<Res<'w, TsyZoneStateRegistry>>,
    pub vfx_events: Option<ResMut<'w, Events<VfxEventRequest>>>,
    /// plan-qi-handling-attrition-v1 P0/P1：AttritionTax 审计转账事件队列。
    pub attrition_qi_transfers: Option<ResMut<'w, Events<crate::qi_physics::ledger::QiTransfer>>>,
    /// plan-qi-handling-attrition-v1 P2：定向客户端粒子反馈事件队列。
    pub attrition_applied_events: Option<ResMut<'w, Events<AttritionAppliedEvent>>>,
    /// plan-fauna-stitched-beast-v1 P3：兽核吸收幻觉事件 (M1 修复：接通 narration/hallucination)
    pub hallucination_events:
        Option<ResMut<'w, Events<crate::fauna::hybrid_beast::CoreAbsorptionHallucinationEvent>>>,
    /// plan-fauna-stitched-beast-v1 P3：叙事容器（M1 修复：兽核吸收后推 player narration）
    pub pending_narrations: Option<ResMut<'w, crate::player::gameplay::PendingGameplayNarrations>>,
}

#[derive(SystemParam)]
pub struct ClientRequestDispatchParams<'w> {
    pub(crate) combat: combat::CombatRequestParams<'w>,
    pub(crate) social: social::SocialRequestParams<'w>,
    pub(crate) world: world::WorldFormationRequestParams<'w>,
    pub gameplay_queue: Option<valence::prelude::ResMut<'w, GameplayActionQueue>>,
    pub gameplay_tick: Option<Res<'w, GameplayTick>>,
    pub harvest_sessions: Option<ResMut<'w, HarvestSessionStore>>,
    pub breakthrough_tx: EventWriter<'w, BreakthroughRequest>,
    pub start_du_xu_tx: Option<ResMut<'w, Events<StartDuXuRequest>>>,
    pub void_action_tx: Option<ResMut<'w, Events<VoidActionIntent>>>,
    pub movement_action_tx: Option<ResMut<'w, Events<MovementActionIntent>>>,
    pub heart_demon_choice_tx: Option<ResMut<'w, Events<HeartDemonChoiceSubmitted>>>,
    pub forge_tx: EventWriter<'w, ForgeRequest>,
    pub insight_tx: EventWriter<'w, InsightChosen>,
    pub lifespan_extension_tx: Option<ResMut<'w, Events<LifespanExtensionIntent>>>,
    pub duo_she_tx: Option<ResMut<'w, Events<DuoSheRequestEvent>>>,
    pub qi_color_inspect_tx: Option<ResMut<'w, Events<QiColorInspectRequest>>>,
    pub life_core_tx: Option<ResMut<'w, Events<UseLifeCoreEvent>>>,
    pub self_antidote_tx: Option<ResMut<'w, Events<SelfAntidoteIntent>>>,
    pub defense_tx: Option<ResMut<'w, Events<DefenseIntent>>>,
    pub place_forge_station_tx: Option<ResMut<'w, Events<PlaceForgeStationRequest>>>,
    /// plan-forge-session-entry-wiring-v1 §4.1#3/#4 — 起炉入口分发（原为 debug-log 死分支）。
    pub start_forge_tx: Option<ResMut<'w, Events<StartForgeRequest>>>,
    pub tempering_hit_tx: Option<ResMut<'w, Events<TemperingHit>>>,
    pub consecration_inject_tx: Option<ResMut<'w, Events<ConsecrationInject>>>,
    pub step_advance_tx: Option<ResMut<'w, Events<StepAdvance>>>,
    pub spirit_niche_place_tx: Option<ResMut<'w, Events<SpiritNichePlaceRequest>>>,
    pub spirit_niche_repair_tx: Option<ResMut<'w, Events<SpiritNicheRepairRequest>>>,
    pub spirit_niche_coordinate_reveal_tx:
        Option<ResMut<'w, Events<SpiritNicheCoordinateRevealRequest>>>,
    pub spirit_niche_activate_guardian_tx:
        Option<ResMut<'w, Events<SpiritNicheActivateGuardianRequest>>>,
    pub coffin_open_tx: Option<ResMut<'w, Events<CoffinOpenRequest>>>,
    pub coffin_place_tx: Option<ResMut<'w, Events<CoffinPlaceRequest>>>,
    pub coffin_enter_tx: Option<ResMut<'w, Events<CoffinEnterRequest>>>,
    pub coffin_leave_tx: Option<ResMut<'w, Events<CoffinLeaveRequest>>>,
    pub coffin_break_tx: Option<ResMut<'w, Events<crate::coffin::CoffinBreakRequest>>>,
    pub coffin_menu_reclaim_tx: Option<ResMut<'w, Events<crate::coffin::CoffinMenuReclaimRequest>>>,
    pub block_place_tx: Option<ResMut<'w, Events<BlockPlaceRequest>>>,
    /// plan-worldgen-v4 P5 §8.1#5 — 画廊 dev-only give-block intent。
    pub block_picker_give_tx:
        Option<ResMut<'w, Events<crate::cmd::dev::block_picker::BlockPickerGiveIntent>>>,
    pub charge_carrier_tx: Option<ResMut<'w, Events<ChargeCarrierIntent>>>,
    pub throw_carrier_tx: Option<ResMut<'w, Events<ThrowCarrierIntent>>>,
    // ─── plan-craft-v1 P2：通用手搓 intent ──────────────────
    pub craft_start_tx: Option<ResMut<'w, Events<crate::craft::CraftStartIntent>>>,
    pub craft_cancel_tx: Option<ResMut<'w, Events<crate::craft::CraftCancelIntent>>>,
    // ─── plan-supply-coffin-loot-ui P2：外部容器 + entity-based open ──────
    pub ext_container_registry:
        Option<ResMut<'w, crate::inventory::external_container::ExternalContainerRegistry>>,
    pub supply_coffin_registry: Option<Res<'w, crate::supply_coffin::SupplyCoffinRegistry>>,
    pub supply_coffin_open_tx:
        Option<ResMut<'w, Events<crate::supply_coffin::interact::SupplyCoffinOpenRequest>>>,
    pub container_open_tx:
        Option<ResMut<'w, Events<crate::world::container_open::ContainerOpenRequest>>>,
    // ─── plan-dying-elder-v1 P1：垂死大能给丹 C2S ──────────────────
    pub give_dan_to_elder_tx:
        Option<ResMut<'w, Events<crate::fauna::dying_elder::GiveDanToElderIntent>>>,
    pub workbench_open_tx: Option<ResMut<'w, Events<crate::craft::WorkbenchOpenRequest>>>,
    // ─── plan-agent-ui-data-v1 P0：天道 UI 面板响应 ─────────────────────────
    pub agent_ui_response_tx: EventWriter<'w, crate::network::agent_ui::AgentUiResponseEvent>,
}
// NOTE: plan-qi-handling-attrition-v1 P0/P1 磨损写权限已合并入 AlchemyRequestParams.zones
// (ResMut) 和 AlchemyRequestParams.attrition_qi_transfers，避免与 AlchemyRequestParams.zones
// (Res) 产生 Bevy B0002 Res/ResMut 冲突。

#[derive(SystemParam)]
pub struct SkillScrollRequestParams<'w, 's> {
    pub skill_xp_tx: Option<ResMut<'w, Events<SkillXpGain>>>,
    pub skill_scroll_used_tx: Option<ResMut<'w, Events<SkillScrollUsed>>>,
    pub technique_scroll_read_tx: Option<ResMut<'w, Events<TechniqueScrollReadEvent>>>,
    pub technique_learned_tx: Option<ResMut<'w, Events<TechniqueLearnedEvent>>>,
    pub mineral_probe_tx: Option<ResMut<'w, Events<MineralProbeIntent>>>,
    pub freshness_probe_tx: Option<ResMut<'w, Events<FreshnessProbeIntent>>>,
    pub skill_sets: Query<'w, 's, &'static mut SkillSet>,
    pub known_techniques: Query<'w, 's, &'static mut KnownTechniques>,
    pub learned_blueprints: Query<'w, 's, &'static mut LearnedBlueprints>,
    pub cultivations: Query<'w, 's, &'static Cultivation>,
    pub severed_meridians: Query<'w, 's, Option<&'static MeridianSeveredPermanent>>,
    pub positions: Query<'w, 's, &'static valence::prelude::Position>,
    pub dimensions: Query<'w, 's, &'static CurrentDimension>,
    pub inscription_scroll_tx: Option<ResMut<'w, Events<InscriptionScrollSubmit>>>,
    pub forge_sessions: Option<Res<'w, ForgeSessions>>,
    pub item_registry: Res<'w, ItemRegistry>,
    pub technique_registry: Res<'w, TechniqueRegistry>,
    /// forge station 查找：station_pos → Entity 寻址（对齐
    /// `with_owned_furnace_mut` 的 BlockPos 寻址模式）。
    pub forge_stations: Query<'w, 's, (Entity, &'static WeaponForgeStation)>,
    /// plan-forge-session-entry-wiring-v1 §4.1#2 — 翻页后回推 `forge_blueprint_book` 需要
    /// blueprint 的 display_name/tier_cap/step_count。`Option` 与 `forge_sessions` 同规则：
    /// 资源缺失时优雅跳过 S2C 回推而不 panic（`forge::register` 正常路径下恒 Some）。
    pub blueprint_registry: Option<Res<'w, BlueprintRegistry>>,
    /// plan-race-system-v1 P3a —— 习得门 race gate 判定（`RaceGate::Humanoid` 档需要本体
    /// `is_humanoid`，见 `learn_technique_if_allowed` 调用点）。`Option` 与其余 registry
    /// 同规则：既有单测未插入这两个资源时优雅退化到 humanoid（`resolve_body_plan_for_target`
    /// 文档化的退化行为）。
    pub body_plans: Option<Res<'w, crate::body_plan::BodyPlanRegistry>>,
    pub race_registry: Option<Res<'w, crate::body_plan::RaceRegistry>>,
    /// plan-race-system-v1 P4 —— 当前易形形态。习得门 `form_anchors_open` 消费点
    /// （`learn_technique_if_allowed` 调用点判定本体经脉是否满足易形前置）与
    /// `handle_inventory_move` Form 身份判定（装备门）共用本查询。
    pub morph_states: Query<'w, 's, Option<&'static crate::body_plan::MorphState>>,
    pub craft_registry: Option<Res<'w, crate::craft::CraftRegistry>>,
    pub craft_unlock_state: Option<ResMut<'w, crate::craft::RecipeUnlockState>>,
    pub craft_unlock_tx: Option<ResMut<'w, Events<crate::craft::CraftUnlockIntent>>>,
}

const CHANNEL: &str = "bong:client_request";
const SUPPORTED_VERSION: u8 = 1;
/// plan-cultivation-v1 §3.1：服用突破辅助丹药的 buff 持续时间（5 分钟）。
/// 20 tick/s × 60 s × 5 = 6000。
const BREAKTHROUGH_BOOST_DURATION_TICKS: u64 = 6_000;

#[cfg(test)]
static CLIENT_REQUEST_DECODE_COUNT: AtomicUsize = AtomicUsize::new(0);

#[cfg(test)]
fn decode_client_request(payload: &str) -> Result<ClientRequestV1, serde_json::Error> {
    if payload
        .as_bytes()
        .get(..128)
        .is_some_and(|prefix| prefix.iter().all(|byte| *byte == b'\n'))
    {
        CLIENT_REQUEST_DECODE_COUNT.fetch_add(1, Ordering::Relaxed);
    }
    serde_json::from_str(payload)
}

#[cfg(not(test))]
fn decode_client_request(payload: &str) -> Result<ClientRequestV1, serde_json::Error> {
    serde_json::from_str(payload)
}

/// plan-race-system-v1 P1c — 参数改为 `MeridianChannelId`（wire 开放化后
/// `SetMeridianTarget.meridian` 不再是闭合 `MeridianId` 枚举）；仅 humanoid 20 条
/// channel id 有中文标签，非 humanoid 构型（P5 飞鲸等）落显式"未知经脉"占位，不伪造。
fn meridian_label(id: &MeridianChannelId) -> &'static str {
    let Some(legacy_id) = id.to_meridian_id() else {
        return "未知经脉";
    };
    match legacy_id {
        MeridianId::Lung => "肺经",
        MeridianId::LargeIntestine => "大肠经",
        MeridianId::Stomach => "胃经",
        MeridianId::Spleen => "脾经",
        MeridianId::Heart => "心经",
        MeridianId::SmallIntestine => "小肠经",
        MeridianId::Bladder => "膀胱经",
        MeridianId::Kidney => "肾经",
        MeridianId::Pericardium => "心包经",
        MeridianId::TripleEnergizer => "三焦经",
        MeridianId::Gallbladder => "胆经",
        MeridianId::Liver => "肝经",
        MeridianId::Ren => "任脉",
        MeridianId::Du => "督脉",
        MeridianId::Chong => "冲脉",
        MeridianId::Dai => "带脉",
        MeridianId::YinQiao => "阴跷脉",
        MeridianId::YangQiao => "阳跷脉",
        MeridianId::YinWei => "阴维脉",
        MeridianId::YangWei => "阳维脉",
    }
}

fn live_gate_request_kind(request: &ClientRequestV1) -> Option<&'static str> {
    match request {
        ClientRequestV1::GiveDanToElder { .. } => Some("give_dan_to_elder"),
        ClientRequestV1::LingtianStartTill { .. } => Some("lingtian_start_till"),
        ClientRequestV1::CraftStart { .. } => Some("craft_start"),
        ClientRequestV1::WorkbenchOpen { .. } => Some("workbench_open"),
        ClientRequestV1::ExternalContainerMove { .. } => Some("external_container_move"),
        _ => None,
    }
}

fn entity_gate_authority(entity: Entity) -> String {
    format!("entity:{}", entity.to_bits())
}

fn gate_position(position: &Position) -> [f64; 3] {
    let position = position.get();
    [position.x, position.y, position.z]
}

fn dimension_for_target_layer(
    current: Option<&CurrentDimension>,
    layer: Option<&EntityLayerId>,
    dimension_layers: Option<&DimensionLayers>,
) -> Option<DimensionKind> {
    current.map(|dimension| dimension.0).or_else(|| {
        let layers = dimension_layers?;
        let layer = layer?.0;
        if layer == layers.overworld {
            Some(DimensionKind::Overworld)
        } else if layer == layers.tsy {
            Some(DimensionKind::Tsy)
        } else {
            None
        }
    })
}

fn requester_gate_context(
    client: Entity,
    ingress: &ClientRequestIngressParams<'_, '_>,
    clients: &mut Query<(&Username, &mut Client)>,
) -> Result<GateContext, GateDenialReason> {
    let lifecycle = ingress
        .lifecycles
        .get(client)
        .ok()
        .flatten()
        .ok_or(GateDenialReason::MissingAuthorityContext)?;
    if lifecycle.state != LifecycleState::Alive {
        return Err(GateDenialReason::InvalidState);
    }
    if clients.get_mut(client).is_err() {
        return Err(GateDenialReason::MissingAuthorityContext);
    }

    let Ok((position, current_dimension, layer, _, _)) = ingress.gate_targets.get(client) else {
        return Err(GateDenialReason::MissingAuthorityContext);
    };
    let dimension = dimension_for_target_layer(
        current_dimension,
        layer,
        ingress.dimension_layers.as_deref(),
    )
    .ok_or(GateDenialReason::MissingAuthorityContext)?;

    Ok(GateContext::new(
        Some(gate_position(position)),
        Some(dimension),
        Some(entity_gate_authority(client)),
    ))
}

#[allow(clippy::too_many_arguments)]
fn evaluate_live_gate(
    request: &ClientRequestV1,
    client: Entity,
    ingress: &ClientRequestIngressParams<'_, '_>,
    lingtian_plot_index: Option<&LingtianPlotIndex>,
    dispatch: &ClientRequestDispatchParams<'_>,
    combat_params: &CombatRequestParams<'_, '_>,
    inventories: &mut Query<&mut PlayerInventory>,
    clients: &mut Query<(&Username, &mut Client)>,
) -> Result<(), GateDenialReason> {
    let gate = request.gate_spec();
    let requester = requester_gate_context(client, ingress, clients)?;

    match request {
        ClientRequestV1::CraftStart { .. } => {
            gate.check(&requester)?;
            if inventories.get_mut(client).is_err() {
                return Err(GateDenialReason::InvalidState);
            }
        }
        ClientRequestV1::LingtianStartTill { x, y, z, .. } => {
            let target_block = BlockPos::new(*x, *y, *z);
            let target_exists = lingtian_plot_index
                .is_some_and(|index| index.contains(&target_block))
                || ingress
                    .chunk_layers
                    .iter()
                    .any(|layer| layer.block(target_block).is_some());
            if !target_exists {
                return Err(GateDenialReason::TargetNotFound);
            }
            let target = [
                f64::from(*x) + 0.5,
                f64::from(*y) + 0.5,
                f64::from(*z) + 0.5,
            ];
            let context = requester.with_target(Some(target), Some(DimensionKind::Overworld), None);
            gate.check(&context)?;
        }
        ClientRequestV1::WorkbenchOpen { entity_id, .. } => {
            let entity_manager = combat_params
                .entity_manager
                .as_deref()
                .ok_or(GateDenialReason::MissingAuthorityContext)?;
            let target = entity_manager
                .get_by_id(*entity_id)
                .ok_or(GateDenialReason::TargetNotFound)?;
            let (position, current_dimension, layer, workbench, _) = ingress
                .gate_targets
                .get(target)
                .map_err(|_| GateDenialReason::TargetNotFound)?;
            let target_dimension = dimension_for_target_layer(
                current_dimension,
                layer,
                ingress.dimension_layers.as_deref(),
            )
            .ok_or(GateDenialReason::TargetNotFound)?;
            let block_position = workbench_block_pos(position);
            let target_position = [
                f64::from(block_position[0]),
                f64::from(block_position[1]),
                f64::from(block_position[2]),
            ];
            let context =
                requester.with_target(Some(target_position), Some(target_dimension), None);
            gate.check(&context)?;
            if workbench.is_none() {
                return Err(GateDenialReason::InvalidState);
            }
        }
        ClientRequestV1::GiveDanToElder {
            elder_entity_id, ..
        } => {
            let entity_manager = combat_params
                .entity_manager
                .as_deref()
                .ok_or(GateDenialReason::MissingAuthorityContext)?;
            let target = entity_manager
                .get_by_id(*elder_entity_id)
                .ok_or(GateDenialReason::TargetNotFound)?;
            let (position, current_dimension, layer, _, elder_state) = ingress
                .gate_targets
                .get(target)
                .map_err(|_| GateDenialReason::TargetNotFound)?;
            let target_dimension = dimension_for_target_layer(
                current_dimension,
                layer,
                ingress.dimension_layers.as_deref(),
            )
            .ok_or(GateDenialReason::TargetNotFound)?;
            let context =
                requester.with_target(Some(gate_position(position)), Some(target_dimension), None);
            gate.check(&context)?;
            let elder_state = elder_state.ok_or(GateDenialReason::InvalidState)?;
            let Ok((_state, archetype)) = combat_params.dying_elder_targets.get(target) else {
                return Err(GateDenialReason::InvalidState);
            };
            if *archetype != NpcArchetype::DyingElder {
                return Err(GateDenialReason::InvalidState);
            }
            match *elder_state {
                DyingElderState::Plea => {}
                DyingElderState::Recovering { dan_received }
                    if dan_received < crate::fauna::dying_elder::DYING_ELDER_DAN_THRESHOLD => {}
                _ => return Err(GateDenialReason::InvalidState),
            }
        }
        ClientRequestV1::ExternalContainerMove { session_id, .. } => {
            let ext_registry = dispatch
                .ext_container_registry
                .as_deref()
                .ok_or(GateDenialReason::MissingAuthorityContext)?;
            let target = *ext_registry
                .sessions
                .get(session_id)
                .ok_or(GateDenialReason::TargetNotFound)?;
            let (opened_by, is_supply_coffin, timeout_wall_secs) = {
                let external = combat_params
                    .ext_containers
                    .get(target)
                    .map_err(|_| GateDenialReason::TargetNotFound)?;
                (
                    external.opened_by,
                    matches!(
                        &external.source_kind,
                        crate::inventory::external_container::ExternalContainerKind::SupplyCoffin { .. }
                    ),
                    external.timeout_wall_secs,
                )
            };
            let active_supply_coffin = dispatch
                .supply_coffin_registry
                .as_deref()
                .and_then(|registry| registry.active.get(&target));
            let ecs_facts = ingress.gate_targets.get(target).ok();
            let target_position = ecs_facts
                .as_ref()
                .map(|(position, _, _, _, _)| gate_position(position))
                .or_else(|| {
                    if !is_supply_coffin {
                        return None;
                    }
                    let position = active_supply_coffin?.pos;
                    Some([position.x, position.y, position.z])
                })
                .ok_or(GateDenialReason::TargetNotFound)?;
            let target_dimension = ecs_facts
                .as_ref()
                .and_then(|(_, current_dimension, layer, _, _)| {
                    dimension_for_target_layer(
                        *current_dimension,
                        *layer,
                        ingress.dimension_layers.as_deref(),
                    )
                })
                .or_else(|| {
                    if !is_supply_coffin {
                        return None;
                    }
                    Some(active_supply_coffin?.dimension)
                })
                .ok_or(GateDenialReason::TargetNotFound)?;
            let context = requester.with_target(
                Some(target_position),
                Some(target_dimension),
                opened_by.map(entity_gate_authority),
            );
            gate.check(&context)?;
            if opened_by.is_none() {
                return Err(GateDenialReason::NotOwner);
            }
            if external_session_is_expired(
                timeout_wall_secs,
                crate::supply_coffin::current_wall_clock_secs(),
            ) {
                return Err(GateDenialReason::Expired);
            }
        }
        _ => return Err(GateDenialReason::InvalidState),
    }

    Ok(())
}

/// Generic external containers use `0` to mean that no wall-clock expiry is
/// configured.  Supply-coffin sessions always carry a positive deadline, so
/// this keeps the live gate aligned with the existing lifecycle contract.
fn external_session_is_expired(timeout_wall_secs: u64, now_wall_secs: u64) -> bool {
    timeout_wall_secs != 0 && now_wall_secs >= timeout_wall_secs
}

fn gate_feedback_message(reason: GateDenialReason) -> &'static str {
    match reason {
        GateDenialReason::TargetNotFound
        | GateDenialReason::NotVisible
        | GateDenialReason::WrongDimension
        | GateDenialReason::OutOfReach
        | GateDenialReason::NotOwner => "目标不可用",
        _ => "当前状态不可用",
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LiveGateFeedback {
    EventAlert,
    Chat(&'static str),
    Silent,
}

fn live_gate_feedback(
    request: Option<&ClientRequestV1>,
    reason: GateDenialReason,
    client: Entity,
    inventories: Option<&mut Query<&mut PlayerInventory>>,
) -> LiveGateFeedback {
    let Some(request) = request else {
        return LiveGateFeedback::EventAlert;
    };

    match request {
        // Preserve the established consumer contract: these two target lookup
        // failures are chat-only, while an out-of-range workbench is a silent
        // interaction rejection.  The budget still bounds both paths.
        ClientRequestV1::WorkbenchOpen { .. } => match reason {
            GateDenialReason::TargetNotFound => LiveGateFeedback::Chat("§c[制作台] 目标不存在。"),
            _ => LiveGateFeedback::Silent,
        },
        ClientRequestV1::GiveDanToElder {
            pill_instance_id, ..
        } => {
            if let Some(inventories) = inventories {
                let template_id = inventories.get_mut(client).ok().and_then(|inventory| {
                    crate::inventory::inventory_item_by_instance_borrow(
                        &inventory,
                        *pill_instance_id,
                    )
                    .map(|item| item.template_id.clone())
                });
                let Some(template_id) = template_id else {
                    return LiveGateFeedback::Chat("§c[垂死大能] 背包中未找到该回元丹。");
                };
                if template_id != "huiyuan_pill" {
                    return LiveGateFeedback::Chat("§c[垂死大能] 只接受回元丹。");
                }
            }

            match reason {
                GateDenialReason::TargetNotFound => {
                    LiveGateFeedback::Chat("§c[垂死大能] 找不到目标大能。")
                }
                GateDenialReason::WrongDimension | GateDenialReason::OutOfReach => {
                    LiveGateFeedback::Chat("§c[垂死大能] 目标不在当前位面或交互范围内。")
                }
                GateDenialReason::InvalidState => {
                    LiveGateFeedback::Chat("§c[垂死大能] 目标不是可交互的大能。")
                }
                _ => LiveGateFeedback::Silent,
            }
        }
        // Till has always been rejected without a client-facing response; its
        // existing post-transfer validator remains the domain-level authority.
        ClientRequestV1::LingtianStartTill { .. } => LiveGateFeedback::Silent,
        _ => LiveGateFeedback::EventAlert,
    }
}

#[allow(clippy::too_many_arguments)]
fn report_live_gate_denial(
    client: Entity,
    tick: u64,
    request_kind: &'static str,
    reason: GateDenialReason,
    request: Option<&ClientRequestV1>,
    inventories: Option<&mut Query<&mut PlayerInventory>>,
    budget: Option<&mut ClientRequestBudget>,
    clients: &mut Query<(&Username, &mut Client)>,
) -> bool {
    let Some(budget) = budget else {
        return false;
    };
    let feedback = budget
        .store
        .admit_feedback(client, tick, request_kind, reason);
    let log = budget.store.admit_log(client, tick, request_kind, reason);

    if log.emit {
        tracing::warn!(
            target: "bong::network::c2s_gate",
            request_kind,
            reason = ?reason,
            suppressed = log.suppressed_count,
            "live C2S request rejected"
        );
    }
    let feedback_mode = live_gate_feedback(request, reason, client, inventories);
    if !feedback.emit || feedback_mode == LiveGateFeedback::Silent {
        return false;
    }

    if let LiveGateFeedback::Chat(message) = feedback_mode {
        if let Ok((_username, mut client)) = clients.get_mut(client) {
            client.send_chat_message(message);
        }
        return true;
    }

    let payload = ServerDataV1::new(ServerDataPayloadV1::EventAlert {
        event: EventKind::Generic,
        message: gate_feedback_message(reason).to_owned(),
        zone: None,
        duration_ticks: Some(70),
    });
    let Ok(bytes) = serialize_server_data_payload(&payload) else {
        tracing::warn!(
            target: "bong::network::c2s_gate",
            request_kind,
            "live C2S rejection feedback serialization failed"
        );
        return false;
    };
    if let Ok((_username, mut client)) = clients.get_mut(client) {
        send_server_data_payload(&mut client, bytes.as_slice());
    }
    true
}

/// Preserve the external-container recovery payload on a gate rejection
/// without entering `handle_external_container_move`.  These snapshots are
/// read-only feedback; the mutation barrier remains closed and the inventory
/// revision/container contents are untouched.
#[allow(clippy::too_many_arguments)]
fn resync_external_container_after_gate_denial(
    player: Entity,
    session_id: u64,
    dispatch: &ClientRequestDispatchParams<'_>,
    combat_params: &mut CombatRequestParams<'_, '_>,
    inventories: &mut Query<&mut PlayerInventory>,
    player_states: &Query<&PlayerState>,
    cultivations: &Query<&Cultivation>,
    clients: &mut Query<(&Username, &mut Client)>,
) {
    let Some(registry) = dispatch.ext_container_registry.as_deref() else {
        resync_inventory_only(player, inventories, player_states, cultivations, clients);
        return;
    };
    let Some(&container_entity) = registry.sessions.get(&session_id) else {
        resync_inventory_only(player, inventories, player_states, cultivations, clients);
        return;
    };
    let external = combat_params
        .ext_containers
        .get(container_entity)
        .ok()
        .cloned();
    let Some(external) = external else {
        resync_inventory_only(player, inventories, player_states, cultivations, clients);
        return;
    };
    if external.opened_by != Some(player) {
        // A gate rejection must not disclose the container to a requester who
        // has not been proven to own the live session. The requester still
        // receives their own authoritative inventory snapshot.
        resync_inventory_only(player, inventories, player_states, cultivations, clients);
        return;
    }
    resync_ext_and_inventory(
        player,
        &external,
        inventories,
        player_states,
        cultivations,
        clients,
    );
}

#[allow(clippy::too_many_arguments)] // Bevy system signature; one resource/query per gameplay area.
pub fn handle_client_request_payloads(
    mut events: EventReader<CustomPayloadEvent>,
    mut dispatch: ClientRequestDispatchParams,
    mut ingress: ClientRequestIngressParams,
    mut commands: Commands,
    mut clients: Query<(&Username, &mut Client)>,
    persistence: Option<Res<PlayerStatePersistence>>,
    mut alchemy_params: AlchemyRequestParams,
    mut inventories: Query<&mut PlayerInventory>,
    player_states: Query<&PlayerState>,
    karma_weights: Option<Res<KarmaWeightStore>>,
    mut durability_changed_tx: Option<ResMut<Events<InventoryDurabilityChangedEvent>>>,
    mut combat_params: CombatRequestParams,
    mut dropped_loot_params: DroppedLootRequestParams,
    mut lingtian_tx: LingtianRequestParams,
    mut skill_scroll_params: SkillScrollRequestParams,
    mut npc_engagement_params: NpcEngagementRequestParams,
) {
    // Production wiring always inserts this resource.  If an alternate app
    // forgets it, fail closed instead of allowing an unbudgeted payload.
    #[cfg(not(test))]
    if ingress.budget.is_none() {
        return;
    }

    let mut pending_forge_steps: HashMap<(u64, ForgeSessionId), ForgeStep> = HashMap::new();
    let combat_clock = &ingress.combat_clock;
    for ev in events.read() {
        if ev.channel.as_str() != CHANNEL {
            continue;
        }

        if let Some(budget) = ingress.budget.as_deref_mut() {
            let character_id = ingress
                .lifecycles
                .get(ev.client)
                .ok()
                .flatten()
                .map(|lifecycle| lifecycle.character_id.as_str());
            if !budget.prepare_client(ev.client, character_id)
                || !budget
                    .store
                    .admit_ingress(ev.client, ingress.combat_clock.tick)
                    .admitted
            {
                report_live_gate_denial(
                    ev.client,
                    ingress.combat_clock.tick,
                    "ingress",
                    GateDenialReason::RateLimited,
                    None,
                    None,
                    ingress.budget.as_deref_mut(),
                    &mut clients,
                );
                continue;
            }
        }

        let payload = match std::str::from_utf8(&ev.data) {
            Ok(s) => s,
            Err(err) => {
                tracing::warn!(
                    "[bong][network] client_request payload not utf8 from {:?}: {err}",
                    ev.client
                );
                continue;
            }
        };

        let decoded_request = decode_client_request(payload);
        let request: ClientRequestV1 = match decoded_request {
            Ok(r) => r,
            Err(err) => {
                // 带 user= 关联键：deserialize-failed 是全局频道共有的 warn，bot
                // 场景要按本 bot 归属计数（否则同窗其他客户端的畸形请求会让
                // 载体作用域断言跨客户端误红，review finding [minor]：全局
                // 反序列化失败计数）。登录中/断连瞬间拿不到 Username 时用
                // <unknown> 占位，不影响正常归属。
                let client_user = clients
                    .get(ev.client)
                    .ok()
                    .map(|(username, _)| username.0.as_str())
                    .unwrap_or("<unknown>");
                tracing::warn!(
                    "[bong][network] client_request deserialize failed from {:?} (user={client_user}): {err}; payload_bytes={}",
                    ev.client,
                    ev.data.len()
                );
                continue;
            }
        };
        // 只记录长度，避免聊天、目标与请求参数进入 server 日志或支持包。
        tracing::info!(
            "[bong][network] client_request received entity={:?} payload_bytes={}",
            ev.client,
            ev.data.len()
        );

        let v = match &request {
            ClientRequestV1::SetMeridianTarget { v, .. }
            | ClientRequestV1::BreakthroughRequest { v }
            | ClientRequestV1::StartDuXu { v }
            | ClientRequestV1::VoidAction { v, .. }
            | ClientRequestV1::MovementAction { v, .. }
            | ClientRequestV1::AbortTribulation { v }
            | ClientRequestV1::HeartDemonDecision { v, .. }
            | ClientRequestV1::ForgeRequest { v, .. }
            | ClientRequestV1::InsightDecision { v, .. }
            | ClientRequestV1::BotanyHarvestRequest { v, .. }
            | ClientRequestV1::AlchemyOpenFurnace { v, .. }
            | ClientRequestV1::AlchemyFeedSlot { v, .. }
            | ClientRequestV1::AlchemyTakeBack { v, .. }
            | ClientRequestV1::AlchemyIgnite { v, .. }
            | ClientRequestV1::AlchemyIntervention { v, .. }
            | ClientRequestV1::AlchemyTurnPage { v, .. }
            | ClientRequestV1::AlchemyLearnRecipe { v, .. }
            | ClientRequestV1::AlchemyLearnRecipeFragment { v, .. }
            | ClientRequestV1::AlchemyTakePill { v, .. }
            | ClientRequestV1::AlchemyFurnacePlace { v, .. }
            | ClientRequestV1::CoffinOpen { v, .. }
            | ClientRequestV1::CoffinPlace { v, .. }
            | ClientRequestV1::BlockPlace { v, .. }
            | ClientRequestV1::BlockPickerGive { v, .. }
            | ClientRequestV1::CoffinEnter { v, .. }
            | ClientRequestV1::CoffinLeave { v }
            | ClientRequestV1::CoffinBreak { v, .. }
            | ClientRequestV1::CoffinMenuReclaim { v, .. }
            | ClientRequestV1::SpiritNichePlace { v, .. }
            | ClientRequestV1::SpiritNicheRepair { v, .. }
            | ClientRequestV1::SpiritNicheGaze { v, .. }
            | ClientRequestV1::SpiritNicheMarkCoordinate { v, .. }
            | ClientRequestV1::SpiritNicheActivateGuardian { v, .. }
            | ClientRequestV1::SparringInviteResponse { v, .. }
            | ClientRequestV1::TradeOfferRequest { v, .. }
            | ClientRequestV1::TradeOfferResponse { v, .. }
            | ClientRequestV1::NpcInspectRequest { v, .. }
            | ClientRequestV1::NpcDialogueChoice { v, .. }
            | ClientRequestV1::NpcTradeRequest { v, .. }
            | ClientRequestV1::ZhenfaPlace { v, .. }
            | ClientRequestV1::ZhenfaTrigger { v, .. }
            | ClientRequestV1::ZhenfaDisarm { v, .. }
            | ClientRequestV1::QiScatterBeadUse { v, .. }
            | ClientRequestV1::LearnSkillScroll { v, .. }
            | ClientRequestV1::TechniqueScrollUse { v, .. }
            | ClientRequestV1::InventoryMoveIntent { v, .. }
            | ClientRequestV1::EquipFalseSkin { v, .. }
            | ClientRequestV1::ForgeFalseSkin { v, .. }
            | ClientRequestV1::InventoryDiscardItem { v, .. }
            | ClientRequestV1::TreasureActivate { v, .. }
            | ClientRequestV1::DropWeaponIntent { v, .. }
            | ClientRequestV1::RepairWeaponIntent { v, .. }
            | ClientRequestV1::PickupDroppedItem { v, .. }
            | ClientRequestV1::RemainsLoot { v, .. }
            | ClientRequestV1::MineralProbe { v, .. }
            | ClientRequestV1::FreshnessProbe { v, .. }
            | ClientRequestV1::ApplyPill { v, .. }
            | ClientRequestV1::SelfAntidote { v, .. }
            | ClientRequestV1::DuoSheRequest { v, .. }
            | ClientRequestV1::QiColorInspect { v, .. }
            | ClientRequestV1::UseLifeCore { v, .. }
            | ClientRequestV1::Jiemai { v }
            | ClientRequestV1::UseQuickSlot { v, .. }
            | ClientRequestV1::QuickSlotBind { v, .. }
            | ClientRequestV1::SkillBarCast { v, .. }
            | ClientRequestV1::SkillBarBind { v, .. }
            | ClientRequestV1::SkillConfigIntent { v, .. }
            | ClientRequestV1::CombatReincarnate { v }
            | ClientRequestV1::CombatTerminate { v }
            | ClientRequestV1::CombatCreateNewCharacter { v }
            | ClientRequestV1::StartExtractRequest { v, .. }
            | ClientRequestV1::CancelExtractRequest { v }
            | ClientRequestV1::StartSearch { v, .. }
            | ClientRequestV1::CancelSearch { v }
            | ClientRequestV1::LingtianStartTill { v, .. }
            | ClientRequestV1::LingtianStartRenew { v, .. }
            | ClientRequestV1::LingtianStartPlanting { v, .. }
            | ClientRequestV1::LingtianStartHarvest { v, .. }
            | ClientRequestV1::LingtianStartReplenish { v, .. }
            | ClientRequestV1::LingtianStartDrainQi { v, .. }
            | ClientRequestV1::ForgeStartSession { v, .. }
            | ClientRequestV1::ForgeTemperingHit { v, .. }
            | ClientRequestV1::ForgeInscriptionScroll { v, .. }
            | ClientRequestV1::ForgeConsecrationInject { v, .. }
            | ClientRequestV1::ForgeStepAdvance { v, .. }
            | ClientRequestV1::ForgeBlueprintTurnPage { v, .. }
            | ClientRequestV1::ForgeLearnBlueprint { v, .. }
            | ClientRequestV1::ForgeStationPlace { v, .. }
            | ClientRequestV1::ChargeCarrier { v, .. }
            | ClientRequestV1::ThrowCarrier { v, .. }
            | ClientRequestV1::AnqiContainerSwitch { v, .. }
            | ClientRequestV1::CraftStart { v, .. }
            | ClientRequestV1::CraftCancel { v }
            | ClientRequestV1::SupplyCoffinOpen { v, .. }
            | ClientRequestV1::ContainerOpen { v, .. }
            | ClientRequestV1::WorkbenchOpen { v, .. }
            | ClientRequestV1::ExternalContainerMove { v, .. }
            | ClientRequestV1::ExternalContainerClose { v, .. }
            | ClientRequestV1::GiveDanToElder { v, .. }
            | ClientRequestV1::RaiseShield { v }
            | ClientRequestV1::LowerShield { v }
            | ClientRequestV1::ScrollReadRequest { v, .. }
            | ClientRequestV1::ScrollReadClosed { v }
            | ClientRequestV1::AgentUiResponse { v, .. } => *v,
        };
        if v != SUPPORTED_VERSION {
            tracing::warn!(
                "[bong][network] client_request unsupported version v={v} from {:?}; payload_bytes={}",
                ev.client,
                ev.data.len()
            );
            continue;
        }

        if let Some(request_kind) = live_gate_request_kind(&request) {
            if let Err(reason) = evaluate_live_gate(
                &request,
                ev.client,
                &ingress,
                ingress.lingtian_plot_index.as_deref(),
                &dispatch,
                &combat_params,
                &mut inventories,
                &mut clients,
            ) {
                report_live_gate_denial(
                    ev.client,
                    ingress.combat_clock.tick,
                    request_kind,
                    reason,
                    Some(&request),
                    Some(&mut inventories),
                    ingress.budget.as_deref_mut(),
                    &mut clients,
                );
                if let ClientRequestV1::ExternalContainerMove { session_id, .. } = &request {
                    if reason == GateDenialReason::NotOwner {
                        resync_inventory_only(
                            ev.client,
                            &inventories,
                            &player_states,
                            &skill_scroll_params.cultivations,
                            &mut clients,
                        );
                    } else {
                        resync_external_container_after_gate_denial(
                            ev.client,
                            *session_id,
                            &dispatch,
                            &mut combat_params,
                            &mut inventories,
                            &player_states,
                            &skill_scroll_params.cultivations,
                            &mut clients,
                        );
                    }
                }
                continue;
            }
        }

        if matches!(
            &request,
            ClientRequestV1::NpcInspectRequest { .. }
                | ClientRequestV1::NpcDialogueChoice { .. }
                | ClientRequestV1::NpcTradeRequest { .. }
        ) {
            npc::dispatch(
                &request,
                ev.client,
                combat_clock.tick,
                &combat_params,
                &mut npc_engagement_params,
                alchemy_params.zones.as_deref(),
                &mut clients,
                &mut inventories,
                &player_states,
                &skill_scroll_params.cultivations,
                &alchemy_params.item_registry,
                &mut alchemy_params.instance_allocator,
            );
            continue;
        }
        let request = match combat::try_into_combat_request(request) {
            Ok(combat_request) => {
                combat::dispatch_combat_request(
                    combat_request,
                    ev.client,
                    combat_clock.tick,
                    &mut dispatch.combat,
                );
                continue;
            }
            Err(request) => request,
        };
        let request = match social::try_into_social_request(request) {
            Ok(social_request) => {
                social::dispatch_social_request(
                    social_request,
                    ev.client,
                    combat_clock.tick,
                    &mut dispatch.social,
                    combat_params.entity_manager.as_deref(),
                );
                continue;
            }
            Err(request) => request,
        };
        let request = match world::try_into_world_formation_request(request) {
            Ok(world_request) => {
                world::dispatch_world_formation_request(
                    world_request,
                    ev.client,
                    combat_clock.tick,
                    &mut dispatch.world,
                );
                continue;
            }
            Err(request) => request,
        };

        let request = match production::try_into_production_request(request) {
            Ok(production_request) => {
                production::dispatch_production_request(
                    production_request,
                    ev.client,
                    combat_clock,
                    &mut alchemy_params,
                    &mut combat_params,
                    &mut dispatch,
                    &mut npc_engagement_params,
                    &mut skill_scroll_params,
                    &mut commands,
                    &mut clients,
                    &mut inventories,
                    &player_states,
                );
                continue;
            }
            Err(request) => request,
        };

        let request = match forge::try_into_forge_request(request) {
            Ok(forge_request) => {
                forge::dispatch_forge_request(
                    forge_request,
                    ev.client,
                    &mut pending_forge_steps,
                    &mut dispatch,
                    &mut skill_scroll_params,
                    &mut commands,
                    &mut clients,
                    &mut inventories,
                    &player_states,
                );
                continue;
            }
            Err(request) => request,
        };

        let request = match inventory::try_into_inventory_request(request) {
            Ok(inventory_request) => {
                inventory::dispatch_inventory_request(
                    inventory_request,
                    ev.client,
                    &mut alchemy_params,
                    &mut combat_params,
                    &mut dispatch,
                    &mut dropped_loot_params,
                    &mut skill_scroll_params,
                    persistence.as_deref(),
                    karma_weights.as_deref(),
                    durability_changed_tx.as_deref_mut(),
                    &mut clients,
                    &mut inventories,
                    &player_states,
                    &mut commands,
                );
                continue;
            }
            Err(request) => request,
        };

        let request = match scroll::try_into_scroll_request(request) {
            Ok(scroll_request) => {
                scroll::dispatch_scroll_request(
                    scroll_request,
                    ev.client,
                    &mut inventories,
                    &mut clients,
                    &player_states,
                    &mut skill_scroll_params,
                    &mut combat_params.meridians,
                );
                continue;
            }
            Err(request) => request,
        };

        if session::dispatch(
            &request,
            ev.client,
            &mut dispatch,
            &mut combat_params,
            &mut inventories,
            &mut clients,
            &mut commands,
            alchemy_params.vfx_events.as_deref_mut(),
        ) {
            continue;
        }

        match request {
            ClientRequestV1::CombatReincarnate { .. }
            | ClientRequestV1::CombatTerminate { .. }
            | ClientRequestV1::CombatCreateNewCharacter { .. }
            | ClientRequestV1::RaiseShield { .. }
            | ClientRequestV1::LowerShield { .. } => {
                unreachable!("Combat requests are dispatched by the typed Combat dispatcher")
            }
            ClientRequestV1::SparringInviteResponse { .. }
            | ClientRequestV1::TradeOfferRequest { .. }
            | ClientRequestV1::TradeOfferResponse { .. } => {
                unreachable!("Social requests are dispatched by the typed Social dispatcher")
            }
            ClientRequestV1::AlchemyOpenFurnace { .. }
            | ClientRequestV1::AlchemyFeedSlot { .. }
            | ClientRequestV1::AlchemyTakeBack { .. }
            | ClientRequestV1::AlchemyIgnite { .. }
            | ClientRequestV1::AlchemyIntervention { .. }
            | ClientRequestV1::AlchemyTurnPage { .. }
            | ClientRequestV1::AlchemyLearnRecipe { .. }
            | ClientRequestV1::AlchemyLearnRecipeFragment { .. }
            | ClientRequestV1::AlchemyTakePill { .. }
            | ClientRequestV1::AlchemyFurnacePlace { .. } => {
                unreachable!(
                    "Production requests are dispatched by the typed Production dispatcher"
                )
            }
            ClientRequestV1::ForgeStartSession { .. }
            | ClientRequestV1::ForgeTemperingHit { .. }
            | ClientRequestV1::ForgeInscriptionScroll { .. }
            | ClientRequestV1::ForgeConsecrationInject { .. }
            | ClientRequestV1::ForgeStepAdvance { .. }
            | ClientRequestV1::ForgeBlueprintTurnPage { .. }
            | ClientRequestV1::ForgeLearnBlueprint { .. }
            | ClientRequestV1::ForgeStationPlace { .. } => {
                unreachable!("Forge requests are dispatched by the typed Forge dispatcher")
            }
            ClientRequestV1::SetMeridianTarget { meridian, .. } => {
                tracing::info!(
                    "[bong][network] client_request set_meridian_target entity={:?} meridian={:?}",
                    ev.client,
                    meridian
                );
                commands
                    .entity(ev.client)
                    .insert(MeridianTarget(meridian.clone()));
                if let Ok((_username, mut client)) = clients.get_mut(ev.client) {
                    client.send_chat_message(format!(
                        "§a[修炼] 已收到经脉目标：{}。",
                        meridian_label(&meridian)
                    ));
                }
            }
            ClientRequestV1::BreakthroughRequest { .. } => {
                tracing::info!(
                    "[bong][network] client_request breakthrough entity={:?}",
                    ev.client
                );
                // material_bonus 的实际来源是玩家身上 StatusEffects 里的
                // BreakthroughBoost buff（由 AlchemyTakePill 吃丹挂上），
                // 在 breakthrough_system 内聚合消费。client 请求本身不传额外 bonus。
                dispatch.breakthrough_tx.send(BreakthroughRequest {
                    entity: ev.client,
                    material_bonus: 0.0,
                });
            }
            ClientRequestV1::StartDuXu { .. } => {
                tracing::info!(
                    "[bong][network] client_request start_du_xu entity={:?}",
                    ev.client,
                );
                if let Some(start_du_xu_tx) = dispatch.start_du_xu_tx.as_deref_mut() {
                    start_du_xu_tx.send(StartDuXuRequest {
                        entity: ev.client,
                        requested_at_tick: ingress.combat_clock.tick,
                    });
                }
            }
            ClientRequestV1::VoidAction { request, .. } => {
                tracing::info!(
                    "[bong][network] client_request void_action entity={:?} kind={:?}",
                    ev.client,
                    request.kind(),
                );
                let Some(void_action_tx) = dispatch.void_action_tx.as_deref_mut() else {
                    tracing::warn!(
                        "[bong][network] dropped void_action because VoidActionIntent event resource is missing"
                    );
                    continue;
                };
                void_action_tx.send(VoidActionIntent {
                    caster: ev.client,
                    request,
                    requested_at_tick: ingress.combat_clock.tick,
                });
            }
            ClientRequestV1::MovementAction {
                action,
                yaw_degrees,
                ..
            } => {
                tracing::debug!(
                    "[bong][network] client_request movement_action entity={:?} action={:?} yaw_degrees={:?}",
                    ev.client,
                    action,
                    yaw_degrees
                );
                let Some(movement_action_tx) = dispatch.movement_action_tx.as_deref_mut() else {
                    tracing::debug!(
                        "[bong][network] dropped movement_action because MovementActionIntent event resource is missing"
                    );
                    continue;
                };
                movement_action_tx.send(MovementActionIntent {
                    entity: ev.client,
                    action: MovementAction::from(action),
                    yaw_degrees,
                });
            }
            ClientRequestV1::AbortTribulation { .. } => {
                tracing::warn!(
                    "[bong][network] client_request abort_tribulation ignored entity={:?}; DuXu cannot be cancelled after confirmation",
                    ev.client,
                );
            }
            ClientRequestV1::HeartDemonDecision { choice_idx, .. } => {
                tracing::info!(
                    "[bong][network] client_request heart_demon_decision entity={:?} idx={:?}",
                    ev.client,
                    choice_idx,
                );
                if let Some(heart_demon_choice_tx) = dispatch.heart_demon_choice_tx.as_deref_mut() {
                    heart_demon_choice_tx.send(HeartDemonChoiceSubmitted {
                        entity: ev.client,
                        choice_idx,
                        submitted_at_tick: combat_clock.tick,
                    });
                }
            }
            ClientRequestV1::InsightDecision {
                trigger_id,
                choice_idx,
                ..
            } => {
                tracing::info!(
                    "[bong][network] client_request insight_decision entity={:?} trigger={} idx={:?}",
                    ev.client,
                    trigger_id,
                    choice_idx
                );
                dispatch.insight_tx.send(InsightChosen {
                    entity: ev.client,
                    trigger_id,
                    choice_idx: choice_idx.map(|n| n as usize),
                });
            }
            ClientRequestV1::ForgeRequest { meridian, axis, .. } => {
                tracing::info!(
                    "[bong][network] client_request forge entity={:?} meridian={:?} axis={:?}",
                    ev.client,
                    meridian,
                    axis
                );
                dispatch.forge_tx.send(ForgeRequest {
                    entity: ev.client,
                    meridian,
                    axis,
                });
            }
            ClientRequestV1::BotanyHarvestRequest {
                session_id, mode, ..
            } => {
                let Some(harvest_sessions) = dispatch.harvest_sessions.as_deref_mut() else {
                    tracing::warn!(
                        "[bong][network] dropped botany_harvest_request because HarvestSessionStore is missing"
                    );
                    continue;
                };
                let player_key = clients
                    .get(ev.client)
                    .map(|(username, _)| canonical_player_id(username.0.as_str()))
                    .unwrap_or_else(|_| format!("offline:{:?}", ev.client));
                let requested_mode = match mode {
                    crate::schema::botany::BotanyHarvestModeV1::Manual => {
                        crate::botany::components::BotanyHarvestMode::Manual
                    }
                    crate::schema::botany::BotanyHarvestModeV1::Auto => {
                        crate::botany::components::BotanyHarvestMode::Auto
                    }
                };
                let now_tick = dispatch
                    .gameplay_tick
                    .as_ref()
                    .map(|tick| tick.current_tick())
                    .unwrap_or(combat_clock.tick);
                if let Err(err) = request_harvest_mode(
                    harvest_sessions,
                    session_id.as_str(),
                    ev.client,
                    requested_mode,
                    now_tick,
                ) {
                    tracing::warn!(
                        "[bong][network] rejected botany_harvest_request player={} session={} mode={:?}: {}",
                        player_key,
                        session_id,
                        requested_mode,
                        err
                    );
                }
            }
            ClientRequestV1::CoffinOpen { x, y, z, .. } => {
                tracing::info!(
                    "[bong][network][spawn-tutorial] coffin_open entity={:?} pos=[{x},{y},{z}]",
                    ev.client
                );
                let Some(coffin_open_tx) = dispatch.coffin_open_tx.as_deref_mut() else {
                    tracing::warn!(
                        "[bong][network] dropped coffin_open because CoffinOpenRequest event resource is missing"
                    );
                    continue;
                };
                coffin_open_tx.send(CoffinOpenRequest {
                    player: ev.client,
                    pos: [x, y, z],
                    tick: combat_clock.tick,
                });
            }
            ClientRequestV1::CoffinPlace {
                x,
                y,
                z,
                item_instance_id,
                ..
            } => {
                tracing::info!(
                    "[bong][network][coffin] place entity={:?} pos=[{x},{y},{z}] instance={item_instance_id}",
                    ev.client
                );
                let Some(coffin_place_tx) = dispatch.coffin_place_tx.as_deref_mut() else {
                    tracing::warn!(
                        "[bong][network] dropped coffin_place because CoffinPlaceRequest event resource is missing"
                    );
                    continue;
                };
                coffin_place_tx.send(CoffinPlaceRequest {
                    player: ev.client,
                    pos: valence::prelude::BlockPos::new(x, y, z),
                    item_instance_id,
                    tick: combat_clock.tick,
                });
            }
            ClientRequestV1::BlockPlace {
                x,
                y,
                z,
                item_instance_id,
                target_face,
                ..
            } => {
                tracing::info!(
                    "[bong][network][block] dispatch block_place entity={:?} pos=[{x},{y},{z}] instance={item_instance_id} target_face={target_face:?}",
                    ev.client
                );
                let Some(block_place_tx) = dispatch.block_place_tx.as_deref_mut() else {
                    tracing::warn!(
                        "[bong][network] dropped block_place because BlockPlaceRequest event resource is missing"
                    );
                    continue;
                };
                block_place_tx.send(BlockPlaceRequest {
                    client: ev.client,
                    x,
                    y,
                    z,
                    item_instance_id,
                    target_face,
                });
            }
            ClientRequestV1::BlockPickerGive {
                block_id, count, ..
            } => {
                tracing::info!(
                    "[bong][network][dev] block_picker_give entity={:?} block_id={block_id} count={count}",
                    ev.client
                );
                let Some(block_picker_give_tx) = dispatch.block_picker_give_tx.as_deref_mut()
                else {
                    tracing::warn!(
                        "[bong][network] dropped block_picker_give because BlockPickerGiveIntent event resource is missing"
                    );
                    continue;
                };
                block_picker_give_tx.send(crate::cmd::dev::block_picker::BlockPickerGiveIntent {
                    player: ev.client,
                    block_id,
                    count,
                });
            }
            ClientRequestV1::CoffinEnter { x, y, z, .. } => {
                tracing::info!(
                    "[bong][network][coffin] enter entity={:?} pos=[{x},{y},{z}]",
                    ev.client
                );
                let Some(coffin_enter_tx) = dispatch.coffin_enter_tx.as_deref_mut() else {
                    tracing::warn!(
                        "[bong][network] dropped coffin_enter because CoffinEnterRequest event resource is missing"
                    );
                    continue;
                };
                coffin_enter_tx.send(CoffinEnterRequest {
                    player: ev.client,
                    pos: valence::prelude::BlockPos::new(x, y, z),
                    tick: combat_clock.tick,
                });
            }
            ClientRequestV1::CoffinLeave { .. } => {
                tracing::info!("[bong][network][coffin] leave entity={:?}", ev.client);
                let Some(coffin_leave_tx) = dispatch.coffin_leave_tx.as_deref_mut() else {
                    tracing::warn!(
                        "[bong][network] dropped coffin_leave because CoffinLeaveRequest event resource is missing"
                    );
                    continue;
                };
                coffin_leave_tx.send(CoffinLeaveRequest { player: ev.client });
            }
            ClientRequestV1::CoffinBreak { x, y, z, .. } => {
                tracing::info!(
                    "[bong][network][coffin] break entity={:?} pos=[{x},{y},{z}]",
                    ev.client
                );
                let Some(coffin_break_tx) = dispatch.coffin_break_tx.as_deref_mut() else {
                    tracing::warn!(
                        "[bong][network] dropped coffin_break because CoffinBreakRequest event resource is missing"
                    );
                    continue;
                };
                coffin_break_tx.send(crate::coffin::CoffinBreakRequest {
                    player: ev.client,
                    pos: valence::prelude::BlockPos::new(x, y, z),
                    tick: combat_clock.tick,
                });
            }
            ClientRequestV1::CoffinMenuReclaim { x, y, z, .. } => {
                tracing::info!(
                    "[bong][network][coffin] menu_reclaim entity={:?} pos=[{x},{y},{z}]",
                    ev.client
                );
                let Some(coffin_menu_reclaim_tx) = dispatch.coffin_menu_reclaim_tx.as_deref_mut()
                else {
                    tracing::warn!(
                        "[bong][network] dropped coffin_menu_reclaim because CoffinMenuReclaimRequest event resource is missing"
                    );
                    continue;
                };
                coffin_menu_reclaim_tx.send(crate::coffin::CoffinMenuReclaimRequest {
                    player: ev.client,
                    pos: valence::prelude::BlockPos::new(x, y, z),
                    tick: combat_clock.tick,
                });
            }
            ClientRequestV1::SpiritNichePlace {
                x,
                y,
                z,
                item_instance_id,
                ..
            } => {
                tracing::info!(
                    "[bong][network][social] spirit_niche_place entity={:?} pos=[{x},{y},{z}] instance={item_instance_id}",
                    ev.client
                );
                let Some(spirit_niche_place_tx) = dispatch.spirit_niche_place_tx.as_deref_mut()
                else {
                    tracing::warn!(
                        "[bong][network] dropped spirit_niche_place because SpiritNichePlaceRequest event resource is missing"
                    );
                    continue;
                };
                spirit_niche_place_tx.send(SpiritNichePlaceRequest {
                    player: ev.client,
                    pos: [x, y, z],
                    item_instance_id: Some(item_instance_id),
                    tick: combat_clock.tick,
                });
            }
            ClientRequestV1::SpiritNicheRepair {
                x,
                y,
                z,
                item_instance_id,
                ..
            } => {
                tracing::info!(
                    "[bong][network][social] spirit_niche_repair entity={:?} pos=[{x},{y},{z}] instance={item_instance_id}",
                    ev.client
                );
                let Some(spirit_niche_repair_tx) = dispatch.spirit_niche_repair_tx.as_deref_mut()
                else {
                    tracing::warn!(
                        "[bong][network] dropped spirit_niche_repair because SpiritNicheRepairRequest event resource is missing"
                    );
                    continue;
                };
                spirit_niche_repair_tx.send(SpiritNicheRepairRequest {
                    player: ev.client,
                    pos: [x, y, z],
                    item_instance_id: Some(item_instance_id),
                    tick: combat_clock.tick,
                });
            }
            ClientRequestV1::SpiritNicheGaze { x, y, z, .. } => {
                tracing::info!(
                    "[bong][network][social] spirit_niche_gaze entity={:?} pos=[{x},{y},{z}]",
                    ev.client
                );
                let Some(reveal_tx) = dispatch.spirit_niche_coordinate_reveal_tx.as_deref_mut()
                else {
                    tracing::warn!(
                        "[bong][network] dropped spirit_niche_gaze because SpiritNicheCoordinateRevealRequest event resource is missing"
                    );
                    continue;
                };
                reveal_tx.send(SpiritNicheCoordinateRevealRequest {
                    observer: ev.client,
                    pos: [x, y, z],
                    source: SpiritNicheRevealSource::Gaze,
                    tick: combat_clock.tick,
                });
            }
            ClientRequestV1::SpiritNicheMarkCoordinate { x, y, z, .. } => {
                tracing::info!(
                    "[bong][network][social] spirit_niche_mark_coordinate entity={:?} pos=[{x},{y},{z}]",
                    ev.client
                );
                let Some(reveal_tx) = dispatch.spirit_niche_coordinate_reveal_tx.as_deref_mut()
                else {
                    tracing::warn!(
                        "[bong][network] dropped spirit_niche_mark_coordinate because SpiritNicheCoordinateRevealRequest event resource is missing"
                    );
                    continue;
                };
                reveal_tx.send(SpiritNicheCoordinateRevealRequest {
                    observer: ev.client,
                    pos: [x, y, z],
                    source: SpiritNicheRevealSource::MarkCoordinate,
                    tick: combat_clock.tick,
                });
            }
            ClientRequestV1::SpiritNicheActivateGuardian {
                niche_pos,
                guardian_kind,
                materials,
                ..
            } => {
                tracing::info!(
                    "[bong][network][social] spirit_niche_activate_guardian entity={:?} pos={:?} kind={:?}",
                    ev.client,
                    niche_pos,
                    guardian_kind
                );
                let Some(activate_tx) = dispatch.spirit_niche_activate_guardian_tx.as_deref_mut()
                else {
                    tracing::warn!(
                        "[bong][network] dropped spirit_niche_activate_guardian because SpiritNicheActivateGuardianRequest event resource is missing"
                    );
                    continue;
                };
                activate_tx.send(SpiritNicheActivateGuardianRequest {
                    player: ev.client,
                    niche_pos,
                    guardian_kind: guardian_kind_from_schema(guardian_kind),
                    materials,
                    tick: combat_clock.tick,
                });
            }
            // NPC requests are consumed by the typed route above. This arm exists only to keep
            // the exhaustive match explicit if the route is ever rearranged.
            ClientRequestV1::NpcInspectRequest { .. }
            | ClientRequestV1::NpcDialogueChoice { .. }
            | ClientRequestV1::NpcTradeRequest { .. } => {
                unreachable!("NPC request bypassed its typed route")
            }
            ClientRequestV1::ZhenfaPlace { .. }
            | ClientRequestV1::ZhenfaTrigger { .. }
            | ClientRequestV1::ZhenfaDisarm { .. }
            | ClientRequestV1::QiScatterBeadUse { .. } => {
                unreachable!(
                    "World formation requests are dispatched by the typed world dispatcher"
                )
            }
            ClientRequestV1::LearnSkillScroll { .. }
            | ClientRequestV1::TechniqueScrollUse { .. } => {
                unreachable!("Scroll requests are dispatched by the typed Scroll dispatcher")
            }
            ClientRequestV1::EquipFalseSkin {
                slot,
                item_instance_id,
                ..
            } => {
                // plan-layered-equip-v1 P0.1: FalseSkin slot removed; 伪皮归 CHEST worn 层.
                // The client may pass any slot in the request; we ignore it and always target Chest/Worn.
                let _ = slot;
                let from = inventories.get(ev.client).ok().and_then(|inventory| {
                    find_inventory_instance_location(inventory, item_instance_id)
                });
                let Some(from) = from else {
                    tracing::warn!(
                        "[bong][network][tuike] equip_false_skin rejected: instance {item_instance_id} not found for entity {:?}",
                        ev.client
                    );
                    continue;
                };
                handle_inventory_move(
                    ev.client,
                    item_instance_id,
                    from,
                    InventoryLocationV1::Equip {
                        slot: EquipSlotV1::Chest,
                        state: EquipStateV1::Worn,
                    },
                    // 伪皮装备走 equip 目标，非网格落位，旋转标志天然不适用。
                    false,
                    &combat_params.item_registry,
                    &mut inventories,
                    &mut clients,
                    &player_states,
                    &skill_scroll_params.cultivations,
                    karma_weights.as_deref(),
                    durability_changed_tx.as_deref_mut(),
                    &skill_scroll_params.positions,
                    &skill_scroll_params.dimensions,
                    alchemy_params.zones.as_deref_mut(),
                    alchemy_params.attrition_qi_transfers.as_deref_mut(),
                    alchemy_params.attrition_applied_events.as_deref_mut(),
                    alchemy_params.tsy_lifecycle.as_deref(),
                    &mut dropped_loot_params.registry,
                    alchemy_params.vfx_events.as_deref_mut(),
                    combat_params.body_plans.as_deref(),
                    combat_params.race_registry.as_deref(),
                    &skill_scroll_params.morph_states,
                );
            }
            ClientRequestV1::ForgeFalseSkin { kind, .. } => {
                if let Some(events) = combat_params.false_skin_forge_tx.as_deref_mut() {
                    events.send(FalseSkinForgeRequest {
                        crafter: ev.client,
                        kind: kind.into(),
                    });
                } else {
                    tracing::warn!(
                        "[bong][network][tuike] forge_false_skin ignored: FalseSkinForgeRequest event resource missing"
                    );
                }
            }
            ClientRequestV1::TreasureActivate {
                instance_id,
                activate,
                ..
            } => {
                handle_treasure_activate(
                    ev.client,
                    instance_id,
                    activate,
                    &combat_params.item_registry,
                    &mut inventories,
                    &mut clients,
                    &player_states,
                    &skill_scroll_params.cultivations,
                );
            }
            ClientRequestV1::DropWeaponIntent {
                instance_id, from, ..
            } => {
                handle_inventory_discard(
                    ev.client,
                    instance_id,
                    from,
                    &mut inventories,
                    &mut dropped_loot_params.registry,
                    &combat_params.item_registry,
                    &mut clients,
                    &player_states,
                    &skill_scroll_params.cultivations,
                    &dropped_loot_params.positions,
                    &skill_scroll_params.dimensions,
                );
            }
            ClientRequestV1::RepairWeaponIntent {
                instance_id,
                station_pos,
                ..
            } => {
                handle_repair_weapon(
                    ev.client,
                    instance_id,
                    station_pos,
                    &combat_params.item_registry,
                    &mut inventories,
                    &mut clients,
                    &player_states,
                    &skill_scroll_params.cultivations,
                );
            }
            ClientRequestV1::RemainsLoot { remains_id, .. } => {
                // 权威校验（同 layer/dimension + 2.5m 范围 + 内容转移）全部在
                // `inventory::handle_remains_loot_intents` 里做；这里只做最基本的
                // 空字符串防御，真正的"遗骸存不存在/够不够得着"交给那个 system 判定。
                if remains_id.trim().is_empty() {
                    tracing::warn!(
                        "[bong][network] client_request remains_loot rejected: empty remains_id from {:?}",
                        ev.client
                    );
                } else {
                    dropped_loot_params
                        .remains_loot_tx
                        .send(crate::inventory::RemainsLootIntent {
                            entity: ev.client,
                            remains_id,
                        });
                }
            }
            ClientRequestV1::MineralProbe { x, y, z, .. } => {
                let position = valence::prelude::BlockPos::new(x, y, z);
                let Ok(player_position) = skill_scroll_params.positions.get(ev.client) else {
                    tracing::warn!(
                        "[bong][network] client_request mineral_probe rejected: entity={:?} has no Position",
                        ev.client
                    );
                    continue;
                };
                let player_pos = player_position.get();
                if !is_probe_target_in_range(player_pos, position) {
                    tracing::warn!(
                        "[bong][network] client_request mineral_probe rejected: entity={:?} pos=[{x},{y},{z}] out of range",
                        ev.client
                    );
                    continue;
                }
                let dimension = skill_scroll_params
                    .dimensions
                    .get(ev.client)
                    .map(|current| current.0)
                    .unwrap_or(DimensionKind::Overworld);
                tracing::info!(
                    "[bong][network] client_request mineral_probe entity={:?} pos=[{x},{y},{z}]",
                    ev.client
                );
                if let Some(mineral_probe_tx) = skill_scroll_params.mineral_probe_tx.as_deref_mut()
                {
                    mineral_probe_tx.send(MineralProbeIntent {
                        player: ev.client,
                        dimension,
                        position,
                    });
                }
            }
            ClientRequestV1::FreshnessProbe { instance_id, .. } => {
                if let Some(freshness_probe_tx) =
                    skill_scroll_params.freshness_probe_tx.as_deref_mut()
                {
                    // client 直接传来 instance_id，server 只需校验该 instance_id
                    // 确实属于该玩家 inventory（containers / equipped / hotbar 三处均扫）。
                    // 使用 inventory_item_by_instance_borrow 与 resolver（shelflife/probe.rs）保持一致，
                    // 避免 hotbar / 装备槽物品被 gate 误拒。
                    let belongs_to_player = inventories.get(ev.client).is_ok_and(|inv| {
                        inventory_item_by_instance_borrow(inv, instance_id).is_some()
                    });

                    if belongs_to_player {
                        tracing::info!(
                            "[bong][network] client_request freshness_probe entity={:?} instance_id={instance_id}",
                            ev.client
                        );
                        freshness_probe_tx.send(FreshnessProbeIntent {
                            player: ev.client,
                            instance_id,
                            issued_at_tick: combat_clock.tick,
                        });
                    } else {
                        tracing::warn!(
                            "[bong][network] client_request freshness_probe rejected: entity={:?} instance_id={instance_id} not found in player inventory",
                            ev.client
                        );
                    }
                }
            }
            ClientRequestV1::ApplyPill {
                instance_id,
                target,
                ..
            } => {
                handle_apply_pill(
                    ev.client,
                    instance_id,
                    target,
                    &mut commands,
                    combat_clock,
                    &mut inventories,
                    &mut clients,
                    &player_states,
                    &skill_scroll_params.cultivations,
                    &mut combat_params,
                    &mut dispatch.lifespan_extension_tx,
                    alchemy_params.vfx_events.as_deref_mut(),
                    &mut npc_engagement_params.audio_events,
                    // plan-fauna-stitched-beast-v1 P3 M1 修复：接通幻觉事件和叙事容器
                    alchemy_params.hallucination_events.as_deref_mut(),
                    alchemy_params.pending_narrations.as_deref_mut(),
                );
            }
            ClientRequestV1::SelfAntidote { instance_id, .. } => {
                if let Some(self_antidote_tx) = dispatch.self_antidote_tx.as_deref_mut() {
                    self_antidote_tx.send(SelfAntidoteIntent {
                        healer: ev.client,
                        target: ev.client,
                        antidote_instance_id: instance_id,
                        source: IntentSource::Client,
                        roll_override: None,
                    });
                }
            }
            ClientRequestV1::DuoSheRequest { target_id, .. } => {
                if let Some(duo_she_tx) = dispatch.duo_she_tx.as_deref_mut() {
                    duo_she_tx.send(DuoSheRequestEvent {
                        host: ev.client,
                        target_id,
                    });
                }
            }
            ClientRequestV1::QiColorInspect { observed, .. } => {
                let Some(observed_entity) = resolve_qi_color_inspect_target(
                    ev.client,
                    observed.as_str(),
                    &combat_params,
                    &skill_scroll_params.positions,
                    &skill_scroll_params.dimensions,
                ) else {
                    tracing::warn!(
                        "[bong][network] rejected qi_color_inspect from {:?}: invalid or out-of-scope observed `{observed}`",
                        ev.client
                    );
                    continue;
                };
                if let Some(qi_color_inspect_tx) = dispatch.qi_color_inspect_tx.as_deref_mut() {
                    qi_color_inspect_tx.send(QiColorInspectRequest {
                        observer: ev.client,
                        observed: observed_entity,
                        requested_at_tick: combat_clock.tick,
                    });
                }
            }
            ClientRequestV1::UseLifeCore { instance_id, .. } => {
                if let Some(life_core_tx) = dispatch.life_core_tx.as_deref_mut() {
                    life_core_tx.send(UseLifeCoreEvent {
                        entity: ev.client,
                        instance_id,
                    });
                }
            }
            ClientRequestV1::Jiemai { .. } => {
                tracing::info!(
                    "[bong][network] client_request jiemai entity={:?} tick={}",
                    ev.client,
                    combat_clock.tick
                );
                if let Some(defense_tx) = dispatch.defense_tx.as_deref_mut() {
                    defense_tx.send(DefenseIntent {
                        defender: ev.client,
                        issued_at_tick: ingress.combat_clock.tick,
                    });
                }
            }
            ClientRequestV1::ChargeCarrier {
                slot, qi_target, ..
            } => {
                if let Some(charge_carrier_tx) = dispatch.charge_carrier_tx.as_deref_mut() {
                    charge_carrier_tx.send(ChargeCarrierIntent {
                        carrier: ev.client,
                        slot: slot.map(map_anqi_carrier_slot),
                        qi_target: Some(qi_target),
                        issued_at_tick: combat_clock.tick,
                    });
                }
            }
            ClientRequestV1::ThrowCarrier {
                slot,
                dir_unit,
                power,
                ..
            } => {
                if let Some(throw_carrier_tx) = dispatch.throw_carrier_tx.as_deref_mut() {
                    throw_carrier_tx.send(ThrowCarrierIntent {
                        thrower: ev.client,
                        slot: map_anqi_carrier_slot(slot),
                        dir_unit,
                        power,
                        issued_at_tick: combat_clock.tick,
                    });
                }
            }
            ClientRequestV1::AnqiContainerSwitch { to, .. } => {
                let target_container = to.map(map_anqi_container_kind);
                let entity = ev.client;
                let tick = combat_clock.tick;
                commands.add(move |world: &mut bevy_ecs::world::World| {
                    let switched = if let Some(to) = target_container {
                        switch_container_slot(world, entity, to, tick)
                    } else {
                        cycle_container_slot(world, entity, tick)
                    };
                    if switched.is_none() {
                        // e2e fenglinghe 拒收护栏的正向证据：switch_container_slot 的
                        // 拒收早退（!allows_combat_swap，仅 fenglinghe）处发
                        // carrier 线缆 id 归属的 guard 标记。场景据此区分「拒收分支
                        // 被走」与「请求在 schema/反序列化/派发环节被丢」——单靠
                        // 无 container_swap 事件无法证明到达了 switch 系统（review
                        // finding [major]：fenglinghe 静默在请求未达 switch 系统时
                        // 照样通过）。经 GuardLogDedup 按 tick 窗口去重：恶意客户端
                        // 反复发同一拒收请求不制造无界日志，且窗口外自动剪除。
                        let wire_id = crate::combat::woliu::entity_wire_id(
                            world.get::<UniqueId>(entity),
                            entity,
                        );
                        let emit = world
                            .get_resource_mut::<crate::combat::guard_log::GuardLogDedup>()
                            .map(|mut g| g.should_emit(&wire_id, "rejected", tick))
                            .unwrap_or(true);
                        if emit {
                            tracing::info!(
                                "[bong][combat] container_switch guard carrier={} reason=rejected",
                                wire_id
                            );
                        }
                        tracing::warn!(
                            ?entity,
                            ?target_container,
                            tick,
                            "rejected anqi container switch request"
                        );
                    }
                });
            }
            ClientRequestV1::UseQuickSlot { slot, .. } => {
                handle_use_quick_slot(
                    ev.client,
                    slot,
                    combat_clock,
                    &mut commands,
                    &mut clients,
                    &mut combat_params,
                    alchemy_params.vfx_events.as_deref_mut(),
                    &inventories,
                );
            }
            ClientRequestV1::QuickSlotBind {
                slot,
                item_id,
                request_id,
                ..
            } => {
                let (quick_bindings, skillbar_bindings) = (
                    &mut combat_params.bindings_q,
                    &mut combat_params.skillbar_bindings_q,
                );
                handle_quick_slot_bind(
                    (ev.client, slot, item_id, request_id),
                    quick_bindings,
                    skillbar_bindings,
                    &inventories,
                    &mut clients,
                    (
                        &combat_params.item_registry,
                        persistence.as_deref(),
                        combat_clock,
                    ),
                );
            }
            ClientRequestV1::SkillBarCast { slot, target, .. } => {
                handle_skill_bar_cast(
                    ev.client,
                    slot,
                    target,
                    combat_clock,
                    &mut commands,
                    &mut clients,
                    &mut combat_params,
                    alchemy_params.vfx_events.as_deref_mut(),
                    &skill_scroll_params.known_techniques,
                );
            }
            ClientRequestV1::SkillBarBind { slot, binding, .. } => {
                handle_skill_bar_bind(
                    ev.client,
                    slot,
                    binding,
                    &mut combat_params.skillbar_bindings_q,
                    &inventories,
                    &clients,
                    persistence.as_deref(),
                    &skill_scroll_params.technique_registry,
                    &skill_scroll_params.known_techniques,
                );
            }
            ClientRequestV1::SkillConfigIntent {
                skill_id, config, ..
            } => {
                handle_skill_config_intent_request(
                    ev.client,
                    skill_id,
                    config,
                    &mut clients,
                    persistence.as_deref(),
                    &mut combat_params,
                );
            }
            // ── 灵田请求 ECS dispatch（plan-lingtian-v1 §1.2-§1.7）─────────
            ClientRequestV1::LingtianStartTill {
                x,
                y,
                z,
                hoe_instance_id,
                mode,
                ..
            } => {
                // fix-spec-1901-v2 §4.1 — producer 只入队：不读位置/维度，不读
                // chunk/terrain，不写 Start*Request；gate + terrain 派生都在
                // post-transfer validator（LingtianPostTransferValidationSet）做。
                lingtian_tx.pending.push(PendingLingtianRequest::Till {
                    actor: ev.client,
                    pos: valence::prelude::BlockPos::new(x, y, z),
                    hoe_instance_id,
                    mode: parse_session_mode(&mode),
                });
                tracing::info!(
                    "[bong][network] client_request lingtian_start_till entity={:?} pos=[{x},{y},{z}] hoe_inst={hoe_instance_id} mode={mode} queued",
                    ev.client
                );
            }
            ClientRequestV1::LingtianStartRenew {
                x,
                y,
                z,
                hoe_instance_id,
                ..
            } => {
                lingtian_tx.pending.push(PendingLingtianRequest::Renew {
                    actor: ev.client,
                    pos: valence::prelude::BlockPos::new(x, y, z),
                    hoe_instance_id,
                });
                tracing::info!(
                    "[bong][network] client_request lingtian_start_renew entity={:?} pos=[{x},{y},{z}] hoe_inst={hoe_instance_id} queued",
                    ev.client
                );
            }
            ClientRequestV1::LingtianStartPlanting {
                x, y, z, plant_id, ..
            } => {
                lingtian_tx.pending.push(PendingLingtianRequest::Planting {
                    actor: ev.client,
                    pos: valence::prelude::BlockPos::new(x, y, z),
                    plant_id: plant_id.clone(),
                });
                tracing::info!(
                    "[bong][network] client_request lingtian_start_planting entity={:?} pos=[{x},{y},{z}] plant_id={plant_id} queued",
                    ev.client
                );
            }
            ClientRequestV1::LingtianStartHarvest { x, y, z, mode, .. } => {
                lingtian_tx.pending.push(PendingLingtianRequest::Harvest {
                    actor: ev.client,
                    pos: valence::prelude::BlockPos::new(x, y, z),
                    mode: parse_session_mode(&mode),
                });
                tracing::info!(
                    "[bong][network] client_request lingtian_start_harvest entity={:?} pos=[{x},{y},{z}] mode={mode} queued",
                    ev.client
                );
            }
            ClientRequestV1::LingtianStartReplenish {
                x, y, z, source, ..
            } => {
                let Some(parsed) = parse_replenish_source(&source) else {
                    tracing::warn!(
                        "[bong][network] lingtian_start_replenish ignored: unknown source `{source}`"
                    );
                    continue;
                };
                lingtian_tx.pending.push(PendingLingtianRequest::Replenish {
                    actor: ev.client,
                    pos: valence::prelude::BlockPos::new(x, y, z),
                    source: parsed,
                });
                tracing::info!(
                    "[bong][network] client_request lingtian_start_replenish entity={:?} pos=[{x},{y},{z}] source={source} queued",
                    ev.client
                );
            }
            ClientRequestV1::LingtianStartDrainQi { x, y, z, .. } => {
                lingtian_tx.pending.push(PendingLingtianRequest::DrainQi {
                    actor: ev.client,
                    pos: valence::prelude::BlockPos::new(x, y, z),
                });
                tracing::info!(
                    "[bong][network] client_request lingtian_start_drain_qi entity={:?} pos=[{x},{y},{z}] queued",
                    ev.client
                );
            }
            // ─── 通用手搓（plan-craft-v1 P2） ────────────────────
            ClientRequestV1::CraftStart {
                recipe_id,
                quantity,
                ..
            } => {
                tracing::info!(
                    "[bong][network][craft] start entity={:?} recipe={recipe_id} quantity={quantity}",
                    ev.client,
                );
                if let Some(craft_start_tx) = dispatch.craft_start_tx.as_deref_mut() {
                    craft_start_tx.send(crate::craft::CraftStartIntent {
                        caster: ev.client,
                        recipe_id: crate::craft::RecipeId::new(recipe_id),
                        quantity,
                    });
                }
            }
            ClientRequestV1::CraftCancel { .. } => {
                tracing::info!("[bong][network][craft] cancel entity={:?}", ev.client);
                if let Some(craft_cancel_tx) = dispatch.craft_cancel_tx.as_deref_mut() {
                    craft_cancel_tx.send(crate::craft::CraftCancelIntent { caster: ev.client });
                }
            }
            // ── 垂死大能给丹（plan-dying-elder-v1 P1）─────────────────────────
            ClientRequestV1::GiveDanToElder {
                pill_instance_id,
                elder_entity_id,
                ..
            } => {
                tracing::info!(
                    "[bong][network][dying_elder] give_dan entity={:?} pill_instance_id={pill_instance_id} elder_entity_id={elder_entity_id}",
                    ev.client,
                );
                handle_give_dan_to_elder(
                    ev.client,
                    pill_instance_id,
                    elder_entity_id,
                    &mut inventories,
                    combat_params.entity_manager.as_deref(),
                    &mut clients,
                    dispatch.give_dan_to_elder_tx.as_deref_mut(),
                    &combat_params.positions,
                    &combat_params.dimensions,
                    &combat_params.dying_elder_targets,
                );
            }
            // ─── plan-agent-ui-data-v1 P0：天道 UI 面板响应 ─────────────
            // agent_ui.rs 的 receive_agent_ui_response_system 负责处理；
            // 此处仅记录 trace 并发出 AgentUiResponseEvent Bevy event。
            ClientRequestV1::AgentUiResponse {
                request_id,
                action,
                params,
                ..
            } => {
                tracing::debug!(
                    "[bong][agent_ui] AgentUiResponse received entity={:?} request_id={request_id}",
                    ev.client,
                );
                dispatch.agent_ui_response_tx.send(
                    crate::network::agent_ui::AgentUiResponseEvent {
                        player: ev.client,
                        request_id: request_id.clone(),
                        action: action.clone(),
                        params: params.clone(),
                    },
                );
            }
            _ => unreachable!(
                "session-domain request must be consumed before the legacy dispatch match"
            ),
        }
    }
}

fn parse_session_mode(raw: &str) -> SessionMode {
    match raw.to_ascii_lowercase().as_str() {
        "auto" => SessionMode::Auto,
        _ => SessionMode::Manual,
    }
}

fn parse_replenish_source(raw: &str) -> Option<ReplenishSource> {
    match raw.to_ascii_lowercase().as_str() {
        "zone" => Some(ReplenishSource::Zone),
        "bone_coin" => Some(ReplenishSource::BoneCoin),
        "beast_core" => Some(ReplenishSource::BeastCore),
        "ling_shui" => Some(ReplenishSource::LingShui),
        "pill_residue_failed_pill" | "failed_pill" => Some(ReplenishSource::PillResidue {
            residue_kind: crate::alchemy::residue::PillResidueKind::FailedPill,
        }),
        "pill_residue_flawed_pill" | "flawed_pill" => Some(ReplenishSource::PillResidue {
            residue_kind: crate::alchemy::residue::PillResidueKind::FlawedPill,
        }),
        "pill_residue_processing_dregs" | "processing_dregs" => {
            Some(ReplenishSource::PillResidue {
                residue_kind: crate::alchemy::residue::PillResidueKind::ProcessingDregs,
            })
        }
        "pill_residue_aging_scraps" | "aging_scraps" => Some(ReplenishSource::PillResidue {
            residue_kind: crate::alchemy::residue::PillResidueKind::AgingScraps,
        }),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_use_quick_slot(
    entity: valence::prelude::Entity,
    slot: u8,
    clock: &CombatClock,
    commands: &mut Commands,
    clients: &mut Query<(&Username, &mut Client)>,
    combat_params: &mut CombatRequestParams,
    vfx_events: Option<&mut Events<VfxEventRequest>>,
    inventories: &Query<&mut PlayerInventory>,
) {
    if slot as usize >= QuickSlotBindings::SLOT_COUNT {
        tracing::warn!(
            "[bong][network] use_quick_slot entity={entity:?} ignored: slot {slot} unavailable"
        );
        return;
    }
    // 契约顺序（未开放 / 无绑定 / 冷却 /
    // 同槽 cast 中 → 静默忽略）：与「异槽 cast 中 UserCancel + 启新」互斥的忽略
    // 条件必须**先行**判定。旧顺序先做 cast 闸门——未绑定/冷却中的请求会先打断
    // 进行中的异槽 cast 再被忽略（central-review 2012 #1 根因：use_quick_slot
    // 未绑定槽不得取消活动 cast，无绑定/冷却/实例缺失都不得扰动异槽 cast）。
    let (bound_instance_id, on_cooldown) = combat_params
        .bindings_q
        .get(entity)
        .ok()
        .map(|b| (b.get(slot), b.is_on_cooldown(slot, clock.tick)))
        .unwrap_or((None, false));
    if on_cooldown {
        tracing::debug!(
            "[bong][network] use_quick_slot entity={entity:?} slot={slot} ignored: on cooldown"
        );
        return;
    }
    let Some(instance_id) = bound_instance_id else {
        tracing::debug!(
            "[bong][network] use_quick_slot entity={entity:?} slot={slot} ignored: no binding"
        );
        return;
    };
    // 校验绑定的物品仍在背包内（player 可能拖出去了）。
    if let Ok(inv) = inventories.get(entity) {
        if !inventory_has_instance(inv, instance_id) {
            tracing::debug!(
                "[bong][network] use_quick_slot entity={entity:?} slot={slot} ignored: bound instance {instance_id} not in inventory"
            );
            return;
        }
    }
    // plan §4.2 cast 状态闸门：同槽 cast 中静默忽略；异槽 cast 中 UserCancel + 启新。
    if let Ok(prev) = combat_params.casting_q.get(entity) {
        if prev.source == CastSource::QuickSlot && prev.slot == slot {
            tracing::debug!(
                "[bong][network] use_quick_slot entity={entity:?} slot={slot} ignored: same-slot during cast"
            );
            return;
        }
        let prev = CastCancelSnapshot::from(prev);
        cancel_previous_cast(
            entity,
            prev,
            clock,
            commands,
            clients,
            combat_params,
            vfx_events,
            slot,
        );
        // 继续到下面启动新 cast。
    }
    // 取真实 cast_duration_ms / cooldown_ms：从背包找到 instance → template_id → registry。
    let (duration_ms, cooldown_ms) = inventories
        .get(entity)
        .ok()
        .and_then(|inv| inventory_template_id_by_instance(inv, instance_id))
        .and_then(|template_id| combat_params.item_registry.get(&template_id).cloned())
        .map(|t| (t.cast_duration_ms, t.cooldown_ms))
        .unwrap_or((TEMPLATE_DEFAULT_CAST_MS, TEMPLATE_DEFAULT_COOLDOWN_MS));
    // 按共享 tick 毫秒值换算；进 1 至少跑 1 tick，避免 0 时长 cast。
    let duration_ticks = u64::from(duration_ms)
        .div_ceil(crate::time::MILLIS_PER_TICK)
        .max(1);
    let complete_cooldown_ticks = u64::from(cooldown_ms)
        .div_ceil(crate::time::MILLIS_PER_TICK)
        .max(1);
    let started_at_ms = current_unix_millis();
    let start_position = combat_params
        .positions
        .get(entity)
        .map(|p| p.get())
        .unwrap_or(valence::prelude::DVec3::ZERO);
    commands.entity(entity).insert(Casting {
        source: CastSource::QuickSlot,
        slot,
        started_at_tick: clock.tick,
        duration_ticks,
        started_at_ms,
        duration_ms,
        bound_instance_id: Some(instance_id),
        start_position,
        complete_cooldown_ticks,
        skill_id: None,
        skill_config: None,
    });
    if let Ok((username, mut client)) = clients.get_mut(entity) {
        push_cast_sync(
            &mut client,
            CastSyncV1 {
                phase: CastPhaseV1::Casting,
                slot,
                duration_ms,
                started_at_ms,
                outcome: CastOutcomeV1::None,
            },
            username.0.as_str(),
            entity,
        );
    }
    tracing::info!(
        "[bong][network] cast started entity={entity:?} slot={slot} duration_ms={duration_ms} cooldown_ms={cooldown_ms} bound_instance={instance_id} tick={}",
        clock.tick
    );
}

fn inventory_has_instance(inv: &PlayerInventory, instance_id: u64) -> bool {
    for c in &inv.containers {
        if c.items
            .iter()
            .any(|p| p.instance.instance_id == instance_id)
        {
            return true;
        }
    }
    if inv
        .equipped
        .values()
        .flat_map(|s| s.iter_all())
        .any(|item| item.instance_id == instance_id)
    {
        return true;
    }
    inv.hotbar
        .iter()
        .flatten()
        .any(|item| item.instance_id == instance_id)
}

fn inventory_template_id_by_instance(inv: &PlayerInventory, instance_id: u64) -> Option<String> {
    for c in &inv.containers {
        if let Some(p) = c
            .items
            .iter()
            .find(|p| p.instance.instance_id == instance_id)
        {
            return Some(p.instance.template_id.clone());
        }
    }
    if let Some(item) = inv
        .equipped
        .values()
        .flat_map(|s| s.iter_all())
        .find(|item| item.instance_id == instance_id)
    {
        return Some(item.template_id.clone());
    }
    inv.hotbar
        .iter()
        .flatten()
        .find(|item| item.instance_id == instance_id)
        .map(|item| item.template_id.clone())
}

const EQUIPPED_QUICK_SLOT_LOOKUP_ORDER: [&str; 8] = [
    crate::inventory::EQUIP_SLOT_MAIN_HAND,
    crate::inventory::EQUIP_SLOT_OFF_HAND,
    crate::inventory::EQUIP_SLOT_EXTRA_HAND_0,
    crate::inventory::EQUIP_SLOT_EXTRA_HAND_1,
    crate::inventory::EQUIP_SLOT_HEAD,
    crate::inventory::EQUIP_SLOT_CHEST,
    crate::inventory::EQUIP_SLOT_LEGS,
    crate::inventory::EQUIP_SLOT_FEET,
];

fn inventory_instance_id_by_template(inv: &PlayerInventory, template: &str) -> Option<u64> {
    for c in &inv.containers {
        if let Some(p) = c.items.iter().find(|p| p.instance.template_id == template) {
            return Some(p.instance.instance_id);
        }
    }
    if let Some(item) = inv
        .hotbar
        .iter()
        .flatten()
        .find(|item| item.template_id == template)
    {
        return Some(item.instance_id);
    }
    EQUIPPED_QUICK_SLOT_LOOKUP_ORDER
        .iter()
        .filter_map(|slot| inv.equipped.get(*slot))
        .flat_map(|contents| contents.iter_all())
        .find(|item| item.template_id == template)
        .map(|item| item.instance_id)
}

fn handle_quick_slot_bind(
    request: (valence::prelude::Entity, u8, Option<String>, String),
    bindings_q: &mut Query<&mut QuickSlotBindings>,
    skillbar_bindings_q: &mut Query<&mut SkillBarBindings>,
    inventories: &Query<&mut PlayerInventory>,
    clients: &mut Query<(&Username, &mut Client)>,
    runtime: (&ItemRegistry, Option<&PlayerStatePersistence>, &CombatClock),
) {
    let (entity, slot, item_id, request_id) = request;
    let (item_registry, persistence, combat_clock) = runtime;
    if request_id.chars().count() == 0 || request_id.chars().count() > 128 {
        tracing::warn!(
            "[bong][network] quick_slot_bind entity={entity:?} rejected invalid request_id chars={}",
            request_id.chars().count()
        );
        return;
    }
    if slot as usize >= QuickSlotBindings::SLOT_COUNT {
        tracing::warn!("[bong][network] quick_slot_bind entity={entity:?} slot={slot} unavailable");
        send_quick_slot_bind_response(
            entity,
            request_id,
            false,
            bindings_q,
            inventories,
            item_registry,
            combat_clock,
            clients,
        );
        return;
    }
    let username = match clients.get_mut(entity) {
        Ok((username, _)) => username.0.clone(),
        Err(_) => {
            tracing::warn!(
                "[bong][network] quick_slot_bind entity={entity:?} rejected: missing client"
            );
            return;
        }
    };
    let requested_template = match item_id.as_deref() {
        Some("") => {
            tracing::warn!(
                "[bong][network] quick_slot_bind entity={entity:?} slot={slot} rejected: empty item_id string"
            );
            send_quick_slot_bind_response(
                entity,
                request_id,
                false,
                bindings_q,
                inventories,
                item_registry,
                combat_clock,
                clients,
            );
            return;
        }
        Some(template) => Some(template),
        None => None,
    };
    let instance_id = match requested_template {
        None => None,
        Some(template) => {
            let instance_id = inventories
                .get(entity)
                .ok()
                .and_then(|inventory| inventory_instance_id_by_template(inventory, template));
            let Some(instance_id) = instance_id else {
                tracing::warn!(
                    "[bong][network] quick_slot_bind entity={entity:?} slot={slot} rejected: item template `{template}` not in inventory"
                );
                send_quick_slot_bind_response(
                    entity,
                    request_id,
                    false,
                    bindings_q,
                    inventories,
                    item_registry,
                    combat_clock,
                    clients,
                );
                return;
            };
            if item_registry.get(template).is_none() {
                tracing::warn!(
                    "[bong][network] quick_slot_bind entity={entity:?} slot={slot} rejected: unknown item template `{template}`"
                );
                send_quick_slot_bind_response(
                    entity,
                    request_id,
                    false,
                    bindings_q,
                    inventories,
                    item_registry,
                    combat_clock,
                    clients,
                );
                return;
            }
            Some(instance_id)
        }
    };
    let mirror_block_to_skillbar = instance_id.is_some()
        && requested_template
            .and_then(|template| item_registry.get(template))
            .is_some_and(|template| template.category == ItemCategory::Block);
    let old_instance_id = match bindings_q.get_mut(entity) {
        Ok(bindings) => bindings.get(slot),
        Err(_) => {
            tracing::warn!(
                "[bong][network] quick_slot_bind entity={entity:?} rejected: missing QuickSlotBindings"
            );
            send_quick_slot_bind_response(
                entity,
                request_id,
                false,
                bindings_q,
                inventories,
                item_registry,
                combat_clock,
                clients,
            );
            return;
        }
    };
    let current_skill_slot = match skillbar_bindings_q.get_mut(entity) {
        Ok(bindings) => bindings.get(slot).cloned().unwrap_or_default(),
        Err(_) => {
            tracing::warn!(
                "[bong][network] quick_slot_bind entity={entity:?} rejected: missing SkillBarBindings"
            );
            send_quick_slot_bind_response(
                entity,
                request_id,
                false,
                bindings_q,
                inventories,
                item_registry,
                combat_clock,
                clients,
            );
            return;
        }
    };
    let clears_old_auto_mirror = old_instance_id.is_some_and(|old_instance_id| {
        current_skill_slot
            == SkillSlot::Item {
                instance_id: old_instance_id,
            }
            && (!mirror_block_to_skillbar || instance_id != Some(old_instance_id))
    });
    let desired_skill_slot = if mirror_block_to_skillbar {
        instance_id.map(|instance_id| SkillSlot::Item { instance_id })
    } else if clears_old_auto_mirror {
        Some(SkillSlot::Empty)
    } else {
        None
    };
    let persisted_item_id = requested_template.map(str::to_string);
    if let Some(persistence) = persistence {
        if let Err(error) = update_player_ui_prefs(persistence, username.as_str(), |prefs| {
            prefs.quick_slots[slot as usize] = persisted_item_id.clone();
            if mirror_block_to_skillbar {
                prefs.skill_bar[slot as usize] = crate::player::state::SkillSlotPersist::Item {
                    template_id: persisted_item_id.clone().unwrap_or_default(),
                };
            } else if clears_old_auto_mirror {
                prefs.skill_bar[slot as usize] = crate::player::state::SkillSlotPersist::Empty;
            }
        }) {
            tracing::warn!(
                "[bong][network] failed to persist quick_slot_bind for `{}` slot={slot}: {error}",
                username
            );
            send_quick_slot_bind_response(
                entity,
                request_id,
                false,
                bindings_q,
                inventories,
                item_registry,
                combat_clock,
                clients,
            );
            return;
        }
    }
    let mut bindings = bindings_q
        .get_mut(entity)
        .expect("quick-slot component was preflighted in the same system");
    let _ = bindings.set(slot, instance_id);
    if let Some(desired_skill_slot) = desired_skill_slot {
        let mut skillbar = skillbar_bindings_q
            .get_mut(entity)
            .expect("skill-bar component was preflighted in the same system");
        let _ = skillbar.set(slot, desired_skill_slot);
    }
    send_quick_slot_bind_response(
        entity,
        request_id.clone(),
        true,
        bindings_q,
        inventories,
        item_registry,
        combat_clock,
        clients,
    );
    tracing::info!(
        "[bong][network] quick_slot_bind entity={entity:?} slot={slot} request_id={} item_id={:?} → instance={:?} mirror_skillbar={mirror_block_to_skillbar} cleared_old_mirror={clears_old_auto_mirror}",
        request_id,
        item_id,
        instance_id
    );
}

#[allow(clippy::too_many_arguments)]
fn send_quick_slot_bind_response(
    entity: valence::prelude::Entity,
    request_id: String,
    accepted: bool,
    bindings_q: &mut Query<&mut QuickSlotBindings>,
    inventories: &Query<&mut PlayerInventory>,
    item_registry: &ItemRegistry,
    combat_clock: &CombatClock,
    clients: &mut Query<(&Username, &mut Client)>,
) {
    let config = {
        let bindings = bindings_q.get_mut(entity).ok();
        let inventory = inventories.get(entity).ok();
        build_quickslot_config(
            bindings.as_deref(),
            inventory,
            item_registry,
            combat_clock.tick,
            current_unix_millis_for_quickslot(),
            Some(request_id),
            Some(accepted),
        )
    };
    if let Ok((username, mut client)) = clients.get_mut(entity) {
        let username = username.0.clone();
        send_quickslot_config_to_client(&mut client, config, entity, username.as_str());
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_skill_bar_cast(
    entity: valence::prelude::Entity,
    slot: u8,
    target: Option<String>,
    clock: &CombatClock,
    commands: &mut Commands,
    clients: &mut Query<(&Username, &mut Client)>,
    combat_params: &mut CombatRequestParams,
    vfx_events: Option<&mut Events<VfxEventRequest>>,
    known_techniques: &Query<&mut KnownTechniques>,
) {
    if slot >= SkillBarBindings::SLOT_COUNT as u8 {
        tracing::warn!(
            "[bong][network] skill_bar_cast entity={entity:?} ignored: slot {slot} out of range"
        );
        return;
    }
    let bound_skill_id = combat_params
        .skillbar_bindings_q
        .get(entity)
        .ok()
        .and_then(|bindings| match bindings.get(slot) {
            Some(SkillSlot::Skill { skill_id }) => Some(skill_id.clone()),
            Some(SkillSlot::Item { .. }) | Some(SkillSlot::Empty) | None => None,
        });
    let Some(skill_id) = bound_skill_id else {
        tracing::warn!(
            "[bong][network] skill_bar_cast entity={entity:?} slot={slot} dropped: empty or item binding"
        );
        return;
    };
    let Some(definition) = combat_params.technique_registry.get(&skill_id).cloned() else {
        tracing::warn!(
            "[bong][network] skill_bar_cast entity={entity:?} slot={slot} dropped: unknown skill `{skill_id}`"
        );
        return;
    };
    // Ownership gate: reject if the player has not learned this technique.
    let player_has_technique = known_techniques
        .get(entity)
        .ok()
        .map(|kt| player_knows_technique(kt, &skill_id))
        .unwrap_or(false);
    if !player_has_technique {
        tracing::warn!(
            "[bong][network] skill_bar_cast entity={entity:?} slot={slot} skill={skill_id} \
             rejected: not in player KnownTechniques"
        );
        return;
    }

    // plan-race-system-v1 P3a（决议 §8.1 #5/#6）—— race gate：拥有门后、经脉门前。
    // 镜像 sword_path::skill_register::build_cast_context 的插入位置（该 resolver 路径
    // 独立于本通用路径，各自需要一份）。
    {
        let cultivation_race = combat_params
            .cultivations
            .get(entity)
            .map(|c| c.race.clone())
            .unwrap_or_else(|_| crate::body_plan::RaceId::new(crate::body_plan::HUMAN_RACE_ID));
        let intrinsic_is_humanoid = crate::body_plan::resolve_body_plan_for_target(
            entity,
            crate::body_plan::BodyPlanPurpose::Intrinsic,
            crate::body_plan::BodyPlanResolveInputs {
                cultivation: combat_params.cultivations.get(entity).ok(),
                beast_kind: None,
                morph_state: None,
            },
            combat_params.body_plans.as_deref(),
            combat_params.race_registry.as_deref(),
        )
        .is_humanoid;
        if !definition
            .required_race
            .allows(&cultivation_race, intrinsic_is_humanoid)
        {
            tracing::warn!(
                "[bong][network] skill_bar_cast entity={entity:?} slot={slot} skill={skill_id} \
                 rejected: race gate (RaceGate::allows returned false)"
            );
            if let Ok((username, mut client)) = clients.get_mut(entity) {
                push_cast_sync(
                    &mut client,
                    CastSyncV1 {
                        phase: CastPhaseV1::Idle,
                        slot,
                        duration_ms: 0,
                        started_at_ms: current_unix_millis(),
                        outcome: CastOutcomeV1::RejectRaceMismatch,
                    },
                    username.0.as_str(),
                    entity,
                );
            }
            return;
        }
    }

    // plan-race-system-v1 P4 —— 易形类技能（`morph.yixing`）专属前置门：race gate 后、
    // 通用经脉门前。判据是本体（Intrinsic）`MeridianProfile` 内全部 `FormAnchor` 经脉
    // 已通且未断（见 `body_plan::form_anchors_open`），与 `learn_technique_if_allowed`
    // 的习得门共用同一判据函数，保持"能学就能放、不能放就不该学"的一致性。
    if crate::body_plan::technique_requires_form_anchor(&skill_id) {
        let meridians_ok = combat_params.meridians.get(entity).ok();
        let severed = combat_params.player_severed.get(entity).ok().flatten();
        let intrinsic_plan = crate::body_plan::resolve_body_plan_for_target(
            entity,
            crate::body_plan::BodyPlanPurpose::Intrinsic,
            crate::body_plan::BodyPlanResolveInputs {
                cultivation: combat_params.cultivations.get(entity).ok(),
                beast_kind: None,
                morph_state: None,
            },
            combat_params.body_plans.as_deref(),
            combat_params.race_registry.as_deref(),
        );
        let anchors_ok = meridians_ok
            .zip(intrinsic_plan.meridian_profile.as_ref())
            .is_some_and(|(meridians, profile)| {
                crate::body_plan::form_anchors_open(profile, meridians, severed)
            });
        if !anchors_ok {
            tracing::warn!(
                "[bong][network] skill_bar_cast entity={entity:?} slot={slot} skill={skill_id} \
                 rejected: form anchor gate closed (FormAnchor channels not fully open/unsevered)"
            );
            if let Ok((username, mut client)) = clients.get_mut(entity) {
                push_cast_sync(
                    &mut client,
                    CastSyncV1 {
                        phase: CastPhaseV1::Idle,
                        slot,
                        duration_ms: 0,
                        started_at_ms: current_unix_millis(),
                        outcome: CastOutcomeV1::MeridianGated,
                    },
                    username.0.as_str(),
                    entity,
                );
            }
            return;
        }
    }

    let skill_fn = combat_params
        .skill_registry
        .as_deref()
        .and_then(|registry| registry.lookup(&skill_id));
    if combat_params
        .skillbar_bindings_q
        .get(entity)
        .map(|bindings| bindings.is_on_cooldown(&skill_id, clock.tick))
        .unwrap_or(false)
    {
        tracing::debug!(
            "[bong][network] skill_bar_cast entity={entity:?} slot={slot} skill={skill_id} ignored: on cooldown"
        );
        return;
    }

    if let Err(reason) =
        validate_skill_config_before_cast(&skill_id, entity, clients, combat_params)
    {
        tracing::warn!(
            "[bong][network] skill_bar_cast entity={entity:?} slot={slot} skill={skill_id} rejected: missing or invalid SkillConfig ({reason:?})"
        );
        return;
    }

    // ─── 经脉门控：覆盖 resolver + generic 两条 cast 路径 ────────────────────────
    // 在 cancel-previous-cast 之前检查，拒绝时不打断已在施放的其他招式。
    {
        let meridians_ok = combat_params.meridians.get(entity).ok();
        let severed = combat_params.player_severed.get(entity).ok().flatten();
        let deps_table = combat_params.skill_meridian_deps.as_deref();
        if let Some(meridians) = meridians_ok {
            if let Err(blocked) = check_player_skill_meridian_gate(
                &skill_id,
                &definition.required_meridians,
                meridians,
                severed,
                deps_table,
            ) {
                tracing::warn!(
                    "[bong][network] skill_bar_cast entity={entity:?} slot={slot} skill={skill_id} \
                     rejected: meridian gate blocked by {blocked:?}"
                );
                if let Ok((username, mut client)) = clients.get_mut(entity) {
                    push_cast_sync(
                        &mut client,
                        CastSyncV1 {
                            // 施放前被拒：没有进行中的 cast，Idle 语义正确。
                            // Interrupt 语义表示"打断进行中 cast"，此处不适用。
                            phase: CastPhaseV1::Idle,
                            slot,
                            duration_ms: 0,
                            started_at_ms: current_unix_millis(),
                            outcome: CastOutcomeV1::MeridianGated,
                        },
                        username.0.as_str(),
                        entity,
                    );
                }
                return;
            }
        }
        // MeridianSystem component 缺失（pre-init 玩家 / entity 无经脉）→ 放行
    }

    if let Ok(prev) = combat_params.casting_q.get(entity) {
        if prev.source == CastSource::SkillBar && prev.slot == slot {
            tracing::debug!(
                "[bong][network] skill_bar_cast entity={entity:?} slot={slot} ignored: same-slot during cast"
            );
            return;
        }
        let prev = CastCancelSnapshot::from(prev);
        cancel_previous_cast(
            entity,
            prev,
            clock,
            commands,
            clients,
            combat_params,
            vfx_events,
            slot,
        );
    }

    let resolved_target = resolve_skill_cast_target(target.as_deref(), combat_params);
    if let Some(skill_fn) = skill_fn {
        let command_target = resolved_target;
        commands.add(move |world: &mut bevy_ecs::world::World| {
            match skill_fn(world, entity, slot, command_target) {
                CastResult::Started {
                    cooldown_ticks,
                    anim_duration_ticks,
                } => {
                    push_skill_cast_started_sync(world, entity, slot);
                    tracing::info!(
                        "[bong][network] skill resolver started entity={entity:?} slot={slot} cooldown_ticks={cooldown_ticks} anim_duration_ticks={anim_duration_ticks}"
                    );
                }
                CastResult::Rejected { reason } => {
                    tracing::debug!(
                        "[bong][network] skill resolver rejected entity={entity:?} slot={slot} reason={reason:?}"
                    );
                    // 通用技能警示 HUD：把 resolver 拒绝原因推回施法者 client。
                    // 纯反馈——cast 已被上面的 resolver 逻辑拒绝，这里只让玩家看到原因。
                    push_skill_cast_rejected_sync(world, entity, slot, reason);
                }
                CastResult::Interrupted => {
                    tracing::debug!(
                        "[bong][network] skill resolver interrupted entity={entity:?} slot={slot}"
                    );
                }
            }
        });
    } else {
        start_generic_skillbar_cast(
            entity,
            slot,
            &skill_id,
            &definition,
            clock,
            commands,
            clients,
            combat_params,
        );
    }
    tracing::info!(
        "[bong][network] skill cast queued entity={entity:?} slot={slot} skill={skill_id} target={target:?} resolved_target={resolved_target:?} duration_ticks={} cooldown_ticks={} tick={}",
        definition.cast_ticks,
        definition.cooldown_ticks,
        clock.tick
    );
}

fn validate_skill_config_before_cast(
    skill_id: &str,
    entity: valence::prelude::Entity,
    clients: &mut Query<(&Username, &mut Client)>,
    combat_params: &CombatRequestParams,
) -> Result<(), SkillConfigRejectReason> {
    let Some(schemas) = combat_params.skill_config_schemas.as_deref() else {
        return Err(SkillConfigRejectReason::SchemaUnavailable);
    };
    if schemas.get(skill_id).is_none() {
        return Ok(());
    }
    let Ok((username, _)) = clients.get_mut(entity) else {
        return Err(SkillConfigRejectReason::UnknownSkill);
    };
    let Some(store) = combat_params.skill_config_store.as_deref() else {
        return Err(SkillConfigRejectReason::StoreUnavailable);
    };
    let player_id = canonical_player_id(username.0.as_str());
    let Some(config) = store.config_for(player_id.as_str(), skill_id) else {
        return Err(SkillConfigRejectReason::MissingRequiredField(
            "config".to_string(),
        ));
    };
    validate_skill_config(skill_id, config.fields.clone(), schemas).map(|_| ())
}

#[allow(clippy::too_many_arguments)]
fn start_generic_skillbar_cast(
    entity: valence::prelude::Entity,
    slot: u8,
    skill_id: &str,
    definition: &TechniqueDefinition,
    clock: &CombatClock,
    commands: &mut Commands,
    clients: &mut Query<(&Username, &mut Client)>,
    combat_params: &CombatRequestParams,
) {
    let duration_ticks = u64::from(definition.cast_ticks).max(1);
    let complete_cooldown_ticks = u64::from(definition.cooldown_ticks).max(1);
    let duration_ms = definition
        .cast_ticks
        .saturating_mul(crate::time::MILLIS_PER_TICK as u32);
    let started_at_ms = current_unix_millis();
    let start_position = combat_params
        .positions
        .get(entity)
        .map(|position| position.get())
        .unwrap_or(valence::prelude::DVec3::ZERO);
    let skill_config = clients.get_mut(entity).ok().and_then(|(username, _)| {
        let player_id = canonical_player_id(username.0.as_str());
        skill_config_snapshot_for_cast(
            combat_params.skill_config_store.as_deref(),
            player_id.as_str(),
            skill_id,
        )
    });
    commands.entity(entity).insert(Casting {
        source: CastSource::SkillBar,
        slot,
        started_at_tick: clock.tick,
        duration_ticks,
        started_at_ms,
        duration_ms,
        bound_instance_id: None,
        start_position,
        complete_cooldown_ticks,
        skill_id: Some(skill_id.to_string()),
        skill_config,
    });
    if let Ok((username, mut client)) = clients.get_mut(entity) {
        push_cast_sync(
            &mut client,
            CastSyncV1 {
                phase: CastPhaseV1::Casting,
                slot,
                duration_ms,
                started_at_ms,
                outcome: CastOutcomeV1::None,
            },
            username.0.as_str(),
            entity,
        );
    }
}

fn resolve_skill_cast_target(
    raw: Option<&str>,
    combat_params: &CombatRequestParams,
) -> Option<Entity> {
    let raw = raw?.trim();
    if raw.is_empty() {
        return None;
    }
    if let Some(id) = raw.strip_prefix("entity:") {
        let protocol_id = id.parse::<i32>().ok()?;
        return combat_params
            .entity_manager
            .as_deref()
            .and_then(|manager| manager.get_by_id(protocol_id));
    }
    let id = raw.strip_prefix("entity_bits:")?;
    id.parse::<u64>()
        .ok()
        .and_then(|bits| Entity::try_from_bits(bits).ok())
}

fn guardian_kind_from_schema(kind: GuardianKindV1) -> crate::social::components::GuardianKind {
    match kind {
        GuardianKindV1::Puppet => crate::social::components::GuardianKind::Puppet,
        GuardianKindV1::ZhenfaTrap => crate::social::components::GuardianKind::ZhenfaTrap,
        GuardianKindV1::BondedDaoxiang => crate::social::components::GuardianKind::BondedDaoxiang,
    }
}

fn map_anqi_carrier_slot(slot: crate::schema::client_request::AnqiCarrierSlotV1) -> CarrierSlot {
    match slot {
        crate::schema::client_request::AnqiCarrierSlotV1::MainHand => CarrierSlot::MainHand,
        crate::schema::client_request::AnqiCarrierSlotV1::OffHand => CarrierSlot::OffHand,
    }
}

fn map_anqi_container_kind(
    kind: crate::schema::combat_carrier::AnqiContainerKindV1,
) -> AnqiContainerKind {
    match kind {
        crate::schema::combat_carrier::AnqiContainerKindV1::HandSlot => AnqiContainerKind::HandSlot,
        crate::schema::combat_carrier::AnqiContainerKindV1::Quiver => AnqiContainerKind::Quiver,
        crate::schema::combat_carrier::AnqiContainerKindV1::PocketPouch => {
            AnqiContainerKind::PocketPouch
        }
        crate::schema::combat_carrier::AnqiContainerKindV1::Fenglinghe => {
            AnqiContainerKind::Fenglinghe
        }
    }
}

fn resolve_qi_color_inspect_target(
    observer: Entity,
    raw: &str,
    combat_params: &CombatRequestParams,
    positions: &Query<&valence::prelude::Position>,
    dimensions: &Query<&CurrentDimension>,
) -> Option<Entity> {
    let protocol_id = parse_qi_color_inspect_protocol_id(raw)?;
    let observed = combat_params
        .entity_manager
        .as_deref()
        .and_then(|manager| manager.get_by_id(protocol_id))?;
    is_qi_color_inspect_target_in_scope(observer, observed, positions, dimensions)
        .then_some(observed)
}

fn parse_qi_color_inspect_protocol_id(raw: &str) -> Option<i32> {
    raw.trim().strip_prefix("entity:")?.parse().ok()
}

fn is_qi_color_inspect_target_in_scope(
    observer: Entity,
    observed: Entity,
    positions: &Query<&valence::prelude::Position>,
    dimensions: &Query<&CurrentDimension>,
) -> bool {
    if observer == observed {
        return false;
    }
    let Ok(observer_position) = positions.get(observer) else {
        return false;
    };
    let Ok(observed_position) = positions.get(observed) else {
        return false;
    };
    let observer_dimension = dimension_kind_for(dimensions, observer);
    let observed_dimension = dimension_kind_for(dimensions, observed);
    is_qi_color_inspect_position_in_scope(
        observer_position.get(),
        observed_position.get(),
        observer_dimension == observed_dimension,
    )
}

fn is_qi_color_inspect_position_in_scope(
    observer_position: DVec3,
    observed_position: DVec3,
    same_dimension: bool,
) -> bool {
    same_dimension
        && crate::reach::DistanceRule::NEARBY_INTERACT.allows(observer_position, observed_position)
}

fn dimension_kind_for(dimensions: &Query<&CurrentDimension>, entity: Entity) -> DimensionKind {
    dimensions
        .get(entity)
        .map(|dimension| dimension.0)
        .unwrap_or_default()
}

fn dying_elder_can_receive_dan(state: &crate::fauna::dying_elder::DyingElderState) -> bool {
    match state {
        crate::fauna::dying_elder::DyingElderState::Plea => true,
        crate::fauna::dying_elder::DyingElderState::Recovering { dan_received } => {
            *dan_received < crate::fauna::dying_elder::DYING_ELDER_DAN_THRESHOLD
        }
        crate::fauna::dying_elder::DyingElderState::Betrayal
        | crate::fauna::dying_elder::DyingElderState::Dead { .. } => false,
    }
}

fn is_give_dan_target_in_scope(
    player_position: DVec3,
    elder_position: DVec3,
    player_dimension: DimensionKind,
    elder_dimension: DimensionKind,
) -> bool {
    player_dimension == elder_dimension
        && crate::reach::DistanceRule::NEARBY_INTERACT.allows(player_position, elder_position)
}

fn reject_give_dan_target(
    clients: &mut Query<(&Username, &mut Client)>,
    player_entity: Entity,
    message: &'static str,
) {
    if let Ok((_username, mut client)) = clients.get_mut(player_entity) {
        client.send_chat_message(message);
    }
}

/// 通用技能警示：resolver-path 施法被拒时把拒绝原因推回施法者 client。
///
/// 走与经脉门控拒绝完全相同的 `CastSyncV1{phase: Idle, outcome: Reject*}` 形态
/// （施放前被拒，没有进行中 cast，Idle 语义正确；client 据 `outcome != None` 弹警示）。
/// 复用既有 `push_cast_sync` / server_data CastSync 通道，不新增 S2C 变体。
/// 纯反馈：cast 已被 resolver 既有逻辑拒绝，本函数只负责"显示原因"，不改施法结果。
fn push_skill_cast_rejected_sync(
    world: &mut bevy_ecs::world::World,
    entity: Entity,
    slot: u8,
    reason: CastRejectReason,
) {
    let username = world
        .get::<Username>(entity)
        .map(|username| username.0.clone())
        .unwrap_or_else(|| format!("entity:{:?}", entity));
    let started_at_ms = current_unix_millis();
    let Some(mut client) = world.get_mut::<Client>(entity) else {
        return;
    };
    push_cast_sync(
        &mut client,
        CastSyncV1 {
            phase: CastPhaseV1::Idle,
            slot,
            duration_ms: 0,
            started_at_ms,
            outcome: reason.to_cast_outcome(),
        },
        username.as_str(),
        entity,
    );
}

fn push_skill_cast_started_sync(world: &mut bevy_ecs::world::World, entity: Entity, slot: u8) {
    let Some(casting) = world.get::<Casting>(entity).cloned() else {
        return;
    };
    let username = world
        .get::<Username>(entity)
        .map(|username| username.0.clone())
        .unwrap_or_else(|| format!("entity:{:?}", entity));
    let Some(mut client) = world.get_mut::<Client>(entity) else {
        return;
    };
    push_cast_sync(
        &mut client,
        CastSyncV1 {
            phase: CastPhaseV1::Casting,
            slot,
            duration_ms: casting.duration_ms,
            started_at_ms: casting.started_at_ms,
            outcome: CastOutcomeV1::None,
        },
        username.as_str(),
        entity,
    );
}

#[allow(clippy::too_many_arguments)]
fn cancel_previous_cast(
    entity: valence::prelude::Entity,
    prev: CastCancelSnapshot,
    clock: &CombatClock,
    commands: &mut Commands,
    clients: &mut Query<(&Username, &mut Client)>,
    combat_params: &mut CombatRequestParams,
    vfx_events: Option<&mut Events<VfxEventRequest>>,
    next_slot: u8,
) {
    let prev_source = prev.source;
    let prev_slot = prev.slot;
    // plan-skill-anim-fidelity-v1 P4（review r1 修）——用户主动切槽取消是
    // `tick_casts_or_interrupt` 三打断分支之外的**第四条**退出路径：Casting 在此
    // 被提前 remove，那边再也看不到它，不补发 StopAnim 循环蓄力段就永卡客户端
    // （yidao 引导窗长达 60s，命中概率远高于 sword.infuse）。
    if let (Some(vfx_events), Ok(unique_id), Ok(position)) = (
        vfx_events,
        combat_params.unique_ids.get(entity),
        combat_params.positions.get(entity),
    ) {
        if let Some(request) = crate::network::cast_emit::cast_loop_stop_anim_request(
            prev.skill_id.as_deref(),
            unique_id,
            position.get(),
            crate::network::cast_emit::CAST_LOOP_ANIM_CANCEL_FADE_OUT_TICKS,
        ) {
            vfx_events.send(request);
        }
    }
    commands.entity(entity).remove::<Casting>();
    match prev_source {
        CastSource::QuickSlot => {
            if let Ok(mut bindings) = combat_params.bindings_q.get_mut(entity) {
                bindings.set_cooldown(
                    prev_slot,
                    clock.tick.saturating_add(CAST_INTERRUPT_COOLDOWN_TICKS),
                );
            }
        }
        // bughunt skillbar-rebind-cooldown-reset：SkillBarBindings 冷却按 skill_id
        // 记账，用户主动切槽取消时同理必须用被取消 cast 的 skill_id（而非 slot）
        // 写入冷却。缺 skill_id 是理论不可达的防御性分支（所有 SkillBar Casting
        // 构造点都填了该字段）。
        CastSource::SkillBar => {
            if let Some(skill_id) = prev.skill_id.as_deref() {
                if let Ok(mut bindings) = combat_params.skillbar_bindings_q.get_mut(entity) {
                    bindings.set_cooldown(
                        skill_id,
                        clock.tick.saturating_add(CAST_INTERRUPT_COOLDOWN_TICKS),
                    );
                }
            } else {
                tracing::warn!(
                    "[bong][network][cast] cancel_previous_cast: SkillBar prev Casting 缺 \
                     skill_id (slot={prev_slot})，无法写入冷却"
                );
            }
        }
    }
    if let Ok((username, mut client)) = clients.get_mut(entity) {
        push_cast_sync(
            &mut client,
            CastSyncV1 {
                phase: CastPhaseV1::Interrupt,
                slot: prev_slot,
                duration_ms: prev.duration_ms,
                started_at_ms: prev.started_at_ms,
                outcome: CastOutcomeV1::UserCancel,
            },
            username.0.as_str(),
            entity,
        );
    }
    tracing::info!(
        "[bong][network][cast] user_cancel entity={entity:?} prev_source={prev_source:?} prev_slot={prev_slot} → switching to slot={next_slot}"
    );
}

#[derive(Debug, Clone)]
struct CastCancelSnapshot {
    source: CastSource,
    slot: u8,
    duration_ms: u32,
    started_at_ms: u64,
    /// plan-skill-anim-fidelity-v1 P4：用户主动取消也要走停止路径（§13 #6），
    /// 循环蓄力段的 `StopAnim` 需要按 skill_id 查表，故快照带上它。
    skill_id: Option<String>,
}

impl From<&Casting> for CastCancelSnapshot {
    fn from(casting: &Casting) -> Self {
        Self {
            source: casting.source,
            slot: casting.slot,
            duration_ms: casting.duration_ms,
            started_at_ms: casting.started_at_ms,
            skill_id: casting.skill_id.clone(),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_skill_config_intent_request(
    entity: valence::prelude::Entity,
    skill_id: String,
    config: std::collections::BTreeMap<String, serde_json::Value>,
    clients: &mut Query<(&Username, &mut Client)>,
    persistence: Option<&PlayerStatePersistence>,
    combat_params: &mut CombatRequestParams,
) {
    let Ok((username, _)) = clients.get_mut(entity) else {
        tracing::warn!(
            "[bong][network] skill_config_intent entity={entity:?} rejected: missing client username"
        );
        return;
    };
    let username = username.0.clone();
    let player_id = canonical_player_id(username.as_str());
    let current_casting = combat_params.casting_q.get(entity).ok().cloned();
    let Some(schemas) = combat_params.skill_config_schemas.as_deref() else {
        let snapshot = combat_params
            .skill_config_store
            .as_deref()
            .map(|store| store.snapshot_for_player(player_id.as_str()))
            .unwrap_or_else(empty_skill_config_snapshot);
        send_authoritative_skill_config_snapshot(clients, entity, username.as_str(), snapshot);
        tracing::warn!(
            "[bong][network] skill_config_intent entity={entity:?} skill={skill_id} rejected: schema resource missing"
        );
        return;
    };
    let Some(store) = combat_params.skill_config_store.as_deref_mut() else {
        send_authoritative_skill_config_snapshot(
            clients,
            entity,
            username.as_str(),
            empty_skill_config_snapshot(),
        );
        tracing::warn!(
            "[bong][network] skill_config_intent entity={entity:?} skill={skill_id} rejected: store resource missing"
        );
        return;
    };
    let snapshot = match handle_config_intent(
        &combat_params.technique_registry,
        player_id.as_str(),
        skill_id.as_str(),
        config,
        current_casting.as_ref(),
        store,
        schemas,
    ) {
        Ok(snapshot) => snapshot,
        Err(reason) => {
            tracing::warn!(
                "[bong][network] skill_config_intent entity={entity:?} skill={skill_id} rejected: {reason:?}"
            );
            let snapshot = store.snapshot_for_player(player_id.as_str());
            send_authoritative_skill_config_snapshot(clients, entity, username.as_str(), snapshot);
            return;
        }
    };

    if let Some(persistence) = persistence {
        if let Err(error) = update_player_ui_prefs(persistence, username.as_str(), |prefs| {
            prefs.skill_configs = snapshot.configs.clone();
        }) {
            tracing::warn!(
                "[bong][network] failed to persist skill_config_intent for `{}` skill={skill_id}: {error}",
                username
            );
        }
    }
    send_authoritative_skill_config_snapshot(clients, entity, username.as_str(), snapshot.clone());
    tracing::info!(
        "[bong][network] skill_config_intent entity={entity:?} skill={skill_id} configs={}",
        snapshot.configs.len()
    );
}

fn empty_skill_config_snapshot() -> SkillConfigSnapshot {
    SkillConfigSnapshot {
        configs: Default::default(),
    }
}

fn send_authoritative_skill_config_snapshot(
    clients: &mut Query<(&Username, &mut Client)>,
    entity: valence::prelude::Entity,
    username: &str,
    snapshot: SkillConfigSnapshot,
) {
    if let Ok((_, mut client)) = clients.get_mut(entity) {
        send_skill_config_snapshot_to_client(&mut client, snapshot, entity, username);
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_skill_bar_bind(
    entity: valence::prelude::Entity,
    slot: u8,
    binding: Option<SkillBarBindingV1>,
    bindings_q: &mut Query<&mut SkillBarBindings>,
    inventories: &Query<&mut PlayerInventory>,
    clients: &Query<(&Username, &mut Client)>,
    persistence: Option<&PlayerStatePersistence>,
    technique_registry: &TechniqueRegistry,
    known_techniques: &Query<&mut KnownTechniques>,
) {
    if slot >= SkillBarBindings::SLOT_COUNT as u8 {
        tracing::warn!("[bong][network] skill_bar_bind entity={entity:?} slot={slot} out of range");
        return;
    }
    let slot_value = match binding.as_ref() {
        None => SkillSlot::Empty,
        Some(SkillBarBindingV1::Item { template_id }) => {
            let instance_id = inventories
                .get(entity)
                .ok()
                .and_then(|inventory| first_instance_for_template(inventory, template_id));
            let Some(instance_id) = instance_id else {
                tracing::warn!(
                    "[bong][network] skill_bar_bind entity={entity:?} slot={slot} rejected: item template `{template_id}` not in inventory"
                );
                return;
            };
            SkillSlot::Item { instance_id }
        }
        Some(SkillBarBindingV1::Skill { skill_id }) => {
            if technique_registry.get(skill_id).is_none() {
                tracing::warn!(
                    "[bong][network] skill_bar_bind entity={entity:?} slot={slot} rejected: unknown skill `{skill_id}`"
                );
                return;
            }
            // Ownership gate: reject if the player has not learned this technique.
            let player_has_technique = known_techniques
                .get(entity)
                .ok()
                .map(|kt| player_knows_technique(kt, skill_id))
                .unwrap_or(false);
            if !player_has_technique {
                tracing::warn!(
                    "[bong][network] skill_bar_bind entity={entity:?} slot={slot} rejected: \
                     technique `{skill_id}` not in player KnownTechniques"
                );
                return;
            }
            SkillSlot::Skill {
                skill_id: skill_id.clone(),
            }
        }
    };
    let mut bindings = match bindings_q.get_mut(entity) {
        Ok(bindings) => bindings,
        Err(_) => {
            tracing::warn!(
                "[bong][network] skill_bar_bind entity={entity:?} has no SkillBarBindings"
            );
            return;
        }
    };
    if !bindings.set(slot, slot_value.clone()) {
        tracing::warn!("[bong][network] skill_bar_bind entity={entity:?} slot={slot} out of range");
        return;
    }
    if let (Some(persistence), Ok((username, _))) = (persistence, clients.get(entity)) {
        if let Err(error) = update_player_ui_prefs(persistence, username.0.as_str(), |prefs| {
            prefs.skill_bar[slot as usize] = binding_to_persist(binding.clone())
        }) {
            tracing::warn!(
                "[bong][network] failed to persist skill_bar_bind for `{}` slot={slot}: {error}",
                username.0
            );
        }
    }
    tracing::info!(
        "[bong][network] skill_bar_bind entity={entity:?} slot={slot} binding={binding:?} → {slot_value:?}"
    );
}

fn binding_to_persist(
    binding: Option<SkillBarBindingV1>,
) -> crate::player::state::SkillSlotPersist {
    match binding {
        None => crate::player::state::SkillSlotPersist::Empty,
        Some(SkillBarBindingV1::Item { template_id }) => {
            crate::player::state::SkillSlotPersist::Item { template_id }
        }
        Some(SkillBarBindingV1::Skill { skill_id }) => {
            crate::player::state::SkillSlotPersist::Skill { skill_id }
        }
    }
}

/// Returns true if the player has `skill_id` in their KnownTechniques with `active == true`.
/// Used as the ownership gate in both skill_bar_bind and skill_bar_cast paths.
fn player_knows_technique(known: &KnownTechniques, skill_id: &str) -> bool {
    known.entries.iter().any(|e| e.id == skill_id && e.active)
}

fn first_instance_for_template(inventory: &PlayerInventory, template_id: &str) -> Option<u64> {
    for container in &inventory.containers {
        if let Some(placed) = container
            .items
            .iter()
            .find(|placed| placed.instance.template_id == template_id)
        {
            return Some(placed.instance.instance_id);
        }
    }
    if let Some(item) = inventory
        .hotbar
        .iter()
        .flatten()
        .find(|item| item.template_id == template_id)
    {
        return Some(item.instance_id);
    }
    inventory
        .equipped
        .values()
        .flat_map(|s| s.iter_all())
        .find(|item| item.template_id == template_id)
        .map(|item| item.instance_id)
}

fn find_inventory_instance_location(
    inventory: &PlayerInventory,
    instance_id: u64,
) -> Option<InventoryLocationV1> {
    for container in &inventory.containers {
        if let Some(placed) = container
            .items
            .iter()
            .find(|placed| placed.instance.instance_id == instance_id)
        {
            let container_id = container_id_v1_for_runtime(container.id.as_str())?;
            return Some(InventoryLocationV1::Container {
                container_id,
                row: u64::from(placed.row),
                col: u64::from(placed.col),
            });
        }
    }

    if let Some(loc) = crate::inventory::find_equipped_instance(inventory, instance_id) {
        use crate::inventory::EquippedInstanceLoc;
        let (slot_key, state) = match loc {
            EquippedInstanceLoc::Worn { slot, .. } => (slot, EquipStateV1::Worn),
            EquippedInstanceLoc::Held { slot } => (slot, EquipStateV1::Held),
        };
        return equip_slot_v1_for_runtime(&slot_key)
            .map(|slot| InventoryLocationV1::Equip { slot, state });
    }

    inventory
        .hotbar
        .iter()
        .enumerate()
        .find_map(|(index, item)| {
            item.as_ref()
                .filter(|item| item.instance_id == instance_id)
                .map(|_| InventoryLocationV1::Hotbar { index: index as u8 })
        })
}

fn container_id_v1_for_runtime(id: &str) -> Option<ContainerIdV1> {
    // plan-backpack-equip-v1 P1 — ContainerIdV1 is now an open String alias;
    // any non-empty container id maps 1:1 to its wire representation.
    if id.is_empty() {
        None
    } else {
        Some(id.to_string())
    }
}

fn equip_slot_v1_for_runtime(slot: &str) -> Option<EquipSlotV1> {
    match slot {
        crate::inventory::EQUIP_SLOT_HEAD => Some(EquipSlotV1::Head),
        crate::inventory::EQUIP_SLOT_CHEST => Some(EquipSlotV1::Chest),
        crate::inventory::EQUIP_SLOT_LEGS => Some(EquipSlotV1::Legs),
        crate::inventory::EQUIP_SLOT_FEET => Some(EquipSlotV1::Feet),
        crate::inventory::EQUIP_SLOT_MAIN_HAND => Some(EquipSlotV1::MainHand),
        crate::inventory::EQUIP_SLOT_OFF_HAND => Some(EquipSlotV1::OffHand),
        crate::inventory::EQUIP_SLOT_EXTRA_HAND_0 => Some(EquipSlotV1::ExtraHand0),
        crate::inventory::EQUIP_SLOT_EXTRA_HAND_1 => Some(EquipSlotV1::ExtraHand1),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_inventory_move(
    entity: valence::prelude::Entity,
    instance_id: u64,
    from: InventoryLocationV1,
    to: InventoryLocationV1,
    // plan-rotate-v1 — 拖拽落位前是否先旋转该 instance（互换 grid_w/grid_h）。
    rotated: bool,
    item_registry: &ItemRegistry,
    inventories: &mut Query<&mut PlayerInventory>,
    clients: &mut Query<(&Username, &mut Client)>,
    player_states: &Query<&PlayerState>,
    cultivations: &Query<&Cultivation>,
    karma_weights: Option<&KarmaWeightStore>,
    durability_changed_tx: Option<&mut Events<InventoryDurabilityChangedEvent>>,
    positions: &Query<&valence::prelude::Position>,
    dimensions: &Query<&CurrentDimension>,
    zones: Option<&mut ZoneRegistry>,
    qi_transfers: Option<&mut Events<crate::qi_physics::ledger::QiTransfer>>,
    attrition_events: Option<&mut Events<AttritionAppliedEvent>>,
    tsy_lifecycle: Option<&TsyZoneStateRegistry>,
    // plan-tarkov-backpack-v1 P0（交付物 #4 红线）— worn 背包件穿/卸时 rebuild 容器，
    // 卸非空背包的 overflow 内含物转掉落物（写入 DroppedLootRegistry，禁止静默丢失）。
    dropped_loot_registry: &mut DroppedLootRegistry,
    // plan-tarkov-backpack-v1 P5 — 套包操作差异化视听反馈（卸/装/拖入）。move 成功后按
    // `classify_pack_move` 判别分支 emit 差异化 VfxEventRequest，client 消费播差异化粒子+音效。
    vfx_events: Option<&mut Events<VfxEventRequest>>,
    // plan-race-system-v1 P3b（决议 §8.1 #5）—— 装备门判定用 Form 身份（当前形态，
    // 未易形时 = 本体）。`Option` 与其余 registry 同规则：既有单测未插入这两个资源时
    // 优雅退化到 humanoid（`resolve_body_plan_for_target` 文档化的退化行为）。
    body_plans: Option<&crate::body_plan::BodyPlanRegistry>,
    race_registry: Option<&crate::body_plan::RaceRegistry>,
    // plan-race-system-v1 P4 —— 当前易形形态（`None` = 未易形），驱动 Form 身份判定的
    // 权威真源（见下方 `form_race_id` 修复注释）。
    morph_states: &Query<Option<&crate::body_plan::MorphState>>,
) {
    let item_before_move = inventories
        .get(entity)
        .ok()
        .and_then(|inventory| inventory_item_by_instance_borrow(inventory, instance_id).cloned());
    let username = clients
        .get(entity)
        .ok()
        .map(|(username, _)| username.0.clone());

    let mut inventory = match inventories.get_mut(entity) {
        Ok(inv) => inv,
        Err(_) => {
            tracing::warn!(
                "[bong][network][inventory] move_intent entity={entity:?} has no PlayerInventory"
            );
            return;
        }
    };

    if let InventoryLocationV1::Equip {
        slot: EquipSlotV1::Chest,
        state: EquipStateV1::Worn,
    } = &to
    {
        if let Some(kind) = item_before_move
            .as_ref()
            .and_then(|item| false_skin_kind_for_item(&item.template_id))
        {
            let realm_allowed = cultivations
                .get(entity)
                .map(|cultivation| can_equip_false_skin(cultivation.realm, kind))
                .unwrap_or(false);
            if !realm_allowed {
                // plan-inventory-hint-panel-v1 P0 —— 伪皮胸槽境界门控并入 InventoryMoveRejectReason
                // ::RealmTooLow（原独立硬编码分支只 tracing::warn! + resync，连 Result 都不走）。
                let reason = InventoryMoveRejectReason::RealmTooLow {
                    required_realm: crate::schema::cultivation::realm_to_string(kind.min_realm())
                        .to_string(),
                };
                tracing::warn!(
                    "[bong][network][tuike] rejected false_skin equip entity={entity:?} instance={instance_id}: {reason}"
                );
                emit_inventory_move_rejected(entity, &reason, clients);
                resync_snapshot(
                    entity,
                    &inventory,
                    clients,
                    player_states,
                    cultivations,
                    "false_skin_realm_rejection",
                );
                return;
            }
        }
    }

    // plan-race-system-v1 P4（决议 §8.1 #5 修复）—— 装备门判定用 Form 身份：已易形
    // （`MorphState` 在场）时权威真源是 `MorphState.form`，**不再**冒用本体
    // `Cultivation.race`——此前这里恒等于本体 race，未易形态下二者恰好相等掩盖了
    // 问题，易形后会让本体应当被拒绝穿戴的装备错误放行 / 应当放行的装备错误拒绝。
    let morph_state = morph_states.get(entity).ok().flatten();
    let form_race_id = morph_state.map(|m| m.form.clone()).unwrap_or_else(|| {
        cultivations
            .get(entity)
            .map(|c| c.race.clone())
            .unwrap_or_else(|_| crate::body_plan::RaceId::new(crate::body_plan::HUMAN_RACE_ID))
    });
    let form_is_humanoid = crate::body_plan::resolve_body_plan_for_target(
        entity,
        crate::body_plan::BodyPlanPurpose::Form,
        crate::body_plan::BodyPlanResolveInputs {
            cultivation: cultivations.get(entity).ok(),
            beast_kind: None,
            morph_state,
        },
        body_plans,
        race_registry,
    )
    .is_humanoid;

    match apply_inventory_move_with_race(
        &mut inventory,
        item_registry,
        instance_id,
        &from,
        &to,
        rotated,
        &form_race_id,
        form_is_humanoid,
    ) {
        Ok(InventoryMoveOutcome::Moved { revision }) => {
            let wear_update = maybe_apply_targeted_item_wear(
                entity,
                &mut inventory,
                item_before_move.as_ref(),
                username.as_deref(),
                karma_weights,
                durability_changed_tx,
            );
            let revision = wear_update
                .map(|update| update.revision)
                .unwrap_or(revision);

            // plan-qi-handling-attrition-v1 P1: SlotMove 磨损，逸散守恒归还 zone
            if let Some(zones) = zones {
                let target_container_exempt =
                    inventory_location_attrition_exempt(&inventory, item_registry, &to);
                let dim = dimensions
                    .get(entity)
                    .map(|d| d.0)
                    .unwrap_or(DimensionKind::Overworld);
                let player_pos_arr = client_position(positions, entity);
                let world_pos = player_pos_arr;
                let pos = valence::prelude::DVec3::new(
                    player_pos_arr[0],
                    player_pos_arr[1],
                    player_pos_arr[2],
                );
                let zone_name = zones.find_zone(dim, pos).map(|z| z.name.clone());
                if let Some(zone_name) = zone_name {
                    if let Some(zone) = zones.find_zone_mut(&zone_name) {
                        if let Some(item) =
                            inventory_item_by_instance_mut(&mut inventory, instance_id)
                        {
                            if !target_container_exempt && !is_attrition_exempt(item) {
                                let before_abs_qi = item_abs_qi_for_attrition(item);
                                apply_attrition_checked(
                                    item,
                                    AttritionOpKind::SlotMove,
                                    Some(zone),
                                    qi_transfers,
                                    tsy_lifecycle,
                                );
                                emit_attrition_applied_if_lost(
                                    attrition_events,
                                    entity,
                                    item,
                                    before_abs_qi,
                                    world_pos,
                                );
                            }
                        }
                    }
                }
            }

            // plan-tarkov-backpack-v1 P0（交付物 #4 红线）— worn 背包件穿/卸触发容器 rebuild。
            //
            // - 卸下（from=Equip{Worn} 且被移走 instance 有 container_spec）：背包件已离 worn 层，
            //   其 `pack_<id>` 容器变孤儿 → rebuild 把内含物 spill 进存活容器，overflow 转掉落物
            //   （连货掉地，禁止静默丢失）。
            // - 穿上（to=Equip{Worn} 且 instance 有 container_spec）：rebuild 即时新建 `pack_<id>`
            //   容器，确保下一帧 snapshot 含该容器（P3 双击有容器可开）。
            //
            // rebuild 改 containers / equipped 后用 resync 推全量快照（Moved delta 表达不了
            // spill/overflow/新建容器），覆盖客户端乐观态。
            let moved_item_is_pack = item_before_move.as_ref().is_some_and(|item| {
                item_registry
                    .get(&item.template_id)
                    .is_some_and(|t| t.container_spec.is_some())
            });
            let from_worn = matches!(
                from,
                InventoryLocationV1::Equip {
                    state: EquipStateV1::Worn,
                    ..
                }
            );
            let to_worn = matches!(
                to,
                InventoryLocationV1::Equip {
                    state: EquipStateV1::Worn,
                    ..
                }
            );
            let worn_pack_unequip = moved_item_is_pack && from_worn && !to_worn;
            let worn_pack_equip = moved_item_is_pack && to_worn && !from_worn;

            // plan-tarkov-backpack-v1 P5 — 套包操作差异化视听反馈。
            // 三类（卸/装/拖入）按 `classify_pack_move` 判别分支 emit 各自差异化的
            // VfxEventRequest（event_id/color/count/duration 互不相同），client
            // `PackOperationVfxPlayer` 派发播差异化粒子 + 内联 audio recipe。
            // emit 放在 worn-pack rebuild/resync 路由之前，三类反馈统一从此触发。
            let to_is_pack_container = matches!(
                &to,
                InventoryLocationV1::Container { container_id, .. }
                    if container_id.starts_with("pack_")
            );
            if let Some(pack_vfx) = gameplay_vfx::classify_pack_move(
                moved_item_is_pack,
                from_worn,
                to_worn,
                to_is_pack_container,
            ) {
                if let Some(events) = vfx_events {
                    let origin_arr = client_position(positions, entity);
                    // worn 件挂胸前 ≈ 玩家中段；落地散落从脚踝偏上铺开。
                    let origin = DVec3::new(origin_arr[0], origin_arr[1] + 0.9, origin_arr[2]);
                    gameplay_vfx::send_spawn(
                        events,
                        gameplay_vfx::pack_move_request(pack_vfx, origin),
                    );
                }
            }

            // plan-tarkov-backpack-v1 套包修复 §4：rebuild 触发条件从「worn 边界跨越」扩到
            // 「任意 pack 件移动」。retention 后 worn↔body_pocket↔另一 pack↔hotbar↔held 的
            // 任意移动都需 rebuild+resync——刷新 max_weight（worn 负重加成）/ owner_instance_id /
            // body_pocket 存在性 + 推全量快照覆盖客户端乐观态。§2 后无孤儿时 overflow 为空、
            // 零副作用（幂等）。worn_pack_unequip/equip 仅保留用于上方 classify_pack_move 的
            // VFX 分类与本日志。
            if moved_item_is_pack {
                let player_pos = client_position(positions, entity);
                let player_dimension = dimensions.get(entity).map(|dim| dim.0).unwrap_or_default();
                let dropped_ids = crate::inventory::rebuild_and_drop_overflow(
                    &mut inventory,
                    item_registry,
                    dropped_loot_registry,
                    player_pos,
                    player_dimension,
                );
                if !dropped_ids.is_empty() {
                    tracing::info!(
                        "[bong][network][inventory] pack-move overflow dropped {} item(s) to world: {dropped_ids:?}",
                        dropped_ids.len()
                    );
                }
                tracing::info!(
                    "[bong][network][inventory] pack-move rebuild after move instance={instance_id} {from:?} -> {to:?} (unequip={worn_pack_unequip} equip={worn_pack_equip})"
                );
                resync_snapshot(
                    entity,
                    &inventory,
                    clients,
                    player_states,
                    cultivations,
                    "worn_pack_rebuild",
                );
                return;
            }

            tracing::info!(
                "[bong][network][inventory] moved instance={instance_id} {from:?} -> {to:?} revision={}",
                revision.0
            );
            send_moved_event(entity, clients, instance_id, from, to, revision.0);
        }
        Ok(InventoryMoveOutcome::Swapped {
            revision,
            displaced_instance_id,
        }) => {
            tracing::info!(
                "[bong][network][inventory] swapped instance={instance_id} <-> {displaced_instance_id} {from:?} <-> {to:?} revision={}",
                revision.0
            );
            // Two ordered Moved events would have an intermediate inconsistent
            // state on the client (the first event would clobber the second
            // item). Push a fresh snapshot instead — correct, idempotent.
            resync_snapshot(
                entity,
                &inventory,
                clients,
                player_states,
                cultivations,
                "swap",
            );
        }
        Err(reason) => {
            tracing::warn!(
                "[bong][network][inventory] rejected move_intent entity={entity:?} instance={instance_id}: {reason}"
            );
            // plan-inventory-hint-panel-v1 P0 —— 结构化拒绝原因下发触发者，供 client
            // 失败 toast（P1）消费；不影响既有 resync 权威覆盖。
            emit_inventory_move_rejected(entity, &reason, clients);
            // Client did optimistic update but server didn't move. Resync to
            // overwrite the diverged client state with authoritative truth.
            resync_snapshot(
                entity,
                &inventory,
                clients,
                player_states,
                cultivations,
                "rejection",
            );
        }
    }
}

/// plan-layered-equip-v1 P4（决议 #8）— 法宝激活/卸下到灵宝 UI 触发位。
///
/// 把 `instance_id` 在 inventory 与触发位之间移动（`activate` 区分方向）。成功 / 拒绝后都
/// `resync_snapshot` 推全量 inventory 快照覆盖客户端乐观态；改 PlayerInventory（get_mut）会触发
/// `Changed<PlayerInventory>`，下一 tick `sync_spirit_treasures`（scan passive_active）与
/// `emit_treasure_equipped_payloads`（触发位 payload）自动重跑。
#[allow(clippy::too_many_arguments)]
fn handle_treasure_activate(
    entity: Entity,
    instance_id: u64,
    activate: bool,
    item_registry: &ItemRegistry,
    inventories: &mut Query<&mut PlayerInventory>,
    clients: &mut Query<(&Username, &mut Client)>,
    player_states: &Query<&PlayerState>,
    cultivations: &Query<&Cultivation>,
) {
    let mut inventory = match inventories.get_mut(entity) {
        Ok(inv) => inv,
        Err(_) => {
            tracing::warn!(
                "[bong][network][inventory] treasure_activate entity={entity:?} has no PlayerInventory"
            );
            return;
        }
    };

    match crate::inventory::apply_treasure_activate(
        &mut inventory,
        item_registry,
        instance_id,
        activate,
    ) {
        Ok(outcome) => {
            tracing::info!(
                "[bong][network][inventory] treasure_activate instance={instance_id} activate={activate} -> {outcome:?}"
            );
            resync_snapshot(
                entity,
                &inventory,
                clients,
                player_states,
                cultivations,
                "treasure_activate",
            );
        }
        Err(reason) => {
            tracing::warn!(
                "[bong][network][inventory] rejected treasure_activate entity={entity:?} instance={instance_id} activate={activate}: {reason}"
            );
            // 客户端做了乐观飞入/卸下；server 拒绝 → 推权威快照覆盖。
            resync_snapshot(
                entity,
                &inventory,
                clients,
                player_states,
                cultivations,
                "treasure_activate_rejection",
            );
        }
    }
}

fn maybe_apply_targeted_item_wear(
    entity: Entity,
    inventory: &mut PlayerInventory,
    item: Option<&ItemInstance>,
    username: Option<&str>,
    karma_weights: Option<&KarmaWeightStore>,
    durability_changed_tx: Option<&mut Events<InventoryDurabilityChangedEvent>>,
) -> Option<crate::inventory::InventorySpiritualWearUpdate> {
    let item = item?;
    if !is_spiritual_item_for_targeted_wear(item) {
        return None;
    }
    let username = username?;
    let weight = karma_weights?.weight_for_player(username);
    if weight < QI_TARGETED_ITEM_WEAR_WEIGHT_THRESHOLD {
        return None;
    }

    let wear_fraction = qi_targeted_item_wear_fraction(item.instance_id, username, weight);
    match apply_item_spiritual_wear(inventory, item.instance_id, wear_fraction) {
        Ok(update) => {
            if let Some(events) = durability_changed_tx {
                events.send(InventoryDurabilityChangedEvent {
                    entity,
                    revision: update.revision,
                    instance_id: update.instance_id,
                    durability: update.durability,
                });
            }
            tracing::info!(
                "[bong][network][inventory] targeted item wear entity={entity:?} instance={} wear={:.4} durability={:.4} spirit_quality={:.4}",
                update.instance_id,
                update.wear_fraction,
                update.durability,
                update.spirit_quality
            );
            Some(update)
        }
        Err(error) => {
            tracing::warn!(
                "[bong][network][inventory] targeted item wear failed entity={entity:?} instance={}: {error}",
                item.instance_id
            );
            None
        }
    }
}

fn is_spiritual_item_for_targeted_wear(item: &ItemInstance) -> bool {
    item.spirit_quality > 0.0 || item.forge_quality.is_some() || item.mineral_id.is_some()
}

fn send_moved_event(
    entity: valence::prelude::Entity,
    clients: &mut Query<(&Username, &mut Client)>,
    instance_id: u64,
    from: InventoryLocationV1,
    to: InventoryLocationV1,
    revision: u64,
) {
    let payload = ServerDataV1::new(ServerDataPayloadV1::InventoryEvent(Box::new(
        InventoryEventV1::Moved {
            revision,
            instance_id,
            from,
            to,
        },
    )));
    let payload_type = payload_type_label(payload.payload_type());
    let payload_bytes = match serialize_server_data_payload(&payload) {
        Ok(bytes) => bytes,
        Err(error) => {
            tracing::error!(
                "[bong][network][inventory] failed to serialize {payload_type}: {error:?}"
            );
            return;
        }
    };

    if let Ok((_username, mut client)) = clients.get_mut(entity) {
        send_server_data_payload(&mut client, payload_bytes.as_slice());
        tracing::info!(
            "[bong][network] sent {} {} payload to client entity {entity:?}",
            SERVER_DATA_CHANNEL,
            payload_type
        );
    }
}

pub(crate) fn resync_snapshot(
    entity: valence::prelude::Entity,
    inventory: &PlayerInventory,
    clients: &mut Query<(&Username, &mut Client)>,
    player_states: &Query<&PlayerState>,
    cultivations: &Query<&Cultivation>,
    reason: &str,
) {
    resync_snapshot_with_cultivation_override(
        entity,
        inventory,
        clients,
        player_states,
        cultivations,
        None,
        reason,
    );
}

fn resync_snapshot_with_cultivation_override(
    entity: valence::prelude::Entity,
    inventory: &PlayerInventory,
    clients: &mut Query<(&Username, &mut Client)>,
    player_states: &Query<&PlayerState>,
    cultivations: &Query<&Cultivation>,
    cultivation_override: Option<&Cultivation>,
    reason: &str,
) {
    let player_state = match player_states.get(entity) {
        Ok(state) => state,
        Err(_) => {
            tracing::warn!(
                "[bong][network][inventory] cannot resync entity={entity:?} — no PlayerState"
            );
            return;
        }
    };
    let fallback_cultivation;
    let cultivation = match cultivation_override {
        Some(cultivation) => cultivation,
        None => {
            fallback_cultivation = match cultivations.get(entity) {
                Ok(cultivation) => cultivation,
                Err(_) => {
                    tracing::warn!(
                        "[bong][network][inventory] cannot resync entity={entity:?} — no Cultivation"
                    );
                    return;
                }
            };
            fallback_cultivation
        }
    };
    if let Ok((username, mut client)) = clients.get_mut(entity) {
        send_inventory_snapshot_to_client(
            entity,
            &mut client,
            username.0.as_str(),
            inventory,
            player_state,
            cultivation,
            reason,
        );
    }
}

fn client_position(positions: &Query<&valence::prelude::Position>, entity: Entity) -> [f64; 3] {
    positions
        .get(entity)
        .map(|pos| {
            let v = pos.get();
            [v.x, v.y, v.z]
        })
        .unwrap_or([0.0, 64.0, 0.0])
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_inventory_discard(
    entity: Entity,
    instance_id: u64,
    from: InventoryLocationV1,
    inventories: &mut Query<&mut PlayerInventory>,
    dropped_loot_registry: &mut DroppedLootRegistry,
    item_registry: &ItemRegistry,
    clients: &mut Query<(&Username, &mut Client)>,
    player_states: &Query<&PlayerState>,
    cultivations: &Query<&Cultivation>,
    positions: &Query<&valence::prelude::Position>,
    dimensions: &Query<&CurrentDimension>,
) {
    let player_pos = client_position(positions, entity);
    let player_dimension = dimensions.get(entity).map(|dim| dim.0).unwrap_or_default();
    let mut inventory = match inventories.get_mut(entity) {
        Ok(inv) => inv,
        Err(_) => {
            tracing::warn!(
                "[bong][network][inventory] discard entity={entity:?} has no PlayerInventory"
            );
            return;
        }
    };

    match discard_inventory_item_to_dropped_loot(
        &mut inventory,
        dropped_loot_registry,
        player_pos,
        player_dimension,
        instance_id,
        &from,
    ) {
        Ok(outcome) => {
            tracing::info!(
                "[bong][network][inventory] discarded instance={instance_id} from {from:?} revision={}",
                outcome.revision.0
            );
            // plan-tarkov-backpack-v1 套包修复 §5：discard 是「真离开玩家」的合法 spill 触发点。
            // 若被 discard 件是背包（有 container_spec），其 pack_<id> 容器现已孤儿 → 补调
            // rebuild_and_drop_overflow：内含物连货掉地（塔科夫式直觉）+ 清孤儿容器，否则内含物
            // 滞留 inventory 落盘，下次 load 触发 inventory_has_orphan_pack_container 重置 loadout
            // （#736 复发面）。无孤儿时 rebuild 幂等。
            let discarded_is_pack = item_registry
                .get(&outcome.dropped.item.template_id)
                .is_some_and(|t| t.container_spec.is_some());
            if discarded_is_pack {
                let player_pos = client_position(positions, entity);
                let player_dimension = dimensions.get(entity).map(|dim| dim.0).unwrap_or_default();
                let dropped_ids = crate::inventory::rebuild_and_drop_overflow(
                    &mut inventory,
                    item_registry,
                    dropped_loot_registry,
                    player_pos,
                    player_dimension,
                );
                if !dropped_ids.is_empty() {
                    tracing::info!(
                        "[bong][network][inventory] discarded pack instance={instance_id}; spilled {} contained item(s) to world: {dropped_ids:?}",
                        dropped_ids.len()
                    );
                }
            }
            resync_snapshot(
                entity,
                &inventory,
                clients,
                player_states,
                cultivations,
                "discard_item",
            );
            // Dropped loot sync is broadcast by dropped_loot_sync_emit.
        }
        Err(reason) => {
            tracing::warn!(
                "[bong][network][inventory] rejected discard entity={entity:?} instance={instance_id}: {reason}"
            );
            resync_snapshot(
                entity,
                &inventory,
                clients,
                player_states,
                cultivations,
                "discard_rejection",
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_pickup_dropped_item(
    entity: Entity,
    instance_id: u64,
    inventories: &mut Query<&mut PlayerInventory>,
    dropped_loot_registry: &mut DroppedLootRegistry,
    item_registry: &ItemRegistry,
    clients: &mut Query<(&Username, &mut Client)>,
    player_states: &Query<&PlayerState>,
    cultivations: &Query<&Cultivation>,
    positions: &Query<&valence::prelude::Position>,
    dimensions: &Query<&CurrentDimension>,
    zones: Option<&mut ZoneRegistry>,
    qi_transfers: Option<&mut Events<crate::qi_physics::ledger::QiTransfer>>,
    attrition_events: Option<&mut Events<AttritionAppliedEvent>>,
    tsy_lifecycle: Option<&TsyZoneStateRegistry>,
    persistence: Option<&PlayerStatePersistence>,
) {
    let player_pos = client_position(positions, entity);
    let mut inventory = match inventories.get_mut(entity) {
        Ok(inv) => inv,
        Err(_) => {
            tracing::warn!(
                "[bong][network][inventory] pickup entity={entity:?} has no PlayerInventory"
            );
            return;
        }
    };

    let mut staged_inventory = inventory.clone();
    let mut staged_dropped_loot = dropped_loot_registry.clone();
    let mut staged_zones = zones.as_deref().cloned();
    let mut staged_qi_transfers = Events::default();
    let mut staged_attrition_events = Events::default();
    match pickup_dropped_loot_instance(
        &mut staged_inventory,
        &mut staged_dropped_loot,
        player_pos,
        instance_id,
    ) {
        Ok(revision) => {
            let dim = dimensions
                .get(entity)
                .map(|d| d.0)
                .unwrap_or(DimensionKind::Overworld);
            let pos = valence::prelude::DVec3::new(player_pos[0], player_pos[1], player_pos[2]);
            let mut zone_runtime = None;
            if let Some(staged_zones) = staged_zones.as_mut() {
                let zone_name = staged_zones
                    .find_zone(dim, pos)
                    .map(|zone| zone.name.clone());
                if let Some(zone_name) = zone_name {
                    if let Some(zone) = staged_zones.find_zone_mut(&zone_name) {
                        let target_container_exempt = inventory_instance_container_attrition_exempt(
                            &staged_inventory,
                            item_registry,
                            instance_id,
                        );
                        if let Some(item) =
                            inventory_item_by_instance_mut(&mut staged_inventory, instance_id)
                        {
                            if !target_container_exempt && !is_attrition_exempt(item) {
                                let before_abs_qi = item_abs_qi_for_attrition(item);
                                apply_attrition_checked(
                                    item,
                                    AttritionOpKind::Pickup,
                                    Some(zone),
                                    Some(&mut staged_qi_transfers),
                                    tsy_lifecycle,
                                );
                                emit_attrition_applied_if_lost(
                                    Some(&mut staged_attrition_events),
                                    entity,
                                    item,
                                    before_abs_qi,
                                    player_pos,
                                );
                            }
                        }
                        zone_runtime = Some(ZoneRuntimeRecord {
                            zone_id: zone.name.clone(),
                            spirit_qi: zone.spirit_qi,
                            danger_level: zone.danger_level,
                        });
                    }
                }
            }
            if let Some(persistence) = persistence {
                let username = match clients.get_mut(entity) {
                    Ok((username, _)) => username.0.clone(),
                    Err(_) => {
                        tracing::error!(
                            "[bong][network][inventory] refusing durable pickup for {entity:?} without Username"
                        );
                        return;
                    }
                };
                if let Err(error) = save_player_inventory_and_delete_dropped_loot(
                    persistence,
                    username.as_str(),
                    &staged_inventory,
                    instance_id,
                    zone_runtime.as_ref(),
                ) {
                    tracing::error!(
                        "[bong][network][inventory] durable pickup persistence failed player={username} instance={instance_id}: {error}"
                    );
                    return;
                }
            }
            *inventory = staged_inventory;
            *dropped_loot_registry = staged_dropped_loot;
            if let (Some(zones), Some(staged_zones)) = (zones, staged_zones) {
                *zones = staged_zones;
            }
            if let Some(qi_transfers) = qi_transfers {
                qi_transfers.extend(staged_qi_transfers.drain());
            }
            if let Some(attrition_events) = attrition_events {
                attrition_events.extend(staged_attrition_events.drain());
            }
            tracing::info!(
                "[bong][network][inventory] picked up dropped instance={instance_id} revision={}",
                revision.0
            );

            resync_snapshot(
                entity,
                &inventory,
                clients,
                player_states,
                cultivations,
                "pickup_dropped_item",
            );
            // Dropped loot sync is broadcast by dropped_loot_sync_emit.
        }
        Err(reason) => {
            tracing::warn!(
                "[bong][network][inventory] rejected pickup entity={entity:?} instance={instance_id}: {reason}"
            );
            resync_snapshot(
                entity,
                &inventory,
                clients,
                player_states,
                cultivations,
                "pickup_rejection",
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_repair_weapon(
    entity: Entity,
    instance_id: u64,
    station_pos: [i32; 3],
    item_registry: &ItemRegistry,
    inventories: &mut Query<&mut PlayerInventory>,
    clients: &mut Query<(&Username, &mut Client)>,
    player_states: &Query<&PlayerState>,
    cultivations: &Query<&Cultivation>,
) {
    let mut inventory = match inventories.get_mut(entity) {
        Ok(inv) => inv,
        Err(_) => {
            tracing::warn!(
                "[bong][network][weapon] repair entity={entity:?} has no PlayerInventory"
            );
            return;
        }
    };

    match fully_repair_weapon_instance(&mut inventory, item_registry, instance_id) {
        Ok(update) => {
            tracing::info!(
                "[bong][network][weapon] repaired instance={instance_id} durability={} revision={} station_pos=[{},{},{}]",
                update.durability,
                update.revision.0,
                station_pos[0],
                station_pos[1],
                station_pos[2]
            );
            resync_snapshot(
                entity,
                &inventory,
                clients,
                player_states,
                cultivations,
                "repair_weapon",
            );
        }
        Err(reason) => {
            tracing::warn!(
                "[bong][network][weapon] rejected repair entity={entity:?} instance={instance_id}: {reason}"
            );
            resync_snapshot(
                entity,
                &inventory,
                clients,
                player_states,
                cultivations,
                "repair_rejection",
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_apply_pill(
    entity: Entity,
    instance_id: u64,
    _target: crate::schema::client_request::ApplyPillTargetV1,
    commands: &mut Commands,
    clock: &CombatClock,
    inventories: &mut Query<&mut PlayerInventory>,
    clients: &mut Query<(&Username, &mut Client)>,
    player_states: &Query<&PlayerState>,
    cultivations: &Query<&Cultivation>,
    combat_params: &mut CombatRequestParams,
    lifespan_extension_tx: &mut Option<ResMut<Events<LifespanExtensionIntent>>>,
    vfx_events: Option<&mut Events<VfxEventRequest>>,
    audio_events: &mut Option<ResMut<Events<PlaySoundRecipeRequest>>>,
    hallucination_events: Option<
        &mut Events<crate::fauna::hybrid_beast::CoreAbsorptionHallucinationEvent>,
    >,
    narrations: Option<&mut crate::player::gameplay::PendingGameplayNarrations>,
) {
    let template_id = inventories
        .get(entity)
        .ok()
        .and_then(|inventory| {
            crate::inventory::inventory_item_by_instance_borrow(inventory, instance_id)
        })
        .map(|item| item.template_id.clone());
    let Some(template_id) = template_id else {
        tracing::warn!(
            "[bong][network][alchemy] apply_pill entity={entity:?} instance={instance_id} missing from inventory"
        );
        return;
    };
    handle_alchemy_take_pill(
        entity,
        &template_id,
        Some(instance_id),
        commands,
        clock,
        inventories,
        clients,
        player_states,
        cultivations,
        combat_params,
        lifespan_extension_tx,
        vfx_events,
        audio_events,
        // plan-fauna-stitched-beast-v1 P3 M1 修复：透传幻觉事件和叙事容器
        hallucination_events,
        narrations,
    );
}

pub(crate) fn handle_alchemy_turn_page(
    entity: valence::prelude::Entity,
    delta: i32,
    clients: &mut Query<(&Username, &mut Client)>,
    learned_q: &mut Query<&mut LearnedRecipes>,
    alchemy_state: &mut AlchemyMockState,
) {
    let Ok((username, mut client)) = clients.get_mut(entity) else {
        return;
    };
    let player_id = canonical_player_id(username.0.as_str());
    if let Ok(mut learned) = learned_q.get_mut(entity) {
        if !learned.ids.is_empty() {
            // Guard against malicious/huge deltas (e.g. i32::MIN.unsigned_abs() ==
            // 2.1B would freeze the ECS tick). next()/prev() wrap modularly, so
            // turning |delta| mod len pages lands at the identical index while
            // bounding the loop to at most len-1 iterations.
            let steps = delta.unsigned_abs() % (learned.ids.len() as u32);
            for _ in 0..steps {
                if delta >= 0 {
                    learned.next();
                } else {
                    learned.prev();
                }
            }
            tracing::info!(
                "[bong][network][alchemy] turn_page delta={delta} → idx={} ({} learned) for `{player_id}`",
                learned.current_index,
                learned.ids.len()
            );
            alchemy_snapshot_emit::send_recipe_book_from_learned(&mut client, &player_id, &learned);
            return;
        }
    }
    // fallback:玩家没有 LearnedRecipes 组件 → 走 mock state
    let current = alchemy_state
        .recipe_index
        .entry(player_id.clone())
        .or_insert(0);
    *current = current.saturating_add(delta);
    let new_index = *current;
    alchemy_snapshot_emit::send_recipe_book(&mut client, &player_id, new_index);
}

pub(crate) fn handle_alchemy_learn(
    entity: valence::prelude::Entity,
    recipe_id: String,
    clients: &mut Query<(&Username, &mut Client)>,
    learned_q: &mut Query<&mut LearnedRecipes>,
    registry: &RecipeRegistry,
) {
    let Ok((username, mut client)) = clients.get_mut(entity) else {
        return;
    };
    let player_id = canonical_player_id(username.0.as_str());
    if registry.get(&recipe_id).is_none() {
        tracing::warn!(
            "[bong][network][alchemy] learn unknown recipe `{recipe_id}` from `{player_id}`"
        );
        return;
    }
    if let Ok(mut learned) = learned_q.get_mut(entity) {
        match learned.learn(recipe_id.clone()) {
            LearnResult::Learned => tracing::info!(
                "[bong][network][alchemy] `{player_id}` learned `{recipe_id}` (total {})",
                learned.ids.len()
            ),
            LearnResult::AlreadyKnown => tracing::debug!(
                "[bong][network][alchemy] `{player_id}` already knows `{recipe_id}`"
            ),
            LearnResult::FragmentMerged => tracing::debug!(
                "[bong][network][alchemy] `{player_id}` merged fragment while learning `{recipe_id}`"
            ),
        }
        alchemy_snapshot_emit::send_recipe_book_from_learned(&mut client, &player_id, &learned);
    }
}

pub(crate) fn handle_alchemy_open_furnace(
    entity: valence::prelude::Entity,
    furnace_pos: (i32, i32, i32),
    clients: &mut Query<(&Username, &mut Client)>,
    furnaces: &mut Query<(Entity, &mut AlchemyFurnace)>,
    learned_q: &mut Query<&mut LearnedRecipes>,
    registry: &RecipeRegistry,
) {
    let Ok((username, mut client)) = clients.get_mut(entity) else {
        return;
    };
    let player_id = canonical_player_id(username.0.as_str());
    match with_owned_furnace_mut(entity, &player_id, furnace_pos, furnaces, |furnace| {
        alchemy_snapshot_emit::send_furnace_from_furnace(&mut client, &player_id, furnace);
        alchemy_snapshot_emit::send_session_from_furnace(
            &mut client,
            &player_id,
            furnace,
            registry,
        );
    }) {
        Ok(()) => {
            if let Ok(learned) = learned_q.get(entity) {
                alchemy_snapshot_emit::send_recipe_book_from_learned(
                    &mut client,
                    &player_id,
                    learned,
                );
            }
            tracing::info!(
                "[bong][network][alchemy] open_furnace pos={furnace_pos:?} for `{player_id}`"
            );
        }
        Err(AlchemyFurnaceRouteError::Missing) => {
            send_alchemy_error(
                &mut client,
                &player_id,
                format!("炼丹炉不存在：{furnace_pos:?}"),
            );
        }
        Err(AlchemyFurnaceRouteError::Forbidden { owner }) => {
            tracing::warn!(
                "[bong][network][alchemy] `{player_id}` tried to open furnace pos={furnace_pos:?} owned by {owner:?}"
            );
            send_alchemy_error(&mut client, &player_id, "这座炉不是你的".to_string());
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_alchemy_intervention(
    entity: valence::prelude::Entity,
    furnace_pos: (i32, i32, i32),
    intervention: Intervention,
    clients: &mut Query<(&Username, &mut Client)>,
    // plan-skill-av-relink-v1 P1 — alchemy_stir 搅拌动画的 target_player uuid。
    unique_ids: &Query<&UniqueId>,
    furnaces: &mut Query<(Entity, &mut AlchemyFurnace)>,
    registry: &RecipeRegistry,
    zones: Option<&ZoneRegistry>,
    redis: Option<&RedisBridgeResource>,
    vfx_events: Option<&mut Events<VfxEventRequest>>,
) {
    let Ok((username, mut client)) = clients.get_mut(entity) else {
        return;
    };
    let player_id = canonical_player_id(username.0.as_str());
    let result = with_owned_furnace_mut(entity, &player_id, furnace_pos, furnaces, |furnace| {
        if matches!(intervention, Intervention::InjectQi(_))
            && furnace_zone_is_collapsed(furnace, zones)
        {
            tracing::debug!(
                "[bong][network][alchemy] `{player_id}` inject_qi ignored: furnace is in collapsed zone"
            );
            return;
        }
        let session = match furnace.session.as_mut() {
            Some(s) => s,
            None => {
                send_alchemy_error(&mut client, &player_id, "尚未起炉".to_string());
                return;
            }
        };
        session.apply_intervention(intervention.clone());
        if let Some(events) = vfx_events {
            let (event_id, color, strength, count) = match intervention {
                Intervention::AdjustTemp(temp) if temp >= 0.85 => {
                    (gameplay_vfx::ALCHEMY_OVERHEAT, "#FF4433", 0.85, 10)
                }
                Intervention::InjectQi(_) => (gameplay_vfx::ALCHEMY_BREW_VAPOR, "#AA66FF", 0.65, 8),
                _ => (gameplay_vfx::ALCHEMY_BREW_VAPOR, "#88CCFF", 0.45, 6),
            };
            gameplay_vfx::send_spawn(
                events,
                gameplay_vfx::spawn_request(
                    event_id,
                    alchemy_furnace_origin(furnace_pos),
                    Some([0.0, 0.6, 0.0]),
                    color,
                    strength,
                    count,
                    30,
                ),
            );
            // plan-skill-av-relink-v1 P1 — 干预生效 → alchemy_stir 搅拌动画（与上方
            // 熬煮粒子同点内联：干预直接在 request handler 处理、无 bevy 事件可订阅）。
            // 未起炉/非炉主等拒绝分支在前面已 return，不会走到这里。
            // AutoProfile 是保留 no-op（session.rs apply_intervention 不改任何状态），
            // 无真实搅拌动作，不发动画——只有生效干预（AdjustTemp/InjectQi）才发。
            if !matches!(intervention, Intervention::AutoProfile(_)) {
                if let Ok(unique_id) = unique_ids.get(entity) {
                    events.send(crate::network::vfx_event_emit::VfxEventRequest::new(
                        alchemy_furnace_origin(furnace_pos),
                        crate::schema::vfx_event::VfxEventPayloadV1::PlayAnim {
                            target_player: unique_id.0.to_string(),
                            anim_id: crate::network::vfx_animation_trigger::ANIM_ALCHEMY_STIR
                                .to_string(),
                            priority: crate::network::vfx_animation_trigger::COMBAT_PRIORITY,
                            fade_in_ticks: Some(2),
                        },
                    ));
                }
            }
        }
        tracing::info!(
            "[bong][network][alchemy] `{player_id}` intervention {intervention:?} pos={furnace_pos:?} → temp={:.2} qi={:.2}",
            session.temp_current, session.qi_injected
        );
        publish_alchemy_intervention_result(
            redis,
            furnace_pos,
            session.recipe.as_str(),
            player_id.as_str(),
            &intervention,
            session.temp_current,
            session.qi_injected,
        );
        alchemy_snapshot_emit::send_session_from_furnace(
            &mut client,
            &player_id,
            furnace,
            registry,
        );
    });
    log_or_send_route_error(result, &mut client, &player_id, furnace_pos, "intervention");
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_alchemy_ignite(
    entity: valence::prelude::Entity,
    furnace_pos: (i32, i32, i32),
    recipe_id: String,
    clients: &mut Query<(&Username, &mut Client)>,
    furnaces: &mut Query<(Entity, &mut AlchemyFurnace)>,
    registry: &RecipeRegistry,
    zones: Option<&ZoneRegistry>,
    redis: Option<&RedisBridgeResource>,
    vfx_events: Option<&mut Events<VfxEventRequest>>,
) {
    let Ok((username, mut client)) = clients.get_mut(entity) else {
        return;
    };
    let player_id = canonical_player_id(username.0.as_str());
    let Some(recipe) = registry.get(&recipe_id) else {
        send_alchemy_error(&mut client, &player_id, format!("未知丹方：{recipe_id}"));
        return;
    };
    if let Err(message) = check_alchemy_zone_qi(furnace_pos, zones, recipe_id.as_str()) {
        send_alchemy_error(&mut client, &player_id, message);
        return;
    }
    let result = with_owned_furnace_mut(entity, &player_id, furnace_pos, furnaces, |furnace| {
        if !furnace.can_run(recipe.furnace_tier_min) {
            send_alchemy_error(
                &mut client,
                &player_id,
                format!("炉阶不足或炉体已损：需要 t{}", recipe.furnace_tier_min),
            );
            return;
        }
        if furnace.is_busy() {
            send_alchemy_error(&mut client, &player_id, "炉中已有丹火".to_string());
            return;
        }
        let session = AlchemySession::new(recipe.id.clone(), player_id.clone());
        if let Err(error) = furnace.start_session(session) {
            send_alchemy_error(&mut client, &player_id, format!("起炉失败：{error}"));
            return;
        }
        tracing::info!(
            "[bong][network][alchemy] `{player_id}` ignite `{recipe_id}` at pos={furnace_pos:?}"
        );
        if let Some(events) = vfx_events {
            gameplay_vfx::send_spawn(
                events,
                gameplay_vfx::spawn_request(
                    gameplay_vfx::ALCHEMY_BREW_VAPOR,
                    alchemy_furnace_origin(furnace_pos),
                    Some([0.0, 0.5, 0.0]),
                    "#88CCFF",
                    0.55,
                    8,
                    40,
                ),
            );
        }
        publish_alchemy_session_start(
            redis,
            furnace_pos,
            furnace.tier,
            recipe_id.as_str(),
            player_id.as_str(),
        );
        alchemy_snapshot_emit::send_furnace_from_furnace(&mut client, &player_id, furnace);
        alchemy_snapshot_emit::send_session_from_furnace(
            &mut client,
            &player_id,
            furnace,
            registry,
        );
    });
    log_or_send_route_error(result, &mut client, &player_id, furnace_pos, "ignite");
}

fn check_alchemy_zone_qi(
    furnace_pos: (i32, i32, i32),
    zones: Option<&ZoneRegistry>,
    recipe_id: &str,
) -> Result<(), String> {
    let zone_qi = zones
        .and_then(|zones| {
            zones
                .find_zone(
                    DimensionKind::Overworld,
                    valence::prelude::DVec3::new(
                        furnace_pos.0 as f64,
                        furnace_pos.1 as f64,
                        furnace_pos.2 as f64,
                    ),
                )
                .or_else(|| zones.find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME))
        })
        .map(|zone| zone.spirit_qi)
        .unwrap_or(0.0);
    if zone_qi < MIN_ZONE_QI_TO_ALCHEMY {
        return Err(format!(
            "区域灵气不足：{zone_qi:.3} < {MIN_ZONE_QI_TO_ALCHEMY:.3}，无法起炉 {recipe_id}"
        ));
    }
    Ok(())
}

fn alchemy_furnace_origin(furnace_pos: (i32, i32, i32)) -> DVec3 {
    DVec3::new(
        f64::from(furnace_pos.0) + 0.5,
        f64::from(furnace_pos.1) + 1.0,
        f64::from(furnace_pos.2) + 0.5,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_alchemy_feed_slot(
    entity: valence::prelude::Entity,
    furnace_pos: (i32, i32, i32),
    slot_idx: u8,
    material: String,
    count: u32,
    clients: &mut Query<(&Username, &mut Client)>,
    furnaces: &mut Query<(Entity, &mut AlchemyFurnace)>,
    registry: &RecipeRegistry,
    inventories: &mut Query<&mut PlayerInventory>,
    player_states: &Query<&PlayerState>,
    cultivations: &Query<&Cultivation>,
    mut zones: Option<&mut ZoneRegistry>,
    mut qi_transfers: Option<&mut Events<crate::qi_physics::ledger::QiTransfer>>,
    mut attrition_events: Option<&mut Events<AttritionAppliedEvent>>,
    tsy_lifecycle: Option<&TsyZoneStateRegistry>,
) {
    let Ok((username, mut client)) = clients.get_mut(entity) else {
        return;
    };
    let player_id = canonical_player_id(username.0.as_str());
    let result = with_owned_furnace_mut(entity, &player_id, furnace_pos, furnaces, |furnace| {
        let Some(session) = furnace.session.as_mut() else {
            send_alchemy_error(&mut client, &player_id, "尚未起炉".to_string());
            return;
        };
        let Some(recipe) = registry.get(&session.recipe) else {
            send_alchemy_error(
                &mut client,
                &player_id,
                format!("未知丹方：{}", session.recipe),
            );
            return;
        };
        let expected = recipe
            .stages
            .get(slot_idx as usize)
            .and_then(|stage| stage.required.iter().find(|spec| spec.material == material));
        let Some(expected) = expected else {
            send_alchemy_error(&mut client, &player_id, format!("此槽不收 {material}"));
            return;
        };
        if count != expected.count {
            send_alchemy_error(
                &mut client,
                &player_id,
                format!("投料数量不符：需要 {}，收到 {count}", expected.count),
            );
            return;
        }
        let mut inventory = match inventories.get_mut(entity) {
            Ok(inventory) => inventory,
            Err(_) => {
                send_alchemy_error(&mut client, &player_id, "未找到背包".to_string());
                return;
            }
        };
        let Some(selected_consumption) =
            select_ingredient_instances_for_consumption(&inventory, expected, count)
        else {
            send_alchemy_error(
                &mut client,
                &player_id,
                format!("材料不足或矿物不符：{material}×{count}"),
            );
            return;
        };
        let staged_before_feed = session.staged.clone();
        let inventory_before_feed = inventory.clone();
        if let Err(error) =
            session.feed_stage(recipe, slot_idx as usize, &[(material.clone(), count, 1.0)])
        {
            send_alchemy_error(&mut client, &player_id, format!("投料失败：{error}"));
            return;
        }

        // plan-qi-handling-attrition-v1 P1：AlchemyLoad 磨损。
        // 在 consume 前对投料 item 施加磨损（item 还在 inventory，可找到并改 spirit_quality）。
        // zone 用炼炉位置（与 MIN_ZONE_QI_TO_ALCHEMY 检查一致）。
        {
            let vfx_pos = alchemy_furnace_origin(furnace_pos);
            let world_pos = [vfx_pos.x, vfx_pos.y, vfx_pos.z];
            let zone_name = zones.as_deref().and_then(|z| {
                z.find_zone(
                    DimensionKind::Overworld,
                    valence::prelude::DVec3::new(
                        furnace_pos.0 as f64,
                        furnace_pos.1 as f64,
                        furnace_pos.2 as f64,
                    ),
                )
                .or_else(|| z.find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME))
                .map(|z| z.name.clone())
            });

            for (instance_id, _) in &selected_consumption {
                if let Some(item) = inventory_item_by_instance_mut(&mut inventory, *instance_id) {
                    if is_attrition_exempt(item) {
                        continue;
                    }
                    if let (Some(zone_name), Some(ref mut zones)) =
                        (zone_name.clone(), zones.as_deref_mut())
                    {
                        if let Some(zone) = zones.find_zone_mut(&zone_name) {
                            let before_abs_qi = item_abs_qi_for_attrition(item);
                            apply_attrition_checked(
                                item,
                                AttritionOpKind::AlchemyLoad,
                                Some(zone),
                                match &mut qi_transfers {
                                    Some(events) => Some(&mut **events),
                                    None => None,
                                },
                                tsy_lifecycle,
                            );
                            emit_attrition_applied_if_lost(
                                match &mut attrition_events {
                                    Some(events) => Some(&mut **events),
                                    None => None,
                                },
                                entity,
                                item,
                                before_abs_qi,
                                world_pos,
                            );
                        }
                    }
                }
            }
        }

        for (instance_id, selected_count) in selected_consumption {
            for _ in 0..selected_count {
                if let Err(error) = consume_item_instance_once(&mut inventory, instance_id) {
                    session.staged = staged_before_feed;
                    *inventory = inventory_before_feed;
                    send_alchemy_error(&mut client, &player_id, format!("投料扣除失败：{error}"));
                    return;
                }
            }
        }
        tracing::info!(
            "[bong][network][alchemy] `{player_id}` feed pos={furnace_pos:?} slot={slot_idx} {material}×{count}"
        );
        alchemy_snapshot_emit::send_session_from_furnace(
            &mut client,
            &player_id,
            furnace,
            registry,
        );
        if let (Ok(player_state), Ok(cultivation)) =
            (player_states.get(entity), cultivations.get(entity))
        {
            send_inventory_snapshot_to_client(
                entity,
                &mut client,
                username.0.as_str(),
                &inventory,
                player_state,
                cultivation,
                "alchemy_feed_slot",
            );
        }
    });
    log_or_send_route_error(result, &mut client, &player_id, furnace_pos, "feed_slot");
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_alchemy_take_back(
    entity: valence::prelude::Entity,
    furnace_pos: (i32, i32, i32),
    slot_idx: u8,
    tick: u64,
    clients: &mut Query<(&Username, &mut Client)>,
    furnaces: &mut Query<(Entity, &mut AlchemyFurnace)>,
    registry: &RecipeRegistry,
    outcome_tx: &mut Option<ResMut<Events<crate::alchemy::AlchemyOutcomeEvent>>>,
    inventories: &mut Query<&mut PlayerInventory>,
    player_states: &Query<&PlayerState>,
    cultivations: &Query<&Cultivation>,
    item_registry: &ItemRegistry,
    mut instance_allocator: Option<&mut InventoryInstanceIdAllocator>,
    vfx_events: Option<&mut Events<VfxEventRequest>>,
) {
    let Ok((username, mut client)) = clients.get_mut(entity) else {
        return;
    };
    let player_id = canonical_player_id(username.0.as_str());
    let result = with_owned_furnace_mut_with_entity(
        entity,
        &player_id,
        furnace_pos,
        furnaces,
        |furnace_entity, furnace| {
            let Some(session) = furnace.session.as_mut() else {
                send_alchemy_error(&mut client, &player_id, "尚未起炉".to_string());
                return;
            };
            let Some(recipe) = registry.get(&session.recipe) else {
                send_alchemy_error(
                    &mut client,
                    &player_id,
                    format!("未知丹方：{}", session.recipe),
                );
                return;
            };
            let remaining = recipe
                .fire_profile
                .target_duration_ticks
                .saturating_sub(session.elapsed_ticks);
            for _ in 0..remaining {
                session.tick();
            }
            session.finished = true;
            let Some(ended) = furnace.end_session() else {
                return;
            };
            let elapsed_ticks = ended.elapsed_ticks;
            // P3 — 催化炉加成：透传炉 tier 给 resolver，对变异丹配方叠加成功率加成。
            let resolved = crate::alchemy::resolver::resolve_with_meta_and_furnace(
                &ended,
                recipe,
                registry,
                0,
                furnace.tier,
            );
            let bucket = resolved.bucket;
            let outcome = resolved.outcome;
            let event_recipe_id = Some(recipe.id.clone());
            // end_session 已成功：无论产物入袋成败，都必须继续推送 finished/空炉终态，
            // 避免客户端残留 active HUD。奖励/VFX/outcome 事件仅在非 explode 且 grant 成功时触发。
            match &outcome {
                crate::alchemy::ResolvedOutcome::Explode {
                    damage,
                    meridian_crack,
                } => {
                    if let Some(events) = vfx_events {
                        gameplay_vfx::send_spawn(
                            events,
                            gameplay_vfx::spawn_request(
                                gameplay_vfx::ALCHEMY_EXPLODE,
                                alchemy_furnace_origin(furnace_pos),
                                Some([0.0, 0.8, 0.0]),
                                "#FF5533",
                                1.0,
                                18,
                                30,
                            ),
                        );
                    }
                    let scaled_damage = scale_alchemy_explosion_damage(*damage, furnace.tier);
                    let scaled_meridian_crack =
                        scale_alchemy_explosion_crack(*meridian_crack, furnace.tier);
                    furnace.apply_explode((*damage / 100.0).clamp(0.05, 0.75));
                    if let Some(instance_allocator) = instance_allocator.as_deref_mut() {
                        let _granted = grant_alchemy_outcome_item(
                            entity,
                            &mut client,
                            username.0.as_str(),
                            &player_id,
                            &outcome,
                            tick,
                            inventories,
                            player_states,
                            cultivations,
                            item_registry,
                            instance_allocator,
                        );
                    }
                    if let Some(outcome_tx) = outcome_tx.as_deref_mut() {
                        outcome_tx.send(crate::alchemy::AlchemyOutcomeEvent {
                            furnace: furnace_entity,
                            caster_id: player_id.clone(),
                            recipe_id: event_recipe_id.clone(),
                            bucket,
                            outcome: crate::alchemy::ResolvedOutcome::Explode {
                                damage: scaled_damage,
                                meridian_crack: scaled_meridian_crack,
                            },
                            elapsed_ticks,
                        });
                    }
                    client.send_chat_message(format!(
                        "§c[炼丹] 炸炉反噬：气血 -{scaled_damage:.1}，经脉裂痕 +{scaled_meridian_crack:.2}"
                    ));
                }
                _ => {
                    let granted = match instance_allocator {
                        Some(instance_allocator) => grant_alchemy_outcome_item(
                            entity,
                            &mut client,
                            username.0.as_str(),
                            &player_id,
                            &outcome,
                            tick,
                            inventories,
                            player_states,
                            cultivations,
                            item_registry,
                            instance_allocator,
                        ),
                        None => {
                            send_alchemy_error(
                                &mut client,
                                &player_id,
                                "炼丹产物入袋失败：实例编号器未就绪".to_string(),
                            );
                            false
                        }
                    };
                    if granted {
                        if let Some(events) = vfx_events {
                            gameplay_vfx::send_spawn(
                                events,
                                gameplay_vfx::spawn_request(
                                    gameplay_vfx::ALCHEMY_COMPLETE,
                                    alchemy_furnace_origin(furnace_pos),
                                    Some([0.0, 0.8, 0.0]),
                                    "#FFD700",
                                    0.9,
                                    10,
                                    40,
                                ),
                            );
                        }
                        if let Some(outcome_tx) = outcome_tx.as_deref_mut() {
                            outcome_tx.send(crate::alchemy::AlchemyOutcomeEvent {
                                furnace: furnace_entity,
                                caster_id: player_id.clone(),
                                recipe_id: event_recipe_id,
                                bucket,
                                outcome,
                                elapsed_ticks,
                            });
                        }
                    }
                }
            }
            tracing::info!(
                "[bong][network][alchemy] `{player_id}` take_back pos={furnace_pos:?} slot={slot_idx} resolved bucket={bucket:?}"
            );
            alchemy_snapshot_emit::send_furnace_from_furnace(&mut client, &player_id, furnace);
            alchemy_snapshot_emit::send_session_from_completed_session(
                &mut client,
                &player_id,
                &ended,
                registry,
            );
        },
    );
    log_or_send_route_error(result, &mut client, &player_id, furnace_pos, "take_back");
}

#[allow(clippy::too_many_arguments)]
fn grant_alchemy_outcome_item(
    entity: Entity,
    client: &mut Client,
    username: &str,
    player_id: &str,
    outcome: &crate::alchemy::ResolvedOutcome,
    tick: u64,
    inventories: &mut Query<&mut PlayerInventory>,
    player_states: &Query<&PlayerState>,
    cultivations: &Query<&Cultivation>,
    item_registry: &ItemRegistry,
    instance_allocator: &mut InventoryInstanceIdAllocator,
) -> bool {
    let (template_id, alchemy, reason) =
        if let Some(residue_kind) = residue_kind_for_recyclable_outcome(outcome) {
            (
                residue_kind.spec().template_id,
                Some(residue_alchemy_data(residue_kind, tick)),
                "alchemy_residue_grant",
            )
        } else if let crate::alchemy::ResolvedOutcome::Pill {
            pill,
            recipe_id,
            quality_tier,
            effect_multiplier,
            consecrated,
            side_effect,
            ..
        } = outcome
        {
            (
                pill.as_str(),
                Some(AlchemyItemData::Pill {
                    recipe_id: recipe_id.clone(),
                    quality_tier: *quality_tier,
                    effect_multiplier: *effect_multiplier,
                    consecrated: *consecrated,
                    side_effect: side_effect.clone(),
                }),
                "alchemy_outcome_grant",
            )
        } else {
            return false;
        };
    let Ok(mut inventory) = inventories.get_mut(entity) else {
        send_alchemy_error(
            client,
            player_id,
            "未找到背包，炼丹产物无法入袋".to_string(),
        );
        return false;
    };
    if let Err(error) = add_item_to_player_inventory_with_alchemy(
        &mut inventory,
        item_registry,
        instance_allocator,
        template_id,
        1,
        alchemy,
        tick,
    ) {
        send_alchemy_error(client, player_id, format!("炼丹产物入袋失败：{error}"));
        return false;
    }
    if let (Ok(player_state), Ok(cultivation)) =
        (player_states.get(entity), cultivations.get(entity))
    {
        send_inventory_snapshot_to_client(
            entity,
            client,
            username,
            &inventory,
            player_state,
            cultivation,
            reason,
        );
    }
    true
}

#[derive(Debug, PartialEq, Eq)]
enum AlchemyFurnaceRouteError {
    Missing,
    Forbidden { owner: Option<String> },
}

fn with_owned_furnace_mut<R>(
    player: Entity,
    player_id: &str,
    furnace_pos: (i32, i32, i32),
    furnaces: &mut Query<(Entity, &mut AlchemyFurnace)>,
    f: impl FnOnce(&mut AlchemyFurnace) -> R,
) -> Result<R, AlchemyFurnaceRouteError> {
    with_owned_furnace_mut_with_entity(player, player_id, furnace_pos, furnaces, |_, furnace| {
        f(furnace)
    })
}

fn with_owned_furnace_mut_with_entity<R>(
    _player: Entity,
    player_id: &str,
    furnace_pos: (i32, i32, i32),
    furnaces: &mut Query<(Entity, &mut AlchemyFurnace)>,
    f: impl FnOnce(Entity, &mut AlchemyFurnace) -> R,
) -> Result<R, AlchemyFurnaceRouteError> {
    let Some((furnace_entity, mut furnace)) = furnaces
        .iter_mut()
        .find(|(_, furnace)| furnace.pos == Some(furnace_pos))
    else {
        return Err(AlchemyFurnaceRouteError::Missing);
    };
    let owner_ok = match furnace.owner.as_deref() {
        None | Some("") => true,
        Some(owner) => {
            owner == player_id || owner == player_id.strip_prefix("offline:").unwrap_or(player_id)
        }
    };
    if !owner_ok {
        return Err(AlchemyFurnaceRouteError::Forbidden {
            owner: furnace.owner.clone(),
        });
    }
    Ok(f(furnace_entity, &mut furnace))
}

fn log_or_send_route_error(
    result: Result<(), AlchemyFurnaceRouteError>,
    client: &mut Client,
    player_id: &str,
    furnace_pos: (i32, i32, i32),
    action: &str,
) {
    match result {
        Ok(()) => {}
        Err(AlchemyFurnaceRouteError::Missing) => {
            tracing::warn!(
                "[bong][network][alchemy] `{player_id}` {action} rejected: missing furnace pos={furnace_pos:?}"
            );
            send_alchemy_error(client, player_id, format!("炼丹炉不存在：{furnace_pos:?}"));
        }
        Err(AlchemyFurnaceRouteError::Forbidden { owner }) => {
            tracing::warn!(
                "[bong][network][alchemy] `{player_id}` {action} rejected: forbidden pos={furnace_pos:?} owner={owner:?}"
            );
            send_alchemy_error(client, player_id, "这座炉不是你的".to_string());
        }
    }
}

fn send_alchemy_error(client: &mut Client, player_id: &str, message: String) {
    client.send_chat_message(format!("§c[炼丹] {message}"));
    tracing::warn!("[bong][network][alchemy] error for `{player_id}`: {message}");
}

fn publish_alchemy_session_start(
    redis: Option<&RedisBridgeResource>,
    furnace_pos: (i32, i32, i32),
    furnace_tier: u8,
    recipe_id: &str,
    caster_id: &str,
) {
    let Some(redis) = redis else {
        return;
    };
    let payload = AlchemySessionStartV1 {
        v: 1,
        session_id: alchemy_session_id(furnace_pos, caster_id, recipe_id),
        recipe_id: recipe_id.to_string(),
        furnace_pos,
        furnace_tier,
        caster_id: caster_id.to_string(),
        ts: current_unix_millis(),
    };
    let _ = redis
        .tx_outbound
        .send(RedisOutbound::AlchemySessionStart(payload));
}

fn publish_alchemy_intervention_result(
    redis: Option<&RedisBridgeResource>,
    furnace_pos: (i32, i32, i32),
    recipe_id: &str,
    caster_id: &str,
    intervention: &Intervention,
    temp_current: f64,
    qi_injected: f64,
) {
    let Some(redis) = redis else {
        return;
    };
    let payload = AlchemyInterventionResultV1 {
        v: 1,
        session_id: alchemy_session_id(furnace_pos, caster_id, recipe_id),
        recipe_id: recipe_id.to_string(),
        furnace_pos,
        caster_id: caster_id.to_string(),
        intervention: crate::schema::alchemy::AlchemyInterventionV1::from(intervention),
        temp_current,
        qi_injected,
        accepted: true,
        message: None,
        ts: current_unix_millis(),
    };
    let _ = redis
        .tx_outbound
        .send(RedisOutbound::AlchemyInterventionResult(payload));
}

fn scale_alchemy_explosion_damage(base_damage: f64, furnace_tier: u8) -> f64 {
    if !base_damage.is_finite() || base_damage <= 0.0 {
        return 0.0;
    }
    let tier = furnace_tier.clamp(1, 3) as f64;
    base_damage * (1.0 + (tier - 1.0) * 0.5)
}

fn scale_alchemy_explosion_crack(base_severity: f64, furnace_tier: u8) -> f64 {
    if !base_severity.is_finite() || base_severity <= 0.0 {
        return 0.0;
    }
    let tier = furnace_tier.clamp(1, 3) as f64;
    (base_severity * (1.0 + (tier - 1.0) * 0.25)).clamp(0.0, 1.0)
}

fn furnace_zone_is_collapsed(
    furnace: &AlchemyFurnace,
    zone_registry: Option<&ZoneRegistry>,
) -> bool {
    let Some(zone_registry) = zone_registry else {
        return false;
    };
    let Some((x, y, z)) = furnace.pos else {
        return false;
    };
    let furnace_pos = DVec3::new(x as f64 + 0.5, y as f64, z as f64 + 0.5);
    zone_registry
        .find_zone(DimensionKind::Overworld, furnace_pos)
        .is_some_and(|zone| {
            zone.active_events
                .iter()
                .any(|event| event == EVENT_REALM_COLLAPSE)
        })
}

/// plan-cultivation-v1 §3.1：玩家服用 pill → 扣一颗 → 根据 ItemEffect 分派运行时效果。
/// `BreakthroughBonus` / `QiRecovery` 已有运行时接入；
/// 其他 kind（MeridianHeal/ContaminationCleanse）待对应 tick 系统就位。
#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_alchemy_take_pill(
    entity: Entity,
    pill_item_id: &str,
    instance_id: Option<u64>,
    commands: &mut Commands,
    clock: &CombatClock,
    inventories: &mut Query<&mut PlayerInventory>,
    clients: &mut Query<(&Username, &mut Client)>,
    player_states: &Query<&PlayerState>,
    cultivations: &Query<&Cultivation>,
    combat_params: &mut CombatRequestParams,
    lifespan_extension_tx: &mut Option<ResMut<Events<LifespanExtensionIntent>>>,
    vfx_events: Option<&mut Events<VfxEventRequest>>,
    audio_events: &mut Option<ResMut<Events<PlaySoundRecipeRequest>>>,
    hallucination_events: Option<
        &mut Events<crate::fauna::hybrid_beast::CoreAbsorptionHallucinationEvent>,
    >,
    narrations: Option<&mut crate::player::gameplay::PendingGameplayNarrations>,
) {
    let Some(template) = combat_params.item_registry.get(pill_item_id).cloned() else {
        tracing::warn!(
            "[bong][network][alchemy] take_pill entity={entity:?} unknown template `{pill_item_id}`"
        );
        return;
    };
    let Some(effect) = template.effect.clone() else {
        tracing::warn!(
            "[bong][network][alchemy] take_pill entity={entity:?} `{pill_item_id}` has no effect"
        );
        return;
    };

    let mut inventory = match inventories.get_mut(entity) {
        Ok(inv) => inv,
        Err(_) => {
            tracing::warn!(
                "[bong][network][alchemy] take_pill entity={entity:?} no PlayerInventory"
            );
            return;
        }
    };
    let Some(consumed_item) = resolve_pill_consume_target(&inventory, pill_item_id, instance_id)
    else {
        tracing::warn!(
            "[bong][network][alchemy] take_pill entity={entity:?} `{pill_item_id}` not in inventory"
        );
        return;
    };
    let (alchemy_multiplier, alchemy_consecrated, alchemy_side_effect) =
        match consumed_item.alchemy.as_ref() {
            Some(AlchemyItemData::Pill {
                effect_multiplier,
                consecrated,
                side_effect,
                ..
            }) => (*effect_multiplier, *consecrated, side_effect.clone()),
            _ => (1.0, false, None),
        };
    let duration_multiplier = if alchemy_consecrated { 2 } else { 1 };
    let foreign_qi = foreign_qi_resistance_for_use(
        &template,
        consumed_item
            .lingering_owner_qi
            .as_ref()
            .is_some_and(|lingering| clock.tick < lingering.expire_at),
    );

    let (spoil, age) = shelflife_checks_for_item(
        &consumed_item,
        clock.tick,
        combat_params.decay_profiles.as_deref(),
        combat_params.season_state.as_deref(),
    );
    emit_shelflife_consume_events(
        entity,
        consumed_item.instance_id,
        &spoil,
        &age,
        &mut combat_params.spoil_warnings,
        &mut combat_params.age_bonus_rolls,
    );

    if matches!(spoil, SpoilCheckOutcome::CriticalBlock { .. }) {
        tracing::warn!(
            "[bong][network][alchemy] take_pill entity={entity:?} `{pill_item_id}` blocked by spoil CriticalBlock"
        );
        resync_snapshot(
            entity,
            &inventory,
            clients,
            player_states,
            cultivations,
            "take_pill_spoil_blocked",
        );
        return;
    }

    let poison_pill_kind = match &effect {
        ItemEffect::PoisonPill { pill_item_id } => {
            let Some(kind) = PoisonPillKind::from_item_id(pill_item_id.as_str()) else {
                tracing::warn!(
                    "[bong][network][alchemy] take_pill entity={entity:?} unknown poison pill id `{pill_item_id}`"
                );
                resync_snapshot(
                    entity,
                    &inventory,
                    clients,
                    player_states,
                    cultivations,
                    "take_pill_poison_invalid",
                );
                return;
            };
            if combat_params.poison_pill_tx.is_none() {
                tracing::warn!(
                    "[bong][network][alchemy] take_pill entity={entity:?} poison intent resource missing"
                );
                resync_snapshot(
                    entity,
                    &inventory,
                    clients,
                    player_states,
                    cultivations,
                    "take_pill_poison_unavailable",
                );
                return;
            }
            Some(kind)
        }
        _ => None,
    };

    // plan-food-v1 P2 / plan-consumable-effects-v1：这些效果必须走 quick slot（cast_emit 路径）。
    // 在此处前置拒绝，避免 consume_item_instance_once 扣掉物品后 noop。
    if matches!(
        effect,
        ItemEffect::FoodRegen { .. }
            | ItemEffect::ComposureRestore { .. }
            | ItemEffect::WoundHeal { .. }
    ) {
        tracing::debug!(
            "[bong][network][alchemy] take_pill entity={entity:?} `{pill_item_id}` rejected: effect must be consumed via quick slot"
        );
        resync_snapshot(
            entity,
            &inventory,
            clients,
            player_states,
            cultivations,
            "take_pill_food_rejected",
        );
        return;
    }

    let consume_result = consume_item_instance_once(&mut inventory, consumed_item.instance_id);
    if let Err(error) = consume_result {
        tracing::warn!(
            "[bong][network][alchemy] take_pill entity={entity:?} `{pill_item_id}` consume failed: {error}"
        );
        return;
    }
    if foreign_qi.health_loss > 0.0 {
        if let Ok(mut wounds) = combat_params.wounds.get_mut(entity) {
            wounds.health_current =
                (wounds.health_current - foreign_qi.health_loss).clamp(0.0, wounds.health_max);
        }
        tracing::info!(
            "[bong][network][alchemy] take_pill entity={entity:?} `{pill_item_id}` triggered foreign qi rejection: effect_multiplier={:.2} health_loss={:.1}",
            foreign_qi.effect_multiplier,
            foreign_qi.health_loss
        );
    }

    // P0 dandao-runtime-wiring: 服丹丹毒增量，用于 emit PillIntakeTracked。
    // CombatPill 路径走 consume_pill 产生真实丹毒；其他路径无丹毒产生（0.0 → 跳过 emit）。
    let toxin_for_intake: f64 = match &effect {
        ItemEffect::CombatPill { pill_item_id } => {
            crate::alchemy::pill::combat_pill_spec(pill_item_id)
                .map(|spec| spec.toxin_amount)
                .unwrap_or(0.0)
        }
        _ => 0.0,
    };

    let mut cultivation_snapshot_override = None;
    match effect {
        ItemEffect::BreakthroughBonus { magnitude } => {
            let scaled_magnitude = magnitude * alchemy_multiplier * foreign_qi.effect_multiplier;
            combat_params.buff_tx.send(ApplyStatusEffectIntent {
                target: entity,
                kind: StatusEffectKind::BreakthroughBoost,
                magnitude: scaled_magnitude as f32,
                duration_ticks: BREAKTHROUGH_BOOST_DURATION_TICKS * duration_multiplier,
                issued_at_tick: clock.tick,
            });
            tracing::info!(
                "[bong][network][alchemy] take_pill entity={entity:?} `{pill_item_id}` → BreakthroughBoost +{scaled_magnitude:.3} for {} ticks",
                BREAKTHROUGH_BOOST_DURATION_TICKS * duration_multiplier
            );
        }
        ItemEffect::QiRecovery { amount } => {
            if let Ok(current) = cultivations.get(entity) {
                let mut cultivation = current.clone();
                let qi_max_before = cultivation.qi_max;
                let recovered = recover_current_qi(
                    &mut cultivation,
                    amount * alchemy_multiplier * foreign_qi.effect_multiplier,
                );
                cultivation_snapshot_override = Some(cultivation.clone());
                commands.entity(entity).insert(cultivation);
                tracing::info!(
                    "[bong][network][alchemy] take_pill entity={entity:?} `{pill_item_id}` recovered current qi +{recovered:.1}; qi_max stays {qi_max_before:.1}"
                );
            } else {
                tracing::debug!(
                    "[bong][network][alchemy] take_pill entity={entity:?} `{pill_item_id}` QiRecovery noop: no Cultivation"
                );
            }
        }
        ItemEffect::LifespanExtension { years, source } => {
            if let Some(lifespan_extension_tx) = lifespan_extension_tx.as_deref_mut() {
                let requested_years =
                    ((f64::from(years) * foreign_qi.effect_multiplier).round() as u32).max(1);
                lifespan_extension_tx.send(LifespanExtensionIntent {
                    entity,
                    requested_years,
                    source: source.clone(),
                });
            }
            tracing::info!(
                "[bong][network][alchemy] take_pill entity={entity:?} lifespan extension {years} years source={source}"
            );
        }
        ItemEffect::AntiSpiritPressure { duration_ticks } => {
            let effective_duration_ticks =
                (duration_ticks as f64 * foreign_qi.effect_multiplier).round() as u64;
            combat_params.buff_tx.send(ApplyStatusEffectIntent {
                target: entity,
                kind: StatusEffectKind::AntiSpiritPressurePill,
                magnitude: 1.0,
                duration_ticks: effective_duration_ticks
                    .max(1)
                    .saturating_mul(duration_multiplier),
                issued_at_tick: clock.tick,
            });
            tracing::info!(
                "[bong][network][alchemy] take_pill entity={entity:?} `{pill_item_id}` → AntiSpiritPressurePill for {} ticks",
                duration_ticks.saturating_mul(duration_multiplier)
            );
        }
        ItemEffect::PoisonPill { pill_item_id } => {
            match (
                poison_pill_kind,
                combat_params.poison_pill_tx.as_deref_mut(),
            ) {
                (Some(pill), Some(poison_pill_tx)) => {
                    poison_pill_tx.send(ConsumePoisonPillIntent {
                        entity,
                        pill,
                        issued_at_tick: clock.tick,
                    });
                    tracing::info!(
                        "[bong][network][alchemy] take_pill entity={entity:?} `{pill_item_id}` → PoisonToxicity intent"
                    );
                }
                (None, _) => {
                    tracing::warn!("[bong][network][alchemy] take_pill entity={entity:?} poisoned pill prevalidation disappeared for `{pill_item_id}`");
                }
                (_, None) => {
                    tracing::warn!(
                        "[bong][network][alchemy] take_pill entity={entity:?} poison intent resource missing"
                    );
                }
            }
        }
        ItemEffect::CombatPill { pill_item_id } => {
            apply_combat_pill_runtime(
                entity,
                pill_item_id.as_str(),
                &template.id,
                alchemy_multiplier,
                foreign_qi.effect_multiplier,
                duration_multiplier,
                commands,
                clock,
                cultivations,
                combat_params,
                &spoil,
                &age,
                &mut cultivation_snapshot_override,
                vfx_events,
                audio_events,
                clients,
            );
        }
        ItemEffect::MeridianHeal { .. } | ItemEffect::ContaminationCleanse { .. } => {
            let meridians = combat_params.meridians.get_mut(entity).ok();
            let contamination = combat_params.contaminations.get_mut(entity).ok();
            apply_item_effect(
                &effect,
                None,
                meridians,
                contamination,
                None,
                pill_item_id,
                entity,
            );
        }
        ItemEffect::ComposureRestore { .. }
        | ItemEffect::WoundHeal { .. }
        | ItemEffect::FoodRegen { .. } => {
            tracing::debug!(
                "[bong][network][alchemy] take_pill entity={entity:?} `{pill_item_id}` quick-slot-only effect reached pill dispatch after prevalidation"
            );
        }
        // plan-fauna-stitched-beast-v1 P3 — 异变兽核吸收：突破加成 + 幻觉 HUD。
        //
        // 步骤：
        // 1. 突破加成（同 BreakthroughBonus）：emit ApplyStatusEffectIntent(BreakthroughBoost)
        // 2. 解析玩家 char_id（Username 组件）
        // 3. emit CoreAbsorptionHallucinationEvent（duration_ticks=固定）
        // 4. S2C push `bong:core_absorption_hallucination` JSON payload（{duration_ticks, cancel:false}）
        // 5. 叙事 1：Perception "核心涌入经脉，感知开始扭曲..."（scope=player）
        // 6. 叙事 2：Perception "眼前世界开始倾斜，手中真元似乎不受控..."（scope=player）
        //
        // 守恒红线：幻觉仅改变 client 显示层，不改玩家实际 HP / qi_current。
        ItemEffect::BeastCoreAbsorption {
            breakthrough_magnitude,
            hallucination_duration_ticks,
        } => {
            // 1. 突破加成（同 BreakthroughBonus 路径）
            let scaled_magnitude =
                breakthrough_magnitude * alchemy_multiplier * foreign_qi.effect_multiplier;
            combat_params.buff_tx.send(ApplyStatusEffectIntent {
                target: entity,
                kind: StatusEffectKind::BreakthroughBoost,
                magnitude: scaled_magnitude as f32,
                duration_ticks: BREAKTHROUGH_BOOST_DURATION_TICKS * duration_multiplier,
                issued_at_tick: clock.tick,
            });
            tracing::info!(
                "[bong][network][alchemy] take_pill entity={entity:?} `{pill_item_id}` → BeastCoreAbsorption BreakthroughBoost +{scaled_magnitude:.3}",
            );

            // 2. 获取 player char_id（"offline:{username}"）
            let player_char_id = clients
                .get(entity)
                .ok()
                .map(|(username, _)| format!("offline:{}", username.0))
                .unwrap_or_else(|| format!("char:{}", entity.to_bits()));

            // 3. emit CoreAbsorptionHallucinationEvent（server 内部事件）
            if let Some(hall_events) = hallucination_events {
                hall_events.send(
                    crate::fauna::hybrid_beast::CoreAbsorptionHallucinationEvent {
                        player_id: player_char_id.clone(),
                        duration_ticks: hallucination_duration_ticks,
                    },
                );
            }

            // 4. S2C push `bong:core_absorption_hallucination` JSON → client
            // payload: {"duration_ticks": N, "cancel": false}
            // duration_ticks=0 表示立即取消（断线或到期时用）；正值表示激活幻觉。
            let s2c_payload = format!(
                r#"{{"duration_ticks":{},"cancel":false}}"#,
                hallucination_duration_ticks
            );
            let s2c_bytes = s2c_payload.as_bytes().to_vec();
            if let Ok((_, mut client)) = clients.get_mut(entity) {
                client.send_custom_payload(
                    valence::prelude::ident!("bong:core_absorption_hallucination"),
                    &s2c_bytes,
                );
            }
            tracing::info!(
                "[bong][network][alchemy] take_pill entity={entity:?} → CoreAbsorptionHallucination S2C pushed (duration_ticks={})",
                hallucination_duration_ticks
            );

            // 5. 叙事 1：感知层第一条（真元冲击感知）
            if let Some(narrations) = narrations {
                use crate::schema::common::NarrationStyle;
                narrations.push_player(
                    &player_char_id,
                    "核心涌入经脉，真元震荡——感知开始扭曲，世界的边缘模糊成绿色光晕。",
                    NarrationStyle::Perception,
                );
                // 6. 叙事 2：感知层第二条（失控感）
                narrations.push_player(
                    &player_char_id,
                    "眼前景物倾斜偏转，手中真元似乎不再听从驱使——这是异兽核心的驻波共鸣。",
                    NarrationStyle::Perception,
                );
            }
        }
    }

    if let Some(side_effect) = alchemy_side_effect.as_ref() {
        let realm = cultivations
            .get(entity)
            .map(|cultivation| cultivation.realm)
            .unwrap_or(crate::cultivation::components::Realm::Awaken);
        let application = crate::alchemy::side_effect_apply::build_side_effect_application(
            entity,
            side_effect,
            clock.tick,
            realm,
        );
        combat_params.buff_tx.send(application.status_intent);
        if let (Some(insight_request), Some(insight_request_tx)) = (
            application.insight_request,
            combat_params.insight_request_tx.as_mut(),
        ) {
            insight_request_tx.send(insight_request);
        }
    }

    // P0 dandao-runtime-wiring: 服丹成功 → emit PillIntakeTracked（toxin > 0 时）。
    // track_pill_intake_system 读此事件并追加 PracticeLog Mellow 权重。
    // 无毒丹（toxin_for_intake == 0.0）不 emit，语义：仅当本次确实产生丹毒时才记录。
    if toxin_for_intake > 0.0 {
        if let Some(tx) = combat_params.pill_intake_tx.as_deref_mut() {
            tx.send(crate::dandao::toxin_tracker::PillIntakeTracked {
                entity,
                toxin_amount: toxin_for_intake,
                new_stage: None,
            });
        }
    }

    resync_snapshot_with_cultivation_override(
        entity,
        &inventory,
        clients,
        player_states,
        cultivations,
        cultivation_snapshot_override.as_ref(),
        "take_pill",
    );
}

#[allow(clippy::too_many_arguments)]
fn apply_combat_pill_runtime(
    entity: Entity,
    pill_item_id: &str,
    template_id: &str,
    alchemy_multiplier: f64,
    foreign_qi_multiplier: f64,
    duration_multiplier: u64,
    commands: &mut Commands,
    clock: &CombatClock,
    cultivations: &Query<&Cultivation>,
    combat_params: &mut CombatRequestParams,
    spoil: &crate::shelflife::SpoilCheckOutcome,
    age: &crate::shelflife::AgePeakCheck,
    cultivation_snapshot_override: &mut Option<Cultivation>,
    vfx_events: Option<&mut Events<VfxEventRequest>>,
    audio_events: &mut Option<ResMut<Events<PlaySoundRecipeRequest>>>,
    clients: &mut Query<(&Username, &mut Client)>,
) {
    let Some(spec) = crate::alchemy::pill::combat_pill_spec(pill_item_id) else {
        tracing::warn!(
            "[bong][network][alchemy] take_pill entity={entity:?} `{template_id}` references unknown combat pill `{pill_item_id}`"
        );
        return;
    };

    let base_cultivation = cultivations.get(entity).ok().cloned().unwrap_or_default();
    let mut next_cultivation = base_cultivation.clone();
    let (realm_pos_scale, realm_neg_scale) =
        crate::alchemy::pill::mortal_pill_realm_scale(base_cultivation.realm);
    let pos_scale =
        (realm_pos_scale * alchemy_multiplier as f32 * foreign_qi_multiplier as f32).max(0.0);
    let neg_scale = realm_neg_scale.max(0.0);

    if let Ok(mut contamination) = combat_params.contaminations.get_mut(entity) {
        let pill_effect = crate::alchemy::pill::PillEffect {
            toxin_amount: spec.toxin_amount,
            toxin_color: spec.toxin_color,
            qi_gain: None,
            meridian_progress_bonus: None,
        };
        let _ = crate::alchemy::pill::consume_pill(
            &pill_effect,
            &mut contamination,
            &mut next_cultivation,
            clock.tick,
            spoil.clone(),
            false,
            age.clone(),
        );
    }

    let mut touched_cultivation = false;
    if let Ok(mut wounds) = combat_params.wounds.get_mut(entity) {
        use crate::alchemy::pill::{
            apply_severed_mend, apply_wound_heal, apply_wound_worsen, scaled_grades,
            worst_non_severed_part, worst_severed_part, CombatPillKind,
        };
        match spec.kind {
            CombatPillKind::HuoXueDan => {
                let grades = scaled_grades(1, pos_scale);
                apply_wound_heal(&mut wounds, None, grades);
            }
            CombatPillKind::XuGuGao => {
                let target = worst_non_severed_part(&wounds);
                let grades = scaled_grades(2, pos_scale);
                apply_wound_heal(&mut wounds, target, grades);
            }
            CombatPillKind::DuanXuSan => {
                let target = worst_severed_part(&wounds);
                apply_severed_mend(&mut wounds, target, pos_scale);
                let qi_max_before = next_cultivation.qi_max;
                next_cultivation.qi_max = (next_cultivation.qi_max * 0.97).max(0.0);
                next_cultivation.qi_current =
                    next_cultivation.qi_current.min(next_cultivation.qi_max);
                touched_cultivation |=
                    (qi_max_before - next_cultivation.qi_max).abs() > f64::EPSILON;
            }
            CombatPillKind::SuoDiSan => {
                let grades = scaled_grades(1, neg_scale);
                apply_wound_worsen(
                    &mut wounds,
                    &[
                        crate::combat::components::BodyPart::LegL,
                        crate::combat::components::BodyPart::LegR,
                    ],
                    grades,
                    clock.tick,
                    Some(format!("alchemy:{pill_item_id}")),
                );
            }
            _ => {}
        }
    }

    if let Ok(mut stamina) = combat_params.staminas.get_mut(entity) {
        if spec.kind == crate::alchemy::pill::CombatPillKind::HuGuSan {
            let boosted_max = (100.0 * (1.0 + 0.50 * pos_scale)).max(stamina.max);
            stamina.max = boosted_max;
            stamina.current = stamina.current.max(boosted_max * 0.80).min(boosted_max);
        }
    }

    for mut intent in crate::alchemy::pill::combat_pill_status_intents(
        entity, spec, pos_scale, neg_scale, clock.tick,
    ) {
        intent.duration_ticks = intent
            .duration_ticks
            .saturating_mul(duration_multiplier.max(1));
        combat_params.buff_tx.send(intent);
    }
    push_combat_pill_buff_status(clients, entity, spec, pos_scale, duration_multiplier);

    if touched_cultivation {
        commands.entity(entity).insert(next_cultivation.clone());
        *cultivation_snapshot_override = Some(next_cultivation);
    }

    emit_combat_pill_feedback(
        entity,
        spec,
        &combat_params.positions,
        &combat_params.unique_ids,
        vfx_events,
        audio_events,
    );
    push_combat_pill_event_stream(
        clients,
        entity,
        spec.id,
        &format!("服下{}，药力入体。", spec.name),
        if realm_pos_scale < 1.0 { 0xFFFFA040 } else { 0 },
    );
}

fn emit_combat_pill_feedback(
    entity: Entity,
    spec: crate::alchemy::pill::CombatPillSpec,
    positions: &Query<&valence::prelude::Position>,
    unique_ids: &Query<&UniqueId>,
    vfx_events: Option<&mut Events<VfxEventRequest>>,
    audio_events: &mut Option<ResMut<Events<PlaySoundRecipeRequest>>>,
) {
    let Ok(position) = positions.get(entity) else {
        return;
    };
    let origin = position.get();
    if let Some(events) = vfx_events {
        if let Ok(unique_id) = unique_ids.get(entity) {
            events.send(VfxEventRequest::new(
                origin,
                crate::schema::vfx_event::VfxEventPayloadV1::PlayAnim {
                    target_player: unique_id.0.to_string(),
                    anim_id: spec.animation_id.to_string(),
                    priority: 250,
                    fade_in_ticks: Some(2),
                },
            ));
        }
        events.send(VfxEventRequest::new(
            origin,
            crate::schema::vfx_event::VfxEventPayloadV1::SpawnParticle {
                event_id: spec.vfx_event_id.to_string(),
                origin: [origin.x, origin.y + 1.0, origin.z],
                direction: Some([0.0, 1.0, 0.0]),
                color: None,
                strength: Some(0.75),
                count: Some(12),
                duration_ticks: Some(30),
            },
        ));
    }
    if let Some(audio_events) = audio_events.as_deref_mut() {
        audio_events.send(PlaySoundRecipeRequest {
            recipe_id: spec.audio_recipe_id.to_string(),
            instance_id: 0,
            pos: None,
            flag: None,
            volume_mul: 1.0,
            pitch_shift: 0.0,
            recipient: AudioRecipient::Radius {
                origin,
                radius: crate::network::audio_event_emit::AUDIO_BROADCAST_RADIUS,
            },
        });
    }
}

fn push_combat_pill_buff_status(
    clients: &mut Query<(&Username, &mut Client)>,
    entity: Entity,
    spec: crate::alchemy::pill::CombatPillSpec,
    effect_multiplier: f32,
    duration_multiplier: u64,
) {
    let Some(payload_bytes) = build_pill_buff_status_payload(
        spec.id,
        spec.positive_duration_ticks,
        effect_multiplier,
        duration_multiplier,
    ) else {
        return;
    };
    let Ok((_username, mut client)) = clients.get_mut(entity) else {
        return;
    };
    send_server_data_payload(&mut client, payload_bytes.as_slice());
}

fn build_pill_buff_status_payload(
    buff_id: &str,
    base_remaining_ticks: u64,
    effect_multiplier: f32,
    duration_multiplier: u64,
) -> Option<Vec<u8>> {
    if buff_id.trim().is_empty() || !effect_multiplier.is_finite() || effect_multiplier <= 0.0 {
        return None;
    }
    let remaining_ticks = base_remaining_ticks
        .saturating_mul(duration_multiplier.max(1))
        .min(u64::from(u32::MAX)) as u32;
    let payload = ServerDataV1::new(ServerDataPayloadV1::PillBuffStatus(PillBuffStatusV1 {
        buff_id: buff_id.to_string(),
        remaining_ticks,
        effect_multiplier: f64::from(effect_multiplier),
    }));
    match serialize_server_data_payload(&payload) {
        Ok(bytes) => Some(bytes),
        Err(error) => {
            tracing::warn!(
                "[bong][network][alchemy] failed to serialize pill_buff_status for {buff_id}: {error:?}"
            );
            None
        }
    }
}

fn push_combat_pill_event_stream(
    clients: &mut Query<(&Username, &mut Client)>,
    entity: Entity,
    source_tag: &str,
    text: &str,
    color: u32,
) {
    let Ok((_username, mut client)) = clients.get_mut(entity) else {
        return;
    };
    let payload = crate::schema::server_data::ServerDataV1::new(
        crate::schema::server_data::ServerDataPayloadV1::EventStreamPush(
            crate::schema::combat_hud::EventStreamPushV1 {
                channel: crate::schema::combat_hud::EventChannelV1::Combat,
                priority: crate::schema::combat_hud::EventPriorityV1::P1Important,
                source_tag: format!("alchemy:{source_tag}"),
                text: text.to_string(),
                color,
                created_at_ms: current_unix_millis(),
            },
        ),
    );
    let Ok(payload_bytes) = serialize_server_data_payload(&payload) else {
        return;
    };
    send_server_data_payload(&mut client, payload_bytes.as_slice());
}

/// 扣除一颗 template 匹配的 item（优先 hotbar → containers → equipped）。
/// stack_count > 1 时减 1；否则移除整个 slot/placement。成功返回 true。
#[cfg(test)]
fn consume_one_by_template(inventory: &mut PlayerInventory, template_id: &str) -> bool {
    for slot in inventory.hotbar.iter_mut() {
        if let Some(item) = slot.as_mut() {
            if item.template_id == template_id {
                if item.stack_count > 1 {
                    item.stack_count -= 1;
                } else {
                    *slot = None;
                }
                inventory.revision.0 = inventory.revision.0.saturating_add(1);
                return true;
            }
        }
    }
    for container in inventory.containers.iter_mut() {
        if let Some(idx) = container
            .items
            .iter()
            .position(|p| p.instance.template_id == template_id)
        {
            if container.items[idx].instance.stack_count > 1 {
                container.items[idx].instance.stack_count -= 1;
            } else {
                container.items.remove(idx);
            }
            inventory.revision.0 = inventory.revision.0.saturating_add(1);
            return true;
        }
    }
    for (_, contents) in inventory.equipped.iter_mut() {
        if let Some(pos) = contents
            .worn
            .iter()
            .position(|item| item.template_id == template_id)
        {
            if contents.worn[pos].stack_count > 1 {
                contents.worn[pos].stack_count -= 1;
            } else {
                contents.worn.remove(pos);
            }
            inventory.revision.0 = inventory.revision.0.saturating_add(1);
            return true;
        }
        if contents
            .held
            .as_ref()
            .is_some_and(|item| item.template_id == template_id)
        {
            let held = contents.held.as_mut().unwrap();
            if held.stack_count > 1 {
                held.stack_count -= 1;
            } else {
                contents.held = None;
            }
            inventory.revision.0 = inventory.revision.0.saturating_add(1);
            return true;
        }
    }
    false
}

#[cfg(test)]
fn select_template_instances_for_consumption(
    inventory: &PlayerInventory,
    template_id: &str,
    required: u32,
) -> Vec<u64> {
    let mut remaining = required;
    let mut instance_ids = Vec::new();
    if remaining == 0 {
        return instance_ids;
    }

    for item in inventory.hotbar.iter().flatten() {
        if item.template_id == template_id && item.stack_count > 0 {
            instance_ids.push(item.instance_id);
            remaining = remaining.saturating_sub(item.stack_count);
            if remaining == 0 {
                return instance_ids;
            }
        }
    }
    for container in &inventory.containers {
        for placed in &container.items {
            let item = &placed.instance;
            if item.template_id == template_id && item.stack_count > 0 {
                instance_ids.push(item.instance_id);
                remaining = remaining.saturating_sub(item.stack_count);
                if remaining == 0 {
                    return instance_ids;
                }
            }
        }
    }
    for item in inventory.equipped.values().flat_map(|s| s.iter_all()) {
        if item.template_id == template_id && item.stack_count > 0 {
            instance_ids.push(item.instance_id);
            remaining = remaining.saturating_sub(item.stack_count);
            if remaining == 0 {
                return instance_ids;
            }
        }
    }
    instance_ids
}

fn select_ingredient_instances_for_consumption(
    inventory: &PlayerInventory,
    ingredient: &crate::alchemy::recipe::IngredientSpec,
    required: u32,
) -> Option<Vec<(u64, u32)>> {
    let mut remaining = required;
    let mut selected = Vec::new();
    if remaining == 0 {
        return Some(selected);
    }

    for item in inventory.hotbar.iter().flatten() {
        select_ingredient_item(ingredient, item, &mut remaining, &mut selected);
        if remaining == 0 {
            return Some(selected);
        }
    }
    for container in &inventory.containers {
        for placed in &container.items {
            select_ingredient_item(ingredient, &placed.instance, &mut remaining, &mut selected);
            if remaining == 0 {
                return Some(selected);
            }
        }
    }
    for item in inventory.equipped.values().flat_map(|s| s.iter_all()) {
        select_ingredient_item(ingredient, item, &mut remaining, &mut selected);
        if remaining == 0 {
            return Some(selected);
        }
    }

    None
}

fn select_ingredient_item(
    ingredient: &crate::alchemy::recipe::IngredientSpec,
    item: &ItemInstance,
    remaining: &mut u32,
    selected: &mut Vec<(u64, u32)>,
) {
    if *remaining == 0 || item.template_id != ingredient.material || item.stack_count == 0 {
        return;
    }
    if ingredient.validate_item(item).is_err() {
        return;
    }
    let take = (*remaining).min(item.stack_count);
    selected.push((item.instance_id, take));
    *remaining -= take;
}

fn resolve_pill_consume_target(
    inventory: &PlayerInventory,
    template_id: &str,
    instance_id: Option<u64>,
) -> Option<crate::inventory::ItemInstance> {
    if let Some(instance_id) = instance_id {
        return inventory_item_by_instance_borrow(inventory, instance_id)
            .and_then(|item| (item.template_id == template_id).then(|| item.clone()));
    }

    inventory
        .hotbar
        .iter()
        .flatten()
        .find(|item| item.template_id == template_id)
        .cloned()
        .or_else(|| {
            inventory
                .containers
                .iter()
                .flat_map(|container| container.items.iter())
                .find(|placed| placed.instance.template_id == template_id)
                .map(|placed| placed.instance.clone())
        })
        .or_else(|| {
            inventory
                .equipped
                .values()
                .flat_map(|s| s.iter_all())
                .find(|item| item.template_id == template_id)
                .cloned()
        })
}

fn shelflife_checks_for_item(
    item: &crate::inventory::ItemInstance,
    now_tick: u64,
    profiles: Option<&DecayProfileRegistry>,
    season_state: Option<&WorldSeasonState>,
) -> (SpoilCheckOutcome, AgePeakCheck) {
    let Some(freshness) = item.freshness.as_ref() else {
        return (
            SpoilCheckOutcome::NotApplicable,
            AgePeakCheck::NotApplicable,
        );
    };
    let Some(profile) = profiles.and_then(|profiles| profiles.get(&freshness.profile)) else {
        tracing::warn!(
            "[bong][network][alchemy] freshness profile `{}` missing for consumed item instance={}",
            freshness.profile.as_str(),
            item.instance_id
        );
        return (
            SpoilCheckOutcome::NotApplicable,
            AgePeakCheck::NotApplicable,
        );
    };

    let multiplier = container_storage_multiplier(&ContainerFreshnessBehavior::Normal, profile);
    let season = season_state
        .map(|state| state.current.season)
        .unwrap_or_else(|| query_season("", now_tick).season);
    (
        spoil_check_with_season(
            freshness,
            profile,
            now_tick,
            multiplier,
            season,
            item.instance_id,
        ),
        age_peak_check_with_season(
            freshness,
            profile,
            now_tick,
            multiplier,
            season,
            item.instance_id,
        ),
    )
}

fn emit_shelflife_consume_events(
    entity: Entity,
    instance_id: u64,
    spoil: &SpoilCheckOutcome,
    age: &AgePeakCheck,
    spoil_warnings: &mut Option<ResMut<Events<SpoilConsumeWarning>>>,
    age_bonus_rolls: &mut Option<ResMut<Events<AgeBonusRoll>>>,
) {
    if let Some(spoil_warnings) = spoil_warnings.as_deref_mut() {
        match spoil {
            SpoilCheckOutcome::Warn {
                current_qi,
                spoil_threshold,
            } => {
                spoil_warnings.send(SpoilConsumeWarning {
                    player: entity,
                    instance_id,
                    severity: SpoilSeverity::Sharp,
                    current_qi: *current_qi,
                    spoil_threshold: *spoil_threshold,
                });
            }
            SpoilCheckOutcome::CriticalBlock {
                current_qi,
                spoil_threshold,
            } => {
                spoil_warnings.send(SpoilConsumeWarning {
                    player: entity,
                    instance_id,
                    severity: SpoilSeverity::CriticalBlock,
                    current_qi: *current_qi,
                    spoil_threshold: *spoil_threshold,
                });
            }
            SpoilCheckOutcome::NotApplicable | SpoilCheckOutcome::Safe { .. } => {}
        }
    }

    if let (Some(age_bonus_rolls), AgePeakCheck::Peaking { bonus_strength }) =
        (age_bonus_rolls.as_deref_mut(), age)
    {
        age_bonus_rolls.send(AgeBonusRoll {
            player: entity,
            instance_id,
            bonus_strength: *bonus_strength,
        });
    }
}

// ── plan-supply-coffin-loot-ui P2：外部容器跨容器 move / close ──

#[allow(clippy::too_many_arguments, clippy::needless_borrow)]
pub(crate) fn handle_external_container_move(
    player_entity: Entity,
    session_id: u64,
    instance_id: u64,
    from: &crate::schema::inventory::InventoryLocationV1,
    to: &crate::schema::inventory::InventoryLocationV1,
    dispatch: &mut ClientRequestDispatchParams,
    combat_params: &mut CombatRequestParams,
    inventories: &mut Query<&mut PlayerInventory>,
    player_states: &Query<&PlayerState>,
    cultivations: &Query<&Cultivation>,
    clients: &mut Query<(&Username, &mut Client)>,
    positions: &Query<&valence::prelude::Position>,
    dimensions: &Query<&CurrentDimension>,
    _commands: &mut Commands,
) {
    use crate::inventory::external_container::{
        place_item_into_container, remove_item_from_container,
    };
    use crate::network::inventory_snapshot_emit::item_view_from_instance;
    use crate::schema::inventory::{InventoryLocationV1, PlacedInventoryItemV1};
    use crate::schema::server_data::{LootContainerUpdateV1, ServerDataPayloadV1, ServerDataV1};

    let supply_coffin_registry = dispatch.supply_coffin_registry.as_deref();
    let Some(ext_reg) = dispatch.ext_container_registry.as_deref_mut() else {
        tracing::warn!("[bong][network] external_container_move: registry missing");
        resync_inventory_only(
            player_entity,
            inventories,
            player_states,
            cultivations,
            clients,
        );
        return;
    };

    let Some(&coffin_entity) = ext_reg.sessions.get(&session_id) else {
        tracing::warn!(
            "[bong][network] external_container_move: unknown session {session_id} from {player_entity:?}"
        );
        resync_inventory_only(
            player_entity,
            inventories,
            player_states,
            cultivations,
            clients,
        );
        return;
    };

    let Ok(mut ext) = combat_params.ext_containers.get_mut(coffin_entity) else {
        tracing::warn!(
            "[bong][network] external_container_move: ExternalContainer component missing on {coffin_entity:?}"
        );
        resync_inventory_only(
            player_entity,
            inventories,
            player_states,
            cultivations,
            clients,
        );
        return;
    };

    if ext.opened_by != Some(player_entity) {
        tracing::warn!(
            "[bong][network] external_container_move: session {session_id} not owned by {player_entity:?}"
        );
        // 非 owner 不得获得外部容器内容；只回推请求者自己的背包状态。
        resync_inventory_only(
            player_entity,
            inventories,
            player_states,
            cultivations,
            clients,
        );
        return;
    }

    if matches!(
        &ext.source_kind,
        crate::inventory::external_container::ExternalContainerKind::SupplyCoffin { .. }
    ) {
        let Ok(player_pos) = positions.get(player_entity) else {
            tracing::warn!(
                "[bong][network] external_container_move: supply coffin session {session_id} player {player_entity:?} missing Position"
            );
            resync_ext_and_inventory(
                player_entity,
                &ext,
                inventories,
                player_states,
                cultivations,
                clients,
            );
            return;
        };
        let active =
            supply_coffin_registry.and_then(|registry| registry.active.get(&coffin_entity));
        let authorization = crate::supply_coffin::authority::authorize_supply_coffin_session(
            active,
            player_pos.get(),
            dimensions
                .get(player_entity)
                .ok()
                .map(|dimension| dimension.0),
        );
        if let Err(reason) = authorization {
            tracing::warn!(
                "[bong][network] external_container_move: supply coffin session {session_id} authority rejected: {reason:?}"
            );
            resync_ext_and_inventory(
                player_entity,
                &ext,
                inventories,
                player_states,
                cultivations,
                clients,
            );
            return;
        }
    }

    let ext_container_id =
        crate::inventory::external_container::ExternalContainer::container_id(session_id);

    let is_from_ext = matches!(from, InventoryLocationV1::Container { container_id, .. } if *container_id == ext_container_id);
    let is_to_ext = matches!(to, InventoryLocationV1::Container { container_id, .. } if *container_id == ext_container_id);

    if is_from_ext == is_to_ext {
        tracing::warn!(
            "[bong][network] external_container_move: both endpoints on same side (from_ext={is_from_ext})"
        );
        resync_ext_and_inventory(
            player_entity,
            &ext,
            inventories,
            player_states,
            cultivations,
            clients,
        );
        return;
    }

    if is_from_ext {
        // 外部容器 → 玩家背包
        let InventoryLocationV1::Container {
            row: to_row,
            col: to_col,
            container_id: to_container_id,
            ..
        } = to
        else {
            tracing::warn!(
                "[bong][network] external_container_move: target must be container slot"
            );
            resync_ext_and_inventory(
                player_entity,
                &ext,
                inventories,
                player_states,
                cultivations,
                clients,
            );
            return;
        };

        let authoritative_source = ext.container.items.iter().find(|placed| {
            placed.instance.instance_id == instance_id
                && matches!(
                    from,
                    InventoryLocationV1::Container {
                        container_id,
                        row,
                        col,
                    } if *container_id == ext_container_id
                        && *row == u64::from(placed.row)
                        && *col == u64::from(placed.col)
                )
        });
        if authoritative_source.is_none() {
            tracing::warn!(
                "[bong][network] external_container_move: instance {instance_id} source location does not match authoritative external placement"
            );
            resync_ext_and_inventory(
                player_entity,
                &ext,
                inventories,
                player_states,
                cultivations,
                clients,
            );
            return;
        }

        let Some(removed) = remove_item_from_container(&mut ext.container, instance_id) else {
            tracing::warn!(
                "[bong][network] external_container_move: instance {instance_id} not found in ext container"
            );
            resync_ext_and_inventory(
                player_entity,
                &ext,
                inventories,
                player_states,
                cultivations,
                clients,
            );
            return;
        };

        let Ok(mut inventory) = inventories.get_mut(player_entity) else {
            place_item_into_container(
                &mut ext.container,
                removed.row,
                removed.col,
                removed.instance,
            )
            .ok();
            return;
        };

        let target_container = inventory
            .containers
            .iter()
            .find(|c| c.id == *to_container_id);
        let Some(target_container) = target_container else {
            tracing::warn!(
                "[bong][network] external_container_move: player container `{to_container_id}` not found"
            );
            place_item_into_container(
                &mut ext.container,
                removed.row,
                removed.col,
                removed.instance,
            )
            .ok();
            resync_ext_and_inventory(
                player_entity,
                &ext,
                inventories,
                player_states,
                cultivations,
                clients,
            );
            return;
        };

        let (to_row, to_col) = match (u8::try_from(*to_row), u8::try_from(*to_col)) {
            (Ok(r), Ok(c)) => (r, c),
            _ => {
                tracing::warn!(
                    "[bong][network] external_container_move: row/col overflow (row={}, col={})",
                    to_row,
                    to_col
                );
                place_item_into_container(
                    &mut ext.container,
                    removed.row,
                    removed.col,
                    removed.instance,
                )
                .ok();
                resync_ext_and_inventory(
                    player_entity,
                    &ext,
                    inventories,
                    player_states,
                    cultivations,
                    clients,
                );
                return;
            }
        };

        if !crate::inventory::item_fits_in_container_bounds(
            target_container,
            to_row,
            to_col,
            removed.instance.grid_w,
            removed.instance.grid_h,
        ) {
            place_item_into_container(
                &mut ext.container,
                removed.row,
                removed.col,
                removed.instance,
            )
            .ok();
            resync_ext_and_inventory(
                player_entity,
                &ext,
                inventories,
                player_states,
                cultivations,
                clients,
            );
            return;
        }

        let probe = crate::inventory::footprint_probe(
            to_row,
            to_col,
            removed.instance.grid_w,
            removed.instance.grid_h,
        );
        let target_container = inventory
            .containers
            .iter()
            .find(|c| c.id == *to_container_id)
            .unwrap();
        let overlaps = target_container
            .items
            .iter()
            .any(|existing| crate::inventory::placed_item_footprints_overlap(&probe, existing));
        if overlaps {
            place_item_into_container(
                &mut ext.container,
                removed.row,
                removed.col,
                removed.instance,
            )
            .ok();
            resync_ext_and_inventory(
                player_entity,
                &ext,
                inventories,
                player_states,
                cultivations,
                clients,
            );
            return;
        }

        let container_mut = inventory
            .containers
            .iter_mut()
            .find(|c| c.id == *to_container_id)
            .unwrap();
        container_mut.items.push(crate::inventory::PlacedItemState {
            row: to_row,
            col: to_col,
            instance: removed.instance,
        });
        inventory.revision.0 = inventory.revision.0.saturating_add(1);
    } else {
        // 玩家背包 → 外部容器
        let InventoryLocationV1::Container {
            container_id: from_container_id,
            row: from_row,
            col: from_col,
        } = from
        else {
            tracing::warn!(
                "[bong][network] external_container_move: player source must be container slot"
            );
            resync_ext_and_inventory(
                player_entity,
                &ext,
                inventories,
                player_states,
                cultivations,
                clients,
            );
            return;
        };

        let InventoryLocationV1::Container {
            row: to_row,
            col: to_col,
            ..
        } = to
        else {
            tracing::warn!(
                "[bong][network] external_container_move: ext target must be container slot"
            );
            resync_ext_and_inventory(
                player_entity,
                &ext,
                inventories,
                player_states,
                cultivations,
                clients,
            );
            return;
        };

        let Ok(mut inventory) = inventories.get_mut(player_entity) else {
            return;
        };

        let authoritative_source = inventory.containers.iter().position(|container| {
            container.id == *from_container_id
                && container.items.iter().any(|placed| {
                    placed.instance.instance_id == instance_id
                        && u64::from(placed.row) == *from_row
                        && u64::from(placed.col) == *from_col
                })
        });
        let Some(source_container_index) = authoritative_source else {
            tracing::warn!(
                "[bong][network] external_container_move: instance {instance_id} source location does not match authoritative player placement"
            );
            resync_ext_and_inventory(
                player_entity,
                &ext,
                inventories,
                player_states,
                cultivations,
                clients,
            );
            return;
        };
        let source_item_index = inventory.containers[source_container_index]
            .items
            .iter()
            .position(|placed| {
                placed.instance.instance_id == instance_id
                    && u64::from(placed.row) == *from_row
                    && u64::from(placed.col) == *from_col
            })
            .expect("authoritative source search found matching item and placement");
        let removed = inventory.containers[source_container_index]
            .items
            .remove(source_item_index);

        let (to_row, to_col) = match (u8::try_from(*to_row), u8::try_from(*to_col)) {
            (Ok(r), Ok(c)) => (r, c),
            _ => {
                tracing::warn!(
                    "[bong][network] external_container_move: row/col overflow (row={}, col={})",
                    to_row,
                    to_col
                );
                // restore to player
                let orig_container = inventory.containers.iter_mut().find(|c| {
                    if let InventoryLocationV1::Container { container_id, .. } = from {
                        c.id == *container_id
                    } else {
                        false
                    }
                });
                if let Some(container) = orig_container {
                    container.items.push(removed);
                }
                resync_ext_and_inventory(
                    player_entity,
                    &ext,
                    inventories,
                    player_states,
                    cultivations,
                    clients,
                );
                return;
            }
        };

        match place_item_into_container(
            &mut ext.container,
            to_row,
            to_col,
            removed.instance.clone(),
        ) {
            Ok(()) => {
                inventory.revision.0 = inventory.revision.0.saturating_add(1);
            }
            Err(reason) => {
                tracing::warn!(
                    "[bong][network] external_container_move: place into ext failed: {reason}"
                );
                // restore to player
                let orig_container = inventory.containers.iter_mut().find(|c| {
                    if let InventoryLocationV1::Container { container_id, .. } = from {
                        c.id == *container_id
                    } else {
                        false
                    }
                });
                if let Some(container) = orig_container {
                    container.items.push(removed);
                }
                resync_ext_and_inventory(
                    player_entity,
                    &ext,
                    inventories,
                    player_states,
                    cultivations,
                    clients,
                );
                return;
            }
        }
    }

    // 成功——发 LootContainerUpdate + InventorySnapshot
    let placed_items: Vec<PlacedInventoryItemV1> = ext
        .container
        .items
        .iter()
        .map(|p| PlacedInventoryItemV1 {
            container_id: ext.container.id.clone(),
            row: u64::from(p.row),
            col: u64::from(p.col),
            item: item_view_from_instance(&p.instance),
        })
        .collect();

    let update_payload = ServerDataV1::new(ServerDataPayloadV1::LootContainerUpdate(
        LootContainerUpdateV1 {
            session_id,
            placed_items,
        },
    ));

    if let Ok(bytes) = serialize_server_data_payload(&update_payload) {
        if let Ok((_username, mut client)) = clients.get_mut(player_entity) {
            send_server_data_payload(&mut client, bytes.as_slice());
        }
    }

    resync_inventory_only(
        player_entity,
        inventories,
        player_states,
        cultivations,
        clients,
    );
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_external_container_close(
    player_entity: Entity,
    session_id: u64,
    dispatch: &mut ClientRequestDispatchParams,
    combat_params: &mut CombatRequestParams,
    inventories: &mut Query<&mut PlayerInventory>,
    player_states: &Query<&PlayerState>,
    cultivations: &Query<&Cultivation>,
    clients: &mut Query<(&Username, &mut Client)>,
    _commands: &mut Commands,
) {
    use crate::schema::server_data::{
        LootContainerCloseReasonV1, LootContainerCloseV1, ServerDataPayloadV1, ServerDataV1,
    };

    let Some(ext_reg) = dispatch.ext_container_registry.as_deref_mut() else {
        return;
    };

    let Some(&coffin_entity) = ext_reg.sessions.get(&session_id) else {
        tracing::warn!("[bong][network] external_container_close: unknown session {session_id}");
        return;
    };

    let Ok(mut ext) = combat_params.ext_containers.get_mut(coffin_entity) else {
        return;
    };

    if ext.opened_by != Some(player_entity) {
        tracing::warn!(
            "[bong][network] external_container_close: session {session_id} not owned by {player_entity:?}"
        );
        return;
    }

    let close_payload = ServerDataV1::new(ServerDataPayloadV1::LootContainerClose(
        LootContainerCloseV1 {
            session_id,
            reason: LootContainerCloseReasonV1::PlayerClosed,
        },
    ));

    if let Ok(bytes) = serialize_server_data_payload(&close_payload) {
        if let Ok((_username, mut client)) = clients.get_mut(player_entity) {
            send_server_data_payload(&mut client, bytes.as_slice());
        }
    }

    // 释放锁——棺不碎，等 lifecycle tick 超时后碎裂
    ext.opened_by = None;

    resync_inventory_only(
        player_entity,
        inventories,
        player_states,
        cultivations,
        clients,
    );

    tracing::info!(
        "[bong][network] external_container_close: session {session_id} closed by player {player_entity:?}"
    );
}

fn resync_ext_and_inventory(
    player_entity: Entity,
    ext: &crate::inventory::external_container::ExternalContainer,
    inventories: &mut Query<&mut PlayerInventory>,
    player_states: &Query<&PlayerState>,
    cultivations: &Query<&Cultivation>,
    clients: &mut Query<(&Username, &mut Client)>,
) {
    use crate::network::inventory_snapshot_emit::item_view_from_instance;
    use crate::schema::inventory::PlacedInventoryItemV1;
    use crate::schema::server_data::{LootContainerUpdateV1, ServerDataPayloadV1, ServerDataV1};

    let placed_items: Vec<PlacedInventoryItemV1> = ext
        .container
        .items
        .iter()
        .map(|p| PlacedInventoryItemV1 {
            container_id: ext.container.id.clone(),
            row: u64::from(p.row),
            col: u64::from(p.col),
            item: item_view_from_instance(&p.instance),
        })
        .collect();

    let update_payload = ServerDataV1::new(ServerDataPayloadV1::LootContainerUpdate(
        LootContainerUpdateV1 {
            session_id: ext.session_id,
            placed_items,
        },
    ));

    if let Ok(bytes) = serialize_server_data_payload(&update_payload) {
        if let Ok((_username, mut client)) = clients.get_mut(player_entity) {
            send_server_data_payload(&mut client, bytes.as_slice());
        }
    }

    resync_inventory_only(
        player_entity,
        inventories,
        player_states,
        cultivations,
        clients,
    );
}

#[allow(clippy::needless_borrow)]
fn resync_inventory_only(
    player_entity: Entity,
    inventories: &Query<&mut PlayerInventory>,
    player_states: &Query<&PlayerState>,
    cultivations: &Query<&Cultivation>,
    clients: &mut Query<(&Username, &mut Client)>,
) {
    let Ok(inventory) = inventories.get(player_entity) else {
        return;
    };
    let Ok(player_state) = player_states.get(player_entity) else {
        return;
    };
    let Ok(cultivation) = cultivations.get(player_entity) else {
        return;
    };
    let Ok((username, mut client)) = clients.get_mut(player_entity) else {
        return;
    };
    send_inventory_snapshot_to_client(
        player_entity,
        &mut client,
        username.as_str(),
        &inventory,
        player_state,
        cultivation,
        "external_container_resync",
    );
}

/// plan-dying-elder-v1 P1 — 处理玩家向垂死大能交付回元丹请求。
///
/// ## 处理流程
/// 1. 校验玩家背包中 `pill_instance_id` 对应物品为 `huiyuan_pill`（pills.toml id，无下划线）；
/// 2. 根据 `elder_entity_id` 找到大能 ECS entity；
/// 3. emit `GiveDanToElderIntent`；
/// 4. `dying_elder_give_dan_system` 按 EventReader 顺序权威重验、消费、读取真实
///    ItemRegistry effect 并提交真元事务。网络层绝不先删物品。
///
/// ## 失败路径（静默 warn，不 crash）
/// - pill_instance_id 不在玩家背包 → warn + reject
/// - 物品不是 `huiyuan_pill` → warn + reject
/// - elder_entity_id 找不到 entity → warn + reject
/// - give_dan_to_elder_tx 缺失 → warn + reject（事件注册未完成）
#[allow(clippy::too_many_arguments)]
fn handle_give_dan_to_elder(
    player_entity: Entity,
    pill_instance_id: u64,
    elder_entity_id: i32,
    inventories: &mut Query<&mut PlayerInventory>,
    entity_manager: Option<&valence::prelude::EntityManager>,
    clients: &mut Query<(&Username, &mut Client)>,
    give_dan_tx: Option<&mut Events<crate::fauna::dying_elder::GiveDanToElderIntent>>,
    positions: &Query<&valence::prelude::Position>,
    dimensions: &Query<&CurrentDimension>,
    dying_elder_targets: &DyingElderTargetQuery<'_, '_>,
) {
    use crate::fauna::dying_elder::GiveDanToElderIntent;

    // ── 校验玩家背包中是否有该 pill instance ──────────────────────────────
    let pill_template_id = {
        let Ok(inventory) = inventories.get(player_entity) else {
            tracing::warn!(
                "[bong][dying_elder] give_dan: player entity {player_entity:?} has no PlayerInventory"
            );
            return;
        };
        match crate::inventory::inventory_item_by_instance_borrow(inventory, pill_instance_id) {
            Some(item) => item.template_id.clone(),
            None => {
                tracing::warn!(
                    "[bong][dying_elder] give_dan: pill_instance_id={pill_instance_id} not in player inventory"
                );
                if let Ok((_u, mut client)) = clients.get_mut(player_entity) {
                    client.send_chat_message("§c[垂死大能] 背包中未找到该回元丹。");
                }
                return;
            }
        }
    };

    // ── 校验物品是 huiyuan_pill（pills.toml 注册 id，无下划线）───────────
    if pill_template_id != "huiyuan_pill" {
        tracing::warn!(
            "[bong][dying_elder] give_dan: item template_id={pill_template_id} is not huiyuan_pill"
        );
        if let Ok((_u, mut client)) = clients.get_mut(player_entity) {
            client.send_chat_message("§c[垂死大能] 只接受回元丹。");
        }
        return;
    }

    // ── 解析并授权大能 entity ───────────────────────────────────────────────
    let Some(entity_manager) = entity_manager else {
        tracing::warn!("[bong][dying_elder] give_dan: EntityManager resource missing");
        return;
    };
    let Some(elder_entity) = entity_manager.get_by_id(elder_entity_id) else {
        tracing::warn!(
            "[bong][dying_elder] give_dan: no entity for elder_entity_id={elder_entity_id}"
        );
        if let Ok((_u, mut client)) = clients.get_mut(player_entity) {
            client.send_chat_message("§c[垂死大能] 找不到目标大能。");
        }
        return;
    };

    // A resolved protocol entity is not sufficient authority: the target must still be
    // the live DyingElder encounter, in an accepting state, nearby, and in the same
    // logical dimension. These checks deliberately run before emitting the intent, so
    // the downstream transaction cannot consume a pill for a stale/forged target.
    let Ok((elder_state, elder_archetype)) = dying_elder_targets.get(elder_entity) else {
        reject_give_dan_target(
            clients,
            player_entity,
            "§c[垂死大能] 目标不是可交互的大能。",
        );
        return;
    };
    if *elder_archetype != NpcArchetype::DyingElder || !dying_elder_can_receive_dan(elder_state) {
        reject_give_dan_target(
            clients,
            player_entity,
            "§c[垂死大能] 目标当前不接受回元丹。",
        );
        return;
    }

    let (Ok(player_position), Ok(elder_position)) =
        (positions.get(player_entity), positions.get(elder_entity))
    else {
        reject_give_dan_target(
            clients,
            player_entity,
            "§c[垂死大能] 无法确认玩家与目标位置。",
        );
        return;
    };
    let (Ok(player_dimension), Ok(elder_dimension)) =
        (dimensions.get(player_entity), dimensions.get(elder_entity))
    else {
        reject_give_dan_target(clients, player_entity, "§c[垂死大能] 无法确认目标位面。");
        return;
    };
    if !is_give_dan_target_in_scope(
        player_position.get(),
        elder_position.get(),
        player_dimension.0,
        elder_dimension.0,
    ) {
        reject_give_dan_target(
            clients,
            player_entity,
            "§c[垂死大能] 目标不在当前位面或交互范围内。",
        );
        return;
    }

    // ── 只 emit intent；权威消费在 give_dan_system 内按顺序执行 ─────────────
    let Some(tx) = give_dan_tx else {
        tracing::warn!(
            "[bong][dying_elder] give_dan: GiveDanToElderIntent event resource missing, dropping intent"
        );
        return;
    };
    tx.send(GiveDanToElderIntent {
        player: player_entity,
        elder: elder_entity,
        pill_instance_id,
    });

    tracing::info!(
        "[bong][dying_elder] give_dan preflight accepted: player {player_entity:?} → elder {elder_entity:?} pill={pill_instance_id}"
    );
}

#[cfg(test)]
#[path = "client_request_handler_tests.rs"]
mod tests;
