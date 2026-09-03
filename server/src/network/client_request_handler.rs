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
#[cfg(test)]
use crate::combat::events::RevivalActionIntent;
use crate::combat::events::{ApplyStatusEffectIntent, DefenseIntent, StatusEffectKind};
use crate::combat::foreign_qi_resistance::foreign_qi_resistance_for_use;
use crate::combat::needle::IntentSource;
use crate::combat::tuike::{can_equip_false_skin, false_skin_kind_for_item, FalseSkinForgeRequest};
use crate::combat::CombatClock;
use crate::craft::workbench::workbench_block_pos;
use crate::craft::WorkbenchBlock;
use crate::cultivation::breakthrough::BreakthroughRequest;
use crate::cultivation::components::{
    recover_current_qi, Cultivation, MeridianChannelId, MeridianId, MeridianSystem,
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
use crate::cultivation::technique_scroll::{
    can_learn_technique, learn_technique_if_allowed, LearnSource, ScrollReadOutcome,
    TechniqueLearnedEvent, TechniqueScrollReadEvent,
};
use crate::cultivation::tribulation::{HeartDemonChoiceSubmitted, StartDuXuRequest};
use crate::cultivation::void::actions::VoidActionIntent;
use crate::fauna::dying_elder::DyingElderState;
use crate::forge::blueprint::BlueprintRegistry;
#[cfg(test)]
use crate::forge::blueprint::TemperBeat;
use crate::forge::events::{
    ConsecrationInject, InscriptionScrollSubmit, StartForgeRequest, StepAdvance, TemperingHit,
};
use crate::forge::learned::LearnedBlueprints;
use crate::forge::session::{ForgeSessionId, ForgeSessions, ForgeStep};
use crate::forge::station::{PlaceForgeStationRequest, WeaponForgeStation};
#[cfg(test)]
use crate::inventory::add_item_to_player_inventory;
use crate::inventory::{
    add_item_to_player_inventory_with_alchemy, apply_inventory_move_with_race,
    apply_item_spiritual_wear, consume_item_instance_once, discard_inventory_item_to_dropped_loot,
    fully_repair_weapon_instance, inventory_instance_container_attrition_exempt,
    inventory_item_by_instance_borrow, inventory_item_by_instance_mut,
    inventory_location_attrition_exempt, pickup_dropped_loot_instance, DroppedLootRegistry,
    InventoryDurabilityChangedEvent, InventoryInstanceIdAllocator, InventoryMoveOutcome,
    InventoryMoveRejectReason, ItemInstance, ItemTemplate, PlayerInventory,
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
use crate::network::client_request::{combat, forge, inventory, npc, production};
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
use crate::network::skill_snapshot_emit::send_skill_snapshot_to_client;
use crate::network::techniques_snapshot_emit::send_techniques_snapshot_to_client;
use crate::network::{
    gameplay_vfx, redis_bridge::RedisOutbound, vfx_event_emit::VfxEventRequest, RedisBridgeResource,
};
#[cfg(test)]
use crate::npc::faction::FactionMembership;
use crate::npc::lifecycle::NpcArchetype;
use crate::npc::spawn::NpcMarker;
#[cfg(test)]
use crate::npc::trade::NpcPlayerReputation;
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
use crate::skill::components::{ScrollId, SkillId, SkillSet};
use crate::skill::config::{
    handle_config_intent, skill_config_snapshot_for_cast, validate_skill_config,
    SkillConfigRejectReason, SkillConfigSchemas, SkillConfigSnapshot, SkillConfigStore,
};
use crate::skill::events::{SkillScrollUsed, SkillXpGain, XpGainSource};
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
#[cfg(test)]
use crate::zhenfa::{
    ScatterBeadUseRequest, ZhenfaDisarmRequest, ZhenfaPlaceRequest, ZhenfaTriggerRequest,
};

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
const GIVE_DAN_MAX_DISTANCE: f64 = 6.0;
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
            ClientRequestV1::LearnSkillScroll { instance_id, .. } => {
                if !handle_craft_recipe_scroll(
                    ev.client,
                    instance_id,
                    &mut inventories,
                    &mut clients,
                    &skill_scroll_params.item_registry,
                    CraftRecipeScrollParams {
                        registry: skill_scroll_params.craft_registry.as_deref(),
                        unlock_state: skill_scroll_params.craft_unlock_state.as_deref_mut(),
                        unlock_tx: skill_scroll_params.craft_unlock_tx.as_deref_mut(),
                    },
                ) {
                    handle_learn_skill_scroll(
                        ev.client,
                        instance_id,
                        &mut inventories,
                        &mut clients,
                        &player_states,
                        &mut skill_scroll_params,
                        &mut combat_params.meridians,
                    );
                }
            }
            ClientRequestV1::TechniqueScrollUse { instance_id, .. } => {
                if !handle_craft_recipe_scroll(
                    ev.client,
                    instance_id,
                    &mut inventories,
                    &mut clients,
                    &skill_scroll_params.item_registry,
                    CraftRecipeScrollParams {
                        registry: skill_scroll_params.craft_registry.as_deref(),
                        unlock_state: skill_scroll_params.craft_unlock_state.as_deref_mut(),
                        unlock_tx: skill_scroll_params.craft_unlock_tx.as_deref_mut(),
                    },
                ) {
                    handle_learn_skill_scroll(
                        ev.client,
                        instance_id,
                        &mut inventories,
                        &mut clients,
                        &player_states,
                        &mut skill_scroll_params,
                        &mut combat_params.meridians,
                    );
                }
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

struct CraftRecipeScrollParams<'a> {
    registry: Option<&'a crate::craft::CraftRegistry>,
    unlock_state: Option<&'a mut crate::craft::RecipeUnlockState>,
    unlock_tx: Option<&'a mut Events<crate::craft::CraftUnlockIntent>>,
}

fn handle_craft_recipe_scroll(
    entity: Entity,
    instance_id: u64,
    inventories: &mut Query<&mut PlayerInventory>,
    clients: &mut Query<(&Username, &mut Client)>,
    item_registry: &ItemRegistry,
    craft: CraftRecipeScrollParams<'_>,
) -> bool {
    let (Some(craft_registry), Some(craft_unlock_state), Some(craft_unlock_tx)) =
        (craft.registry, craft.unlock_state, craft.unlock_tx)
    else {
        return false;
    };
    let Some(template_id) = inventories
        .get(entity)
        .ok()
        .and_then(|inventory| inventory_item_by_instance_borrow(inventory, instance_id))
        .map(|instance| instance.template_id.clone())
    else {
        return false;
    };
    let Some(template) = item_registry.get(&template_id) else {
        return false;
    };
    if template.category != ItemCategory::Scroll {
        return false;
    }
    let Ok((username, _)) = clients.get_mut(entity) else {
        return false;
    };
    let player_id = canonical_player_id(username.0.as_str());
    let recipe_ids: Vec<_> =
        crate::craft::unlock::find_recipes_unlockable_by_scroll(craft_registry, &template_id)
            .into_iter()
            .filter(|recipe| craft_unlock_state.reserve_scroll_unlock(&player_id, &recipe.id))
            .map(|recipe| recipe.id.clone())
            .collect();
    if recipe_ids.is_empty() {
        let is_craft_scroll =
            crate::craft::unlock::find_recipes_unlockable_by_scroll(craft_registry, &template_id)
                .into_iter()
                .next()
                .is_some();
        return is_craft_scroll;
    }
    let Ok(mut inventory) = inventories.get_mut(entity) else {
        for recipe_id in &recipe_ids {
            craft_unlock_state.release_scroll_unlock_reservation(&player_id, recipe_id);
        }
        return true;
    };
    if consume_item_instance_once(&mut inventory, instance_id).is_err() {
        for recipe_id in &recipe_ids {
            craft_unlock_state.release_scroll_unlock_reservation(&player_id, recipe_id);
        }
        return true;
    }
    for recipe_id in recipe_ids {
        craft_unlock_tx.send(crate::craft::CraftUnlockIntent {
            caster: entity,
            player_id: player_id.clone(),
            recipe_id,
            source: crate::craft::UnlockEventSource::Scroll {
                item_template: template_id.clone(),
            },
        });
    }
    true
}

fn handle_learn_skill_scroll(
    entity: Entity,
    instance_id: u64,
    inventories: &mut Query<&mut PlayerInventory>,
    clients: &mut Query<(&Username, &mut Client)>,
    player_states: &Query<&PlayerState>,
    skill_scroll_params: &mut SkillScrollRequestParams,
    meridians_q: &mut Query<&mut MeridianSystem>,
) {
    let Some(template_id) = ({
        let inventory = match inventories.get(entity) {
            Ok(inv) => inv,
            Err(_) => return,
        };
        inventory_item_by_instance_borrow(inventory, instance_id)
            .map(|instance| instance.template_id.clone())
    }) else {
        return;
    };

    if let Some(template) = skill_scroll_params
        .item_registry
        .get(template_id.as_str())
        .cloned()
        .filter(|template| template.technique_scroll_spec.is_some())
    {
        handle_learn_technique_scroll(
            entity,
            instance_id,
            inventories,
            clients,
            player_states,
            skill_scroll_params,
            meridians_q,
            &template,
        );
        return;
    }

    let Some((skill, scroll_id, xp_grant)) = ({
        skill_scroll_spec(template_id.as_str())
            .map(|(skill, xp_grant)| (skill, ScrollId::new(template_id.clone()), xp_grant))
    }) else {
        tracing::warn!(
            "[bong][network][skill] learn_skill_scroll rejected: instance_id={} is not a known skill scroll",
            instance_id
        );
        return;
    };

    let is_duplicate = match skill_scroll_params.skill_sets.get(entity) {
        Ok(skill_set) => skill_set.consumed_scrolls.contains(&scroll_id),
        Err(_) => return,
    };

    if is_duplicate {
        if let Some(skill_scroll_used_tx) = skill_scroll_params.skill_scroll_used_tx.as_deref_mut()
        {
            skill_scroll_used_tx.send(SkillScrollUsed {
                char_entity: entity,
                scroll_id,
                skill,
                xp_granted: 0,
                was_duplicate: true,
            });
        }
        if let Ok(inventory) = inventories.get(entity) {
            resync_snapshot(
                entity,
                inventory,
                clients,
                player_states,
                &skill_scroll_params.cultivations,
                "skill_scroll_duplicate",
            );
        }
        if let Ok((username, mut client)) = clients.get_mut(entity) {
            if let (Ok(skill_set), Ok(cultivation)) = (
                skill_scroll_params.skill_sets.get(entity),
                skill_scroll_params.cultivations.get(entity),
            ) {
                send_skill_snapshot_to_client(
                    entity,
                    &mut client,
                    username.0.as_str(),
                    skill_set,
                    cultivation,
                    "skill_scroll_duplicate",
                );
            }
        }
        return;
    }

    {
        let Ok(mut inventory) = inventories.get_mut(entity) else {
            return;
        };
        if consume_item_instance_once(&mut inventory, instance_id).is_err() {
            return;
        }
    }

    if let Ok(mut skill_set) = skill_scroll_params.skill_sets.get_mut(entity) {
        skill_set.consumed_scrolls.insert(scroll_id.clone());
    } else {
        return;
    }

    if let Some(skill_xp_tx) = skill_scroll_params.skill_xp_tx.as_deref_mut() {
        skill_xp_tx.send(SkillXpGain {
            char_entity: entity,
            skill,
            amount: xp_grant,
            source: XpGainSource::Scroll {
                scroll_id: scroll_id.clone(),
                xp_grant,
            },
        });
    }
    if let Some(skill_scroll_used_tx) = skill_scroll_params.skill_scroll_used_tx.as_deref_mut() {
        skill_scroll_used_tx.send(SkillScrollUsed {
            char_entity: entity,
            scroll_id,
            skill,
            xp_granted: xp_grant,
            was_duplicate: false,
        });
    }

    let Ok(player_state) = player_states.get(entity) else {
        return;
    };
    let Ok(cultivation) = skill_scroll_params.cultivations.get(entity) else {
        return;
    };
    if let Ok((username, mut client)) = clients.get_mut(entity) {
        if let Ok(inventory) = inventories.get(entity) {
            send_inventory_snapshot_to_client(
                entity,
                &mut client,
                username.0.as_str(),
                inventory,
                player_state,
                cultivation,
                "skill_scroll_consumed",
            );
        }
        if let Ok(skill_set) = skill_scroll_params.skill_sets.get(entity) {
            send_skill_snapshot_to_client(
                entity,
                &mut client,
                username.0.as_str(),
                skill_set,
                cultivation,
                "skill_scroll_consumed",
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_learn_technique_scroll(
    entity: Entity,
    instance_id: u64,
    inventories: &mut Query<&mut PlayerInventory>,
    clients: &mut Query<(&Username, &mut Client)>,
    player_states: &Query<&PlayerState>,
    skill_scroll_params: &mut SkillScrollRequestParams,
    meridians_q: &mut Query<&mut MeridianSystem>,
    template: &ItemTemplate,
) {
    let Some(spec) = template.technique_scroll_spec.as_ref() else {
        return;
    };
    let technique_id = spec.skill_id.clone();
    let outcome = {
        let Ok(known) = skill_scroll_params.known_techniques.get(entity) else {
            return;
        };
        let Ok(cultivation) = skill_scroll_params.cultivations.get(entity) else {
            return;
        };
        let Ok(meridians) = meridians_q.get_mut(entity) else {
            return;
        };
        let severed = skill_scroll_params
            .severed_meridians
            .get(entity)
            .ok()
            .flatten();
        let intrinsic_plan = crate::body_plan::resolve_body_plan_for_target(
            entity,
            crate::body_plan::BodyPlanPurpose::Intrinsic,
            crate::body_plan::BodyPlanResolveInputs {
                cultivation: Some(cultivation),
                beast_kind: None,
                morph_state: None,
            },
            skill_scroll_params.body_plans.as_deref(),
            skill_scroll_params.race_registry.as_deref(),
        );
        can_learn_technique(
            &skill_scroll_params.technique_registry,
            known,
            cultivation,
            &meridians,
            severed,
            technique_id.as_str(),
            intrinsic_plan.is_humanoid,
            intrinsic_plan.meridian_profile.as_ref(),
        )
    };

    if matches!(outcome, ScrollReadOutcome::Learned) {
        {
            let Ok(mut inventory) = inventories.get_mut(entity) else {
                return;
            };
            if consume_item_instance_once(&mut inventory, instance_id).is_err() {
                return;
            }
        }

        let learned = {
            let Ok(mut known) = skill_scroll_params.known_techniques.get_mut(entity) else {
                return;
            };
            let Ok(cultivation) = skill_scroll_params.cultivations.get(entity) else {
                return;
            };
            let Ok(meridians) = meridians_q.get_mut(entity) else {
                return;
            };
            let severed = skill_scroll_params
                .severed_meridians
                .get(entity)
                .ok()
                .flatten();
            let intrinsic_plan = crate::body_plan::resolve_body_plan_for_target(
                entity,
                crate::body_plan::BodyPlanPurpose::Intrinsic,
                crate::body_plan::BodyPlanResolveInputs {
                    cultivation: Some(cultivation),
                    beast_kind: None,
                    morph_state: None,
                },
                skill_scroll_params.body_plans.as_deref(),
                skill_scroll_params.race_registry.as_deref(),
            );
            matches!(
                learn_technique_if_allowed(
                    &skill_scroll_params.technique_registry,
                    &mut known,
                    cultivation,
                    &meridians,
                    severed,
                    technique_id.as_str(),
                    0.0,
                    intrinsic_plan.is_humanoid,
                    intrinsic_plan.meridian_profile.as_ref(),
                ),
                ScrollReadOutcome::Learned
            )
        };
        if learned {
            if let Some(tx) = skill_scroll_params.technique_learned_tx.as_deref_mut() {
                tx.send(TechniqueLearnedEvent {
                    player: entity,
                    technique_id: technique_id.clone(),
                    source: LearnSource::Scroll {
                        item_id: template.id.clone(),
                    },
                });
            }
        }
    }

    if let Some(tx) = skill_scroll_params.technique_scroll_read_tx.as_deref_mut() {
        tx.send(TechniqueScrollReadEvent {
            player: entity,
            technique_id: technique_id.clone(),
            source_item: template.id.clone(),
            outcome: outcome.clone(),
        });
    }

    // central-review 2012 #3：拒绝原因必须在 wire 上可观察——只下发不变快照时，
    // client 无法区分「RealmTooLow 拒绝」与「静默忽略/错误原因拒绝」。非习得拒绝
    // 走既有 `InventoryMoveRejectedV1` 契约（reason=realm_too_low / race_mismatch，
    // RealmTooLow 带 required_realm），bot 场景据 reason 断言具体原因。
    if let Some(reject_reason) = match &outcome {
        ScrollReadOutcome::RealmTooLow { required, .. } => {
            Some(InventoryMoveRejectReason::RealmTooLow {
                required_realm: crate::schema::cultivation::realm_to_string(*required).to_string(),
            })
        }
        ScrollReadOutcome::RaceMismatch => Some(InventoryMoveRejectReason::RaceMismatch),
        ScrollReadOutcome::Learned
        | ScrollReadOutcome::AlreadyKnown
        | ScrollReadOutcome::MeridianSevered { .. }
        | ScrollReadOutcome::MeridianMissing { .. }
        | ScrollReadOutcome::FormAnchorClosed
        | ScrollReadOutcome::InvalidScroll => None,
    } {
        emit_inventory_move_rejected(entity, &reject_reason, clients);
    }

    resync_technique_scroll_use(
        entity,
        inventories,
        clients,
        player_states,
        skill_scroll_params,
        match outcome {
            ScrollReadOutcome::Learned => "technique_scroll_learned",
            ScrollReadOutcome::AlreadyKnown => "technique_scroll_already_known",
            ScrollReadOutcome::RealmTooLow { .. } => "technique_scroll_realm_too_low",
            ScrollReadOutcome::RaceMismatch => "technique_scroll_race_mismatch",
            ScrollReadOutcome::MeridianSevered { .. } => "technique_scroll_meridian_severed",
            ScrollReadOutcome::MeridianMissing { .. } => "technique_scroll_meridian_missing",
            ScrollReadOutcome::FormAnchorClosed => "technique_scroll_form_anchor_closed",
            ScrollReadOutcome::InvalidScroll => "technique_scroll_invalid",
        },
    );
}

fn resync_technique_scroll_use(
    entity: Entity,
    inventories: &Query<&mut PlayerInventory>,
    clients: &mut Query<(&Username, &mut Client)>,
    player_states: &Query<&PlayerState>,
    skill_scroll_params: &SkillScrollRequestParams,
    reason: &str,
) {
    let Ok(player_state) = player_states.get(entity) else {
        return;
    };
    let Ok(cultivation) = skill_scroll_params.cultivations.get(entity) else {
        return;
    };
    let Ok((username, mut client)) = clients.get_mut(entity) else {
        return;
    };
    if let Ok(inventory) = inventories.get(entity) {
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
    if let Ok(known) = skill_scroll_params.known_techniques.get(entity) {
        send_techniques_snapshot_to_client(
            &skill_scroll_params.technique_registry,
            entity,
            &mut client,
            username.0.as_str(),
            known,
        );
    }
}

fn skill_scroll_spec(template_id: &str) -> Option<(SkillId, u32)> {
    match template_id {
        "skill_scroll_herbalism_baicao_can" => Some((SkillId::Herbalism, 500)),
        "skill_scroll_alchemy_danhuo_can" => Some((SkillId::Alchemy, 500)),
        "skill_scroll_forging_duantie_can" => Some((SkillId::Forging, 500)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alchemy::recipe::{
        FireProfile, IngredientSpec, Outcomes, Recipe, RecipeStage, ToleranceSpec,
    };
    use crate::botany::components::{
        BotanyHarvestMode, BotanyPhase, HarvestSession, HarvestSessionStore,
    };
    use crate::botany::harvest::harvest_duration_ticks_for;
    use crate::botany::registry::BotanyPlantId;
    use crate::combat::components::{Lifecycle, UnlockedStyles, WoundKind, Wounds};
    use crate::cultivation::components::{Cultivation, MeridianId, MeridianSystem, Realm};
    use crate::cultivation::known_techniques::KnownTechniques;
    use crate::cultivation::tribulation::TribulationState;
    use crate::forge::session::{ForgeSession, StepState};
    use crate::inventory::{
        BlueprintScrollSpec, ContainerState, InscriptionScrollSpec, InventoryRevision,
        ItemCategory, ItemEffect, ItemInstance, ItemRarity, ItemTemplate, PlacedItemState,
    };
    use crate::lingtian::events::{
        StartDrainQiRequest, StartHarvestRequest, StartPlantingRequest, StartRenewRequest,
        StartReplenishRequest, StartTillRequest,
    };
    use crate::npc::faction::{FactionId, FactionRank, MissionQueue, NamedFactionId, Reputation};
    use crate::skill::components::SkillSet;
    use crate::zhenfa::trap_content::TrapTargetFace;
    use valence::entity::{EntityId, EntityPlugin};
    use valence::prelude::{
        ident, App, BlockPos, BlockState, DVec3, Entity, EntityKind, EventReader,
        IntoSystemConfigs, OldPosition, Position, ResMut, UnloadedChunk, Update,
    };
    use valence::protocol::packets::play::{CustomPayloadS2c, GameMessageS2c};
    use valence::testing::{create_mock_client, MockClientHelper, ScenarioSingleClient};

    #[derive(Default)]
    struct CapturedBreakthroughRequests(Vec<BreakthroughRequest>);

    impl valence::prelude::Resource for CapturedBreakthroughRequests {}

    #[derive(Default)]
    struct CapturedForgeRequests(Vec<ForgeRequest>);

    impl valence::prelude::Resource for CapturedForgeRequests {}

    #[derive(Default)]
    struct CapturedStartDuXuRequests(Vec<StartDuXuRequest>);

    impl valence::prelude::Resource for CapturedStartDuXuRequests {}

    #[derive(Default)]
    struct CapturedInsightChoices(Vec<InsightChosen>);

    impl valence::prelude::Resource for CapturedInsightChoices {}

    #[derive(Default)]
    struct CapturedMineralProbes(Vec<MineralProbeIntent>);

    impl valence::prelude::Resource for CapturedMineralProbes {}

    #[test]
    fn combat_pill_buff_status_payload_preserves_hud_fields() {
        let bytes = build_pill_buff_status_payload("tie_bi_san", 1800, 1.25, 2)
            .expect("valid pill buff status payload should serialize");
        let value: serde_json::Value =
            serde_json::from_slice(&bytes).expect("test build emits JSON server_data");

        assert_eq!(
            value["type"], "pill_buff_status",
            "expected pill_buff_status because the client router dispatches by type"
        );
        assert_eq!(
            value["buff_id"], "tie_bi_san",
            "expected buff_id because the HUD replaces buffs by id"
        );
        assert_eq!(
            value["remaining_ticks"], 3600,
            "expected base ticks multiplied by duration_multiplier"
        );
        assert_eq!(
            value["effect_multiplier"], 1.25,
            "expected positive effect multiplier to be preserved for HUD display"
        );
    }

    #[test]
    fn combat_pill_buff_status_rejects_invalid_multiplier() {
        assert!(
            build_pill_buff_status_payload("tie_bi_san", 1800, f32::NAN, 1).is_none(),
            "NaN multiplier must not produce a client HUD payload"
        );
        assert!(
            build_pill_buff_status_payload("tie_bi_san", 1800, 0.0, 1).is_none(),
            "zero multiplier must not produce a client HUD payload"
        );
    }

    #[test]
    fn combat_pill_buff_status_rejects_empty_buff_id() {
        assert!(
            build_pill_buff_status_payload("  ", 1800, 1.25, 1).is_none(),
            "blank buff_id must not produce a client HUD payload"
        );
    }

    #[test]
    fn combat_pill_buff_status_duration_zero_uses_base_ticks() {
        let bytes = build_pill_buff_status_payload("tie_bi_san", 1800, 1.25, 0)
            .expect("duration_multiplier=0 should fall back to one duration");
        let value: serde_json::Value =
            serde_json::from_slice(&bytes).expect("test build emits JSON server_data");

        assert_eq!(
            value["remaining_ticks"], 1800,
            "expected duration_multiplier=0 to use max(1) fallback"
        );
    }

    #[test]
    fn combat_pill_buff_status_remaining_ticks_clamps_to_u32_max() {
        let bytes = build_pill_buff_status_payload("tie_bi_san", u64::from(u32::MAX), 1.25, 2)
            .expect("oversized duration should serialize after clamping");
        let value: serde_json::Value =
            serde_json::from_slice(&bytes).expect("test build emits JSON server_data");

        assert_eq!(
            value["remaining_ticks"],
            u64::from(u32::MAX),
            "expected remaining_ticks to clamp at u32::MAX for proto/client compatibility"
        );
    }

    #[derive(Default)]
    struct CapturedSpiritNichePlaces(Vec<SpiritNichePlaceRequest>);

    impl valence::prelude::Resource for CapturedSpiritNichePlaces {}

    #[derive(Default)]
    struct CapturedSpiritNicheRepairs(Vec<SpiritNicheRepairRequest>);

    impl valence::prelude::Resource for CapturedSpiritNicheRepairs {}

    #[derive(Default)]
    struct CapturedSpiritNicheCoordinateReveals(Vec<SpiritNicheCoordinateRevealRequest>);

    impl valence::prelude::Resource for CapturedSpiritNicheCoordinateReveals {}

    #[derive(Default)]
    struct CapturedCoffinOpenRequests(Vec<CoffinOpenRequest>);

    impl valence::prelude::Resource for CapturedCoffinOpenRequests {}

    #[derive(Default)]
    struct CapturedCoffinBreakRequests(Vec<crate::coffin::CoffinBreakRequest>);

    impl valence::prelude::Resource for CapturedCoffinBreakRequests {}

    #[derive(Default)]
    struct CapturedCoffinMenuReclaimRequests(Vec<crate::coffin::CoffinMenuReclaimRequest>);

    impl valence::prelude::Resource for CapturedCoffinMenuReclaimRequests {}

    #[derive(Default)]
    struct CapturedInscriptionScrolls(Vec<InscriptionScrollSubmit>);

    impl valence::prelude::Resource for CapturedInscriptionScrolls {}

    #[derive(Default)]
    struct CapturedTemperingHits(Vec<TemperingHit>);

    impl valence::prelude::Resource for CapturedTemperingHits {}

    #[derive(Default)]
    struct CapturedConsecrationInjects(Vec<ConsecrationInject>);

    impl valence::prelude::Resource for CapturedConsecrationInjects {}

    #[derive(Default)]
    struct CapturedStepAdvances(Vec<StepAdvance>);

    impl valence::prelude::Resource for CapturedStepAdvances {}

    #[derive(Default)]
    struct CapturedQiColorInspectRequests(Vec<QiColorInspectRequest>);

    impl valence::prelude::Resource for CapturedQiColorInspectRequests {}

    fn capture_breakthrough_requests(
        mut events: EventReader<BreakthroughRequest>,
        mut captured: ResMut<CapturedBreakthroughRequests>,
    ) {
        captured.0.extend(events.read().cloned());
    }

    fn capture_forge_requests(
        mut events: EventReader<ForgeRequest>,
        mut captured: ResMut<CapturedForgeRequests>,
    ) {
        captured.0.extend(events.read().cloned());
    }

    fn capture_start_du_xu_requests(
        mut events: EventReader<StartDuXuRequest>,
        mut captured: ResMut<CapturedStartDuXuRequests>,
    ) {
        captured.0.extend(events.read().cloned());
    }

    fn capture_insight_choices(
        mut events: EventReader<InsightChosen>,
        mut captured: ResMut<CapturedInsightChoices>,
    ) {
        captured.0.extend(events.read().cloned());
    }

    fn capture_mineral_probes(
        mut events: EventReader<MineralProbeIntent>,
        mut captured: ResMut<CapturedMineralProbes>,
    ) {
        captured.0.extend(events.read().cloned());
    }

    fn capture_spirit_niche_places(
        mut events: EventReader<SpiritNichePlaceRequest>,
        mut captured: ResMut<CapturedSpiritNichePlaces>,
    ) {
        captured.0.extend(events.read().cloned());
    }

    fn capture_spirit_niche_repairs(
        mut events: EventReader<SpiritNicheRepairRequest>,
        mut captured: ResMut<CapturedSpiritNicheRepairs>,
    ) {
        captured.0.extend(events.read().cloned());
    }

    fn capture_spirit_niche_coordinate_reveals(
        mut events: EventReader<SpiritNicheCoordinateRevealRequest>,
        mut captured: ResMut<CapturedSpiritNicheCoordinateReveals>,
    ) {
        captured.0.extend(events.read().cloned());
    }

    fn capture_coffin_open_requests(
        mut events: EventReader<CoffinOpenRequest>,
        mut captured: ResMut<CapturedCoffinOpenRequests>,
    ) {
        captured.0.extend(events.read().cloned());
    }

    fn capture_coffin_break_requests(
        mut events: EventReader<crate::coffin::CoffinBreakRequest>,
        mut captured: ResMut<CapturedCoffinBreakRequests>,
    ) {
        captured.0.extend(events.read().cloned());
    }

    fn capture_coffin_menu_reclaim_requests(
        mut events: EventReader<crate::coffin::CoffinMenuReclaimRequest>,
        mut captured: ResMut<CapturedCoffinMenuReclaimRequests>,
    ) {
        captured.0.extend(events.read().cloned());
    }

    fn capture_inscription_scrolls(
        mut events: EventReader<InscriptionScrollSubmit>,
        mut captured: ResMut<CapturedInscriptionScrolls>,
    ) {
        captured.0.extend(events.read().cloned());
    }

    fn capture_tempering_hits(
        mut events: EventReader<TemperingHit>,
        mut captured: ResMut<CapturedTemperingHits>,
    ) {
        captured.0.extend(events.read().cloned());
    }

    fn capture_consecration_injects(
        mut events: EventReader<ConsecrationInject>,
        mut captured: ResMut<CapturedConsecrationInjects>,
    ) {
        captured.0.extend(events.read().cloned());
    }

    fn capture_step_advances(
        mut events: EventReader<StepAdvance>,
        mut captured: ResMut<CapturedStepAdvances>,
    ) {
        captured.0.extend(events.read().cloned());
    }

    fn capture_qi_color_inspect_requests(
        mut events: EventReader<QiColorInspectRequest>,
        mut captured: ResMut<CapturedQiColorInspectRequests>,
    ) {
        captured.0.extend(events.read().cloned());
    }

    fn skill_scroll_item(instance_id: u64, template_id: &str) -> ItemInstance {
        ItemInstance {
            instance_id,
            template_id: template_id.to_string(),
            display_name: template_id.to_string(),
            grid_w: 1,
            grid_h: 2,
            weight: 0.05,
            rarity: ItemRarity::Uncommon,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 1.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        }
    }

    fn test_forge_template_registry() -> ItemRegistry {
        ItemRegistry::from_map(HashMap::from([
            (
                "blueprint_scroll_ling_feng".to_string(),
                ItemTemplate {
                    id: "blueprint_scroll_ling_feng".to_string(),
                    display_name: "灵锋图谱残卷".to_string(),
                    category: ItemCategory::Misc,
                    placeable: None,
                    max_stack_count: 1,
                    grid_w: 1,
                    grid_h: 1,
                    base_weight: 0.05,
                    rarity: ItemRarity::Rare,
                    spirit_quality_initial: 0.9,
                    description: String::new(),
                    effect: None,
                    cast_duration_ms: crate::inventory::DEFAULT_CAST_DURATION_MS,
                    cooldown_ms: crate::inventory::DEFAULT_COOLDOWN_MS,
                    weapon_spec: None,
                    forge_station_spec: None,
                    blueprint_scroll_spec: Some(BlueprintScrollSpec {
                        blueprint_id: "ling_feng_v0".to_string(),
                    }),
                    inscription_scroll_spec: None,
                    technique_scroll_spec: None,
                    readable_scroll_spec: None,
                    recipe_fragment_spec: None,
                    container_spec: None,
                    shelflife_profile: None,
                    shield_spec: None,
                    shelflife_track: None,
                    wearer_race: crate::body_plan::types::RaceGateOwned::default(),
                },
            ),
            (
                "inscription_scroll_sharp_v0".to_string(),
                ItemTemplate {
                    id: "inscription_scroll_sharp_v0".to_string(),
                    display_name: "锐意铭文残卷".to_string(),
                    category: ItemCategory::Misc,
                    placeable: None,
                    max_stack_count: 1,
                    grid_w: 1,
                    grid_h: 1,
                    base_weight: 0.03,
                    rarity: ItemRarity::Uncommon,
                    spirit_quality_initial: 0.8,
                    description: String::new(),
                    effect: None,
                    cast_duration_ms: crate::inventory::DEFAULT_CAST_DURATION_MS,
                    cooldown_ms: crate::inventory::DEFAULT_COOLDOWN_MS,
                    weapon_spec: None,
                    forge_station_spec: None,
                    blueprint_scroll_spec: None,
                    inscription_scroll_spec: Some(InscriptionScrollSpec {
                        inscription_id: "sharp_v0".to_string(),
                    }),
                    technique_scroll_spec: None,
                    readable_scroll_spec: None,
                    recipe_fragment_spec: None,
                    container_spec: None,
                    shelflife_profile: None,
                    shield_spec: None,
                    shelflife_track: None,
                    wearer_race: crate::body_plan::types::RaceGateOwned::default(),
                },
            ),
        ]))
    }

    fn inventory_with_skill_scroll(item: ItemInstance) -> PlayerInventory {
        PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(0),
            containers: vec![ContainerState {
                quick_access: false,
                id: "main_pack".into(),
                name: "main_pack".into(),
                rows: 5,
                cols: 7,
                items: vec![PlacedItemState {
                    row: 0,
                    col: 0,
                    instance: item,
                }],

                owner_instance_id: None,
            }],
            equipped: Default::default(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 50.0,
        }
    }

    fn inventory_with_stack(template_id: &str, count: u32) -> PlayerInventory {
        PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(0),
            containers: vec![ContainerState {
                quick_access: false,
                id: "main_pack".into(),
                name: "main_pack".into(),
                rows: 5,
                cols: 7,
                items: vec![PlacedItemState {
                    row: 0,
                    col: 0,
                    instance: inventory_test_item(9001, template_id, count),
                }],

                owner_instance_id: None,
            }],
            equipped: Default::default(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 50.0,
        }
    }

    fn inventory_test_item(instance_id: u64, template_id: &str, stack_count: u32) -> ItemInstance {
        ItemInstance {
            instance_id,
            template_id: template_id.to_string(),
            display_name: template_id.to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.1,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count,
            spirit_quality: 1.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        }
    }

    fn empty_inventory() -> PlayerInventory {
        PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(0),
            containers: vec![ContainerState {
                quick_access: false,
                id: "main_pack".into(),
                name: "main_pack".into(),
                rows: 5,
                cols: 7,
                items: Vec::new(),

                owner_instance_id: None,
            }],
            equipped: Default::default(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 50.0,
        }
    }

    fn collect_server_data_payload_types(helper: &mut MockClientHelper) -> Vec<String> {
        helper
            .collect_received()
            .0
            .into_iter()
            .filter_map(|frame| {
                let packet = frame.decode::<CustomPayloadS2c>().ok()?;
                if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                    return None;
                }
                let value = serde_json::from_slice::<serde_json::Value>(packet.data.0 .0).ok()?;
                value.get("type")?.as_str().map(str::to_string)
            })
            .collect()
    }

    fn run_supply_coffin_open_payload_case(player_pos: DVec3) -> (App, Entity, u64, Vec<String>) {
        use crate::inventory::external_container::{ExternalContainer, ExternalContainerRegistry};
        use crate::supply_coffin::interact::{
            handle_supply_coffin_interact, SupplyCoffinOpenRequest, SupplyCoffinOpened,
        };
        use crate::supply_coffin::{SupplyCoffinGrade, SupplyCoffinRegistry};

        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        app.add_plugins(EntityPlugin);
        register_request_app(&mut app);
        app.add_event::<SupplyCoffinOpenRequest>();
        app.add_event::<SupplyCoffinOpened>();
        app.insert_resource(ExternalContainerRegistry::default());
        app.insert_resource(crate::inventory::InventoryInstanceIdAllocator::default());
        app.insert_resource(crate::inventory::load_item_registry().expect("item registry"));
        app.add_systems(
            Update,
            handle_supply_coffin_interact.after(handle_client_request_payloads),
        );

        let target = app
            .world_mut()
            .spawn((
                crate::world::entity_model::COFFIN_COMMON_ENTITY_KIND,
                EntityId::default(),
                Position::new(DVec3::new(0.0, 64.0, 0.0)),
                OldPosition::new(DVec3::new(0.0, 64.0, 0.0)),
            ))
            .id();
        let mut registry =
            SupplyCoffinRegistry::new((DVec3::ZERO, DVec3::new(100.0, 100.0, 100.0)), 65.0, 0x2468);
        registry.insert_active(
            target,
            SupplyCoffinGrade::Common,
            DVec3::new(0.0, 64.0, 0.0),
            crate::supply_coffin::current_wall_clock_secs(),
        );
        let rng_before = registry.rng_state;
        app.insert_resource(registry);

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let player = app
            .world_mut()
            .spawn((
                client_bundle,
                empty_inventory(),
                Cultivation::default(),
                PlayerState::default(),
                CurrentDimension(DimensionKind::Overworld),
            ))
            .id();
        app.world_mut()
            .entity_mut(player)
            .insert(Position::new(player_pos));

        app.update();
        let entity_id = app
            .world()
            .get::<EntityId>(target)
            .expect("EntityPlugin must assign the supply-coffin protocol id")
            .get();
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: player,
                channel: ident!("bong:client_request").into(),
                data: serde_json::to_vec(&ClientRequestV1::SupplyCoffinOpen { v: 1, entity_id })
                    .expect("supply_coffin_open request should serialize")
                    .into_boxed_slice(),
            });

        app.update();
        flush_all_client_packets(&mut app);
        let payload_types = collect_server_data_payload_types(&mut helper);
        let opened = app.world().get::<ExternalContainer>(target).is_some();
        if opened {
            assert!(
                app.world()
                    .resource::<ExternalContainerRegistry>()
                    .sessions
                    .values()
                    .any(|entity| *entity == target),
                "successful C2S open must register the target session"
            );
        }
        (app, target, rng_before, payload_types)
    }

    #[test]
    fn supply_coffin_open_payload_wiring_accepts_finite_and_rejects_non_finite_coordinates() {
        let (finite, target, _rng_before, payload_types) =
            run_supply_coffin_open_payload_case(DVec3::new(0.0, 64.0, 0.0));
        assert!(
            finite
                .world()
                .get::<crate::inventory::external_container::ExternalContainer>(target)
                .is_some(),
            "real supply_coffin_open C2S payload must reach the interact consumer"
        );
        assert!(
            payload_types.iter().any(|ty| ty == "loot_container_open"),
            "successful C2S open must emit loot_container_open S2C; payloads={payload_types:?}"
        );

        for (label, x) in [
            ("nan", f64::NAN),
            ("positive_infinity", f64::INFINITY),
            ("negative_infinity", f64::NEG_INFINITY),
        ] {
            let (app, target, rng_before, payload_types) =
                run_supply_coffin_open_payload_case(DVec3::new(x, 64.0, 0.0));
            assert!(
                app.world()
                    .get::<crate::inventory::external_container::ExternalContainer>(target)
                    .is_none(),
                "{label} C2S open must not create a session container"
            );
            assert!(
                app.world()
                    .resource::<crate::inventory::external_container::ExternalContainerRegistry>()
                    .sessions
                    .is_empty(),
                "{label} C2S open must not allocate a session"
            );
            assert_eq!(
                app.world()
                    .resource::<crate::supply_coffin::SupplyCoffinRegistry>()
                    .rng_state,
                rng_before,
                "{label} C2S open must reject before RNG advances"
            );
            assert!(
                payload_types.iter().all(|ty| ty != "loot_container_open"),
                "{label} C2S open must not emit loot_container_open; payloads={payload_types:?}"
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn run_external_container_move_case(
        player_dimension: Option<DimensionKind>,
        player_pos: DVec3,
        source_kind: crate::inventory::external_container::ExternalContainerKind,
        source_active: bool,
        session_registered: bool,
        owner_is_player: bool,
    ) -> (App, Entity, Entity, Vec<String>) {
        run_external_container_move_case_with_source(
            player_dimension,
            player_pos,
            source_kind,
            source_active,
            session_registered,
            owner_is_player,
            0,
            0,
            1,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn run_external_container_move_case_with_source(
        player_dimension: Option<DimensionKind>,
        player_pos: DVec3,
        source_kind: crate::inventory::external_container::ExternalContainerKind,
        source_active: bool,
        session_registered: bool,
        owner_is_player: bool,
        source_row: u64,
        source_col: u64,
        denial_count: usize,
    ) -> (App, Entity, Entity, Vec<String>) {
        use crate::inventory::external_container::{ExternalContainer, ExternalContainerRegistry};
        use crate::supply_coffin::{SupplyCoffinGrade, SupplyCoffinRegistry};

        const SESSION_ID: u64 = 77;
        const INSTANCE_ID: u64 = 7001;
        const COFFIN_POS: DVec3 = DVec3::new(0.0, 64.0, 0.0);

        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let player = app
            .world_mut()
            .spawn((
                client_bundle,
                empty_inventory(),
                Cultivation::default(),
                PlayerState::default(),
                Lifecycle::default(),
            ))
            .id();
        app.world_mut()
            .entity_mut(player)
            .insert(Position::new(player_pos));
        if let Some(dimension) = player_dimension {
            app.world_mut()
                .entity_mut(player)
                .insert(CurrentDimension(dimension));
        }

        let owner = if owner_is_player {
            player
        } else {
            app.world_mut().spawn_empty().id()
        };
        let coffin = app
            .world_mut()
            .spawn((
                ExternalContainer {
                    session_id: SESSION_ID,
                    container: ContainerState {
                        id: ExternalContainer::container_id(SESSION_ID),
                        name: "external_test".to_string(),
                        rows: 3,
                        cols: 4,
                        items: vec![PlacedItemState {
                            row: 0,
                            col: 0,
                            instance: inventory_test_item(INSTANCE_ID, "spiritual_ore", 1),
                        }],
                        owner_instance_id: None,
                        quick_access: false,
                    },
                    opened_by: Some(owner),
                    timeout_wall_secs: u64::MAX,
                    source_kind,
                },
                Position::new(COFFIN_POS),
                CurrentDimension(DimensionKind::Overworld),
            ))
            .id();

        let mut ext_registry = ExternalContainerRegistry {
            next_session_id: SESSION_ID + 1,
            ..Default::default()
        };
        if session_registered {
            ext_registry.sessions.insert(SESSION_ID, coffin);
        }
        app.insert_resource(ext_registry);

        let mut coffin_registry =
            SupplyCoffinRegistry::new((DVec3::ZERO, DVec3::new(100.0, 100.0, 100.0)), 65.0, 0x9876);
        if source_active {
            coffin_registry.insert_active(
                coffin,
                SupplyCoffinGrade::Common,
                COFFIN_POS,
                crate::supply_coffin::current_wall_clock_secs(),
            );
        }
        app.insert_resource(coffin_registry);

        for _ in 0..denial_count {
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: player,
                    channel: ident!("bong:client_request").into(),
                    data: format!(
                        r#"{{"type":"external_container_move","v":1,"session_id":{SESSION_ID},"instance_id":{INSTANCE_ID},"from":{{"kind":"container","container_id":"ext_{SESSION_ID}","row":{source_row},"col":{source_col}}},"to":{{"kind":"container","container_id":"main_pack","row":0,"col":0}}}}"#
                    )
                    .into_bytes()
                    .into_boxed_slice(),
                });
        }

        app.update();
        flush_all_client_packets(&mut app);
        let payload_types = collect_server_data_payload_types(&mut helper);
        (app, player, coffin, payload_types)
    }

    fn assert_external_move_rejected_without_mutation(app: &App, player: Entity, coffin: Entity) {
        let ext = app
            .world()
            .get::<crate::inventory::external_container::ExternalContainer>(coffin)
            .expect("external container must remain attached after rejection");
        assert!(
            ext.container
                .items
                .iter()
                .any(|item| item.instance.instance_id == 7001),
            "rejected move must keep instance 7001 in the external container; actual items={:?}",
            ext.container
                .items
                .iter()
                .map(|item| item.instance.instance_id)
                .collect::<Vec<_>>()
        );
        let inventory = app
            .world()
            .get::<PlayerInventory>(player)
            .expect("test player keeps inventory component");
        assert!(
            inventory.containers.iter().all(|container| container
                .items
                .iter()
                .all(|item| item.instance.instance_id != 7001)),
            "rejected move must not copy instance 7001 into player inventory"
        );
        assert_eq!(
            inventory.revision,
            InventoryRevision(0),
            "rejected move must not advance inventory revision"
        );
    }

    #[test]
    fn supply_coffin_external_move_real_c2s_rejects_cross_dimension_same_xyz_while_session_is_valid_and_resyncs(
    ) {
        let (app, player, coffin, payload_types) = run_external_container_move_case(
            Some(DimensionKind::Tsy),
            DVec3::new(0.0, 64.0, 0.0),
            crate::inventory::external_container::ExternalContainerKind::SupplyCoffin {
                grade: crate::supply_coffin::SupplyCoffinGrade::Common,
            },
            true,
            true,
            true,
        );

        assert_external_move_rejected_without_mutation(&app, player, coffin);
        assert_eq!(
            app.world()
                .resource::<crate::inventory::external_container::ExternalContainerRegistry>()
                .sessions
                .get(&77),
            Some(&coffin),
            "real C2S move must reach dimension authority while session mapping is still valid"
        );
        assert_eq!(
            app.world()
                .get::<crate::inventory::external_container::ExternalContainer>(coffin)
                .expect("supply coffin session must remain attached")
                .opened_by,
            Some(player),
            "real C2S move must be rejected while opened_by still proves requester ownership"
        );
        assert!(
            app.world()
                .resource::<crate::supply_coffin::SupplyCoffinRegistry>()
                .active
                .contains_key(&coffin),
            "real C2S move must be rejected while authoritative supply-coffin source is still active"
        );
        assert!(
            payload_types.iter().any(|ty| ty == "event_alert"),
            "live gate rejection must use the bounded feedback path; payloads={payload_types:?}"
        );
        assert!(
            payload_types.iter().any(|ty| ty == "loot_container_update")
                && payload_types.iter().any(|ty| ty == "inventory_snapshot"),
            "a gate rejection must keep the existing read-only external/inventory resync contract without entering the mutation handler; payloads={payload_types:?}"
        );
    }

    #[test]
    fn supply_coffin_external_move_rejects_missing_dimension() {
        let (app, player, coffin, _payload_types) = run_external_container_move_case(
            None,
            DVec3::new(0.0, 64.0, 0.0),
            crate::inventory::external_container::ExternalContainerKind::SupplyCoffin {
                grade: crate::supply_coffin::SupplyCoffinGrade::Common,
            },
            true,
            true,
            true,
        );

        assert_external_move_rejected_without_mutation(&app, player, coffin);
    }

    #[test]
    fn supply_coffin_external_move_rejects_non_finite_coordinates_and_resyncs() {
        for (label, x) in [
            ("nan", f64::NAN),
            ("positive_infinity", f64::INFINITY),
            ("negative_infinity", f64::NEG_INFINITY),
        ] {
            let (app, player, coffin, payload_types) = run_external_container_move_case(
                Some(DimensionKind::Overworld),
                DVec3::new(x, 64.0, 0.0),
                crate::inventory::external_container::ExternalContainerKind::SupplyCoffin {
                    grade: crate::supply_coffin::SupplyCoffinGrade::Common,
                },
                true,
                true,
                true,
            );

            assert_external_move_rejected_without_mutation(&app, player, coffin);
            assert!(
                payload_types.iter().any(|ty| ty == "event_alert"),
                "{label} live gate rejection must emit bounded feedback; payloads={payload_types:?}"
            );
        }
    }

    #[test]
    fn supply_coffin_external_move_rejects_out_of_lifecycle_range() {
        let (app, player, coffin, _payload_types) = run_external_container_move_case(
            Some(DimensionKind::Overworld),
            DVec3::new(6.501, 64.0, 0.0),
            crate::inventory::external_container::ExternalContainerKind::SupplyCoffin {
                grade: crate::supply_coffin::SupplyCoffinGrade::Common,
            },
            true,
            true,
            true,
        );

        assert_external_move_rejected_without_mutation(&app, player, coffin);
    }

    #[test]
    fn supply_coffin_external_move_rejects_when_active_source_disappears() {
        let (app, player, coffin, _payload_types) = run_external_container_move_case(
            Some(DimensionKind::Overworld),
            DVec3::new(0.0, 64.0, 0.0),
            crate::inventory::external_container::ExternalContainerKind::SupplyCoffin {
                grade: crate::supply_coffin::SupplyCoffinGrade::Common,
            },
            false,
            true,
            true,
        );

        assert_external_move_rejected_without_mutation(&app, player, coffin);
    }

    #[test]
    fn external_move_owner_mismatch_keeps_items_and_resyncs_requester_inventory() {
        let (app, player, coffin, payload_types) = run_external_container_move_case(
            Some(DimensionKind::Overworld),
            DVec3::new(0.0, 64.0, 0.0),
            crate::inventory::external_container::ExternalContainerKind::SupplyCoffin {
                grade: crate::supply_coffin::SupplyCoffinGrade::Common,
            },
            true,
            true,
            false,
        );

        assert_external_move_rejected_without_mutation(&app, player, coffin);
        assert!(
            payload_types.iter().any(|ty| ty == "event_alert"),
            "non-owner live gate rejection must emit bounded feedback; payloads={payload_types:?}"
        );
        assert!(
            payload_types.iter().any(|ty| ty == "inventory_snapshot")
                && payload_types.iter().all(|ty| ty != "loot_container_update"),
            "non-owner rejection may resync only the requester's inventory and must not expose external contents; payloads={payload_types:?}"
        );
    }

    #[test]
    fn external_move_non_owner_cross_dimension_does_not_disclose_container() {
        let (app, player, coffin, payload_types) = run_external_container_move_case(
            Some(DimensionKind::Tsy),
            DVec3::new(0.0, 64.0, 0.0),
            crate::inventory::external_container::ExternalContainerKind::SupplyCoffin {
                grade: crate::supply_coffin::SupplyCoffinGrade::Common,
            },
            true,
            true,
            false,
        );

        assert_external_move_rejected_without_mutation(&app, player, coffin);
        assert!(
            payload_types.iter().any(|ty| ty == "event_alert"),
            "non-owner cross-dimension rejection must emit bounded feedback; payloads={payload_types:?}"
        );
        assert!(
            payload_types.iter().any(|ty| ty == "inventory_snapshot")
                && payload_types.iter().all(|ty| ty != "loot_container_update"),
            "non-owner rejection must not disclose external contents even when dimension gate rejects first; payloads={payload_types:?}"
        );
    }

    #[test]
    fn external_move_stale_session_resyncs_inventory_without_mutation() {
        let (app, player, coffin, payload_types) = run_external_container_move_case(
            Some(DimensionKind::Overworld),
            DVec3::new(0.0, 64.0, 0.0),
            crate::inventory::external_container::ExternalContainerKind::SupplyCoffin {
                grade: crate::supply_coffin::SupplyCoffinGrade::Common,
            },
            true,
            false,
            true,
        );

        assert_external_move_rejected_without_mutation(&app, player, coffin);
        assert!(
            payload_types.iter().any(|ty| ty == "event_alert"),
            "unknown/stale session must use bounded gate feedback; payloads={payload_types:?}"
        );
    }

    #[test]
    fn external_move_stale_session_resyncs_even_when_feedback_budget_suppresses_alert() {
        let (app, player, coffin, payload_types) = run_external_container_move_case_with_source(
            Some(DimensionKind::Overworld),
            DVec3::new(0.0, 64.0, 0.0),
            crate::inventory::external_container::ExternalContainerKind::SupplyCoffin {
                grade: crate::supply_coffin::SupplyCoffinGrade::Common,
            },
            true,
            false,
            true,
            0,
            0,
            2,
        );

        assert_external_move_rejected_without_mutation(&app, player, coffin);
        assert_eq!(
            payload_types
                .iter()
                .filter(|payload_type| payload_type.as_str() == "event_alert")
                .count(),
            1,
            "feedback budget must suppress the second duplicate alert while preserving the first"
        );
        assert_eq!(
            payload_types
                .iter()
                .filter(|payload_type| payload_type.as_str() == "inventory_snapshot")
                .count(),
            2,
            "each stale-session rejection must still resync the authoritative player inventory even when alert feedback is suppressed"
        );
    }

    #[test]
    fn supply_coffin_external_move_accepts_exact_lifecycle_boundary() {
        let (app, player, coffin, payload_types) = run_external_container_move_case(
            Some(DimensionKind::Overworld),
            DVec3::new(6.5, 64.0, 0.0),
            crate::inventory::external_container::ExternalContainerKind::SupplyCoffin {
                grade: crate::supply_coffin::SupplyCoffinGrade::Common,
            },
            true,
            true,
            true,
        );

        let ext = app
            .world()
            .get::<crate::inventory::external_container::ExternalContainer>(coffin)
            .expect("coffin remains after successful move");
        assert!(
            ext.container.items.is_empty(),
            "authorized boundary move must remove the item from the external container"
        );
        let inventory = app.world().get::<PlayerInventory>(player).unwrap();
        assert!(
            inventory.containers.iter().any(|container| container
                .items
                .iter()
                .any(|item| item.instance.instance_id == 7001)),
            "authorized boundary move must place instance 7001 into player inventory"
        );
        assert!(
            payload_types.iter().any(|ty| ty == "loot_container_update")
                && payload_types.iter().any(|ty| ty == "inventory_snapshot"),
            "successful move must keep existing update + inventory snapshot contract; payloads={payload_types:?}"
        );
    }

    #[test]
    fn external_move_rejects_forged_external_source_coordinates_without_mutation() {
        let (app, player, coffin, payload_types) = run_external_container_move_case_with_source(
            Some(DimensionKind::Overworld),
            DVec3::new(0.0, 64.0, 0.0),
            crate::inventory::external_container::ExternalContainerKind::StorageCrate {
                is_herb: false,
            },
            false,
            true,
            true,
            0,
            1,
            1,
        );

        assert_external_move_rejected_without_mutation(&app, player, coffin);
        assert!(
            payload_types.iter().any(|ty| ty == "loot_container_update"),
            "authorized owner with forged source coordinates must receive authoritative external resync; payloads={payload_types:?}"
        );
        assert!(
            payload_types.iter().any(|ty| ty == "inventory_snapshot"),
            "forged source rejection must resync player inventory; payloads={payload_types:?}"
        );
    }

    #[test]
    fn external_move_rejects_forged_player_source_container_and_coordinates_without_mutation() {
        for (label, source_container_id, source_row, source_col) in [
            ("container", "body_pocket", 0, 0),
            ("row", "main_pack", 1, 0),
            ("column", "main_pack", 0, 1),
        ] {
            let (app, player, coffin, payload_types) = run_player_to_external_move_case_with_source(
                source_container_id,
                source_row,
                source_col,
            );
            let inventory = app
                .world()
                .get::<PlayerInventory>(player)
                .expect("test player keeps inventory component");
            assert_eq!(
                inventory.revision,
                InventoryRevision(0),
                "forged player {label} source must not advance inventory revision"
            );
            assert!(
                inventory.containers.iter().any(|container| {
                    container.id == "main_pack"
                        && container.items.iter().any(|item| {
                            item.instance.instance_id == 7001 && item.row == 0 && item.col == 0
                        })
                }),
                "forged player {label} source must keep instance 7001 at its authoritative slot"
            );
            let ext = app
                .world()
                .get::<crate::inventory::external_container::ExternalContainer>(coffin)
                .expect("external container must remain attached after rejection");
            assert!(
                ext.container.items.is_empty(),
                "forged player {label} source must not move instance 7001 into external storage"
            );
            assert!(
                payload_types.iter().any(|ty| ty == "loot_container_update"),
                "forged player {label} source must resync external state; payloads={payload_types:?}"
            );
            assert!(
                payload_types.iter().any(|ty| ty == "inventory_snapshot"),
                "forged player {label} source must resync player state; payloads={payload_types:?}"
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn run_player_to_external_move_case_with_source(
        source_container_id: &str,
        source_row: u64,
        source_col: u64,
    ) -> (App, Entity, Entity, Vec<String>) {
        use crate::inventory::external_container::{ExternalContainer, ExternalContainerRegistry};

        const SESSION_ID: u64 = 77;
        const INSTANCE_ID: u64 = 7001;

        let mut app = App::new();
        register_request_app(&mut app);
        let (client_bundle, mut helper) = create_mock_client("Azure");
        let mut inventory = empty_inventory();
        inventory.containers[0].items.push(PlacedItemState {
            row: 0,
            col: 0,
            instance: inventory_test_item(INSTANCE_ID, "spiritual_ore", 1),
        });
        let player = app
            .world_mut()
            .spawn((
                client_bundle,
                inventory,
                Cultivation::default(),
                PlayerState::default(),
                Lifecycle::default(),
                CurrentDimension(DimensionKind::Overworld),
            ))
            .id();
        app.world_mut()
            .entity_mut(player)
            .insert(Position::new(DVec3::ZERO));
        let coffin = app
            .world_mut()
            .spawn((
                ExternalContainer {
                    session_id: SESSION_ID,
                    container: ContainerState {
                        id: ExternalContainer::container_id(SESSION_ID),
                        name: "external_test".to_string(),
                        rows: 3,
                        cols: 4,
                        items: Vec::new(),
                        owner_instance_id: None,
                        quick_access: false,
                    },
                    opened_by: Some(player),
                    timeout_wall_secs: u64::MAX,
                    source_kind:
                        crate::inventory::external_container::ExternalContainerKind::StorageCrate {
                            is_herb: false,
                        },
                },
                Position::new(DVec3::ZERO),
                CurrentDimension(DimensionKind::Overworld),
            ))
            .id();
        app.insert_resource(ExternalContainerRegistry {
            next_session_id: SESSION_ID + 1,
            sessions: [(SESSION_ID, coffin)].into_iter().collect(),
        });
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: player,
                channel: ident!("bong:client_request").into(),
                data: format!(
                    r#"{{"type":"external_container_move","v":1,"session_id":{SESSION_ID},"instance_id":{INSTANCE_ID},"from":{{"kind":"container","container_id":"{source_container_id}","row":{source_row},"col":{source_col}}},"to":{{"kind":"container","container_id":"ext_{SESSION_ID}","row":0,"col":0}}}}"#
                )
                .into_bytes()
                .into_boxed_slice(),
            });
        app.update();
        flush_all_client_packets(&mut app);
        let payload_types = collect_server_data_payload_types(&mut helper);
        (app, player, coffin, payload_types)
    }

    #[test]
    fn non_supply_external_container_move_keeps_existing_contract_after_live_gate() {
        let (app, player, coffin, _payload_types) = run_external_container_move_case(
            Some(DimensionKind::Overworld),
            DVec3::new(0.0, 64.0, 0.0),
            crate::inventory::external_container::ExternalContainerKind::StorageCrate {
                is_herb: false,
            },
            false,
            true,
            true,
        );

        let ext = app
            .world()
            .get::<crate::inventory::external_container::ExternalContainer>(coffin)
            .expect("storage crate remains after move");
        assert!(
            ext.container.items.is_empty(),
            "supply-coffin authority rules must not spill into storage-crate move handling"
        );
        let inventory = app.world().get::<PlayerInventory>(player).unwrap();
        assert!(
            inventory.containers.iter().any(|container| container
                .items
                .iter()
                .any(|item| item.instance.instance_id == 7001)),
            "storage-crate move contract must remain unchanged"
        );
    }

    #[test]
    fn external_session_zero_timeout_is_not_expired_but_finite_deadline_is_inclusive() {
        assert!(
            !external_session_is_expired(0, u64::MAX),
            "zero timeout is the established no-expiry value for generic external containers"
        );
        assert!(
            !external_session_is_expired(101, 100),
            "a finite external session remains live before its deadline"
        );
        assert!(
            external_session_is_expired(100, 100),
            "a finite external session expires exactly at its inclusive deadline"
        );
    }

    fn inventory_with_item(item: ItemInstance) -> PlayerInventory {
        PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(0),
            containers: vec![ContainerState {
                quick_access: false,
                id: "main_pack".into(),
                name: "main_pack".into(),
                rows: 5,
                cols: 7,
                items: vec![PlacedItemState {
                    row: 0,
                    col: 0,
                    instance: item,
                }],

                owner_instance_id: None,
            }],
            equipped: Default::default(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 50.0,
        }
    }

    fn flush_all_client_packets(app: &mut App) {
        let world = app.world_mut();
        let mut query = world.query::<&mut Client>();
        for mut client in query.iter_mut(world) {
            client
                .flush_packets()
                .expect("mock client packets should flush successfully");
        }
    }

    fn has_inventory_snapshot_payload(helper: &mut MockClientHelper) -> bool {
        for frame in helper.collect_received().0 {
            let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                continue;
            };
            if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                continue;
            }
            let Ok(value) = serde_json::from_slice::<serde_json::Value>(packet.data.0 .0) else {
                continue;
            };
            if value.get("type").and_then(|ty| ty.as_str()) == Some("inventory_snapshot") {
                return true;
            }
        }
        false
    }

    fn collect_alchemy_session_snapshots(
        helper: &mut MockClientHelper,
    ) -> Vec<crate::schema::alchemy::AlchemySessionDataV1> {
        helper
            .collect_received()
            .0
            .into_iter()
            .filter_map(|frame| {
                let packet = frame.decode::<CustomPayloadS2c>().ok()?;
                if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                    return None;
                }
                let payload = serde_json::from_slice::<ServerDataV1>(packet.data.0 .0).ok()?;
                match payload.payload {
                    ServerDataPayloadV1::AlchemySession(snapshot) => Some(*snapshot),
                    _ => None,
                }
            })
            .collect()
    }

    const ALCHEMY_SNAPSHOT_RECIPE_ID: &str = "handler_snapshot_contract";
    const ALCHEMY_SNAPSHOT_FURNACE_POS: (i32, i32, i32) = (2, 64, 3);
    const ALCHEMY_SNAPSHOT_MATERIAL: &str = "handler_snapshot_herb";

    fn alchemy_snapshot_recipe_registry() -> RecipeRegistry {
        let mut registry = RecipeRegistry::new();
        registry
            .insert(Recipe {
                id: ALCHEMY_SNAPSHOT_RECIPE_ID.into(),
                name: "handler snapshot contract".into(),
                furnace_tier_min: 1,
                stages: vec![
                    RecipeStage {
                        at_tick: 0,
                        required: vec![IngredientSpec {
                            material: ALCHEMY_SNAPSHOT_MATERIAL.into(),
                            count: 2,
                            mineral_id: None,
                        }],
                        window: 0,
                    },
                    RecipeStage {
                        at_tick: 12,
                        required: vec![],
                        window: 3,
                    },
                ],
                fire_profile: FireProfile {
                    target_temp: 0.67,
                    target_duration_ticks: 48,
                    qi_cost: 9.75,
                    tolerance: ToleranceSpec {
                        temp_band: 0.07,
                        duration_band: 5,
                    },
                },
                outcomes: Outcomes {
                    perfect: None,
                    good: None,
                    flawed: None,
                    waste: None,
                    explode: None,
                },
                flawed_fallback: None,
            })
            .expect("handler snapshot recipe fixture must have a unique id");
        registry
    }

    fn alchemy_snapshot_active_session(player_id: &str) -> AlchemySession {
        let mut session =
            AlchemySession::new(ALCHEMY_SNAPSHOT_RECIPE_ID.into(), player_id.to_string());
        session.temp_current = 0.61;
        session.qi_injected = 4.25;
        session
    }

    fn spawn_owned_alchemy_snapshot_furnace(
        app: &mut App,
        player_id: &str,
        session: Option<AlchemySession>,
    ) -> Entity {
        let mut furnace = AlchemyFurnace::placed(
            valence::prelude::BlockPos::new(
                ALCHEMY_SNAPSHOT_FURNACE_POS.0,
                ALCHEMY_SNAPSHOT_FURNACE_POS.1,
                ALCHEMY_SNAPSHOT_FURNACE_POS.2,
            ),
            1,
        );
        furnace.owner = Some(player_id.to_string());
        furnace.session = session;
        app.world_mut().spawn(furnace).id()
    }

    fn send_alchemy_snapshot_request(app: &mut App, client: Entity, body: serde_json::Value) {
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client,
                channel: ident!("bong:client_request").into(),
                data: body.to_string().into_bytes().into_boxed_slice(),
            });
    }

    fn run_alchemy_snapshot_request(
        app: &mut App,
        client: Entity,
        helper: &mut MockClientHelper,
        body: serde_json::Value,
    ) -> Vec<crate::schema::alchemy::AlchemySessionDataV1> {
        send_alchemy_snapshot_request(app, client, body);
        app.update();
        flush_all_client_packets(app);
        collect_alchemy_session_snapshots(helper)
    }

    fn assert_authoritative_alchemy_guidance(
        snapshot: &crate::schema::alchemy::AlchemySessionDataV1,
        stage_states: [(bool, bool); 2],
    ) {
        assert_eq!(
            snapshot.recipe_id.as_deref(),
            Some(ALCHEMY_SNAPSHOT_RECIPE_ID),
            "handler payload must identify the same recipe fixture that supplies its targets"
        );
        assert_eq!(
            snapshot.target_ticks, 48,
            "target duration must come from the authoritative RecipeRegistry fixture"
        );
        assert_eq!(
            snapshot.temp_target, 0.67,
            "target temperature must come from the authoritative RecipeRegistry fixture"
        );
        assert_eq!(
            snapshot.temp_band, 0.07,
            "temperature band must come from the authoritative RecipeRegistry fixture"
        );
        assert_eq!(
            snapshot.qi_target, 9.75,
            "qi target must come from the authoritative RecipeRegistry fixture"
        );
        assert_eq!(
            snapshot.stages,
            vec![
                crate::schema::alchemy::AlchemyStageHintV1 {
                    at_tick: 0,
                    window: 0,
                    summary: format!("{ALCHEMY_SNAPSHOT_MATERIAL}×2"),
                    completed: stage_states[0].0,
                    missed: stage_states[0].1,
                },
                crate::schema::alchemy::AlchemyStageHintV1 {
                    at_tick: 12,
                    window: 3,
                    summary: String::new(),
                    completed: stage_states[1].0,
                    missed: stage_states[1].1,
                },
            ],
            "handler payload must preserve declared stage order and an exact empty summary for required=[]"
        );
    }

    #[test]
    fn alchemy_open_furnace_repushes_authoritative_recipe_snapshot_over_wire() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(alchemy_snapshot_recipe_registry());
        let (client_bundle, mut helper) = create_mock_client("Azure");
        let client = app.world_mut().spawn(client_bundle).id();
        spawn_owned_alchemy_snapshot_furnace(
            &mut app,
            "offline:Azure",
            Some(alchemy_snapshot_active_session("offline:Azure")),
        );

        let snapshots = run_alchemy_snapshot_request(
            &mut app,
            client,
            &mut helper,
            serde_json::json!({
                "type": "alchemy_open_furnace",
                "v": 1,
                "furnace_pos": ALCHEMY_SNAPSHOT_FURNACE_POS,
            }),
        );

        assert_eq!(snapshots.len(), 1, "open must emit one session snapshot");
        assert!(
            snapshots[0].active,
            "open must expose the active furnace session"
        );
        assert_authoritative_alchemy_guidance(&snapshots[0], [(false, false), (false, false)]);
    }

    #[test]
    fn alchemy_ignite_repushes_authoritative_recipe_snapshot_over_wire() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(alchemy_snapshot_recipe_registry());
        let (client_bundle, mut helper) = create_mock_client("Azure");
        let client = app.world_mut().spawn(client_bundle).id();
        spawn_owned_alchemy_snapshot_furnace(&mut app, "offline:Azure", None);

        let snapshots = run_alchemy_snapshot_request(
            &mut app,
            client,
            &mut helper,
            serde_json::json!({
                "type": "alchemy_ignite",
                "v": 1,
                "furnace_pos": ALCHEMY_SNAPSHOT_FURNACE_POS,
                "recipe_id": ALCHEMY_SNAPSHOT_RECIPE_ID,
            }),
        );

        assert_eq!(snapshots.len(), 1, "ignite must emit one session snapshot");
        assert!(
            snapshots[0].active,
            "ignite must expose its newly active session"
        );
        assert_eq!(snapshots[0].elapsed_ticks, 0);
        assert_authoritative_alchemy_guidance(&snapshots[0], [(false, false), (false, false)]);
    }

    #[test]
    fn alchemy_intervention_repushes_authoritative_recipe_snapshot_over_wire() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(alchemy_snapshot_recipe_registry());
        let (client_bundle, mut helper) = create_mock_client("Azure");
        let client = app.world_mut().spawn(client_bundle).id();
        spawn_owned_alchemy_snapshot_furnace(
            &mut app,
            "offline:Azure",
            Some(alchemy_snapshot_active_session("offline:Azure")),
        );

        let snapshots = run_alchemy_snapshot_request(
            &mut app,
            client,
            &mut helper,
            serde_json::json!({
                "type": "alchemy_intervention",
                "v": 1,
                "furnace_pos": ALCHEMY_SNAPSHOT_FURNACE_POS,
                "intervention": {"kind": "adjust_temp", "temp": 0.73},
            }),
        );

        assert_eq!(
            snapshots.len(),
            1,
            "intervention must emit one session snapshot"
        );
        assert_eq!(snapshots[0].temp_current, 0.73);
        assert_eq!(
            snapshots[0].interventions_recent,
            vec!["§7AdjustTemp(0.73)"],
            "wire snapshot must expose the intervention applied by the production handler"
        );
        assert_authoritative_alchemy_guidance(&snapshots[0], [(false, false), (false, false)]);
    }

    #[test]
    fn alchemy_feed_repushes_completed_stage_with_authoritative_recipe_snapshot_over_wire() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(alchemy_snapshot_recipe_registry());
        let (client_bundle, mut helper) = create_mock_client("Azure");
        let client = app.world_mut().spawn(client_bundle).id();
        app.world_mut().entity_mut(client).insert((
            Cultivation::default(),
            PlayerState::default(),
            inventory_with_stack(ALCHEMY_SNAPSHOT_MATERIAL, 2),
        ));
        spawn_owned_alchemy_snapshot_furnace(
            &mut app,
            "offline:Azure",
            Some(alchemy_snapshot_active_session("offline:Azure")),
        );

        let snapshots = run_alchemy_snapshot_request(
            &mut app,
            client,
            &mut helper,
            serde_json::json!({
                "type": "alchemy_feed_slot",
                "v": 1,
                "furnace_pos": ALCHEMY_SNAPSHOT_FURNACE_POS,
                "slot_idx": 0,
                "material": ALCHEMY_SNAPSHOT_MATERIAL,
                "count": 2,
            }),
        );

        assert_eq!(
            snapshots.len(),
            1,
            "successful feed must emit one session snapshot"
        );
        assert_authoritative_alchemy_guidance(&snapshots[0], [(true, false), (false, false)]);
    }

    #[test]
    fn alchemy_take_back_repushes_finished_guidance_after_furnace_session_is_removed() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(alchemy_snapshot_recipe_registry());
        app.insert_resource(ItemRegistry::from_map(HashMap::from([(
            crate::alchemy::residue::FAILED_PILL_RESIDUE_TEMPLATE_ID.into(),
            ItemTemplate::minimal_for_test(
                crate::alchemy::residue::FAILED_PILL_RESIDUE_TEMPLATE_ID,
            ),
        )])));
        app.insert_resource(InventoryInstanceIdAllocator::default());
        let (client_bundle, mut helper) = create_mock_client("Azure");
        let client = app.world_mut().spawn(client_bundle).id();
        app.world_mut().entity_mut(client).insert((
            Cultivation::default(),
            PlayerState::default(),
            empty_inventory(),
        ));
        let mut session = alchemy_snapshot_active_session("offline:Azure");
        session.staged.completed_stages = vec![0, 1];
        session
            .staged
            .materials
            .insert(ALCHEMY_SNAPSHOT_MATERIAL.into(), 2);
        let furnace =
            spawn_owned_alchemy_snapshot_furnace(&mut app, "offline:Azure", Some(session));

        let snapshots = run_alchemy_snapshot_request(
            &mut app,
            client,
            &mut helper,
            serde_json::json!({
                "type": "alchemy_take_back",
                "v": 1,
                "furnace_pos": ALCHEMY_SNAPSHOT_FURNACE_POS,
                "slot_idx": 0,
            }),
        );

        assert_eq!(
            snapshots.len(),
            1,
            "successful take-back must emit one finished session snapshot rather than an empty-furnace snapshot"
        );
        assert!(!snapshots[0].active, "finished snapshot must be inactive");
        assert_eq!(snapshots[0].status_label, "已结束");
        assert_authoritative_alchemy_guidance(&snapshots[0], [(true, false), (true, false)]);
        assert!(
            app.world()
                .get::<AlchemyFurnace>(furnace)
                .is_some_and(|furnace| furnace.session.is_none()),
            "take-back must keep the furnace empty after sending guidance from the completed session"
        );
    }

    #[test]
    fn alchemy_take_back_missing_allocator_still_pushes_finished_session() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(alchemy_snapshot_recipe_registry());
        app.insert_resource(ItemRegistry::from_map(HashMap::from([(
            crate::alchemy::residue::FAILED_PILL_RESIDUE_TEMPLATE_ID.into(),
            ItemTemplate::minimal_for_test(
                crate::alchemy::residue::FAILED_PILL_RESIDUE_TEMPLATE_ID,
            ),
        )])));
        // 故意不插入 InventoryInstanceIdAllocator：覆盖 non-explode 缺编号器分支。
        let (client_bundle, mut helper) = create_mock_client("Azure");
        let client = app.world_mut().spawn(client_bundle).id();
        app.world_mut().entity_mut(client).insert((
            Cultivation::default(),
            PlayerState::default(),
            empty_inventory(),
        ));
        let mut session = alchemy_snapshot_active_session("offline:Azure");
        session.staged.completed_stages = vec![0, 1];
        session
            .staged
            .materials
            .insert(ALCHEMY_SNAPSHOT_MATERIAL.into(), 2);
        let furnace =
            spawn_owned_alchemy_snapshot_furnace(&mut app, "offline:Azure", Some(session));

        send_alchemy_snapshot_request(
            &mut app,
            client,
            serde_json::json!({
                "type": "alchemy_take_back",
                "v": 1,
                "furnace_pos": ALCHEMY_SNAPSHOT_FURNACE_POS,
                "slot_idx": 0,
            }),
        );
        app.update();
        flush_all_client_packets(&mut app);

        let frames = helper.collect_received().0;
        let messages: Vec<String> = frames
            .iter()
            .filter_map(|frame| {
                frame
                    .decode::<GameMessageS2c>()
                    .ok()
                    .map(|packet| packet.chat.to_legacy_lossy())
            })
            .collect();
        let snapshots: Vec<crate::schema::alchemy::AlchemySessionDataV1> = frames
            .iter()
            .filter_map(|frame| {
                let packet = frame.decode::<CustomPayloadS2c>().ok()?;
                if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                    return None;
                }
                let payload = serde_json::from_slice::<ServerDataV1>(packet.data.0 .0).ok()?;
                match payload.payload {
                    ServerDataPayloadV1::AlchemySession(snapshot) => Some(*snapshot),
                    _ => None,
                }
            })
            .collect();
        let furnace_payloads = frames
            .iter()
            .filter_map(|frame| {
                let packet = frame.decode::<CustomPayloadS2c>().ok()?;
                if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                    return None;
                }
                let payload = serde_json::from_slice::<ServerDataV1>(packet.data.0 .0).ok()?;
                match payload.payload {
                    ServerDataPayloadV1::AlchemyFurnace(data) => Some(*data),
                    _ => None,
                }
            })
            .collect::<Vec<_>>();

        assert!(
            messages
                .iter()
                .any(|message| message.contains("实例编号器未就绪")),
            "missing allocator must surface alchemy error chat, messages={messages:?}"
        );
        assert_eq!(
            snapshots.len(),
            1,
            "allocator missing must still emit exactly one finished session snapshot so HUD clears"
        );
        assert!(!snapshots[0].active, "finished snapshot must be inactive");
        assert_eq!(snapshots[0].status_label, "已结束");
        assert_authoritative_alchemy_guidance(&snapshots[0], [(true, false), (true, false)]);
        assert_eq!(
            furnace_payloads.len(),
            1,
            "allocator missing must still push empty-furnace authority once"
        );
        assert!(
            !furnace_payloads[0].has_session,
            "empty furnace payload must report has_session=false"
        );
        assert!(
            app.world()
                .get::<AlchemyFurnace>(furnace)
                .is_some_and(|furnace| furnace.session.is_none()),
            "session must remain ended even when reward grant is skipped"
        );
        assert!(
            app.world()
                .get::<PlayerInventory>(client)
                .is_some_and(|inventory| inventory
                    .containers
                    .iter()
                    .all(|container| container.items.is_empty())),
            "missing allocator must not invent reward items"
        );
        let outcome_events = app
            .world()
            .resource::<valence::prelude::Events<crate::alchemy::AlchemyOutcomeEvent>>();
        let mut reader = outcome_events.get_reader();
        assert!(
            reader.read(outcome_events).next().is_none(),
            "failed grant path must not emit AlchemyOutcomeEvent"
        );
    }

    #[test]
    fn alchemy_take_back_grant_failure_still_pushes_finished_session() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(alchemy_snapshot_recipe_registry());
        // 编号器就绪，但 registry 故意缺少 failed-pill 模板，强制 grant 失败。
        app.insert_resource(ItemRegistry::default());
        app.insert_resource(InventoryInstanceIdAllocator::default());
        let (client_bundle, mut helper) = create_mock_client("Azure");
        let client = app.world_mut().spawn(client_bundle).id();
        app.world_mut().entity_mut(client).insert((
            Cultivation::default(),
            PlayerState::default(),
            empty_inventory(),
        ));
        let mut session = alchemy_snapshot_active_session("offline:Azure");
        session.staged.completed_stages = vec![0, 1];
        session
            .staged
            .materials
            .insert(ALCHEMY_SNAPSHOT_MATERIAL.into(), 2);
        let furnace =
            spawn_owned_alchemy_snapshot_furnace(&mut app, "offline:Azure", Some(session));

        send_alchemy_snapshot_request(
            &mut app,
            client,
            serde_json::json!({
                "type": "alchemy_take_back",
                "v": 1,
                "furnace_pos": ALCHEMY_SNAPSHOT_FURNACE_POS,
                "slot_idx": 0,
            }),
        );
        app.update();
        flush_all_client_packets(&mut app);

        let frames = helper.collect_received().0;
        let messages: Vec<String> = frames
            .iter()
            .filter_map(|frame| {
                frame
                    .decode::<GameMessageS2c>()
                    .ok()
                    .map(|packet| packet.chat.to_legacy_lossy())
            })
            .collect();
        let snapshots: Vec<crate::schema::alchemy::AlchemySessionDataV1> = frames
            .iter()
            .filter_map(|frame| {
                let packet = frame.decode::<CustomPayloadS2c>().ok()?;
                if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                    return None;
                }
                let payload = serde_json::from_slice::<ServerDataV1>(packet.data.0 .0).ok()?;
                match payload.payload {
                    ServerDataPayloadV1::AlchemySession(snapshot) => Some(*snapshot),
                    _ => None,
                }
            })
            .collect();
        let furnace_payloads = frames
            .iter()
            .filter_map(|frame| {
                let packet = frame.decode::<CustomPayloadS2c>().ok()?;
                if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                    return None;
                }
                let payload = serde_json::from_slice::<ServerDataV1>(packet.data.0 .0).ok()?;
                match payload.payload {
                    ServerDataPayloadV1::AlchemyFurnace(data) => Some(*data),
                    _ => None,
                }
            })
            .collect::<Vec<_>>();

        assert!(
            messages.iter().any(|message| {
                message.contains("炼丹产物入袋失败")
                    && message.contains(crate::alchemy::residue::FAILED_PILL_RESIDUE_TEMPLATE_ID)
            }),
            "grant failure must surface alchemy error chat, messages={messages:?}"
        );
        assert_eq!(
            snapshots.len(),
            1,
            "grant failure must still emit exactly one finished session snapshot"
        );
        assert!(!snapshots[0].active, "finished snapshot must be inactive");
        assert_eq!(snapshots[0].status_label, "已结束");
        assert_authoritative_alchemy_guidance(&snapshots[0], [(true, false), (true, false)]);
        assert_eq!(
            furnace_payloads.len(),
            1,
            "grant failure must still push empty-furnace authority once"
        );
        assert!(
            !furnace_payloads[0].has_session,
            "empty furnace payload must report has_session=false"
        );
        assert!(
            app.world()
                .get::<AlchemyFurnace>(furnace)
                .is_some_and(|furnace| furnace.session.is_none()),
            "grant failure must not resurrect the ended furnace session"
        );
        assert!(
            app.world()
                .get::<PlayerInventory>(client)
                .is_some_and(|inventory| inventory
                    .containers
                    .iter()
                    .all(|container| container.items.is_empty())),
            "failed grant must not leave partial reward items"
        );
        let outcome_events = app
            .world()
            .resource::<valence::prelude::Events<crate::alchemy::AlchemyOutcomeEvent>>();
        let mut reader = outcome_events.get_reader();
        assert!(
            reader.read(outcome_events).next().is_none(),
            "failed grant path must not emit AlchemyOutcomeEvent"
        );
    }

    fn collect_skill_config_snapshots(
        helper: &mut MockClientHelper,
    ) -> Vec<crate::skill::config::SkillConfigSnapshot> {
        helper
            .collect_received()
            .0
            .into_iter()
            .filter_map(|frame| {
                let packet = frame.decode::<CustomPayloadS2c>().ok()?;
                if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                    return None;
                }
                let payload = serde_json::from_slice::<ServerDataV1>(packet.data.0 .0).ok()?;
                match payload.payload {
                    ServerDataPayloadV1::SkillConfigSnapshot(snapshot) => Some(snapshot),
                    _ => None,
                }
            })
            .collect()
    }

    fn collect_quickslot_configs(
        helper: &mut MockClientHelper,
    ) -> Vec<crate::schema::combat_hud::QuickSlotConfigV1> {
        helper
            .collect_received()
            .0
            .into_iter()
            .filter_map(|frame| {
                let packet = frame.decode::<CustomPayloadS2c>().ok()?;
                if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                    return None;
                }
                let payload = serde_json::from_slice::<ServerDataV1>(packet.data.0 .0).ok()?;
                match payload.payload {
                    ServerDataPayloadV1::QuickSlotConfig(config) => Some(config),
                    _ => None,
                }
            })
            .collect()
    }

    fn send_quick_slot_bind_request(
        app: &mut App,
        entity: Entity,
        slot: u8,
        item_id: Option<&str>,
        request_id: &str,
    ) {
        let body = serde_json::json!({
            "type": "quick_slot_bind",
            "v": 1,
            "slot": slot,
            "item_id": item_id,
            "request_id": request_id,
        });
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: body.to_string().into_bytes().into_boxed_slice(),
            });
    }

    fn collect_game_messages(helper: &mut MockClientHelper) -> Vec<String> {
        helper
            .collect_received()
            .0
            .into_iter()
            .filter_map(|frame| {
                frame
                    .decode::<GameMessageS2c>()
                    .ok()
                    .map(|packet| packet.chat.to_legacy_lossy())
            })
            .collect()
    }

    fn has_inventory_durability_payload(helper: &mut MockClientHelper, instance_id: u64) -> bool {
        for frame in helper.collect_received().0 {
            let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                continue;
            };
            if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                continue;
            }
            let Ok(value) = serde_json::from_slice::<serde_json::Value>(packet.data.0 .0) else {
                continue;
            };
            if value.get("type").and_then(|ty| ty.as_str()) != Some("inventory_event") {
                continue;
            }
            if value.get("kind").and_then(|kind| kind.as_str()) == Some("durability_changed")
                && value.get("instance_id").and_then(|id| id.as_u64()) == Some(instance_id)
            {
                return true;
            }
        }
        false
    }

    fn insert_test_forge_session(app: &mut App, session_id: u64, caster: Entity, step: ForgeStep) {
        let station = app.world_mut().spawn_empty().id();
        let mut sessions = ForgeSessions::new();
        let mut session = ForgeSession::new(
            ForgeSessionId(session_id),
            "qing_feng_v0".to_string(),
            station,
            caster,
        );
        session.current_step = step;
        session.step_state = match step {
            ForgeStep::Inscription => StepState::Inscription(Default::default()),
            ForgeStep::Tempering => StepState::Tempering(Default::default()),
            ForgeStep::Consecration => StepState::Consecration(Default::default()),
            ForgeStep::Billet => StepState::Billet(Default::default()),
            ForgeStep::Done => StepState::None,
        };
        sessions.insert(session);
        app.insert_resource(sessions);
    }

    /// C2S lingtian 测试的完整 payload 捕获：不只是 kind/pos，还要锁住
    /// actor 与 action 专属字段（hoe_instance_id / mode / plant_id / source），
    /// 让 validator→queue→handler 契约的任何字段丢失都撞红（fix-spec §9.4）。
    #[derive(Debug, PartialEq)]
    struct LingtianDispatchCapture {
        kind: &'static str,
        pos: BlockPos,
        player: Entity,
        hoe_instance_id: Option<u64>,
        mode: Option<SessionMode>,
        plant_id: Option<String>,
        source: Option<ReplenishSource>,
    }

    fn drain_lingtian_request_captures(app: &mut App) -> Vec<LingtianDispatchCapture> {
        let world = app.world_mut();
        let mut captured = Vec::new();
        captured.extend(
            world
                .resource_mut::<Events<StartTillRequest>>()
                .drain()
                .map(|event| LingtianDispatchCapture {
                    kind: "till",
                    pos: event.pos,
                    player: event.player,
                    hoe_instance_id: Some(event.hoe_instance_id),
                    mode: Some(event.mode),
                    plant_id: None,
                    source: None,
                }),
        );
        captured.extend(
            world
                .resource_mut::<Events<StartRenewRequest>>()
                .drain()
                .map(|event| LingtianDispatchCapture {
                    kind: "renew",
                    pos: event.pos,
                    player: event.player,
                    hoe_instance_id: Some(event.hoe_instance_id),
                    mode: None,
                    plant_id: None,
                    source: None,
                }),
        );
        captured.extend(
            world
                .resource_mut::<Events<StartPlantingRequest>>()
                .drain()
                .map(|event| LingtianDispatchCapture {
                    kind: "planting",
                    pos: event.pos,
                    player: event.player,
                    hoe_instance_id: None,
                    mode: None,
                    plant_id: Some(event.plant_id),
                    source: None,
                }),
        );
        captured.extend(
            world
                .resource_mut::<Events<StartHarvestRequest>>()
                .drain()
                .map(|event| LingtianDispatchCapture {
                    kind: "harvest",
                    pos: event.pos,
                    player: event.player,
                    hoe_instance_id: None,
                    mode: Some(event.mode),
                    plant_id: None,
                    source: None,
                }),
        );
        captured.extend(
            world
                .resource_mut::<Events<StartReplenishRequest>>()
                .drain()
                .map(|event| LingtianDispatchCapture {
                    kind: "replenish",
                    pos: event.pos,
                    player: event.player,
                    hoe_instance_id: None,
                    mode: None,
                    plant_id: None,
                    source: Some(event.source),
                }),
        );
        captured.extend(
            world
                .resource_mut::<Events<StartDrainQiRequest>>()
                .drain()
                .map(|event| LingtianDispatchCapture {
                    kind: "drain_qi",
                    pos: event.pos,
                    player: event.player,
                    hoe_instance_id: None,
                    mode: None,
                    plant_id: None,
                    source: None,
                }),
        );
        captured
    }

    fn run_lingtian_dispatch_case(
        payload: serde_json::Value,
        position: Option<DVec3>,
        dimension: Option<DimensionKind>,
    ) -> (Entity, Vec<LingtianDispatchCapture>) {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        let (client_bundle, _helper) = create_mock_client("LingtianDispatch");
        let client = app
            .world_mut()
            .spawn((client_bundle, Lifecycle::default()))
            .id();
        if let Some(position) = position {
            app.world_mut()
                .entity_mut(client)
                .insert(Position::new(position));
        }
        if let Some(dimension) = dimension {
            app.world_mut()
                .entity_mut(client)
                .insert(CurrentDimension(dimension));
        }
        // `LingtianStartTill` resolves its target from the authoritative plot
        // store before entering the pending queue.  Keep the shared matrix
        // helper's canonical target present so its boundary cases exercise
        // the reach/dimension checks rather than the missing-target branch.
        app.world_mut()
            .spawn(LingtianPlot::new(BlockPos::new(0, 64, 0), None));
        app.world_mut()
            .resource_mut::<Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client,
                channel: ident!("bong:client_request").into(),
                data: payload.to_string().into_bytes().into_boxed_slice(),
            });
        app.update();
        (client, drain_lingtian_request_captures(&mut app))
    }

    #[test]
    fn requester_gate_dimension_falls_back_to_entity_layer() {
        let overworld = Entity::from_raw(101);
        let tsy = Entity::from_raw(102);
        let layers = DimensionLayers { overworld, tsy };

        assert_eq!(
            dimension_for_target_layer(
                None,
                Some(&EntityLayerId(overworld)),
                Some(&layers),
            ),
            Some(DimensionKind::Overworld),
            "a live client without CurrentDimension must still resolve its authoritative overworld layer"
        );
        assert_eq!(
            dimension_for_target_layer(None, Some(&EntityLayerId(tsy)), Some(&layers)),
            Some(DimensionKind::Tsy),
            "a live client without CurrentDimension must still resolve its authoritative Tsy layer"
        );
        assert_eq!(
            dimension_for_target_layer(
                None,
                Some(&EntityLayerId(Entity::from_raw(103))),
                Some(&layers),
            ),
            None,
            "an unknown layer must fail closed instead of guessing a dimension"
        );
    }

    #[test]
    fn lingtian_plot_index_tracks_authoritative_positions() {
        let mut app = App::new();
        app.init_resource::<LingtianPlotIndex>();
        app.add_systems(Update, refresh_lingtian_plot_index);

        let first = BlockPos::new(-3, 64, 7);
        let second = BlockPos::new(9, 65, -11);
        let first_entity = app.world_mut().spawn(LingtianPlot::new(first, None)).id();
        app.world_mut().spawn(LingtianPlot::new(second, None));

        app.update();
        let index = app.world().resource::<LingtianPlotIndex>();
        assert!(
            index.contains(&first),
            "the refreshed index must admit the first authoritative plot position"
        );
        assert!(
            index.contains(&second),
            "the refreshed index must admit the second authoritative plot position"
        );

        app.world_mut().despawn(first_entity);
        app.update();
        assert!(
            !app.world().resource::<LingtianPlotIndex>().contains(&first),
            "despawned plots must disappear from the next ingress index snapshot"
        );
        assert!(
            app.world()
                .resource::<LingtianPlotIndex>()
                .contains(&second),
            "remaining plots must stay addressable after index refresh"
        );
    }

    #[test]
    fn lingtian_c2s_dispatch_applies_shared_position_and_dimension_gate_to_all_actions() {
        let target = BlockPos::new(0, 64, 0);
        let boundary = DVec3::new(5.0, 64.5, 0.5);
        let just_beyond = DVec3::new(5.000_001, 64.5, 0.5);
        let cases = [
            (
                "till",
                serde_json::json!({
                    "type": "lingtian_start_till", "v": 1, "x": 0, "y": 64, "z": 0,
                    "hoe_instance_id": 7, "mode": "manual"
                }),
            ),
            (
                "renew",
                serde_json::json!({
                    "type": "lingtian_start_renew", "v": 1, "x": 0, "y": 64, "z": 0,
                    "hoe_instance_id": 7
                }),
            ),
            (
                "planting",
                serde_json::json!({
                    "type": "lingtian_start_planting", "v": 1, "x": 0, "y": 64, "z": 0,
                    "plant_id": "ci_she_hao"
                }),
            ),
            (
                "harvest",
                serde_json::json!({
                    "type": "lingtian_start_harvest", "v": 1, "x": 0, "y": 64, "z": 0,
                    "mode": "manual"
                }),
            ),
            (
                "replenish",
                serde_json::json!({
                    "type": "lingtian_start_replenish", "v": 1, "x": 0, "y": 64, "z": 0,
                    "source": "bone_coin"
                }),
            ),
            (
                "drain_qi",
                serde_json::json!({
                    "type": "lingtian_start_drain_qi", "v": 1, "x": 0, "y": 64, "z": 0
                }),
            ),
        ];

        for (kind, payload) in cases {
            let (client, captures) = run_lingtian_dispatch_case(
                payload.clone(),
                Some(boundary),
                Some(DimensionKind::Overworld),
            );
            let expected = LingtianDispatchCapture {
                kind,
                pos: target,
                player: client,
                hoe_instance_id: (kind == "till" || kind == "renew").then_some(7),
                mode: (kind == "till" || kind == "harvest").then_some(SessionMode::Manual),
                plant_id: (kind == "planting").then(|| "ci_she_hao".to_string()),
                source: (kind == "replenish").then_some(ReplenishSource::BoneCoin),
            };
            assert_eq!(
                captures,
                vec![expected],
                "boundary Overworld {kind} request must preserve the full wire payload \
                 (actor, BlockPos, and action-specific fields) and dispatch exactly once"
            );
            for (label, position, dimension) in [
                (
                    "just beyond boundary",
                    Some(just_beyond),
                    Some(DimensionKind::Overworld),
                ),
                ("wrong dimension", Some(boundary), Some(DimensionKind::Tsy)),
                ("missing position", None, Some(DimensionKind::Overworld)),
                ("missing dimension", Some(boundary), None),
            ] {
                assert!(
                    run_lingtian_dispatch_case(payload.clone(), position, dimension)
                        .1
                        .is_empty(),
                    "{label} {kind} request must be rejected before ECS dispatch"
                );
            }
        }

        assert!(
            run_lingtian_dispatch_case(
                serde_json::json!({
                    "type": "lingtian_start_replenish", "v": 1,
                    "x": 0, "y": 64, "z": 0, "source": "unknown_source"
                }),
                Some(boundary),
                Some(DimensionKind::Overworld),
            )
            .1
            .is_empty(),
            "unknown replenish source must preserve its existing parse rejection"
        );
    }

    #[test]
    fn lingtian_start_till_missing_plot_is_rejected_before_pending_queue() {
        let mut app = App::new();
        register_request_app(&mut app);
        let (client_bundle, _helper) = create_mock_client("LingtianMissingTarget");
        let client = app
            .world_mut()
            .spawn((
                client_bundle,
                Lifecycle::default(),
                CurrentDimension(DimensionKind::Overworld),
            ))
            .id();
        app.world_mut()
            .entity_mut(client)
            .insert(Position::new(DVec3::new(0.5, 64.5, 0.5)));

        send_gate_test_payload(
            &mut app,
            client,
            serde_json::json!({
                "type": "lingtian_start_till",
                "v": 1,
                "x": 99,
                "y": 64,
                "z": 99,
                "hoe_instance_id": 7,
                "mode": "manual"
            }),
        );
        app.update();

        assert!(
            app.world()
                .resource::<crate::lingtian::requests::PendingLingtianRequests>()
                .is_empty(),
            "a missing authoritative plot must be rejected before the pending mutation queue"
        );
        assert!(
            app.world_mut()
                .resource_mut::<Events<StartTillRequest>>()
                .drain()
                .next()
                .is_none(),
            "a missing plot must not dispatch StartTillRequest"
        );
    }

    #[test]
    fn lingtian_start_till_accepts_authoritative_chunk_without_existing_plot() {
        let scenario = ScenarioSingleClient::new();
        let valence::testing::ScenarioSingleClient {
            mut app,
            client,
            layer,
            ..
        } = scenario;
        crate::world::dimension::mark_test_layer_as_overworld(&mut app);
        register_request_app(&mut app);

        let target = BlockPos::new(0, 64, 0);
        let mut chunk_layer = app
            .world_mut()
            .get_mut::<ChunkLayer>(layer)
            .expect("ScenarioSingleClient must provide the authoritative overworld layer");
        chunk_layer.insert_chunk([0, 0], UnloadedChunk::new());
        chunk_layer.set_block(target, BlockState::DIRT);

        app.world_mut().entity_mut(client).insert((
            Lifecycle::default(),
            CurrentDimension(DimensionKind::Overworld),
            Position::new(DVec3::new(0.5, 64.5, 0.5)),
        ));
        send_gate_test_payload(
            &mut app,
            client,
            serde_json::json!({
                "type": "lingtian_start_till",
                "v": 1,
                "x": target.x,
                "y": target.y,
                "z": target.z,
                "hoe_instance_id": 7,
                "mode": "manual"
            }),
        );
        app.update();

        assert_eq!(
            drain_lingtian_request_captures(&mut app),
            vec![LingtianDispatchCapture {
                kind: "till",
                pos: target,
                player: client,
                hoe_instance_id: Some(7),
                mode: Some(SessionMode::Manual),
                plant_id: None,
                source: None,
            }],
            "a loaded authoritative world block must admit till ingress even before a LingtianPlot exists"
        );
        assert!(
            app.world()
                .resource::<crate::lingtian::requests::PendingLingtianRequests>()
                .is_empty(),
            "an admitted till request must leave the ingress queue after the real validator dispatches it"
        );
    }

    /// #13 — network ingress 集成契约：真实 producer → 真实 queue → 真实
    /// validator 的多请求 wire FIFO。同 actor 一批三请求只 dispatch 第一条，
    /// 其余保序回到队列；逐 tick 推进后按 wire 顺序逐条 dispatch。
    #[test]
    fn lingtian_c2s_ingress_queue_preserves_wire_fifo_order() {
        let mut app = App::new();
        register_request_app(&mut app);
        let (client_bundle, _helper) = create_mock_client("LingtianFifo");
        let client = app
            .world_mut()
            .spawn((client_bundle, Lifecycle::default()))
            .id();
        app.world_mut()
            .entity_mut(client)
            .insert(Position::new(DVec3::new(5.0, 64.5, 0.5)));
        app.world_mut()
            .entity_mut(client)
            .insert(CurrentDimension(DimensionKind::Overworld));
        app.world_mut()
            .spawn(LingtianPlot::new(BlockPos::new(1, 64, 0), None));

        let send = |app: &mut App, payload: serde_json::Value| {
            app.world_mut()
                .resource_mut::<Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client,
                    channel: ident!("bong:client_request").into(),
                    data: payload.to_string().into_bytes().into_boxed_slice(),
                });
        };
        send(
            &mut app,
            serde_json::json!({
                "type": "lingtian_start_till", "v": 1, "x": 1, "y": 64, "z": 0,
                "hoe_instance_id": 7, "mode": "manual"
            }),
        );
        send(
            &mut app,
            serde_json::json!({
                "type": "lingtian_start_harvest", "v": 1, "x": 2, "y": 64, "z": 0,
                "mode": "manual"
            }),
        );
        send(
            &mut app,
            serde_json::json!({
                "type": "lingtian_start_planting", "v": 1, "x": 3, "y": 64, "z": 0,
                "plant_id": "ci_she_hao"
            }),
        );

        let remaining_positions = |app: &App| -> Vec<BlockPos> {
            app.world()
                .resource::<crate::lingtian::requests::PendingLingtianRequests>()
                .inbox
                .iter()
                .map(|request| request.actor_and_pos().1)
                .collect()
        };

        app.update();
        assert_eq!(
            remaining_positions(&app),
            vec![BlockPos::new(2, 64, 0), BlockPos::new(3, 64, 0)],
            "same-tick same-actor later requests must stay queued in wire order"
        );
        assert_eq!(
            drain_lingtian_request_captures(&mut app),
            vec![LingtianDispatchCapture {
                kind: "till",
                pos: BlockPos::new(1, 64, 0),
                player: client,
                hoe_instance_id: Some(7),
                mode: Some(SessionMode::Manual),
                plant_id: None,
                source: None,
            }],
            "first wire request dispatches first"
        );

        app.update();
        assert_eq!(
            remaining_positions(&app),
            vec![BlockPos::new(3, 64, 0)],
            "second update must advance to the second wire request only"
        );
        assert_eq!(
            drain_lingtian_request_captures(&mut app)
                .iter()
                .map(|capture| capture.kind)
                .collect::<Vec<_>>(),
            vec!["harvest"],
            "second wire request dispatches second"
        );

        app.update();
        assert!(
            remaining_positions(&app).is_empty(),
            "third update must drain the final wire request"
        );
        assert_eq!(
            drain_lingtian_request_captures(&mut app)
                .iter()
                .map(|capture| capture.kind)
                .collect::<Vec<_>>(),
            vec!["planting"],
            "third wire request dispatches last"
        );
    }

    /// #16 — 生产装配回归：`LingtianRequestIngressSet` 的排序边是 producer 先于
    /// validator 的唯一机制。validator 先注册、producer 后注册（反插入序）时，
    /// 删除 `network/mod.rs` 里 producer 的 `.in_set(...)` 会让本测试撞红
    /// （请求停留在持久队列、本 tick 无 dispatch）。
    #[test]
    fn production_ingress_wiring_orders_producer_before_validator() {
        let mut app = App::new();
        register_request_resources(&mut app);
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        app.add_systems(
            Update,
            crate::lingtian::systems::validate_and_dispatch_lingtian_requests
                .after(crate::lingtian::LingtianRequestIngressSet),
        );
        crate::network::register_lingtian_ingress_wiring(&mut app);
        app.add_systems(
            Update,
            crate::alchemy::apply_alchemy_explode_outcomes.after(handle_client_request_payloads),
        );

        let (client_bundle, _helper) = create_mock_client("IngressWiring");
        let client = app
            .world_mut()
            .spawn((client_bundle, Lifecycle::default()))
            .id();
        app.world_mut()
            .entity_mut(client)
            .insert(Position::new(DVec3::new(0.5, 64.5, 0.5)));
        app.world_mut()
            .entity_mut(client)
            .insert(CurrentDimension(DimensionKind::Overworld));
        app.world_mut()
            .spawn(LingtianPlot::new(BlockPos::new(0, 64, 0), None));
        app.world_mut()
            .resource_mut::<Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client,
                channel: ident!("bong:client_request").into(),
                data: serde_json::json!({
                    "type": "lingtian_start_till", "v": 1, "x": 0, "y": 64, "z": 0,
                    "hoe_instance_id": 7, "mode": "manual"
                })
                .to_string()
                .into_bytes()
                .into_boxed_slice(),
            });

        app.update();

        assert_eq!(
            drain_lingtian_request_captures(&mut app),
            vec![LingtianDispatchCapture {
                kind: "till",
                pos: BlockPos::new(0, 64, 0),
                player: client,
                hoe_instance_id: Some(7),
                mode: Some(SessionMode::Manual),
                plant_id: None,
                source: None,
            }],
            "production ingress wiring must dispatch the wire request in the same tick"
        );
        assert!(
            app.world()
                .resource::<crate::lingtian::requests::PendingLingtianRequests>()
                .is_empty(),
            "dispatched request must leave the persistent ingress queue"
        );
    }

    fn send_gate_test_payload(app: &mut App, client: Entity, payload: serde_json::Value) {
        app.world_mut()
            .resource_mut::<Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client,
                channel: ident!("bong:client_request").into(),
                data: payload.to_string().into_bytes().into_boxed_slice(),
            });
    }

    #[test]
    fn c2s_ingress_budget_drops_the_thirty_third_payload_before_decode() {
        let mut app = App::new();
        register_request_app(&mut app);
        let (client_bundle, mut helper) = create_mock_client("BudgetIngress");
        let client = app.world_mut().spawn(client_bundle).id();

        for _ in 0..33 {
            app.world_mut()
                .resource_mut::<Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client,
                    channel: ident!("bong:client_request").into(),
                    data: vec![0xff].into_boxed_slice(),
                });
        }

        app.update();

        assert_eq!(
            app.world()
                .resource::<ClientRequestBudget>()
                .store
                .tokens_for(&client),
            Some(0),
            "the 33rd same-tick payload must be refused by ingress after 32 admissions"
        );
        flush_all_client_packets(&mut app);
        let payload_types = collect_server_data_payload_types(&mut helper);
        assert_eq!(
            payload_types,
            vec!["event_alert"],
            "only the budgeted rate-limit feedback may be emitted; malformed payload #33 must not be decoded"
        );
    }

    #[test]
    fn craft_start_live_gate_rejects_missing_inventory_without_emitting_intent() {
        let mut app = App::new();
        register_request_app(&mut app);
        let (client_bundle, _helper) = create_mock_client("CraftGate");
        let client = app
            .world_mut()
            .spawn((
                client_bundle,
                empty_inventory(),
                Lifecycle::default(),
                CurrentDimension(DimensionKind::Overworld),
            ))
            .id();
        app.world_mut()
            .entity_mut(client)
            .insert(Position::new(DVec3::new(0.5, 64.5, 0.5)));

        send_gate_test_payload(
            &mut app,
            client,
            serde_json::json!({
                "type": "craft_start",
                "v": 1,
                "recipe_id": "craft.example.herb_knife.iron",
                "quantity": 1
            }),
        );
        app.update();
        let accepted = app
            .world_mut()
            .resource_mut::<Events<crate::craft::CraftStartIntent>>()
            .drain()
            .collect::<Vec<_>>();
        assert_eq!(
            accepted.len(),
            1,
            "valid craft ingress must emit exactly one intent"
        );

        app.world_mut()
            .entity_mut(client)
            .remove::<PlayerInventory>();
        send_gate_test_payload(
            &mut app,
            client,
            serde_json::json!({
                "type": "craft_start",
                "v": 1,
                "recipe_id": "craft.example.herb_knife.iron",
                "quantity": 1
            }),
        );
        app.update();
        assert!(
            app.world_mut()
                .resource_mut::<Events<crate::craft::CraftStartIntent>>()
                .drain()
                .next()
                .is_none(),
            "missing inventory must be rejected before CraftStartIntent and therefore before mutation"
        );
    }

    #[test]
    fn workbench_open_live_gate_dispatches_only_a_resolved_nearby_workbench() {
        let mut app = App::new();
        app.add_plugins(EntityPlugin);
        register_request_app(&mut app);
        let (client_bundle, _helper) = create_mock_client("WorkbenchGate");
        let client = app
            .world_mut()
            .spawn((
                client_bundle,
                Lifecycle::default(),
                CurrentDimension(DimensionKind::Overworld),
            ))
            .id();
        app.world_mut()
            .entity_mut(client)
            .insert(Position::new(DVec3::new(0.5, 64.5, 0.5)));
        let workbench = app
            .world_mut()
            .spawn((
                crate::world::entity_model::WORKBENCH_ENTITY_KIND,
                EntityId::default(),
                Position::new(DVec3::new(0.5, 64.0, 0.5)),
                OldPosition::new(DVec3::new(0.5, 64.0, 0.5)),
                CurrentDimension(DimensionKind::Overworld),
                WorkbenchBlock {
                    placed_by: client,
                    placed_at_tick: 0,
                },
            ))
            .id();
        app.update();
        let entity_id = app
            .world()
            .get::<EntityId>(workbench)
            .expect("EntityPlugin must assign the workbench protocol id")
            .get();

        send_gate_test_payload(
            &mut app,
            client,
            serde_json::json!({ "type": "workbench_open", "v": 1, "entity_id": entity_id }),
        );
        app.update();
        let requests = app
            .world_mut()
            .resource_mut::<Events<crate::craft::WorkbenchOpenRequest>>()
            .drain()
            .collect::<Vec<_>>();
        assert_eq!(
            requests.len(),
            1,
            "nearby workbench must reach its open consumer"
        );
        assert_eq!(requests[0].workbench, workbench);

        app.world_mut()
            .entity_mut(client)
            .insert(Position::new(DVec3::new(4.000_001, 64.5, 0.5)));
        send_gate_test_payload(
            &mut app,
            client,
            serde_json::json!({ "type": "workbench_open", "v": 1, "entity_id": entity_id }),
        );
        app.update();
        assert!(
            app.world_mut()
                .resource_mut::<Events<crate::craft::WorkbenchOpenRequest>>()
                .drain()
                .next()
                .is_none(),
            "out-of-reach workbench must be rejected before the open event"
        );

        app.world_mut()
            .entity_mut(client)
            .insert(Position::new(DVec3::new(0.5, 64.5, 0.5)));
        app.world_mut()
            .entity_mut(workbench)
            .insert(CurrentDimension(DimensionKind::Tsy));
        send_gate_test_payload(
            &mut app,
            client,
            serde_json::json!({ "type": "workbench_open", "v": 1, "entity_id": entity_id }),
        );
        app.update();
        assert!(
            app.world_mut()
                .resource_mut::<Events<crate::craft::WorkbenchOpenRequest>>()
                .drain()
                .next()
                .is_none(),
            "cross-dimension workbench must be rejected before the open event"
        );

        app.world_mut()
            .entity_mut(workbench)
            .insert(CurrentDimension(DimensionKind::Overworld));
        app.world_mut()
            .entity_mut(workbench)
            .remove::<WorkbenchBlock>();
        send_gate_test_payload(
            &mut app,
            client,
            serde_json::json!({ "type": "workbench_open", "v": 1, "entity_id": entity_id }),
        );
        app.update();
        assert!(
            app.world_mut()
                .resource_mut::<Events<crate::craft::WorkbenchOpenRequest>>()
                .drain()
                .next()
                .is_none(),
            "a target without WorkbenchBlock must be rejected before the open event"
        );

        send_gate_test_payload(
            &mut app,
            client,
            serde_json::json!({ "type": "workbench_open", "v": 1, "entity_id": entity_id + 999 }),
        );
        app.update();
        assert!(
            app.world_mut()
                .resource_mut::<Events<crate::craft::WorkbenchOpenRequest>>()
                .drain()
                .next()
                .is_none(),
            "an unresolved workbench entity id must be rejected before the open event"
        );
    }

    #[test]
    fn give_dan_live_gate_preserves_inventory_when_elder_state_is_invalid() {
        let mut app = App::new();
        app.add_plugins(EntityPlugin);
        register_request_app(&mut app);
        let (client_bundle, _helper) = create_mock_client("ElderGate");
        let client = app
            .world_mut()
            .spawn((
                client_bundle,
                inventory_with_stack("huiyuan_pill", 1),
                Lifecycle::default(),
                CurrentDimension(DimensionKind::Overworld),
            ))
            .id();
        app.world_mut()
            .entity_mut(client)
            .insert(Position::new(DVec3::new(0.5, 64.5, 0.5)));
        let elder = app
            .world_mut()
            .spawn((
                EntityKind::new(164),
                EntityId::default(),
                crate::npc::lifecycle::NpcArchetype::DyingElder,
                crate::npc::spawn::NpcMarker,
                Position::new(DVec3::new(0.5, 64.0, 0.5)),
                OldPosition::new(DVec3::new(0.5, 64.0, 0.5)),
                CurrentDimension(DimensionKind::Overworld),
                DyingElderState::Plea,
            ))
            .id();
        app.update();
        let elder_id = app
            .world()
            .get::<EntityId>(elder)
            .expect("EntityPlugin must assign the elder protocol id")
            .get();

        send_gate_test_payload(
            &mut app,
            client,
            serde_json::json!({
                "type": "give_dan_to_elder",
                "v": 1,
                "pill_instance_id": 9001,
                "elder_entity_id": elder_id
            }),
        );
        app.update();
        let accepted = app
            .world_mut()
            .resource_mut::<Events<crate::fauna::dying_elder::GiveDanToElderIntent>>()
            .drain()
            .collect::<Vec<_>>();
        assert_eq!(
            accepted.len(),
            1,
            "Plea elder must accept a live give-dan intent"
        );
        let revision_before = app.world().get::<PlayerInventory>(client).unwrap().revision;

        app.world_mut()
            .entity_mut(elder)
            .insert(DyingElderState::Dead {
                dead_by_betrayal: false,
            });
        send_gate_test_payload(
            &mut app,
            client,
            serde_json::json!({
                "type": "give_dan_to_elder",
                "v": 1,
                "pill_instance_id": 9001,
                "elder_entity_id": elder_id
            }),
        );
        app.update();
        assert!(
            app.world_mut()
                .resource_mut::<Events<crate::fauna::dying_elder::GiveDanToElderIntent>>()
                .drain()
                .next()
                .is_none(),
            "dead elder must be rejected before the give-dan mutation path"
        );
        assert_eq!(
            app.world().get::<PlayerInventory>(client).unwrap().revision,
            revision_before,
            "gate rejection must leave the pill inventory revision unchanged"
        );

        app.world_mut()
            .entity_mut(elder)
            .insert(DyingElderState::Plea);
        app.world_mut()
            .entity_mut(elder)
            .insert(CurrentDimension(DimensionKind::Tsy));
        send_gate_test_payload(
            &mut app,
            client,
            serde_json::json!({
                "type": "give_dan_to_elder",
                "v": 1,
                "pill_instance_id": 9001,
                "elder_entity_id": elder_id
            }),
        );
        app.update();
        assert!(
            app.world_mut()
                .resource_mut::<Events<crate::fauna::dying_elder::GiveDanToElderIntent>>()
                .drain()
                .next()
                .is_none(),
            "cross-dimension elder must be rejected before the give-dan intent"
        );

        app.world_mut()
            .entity_mut(elder)
            .insert(CurrentDimension(DimensionKind::Overworld))
            .insert(Position::new(DVec3::new(100.0, 64.0, 100.0)));
        send_gate_test_payload(
            &mut app,
            client,
            serde_json::json!({
                "type": "give_dan_to_elder",
                "v": 1,
                "pill_instance_id": 9001,
                "elder_entity_id": elder_id
            }),
        );
        app.update();
        assert!(
            app.world_mut()
                .resource_mut::<Events<crate::fauna::dying_elder::GiveDanToElderIntent>>()
                .drain()
                .next()
                .is_none(),
            "out-of-reach elder must be rejected before the give-dan intent"
        );

        app.world_mut()
            .entity_mut(elder)
            .insert(Position::new(DVec3::new(0.5, 64.0, 0.5)));
        send_gate_test_payload(
            &mut app,
            client,
            serde_json::json!({
                "type": "give_dan_to_elder",
                "v": 1,
                "pill_instance_id": 9001,
                "elder_entity_id": elder_id + 999
            }),
        );
        app.update();
        assert!(
            app.world_mut()
                .resource_mut::<Events<crate::fauna::dying_elder::GiveDanToElderIntent>>()
                .drain()
                .next()
                .is_none(),
            "an unresolved elder entity id must be rejected before the give-dan intent"
        );
        assert_eq!(
            app.world().get::<PlayerInventory>(client).unwrap().revision,
            revision_before,
            "all live gate denials must preserve the pill inventory revision"
        );
    }

    fn register_request_resources(app: &mut App) {
        app.insert_resource(CombatClock::default());
        app.init_resource::<ClientRequestBudget>();
        app.insert_resource(crate::cultivation::skill_registry::init_registry());
        app.insert_resource(TechniqueRegistry::load_for_tests());
        // plan-bug-qc-p1 §skill-cast P0：经脉依赖表（测试场景 default 空，各测可再声明）
        app.insert_resource(SkillMeridianDependencies::default());
        app.insert_resource(GameplayActionQueue::default());
        app.insert_resource(AlchemyMockState::default());
        app.insert_resource(DroppedLootRegistry::default());
        // plan-remains-suite P0 — DroppedLootRequestParams 新增 EventWriter<RemainsLootIntent>。
        app.add_event::<crate::inventory::RemainsLootIntent>();
        app.insert_resource(ItemRegistry::default());
        app.insert_resource(RecipeRegistry::default());
        app.insert_resource(ZoneRegistry::fallback());
        app.init_resource::<SkillConfigStore>();
        app.insert_resource(SkillConfigSchemas::default());
        app.add_event::<CustomPayloadEvent>();
        app.add_event::<crate::combat::events::AttackIntent>();
        app.add_event::<crate::cultivation::burst_meridian::BurstMeridianEvent>();
        app.add_event::<crate::network::vfx_event_emit::VfxEventRequest>();
        app.add_event::<crate::network::audio_event_emit::PlaySoundRecipeRequest>();
        app.add_event::<BreakthroughRequest>();
        app.add_event::<ForgeRequest>();
        app.add_event::<InsightChosen>();
        app.add_event::<DefenseIntent>();
        app.add_event::<RevivalActionIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<FalseSkinForgeRequest>();
        app.add_event::<PlaceFurnaceRequest>();
        app.add_event::<crate::alchemy::LearnRecipeFragmentIntent>();
        app.add_event::<SpiritNichePlaceRequest>();
        app.add_event::<SpiritNicheRepairRequest>();
        app.add_event::<SpiritNicheCoordinateRevealRequest>();
        app.add_event::<CoffinOpenRequest>();
        app.add_event::<crate::coffin::CoffinBreakRequest>();
        app.add_event::<crate::coffin::CoffinMenuReclaimRequest>();
        app.add_event::<crate::craft::CraftStartIntent>();
        app.add_event::<crate::fauna::dying_elder::GiveDanToElderIntent>();
        app.add_event::<crate::craft::WorkbenchOpenRequest>();
        app.add_event::<crate::world::container_open::ContainerOpenRequest>();
        app.add_event::<StartTillRequest>();
        app.add_event::<StartRenewRequest>();
        app.add_event::<StartPlantingRequest>();
        app.add_event::<StartHarvestRequest>();
        app.add_event::<StartReplenishRequest>();
        app.add_event::<StartDrainQiRequest>();
        app.add_event::<StartExtractRequestEvent>();
        app.add_event::<CancelExtractRequestEvent>();
        app.add_event::<QiColorInspectRequest>();
        app.add_event::<MineralProbeIntent>();
        app.add_event::<FreshnessProbeIntent>();
        app.add_event::<SkillXpGain>();
        app.add_event::<SkillScrollUsed>();
        app.add_event::<BlockPlaceRequest>();
        app.add_event::<ZhenfaPlaceRequest>();
        app.add_event::<ZhenfaTriggerRequest>();
        app.add_event::<ZhenfaDisarmRequest>();
        app.add_event::<ScatterBeadUseRequest>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_event::<crate::alchemy::AlchemyOutcomeEvent>();
        app.add_event::<crate::combat::events::CombatEvent>();
        app.add_event::<crate::combat::events::DeathEvent>();
        app.add_event::<crate::combat::zhenmai_v2::LocalNeutralizeEvent>();
        app.add_event::<crate::combat::zhenmai_v2::MultiPointBackfireEvent>();
        app.add_event::<crate::combat::zhenmai_v2::MeridianHardenEvent>();
        app.add_event::<crate::combat::zhenmai_v2::MeridianSeveredVoluntaryEvent>();
        app.add_event::<crate::combat::zhenmai_v2::BackfireAmplificationActiveEvent>();
        app.add_event::<crate::cultivation::meridian::severed::MeridianSeveredEvent>();
        app.add_event::<crate::cultivation::overload::MeridianOverloadEvent>();
        // plan-shield-block-v1 P1 — 举盾 intent events（ClientRequestDispatchParams 需要）。
        app.add_event::<crate::combat::shield_block::RaiseShieldIntent>();
        app.add_event::<crate::combat::shield_block::LowerShieldIntent>();
        // plan-agent-ui-data-v1 P0 — 天道 UI 响应 event（ClientRequestDispatchParams 需要）。
        app.add_event::<crate::network::agent_ui::AgentUiResponseEvent>();
        // plan-worldgen-v4 P5 §8.1#5 — dev give-block intent（ClientRequestDispatchParams 需要）。
        app.add_event::<crate::cmd::dev::block_picker::BlockPickerGiveIntent>();
    }

    /// 生产装配：producer 经 `LingtianRequestIngressSet`（与
    /// `network::register_app_wiring` 同路径），validator 排在该 set 之后
    /// （与 `lingtian::register` 的 chain 同合同）。测试删掉 set membership
    /// 会直接破坏这里的排序边（见 `production_ingress_wiring_orders_*`）。
    fn register_request_systems(app: &mut App) {
        crate::network::register_lingtian_ingress_wiring(app);
        app.add_systems(
            Update,
            crate::network::inventory_event_emit::emit_durability_changed_inventory_events
                // 原 test 装配对 producer 与 emitter 用了 `.chain()`：inventory move
                // 的 durability payload 必须同帧发出（`inventory_move_applies_*` 单
                // update + flush 断言）。拆生产装配后 chain 没了，改挂 set 后置边保
                // 持同帧语义——生产路径不依赖此边（每帧全扫，晚一帧无害）。
                .after(crate::lingtian::LingtianRequestIngressSet),
        );
        // fix-spec-1901-v2 §9.2 — exercise the real C2S ingress queue followed by
        // the single post-transfer validator, rather than treating Start* events
        // emitted by the producer as the security boundary.
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        app.add_systems(
            Update,
            crate::lingtian::systems::validate_and_dispatch_lingtian_requests
                .after(crate::lingtian::LingtianRequestIngressSet),
        );
        app.add_systems(
            Update,
            crate::alchemy::apply_alchemy_explode_outcomes.after(handle_client_request_payloads),
        );
    }

    fn register_request_app(app: &mut App) {
        register_request_resources(app);
        register_request_systems(app);
    }

    fn upsert_test_harvest_session(
        app: &mut App,
        player_id: &str,
        client_entity: Entity,
        mode: BotanyHarvestMode,
        started_at_tick: u64,
        last_progress: f32,
    ) -> Entity {
        let plant = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<HarvestSessionStore>()
            .upsert_session(HarvestSession {
                player_id: player_id.to_string(),
                client_entity,
                target_entity: Some(plant),
                target_plant: BotanyPlantId::CiSheHao,
                mode,
                started_at_tick,
                duration_ticks: harvest_duration_ticks_for(mode),
                phase: BotanyPhase::InProgress,
                last_progress,
                origin_position: [1.0, 64.0, 1.0],
            });
        plant
    }

    fn send_botany_harvest_request(app: &mut App, client: Entity, session_id: &str, mode: &str) {
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client,
                channel: ident!("bong:client_request").into(),
                data: format!(
                    r#"{{"type":"botany_harvest_request","v":1,"session_id":"{session_id}","mode":"{mode}"}}"#
                )
                .into_bytes()
                .into_boxed_slice(),
            });
    }

    fn neutral_faction_membership() -> FactionMembership {
        FactionMembership {
            faction_id: FactionId::Neutral,
            rank: FactionRank::Disciple,
            reputation: Reputation::default(),
            lineage: None,
            mission_queue: MissionQueue::default(),
        }
    }

    // ══════════════════════════════════════════════════════════════════════════
    // plan-race-system-v1 P4 opus verifier MINOR —— 装备门 `form_race_id` pin
    // 测试。镜像已锁的 emit 路径测试
    // （`cultivation_detail_emit::morph_state_present_overrides_form_race_id_away_from_intrinsic_race`）：
    // `handle_inventory_move` 内 `form_race_id` 的推导（§13303 附近）此前只有 emit
    // 侧的回归 pin，装备门（`InventoryMoveIntent` → `handle_inventory_move` →
    // `apply_inventory_move_with_race`）这条真正决定"能不能穿"的路径完全没有端到端
    // 测试锁住"用 Form 身份而不是本体 intrinsic 身份"这条契约。走真实
    // `ClientRequestV1::InventoryMoveIntent` C2S 事件 → `handle_client_request_payloads`
    // 全链路。
    // ══════════════════════════════════════════════════════════════════════════

    fn make_armor_straw_chestplate_registry(allowed_race: &str) -> ItemRegistry {
        ItemRegistry::from_map(HashMap::from([(
            "armor_straw_chestplate".to_string(),
            ItemTemplate {
                id: "armor_straw_chestplate".to_string(),
                display_name: "species-gated chestplate".to_string(),
                category: ItemCategory::Armor,
                placeable: None,
                max_stack_count: 1,
                grid_w: 1,
                grid_h: 1,
                base_weight: 1.0,
                rarity: ItemRarity::Common,
                spirit_quality_initial: 0.0,
                description: "test".to_string(),
                effect: None,
                cast_duration_ms: crate::inventory::DEFAULT_CAST_DURATION_MS,
                cooldown_ms: crate::inventory::DEFAULT_COOLDOWN_MS,
                weapon_spec: None,
                forge_station_spec: None,
                blueprint_scroll_spec: None,
                inscription_scroll_spec: None,
                technique_scroll_spec: None,
                readable_scroll_spec: None,
                recipe_fragment_spec: None,
                container_spec: None,
                shield_spec: None,
                shelflife_profile: None,
                shelflife_track: None,
                wearer_race: crate::body_plan::types::RaceGateOwned::Species {
                    species: vec![crate::body_plan::RaceId::new(allowed_race)],
                },
            },
        )]))
    }

    fn spawn_player_with_armor_straw_chestplate_in_pack(
        app: &mut App,
        username: &str,
        intrinsic_race: &str,
    ) -> Entity {
        let (client_bundle, _helper) = create_mock_client(username);
        let item = ItemInstance {
            instance_id: 1,
            template_id: "armor_straw_chestplate".to_string(),
            display_name: "species-gated chestplate".to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 1.0,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 0.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        };
        let inventory = PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(0),
            containers: vec![ContainerState {
                quick_access: false,
                id: crate::inventory::MAIN_PACK_CONTAINER_ID.to_string(),
                name: "主背包".to_string(),
                rows: 4,
                cols: 4,
                items: vec![PlacedItemState {
                    row: 0,
                    col: 0,
                    instance: item,
                }],
                owner_instance_id: None,
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 99.0,
        };
        app.world_mut()
            .spawn((
                client_bundle,
                inventory,
                Cultivation {
                    race: crate::body_plan::RaceId::new(intrinsic_race),
                    ..Cultivation::default()
                },
                PlayerState {
                    karma: 0.0,
                    inventory_score: 0.0,
                },
            ))
            .id()
    }

    fn send_species_gated_equip_intent(app: &mut App, client: Entity) {
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client,
                channel: ident!("bong:client_request").into(),
                data: serde_json::to_vec(&ClientRequestV1::InventoryMoveIntent {
                    v: 1,
                    instance_id: 1,
                    from: InventoryLocationV1::Container {
                        container_id: crate::inventory::MAIN_PACK_CONTAINER_ID.to_string(),
                        row: 0,
                        col: 0,
                    },
                    to: InventoryLocationV1::Equip {
                        slot: EquipSlotV1::Chest,
                        state: EquipStateV1::Worn,
                    },
                    rotated: false,
                })
                .expect("InventoryMoveIntent must serialize")
                .into_boxed_slice(),
            });
        app.update();
    }

    #[test]
    fn morphed_player_equip_gate_uses_form_race_not_intrinsic_race() {
        // 本体（intrinsic）种族是 whale，但已易形为 human——胸甲只认 human，装备门
        // 判定必须用 MorphState.form="human"（放行），而不是冒用本体 Cultivation.race
        // ="whale"（会误拒）。
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        app.add_plugins(EntityPlugin);
        register_request_app(&mut app);
        app.insert_resource(make_armor_straw_chestplate_registry(
            crate::body_plan::HUMAN_RACE_ID,
        ));

        let client = spawn_player_with_armor_straw_chestplate_in_pack(&mut app, "Morpher", "whale");
        app.world_mut()
            .entity_mut(client)
            .insert(crate::body_plan::MorphState::new(
                crate::body_plan::RaceId::new(crate::body_plan::HUMAN_RACE_ID),
                0,
                0,
            ));

        send_species_gated_equip_intent(&mut app, client);

        let inventory = app.world().entity(client).get::<PlayerInventory>().unwrap();
        let equipped_chest = inventory
            .equipped
            .get(crate::inventory::EQUIP_SLOT_CHEST)
            .and_then(|contents| contents.worn.first());
        assert_eq!(
            equipped_chest.map(|item| item.instance_id),
            Some(1),
            "已易形为 human 的 whale 本体应能穿上 Species([human]) 门的胸甲——装备门必须\
             用 MorphState.form 而不是继续冒用本体 intrinsic race，实测装备槽：{:?}",
            inventory.equipped.get(crate::inventory::EQUIP_SLOT_CHEST)
        );
    }

    #[test]
    fn unmorphed_whale_intrinsic_is_rejected_by_same_species_gate() {
        // 对照组：同一件甲、同一本体，缺 MorphState（未易形）时装备门应回落到本体
        // intrinsic race="whale"，被 Species([human]) 门拒绝——证明上一条测试确实
        // 是因为 MorphState 生效才放行，不是这件甲本来就对谁都放行。
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        app.add_plugins(EntityPlugin);
        register_request_app(&mut app);
        app.insert_resource(make_armor_straw_chestplate_registry(
            crate::body_plan::HUMAN_RACE_ID,
        ));

        let client = spawn_player_with_armor_straw_chestplate_in_pack(&mut app, "Morpher", "whale");

        send_species_gated_equip_intent(&mut app, client);

        let inventory = app.world().entity(client).get::<PlayerInventory>().unwrap();
        let equipped_chest = inventory
            .equipped
            .get(crate::inventory::EQUIP_SLOT_CHEST)
            .and_then(|contents| contents.worn.first());
        assert!(
            equipped_chest.is_none(),
            "未易形的 whale 本体应被 Species([human]) 门拒绝穿戴，实测装备槽：{:?}",
            inventory.equipped.get(crate::inventory::EQUIP_SLOT_CHEST)
        );
        // 应仍留在原背包容器里，而不是被静默吞掉。
        let still_in_pack = inventory.containers[0]
            .items
            .iter()
            .any(|placed| placed.instance.instance_id == 1);
        assert!(still_in_pack, "拒绝装备后物品应留在原容器，不能凭空消失");
    }

    #[test]
    fn meridian_label_maps_regular_and_extraordinary_channels() {
        let cases = [
            (MeridianId::Lung, "肺经"),
            (MeridianId::LargeIntestine, "大肠经"),
            (MeridianId::Stomach, "胃经"),
            (MeridianId::Spleen, "脾经"),
            (MeridianId::Heart, "心经"),
            (MeridianId::SmallIntestine, "小肠经"),
            (MeridianId::Bladder, "膀胱经"),
            (MeridianId::Kidney, "肾经"),
            (MeridianId::Pericardium, "心包经"),
            (MeridianId::TripleEnergizer, "三焦经"),
            (MeridianId::Gallbladder, "胆经"),
            (MeridianId::Liver, "肝经"),
            (MeridianId::Ren, "任脉"),
            (MeridianId::Du, "督脉"),
            (MeridianId::Chong, "冲脉"),
            (MeridianId::Dai, "带脉"),
            (MeridianId::YinQiao, "阴跷脉"),
            (MeridianId::YangQiao, "阳跷脉"),
            (MeridianId::YinWei, "阴维脉"),
            (MeridianId::YangWei, "阳维脉"),
        ];

        for (id, expected) in cases {
            assert_eq!(
                meridian_label(&id.channel_id()),
                expected,
                "expected stable chat label for {id:?}"
            );
        }
    }

    #[test]
    fn meridian_label_falls_back_for_unknown_channel_id() {
        // plan-race-system-v1 P1c — 非 humanoid channel id（如未来 whale 部位）没有中文
        // 标签映射时，必须显式回落"未知经脉"，不得 panic 或伪造某个已知标签。
        assert_eq!(
            meridian_label(&MeridianChannelId::new("tail_fin_channel")),
            "未知经脉"
        );
    }

    #[test]
    fn npc_trade_request_rejects_wanted_player_through_engagement_wiring() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        app.add_plugins(EntityPlugin);
        register_request_app(&mut app);
        app.insert_resource(ZoneRegistry::load_from_path(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("zones.json"),
        ));

        let qingyun_pos = DVec3::new(-3000.0, 120.0, -2000.0);
        let (client_bundle, mut helper) = create_mock_client("Azure");
        let mut faction_reputation = FactionReputation::default();
        faction_reputation.apply_delta(NamedFactionId::QingyunHunters, -51);
        let mut npc_membership = neutral_faction_membership();
        npc_membership.reputation = Reputation { loyalty: 0.8 };
        let score_gate_value = reputation_to_player_score_for_npc_zone(
            Some(&npc_membership),
            None,
            Some(&faction_reputation),
            Some("qingyun_peaks"),
        );
        assert!(
            score_gate_value >= -30,
            "test precondition: legacy score gate must allow trade so Wanted tier is the rejection source, actual {score_gate_value}"
        );
        let player = app
            .world_mut()
            .spawn((
                client_bundle,
                PlayerIdentities::with_default("Azure", 0),
                faction_reputation,
            ))
            .id();
        app.world_mut()
            .entity_mut(player)
            .insert(Position::new(qingyun_pos));
        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                EntityKind::VILLAGER,
                EntityId::default(),
                Position::new(qingyun_pos + DVec3::new(1.0, 0.0, 0.0)),
                OldPosition::new(qingyun_pos + DVec3::new(1.0, 0.0, 0.0)),
                NpcArchetype::Commoner,
                npc_membership,
            ))
            .id();

        app.update();
        let npc_entity_id = app
            .world()
            .get::<EntityId>(npc)
            .expect("EntityPlugin must assign protocol id to NPC")
            .get();
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: player,
                channel: ident!("bong:client_request").into(),
                data: serde_json::to_vec(&ClientRequestV1::NpcTradeRequest {
                    v: 1,
                    npc_entity_id,
                    offered_items: Vec::new(),
                    requested_item_id: "spirit_grass".to_string(),
                })
                .expect("npc trade request should serialize")
                .into_boxed_slice(),
            });

        app.update();
        flush_all_client_packets(&mut app);

        let messages = collect_game_messages(&mut helper);
        assert!(
            messages.iter().any(|message| message.contains("不做买卖")),
            "Wanted player should be refused by NpcTradeRequest via resolve_npc_engagement_target/can_trade wiring, messages={messages:?}"
        );
        assert!(
            app.world().get::<PlayerInventory>(player).is_none(),
            "Wanted rejection happens before trade side effects or inventory mutation"
        );
    }

    fn setup_npc_request_app(
        player_position: DVec3,
        npc_position: DVec3,
        player_dimension: Option<DimensionKind>,
        npc_dimension: Option<DimensionKind>,
        archetype: NpcArchetype,
    ) -> (App, Entity, Entity, i32, MockClientHelper) {
        let mut app = App::new();
        app.add_plugins(EntityPlugin);
        register_request_app(&mut app);

        let (client_bundle, helper) = create_mock_client("NpcRoute");
        let player = app
            .world_mut()
            .spawn((client_bundle, empty_inventory()))
            .id();
        app.world_mut()
            .entity_mut(player)
            .insert(Position::new(player_position));
        if let Some(dimension) = player_dimension {
            app.world_mut()
                .entity_mut(player)
                .insert(CurrentDimension(dimension));
        }

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                EntityKind::VILLAGER,
                EntityId::default(),
                Position::new(npc_position),
                OldPosition::new(npc_position),
                archetype,
            ))
            .id();
        if let Some(dimension) = npc_dimension {
            app.world_mut()
                .entity_mut(npc)
                .insert(CurrentDimension(dimension));
        }

        app.update();
        let npc_entity_id = app
            .world()
            .get::<EntityId>(npc)
            .expect("EntityPlugin must assign protocol id to NPC")
            .get();
        (app, player, npc, npc_entity_id, helper)
    }

    fn send_npc_request(app: &mut App, client: Entity, request: ClientRequestV1) {
        app.world_mut()
            .resource_mut::<Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client,
                channel: ident!("bong:client_request").into(),
                data: serde_json::to_vec(&request)
                    .expect("NPC request should serialize")
                    .into_boxed_slice(),
            });
    }

    #[test]
    fn npc_inspect_request_preserves_feedback_and_rejects_invalid_targets() {
        let (mut app, player, _npc, npc_entity_id, mut helper) = setup_npc_request_app(
            DVec3::new(0.0, 64.0, 0.0),
            DVec3::new(1.0, 64.0, 0.0),
            None,
            None,
            NpcArchetype::Commoner,
        );
        let revision_before = app.world().get::<PlayerInventory>(player).unwrap().revision;

        send_npc_request(
            &mut app,
            player,
            ClientRequestV1::NpcInspectRequest {
                v: 1,
                npc_entity_id,
            },
        );
        app.update();
        flush_all_client_packets(&mut app);
        let messages = collect_game_messages(&mut helper);
        assert_eq!(
            messages.len(),
            1,
            "a nearby inspect must emit exactly one chat line"
        );
        assert!(
            messages[0].starts_with("§7[NPC] "),
            "inspect must preserve the existing NPC greeting feedback, messages={messages:?}"
        );
        assert_eq!(
            app.world().get::<PlayerInventory>(player).unwrap().revision,
            revision_before,
            "inspect must not mutate the player inventory"
        );

        send_npc_request(
            &mut app,
            player,
            ClientRequestV1::NpcInspectRequest {
                v: 1,
                npc_entity_id: npc_entity_id.saturating_add(9999),
            },
        );
        app.update();
        flush_all_client_packets(&mut app);
        let messages = collect_game_messages(&mut helper);
        assert_eq!(
            messages,
            vec!["[NPC] 目标已不在附近，无法查看。"],
            "an unresolved NPC id must use the existing inspect rejection feedback"
        );

        app.world_mut()
            .entity_mut(player)
            .insert(Position::new(DVec3::new(0.0, 64.0, 0.0)));
        app.world_mut()
            .entity_mut(_npc)
            .insert(Position::new(DVec3::new(6.000_001, 64.0, 0.0)));
        send_npc_request(
            &mut app,
            player,
            ClientRequestV1::NpcInspectRequest {
                v: 1,
                npc_entity_id,
            },
        );
        app.update();
        flush_all_client_packets(&mut app);
        let messages = collect_game_messages(&mut helper);
        assert_eq!(
            messages,
            vec!["[NPC] 目标已不在附近，无法查看。"],
            "an NPC beyond the six-block interaction boundary must be rejected"
        );

        app.world_mut()
            .entity_mut(_npc)
            .insert(Position::new(DVec3::new(1.0, 64.0, 0.0)));
        app.world_mut()
            .entity_mut(player)
            .insert(CurrentDimension(DimensionKind::Overworld));
        app.world_mut()
            .entity_mut(_npc)
            .insert(CurrentDimension(DimensionKind::Tsy));
        send_npc_request(
            &mut app,
            player,
            ClientRequestV1::NpcInspectRequest {
                v: 1,
                npc_entity_id,
            },
        );
        app.update();
        flush_all_client_packets(&mut app);
        let messages = collect_game_messages(&mut helper);
        assert_eq!(
            messages,
            vec!["[NPC] 目标已不在附近，无法查看。"],
            "an NPC in another dimension must be rejected before feedback lookup"
        );
    }

    #[test]
    fn npc_dialogue_request_preserves_choices_and_refusal_audio() {
        let (mut app, player, _npc, npc_entity_id, mut helper) = setup_npc_request_app(
            DVec3::new(0.0, 64.0, 0.0),
            DVec3::new(1.0, 64.0, 0.0),
            None,
            None,
            NpcArchetype::Commoner,
        );
        let revision_before = app.world().get::<PlayerInventory>(player).unwrap().revision;

        for (option_id, expected_message) in
            [("inspect", "端详了一眼"), ("trade", "摊开了随身货物")]
        {
            send_npc_request(
                &mut app,
                player,
                ClientRequestV1::NpcDialogueChoice {
                    v: 1,
                    npc_entity_id,
                    option_id: option_id.to_string(),
                },
            );
            app.update();
            flush_all_client_packets(&mut app);
            let messages = collect_game_messages(&mut helper);
            assert_eq!(
                messages.len(),
                1,
                "dialogue option {option_id} must emit one reply"
            );
            assert!(
                messages[0].contains(expected_message),
                "dialogue option {option_id} must preserve its existing reply, messages={messages:?}"
            );
            assert!(
                app.world_mut()
                    .resource_mut::<Events<PlaySoundRecipeRequest>>()
                    .drain()
                    .next()
                    .is_none(),
                "accepted dialogue option {option_id} must not emit refusal audio"
            );
        }

        send_npc_request(
            &mut app,
            player,
            ClientRequestV1::NpcDialogueChoice {
                v: 1,
                npc_entity_id,
                option_id: "leave".to_string(),
            },
        );
        app.update();
        flush_all_client_packets(&mut app);
        assert!(
            collect_game_messages(&mut helper).is_empty(),
            "leave must preserve the existing silent dialogue behavior"
        );
        assert!(
            app.world_mut()
                .resource_mut::<Events<PlaySoundRecipeRequest>>()
                .drain()
                .next()
                .is_none(),
            "leave must not emit refusal audio"
        );

        send_npc_request(
            &mut app,
            player,
            ClientRequestV1::NpcDialogueChoice {
                v: 1,
                npc_entity_id,
                option_id: "not-a-dialogue-option".to_string(),
            },
        );
        app.update();
        flush_all_client_packets(&mut app);
        let messages = collect_game_messages(&mut helper);
        assert_eq!(
            messages.len(),
            1,
            "an invalid dialogue option must emit one refusal"
        );
        assert!(
            messages[0].contains("不愿回应这个选择"),
            "invalid dialogue option must preserve the refusal feedback, messages={messages:?}"
        );
        let refusal_audio = app
            .world_mut()
            .resource_mut::<Events<PlaySoundRecipeRequest>>()
            .drain()
            .collect::<Vec<_>>();
        assert_eq!(
            refusal_audio.len(),
            1,
            "invalid dialogue option must emit one refusal sound"
        );
        assert_eq!(refusal_audio[0].recipe_id, "npc_refuse");
        assert_eq!(
            app.world().get::<PlayerInventory>(player).unwrap().revision,
            revision_before,
            "dialogue choices must not mutate the player inventory revision"
        );
    }

    fn run_npc_trade_request(
        player_inventory: PlayerInventory,
        trade_inventory: Option<crate::npc::trade::NpcTradeInventory>,
        requested_item_id: &str,
    ) -> (App, Entity, MockClientHelper) {
        run_npc_trade_request_with_reputation(
            player_inventory,
            trade_inventory,
            requested_item_id,
            None,
        )
    }

    fn run_npc_trade_request_with_reputation(
        player_inventory: PlayerInventory,
        trade_inventory: Option<crate::npc::trade::NpcTradeInventory>,
        requested_item_id: &str,
        npc_player_reputation: Option<NpcPlayerReputation>,
    ) -> (App, Entity, MockClientHelper) {
        run_npc_trade_request_with_context(
            player_inventory,
            trade_inventory,
            requested_item_id,
            npc_player_reputation,
            None,
            DVec3::new(0.0, 64.0, 0.0),
            None,
        )
    }

    fn run_npc_trade_request_with_context(
        player_inventory: PlayerInventory,
        trade_inventory: Option<crate::npc::trade::NpcTradeInventory>,
        requested_item_id: &str,
        npc_player_reputation: Option<NpcPlayerReputation>,
        player_faction_reputation: Option<FactionReputation>,
        position: DVec3,
        npc_membership: Option<FactionMembership>,
    ) -> (App, Entity, MockClientHelper) {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        app.add_plugins(EntityPlugin);
        register_request_app(&mut app);
        app.insert_resource(crate::inventory::load_item_registry().unwrap());
        app.insert_resource(crate::inventory::InventoryInstanceIdAllocator::default());
        if player_faction_reputation.is_some() {
            app.insert_resource(ZoneRegistry::load_from_path(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("zones.json"),
            ));
        }

        let (client_bundle, helper) = create_mock_client("Azure");
        let player = app
            .world_mut()
            .spawn((client_bundle, player_inventory))
            .id();
        app.world_mut()
            .entity_mut(player)
            .insert(Position::new(position));
        if let Some(player_faction_reputation) = player_faction_reputation {
            app.world_mut()
                .entity_mut(player)
                .insert(player_faction_reputation);
        }
        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                EntityKind::VILLAGER,
                EntityId::default(),
                Position::new(position + DVec3::new(1.0, 0.0, 0.0)),
                OldPosition::new(position + DVec3::new(1.0, 0.0, 0.0)),
                NpcArchetype::Commoner,
            ))
            .id();
        if let Some(trade_inventory) = trade_inventory {
            app.world_mut().entity_mut(npc).insert(trade_inventory);
        }
        if let Some(npc_player_reputation) = npc_player_reputation {
            app.world_mut()
                .entity_mut(npc)
                .insert(npc_player_reputation);
        }
        if let Some(npc_membership) = npc_membership {
            app.world_mut().entity_mut(npc).insert(npc_membership);
        }

        app.update();
        let npc_entity_id = app
            .world()
            .get::<EntityId>(npc)
            .expect("EntityPlugin must assign protocol id to NPC")
            .get();
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: player,
                channel: ident!("bong:client_request").into(),
                data: serde_json::to_vec(&ClientRequestV1::NpcTradeRequest {
                    v: 1,
                    npc_entity_id,
                    offered_items: Vec::new(),
                    requested_item_id: requested_item_id.to_string(),
                })
                .expect("npc trade request should serialize")
                .into_boxed_slice(),
            });

        app.update();
        flush_all_client_packets(&mut app);
        (app, player, helper)
    }

    fn live_trade_offer(
        template_id: &str,
        display_name: &str,
        count: u32,
        price_bone_coins: u32,
    ) -> crate::npc::trade::TradeOffer {
        crate::npc::trade::TradeOffer {
            template_id: template_id.to_string(),
            display_name: display_name.to_string(),
            count,
            price_bone_coins,
        }
    }

    fn inventory_item_count(inventory: &PlayerInventory, template_id: &str) -> u32 {
        inventory
            .containers
            .iter()
            .flat_map(|container| container.items.iter())
            .filter(|placed| placed.instance.template_id == template_id)
            .map(|placed| placed.instance.stack_count)
            .sum()
    }

    #[test]
    fn npc_trade_request_grants_full_bundle_count() {
        let mut inventory = empty_inventory();
        inventory.bone_coins = 100;
        let (app, player, _helper) = run_npc_trade_request(
            inventory,
            Some(crate::npc::trade::NpcTradeInventory {
                offers: vec![live_trade_offer("spirit_grass", "灵草", 3, 12)],
            }),
            "spirit_grass",
        );

        let inventory = app
            .world()
            .get::<PlayerInventory>(player)
            .expect("trade should keep player inventory attached");
        assert_eq!(
            inventory_item_count(inventory, "spirit_grass"),
            3,
            "one accepted bundle offer must grant its full live count"
        );
        assert_eq!(
            inventory.bone_coins, 88,
            "one accepted bundle offer must deduct its live total price exactly once"
        );
    }

    #[test]
    fn npc_trade_request_single_count_offer_still_grants_one() {
        let mut inventory = empty_inventory();
        inventory.bone_coins = 100;
        let (app, player, _helper) = run_npc_trade_request(
            inventory,
            Some(crate::npc::trade::NpcTradeInventory {
                offers: vec![live_trade_offer("spirit_grass", "灵草", 1, 12)],
            }),
            "spirit_grass",
        );
        let inventory = app.world().get::<PlayerInventory>(player).unwrap();
        assert_eq!(inventory_item_count(inventory, "spirit_grass"), 1);
        assert_eq!(inventory.bone_coins, 88);
    }

    #[test]
    fn npc_trade_request_rejects_offer_not_present_in_live_inventory() {
        let mut inventory = empty_inventory();
        inventory.bone_coins = 100;
        let original_revision = inventory.revision;
        let (app, player, mut helper) = run_npc_trade_request(
            inventory,
            Some(crate::npc::trade::NpcTradeInventory {
                offers: vec![live_trade_offer(
                    "ling_xi_wan_flawed",
                    "灵息丸（次品）",
                    2,
                    8,
                )],
            }),
            "spirit_grass",
        );
        let inventory = app.world().get::<PlayerInventory>(player).unwrap();
        assert_eq!(inventory_item_count(inventory, "spirit_grass"), 0);
        assert_eq!(inventory.bone_coins, 100);
        assert_eq!(inventory.revision, original_revision);
        let messages = collect_game_messages(&mut helper);
        assert!(
            messages
                .iter()
                .any(|message| message.contains("当前没有这件货")),
            "live subset rejection must be visible to the player, messages={messages:?}"
        );
        assert!(
            messages.iter().all(|message| !message.contains("买下")),
            "live subset rejection must not emit success feedback, messages={messages:?}"
        );
    }

    #[test]
    fn npc_trade_request_rejects_empty_live_inventory_without_side_effects() {
        let mut inventory = empty_inventory();
        inventory.bone_coins = 100;
        inventory.revision = InventoryRevision(8);
        let (app, player, mut helper) = run_npc_trade_request(
            inventory,
            Some(crate::npc::trade::NpcTradeInventory { offers: vec![] }),
            "spirit_grass",
        );
        let inventory = app.world().get::<PlayerInventory>(player).unwrap();
        assert_eq!(inventory_item_count(inventory, "spirit_grass"), 0);
        assert_eq!(inventory.bone_coins, 100);
        assert_eq!(inventory.revision, InventoryRevision(8));
        let messages = collect_game_messages(&mut helper);
        assert!(
            messages
                .iter()
                .any(|message| message.contains("当前没有这件货")),
            "empty live inventory must use the visible missing-offer rejection, messages={messages:?}"
        );
        assert!(
            messages.iter().all(|message| !message.contains("买下")),
            "empty live inventory must not emit success feedback, messages={messages:?}"
        );
    }

    #[test]
    fn npc_trade_request_uses_live_offer_count_not_catalogue_default() {
        let mut inventory = empty_inventory();
        inventory.bone_coins = 100;
        let (app, player, _helper) = run_npc_trade_request(
            inventory,
            Some(crate::npc::trade::NpcTradeInventory {
                offers: vec![live_trade_offer("spirit_grass", "灵草", 5, 10)],
            }),
            "lingcao",
        );
        let inventory = app.world().get::<PlayerInventory>(player).unwrap();
        assert_eq!(inventory_item_count(inventory, "spirit_grass"), 5);
        assert_eq!(inventory.bone_coins, 90);
    }

    #[test]
    fn npc_trade_request_success_message_includes_bundle_count() {
        let mut inventory = empty_inventory();
        inventory.bone_coins = 100;
        let (_app, _player, mut helper) = run_npc_trade_request(
            inventory,
            Some(crate::npc::trade::NpcTradeInventory {
                offers: vec![live_trade_offer("spirit_grass", "灵草", 3, 12)],
            }),
            "spirit_grass",
        );
        let messages = collect_game_messages(&mut helper);
        assert!(
            messages.iter().any(|message| message.contains("灵草 x3")),
            "success feedback must expose the granted bundle count, messages={messages:?}"
        );
    }

    #[test]
    fn npc_trade_request_uses_live_offer_total_price() {
        let mut inventory = empty_inventory();
        inventory.bone_coins = 100;
        let (app, player, _helper) = run_npc_trade_request(
            inventory,
            Some(crate::npc::trade::NpcTradeInventory {
                offers: vec![live_trade_offer("spirit_grass", "灵草", 3, 17)],
            }),
            "spirit_grass",
        );
        let inventory = app.world().get::<PlayerInventory>(player).unwrap();
        assert_eq!(inventory_item_count(inventory, "spirit_grass"), 3);
        assert_eq!(
            inventory.bone_coins, 83,
            "live offer price is the bundle total and must override catalogue price"
        );
    }

    #[test]
    fn npc_trade_request_applies_non_neutral_reputation_to_live_bundle_total() {
        let mut inventory = empty_inventory();
        inventory.bone_coins = 18;
        let mut reputation = NpcPlayerReputation::default();
        reputation.adjust("offline:Azure", 0.3);
        let (app, player, _helper) = run_npc_trade_request_with_reputation(
            inventory,
            Some(crate::npc::trade::NpcTradeInventory {
                offers: vec![live_trade_offer("spirit_grass", "灵草", 3, 20)],
            }),
            "spirit_grass",
            Some(reputation),
        );
        let inventory = app.world().get::<PlayerInventory>(player).unwrap();
        assert_eq!(inventory_item_count(inventory, "spirit_grass"), 3);
        assert_eq!(
            inventory.bone_coins, 0,
            "high reputation discount must apply to live total 20 (current ceil result 18), not catalogue 10"
        );
    }

    #[test]
    fn npc_trade_request_applies_faction_reputation_to_live_bundle_total() {
        let mut inventory = empty_inventory();
        inventory.bone_coins = 18;
        let mut faction_reputation = FactionReputation::default();
        faction_reputation.apply_delta(NamedFactionId::QingyunHunters, 51);
        let (app, player, _helper) = run_npc_trade_request_with_context(
            inventory,
            Some(crate::npc::trade::NpcTradeInventory {
                offers: vec![live_trade_offer("spirit_grass", "灵草", 3, 20)],
            }),
            "spirit_grass",
            None,
            Some(faction_reputation),
            DVec3::new(-3000.0, 120.0, -2000.0),
            Some(neutral_faction_membership()),
        );
        let inventory = app.world().get::<PlayerInventory>(player).unwrap();
        assert_eq!(inventory_item_count(inventory, "spirit_grass"), 3);
        assert_eq!(
            inventory.bone_coins, 0,
            "Qingyun high faction reputation must discount live total 20, not catalogue 10"
        );
    }

    #[test]
    fn npc_trade_request_rejects_when_only_catalogue_price_is_affordable() {
        let mut inventory = empty_inventory();
        inventory.bone_coins = 11;
        inventory.revision = InventoryRevision(13);
        let (app, player, mut helper) = run_npc_trade_request(
            inventory,
            Some(crate::npc::trade::NpcTradeInventory {
                offers: vec![live_trade_offer("spirit_grass", "灵草", 3, 12)],
            }),
            "spirit_grass",
        );
        let inventory = app.world().get::<PlayerInventory>(player).unwrap();
        assert_eq!(
            inventory_item_count(inventory, "spirit_grass"),
            0,
            "affording the catalogue price must not grant any part of a dearer live bundle"
        );
        assert_eq!(inventory.bone_coins, 11);
        assert_eq!(inventory.revision, InventoryRevision(13));
        let messages = collect_game_messages(&mut helper);
        assert!(
            messages
                .iter()
                .any(|message| message.contains("骨币不足，需要 12 枚")),
            "rejection must expose the live bundle total, messages={messages:?}"
        );
        assert!(
            messages.iter().all(|message| !message.contains("买下")),
            "an unaffordable live bundle must not emit success feedback, messages={messages:?}"
        );
    }

    #[test]
    fn npc_trade_request_rejects_missing_trade_inventory_without_side_effects() {
        let mut inventory = empty_inventory();
        inventory.bone_coins = 100;
        inventory.revision = InventoryRevision(7);
        let (app, player, mut helper) = run_npc_trade_request(inventory, None, "spirit_grass");
        let inventory = app.world().get::<PlayerInventory>(player).unwrap();
        assert_eq!(inventory_item_count(inventory, "spirit_grass"), 0);
        assert_eq!(inventory.bone_coins, 100);
        assert_eq!(inventory.revision, InventoryRevision(7));
        let messages = collect_game_messages(&mut helper);
        assert!(
            messages
                .iter()
                .any(|message| message.contains("当前没有可成交的货物")),
            "missing trade component rejection must be visible, messages={messages:?}"
        );
        assert!(
            messages.iter().all(|message| !message.contains("买下")),
            "missing trade component must not emit success feedback, messages={messages:?}"
        );
    }

    #[test]
    fn npc_trade_request_inventory_failure_keeps_coins_and_revision() {
        let inventory = PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(11),
            containers: Vec::new(),
            equipped: Default::default(),
            hotbar: Default::default(),
            bone_coins: 100,
            max_weight: 50.0,
        };
        let (app, player, _helper) = run_npc_trade_request(
            inventory,
            Some(crate::npc::trade::NpcTradeInventory {
                offers: vec![live_trade_offer("spirit_grass", "灵草", 3, 12)],
            }),
            "spirit_grass",
        );
        let inventory = app.world().get::<PlayerInventory>(player).unwrap();
        assert_eq!(inventory.bone_coins, 100);
        assert_eq!(inventory.revision, InventoryRevision(11));
    }

    #[test]
    fn npc_trade_request_partial_bundle_capacity_fails_atomically() {
        let registry = crate::inventory::load_item_registry().unwrap();
        let mut allocator = crate::inventory::InventoryInstanceIdAllocator::default();
        let mut inventory = empty_inventory();
        add_item_to_player_inventory(
            &mut inventory,
            &registry,
            &mut allocator,
            "spirit_grass",
            63,
            0,
        )
        .expect("test precondition: one compatible spirit grass stack must fit");
        inventory.containers[0].rows = 1;
        inventory.containers[0].cols = 1;
        inventory.bone_coins = 100;
        inventory.revision = InventoryRevision(17);

        let (app, player, mut helper) = run_npc_trade_request(
            inventory,
            Some(crate::npc::trade::NpcTradeInventory {
                offers: vec![live_trade_offer("spirit_grass", "灵草", 2, 12)],
            }),
            "spirit_grass",
        );
        let inventory = app.world().get::<PlayerInventory>(player).unwrap();
        assert_eq!(
            inventory_item_count(inventory, "spirit_grass"),
            63,
            "a bundle that can merge only one of two items must not partially mutate the stack"
        );
        assert_eq!(inventory.containers[0].items.len(), 1);
        assert_eq!(inventory.bone_coins, 100);
        assert_eq!(inventory.revision, InventoryRevision(17));
        let messages = collect_game_messages(&mut helper);
        assert!(
            messages.iter().any(|message| message.contains("交易失败")),
            "partial-capacity rejection must remain player-visible, messages={messages:?}"
        );
        assert!(messages.iter().all(|message| !message.contains("买下")));
    }

    #[test]
    fn set_meridian_target_sends_generic_meridian_chat_echo() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: serde_json::to_vec(&ClientRequestV1::SetMeridianTarget {
                    v: 1,
                    meridian: MeridianId::Du.channel_id(),
                })
                .expect("set meridian target request should serialize")
                .into_boxed_slice(),
            });

        app.update();
        flush_all_client_packets(&mut app);

        let actual_target = app
            .world()
            .get::<MeridianTarget>(entity)
            .map(|target| target.0.clone());
        assert_eq!(
            actual_target,
            Some(MeridianId::Du.channel_id()),
            "expected SetMeridianTarget to insert selected meridian target, actual={:?}",
            actual_target
        );
        let messages = collect_game_messages(&mut helper);
        assert!(
            messages
                .iter()
                .any(|message| message.contains("[修炼] 已收到经脉目标：督脉。")),
            "expected generic meridian target chat echo because request is not limited to Chong, actual messages={messages:?}"
        );
    }

    /// plan-race-system-v1 P1 对抗审查 M4：`SetMeridianTarget` 消费边界收到未知
    /// channel id（伪造串 / 旧 PascalCase `MeridianId::Lung` 字面量 "Lung"）时必须
    /// 安全处理（回执标注"未知经脉"，`MeridianTarget` component 允许被设置但下游
    /// `meridian_open_tick` 会安全跳过，见该 system 的 debug 分支）——绝不 panic，
    /// 也不能把未知串误当合法经脉给出中文标签回执。
    #[test]
    fn set_meridian_target_with_unknown_channel_id_is_handled_safely_not_panicking() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: serde_json::to_vec(&ClientRequestV1::SetMeridianTarget {
                    v: 1,
                    meridian: crate::cultivation::components::MeridianChannelId::new(
                        "totally_made_up_channel",
                    ),
                })
                .expect("set meridian target request should serialize")
                .into_boxed_slice(),
            });

        // 必须不 panic —— 这是本用例的核心断言。
        app.update();
        flush_all_client_packets(&mut app);

        let messages = collect_game_messages(&mut helper);
        assert!(
            messages.iter().any(|message| message.contains("未知经脉")),
            "unknown channel id 必须回执'未知经脉'占位而不是伪造某个真实经脉名，\
             actual messages={messages:?}"
        );
    }

    /// 旧 PascalCase 字面量 "Lung"（`MeridianId::Lung` 的 `Debug`/枚举名拼写，非合法
    /// wire channel id）同样必须走"未知经脉"安全分支，不能被误认成合法的 lung 经脉。
    #[test]
    fn set_meridian_target_with_legacy_pascal_case_lung_string_is_rejected_as_unknown() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: serde_json::to_vec(&ClientRequestV1::SetMeridianTarget {
                    v: 1,
                    meridian: crate::cultivation::components::MeridianChannelId::new("Lung"),
                })
                .expect("set meridian target request should serialize")
                .into_boxed_slice(),
            });

        app.update();
        flush_all_client_packets(&mut app);

        let messages = collect_game_messages(&mut helper);
        assert!(
            messages.iter().any(|message| message.contains("未知经脉")),
            "旧 PascalCase 'Lung' 不是合法 snake_case wire channel id ('lung')，必须走\
             未知经脉分支而非被误认作肺经，actual messages={messages:?}"
        );
    }

    #[test]
    fn qi_scatter_bead_use_dispatches_zhenfa_event() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.world_mut().resource_mut::<CombatClock>().tick = 33;

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: serde_json::to_vec(&ClientRequestV1::QiScatterBeadUse {
                    v: 1,
                    item_instance_id: 7001,
                    x: None,
                    y: None,
                    z: None,
                })
                .expect("qi scatter bead request should serialize")
                .into_boxed_slice(),
            });

        app.update();

        let mut events = app
            .world()
            .resource::<Events<ScatterBeadUseRequest>>()
            .iter_current_update_events();
        let event = events
            .next()
            .expect("qi_scatter_bead_use must dispatch ScatterBeadUseRequest");
        assert_eq!(event.player, entity);
        assert_eq!(event.item_instance_id, 7001);
        assert_eq!(event.bury_pos, None);
        assert_eq!(event.requested_at_tick, 33);
        assert!(
            events.next().is_none(),
            "qi_scatter_bead_use should emit exactly one request event"
        );
    }

    #[test]
    fn qi_scatter_bead_use_with_coords_dispatches_burial_pos() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"qi_scatter_bead_use","v":1,"item_instance_id":7002,"x":1,"y":64,"z":-2}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

        app.update();

        let event = app
            .world()
            .resource::<Events<ScatterBeadUseRequest>>()
            .iter_current_update_events()
            .next()
            .expect("qi_scatter_bead_use with coords must dispatch burial request");
        assert_eq!(event.player, entity);
        assert_eq!(event.item_instance_id, 7002);
        assert_eq!(event.bury_pos, Some([1, 64, -2]));
    }

    #[test]
    fn block_place_payload_dispatches_runtime_request_event() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"block_place","v":1,"x":8,"y":64,"z":8,"item_instance_id":4242,"target_face":"north"}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

        app.update();

        let events = app
            .world()
            .resource::<valence::prelude::Events<BlockPlaceRequest>>();
        let requests = events.iter_current_update_events().collect::<Vec<_>>();
        assert_eq!(
            requests.len(),
            1,
            "expected exactly one BlockPlaceRequest from one valid block_place payload"
        );
        let request = requests[0];
        assert_eq!(request.client, entity);
        assert_eq!((request.x, request.y, request.z), (8, 64, 8));
        assert_eq!(request.item_instance_id, 4242);
        assert_eq!(request.target_face, TrapTargetFace::North);
    }

    // ─── plan-worldgen-v4 P5 §8.1#5 — block_picker_give 路由测试矩阵 ───

    /// 把一段 wire JSON 喂给 handler，返回这一轮 emit 的 BlockPickerGiveIntent 列表。
    fn dispatch_block_picker_give(
        json: &[u8],
    ) -> (
        App,
        valence::prelude::Entity,
        Vec<crate::cmd::dev::block_picker::BlockPickerGiveIntent>,
    ) {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: json.to_vec().into_boxed_slice(),
            });
        app.update();
        let events = app
            .world()
            .resource::<valence::prelude::Events<crate::cmd::dev::block_picker::BlockPickerGiveIntent>>();
        let collected = events
            .iter_current_update_events()
            .cloned()
            .collect::<Vec<_>>();
        (app, entity, collected)
    }

    /// happy path + 链路：合法 block_picker_give payload → 恰好 1 次 BlockPickerGiveIntent，
    /// 且 block_id / count / player 字段一路透传。
    #[test]
    fn block_picker_give_payload_dispatches_intent_with_fields() {
        let (_app, entity, intents) = dispatch_block_picker_give(
            br#"{"type":"block_picker_give","v":1,"block_id":"stone_bricks","count":16}"#,
        );
        assert_eq!(
            intents.len(),
            1,
            "一条合法 block_picker_give payload 应 emit 恰好 1 次 BlockPickerGiveIntent，实为 {}",
            intents.len()
        );
        assert_eq!(intents[0].player, entity, "intent 必须带回发起玩家 entity");
        assert_eq!(
            intents[0].block_id, "stone_bricks",
            "block_id 必须透传，实为 {}",
            intents[0].block_id
        );
        assert_eq!(
            intents[0].count, 16,
            "count 必须透传，实为 {}",
            intents[0].count
        );
    }

    fn dispatch_remains_loot(
        json: &[u8],
    ) -> (
        valence::prelude::Entity,
        Vec<crate::inventory::RemainsLootIntent>,
    ) {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: json.to_vec().into_boxed_slice(),
            });
        app.update();
        let events = app
            .world()
            .resource::<valence::prelude::Events<crate::inventory::RemainsLootIntent>>();
        let collected = events
            .iter_current_update_events()
            .cloned()
            .collect::<Vec<_>>();
        (entity, collected)
    }

    #[test]
    fn remains_loot_request_dispatches_intent_with_fields() {
        let (entity, intents) = dispatch_remains_loot(
            br#"{"type":"remains_loot","v":1,"remains_id":"3fa85f64-5717-4562-b3fc-2c963f66afa6"}"#,
        );

        assert_eq!(
            intents.len(),
            1,
            "合法 remains_loot payload 应 emit 恰好 1 次 RemainsLootIntent，实为 {}",
            intents.len()
        );
        assert_eq!(intents[0].entity, entity, "intent 必须带回发起玩家 entity");
        assert_eq!(
            intents[0].remains_id, "3fa85f64-5717-4562-b3fc-2c963f66afa6",
            "remains_id 必须从 wire payload 原样透传"
        );
    }

    #[test]
    fn remains_loot_request_with_blank_id_is_dropped() {
        let (_entity, intents) =
            dispatch_remains_loot(br#"{"type":"remains_loot","v":1,"remains_id":"   "}"#);

        assert!(
            intents.is_empty(),
            "空白 remains_id 应被 handler 拦截，不应 emit RemainsLootIntent；实际 {} 条",
            intents.len()
        );
    }

    /// 边界透传：count=1（下界）与 count=64（上界）合法值都能派发并保值。
    #[test]
    fn block_picker_give_boundary_counts_dispatch() {
        for count in [1u32, 64u32] {
            let json = format!(
                r#"{{"type":"block_picker_give","v":1,"block_id":"stone","count":{count}}}"#
            );
            let (_app, _entity, intents) = dispatch_block_picker_give(json.as_bytes());
            assert_eq!(
                intents.len(),
                1,
                "count={count} 是合法边界，应 emit 1 次 intent，实为 {}",
                intents.len()
            );
            assert_eq!(
                intents[0].count, count,
                "count={count} 应透传保值，实为 {}",
                intents[0].count
            );
        }
    }

    /// 错误分支：count=0 / count=65 越界 payload 在 wire serde 层即被拒，handler 不 emit 任何 intent
    /// （schema serde deserialize_block_picker_count 守门，未通过 → 整个 payload 反序列化失败 → drop）。
    #[test]
    fn block_picker_give_out_of_range_count_is_dropped_before_dispatch() {
        for bad in [
            &br#"{"type":"block_picker_give","v":1,"block_id":"stone","count":0}"#[..],
            &br#"{"type":"block_picker_give","v":1,"block_id":"stone","count":65}"#[..],
        ] {
            let (_app, _entity, intents) = dispatch_block_picker_give(bad);
            assert!(
                intents.is_empty(),
                "越界 count payload 必须在 serde 层被拒、handler 不派发任何 intent，实为 {} 条",
                intents.len()
            );
        }
    }

    /// 错误分支：malformed JSON / 未知字段 payload 不得 emit intent（坏包安静丢弃）。
    #[test]
    fn block_picker_give_malformed_payload_is_dropped() {
        for bad in [
            &b"{not json at all"[..],
            // 多了未知字段 surprise，deny_unknown_fields 拒绝。
            &br#"{"type":"block_picker_give","v":1,"block_id":"stone","count":4,"surprise":true}"#
                [..],
            // 缺 block_id 必填字段。
            &br#"{"type":"block_picker_give","v":1,"count":4}"#[..],
        ] {
            let (_app, _entity, intents) = dispatch_block_picker_give(bad);
            assert!(
                intents.is_empty(),
                "malformed / 非法 block_picker_give payload 不得派发 intent，实为 {} 条",
                intents.len()
            );
        }
    }

    #[test]
    fn workbench_open_payload_requires_entity_manager_before_dispatch() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let client = app.world_mut().spawn(client_bundle).id();
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"workbench_open","v":1,"entity_id":42}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

        app.update();

        let events = app
            .world()
            .resource::<valence::prelude::Events<crate::craft::WorkbenchOpenRequest>>();
        assert_eq!(
            events.iter_current_update_events().count(),
            0,
            "workbench_open must not fabricate an ECS entity when EntityManager is unavailable"
        );
    }

    #[test]
    fn container_open_payload_requires_entity_manager_before_dispatch() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let client = app.world_mut().spawn(client_bundle).id();
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"container_open","v":1,"entity_id":42}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

        app.update();

        let events = app
            .world()
            .resource::<valence::prelude::Events<crate::world::container_open::ContainerOpenRequest>>();
        assert_eq!(
            events.iter_current_update_events().count(),
            0,
            "container_open must not fabricate an ECS entity when EntityManager is unavailable"
        );
    }

    fn assert_movement_action_yaw_forwarded(yaw_degrees: f32) {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.add_event::<MovementActionIntent>();

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        let payload = format!(
            r#"{{"type":"movement_action","v":1,"action":"dash","yaw_degrees":{yaw_degrees}}}"#
        );
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: payload.into_bytes().into_boxed_slice(),
            });

        app.update();

        let events = app
            .world()
            .resource::<valence::prelude::Events<MovementActionIntent>>();
        let intents: Vec<_> = events.iter_current_update_events().collect();
        assert_eq!(
            intents.len(),
            1,
            "expected one MovementActionIntent because movement_action yaw payload was valid, actual: {}",
            intents.len()
        );
        assert_eq!(
            intents[0].entity, entity,
            "expected MovementActionIntent entity to match sending client for yaw_degrees={yaw_degrees}"
        );
        assert_eq!(
            intents[0].action,
            MovementAction::Dashing,
            "expected movement_action dash payload to map to MovementAction::Dashing for yaw_degrees={yaw_degrees}"
        );
        assert_eq!(
            intents[0].yaw_degrees,
            Some(yaw_degrees),
            "expected server to forward numeric yaw_degrees unchanged, actual intent: {:?}",
            intents[0]
        );
    }

    #[test]
    fn alchemy_inject_qi_ignored_for_furnace_in_collapsed_zone() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        let mut zones = ZoneRegistry::fallback();
        zones
            .find_zone_mut("spawn")
            .unwrap()
            .active_events
            .push(EVENT_REALM_COLLAPSE.to_string());
        app.insert_resource(zones);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        let mut furnace = AlchemyFurnace::placed(valence::prelude::BlockPos::new(8, 66, 8), 1);
        furnace.owner = Some("offline:Azure".into());
        furnace.session = Some(AlchemySession::new(
            "kai_mai_pill_v0".into(),
            "offline:Azure".into(),
        ));
        let furnace_entity = app.world_mut().spawn(furnace).id();
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"alchemy_intervention","v":1,"furnace_pos":[8,66,8],"intervention":{"kind":"inject_qi","qi":5.0}}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

        app.update();

        let furnace = app.world().get::<AlchemyFurnace>(furnace_entity).unwrap();
        assert_eq!(furnace.session.as_ref().unwrap().qi_injected, 0.0);
    }

    #[test]
    fn alchemy_explode_take_back_applies_damage_and_meridian_crack() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(crate::alchemy::recipe::load_recipe_registry().unwrap());
        app.insert_resource(crate::inventory::load_item_registry().unwrap());
        app.insert_resource(crate::inventory::InventoryInstanceIdAllocator::default());

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        let mut meridians = crate::cultivation::components::MeridianSystem::default();
        meridians
            .get_mut(crate::cultivation::components::MeridianId::Lung)
            .opened = true;
        app.world_mut().entity_mut(entity).insert((
            crate::combat::components::Wounds {
                health_current: 100.0,
                health_max: 100.0,
                entries: Vec::new(),
            },
            meridians,
            crate::cultivation::components::Cultivation::default(),
            PlayerState::default(),
            inventory_with_stack("ci_she_hao", 3),
        ));

        let mut furnace = AlchemyFurnace::placed(valence::prelude::BlockPos::new(2, 64, 3), 1);
        furnace.owner = Some("offline:Azure".into());
        app.world_mut().spawn(furnace);
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"alchemy_ignite","v":1,"furnace_pos":[2,64,3],"recipe_id":"kai_mai_pill_v0"}"#
                    .to_vec()
                    .into_boxed_slice(),
            });
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"alchemy_feed_slot","v":1,"furnace_pos":[2,64,3],"slot_idx":0,"material":"ci_she_hao","count":3}"#
                    .to_vec()
                    .into_boxed_slice(),
            });
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"alchemy_intervention","v":1,"furnace_pos":[2,64,3],"intervention":{"kind":"adjust_temp","temp":1.0}}"#
                    .to_vec()
                    .into_boxed_slice(),
            });
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"alchemy_take_back","v":1,"furnace_pos":[2,64,3],"slot_idx":0}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

        app.update();

        let wounds = app.world().get::<Wounds>(entity).unwrap();
        assert_eq!(wounds.health_current, 80.0);
        assert!(wounds.entries.iter().any(|wound| {
            wound.kind == WoundKind::Burn && (wound.severity - 20.0).abs() < f32::EPSILON
        }));
        let overload_events = app
            .world()
            .resource::<valence::prelude::Events<crate::cultivation::overload::MeridianOverloadEvent>>();
        let mut reader = overload_events.get_reader();
        let events: Vec<_> = reader.read(overload_events).collect();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].entity, entity);
        assert!((events[0].severity - 0.15).abs() < 1e-9);
    }

    #[test]
    fn alchemy_flawed_take_back_grants_flawed_pill_residue() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(crate::alchemy::recipe::load_recipe_registry().unwrap());
        app.insert_resource(crate::inventory::load_item_registry().unwrap());
        app.insert_resource(crate::inventory::InventoryInstanceIdAllocator::default());

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut().entity_mut(entity).insert((
            crate::cultivation::components::Cultivation::default(),
            PlayerState::default(),
            inventory_with_stack("ci_she_hao", 3),
        ));

        let mut furnace = AlchemyFurnace::placed(valence::prelude::BlockPos::new(3, 64, 4), 1);
        furnace.owner = Some("offline:Azure".into());
        app.world_mut().spawn(furnace);
        for data in [
            br#"{"type":"alchemy_ignite","v":1,"furnace_pos":[3,64,4],"recipe_id":"kai_mai_pill_v0"}"#.as_slice(),
            br#"{"type":"alchemy_feed_slot","v":1,"furnace_pos":[3,64,4],"slot_idx":0,"material":"ci_she_hao","count":3}"#.as_slice(),
            br#"{"type":"alchemy_intervention","v":1,"furnace_pos":[3,64,4],"intervention":{"kind":"inject_qi","qi":15.0}}"#.as_slice(),
            br#"{"type":"alchemy_intervention","v":1,"furnace_pos":[3,64,4],"intervention":{"kind":"adjust_temp","temp":0.60}}"#.as_slice(),
            br#"{"type":"alchemy_take_back","v":1,"furnace_pos":[3,64,4],"slot_idx":0}"#.as_slice(),
        ] {
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: data.to_vec().into_boxed_slice(),
                });
        }

        app.update();

        let inventory = app.world().get::<PlayerInventory>(entity).unwrap();
        let item_summary: Vec<_> = inventory
            .containers
            .iter()
            .flat_map(|container| container.items.iter())
            .map(|placed| {
                format!(
                    "{}:{:?}",
                    placed.instance.template_id, placed.instance.alchemy
                )
            })
            .collect();
        assert!(
            inventory.containers.iter().any(|container| {
                container.items.iter().any(|placed| {
                    placed.instance.template_id
                        == crate::alchemy::residue::FLAWED_PILL_RESIDUE_TEMPLATE_ID
                        && matches!(
                            placed.instance.alchemy,
                            Some(AlchemyItemData::PillResidue {
                                residue_kind: crate::alchemy::residue::PillResidueKind::FlawedPill,
                                ..
                            })
                        )
                })
            }),
            "expected flawed pill residue in inventory, got {item_summary:?}"
        );
    }

    #[test]
    fn alchemy_feed_slot_rejects_wrong_mineral_instance_on_live_request_path() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(crate::alchemy::recipe::load_recipe_registry().unwrap());

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        let mut wrong_mineral = inventory_test_item(9002, "dan_sha_aux", 1);
        wrong_mineral.display_name = "假丹砂辅料".to_string();
        wrong_mineral.mineral_id = Some("zhu_sha".to_string());
        app.world_mut().entity_mut(entity).insert((
            crate::cultivation::components::Cultivation::default(),
            PlayerState::default(),
            PlayerInventory {
                revision: InventoryRevision(0),
                containers: vec![ContainerState {
                    quick_access: false,
                    id: "main_pack".into(),
                    name: "main_pack".into(),
                    rows: 5,
                    cols: 7,
                    items: vec![
                        PlacedItemState {
                            row: 0,
                            col: 0,
                            instance: inventory_test_item(9001, "ci_she_hao", 2),
                        },
                        PlacedItemState {
                            row: 0,
                            col: 1,
                            instance: wrong_mineral,
                        },
                    ],

                    owner_instance_id: None,
                }],
                equipped: Default::default(),
                hotbar: Default::default(),
                triggered_treasures: Vec::new(),
                bone_coins: 0,
                max_weight: 50.0,
            },
        ));

        let mut furnace = AlchemyFurnace::placed(valence::prelude::BlockPos::new(5, 64, 6), 1);
        furnace.owner = Some("offline:Azure".into());
        let furnace_entity = app.world_mut().spawn(furnace).id();
        for data in [
            br#"{"type":"alchemy_ignite","v":1,"furnace_pos":[5,64,6],"recipe_id":"jie_du_dan_v1"}"#.as_slice(),
            br#"{"type":"alchemy_feed_slot","v":1,"furnace_pos":[5,64,6],"slot_idx":0,"material":"ci_she_hao","count":2}"#.as_slice(),
            br#"{"type":"alchemy_feed_slot","v":1,"furnace_pos":[5,64,6],"slot_idx":0,"material":"dan_sha_aux","count":1}"#.as_slice(),
        ] {
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: data.to_vec().into_boxed_slice(),
                });
        }

        app.update();
        flush_all_client_packets(&mut app);

        let furnace = app.world().get::<AlchemyFurnace>(furnace_entity).unwrap();
        let staged = &furnace.session.as_ref().unwrap().staged.materials;
        let staged_ci_she_hao = staged.get("ci_she_hao").copied();
        assert_eq!(
            staged_ci_she_hao,
            Some(2),
            "expected ci_she_hao×2 to stay staged because the first feed request succeeded before wrong mineral rejection, actual staged={staged:?}"
        );
        assert!(
            !staged.contains_key("dan_sha_aux"),
            "wrong mineral_id must not satisfy dan_sha_aux ingredient: {staged:?}"
        );
        let messages = collect_game_messages(&mut helper);
        assert!(
            messages
                .iter()
                .any(|message| message.contains("材料不足或矿物不符")),
            "expected wrong-mineral live request to send alchemy rejection chat, actual messages={messages:?}"
        );
        let inventory = app.world().get::<PlayerInventory>(entity).unwrap();
        assert!(
            inventory.containers.iter().any(|container| {
                container.items.iter().any(|placed| {
                    placed.instance.instance_id == 9002 && placed.instance.stack_count == 1
                })
            }),
            "rejected wrong-mineral item must remain in inventory"
        );
    }

    #[test]
    fn alchemy_ignite_rejects_low_zone_qi_on_live_request_path() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(crate::alchemy::recipe::load_recipe_registry().unwrap());
        app.insert_resource(crate::inventory::load_item_registry().unwrap());
        app.insert_resource(crate::world::zone::ZoneRegistry {
            spatial_revision: 0,
            zones: vec![crate::world::zone::Zone {
                name: "spawn".to_string(),
                dimension: DimensionKind::Overworld,
                bounds: (
                    valence::prelude::DVec3::new(0.0, 0.0, 0.0),
                    valence::prelude::DVec3::new(10.0, 100.0, 10.0),
                ),
                spirit_qi: 0.0,
                danger_level: 0,
                active_events: Vec::new(),
                patrol_anchors: Vec::new(),
                blocked_tiles: Vec::new(),
                qi_equilibrium: 0.0,
                qi_inflow_per_min: 0.0,
            }],
        });

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        let mut furnace = AlchemyFurnace::placed(valence::prelude::BlockPos::new(2, 64, 3), 1);
        furnace.owner = Some("offline:Azure".into());
        let furnace_entity = app.world_mut().spawn(furnace).id();
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"alchemy_ignite","v":1,"furnace_pos":[2,64,3],"recipe_id":"kai_mai_pill_v0"}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

        app.update();

        let furnace = app.world().get::<AlchemyFurnace>(furnace_entity).unwrap();
        assert!(furnace.session.is_none());
    }

    #[test]
    fn brew_emits_vapor() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(crate::alchemy::recipe::load_recipe_registry().unwrap());

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        let mut furnace = AlchemyFurnace::placed(valence::prelude::BlockPos::new(2, 64, 3), 1);
        furnace.owner = Some("offline:Azure".into());
        let furnace_entity = app.world_mut().spawn(furnace).id();
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"alchemy_ignite","v":1,"furnace_pos":[2,64,3],"recipe_id":"kai_mai_pill_v0"}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

        app.update();

        assert!(app
            .world()
            .get::<AlchemyFurnace>(furnace_entity)
            .unwrap()
            .session
            .is_some());
        let events = app
            .world()
            .resource::<valence::prelude::Events<VfxEventRequest>>();
        let emitted = events
            .iter_current_update_events()
            .next()
            .expect("alchemy ignite should emit vapor vfx");
        match &emitted.payload {
            crate::schema::vfx_event::VfxEventPayloadV1::SpawnParticle { event_id, .. } => {
                assert_eq!(event_id, gameplay_vfx::ALCHEMY_BREW_VAPOR);
            }
            other => panic!("expected SpawnParticle, got {other:?}"),
        }
    }

    // ── plan-skill-av-relink-v1 P3 —— alchemy_stir 内联 emit pin ─────────────────

    fn drain_alchemy_stir_anims(app: &mut App) -> Vec<(String, u16)> {
        app.world_mut()
            .resource_mut::<valence::prelude::Events<VfxEventRequest>>()
            .drain()
            .filter_map(|request| match request.payload {
                crate::schema::vfx_event::VfxEventPayloadV1::PlayAnim {
                    target_player,
                    anim_id,
                    priority,
                    ..
                } if anim_id == crate::network::vfx_animation_trigger::ANIM_ALCHEMY_STIR => {
                    Some((target_player, priority))
                }
                _ => None,
            })
            .collect()
    }

    fn spawn_azure_furnace_with_session(app: &mut App, owner: &str) -> valence::prelude::Entity {
        let mut furnace = AlchemyFurnace::placed(valence::prelude::BlockPos::new(8, 66, 8), 1);
        furnace.owner = Some(owner.into());
        furnace.session = Some(AlchemySession::new("kai_mai_pill_v0".into(), owner.into()));
        app.world_mut().spawn(furnace).id()
    }

    fn send_alchemy_intervention_payload(
        app: &mut App,
        client: valence::prelude::Entity,
        intervention_json: &str,
    ) {
        let data = format!(
            r#"{{"type":"alchemy_intervention","v":1,"furnace_pos":[8,66,8],"intervention":{intervention_json}}}"#
        );
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client,
                channel: ident!("bong:client_request").into(),
                data: data.into_bytes().into_boxed_slice(),
            });
    }

    /// happy path：炉主对起炉中的丹炉干预生效 → 恰发一条 alchemy_stir 搅拌动画，
    /// target = 干预者本人 uuid、优先级战斗动作档。
    #[test]
    fn alchemy_intervention_emits_stir_animation_for_owner() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        let player_uuid = app
            .world()
            .get::<UniqueId>(entity)
            .expect("mock client should carry UniqueId")
            .0
            .to_string();
        spawn_azure_furnace_with_session(&mut app, "offline:Azure");

        send_alchemy_intervention_payload(&mut app, entity, r#"{"kind":"adjust_temp","temp":0.5}"#);
        app.update();

        let stirs = drain_alchemy_stir_anims(&mut app);
        assert_eq!(
            stirs.len(),
            1,
            "干预生效应恰发一条 alchemy_stir 搅拌动画，实际 {stirs:?}"
        );
        assert_eq!(
            stirs[0].0, player_uuid,
            "alchemy_stir 应发给干预者本人（target_player = 干预者 uuid）"
        );
        assert_eq!(
            stirs[0].1,
            crate::network::vfx_animation_trigger::COMBAT_PRIORITY,
            "alchemy_stir 优先级应为战斗动作档"
        );
    }

    /// 重复触发语义：每次干预生效各配一次搅拌动画（两次干预两动画，1:1 无去重）。
    #[test]
    fn each_alchemy_intervention_emits_its_own_stir() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        spawn_azure_furnace_with_session(&mut app, "offline:Azure");

        send_alchemy_intervention_payload(&mut app, entity, r#"{"kind":"adjust_temp","temp":0.5}"#);
        send_alchemy_intervention_payload(&mut app, entity, r#"{"kind":"inject_qi","qi":2.0}"#);
        app.update();

        assert_eq!(
            drain_alchemy_stir_anims(&mut app).len(),
            2,
            "每次干预生效各配一次 alchemy_stir（1:1）"
        );
    }

    /// enum 变体饱和：AutoProfile 是保留 no-op（`apply_intervention` 不改任何
    /// 状态、无真实搅拌动作），不发 alchemy_stir 动画。
    #[test]
    fn auto_profile_intervention_emits_no_stir_animation() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        spawn_azure_furnace_with_session(&mut app, "offline:Azure");

        send_alchemy_intervention_payload(
            &mut app,
            entity,
            r#"{"kind":"auto_profile","profile_id":"gentle"}"#,
        );
        app.update();

        assert!(
            drain_alchemy_stir_anims(&mut app).is_empty(),
            "AutoProfile 是保留 no-op 干预（不改炉温/真元），不应发 alchemy_stir 搅拌动画"
        );
    }

    /// 错误分支：尚未起炉（furnace 无 session）→ 干预被拒不发搅拌动画。
    #[test]
    fn alchemy_intervention_without_session_does_not_emit_stir() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        let mut furnace = AlchemyFurnace::placed(valence::prelude::BlockPos::new(8, 66, 8), 1);
        furnace.owner = Some("offline:Azure".into());
        app.world_mut().spawn(furnace);

        send_alchemy_intervention_payload(&mut app, entity, r#"{"kind":"adjust_temp","temp":0.5}"#);
        app.update();

        assert!(
            drain_alchemy_stir_anims(&mut app).is_empty(),
            "未起炉的干预被拒时不应发 alchemy_stir"
        );
    }

    /// 错误分支：非炉主干预他人丹炉 → 路由拒绝不发搅拌动画。
    #[test]
    fn alchemy_intervention_on_foreign_furnace_does_not_emit_stir() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        spawn_azure_furnace_with_session(&mut app, "offline:Bob");

        send_alchemy_intervention_payload(&mut app, entity, r#"{"kind":"adjust_temp","temp":0.5}"#);
        app.update();

        assert!(
            drain_alchemy_stir_anims(&mut app).is_empty(),
            "非炉主的干预被拒时不应发 alchemy_stir"
        );
    }

    /// 状态前置分支：坍缩 zone 内 inject_qi 被忽略（干预未生效）→ 不发搅拌动画。
    #[test]
    fn alchemy_inject_qi_in_collapsed_zone_does_not_emit_stir() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        let mut zones = ZoneRegistry::fallback();
        zones
            .find_zone_mut("spawn")
            .unwrap()
            .active_events
            .push(EVENT_REALM_COLLAPSE.to_string());
        app.insert_resource(zones);
        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        spawn_azure_furnace_with_session(&mut app, "offline:Azure");

        send_alchemy_intervention_payload(&mut app, entity, r#"{"kind":"inject_qi","qi":5.0}"#);
        app.update();

        assert!(
            drain_alchemy_stir_anims(&mut app).is_empty(),
            "坍缩 zone 内被忽略的 inject_qi 不应发 alchemy_stir（干预未生效）"
        );
    }

    #[test]
    fn alchemy_explode_tier_three_scales_backlash_above_tier_one() {
        let tier_one = scale_alchemy_explosion_damage(40.0, 1);
        let tier_three = scale_alchemy_explosion_damage(40.0, 3);

        assert!(tier_one > 0.0);
        assert!(tier_three > tier_one);
        assert_eq!(tier_three, 80.0);
        assert!(scale_alchemy_explosion_crack(0.3, 3) > scale_alchemy_explosion_crack(0.3, 1));
    }

    #[test]
    fn alchemy_explode_backlash_without_components_does_not_crash() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(crate::alchemy::recipe::load_recipe_registry().unwrap());
        app.insert_resource(crate::inventory::load_item_registry().unwrap());
        app.insert_resource(crate::inventory::InventoryInstanceIdAllocator::default());

        let (client_bundle, _helper) = create_mock_client("NpcLike");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut()
            .entity_mut(entity)
            .insert(inventory_with_stack("ci_she_hao", 3));
        let mut furnace = AlchemyFurnace::placed(valence::prelude::BlockPos::new(4, 64, 5), 1);
        furnace.owner = Some("offline:NpcLike".into());
        app.world_mut().spawn(furnace);
        for data in [
            br#"{"type":"alchemy_ignite","v":1,"furnace_pos":[4,64,5],"recipe_id":"kai_mai_pill_v0"}"#.as_slice(),
            br#"{"type":"alchemy_feed_slot","v":1,"furnace_pos":[4,64,5],"slot_idx":0,"material":"ci_she_hao","count":3}"#.as_slice(),
            br#"{"type":"alchemy_intervention","v":1,"furnace_pos":[4,64,5],"intervention":{"kind":"adjust_temp","temp":1.0}}"#.as_slice(),
            br#"{"type":"alchemy_take_back","v":1,"furnace_pos":[4,64,5],"slot_idx":0}"#.as_slice(),
        ] {
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: data.to_vec().into_boxed_slice(),
                });
        }

        app.update();

        assert!(app.world().get::<Wounds>(entity).is_none());
    }

    /// P3 端到端：高阶炉（tier=4）炼 tui_gu_dan_v1 → AlchemyTakeBack → bucket=Perfect；
    /// 低阶对照（tier=2，满足最低炉阶但无催化加成）→ bucket=Good。
    ///
    /// 覆盖边界：handler 中 `furnace.tier` 被透传至 `resolve_with_meta_and_furnace`，
    /// 确保"改 furnace tier → resolver 接收到正确 tier → 分桶变化"这条 wiring 不被悄悄断掉。
    /// （若改为传 0，tier-4 结果仍等于 tier-2 的 Good，测试立即红。）
    #[test]
    fn p3_take_back_high_tier_furnace_upgrades_bucket_vs_tier0_control() {
        // tui_gu_dan_v1: target_temp=0.70, temp_band=0.08, qi_cost=25.0, duration=200
        // temp=0.87 → over=0.17, score=(0.17/0.08 - 1.0)=1.125 → Good（无加成）
        // tier=4 催化炉加成 → score 下降到 ≤1.0 → Perfect
        use crate::alchemy::outcome::OutcomeBucket;

        fn build_tui_gu_dan_app() -> (App, valence::prelude::Entity, valence::prelude::Entity) {
            let mut app = App::new();
            app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
            register_request_app(&mut app);
            app.insert_resource(crate::alchemy::recipe::load_recipe_registry().unwrap());
            app.insert_resource(crate::inventory::load_item_registry().unwrap());
            app.insert_resource(crate::inventory::InventoryInstanceIdAllocator::default());

            let (client_bundle, _helper) = create_mock_client("Alchemist");
            let entity = app.world_mut().spawn(client_bundle).id();
            app.world_mut().entity_mut(entity).insert((
                crate::cultivation::components::Cultivation::default(),
                PlayerState::default(),
                // tui_gu_dan 需要 tui_gu_teng×2 + fauna.mutated_bone×1
                PlayerInventory {
                    triggered_treasures: Vec::new(),
                    revision: InventoryRevision(0),
                    containers: vec![ContainerState {
                        quick_access: false,
                        id: "main_pack".into(),
                        name: "main_pack".into(),
                        rows: 5,
                        cols: 7,
                        items: vec![
                            PlacedItemState {
                                row: 0,
                                col: 0,
                                instance: ItemInstance {
                                    instance_id: 9001,
                                    template_id: "tui_gu_teng".to_string(),
                                    display_name: "tui_gu_teng".to_string(),
                                    grid_w: 1,
                                    grid_h: 1,
                                    weight: 0.1,
                                    rarity: ItemRarity::Common,
                                    description: String::new(),
                                    stack_count: 2,
                                    spirit_quality: 1.0,
                                    durability: 1.0,
                                    freshness: None,
                                    mineral_id: None,
                                    charges: None,
                                    forge_quality: None,
                                    forge_color: None,
                                    forge_side_effects: Vec::new(),
                                    forge_achieved_tier: None,
                                    alchemy: None,
                                    lingering_owner_qi: None,
                                },
                            },
                            PlacedItemState {
                                row: 0,
                                col: 1,
                                instance: ItemInstance {
                                    instance_id: 9002,
                                    template_id: "fauna.mutated_bone".to_string(),
                                    display_name: "fauna.mutated_bone".to_string(),
                                    grid_w: 1,
                                    grid_h: 1,
                                    weight: 0.1,
                                    rarity: ItemRarity::Common,
                                    description: String::new(),
                                    stack_count: 1,
                                    spirit_quality: 1.0,
                                    durability: 1.0,
                                    freshness: None,
                                    mineral_id: None,
                                    charges: None,
                                    forge_quality: None,
                                    forge_color: None,
                                    forge_side_effects: Vec::new(),
                                    forge_achieved_tier: None,
                                    alchemy: None,
                                    lingering_owner_qi: None,
                                },
                            },
                        ],

                        owner_instance_id: None,
                    }],
                    equipped: Default::default(),
                    hotbar: Default::default(),
                    bone_coins: 0,
                    max_weight: 50.0,
                },
            ));
            let furnace_entity = app.world_mut().spawn_empty().id();
            (app, entity, furnace_entity)
        }

        fn run_tui_gu_dan_brew(
            app: &mut App,
            entity: valence::prelude::Entity,
            furnace_pos: [i32; 3],
            furnace_tier: u8,
        ) -> OutcomeBucket {
            // 注册炉体
            let mut furnace = AlchemyFurnace::placed(
                valence::prelude::BlockPos::new(furnace_pos[0], furnace_pos[1], furnace_pos[2]),
                furnace_tier,
            );
            furnace.owner = Some("offline:Alchemist".into());
            app.world_mut().spawn(furnace);

            let pos_json = format!("[{},{},{}]", furnace_pos[0], furnace_pos[1], furnace_pos[2]);
            let requests: Vec<String> = vec![
                format!(
                    r#"{{"type":"alchemy_ignite","v":1,"furnace_pos":{pos_json},"recipe_id":"tui_gu_dan_v1"}}"#
                ),
                // stage 0: tui_gu_teng×2
                format!(
                    r#"{{"type":"alchemy_feed_slot","v":1,"furnace_pos":{pos_json},"slot_idx":0,"material":"tui_gu_teng","count":2}}"#
                ),
                // stage 0: fauna.mutated_bone×1
                format!(
                    r#"{{"type":"alchemy_feed_slot","v":1,"furnace_pos":{pos_json},"slot_idx":0,"material":"fauna.mutated_bone","count":1}}"#
                ),
                // temp=0.87 → score=1.125 (Good without bonus; Perfect with tier-4 bonus)
                format!(
                    r#"{{"type":"alchemy_intervention","v":1,"furnace_pos":{pos_json},"intervention":{{"kind":"adjust_temp","temp":0.87}}}}"#
                ),
                // qi_cost=25.0 → inject full amount
                format!(
                    r#"{{"type":"alchemy_intervention","v":1,"furnace_pos":{pos_json},"intervention":{{"kind":"inject_qi","qi":25.0}}}}"#
                ),
                format!(
                    r#"{{"type":"alchemy_take_back","v":1,"furnace_pos":{pos_json},"slot_idx":0}}"#
                ),
            ];
            for req in &requests {
                app.world_mut()
                    .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                    .send(CustomPayloadEvent {
                        client: entity,
                        channel: ident!("bong:client_request").into(),
                        data: req.as_bytes().to_vec().into_boxed_slice(),
                    });
            }
            app.update();

            // 读取 AlchemyOutcomeEvent 中的 bucket
            let events = app
                .world()
                .resource::<valence::prelude::Events<crate::alchemy::AlchemyOutcomeEvent>>();
            let mut reader = events.get_reader();
            let evts: Vec<_> = reader.read(events).collect();
            assert!(
                !evts.is_empty(),
                "furnace_tier={furnace_tier}: AlchemyTakeBack 应产生 AlchemyOutcomeEvent，但未收到任何事件"
            );
            evts.last().unwrap().bucket
        }

        // --- tier=2 对照组（无加成 → Good） ---
        // tui_gu_dan_v1 的 furnace_tier_min=2，tier=2 满足最低炉阶要求但不触发催化加成。
        // catalyst_furnace_bonus 仅在 tier >= CATALYST_FURNACE_TIER(4) 时返回正值，
        // 故 tier=2 等价于"无加成"基线。
        let (mut app2, entity2, _) = build_tui_gu_dan_app();
        let bucket_tier2 = run_tui_gu_dan_brew(&mut app2, entity2, [10, 64, 10], 2);
        assert_eq!(
            bucket_tier2,
            OutcomeBucket::Good,
            "tier=2 炉 + tui_gu_dan_v1(temp=0.87) 应为 Good（无催化加成），实际 {:?}。\
             若非 Good，说明 session 参数或配方数据发生变化，需更新测试基线。",
            bucket_tier2
        );

        // --- tier=4 高阶炉（催化加成 → Perfect） ---
        let (mut app4, entity4, _) = build_tui_gu_dan_app();
        let bucket_tier4 = run_tui_gu_dan_brew(&mut app4, entity4, [20, 64, 20], 4);
        assert_eq!(
            bucket_tier4,
            OutcomeBucket::Perfect,
            "tier=4 炉 + 变异丹 tui_gu_dan_v1(temp=0.87) 应升格到 Perfect，实际 {:?}。\
             若仍是 Good，说明 handle_alchemy_take_back 未将 furnace.tier 透传给 resolver（wiring 断裂）。",
            bucket_tier4
        );

        // 核心断言：高阶炉结果优于低阶炉，证明 furnace.tier wiring 有效
        assert_ne!(
            bucket_tier4, bucket_tier2,
            "tier=4 与 tier=2 的结果应不同（前者 Perfect，后者 Good），\
             若相同说明 furnace.tier 没有被传入 resolver"
        );
    }

    #[test]
    fn unsupported_client_request_version_is_ignored_without_side_effects() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        app.insert_resource(TechniqueRegistry::load_for_tests());
        app.insert_resource(CapturedBreakthroughRequests::default());
        app.insert_resource(CapturedForgeRequests::default());
        app.insert_resource(CapturedInsightChoices::default());
        app.insert_resource(CombatClock::default());
        app.insert_resource(GameplayActionQueue::default());
        app.insert_resource(AlchemyMockState::default());
        app.insert_resource(DroppedLootRegistry::default());
        // plan-remains-suite P0 — DroppedLootRequestParams 新增 EventWriter<RemainsLootIntent>。
        app.add_event::<crate::inventory::RemainsLootIntent>();
        app.insert_resource(ItemRegistry::default());
        app.insert_resource(RecipeRegistry::default());
        app.add_event::<CustomPayloadEvent>();
        app.add_event::<BreakthroughRequest>();
        app.add_event::<ForgeRequest>();
        app.add_event::<InsightChosen>();
        app.add_event::<DefenseIntent>();
        app.add_event::<RevivalActionIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<PlaceFurnaceRequest>();
        app.add_event::<crate::alchemy::LearnRecipeFragmentIntent>();
        app.add_event::<StartTillRequest>();
        app.add_event::<StartRenewRequest>();
        app.add_event::<StartPlantingRequest>();
        app.add_event::<StartHarvestRequest>();
        app.add_event::<StartReplenishRequest>();
        app.add_event::<StartDrainQiRequest>();
        app.add_event::<StartExtractRequestEvent>();
        app.add_event::<CancelExtractRequestEvent>();
        app.add_event::<MineralProbeIntent>();
        app.add_event::<SkillXpGain>();
        app.add_event::<SkillScrollUsed>();
        // plan-shield-block-v1 P1 — 举盾 intent events（ClientRequestDispatchParams 需要）。
        app.add_event::<crate::combat::shield_block::RaiseShieldIntent>();
        app.add_event::<crate::combat::shield_block::LowerShieldIntent>();
        app.add_event::<crate::network::agent_ui::AgentUiResponseEvent>();
        app.add_systems(
            Update,
            (
                handle_client_request_payloads,
                capture_breakthrough_requests,
                capture_forge_requests,
                capture_insight_choices,
            )
                .chain(),
        );

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"breakthrough_request","v":99}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

        app.update();

        assert!(
            app.world().get::<MeridianTarget>(entity).is_none(),
            "unsupported request version should not attach MeridianTarget"
        );
        assert!(
            app.world()
                .resource::<CapturedBreakthroughRequests>()
                .0
                .is_empty(),
            "unsupported request version should not emit BreakthroughRequest"
        );
        assert!(
            app.world().resource::<CapturedForgeRequests>().0.is_empty(),
            "unsupported request version should not emit ForgeRequest"
        );
        assert!(
            app.world()
                .resource::<CapturedInsightChoices>()
                .0
                .is_empty(),
            "unsupported request version should not emit InsightChosen"
        );
    }

    #[test]
    fn ingress_budget_rejects_33rd_same_tick_before_decode_or_dispatch() {
        CLIENT_REQUEST_DECODE_COUNT.store(0, Ordering::Relaxed);

        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(CapturedBreakthroughRequests::default());
        app.add_systems(
            Update,
            capture_breakthrough_requests.after(handle_client_request_payloads),
        );

        let (client_bundle, _helper) = create_mock_client("BudgetIngress");
        let client = app.world_mut().spawn(client_bundle).id();
        for _ in 0..33 {
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client,
                    channel: ident!("bong:client_request").into(),
                    data: format!(
                        "{}{{\"type\":\"breakthrough_request\",\"v\":1}}",
                        "\n".repeat(128)
                    )
                    .into_bytes()
                    .into_boxed_slice(),
                });
        }

        app.update();

        assert_eq!(
            CLIENT_REQUEST_DECODE_COUNT.load(Ordering::Relaxed),
            32,
            "the 33rd same-tick payload must be rejected before JSON decode"
        );
        assert_eq!(
            app.world()
                .resource::<CapturedBreakthroughRequests>()
                .0
                .len(),
            32,
            "the 33rd same-tick payload must not dispatch a handler event"
        );
    }

    #[test]
    fn ingress_budget_clears_bucket_when_character_role_changes() {
        let mut app = App::new();
        app.init_resource::<ClientRequestBudget>();
        app.add_systems(Update, cleanup_client_request_budget);

        let (client_bundle, _helper) = create_mock_client("RoleSwitch");
        let client = app
            .world_mut()
            .spawn((
                client_bundle,
                Lifecycle {
                    character_id: "character-a".to_string(),
                    ..Lifecycle::default()
                },
            ))
            .id();
        app.update();

        {
            let mut budget = app.world_mut().resource_mut::<ClientRequestBudget>();
            for _ in 0..32 {
                assert!(budget.store.admit_ingress(client, 0).admitted);
            }
            assert_eq!(budget.store.tokens_for(&client), Some(0));
        }

        app.world_mut()
            .get_mut::<Lifecycle>(client)
            .expect("connected client must retain lifecycle")
            .character_id = "character-b".to_string();
        app.update();

        let budget = app.world().resource::<ClientRequestBudget>();
        assert_eq!(
            budget.store.tokens_for(&client),
            None,
            "role switch must discard the old entity bucket before the next ingress"
        );
        assert_eq!(
            budget.character_ids.get(&client).map(String::as_str),
            Some("character-b")
        );
        assert!(
            app.world_mut()
                .resource_mut::<ClientRequestBudget>()
                .store
                .admit_ingress(client, 0)
                .admitted,
            "a switched role must receive a clean 32-token bucket"
        );
    }

    #[test]
    fn ingress_budget_clears_bucket_when_client_disconnects() {
        let mut app = App::new();
        app.init_resource::<ClientRequestBudget>();
        app.add_systems(Update, cleanup_client_request_budget);

        let (client_bundle, _helper) = create_mock_client("Disconnect");
        let client = app
            .world_mut()
            .spawn((
                client_bundle,
                Lifecycle {
                    character_id: "character-a".to_string(),
                    ..Lifecycle::default()
                },
            ))
            .id();
        app.update();
        app.world_mut()
            .resource_mut::<ClientRequestBudget>()
            .store
            .admit_ingress(client, 0);
        assert!(app
            .world()
            .resource::<ClientRequestBudget>()
            .store
            .contains_client(&client));

        app.world_mut().despawn(client);
        app.update();

        let budget = app.world().resource::<ClientRequestBudget>();
        assert!(!budget.store.contains_client(&client));
        assert!(!budget.character_ids.contains_key(&client));
    }

    #[test]
    fn botany_harvest_request_updates_existing_session_without_gather_enqueue() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(HarvestSessionStore::default());

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        upsert_test_harvest_session(
            &mut app,
            "offline:Azure",
            entity,
            BotanyHarvestMode::Manual,
            10,
            0.5,
        );
        app.world_mut()
            .resource_mut::<GameplayActionQueue>()
            .enqueue(
                "offline:Other",
                crate::player::gameplay::GameplayAction::AttemptBreakthrough,
            );

        send_botany_harvest_request(&mut app, entity, "offline:Azure", "auto");

        app.update();

        let store = app.world().resource::<HarvestSessionStore>();
        let session = store.session_for("offline:Azure").unwrap();
        assert_eq!(session.mode, BotanyHarvestMode::Auto);
        assert_eq!(
            session.duration_ticks,
            harvest_duration_ticks_for(BotanyHarvestMode::Auto)
        );
        assert_eq!(session.started_at_tick, 0);
        assert_eq!(session.last_progress, 0.0);
        assert_eq!(session.phase, BotanyPhase::InProgress);

        let pending = app
            .world()
            .resource::<GameplayActionQueue>()
            .pending_actions_snapshot();
        assert_eq!(
            pending.len(),
            1,
            "botany_harvest_request must not enqueue a legacy Gather action"
        );
        assert!(matches!(
            pending[0].action,
            crate::player::gameplay::GameplayAction::AttemptBreakthrough
        ));
    }

    #[test]
    fn botany_harvest_request_rejects_missing_session_without_gather_enqueue() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(HarvestSessionStore::default());

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();

        send_botany_harvest_request(&mut app, entity, "expired-session-token", "auto");

        app.update();

        assert!(
            app.world()
                .resource::<HarvestSessionStore>()
                .session_for("expired-session-token")
                .is_none(),
            "invalid botany session_id must not create a harvest session"
        );
        assert!(
            app.world()
                .resource::<GameplayActionQueue>()
                .pending_actions_snapshot()
                .is_empty(),
            "invalid botany session_id must not be rerouted into legacy Gather"
        );
    }

    #[test]
    fn botany_harvest_request_rejects_different_client_session_without_mutation() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(HarvestSessionStore::default());

        let (azure_bundle, _azure_helper) = create_mock_client("Azure");
        let azure = app.world_mut().spawn(azure_bundle).id();
        let (crimson_bundle, _crimson_helper) = create_mock_client("Crimson");
        let crimson = app.world_mut().spawn(crimson_bundle).id();
        upsert_test_harvest_session(
            &mut app,
            "offline:Azure",
            azure,
            BotanyHarvestMode::Manual,
            10,
            0.5,
        );

        send_botany_harvest_request(&mut app, crimson, "offline:Azure", "auto");

        app.update();

        let store = app.world().resource::<HarvestSessionStore>();
        let session = store.session_for("offline:Azure").unwrap();
        assert_eq!(
            session.mode,
            BotanyHarvestMode::Manual,
            "cross-client mode request must not mutate another player's session"
        );
        assert_eq!(session.started_at_tick, 10);
        assert_eq!(
            session.duration_ticks,
            harvest_duration_ticks_for(BotanyHarvestMode::Manual)
        );
        assert_eq!(session.last_progress, 0.5);
        assert!(
            app.world()
                .resource::<GameplayActionQueue>()
                .pending_actions_snapshot()
                .is_empty(),
            "rejected cross-client request must not enqueue legacy Gather"
        );
    }

    #[test]
    fn botany_harvest_request_invalid_session_does_not_grant_gather_rewards() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(HarvestSessionStore::default());
        app.insert_resource(GameplayTick::default());
        app.insert_resource(crate::player::gameplay::PendingGameplayNarrations::default());
        app.insert_resource(crate::qi_physics::WorldQiAccount::default());
        app.add_systems(
            Update,
            crate::player::gameplay::apply_queued_gameplay_actions
                .after(handle_client_request_payloads),
        );

        let initial_state = PlayerState {
            karma: 0.12,
            inventory_score: 0.34,
        };
        let initial_qi = 20.0;
        let (mut client_bundle, _helper) = create_mock_client("Azure");
        client_bundle.player.position = Position::new([8.0, 66.0, 8.0]);
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                initial_state.clone(),
                Cultivation {
                    qi_current: initial_qi,
                    qi_max: 100.0,
                    ..Cultivation::default()
                },
            ))
            .id();
        let zone_qi_before = app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
            .expect("fallback spawn zone should exist")
            .spirit_qi;

        send_botany_harvest_request(&mut app, entity, "expired-session-token", "auto");

        app.update();

        let player_state = app
            .world()
            .entity(entity)
            .get::<PlayerState>()
            .expect("player state should remain attached");
        assert_eq!(
            player_state, &initial_state,
            "invalid mode request must not mutate karma or inventory_score via Gather"
        );
        let cultivation = app
            .world()
            .entity(entity)
            .get::<Cultivation>()
            .expect("cultivation should remain attached");
        assert_eq!(
            cultivation.qi_current, initial_qi,
            "invalid mode request must not drain zone qi into the player"
        );
        let zone_qi_after = app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
            .expect("fallback spawn zone should still exist")
            .spirit_qi;
        assert_eq!(
            zone_qi_after, zone_qi_before,
            "invalid mode request must not mutate zone spirit_qi"
        );
        assert!(
            app.world()
                .resource::<crate::qi_physics::WorldQiAccount>()
                .transfers()
                .is_empty(),
            "invalid mode request must not append gather qi audit transfers"
        );
        let narrations = app
            .world_mut()
            .resource_mut::<crate::player::gameplay::PendingGameplayNarrations>()
            .drain();
        assert!(
            narrations.is_empty(),
            "invalid mode request must not emit legacy gather narration"
        );
    }

    #[test]
    fn abort_tribulation_request_is_ignored_after_start_confirmation() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(CapturedStartDuXuRequests::default());
        app.add_event::<StartDuXuRequest>();
        app.add_systems(
            Update,
            capture_start_du_xu_requests.after(handle_client_request_payloads),
        );

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"start_du_xu","v":1}"#.to_vec().into_boxed_slice(),
            });

        app.update();

        assert_eq!(
            app.world().resource::<CapturedStartDuXuRequests>().0.len(),
            1,
            "control start_du_xu request should emit StartDuXuRequest"
        );

        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"abort_tribulation","v":1}"#.to_vec().into_boxed_slice(),
            });

        app.update();

        assert_eq!(
            app.world().resource::<CapturedStartDuXuRequests>().0.len(),
            1,
            "abort_tribulation must not emit another StartDuXuRequest or cancellation side effect"
        );
    }

    #[test]
    fn movement_action_request_emits_intent_when_event_resource_exists() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.add_event::<MovementActionIntent>();

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"movement_action","v":1,"action":"dash"}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

        app.update();

        let events = app
            .world()
            .resource::<valence::prelude::Events<MovementActionIntent>>();
        let intents: Vec<_> = events.iter_current_update_events().collect();
        assert_eq!(
            intents.len(),
            1,
            "expected one MovementActionIntent because one valid movement_action payload was sent, actual: {}",
            intents.len()
        );
        assert_eq!(
            intents[0].entity, entity,
            "expected MovementActionIntent entity to match the sending client"
        );
        assert_eq!(
            intents[0].action,
            MovementAction::Dashing,
            "expected movement_action dash payload to map to MovementAction::Dashing"
        );
        assert_eq!(
            intents[0].yaw_degrees, None,
            "expected missing yaw_degrees to stay None for legacy movement_action payloads"
        );
    }

    #[test]
    fn movement_action_request_emits_client_yaw_when_present() {
        assert_movement_action_yaw_forwarded(90.5);
    }

    #[test]
    fn movement_action_request_accepts_yaw_boundaries() {
        for yaw_degrees in [0.0, 360.0, -45.0, 359.999] {
            assert_movement_action_yaw_forwarded(yaw_degrees);
        }
    }

    #[test]
    fn movement_action_request_rejects_non_numeric_yaw_degrees() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.add_event::<MovementActionIntent>();

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"movement_action","v":1,"action":"dash","yaw_degrees":"east"}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

        app.update();

        let events = app
            .world()
            .resource::<valence::prelude::Events<MovementActionIntent>>();
        let intents: Vec<_> = events.iter_current_update_events().collect();
        assert!(
            intents.is_empty(),
            "expected no MovementActionIntent because yaw_degrees had invalid JSON type, actual: {}",
            intents.len()
        );
    }

    #[test]
    fn movement_action_request_without_event_resource_is_dropped() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"movement_action","v":1,"action":"dash"}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

        app.update();

        assert!(app
            .world()
            .get_resource::<valence::prelude::Events<MovementActionIntent>>()
            .is_none());
    }

    #[test]
    fn use_quick_slot_reads_template_from_equipped_instance() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(ItemRegistry::from_map(HashMap::from([(
            "bone_whistle".to_string(),
            ItemTemplate {
                id: "bone_whistle".to_string(),
                display_name: "骨哨".to_string(),
                category: ItemCategory::Misc,
                placeable: None,
                max_stack_count: 1,
                grid_w: 1,
                grid_h: 1,
                base_weight: 0.1,
                rarity: ItemRarity::Common,
                spirit_quality_initial: 1.0,
                description: String::new(),
                effect: None,
                cast_duration_ms: 250,
                cooldown_ms: 450,
                weapon_spec: None,
                forge_station_spec: None,
                blueprint_scroll_spec: None,
                inscription_scroll_spec: None,
                technique_scroll_spec: None,
                readable_scroll_spec: None,
                recipe_fragment_spec: None,
                container_spec: None,
                shelflife_profile: None,
                shield_spec: None,
                shelflife_track: None,
                wearer_race: crate::body_plan::types::RaceGateOwned::default(),
            },
        )])));

        let mut inventory = empty_inventory();
        inventory.equipped.insert(
            crate::inventory::EQUIP_SLOT_OFF_HAND.to_string(),
            crate::inventory::SlotContents::held_single(inventory_test_item(77, "bone_whistle", 1)),
        );
        let mut quick_slots = QuickSlotBindings::default();
        assert!(quick_slots.set(0, Some(77)));

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((client_bundle, quick_slots, inventory))
            .id();
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"use_quick_slot","v":1,"slot":0}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

        app.update();

        let casting = app
            .world()
            .get::<Casting>(entity)
            .expect("equipped quick slot item should start casting");
        assert_eq!(casting.bound_instance_id, Some(77));
        assert_eq!(casting.duration_ms, 250);
        assert_eq!(casting.duration_ticks, 5);
        assert_eq!(casting.complete_cooldown_ticks, 9);
    }

    #[test]
    fn use_quick_slot_unbound_slot_preserves_active_cross_slot_cast() {
        // central-review 2012 #1 回归：未绑定槽 use 必须静默忽略且**不得打断**
        // 进行中的异槽 cast。旧实现先走 cast 闸门（异槽 → cancel_previous_cast 发
        // cast_sync{Interrupt, UserCancel} 并 remove Casting），再发现槽 5 无绑定
        // 才返回——活动 cast 被无谓取消。契约（network_quickslot_config.py docstring：
        // 无绑定 → 静默忽略）下无绑定请求是无副作用的 no-op。
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(ItemRegistry::from_map(HashMap::from([(
            "guyuan_pill".to_string(),
            ItemTemplate {
                id: "guyuan_pill".to_string(),
                display_name: "guyuan_pill".to_string(),
                category: ItemCategory::Misc,
                placeable: None,
                max_stack_count: 64,
                grid_w: 1,
                grid_h: 1,
                base_weight: 0.1,
                rarity: ItemRarity::Common,
                spirit_quality_initial: 1.0,
                description: String::new(),
                effect: None,
                cast_duration_ms: 1500,
                cooldown_ms: 1500,
                weapon_spec: None,
                forge_station_spec: None,
                blueprint_scroll_spec: None,
                inscription_scroll_spec: None,
                technique_scroll_spec: None,
                readable_scroll_spec: None,
                recipe_fragment_spec: None,
                container_spec: None,
                shelflife_profile: None,
                shield_spec: None,
                shelflife_track: None,
                wearer_race: crate::body_plan::types::RaceGateOwned::default(),
            },
        )])));
        let mut inventory = empty_inventory();
        inventory.equipped.insert(
            crate::inventory::EQUIP_SLOT_MAIN_HAND.to_string(),
            crate::inventory::SlotContents::held_single(inventory_test_item(77, "guyuan_pill", 1)),
        );
        let mut quick_slots = QuickSlotBindings::default();
        assert!(quick_slots.set(0, Some(77)));
        let (client_bundle, mut helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((client_bundle, quick_slots, inventory))
            .id();

        // 请求 1：启动 slot 0 cast。
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"use_quick_slot","v":1,"slot":0}"#
                    .to_vec()
                    .into_boxed_slice(),
            });
        app.update();
        assert!(
            app.world().get::<Casting>(entity).is_some(),
            "前置：slot 0 应处于 casting 状态"
        );

        // 请求 2：slot 0 仍在 cast 时使用未绑定槽 5。
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"use_quick_slot","v":1,"slot":5}"#
                    .to_vec()
                    .into_boxed_slice(),
            });
        app.update();
        flush_all_client_packets(&mut app);

        // 活动 cast 必须原样保留（未被打断）。
        let casting = app
            .world()
            .get::<Casting>(entity)
            .expect("未绑定槽 use 不得取消进行中的 slot 0 cast");
        assert_eq!(casting.slot, 0);
        // 且不得下发 slot 0 的 Interrupt（UserCancel）cast_sync。
        let syncs = collect_cast_syncs(&mut helper);
        assert!(
            !syncs.iter().any(|s| s.phase == CastPhaseV1::Interrupt),
            "未绑定槽 use 不得产生任何 interrupt cast_sync，实际 {syncs:?}"
        );
    }

    #[test]
    fn use_quick_slot_on_cooldown_slot_preserves_active_cross_slot_cast() {
        // central-review 2012 #4 回归：handler 把「冷却未到期」早返回移到 cast 闸门
        // 之前——旧顺序下用冷却中的异槽会先 cancel_previous_cast（发
        // cast_sync{Interrupt, UserCancel} 并 remove Casting）再返回，活动 cast 被
        // 无谓打断。此前只有未绑定分支有测试，冷却分支完全没保护。本测试在 slot 0
        // 进行 cast 时 use 冷却中的 slot 5，断言 slot 0 cast 原样保留、无 interrupt。
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(ItemRegistry::from_map(HashMap::from([(
            "guyuan_pill".to_string(),
            ItemTemplate {
                id: "guyuan_pill".to_string(),
                display_name: "guyuan_pill".to_string(),
                category: ItemCategory::Misc,
                placeable: None,
                max_stack_count: 64,
                grid_w: 1,
                grid_h: 1,
                base_weight: 0.1,
                rarity: ItemRarity::Common,
                spirit_quality_initial: 1.0,
                description: String::new(),
                effect: None,
                cast_duration_ms: 1500,
                cooldown_ms: 1500,
                weapon_spec: None,
                forge_station_spec: None,
                blueprint_scroll_spec: None,
                inscription_scroll_spec: None,
                technique_scroll_spec: None,
                readable_scroll_spec: None,
                recipe_fragment_spec: None,
                container_spec: None,
                shelflife_profile: None,
                shield_spec: None,
                shelflife_track: None,
                wearer_race: crate::body_plan::types::RaceGateOwned::default(),
            },
        )])));
        let mut inventory = empty_inventory();
        inventory.equipped.insert(
            crate::inventory::EQUIP_SLOT_MAIN_HAND.to_string(),
            crate::inventory::SlotContents::held_single(inventory_test_item(77, "guyuan_pill", 1)),
        );
        let mut quick_slots = QuickSlotBindings::default();
        assert!(quick_slots.set(0, Some(77)));
        // slot 5 绑定同实例但处于冷却中（until_tick 设到远离默认 tick 0 的 u64::MAX）。
        assert!(quick_slots.set(5, Some(77)));
        quick_slots.set_cooldown(5, u64::MAX);
        let (client_bundle, mut helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((client_bundle, quick_slots, inventory))
            .id();

        // 请求 1：启动 slot 0 cast。
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"use_quick_slot","v":1,"slot":0}"#
                    .to_vec()
                    .into_boxed_slice(),
            });
        app.update();
        assert!(
            app.world().get::<Casting>(entity).is_some(),
            "前置：slot 0 应处于 casting 状态"
        );

        // 请求 2：slot 0 仍在 cast 时使用冷却中的 slot 5。
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"use_quick_slot","v":1,"slot":5}"#
                    .to_vec()
                    .into_boxed_slice(),
            });
        app.update();
        flush_all_client_packets(&mut app);

        let casting = app
            .world()
            .get::<Casting>(entity)
            .expect("冷却中的异槽 use 不得取消进行中的 slot 0 cast");
        assert_eq!(casting.slot, 0);
        let syncs = collect_cast_syncs(&mut helper);
        assert!(
            !syncs.iter().any(|s| s.phase == CastPhaseV1::Interrupt),
            "冷却中的异槽 use 不得产生任何 interrupt cast_sync，实际 {syncs:?}"
        );
    }

    #[test]
    fn use_quick_slot_stale_binding_missing_instance_preserves_active_cross_slot_cast() {
        // central-review 2012 #4 回归：绑定实例已不在背包（player 拖出去了）时 use
        // 必须静默忽略且不得打断进行中的异槽 cast。此前没有任何测试构造陈旧绑定
        // 覆盖 missing-instance 早返回分支——旧顺序把它放回 cast 闸门之后，用失效
        // 绑定的异槽会在活动 cast 期间先 cancel_previous_cast 再返回。本测试在
        // slot 0 进行 cast 时 use 绑定已失效实例（999，不在背包）的 slot 5，断言
        // slot 0 cast 原样保留、无 interrupt。
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(ItemRegistry::from_map(HashMap::from([(
            "guyuan_pill".to_string(),
            ItemTemplate {
                id: "guyuan_pill".to_string(),
                display_name: "guyuan_pill".to_string(),
                category: ItemCategory::Misc,
                placeable: None,
                max_stack_count: 64,
                grid_w: 1,
                grid_h: 1,
                base_weight: 0.1,
                rarity: ItemRarity::Common,
                spirit_quality_initial: 1.0,
                description: String::new(),
                effect: None,
                cast_duration_ms: 1500,
                cooldown_ms: 1500,
                weapon_spec: None,
                forge_station_spec: None,
                blueprint_scroll_spec: None,
                inscription_scroll_spec: None,
                technique_scroll_spec: None,
                readable_scroll_spec: None,
                recipe_fragment_spec: None,
                container_spec: None,
                shelflife_profile: None,
                shield_spec: None,
                shelflife_track: None,
                wearer_race: crate::body_plan::types::RaceGateOwned::default(),
            },
        )])));
        let mut inventory = empty_inventory();
        inventory.equipped.insert(
            crate::inventory::EQUIP_SLOT_MAIN_HAND.to_string(),
            crate::inventory::SlotContents::held_single(inventory_test_item(77, "guyuan_pill", 1)),
        );
        let mut quick_slots = QuickSlotBindings::default();
        assert!(quick_slots.set(0, Some(77)));
        // slot 5 绑定陈旧实例 999（不在背包），且不在冷却——恰好命中 missing-instance
        // 早返回分支（越过 cooldown 与 unbound 两个更靠前的检查）。
        assert!(quick_slots.set(5, Some(999)));
        let (client_bundle, mut helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((client_bundle, quick_slots, inventory))
            .id();

        // 请求 1：启动 slot 0 cast。
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"use_quick_slot","v":1,"slot":0}"#
                    .to_vec()
                    .into_boxed_slice(),
            });
        app.update();
        assert!(
            app.world().get::<Casting>(entity).is_some(),
            "前置：slot 0 应处于 casting 状态"
        );

        // 请求 2：slot 0 仍在 cast 时使用绑定失效实例的 slot 5。
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"use_quick_slot","v":1,"slot":5}"#
                    .to_vec()
                    .into_boxed_slice(),
            });
        app.update();
        flush_all_client_packets(&mut app);

        let casting = app
            .world()
            .get::<Casting>(entity)
            .expect("陈旧绑定（实例不在背包）use 不得取消进行中的 slot 0 cast");
        assert_eq!(casting.slot, 0);
        let syncs = collect_cast_syncs(&mut helper);
        assert!(
            !syncs.iter().any(|s| s.phase == CastPhaseV1::Interrupt),
            "陈旧绑定 use 不得产生任何 interrupt cast_sync，实际 {syncs:?}"
        );
    }

    #[test]
    fn quick_slot_bind_resolves_equipped_template_instance() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(crate::inventory::load_item_registry().expect("item registry loads"));

        let mut inventory = empty_inventory();
        inventory.equipped.insert(
            crate::inventory::EQUIP_SLOT_OFF_HAND.to_string(),
            crate::inventory::SlotContents::held_single(inventory_test_item(77, "earth_crumb", 1)),
        );

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                QuickSlotBindings::default(),
                SkillBarBindings::default(),
                inventory,
            ))
            .id();
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"quick_slot_bind","v":1,"slot":0,"item_id":"earth_crumb","request_id":"bind-equipped"}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

        app.update();

        let bindings = app
            .world()
            .get::<QuickSlotBindings>(entity)
            .expect("player should keep quick slot bindings");
        assert_eq!(
            bindings.get(0),
            Some(77),
            "quick_slot_bind must resolve template ids from equipped held/worn items"
        );
    }

    #[test]
    fn quick_slot_bind_atomically_mirrors_block_item_into_skill_bar() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(crate::inventory::load_item_registry().expect("item registry loads"));

        let inventory = inventory_with_item(inventory_test_item(88, "earth_crumb", 1));
        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                QuickSlotBindings::default(),
                SkillBarBindings::default(),
                inventory,
            ))
            .id();
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"quick_slot_bind","v":1,"slot":3,"item_id":"earth_crumb","request_id":"bind-block"}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

        app.update();

        let quick = app
            .world()
            .get::<QuickSlotBindings>(entity)
            .expect("player should keep quick slot bindings");
        assert_eq!(
            quick.get(3),
            Some(88),
            "expected block quick-slot intent to bind instance 88, actual {:?}",
            quick.get(3)
        );
        let skillbar = app
            .world()
            .get::<SkillBarBindings>(entity)
            .expect("player should keep skill bar bindings");
        assert_eq!(
            skillbar.get(3),
            Some(&SkillSlot::Item { instance_id: 88 }),
            "expected the same server intent to atomically mirror the block into skill bar"
        );
    }

    #[test]
    fn quick_slot_bind_rejects_unheld_item_without_mutating_or_persisting() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(crate::inventory::load_item_registry().expect("item registry loads"));
        let mut quick = QuickSlotBindings::default();
        let _ = quick.set(3, Some(77));
        let mut skillbar = SkillBarBindings::default();
        let _ = skillbar.set(3, SkillSlot::Item { instance_id: 77 });
        let (client_bundle, mut helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((client_bundle, quick, skillbar, empty_inventory()))
            .id();

        send_quick_slot_bind_request(&mut app, entity, 3, Some("earth_crumb"), "reject-unheld");
        app.update();
        flush_all_client_packets(&mut app);

        assert_eq!(
            app.world().get::<QuickSlotBindings>(entity).unwrap().get(3),
            Some(77)
        );
        assert_eq!(
            app.world().get::<SkillBarBindings>(entity).unwrap().get(3),
            Some(&SkillSlot::Item { instance_id: 77 })
        );
        let configs = collect_quickslot_configs(&mut helper);
        assert!(configs.iter().any(|config| {
            config.ack_request_id.as_deref() == Some("reject-unheld")
                && config.bind_accepted == Some(false)
        }));
    }

    #[test]
    fn quick_slot_bind_missing_skillbar_rejects_before_quick_slot_mutation() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(crate::inventory::load_item_registry().expect("item registry loads"));
        let inventory = inventory_with_item(inventory_test_item(88, "earth_crumb", 1));
        let (client_bundle, mut helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((client_bundle, QuickSlotBindings::default(), inventory))
            .id();

        send_quick_slot_bind_request(
            &mut app,
            entity,
            3,
            Some("earth_crumb"),
            "reject-missing-skillbar",
        );
        app.update();
        flush_all_client_packets(&mut app);

        assert_eq!(
            app.world().get::<QuickSlotBindings>(entity).unwrap().get(3),
            None
        );
        let configs = collect_quickslot_configs(&mut helper);
        assert!(configs.iter().any(|config| {
            config.ack_request_id.as_deref() == Some("reject-missing-skillbar")
                && config.bind_accepted == Some(false)
        }));
    }

    #[test]
    fn quick_slot_bind_clears_only_the_old_auto_mirrored_item() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(crate::inventory::load_item_registry().expect("item registry loads"));
        let mut inventory = inventory_with_item(inventory_test_item(88, "earth_crumb", 1));
        inventory.hotbar[0] = Some(inventory_test_item(89, "guyuan_pill", 1));
        let mut quick = QuickSlotBindings::default();
        let _ = quick.set(3, Some(88));
        let mut skillbar = SkillBarBindings::default();
        let _ = skillbar.set(3, SkillSlot::Item { instance_id: 88 });
        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((client_bundle, quick, skillbar, inventory))
            .id();

        send_quick_slot_bind_request(&mut app, entity, 3, Some("guyuan_pill"), "block-to-pill");
        app.update();

        assert_eq!(
            app.world().get::<QuickSlotBindings>(entity).unwrap().get(3),
            Some(89)
        );
        assert_eq!(
            app.world().get::<SkillBarBindings>(entity).unwrap().get(3),
            Some(&SkillSlot::Empty),
            "expected block→non-block to clear only the stale automatic item mirror"
        );

        {
            let mut quick = app
                .world_mut()
                .get_mut::<QuickSlotBindings>(entity)
                .unwrap();
            let _ = quick.set(3, Some(88));
        }
        {
            let mut skillbar = app.world_mut().get_mut::<SkillBarBindings>(entity).unwrap();
            let _ = skillbar.set(3, SkillSlot::Item { instance_id: 88 });
        }
        send_quick_slot_bind_request(&mut app, entity, 3, None, "block-to-clear");
        app.update();
        assert_eq!(
            app.world().get::<QuickSlotBindings>(entity).unwrap().get(3),
            None
        );
        assert_eq!(
            app.world().get::<SkillBarBindings>(entity).unwrap().get(3),
            Some(&SkillSlot::Empty),
            "expected block→clear to remove the matching automatic item mirror"
        );

        {
            let mut quick = app
                .world_mut()
                .get_mut::<QuickSlotBindings>(entity)
                .unwrap();
            let _ = quick.set(3, Some(88));
        }
        {
            let mut skillbar = app.world_mut().get_mut::<SkillBarBindings>(entity).unwrap();
            let _ = skillbar.set(
                3,
                SkillSlot::Skill {
                    skill_id: "sword.cleave".to_string(),
                },
            );
        }
        send_quick_slot_bind_request(&mut app, entity, 3, None, "protect-independent-skill");
        app.update();

        assert_eq!(
            app.world().get::<QuickSlotBindings>(entity).unwrap().get(3),
            None
        );
        assert_eq!(
            app.world().get::<SkillBarBindings>(entity).unwrap().get(3),
            Some(&SkillSlot::Skill {
                skill_id: "sword.cleave".to_string()
            }),
            "expected clearing quick slot not to overwrite a later independent skill binding"
        );
    }

    #[test]
    fn quick_slot_bind_persistence_failure_leaves_both_components_unchanged() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(crate::inventory::load_item_registry().expect("item registry loads"));
        let invalid_db_path = std::env::temp_dir();
        app.insert_resource(PlayerStatePersistence::with_db_path(
            std::env::temp_dir(),
            invalid_db_path,
        ));
        let inventory = inventory_with_item(inventory_test_item(88, "earth_crumb", 1));
        let (client_bundle, mut helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                QuickSlotBindings::default(),
                SkillBarBindings::default(),
                inventory,
            ))
            .id();

        send_quick_slot_bind_request(
            &mut app,
            entity,
            3,
            Some("earth_crumb"),
            "reject-persistence",
        );
        app.update();
        flush_all_client_packets(&mut app);

        assert_eq!(
            app.world().get::<QuickSlotBindings>(entity).unwrap().get(3),
            None
        );
        assert_eq!(
            app.world().get::<SkillBarBindings>(entity).unwrap().get(3),
            Some(&SkillSlot::Empty)
        );
        assert!(collect_quickslot_configs(&mut helper).iter().any(|config| {
            config.ack_request_id.as_deref() == Some("reject-persistence")
                && config.bind_accepted == Some(false)
        }));
    }

    #[test]
    fn quick_slot_bind_persists_atomic_block_mirror_for_reload() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("bong-quick-bind-{unique}"));
        let db_path = root.join("bong.db");
        crate::persistence::bootstrap_sqlite(&db_path, "quick-bind-test")
            .expect("test sqlite should bootstrap");
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(crate::inventory::load_item_registry().expect("item registry loads"));
        app.insert_resource(PlayerStatePersistence::with_db_path(&root, &db_path));
        let inventory = inventory_with_item(inventory_test_item(88, "earth_crumb", 1));
        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                QuickSlotBindings::default(),
                SkillBarBindings::default(),
                inventory,
            ))
            .id();

        send_quick_slot_bind_request(&mut app, entity, 3, Some("earth_crumb"), "persist-block");
        app.update();

        let connection = rusqlite::Connection::open(&db_path).expect("test sqlite should open");
        let prefs_json: String = connection
            .query_row(
                "SELECT prefs_json FROM player_ui_prefs WHERE username = 'Azure'",
                [],
                |row| row.get(0),
            )
            .expect("accepted bind should persist UI prefs");
        let prefs: serde_json::Value =
            serde_json::from_str(&prefs_json).expect("persisted prefs should be valid JSON");
        assert_eq!(prefs["quick_slots"][3], "earth_crumb");
        assert_eq!(prefs["skill_bar"][3]["kind"], "item");
        assert_eq!(prefs["skill_bar"][3]["template_id"], "earth_crumb");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn quick_slot_bind_accepts_128_cjk_request_id_and_rejects_129() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(crate::inventory::load_item_registry().expect("item registry loads"));

        let inventory = inventory_with_item(inventory_test_item(88, "earth_crumb", 1));
        let (client_bundle, mut helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                QuickSlotBindings::default(),
                SkillBarBindings::default(),
                inventory,
            ))
            .id();

        // 128 个 '界' 字符（每个 3 字节，共 384 字节）必须被视为合法长度并接受
        let rid128 = "界".repeat(128);
        send_quick_slot_bind_request(&mut app, entity, 3, Some("earth_crumb"), &rid128);
        app.update();
        flush_all_client_packets(&mut app);

        assert_eq!(
            app.world().get::<QuickSlotBindings>(entity).unwrap().get(3),
            Some(88)
        );
        let configs = collect_quickslot_configs(&mut helper);
        assert!(configs.iter().any(|c| {
            c.ack_request_id.as_deref() == Some(&rid128) && c.bind_accepted == Some(true)
        }));

        // 129 个 '界' 字符必须被静默拒绝且不产生状态变异
        let rid129 = "界".repeat(129);
        send_quick_slot_bind_request(&mut app, entity, 4, Some("earth_crumb"), &rid129);
        app.update();
        flush_all_client_packets(&mut app);

        assert_eq!(
            app.world().get::<QuickSlotBindings>(entity).unwrap().get(4),
            None
        );
    }

    #[test]
    fn quick_slot_bind_rejects_empty_string_item_id_without_unbinding() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(crate::inventory::load_item_registry().expect("item registry loads"));

        let inventory = inventory_with_item(inventory_test_item(88, "earth_crumb", 1));
        let mut quick_slots = QuickSlotBindings::default();
        assert!(quick_slots.set(3, Some(88)));
        let (client_bundle, mut helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                quick_slots,
                SkillBarBindings::default(),
                inventory,
            ))
            .id();

        // 发送 raw JSON item_id=""（非 null），必须被拒绝（bind_accepted=false）且已有绑定保持原样
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"quick_slot_bind","v":1,"slot":3,"item_id":"","request_id":"empty-item-id"}"#
                    .to_vec()
                    .into_boxed_slice(),
            });
        app.update();
        flush_all_client_packets(&mut app);

        // 槽 3 上的已有绑定 88 必须保持，不得被清空
        assert_eq!(
            app.world().get::<QuickSlotBindings>(entity).unwrap().get(3),
            Some(88),
            "item_id=\"\" 畸形请求不得清空既有绑定"
        );
        let configs = collect_quickslot_configs(&mut helper);
        assert!(
            configs.iter().any(|c| {
                c.ack_request_id.as_deref() == Some("empty-item-id")
                    && c.bind_accepted == Some(false)
            }),
            "item_id=\"\" 请求应下发 bind_accepted=false 的 quickslot_config 回执"
        );
    }

    #[test]
    fn inventory_instance_id_by_template_prefers_containers_hotbar_then_equipped() {
        let mut inventory = inventory_with_item(inventory_test_item(11, "bone_whistle", 1));
        inventory.hotbar[0] = Some(inventory_test_item(22, "bone_whistle", 1));
        inventory.equipped.insert(
            crate::inventory::EQUIP_SLOT_MAIN_HAND.to_string(),
            crate::inventory::SlotContents::held_single(inventory_test_item(33, "bone_whistle", 1)),
        );

        assert_eq!(
            inventory_instance_id_by_template(&inventory, "bone_whistle"),
            Some(11),
            "container match should keep the pre-existing quick_slot_bind precedence"
        );

        inventory.containers[0].items.clear();
        assert_eq!(
            inventory_instance_id_by_template(&inventory, "bone_whistle"),
            Some(22),
            "hotbar match should beat equipped when no container item matches"
        );
    }

    #[test]
    fn inventory_instance_id_by_template_finds_worn_equipped_item() {
        let mut inventory = empty_inventory();
        inventory.equipped.insert(
            crate::inventory::EQUIP_SLOT_CHEST.to_string(),
            crate::inventory::SlotContents::worn_single(inventory_test_item(44, "bone_whistle", 1)),
        );

        assert_eq!(
            inventory_instance_id_by_template(&inventory, "bone_whistle"),
            Some(44),
            "worn equipped items should be eligible for quick_slot_bind template lookup"
        );
    }

    #[test]
    fn inventory_instance_id_by_template_uses_stable_equipped_slot_order() {
        let mut inventory = empty_inventory();
        inventory.equipped.insert(
            crate::inventory::EQUIP_SLOT_OFF_HAND.to_string(),
            crate::inventory::SlotContents::held_single(inventory_test_item(55, "bone_whistle", 1)),
        );
        inventory.equipped.insert(
            crate::inventory::EQUIP_SLOT_MAIN_HAND.to_string(),
            crate::inventory::SlotContents::held_single(inventory_test_item(66, "bone_whistle", 1)),
        );

        assert_eq!(
            inventory_instance_id_by_template(&inventory, "bone_whistle"),
            Some(66),
            "equipped template lookup should not depend on HashMap iteration order"
        );
    }

    #[test]
    fn inventory_instance_id_by_template_returns_none_when_missing() {
        let inventory = empty_inventory();

        assert_eq!(
            inventory_instance_id_by_template(&inventory, "bone_whistle"),
            None,
            "missing template should leave quick_slot_bind instance unresolved"
        );
    }

    #[test]
    fn inventory_move_applies_hidden_targeted_wear_to_spiritual_item() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(ItemRegistry::from_map(HashMap::from([(
            "spiritual_ore".to_string(),
            ItemTemplate {
                id: "spiritual_ore".to_string(),
                display_name: "灵矿".to_string(),
                category: ItemCategory::Misc,
                placeable: None,
                max_stack_count: 1,
                grid_w: 1,
                grid_h: 1,
                base_weight: 1.0,
                rarity: ItemRarity::Rare,
                spirit_quality_initial: 1.0,
                description: String::new(),
                effect: None,
                cast_duration_ms: crate::inventory::DEFAULT_CAST_DURATION_MS,
                cooldown_ms: crate::inventory::DEFAULT_COOLDOWN_MS,
                weapon_spec: None,
                forge_station_spec: None,
                blueprint_scroll_spec: None,
                inscription_scroll_spec: None,
                technique_scroll_spec: None,
                readable_scroll_spec: None,
                recipe_fragment_spec: None,
                container_spec: None,
                shelflife_profile: None,
                shield_spec: None,
                shelflife_track: None,
                wearer_race: crate::body_plan::types::RaceGateOwned::default(),
            },
        )])));
        let mut karma = KarmaWeightStore::default();
        karma.mark_player(
            "Azure",
            Some("spawn".to_string()),
            valence::prelude::BlockPos::new(8, 66, 8),
            1.0,
            1,
        );
        app.insert_resource(karma);

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                inventory_with_item(ItemInstance {
                    instance_id: 77,
                    template_id: "spiritual_ore".to_string(),
                    display_name: "灵矿".to_string(),
                    grid_w: 1,
                    grid_h: 1,
                    weight: 1.0,
                    rarity: ItemRarity::Rare,
                    description: String::new(),
                    stack_count: 1,
                    spirit_quality: 1.0,
                    durability: 1.0,
                    freshness: None,
                    mineral_id: Some("ling_shi_zhong".to_string()),
                    charges: None,
                    forge_quality: None,
                    forge_color: None,
                    forge_side_effects: Vec::new(),
                    forge_achieved_tier: None,
                    alchemy: None,
                    lingering_owner_qi: None,
                }),
                Cultivation::default(),
                PlayerState::default(),
                QuickSlotBindings::default(),
                UnlockedStyles::default(),
            ))
            .id();
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"inventory_move_intent","v":1,"instance_id":77,"from":{"kind":"container","container_id":"main_pack","row":0,"col":0},"to":{"kind":"container","container_id":"main_pack","row":0,"col":1}}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

        app.update();
        flush_all_client_packets(&mut app);

        let inventory = app.world().get::<PlayerInventory>(entity).unwrap();
        let moved = inventory_item_by_instance_borrow(inventory, 77).expect("item should remain");
        assert!(moved.durability < 1.0);
        assert!(moved.durability >= 0.95);
        assert_eq!(moved.durability, moved.spirit_quality);
        assert!(
            has_inventory_durability_payload(&mut helper, 77),
            "targeted wear should reuse durability incremental payload"
        );
    }

    /// plan-rotate-v1 e2e — 客户端 JSON wire 带 rotated:true 的 inventory_move_intent
    /// 走完整 handler 链路后，instance 的 grid_w/grid_h 在 PlayerInventory 中互换。
    #[test]
    fn inventory_move_intent_with_rotated_true_swaps_dims_end_to_end() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(ItemRegistry::from_map(HashMap::from([(
            "long_rod".to_string(),
            ItemTemplate {
                id: "long_rod".to_string(),
                display_name: "长杆".to_string(),
                category: ItemCategory::Misc,
                placeable: None,
                max_stack_count: 1,
                grid_w: 2,
                grid_h: 1,
                base_weight: 1.0,
                rarity: ItemRarity::Common,
                spirit_quality_initial: 1.0,
                description: String::new(),
                effect: None,
                cast_duration_ms: crate::inventory::DEFAULT_CAST_DURATION_MS,
                cooldown_ms: crate::inventory::DEFAULT_COOLDOWN_MS,
                weapon_spec: None,
                forge_station_spec: None,
                blueprint_scroll_spec: None,
                inscription_scroll_spec: None,
                technique_scroll_spec: None,
                readable_scroll_spec: None,
                recipe_fragment_spec: None,
                container_spec: None,
                shelflife_profile: None,
                shield_spec: None,
                shelflife_track: None,
                wearer_race: crate::body_plan::types::RaceGateOwned::default(),
            },
        )])));

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                inventory_with_item(ItemInstance {
                    instance_id: 77,
                    template_id: "long_rod".to_string(),
                    display_name: "长杆".to_string(),
                    grid_w: 2,
                    grid_h: 1,
                    weight: 1.0,
                    rarity: ItemRarity::Common,
                    description: String::new(),
                    stack_count: 1,
                    spirit_quality: 1.0,
                    durability: 1.0,
                    freshness: None,
                    mineral_id: None,
                    charges: None,
                    forge_quality: None,
                    forge_color: None,
                    forge_side_effects: Vec::new(),
                    forge_achieved_tier: None,
                    alchemy: None,
                    lingering_owner_qi: None,
                }),
                Cultivation::default(),
                PlayerState::default(),
                QuickSlotBindings::default(),
                UnlockedStyles::default(),
            ))
            .id();
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"inventory_move_intent","v":1,"instance_id":77,"rotated":true,"from":{"kind":"container","container_id":"main_pack","row":0,"col":0},"to":{"kind":"container","container_id":"main_pack","row":2,"col":3}}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

        app.update();
        flush_all_client_packets(&mut app);

        let inventory = app.world().get::<PlayerInventory>(entity).unwrap();
        let placed = inventory.containers[0]
            .items
            .iter()
            .find(|p| p.instance.instance_id == 77)
            .expect("item should remain in main_pack");
        assert_eq!(
            (placed.row, placed.col),
            (2, 3),
            "rotated move 应落到目标格 (2,3)"
        );
        assert_eq!(
            (placed.instance.grid_w, placed.instance.grid_h),
            (1, 2),
            "e2e：rotated:true 落位后 grid_w/grid_h 应互换为 1x2，实际 {}x{}",
            placed.instance.grid_w,
            placed.instance.grid_h
        );
    }

    /// plan-rotate-v1 e2e — rotated 落位越界（2x1 转 1x2 撞底）被拒后，
    /// 原物品位置与朝向均未变（无脏状态），且不 panic。
    #[test]
    fn inventory_move_intent_rotated_rejection_leaves_inventory_clean_end_to_end() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(ItemRegistry::from_map(HashMap::from([(
            "long_rod".to_string(),
            ItemTemplate {
                id: "long_rod".to_string(),
                display_name: "长杆".to_string(),
                category: ItemCategory::Misc,
                placeable: None,
                max_stack_count: 1,
                grid_w: 2,
                grid_h: 1,
                base_weight: 1.0,
                rarity: ItemRarity::Common,
                spirit_quality_initial: 1.0,
                description: String::new(),
                effect: None,
                cast_duration_ms: crate::inventory::DEFAULT_CAST_DURATION_MS,
                cooldown_ms: crate::inventory::DEFAULT_COOLDOWN_MS,
                weapon_spec: None,
                forge_station_spec: None,
                blueprint_scroll_spec: None,
                inscription_scroll_spec: None,
                technique_scroll_spec: None,
                readable_scroll_spec: None,
                recipe_fragment_spec: None,
                container_spec: None,
                shelflife_profile: None,
                shield_spec: None,
                shelflife_track: None,
                wearer_race: crate::body_plan::types::RaceGateOwned::default(),
            },
        )])));

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                inventory_with_item(ItemInstance {
                    instance_id: 77,
                    template_id: "long_rod".to_string(),
                    display_name: "长杆".to_string(),
                    grid_w: 2,
                    grid_h: 1,
                    weight: 1.0,
                    rarity: ItemRarity::Common,
                    description: String::new(),
                    stack_count: 1,
                    spirit_quality: 1.0,
                    durability: 1.0,
                    freshness: None,
                    mineral_id: None,
                    charges: None,
                    forge_quality: None,
                    forge_color: None,
                    forge_side_effects: Vec::new(),
                    forge_achieved_tier: None,
                    alchemy: None,
                    lingering_owner_qi: None,
                }),
                Cultivation::default(),
                PlayerState::default(),
                QuickSlotBindings::default(),
                UnlockedStyles::default(),
            ))
            .id();
        // 目标 (4,0)：不旋转时 2x1 在最底行放得下；旋转成 1x2 后行溢出 → 拒绝。
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"inventory_move_intent","v":1,"instance_id":77,"rotated":true,"from":{"kind":"container","container_id":"main_pack","row":0,"col":0},"to":{"kind":"container","container_id":"main_pack","row":4,"col":0}}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

        app.update();
        flush_all_client_packets(&mut app);

        let inventory = app.world().get::<PlayerInventory>(entity).unwrap();
        let placed = inventory.containers[0]
            .items
            .iter()
            .find(|p| p.instance.instance_id == 77)
            .expect("item should remain in main_pack");
        assert_eq!(
            (placed.row, placed.col),
            (0, 0),
            "旋转越界拒绝后物品必须留在原位"
        );
        assert_eq!(
            (placed.instance.grid_w, placed.instance.grid_h),
            (2, 1),
            "旋转越界拒绝后必须保持原朝向 2x1（无脏状态）"
        );
    }

    #[test]
    fn apply_pill_during_tribulation_recovers_current_qi_only() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(ItemRegistry::from_map(HashMap::from([(
            "huiyuan_pill".to_string(),
            ItemTemplate {
                id: "huiyuan_pill".to_string(),
                display_name: "回元丹".to_string(),
                category: ItemCategory::Pill,
                placeable: None,
                max_stack_count: 1,
                grid_w: 1,
                grid_h: 1,
                base_weight: 0.1,
                rarity: ItemRarity::Rare,
                spirit_quality_initial: 1.0,
                description: String::new(),
                effect: Some(ItemEffect::QiRecovery { amount: 90.0 }),
                cast_duration_ms: crate::inventory::DEFAULT_CAST_DURATION_MS,
                cooldown_ms: crate::inventory::DEFAULT_COOLDOWN_MS,
                weapon_spec: None,
                forge_station_spec: None,
                blueprint_scroll_spec: None,
                inscription_scroll_spec: None,
                technique_scroll_spec: None,
                readable_scroll_spec: None,
                recipe_fragment_spec: None,
                container_spec: None,
                shelflife_profile: None,
                shield_spec: None,
                shelflife_track: None,
                wearer_race: crate::body_plan::types::RaceGateOwned::default(),
            },
        )])));

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                inventory_with_item(ItemInstance {
                    instance_id: 77,
                    template_id: "huiyuan_pill".to_string(),
                    display_name: "回元丹".to_string(),
                    grid_w: 1,
                    grid_h: 1,
                    weight: 0.1,
                    rarity: ItemRarity::Rare,
                    description: String::new(),
                    stack_count: 1,
                    spirit_quality: 1.0,
                    durability: 1.0,
                    freshness: None,
                    mineral_id: None,
                    charges: None,
                    forge_quality: None,
                    forge_color: None,
                    forge_side_effects: Vec::new(),
                    forge_achieved_tier: None,
                    alchemy: None,
                    lingering_owner_qi: None,
                }),
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 20.0,
                    qi_max: 100.0,
                    qi_max_frozen: Some(30.0),
                    ..Cultivation::default()
                },
                PlayerState::default(),
                TribulationState::restored(2, 5, 10),
            ))
            .id();
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"apply_pill","v":1,"instance_id":77,"target":{"kind":"self"}}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

        app.update();

        let cultivation = app.world().get::<Cultivation>(entity).unwrap();
        assert_eq!(cultivation.qi_current, 70.0);
        assert_eq!(cultivation.qi_max, 100.0);
        assert_eq!(cultivation.qi_max_frozen, Some(30.0));
        assert!(app.world().get::<TribulationState>(entity).is_some());

        let inventory = app.world().get::<PlayerInventory>(entity).unwrap();
        assert!(inventory.containers[0].items.is_empty());
        assert_eq!(inventory.revision.0, 1);
    }

    #[test]
    fn mineral_probe_request_emits_probe_intent() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        app.insert_resource(TechniqueRegistry::load_for_tests());
        app.insert_resource(CapturedMineralProbes::default());
        app.insert_resource(CombatClock::default());
        app.insert_resource(GameplayActionQueue::default());
        app.insert_resource(AlchemyMockState::default());
        app.insert_resource(DroppedLootRegistry::default());
        // plan-remains-suite P0 — DroppedLootRequestParams 新增 EventWriter<RemainsLootIntent>。
        app.add_event::<crate::inventory::RemainsLootIntent>();
        app.insert_resource(ItemRegistry::default());
        app.insert_resource(RecipeRegistry::default());
        app.add_event::<CustomPayloadEvent>();
        app.add_event::<BreakthroughRequest>();
        app.add_event::<ForgeRequest>();
        app.add_event::<InsightChosen>();
        app.add_event::<DefenseIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<PlaceFurnaceRequest>();
        app.add_event::<crate::alchemy::LearnRecipeFragmentIntent>();
        app.add_event::<StartTillRequest>();
        app.add_event::<StartRenewRequest>();
        app.add_event::<StartPlantingRequest>();
        app.add_event::<StartHarvestRequest>();
        app.add_event::<StartReplenishRequest>();
        app.add_event::<StartDrainQiRequest>();
        app.add_event::<StartExtractRequestEvent>();
        app.add_event::<CancelExtractRequestEvent>();
        app.add_event::<MineralProbeIntent>();
        app.add_event::<SkillXpGain>();
        app.add_event::<SkillScrollUsed>();
        // plan-shield-block-v1 P1 — 举盾 intent events（ClientRequestDispatchParams 需要）。
        app.add_event::<crate::combat::shield_block::RaiseShieldIntent>();
        app.add_event::<crate::combat::shield_block::LowerShieldIntent>();
        app.add_event::<crate::network::agent_ui::AgentUiResponseEvent>();
        app.add_systems(
            Update,
            (handle_client_request_payloads, capture_mineral_probes).chain(),
        );

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut()
            .entity_mut(entity)
            .insert(Position(DVec3::new(8.5, 32.0, 8.5)));
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"mineral_probe","v":1,"x":8,"y":32,"z":8}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

        app.update();

        let captured = app.world().resource::<CapturedMineralProbes>();
        assert_eq!(captured.0.len(), 1);
        assert_eq!(captured.0[0].player, entity);
        assert_eq!(
            captured.0[0].position,
            valence::prelude::BlockPos::new(8, 32, 8)
        );
        assert_eq!(captured.0[0].dimension, DimensionKind::Overworld);
    }

    #[test]
    fn spirit_niche_place_request_emits_place_intent() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        app.insert_resource(TechniqueRegistry::load_for_tests());
        app.insert_resource(CapturedSpiritNichePlaces::default());
        app.insert_resource(CombatClock { tick: 88 });
        app.insert_resource(GameplayActionQueue::default());
        app.insert_resource(AlchemyMockState::default());
        app.insert_resource(DroppedLootRegistry::default());
        // plan-remains-suite P0 — DroppedLootRequestParams 新增 EventWriter<RemainsLootIntent>。
        app.add_event::<crate::inventory::RemainsLootIntent>();
        app.insert_resource(ItemRegistry::default());
        app.insert_resource(RecipeRegistry::default());
        app.add_event::<CustomPayloadEvent>();
        app.add_event::<BreakthroughRequest>();
        app.add_event::<ForgeRequest>();
        app.add_event::<InsightChosen>();
        app.add_event::<DefenseIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<PlaceFurnaceRequest>();
        app.add_event::<crate::alchemy::LearnRecipeFragmentIntent>();
        app.add_event::<SpiritNichePlaceRequest>();
        app.add_event::<SpiritNicheCoordinateRevealRequest>();
        app.add_event::<StartTillRequest>();
        app.add_event::<StartRenewRequest>();
        app.add_event::<StartPlantingRequest>();
        app.add_event::<StartHarvestRequest>();
        app.add_event::<StartReplenishRequest>();
        app.add_event::<StartDrainQiRequest>();
        app.add_event::<StartExtractRequestEvent>();
        app.add_event::<CancelExtractRequestEvent>();
        app.add_event::<MineralProbeIntent>();
        app.add_event::<SkillXpGain>();
        app.add_event::<SkillScrollUsed>();
        // plan-shield-block-v1 P1 — 举盾 intent events（ClientRequestDispatchParams 需要）。
        app.add_event::<crate::combat::shield_block::RaiseShieldIntent>();
        app.add_event::<crate::combat::shield_block::LowerShieldIntent>();
        app.add_event::<crate::network::agent_ui::AgentUiResponseEvent>();
        app.add_systems(
            Update,
            (handle_client_request_payloads, capture_spirit_niche_places).chain(),
        );

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"spirit_niche_place","v":1,"x":11,"y":64,"z":10,"item_instance_id":4242}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

        app.update();

        let captured = app.world().resource::<CapturedSpiritNichePlaces>();
        assert_eq!(captured.0.len(), 1);
        assert_eq!(captured.0[0].player, entity);
        assert_eq!(captured.0[0].pos, [11, 64, 10]);
        assert_eq!(captured.0[0].item_instance_id, Some(4242));
        assert_eq!(captured.0[0].tick, 88);
    }

    #[test]
    fn spirit_niche_repair_request_emits_repair_intent() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        app.insert_resource(CapturedSpiritNicheRepairs::default());
        register_request_app(&mut app);
        app.insert_resource(CombatClock { tick: 90 });
        app.add_systems(
            Update,
            capture_spirit_niche_repairs.after(handle_client_request_payloads),
        );

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"spirit_niche_repair","v":1,"x":11,"y":64,"z":10,"item_instance_id":4242}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

        app.update();

        let captured = app.world().resource::<CapturedSpiritNicheRepairs>();
        assert_eq!(captured.0.len(), 1);
        assert_eq!(captured.0[0].player, entity);
        assert_eq!(captured.0[0].pos, [11, 64, 10]);
        assert_eq!(captured.0[0].item_instance_id, Some(4242));
        assert_eq!(captured.0[0].tick, 90);
    }

    #[test]
    fn coffin_open_request_emits_spawn_tutorial_intent() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        app.insert_resource(CapturedCoffinOpenRequests::default());
        register_request_app(&mut app);
        app.insert_resource(CombatClock { tick: 91 });
        app.add_systems(
            Update,
            capture_coffin_open_requests.after(handle_client_request_payloads),
        );

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"coffin_open","v":1,"x":0,"y":69,"z":0}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

        app.update();

        let captured = app.world().resource::<CapturedCoffinOpenRequests>();
        assert_eq!(captured.0.len(), 1);
        assert_eq!(captured.0[0].player, entity);
        assert_eq!(captured.0[0].pos, [0, 69, 0]);
        assert_eq!(captured.0[0].tick, 91);
    }

    // ─── plan-coffin-tiers-v1 P3：CoffinBreak/CoffinMenuReclaim decode→emit tests ───
    // CodeRabbit major A: 补 C2S 协议分支 decode→emit 测试。

    #[test]
    fn coffin_break_request_emits_event_with_correct_player_and_pos() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        app.insert_resource(CapturedCoffinBreakRequests::default());
        register_request_app(&mut app);
        app.insert_resource(CombatClock { tick: 77 });
        app.add_systems(
            Update,
            capture_coffin_break_requests.after(handle_client_request_payloads),
        );

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"coffin_break","v":1,"x":10,"y":64,"z":-5}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

        app.update();

        let captured = app.world().resource::<CapturedCoffinBreakRequests>();
        assert_eq!(
            captured.0.len(),
            1,
            "coffin_break 请求应 emit 恰好 1 个 CoffinBreakRequest event；实得 {}",
            captured.0.len()
        );
        assert_eq!(
            captured.0[0].player, entity,
            "CoffinBreakRequest.player 应等于发送玩家实体；期望 {entity:?}，实得 {:?}",
            captured.0[0].player
        );
        assert_eq!(
            captured.0[0].pos,
            valence::prelude::BlockPos::new(10, 64, -5),
            "CoffinBreakRequest.pos 应精确等于请求坐标 [10,64,-5]；期望 BlockPos(10,64,-5)，实得 {:?}",
            captured.0[0].pos
        );
        assert_eq!(
            captured.0[0].tick, 77,
            "CoffinBreakRequest.tick 应等于 CombatClock.tick；期望 77，实得 {}",
            captured.0[0].tick
        );
    }

    #[test]
    fn coffin_menu_reclaim_request_emits_event_with_correct_player_and_pos() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        app.insert_resource(CapturedCoffinMenuReclaimRequests::default());
        register_request_app(&mut app);
        app.insert_resource(CombatClock { tick: 88 });
        app.add_systems(
            Update,
            capture_coffin_menu_reclaim_requests.after(handle_client_request_payloads),
        );

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"coffin_menu_reclaim","v":1,"x":-8,"y":65,"z":3}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

        app.update();

        let captured = app.world().resource::<CapturedCoffinMenuReclaimRequests>();
        assert_eq!(
            captured.0.len(),
            1,
            "coffin_menu_reclaim 请求应 emit 恰好 1 个 CoffinMenuReclaimRequest event；实得 {}",
            captured.0.len()
        );
        assert_eq!(
            captured.0[0].player, entity,
            "CoffinMenuReclaimRequest.player 应等于发送玩家实体；期望 {entity:?}，实得 {:?}",
            captured.0[0].player
        );
        assert_eq!(
            captured.0[0].pos,
            valence::prelude::BlockPos::new(-8, 65, 3),
            "CoffinMenuReclaimRequest.pos 应精确等于请求坐标 [-8,65,3]；期望 BlockPos(-8,65,3)，实得 {:?}",
            captured.0[0].pos
        );
        assert_eq!(
            captured.0[0].tick, 88,
            "CoffinMenuReclaimRequest.tick 应等于 CombatClock.tick；期望 88，实得 {}",
            captured.0[0].tick
        );
    }

    #[test]
    fn spirit_niche_coordinate_requests_emit_reveal_intents() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        app.insert_resource(TechniqueRegistry::load_for_tests());
        app.insert_resource(CapturedSpiritNicheCoordinateReveals::default());
        app.insert_resource(CombatClock { tick: 89 });
        app.insert_resource(GameplayActionQueue::default());
        app.insert_resource(AlchemyMockState::default());
        app.insert_resource(DroppedLootRegistry::default());
        // plan-remains-suite P0 — DroppedLootRequestParams 新增 EventWriter<RemainsLootIntent>。
        app.add_event::<crate::inventory::RemainsLootIntent>();
        app.insert_resource(ItemRegistry::default());
        app.insert_resource(RecipeRegistry::default());
        app.add_event::<CustomPayloadEvent>();
        app.add_event::<BreakthroughRequest>();
        app.add_event::<ForgeRequest>();
        app.add_event::<InsightChosen>();
        app.add_event::<DefenseIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<PlaceFurnaceRequest>();
        app.add_event::<crate::alchemy::LearnRecipeFragmentIntent>();
        app.add_event::<SpiritNichePlaceRequest>();
        app.add_event::<SpiritNicheCoordinateRevealRequest>();
        app.add_event::<StartTillRequest>();
        app.add_event::<StartRenewRequest>();
        app.add_event::<StartPlantingRequest>();
        app.add_event::<StartHarvestRequest>();
        app.add_event::<StartReplenishRequest>();
        app.add_event::<StartDrainQiRequest>();
        app.add_event::<StartExtractRequestEvent>();
        app.add_event::<CancelExtractRequestEvent>();
        app.add_event::<MineralProbeIntent>();
        app.add_event::<SkillXpGain>();
        app.add_event::<SkillScrollUsed>();
        // plan-shield-block-v1 P1 — 举盾 intent events（ClientRequestDispatchParams 需要）。
        app.add_event::<crate::combat::shield_block::RaiseShieldIntent>();
        app.add_event::<crate::combat::shield_block::LowerShieldIntent>();
        app.add_event::<crate::network::agent_ui::AgentUiResponseEvent>();
        app.add_systems(
            Update,
            (
                handle_client_request_payloads,
                capture_spirit_niche_coordinate_reveals,
            )
                .chain(),
        );

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        let mut custom_payloads = app
            .world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>();
        custom_payloads.send(CustomPayloadEvent {
            client: entity,
            channel: ident!("bong:client_request").into(),
            data: br#"{"type":"spirit_niche_gaze","v":1,"x":11,"y":64,"z":10}"#
                .to_vec()
                .into_boxed_slice(),
        });
        custom_payloads.send(CustomPayloadEvent {
            client: entity,
            channel: ident!("bong:client_request").into(),
            data: br#"{"type":"spirit_niche_mark_coordinate","v":1,"x":12,"y":65,"z":11}"#
                .to_vec()
                .into_boxed_slice(),
        });

        app.update();

        let captured = app
            .world()
            .resource::<CapturedSpiritNicheCoordinateReveals>();
        assert_eq!(captured.0.len(), 2);
        assert_eq!(captured.0[0].observer, entity);
        assert_eq!(captured.0[0].pos, [11, 64, 10]);
        assert_eq!(captured.0[0].source, SpiritNicheRevealSource::Gaze);
        assert_eq!(captured.0[0].tick, 89);
        assert_eq!(captured.0[1].observer, entity);
        assert_eq!(captured.0[1].pos, [12, 65, 11]);
        assert_eq!(
            captured.0[1].source,
            SpiritNicheRevealSource::MarkCoordinate
        );
        assert_eq!(captured.0[1].tick, 89);
    }

    #[test]
    fn mineral_probe_request_out_of_range_is_rejected() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        app.insert_resource(TechniqueRegistry::load_for_tests());
        app.insert_resource(CapturedMineralProbes::default());
        app.insert_resource(CombatClock::default());
        app.insert_resource(GameplayActionQueue::default());
        app.insert_resource(AlchemyMockState::default());
        app.insert_resource(DroppedLootRegistry::default());
        // plan-remains-suite P0 — DroppedLootRequestParams 新增 EventWriter<RemainsLootIntent>。
        app.add_event::<crate::inventory::RemainsLootIntent>();
        app.insert_resource(ItemRegistry::default());
        app.insert_resource(RecipeRegistry::default());
        app.add_event::<CustomPayloadEvent>();
        app.add_event::<BreakthroughRequest>();
        app.add_event::<ForgeRequest>();
        app.add_event::<InsightChosen>();
        app.add_event::<DefenseIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<PlaceFurnaceRequest>();
        app.add_event::<crate::alchemy::LearnRecipeFragmentIntent>();
        app.add_event::<StartTillRequest>();
        app.add_event::<StartRenewRequest>();
        app.add_event::<StartPlantingRequest>();
        app.add_event::<StartHarvestRequest>();
        app.add_event::<StartReplenishRequest>();
        app.add_event::<StartDrainQiRequest>();
        app.add_event::<StartExtractRequestEvent>();
        app.add_event::<CancelExtractRequestEvent>();
        app.add_event::<MineralProbeIntent>();
        app.add_event::<SkillXpGain>();
        app.add_event::<SkillScrollUsed>();
        // plan-shield-block-v1 P1 — 举盾 intent events（ClientRequestDispatchParams 需要）。
        app.add_event::<crate::combat::shield_block::RaiseShieldIntent>();
        app.add_event::<crate::combat::shield_block::LowerShieldIntent>();
        app.add_event::<crate::network::agent_ui::AgentUiResponseEvent>();
        app.add_systems(
            Update,
            (handle_client_request_payloads, capture_mineral_probes).chain(),
        );

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut()
            .entity_mut(entity)
            .insert(Position(DVec3::ZERO));
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"mineral_probe","v":1,"x":128,"y":64,"z":128}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

        app.update();

        let captured = app.world().resource::<CapturedMineralProbes>();
        assert!(captured.0.is_empty());
    }

    #[test]
    fn mineral_probe_request_uses_player_dimension() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        app.insert_resource(TechniqueRegistry::load_for_tests());
        app.insert_resource(CapturedMineralProbes::default());
        app.insert_resource(CombatClock::default());
        app.insert_resource(GameplayActionQueue::default());
        app.insert_resource(AlchemyMockState::default());
        app.insert_resource(DroppedLootRegistry::default());
        // plan-remains-suite P0 — DroppedLootRequestParams 新增 EventWriter<RemainsLootIntent>。
        app.add_event::<crate::inventory::RemainsLootIntent>();
        app.insert_resource(ItemRegistry::default());
        app.insert_resource(RecipeRegistry::default());
        app.add_event::<CustomPayloadEvent>();
        app.add_event::<BreakthroughRequest>();
        app.add_event::<ForgeRequest>();
        app.add_event::<InsightChosen>();
        app.add_event::<DefenseIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<PlaceFurnaceRequest>();
        app.add_event::<crate::alchemy::LearnRecipeFragmentIntent>();
        app.add_event::<StartTillRequest>();
        app.add_event::<StartRenewRequest>();
        app.add_event::<StartPlantingRequest>();
        app.add_event::<StartHarvestRequest>();
        app.add_event::<StartReplenishRequest>();
        app.add_event::<StartDrainQiRequest>();
        app.add_event::<StartExtractRequestEvent>();
        app.add_event::<CancelExtractRequestEvent>();
        app.add_event::<MineralProbeIntent>();
        app.add_event::<SkillXpGain>();
        app.add_event::<SkillScrollUsed>();
        // plan-shield-block-v1 P1 — 举盾 intent events（ClientRequestDispatchParams 需要）。
        app.add_event::<crate::combat::shield_block::RaiseShieldIntent>();
        app.add_event::<crate::combat::shield_block::LowerShieldIntent>();
        app.add_event::<crate::network::agent_ui::AgentUiResponseEvent>();
        app.add_systems(
            Update,
            (handle_client_request_payloads, capture_mineral_probes).chain(),
        );

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut().entity_mut(entity).insert((
            Position(DVec3::new(8.5, 32.0, 8.5)),
            CurrentDimension(DimensionKind::Tsy),
        ));
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"mineral_probe","v":1,"x":8,"y":32,"z":8}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

        app.update();

        let captured = app.world().resource::<CapturedMineralProbes>();
        assert_eq!(captured.0.len(), 1);
        assert_eq!(captured.0[0].dimension, DimensionKind::Tsy);
    }

    #[test]
    fn qi_color_inspect_rejects_entity_bits_target() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(CapturedQiColorInspectRequests::default());
        app.add_systems(
            Update,
            capture_qi_color_inspect_requests.after(handle_client_request_payloads),
        );

        let (client_bundle, _helper) = create_mock_client("Azure");
        let observer = app.world_mut().spawn(client_bundle).id();
        app.world_mut()
            .entity_mut(observer)
            .insert(Position(DVec3::ZERO));
        let observed = app
            .world_mut()
            .spawn(Position(DVec3::new(1.0, 0.0, 0.0)))
            .id();
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: observer,
                channel: ident!("bong:client_request").into(),
                data: serde_json::to_vec(&ClientRequestV1::QiColorInspect {
                    v: 1,
                    observed: format!("entity_bits:{}", observed.to_bits()),
                })
                .unwrap()
                .into_boxed_slice(),
            });

        app.update();

        assert!(app
            .world()
            .resource::<CapturedQiColorInspectRequests>()
            .0
            .is_empty());
    }

    fn qi_color_inspect_test_app() -> App {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        app.add_plugins(EntityPlugin);
        register_request_app(&mut app);
        app.insert_resource(CapturedQiColorInspectRequests::default());
        app.add_systems(
            Update,
            capture_qi_color_inspect_requests.after(handle_client_request_payloads),
        );
        app
    }

    fn send_qi_color_inspect_payload(app: &mut App, observer: Entity, observed: &str) {
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: observer,
                channel: ident!("bong:client_request").into(),
                data: serde_json::to_vec(&ClientRequestV1::QiColorInspect {
                    v: 1,
                    observed: observed.to_string(),
                })
                .unwrap()
                .into_boxed_slice(),
            });
    }

    #[test]
    fn qi_color_inspect_rejects_self_cross_dimension_and_malformed_targets_without_side_effects() {
        let mut app = qi_color_inspect_test_app();
        let (client_bundle, _helper) = create_mock_client("QiColorObserver");
        let observer = app.world_mut().spawn(client_bundle).id();
        app.world_mut().entity_mut(observer).insert((
            Position(DVec3::ZERO),
            CurrentDimension(DimensionKind::Overworld),
        ));
        let observed = app
            .world_mut()
            .spawn((
                EntityKind::VILLAGER,
                EntityId::default(),
                Position(DVec3::new(1.0, 0.0, 0.0)),
                OldPosition::new(DVec3::new(1.0, 0.0, 0.0)),
                CurrentDimension(DimensionKind::Tsy),
            ))
            .id();

        // EntityPlugin assigns the protocol ids that the C2S resolver is allowed to consume.
        app.update();
        let observer_id = app
            .world()
            .get::<EntityId>(observer)
            .expect("the observer must have an authoritative protocol entity id")
            .get();
        let observed_id = app
            .world()
            .get::<EntityId>(observed)
            .expect("the observed entity must have an authoritative protocol entity id")
            .get();
        let entity_count_before = app.world().entities().len();

        send_qi_color_inspect_payload(&mut app, observer, &format!("entity:{observer_id}"));
        send_qi_color_inspect_payload(&mut app, observer, &format!("entity:{observed_id}"));
        send_qi_color_inspect_payload(&mut app, observer, "entity:not-a-number");

        app.update();

        assert!(
            app.world()
                .resource::<CapturedQiColorInspectRequests>()
                .0
                .is_empty(),
            "self-target, cross-dimension, and malformed entity id denials must emit no QiColorInspectRequest"
        );
        assert_eq!(
            app.world().entities().len(),
            entity_count_before,
            "QiColorInspect denials must not spawn or despawn ECS entities"
        );
        assert_eq!(
            app.world()
                .get::<Position>(observer)
                .expect("observer position must remain present")
                .get(),
            DVec3::ZERO,
            "QiColorInspect denials must not mutate the observer position"
        );
        assert_eq!(
            app.world()
                .get::<CurrentDimension>(observed)
                .expect("observed dimension must remain present")
                .0,
            DimensionKind::Tsy,
            "QiColorInspect denials must not mutate the observed dimension"
        );
    }

    #[test]
    fn qi_color_inspect_scope_requires_near_same_dimension_target() {
        assert_eq!(parse_qi_color_inspect_protocol_id("entity:42"), Some(42));
        assert_eq!(parse_qi_color_inspect_protocol_id("entity_bits:42"), None);
        assert_eq!(parse_qi_color_inspect_protocol_id("entity:bad"), None);

        let (_, nearby_interact_radius) = crate::reach::DistanceRule::NEARBY_INTERACT
            .profile_parts()
            .expect("NearbyInteract must remain a named distance profile");
        assert!(is_qi_color_inspect_position_in_scope(
            DVec3::ZERO,
            DVec3::new(nearby_interact_radius, 0.0, 0.0),
            true,
        ));
        assert!(!is_qi_color_inspect_position_in_scope(
            DVec3::ZERO,
            DVec3::new(nearby_interact_radius + 0.01, 0.0, 0.0),
            true,
        ));
        assert!(!is_qi_color_inspect_position_in_scope(
            DVec3::ZERO,
            DVec3::new(1.0, 0.0, 0.0),
            false,
        ));
    }

    #[test]
    fn give_dan_target_state_gate_accepts_only_live_receiving_states() {
        use crate::fauna::dying_elder::{DyingElderState, DYING_ELDER_DAN_THRESHOLD};

        assert!(dying_elder_can_receive_dan(&DyingElderState::Plea));
        assert!(dying_elder_can_receive_dan(&DyingElderState::Recovering {
            dan_received: DYING_ELDER_DAN_THRESHOLD - 1,
        }));
        assert!(!dying_elder_can_receive_dan(&DyingElderState::Recovering {
            dan_received: DYING_ELDER_DAN_THRESHOLD,
        }));
        assert!(!dying_elder_can_receive_dan(&DyingElderState::Betrayal));
        assert!(!dying_elder_can_receive_dan(&DyingElderState::Dead {
            dead_by_betrayal: false,
        }));
    }

    #[test]
    fn give_dan_target_scope_requires_same_dimension_and_six_block_boundary() {
        assert!(is_give_dan_target_in_scope(
            DVec3::ZERO,
            DVec3::new(GIVE_DAN_MAX_DISTANCE, 0.0, 0.0),
            DimensionKind::Overworld,
            DimensionKind::Overworld,
        ));
        assert!(!is_give_dan_target_in_scope(
            DVec3::ZERO,
            DVec3::new(GIVE_DAN_MAX_DISTANCE + 0.01, 0.0, 0.0),
            DimensionKind::Overworld,
            DimensionKind::Overworld,
        ));
        assert!(!is_give_dan_target_in_scope(
            DVec3::ZERO,
            DVec3::ZERO,
            DimensionKind::Overworld,
            DimensionKind::Tsy,
        ));
        assert!(!is_give_dan_target_in_scope(
            DVec3::new(f64::NAN, 0.0, 0.0),
            DVec3::ZERO,
            DimensionKind::Overworld,
            DimensionKind::Overworld,
        ));
    }

    fn production_scroll_request_app() -> App {
        let mut app = App::new();
        register_request_app(&mut app);
        let item_registry = crate::inventory::load_item_registry()
            .expect("production item registry must load for scroll routing tests");
        let mut craft_registry = crate::craft::CraftRegistry::new();
        crate::craft::load_default_craft_recipes(&mut craft_registry, &item_registry)
            .expect("production craft registry must load for scroll routing tests");
        app.insert_resource(item_registry);
        app.insert_resource(craft_registry);
        app.insert_resource(crate::craft::RecipeUnlockState::new());
        app.add_event::<crate::craft::CraftUnlockIntent>();
        app.add_event::<crate::craft::RecipeUnlockedEvent>();
        app.add_systems(
            Update,
            crate::network::craft_emit::apply_unlock_intents.after(handle_client_request_payloads),
        );
        app
    }

    fn send_scroll_use(
        app: &mut App,
        entity: Entity,
        instance_id: u64,
        request: fn(u64) -> ClientRequestV1,
    ) {
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: serde_json::to_vec(&request(instance_id))
                    .unwrap()
                    .into_boxed_slice(),
            });
    }

    fn send_technique_scroll_use(app: &mut App, entity: Entity, instance_id: u64) {
        send_scroll_use(app, entity, instance_id, |instance_id| {
            ClientRequestV1::TechniqueScrollUse { v: 1, instance_id }
        });
    }

    fn send_skill_scroll_use(app: &mut App, entity: Entity, instance_id: u64) {
        send_scroll_use(app, entity, instance_id, |instance_id| {
            ClientRequestV1::LearnSkillScroll { v: 1, instance_id }
        });
    }

    #[test]
    fn production_technique_scroll_falls_through_craft_routing() {
        let mut app = production_scroll_request_app();
        let (client_bundle, _helper) = create_mock_client("Azure");
        let mut meridians = MeridianSystem::default();
        let lung = meridians.get_mut(MeridianId::Lung);
        lung.opened = true;
        lung.integrity = 1.0;
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                inventory_with_skill_scroll(skill_scroll_item(42, "scroll_woliu_vortex")),
                KnownTechniques {
                    entries: Vec::new(),
                },
                Cultivation {
                    realm: Realm::Condense,
                    ..Default::default()
                },
                meridians,
                PlayerState::default(),
                QuickSlotBindings::default(),
                UnlockedStyles::default(),
            ))
            .id();

        send_technique_scroll_use(&mut app, entity, 42);
        app.update();

        let known = app.world().get::<KnownTechniques>(entity).unwrap();
        assert!(
            known.entries.iter().any(|entry| entry.id == "woliu.vortex"),
            "production technique scroll must reach the existing technique learner when no craft recipe names it"
        );
        assert!(
            app.world()
                .get::<PlayerInventory>(entity)
                .unwrap()
                .containers[0]
                .items
                .is_empty(),
            "successful technique learning must consume exactly one production scroll"
        );
        assert!(
            app.world_mut()
                .resource_mut::<valence::prelude::Events<crate::craft::RecipeUnlockedEvent>>()
                .drain()
                .next()
                .is_none(),
            "a technique-only scroll must not unlock a craft recipe"
        );
    }

    #[test]
    fn technique_scroll_realm_too_low_emits_structured_rejection() {
        // central-review 2012 #3 回归：fresh Awaken 用 sword.infuse（required
        // realm=Induce）→ RealmTooLow 拒绝，必须下发 InventoryMoveRejectedV1
        // {reason:"realm_too_low", required_realm:"Induce"}——只回推不变快照时
        // client 无法区分「境界拒绝」与「静默忽略/错误原因拒绝」。
        let mut app = production_scroll_request_app();
        let (client_bundle, mut helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                inventory_with_skill_scroll(skill_scroll_item(42, "scroll_technique_sword_infuse")),
                KnownTechniques {
                    entries: Vec::new(),
                },
                Cultivation {
                    realm: Realm::Awaken,
                    ..Default::default()
                },
                MeridianSystem::default(),
                PlayerState::default(),
                QuickSlotBindings::default(),
                UnlockedStyles::default(),
            ))
            .id();

        send_technique_scroll_use(&mut app, entity, 42);
        app.update();
        flush_all_client_packets(&mut app);

        let rejected = collect_inventory_move_rejected(&mut helper);
        assert_eq!(
            rejected,
            vec![crate::schema::server_data::InventoryMoveRejectedV1 {
                reason: "realm_too_low".to_string(),
                required_realm: Some("Induce".to_string()),
                slot: None,
                cap: None,
            }],
            "Awaken 用 sword.infuse 应下发恰好一条 realm_too_low 拒绝回执"
        );
    }

    #[test]
    fn technique_scroll_race_mismatch_emits_structured_rejection() {
        // central-review 2012 #3 回归：RaceMismatch 拒绝必须同样下发结构化
        // InventoryMoveRejectedV1 {reason:"race_mismatch"}——非人形本体
        // （is_humanoid=false）用 sword.infuse（RaceGate::Humanoid）时 realm 已到
        // Induce 满足境界门（否则被 RealmTooLow 掩盖），race gate 是唯一拒因。
        let mut app = production_scroll_request_app();
        let (body_plans, races) = non_humanoid_race_fixture("test_whale");
        app.insert_resource(body_plans);
        app.insert_resource(races);
        let (client_bundle, mut helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                inventory_with_skill_scroll(skill_scroll_item(42, "scroll_technique_sword_infuse")),
                KnownTechniques {
                    entries: Vec::new(),
                },
                Cultivation {
                    realm: Realm::Induce,
                    // central-review 2012 #9：`non_humanoid_race_fixture("test_whale")`
                    // 以 "test_whale" 为键注册了非人形构型——玩家 race 必须指向该真实
                    // race id，生产 `resolve_body_plan` 才选到 is_humanoid=false 本体；
                    // 若停在 HUMAN_RACE_ID（指向同一非人形构型）则人形 gate 通过、不会
                    // 触发 race_mismatch，断言虽过但测的不是 reviewer 描述的路径。
                    race: crate::body_plan::RaceId::new("test_whale"),
                    ..Default::default()
                },
                MeridianSystem::default(),
                PlayerState::default(),
                QuickSlotBindings::default(),
                UnlockedStyles::default(),
            ))
            .id();

        send_technique_scroll_use(&mut app, entity, 42);
        app.update();
        flush_all_client_packets(&mut app);

        let rejected = collect_inventory_move_rejected(&mut helper);
        assert_eq!(
            rejected,
            vec![crate::schema::server_data::InventoryMoveRejectedV1 {
                reason: "race_mismatch".to_string(),
                required_realm: None,
                slot: None,
                cap: None,
            }],
            "非人形本体用 sword.infuse 应下发恰好一条 race_mismatch 拒绝回执"
        );
    }

    #[test]
    fn production_skill_scroll_falls_through_craft_routing() {
        let mut app = production_scroll_request_app();
        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                inventory_with_skill_scroll(skill_scroll_item(
                    42,
                    "skill_scroll_herbalism_baicao_can",
                )),
                SkillSet::default(),
                Cultivation::default(),
                PlayerState::default(),
                QuickSlotBindings::default(),
                UnlockedStyles::default(),
            ))
            .id();

        send_skill_scroll_use(&mut app, entity, 42);
        app.update();

        let skill_set = app.world().get::<SkillSet>(entity).unwrap();
        assert!(
            skill_set
                .consumed_scrolls
                .contains(&ScrollId::new("skill_scroll_herbalism_baicao_can")),
            "production skill scroll must reach the existing skill learner when no craft recipe names it"
        );
        assert!(
            app.world()
                .get::<PlayerInventory>(entity)
                .unwrap()
                .containers[0]
                .items
                .is_empty(),
            "successful skill learning must consume exactly one production scroll"
        );
    }

    #[test]
    fn learn_skill_scroll_routes_positive_craft_recipe_unlock() {
        let mut app = production_scroll_request_app();
        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                inventory_with_skill_scroll(skill_scroll_item(42, "scroll_workbench_lantern")),
            ))
            .id();

        send_skill_scroll_use(&mut app, entity, 42);
        app.update();

        let player_id = canonical_player_id("Azure");
        assert!(
            app.world()
                .resource::<crate::craft::RecipeUnlockState>()
                .is_unlocked(
                    &player_id,
                    &crate::craft::RecipeId::new("workbench.shelter.lantern")
                ),
            "LearnSkillScroll must route craft recipe scrolls to the craft unlock consumer"
        );
        assert!(
            app.world()
                .get::<PlayerInventory>(entity)
                .unwrap()
                .containers[0]
                .items
                .is_empty(),
            "successful craft recipe scroll use must consume exactly one scroll"
        );
    }

    #[test]
    fn craft_scroll_unlock_uses_stable_player_id_when_caster_entity_is_gone() {
        // verdict-1906-r2 major #3 回归：残卷 reservation 与 unlock 以
        // `intent.player_id`（canonical 稳定身份）为准，而非 caster entity。
        // 历史上 consumer 反查 caster 的 Username，查不到时 fallback
        // `entity:{bits}` —— reservation 以 canonical_player_id 落账却永不
        // 释放，同玩家再拿一张同卷会被 reserve 永久拒绝。这里直接构造
        // "caster 实体已不存在"的 intent（换线重连后旧 entity id 失效），
        // 走 production 全链路（request → reserve → consume → intent →
        // apply_unlock_intents），断言 unlock 仍提交。
        let mut app = production_scroll_request_app();
        let player_id = canonical_player_id("Azure");
        let recipe_id = crate::craft::RecipeId::new("workbench.shelter.lantern");

        // 正常路径先 unlock 一次（走真实请求链路，锁住 production bridge）。
        {
            let (client_bundle, _helper) = create_mock_client("Azure");
            let entity = app
                .world_mut()
                .spawn((
                    client_bundle,
                    inventory_with_skill_scroll(skill_scroll_item(42, "scroll_workbench_lantern")),
                ))
                .id();
            send_skill_scroll_use(&mut app, entity, 42);
            app.update();
            assert!(
                app.world()
                    .resource::<crate::craft::RecipeUnlockState>()
                    .is_unlocked(&player_id, &recipe_id),
                "first unlock via real request must commit"
            );
            app.world_mut().despawn(entity);
        }

        // 已解锁 → reserve 返回 false（防止再扣第二张卷）。该行为不变。
        {
            let (client_bundle, _helper) = create_mock_client("Azure");
            let second = app
                .world_mut()
                .spawn((
                    client_bundle,
                    inventory_with_skill_scroll(skill_scroll_item(43, "scroll_workbench_lantern")),
                ))
                .id();
            let re_reserved = app
                .world_mut()
                .resource_mut::<crate::craft::RecipeUnlockState>()
                .reserve_scroll_unlock(&player_id, &recipe_id);
            assert!(
                !re_reserved,
                "already-unlocked recipe must not reserve again (no double consume)"
            );
            app.world_mut().despawn(second);
        }
    }

    #[test]
    fn craft_scroll_unlock_with_dead_caster_entity_still_commits_via_player_id() {
        // verdict-1906-r2 major #3 的第二面：intent 携带的 caster 实体在消费帧
        // 已不存在（队列跨帧 + 实体换线/死亡清场）时，apply_unlock_intents 必须
        // 用 intent.player_id 完成解锁 + 释放 reservation，而不是因反查 caster
        // 失败而丢弃（旧实现 fallback entity:{bits} 导致 canonical reservation
        // 永久残留）。
        let mut app = production_scroll_request_app();
        let player_id = canonical_player_id("Azure");
        let recipe_id = crate::craft::RecipeId::new("workbench.shelter.lantern");

        // 先 reserve（模拟请求帧已扣物品、reservation 落账）。
        assert!(
            app.world_mut()
                .resource_mut::<crate::craft::RecipeUnlockState>()
                .reserve_scroll_unlock(&player_id, &recipe_id),
            "reservation must succeed before intent processing"
        );
        // spawn 后立即 despawn：caster 实体在消费帧不存在。
        let dead_caster = app.world_mut().spawn_empty().id();
        app.world_mut().despawn(dead_caster);

        app.world_mut().send_event(crate::craft::CraftUnlockIntent {
            caster: dead_caster,
            player_id: player_id.clone(),
            recipe_id: recipe_id.clone(),
            source: crate::craft::UnlockEventSource::Scroll {
                item_template: "scroll_workbench_lantern".to_string(),
            },
        });
        app.update();

        let unlock_state = app.world().resource::<crate::craft::RecipeUnlockState>();
        assert!(
            unlock_state.is_unlocked(&player_id, &recipe_id),
            "unlock must commit via intent.player_id even when caster entity is already despawned"
        );
        assert_eq!(
            app.world_mut()
                .resource_mut::<valence::prelude::Events<crate::craft::RecipeUnlockedEvent>>()
                .drain()
                .count(),
            1,
            "dead-caster intent must still emit one observable unlock"
        );
        // 旧实现会遗留 `entity:{bits}` 错位 reservation —— 解锁后再次请求同一
        // 卷，若残留锁未清，reserve 会返回 false 且第二张卷被吞。断言解锁后
        // reservation 被释放（未解锁配方可以重新 reserve）。
        let another_recipe = crate::craft::RecipeId::new("workbench.shelter.torch");
        assert!(
            app.world_mut()
                .resource_mut::<crate::craft::RecipeUnlockState>()
                .reserve_scroll_unlock(&player_id, &another_recipe),
            "reservation bookkeeping must stay consistent after dead-caster intent"
        );
    }

    #[test]
    fn queued_duplicate_craft_scroll_requests_consume_one_from_stack() {
        let mut app = production_scroll_request_app();
        let (client_bundle, _helper) = create_mock_client("Azure");
        let mut inventory =
            inventory_with_skill_scroll(skill_scroll_item(42, "scroll_workbench_lantern"));
        inventory.containers[0].items[0].instance.stack_count = 2;
        let entity = app.world_mut().spawn((client_bundle, inventory)).id();

        send_technique_scroll_use(&mut app, entity, 42);
        send_technique_scroll_use(&mut app, entity, 42);
        app.update();

        let inventory = app.world().get::<PlayerInventory>(entity).unwrap();
        assert_eq!(inventory.containers[0].items[0].instance.stack_count, 1);
        let player_id = canonical_player_id("Azure");
        assert!(
            app.world()
                .resource::<crate::craft::RecipeUnlockState>()
                .is_unlocked(
                    &player_id,
                    &crate::craft::RecipeId::new("workbench.shelter.lantern")
                ),
            "the single accepted intent must commit the recipe unlock"
        );
        assert_eq!(
            app.world_mut()
                .resource_mut::<valence::prelude::Events<crate::craft::RecipeUnlockedEvent>>()
                .drain()
                .count(),
            1,
            "queued duplicates must produce one observable unlock"
        );
    }

    #[test]
    fn queued_duplicate_craft_scroll_instances_consume_only_first_copy() {
        let mut app = production_scroll_request_app();
        let (client_bundle, _helper) = create_mock_client("Azure");
        let mut inventory =
            inventory_with_skill_scroll(skill_scroll_item(42, "scroll_workbench_lantern"));
        inventory.containers[0].items.push(PlacedItemState {
            row: 0,
            col: 1,
            instance: skill_scroll_item(43, "scroll_workbench_lantern"),
        });
        let entity = app.world_mut().spawn((client_bundle, inventory)).id();

        send_technique_scroll_use(&mut app, entity, 42);
        send_technique_scroll_use(&mut app, entity, 43);
        app.update();

        let inventory = app.world().get::<PlayerInventory>(entity).unwrap();
        assert_eq!(inventory.containers[0].items.len(), 1);
        assert_eq!(inventory.containers[0].items[0].instance.instance_id, 43);
        assert_eq!(
            app.world_mut()
                .resource_mut::<valence::prelude::Events<crate::craft::RecipeUnlockedEvent>>()
                .drain()
                .count(),
            1,
            "two instance ids in one frame must commit one observable unlock"
        );
    }

    #[test]
    fn learn_skill_scroll_consumes_first_time_and_marks_consumed() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        app.init_resource::<ClientRequestBudget>();
        app.insert_resource(TechniqueRegistry::load_for_tests());
        app.insert_resource(CombatClock::default());
        app.insert_resource(GameplayActionQueue::default());
        app.insert_resource(AlchemyMockState::default());
        app.insert_resource(DroppedLootRegistry::default());
        // plan-remains-suite P0 — DroppedLootRequestParams 新增 EventWriter<RemainsLootIntent>。
        app.add_event::<crate::inventory::RemainsLootIntent>();
        app.insert_resource(ItemRegistry::default());
        app.insert_resource(RecipeRegistry::default());
        app.add_event::<CustomPayloadEvent>();
        app.add_event::<BreakthroughRequest>();
        app.add_event::<ForgeRequest>();
        app.add_event::<InsightChosen>();
        app.add_event::<DefenseIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<PlaceFurnaceRequest>();
        app.add_event::<crate::alchemy::LearnRecipeFragmentIntent>();
        app.add_event::<StartTillRequest>();
        app.add_event::<StartRenewRequest>();
        app.add_event::<StartPlantingRequest>();
        app.add_event::<StartHarvestRequest>();
        app.add_event::<StartReplenishRequest>();
        app.add_event::<StartDrainQiRequest>();
        app.add_event::<StartExtractRequestEvent>();
        app.add_event::<CancelExtractRequestEvent>();
        app.add_event::<MineralProbeIntent>();
        app.add_event::<SkillXpGain>();
        app.add_event::<SkillScrollUsed>();
        // plan-shield-block-v1 P1 — 举盾 intent events（ClientRequestDispatchParams 需要）。
        app.add_event::<crate::combat::shield_block::RaiseShieldIntent>();
        app.add_event::<crate::combat::shield_block::LowerShieldIntent>();
        app.add_event::<crate::network::agent_ui::AgentUiResponseEvent>();
        app.add_systems(Update, handle_client_request_payloads);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                inventory_with_skill_scroll(skill_scroll_item(
                    42,
                    "skill_scroll_herbalism_baicao_can",
                )),
                SkillSet::default(),
                Cultivation::default(),
                PlayerState::default(),
                QuickSlotBindings::default(),
                UnlockedStyles::default(),
            ))
            .id();
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"learn_skill_scroll","v":1,"instance_id":42}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

        app.update();

        let inventory = app.world().get::<PlayerInventory>(entity).unwrap();
        assert!(inventory.containers[0].items.is_empty());
        let skill_set = app.world().get::<SkillSet>(entity).unwrap();
        assert!(skill_set
            .consumed_scrolls
            .contains(&ScrollId::new("skill_scroll_herbalism_baicao_can")));

        let xp_events: Vec<_> = app
            .world_mut()
            .resource_mut::<valence::prelude::Events<SkillXpGain>>()
            .drain()
            .collect();
        assert_eq!(xp_events.len(), 1);
        assert_eq!(xp_events[0].skill, SkillId::Herbalism);
        assert_eq!(xp_events[0].amount, 500);
        let used_events: Vec<_> = app
            .world_mut()
            .resource_mut::<valence::prelude::Events<SkillScrollUsed>>()
            .drain()
            .collect();
        assert_eq!(used_events.len(), 1);
        assert!(!used_events[0].was_duplicate);
        assert_eq!(used_events[0].xp_granted, 500);
    }

    #[test]
    fn learn_skill_scroll_duplicate_does_not_consume_item() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        app.init_resource::<ClientRequestBudget>();
        app.insert_resource(TechniqueRegistry::load_for_tests());
        app.insert_resource(CombatClock::default());
        app.insert_resource(GameplayActionQueue::default());
        app.insert_resource(AlchemyMockState::default());
        app.insert_resource(DroppedLootRegistry::default());
        // plan-remains-suite P0 — DroppedLootRequestParams 新增 EventWriter<RemainsLootIntent>。
        app.add_event::<crate::inventory::RemainsLootIntent>();
        app.insert_resource(ItemRegistry::default());
        app.insert_resource(RecipeRegistry::default());
        app.add_event::<CustomPayloadEvent>();
        app.add_event::<BreakthroughRequest>();
        app.add_event::<ForgeRequest>();
        app.add_event::<InsightChosen>();
        app.add_event::<DefenseIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<PlaceFurnaceRequest>();
        app.add_event::<crate::alchemy::LearnRecipeFragmentIntent>();
        app.add_event::<StartTillRequest>();
        app.add_event::<StartRenewRequest>();
        app.add_event::<StartPlantingRequest>();
        app.add_event::<StartHarvestRequest>();
        app.add_event::<StartReplenishRequest>();
        app.add_event::<StartDrainQiRequest>();
        app.add_event::<StartExtractRequestEvent>();
        app.add_event::<CancelExtractRequestEvent>();
        app.add_event::<MineralProbeIntent>();
        app.add_event::<SkillXpGain>();
        app.add_event::<SkillScrollUsed>();
        // plan-shield-block-v1 P1 — 举盾 intent events（ClientRequestDispatchParams 需要）。
        app.add_event::<crate::combat::shield_block::RaiseShieldIntent>();
        app.add_event::<crate::combat::shield_block::LowerShieldIntent>();
        app.add_event::<crate::network::agent_ui::AgentUiResponseEvent>();
        app.add_systems(Update, handle_client_request_payloads);

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let mut skill_set = SkillSet::default();
        skill_set
            .consumed_scrolls
            .insert(ScrollId::new("skill_scroll_herbalism_baicao_can"));
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                inventory_with_skill_scroll(skill_scroll_item(
                    42,
                    "skill_scroll_herbalism_baicao_can",
                )),
                skill_set,
                Cultivation::default(),
                PlayerState::default(),
                QuickSlotBindings::default(),
                UnlockedStyles::default(),
            ))
            .id();
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"learn_skill_scroll","v":1,"instance_id":42}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

        app.update();
        flush_all_client_packets(&mut app);

        let inventory = app.world().get::<PlayerInventory>(entity).unwrap();
        assert_eq!(inventory.containers[0].items.len(), 1);
        assert!(
            has_inventory_snapshot_payload(&mut helper),
            "duplicate rejection must resync inventory after optimistic client drop"
        );
        let xp_events: Vec<_> = app
            .world_mut()
            .resource_mut::<valence::prelude::Events<SkillXpGain>>()
            .drain()
            .collect();
        assert!(xp_events.is_empty());
        let used_events: Vec<_> = app
            .world_mut()
            .resource_mut::<valence::prelude::Events<SkillScrollUsed>>()
            .drain()
            .collect();
        assert_eq!(used_events.len(), 1);
        assert!(used_events[0].was_duplicate);
        assert_eq!(used_events[0].xp_granted, 0);
    }

    #[test]
    fn learn_blueprint_consumes_scroll_item() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        app.init_resource::<ClientRequestBudget>();
        app.insert_resource(TechniqueRegistry::load_for_tests());
        app.insert_resource(CombatClock::default());
        app.insert_resource(GameplayActionQueue::default());
        app.insert_resource(AlchemyMockState::default());
        app.insert_resource(DroppedLootRegistry::default());
        // plan-remains-suite P0 — DroppedLootRequestParams 新增 EventWriter<RemainsLootIntent>。
        app.add_event::<crate::inventory::RemainsLootIntent>();
        app.insert_resource(test_forge_template_registry());
        app.insert_resource(RecipeRegistry::default());
        app.add_event::<CustomPayloadEvent>();
        app.add_event::<BreakthroughRequest>();
        app.add_event::<ForgeRequest>();
        app.add_event::<InsightChosen>();
        app.add_event::<DefenseIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<PlaceFurnaceRequest>();
        app.add_event::<crate::alchemy::LearnRecipeFragmentIntent>();
        app.add_event::<StartTillRequest>();
        app.add_event::<StartRenewRequest>();
        app.add_event::<StartPlantingRequest>();
        app.add_event::<StartHarvestRequest>();
        app.add_event::<StartReplenishRequest>();
        app.add_event::<StartDrainQiRequest>();
        app.add_event::<StartExtractRequestEvent>();
        app.add_event::<CancelExtractRequestEvent>();
        app.add_event::<MineralProbeIntent>();
        app.add_event::<SkillXpGain>();
        app.add_event::<SkillScrollUsed>();
        // plan-shield-block-v1 P1 — 举盾 intent events（ClientRequestDispatchParams 需要）。
        app.add_event::<crate::combat::shield_block::RaiseShieldIntent>();
        app.add_event::<crate::combat::shield_block::LowerShieldIntent>();
        app.add_event::<crate::network::agent_ui::AgentUiResponseEvent>();
        app.add_event::<InscriptionScrollSubmit>();
        app.add_systems(Update, handle_client_request_payloads);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                inventory_with_skill_scroll(skill_scroll_item(42, "blueprint_scroll_ling_feng")),
                Cultivation::default(),
                PlayerState::default(),
                QuickSlotBindings::default(),
                UnlockedStyles::default(),
            ))
            .id();
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"forge_learn_blueprint","v":1,"blueprint_id":"ling_feng_v0"}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

        app.update();
        app.update();

        let inventory = app.world().get::<PlayerInventory>(entity).unwrap();
        assert!(inventory.containers[0].items.is_empty());
        let learned = app.world().get::<LearnedBlueprints>(entity).unwrap();
        assert!(learned.knows("ling_feng_v0"));
    }

    // ══════════ plan-forge-session-entry-wiring-v1 §4.1#2/#3 — 分发层饱和测试 ══════════

    fn send_forge_start_session(
        app: &mut App,
        client: Entity,
        station_pos: (i32, i32, i32),
        blueprint_id: &str,
        materials: &[(&str, u32)],
    ) {
        let materials_json: Vec<String> = materials
            .iter()
            .map(|(m, c)| format!("[\"{m}\",{c}]"))
            .collect();
        let body = format!(
            "{{\"type\":\"forge_start_session\",\"v\":1,\"station_pos\":[{},{},{}],\"blueprint_id\":\"{blueprint_id}\",\"materials\":[{}]}}",
            station_pos.0,
            station_pos.1,
            station_pos.2,
            materials_json.join(",")
        );
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client,
                channel: ident!("bong:client_request").into(),
                data: body.into_bytes().into_boxed_slice(),
            });
    }

    fn send_forge_turn_page(app: &mut App, client: Entity, delta: i32) {
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client,
                channel: ident!("bong:client_request").into(),
                data: format!(r#"{{"type":"forge_blueprint_turn_page","v":1,"delta":{delta}}}"#)
                    .into_bytes()
                    .into_boxed_slice(),
            });
    }

    fn collect_forge_blueprint_books(
        helper: &mut MockClientHelper,
    ) -> Vec<crate::schema::forge::ForgeBlueprintBookDataV1> {
        helper
            .collect_received()
            .0
            .into_iter()
            .filter_map(|frame| {
                let packet = frame.decode::<CustomPayloadS2c>().ok()?;
                if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                    return None;
                }
                let payload = serde_json::from_slice::<ServerDataV1>(packet.data.0 .0).ok()?;
                match payload.payload {
                    ServerDataPayloadV1::ForgeBlueprintBook(data) => Some(*data),
                    _ => None,
                }
            })
            .collect()
    }

    #[test]
    fn forge_start_session_dispatches_start_forge_request_for_owned_station() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.add_event::<StartForgeRequest>();

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        let station = app
            .world_mut()
            .spawn(WeaponForgeStation::placed(
                valence::prelude::BlockPos::new(8, 66, 8),
                1,
                entity,
            ))
            .id();

        send_forge_start_session(
            &mut app,
            entity,
            (8, 66, 8),
            "iron_sword_v0",
            &[("fan_tie", 3)],
        );
        app.update();

        let events = app
            .world()
            .resource::<valence::prelude::Events<StartForgeRequest>>();
        let sent: Vec<_> = events.iter_current_update_events().collect();
        assert_eq!(
            sent.len(),
            1,
            "本人拥有的砧 + 合法 pos 应恰好分发 1 条 StartForgeRequest"
        );
        assert_eq!(sent[0].station, station);
        assert_eq!(sent[0].caster, entity);
        assert_eq!(sent[0].blueprint, "iron_sword_v0");
        assert_eq!(sent[0].materials, vec![("fan_tie".to_string(), 3)]);
        flush_all_client_packets(&mut app);
        assert!(
            collect_game_messages(&mut helper)
                .iter()
                .all(|m| !m.contains("炼器")),
            "受理路径不应发出炼器错误 chat"
        );
    }

    #[test]
    fn forge_start_session_dispatches_for_unclaimed_station_with_no_owner() {
        // owner=None 的砧（系统/公用砧）应放行任何玩家起炉。
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.add_event::<StartForgeRequest>();

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut().spawn(WeaponForgeStation {
            tier: 1,
            owner: None,
            session: None,
            integrity: 1.0,
            pos: Some((8, 66, 8)),
        });

        send_forge_start_session(
            &mut app,
            entity,
            (8, 66, 8),
            "iron_sword_v0",
            &[("fan_tie", 3)],
        );
        app.update();

        let events = app
            .world()
            .resource::<valence::prelude::Events<StartForgeRequest>>();
        assert_eq!(events.iter_current_update_events().count(), 1);
    }

    #[test]
    fn forge_start_session_rejects_missing_station_with_chat_error_and_no_dispatch() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.add_event::<StartForgeRequest>();

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        // 故意不 spawn 任何 WeaponForgeStation。

        send_forge_start_session(
            &mut app,
            entity,
            (8, 66, 8),
            "iron_sword_v0",
            &[("fan_tie", 3)],
        );
        app.update();

        let events = app
            .world()
            .resource::<valence::prelude::Events<StartForgeRequest>>();
        assert_eq!(
            events.iter_current_update_events().count(),
            0,
            "砧不存在时不应分发 StartForgeRequest"
        );
        flush_all_client_packets(&mut app);
        let messages = collect_game_messages(&mut helper);
        assert!(
            messages.iter().any(|m| m.contains("锻炉不存在")),
            "应回执锻炉不存在，实际收到：{messages:?}"
        );
    }

    #[test]
    fn forge_start_session_rejects_station_owned_by_someone_else() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.add_event::<StartForgeRequest>();

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        let (other_bundle, _other_helper) = create_mock_client("Bob");
        let other_owner = app.world_mut().spawn(other_bundle).id();
        app.world_mut().spawn(WeaponForgeStation::placed(
            valence::prelude::BlockPos::new(8, 66, 8),
            1,
            other_owner,
        ));

        send_forge_start_session(
            &mut app,
            entity,
            (8, 66, 8),
            "iron_sword_v0",
            &[("fan_tie", 3)],
        );
        app.update();

        let events = app
            .world()
            .resource::<valence::prelude::Events<StartForgeRequest>>();
        assert_eq!(
            events.iter_current_update_events().count(),
            0,
            "非本人的砧不应分发 StartForgeRequest"
        );
        flush_all_client_packets(&mut app);
        let messages = collect_game_messages(&mut helper);
        assert!(
            messages.iter().any(|m| m.contains("不是你的")),
            "应回执所有权错误，实际收到：{messages:?}"
        );
    }

    fn forge_blueprint_registry_for_tests() -> BlueprintRegistry {
        BlueprintRegistry::load_dir_with_minerals(
            crate::forge::blueprint::DEFAULT_BLUEPRINTS_DIR,
            None,
        )
        .expect("default forge blueprints should load for dispatch tests")
    }

    #[test]
    fn forge_blueprint_turn_page_positive_delta_advances_and_echoes_s2c() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(forge_blueprint_registry_for_tests());

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                LearnedBlueprints {
                    ids: vec![
                        "iron_sword_v0".to_string(),
                        "qing_feng_v0".to_string(),
                        "ling_feng_v0".to_string(),
                    ],
                    current_index: 0,
                },
            ))
            .id();

        send_forge_turn_page(&mut app, entity, 1);
        app.update();

        let learned = app.world().get::<LearnedBlueprints>(entity).unwrap();
        assert_eq!(learned.current_index, 1, "delta=1 应恰好前进 1 页");

        flush_all_client_packets(&mut app);
        let books = collect_forge_blueprint_books(&mut helper);
        assert_eq!(books.len(), 1, "翻页应恰好回推 1 条 forge_blueprint_book");
        assert_eq!(books[0].current_index, 1);
    }

    #[test]
    fn forge_blueprint_turn_page_negative_delta_wraps_to_last_page() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(forge_blueprint_registry_for_tests());

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                LearnedBlueprints {
                    ids: vec![
                        "iron_sword_v0".to_string(),
                        "qing_feng_v0".to_string(),
                        "ling_feng_v0".to_string(),
                    ],
                    current_index: 0,
                },
            ))
            .id();

        send_forge_turn_page(&mut app, entity, -1);
        app.update();

        let learned = app.world().get::<LearnedBlueprints>(entity).unwrap();
        assert_eq!(
            learned.current_index, 2,
            "从第 0 页向前翻应 wrap 到最后一页（索引 2）"
        );
        flush_all_client_packets(&mut app);
        assert_eq!(collect_forge_blueprint_books(&mut helper).len(), 1);
    }

    #[test]
    fn forge_blueprint_turn_page_multi_step_delta_advances_that_many_pages() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(forge_blueprint_registry_for_tests());

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                LearnedBlueprints {
                    ids: vec![
                        "iron_sword_v0".to_string(),
                        "qing_feng_v0".to_string(),
                        "ling_feng_v0".to_string(),
                    ],
                    current_index: 0,
                },
            ))
            .id();

        send_forge_turn_page(&mut app, entity, 2);
        app.update();

        let learned = app.world().get::<LearnedBlueprints>(entity).unwrap();
        assert_eq!(learned.current_index, 2, "delta=2 应前进恰好 2 页");
        flush_all_client_packets(&mut app);
        assert_eq!(collect_forge_blueprint_books(&mut helper).len(), 1);
    }

    #[test]
    fn forge_blueprint_turn_page_extreme_delta_is_bounded_by_len_modulo() {
        // 修复轮 major——恶意单包 delta=i32::MIN（unsigned_abs=2.1B）曾按次循环，
        // 一个包冻结整个 ECS tick 数秒（DoS）。守卫后按 |delta| % len 步进：
        // 2_147_483_648 % 3 = 2，负方向 prev 2 页，0 → 2 → 1，落点必须与逐步等价。
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(forge_blueprint_registry_for_tests());

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                LearnedBlueprints {
                    ids: vec![
                        "iron_sword_v0".to_string(),
                        "qing_feng_v0".to_string(),
                        "ling_feng_v0".to_string(),
                    ],
                    current_index: 0,
                },
            ))
            .id();

        send_forge_turn_page(&mut app, entity, i32::MIN);
        app.update();

        let learned = app.world().get::<LearnedBlueprints>(entity).unwrap();
        assert_eq!(
            learned.current_index, 1,
            "i32::MIN 应按 2.1B % 3 = 2 步 prev 处理（0→2→1），且不冻结 tick"
        );
        flush_all_client_packets(&mut app);
        assert_eq!(
            collect_forge_blueprint_books(&mut helper).len(),
            1,
            "极端 delta 仍应回推一次 S2C（server 权威页码）"
        );
    }

    #[test]
    fn forge_blueprint_turn_page_delta_multiple_of_len_is_identity_but_echoes() {
        // 边界：|delta| 恰为 len 的整数倍 → %len 后 0 步，页码不动；但请求本身
        // 合法，仍回推 S2C（与 delta=0 的静默 noop 区分——那是无意义输入）。
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(forge_blueprint_registry_for_tests());

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                LearnedBlueprints {
                    ids: vec![
                        "iron_sword_v0".to_string(),
                        "qing_feng_v0".to_string(),
                        "ling_feng_v0".to_string(),
                    ],
                    current_index: 1,
                },
            ))
            .id();

        send_forge_turn_page(&mut app, entity, 3);
        app.update();

        let learned = app.world().get::<LearnedBlueprints>(entity).unwrap();
        assert_eq!(learned.current_index, 1, "delta=len(3) 环回原页");
        flush_all_client_packets(&mut app);
        assert_eq!(collect_forge_blueprint_books(&mut helper).len(), 1);
    }

    #[test]
    fn forge_blueprint_turn_page_delta_zero_is_noop_no_s2c() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(forge_blueprint_registry_for_tests());

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                LearnedBlueprints {
                    ids: vec!["iron_sword_v0".to_string()],
                    current_index: 0,
                },
            ))
            .id();

        send_forge_turn_page(&mut app, entity, 0);
        app.update();

        let learned = app.world().get::<LearnedBlueprints>(entity).unwrap();
        assert_eq!(learned.current_index, 0, "delta=0 不应改变页码");
        flush_all_client_packets(&mut app);
        assert!(
            collect_forge_blueprint_books(&mut helper).is_empty(),
            "delta=0 不应回推 S2C"
        );
    }

    #[test]
    fn forge_blueprint_turn_page_noop_when_never_learned_any_blueprint() {
        // LearnedBlueprints 组件懒插入：从未学过图谱的玩家没有这个组件，无书可翻。
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(forge_blueprint_registry_for_tests());

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();

        send_forge_turn_page(&mut app, entity, 1);
        app.update();

        assert!(
            app.world().get::<LearnedBlueprints>(entity).is_none(),
            "不应凭空创建 LearnedBlueprints 组件"
        );
        flush_all_client_packets(&mut app);
        assert!(collect_forge_blueprint_books(&mut helper).is_empty());
    }

    #[test]
    fn forge_blueprint_turn_page_noop_when_learned_list_empty() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(forge_blueprint_registry_for_tests());

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                LearnedBlueprints {
                    ids: vec![],
                    current_index: 0,
                },
            ))
            .id();

        send_forge_turn_page(&mut app, entity, 1);
        app.update();

        let learned = app.world().get::<LearnedBlueprints>(entity).unwrap();
        assert_eq!(learned.current_index, 0, "空图谱列表翻页应无操作");
        flush_all_client_packets(&mut app);
        assert!(collect_forge_blueprint_books(&mut helper).is_empty());
    }

    #[test]
    fn forge_inscription_scroll_defers_consumption_and_emits_exact_item_event() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        app.insert_resource(TechniqueRegistry::load_for_tests());
        app.insert_resource(CapturedInscriptionScrolls::default());
        app.insert_resource(CombatClock::default());
        app.insert_resource(GameplayActionQueue::default());
        app.insert_resource(AlchemyMockState::default());
        app.insert_resource(DroppedLootRegistry::default());
        // plan-remains-suite P0 — DroppedLootRequestParams 新增 EventWriter<RemainsLootIntent>。
        app.add_event::<crate::inventory::RemainsLootIntent>();
        app.insert_resource(test_forge_template_registry());
        app.insert_resource(RecipeRegistry::default());
        app.add_event::<CustomPayloadEvent>();
        app.add_event::<BreakthroughRequest>();
        app.add_event::<ForgeRequest>();
        app.add_event::<InsightChosen>();
        app.add_event::<DefenseIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<PlaceFurnaceRequest>();
        app.add_event::<crate::alchemy::LearnRecipeFragmentIntent>();
        app.add_event::<StartTillRequest>();
        app.add_event::<StartRenewRequest>();
        app.add_event::<StartPlantingRequest>();
        app.add_event::<StartHarvestRequest>();
        app.add_event::<StartReplenishRequest>();
        app.add_event::<StartDrainQiRequest>();
        app.add_event::<StartExtractRequestEvent>();
        app.add_event::<CancelExtractRequestEvent>();
        app.add_event::<MineralProbeIntent>();
        app.add_event::<SkillXpGain>();
        app.add_event::<SkillScrollUsed>();
        // plan-shield-block-v1 P1 — 举盾 intent events（ClientRequestDispatchParams 需要）。
        app.add_event::<crate::combat::shield_block::RaiseShieldIntent>();
        app.add_event::<crate::combat::shield_block::LowerShieldIntent>();
        app.add_event::<crate::network::agent_ui::AgentUiResponseEvent>();
        app.add_event::<InscriptionScrollSubmit>();
        app.add_systems(
            Update,
            (handle_client_request_payloads, capture_inscription_scrolls).chain(),
        );

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                inventory_with_skill_scroll(skill_scroll_item(43, "inscription_scroll_sharp_v0")),
                Cultivation::default(),
                PlayerState::default(),
                QuickSlotBindings::default(),
                UnlockedStyles::default(),
            ))
            .id();
        insert_test_forge_session(&mut app, 9, entity, ForgeStep::Inscription);
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"forge_inscription_scroll","v":1,"session_id":9,"inscription_id":"sharp_v0"}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

        app.update();

        let inventory = app.world().get::<PlayerInventory>(entity).unwrap();
        assert_eq!(
            inventory.containers[0].items.len(),
            1,
            "C2S 网关只能预检残卷，实际消费必须留给确认进入 Inscription 的 forge 系统"
        );
        let captured = app.world().resource::<CapturedInscriptionScrolls>();
        assert_eq!(captured.0.len(), 1);
        assert_eq!(captured.0[0].session, ForgeSessionId(9));
        assert_eq!(captured.0[0].caster, entity);
        assert_eq!(captured.0[0].item_instance_id, 43);
        assert_eq!(captured.0[0].inscription_id, "sharp_v0");
    }

    #[test]
    fn forge_inscription_scroll_rejects_invalid_session_before_consuming_item() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        app.insert_resource(TechniqueRegistry::load_for_tests());
        app.insert_resource(CapturedInscriptionScrolls::default());
        app.insert_resource(CombatClock::default());
        app.insert_resource(GameplayActionQueue::default());
        app.insert_resource(AlchemyMockState::default());
        app.insert_resource(DroppedLootRegistry::default());
        // plan-remains-suite P0 — DroppedLootRequestParams 新增 EventWriter<RemainsLootIntent>。
        app.add_event::<crate::inventory::RemainsLootIntent>();
        app.insert_resource(test_forge_template_registry());
        app.insert_resource(RecipeRegistry::default());
        app.add_event::<CustomPayloadEvent>();
        app.add_event::<BreakthroughRequest>();
        app.add_event::<ForgeRequest>();
        app.add_event::<InsightChosen>();
        app.add_event::<DefenseIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<PlaceFurnaceRequest>();
        app.add_event::<crate::alchemy::LearnRecipeFragmentIntent>();
        app.add_event::<StartTillRequest>();
        app.add_event::<StartRenewRequest>();
        app.add_event::<StartPlantingRequest>();
        app.add_event::<StartHarvestRequest>();
        app.add_event::<StartReplenishRequest>();
        app.add_event::<StartDrainQiRequest>();
        app.add_event::<StartExtractRequestEvent>();
        app.add_event::<CancelExtractRequestEvent>();
        app.add_event::<MineralProbeIntent>();
        app.add_event::<SkillXpGain>();
        app.add_event::<SkillScrollUsed>();
        // plan-shield-block-v1 P1 — 举盾 intent events（ClientRequestDispatchParams 需要）。
        app.add_event::<crate::combat::shield_block::RaiseShieldIntent>();
        app.add_event::<crate::combat::shield_block::LowerShieldIntent>();
        app.add_event::<crate::network::agent_ui::AgentUiResponseEvent>();
        app.add_event::<InscriptionScrollSubmit>();
        app.add_systems(
            Update,
            (handle_client_request_payloads, capture_inscription_scrolls).chain(),
        );

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                inventory_with_skill_scroll(skill_scroll_item(43, "inscription_scroll_sharp_v0")),
                Cultivation::default(),
                PlayerState::default(),
                QuickSlotBindings::default(),
                UnlockedStyles::default(),
            ))
            .id();
        insert_test_forge_session(&mut app, 9, entity, ForgeStep::Tempering);
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"forge_inscription_scroll","v":1,"session_id":9,"inscription_id":"sharp_v0"}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

        app.update();

        let inventory = app.world().get::<PlayerInventory>(entity).unwrap();
        assert_eq!(inventory.containers[0].items.len(), 1);
        let captured = app.world().resource::<CapturedInscriptionScrolls>();
        assert!(captured.0.is_empty());
    }

    #[test]
    fn forge_tempering_hit_emits_event() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        app.insert_resource(TechniqueRegistry::load_for_tests());
        app.insert_resource(CapturedTemperingHits::default());
        app.insert_resource(CombatClock::default());
        app.insert_resource(GameplayActionQueue::default());
        app.insert_resource(AlchemyMockState::default());
        app.insert_resource(DroppedLootRegistry::default());
        // plan-remains-suite P0 — DroppedLootRequestParams 新增 EventWriter<RemainsLootIntent>。
        app.add_event::<crate::inventory::RemainsLootIntent>();
        app.insert_resource(ItemRegistry::default());
        app.insert_resource(RecipeRegistry::default());
        app.add_event::<CustomPayloadEvent>();
        app.add_event::<BreakthroughRequest>();
        app.add_event::<ForgeRequest>();
        app.add_event::<InsightChosen>();
        app.add_event::<DefenseIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<PlaceFurnaceRequest>();
        app.add_event::<crate::alchemy::LearnRecipeFragmentIntent>();
        app.add_event::<StartTillRequest>();
        app.add_event::<StartRenewRequest>();
        app.add_event::<StartPlantingRequest>();
        app.add_event::<StartHarvestRequest>();
        app.add_event::<StartReplenishRequest>();
        app.add_event::<StartDrainQiRequest>();
        app.add_event::<StartExtractRequestEvent>();
        app.add_event::<CancelExtractRequestEvent>();
        app.add_event::<MineralProbeIntent>();
        app.add_event::<SkillXpGain>();
        app.add_event::<SkillScrollUsed>();
        // plan-shield-block-v1 P1 — 举盾 intent events（ClientRequestDispatchParams 需要）。
        app.add_event::<crate::combat::shield_block::RaiseShieldIntent>();
        app.add_event::<crate::combat::shield_block::LowerShieldIntent>();
        app.add_event::<crate::network::agent_ui::AgentUiResponseEvent>();
        app.add_event::<TemperingHit>();
        app.add_systems(
            Update,
            (handle_client_request_payloads, capture_tempering_hits).chain(),
        );

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        insert_test_forge_session(&mut app, 9, entity, ForgeStep::Tempering);
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"forge_tempering_hit","v":1,"session_id":9,"beat":"H","ticks_remaining":4}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

        app.update();

        let captured = app.world().resource::<CapturedTemperingHits>();
        assert_eq!(captured.0.len(), 1);
        assert_eq!(captured.0[0].session, ForgeSessionId(9));
        assert_eq!(captured.0[0].beat, TemperBeat::Heavy);
        assert_eq!(captured.0[0].ticks_remaining, 4);
    }

    #[test]
    fn forge_tempering_hit_rejects_unknown_beat() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        app.insert_resource(TechniqueRegistry::load_for_tests());
        app.insert_resource(CapturedTemperingHits::default());
        app.insert_resource(CombatClock::default());
        app.insert_resource(GameplayActionQueue::default());
        app.insert_resource(AlchemyMockState::default());
        app.insert_resource(DroppedLootRegistry::default());
        // plan-remains-suite P0 — DroppedLootRequestParams 新增 EventWriter<RemainsLootIntent>。
        app.add_event::<crate::inventory::RemainsLootIntent>();
        app.insert_resource(ItemRegistry::default());
        app.insert_resource(RecipeRegistry::default());
        app.add_event::<CustomPayloadEvent>();
        app.add_event::<BreakthroughRequest>();
        app.add_event::<ForgeRequest>();
        app.add_event::<InsightChosen>();
        app.add_event::<DefenseIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<PlaceFurnaceRequest>();
        app.add_event::<crate::alchemy::LearnRecipeFragmentIntent>();
        app.add_event::<StartTillRequest>();
        app.add_event::<StartRenewRequest>();
        app.add_event::<StartPlantingRequest>();
        app.add_event::<StartHarvestRequest>();
        app.add_event::<StartReplenishRequest>();
        app.add_event::<StartDrainQiRequest>();
        app.add_event::<StartExtractRequestEvent>();
        app.add_event::<CancelExtractRequestEvent>();
        app.add_event::<MineralProbeIntent>();
        app.add_event::<SkillXpGain>();
        app.add_event::<SkillScrollUsed>();
        // plan-shield-block-v1 P1 — 举盾 intent events（ClientRequestDispatchParams 需要）。
        app.add_event::<crate::combat::shield_block::RaiseShieldIntent>();
        app.add_event::<crate::combat::shield_block::LowerShieldIntent>();
        app.add_event::<crate::network::agent_ui::AgentUiResponseEvent>();
        app.add_event::<TemperingHit>();
        app.add_systems(
            Update,
            (handle_client_request_payloads, capture_tempering_hits).chain(),
        );

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"forge_tempering_hit","v":1,"session_id":9,"beat":"X","ticks_remaining":4}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

        app.update();

        let captured = app.world().resource::<CapturedTemperingHits>();
        assert!(captured.0.is_empty());
    }

    #[test]
    fn forge_consecration_inject_emits_event() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        app.insert_resource(TechniqueRegistry::load_for_tests());
        app.insert_resource(CapturedConsecrationInjects::default());
        app.insert_resource(CombatClock::default());
        app.insert_resource(GameplayActionQueue::default());
        app.insert_resource(AlchemyMockState::default());
        app.insert_resource(DroppedLootRegistry::default());
        // plan-remains-suite P0 — DroppedLootRequestParams 新增 EventWriter<RemainsLootIntent>。
        app.add_event::<crate::inventory::RemainsLootIntent>();
        app.insert_resource(ItemRegistry::default());
        app.insert_resource(RecipeRegistry::default());
        app.add_event::<CustomPayloadEvent>();
        app.add_event::<BreakthroughRequest>();
        app.add_event::<ForgeRequest>();
        app.add_event::<InsightChosen>();
        app.add_event::<DefenseIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<PlaceFurnaceRequest>();
        app.add_event::<crate::alchemy::LearnRecipeFragmentIntent>();
        app.add_event::<StartTillRequest>();
        app.add_event::<StartRenewRequest>();
        app.add_event::<StartPlantingRequest>();
        app.add_event::<StartHarvestRequest>();
        app.add_event::<StartReplenishRequest>();
        app.add_event::<StartDrainQiRequest>();
        app.add_event::<StartExtractRequestEvent>();
        app.add_event::<CancelExtractRequestEvent>();
        app.add_event::<MineralProbeIntent>();
        app.add_event::<SkillXpGain>();
        app.add_event::<SkillScrollUsed>();
        // plan-shield-block-v1 P1 — 举盾 intent events（ClientRequestDispatchParams 需要）。
        app.add_event::<crate::combat::shield_block::RaiseShieldIntent>();
        app.add_event::<crate::combat::shield_block::LowerShieldIntent>();
        app.add_event::<crate::network::agent_ui::AgentUiResponseEvent>();
        app.add_event::<ConsecrationInject>();
        app.add_systems(
            Update,
            (handle_client_request_payloads, capture_consecration_injects).chain(),
        );

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        insert_test_forge_session(&mut app, 11, entity, ForgeStep::Consecration);
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data:
                    br#"{"type":"forge_consecration_inject","v":1,"session_id":11,"qi_amount":2.5}"#
                        .to_vec()
                        .into_boxed_slice(),
            });

        app.update();

        let captured = app.world().resource::<CapturedConsecrationInjects>();
        assert_eq!(captured.0.len(), 1);
        assert_eq!(captured.0[0].session, ForgeSessionId(11));
        assert_eq!(captured.0[0].qi_amount, 2.5);
    }

    #[test]
    fn forge_consecration_inject_rejects_negative_qi() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        app.insert_resource(TechniqueRegistry::load_for_tests());
        app.insert_resource(CapturedConsecrationInjects::default());
        app.insert_resource(CombatClock::default());
        app.insert_resource(GameplayActionQueue::default());
        app.insert_resource(AlchemyMockState::default());
        app.insert_resource(DroppedLootRegistry::default());
        // plan-remains-suite P0 — DroppedLootRequestParams 新增 EventWriter<RemainsLootIntent>。
        app.add_event::<crate::inventory::RemainsLootIntent>();
        app.insert_resource(ItemRegistry::default());
        app.insert_resource(RecipeRegistry::default());
        app.add_event::<CustomPayloadEvent>();
        app.add_event::<BreakthroughRequest>();
        app.add_event::<ForgeRequest>();
        app.add_event::<InsightChosen>();
        app.add_event::<DefenseIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<PlaceFurnaceRequest>();
        app.add_event::<crate::alchemy::LearnRecipeFragmentIntent>();
        app.add_event::<StartTillRequest>();
        app.add_event::<StartRenewRequest>();
        app.add_event::<StartPlantingRequest>();
        app.add_event::<StartHarvestRequest>();
        app.add_event::<StartReplenishRequest>();
        app.add_event::<StartDrainQiRequest>();
        app.add_event::<StartExtractRequestEvent>();
        app.add_event::<CancelExtractRequestEvent>();
        app.add_event::<MineralProbeIntent>();
        app.add_event::<SkillXpGain>();
        app.add_event::<SkillScrollUsed>();
        // plan-shield-block-v1 P1 — 举盾 intent events（ClientRequestDispatchParams 需要）。
        app.add_event::<crate::combat::shield_block::RaiseShieldIntent>();
        app.add_event::<crate::combat::shield_block::LowerShieldIntent>();
        app.add_event::<crate::network::agent_ui::AgentUiResponseEvent>();
        app.add_event::<ConsecrationInject>();
        app.add_systems(
            Update,
            (handle_client_request_payloads, capture_consecration_injects).chain(),
        );

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"forge_consecration_inject","v":1,"session_id":11,"qi_amount":-0.5}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

        app.update();

        let captured = app.world().resource::<CapturedConsecrationInjects>();
        assert!(captured.0.is_empty());
    }

    #[test]
    fn forge_step_advance_emits_event() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        app.insert_resource(TechniqueRegistry::load_for_tests());
        app.insert_resource(CapturedStepAdvances::default());
        app.insert_resource(CombatClock::default());
        app.insert_resource(GameplayActionQueue::default());
        app.insert_resource(AlchemyMockState::default());
        app.insert_resource(DroppedLootRegistry::default());
        // plan-remains-suite P0 — DroppedLootRequestParams 新增 EventWriter<RemainsLootIntent>。
        app.add_event::<crate::inventory::RemainsLootIntent>();
        app.insert_resource(ItemRegistry::default());
        app.insert_resource(RecipeRegistry::default());
        app.add_event::<CustomPayloadEvent>();
        app.add_event::<BreakthroughRequest>();
        app.add_event::<ForgeRequest>();
        app.add_event::<InsightChosen>();
        app.add_event::<DefenseIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<PlaceFurnaceRequest>();
        app.add_event::<crate::alchemy::LearnRecipeFragmentIntent>();
        app.add_event::<StartTillRequest>();
        app.add_event::<StartRenewRequest>();
        app.add_event::<StartPlantingRequest>();
        app.add_event::<StartHarvestRequest>();
        app.add_event::<StartReplenishRequest>();
        app.add_event::<StartDrainQiRequest>();
        app.add_event::<StartExtractRequestEvent>();
        app.add_event::<CancelExtractRequestEvent>();
        app.add_event::<MineralProbeIntent>();
        app.add_event::<SkillXpGain>();
        app.add_event::<SkillScrollUsed>();
        // plan-shield-block-v1 P1 — 举盾 intent events（ClientRequestDispatchParams 需要）。
        app.add_event::<crate::combat::shield_block::RaiseShieldIntent>();
        app.add_event::<crate::combat::shield_block::LowerShieldIntent>();
        app.add_event::<crate::network::agent_ui::AgentUiResponseEvent>();
        app.add_event::<StepAdvance>();
        app.add_systems(
            Update,
            (handle_client_request_payloads, capture_step_advances).chain(),
        );

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        insert_test_forge_session(&mut app, 12, entity, ForgeStep::Tempering);
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"forge_step_advance","v":1,"session_id":12}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

        app.update();

        let captured = app.world().resource::<CapturedStepAdvances>();
        assert_eq!(captured.0.len(), 1);
        assert_eq!(captured.0[0].session, ForgeSessionId(12));
        assert_eq!(captured.0[0].from_step, ForgeStep::Tempering);
    }

    #[test]
    fn forge_session_inputs_reject_wrong_caster() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        app.insert_resource(TechniqueRegistry::load_for_tests());
        app.insert_resource(CapturedTemperingHits::default());
        app.insert_resource(CapturedConsecrationInjects::default());
        app.insert_resource(CapturedStepAdvances::default());
        app.insert_resource(CombatClock::default());
        app.insert_resource(GameplayActionQueue::default());
        app.insert_resource(AlchemyMockState::default());
        app.insert_resource(DroppedLootRegistry::default());
        // plan-remains-suite P0 — DroppedLootRequestParams 新增 EventWriter<RemainsLootIntent>。
        app.add_event::<crate::inventory::RemainsLootIntent>();
        app.insert_resource(ItemRegistry::default());
        app.insert_resource(RecipeRegistry::default());
        app.add_event::<CustomPayloadEvent>();
        app.add_event::<BreakthroughRequest>();
        app.add_event::<ForgeRequest>();
        app.add_event::<InsightChosen>();
        app.add_event::<DefenseIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<PlaceFurnaceRequest>();
        app.add_event::<crate::alchemy::LearnRecipeFragmentIntent>();
        app.add_event::<StartTillRequest>();
        app.add_event::<StartRenewRequest>();
        app.add_event::<StartPlantingRequest>();
        app.add_event::<StartHarvestRequest>();
        app.add_event::<StartReplenishRequest>();
        app.add_event::<StartDrainQiRequest>();
        app.add_event::<StartExtractRequestEvent>();
        app.add_event::<CancelExtractRequestEvent>();
        app.add_event::<MineralProbeIntent>();
        app.add_event::<SkillXpGain>();
        app.add_event::<SkillScrollUsed>();
        // plan-shield-block-v1 P1 — 举盾 intent events（ClientRequestDispatchParams 需要）。
        app.add_event::<crate::combat::shield_block::RaiseShieldIntent>();
        app.add_event::<crate::combat::shield_block::LowerShieldIntent>();
        app.add_event::<crate::network::agent_ui::AgentUiResponseEvent>();
        app.add_event::<TemperingHit>();
        app.add_event::<ConsecrationInject>();
        app.add_event::<StepAdvance>();
        app.add_systems(
            Update,
            (
                handle_client_request_payloads,
                capture_tempering_hits,
                capture_consecration_injects,
                capture_step_advances,
            )
                .chain(),
        );

        let (owner_bundle, _owner_helper) = create_mock_client("Owner");
        let owner = app.world_mut().spawn(owner_bundle).id();
        let (attacker_bundle, _attacker_helper) = create_mock_client("Attacker");
        let attacker = app.world_mut().spawn(attacker_bundle).id();

        insert_test_forge_session(&mut app, 21, owner, ForgeStep::Tempering);
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: attacker,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"forge_tempering_hit","v":1,"session_id":21,"beat":"H","ticks_remaining":4}"#
                    .to_vec()
                    .into_boxed_slice(),
            });
        app.update();
        assert!(app.world().resource::<CapturedTemperingHits>().0.is_empty());

        insert_test_forge_session(&mut app, 22, owner, ForgeStep::Consecration);
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: attacker,
                channel: ident!("bong:client_request").into(),
                data:
                    br#"{"type":"forge_consecration_inject","v":1,"session_id":22,"qi_amount":2.5}"#
                        .to_vec()
                        .into_boxed_slice(),
            });
        app.update();
        assert!(app
            .world()
            .resource::<CapturedConsecrationInjects>()
            .0
            .is_empty());

        insert_test_forge_session(&mut app, 23, owner, ForgeStep::Tempering);
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: attacker,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"forge_step_advance","v":1,"session_id":23}"#
                    .to_vec()
                    .into_boxed_slice(),
            });
        app.update();
        assert!(app.world().resource::<CapturedStepAdvances>().0.is_empty());
    }

    #[test]
    fn skill_bar_bind_skill_then_cast_starts_skillbar_cast() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let target = app.world_mut().spawn(Position::new([1.0, 0.0, 0.0])).id();
        let entity = app.world_mut().spawn(client_bundle).id();
        // beng_quan 需要 LargeIntestine/SmallIntestine/TripleEnergizer opened=true + integrity ≥ 0.01
        let mut ms = crate::cultivation::components::MeridianSystem::default();
        for id in [
            crate::cultivation::components::MeridianId::LargeIntestine,
            crate::cultivation::components::MeridianId::SmallIntestine,
            crate::cultivation::components::MeridianId::TripleEnergizer,
        ] {
            let m = ms.get_mut(id);
            m.opened = true;
            m.integrity = 1.0;
        }
        app.world_mut().entity_mut(entity).insert((
            Position::new([0.0, 0.0, 0.0]),
            crate::cultivation::components::Cultivation {
                realm: crate::cultivation::components::Realm::Induce,
                qi_current: 100.0,
                qi_max: 100.0,
                ..Default::default()
            },
            ms,
            SkillBarBindings::default(),
            QuickSlotBindings::default(),
            empty_inventory(),
            known(&["burst_meridian.beng_quan"]),
        ));
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"skill_bar_bind","v":1,"slot":0,"binding":{"kind":"skill","skill_id":"burst_meridian.beng_quan"}}"#
                    .to_vec()
                    .into_boxed_slice(),
            });
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: serde_json::to_vec(&ClientRequestV1::SkillBarCast {
                    v: 1,
                    slot: 0,
                    target: Some(format!("entity_bits:{}", target.to_bits())),
                })
                .unwrap()
                .into_boxed_slice(),
            });

        app.update();

        let bindings = app.world().get::<SkillBarBindings>(entity).unwrap();
        assert!(matches!(
            &bindings.slots[0],
            SkillSlot::Skill { skill_id } if skill_id == "burst_meridian.beng_quan"
        ));
        let casting = app.world().get::<Casting>(entity).unwrap();
        assert_eq!(casting.source, CastSource::SkillBar);
        assert_eq!(casting.slot, 0);
        assert_eq!(casting.bound_instance_id, None);
        assert_eq!(casting.duration_ticks, 8);
        assert_eq!(casting.complete_cooldown_ticks, 60);
    }

    #[test]
    fn runtime_only_direct_generic_can_be_learned_bound_and_cast() {
        const TECHNIQUE_ID: &str = "test.runtime_only_direct";
        const SCROLL_TEMPLATE_ID: &str = "test_runtime_only_direct_scroll";
        const SCROLL_INSTANCE_ID: u64 = 91_001;

        let mut definition = TechniqueRegistry::load_for_tests()
            .get("movement.dash")
            .expect("direct-generic fixture must exist")
            .clone();
        definition.id = TECHNIQUE_ID.to_string();
        definition.display_name = "运行时直施".to_string();
        definition.cast_ticks = 17;
        definition.cooldown_ticks = 83;
        definition.required_meridians.clear();
        let registry = TechniqueRegistry::load_for_tests_with_definition(definition);

        let mut scroll_template = ItemTemplate::minimal_for_test(SCROLL_TEMPLATE_ID);
        scroll_template.category = ItemCategory::Scroll;
        scroll_template.technique_scroll_spec = Some(crate::inventory::TechniqueScrollSpec {
            kind: "technique".to_string(),
            skill_id: TECHNIQUE_ID.to_string(),
        });
        let item_registry = ItemRegistry::from_map(HashMap::from([(
            SCROLL_TEMPLATE_ID.to_string(),
            scroll_template,
        )]));

        let mut app = App::new();
        register_request_app(&mut app);
        app.insert_resource(registry);
        app.insert_resource(item_registry);
        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                crate::cultivation::components::Cultivation::default(),
                crate::cultivation::components::MeridianSystem::default(),
                SkillBarBindings::default(),
                QuickSlotBindings::default(),
                inventory_with_skill_scroll(skill_scroll_item(
                    SCROLL_INSTANCE_ID,
                    SCROLL_TEMPLATE_ID,
                )),
                KnownTechniques::default(),
            ))
            .id();

        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: serde_json::to_vec(&ClientRequestV1::TechniqueScrollUse {
                    v: 1,
                    instance_id: SCROLL_INSTANCE_ID,
                })
                .expect("technique-scroll request should serialize")
                .into_boxed_slice(),
            });
        app.update();

        let known = app.world().get::<KnownTechniques>(entity).unwrap();
        assert!(
            known
                .entries
                .iter()
                .any(|entry| entry.id == TECHNIQUE_ID && entry.active),
            "request-level scroll use must learn and activate a runtime-only technique"
        );
        let inventory = app.world().get::<PlayerInventory>(entity).unwrap();
        assert!(
            inventory_item_by_instance_borrow(inventory, SCROLL_INSTANCE_ID).is_none(),
            "successful request-level learning must consume the exact scroll instance"
        );

        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"skill_bar_bind","v":1,"slot":0,"binding":{"kind":"skill","skill_id":"test.runtime_only_direct"}}"#
                    .to_vec()
                    .into_boxed_slice(),
            });
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: serde_json::to_vec(&ClientRequestV1::SkillBarCast {
                    v: 1,
                    slot: 0,
                    target: None,
                })
                .unwrap()
                .into_boxed_slice(),
            });

        app.update();

        assert!(matches!(
            app.world()
                .get::<SkillBarBindings>(entity)
                .unwrap()
                .get(0),
            Some(SkillSlot::Skill { skill_id }) if skill_id == "test.runtime_only_direct"
        ));
        let casting = app
            .world()
            .get::<Casting>(entity)
            .expect("runtime-only direct-generic cast must start");
        assert_eq!(
            casting.skill_id.as_deref(),
            Some("test.runtime_only_direct")
        );
        assert_eq!(casting.duration_ticks, 17);
        assert_eq!(casting.complete_cooldown_ticks, 83);
    }

    /// 槽位 3 绑定崩拳——「主动切槽取消」用例里那条**通过全部门禁**的新 cast
    /// （空槽位/未学会都会在 cancel 判定之前早退，测不到取消路径）。
    fn slot3_bound_to_beng_quan() -> SkillBarBindings {
        let mut bindings = SkillBarBindings::default();
        bindings.slots[3] = SkillSlot::Skill {
            skill_id: "burst_meridian.beng_quan".to_string(),
        };
        bindings
    }

    /// 崩拳的经脉前置（大肠/小肠/三焦 opened + integrity 足量）。
    fn beng_quan_ready_meridians() -> crate::cultivation::components::MeridianSystem {
        let mut ms = crate::cultivation::components::MeridianSystem::default();
        for id in [
            crate::cultivation::components::MeridianId::LargeIntestine,
            crate::cultivation::components::MeridianId::SmallIntestine,
            crate::cultivation::components::MeridianId::TripleEnergizer,
        ] {
            let m = ms.get_mut(id);
            m.opened = true;
            m.integrity = 1.0;
        }
        ms
    }

    /// 引导中的施法快照（槽位 0），供「主动切槽取消」两用例复用。
    fn yidao_charge_casting(skill_id: &str) -> Casting {
        Casting {
            source: CastSource::SkillBar,
            slot: 0,
            started_at_tick: 0,
            duration_ticks: 1200,
            started_at_ms: 0,
            duration_ms: 60_000,
            bound_instance_id: None,
            start_position: DVec3::new(0.0, 64.0, 0.0),
            complete_cooldown_ticks: 60,
            skill_id: Some(skill_id.to_string()),
            skill_config: None,
        }
    }

    /// plan-skill-anim-fidelity-v1 P4（review r1 补）——**用户主动切槽取消**是
    /// `tick_casts_or_interrupt` 三打断分支之外的第四条退出路径：`Casting` 在
    /// `cancel_previous_cast` 里被提前 remove，那边再也看不到它。若此处不补发
    /// StopAnim，`bong:yidao_*_loop` 这类 isLoop 蓄力段会永卡客户端（yidao 引导
    /// 窗长达 60s，命中概率远高于 sword.infuse）。
    #[test]
    fn user_cancel_by_slot_switch_stops_looping_charge_anim() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        let unique_id = UniqueId::default();
        let expected_target = unique_id.0.to_string();
        app.world_mut().entity_mut(entity).insert((
            Position::new([0.0, 64.0, 0.0]),
            unique_id,
            crate::cultivation::components::Cultivation {
                realm: crate::cultivation::components::Realm::Induce,
                qi_current: 100.0,
                qi_max: 100.0,
                ..Default::default()
            },
            beng_quan_ready_meridians(),
            slot3_bound_to_beng_quan(),
            QuickSlotBindings::default(),
            empty_inventory(),
            known(&["burst_meridian.beng_quan"]),
            // 引导中的接经术（循环蓄力段已在客户端播放）。
            yidao_charge_casting(crate::combat::yidao::MERIDIAN_REPAIR_SKILL_ID),
        ));

        // 切到另一个槽位施法 → 走 cancel_previous_cast（UserCancel）。
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: serde_json::to_vec(&ClientRequestV1::SkillBarCast {
                    v: 1,
                    slot: 3,
                    target: None,
                })
                .unwrap()
                .into_boxed_slice(),
            });

        app.update();

        let stop_anims: Vec<(String, Option<u8>)> = app
            .world_mut()
            .resource_mut::<valence::prelude::Events<VfxEventRequest>>()
            .drain()
            .filter_map(|request| match request.payload {
                VfxEventPayloadV1::StopAnim {
                    target_player,
                    anim_id,
                    fade_out_ticks,
                } => {
                    assert_eq!(
                        target_player, expected_target,
                        "StopAnim 必须寻址到取消施法的玩家本人"
                    );
                    Some((anim_id, fade_out_ticks))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            stop_anims,
            vec![(
                crate::combat::yidao::ANIM_YIDAO_MERIDIAN_REPAIR_LOOP.to_string(),
                Some(crate::network::cast_emit::CAST_LOOP_ANIM_CANCEL_FADE_OUT_TICKS),
            )],
            "主动切槽取消必须恰停一次被取消招的循环蓄力段（否则动画永卡客户端）"
        );
    }

    /// 负向：被取消的招式**没有**登记循环蓄力段时，取消路径不得发多余 StopAnim
    /// （查表 miss = 该招本就没有需要停的循环动画）。
    #[test]
    fn user_cancel_of_non_looping_cast_emits_no_stop_anim() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut().entity_mut(entity).insert((
            Position::new([0.0, 64.0, 0.0]),
            UniqueId::default(),
            crate::cultivation::components::Cultivation {
                realm: crate::cultivation::components::Realm::Induce,
                qi_current: 100.0,
                qi_max: 100.0,
                ..Default::default()
            },
            beng_quan_ready_meridians(),
            slot3_bound_to_beng_quan(),
            QuickSlotBindings::default(),
            empty_inventory(),
            known(&["burst_meridian.beng_quan"]),
            // 被取消的招式未登记循环蓄力段（崩拳是瞬发三段式）。
            yidao_charge_casting("burst_meridian.beng_quan"),
        ));

        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: serde_json::to_vec(&ClientRequestV1::SkillBarCast {
                    v: 1,
                    slot: 3,
                    target: None,
                })
                .unwrap()
                .into_boxed_slice(),
            });

        app.update();

        let stop_anim_count = app
            .world_mut()
            .resource_mut::<valence::prelude::Events<VfxEventRequest>>()
            .drain()
            .filter(|request| matches!(request.payload, VfxEventPayloadV1::StopAnim { .. }))
            .count();
        assert_eq!(
            stop_anim_count, 0,
            "非循环段招式被取消时不得发 StopAnim（查表 miss 即无循环动画需要停）"
        );
    }

    #[test]
    fn skill_bar_cast_defined_skill_without_resolver_uses_generic_cast_path() {
        // body.guangbo_ticao 是仍未实装 resolver 的 skeleton 招（不在 SkillRegistry 内，
        // 无 required_meridians、无 SkillMeridianDependencies）→ 走通用施法路径，
        // 通用路径无条件插入 Casting 并把 SkillConfigStore 里的配置带入 Casting.skill_config。
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.world_mut()
            .resource_mut::<SkillConfigStore>()
            .set_config(
                "offline:Azure",
                "body.guangbo_ticao",
                crate::skill::config::SkillConfig::new(std::collections::BTreeMap::from([(
                    "stance".to_string(),
                    serde_json::json!("short"),
                )])),
            );

        let (client_bundle, _helper) = create_mock_client("Azure");
        let mut skill_bar = SkillBarBindings::default();
        assert!(skill_bar.set(
            0,
            SkillSlot::Skill {
                skill_id: "body.guangbo_ticao".to_string(),
            },
        ));
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut().entity_mut(entity).insert((
            Position::new([0.0, 0.0, 0.0]),
            skill_bar,
            QuickSlotBindings::default(),
            empty_inventory(),
            known(&["body.guangbo_ticao"]),
        ));
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: serde_json::to_vec(&ClientRequestV1::SkillBarCast {
                    v: 1,
                    slot: 0,
                    target: None,
                })
                .unwrap()
                .into_boxed_slice(),
            });

        app.update();

        let casting = app.world().get::<Casting>(entity).unwrap();
        assert_eq!(casting.source, CastSource::SkillBar);
        assert_eq!(casting.slot, 0);
        // cast/cd 来自 known_techniques.body.guangbo_ticao（cast 60 / cooldown 200）。
        assert_eq!(casting.duration_ticks, 60);
        assert_eq!(casting.complete_cooldown_ticks, 200);
        assert_eq!(casting.skill_id.as_deref(), Some("body.guangbo_ticao"));
        assert_eq!(
            casting
                .skill_config
                .as_ref()
                .and_then(|config| config.fields.get("stance")),
            Some(&serde_json::json!("short"))
        );
    }

    #[test]
    fn skill_bar_cast_requires_config_for_schema_fixture() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let mut skill_bar = SkillBarBindings::default();
        assert!(skill_bar.set(
            0,
            SkillSlot::Skill {
                skill_id: "zhenmai.sever_chain".to_string(),
            },
        ));
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut().entity_mut(entity).insert((
            Position::new([0.0, 0.0, 0.0]),
            skill_bar,
            QuickSlotBindings::default(),
            empty_inventory(),
            Cultivation {
                realm: Realm::Void,
                qi_current: 100.0,
                qi_max: 100.0,
                ..Default::default()
            },
            MeridianSystem::default(),
            known(&["zhenmai.sever_chain"]),
        ));

        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: serde_json::to_vec(&ClientRequestV1::SkillBarCast {
                    v: 1,
                    slot: 0,
                    target: None,
                })
                .unwrap()
                .into_boxed_slice(),
            });
        app.update();
        assert!(app.world().get::<Casting>(entity).is_none());

        app.world_mut()
            .resource_mut::<SkillConfigStore>()
            .set_config(
                "offline:Azure",
                "zhenmai.sever_chain",
                crate::skill::config::SkillConfig::new(std::collections::BTreeMap::from([
                    ("meridian_id".to_string(), serde_json::json!("Pericardium")),
                    ("backfire_kind".to_string(), serde_json::json!("array")),
                ])),
            );
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: serde_json::to_vec(&ClientRequestV1::SkillBarCast {
                    v: 1,
                    slot: 0,
                    target: None,
                })
                .unwrap()
                .into_boxed_slice(),
            });
        app.update();

        let casting = app.world().get::<Casting>(entity).unwrap();
        assert_eq!(casting.skill_id.as_deref(), Some("zhenmai.sever_chain"));
        assert_eq!(
            casting
                .skill_config
                .as_ref()
                .and_then(|config| config.fields.get("backfire_kind")),
            Some(&serde_json::json!("array"))
        );

        app.world_mut()
            .resource_mut::<SkillConfigStore>()
            .set_config(
                "offline:Azure",
                "zhenmai.sever_chain",
                crate::skill::config::SkillConfig::new(std::collections::BTreeMap::from([
                    ("meridian_id".to_string(), serde_json::json!("Pericardium")),
                    (
                        "backfire_kind".to_string(),
                        serde_json::json!("tainted_yuan"),
                    ),
                ])),
            );
        let casting = app.world().get::<Casting>(entity).unwrap();
        assert_eq!(
            casting
                .skill_config
                .as_ref()
                .and_then(|config| config.fields.get("backfire_kind")),
            Some(&serde_json::json!("array"))
        );

        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: serde_json::to_vec(&ClientRequestV1::SkillConfigIntent {
                    v: 1,
                    skill_id: "zhenmai.sever_chain".to_string(),
                    config: std::collections::BTreeMap::from([(
                        "backfire_kind".to_string(),
                        serde_json::json!("invalid"),
                    )]),
                })
                .unwrap()
                .into_boxed_slice(),
            });
        app.update();
        flush_all_client_packets(&mut app);
        let snapshots = collect_skill_config_snapshots(&mut helper);
        assert_eq!(snapshots.len(), 1);
        assert_eq!(
            snapshots[0]
                .configs
                .get("zhenmai.sever_chain")
                .and_then(|config| config.fields.get("backfire_kind")),
            Some(&serde_json::json!("tainted_yuan"))
        );
    }

    #[test]
    fn valid_skill_config_intent_replies_with_authoritative_snapshot() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: serde_json::to_vec(&ClientRequestV1::SkillConfigIntent {
                    v: 1,
                    skill_id: "zhenmai.sever_chain".to_string(),
                    config: std::collections::BTreeMap::from([
                        ("meridian_id".to_string(), serde_json::json!("Pericardium")),
                        ("backfire_kind".to_string(), serde_json::json!("array")),
                    ]),
                })
                .unwrap()
                .into_boxed_slice(),
            });

        app.update();
        flush_all_client_packets(&mut app);
        let snapshots = collect_skill_config_snapshots(&mut helper);

        assert_eq!(snapshots.len(), 1);
        assert_eq!(
            snapshots[0]
                .configs
                .get("zhenmai.sever_chain")
                .and_then(|config| config.fields.get("backfire_kind")),
            Some(&serde_json::json!("array"))
        );
    }

    #[test]
    fn skill_bar_cast_rejects_when_skill_config_schemas_missing() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.world_mut().remove_resource::<SkillConfigSchemas>();

        let (client_bundle, _helper) = create_mock_client("Azure");
        let mut skill_bar = SkillBarBindings::default();
        assert!(skill_bar.set(
            0,
            SkillSlot::Skill {
                skill_id: "zhenmai.sever_chain".to_string(),
            },
        ));
        let entity = app.world_mut().spawn(client_bundle).id();
        // Grant the technique so the ownership gate passes; the rejection is caused by the
        // missing SkillConfigSchemas resource, not by lack of ownership.
        app.world_mut().entity_mut(entity).insert((
            Position::new([0.0, 0.0, 0.0]),
            skill_bar,
            QuickSlotBindings::default(),
            empty_inventory(),
            known(&["zhenmai.sever_chain"]),
        ));
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: serde_json::to_vec(&ClientRequestV1::SkillBarCast {
                    v: 1,
                    slot: 0,
                    target: None,
                })
                .unwrap()
                .into_boxed_slice(),
            });

        app.update();

        assert!(app.world().get::<Casting>(entity).is_none());
    }

    // ─────────────────────────────────────────────────────────────────────────
    // plan-bug-qc-p1 §skill-cast P0：经脉门控单元 + 集成测试 (11 tests)
    // ─────────────────────────────────────────────────────────────────────────

    /// 测试辅助：从 MockClientHelper 中提取第一个 CastSync payload。
    fn collect_cast_syncs(helper: &mut MockClientHelper) -> Vec<CastSyncV1> {
        helper
            .collect_received()
            .0
            .into_iter()
            .filter_map(|frame| {
                let packet = frame.decode::<CustomPayloadS2c>().ok()?;
                if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                    return None;
                }
                let payload = serde_json::from_slice::<ServerDataV1>(packet.data.0 .0).ok()?;
                match payload.payload {
                    ServerDataPayloadV1::CastSync(s) => Some(s),
                    _ => None,
                }
            })
            .collect()
    }

    /// Build a minimal KnownTechniques component with exactly the listed technique ids
    /// (active=true, proficiency=0.5). Use in skill_bar tests to grant only the
    /// technique under test so the ownership gate passes without granting everything.
    fn known(ids: &[&str]) -> KnownTechniques {
        use crate::cultivation::known_techniques::KnownTechnique;
        KnownTechniques {
            entries: ids
                .iter()
                .map(|id| KnownTechnique {
                    id: (*id).to_string(),
                    proficiency: 0.5,
                    active: true,
                })
                .collect(),
        }
    }

    /// 发送一个 skill_bar_cast 消息（slot 0）给 entity，并驱动一次 app.update()。
    fn send_skill_bar_cast(app: &mut App, entity: Entity) {
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: serde_json::to_vec(&ClientRequestV1::SkillBarCast {
                    v: 1,
                    slot: 0,
                    target: None,
                })
                .unwrap()
                .into_boxed_slice(),
            });
        app.update();
    }

    /// 同 `send_skill_bar_cast`，但带 `entity_bits:` 目标（resolver 招式需要 target）。
    fn send_skill_bar_cast_with_target(app: &mut App, entity: Entity, target: Entity) {
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: serde_json::to_vec(&ClientRequestV1::SkillBarCast {
                    v: 1,
                    slot: 0,
                    target: Some(format!("entity_bits:{}", target.to_bits())),
                })
                .unwrap()
                .into_boxed_slice(),
            });
        app.update();
    }

    // ── 1. happy path：经脉门通过 → resolver 施放成功 ─────────────────────────

    #[test]
    fn skill_bar_cast_meridian_gate_passes_when_all_deps_satisfied_resolver_path() {
        // burst_meridian.tie_shan_kao 现已实装 resolver，required_meridians 要 Stomach
        // opened=true + integrity ≥ 0.5。把经脉门和 resolver 自身的前置（target / realm
        // Condense / qi ≥ 35 / Stomach 可用）全补齐 → 经脉门放行后 resolver 真正施放 →
        // Casting 由 resolver 插入（cast 10 / cd 70，来自 known_techniques.tie_shan_kao）。
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);

        let (client_bundle, _helper) = create_mock_client("Azure");
        // 近身目标（TIE_SHAN_KAO reach max = 1.0，距离 1.0 命中）。
        let target = app.world_mut().spawn(Position::new([1.0, 0.0, 0.0])).id();
        let mut skill_bar = SkillBarBindings::default();
        skill_bar.set(
            0,
            SkillSlot::Skill {
                skill_id: "burst_meridian.tie_shan_kao".to_string(),
            },
        );
        let entity = app.world_mut().spawn(client_bundle).id();
        let mut ms = crate::cultivation::components::MeridianSystem::default();
        // Stomach opened=true + integrity=1.0 ≥ min_health(0.5)：经脉门 + resolver 均放行。
        {
            let stomach = ms.get_mut(crate::cultivation::components::MeridianId::Stomach);
            stomach.opened = true;
            stomach.integrity = 1.0;
        }
        app.world_mut().entity_mut(entity).insert((
            Position::new([0.0, 0.0, 0.0]),
            skill_bar,
            QuickSlotBindings::default(),
            empty_inventory(),
            ms,
            crate::cultivation::meridian::severed::MeridianSeveredPermanent::default(),
            // resolver 前置：realm Condense + qi ≥ 35。
            crate::cultivation::components::Cultivation {
                realm: crate::cultivation::components::Realm::Condense,
                qi_current: 100.0,
                qi_max: 100.0,
                ..Default::default()
            },
            known(&["burst_meridian.tie_shan_kao"]),
        ));

        send_skill_bar_cast_with_target(&mut app, entity, target);

        let casting = app.world().get::<Casting>(entity).expect(
            "Stomach opened=true + integrity=1.0 ≥ min_health=0.5 + realm/qi/target 满足时，\
             经脉门应放行 → resolver 施放成功；期望 Casting 存在；实际 Casting=None，\
             说明经脉门错误拦截了满足条件的 cast",
        );
        // resolver 路径插入的 Casting：cast/cd 来自 known_techniques.tie_shan_kao（10 / 70）。
        assert_eq!(casting.source, CastSource::SkillBar);
        assert_eq!(casting.duration_ticks, 10, "tie_shan_kao cast_ticks");
        assert_eq!(casting.complete_cooldown_ticks, 70, "tie_shan_kao cooldown");
    }

    // ── 2. 门控：required_meridians integrity 不足 → 拒绝（generic 路径）────

    #[test]
    fn skill_bar_cast_meridian_gate_rejects_when_required_meridian_integrity_too_low() {
        // burst_meridian.beng_quan 需要 LargeIntestine/SmallIntestine/TripleEnergizer integrity >= 0.01
        // 把 LargeIntestine 降到 0.0 → gate 应拒绝
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let mut skill_bar = SkillBarBindings::default();
        skill_bar.set(
            0,
            SkillSlot::Skill {
                skill_id: "burst_meridian.beng_quan".to_string(),
            },
        );
        let entity = app.world_mut().spawn(client_bundle).id();
        let mut ms = crate::cultivation::components::MeridianSystem::default();
        // LargeIntestine integrity = 0.0 < min_health(0.01) → 应触发 gate
        ms.get_mut(crate::cultivation::components::MeridianId::LargeIntestine)
            .integrity = 0.0;
        ms.get_mut(crate::cultivation::components::MeridianId::SmallIntestine)
            .integrity = 0.5;
        ms.get_mut(crate::cultivation::components::MeridianId::TripleEnergizer)
            .integrity = 0.5;
        app.world_mut().entity_mut(entity).insert((
            Position::new([0.0, 0.0, 0.0]),
            skill_bar,
            QuickSlotBindings::default(),
            empty_inventory(),
            ms,
            crate::cultivation::meridian::severed::MeridianSeveredPermanent::default(),
            // Grant ownership so the rejection is caused by the meridian gate, not by missing KnownTechniques.
            known(&["burst_meridian.beng_quan"]),
        ));

        send_skill_bar_cast(&mut app, entity);
        flush_all_client_packets(&mut app);
        let syncs = collect_cast_syncs(&mut helper);

        assert!(
            app.world().get::<Casting>(entity).is_none(),
            "LargeIntestine integrity=0.0 < min_health=0.01 时 cast 应被拒绝（无 Casting component）；\
             期望无 Casting 因为经脉 integrity 不足；实际 Casting 存在，说明 gate 未生效"
        );
        assert!(
            syncs
                .iter()
                .any(|s| s.outcome == CastOutcomeV1::MeridianGated),
            "gate 拒绝时应推送 CastSyncV1{{outcome=MeridianGated}} 反馈；\
             期望至少一条 MeridianGated sync 因为经脉 integrity 不足；\
             实际 syncs={syncs:?}"
        );
    }

    // ── 3. 门控：SEVERED 经脉 → 拒绝（generic 路径）──────────────────────────

    #[test]
    fn skill_bar_cast_meridian_gate_rejects_when_required_meridian_severed() {
        // burst_meridian.beng_quan 需要 LargeIntestine；SEVERED → gate 拒绝
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let mut skill_bar = SkillBarBindings::default();
        skill_bar.set(
            0,
            SkillSlot::Skill {
                skill_id: "burst_meridian.beng_quan".to_string(),
            },
        );
        let entity = app.world_mut().spawn(client_bundle).id();
        let mut ms = crate::cultivation::components::MeridianSystem::default();
        ms.get_mut(crate::cultivation::components::MeridianId::LargeIntestine)
            .integrity = 0.5;
        ms.get_mut(crate::cultivation::components::MeridianId::SmallIntestine)
            .integrity = 0.5;
        ms.get_mut(crate::cultivation::components::MeridianId::TripleEnergizer)
            .integrity = 0.5;
        let mut severed =
            crate::cultivation::meridian::severed::MeridianSeveredPermanent::default();
        severed.insert(
            crate::cultivation::components::MeridianId::LargeIntestine,
            crate::cultivation::meridian::severed::SeveredSource::CombatWound,
            1,
        );
        app.world_mut().entity_mut(entity).insert((
            Position::new([0.0, 0.0, 0.0]),
            skill_bar,
            QuickSlotBindings::default(),
            empty_inventory(),
            ms,
            severed,
            // Grant ownership so the rejection is caused by the meridian gate, not by missing KnownTechniques.
            known(&["burst_meridian.beng_quan"]),
        ));

        send_skill_bar_cast(&mut app, entity);
        flush_all_client_packets(&mut app);
        let syncs = collect_cast_syncs(&mut helper);

        assert!(
            app.world().get::<Casting>(entity).is_none(),
            "LargeIntestine SEVERED 时 burst_meridian.beng_quan cast 应被拒绝；\
             期望无 Casting 因为 SEVERED 经脉在 required_meridians 中；实际 Casting 存在"
        );
        assert!(
            syncs
                .iter()
                .any(|s| s.outcome == CastOutcomeV1::MeridianGated),
            "SEVERED 拒绝时应推送 MeridianGated sync；期望 MeridianGated；实际 syncs={syncs:?}"
        );
    }

    // ── 4. SkillMeridianDependencies 表控：声明依赖但未打通 → 拒绝（generic 路径）

    #[test]
    fn skill_bar_cast_meridian_gate_rejects_via_deps_table_when_severed() {
        // 在 SkillMeridianDependencies 表中声明 "sword.cleave"（无内置 required_meridians）
        // 依赖 LargeIntestine，把它 SEVERED → gate 应拒绝
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        // 声明依赖
        app.world_mut()
            .resource_mut::<SkillMeridianDependencies>()
            .declare(
                "sword.cleave",
                vec![crate::cultivation::components::MeridianId::LargeIntestine],
            );

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let mut skill_bar = SkillBarBindings::default();
        skill_bar.set(
            0,
            SkillSlot::Skill {
                skill_id: "sword.cleave".to_string(),
            },
        );
        let entity = app.world_mut().spawn(client_bundle).id();
        let ms = crate::cultivation::components::MeridianSystem::default(); // LargeIntestine integrity 默认 1.0
        let mut severed =
            crate::cultivation::meridian::severed::MeridianSeveredPermanent::default();
        severed.insert(
            crate::cultivation::components::MeridianId::LargeIntestine,
            crate::cultivation::meridian::severed::SeveredSource::TribulationFail,
            100,
        );
        app.world_mut().entity_mut(entity).insert((
            Position::new([0.0, 0.0, 0.0]),
            skill_bar,
            QuickSlotBindings::default(),
            empty_inventory(),
            ms,
            severed,
            // Grant ownership so the rejection is caused by the meridian deps_table gate, not by missing KnownTechniques.
            known(&["sword.cleave"]),
        ));

        send_skill_bar_cast(&mut app, entity);
        flush_all_client_packets(&mut app);
        let syncs = collect_cast_syncs(&mut helper);

        assert!(
            app.world().get::<Casting>(entity).is_none(),
            "SkillMeridianDependencies 中声明 LargeIntestine 依赖且该经脉 SEVERED 时应拒绝 cast；\
             期望无 Casting；实际 Casting 存在，说明 deps_table 路径未被 gate 覆盖"
        );
        assert!(
            syncs
                .iter()
                .any(|s| s.outcome == CastOutcomeV1::MeridianGated),
            "deps_table 拒绝应推送 MeridianGated；实际 syncs={syncs:?}"
        );
    }

    // ── 5. 无 deps 的招 → 放行──────────────────────────────────────────────────

    #[test]
    fn skill_bar_cast_meridian_gate_passes_for_skill_with_no_deps() {
        // sword.cleave 无内置 required_meridians，且 deps_table 未声明依赖 → gate 不拦
        // 有非依赖经脉 SEVERED（Gallbladder）—— 验证 gate 不误伤无关经脉
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        // 不声明任何 SkillMeridianDependencies

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let mut skill_bar = SkillBarBindings::default();
        skill_bar.set(
            0,
            SkillSlot::Skill {
                skill_id: "sword.cleave".to_string(),
            },
        );
        let entity = app.world_mut().spawn(client_bundle).id();
        let ms = crate::cultivation::components::MeridianSystem::default();
        // 设置一条无关经脉 SEVERED，验证不会误伤
        let mut severed =
            crate::cultivation::meridian::severed::MeridianSeveredPermanent::default();
        severed.insert(
            crate::cultivation::components::MeridianId::Gallbladder, // 非依赖
            crate::cultivation::meridian::severed::SeveredSource::CombatWound,
            1,
        );
        app.world_mut().entity_mut(entity).insert((
            Position::new([0.0, 0.0, 0.0]),
            skill_bar,
            QuickSlotBindings::default(),
            empty_inventory(),
            ms,
            severed,
            // Grant ownership so the cast can reach the meridian gate (and pass it), making the
            // "no MeridianGated" assertion test the gate rather than the ownership gate.
            known(&["sword.cleave"]),
        ));

        send_skill_bar_cast(&mut app, entity);
        flush_all_client_packets(&mut app);
        let syncs = collect_cast_syncs(&mut helper);

        // 核心不变量：无 deps 招式 gate 不误拦，不发 MeridianGated
        // sword.cleave 走 resolver 路径，resolver 可能因 Weapon/Qi 不足拒绝（非 gate 原因）
        // 我们只锁住"gate 未因无关 SEVERED 经脉误触 MeridianGated"
        assert!(
            !syncs
                .iter()
                .any(|s| s.outcome == CastOutcomeV1::MeridianGated),
            "无 deps 招式（sword.cleave）不应被经脉门拦截，不应出现 MeridianGated sync；\
             期望 syncs 中无 MeridianGated（因为该招无经脉依赖，Gallbladder SEVERED 是无关经脉）；\
             实际 syncs={syncs:?}"
        );
    }

    // ── 6. resolver 路径也受门控（以 SkillMeridianDependencies 为例）────────────

    #[test]
    fn skill_bar_cast_meridian_gate_covers_resolver_path_via_deps_table() {
        // sword.cleave 有 resolver；在 deps_table 里声明 LargeIntestine 依赖，SEVERED → gate 拒绝
        // 验证 gate 在 resolver 路径也生效（gate 在 resolver 分支之前检查）
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.world_mut()
            .resource_mut::<SkillMeridianDependencies>()
            .declare(
                "sword.cleave",
                vec![crate::cultivation::components::MeridianId::LargeIntestine],
            );

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let mut skill_bar = SkillBarBindings::default();
        skill_bar.set(
            0,
            SkillSlot::Skill {
                skill_id: "sword.cleave".to_string(),
            },
        );
        let entity = app.world_mut().spawn(client_bundle).id();
        let ms = crate::cultivation::components::MeridianSystem::default();
        let mut severed =
            crate::cultivation::meridian::severed::MeridianSeveredPermanent::default();
        severed.insert(
            crate::cultivation::components::MeridianId::LargeIntestine,
            crate::cultivation::meridian::severed::SeveredSource::BackfireOverload,
            200,
        );
        app.world_mut().entity_mut(entity).insert((
            Position::new([0.0, 0.0, 0.0]),
            skill_bar,
            QuickSlotBindings::default(),
            empty_inventory(),
            ms,
            severed,
            // Grant ownership so the rejection is caused by the meridian deps_table gate, not by missing KnownTechniques.
            known(&["sword.cleave"]),
        ));

        send_skill_bar_cast(&mut app, entity);
        flush_all_client_packets(&mut app);
        let syncs = collect_cast_syncs(&mut helper);

        // resolver 路径被 gate 拦截时：gate 在 commands.add() 之前 return
        // → resolver 的 World closure 根本不会运行（没有 commands.add 被提交）
        // → entity 无 Casting component（resolver 未运行）
        assert!(
            app.world().get::<Casting>(entity).is_none(),
            "gate 在 commands.add() 之前 return，resolver 闭包不运行 → 不应插入 Casting；\
             期望 Casting=None；实际 Casting 存在，说明 resolver 路径未被门控（gate return 没阻止 commands.add）"
        );
        assert!(
            syncs
                .iter()
                .any(|s| s.outcome == CastOutcomeV1::MeridianGated),
            "resolver 路径下 SEVERED deps_table 依赖应触发 MeridianGated；\
             期望 MeridianGated sync；实际 syncs={syncs:?}"
        );
    }

    // ── 7. 边界：integrity 刚好等于阈值（off-by-one）────────────────────────────

    #[test]
    fn skill_bar_cast_meridian_gate_passes_when_integrity_exactly_at_min_health() {
        // 经脉门边界：burst_meridian.tie_shan_kao 需要 Stomach integrity >= 0.5。
        // integrity 恰好 = 0.5（off-by-one 边界）应放行（>= 成立）；resolver 其余前置补齐
        // → 经脉门放行后 resolver 真正插入 Casting。
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let target = app.world_mut().spawn(Position::new([1.0, 0.0, 0.0])).id();
        let mut skill_bar = SkillBarBindings::default();
        skill_bar.set(
            0,
            SkillSlot::Skill {
                skill_id: "burst_meridian.tie_shan_kao".to_string(),
            },
        );
        let entity = app.world_mut().spawn(client_bundle).id();
        let mut ms = crate::cultivation::components::MeridianSystem::default();
        // Stomach opened=true + integrity=0.5 恰好等于 min_health：应放行
        {
            let stomach = ms.get_mut(crate::cultivation::components::MeridianId::Stomach);
            stomach.opened = true;
            stomach.integrity = 0.5;
        }
        app.world_mut().entity_mut(entity).insert((
            Position::new([0.0, 0.0, 0.0]),
            skill_bar,
            QuickSlotBindings::default(),
            empty_inventory(),
            ms,
            crate::cultivation::meridian::severed::MeridianSeveredPermanent::default(),
            crate::cultivation::components::Cultivation {
                realm: crate::cultivation::components::Realm::Condense,
                qi_current: 100.0,
                qi_max: 100.0,
                ..Default::default()
            },
            known(&["burst_meridian.tie_shan_kao"]),
        ));

        send_skill_bar_cast_with_target(&mut app, entity, target);

        assert!(
            app.world().get::<Casting>(entity).is_some(),
            "Stomach opened=true + integrity=0.5 恰好等于 min_health=0.5 时经脉门应放行（>= 成立）\
             → resolver 施放；期望 Casting 存在；实际无 Casting，说明经脉门边界判断为 < 而非 >=（off-by-one）"
        );
    }

    #[test]
    fn skill_bar_cast_meridian_gate_rejects_when_integrity_just_below_min_health() {
        // burst_meridian.tie_shan_kao 需要 Stomach integrity >= 0.5
        // 设置 integrity = 0.499（低于 min_health）→ 应拒绝
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let mut skill_bar = SkillBarBindings::default();
        skill_bar.set(
            0,
            SkillSlot::Skill {
                skill_id: "burst_meridian.tie_shan_kao".to_string(),
            },
        );
        let entity = app.world_mut().spawn(client_bundle).id();
        let mut ms = crate::cultivation::components::MeridianSystem::default();
        ms.get_mut(crate::cultivation::components::MeridianId::Stomach)
            .integrity = 0.499; // 低于阈值
        app.world_mut().entity_mut(entity).insert((
            Position::new([0.0, 0.0, 0.0]),
            skill_bar,
            QuickSlotBindings::default(),
            empty_inventory(),
            ms,
            crate::cultivation::meridian::severed::MeridianSeveredPermanent::default(),
            // Grant ownership so the rejection is caused by the meridian gate (integrity too low), not by missing KnownTechniques.
            known(&["burst_meridian.tie_shan_kao"]),
        ));

        send_skill_bar_cast(&mut app, entity);
        flush_all_client_packets(&mut app);
        let syncs = collect_cast_syncs(&mut helper);

        assert!(
            app.world().get::<Casting>(entity).is_none(),
            "Stomach integrity=0.499 低于 min_health=0.5 时应拒绝 cast；\
             期望无 Casting；实际 Casting 存在，说明 integrity 检查 off-by-one（应为 < 而非 <=）"
        );
        assert!(
            syncs
                .iter()
                .any(|s| s.outcome == CastOutcomeV1::MeridianGated),
            "integrity 不足应推送 MeridianGated；实际 syncs={syncs:?}"
        );
    }

    // ── 7b. 未打通经脉（integrity 满足但 opened=false）→ 拒绝，锁住核心不变量 ────

    #[test]
    fn skill_bar_cast_meridian_gate_rejects_when_required_meridian_not_opened() {
        // burst_meridian.tie_shan_kao 需要 Stomach integrity >= 0.5
        // 设置 Stomach integrity=1.0（满足阈值）但 opened=false（未打通）→ gate 应拒绝
        // 这是核心正典约束：「经脉没通就放不出招」，opened 先于 integrity 决定能否施放
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.world_mut()
            .resource_mut::<SkillConfigStore>()
            .set_config(
                "offline:Azure",
                "burst_meridian.tie_shan_kao",
                crate::skill::config::SkillConfig::new(std::collections::BTreeMap::from([(
                    "stance".to_string(),
                    serde_json::json!("short"),
                )])),
            );

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let mut skill_bar = SkillBarBindings::default();
        skill_bar.set(
            0,
            SkillSlot::Skill {
                skill_id: "burst_meridian.tie_shan_kao".to_string(),
            },
        );
        let entity = app.world_mut().spawn(client_bundle).id();
        let mut ms = crate::cultivation::components::MeridianSystem::default();
        // integrity 满足但经脉未打通：应拒绝（opened=false 默认）
        ms.get_mut(crate::cultivation::components::MeridianId::Stomach)
            .integrity = 1.0; // ≥ min_health=0.5，但 opened 仍为 false（默认）
        app.world_mut().entity_mut(entity).insert((
            Position::new([0.0, 0.0, 0.0]),
            skill_bar,
            QuickSlotBindings::default(),
            empty_inventory(),
            ms,
            crate::cultivation::meridian::severed::MeridianSeveredPermanent::default(),
            // Grant ownership so the rejection is caused by the meridian gate (not opened), not by missing KnownTechniques.
            known(&["burst_meridian.tie_shan_kao"]),
        ));

        send_skill_bar_cast(&mut app, entity);
        flush_all_client_packets(&mut app);
        let syncs = collect_cast_syncs(&mut helper);

        assert!(
            app.world().get::<Casting>(entity).is_none(),
            "Stomach opened=false 时 cast 应被拒绝，即使 integrity=1.0 满足阈值；\
             期望无 Casting 因为经脉未打通（正典：经脉没通就放不出招）；\
             实际 Casting 存在，说明 opened 检查未生效"
        );
        assert!(
            syncs
                .iter()
                .any(|s| s.outcome == CastOutcomeV1::MeridianGated),
            "未打通经脉拒绝时应推送 MeridianGated sync；\
             期望 MeridianGated 因为 opened=false；实际 syncs={syncs:?}"
        );
    }

    // ── 8. 多经脉部分满足 → 拒绝（generic 路径）───────────────────────────────

    #[test]
    fn skill_bar_cast_meridian_gate_rejects_when_only_partial_deps_satisfied() {
        // burst_meridian.beng_quan 需要 LargeIntestine + SmallIntestine + TripleEnergizer
        // 满足前两个，第三个 integrity=0.0 → 应拒绝
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let mut skill_bar = SkillBarBindings::default();
        skill_bar.set(
            0,
            SkillSlot::Skill {
                skill_id: "burst_meridian.beng_quan".to_string(),
            },
        );
        let entity = app.world_mut().spawn(client_bundle).id();
        let mut ms = crate::cultivation::components::MeridianSystem::default();
        ms.get_mut(crate::cultivation::components::MeridianId::LargeIntestine)
            .integrity = 0.5; // 满足
        ms.get_mut(crate::cultivation::components::MeridianId::SmallIntestine)
            .integrity = 0.5; // 满足
        ms.get_mut(crate::cultivation::components::MeridianId::TripleEnergizer)
            .integrity = 0.0; // 不满足
        app.world_mut().entity_mut(entity).insert((
            Position::new([0.0, 0.0, 0.0]),
            skill_bar,
            QuickSlotBindings::default(),
            empty_inventory(),
            ms,
            crate::cultivation::meridian::severed::MeridianSeveredPermanent::default(),
            // Grant ownership so the rejection is caused by the meridian gate (partial deps), not by missing KnownTechniques.
            known(&["burst_meridian.beng_quan"]),
        ));

        send_skill_bar_cast(&mut app, entity);
        flush_all_client_packets(&mut app);
        let syncs = collect_cast_syncs(&mut helper);

        assert!(
            app.world().get::<Casting>(entity).is_none(),
            "多经脉依赖中 TripleEnergizer integrity=0.0 < min_health=0.01 时应拒绝；\
             期望无 Casting；实际 Casting 存在，说明 gate 未检查全部依赖"
        );
        assert!(
            syncs
                .iter()
                .any(|s| s.outcome == CastOutcomeV1::MeridianGated),
            "部分满足多依赖时应推送 MeridianGated；实际 syncs={syncs:?}"
        );
    }

    // ── 9. entity 无 MeridianSystem → 放行（pre-init 玩家兼容）───────────────

    #[test]
    fn skill_bar_cast_meridian_gate_passes_when_no_meridian_system_component() {
        // entity 无 MeridianSystem component（pre-init 玩家）→ 经脉门应 skip 放行。
        // 用 body.guangbo_ticao（仍是 skeleton：无 resolver、无 required_meridians、无 deps）
        // 作载体：经脉门放行后走通用路径，无条件插入 Casting，纯粹锁住「无 MeridianSystem 放行」语义。
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let mut skill_bar = SkillBarBindings::default();
        skill_bar.set(
            0,
            SkillSlot::Skill {
                skill_id: "body.guangbo_ticao".to_string(),
            },
        );
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut().entity_mut(entity).insert((
            Position::new([0.0, 0.0, 0.0]),
            skill_bar,
            QuickSlotBindings::default(),
            empty_inventory(),
            // 故意不插入 MeridianSystem
            known(&["body.guangbo_ticao"]),
        ));

        send_skill_bar_cast(&mut app, entity);

        // 无 MeridianSystem → gate skip → cast 放行（generic 路径）→ Casting 存在
        assert!(
            app.world().get::<Casting>(entity).is_some(),
            "entity 无 MeridianSystem 时 gate 应 skip 放行（pre-init 兼容）；\
             期望 Casting 存在；实际无 Casting，说明 gate 在无 MeridianSystem 时错误拒绝了"
        );
    }

    // ── 10. 回归：既有 skill_bar_cast_defined_skill_without_resolver 不破 ────────

    #[test]
    fn skill_bar_cast_meridian_gate_regression_no_deps_generic_path_still_works() {
        // body.guangbo_ticao 是无 resolver / 无 required_meridians / 无 deps 的 skeleton 招，
        // entity 有 MeridianSystem → 经脉门无依赖可查直接放行 → 走通用路径成功施放。
        // 这是对 "skill_bar_cast_defined_skill_without_resolver_uses_generic_cast_path" 的回归验证：
        // 引入经脉门后，无依赖招的通用路径行为不变。
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.world_mut()
            .resource_mut::<SkillConfigStore>()
            .set_config(
                "offline:Azure",
                "body.guangbo_ticao",
                crate::skill::config::SkillConfig::new(std::collections::BTreeMap::from([(
                    "stance".to_string(),
                    serde_json::json!("short"),
                )])),
            );

        let (client_bundle, _helper) = create_mock_client("Azure");
        let mut skill_bar = SkillBarBindings::default();
        skill_bar.set(
            0,
            SkillSlot::Skill {
                skill_id: "body.guangbo_ticao".to_string(),
            },
        );
        let entity = app.world_mut().spawn(client_bundle).id();
        // 带一条 SEVERED 的无关经脉，验证经脉门不误伤无依赖招。
        let mut ms = crate::cultivation::components::MeridianSystem::default();
        {
            let stomach = ms.get_mut(crate::cultivation::components::MeridianId::Stomach);
            stomach.opened = true;
            stomach.integrity = 1.0;
        }
        app.world_mut().entity_mut(entity).insert((
            Position::new([0.0, 0.0, 0.0]),
            skill_bar,
            QuickSlotBindings::default(),
            empty_inventory(),
            ms,
            crate::cultivation::meridian::severed::MeridianSeveredPermanent::default(),
            known(&["body.guangbo_ticao"]),
        ));

        send_skill_bar_cast(&mut app, entity);

        let casting = app.world().get::<Casting>(entity).expect(
            "回归：body.guangbo_ticao（无依赖 skeleton 招）有 MeridianSystem 时应成功施放（与引入 gate 前行为一致）",
        );
        assert_eq!(casting.source, CastSource::SkillBar);
        assert_eq!(
            casting.skill_id.as_deref(),
            Some("body.guangbo_ticao"),
            "skill_id 应与绑定技能一致"
        );
    }

    // ── 10b. 通用技能警示 HUD：resolver-path 拒绝把原因推回 client ───────────────
    //
    // plan-skill-warn-hud：以前 resolver 路径的 CastResult::Rejected 只 tracing::debug
    // 默默 return，client 完全收不到 → 玩家"按了键没反应"。现在每个 resolver 拒绝都推
    // 一条 CastSyncV1{phase: Idle, outcome: Reject*}，通用警示 HUD 据此弹中文提示。

    #[test]
    fn skill_bar_cast_resolver_reject_pushes_cast_sync_with_reason() {
        // 经脉门放行（Stomach opened+integrity 满足）+ 提供近身目标，但 realm 默认 Awaken
        // < tie_shan_kao 要求的 Condense → resolver 在 check_realm_gate 处拒绝 RealmTooLow。
        // 期望：① 无 Casting（被 resolver 拒绝）② 推送 CastSyncV1{outcome=RejectRealmTooLow}。
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let target = app.world_mut().spawn(Position::new([1.0, 0.0, 0.0])).id();
        let mut skill_bar = SkillBarBindings::default();
        skill_bar.set(
            0,
            SkillSlot::Skill {
                skill_id: "burst_meridian.tie_shan_kao".to_string(),
            },
        );
        let entity = app.world_mut().spawn(client_bundle).id();
        let mut ms = crate::cultivation::components::MeridianSystem::default();
        {
            // 经脉门放行：opened=true + integrity=1.0 ≥ min_health=0.5。
            let stomach = ms.get_mut(crate::cultivation::components::MeridianId::Stomach);
            stomach.opened = true;
            stomach.integrity = 1.0;
        }
        app.world_mut().entity_mut(entity).insert((
            Position::new([0.0, 0.0, 0.0]),
            skill_bar,
            QuickSlotBindings::default(),
            empty_inventory(),
            ms,
            crate::cultivation::meridian::severed::MeridianSeveredPermanent::default(),
            // realm = Awaken（默认）< Condense → resolver check_realm_gate 拒绝 RealmTooLow。
            crate::cultivation::components::Cultivation {
                realm: crate::cultivation::components::Realm::Awaken,
                qi_current: 100.0,
                qi_max: 100.0,
                ..Default::default()
            },
            // Grant ownership so the rejection is caused by the resolver (RealmTooLow), not by missing KnownTechniques.
            known(&["burst_meridian.tie_shan_kao"]),
        ));

        send_skill_bar_cast_with_target(&mut app, entity, target);
        flush_all_client_packets(&mut app);
        let syncs = collect_cast_syncs(&mut helper);

        assert!(
            app.world().get::<Casting>(entity).is_none(),
            "realm Awaken < Condense 时 resolver 应拒绝；期望无 Casting；实际 Casting 存在",
        );
        assert!(
            syncs.iter().any(|s| s.outcome
                == crate::schema::combat_hud::CastOutcomeV1::RejectRealmTooLow
                && s.phase == CastPhaseV1::Idle),
            "resolver 拒绝 RealmTooLow 时应推 CastSyncV1{{phase=Idle, outcome=RejectRealmTooLow}} \
             让通用警示 HUD 显示「境界不足」；期望命中该 sync；实际 syncs={syncs:?}",
        );
    }

    // ── 10c. F1（P3 opus verify 发现）：施放门行为测试 —— 通用 skill_bar 路径 ─────
    //
    // 修复前只有 known_techniques.rs 的 `required_race.allows(...)` 真值表 pin，从不
    // 触达 `handle_skill_bar_cast` 里真实的 race gate 判定代码（line ~12013-12036）
    // ——回归删掉那段 `if !definition.required_race.allows(...) { RejectRaceMismatch }`
    // 整块不会撞红。本节直接驱动真实 cast 入口（`send_skill_bar_cast`）锁死该行为。

    /// 构造一个 is_humanoid=false 的合成种族 `RaceRegistry`/`BodyPlanRegistry` fixture
    /// （与 `combat::resolve` 的 `single_part_registries` 同款手法：单部位
    /// `HeightBands` 几何，够 `resolve_body_plan` 校验通过即可，不关心命中判定）。
    fn non_humanoid_race_fixture(
        race_id: &str,
    ) -> (
        crate::body_plan::BodyPlanRegistry,
        crate::body_plan::RaceRegistry,
    ) {
        use crate::body_plan::race_registry::RaceEntry;
        use crate::body_plan::{
            BodyPartDef, BodyPlan, BodyPlanRegistry, HeightBand, HeightBandAssignment, HitGeometry,
            PartConsequence, RaceId, RaceRegistry, StandingAabbSpec,
        };
        use std::collections::HashMap;

        let plan = BodyPlan {
            id: format!("test_{race_id}_plan").into(),
            display_name: "测试非人形构型".to_string(),
            is_humanoid: false,
            parts: vec![BodyPartDef {
                id: "core".into(),
                damage_mul: 1.0,
                contam_mul: 1.0,
                bleed_mul: 1.0,
                consequence: PartConsequence::Core,
            }],
            hit_geometry: HitGeometry::HeightBands {
                aabb: StandingAabbSpec {
                    half_width: 0.3,
                    height: 1.8,
                },
                bands: vec![HeightBand {
                    min_rel_y: -1.0,
                    assignment: HeightBandAssignment::Single {
                        part: "core".into(),
                    },
                }],
                lateral_threshold: 0.19,
            },
            equip_slots: vec![],
            meridian_profile: None,
            mutation_slot_mapping: HashMap::new(),
        };
        let plan_id = plan.id.clone();
        let body_plans = BodyPlanRegistry::from_plans(vec![plan]).expect("plan must validate");
        // `RaceRegistry::from_file_contents` 要求表内必须有一条 id=HUMAN_RACE_ID 的
        // 默认条目（是否人形只看 body plan，不看 race id 字面意义）。fixture 注册
        // 两条 race：① id=race_id 指向 is_humanoid=false 构型——被测方（如
        // central-review 2012 #9 的 sword.infuse）把 `Cultivation.race` 设为该真实
        // race id，生产 `resolve_body_plan` 路径即按此 id 解析出非人形本体；② 必需的
        // HUMAN_RACE_ID 占位条目指向同一非人形构型，满足表加载校验。
        let races = RaceRegistry::from_parts_for_test(
            vec![
                RaceEntry {
                    id: RaceId::new(race_id),
                    display_name: format!("测试非人形种族({race_id})"),
                    body_plan_id: plan_id.clone(),
                    beast_kinds: vec![],
                },
                RaceEntry {
                    id: RaceId::new(crate::body_plan::HUMAN_RACE_ID),
                    display_name: "默认人形占位".to_string(),
                    body_plan_id: plan_id,
                    beast_kinds: vec![],
                },
            ],
            vec![],
            &body_plans,
        )
        .expect("races fixture must validate");
        (body_plans, races)
    }

    /// 装配一个持剑、已习得 sword.cleave 的 caster；`race` 为 `None` 时不插入
    /// `RaceRegistry`/`BodyPlanRegistry`（退化到 humanoid 单例，人形本体基线）；
    /// 为 `Some(race_id)` 时插入 `non_humanoid_race_fixture` 并把 Cultivation.race
    /// 设为该 id（非人形本体）。
    fn setup_sword_cleave_caster(
        app: &mut App,
        username: &str,
        race: Option<&str>,
    ) -> (Entity, MockClientHelper) {
        if let Some(race_id) = race {
            let (body_plans, races) = non_humanoid_race_fixture(race_id);
            app.insert_resource(body_plans);
            app.insert_resource(races);
        }
        let (client_bundle, helper) = create_mock_client(username);
        let mut skill_bar = SkillBarBindings::default();
        skill_bar.set(
            0,
            SkillSlot::Skill {
                skill_id: "sword.cleave".to_string(),
            },
        );
        let entity = app.world_mut().spawn(client_bundle).id();
        // `Some(race_id)` 时 `Cultivation.race` 设为该真实 race id（fixture 以它为
        // 键注册了 is_humanoid=false 构型，生产 `resolve_body_plan` 即解析出非人形
        // 本体）；`None` 时不插 fixture，退化到 humanoid 单例（HUMAN_RACE_ID）。
        // 是否人形由「race 是否落在非人形 fixture」决定，不看 id 字符串字面意义。
        let cultivation = crate::cultivation::components::Cultivation {
            realm: Realm::Induce,
            qi_current: 42.0,
            qi_max: 100.0,
            race: match race {
                Some(race_id) => crate::body_plan::RaceId::new(race_id),
                None => crate::body_plan::RaceId::new(crate::body_plan::HUMAN_RACE_ID),
            },
            ..Default::default()
        };
        app.world_mut().entity_mut(entity).insert((
            Position::new([0.0, 0.0, 0.0]),
            skill_bar,
            QuickSlotBindings::default(),
            empty_inventory(),
            crate::combat::weapon::Weapon {
                slot: crate::combat::weapon::EquipSlot::MainHand,
                instance_id: 1,
                template_id: "test_sword".to_string(),
                weapon_kind: crate::combat::weapon::WeaponKind::Sword,
                base_attack: 10.0,
                quality_tier: 0,
                durability: 100.0,
                durability_max: 100.0,
            },
            cultivation,
            known(&["sword.cleave"]),
        ));
        (entity, helper)
    }

    #[test]
    fn skill_bar_cast_race_gate_rejects_non_humanoid_caster_before_resolver_qi_untouched() {
        // sword.cleave 全数据表标 RaceGate::Humanoid（§8.1 #6）。非人形本体
        // （race="test_whale" + BodyPlan.is_humanoid=false）施放必须在到达 resolver
        // （cast_sword_cleave）之前被通用路径的 race gate 拒绝：① 推
        // CastSyncV1{outcome=RejectRaceMismatch} ② resolver 从未运行——用零
        // AttackIntent 事件锁死（resolver 只要跑起来必发一条 AttackIntent，见
        // `combat::sword_basics::cast_sword_attack`）③ qi_current 分毫不动（守恒律：
        // race gate 拒绝不该扣任何真元）。
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        let (entity, mut helper) = setup_sword_cleave_caster(&mut app, "Whale", Some("test_whale"));

        send_skill_bar_cast(&mut app, entity);
        flush_all_client_packets(&mut app);
        let syncs = collect_cast_syncs(&mut helper);

        assert!(
            app.world().get::<Casting>(entity).is_none(),
            "非人形本体施放人形专属剑技必须被 race gate 拒绝在 resolver 之前；\
             期望无 Casting；实际 Casting 存在"
        );
        assert!(
            syncs.iter().any(|s| s.outcome
                == crate::schema::combat_hud::CastOutcomeV1::RejectRaceMismatch
                && s.phase == CastPhaseV1::Idle),
            "race gate 拒绝应推 CastSyncV1{{phase=Idle, outcome=RejectRaceMismatch}}；\
             实际 syncs={syncs:?}"
        );
        let attack_intents = app
            .world()
            .resource::<valence::prelude::Events<crate::combat::events::AttackIntent>>();
        assert!(
            attack_intents.is_empty(),
            "race gate 应在 resolver 之前拦截，resolver 从未运行；\
             期望零 AttackIntent；实际存在事件（说明 resolver 被误放行了）"
        );
        let qi_current = app
            .world()
            .get::<crate::cultivation::components::Cultivation>(entity)
            .expect("Cultivation must still exist")
            .qi_current;
        assert!(
            (qi_current - 42.0).abs() < f64::EPSILON,
            "race gate 拒绝不应扣真元（守恒律，见 CLAUDE.md 真元守恒律）；\
             期望 qi_current=42.0 不变，实际 {qi_current}"
        );
    }

    #[test]
    fn skill_bar_cast_race_gate_passes_for_humanoid_caster_reaches_resolver() {
        // 反向 happy：人形本体（race=human 默认，未插入 RaceRegistry/BodyPlanRegistry
        // → `resolve_body_plan_for_target` 退化到 humanoid 单例）施放同一招
        // sword.cleave 不应被 race gate 拦下——必须真正走到 resolver 并挥出
        // （非零 AttackIntent，且不应出现 RejectRaceMismatch）。与上一测试对照，
        // 证明 race gate 只挡非人形、不误伤人形本体。
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        let (entity, mut helper) = setup_sword_cleave_caster(&mut app, "Human", None);

        send_skill_bar_cast(&mut app, entity);
        flush_all_client_packets(&mut app);
        let syncs = collect_cast_syncs(&mut helper);

        assert!(
            !syncs
                .iter()
                .any(|s| s.outcome == crate::schema::combat_hud::CastOutcomeV1::RejectRaceMismatch),
            "人形本体不应被 race gate 拒绝；实际 syncs={syncs:?}"
        );
        let attack_intents = app
            .world()
            .resource::<valence::prelude::Events<crate::combat::events::AttackIntent>>();
        assert!(
            !attack_intents.is_empty(),
            "人形本体施放 sword.cleave 应真正走到 resolver 并挥出（发 AttackIntent）；\
             期望非空事件，实际为空——说明 race gate 误挡了人形本体"
        );
    }

    // ── 11. helper 单元：check_player_skill_meridian_gate 直接单元测试 ───────────

    #[test]
    fn check_player_skill_meridian_gate_helper_unit_no_deps_passes() {
        use crate::cultivation::meridian::severed::check_player_skill_meridian_gate;
        let ms = crate::cultivation::components::MeridianSystem::default();
        let result = check_player_skill_meridian_gate("unknown.skill", &[], &ms, None, None);
        assert!(
            result.is_ok(),
            "无 deps（required_meridians=[] + deps_table=None）时应放行；\
             期望 Ok；实际 {result:?}"
        );
    }

    #[test]
    fn check_player_skill_meridian_gate_helper_unit_rejects_severed_via_required() {
        use crate::cultivation::known_techniques::TechniqueRequiredMeridian;
        use crate::cultivation::meridian::severed::check_player_skill_meridian_gate;
        let ms = crate::cultivation::components::MeridianSystem::default();
        let mut severed =
            crate::cultivation::meridian::severed::MeridianSeveredPermanent::default();
        severed.insert(
            crate::cultivation::components::MeridianId::Lung,
            crate::cultivation::meridian::severed::SeveredSource::CombatWound,
            1,
        );
        let req = [TechniqueRequiredMeridian {
            channel: "Lung".to_string(),
            min_health: 0.5,
        }];
        let result =
            check_player_skill_meridian_gate("test.skill", &req, &ms, Some(&severed), None);
        assert_eq!(
            result,
            Err(crate::cultivation::components::MeridianId::Lung),
            "Lung SEVERED 时应返回 Err(Lung)；期望 Err(Lung) 因为 required_meridians 包含 Lung 且已 SEVERED；实际 {result:?}"
        );
    }

    #[test]
    fn check_player_skill_meridian_gate_helper_unit_rejects_low_integrity_via_required() {
        use crate::cultivation::known_techniques::TechniqueRequiredMeridian;
        use crate::cultivation::meridian::severed::check_player_skill_meridian_gate;
        let mut ms = crate::cultivation::components::MeridianSystem::default();
        // Lung opened=true 但 integrity=0.3 < min_health=0.5：应因 integrity 不足拒绝
        {
            let lung = ms.get_mut(crate::cultivation::components::MeridianId::Lung);
            lung.opened = true;
            lung.integrity = 0.3;
        }
        let req = [TechniqueRequiredMeridian {
            channel: "Lung".to_string(),
            min_health: 0.5,
        }];
        let result = check_player_skill_meridian_gate("test.skill", &req, &ms, None, None);
        assert_eq!(
            result,
            Err(crate::cultivation::components::MeridianId::Lung),
            "Lung opened=true 但 integrity=0.3 < min_health=0.5 时应返回 Err(Lung)；\
             期望 Err(Lung) 因为 integrity 不足（opened 已满足，integrity 是拒绝原因）；实际 {result:?}"
        );
    }

    #[test]
    fn check_player_skill_meridian_gate_helper_unit_rejects_via_deps_table_severed() {
        use crate::cultivation::meridian::severed::{
            check_player_skill_meridian_gate, SkillMeridianDependencies,
        };
        let ms = crate::cultivation::components::MeridianSystem::default();
        let mut severed =
            crate::cultivation::meridian::severed::MeridianSeveredPermanent::default();
        severed.insert(
            crate::cultivation::components::MeridianId::Heart,
            crate::cultivation::meridian::severed::SeveredSource::BackfireOverload,
            5,
        );
        let mut deps = SkillMeridianDependencies::default();
        deps.declare(
            "test.skill",
            vec![crate::cultivation::components::MeridianId::Heart],
        );
        let result =
            check_player_skill_meridian_gate("test.skill", &[], &ms, Some(&severed), Some(&deps));
        assert_eq!(
            result,
            Err(crate::cultivation::components::MeridianId::Heart),
            "deps_table 中声明 Heart 依赖且 Heart SEVERED 时应返回 Err(Heart)；\
             期望 Err(Heart)；实际 {result:?}"
        );
    }

    // ── 11b. unit：required_meridians 路径 opened=false → Err（核心不变量单元测试）──

    #[test]
    fn check_player_skill_meridian_gate_helper_unit_rejects_not_opened_via_required() {
        // 经脉 integrity 满足阈值但 opened=false → 应返回 Err（未打通不能施放）
        use crate::cultivation::known_techniques::TechniqueRequiredMeridian;
        use crate::cultivation::meridian::severed::check_player_skill_meridian_gate;
        let mut ms = crate::cultivation::components::MeridianSystem::default();
        // Lung opened=false（默认），integrity=1.0（超过任何 min_health）
        ms.get_mut(crate::cultivation::components::MeridianId::Lung)
            .integrity = 1.0;
        let req = [TechniqueRequiredMeridian {
            channel: "Lung".to_string(),
            min_health: 0.5,
        }];
        let result = check_player_skill_meridian_gate("test.skill", &req, &ms, None, None);
        assert_eq!(
            result,
            Err(crate::cultivation::components::MeridianId::Lung),
            "Lung opened=false 时应返回 Err(Lung) 即使 integrity=1.0 满足阈值；\
             期望 Err(Lung) 因为经脉未打通（正典约束）；实际 {result:?}"
        );
    }

    #[test]
    fn check_player_skill_meridian_gate_helper_unit_rejects_not_opened_via_deps_table() {
        // deps_table 路径：经脉已注册依赖但 opened=false → 应返回 Err（未打通不能施放）
        use crate::cultivation::meridian::severed::{
            check_player_skill_meridian_gate, SkillMeridianDependencies,
        };
        let ms = crate::cultivation::components::MeridianSystem::default(); // Stomach opened=false 默认
        let mut deps = SkillMeridianDependencies::default();
        deps.declare(
            "test.skill",
            vec![crate::cultivation::components::MeridianId::Stomach],
        );
        let result = check_player_skill_meridian_gate("test.skill", &[], &ms, None, Some(&deps));
        assert_eq!(
            result,
            Err(crate::cultivation::components::MeridianId::Stomach),
            "deps_table 中声明 Stomach 依赖且 Stomach opened=false 时应返回 Err(Stomach)；\
             期望 Err(Stomach) 因为经脉未打通；实际 {result:?}"
        );
    }

    #[test]
    fn check_player_skill_meridian_gate_helper_unit_passes_when_opened_and_integrity_satisfied() {
        // happy path unit test：opened=true + integrity 满足 → Ok(())
        use crate::cultivation::known_techniques::TechniqueRequiredMeridian;
        use crate::cultivation::meridian::severed::check_player_skill_meridian_gate;
        let mut ms = crate::cultivation::components::MeridianSystem::default();
        {
            let lung = ms.get_mut(crate::cultivation::components::MeridianId::Lung);
            lung.opened = true;
            lung.integrity = 0.8; // ≥ min_health=0.5
        }
        let req = [TechniqueRequiredMeridian {
            channel: "Lung".to_string(),
            min_health: 0.5,
        }];
        let result = check_player_skill_meridian_gate("test.skill", &req, &ms, None, None);
        assert!(
            result.is_ok(),
            "Lung opened=true + integrity=0.8 ≥ min_health=0.5 时应放行 Ok(())；\
             期望 Ok；实际 {result:?}"
        );
    }

    #[test]
    fn check_player_skill_meridian_gate_helper_unit_deps_table_passes_when_opened() {
        // deps_table happy path：经脉 opened=true → Ok(())
        use crate::cultivation::meridian::severed::{
            check_player_skill_meridian_gate, SkillMeridianDependencies,
        };
        let mut ms = crate::cultivation::components::MeridianSystem::default();
        ms.get_mut(crate::cultivation::components::MeridianId::Kidney)
            .opened = true;
        let mut deps = SkillMeridianDependencies::default();
        deps.declare(
            "test.skill",
            vec![crate::cultivation::components::MeridianId::Kidney],
        );
        let result = check_player_skill_meridian_gate("test.skill", &[], &ms, None, Some(&deps));
        assert!(
            result.is_ok(),
            "deps_table 中声明 Kidney 依赖且 Kidney opened=true 时应放行 Ok(())；\
             期望 Ok；实际 {result:?}"
        );
    }

    #[test]
    fn skill_config_intent_resource_failures_reply_with_authoritative_snapshot() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.world_mut()
            .resource_mut::<SkillConfigStore>()
            .set_config(
                "offline:Azure",
                "zhenmai.sever_chain",
                crate::skill::config::SkillConfig::new(std::collections::BTreeMap::from([
                    ("meridian_id".to_string(), serde_json::json!("Pericardium")),
                    (
                        "backfire_kind".to_string(),
                        serde_json::json!("tainted_yuan"),
                    ),
                ])),
            );
        app.world_mut().remove_resource::<SkillConfigSchemas>();
        let (client_bundle, mut helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: serde_json::to_vec(&ClientRequestV1::SkillConfigIntent {
                    v: 1,
                    skill_id: "zhenmai.sever_chain".to_string(),
                    config: std::collections::BTreeMap::from([
                        ("meridian_id".to_string(), serde_json::json!("Pericardium")),
                        ("backfire_kind".to_string(), serde_json::json!("array")),
                    ]),
                })
                .unwrap()
                .into_boxed_slice(),
            });
        app.update();
        flush_all_client_packets(&mut app);
        let snapshots = collect_skill_config_snapshots(&mut helper);
        assert_eq!(snapshots.len(), 1);
        assert_eq!(
            snapshots[0]
                .configs
                .get("zhenmai.sever_chain")
                .and_then(|config| config.fields.get("backfire_kind")),
            Some(&serde_json::json!("tainted_yuan"))
        );

        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.world_mut().remove_resource::<SkillConfigStore>();
        let (client_bundle, mut helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: serde_json::to_vec(&ClientRequestV1::SkillConfigIntent {
                    v: 1,
                    skill_id: "zhenmai.sever_chain".to_string(),
                    config: std::collections::BTreeMap::from([
                        ("meridian_id".to_string(), serde_json::json!("Pericardium")),
                        ("backfire_kind".to_string(), serde_json::json!("array")),
                    ]),
                })
                .unwrap()
                .into_boxed_slice(),
            });
        app.update();
        flush_all_client_packets(&mut app);
        let snapshots = collect_skill_config_snapshots(&mut helper);
        assert_eq!(snapshots.len(), 1);
        assert!(snapshots[0].configs.is_empty());
    }

    #[test]
    fn skill_bar_cast_protocol_entity_id_does_not_fallback_to_entity_bits() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let target = app.world_mut().spawn(Position::new([1.0, 0.0, 0.0])).id();
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut().entity_mut(entity).insert((
            Position::new([0.0, 0.0, 0.0]),
            crate::cultivation::components::Cultivation {
                realm: crate::cultivation::components::Realm::Induce,
                qi_current: 100.0,
                qi_max: 100.0,
                ..Default::default()
            },
            crate::cultivation::components::MeridianSystem::default(),
            SkillBarBindings::default(),
            QuickSlotBindings::default(),
            empty_inventory(),
        ));
        app.world_mut()
            .get_mut::<SkillBarBindings>(entity)
            .unwrap()
            .set(
                0,
                SkillSlot::Skill {
                    skill_id: "burst_meridian.beng_quan".to_string(),
                },
            );
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: serde_json::to_vec(&ClientRequestV1::SkillBarCast {
                    v: 1,
                    slot: 0,
                    target: Some(format!("entity:{}", target.to_bits())),
                })
                .unwrap()
                .into_boxed_slice(),
            });

        app.update();

        assert!(app.world().get::<Casting>(entity).is_none());
        assert_eq!(
            app.world()
                .resource::<valence::prelude::Events<crate::combat::events::AttackIntent>>()
                .len(),
            0
        );
    }

    #[test]
    fn skill_bar_cast_empty_item_or_cooldown_does_not_start_cast() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let mut skill_bar = SkillBarBindings::default();
        assert!(skill_bar.set(1, SkillSlot::Item { instance_id: 7 }));
        assert!(skill_bar.set(
            2,
            SkillSlot::Skill {
                skill_id: "burst_meridian.beng_quan".to_string(),
            },
        ));
        skill_bar.set_cooldown("burst_meridian.beng_quan", 100);
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                skill_bar,
                QuickSlotBindings::default(),
                empty_inventory(),
                // Grant ownership for slot 2's technique so the cooldown gate (not the ownership
                // gate) is what blocks the cast, keeping the test non-vacuous.
                known(&["burst_meridian.beng_quan"]),
            ))
            .id();
        for slot in [0_u8, 1, 2] {
            app.world_mut()
                .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
                .send(CustomPayloadEvent {
                    client: entity,
                    channel: ident!("bong:client_request").into(),
                    data: serde_json::to_vec(&ClientRequestV1::SkillBarCast {
                        v: 1,
                        slot,
                        target: None,
                    })
                    .unwrap()
                    .into_boxed_slice(),
                });
        }

        app.update();

        assert!(app.world().get::<Casting>(entity).is_none());
    }

    #[test]
    fn skill_bar_bind_rejects_unknown_skill() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                SkillBarBindings::default(),
                QuickSlotBindings::default(),
                empty_inventory(),
            ))
            .id();
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"skill_bar_bind","v":1,"slot":0,"binding":{"kind":"skill","skill_id":"unknown.skill"}}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

        app.update();

        let bindings = app.world().get::<SkillBarBindings>(entity).unwrap();
        assert!(matches!(bindings.slots[0], SkillSlot::Empty));
    }

    /// bughunt skillbar-rebind-cooldown-reset 返工共用：一个只要不在冷却中就一定能
    /// 把 `burst_meridian.beng_quan` 放出去的实体（Cultivation Induce+100 真元 +
    /// RIGHT_ARM_MERIDIANS opened + Position + 已学会两条技能），镜像
    /// `skill_bar_bind_skill_then_cast_starts_skillbar_cast`（本文件上方）验证过的
    /// 配方——用来让「冷却中不产生 Casting」这类断言不再因为缺前置组件而空洞：
    /// 必须证明"同一实体、不在冷却时确实能拿到 Casting"，才能说清"没拿到 Casting"
    /// 是冷却门挡的，不是别的前置缺失挡的。
    fn spawn_beng_quan_capable_entity(
        app: &mut App,
        skill_bar: SkillBarBindings,
    ) -> valence::prelude::Entity {
        let (client_bundle, _helper) = create_mock_client("Azure");
        // `ClientBundle` 自带 `Position`，与其它组件放进同一个 spawn 元组会因重复
        // component 类型 panic（Bevy bundle 校验）——必须先单独 spawn client_bundle，
        // 再用 `insert` 覆盖/追加其余组件（`insert` 允许覆盖既有 component，`spawn`
        // 的 bundle 元组不允许同类型出现两次）。
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut().entity_mut(entity).insert((
            Position::new([0.0, 0.0, 0.0]),
            crate::cultivation::components::Cultivation {
                realm: crate::cultivation::components::Realm::Induce,
                qi_current: 100.0,
                qi_max: 100.0,
                ..Default::default()
            },
            beng_quan_ready_meridians(),
            skill_bar,
            QuickSlotBindings::default(),
            empty_inventory(),
            known(&["burst_meridian.beng_quan", "burst_meridian.tie_shan_kao"]),
        ));
        entity
    }

    /// bughunt skillbar-rebind-cooldown-reset — 重新绑定同一槽位为**相同**技能绝不能清零冷
    /// 却，否则玩家可通过「施放高冷却大招 → 立刻把同一招式重新拖回原槽位 → 立刻再次施放」
    /// 无限绕过任何走 `SkillBarBindings` 冷却的招式（含化虚终极技）。实体带全套前置组件
    /// （见 `spawn_beng_quan_capable_entity`），与下方
    /// `skill_bar_bind_same_skill_when_off_cooldown_produces_casting` 正向对照——
    /// 唯一差异是冷却状态，从而排除"没 Casting 是因为缺组件"的空洞断言。
    #[test]
    fn skill_bar_bind_same_skill_does_not_reset_cooldown() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);

        let mut skill_bar = SkillBarBindings::default();
        assert!(skill_bar.set(
            2,
            SkillSlot::Skill {
                skill_id: "burst_meridian.beng_quan".to_string(),
            },
        ));
        // 冷却设在 clock.tick(=0) 之后很远，模拟刚放完一记高冷却招式。
        skill_bar.set_cooldown("burst_meridian.beng_quan", 1_000);
        let entity = spawn_beng_quan_capable_entity(&mut app, skill_bar);

        // 玩家把同一招式重新拖回同一槽位——绑定内容完全没变。
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"skill_bar_bind","v":1,"slot":2,"binding":{"kind":"skill","skill_id":"burst_meridian.beng_quan"}}"#
                    .to_vec()
                    .into_boxed_slice(),
            });

        app.update();

        let bindings = app.world().get::<SkillBarBindings>(entity).unwrap();
        assert!(matches!(
            &bindings.slots[2],
            SkillSlot::Skill { skill_id } if skill_id == "burst_meridian.beng_quan"
        ));
        assert_eq!(
            bindings.cooldowns.get("burst_meridian.beng_quan").copied(),
            Some(1_000),
            "重绑内容与原绑定相同——冷却必须原样保留，否则重复拖拽同一招式=无限缩短冷却"
        );
        assert!(
            bindings.is_on_cooldown("burst_meridian.beng_quan", 0),
            "冷却状态应保持——重绑同值不是重置冷却的合法手段"
        );

        // 冷却仍未清空 → 再次尝试施放应仍被拒绝（不产生 Casting）。此断言之所以不空洞，
        // 是因为同一套实体前置组件在下方正向对照用例里已证明"不在冷却时确实会产生 Casting"。
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: serde_json::to_vec(&ClientRequestV1::SkillBarCast {
                    v: 1,
                    slot: 2,
                    target: None,
                })
                .unwrap()
                .into_boxed_slice(),
            });
        app.update();
        assert!(
            app.world().get::<Casting>(entity).is_none(),
            "冷却未被清零，重复施放不应产生 Casting（否则等于绕过了冷却）"
        );
    }

    /// 正向对照（opus verify 指出的空洞断言修复）：与上一条用例**完全相同**的实体前置
    /// （Cultivation+MeridianSystem+Position+已学会），唯一差异是不在冷却中——必须
    /// 产生 Casting。这条用例存在的意义就是证明上一条的"无 Casting"确实是冷却门挡的，
    /// 而不是随便一个缺前置的实体本来就永远拿不到 Casting。
    #[test]
    fn skill_bar_bind_same_skill_when_off_cooldown_produces_casting() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);

        let mut skill_bar = SkillBarBindings::default();
        assert!(skill_bar.set(
            2,
            SkillSlot::Skill {
                skill_id: "burst_meridian.beng_quan".to_string(),
            },
        ));
        // 故意不设冷却（默认 cooldowns map 为空）——同值重绑不应把"从未 cast 过"
        // 变成"被清零过"以外的任何状态，就绪态应保持就绪。
        let entity = spawn_beng_quan_capable_entity(&mut app, skill_bar);

        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"skill_bar_bind","v":1,"slot":2,"binding":{"kind":"skill","skill_id":"burst_meridian.beng_quan"}}"#
                    .to_vec()
                    .into_boxed_slice(),
            });
        app.update();
        assert!(
            !app.world()
                .get::<SkillBarBindings>(entity)
                .unwrap()
                .is_on_cooldown("burst_meridian.beng_quan", 0),
            "同值重绑前本就未 cast 过，不应凭空产生冷却"
        );

        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: serde_json::to_vec(&ClientRequestV1::SkillBarCast {
                    v: 1,
                    slot: 2,
                    target: None,
                })
                .unwrap()
                .into_boxed_slice(),
            });
        app.update();

        assert!(
            app.world().get::<Casting>(entity).is_some(),
            "正向对照：同一套前置组件、不在冷却中时必须产生 Casting——否则说明上面\
             「无 Casting」的断言是被缺前置组件挡住的空洞断言，而不是被冷却门挡住的"
        );
    }

    /// bughunt skillbar-rebind-cooldown-reset 阻塞问题 A（往返换绑路径）——换绑到
    /// **不同**技能，绝不能清零任何技能的冷却（旧行为"内容变化即清零"正是 A→B→A
    /// 换绑能清空原技能冷却的入口）。冷却按 skill_id 归属后，换绑动作本身完全不再
    /// 触碰任何 cooldowns entry；随后再换绑回原技能，原技能的冷却必须依然健在。
    #[test]
    fn skill_bar_bind_different_skill_never_touches_either_skills_cooldown() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);

        let mut skill_bar = SkillBarBindings::default();
        assert!(skill_bar.set(
            2,
            SkillSlot::Skill {
                skill_id: "burst_meridian.beng_quan".to_string(),
            },
        ));
        skill_bar.set_cooldown("burst_meridian.beng_quan", 1_000);
        let entity = spawn_beng_quan_capable_entity(&mut app, skill_bar);

        // 换绑到另一招式——绑定内容确实变了，但这不再是清零任何冷却的手段。
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"skill_bar_bind","v":1,"slot":2,"binding":{"kind":"skill","skill_id":"burst_meridian.tie_shan_kao"}}"#
                    .to_vec()
                    .into_boxed_slice(),
            });
        app.update();

        {
            let bindings = app.world().get::<SkillBarBindings>(entity).unwrap();
            assert!(matches!(
                &bindings.slots[2],
                SkillSlot::Skill { skill_id } if skill_id == "burst_meridian.tie_shan_kao"
            ));
            assert!(
                bindings.is_on_cooldown("burst_meridian.beng_quan", 0),
                "换绑到不同技能不得清零 beng_quan 的冷却——这正是 A→B→A 换绑能清空原技能\
                 冷却的攻击面，冷却按 skill_id 归属后必须与槽位内容变化完全解耦"
            );
            assert!(
                !bindings.is_on_cooldown("burst_meridian.tie_shan_kao", 0),
                "tie_shan_kao 从未被 cast 过，不应凭空产生冷却"
            );
        }

        // A→B→A 收尾：再换绑回 beng_quan——冷却必须依然健在（往返换绑不是清零手段）。
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"skill_bar_bind","v":1,"slot":2,"binding":{"kind":"skill","skill_id":"burst_meridian.beng_quan"}}"#
                    .to_vec()
                    .into_boxed_slice(),
            });
        app.update();

        let bindings = app.world().get::<SkillBarBindings>(entity).unwrap();
        assert!(matches!(
            &bindings.slots[2],
            SkillSlot::Skill { skill_id } if skill_id == "burst_meridian.beng_quan"
        ));
        assert!(
            bindings.is_on_cooldown("burst_meridian.beng_quan", 0),
            "A→B→A 往返换绑收尾后，beng_quan 的冷却必须全程原样保留"
        );
    }

    /// bughunt skillbar-rebind-cooldown-reset 阻塞问题 A 的另一半（清空→重绑路径）：
    /// 客户端「右键清空 / 拖出槽外」发送 `binding: null`（`SkillBarBind{binding: None}`
    /// → `SkillSlot::Empty`），随后把同一招式重新拖回——这条链路在 opus verify 里被
    /// 明确点名为"净效果=两次点击、零代价绕过冷却"的等价路径，必须同样锁死。
    #[test]
    fn skill_bar_bind_clear_then_rebind_same_skill_does_not_reset_cooldown() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);

        let mut skill_bar = SkillBarBindings::default();
        assert!(skill_bar.set(
            2,
            SkillSlot::Skill {
                skill_id: "burst_meridian.beng_quan".to_string(),
            },
        ));
        skill_bar.set_cooldown("burst_meridian.beng_quan", 1_000);
        let entity = spawn_beng_quan_capable_entity(&mut app, skill_bar);

        // 右键清空 / 拖出槽外：binding=null → SkillSlot::Empty。
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"skill_bar_bind","v":1,"slot":2,"binding":null}"#
                    .to_vec()
                    .into_boxed_slice(),
            });
        app.update();
        {
            let bindings = app.world().get::<SkillBarBindings>(entity).unwrap();
            assert!(matches!(bindings.slots[2], SkillSlot::Empty));
            assert!(
                bindings.is_on_cooldown("burst_meridian.beng_quan", 0),
                "清空槽位不得清零 beng_quan 的冷却——否则「清空→重绑」两次点击即可绕过冷却"
            );
        }

        // 把同一招式重新拖回原槽位。
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"skill_bar_bind","v":1,"slot":2,"binding":{"kind":"skill","skill_id":"burst_meridian.beng_quan"}}"#
                    .to_vec()
                    .into_boxed_slice(),
            });
        app.update();

        let bindings = app.world().get::<SkillBarBindings>(entity).unwrap();
        assert!(
            bindings.is_on_cooldown("burst_meridian.beng_quan", 0),
            "清空→重绑完整走一遍后，beng_quan 的冷却仍必须原样保留"
        );

        // 冷却仍未清空 → 尝试施放应仍被拒绝。
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: serde_json::to_vec(&ClientRequestV1::SkillBarCast {
                    v: 1,
                    slot: 2,
                    target: None,
                })
                .unwrap()
                .into_boxed_slice(),
            });
        app.update();
        assert!(
            app.world().get::<Casting>(entity).is_none(),
            "清空→重绑不是绕过冷却的合法手段，冷却仍在时不应产生 Casting"
        );
    }

    // ─────────────────────────────────────────────────────────────────
    // plan-inventory-hint-panel-v1 P0 — 伪皮胸槽境界门控并入 InventoryMoveRejectReason::
    // RealmTooLow：拒绝走 enum（走 emit_inventory_move_rejected 下发结构化 payload），
    // 而不是原独立硬编码分支的 warn-only（连 Result 都不走）。
    // ─────────────────────────────────────────────────────────────────

    /// 从 `MockClientHelper` 收到的包里解出所有 `InventoryMoveRejected` payload
    /// （测试构建走 JSON 序列化，见 `serialize_server_data_payload` 的 `#[cfg(test)]` 分支）。
    fn collect_inventory_move_rejected(
        helper: &mut MockClientHelper,
    ) -> Vec<crate::schema::server_data::InventoryMoveRejectedV1> {
        let mut payloads = Vec::new();
        for frame in helper.collect_received().0 {
            let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                continue;
            };
            if packet.channel.as_str() != crate::network::agent_bridge::SERVER_DATA_CHANNEL {
                continue;
            }
            let Ok(payload) = serde_json::from_slice::<crate::schema::server_data::ServerDataV1>(
                packet.data.0 .0,
            ) else {
                continue;
            };
            if let crate::schema::server_data::ServerDataPayloadV1::InventoryMoveRejected(data) =
                payload.payload
            {
                payloads.push(data);
            }
        }
        payloads
    }

    /// 境界不足时装备伪皮（fake_spirit_hide → SpiderSilk，min_realm=Induce）：
    /// realm=Awaken（< Induce）应被拒绝，走 `InventoryMoveRejectReason::RealmTooLow`
    /// → 下发 `InventoryMoveRejectedV1{reason:"realm_too_low", required_realm:"Induce"}`
    /// → 不修改 inventory（伪皮件仍在原容器格，未落进 chest worn）。
    #[test]
    fn equip_false_skin_realm_too_low_emits_structured_rejection() {
        use crate::combat::tuike::FAKE_SPIRIT_HIDE_ITEM_ID;
        use crate::cultivation::components::{Cultivation, Realm};

        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);

        let (client_bundle, mut helper) = create_mock_client("Kiz");
        let item = inventory_test_item(9101, FAKE_SPIRIT_HIDE_ITEM_ID, 1);
        let player = app
            .world_mut()
            .spawn((
                client_bundle,
                inventory_with_item(item),
                Cultivation {
                    realm: Realm::Awaken,
                    ..Default::default()
                },
            ))
            .id();

        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: player,
                channel: ident!("bong:client_request").into(),
                data: serde_json::to_vec(&ClientRequestV1::EquipFalseSkin {
                    v: 1,
                    slot: crate::schema::inventory::EquipSlotV1::Chest,
                    item_instance_id: 9101,
                })
                .expect("equip_false_skin request should serialize")
                .into_boxed_slice(),
            });

        app.update();
        flush_all_client_packets(&mut app);

        let rejections = collect_inventory_move_rejected(&mut helper);
        assert_eq!(
            rejections.len(),
            1,
            "境界不足的伪皮装备应下发恰好 1 条 InventoryMoveRejected"
        );
        let rejection = &rejections[0];
        assert_eq!(rejection.reason, "realm_too_low");
        assert_eq!(
            rejection.required_realm.as_deref(),
            Some("Induce"),
            "SpiderSilk 型伪皮 min_realm=Induce，应下发英文 tag 供 client RealmLabel 转中文"
        );
        assert!(rejection.slot.is_none(), "realm_too_low 不带 slot/cap");
        assert!(rejection.cap.is_none());

        // 拒绝后伪皮件仍留在原格，未被写入 chest worn（走 enum 拒绝而非静默放行）。
        let inventory = app
            .world()
            .get::<PlayerInventory>(player)
            .expect("player inventory should still exist");
        assert!(
            inventory
                .equipped
                .get(crate::inventory::EQUIP_SLOT_CHEST)
                .map(|c| c.worn.is_empty())
                .unwrap_or(true),
            "境界不足时伪皮不应落进 chest worn"
        );
    }

    // ─── plan-scroll-reading-v1 P2 — ScrollReadRequest/ScrollReadClosed 循环动画 e2e ───
    // 覆盖「开卷插 marker + 发 PlayAnim」→「关屏发 StopAnim + 移除 marker」全链路，以及
    // anim_id=None（无动画残卷）/ 未开卷即关屏（no-op）/ 重复关屏（幂等）三个边界。
    use crate::network::scroll_open_emit::ScrollReading;
    use crate::schema::vfx_event::VfxEventPayloadV1;

    fn readable_scroll_template(id: &str, anim_id: Option<&str>) -> ItemTemplate {
        ItemTemplate {
            id: id.to_string(),
            display_name: "《测试残卷》".to_string(),
            category: ItemCategory::Scroll,
            placeable: None,
            max_stack_count: 1,
            grid_w: 1,
            grid_h: 2,
            base_weight: 0.05,
            rarity: ItemRarity::Common,
            spirit_quality_initial: 0.3,
            description: "test".to_string(),
            effect: None,
            cast_duration_ms: 1500,
            cooldown_ms: 1500,
            weapon_spec: None,
            forge_station_spec: None,
            blueprint_scroll_spec: None,
            inscription_scroll_spec: None,
            technique_scroll_spec: None,
            readable_scroll_spec: Some(crate::inventory::ReadableScrollSpec {
                title: "《测试残卷》".to_string(),
                body_pages: vec!["第一页".to_string()],
                anim_id: anim_id.map(|s| s.to_string()),
            }),
            recipe_fragment_spec: None,
            container_spec: None,
            shelflife_profile: None,
            shield_spec: None,
            shelflife_track: None,
            wearer_race: crate::body_plan::types::RaceGateOwned::default(),
        }
    }

    fn inventory_with_scroll(instance_id: u64, template_id: &str) -> PlayerInventory {
        let mut inv = empty_inventory();
        inv.containers[0].items.push(PlacedItemState {
            row: 0,
            col: 0,
            instance: inventory_test_item(instance_id, template_id, 1),
        });
        inv
    }

    fn scroll_anim_drain_vfx(
        app: &mut App,
    ) -> Vec<crate::network::vfx_event_emit::VfxEventRequest> {
        app.world_mut()
            .resource_mut::<valence::prelude::Events<crate::network::vfx_event_emit::VfxEventRequest>>()
            .drain()
            .collect()
    }

    fn scroll_anim_find_play<'a>(
        reqs: &'a [crate::network::vfx_event_emit::VfxEventRequest],
        anim_id: &str,
    ) -> Option<&'a crate::network::vfx_event_emit::VfxEventRequest> {
        reqs.iter().find(|r| {
            matches!(&r.payload, VfxEventPayloadV1::PlayAnim { anim_id: id, .. } if id == anim_id)
        })
    }

    fn scroll_anim_find_stop<'a>(
        reqs: &'a [crate::network::vfx_event_emit::VfxEventRequest],
        anim_id: &str,
    ) -> Option<&'a crate::network::vfx_event_emit::VfxEventRequest> {
        reqs.iter().find(|r| {
            matches!(&r.payload, VfxEventPayloadV1::StopAnim { anim_id: id, .. } if id == anim_id)
        })
    }

    fn scroll_anim_find_spawn_particle<'a>(
        reqs: &'a [crate::network::vfx_event_emit::VfxEventRequest],
        event_id: &str,
    ) -> Option<&'a crate::network::vfx_event_emit::VfxEventRequest> {
        reqs.iter().find(|r| {
            matches!(&r.payload, VfxEventPayloadV1::SpawnParticle { event_id: id, .. } if id == event_id)
        })
    }

    fn send_scroll_read_request(app: &mut App, entity: Entity, instance_id: u64) {
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: serde_json::to_vec(&ClientRequestV1::ScrollReadRequest { v: 1, instance_id })
                    .expect("scroll_read_request should serialize")
                    .into_boxed_slice(),
            });
    }

    fn send_scroll_read_closed(app: &mut App, entity: Entity) {
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: serde_json::to_vec(&ClientRequestV1::ScrollReadClosed { v: 1 })
                    .expect("scroll_read_closed should serialize")
                    .into_boxed_slice(),
            });
    }

    // ── happy path: 开卷插 marker + 发 PlayAnim ─────────────────────────
    #[test]
    fn scroll_read_request_inserts_marker_and_emits_play_anim() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(ItemRegistry::from_map(HashMap::from([(
            "scroll_meridian_primer".to_string(),
            readable_scroll_template("scroll_meridian_primer", Some("bong:read_scroll")),
        )])));

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                inventory_with_scroll(42, "scroll_meridian_primer"),
            ))
            .id();

        send_scroll_read_request(&mut app, entity, 42);
        app.update();

        assert!(
            app.world().get::<ScrollReading>(entity).is_some(),
            "ScrollReadRequest with anim_id must insert ScrollReading marker \
             (真相源 for ScrollReadClosed / death cleanup to find later)"
        );
        let emitted = scroll_anim_drain_vfx(&mut app);
        assert!(
            scroll_anim_find_play(&emitted, "bong:read_scroll").is_some(),
            "ScrollReadRequest must emit PlayAnim{{anim_id==\"bong:read_scroll\"}} \
             when spec has anim_id, got {emitted:?}"
        );
    }

    // ── 边界: spec.anim_id=None → 不插 marker、不发 PlayAnim ──────────────
    #[test]
    fn scroll_read_request_without_anim_id_does_not_insert_marker() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(ItemRegistry::from_map(HashMap::from([(
            "scroll_no_anim".to_string(),
            readable_scroll_template("scroll_no_anim", None),
        )])));

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((client_bundle, inventory_with_scroll(7, "scroll_no_anim")))
            .id();

        send_scroll_read_request(&mut app, entity, 7);
        app.update();

        assert!(
            app.world().get::<ScrollReading>(entity).is_none(),
            "spec.anim_id=None must not insert a ScrollReading marker — there is no \
             loop animation to stop later"
        );
        let emitted = scroll_anim_drain_vfx(&mut app);
        assert!(
            scroll_anim_find_play(&emitted, "bong:read_scroll").is_none(),
            "no anim_id means no PlayAnim should be emitted, got {emitted:?}"
        );
        assert!(
            scroll_anim_find_spawn_particle(&emitted, "bong:scroll_open_glow").is_some(),
            "展开微光与 anim_id 是否存在无关——即便残卷没有阅读动画，开卷仍应有 \
             SpawnParticle{{event_id==\"bong:scroll_open_glow\"}}，got {emitted:?}"
        );
    }

    // ── happy path: 开卷发展开微光 SpawnParticle（与 anim_id 是否存在无关）──────
    #[test]
    fn scroll_read_request_emits_scroll_open_glow_particle() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(ItemRegistry::from_map(HashMap::from([(
            "scroll_meridian_primer".to_string(),
            readable_scroll_template("scroll_meridian_primer", Some("bong:read_scroll")),
        )])));

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                inventory_with_scroll(42, "scroll_meridian_primer"),
            ))
            .id();

        send_scroll_read_request(&mut app, entity, 42);
        app.update();

        let emitted = scroll_anim_drain_vfx(&mut app);
        let glow = scroll_anim_find_spawn_particle(&emitted, "bong:scroll_open_glow").expect(
            "ScrollReadRequest must emit SpawnParticle{event_id==\"bong:scroll_open_glow\"}",
        );
        match &glow.payload {
            VfxEventPayloadV1::SpawnParticle {
                color,
                count,
                strength,
                duration_ticks,
                ..
            } => {
                assert_eq!(
                    color.as_deref(),
                    Some("#E8D9A0"),
                    "scroll_open_glow must use the pinned pale-gold color #E8D9A0"
                );
                assert_eq!(*count, Some(12), "burst count must be pinned to 12");
                assert_eq!(*strength, Some(0.85));
                assert_eq!(*duration_ticks, Some(20));
            }
            other => panic!("expected SpawnParticle, got {other:?}"),
        }
    }

    // ── happy path: 关屏发 StopAnim + 移除 marker ───────────────────────
    #[test]
    fn scroll_read_closed_emits_stop_anim_and_removes_marker() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(ItemRegistry::from_map(HashMap::from([(
            "scroll_meridian_primer".to_string(),
            readable_scroll_template("scroll_meridian_primer", Some("bong:read_scroll")),
        )])));

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                inventory_with_scroll(42, "scroll_meridian_primer"),
            ))
            .id();

        send_scroll_read_request(&mut app, entity, 42);
        app.update();
        let _ = scroll_anim_drain_vfx(&mut app); // discard open events, focus on close

        send_scroll_read_closed(&mut app, entity);
        app.update();

        let emitted = scroll_anim_drain_vfx(&mut app);
        assert!(
            scroll_anim_find_stop(&emitted, "bong:read_scroll").is_some(),
            "ScrollReadClosed must emit StopAnim{{anim_id==\"bong:read_scroll\"}} \
             when a ScrollReading marker was present, got {emitted:?}"
        );
        assert!(
            app.world().get::<ScrollReading>(entity).is_none(),
            "ScrollReadClosed must remove the ScrollReading marker after stopping the anim"
        );
    }

    // ── 边界: 未开卷即发 ScrollReadClosed → no-op（不 panic，不发 StopAnim）──
    #[test]
    fn scroll_read_closed_without_active_reading_is_noop() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();

        send_scroll_read_closed(&mut app, entity);
        app.update();

        let emitted = scroll_anim_drain_vfx(&mut app);
        assert!(
            scroll_anim_find_stop(&emitted, "bong:read_scroll").is_none(),
            "ScrollReadClosed with no prior ScrollReadRequest must not emit StopAnim \
             (no ScrollReading marker to act on), got {emitted:?}"
        );
        assert!(
            app.world().get::<ScrollReading>(entity).is_none(),
            "no marker should exist to begin with"
        );
    }

    // ── 重复关屏: 第二次 ScrollReadClosed 不再重复发 StopAnim ────────────
    #[test]
    fn repeated_scroll_read_closed_only_stops_once() {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        register_request_app(&mut app);
        app.insert_resource(ItemRegistry::from_map(HashMap::from([(
            "scroll_meridian_primer".to_string(),
            readable_scroll_template("scroll_meridian_primer", Some("bong:read_scroll")),
        )])));

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                inventory_with_scroll(42, "scroll_meridian_primer"),
            ))
            .id();

        send_scroll_read_request(&mut app, entity, 42);
        app.update();
        let _ = scroll_anim_drain_vfx(&mut app);

        send_scroll_read_closed(&mut app, entity);
        app.update();
        let first_close = scroll_anim_drain_vfx(&mut app);
        assert!(
            scroll_anim_find_stop(&first_close, "bong:read_scroll").is_some(),
            "first ScrollReadClosed must stop the animation, got {first_close:?}"
        );

        send_scroll_read_closed(&mut app, entity);
        app.update();
        let second_close = scroll_anim_drain_vfx(&mut app);
        assert!(
            scroll_anim_find_stop(&second_close, "bong:read_scroll").is_none(),
            "second ScrollReadClosed after marker already removed must be a no-op \
             (idempotent close, not a repeated StopAnim), got {second_close:?}"
        );
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
    if slot >= 9 {
        tracing::warn!(
            "[bong][network] use_quick_slot entity={entity:?} ignored: slot {slot} out of range"
        );
        return;
    }
    // 契约顺序（network_quickslot_config.py docstring：slot>=9 / 无绑定 / 冷却 /
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
    if slot >= QuickSlotBindings::SLOT_COUNT as u8 {
        tracing::warn!(
            "[bong][network] quick_slot_bind entity={entity:?} slot={slot} out of range"
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
        && player_position.distance_squared(elder_position)
            <= GIVE_DAN_MAX_DISTANCE * GIVE_DAN_MAX_DISTANCE
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
mod take_pill_tests {
    use super::*;
    use crate::inventory::{ContainerState, InventoryRevision, ItemInstance, ItemRarity};

    fn make_pill(instance_id: u64, template_id: &str, stack: u32) -> ItemInstance {
        ItemInstance {
            instance_id,
            template_id: template_id.to_string(),
            display_name: template_id.to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.1,
            rarity: ItemRarity::Rare,
            description: String::new(),
            stack_count: stack,
            spirit_quality: 1.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        }
    }

    fn fresh_inventory() -> PlayerInventory {
        PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(0),
            containers: vec![ContainerState {
                quick_access: false,
                id: "main".into(),
                name: "main".into(),
                rows: 4,
                cols: 4,
                items: Vec::new(),

                owner_instance_id: None,
            }],
            equipped: Default::default(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 100.0,
        }
    }

    #[test]
    fn consume_hotbar_decrements_stack() {
        let mut inv = fresh_inventory();
        inv.hotbar[2] = Some(make_pill(1, "guyuan_pill", 3));
        assert!(consume_one_by_template(&mut inv, "guyuan_pill"));
        assert_eq!(inv.hotbar[2].as_ref().unwrap().stack_count, 2);
        assert_eq!(inv.revision.0, 1);
    }

    #[test]
    fn consume_hotbar_removes_slot_when_stack_one() {
        let mut inv = fresh_inventory();
        inv.hotbar[0] = Some(make_pill(1, "guyuan_pill", 1));
        assert!(consume_one_by_template(&mut inv, "guyuan_pill"));
        assert!(inv.hotbar[0].is_none());
    }

    #[test]
    fn consume_falls_back_to_container_when_hotbar_missing() {
        let mut inv = fresh_inventory();
        inv.containers[0]
            .items
            .push(crate::inventory::PlacedItemState {
                row: 0,
                col: 0,
                instance: make_pill(7, "guyuan_pill", 2),
            });
        assert!(consume_one_by_template(&mut inv, "guyuan_pill"));
        assert_eq!(inv.containers[0].items[0].instance.stack_count, 1);
    }

    #[test]
    fn alchemy_attrition_selection_matches_consume_order() {
        let mut inv = fresh_inventory();
        inv.hotbar[0] = Some(make_pill(11, "guyuan_pill", 1));
        inv.containers[0]
            .items
            .push(crate::inventory::PlacedItemState {
                row: 0,
                col: 0,
                instance: make_pill(22, "guyuan_pill", 1),
            });

        assert_eq!(
            select_template_instances_for_consumption(&inv, "guyuan_pill", 1),
            vec![11],
            "投料磨损应命中 hotbar 中即将被 consume_one_by_template 消耗的实例"
        );
        assert!(consume_one_by_template(&mut inv, "guyuan_pill"));
        assert!(inv.hotbar[0].is_none());
        assert_eq!(inv.containers[0].items[0].instance.instance_id, 22);
    }

    #[test]
    fn alchemy_attrition_selection_spans_consumed_stacks_once_per_instance() {
        let mut inv = fresh_inventory();
        inv.hotbar[0] = Some(make_pill(11, "guyuan_pill", 2));
        inv.containers[0]
            .items
            .push(crate::inventory::PlacedItemState {
                row: 0,
                col: 0,
                instance: make_pill(22, "guyuan_pill", 3),
            });
        inv.equipped.insert(
            "off_hand".into(),
            crate::inventory::SlotContents::held_single(make_pill(33, "guyuan_pill", 1)),
        );

        assert_eq!(
            select_template_instances_for_consumption(&inv, "guyuan_pill", 5),
            vec![11, 22],
            "投料磨损应按 hotbar → containers → equipped 覆盖将被消耗的实例"
        );
    }

    #[test]
    fn alchemy_ingredient_selection_skips_wrong_mineral_and_uses_matching_instances() {
        let mut inv = fresh_inventory();
        let mut hotbar_wrong = make_pill(11, "dan_sha_aux", 2);
        hotbar_wrong.mineral_id = Some("zhu_sha".into());
        inv.hotbar[0] = Some(hotbar_wrong);

        let mut container_match = make_pill(22, "dan_sha_aux", 1);
        container_match.mineral_id = Some("dan_sha".into());
        inv.containers[0]
            .items
            .push(crate::inventory::PlacedItemState {
                row: 0,
                col: 0,
                instance: container_match,
            });

        let mut equipped_match = make_pill(33, "dan_sha_aux", 3);
        equipped_match.mineral_id = Some("dan_sha".into());
        inv.equipped.insert(
            "off_hand".into(),
            crate::inventory::SlotContents::held_single(equipped_match),
        );
        let ingredient = crate::alchemy::recipe::IngredientSpec {
            material: "dan_sha_aux".into(),
            count: 2,
            mineral_id: Some("dan_sha".into()),
        };

        assert_eq!(
            select_ingredient_instances_for_consumption(&inv, &ingredient, 2),
            Some(vec![(22, 1), (33, 1)]),
            "expected wrong mineral instance 11 to be skipped and matching instances to fill required count across inventory positions"
        );
    }

    #[test]
    fn alchemy_ingredient_selection_returns_none_when_matching_mineral_is_short() {
        let mut inv = fresh_inventory();
        let mut wrong_mineral = make_pill(11, "dan_sha_aux", 5);
        wrong_mineral.mineral_id = Some("zhu_sha".into());
        inv.hotbar[0] = Some(wrong_mineral);
        let mut matching_mineral = make_pill(22, "dan_sha_aux", 1);
        matching_mineral.mineral_id = Some("dan_sha".into());
        inv.containers[0]
            .items
            .push(crate::inventory::PlacedItemState {
                row: 0,
                col: 0,
                instance: matching_mineral,
            });
        let ingredient = crate::alchemy::recipe::IngredientSpec {
            material: "dan_sha_aux".into(),
            count: 2,
            mineral_id: Some("dan_sha".into()),
        };

        assert_eq!(
            select_ingredient_instances_for_consumption(&inv, &ingredient, 2),
            None,
            "expected selection to reject shortage when only one matching dan_sha item exists and wrong-mineral stacks cannot satisfy the ingredient"
        );
    }

    #[test]
    fn consume_returns_false_if_template_missing() {
        let mut inv = fresh_inventory();
        assert!(!consume_one_by_template(&mut inv, "ghost_pill"));
        assert_eq!(inv.revision.0, 0);
    }

    #[test]
    fn resolve_pill_consume_target_uses_exact_instance_when_provided() {
        let mut inv = fresh_inventory();
        inv.containers[0]
            .items
            .push(crate::inventory::PlacedItemState {
                row: 0,
                col: 0,
                instance: make_pill(7, "guyuan_pill", 1),
            });
        inv.containers[0]
            .items
            .push(crate::inventory::PlacedItemState {
                row: 0,
                col: 1,
                instance: make_pill(8, "guyuan_pill", 1),
            });

        let item = resolve_pill_consume_target(&inv, "guyuan_pill", Some(8)).unwrap();

        assert_eq!(item.instance_id, 8);
    }

    #[test]
    fn shelflife_warn_emits_spoil_warning() {
        let profile = crate::shelflife::DecayProfile::Spoil {
            id: crate::shelflife::DecayProfileId::new("test_spoil"),
            formula: crate::shelflife::DecayFormula::Exponential {
                half_life_ticks: 100,
            },
            spoil_threshold: 60.0,
        };
        let mut profiles = DecayProfileRegistry::new();
        profiles.insert(profile.clone()).unwrap();
        let mut item = make_pill(9, "guyuan_pill", 1);
        item.freshness = Some(crate::shelflife::Freshness::new(0, 100.0, &profile));

        let (spoil, age) = shelflife_checks_for_item(&item, 100, Some(&profiles), None);

        assert!(matches!(spoil, SpoilCheckOutcome::Warn { .. }));
        assert!(matches!(age, AgePeakCheck::NotApplicable));
    }

    #[test]
    fn shelflife_critical_block_is_detected_before_consumption() {
        let profile = crate::shelflife::DecayProfile::Spoil {
            id: crate::shelflife::DecayProfileId::new("test_spoil"),
            formula: crate::shelflife::DecayFormula::Exponential {
                half_life_ticks: 100,
            },
            spoil_threshold: 60.0,
        };
        let mut profiles = DecayProfileRegistry::new();
        profiles.insert(profile.clone()).unwrap();
        let mut item = make_pill(9, "guyuan_pill", 1);
        item.freshness = Some(crate::shelflife::Freshness::new(0, 100.0, &profile));

        let (spoil, _age) = shelflife_checks_for_item(&item, 1_000, Some(&profiles), None);

        assert!(matches!(spoil, SpoilCheckOutcome::CriticalBlock { .. }));
    }

    #[test]
    fn shelflife_checks_use_forced_world_season_state() {
        let profile = crate::shelflife::DecayProfile::Spoil {
            id: crate::shelflife::DecayProfileId::new("test_spoil"),
            formula: crate::shelflife::DecayFormula::Exponential {
                half_life_ticks: 100,
            },
            spoil_threshold: 60.0,
        };
        let mut profiles = DecayProfileRegistry::new();
        profiles.insert(profile.clone()).unwrap();
        let mut item = make_pill(9, "guyuan_pill", 1);
        item.freshness = Some(crate::shelflife::Freshness::new(0, 100.0, &profile));
        let now_tick = 70;
        let mut forced = WorldSeasonState::default();
        forced.set_phase(crate::world::season::Season::Winter, now_tick);

        let (raw_spoil, _) = shelflife_checks_for_item(&item, now_tick, Some(&profiles), None);
        let (forced_spoil, _) =
            shelflife_checks_for_item(&item, now_tick, Some(&profiles), Some(&forced));

        assert!(
            matches!(raw_spoil, SpoilCheckOutcome::Warn { .. }),
            "raw tick should still be summer-fast enough to warn"
        );
        assert!(
            matches!(forced_spoil, SpoilCheckOutcome::Safe { .. }),
            "forced winter phase should slow spoil checks immediately"
        );
    }
}

#[cfg(test)]
mod named_faction_reputation_tests {
    use super::*;
    use crate::npc::faction::{FactionId, FactionRank, MissionQueue, NamedFactionId, Reputation};

    fn membership_with_loyalty(loyalty: f64) -> FactionMembership {
        FactionMembership {
            faction_id: FactionId::Neutral,
            rank: FactionRank::Disciple,
            reputation: Reputation { loyalty },
            lineage: None,
            mission_queue: MissionQueue::default(),
        }
    }

    #[test]
    fn npc_zone_faction_reputation_replaces_global_identity_renown() {
        let mut identities = PlayerIdentities::with_default("Azure", 0);
        identities.active_mut().unwrap().renown.notoriety = 80;
        let mut faction_reputation = FactionReputation::default();
        faction_reputation.apply_delta(NamedFactionId::QingyunHunters, 60);

        let score = reputation_to_player_score_for_npc_zone(
            None,
            Some(&identities),
            Some(&faction_reputation),
            Some("qingyun_peaks"),
        );

        assert_eq!(
            score, 60,
            "青云 zone NPC 应读取 QingyunHunters per_faction 信誉，而不是全局 identity Renown"
        );
    }

    #[test]
    fn npc_zone_faction_reputation_falls_back_to_identity_for_unknown_zone() {
        let mut identities = PlayerIdentities::with_default("Azure", 0);
        identities.active_mut().unwrap().renown.notoriety = 80;
        let mut faction_reputation = FactionReputation::default();
        faction_reputation.apply_delta(NamedFactionId::QingyunHunters, 60);

        let score = reputation_to_player_score_for_npc_zone(
            None,
            Some(&identities),
            Some(&faction_reputation),
            Some("spawn"),
        );

        assert_eq!(
            score, -80,
            "未映射到具名势力的 zone 应保持 legacy identity Renown fallback"
        );
    }

    #[test]
    fn npc_zone_faction_reputation_falls_back_when_zone_or_reputation_missing() {
        let mut identities = PlayerIdentities::with_default("Azure", 0);
        identities.active_mut().unwrap().renown.notoriety = 40;
        let mut faction_reputation = FactionReputation::default();
        faction_reputation.apply_delta(NamedFactionId::QingyunHunters, 60);

        let missing_zone_score = reputation_to_player_score_for_npc_zone(
            None,
            Some(&identities),
            Some(&faction_reputation),
            None,
        );
        let missing_reputation_score = reputation_to_player_score_for_npc_zone(
            None,
            Some(&identities),
            None,
            Some("qingyun_peaks"),
        );
        let empty_score = reputation_to_player_score_for_npc_zone(None, None, None, None);

        assert_eq!(
            missing_zone_score, -40,
            "zone_name=None 时必须回退 legacy identity reputation，避免误读具名势力信誉"
        );
        assert_eq!(
            missing_reputation_score, -40,
            "玩家缺少 FactionReputation 组件时必须回退 legacy identity reputation"
        );
        assert_eq!(
            empty_score, 0,
            "缺少 membership/identity/faction reputation 的空输入应保持中立 0"
        );
    }

    #[test]
    fn npc_zone_faction_reputation_clamps_membership_plus_faction_score() {
        let high_membership = membership_with_loyalty(1.0);
        let low_membership = membership_with_loyalty(0.0);
        let medium_membership = membership_with_loyalty(0.245);
        let mut high_reputation = FactionReputation::default();
        high_reputation.apply_delta(NamedFactionId::QingyunHunters, 1);
        let mut low_reputation = FactionReputation::default();
        low_reputation.apply_delta(NamedFactionId::QingyunHunters, -1);
        let mut off_by_one_reputation = FactionReputation::default();
        off_by_one_reputation.apply_delta(NamedFactionId::QingyunHunters, 50);

        let upper = reputation_to_player_score_for_npc_zone(
            Some(&high_membership),
            None,
            Some(&high_reputation),
            Some("qingyun_peaks"),
        );
        let lower = reputation_to_player_score_for_npc_zone(
            Some(&low_membership),
            None,
            Some(&low_reputation),
            Some("qingyun_peaks"),
        );
        let off_by_one = reputation_to_player_score_for_npc_zone(
            Some(&medium_membership),
            None,
            Some(&off_by_one_reputation),
            Some("qingyun_peaks"),
        );

        assert_eq!(
            upper, 100,
            "membership baseline + faction score 超过上界时必须 clamp 到 100"
        );
        assert_eq!(
            lower, -100,
            "membership baseline + faction score 低于下界时必须 clamp 到 -100"
        );
        assert_eq!(
            off_by_one, -1,
            "未触及边界的 membership baseline + faction score 不应被误 clamp"
        );
    }

    #[test]
    fn wanted_tier_blocks_trade_even_when_score_would_otherwise_allow() {
        let target = NpcEngagementTarget {
            entity: Entity::PLACEHOLDER,
            archetype: NpcArchetype::Commoner,
            reputation_to_player: 100,
            faction_reputation_tier: FactionReputationTier::Wanted,
            display_name: "青云残峰散修".to_string(),
            greeting_text: String::new(),
            position: DVec3::ZERO,
            npc_player_rep: None,
        };

        assert!(
            !target.can_trade(),
            "Wanted tier 必须优先阻断交易，即使 reputation_to_player 分数本身足够高"
        );
    }
}

// ── plan-cultivation-pacing-v1 P2.2 NPC 丹药交易测试 ──

#[cfg(test)]
mod npc_flawed_pill_trade_tests {
    use super::*;
    use crate::npc::lifecycle::NpcArchetype;

    #[test]
    fn commoner_sells_flawed_ling_xi_wan_at_8_bones() {
        let result = npc_trade_catalog_entry(NpcArchetype::Commoner, "ling_xi_wan_flawed");
        assert_eq!(
            result,
            Some(("ling_xi_wan_flawed", 8)),
            "Commoner 应以 8 骨币售卖次品灵息丸"
        );
    }

    #[test]
    fn commoner_sells_flawed_ju_ling_dan_at_15_bones() {
        let result = npc_trade_catalog_entry(NpcArchetype::Commoner, "ju_ling_dan_flawed");
        assert_eq!(
            result,
            Some(("ju_ling_dan_flawed", 15)),
            "Commoner 应以 15 骨币售卖次品聚灵丹"
        );
    }

    #[test]
    fn rogue_sells_flawed_ling_xi_wan_at_8_bones() {
        let result = npc_trade_catalog_entry(NpcArchetype::Rogue, "ling_xi_wan_flawed");
        assert_eq!(
            result,
            Some(("ling_xi_wan_flawed", 8)),
            "Rogue 也应以 8 骨币售卖次品灵息丸"
        );
    }

    #[test]
    fn rogue_sells_flawed_ju_ling_dan_at_15_bones() {
        let result = npc_trade_catalog_entry(NpcArchetype::Rogue, "ju_ling_dan_flawed");
        assert_eq!(
            result,
            Some(("ju_ling_dan_flawed", 15)),
            "Rogue 也应以 15 骨币售卖次品聚灵丹"
        );
    }

    #[test]
    fn chinese_alias_also_resolves_for_commoner() {
        assert_eq!(
            npc_trade_catalog_entry(NpcArchetype::Commoner, "ling_xi_wan_次品"),
            Some(("ling_xi_wan_flawed", 8)),
            "中文别名 ling_xi_wan_次品 应解析到同一物品"
        );
        assert_eq!(
            npc_trade_catalog_entry(NpcArchetype::Commoner, "ju_ling_dan_次品"),
            Some(("ju_ling_dan_flawed", 15)),
            "中文别名 ju_ling_dan_次品 应解析到同一物品"
        );
    }

    #[test]
    fn beast_does_not_sell_flawed_pills() {
        assert!(
            npc_trade_catalog_entry(NpcArchetype::Beast, "ling_xi_wan_flawed").is_none(),
            "Beast 不应售卖次品丹药"
        );
    }

    #[test]
    fn zombie_does_not_sell_flawed_pills() {
        assert!(
            npc_trade_catalog_entry(NpcArchetype::Zombie, "ling_xi_wan_flawed").is_none(),
            "Zombie 不应售卖次品丹药"
        );
    }

    #[test]
    fn normal_pills_not_in_npc_catalog() {
        assert!(
            npc_trade_catalog_entry(NpcArchetype::Commoner, "ling_xi_wan").is_none(),
            "正品灵息丸不应在 NPC 交易目录中"
        );
        assert!(
            npc_trade_catalog_entry(NpcArchetype::Commoner, "ju_ling_dan").is_none(),
            "正品聚灵丹不应在 NPC 交易目录中"
        );
    }

    #[test]
    fn higher_pills_not_in_npc_catalog() {
        assert!(
            npc_trade_catalog_entry(NpcArchetype::Commoner, "tong_mai_san_flawed").is_none(),
            "通脉散以上 NPC 不售卖"
        );
        assert!(
            npc_trade_catalog_entry(NpcArchetype::Rogue, "xi_sui_ye_flawed").is_none(),
            "洗髓液以上 NPC 不售卖"
        );
    }

    /// 买路 spirit_grass 条目价格应为 10 骨币（与 TRADE_CATALOGUE 对齐）。
    #[test]
    fn buy_path_spirit_grass_price_10() {
        let result = npc_trade_catalog_entry(NpcArchetype::Commoner, "spirit_grass");
        assert_eq!(
            result,
            Some(("spirit_grass", 10)),
            "买路 spirit_grass 应以 10 骨币售卖（与 TRADE_CATALOGUE 对齐），\
             期望: Some((\"spirit_grass\", 10))，实际: {:?}",
            result
        );
    }

    /// 买路 broken_artifact_scroll 条目价格应为 40 骨币（与 TRADE_CATALOGUE 对齐）。
    #[test]
    fn buy_path_broken_artifact_scroll_price_40() {
        let result = npc_trade_catalog_entry(NpcArchetype::Rogue, "broken_artifact_scroll");
        assert_eq!(
            result,
            Some(("broken_artifact_scroll", 40)),
            "买路 broken_artifact_scroll 应以 40 骨币售卖（与 TRADE_CATALOGUE 对齐），\
             期望: Some((\"broken_artifact_scroll\", 40))，实际: {:?}",
            result
        );
    }
}

// ── RefuseRare rarity 门控逻辑单元测试 ─────────────────────────────────────
// 验证 TradeEligibility::RefuseRare arm 对不同 ItemRarity 的判断逻辑是正确的：
// - Rare+ (Rare/Epic/Legendary/Ancient) → 拒绝
// - Common/Uncommon → 通过（1.3x markup）
//
// NOTE：这组测试直接调用生产函数 is_rarity_refused_at_low_rep，
// 确保任何变体增删/修改都会立刻让测试撞红。
#[cfg(test)]
mod refuse_rare_rarity_gate_tests {
    use crate::inventory::ItemRarity;
    use crate::network::client_request_handler::is_rarity_refused_at_low_rep;

    /// Low 信誉买 Rare 物品（broken_artifact_scroll，rarity=Rare）→ 应被拒绝。
    /// 期望：is_rarity_refused_at_low_rep(Rare) = true（触发 continue，不走到 add_item）。
    #[test]
    fn rare_rarity_is_refused_for_low_rep() {
        assert!(
            is_rarity_refused_at_low_rep(ItemRarity::Rare),
            "ItemRarity::Rare 应触发 RefuseRare 拒绝门控，\
             期望: is_rarity_refused_at_low_rep(Rare) = true，实际: false"
        );
    }

    /// Low 信誉买 Common 物品（spirit_grass，rarity=Common）→ 应通过。
    /// 期望：is_rarity_refused_at_low_rep(Common) = false（走到 1.3x 加价路径）。
    #[test]
    fn common_rarity_allowed_for_low_rep_with_markup() {
        assert!(
            !is_rarity_refused_at_low_rep(ItemRarity::Common),
            "ItemRarity::Common 不应触发 RefuseRare 门控，\
             期望: is_rarity_refused_at_low_rep(Common) = false，实际: true"
        );
    }

    /// Low 信誉买 Uncommon 物品（skill_scroll_herbalism_baicao_can，rarity=Uncommon）→ 应通过。
    /// 这是 Rare 阈值 off-by-one 边界：Uncommon 在 Rare 之下，应允许（1.3x）。
    #[test]
    fn uncommon_rarity_is_allowed_off_by_one_boundary() {
        assert!(
            !is_rarity_refused_at_low_rep(ItemRarity::Uncommon),
            "ItemRarity::Uncommon 是 Rare 阈值 off-by-one 边界（低于 Rare），\
             期望: is_rarity_refused_at_low_rep(Uncommon) = false（允许 1.3x markup），实际: true"
        );
    }

    /// Epic/Legendary/Ancient 全部应被拒绝（Rare+ 全覆盖）。
    #[test]
    fn epic_legendary_ancient_all_refused() {
        assert!(
            is_rarity_refused_at_low_rep(ItemRarity::Epic),
            "ItemRarity::Epic 应触发 RefuseRare 门控，\
             期望: true，实际: false"
        );
        assert!(
            is_rarity_refused_at_low_rep(ItemRarity::Legendary),
            "ItemRarity::Legendary 应触发 RefuseRare 门控，\
             期望: true，实际: false"
        );
        assert!(
            is_rarity_refused_at_low_rep(ItemRarity::Ancient),
            "ItemRarity::Ancient 应触发 RefuseRare 门控，\
             期望: true，实际: false"
        );
    }

    /// High/Mid 信誉不触发 RefuseRare——check_trade_eligibility 返回 Allowed，
    /// 不走 RefuseRare arm，所以 rarity 门控根本不会执行。
    /// 此测试通过验证 TradeEligibility 确认逻辑路径分叉正确。
    #[test]
    fn high_mid_rep_not_refused_by_eligibility() {
        use crate::npc::trade::{check_trade_eligibility, RepTier, TradeEligibility};
        // High tier → Allowed（不走 RefuseRare arm）
        assert!(
            matches!(
                check_trade_eligibility(RepTier::High),
                TradeEligibility::Allowed { .. }
            ),
            "High 信誉不应走 RefuseRare arm，期望: Allowed，实际: 非 Allowed"
        );
        // Mid tier → Allowed（不走 RefuseRare arm）
        assert!(
            matches!(
                check_trade_eligibility(RepTier::Mid),
                TradeEligibility::Allowed { .. }
            ),
            "Mid 信誉不应走 RefuseRare arm，期望: Allowed，实际: 非 Allowed"
        );
    }

    /// Hostile 信誉触发 Refused（全拒），与 RefuseRare 是不同分支。
    #[test]
    fn hostile_rep_is_fully_refused_not_rare_gated() {
        use crate::npc::trade::{check_trade_eligibility, RepTier, TradeEligibility};
        assert_eq!(
            check_trade_eligibility(RepTier::Hostile),
            TradeEligibility::Refused,
            "Hostile 信誉应触发 Refused（全拒），期望: Refused，实际: 非 Refused"
        );
    }

    /// Low 信誉对应 RefuseRare 资格——买路 broken_artifact_scroll(Rare) 在此分支下应被拒绝。
    #[test]
    fn low_rep_eligibility_is_refuse_rare() {
        use crate::npc::trade::{check_trade_eligibility, RepTier, TradeEligibility};
        assert_eq!(
            check_trade_eligibility(RepTier::Low),
            TradeEligibility::RefuseRare,
            "Low 信誉应触发 RefuseRare，期望: RefuseRare，实际: 非 RefuseRare"
        );
    }

    /// 完整 RefuseRare 链路验证：Low rep + Rare 物品 → 被拒绝。
    /// 模拟 broken_artifact_scroll(Rare) 在 Low 声望下的完整判断链。
    #[test]
    fn full_refuse_rare_chain_rare_item_low_rep_refused() {
        use crate::npc::trade::{check_trade_eligibility, RepTier, TradeEligibility};
        let rep_tier = RepTier::Low; // score ∈ (0.1, 0.3]
        let eligibility = check_trade_eligibility(rep_tier);
        assert_eq!(
            eligibility,
            TradeEligibility::RefuseRare,
            "Low rep 应得到 RefuseRare 资格"
        );
        // Rare 物品：应触发拒绝
        let is_rare = is_rarity_refused_at_low_rep(ItemRarity::Rare);
        assert!(
            is_rare,
            "broken_artifact_scroll(Rare) 应触发 RefuseRare 拒绝门控，\
             期望: is_rare = true，实际: false"
        );
    }

    /// 完整 RefuseRare 链路验证：Low rep + Common 物品 → 通过（1.3x markup）。
    /// 模拟 spirit_grass(Common) 在 Low 声望下的完整判断链。
    #[test]
    fn full_refuse_rare_chain_common_item_low_rep_allowed() {
        use crate::npc::trade::{check_trade_eligibility, RepTier, TradeEligibility};
        let rep_tier = RepTier::Low;
        let eligibility = check_trade_eligibility(rep_tier);
        assert_eq!(
            eligibility,
            TradeEligibility::RefuseRare,
            "Low rep 应得到 RefuseRare 资格"
        );
        let is_rare = is_rarity_refused_at_low_rep(ItemRarity::Common);
        assert!(
            !is_rare,
            "spirit_grass(Common) 不应触发 RefuseRare 拒绝，\
             期望: is_rare = false（走 1.3x markup 路径），实际: true"
        );
        // 验证 1.3x 价格计算
        use crate::npc::trade::TradePricingConfig;
        let config = TradePricingConfig::default();
        let base_price = 10u64; // spirit_grass base price
        let final_price = (base_price as f64 * config.rep_low_markup as f64)
            .ceil()
            .max(1.0) as u64;
        assert_eq!(
            final_price, 13,
            "spirit_grass(10 骨币) 在 Low rep 1.3x markup 下应为 13 骨币，\
             期望: 13，实际: {}",
            final_price
        );
    }
}

// ─── plan-exploration-probe-return-v1 P1 — FreshnessProbe handler 测试 ───
#[cfg(test)]
mod freshness_probe_handler_tests {
    use super::*;
    use crate::inventory::{
        ContainerState, InventoryRevision, ItemInstance, ItemRarity, PlacedItemState,
    };
    use crate::lingtian::events::{
        StartDrainQiRequest, StartHarvestRequest, StartPlantingRequest, StartRenewRequest,
        StartReplenishRequest, StartTillRequest,
    };
    use valence::prelude::{ident, App, EventReader, IntoSystemConfigs, ResMut, Update};
    use valence::testing::create_mock_client;

    #[derive(Default)]
    struct CapturedFreshnessProbes(Vec<FreshnessProbeIntent>);
    impl valence::prelude::Resource for CapturedFreshnessProbes {}

    fn capture_freshness_probes(
        mut events: EventReader<FreshnessProbeIntent>,
        mut captured: ResMut<CapturedFreshnessProbes>,
    ) {
        captured.0.extend(events.read().cloned());
    }

    fn empty_inventory() -> PlayerInventory {
        PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(0),
            containers: vec![ContainerState {
                quick_access: false,
                id: "main_pack".into(),
                name: "main_pack".into(),
                rows: 5,
                cols: 7,
                items: Vec::new(),

                owner_instance_id: None,
            }],
            equipped: Default::default(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 50.0,
        }
    }

    fn inventory_with_item(item: ItemInstance) -> PlayerInventory {
        PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(0),
            containers: vec![ContainerState {
                quick_access: false,
                id: "main_pack".into(),
                name: "main_pack".into(),
                rows: 5,
                cols: 7,
                items: vec![PlacedItemState {
                    row: 0,
                    col: 0,
                    instance: item,
                }],

                owner_instance_id: None,
            }],
            equipped: Default::default(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 50.0,
        }
    }

    /// helper：为 FreshnessProbe 测试注册最小 app。
    /// 镜像 mineral_probe_request_emits_probe_intent 的 app 构造模式。
    fn setup_freshness_probe_app() -> (App, valence::prelude::Entity) {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        app.insert_resource(TechniqueRegistry::load_for_tests());
        app.insert_resource(CapturedFreshnessProbes::default());
        app.insert_resource(CombatClock { tick: 42 });
        app.insert_resource(GameplayActionQueue::default());
        app.insert_resource(AlchemyMockState::default());
        app.insert_resource(DroppedLootRegistry::default());
        // plan-remains-suite P0 — DroppedLootRequestParams 新增 EventWriter<RemainsLootIntent>。
        app.add_event::<crate::inventory::RemainsLootIntent>();
        app.insert_resource(ItemRegistry::default());
        app.insert_resource(RecipeRegistry::default());
        app.add_event::<CustomPayloadEvent>();
        app.add_event::<BreakthroughRequest>();
        app.add_event::<ForgeRequest>();
        app.add_event::<InsightChosen>();
        app.add_event::<DefenseIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<PlaceFurnaceRequest>();
        app.add_event::<crate::alchemy::LearnRecipeFragmentIntent>();
        app.add_event::<StartTillRequest>();
        app.add_event::<StartRenewRequest>();
        app.add_event::<StartPlantingRequest>();
        app.add_event::<StartHarvestRequest>();
        app.add_event::<StartReplenishRequest>();
        app.add_event::<StartDrainQiRequest>();
        app.add_event::<StartExtractRequestEvent>();
        app.add_event::<CancelExtractRequestEvent>();
        app.add_event::<MineralProbeIntent>();
        app.add_event::<FreshnessProbeIntent>();
        app.add_event::<SkillXpGain>();
        app.add_event::<SkillScrollUsed>();
        // plan-shield-block-v1 P1 — 举盾 intent events（ClientRequestDispatchParams 需要）。
        app.add_event::<crate::combat::shield_block::RaiseShieldIntent>();
        app.add_event::<crate::combat::shield_block::LowerShieldIntent>();
        app.add_event::<crate::network::agent_ui::AgentUiResponseEvent>();
        app.add_systems(
            Update,
            (handle_client_request_payloads, capture_freshness_probes).chain(),
        );
        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        (app, entity)
    }

    /// FreshnessProbe 请求：inventory 中存在 instance_id → emit FreshnessProbeIntent 正确字段。
    #[test]
    fn freshness_probe_request_emits_probe_intent() {
        let (mut app, entity) = setup_freshness_probe_app();
        let item = ItemInstance {
            instance_id: 7777,
            template_id: "xi_zhi_herb".to_string(),
            display_name: "细枝草".to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.1,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 0.8,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        };
        app.world_mut()
            .entity_mut(entity)
            .insert(inventory_with_item(item));
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"freshness_probe","v":1,"instance_id":7777}"#
                    .to_vec()
                    .into_boxed_slice(),
            });
        app.update();

        let captured = app.world().resource::<CapturedFreshnessProbes>();
        assert_eq!(
            captured.0.len(),
            1,
            "应 emit 1 个 FreshnessProbeIntent，实际 {}",
            captured.0.len()
        );
        assert_eq!(captured.0[0].player, entity, "player entity 应匹配");
        assert_eq!(
            captured.0[0].instance_id, 7777,
            "instance_id 应 round-trip 为 7777"
        );
        assert_eq!(
            captured.0[0].issued_at_tick, 42,
            "issued_at_tick 应等于 CombatClock.tick=42"
        );
    }

    /// FreshnessProbe 请求：instance_id 不在 inventory → 不 emit，不 panic。
    #[test]
    fn freshness_probe_request_not_found_does_not_emit() {
        let (mut app, entity) = setup_freshness_probe_app();
        // inventory 为空，instance_id=9999 不存在
        app.world_mut().entity_mut(entity).insert(empty_inventory());
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"freshness_probe","v":1,"instance_id":9999}"#
                    .to_vec()
                    .into_boxed_slice(),
            });
        app.update();

        let captured = app.world().resource::<CapturedFreshnessProbes>();
        assert!(
            captured.0.is_empty(),
            "instance_id 不存在时不应 emit FreshnessProbeIntent"
        );
    }

    /// FreshnessProbe 请求：instance_id 在非首容器中也能找到并 emit（多容器覆盖）。
    #[test]
    fn freshness_probe_request_finds_item_in_secondary_container() {
        let (mut app, entity) = setup_freshness_probe_app();
        // 构造含两个容器的 inventory，物品在第二个容器
        let item = ItemInstance {
            instance_id: 1234,
            template_id: "xi_zhi_herb".to_string(),
            display_name: "细枝草".to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.1,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 1.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        };
        let inv = PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(0),
            containers: vec![
                // 第一容器（空）
                ContainerState {
                    quick_access: false,
                    id: "main_pack".into(),
                    name: "main_pack".into(),
                    rows: 5,
                    cols: 7,
                    items: Vec::new(),

                    owner_instance_id: None,
                },
                // 第二容器持有目标物品
                ContainerState {
                    quick_access: false,
                    id: "side_pack".into(),
                    name: "side_pack".into(),
                    rows: 3,
                    cols: 4,
                    items: vec![PlacedItemState {
                        row: 1,
                        col: 2,
                        instance: item,
                    }],

                    owner_instance_id: None,
                },
            ],
            equipped: Default::default(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 50.0,
        };
        app.world_mut().entity_mut(entity).insert(inv);
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"freshness_probe","v":1,"instance_id":1234}"#
                    .to_vec()
                    .into_boxed_slice(),
            });
        app.update();

        let captured = app.world().resource::<CapturedFreshnessProbes>();
        assert_eq!(
            captured.0.len(),
            1,
            "第二容器中的物品也应能 emit FreshnessProbeIntent"
        );
        assert_eq!(
            captured.0[0].instance_id, 1234,
            "instance_id 应匹配第二容器物品"
        );
    }

    /// FreshnessProbe gate 扩展：instance_id 在 hotbar 中也应 emit（原 bug：只扫 containers）。
    #[test]
    fn freshness_probe_request_finds_item_in_hotbar() {
        let (mut app, entity) = setup_freshness_probe_app();
        let item = ItemInstance {
            instance_id: 5555,
            template_id: "zhi_xiang_cao".to_string(),
            display_name: "止香草".to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.05,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 0.6,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        };
        // 放进 hotbar slot 3（容器为空）
        let mut inv = empty_inventory();
        inv.hotbar[3] = Some(item);
        app.world_mut().entity_mut(entity).insert(inv);

        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"freshness_probe","v":1,"instance_id":5555}"#
                    .to_vec()
                    .into_boxed_slice(),
            });
        app.update();

        let captured = app.world().resource::<CapturedFreshnessProbes>();
        assert_eq!(
            captured.0.len(),
            1,
            "hotbar 中的物品也应能通过 gate 并 emit FreshnessProbeIntent（修复前只扫 containers 导致误拒）"
        );
        assert_eq!(
            captured.0[0].instance_id, 5555,
            "instance_id 应匹配 hotbar 物品"
        );
    }

    /// FreshnessProbe gate 扩展：instance_id 在 equipped 中也应 emit。
    #[test]
    fn freshness_probe_request_finds_item_in_equipped() {
        let (mut app, entity) = setup_freshness_probe_app();
        let item = ItemInstance {
            instance_id: 6666,
            template_id: "spirit_robe".to_string(),
            display_name: "灵袍".to_string(),
            grid_w: 2,
            grid_h: 3,
            weight: 1.5,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 0.9,
            durability: 0.8,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        };
        // 放进 equipped（模拟穿戴槽），容器与 hotbar 均为空
        let mut inv = empty_inventory();
        inv.equipped.insert(
            "chest".to_string(),
            crate::inventory::SlotContents::worn_single(item),
        );
        app.world_mut().entity_mut(entity).insert(inv);

        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"freshness_probe","v":1,"instance_id":6666}"#
                    .to_vec()
                    .into_boxed_slice(),
            });
        app.update();

        let captured = app.world().resource::<CapturedFreshnessProbes>();
        assert_eq!(
            captured.0.len(),
            1,
            "equipped 中的物品也应能通过 gate 并 emit FreshnessProbeIntent（修复前只扫 containers 导致误拒）"
        );
        assert_eq!(
            captured.0[0].instance_id, 6666,
            "instance_id 应匹配 equipped 物品"
        );
    }

    // ── plan-shield-block-v1 P1 e2e — 举盾 / 放盾全链路 ─────────────────────
    // 验证：JSON payload {"type":"raise_shield","v":1} → handle_client_request_payloads
    // 解析 → 投递 RaiseShieldIntent（client entity 匹配）；
    // 以及 lower_shield payload → LowerShieldIntent 投递。
    // 这是「客户端发 CustomPayload → server dispatch intent」的完整链路断言。

    #[derive(Default)]
    struct CapturedRaiseShieldIntents(Vec<crate::combat::shield_block::RaiseShieldIntent>);
    impl valence::prelude::Resource for CapturedRaiseShieldIntents {}

    #[derive(Default)]
    struct CapturedLowerShieldIntents(Vec<crate::combat::shield_block::LowerShieldIntent>);
    impl valence::prelude::Resource for CapturedLowerShieldIntents {}

    fn capture_raise_shield_intents(
        mut events: EventReader<crate::combat::shield_block::RaiseShieldIntent>,
        mut captured: ResMut<CapturedRaiseShieldIntents>,
    ) {
        captured.0.extend(events.read().cloned());
    }

    fn capture_lower_shield_intents(
        mut events: EventReader<crate::combat::shield_block::LowerShieldIntent>,
        mut captured: ResMut<CapturedLowerShieldIntents>,
    ) {
        captured.0.extend(events.read().cloned());
    }

    fn setup_shield_e2e_app() -> (App, valence::prelude::Entity) {
        let mut app = App::new();
        app.init_resource::<crate::lingtian::requests::PendingLingtianRequests>();
        app.init_resource::<ClientRequestBudget>();
        app.insert_resource(TechniqueRegistry::load_for_tests());
        app.insert_resource(CapturedRaiseShieldIntents::default());
        app.insert_resource(CapturedLowerShieldIntents::default());
        app.insert_resource(CombatClock::default());
        app.insert_resource(GameplayActionQueue::default());
        app.insert_resource(AlchemyMockState::default());
        app.insert_resource(DroppedLootRegistry::default());
        // plan-remains-suite P0 — DroppedLootRequestParams 新增 EventWriter<RemainsLootIntent>。
        app.add_event::<crate::inventory::RemainsLootIntent>();
        app.insert_resource(ItemRegistry::default());
        app.insert_resource(RecipeRegistry::default());
        app.add_event::<CustomPayloadEvent>();
        app.add_event::<BreakthroughRequest>();
        app.add_event::<ForgeRequest>();
        app.add_event::<InsightChosen>();
        app.add_event::<DefenseIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<PlaceFurnaceRequest>();
        app.add_event::<crate::alchemy::LearnRecipeFragmentIntent>();
        app.add_event::<StartTillRequest>();
        app.add_event::<StartRenewRequest>();
        app.add_event::<StartPlantingRequest>();
        app.add_event::<StartHarvestRequest>();
        app.add_event::<StartReplenishRequest>();
        app.add_event::<StartDrainQiRequest>();
        app.add_event::<StartExtractRequestEvent>();
        app.add_event::<CancelExtractRequestEvent>();
        app.add_event::<MineralProbeIntent>();
        app.add_event::<FreshnessProbeIntent>();
        app.add_event::<SkillXpGain>();
        app.add_event::<SkillScrollUsed>();
        app.add_event::<crate::combat::shield_block::RaiseShieldIntent>();
        app.add_event::<crate::combat::shield_block::LowerShieldIntent>();
        app.add_event::<crate::network::agent_ui::AgentUiResponseEvent>();
        app.add_systems(
            Update,
            (
                handle_client_request_payloads,
                capture_raise_shield_intents,
                capture_lower_shield_intents,
            )
                .chain(),
        );
        let (client_bundle, _helper) = create_mock_client("Shield");
        let entity = app.world_mut().spawn(client_bundle).id();
        (app, entity)
    }

    /// e2e：JSON {"type":"raise_shield","v":1} payload → RaiseShieldIntent(player=entity) 投递。
    /// 验证 client_request_handler 正确解析 raise_shield 并路由到 intent event。
    #[test]
    fn raise_shield_payload_dispatches_raise_shield_intent() {
        let (mut app, entity) = setup_shield_e2e_app();
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"raise_shield","v":1}"#.to_vec().into_boxed_slice(),
            });
        app.update();

        let captured = app.world().resource::<CapturedRaiseShieldIntents>();
        assert_eq!(
            captured.0.len(),
            1,
            "raise_shield payload 应 dispatch 恰好 1 个 RaiseShieldIntent，实际 {}",
            captured.0.len()
        );
        assert_eq!(
            captured.0[0].player, entity,
            "RaiseShieldIntent.player 应等于发送 payload 的 client entity"
        );
    }

    /// e2e：JSON {"type":"lower_shield","v":1} payload → LowerShieldIntent(player=entity) 投递。
    /// 验证松开右键边沿的 lower_shield 路由正确。
    #[test]
    fn lower_shield_payload_dispatches_lower_shield_intent() {
        let (mut app, entity) = setup_shield_e2e_app();
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"lower_shield","v":1}"#.to_vec().into_boxed_slice(),
            });
        app.update();

        let captured = app.world().resource::<CapturedLowerShieldIntents>();
        assert_eq!(
            captured.0.len(),
            1,
            "lower_shield payload 应 dispatch 恰好 1 个 LowerShieldIntent，实际 {}",
            captured.0.len()
        );
        assert_eq!(
            captured.0[0].player, entity,
            "LowerShieldIntent.player 应等于发送 payload 的 client entity"
        );
    }

    /// e2e：raise 后接 lower → 两个 intent 均投递，顺序正确。
    #[test]
    fn raise_then_lower_shield_payload_dispatches_both_intents_in_order() {
        let (mut app, entity) = setup_shield_e2e_app();

        // Raise
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"raise_shield","v":1}"#.to_vec().into_boxed_slice(),
            });
        app.update();

        // Lower
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"lower_shield","v":1}"#.to_vec().into_boxed_slice(),
            });
        app.update();

        let raised = app.world().resource::<CapturedRaiseShieldIntents>();
        let lowered = app.world().resource::<CapturedLowerShieldIntents>();
        assert_eq!(
            raised.0.len(),
            1,
            "raise 后应有 1 个 RaiseShieldIntent，实际 {}",
            raised.0.len()
        );
        assert_eq!(
            lowered.0.len(),
            1,
            "lower 后应有 1 个 LowerShieldIntent，实际 {}",
            lowered.0.len()
        );
    }

    /// plan-shield-block-v1 P1 CR#4 — 同 tick 内同时发送 raise + lower 两个 payload，
    /// 断言两个 intent 在同一 update() 内均被 dispatch（区别于 raise_then_lower 使用两次 update）。
    #[test]
    fn raise_and_lower_same_tick_dispatches_both_intents() {
        let (mut app, entity) = setup_shield_e2e_app();

        // 在同一 update 前发送 raise + lower 两个 CustomPayloadEvent
        let mut events = app
            .world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>();
        events.send(CustomPayloadEvent {
            client: entity,
            channel: ident!("bong:client_request").into(),
            data: br#"{"type":"raise_shield","v":1}"#.to_vec().into_boxed_slice(),
        });
        events.send(CustomPayloadEvent {
            client: entity,
            channel: ident!("bong:client_request").into(),
            data: br#"{"type":"lower_shield","v":1}"#.to_vec().into_boxed_slice(),
        });

        // 单次 update —— 两个 payload 在同一 tick 内被 handle_client_request_payloads 处理
        app.update();

        let raised = app.world().resource::<CapturedRaiseShieldIntents>();
        let lowered = app.world().resource::<CapturedLowerShieldIntents>();
        assert_eq!(
            raised.0.len(),
            1,
            "同 tick raise+lower：应有 1 个 RaiseShieldIntent，实际 {}; \
             期望 handle_client_request_payloads 在单次 update 内 dispatch raise+lower 两个 intent",
            raised.0.len()
        );
        assert_eq!(
            lowered.0.len(),
            1,
            "同 tick raise+lower：应有 1 个 LowerShieldIntent，实际 {}; \
             期望 handle_client_request_payloads 在单次 update 内 dispatch raise+lower 两个 intent",
            lowered.0.len()
        );
        assert_eq!(
            raised.0[0].player, entity,
            "RaiseShieldIntent.player 应等于发送 payload 的 client entity，同 tick 场景"
        );
        assert_eq!(
            lowered.0[0].player, entity,
            "LowerShieldIntent.player 应等于发送 payload 的 client entity，同 tick 场景"
        );
    }

    /// plan-shield-block-v1 P1 CR#4 — 协议错误分支：v!=1 的 raise_shield payload 被版本校验拒绝，不 dispatch intent。
    #[test]
    fn raise_shield_bad_version_is_not_dispatched() {
        let (mut app, entity) = setup_shield_e2e_app();

        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                // v:2 应被 SUPPORTED_VERSION 校验拒绝（warn + continue，不 dispatch）
                data: br#"{"type":"raise_shield","v":2}"#.to_vec().into_boxed_slice(),
            });
        app.update();

        let captured = app.world().resource::<CapturedRaiseShieldIntents>();
        assert_eq!(
            captured.0.len(),
            0,
            "raise_shield with v:2 must not dispatch RaiseShieldIntent \
             because SUPPORTED_VERSION check rejects unsupported protocol versions; \
             actual intent count={}",
            captured.0.len()
        );
    }

    /// plan-shield-block-v1 P1 CR#4 — 协议错误分支：malformed JSON 不 dispatch 任何 intent。
    #[test]
    fn raise_shield_malformed_json_is_not_dispatched() {
        let (mut app, entity) = setup_shield_e2e_app();

        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"not valid json"#.to_vec().into_boxed_slice(),
            });
        app.update();

        let captured = app.world().resource::<CapturedRaiseShieldIntents>();
        assert_eq!(
            captured.0.len(),
            0,
            "malformed JSON payload must not dispatch any RaiseShieldIntent; \
             actual intent count={}",
            captured.0.len()
        );
    }
}

// ── skill_bar ownership gate — player_knows_technique unit tests ────────────
// Locks the gate that blocks SkillBarBind/Cast for techniques not in KnownTechniques.
// Tests the pure helper function directly; no ECS required.
#[cfg(test)]
mod skill_bar_ownership_gate_tests {
    use super::*;
    use crate::cultivation::known_techniques::{KnownTechnique, KnownTechniques};

    fn make_known(entries: &[(&str, bool)]) -> KnownTechniques {
        KnownTechniques {
            entries: entries
                .iter()
                .map(|(id, active)| KnownTechnique {
                    id: (*id).to_string(),
                    proficiency: 0.5,
                    active: *active,
                })
                .collect(),
        }
    }

    /// Happy path: technique is present and active → gate passes.
    #[test]
    fn active_technique_is_known() {
        let kt = make_known(&[("sword.cleave", true)]);
        assert!(
            player_knows_technique(&kt, "sword.cleave"),
            "player_knows_technique must return true when technique is present and active; \
             entries={:?}",
            kt.entries
        );
    }

    /// Inactive technique in list → gate rejects (inactive = not in use / suspended).
    #[test]
    fn inactive_technique_is_not_known() {
        let kt = make_known(&[("sword.cleave", false)]);
        assert!(
            !player_knows_technique(&kt, "sword.cleave"),
            "player_knows_technique must return false when technique.active=false; \
             entries={:?}",
            kt.entries
        );
    }

    /// Technique not in list at all → gate rejects.
    #[test]
    fn absent_technique_is_not_known() {
        let kt = make_known(&[("sword.cleave", true)]);
        assert!(
            !player_knows_technique(&kt, "baomai.full_power_charge"),
            "player_knows_technique must return false when technique is absent from entries; \
             entries={:?}",
            kt.entries
        );
    }

    /// Empty KnownTechniques → gate rejects all techniques.
    #[test]
    fn empty_known_techniques_rejects_all() {
        let kt = KnownTechniques { entries: vec![] };
        assert!(
            !player_knows_technique(&kt, "sword.cleave"),
            "player_knows_technique must return false when KnownTechniques.entries is empty"
        );
        assert!(
            !player_knows_technique(&kt, "baomai.full_power_charge"),
            "player_knows_technique must return false for any technique when entries is empty"
        );
    }

    /// Multiple techniques, the target one active → gate passes.
    #[test]
    fn active_among_many_is_known() {
        let kt = make_known(&[
            ("sword.cleave", true),
            ("baomai.full_power_charge", true),
            ("burst_meridian.ni_mai_hu_ti", false),
        ]);
        assert!(
            player_knows_technique(&kt, "baomai.full_power_charge"),
            "player_knows_technique must return true for the active target technique \
             even when other techniques are also present; entries={:?}",
            kt.entries
        );
    }

    /// Multiple techniques, the target one inactive while others are active → gate rejects.
    #[test]
    fn inactive_among_active_siblings_is_not_known() {
        let kt = make_known(&[
            ("sword.cleave", true),
            ("baomai.full_power_charge", false),
            ("movement.dash", true),
        ]);
        assert!(
            !player_knows_technique(&kt, "baomai.full_power_charge"),
            "player_knows_technique must return false for inactive technique \
             even when other active techniques exist; entries={:?}",
            kt.entries
        );
    }

    /// The dangerous real-world case from the bug report: baomai.full_power_charge with
    /// empty required_meridians should be blocked at the ownership gate when not learned.
    #[test]
    fn baomai_full_power_charge_blocked_when_not_learned() {
        // Player has only basic sword techniques — has NOT learned baomai.
        let kt = make_known(&[("sword.cleave", true), ("sword.thrust", true)]);
        assert!(
            !player_knows_technique(&kt, "baomai.full_power_charge"),
            "An Awaken-realm player without baomai in KnownTechniques must be blocked \
             from casting baomai.full_power_charge (no meridian gate exists for this technique); \
             entries={:?}",
            kt.entries
        );
    }

    /// Gate passes for the ni_mai_hu_ti case from the bug report when the player has it.
    #[test]
    fn ni_mai_hu_ti_passes_when_learned() {
        let kt = make_known(&[("burst_meridian.ni_mai_hu_ti", true)]);
        assert!(
            player_knows_technique(&kt, "burst_meridian.ni_mai_hu_ti"),
            "player_knows_technique must return true for ni_mai_hu_ti when learned and active"
        );
    }

    /// Gate rejects ni_mai_hu_ti when not learned (original exploit path from bug report).
    #[test]
    fn ni_mai_hu_ti_blocked_when_not_learned() {
        let kt = make_known(&[("sword.cleave", true)]);
        assert!(
            !player_knows_technique(&kt, "burst_meridian.ni_mai_hu_ti"),
            "An Awaken-realm player without ni_mai_hu_ti in KnownTechniques must not be \
             able to bind or cast it, even though technique_definition lookup would succeed; \
             entries={:?}",
            kt.entries
        );
    }
}
