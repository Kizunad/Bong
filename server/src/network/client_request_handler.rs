//! 客户端 → 服务端 `bong:client_request` 通道处理（plan-cultivation-v1 §P1 剩余）。
//!
//! Fabric 客户端通过 Minecraft CustomPayload 发送 `ClientRequestV1` JSON；
//! 本系统读取 Valence `CustomPayloadEvent`，按 channel 过滤 → 反序列化
//! → 发射对应 Bevy 事件：
//!   - SetMeridianTarget → 插入/更新 `MeridianTarget` Component
//!   - BreakthroughRequest → emit `BreakthroughRequest` Bevy event
//!   - ForgeRequest → emit `ForgeRequest` Bevy event

use std::collections::HashMap;

use bevy_ecs::system::SystemParam;
use valence::custom_payload::CustomPayloadEvent;
use valence::message::SendMessage;
use valence::prelude::{
    bevy_ecs, ChunkLayer, Client, Commands, DVec3, Entity, EntityManager, EventReader, EventWriter,
    Events, Query, Res, ResMut, Resource, UniqueId, Username, With,
};

use crate::alchemy::residue::{residue_alchemy_data, residue_kind_for_recyclable_outcome};
use crate::alchemy::{
    learned::LearnResult, AlchemyFurnace, AlchemySession, Intervention, LearnedRecipes,
    PlaceFurnaceRequest, RecipeRegistry, MIN_ZONE_QI_TO_ALCHEMY,
};
use crate::coffin::{CoffinEnterRequest, CoffinLeaveRequest, CoffinPlaceRequest};
use crate::combat::anqi_v2::{cycle_container_slot, switch_container_slot};
use crate::combat::carrier::{CarrierSlot, ChargeCarrierIntent, ThrowCarrierIntent};
use crate::combat::components::{
    CastSource, Casting, Lifecycle, LifecycleState, QuickSlotBindings, SkillBarBindings, SkillSlot,
    Stamina, Wounds,
};
use crate::combat::events::{
    ApplyStatusEffectIntent, DefenseIntent, RevivalActionIntent, RevivalActionKind,
    StatusEffectKind,
};
use crate::combat::foreign_qi_resistance::foreign_qi_resistance_for_use;
use crate::combat::needle::IntentSource;
use crate::combat::tuike::{can_equip_false_skin, false_skin_kind_for_item, FalseSkinForgeRequest};
use crate::combat::CombatClock;
use crate::cultivation::breakthrough::BreakthroughRequest;
use crate::cultivation::components::{recover_current_qi, Cultivation, MeridianId, MeridianSystem};
use crate::cultivation::dugu::SelfAntidoteIntent;
use crate::cultivation::forging::ForgeRequest;
use crate::cultivation::insight::{InsightChosen, InsightRequest};
use crate::cultivation::known_techniques::{
    technique_definition, KnownTechniques, TechniqueDefinition,
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
use crate::forge::blueprint::TemperBeat;
use crate::forge::events::{
    ConsecrationInject, InscriptionScrollSubmit, StepAdvance, TemperingHit,
};
use crate::forge::learned::LearnedBlueprints;
use crate::forge::session::{ForgeSessionId, ForgeSessions, ForgeStep};
use crate::forge::station::PlaceForgeStationRequest;
use crate::inventory::{
    add_item_to_player_inventory, add_item_to_player_inventory_with_alchemy, apply_inventory_move,
    apply_item_spiritual_wear, consume_item_instance_once, discard_inventory_item_to_dropped_loot,
    fully_repair_weapon_instance, inventory_instance_container_attrition_exempt,
    inventory_item_by_instance_borrow, inventory_item_by_instance_mut,
    inventory_location_attrition_exempt, pickup_dropped_loot_instance, DroppedLootRegistry,
    InventoryDurabilityChangedEvent, InventoryInstanceIdAllocator, InventoryMoveOutcome,
    InventoryMoveRejectReason, ItemInstance, ItemTemplate, PlayerInventory,
};
use crate::inventory::{
    AlchemyItemData, ItemEffect, ItemRegistry,
    DEFAULT_CAST_DURATION_MS as TEMPLATE_DEFAULT_CAST_MS,
    DEFAULT_COOLDOWN_MS as TEMPLATE_DEFAULT_COOLDOWN_MS,
};
use crate::lingtian::environment::read_environment_at;
use crate::lingtian::events::{
    StartDrainQiRequest, StartHarvestRequest, StartPlantingRequest, StartRenewRequest,
    StartReplenishRequest, StartTillRequest,
};
use crate::lingtian::session::{ReplenishSource, SessionMode};
use crate::lingtian::terrain::{terrain_from_block_kind, TerrainKind};
use crate::lingtian::PlotEnvironment;
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
use crate::shelflife::probe::FreshnessProbeIntent;
// dropped_loot_sync is emitted by dropped_loot_sync_emit.
use crate::combat::shield_block::{LowerShieldIntent, RaiseShieldIntent};
use crate::identity::PlayerIdentities;
use crate::network::inventory_move_rejected_emit::emit_inventory_move_rejected;
use crate::network::inventory_snapshot_emit::send_inventory_snapshot_to_client;
use crate::network::npc_metadata::{
    display_name as npc_display_name, greeting_text_for_archetype,
    reputation_to_player_score_for_client,
};
use crate::network::qi_attrition_emit::{
    emit_attrition_applied_if_lost, item_abs_qi_for_attrition, AttritionAppliedEvent,
};
use crate::network::qi_color_observed_emit::QiColorInspectRequest;
use crate::network::send_server_data_payload;
use crate::network::skill_config_emit::send_skill_config_snapshot_to_client;
use crate::network::skill_snapshot_emit::send_skill_snapshot_to_client;
use crate::network::techniques_snapshot_emit::send_techniques_snapshot_to_client;
use crate::network::{
    gameplay_vfx, redis_bridge::RedisOutbound, vfx_event_emit::VfxEventRequest, RedisBridgeResource,
};
use crate::npc::faction::FactionMembership;
use crate::npc::interaction_memory::{
    record_player_npc_interaction, NpcInteractionOutcome, NpcInteractionType,
};
use crate::npc::lifecycle::NpcArchetype;
use crate::npc::spawn::NpcMarker;
use crate::npc::trade::NpcPlayerReputation;
use crate::player::gameplay::{GameplayAction, GameplayActionQueue, GatherAction};
use crate::player::state::{
    canonical_player_id, update_player_ui_prefs, PlayerState, PlayerStatePersistence,
};
use crate::qi_physics::attrition::{apply_attrition_checked, is_attrition_exempt};
use crate::qi_physics::constants::QI_TARGETED_ITEM_WEAR_WEIGHT_THRESHOLD;
use crate::qi_physics::ledger::AttritionOpKind;
use crate::qi_physics::qi_targeted_item_wear_fraction;
use crate::qi_physics::AnqiContainerKind;
use crate::schema::alchemy::{AlchemyInterventionResultV1, AlchemySessionStartV1};
use crate::schema::client_request::{ClientRequestV1, SkillBarBindingV1};
use crate::schema::combat_hud::{CastOutcomeV1, CastPhaseV1, CastSyncV1};
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
use crate::social::components::{faction_for_zone, FactionReputation, FactionReputationTier};
use crate::social::events::{
    SparringInviteResponseEvent, SparringInviteResponseKind, SpiritNicheActivateGuardianRequest,
    SpiritNicheCoordinateRevealRequest, SpiritNichePlaceRequest, SpiritNicheRepairRequest,
    SpiritNicheRevealSource, TradeOfferRequest, TradeOfferResponseEvent,
};
use crate::world::block_place::BlockPlaceRequest;
use crate::world::dimension::{CurrentDimension, DimensionKind};
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
use crate::zhenfa::{
    ScatterBeadUseRequest, ZhenfaDisarmRequest, ZhenfaPlaceRequest, ZhenfaTriggerRequest,
};

/// RefuseRare arm 中对 rarity 的门控判断。
///
/// 返回 `true` 表示该 rarity 属于 Rare+（Rare/Epic/Legendary/Ancient），
/// 低信誉玩家购买此类物品时将被拒绝。
/// Common/Uncommon 返回 `false`，允许以 1.3x 加价购买。
///
/// NOTE: `ItemRarity` 未实现 `PartialOrd`，使用 `matches!` 枚举变体。
/// 如需新增更高 rarity 变体，必须同步更新此处。
pub(crate) fn is_rarity_refused_at_low_rep(r: crate::inventory::ItemRarity) -> bool {
    matches!(
        r,
        crate::inventory::ItemRarity::Rare
            | crate::inventory::ItemRarity::Epic
            | crate::inventory::ItemRarity::Legendary
            | crate::inventory::ItemRarity::Ancient
    )
}

/// per-client alchemy mock 状态，让 client→server 操作（翻页/学方）有可观察的回响。
/// 真实数据流（ECS 接入后）会替换掉本 resource。
#[derive(Default, Resource, Debug)]
pub struct AlchemyMockState {
    /// player_id → current recipe-book index
    pub recipe_index: HashMap<String, i32>,
}

/// 把 cast / quickslot 相关查询打包，避免 `handle_client_request_payloads`
/// 顶部参数 tuple 超出 Bevy 0.14 SystemParam 16-tuple 上限。
#[derive(SystemParam)]
pub struct CombatRequestParams<'w, 's> {
    pub casting_q: Query<'w, 's, &'static Casting>,
    pub bindings_q: Query<'w, 's, &'static mut QuickSlotBindings>,
    pub skillbar_bindings_q: Query<'w, 's, &'static mut SkillBarBindings>,
    pub positions: Query<'w, 's, &'static valence::prelude::Position>,
    pub unique_ids: Query<'w, 's, &'static UniqueId>,
    pub skill_registry: Option<Res<'w, SkillRegistry>>,
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
}

#[derive(SystemParam)]
pub struct DroppedLootRequestParams<'w, 's> {
    pub registry: ResMut<'w, DroppedLootRegistry>,
    pub positions: Query<'w, 's, &'static valence::prelude::Position>,
}

/// plan-lingtian-v1 §1.2-§1.7 — 6 类 intent 共享 EventWriter 包，避开
/// SystemParam 16 上限。`layers` 用于 `StartTill` 时读 chunk 派生真实
/// `TerrainKind` + `PlotEnvironment`，避免客户端伪造地形。
#[derive(SystemParam)]
pub struct LingtianRequestParams<'w, 's> {
    pub till_tx: EventWriter<'w, StartTillRequest>,
    pub renew_tx: EventWriter<'w, StartRenewRequest>,
    pub planting_tx: EventWriter<'w, StartPlantingRequest>,
    pub harvest_tx: EventWriter<'w, StartHarvestRequest>,
    pub replenish_tx: EventWriter<'w, StartReplenishRequest>,
    pub drain_qi_tx: EventWriter<'w, StartDrainQiRequest>,
    pub layers: Query<'w, 's, &'static ChunkLayer, With<crate::world::dimension::OverworldLayer>>,
}

/// 合并 alchemy 相关 Resource/Query，避开 `handle_client_request_payloads`
/// 顶部参数的 16-tuple Bevy 0.14 SystemParam 上限。
#[derive(SystemParam)]
pub struct AlchemyRequestParams<'w, 's> {
    pub state: ResMut<'w, AlchemyMockState>,
    pub furnaces: Query<'w, 's, (Entity, &'static mut AlchemyFurnace)>,
    pub learned: Query<'w, 's, &'static mut LearnedRecipes>,
    pub recipe_registry: Res<'w, RecipeRegistry>,
    pub learn_fragment_tx: EventWriter<'w, crate::alchemy::LearnRecipeFragmentIntent>,
    pub place_furnace_tx: EventWriter<'w, PlaceFurnaceRequest>,
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
    pub gameplay_queue: Option<valence::prelude::ResMut<'w, GameplayActionQueue>>,
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
    pub revival_tx: Option<ResMut<'w, Events<RevivalActionIntent>>>,
    pub place_forge_station_tx: Option<ResMut<'w, Events<PlaceForgeStationRequest>>>,
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
    pub sparring_invite_response_tx: Option<ResMut<'w, Events<SparringInviteResponseEvent>>>,
    pub trade_offer_request_tx: Option<ResMut<'w, Events<TradeOfferRequest>>>,
    pub trade_offer_response_tx: Option<ResMut<'w, Events<TradeOfferResponseEvent>>>,
    pub block_place_tx: Option<ResMut<'w, Events<BlockPlaceRequest>>>,
    /// plan-worldgen-v4 P5 §8.1#5 — 画廊 dev-only give-block intent。
    pub block_picker_give_tx:
        Option<ResMut<'w, Events<crate::cmd::dev::block_picker::BlockPickerGiveIntent>>>,
    pub zhenfa_place_tx: Option<ResMut<'w, Events<ZhenfaPlaceRequest>>>,
    pub zhenfa_trigger_tx: Option<ResMut<'w, Events<ZhenfaTriggerRequest>>>,
    pub zhenfa_disarm_tx: Option<ResMut<'w, Events<ZhenfaDisarmRequest>>>,
    pub qi_scatter_bead_use_tx: Option<ResMut<'w, Events<ScatterBeadUseRequest>>>,
    pub charge_carrier_tx: Option<ResMut<'w, Events<ChargeCarrierIntent>>>,
    pub throw_carrier_tx: Option<ResMut<'w, Events<ThrowCarrierIntent>>>,
    // ─── plan-craft-v1 P2：通用手搓 intent ──────────────────
    pub craft_start_tx: Option<ResMut<'w, Events<crate::craft::CraftStartIntent>>>,
    pub craft_cancel_tx: Option<ResMut<'w, Events<crate::craft::CraftCancelIntent>>>,
    // ─── plan-supply-coffin-loot-ui P2：外部容器 + entity-based open ──────
    pub ext_container_registry:
        Option<ResMut<'w, crate::inventory::external_container::ExternalContainerRegistry>>,
    pub supply_coffin_open_tx:
        Option<ResMut<'w, Events<crate::supply_coffin::interact::SupplyCoffinOpenRequest>>>,
    pub container_open_tx:
        Option<ResMut<'w, Events<crate::world::container_open::ContainerOpenRequest>>>,
    // ─── plan-dying-elder-v1 P1：垂死大能给丹 C2S ──────────────────
    pub give_dan_to_elder_tx:
        Option<ResMut<'w, Events<crate::fauna::dying_elder::GiveDanToElderIntent>>>,
    pub workbench_open_tx: Option<ResMut<'w, Events<crate::craft::WorkbenchOpenRequest>>>,
    // ─── plan-shield-block-v1 P1：持续举盾 intent ─────────────────────────
    pub raise_shield_tx: EventWriter<'w, RaiseShieldIntent>,
    pub lower_shield_tx: EventWriter<'w, LowerShieldIntent>,
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
}

type NpcEngagementItem = (
    &'static valence::prelude::Position,
    &'static NpcArchetype,
    Option<&'static FactionMembership>,
    Option<&'static Cultivation>,
    Option<&'static Lifecycle>,
    // plan-territory-v1 P1: per-NPC per-player 信誉度（霸主驻守加成写入此组件，
    // 这里读取后叠加到 faction baseline，让 dominance rep 真正影响交易价格）。
    Option<&'static NpcPlayerReputation>,
);

#[derive(SystemParam)]
pub struct NpcEngagementRequestParams<'w, 's> {
    pub npcs: Query<'w, 's, NpcEngagementItem, With<NpcMarker>>,
    pub lifecycles: Query<'w, 's, &'static Lifecycle>,
    pub memories: Query<
        'w,
        's,
        &'static mut crate::npc::interaction_memory::NpcMemoryComponent,
        With<NpcMarker>,
    >,
    pub positions: Query<'w, 's, &'static valence::prelude::Position>,
    pub dimensions: Query<'w, 's, &'static CurrentDimension>,
    pub identities: Query<'w, 's, &'static PlayerIdentities, With<Client>>,
    pub faction_reputations: Query<'w, 's, &'static FactionReputation, With<Client>>,
    pub audio_events: Option<ResMut<'w, Events<PlaySoundRecipeRequest>>>,
}

const CHANNEL: &str = "bong:client_request";
const SUPPORTED_VERSION: u8 = 1;
const QI_COLOR_INSPECT_MAX_DISTANCE: f64 = 6.0;
const NPC_INTERACTION_MAX_DISTANCE: f64 = 6.0;
/// plan-cultivation-v1 §3.1：服用突破辅助丹药的 buff 持续时间（5 分钟）。
/// 20 tick/s × 60 s × 5 = 6000。
const BREAKTHROUGH_BOOST_DURATION_TICKS: u64 = 6_000;

/// plan-scroll-reading-v1 P0/P2：阅读残卷循环姿态动画 priority——"中低"档位，
/// 低于战斗层（`COMBAT_PRIORITY`=1000）、高于仪式套路层（`GUANGBO_TICAO_PRIORITY`=500）。
/// 合法区间 [`VFX_ANIM_PRIORITY_MIN`, `VFX_ANIM_PRIORITY_MAX`] = [100, 3999]。
const SCROLL_READ_ANIM_PRIORITY: u16 = 600;
/// 淡入 tick 数（§8.1 #4 决议：fadeIn 4 tick）。
const SCROLL_READ_ANIM_FADE_IN_TICKS: u8 = 4;

/// plan-scroll-reading-v1 P2 — 展开微光 VFX `bong:scroll_open_glow`，淡金色，
/// burst 12 粒（client `ScrollOpenGlowPlayer` 再叠加自身的 continuous 层，本端只发一次
/// SpawnParticle，两层视觉由 client 侧固定常量生成，不经 payload 传递）。
const SCROLL_OPEN_GLOW_EVENT_ID: &str = "bong:scroll_open_glow";
const SCROLL_OPEN_GLOW_COLOR: &str = "#E8D9A0";
const SCROLL_OPEN_GLOW_COUNT: u16 = 12;
const SCROLL_OPEN_GLOW_STRENGTH: f32 = 0.85;
const SCROLL_OPEN_GLOW_DURATION_TICKS: u16 = 20;

fn meridian_label(id: MeridianId) -> &'static str {
    match id {
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

#[allow(clippy::too_many_arguments)] // Bevy system signature; one resource/query per gameplay area.
pub fn handle_client_request_payloads(
    mut events: EventReader<CustomPayloadEvent>,
    mut dispatch: ClientRequestDispatchParams,
    combat_clock: Res<CombatClock>,
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
    for ev in events.read() {
        if ev.channel.as_str() != CHANNEL {
            continue;
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

        let request: ClientRequestV1 = match serde_json::from_str(payload) {
            Ok(r) => r,
            Err(err) => {
                tracing::warn!(
                    "[bong][network] client_request deserialize failed from {:?}: {err}; body={payload}",
                    ev.client
                );
                continue;
            }
        };
        // 调试：每条 intent 都 log 一行，帮助诊断 client 到 server 通路。
        tracing::info!(
            "[bong][network] client_request received entity={:?} body={payload}",
            ev.client
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
                "[bong][network] client_request unsupported version v={v} from {:?}; body={payload}",
                ev.client
            );
            continue;
        }

        match request {
            ClientRequestV1::SetMeridianTarget { meridian, .. } => {
                tracing::info!(
                    "[bong][network] client_request set_meridian_target entity={:?} meridian={:?}",
                    ev.client,
                    meridian
                );
                commands.entity(ev.client).insert(MeridianTarget(meridian));
                if let Ok((_username, mut client)) = clients.get_mut(ev.client) {
                    client.send_chat_message(format!(
                        "§a[修炼] 已收到经脉目标：{}。",
                        meridian_label(meridian)
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
                        requested_at_tick: combat_clock.tick,
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
                    requested_at_tick: combat_clock.tick,
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
                let Some(queue) = dispatch.gameplay_queue.as_deref_mut() else {
                    tracing::warn!(
                        "[bong][network] dropped botany_harvest_request because GameplayActionQueue is missing"
                    );
                    continue;
                };
                let player_key = clients
                    .get(ev.client)
                    .map(|(username, _)| canonical_player_id(username.0.as_str()))
                    .unwrap_or_else(|_| format!("offline:{:?}", ev.client));
                queue.enqueue(
                    player_key,
                    GameplayAction::Gather(GatherAction {
                        resource: session_id,
                        target_entity: None,
                        mode: Some(match mode {
                            crate::schema::botany::BotanyHarvestModeV1::Manual => {
                                crate::botany::components::BotanyHarvestMode::Manual
                            }
                            crate::schema::botany::BotanyHarvestModeV1::Auto => {
                                crate::botany::components::BotanyHarvestMode::Auto
                            }
                        }),
                    }),
                );
            }
            // ── 炼丹请求 ECS dispatch (plan-alchemy-v1 §4) ──────────────────
            ClientRequestV1::AlchemyTurnPage { delta, .. } => {
                handle_alchemy_turn_page(
                    ev.client,
                    delta,
                    &mut clients,
                    &mut alchemy_params.learned,
                    &mut alchemy_params.state,
                );
            }
            ClientRequestV1::AlchemyLearnRecipe { recipe_id, .. } => {
                handle_alchemy_learn(
                    ev.client,
                    recipe_id,
                    &mut clients,
                    &mut alchemy_params.learned,
                    &alchemy_params.recipe_registry,
                );
            }
            ClientRequestV1::AlchemyLearnRecipeFragment {
                item_instance_id, ..
            } => {
                tracing::info!(
                    "[bong][network][alchemy] learn_recipe_fragment entity={:?} item_instance_id={item_instance_id}",
                    ev.client
                );
                alchemy_params
                    .learn_fragment_tx
                    .send(crate::alchemy::LearnRecipeFragmentIntent {
                        player: ev.client,
                        item_instance_id,
                    });
            }
            ClientRequestV1::AlchemyIntervention {
                furnace_pos,
                intervention,
                ..
            } => {
                handle_alchemy_intervention(
                    ev.client,
                    furnace_pos,
                    intervention.into(),
                    &mut clients,
                    &mut alchemy_params.furnaces,
                    alchemy_params.zones.as_deref(),
                    alchemy_params.redis.as_deref(),
                    alchemy_params.vfx_events.as_deref_mut(),
                );
            }
            ClientRequestV1::AlchemyOpenFurnace { furnace_pos, .. } => {
                handle_alchemy_open_furnace(
                    ev.client,
                    furnace_pos,
                    &mut clients,
                    &mut alchemy_params.furnaces,
                    &mut alchemy_params.learned,
                );
            }
            ClientRequestV1::AlchemyTakePill { pill_item_id, .. } => {
                handle_alchemy_take_pill(
                    ev.client,
                    &pill_item_id,
                    None,
                    &mut commands,
                    &combat_clock,
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
            ClientRequestV1::AlchemyFurnacePlace {
                x,
                y,
                z,
                item_instance_id,
                ..
            } => {
                let pos = valence::prelude::BlockPos::new(x, y, z);
                tracing::info!(
                    "[bong][network][alchemy] furnace_place entity={:?} pos=[{x},{y},{z}] instance={item_instance_id}",
                    ev.client
                );
                alchemy_params.place_furnace_tx.send(PlaceFurnaceRequest {
                    player: ev.client,
                    pos,
                    item_instance_id,
                });
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
            ClientRequestV1::SparringInviteResponse {
                invite_id,
                accepted,
                timed_out,
                ..
            } => {
                let Some(response_tx) = dispatch.sparring_invite_response_tx.as_deref_mut() else {
                    tracing::warn!(
                        "[bong][network] dropped sparring_invite_response because SparringInviteResponseEvent resource is missing"
                    );
                    continue;
                };
                let kind = if timed_out {
                    SparringInviteResponseKind::Timeout
                } else if accepted {
                    SparringInviteResponseKind::Accept
                } else {
                    SparringInviteResponseKind::Decline
                };
                response_tx.send(SparringInviteResponseEvent {
                    player: ev.client,
                    invite_id,
                    kind,
                    tick: combat_clock.tick,
                });
            }
            ClientRequestV1::TradeOfferRequest {
                target,
                offered_instance_id,
                ..
            } => {
                let Some(request_tx) = dispatch.trade_offer_request_tx.as_deref_mut() else {
                    tracing::warn!(
                        "[bong][network] dropped trade_offer_request because TradeOfferRequest event resource is missing"
                    );
                    continue;
                };
                let Some(target_entity) =
                    resolve_trade_offer_target(target.as_str(), &combat_params)
                else {
                    tracing::warn!(
                        "[bong][network] rejected trade_offer_request from {:?}: invalid target `{target}`",
                        ev.client
                    );
                    continue;
                };
                request_tx.send(TradeOfferRequest {
                    initiator: ev.client,
                    target: target_entity,
                    offered_instance_id,
                    tick: combat_clock.tick,
                });
            }
            ClientRequestV1::TradeOfferResponse {
                offer_id,
                accepted,
                requested_instance_id,
                ..
            } => {
                let Some(response_tx) = dispatch.trade_offer_response_tx.as_deref_mut() else {
                    tracing::warn!(
                        "[bong][network] dropped trade_offer_response because TradeOfferResponseEvent resource is missing"
                    );
                    continue;
                };
                response_tx.send(TradeOfferResponseEvent {
                    player: ev.client,
                    offer_id,
                    accepted,
                    requested_instance_id,
                    tick: combat_clock.tick,
                });
            }
            ClientRequestV1::NpcInspectRequest { npc_entity_id, .. } => {
                let Some(target) = resolve_npc_engagement_target(
                    ev.client,
                    npc_entity_id,
                    &combat_params,
                    &npc_engagement_params,
                    alchemy_params.zones.as_deref(),
                ) else {
                    send_npc_interaction_feedback(
                        ev.client,
                        &mut clients,
                        "[NPC] 目标已不在附近，无法查看。",
                    );
                    continue;
                };
                if target.reputation_to_player < -30 {
                    emit_npc_refuse_audio(
                        &mut npc_engagement_params.audio_events,
                        ev.client,
                        target.position,
                    );
                }
                send_npc_interaction_feedback(
                    ev.client,
                    &mut clients,
                    format!("§7[NPC] {}：{}", target.display_name, target.greeting_text),
                );
            }
            ClientRequestV1::NpcDialogueChoice {
                npc_entity_id,
                option_id,
                ..
            } => {
                let Some(target) = resolve_npc_engagement_target(
                    ev.client,
                    npc_entity_id,
                    &combat_params,
                    &npc_engagement_params,
                    alchemy_params.zones.as_deref(),
                ) else {
                    send_npc_interaction_feedback(
                        ev.client,
                        &mut clients,
                        "[NPC] 目标已不在附近，无法交谈。",
                    );
                    continue;
                };
                let option = option_id.trim();
                match option {
                    "inspect" => send_npc_interaction_feedback(
                        ev.client,
                        &mut clients,
                        format!("§7[NPC] 你端详了一眼 {}。", target.display_name),
                    ),
                    "trade" if target.can_trade() => send_npc_interaction_feedback(
                        ev.client,
                        &mut clients,
                        format!("§7[NPC] {} 摊开了随身货物。", target.display_name),
                    ),
                    "leave" => {}
                    _ => {
                        emit_npc_refuse_audio(
                            &mut npc_engagement_params.audio_events,
                            ev.client,
                            target.position,
                        );
                        send_npc_interaction_feedback(
                            ev.client,
                            &mut clients,
                            format!("§c[NPC] {} 不愿回应这个选择。", target.display_name),
                        );
                    }
                }
            }
            ClientRequestV1::NpcTradeRequest {
                npc_entity_id,
                offered_items,
                requested_item_id,
                ..
            } => {
                let Some(target) = resolve_npc_engagement_target(
                    ev.client,
                    npc_entity_id,
                    &combat_params,
                    &npc_engagement_params,
                    alchemy_params.zones.as_deref(),
                ) else {
                    send_npc_interaction_feedback(
                        ev.client,
                        &mut clients,
                        "[NPC] 目标已不在附近，无法交易。",
                    );
                    continue;
                };
                if !offered_items.is_empty() {
                    emit_npc_refuse_audio(
                        &mut npc_engagement_params.audio_events,
                        ev.client,
                        target.position,
                    );
                    send_npc_interaction_feedback(
                        ev.client,
                        &mut clients,
                        "§c[NPC] 当前交易只支持骨币结算。",
                    );
                    continue;
                }
                let Some((template_id, base_price)) =
                    npc_trade_catalog_entry(target.archetype, &requested_item_id)
                else {
                    emit_npc_refuse_audio(
                        &mut npc_engagement_params.audio_events,
                        ev.client,
                        target.position,
                    );
                    send_npc_interaction_feedback(
                        ev.client,
                        &mut clients,
                        format!("§c[NPC] {} 没有这件货。", target.display_name),
                    );
                    continue;
                };
                if !target.can_trade() {
                    emit_npc_refuse_audio(
                        &mut npc_engagement_params.audio_events,
                        ev.client,
                        target.position,
                    );
                    send_npc_interaction_feedback(
                        ev.client,
                        &mut clients,
                        format!("§c[NPC] {} 不做买卖。", target.display_name),
                    );
                    continue;
                }
                // P3: 将旧 i32 信誉转为 0.0-1.0 范围用于新定价系统。
                // plan-territory-v1 P1: 叠加 NpcPlayerReputation（霸主驻守 rep 加成写入此组件）。
                // 叠加策略：先取 FactionMembership baseline (i32 → [0,1])，
                // 再加 NpcPlayerReputation 的偏移量（默认 0.5 对应"中立=0 偏移"），
                // 即 delta = npc_rep_score - 0.5，faction_baseline + delta，再 clamp。
                let faction_rep_f32 =
                    ((target.reputation_to_player as f32 + 100.0) / 200.0).clamp(0.0, 1.0);
                let npc_rep_delta = target
                    .npc_player_rep
                    .as_ref()
                    .map(|rep| {
                        let player_id = clients
                            .get(ev.client)
                            .map(|(username, _)| canonical_player_id(username.0.as_str()))
                            .unwrap_or_default();
                        // NpcPlayerReputation.get() 默认 0.5（中立），
                        // 霸主驻守后逼近 0.7+（High tier）。
                        // delta = score - 0.5（正 = 比中立好，负 = 比中立差）。
                        rep.get(player_id.as_str()) - 0.5
                    })
                    .unwrap_or(0.0);
                let rep_f32 = (faction_rep_f32 + npc_rep_delta).clamp(0.0, 1.0);
                let rep_tier = crate::npc::trade::RepTier::from_score(rep_f32);
                let eligibility = crate::npc::trade::check_trade_eligibility(rep_tier);
                let price = match eligibility {
                    crate::npc::trade::TradeEligibility::Refused => {
                        let attack_hint = if rep_f32 <= 0.05 {
                            "，已经起了杀心"
                        } else {
                            ""
                        };
                        emit_npc_refuse_audio(
                            &mut npc_engagement_params.audio_events,
                            ev.client,
                            target.position,
                        );
                        send_npc_interaction_feedback(
                            ev.client,
                            &mut clients,
                            format!(
                                "§c[NPC] {} 对你充满敌意，拒绝交易{attack_hint}。",
                                target.display_name
                            ),
                        );
                        continue;
                    }
                    crate::npc::trade::TradeEligibility::RefuseRare => {
                        // Low 信誉：Rare+（含 Rare/Epic/Legendary/Ancient）直接拒绝；
                        // Common/Uncommon 允许，但加 1.3x markup。
                        // 阈值注释见 trade.rs RepTier::Low（"加价 + 拒绝稀有品"）。
                        //
                        // NOTE: ItemRarity 未实现 PartialOrd，用 matches! 枚举 Rare+ 变体。
                        // 如需新增更高 rarity 变体，记得同步更新此处。
                        let item_rarity = alchemy_params
                            .item_registry
                            .get(template_id)
                            .map(|t| t.rarity)
                            .unwrap_or(crate::inventory::ItemRarity::Common);
                        if is_rarity_refused_at_low_rep(item_rarity) {
                            emit_npc_refuse_audio(
                                &mut npc_engagement_params.audio_events,
                                ev.client,
                                target.position,
                            );
                            send_npc_interaction_feedback(
                                ev.client,
                                &mut clients,
                                format!("§c[NPC] {} 不愿将此物卖给你。", target.display_name),
                            );
                            continue;
                        }
                        // Common/Uncommon：允许，1.3x 加价
                        let config = crate::npc::trade::TradePricingConfig::default();
                        (base_price as f64 * config.rep_low_markup as f64)
                            .ceil()
                            .max(1.0) as u64
                    }
                    crate::npc::trade::TradeEligibility::Allowed { price_modifier } => {
                        (base_price as f64 * price_modifier as f64).ceil().max(1.0) as u64
                    }
                };
                let Ok(mut inventory) = inventories.get_mut(ev.client) else {
                    send_npc_interaction_feedback(
                        ev.client,
                        &mut clients,
                        "[NPC] 你的行囊尚未就绪，交易失败。",
                    );
                    continue;
                };
                if inventory.bone_coins < price {
                    emit_npc_refuse_audio(
                        &mut npc_engagement_params.audio_events,
                        ev.client,
                        target.position,
                    );
                    send_npc_interaction_feedback(
                        ev.client,
                        &mut clients,
                        format!("§c[NPC] 骨币不足，需要 {price} 枚。"),
                    );
                    continue;
                }
                let Some(instance_allocator) = alchemy_params.instance_allocator.as_deref_mut()
                else {
                    send_npc_interaction_feedback(
                        ev.client,
                        &mut clients,
                        "[NPC] 交易账本未就绪。",
                    );
                    continue;
                };
                if let Err(error) = add_item_to_player_inventory(
                    &mut inventory,
                    &alchemy_params.item_registry,
                    instance_allocator,
                    template_id,
                    1,
                    combat_clock.tick,
                ) {
                    send_npc_interaction_feedback(
                        ev.client,
                        &mut clients,
                        format!("§c[NPC] 交易失败：{error}"),
                    );
                    continue;
                }
                inventory.bone_coins = inventory.bone_coins.saturating_sub(price);
                inventory.revision.0 = inventory.revision.0.saturating_add(1);
                let Ok((username, mut client)) = clients.get_mut(ev.client) else {
                    continue;
                };
                client.send_chat_message(format!(
                    "§a[NPC] 你用 {price} 枚骨币从 {} 手中买下 {}。",
                    target.display_name, template_id
                ));
                record_player_npc_interaction(
                    &mut npc_engagement_params.memories,
                    &npc_engagement_params.lifecycles,
                    target.entity,
                    ev.client,
                    NpcInteractionType::Trade,
                    NpcInteractionOutcome::Friendly,
                    combat_clock.tick,
                );
                if let (Ok(player_state), Ok(cultivation)) = (
                    player_states.get(ev.client),
                    skill_scroll_params.cultivations.get(ev.client),
                ) {
                    send_inventory_snapshot_to_client(
                        ev.client,
                        &mut client,
                        username.0.as_str(),
                        &inventory,
                        player_state,
                        cultivation,
                        "npc_trade",
                    );
                }
            }
            ClientRequestV1::ZhenfaPlace {
                x,
                y,
                z,
                kind,
                carrier,
                qi_invest_ratio,
                trigger,
                item_instance_id,
                target_face,
                ..
            } => {
                let Some(place_tx) = dispatch.zhenfa_place_tx.as_deref_mut() else {
                    tracing::warn!(
                        "[bong][network] dropped zhenfa_place because ZhenfaPlaceRequest event resource is missing"
                    );
                    continue;
                };
                place_tx.send(ZhenfaPlaceRequest {
                    player: ev.client,
                    pos: [x, y, z],
                    kind,
                    carrier: carrier.unwrap_or_default(),
                    qi_invest_ratio,
                    trigger,
                    item_instance_id,
                    target_face,
                    requested_at_tick: combat_clock.tick,
                });
            }
            ClientRequestV1::ZhenfaTrigger { instance_id, .. } => {
                let Some(trigger_tx) = dispatch.zhenfa_trigger_tx.as_deref_mut() else {
                    tracing::warn!(
                        "[bong][network] dropped zhenfa_trigger because ZhenfaTriggerRequest event resource is missing"
                    );
                    continue;
                };
                trigger_tx.send(ZhenfaTriggerRequest {
                    player: ev.client,
                    instance_id,
                    requested_at_tick: combat_clock.tick,
                });
            }
            ClientRequestV1::ZhenfaDisarm { x, y, z, mode, .. } => {
                let Some(disarm_tx) = dispatch.zhenfa_disarm_tx.as_deref_mut() else {
                    tracing::warn!(
                        "[bong][network] dropped zhenfa_disarm because ZhenfaDisarmRequest event resource is missing"
                    );
                    continue;
                };
                disarm_tx.send(ZhenfaDisarmRequest {
                    player: ev.client,
                    pos: [x, y, z],
                    mode,
                    requested_at_tick: combat_clock.tick,
                });
            }
            ClientRequestV1::QiScatterBeadUse {
                item_instance_id,
                x,
                y,
                z,
                ..
            } => {
                let Some(use_tx) = dispatch.qi_scatter_bead_use_tx.as_deref_mut() else {
                    tracing::warn!(
                        "[bong][network] dropped qi_scatter_bead_use because ScatterBeadUseRequest event resource is missing"
                    );
                    continue;
                };
                let bury_pos = match (x, y, z) {
                    (Some(x), Some(y), Some(z)) => Some([x, y, z]),
                    (None, None, None) => None,
                    _ => {
                        tracing::warn!(
                            "[bong][network] dropped malformed qi_scatter_bead_use: x/y/z must be all present or all absent"
                        );
                        continue;
                    }
                };
                use_tx.send(ScatterBeadUseRequest {
                    player: ev.client,
                    item_instance_id,
                    bury_pos,
                    requested_at_tick: combat_clock.tick,
                });
            }
            ClientRequestV1::LearnSkillScroll { instance_id, .. } => {
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
            ClientRequestV1::TechniqueScrollUse { instance_id, .. } => {
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
            ClientRequestV1::AlchemyIgnite {
                furnace_pos,
                recipe_id,
                ..
            } => {
                handle_alchemy_ignite(
                    ev.client,
                    furnace_pos,
                    recipe_id,
                    &mut clients,
                    &mut alchemy_params.furnaces,
                    &alchemy_params.recipe_registry,
                    alchemy_params.zones.as_deref(),
                    alchemy_params.redis.as_deref(),
                    alchemy_params.vfx_events.as_deref_mut(),
                );
            }
            ClientRequestV1::AlchemyFeedSlot {
                furnace_pos,
                slot_idx,
                material,
                count,
                ..
            } => {
                handle_alchemy_feed_slot(
                    ev.client,
                    furnace_pos,
                    slot_idx,
                    material,
                    count,
                    &mut clients,
                    &mut alchemy_params.furnaces,
                    &alchemy_params.recipe_registry,
                    &mut inventories,
                    &player_states,
                    &skill_scroll_params.cultivations,
                    alchemy_params.zones.as_deref_mut(),
                    alchemy_params.attrition_qi_transfers.as_deref_mut(),
                    alchemy_params.attrition_applied_events.as_deref_mut(),
                    alchemy_params.tsy_lifecycle.as_deref(),
                );
            }
            ClientRequestV1::AlchemyTakeBack {
                furnace_pos,
                slot_idx,
                ..
            } => {
                handle_alchemy_take_back(
                    ev.client,
                    furnace_pos,
                    slot_idx,
                    combat_clock.tick,
                    &mut clients,
                    &mut alchemy_params.furnaces,
                    &alchemy_params.recipe_registry,
                    &mut alchemy_params.outcome_tx,
                    &mut inventories,
                    &player_states,
                    &skill_scroll_params.cultivations,
                    &alchemy_params.item_registry,
                    alchemy_params.instance_allocator.as_deref_mut(),
                    alchemy_params.vfx_events.as_deref_mut(),
                );
            }
            ClientRequestV1::InventoryMoveIntent {
                instance_id,
                from,
                to,
                ..
            } => {
                handle_inventory_move(
                    ev.client,
                    instance_id,
                    from,
                    to,
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
                );
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
            ClientRequestV1::InventoryDiscardItem {
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
            ClientRequestV1::PickupDroppedItem { instance_id, .. } => {
                handle_pickup_dropped_item(
                    ev.client,
                    instance_id,
                    &mut inventories,
                    &mut dropped_loot_params.registry,
                    &combat_params.item_registry,
                    &mut clients,
                    &player_states,
                    &skill_scroll_params.cultivations,
                    &dropped_loot_params.positions,
                    &skill_scroll_params.dimensions,
                    alchemy_params.zones.as_deref_mut(),
                    alchemy_params.attrition_qi_transfers.as_deref_mut(),
                    alchemy_params.attrition_applied_events.as_deref_mut(),
                    alchemy_params.tsy_lifecycle.as_deref(),
                );
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
                    &combat_clock,
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
                        issued_at_tick: combat_clock.tick,
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
                    &combat_clock,
                    &mut commands,
                    &mut clients,
                    &mut combat_params,
                    &inventories,
                );
            }
            ClientRequestV1::QuickSlotBind { slot, item_id, .. } => {
                handle_quick_slot_bind(
                    ev.client,
                    slot,
                    item_id,
                    &mut combat_params.bindings_q,
                    &inventories,
                    &clients,
                    persistence.as_deref(),
                );
            }
            ClientRequestV1::SkillBarCast { slot, target, .. } => {
                handle_skill_bar_cast(
                    ev.client,
                    slot,
                    target,
                    &combat_clock,
                    &mut commands,
                    &mut clients,
                    &mut combat_params,
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
            ClientRequestV1::CombatReincarnate { .. } => {
                if let Some(revival_tx) = dispatch.revival_tx.as_deref_mut() {
                    revival_tx.send(RevivalActionIntent {
                        entity: ev.client,
                        action: RevivalActionKind::Reincarnate,
                        issued_at_tick: combat_clock.tick,
                    });
                }
            }
            ClientRequestV1::CombatTerminate { .. } => {
                if let Some(revival_tx) = dispatch.revival_tx.as_deref_mut() {
                    revival_tx.send(RevivalActionIntent {
                        entity: ev.client,
                        action: RevivalActionKind::Terminate,
                        issued_at_tick: combat_clock.tick,
                    });
                }
            }
            ClientRequestV1::CombatCreateNewCharacter { .. } => {
                if let Some(revival_tx) = dispatch.revival_tx.as_deref_mut() {
                    revival_tx.send(RevivalActionIntent {
                        entity: ev.client,
                        action: RevivalActionKind::CreateNewCharacter,
                        issued_at_tick: combat_clock.tick,
                    });
                }
            }
            ClientRequestV1::StartExtractRequest {
                portal_entity_id, ..
            } => {
                tracing::info!(
                    "[bong][network] client_request start_extract entity={:?} portal_bits={portal_entity_id}",
                    ev.client
                );
                let Some(start_extract_tx) = combat_params.start_extract_tx.as_deref_mut() else {
                    tracing::warn!(
                        "[bong][network] dropped start_extract because StartExtractRequest event resource is missing"
                    );
                    continue;
                };
                let Ok(portal) = Entity::try_from_bits(portal_entity_id) else {
                    tracing::warn!(
                        "[bong][network] dropped start_extract: invalid portal_entity_id bits={portal_entity_id}"
                    );
                    continue;
                };
                start_extract_tx.send(StartExtractRequestEvent {
                    player: ev.client,
                    portal,
                });
            }
            ClientRequestV1::CancelExtractRequest { .. } => {
                tracing::info!(
                    "[bong][network] client_request cancel_extract entity={:?}",
                    ev.client
                );
                let Some(cancel_extract_tx) = combat_params.cancel_extract_tx.as_deref_mut() else {
                    tracing::warn!(
                        "[bong][network] dropped cancel_extract because CancelExtractRequest event resource is missing"
                    );
                    continue;
                };
                cancel_extract_tx.send(CancelExtractRequestEvent { player: ev.client });
            }
            ClientRequestV1::StartSearch {
                container_entity_id,
                ..
            } => {
                tracing::info!(
                    "[bong][network] client_request start_search entity={:?} container_bits={container_entity_id}",
                    ev.client
                );
                let Some(start_search_tx) = combat_params.start_search_tx.as_deref_mut() else {
                    tracing::warn!(
                        "[bong][network] dropped start_search because StartSearchRequest event resource is missing"
                    );
                    continue;
                };
                let Ok(container) = Entity::try_from_bits(container_entity_id) else {
                    tracing::warn!(
                        "[bong][network] dropped start_search: invalid container_entity_id bits={container_entity_id}"
                    );
                    continue;
                };
                start_search_tx.send(StartSearchRequestEvent {
                    player: ev.client,
                    container,
                });
            }
            ClientRequestV1::CancelSearch { .. } => {
                tracing::info!(
                    "[bong][network] client_request cancel_search entity={:?}",
                    ev.client
                );
                let Some(cancel_search_tx) = combat_params.cancel_search_tx.as_deref_mut() else {
                    tracing::warn!(
                        "[bong][network] dropped cancel_search because CancelSearchRequest event resource is missing"
                    );
                    continue;
                };
                cancel_search_tx.send(CancelSearchRequestEvent { player: ev.client });
            }
            // ── 物资棺 entity-based open（plan-supply-coffin-loot-ui P2）──
            ClientRequestV1::SupplyCoffinOpen { entity_id, .. } => {
                tracing::info!(
                    "[bong][network] client_request supply_coffin_open entity={:?} target_id={entity_id}",
                    ev.client
                );
                let Some(entity_manager) = combat_params.entity_manager.as_deref() else {
                    tracing::warn!(
                        "[bong][network] dropped supply_coffin_open because EntityManager resource is missing"
                    );
                    continue;
                };
                let Some(target) = entity_manager.get_by_id(entity_id) else {
                    tracing::debug!(
                        "[bong][network] supply_coffin_open rejected: no entity for protocol id {entity_id}"
                    );
                    if let Ok((_username, mut client)) = clients.get_mut(ev.client) {
                        client.send_chat_message("§c[物资棺] 目标不存在。");
                    }
                    continue;
                };
                if let Some(supply_coffin_open_tx) = dispatch.supply_coffin_open_tx.as_deref_mut() {
                    supply_coffin_open_tx.send(
                        crate::supply_coffin::interact::SupplyCoffinOpenRequest {
                            client: ev.client,
                            target,
                        },
                    );
                } else {
                    tracing::warn!(
                        "[bong][network] dropped supply_coffin_open because SupplyCoffinOpenRequest event resource is missing"
                    );
                }
            }
            // ── 通用世界容器 entity-based open（plan-placeable-container-blocks-v1 P1）──
            ClientRequestV1::ContainerOpen { entity_id, .. } => {
                tracing::info!(
                    "[bong][network] client_request container_open entity={:?} target_id={entity_id}",
                    ev.client
                );
                let Some(entity_manager) = combat_params.entity_manager.as_deref() else {
                    tracing::warn!(
                        "[bong][network] dropped container_open because EntityManager resource is missing"
                    );
                    continue;
                };
                let Some(target) = entity_manager.get_by_id(entity_id) else {
                    tracing::debug!(
                        "[bong][network] container_open rejected: no entity for protocol id {entity_id}"
                    );
                    if let Ok((_username, mut client)) = clients.get_mut(ev.client) {
                        client.send_chat_message("§c[容器] 目标不存在。");
                    }
                    continue;
                };
                if let Some(container_open_tx) = dispatch.container_open_tx.as_deref_mut() {
                    container_open_tx.send(crate::world::container_open::ContainerOpenRequest {
                        client: ev.client,
                        target,
                    });
                } else {
                    tracing::warn!(
                        "[bong][network] dropped container_open because ContainerOpenRequest event resource is missing"
                    );
                }
            }
            // ── 制作台 entity-based open（plan-workbench-place-runtime-v1 P2）──
            ClientRequestV1::WorkbenchOpen { entity_id, .. } => {
                tracing::info!(
                    "[bong][network] client_request workbench_open entity={:?} target_id={entity_id}",
                    ev.client
                );
                let Some(entity_manager) = combat_params.entity_manager.as_deref() else {
                    tracing::warn!(
                        "[bong][network] dropped workbench_open because EntityManager resource is missing"
                    );
                    continue;
                };
                let Some(workbench) = entity_manager.get_by_id(entity_id) else {
                    tracing::debug!(
                        "[bong][network] workbench_open rejected: no entity for protocol id {entity_id}"
                    );
                    if let Ok((_username, mut client)) = clients.get_mut(ev.client) {
                        client.send_chat_message("§c[制作台] 目标不存在。");
                    }
                    continue;
                };
                if let Some(workbench_open_tx) = dispatch.workbench_open_tx.as_deref_mut() {
                    workbench_open_tx.send(crate::craft::WorkbenchOpenRequest {
                        client: ev.client,
                        workbench,
                    });
                } else {
                    tracing::warn!(
                        "[bong][network] dropped workbench_open because WorkbenchOpenRequest event resource is missing"
                    );
                }
            }
            // ── 外部容器 move / close ─────────────
            ClientRequestV1::ExternalContainerMove {
                session_id,
                instance_id,
                from,
                to,
                ..
            } => {
                handle_external_container_move(
                    ev.client,
                    session_id,
                    instance_id,
                    &from,
                    &to,
                    &mut dispatch,
                    &mut combat_params,
                    &mut inventories,
                    &player_states,
                    &skill_scroll_params.cultivations,
                    &mut clients,
                    &mut commands,
                );
            }
            ClientRequestV1::ExternalContainerClose { session_id, .. } => {
                handle_external_container_close(
                    ev.client,
                    session_id,
                    &mut dispatch,
                    &mut combat_params,
                    &mut inventories,
                    &player_states,
                    &skill_scroll_params.cultivations,
                    &mut clients,
                    &mut commands,
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
                let pos = valence::prelude::BlockPos::new(x, y, z);
                // plan §1.2.2 — terrain / environment 由 server 从 chunk_layer 派生，
                // 避免客户端伪造；session 再按 `TerrainKind::is_tillable` 决定放行。
                let (terrain, environment) = match lingtian_tx.layers.get_single() {
                    Ok(layer) => {
                        let terrain = layer
                            .block(pos)
                            .map(|b| terrain_from_block_kind(b.state.to_kind()))
                            .unwrap_or(TerrainKind::Unknown);
                        (terrain, read_environment_at(layer, pos))
                    }
                    Err(err) => {
                        tracing::warn!(
                            "[bong][network] lingtian_start_till: chunk layer unavailable ({err:?}); \
                             falling back to Unknown terrain — session will reject."
                        );
                        (TerrainKind::Unknown, PlotEnvironment::base())
                    }
                };
                tracing::info!(
                    "[bong][network] client_request lingtian_start_till entity={:?} pos=[{x},{y},{z}] hoe_inst={hoe_instance_id} mode={mode} terrain={terrain:?}",
                    ev.client
                );
                lingtian_tx.till_tx.send(StartTillRequest {
                    player: ev.client,
                    pos,
                    hoe_instance_id,
                    mode: parse_session_mode(&mode),
                    terrain,
                    environment,
                });
            }
            ClientRequestV1::LingtianStartRenew {
                x,
                y,
                z,
                hoe_instance_id,
                ..
            } => {
                tracing::info!(
                    "[bong][network] client_request lingtian_start_renew entity={:?} pos=[{x},{y},{z}] hoe_inst={hoe_instance_id}",
                    ev.client
                );
                lingtian_tx.renew_tx.send(StartRenewRequest {
                    player: ev.client,
                    pos: valence::prelude::BlockPos::new(x, y, z),
                    hoe_instance_id,
                });
            }
            ClientRequestV1::LingtianStartPlanting {
                x, y, z, plant_id, ..
            } => {
                tracing::info!(
                    "[bong][network] client_request lingtian_start_planting entity={:?} pos=[{x},{y},{z}] plant_id={plant_id}",
                    ev.client
                );
                lingtian_tx.planting_tx.send(StartPlantingRequest {
                    player: ev.client,
                    pos: valence::prelude::BlockPos::new(x, y, z),
                    plant_id,
                });
            }
            ClientRequestV1::LingtianStartHarvest { x, y, z, mode, .. } => {
                tracing::info!(
                    "[bong][network] client_request lingtian_start_harvest entity={:?} pos=[{x},{y},{z}] mode={mode}",
                    ev.client
                );
                lingtian_tx.harvest_tx.send(StartHarvestRequest {
                    player: ev.client,
                    pos: valence::prelude::BlockPos::new(x, y, z),
                    mode: parse_session_mode(&mode),
                });
            }
            ClientRequestV1::LingtianStartReplenish {
                x, y, z, source, ..
            } => {
                tracing::info!(
                    "[bong][network] client_request lingtian_start_replenish entity={:?} pos=[{x},{y},{z}] source={source}",
                    ev.client
                );
                let Some(parsed) = parse_replenish_source(&source) else {
                    tracing::warn!(
                        "[bong][network] lingtian_start_replenish ignored: unknown source `{source}`"
                    );
                    continue;
                };
                lingtian_tx.replenish_tx.send(StartReplenishRequest {
                    player: ev.client,
                    pos: valence::prelude::BlockPos::new(x, y, z),
                    source: parsed,
                });
            }
            ClientRequestV1::LingtianStartDrainQi { x, y, z, .. } => {
                tracing::info!(
                    "[bong][network] client_request lingtian_start_drain_qi entity={:?} pos=[{x},{y},{z}]",
                    ev.client
                );
                lingtian_tx.drain_qi_tx.send(StartDrainQiRequest {
                    player: ev.client,
                    pos: valence::prelude::BlockPos::new(x, y, z),
                });
            }
            ClientRequestV1::ForgeStationPlace {
                x,
                y,
                z,
                item_instance_id,
                station_tier,
                ..
            } => {
                tracing::info!(
                    "[bong][network][forge] station_place entity={:?} pos=[{x},{y},{z}] instance={item_instance_id} tier={station_tier}",
                    ev.client
                );
                if let Some(place_forge_station_tx) = dispatch.place_forge_station_tx.as_deref_mut()
                {
                    place_forge_station_tx.send(PlaceForgeStationRequest {
                        player: ev.client,
                        pos: valence::prelude::BlockPos::new(x, y, z),
                        item_instance_id,
                        station_tier,
                    });
                }
            }
            ClientRequestV1::ForgeInscriptionScroll {
                session_id,
                inscription_id,
                ..
            } => {
                handle_forge_inscription_scroll(
                    ev.client,
                    session_id,
                    &inscription_id,
                    &mut inventories,
                    &combat_params.item_registry,
                    &mut clients,
                    &player_states,
                    &skill_scroll_params.cultivations,
                    &mut skill_scroll_params.inscription_scroll_tx,
                    skill_scroll_params.forge_sessions.as_deref(),
                );
            }
            ClientRequestV1::ForgeTemperingHit {
                session_id,
                beat,
                ticks_remaining,
                ..
            } => {
                handle_forge_tempering_hit(
                    ev.client,
                    session_id,
                    &beat,
                    ticks_remaining,
                    &mut dispatch.tempering_hit_tx,
                    skill_scroll_params.forge_sessions.as_deref(),
                );
            }
            ClientRequestV1::ForgeConsecrationInject {
                session_id,
                qi_amount,
                ..
            } => {
                handle_forge_consecration_inject(
                    ev.client,
                    session_id,
                    qi_amount,
                    &mut dispatch.consecration_inject_tx,
                    skill_scroll_params.forge_sessions.as_deref(),
                );
            }
            ClientRequestV1::ForgeStepAdvance { session_id, .. } => {
                handle_forge_step_advance(
                    ev.client,
                    session_id,
                    &mut dispatch.step_advance_tx,
                    skill_scroll_params.forge_sessions.as_deref(),
                );
            }
            ClientRequestV1::ForgeLearnBlueprint { blueprint_id, .. } => {
                handle_forge_learn_blueprint(
                    ev.client,
                    &blueprint_id,
                    &mut commands,
                    &mut inventories,
                    &combat_params.item_registry,
                    &mut clients,
                    &player_states,
                    &skill_scroll_params.cultivations,
                    &mut skill_scroll_params.learned_blueprints,
                );
            }
            // ─── 炼器（武器）（plan-forge-v1 §1.3-§1.4）── wait for wiring ───
            ClientRequestV1::ForgeStartSession { .. }
            | ClientRequestV1::ForgeBlueprintTurnPage { .. } => {
                tracing::debug!(
                    "[bong][forge][network] plan-forge-v1 client_request not yet wired"
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
                    &combat_params.item_registry,
                    combat_params.entity_manager.as_deref(),
                    &mut clients,
                    dispatch.give_dan_to_elder_tx.as_deref_mut(),
                );
            }
            // ─── plan-shield-block-v1 P1：盾牌举盾 intent ─────────────────────
            ClientRequestV1::RaiseShield { .. } => {
                tracing::debug!("[bong][shield] RaiseShield received entity={:?}", ev.client);
                dispatch
                    .raise_shield_tx
                    .send(RaiseShieldIntent { player: ev.client });
            }
            ClientRequestV1::LowerShield { .. } => {
                tracing::debug!("[bong][shield] LowerShield received entity={:?}", ev.client);
                dispatch
                    .lower_shield_tx
                    .send(LowerShieldIntent { player: ev.client });
            }
            // ─── plan-scroll-reading-v1 P0：可阅读残卷阅读请求 ─────────────
            // 读取不消耗物品（区别于 read_combat_technique_scroll 消耗式学招）。
            // 无 spec / 伪 instance_id / 非本人物品三类均静默拒绝 + warn（不向 client
            // 暴露具体拒绝原因，避免给作弊 client 探测 instance_id 分布的信号）。
            ClientRequestV1::ScrollReadRequest { instance_id, .. } => {
                let Ok(inventory) = inventories.get(ev.client) else {
                    tracing::warn!(
                        "[bong][network] client_request scroll_read_request rejected: entity={:?} has no PlayerInventory",
                        ev.client
                    );
                    continue;
                };
                match crate::network::scroll_open_emit::resolve_scroll_read_request(
                    inventory,
                    &combat_params.item_registry,
                    instance_id,
                ) {
                    Ok(resolution) => {
                        tracing::info!(
                            "[bong][network] client_request scroll_read_request entity={:?} instance_id={instance_id}",
                            ev.client
                        );
                        let anim_id = resolution.anim_id.clone();
                        crate::network::scroll_open_emit::emit_scroll_open(
                            ev.client,
                            resolution.into_payload(),
                            &mut clients,
                        );
                        // P2 — 展开微光：与 anim_id 是否存在无关，任意成功开卷都应有视觉反馈。
                        if let Ok(position) = combat_params.positions.get(ev.client) {
                            if let Some(vfx_events) = alchemy_params.vfx_events.as_deref_mut() {
                                vfx_events
                                    .send(crate::network::vfx_event_emit::VfxEventRequest::new(
                                    position.get(),
                                    crate::schema::vfx_event::VfxEventPayloadV1::SpawnParticle {
                                        event_id: SCROLL_OPEN_GLOW_EVENT_ID.to_string(),
                                        origin: [
                                            position.get().x,
                                            position.get().y,
                                            position.get().z,
                                        ],
                                        direction: None,
                                        color: Some(SCROLL_OPEN_GLOW_COLOR.to_string()),
                                        strength: Some(SCROLL_OPEN_GLOW_STRENGTH),
                                        count: Some(SCROLL_OPEN_GLOW_COUNT),
                                        duration_ticks: Some(SCROLL_OPEN_GLOW_DURATION_TICKS),
                                    },
                                ));
                            }
                        }
                        // §8.1 #1：动画只在模板挂了 anim_id 时才播（残卷不强制有阅读动画）。
                        if let Some(anim_id) = anim_id {
                            // P2 — 插入 ScrollReading marker（真相源，供 ScrollReadClosed /
                            // 死亡兜底停止动画）。插入不依赖 Position/UniqueId 查得到——就算
                            // entity 暂查不到坐标，"该玩家正在读卷"这件事本身仍然成立。
                            commands.entity(ev.client).insert(
                                crate::network::scroll_open_emit::ScrollReading {
                                    anim_id: anim_id.clone(),
                                },
                            );
                            if let (Ok(position), Ok(unique_id)) = (
                                combat_params.positions.get(ev.client),
                                combat_params.unique_ids.get(ev.client),
                            ) {
                                if let Some(vfx_events) = alchemy_params.vfx_events.as_deref_mut() {
                                    vfx_events.send(
                                        crate::network::vfx_event_emit::VfxEventRequest::new(
                                            position.get(),
                                            crate::schema::vfx_event::VfxEventPayloadV1::PlayAnim {
                                                target_player: unique_id.0.to_string(),
                                                anim_id,
                                                priority: SCROLL_READ_ANIM_PRIORITY,
                                                fade_in_ticks: Some(SCROLL_READ_ANIM_FADE_IN_TICKS),
                                            },
                                        ),
                                    );
                                }
                            }
                        }
                    }
                    Err(reason) => {
                        tracing::warn!(
                            "[bong][network] client_request scroll_read_request rejected: entity={:?} instance_id={instance_id} reason={reason:?}",
                            ev.client
                        );
                    }
                }
            }
            // ─── plan-scroll-reading-v1 P2 §8.1#4：阅读屏关闭 → 停止循环阅读动画 ─────
            // ScrollReading marker 是"读卷中"的真相源（而非 status）；命中就发
            // StopAnim + 移除 marker，未命中（该玩家当时没有挂动画，或已被死亡/断线
            // 兜底清理过）静默跳过，不重复停止。
            ClientRequestV1::ScrollReadClosed { .. } => {
                if let Ok(reading) = combat_params.scroll_reading_q.get(ev.client) {
                    let anim_id = reading.anim_id.clone();
                    if let (Ok(position), Ok(unique_id)) = (
                        combat_params.positions.get(ev.client),
                        combat_params.unique_ids.get(ev.client),
                    ) {
                        if let Some(vfx_events) = alchemy_params.vfx_events.as_deref_mut() {
                            vfx_events.send(crate::network::vfx_event_emit::VfxEventRequest::new(
                                position.get(),
                                crate::schema::vfx_event::VfxEventPayloadV1::StopAnim {
                                    target_player: unique_id.0.to_string(),
                                    anim_id,
                                    fade_out_ticks: Some(
                                        crate::network::vfx_animation_trigger::SCROLL_READ_ANIM_FADE_OUT_TICKS,
                                    ),
                                },
                            ));
                        }
                    }
                    commands
                        .entity(ev.client)
                        .remove::<crate::network::scroll_open_emit::ScrollReading>();
                    tracing::debug!(
                        "[bong][network] client_request scroll_read_closed entity={:?} anim stopped",
                        ev.client
                    );
                } else {
                    tracing::debug!(
                        "[bong][network] client_request scroll_read_closed entity={:?} (no ScrollReading marker, no-op)",
                        ev.client
                    );
                }
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
        }
    }
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
        can_learn_technique(
            known,
            cultivation,
            &meridians,
            severed,
            technique_id.as_str(),
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
            matches!(
                learn_technique_if_allowed(
                    &mut known,
                    cultivation,
                    &meridians,
                    severed,
                    technique_id.as_str(),
                    0.0,
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
            ScrollReadOutcome::MeridianSevered { .. } => "technique_scroll_meridian_severed",
            ScrollReadOutcome::MeridianMissing { .. } => "technique_scroll_meridian_missing",
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
        send_techniques_snapshot_to_client(entity, &mut client, username.0.as_str(), known);
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_forge_learn_blueprint(
    entity: Entity,
    blueprint_id: &str,
    commands: &mut Commands,
    inventories: &mut Query<&mut PlayerInventory>,
    registry: &ItemRegistry,
    clients: &mut Query<(&Username, &mut Client)>,
    player_states: &Query<&PlayerState>,
    cultivations: &Query<&Cultivation>,
    learned_blueprints: &mut Query<&mut LearnedBlueprints>,
) {
    let blueprint_id = blueprint_id.trim();
    if blueprint_id.is_empty() {
        return;
    }

    if let Ok(learned) = learned_blueprints.get_mut(entity) {
        if learned.knows(blueprint_id) {
            if let Ok(inventory) = inventories.get(entity) {
                resync_snapshot(
                    entity,
                    inventory,
                    clients,
                    player_states,
                    cultivations,
                    "forge_blueprint_already_known",
                );
            }
            return;
        }
    }

    let Some(instance_id) = inventories
        .get(entity)
        .ok()
        .and_then(|inventory| find_blueprint_scroll_instance_id(inventory, registry, blueprint_id))
    else {
        if let Ok(inventory) = inventories.get(entity) {
            resync_snapshot(
                entity,
                inventory,
                clients,
                player_states,
                cultivations,
                "forge_blueprint_scroll_missing",
            );
        }
        tracing::warn!(
            "[bong][network][forge] learn_blueprint rejected: no scroll for blueprint_id={blueprint_id} on entity={entity:?}"
        );
        return;
    };

    {
        let Ok(mut inventory) = inventories.get_mut(entity) else {
            return;
        };
        if let Err(err) = consume_item_instance_once(&mut inventory, instance_id) {
            tracing::warn!(
                "[bong][network][forge] learn_blueprint consume failed for instance_id={instance_id}: {err}"
            );
            return;
        }
        resync_snapshot(
            entity,
            &inventory,
            clients,
            player_states,
            cultivations,
            "forge_blueprint_learned",
        );
    }

    if let Ok(mut learned) = learned_blueprints.get_mut(entity) {
        learned.learn(blueprint_id.to_string());
    } else {
        let mut learned = LearnedBlueprints::new();
        learned.learn(blueprint_id.to_string());
        commands.entity(entity).insert(learned);
    }
}

fn require_owned_active_step(
    forge_sessions: Option<&ForgeSessions>,
    session: ForgeSessionId,
    entity: Entity,
    expected: ForgeStep,
    request_label: &str,
) -> bool {
    let Some(forge_sessions) = forge_sessions else {
        tracing::warn!(
            "[bong][network][forge] {request_label} rejected: ForgeSessions unavailable"
        );
        return false;
    };
    let Some(session_state) = forge_sessions.get(session) else {
        tracing::warn!(
            "[bong][network][forge] {request_label} rejected: missing session_id={}",
            session.0
        );
        return false;
    };
    if session_state.current_step != expected {
        tracing::warn!(
            "[bong][network][forge] {request_label} rejected: session_id={} step={:?}, expected={expected:?}",
            session.0,
            session_state.current_step
        );
        return false;
    }
    if session_state.caster != entity {
        tracing::warn!(
            "[bong][network][forge] {request_label} rejected: session_id={} caster mismatch entity={entity:?} session_caster={:?}",
            session.0,
            session_state.caster
        );
        return false;
    }
    true
}

#[allow(clippy::too_many_arguments)]
fn handle_forge_inscription_scroll(
    entity: Entity,
    session_id: u64,
    inscription_id: &str,
    inventories: &mut Query<&mut PlayerInventory>,
    registry: &ItemRegistry,
    clients: &mut Query<(&Username, &mut Client)>,
    player_states: &Query<&PlayerState>,
    cultivations: &Query<&Cultivation>,
    inscription_scroll_tx: &mut Option<ResMut<Events<InscriptionScrollSubmit>>>,
    forge_sessions: Option<&ForgeSessions>,
) {
    let inscription_id = inscription_id.trim();
    if inscription_id.is_empty() {
        return;
    }
    let session = ForgeSessionId(session_id);
    if !require_owned_active_step(
        forge_sessions,
        session,
        entity,
        ForgeStep::Inscription,
        "inscription_scroll",
    ) {
        return;
    }
    let Some(inscription_scroll_tx) = inscription_scroll_tx.as_deref_mut() else {
        tracing::warn!(
            "[bong][network][forge] inscription_scroll rejected: ForgePlugin events unavailable"
        );
        return;
    };

    let Some(instance_id) = inventories.get(entity).ok().and_then(|inventory| {
        find_inscription_scroll_instance_id(inventory, registry, inscription_id)
    }) else {
        if let Ok(inventory) = inventories.get(entity) {
            resync_snapshot(
                entity,
                inventory,
                clients,
                player_states,
                cultivations,
                "forge_inscription_scroll_missing",
            );
        }
        tracing::warn!(
            "[bong][network][forge] inscription_scroll rejected: no scroll for inscription_id={inscription_id} on entity={entity:?}"
        );
        return;
    };

    let Ok(mut inventory) = inventories.get_mut(entity) else {
        return;
    };
    if let Err(err) = consume_item_instance_once(&mut inventory, instance_id) {
        tracing::warn!(
            "[bong][network][forge] inscription_scroll consume failed for instance_id={instance_id}: {err}"
        );
        return;
    }
    resync_snapshot(
        entity,
        &inventory,
        clients,
        player_states,
        cultivations,
        "forge_inscription_scroll_consumed",
    );

    inscription_scroll_tx.send(InscriptionScrollSubmit {
        session,
        inscription_id: inscription_id.to_string(),
    });
}

fn handle_forge_tempering_hit(
    entity: Entity,
    session_id: u64,
    beat: &str,
    ticks_remaining: u32,
    tempering_hit_tx: &mut Option<ResMut<Events<TemperingHit>>>,
    forge_sessions: Option<&ForgeSessions>,
) {
    let Some(beat) = parse_temper_beat(beat) else {
        tracing::warn!("[bong][network][forge] tempering_hit rejected: unknown beat `{beat}`");
        return;
    };
    let session = ForgeSessionId(session_id);
    if !require_owned_active_step(
        forge_sessions,
        session,
        entity,
        ForgeStep::Tempering,
        "tempering_hit",
    ) {
        return;
    }
    let Some(tempering_hit_tx) = tempering_hit_tx.as_deref_mut() else {
        tracing::warn!(
            "[bong][network][forge] tempering_hit rejected: ForgePlugin events unavailable"
        );
        return;
    };
    tempering_hit_tx.send(TemperingHit {
        session,
        beat,
        ticks_remaining,
    });
}

fn handle_forge_consecration_inject(
    entity: Entity,
    session_id: u64,
    qi_amount: f64,
    consecration_inject_tx: &mut Option<ResMut<Events<ConsecrationInject>>>,
    forge_sessions: Option<&ForgeSessions>,
) {
    if !qi_amount.is_finite() || qi_amount < 0.0 {
        tracing::warn!(
            "[bong][network][forge] consecration_inject rejected: invalid qi_amount={qi_amount}"
        );
        return;
    }
    let session = ForgeSessionId(session_id);
    if !require_owned_active_step(
        forge_sessions,
        session,
        entity,
        ForgeStep::Consecration,
        "consecration_inject",
    ) {
        return;
    }
    let Some(consecration_inject_tx) = consecration_inject_tx.as_deref_mut() else {
        tracing::warn!(
            "[bong][network][forge] consecration_inject rejected: ForgePlugin events unavailable"
        );
        return;
    };
    consecration_inject_tx.send(ConsecrationInject { session, qi_amount });
}

fn handle_forge_step_advance(
    entity: Entity,
    session_id: u64,
    step_advance_tx: &mut Option<ResMut<Events<StepAdvance>>>,
    forge_sessions: Option<&ForgeSessions>,
) {
    let session = ForgeSessionId(session_id);
    let Some(forge_sessions) = forge_sessions else {
        tracing::warn!("[bong][network][forge] step_advance rejected: ForgeSessions unavailable");
        return;
    };
    let Some(session_state) = forge_sessions.get(session) else {
        tracing::warn!(
            "[bong][network][forge] step_advance rejected: missing session_id={session_id}"
        );
        return;
    };
    if session_state.caster != entity {
        tracing::warn!(
            "[bong][network][forge] step_advance rejected: session_id={session_id} caster mismatch entity={entity:?} session_caster={:?}",
            session_state.caster
        );
        return;
    }
    if matches!(session_state.current_step, ForgeStep::Done) {
        tracing::warn!(
            "[bong][network][forge] step_advance rejected: session_id={session_id} already done"
        );
        return;
    }
    let Some(step_advance_tx) = step_advance_tx.as_deref_mut() else {
        tracing::warn!(
            "[bong][network][forge] step_advance rejected: ForgePlugin events unavailable"
        );
        return;
    };
    step_advance_tx.send(StepAdvance { session });
}

fn parse_temper_beat(raw: &str) -> Option<TemperBeat> {
    match raw {
        "L" => Some(TemperBeat::Light),
        "H" => Some(TemperBeat::Heavy),
        "F" => Some(TemperBeat::Fold),
        _ => None,
    }
}

fn find_blueprint_scroll_instance_id(
    inventory: &PlayerInventory,
    registry: &ItemRegistry,
    blueprint_id: &str,
) -> Option<u64> {
    find_inventory_instance_id_matching(inventory, |template_id| {
        registry
            .get(template_id)
            .and_then(|template| template.blueprint_scroll_spec.as_ref())
            .is_some_and(|spec| spec.blueprint_id == blueprint_id)
    })
}

fn find_inscription_scroll_instance_id(
    inventory: &PlayerInventory,
    registry: &ItemRegistry,
    inscription_id: &str,
) -> Option<u64> {
    find_inventory_instance_id_matching(inventory, |template_id| {
        registry
            .get(template_id)
            .and_then(|template| template.inscription_scroll_spec.as_ref())
            .is_some_and(|spec| spec.inscription_id == inscription_id)
    })
}

fn find_inventory_instance_id_matching(
    inventory: &PlayerInventory,
    mut predicate: impl FnMut(&str) -> bool,
) -> Option<u64> {
    for item in inventory.hotbar.iter().flatten() {
        if predicate(item.template_id.as_str()) {
            return Some(item.instance_id);
        }
    }
    for container in &inventory.containers {
        for placed in &container.items {
            if predicate(placed.instance.template_id.as_str()) {
                return Some(placed.instance.instance_id);
            }
        }
    }
    for item in inventory.equipped.values().flat_map(|s| s.iter_all()) {
        if predicate(item.template_id.as_str()) {
            return Some(item.instance_id);
        }
    }
    None
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
    use crate::combat::components::{UnlockedStyles, WoundKind, Wounds};
    use crate::cultivation::components::{MeridianSystem, Realm};
    use crate::cultivation::tribulation::TribulationState;
    use crate::forge::session::{ForgeSession, StepState};
    use crate::inventory::{
        BlueprintScrollSpec, ContainerState, InscriptionScrollSpec, InventoryRevision,
        ItemCategory, ItemEffect, ItemInstance, ItemRarity, ItemTemplate, PlacedItemState,
    };
    use crate::npc::faction::{FactionId, FactionRank, MissionQueue, NamedFactionId, Reputation};
    use crate::skill::components::SkillSet;
    use crate::zhenfa::trap_content::TrapTargetFace;
    use valence::entity::{EntityId, EntityPlugin};
    use valence::prelude::{
        ident, App, DVec3, EntityKind, EventReader, IntoSystemConfigs, OldPosition, Position,
        ResMut, Update,
    };
    use valence::protocol::packets::play::{CustomPayloadS2c, GameMessageS2c};
    use valence::testing::{create_mock_client, MockClientHelper};

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

    fn register_request_app(app: &mut App) {
        app.insert_resource(CombatClock::default());
        app.insert_resource(crate::cultivation::skill_registry::init_registry());
        // plan-bug-qc-p1 §skill-cast P0：经脉依赖表（测试场景 default 空，各测可再声明）
        app.insert_resource(SkillMeridianDependencies::default());
        app.insert_resource(GameplayActionQueue::default());
        app.insert_resource(AlchemyMockState::default());
        app.insert_resource(DroppedLootRegistry::default());
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
        app.add_systems(
            Update,
            (
                handle_client_request_payloads,
                crate::network::inventory_event_emit::emit_durability_changed_inventory_events,
            )
                .chain(),
        );
        app.add_systems(
            Update,
            crate::alchemy::apply_alchemy_explode_outcomes.after(handle_client_request_payloads),
        );
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
                meridian_label(id),
                expected,
                "expected stable chat label for {id:?}"
            );
        }
    }

    #[test]
    fn npc_trade_request_rejects_wanted_player_through_engagement_wiring() {
        let mut app = App::new();
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

    #[test]
    fn set_meridian_target_sends_generic_meridian_chat_echo() {
        let mut app = App::new();
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
                    meridian: MeridianId::Du,
                })
                .expect("set meridian target request should serialize")
                .into_boxed_slice(),
            });

        app.update();
        flush_all_client_packets(&mut app);

        let actual_target = app
            .world()
            .get::<MeridianTarget>(entity)
            .map(|target| target.0);
        assert_eq!(
            actual_target,
            Some(MeridianId::Du),
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

    #[test]
    fn qi_scatter_bead_use_dispatches_zhenfa_event() {
        let mut app = App::new();
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
        register_request_app(&mut app);
        app.insert_resource(crate::alchemy::recipe::load_recipe_registry().unwrap());
        app.insert_resource(crate::inventory::load_item_registry().unwrap());
        app.insert_resource(crate::world::zone::ZoneRegistry {
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
        app.insert_resource(CapturedBreakthroughRequests::default());
        app.insert_resource(CapturedForgeRequests::default());
        app.insert_resource(CapturedInsightChoices::default());
        app.insert_resource(CombatClock::default());
        app.insert_resource(GameplayActionQueue::default());
        app.insert_resource(AlchemyMockState::default());
        app.insert_resource(DroppedLootRegistry::default());
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
    fn abort_tribulation_request_is_ignored_after_start_confirmation() {
        let mut app = App::new();
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
    fn quick_slot_bind_resolves_equipped_template_instance() {
        let mut app = App::new();
        register_request_app(&mut app);

        let mut inventory = empty_inventory();
        inventory.equipped.insert(
            crate::inventory::EQUIP_SLOT_OFF_HAND.to_string(),
            crate::inventory::SlotContents::held_single(inventory_test_item(77, "bone_whistle", 1)),
        );

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((client_bundle, QuickSlotBindings::default(), inventory))
            .id();
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"quick_slot_bind","v":1,"slot":0,"item_id":"bone_whistle"}"#
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

    #[test]
    fn apply_pill_during_tribulation_recovers_current_qi_only() {
        let mut app = App::new();
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
        app.insert_resource(CapturedMineralProbes::default());
        app.insert_resource(CombatClock::default());
        app.insert_resource(GameplayActionQueue::default());
        app.insert_resource(AlchemyMockState::default());
        app.insert_resource(DroppedLootRegistry::default());
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
        app.insert_resource(CapturedSpiritNichePlaces::default());
        app.insert_resource(CombatClock { tick: 88 });
        app.insert_resource(GameplayActionQueue::default());
        app.insert_resource(AlchemyMockState::default());
        app.insert_resource(DroppedLootRegistry::default());
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
        app.insert_resource(CapturedSpiritNicheCoordinateReveals::default());
        app.insert_resource(CombatClock { tick: 89 });
        app.insert_resource(GameplayActionQueue::default());
        app.insert_resource(AlchemyMockState::default());
        app.insert_resource(DroppedLootRegistry::default());
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
        app.insert_resource(CapturedMineralProbes::default());
        app.insert_resource(CombatClock::default());
        app.insert_resource(GameplayActionQueue::default());
        app.insert_resource(AlchemyMockState::default());
        app.insert_resource(DroppedLootRegistry::default());
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
        app.insert_resource(CapturedMineralProbes::default());
        app.insert_resource(CombatClock::default());
        app.insert_resource(GameplayActionQueue::default());
        app.insert_resource(AlchemyMockState::default());
        app.insert_resource(DroppedLootRegistry::default());
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

    #[test]
    fn qi_color_inspect_scope_requires_near_same_dimension_target() {
        assert_eq!(parse_qi_color_inspect_protocol_id("entity:42"), Some(42));
        assert_eq!(parse_qi_color_inspect_protocol_id("entity_bits:42"), None);
        assert_eq!(parse_qi_color_inspect_protocol_id("entity:bad"), None);

        assert!(is_qi_color_inspect_position_in_scope(
            DVec3::ZERO,
            DVec3::new(QI_COLOR_INSPECT_MAX_DISTANCE, 0.0, 0.0),
            true,
        ));
        assert!(!is_qi_color_inspect_position_in_scope(
            DVec3::ZERO,
            DVec3::new(QI_COLOR_INSPECT_MAX_DISTANCE + 0.01, 0.0, 0.0),
            true,
        ));
        assert!(!is_qi_color_inspect_position_in_scope(
            DVec3::ZERO,
            DVec3::new(1.0, 0.0, 0.0),
            false,
        ));
    }

    #[test]
    fn learn_skill_scroll_consumes_first_time_and_marks_consumed() {
        let mut app = App::new();
        app.insert_resource(CombatClock::default());
        app.insert_resource(GameplayActionQueue::default());
        app.insert_resource(AlchemyMockState::default());
        app.insert_resource(DroppedLootRegistry::default());
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
        app.insert_resource(CombatClock::default());
        app.insert_resource(GameplayActionQueue::default());
        app.insert_resource(AlchemyMockState::default());
        app.insert_resource(DroppedLootRegistry::default());
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
        app.insert_resource(CombatClock::default());
        app.insert_resource(GameplayActionQueue::default());
        app.insert_resource(AlchemyMockState::default());
        app.insert_resource(DroppedLootRegistry::default());
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

    #[test]
    fn forge_inscription_scroll_consumes_item_and_emits_event() {
        let mut app = App::new();
        app.insert_resource(CapturedInscriptionScrolls::default());
        app.insert_resource(CombatClock::default());
        app.insert_resource(GameplayActionQueue::default());
        app.insert_resource(AlchemyMockState::default());
        app.insert_resource(DroppedLootRegistry::default());
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
        assert!(inventory.containers[0].items.is_empty());
        let captured = app.world().resource::<CapturedInscriptionScrolls>();
        assert_eq!(captured.0.len(), 1);
        assert_eq!(captured.0[0].session, ForgeSessionId(9));
        assert_eq!(captured.0[0].inscription_id, "sharp_v0");
    }

    #[test]
    fn forge_inscription_scroll_rejects_invalid_session_before_consuming_item() {
        let mut app = App::new();
        app.insert_resource(CapturedInscriptionScrolls::default());
        app.insert_resource(CombatClock::default());
        app.insert_resource(GameplayActionQueue::default());
        app.insert_resource(AlchemyMockState::default());
        app.insert_resource(DroppedLootRegistry::default());
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
        app.insert_resource(CapturedTemperingHits::default());
        app.insert_resource(CombatClock::default());
        app.insert_resource(GameplayActionQueue::default());
        app.insert_resource(AlchemyMockState::default());
        app.insert_resource(DroppedLootRegistry::default());
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
        app.insert_resource(CapturedTemperingHits::default());
        app.insert_resource(CombatClock::default());
        app.insert_resource(GameplayActionQueue::default());
        app.insert_resource(AlchemyMockState::default());
        app.insert_resource(DroppedLootRegistry::default());
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
        app.insert_resource(CapturedConsecrationInjects::default());
        app.insert_resource(CombatClock::default());
        app.insert_resource(GameplayActionQueue::default());
        app.insert_resource(AlchemyMockState::default());
        app.insert_resource(DroppedLootRegistry::default());
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
        app.insert_resource(CapturedConsecrationInjects::default());
        app.insert_resource(CombatClock::default());
        app.insert_resource(GameplayActionQueue::default());
        app.insert_resource(AlchemyMockState::default());
        app.insert_resource(DroppedLootRegistry::default());
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
        app.insert_resource(CapturedStepAdvances::default());
        app.insert_resource(CombatClock::default());
        app.insert_resource(GameplayActionQueue::default());
        app.insert_resource(AlchemyMockState::default());
        app.insert_resource(DroppedLootRegistry::default());
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
    }

    #[test]
    fn forge_session_inputs_reject_wrong_caster() {
        let mut app = App::new();
        app.insert_resource(CapturedTemperingHits::default());
        app.insert_resource(CapturedConsecrationInjects::default());
        app.insert_resource(CapturedStepAdvances::default());
        app.insert_resource(CombatClock::default());
        app.insert_resource(GameplayActionQueue::default());
        app.insert_resource(AlchemyMockState::default());
        app.insert_resource(DroppedLootRegistry::default());
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
    fn skill_bar_cast_defined_skill_without_resolver_uses_generic_cast_path() {
        // body.guangbo_ticao 是仍未实装 resolver 的 skeleton 招（不在 SkillRegistry 内，
        // 无 required_meridians、无 SkillMeridianDependencies）→ 走通用施法路径，
        // 通用路径无条件插入 Casting 并把 SkillConfigStore 里的配置带入 Casting.skill_config。
        let mut app = App::new();
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
            channel: "Lung",
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
            channel: "Lung",
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
            channel: "Lung",
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
            channel: "Lung",
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
        skill_bar.set_cooldown(2, 100);
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

fn handle_use_quick_slot(
    entity: valence::prelude::Entity,
    slot: u8,
    clock: &CombatClock,
    commands: &mut Commands,
    clients: &mut Query<(&Username, &mut Client)>,
    combat_params: &mut CombatRequestParams,
    inventories: &Query<&mut PlayerInventory>,
) {
    if slot >= 9 {
        tracing::warn!(
            "[bong][network] use_quick_slot entity={entity:?} ignored: slot {slot} out of range"
        );
        return;
    }
    // plan §4.2: 已 cast 时——同来源同 slot 静默忽略；否则 UserCancel + 启新 cast。
    if let Ok(prev) = combat_params.casting_q.get(entity) {
        if prev.source == CastSource::QuickSlot && prev.slot == slot {
            tracing::debug!(
                "[bong][network] use_quick_slot entity={entity:?} slot={slot} ignored: same-slot during cast"
            );
            return;
        }
        let prev = CastCancelSnapshot::from(prev);
        cancel_previous_cast(entity, prev, clock, commands, clients, combat_params, slot);
        // 继续到下面启动新 cast。
    }
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
    entity: valence::prelude::Entity,
    slot: u8,
    item_id: Option<String>,
    bindings_q: &mut Query<&mut QuickSlotBindings>,
    inventories: &Query<&mut PlayerInventory>,
    clients: &Query<(&Username, &mut Client)>,
    persistence: Option<&PlayerStatePersistence>,
) {
    let mut bindings = match bindings_q.get_mut(entity) {
        Ok(b) => b,
        Err(_) => {
            tracing::warn!(
                "[bong][network] quick_slot_bind entity={entity:?} has no QuickSlotBindings"
            );
            return;
        }
    };
    // 把 item_id (template) 解析成实际持有的第一个 instance_id。
    // None / "" → 清空。Plan §10.4 wire 是 ItemId（template id），server 自己
    // 在 player inventory 里查匹配的 instance。
    let persisted_item_id = item_id.as_deref().filter(|item_id| !item_id.is_empty());
    let instance_id = match persisted_item_id {
        None => None,
        Some(template) => inventories
            .get(entity)
            .ok()
            .and_then(|inv| inventory_instance_id_by_template(inv, template)),
    };
    if !bindings.set(slot, instance_id) {
        tracing::warn!(
            "[bong][network] quick_slot_bind entity={entity:?} slot={slot} out of range"
        );
        return;
    }
    let persisted_item_id = persisted_item_id.map(str::to_string);
    if let (Some(persistence), Ok((username, _))) = (persistence, clients.get(entity)) {
        if let Err(error) = update_player_ui_prefs(persistence, username.0.as_str(), |prefs| {
            prefs.quick_slots[slot as usize] = persisted_item_id.clone()
        }) {
            tracing::warn!(
                "[bong][network] failed to persist quick_slot_bind for `{}` slot={slot}: {error}",
                username.0
            );
        }
    }
    tracing::info!(
        "[bong][network] quick_slot_bind entity={entity:?} slot={slot} item_id={:?} → instance={:?}",
        item_id,
        instance_id
    );
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
    let Some(definition) = technique_definition(&skill_id) else {
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
    let skill_fn = combat_params
        .skill_registry
        .as_deref()
        .and_then(|registry| registry.lookup(&skill_id));
    if combat_params
        .skillbar_bindings_q
        .get(entity)
        .map(|bindings| bindings.is_on_cooldown(slot, clock.tick))
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
                definition.required_meridians,
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
        cancel_previous_cast(entity, prev, clock, commands, clients, combat_params, slot);
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
            definition,
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
        && observer_position.distance_squared(observed_position)
            <= QI_COLOR_INSPECT_MAX_DISTANCE * QI_COLOR_INSPECT_MAX_DISTANCE
}

fn dimension_kind_for(dimensions: &Query<&CurrentDimension>, entity: Entity) -> DimensionKind {
    dimensions
        .get(entity)
        .map(|dimension| dimension.0)
        .unwrap_or_default()
}

fn resolve_trade_offer_target(raw: &str, combat_params: &CombatRequestParams) -> Option<Entity> {
    let raw = raw.trim();
    if raw.is_empty() || raw.starts_with("entity_bits:") {
        return None;
    }
    resolve_skill_cast_target(Some(raw), combat_params)
}

#[derive(Debug, Clone)]
struct NpcEngagementTarget {
    entity: Entity,
    archetype: NpcArchetype,
    reputation_to_player: i32,
    faction_reputation_tier: FactionReputationTier,
    display_name: String,
    greeting_text: String,
    position: DVec3,
    /// plan-territory-v1 P1: per-NPC per-player 信誉组件（Optional clone）。
    /// trade handler 读取时传入 player 的 canonical_player_id 叠加到 rep_f32。
    npc_player_rep: Option<NpcPlayerReputation>,
}

impl NpcEngagementTarget {
    fn can_trade(&self) -> bool {
        matches!(self.archetype, NpcArchetype::Rogue | NpcArchetype::Commoner)
            && self.faction_reputation_tier != FactionReputationTier::Wanted
            && self.reputation_to_player >= -30
    }
}

fn resolve_npc_engagement_target(
    player: Entity,
    npc_entity_id: i32,
    combat_params: &CombatRequestParams,
    npc_params: &NpcEngagementRequestParams,
    zone_registry: Option<&ZoneRegistry>,
) -> Option<NpcEngagementTarget> {
    let npc = combat_params
        .entity_manager
        .as_deref()
        .and_then(|manager| manager.get_by_id(npc_entity_id))?;
    if dimension_kind_for(&npc_params.dimensions, player)
        != dimension_kind_for(&npc_params.dimensions, npc)
    {
        return None;
    }
    let player_position = npc_params.positions.get(player).ok()?.get();
    let (npc_position, archetype, membership, cultivation, lifecycle, npc_player_rep) =
        npc_params.npcs.get(npc).ok()?;
    if lifecycle.is_some_and(|lifecycle| lifecycle.state == LifecycleState::Terminated) {
        return None;
    }
    let npc_position = npc_position.get();
    if player_position.distance_squared(npc_position)
        > NPC_INTERACTION_MAX_DISTANCE * NPC_INTERACTION_MAX_DISTANCE
    {
        return None;
    }
    let player_identities = npc_params.identities.get(player).ok();
    let player_faction_reputation = npc_params.faction_reputations.get(player).ok();
    let realm = cultivation
        .map(|cultivation| cultivation.realm)
        .unwrap_or(crate::cultivation::components::Realm::Awaken);
    let npc_dimension = dimension_kind_for(&npc_params.dimensions, npc);
    let npc_zone_name = zone_registry
        .and_then(|zones| zones.find_zone(npc_dimension, npc_position))
        .map(|zone| zone.name.as_str());
    let faction_reputation_tier = player_faction_reputation
        .and_then(|reputation| npc_zone_name.map(|zone| reputation.tier_for_zone(zone)))
        .unwrap_or(FactionReputationTier::Normal);
    Some(NpcEngagementTarget {
        entity: npc,
        archetype: *archetype,
        reputation_to_player: reputation_to_player_score_for_npc_zone(
            membership,
            player_identities,
            player_faction_reputation,
            npc_zone_name,
        ),
        faction_reputation_tier,
        display_name: npc_display_name(*archetype, realm, membership),
        greeting_text: greeting_text_for_archetype(*archetype).to_string(),
        position: npc_position,
        // plan-territory-v1 P1: clone 可选信誉组件，trade handler 中叠加霸主 rep 加成。
        npc_player_rep: npc_player_rep.cloned(),
    })
}

fn reputation_to_player_score_for_npc_zone(
    membership: Option<&FactionMembership>,
    player_identities: Option<&PlayerIdentities>,
    faction_reputation: Option<&FactionReputation>,
    zone_name: Option<&str>,
) -> i32 {
    let Some(faction_score) = faction_reputation.and_then(|reputation| {
        zone_name
            .and_then(faction_for_zone)
            .map(|faction| reputation.score(faction))
    }) else {
        return reputation_to_player_score_for_client(membership, player_identities);
    };
    let faction_baseline = membership
        .map(crate::network::npc_metadata::reputation_to_player_score)
        .unwrap_or_default();
    faction_baseline
        .saturating_add(faction_score)
        .clamp(-100, 100)
}

pub(crate) fn npc_trade_catalog_entry(
    archetype: NpcArchetype,
    requested_item_id: &str,
) -> Option<(&'static str, u64)> {
    match (archetype, requested_item_id.trim()) {
        (NpcArchetype::Commoner, "lingcao" | "spirit_grass") => Some(("spirit_grass", 10)),
        (NpcArchetype::Rogue, "lingcao" | "spirit_grass") => Some(("spirit_grass", 10)),
        (NpcArchetype::Rogue, "fragment_scroll" | "broken_artifact_scroll") => {
            Some(("broken_artifact_scroll", 40))
        }
        (NpcArchetype::Rogue, "skill_scroll_herbalism_baicao_can") => {
            Some(("skill_scroll_herbalism_baicao_can", 30))
        }
        // plan-cultivation-pacing-v1 P2.2：NPC 售卖低品质修炼丹药。
        // Commoner/Rogue 均可购买次品灵息丸（8 骨币）和次品聚灵丹（15 骨币），
        // 效果 ×0.6，引导玩家自炼正品。
        (
            NpcArchetype::Commoner | NpcArchetype::Rogue,
            "ling_xi_wan_flawed" | "ling_xi_wan_次品",
        ) => Some(("ling_xi_wan_flawed", 8)),
        (
            NpcArchetype::Commoner | NpcArchetype::Rogue,
            "ju_ling_dan_flawed" | "ju_ling_dan_次品",
        ) => Some(("ju_ling_dan_flawed", 15)),
        _ => None,
    }
}

fn send_npc_interaction_feedback(
    player: Entity,
    clients: &mut Query<(&Username, &mut Client)>,
    message: impl Into<String>,
) {
    let Ok((_, mut client)) = clients.get_mut(player) else {
        return;
    };
    client.send_chat_message(message.into());
}

fn emit_npc_refuse_audio(
    audio_events: &mut Option<ResMut<Events<PlaySoundRecipeRequest>>>,
    player: Entity,
    position: DVec3,
) {
    let Some(audio_events) = audio_events.as_mut() else {
        return;
    };
    audio_events.send(PlaySoundRecipeRequest {
        recipe_id: "npc_refuse".to_string(),
        instance_id: 0,
        pos: Some([
            position.x.floor() as i32,
            position.y.floor() as i32,
            position.z.floor() as i32,
        ]),
        flag: None,
        volume_mul: 1.0,
        pitch_shift: 0.0,
        recipient: AudioRecipient::Single(player),
    });
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

fn cancel_previous_cast(
    entity: valence::prelude::Entity,
    prev: CastCancelSnapshot,
    clock: &CombatClock,
    commands: &mut Commands,
    clients: &mut Query<(&Username, &mut Client)>,
    combat_params: &mut CombatRequestParams,
    next_slot: u8,
) {
    let prev_source = prev.source;
    let prev_slot = prev.slot;
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
        CastSource::SkillBar => {
            if let Ok(mut bindings) = combat_params.skillbar_bindings_q.get_mut(entity) {
                bindings.set_cooldown(
                    prev_slot,
                    clock.tick.saturating_add(CAST_INTERRUPT_COOLDOWN_TICKS),
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

#[derive(Debug, Clone, Copy)]
struct CastCancelSnapshot {
    source: CastSource,
    slot: u8,
    duration_ms: u32,
    started_at_ms: u64,
}

impl From<&Casting> for CastCancelSnapshot {
    fn from(casting: &Casting) -> Self {
        Self {
            source: casting.source,
            slot: casting.slot,
            duration_ms: casting.duration_ms,
            started_at_ms: casting.started_at_ms,
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
            if technique_definition(skill_id).is_none() {
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
fn handle_inventory_move(
    entity: valence::prelude::Entity,
    instance_id: u64,
    from: InventoryLocationV1,
    to: InventoryLocationV1,
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

    match apply_inventory_move(&mut inventory, item_registry, instance_id, &from, &to) {
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

fn resync_snapshot(
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
fn handle_inventory_discard(
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
fn handle_pickup_dropped_item(
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

    match pickup_dropped_loot_instance(
        &mut inventory,
        dropped_loot_registry,
        player_pos,
        instance_id,
    ) {
        Ok(revision) => {
            tracing::info!(
                "[bong][network][inventory] picked up dropped instance={instance_id} revision={}",
                revision.0
            );

            // plan-qi-handling-attrition-v1 P0: Pickup 磨损，逸散守恒归还 zone
            if let Some(zones) = zones {
                let dim = dimensions
                    .get(entity)
                    .map(|d| d.0)
                    .unwrap_or(DimensionKind::Overworld);
                let pos = valence::prelude::DVec3::new(player_pos[0], player_pos[1], player_pos[2]);
                // find_zone 借不可变引用获取 zone_name，再 find_zone_mut 借可变引用
                let zone_name = zones.find_zone(dim, pos).map(|z| z.name.clone());
                if let Some(zone_name) = zone_name {
                    if let Some(zone) = zones.find_zone_mut(&zone_name) {
                        // 在 inventory 里按 instance_id 找到拾起的 item 并应用磨损
                        let target_container_exempt = inventory_instance_container_attrition_exempt(
                            &inventory,
                            item_registry,
                            instance_id,
                        );
                        if let Some(item) =
                            inventory_item_by_instance_mut(&mut inventory, instance_id)
                        {
                            if !target_container_exempt && !is_attrition_exempt(item) {
                                let before_abs_qi = item_abs_qi_for_attrition(item);
                                apply_attrition_checked(
                                    item,
                                    AttritionOpKind::Pickup,
                                    Some(zone),
                                    qi_transfers,
                                    tsy_lifecycle,
                                );
                                emit_attrition_applied_if_lost(
                                    attrition_events,
                                    entity,
                                    item,
                                    before_abs_qi,
                                    player_pos,
                                );
                            }
                        }
                    }
                }
            }

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

fn handle_alchemy_turn_page(
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

fn handle_alchemy_learn(
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

fn handle_alchemy_open_furnace(
    entity: valence::prelude::Entity,
    furnace_pos: (i32, i32, i32),
    clients: &mut Query<(&Username, &mut Client)>,
    furnaces: &mut Query<(Entity, &mut AlchemyFurnace)>,
    learned_q: &mut Query<&mut LearnedRecipes>,
) {
    let Ok((username, mut client)) = clients.get_mut(entity) else {
        return;
    };
    let player_id = canonical_player_id(username.0.as_str());
    match with_owned_furnace_mut(entity, &player_id, furnace_pos, furnaces, |furnace| {
        alchemy_snapshot_emit::send_furnace_from_furnace(&mut client, &player_id, furnace);
        alchemy_snapshot_emit::send_session_from_furnace(&mut client, &player_id, furnace);
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
fn handle_alchemy_intervention(
    entity: valence::prelude::Entity,
    furnace_pos: (i32, i32, i32),
    intervention: Intervention,
    clients: &mut Query<(&Username, &mut Client)>,
    furnaces: &mut Query<(Entity, &mut AlchemyFurnace)>,
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
        alchemy_snapshot_emit::send_session_from_furnace(&mut client, &player_id, furnace);
    });
    log_or_send_route_error(result, &mut client, &player_id, furnace_pos, "intervention");
}

#[allow(clippy::too_many_arguments)]
fn handle_alchemy_ignite(
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
        alchemy_snapshot_emit::send_session_from_furnace(&mut client, &player_id, furnace);
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
fn handle_alchemy_feed_slot(
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
        alchemy_snapshot_emit::send_session_from_furnace(&mut client, &player_id, furnace);
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
fn handle_alchemy_take_back(
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
                    let Some(instance_allocator) = instance_allocator else {
                        send_alchemy_error(
                            &mut client,
                            &player_id,
                            "炼丹产物入袋失败：实例编号器未就绪".to_string(),
                        );
                        return;
                    };
                    let granted = grant_alchemy_outcome_item(
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
                    if !granted {
                        return;
                    }
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
            tracing::info!(
                "[bong][network][alchemy] `{player_id}` take_back pos={furnace_pos:?} slot={slot_idx} resolved bucket={bucket:?}"
            );
            alchemy_snapshot_emit::send_furnace_from_furnace(&mut client, &player_id, furnace);
            alchemy_snapshot_emit::send_session_from_furnace(&mut client, &player_id, furnace);
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
fn handle_alchemy_take_pill(
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
fn handle_external_container_move(
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
    _commands: &mut Commands,
) {
    use crate::inventory::external_container::{
        place_item_into_container, remove_item_from_container,
    };
    use crate::network::inventory_snapshot_emit::item_view_from_instance;
    use crate::schema::inventory::{InventoryLocationV1, PlacedInventoryItemV1};
    use crate::schema::server_data::{LootContainerUpdateV1, ServerDataPayloadV1, ServerDataV1};

    let Some(ext_reg) = dispatch.ext_container_registry.as_deref_mut() else {
        tracing::warn!("[bong][network] external_container_move: registry missing");
        return;
    };

    let Some(&coffin_entity) = ext_reg.sessions.get(&session_id) else {
        tracing::warn!(
            "[bong][network] external_container_move: unknown session {session_id} from {player_entity:?}"
        );
        return;
    };

    let Ok(mut ext) = combat_params.ext_containers.get_mut(coffin_entity) else {
        tracing::warn!(
            "[bong][network] external_container_move: ExternalContainer component missing on {coffin_entity:?}"
        );
        return;
    };

    if ext.opened_by != Some(player_entity) {
        tracing::warn!(
            "[bong][network] external_container_move: session {session_id} not owned by {player_entity:?}"
        );
        return;
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

        let mut found_item = None;
        for container in inventory.containers.iter_mut() {
            if let Some(idx) = container
                .items
                .iter()
                .position(|p| p.instance.instance_id == instance_id)
            {
                found_item = Some(container.items.remove(idx));
                break;
            }
        }

        let Some(removed) = found_item else {
            tracing::warn!(
                "[bong][network] external_container_move: instance {instance_id} not in player inventory"
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
fn handle_external_container_close(
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
/// 3. 消耗丹（inventory 真删）；
/// 4. 读取 ItemEffect::QiRecovery { amount } 作为 qi_gain（默认 24.0）；
/// 5. emit `GiveDanToElderIntent` 供 `dying_elder_give_dan_system` 在下一 tick 处理
///    真元转移（解耦网络层与守恒系统）；
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
    item_registry: &ItemRegistry,
    entity_manager: Option<&valence::prelude::EntityManager>,
    clients: &mut Query<(&Username, &mut Client)>,
    give_dan_tx: Option<&mut Events<crate::fauna::dying_elder::GiveDanToElderIntent>>,
) {
    use crate::fauna::dying_elder::GiveDanToElderIntent;
    use crate::inventory::ItemEffect;

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

    // ── 获取丹携带的 qi_gain（从 ItemEffect::QiRecovery，默认 24.0）────────
    let qi_gain = item_registry
        .get("huiyuan_pill")
        .and_then(|tmpl| {
            if let Some(ItemEffect::QiRecovery { amount }) = tmpl.effect {
                Some(amount)
            } else {
                None
            }
        })
        .unwrap_or(24.0); // fallback to canonical value

    // ── 解析大能 entity ────────────────────────────────────────────────────
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

    // ── 消耗丹（inventory 真删）───────────────────────────────────────────
    {
        let Ok(mut inventory) = inventories.get_mut(player_entity) else {
            return;
        };
        match crate::inventory::consume_item_instance_once(&mut inventory, pill_instance_id) {
            Ok(_) => {}
            Err(e) => {
                tracing::warn!(
                    "[bong][dying_elder] give_dan: consume_item_instance_once failed: {e}"
                );
                return;
            }
        }
    }

    // ── emit GiveDanToElderIntent 供 dying_elder_give_dan_system 处理 ────────
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
        qi_gain,
    });

    tracing::info!(
        "[bong][dying_elder] give_dan: player {player_entity:?} → elder {elder_entity:?} qi_gain={qi_gain} pill={pill_instance_id}"
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
        app.insert_resource(CapturedFreshnessProbes::default());
        app.insert_resource(CombatClock { tick: 42 });
        app.insert_resource(GameplayActionQueue::default());
        app.insert_resource(AlchemyMockState::default());
        app.insert_resource(DroppedLootRegistry::default());
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
        app.insert_resource(CapturedRaiseShieldIntents::default());
        app.insert_resource(CapturedLowerShieldIntents::default());
        app.insert_resource(CombatClock::default());
        app.insert_resource(GameplayActionQueue::default());
        app.insert_resource(AlchemyMockState::default());
        app.insert_resource(DroppedLootRegistry::default());
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
