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
    Events, Query, Res, ResMut, Resource, Username, With,
};

use crate::alchemy::residue::{residue_alchemy_data, residue_kind_for_recyclable_outcome};
use crate::alchemy::{
    learned::LearnResult, AlchemyFurnace, AlchemySession, Intervention, LearnedRecipes,
    PlaceFurnaceRequest, RecipeRegistry, MIN_ZONE_QI_TO_ALCHEMY,
};
use crate::combat::anqi_v2::{cycle_container_slot, switch_container_slot};
use crate::combat::carrier::{CarrierSlot, ChargeCarrierIntent, ThrowCarrierIntent};
use crate::combat::components::{
    CastSource, Casting, Lifecycle, LifecycleState, QuickSlotBindings, SkillBarBindings, SkillSlot,
    Wounds,
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
use crate::cultivation::components::{recover_current_qi, Cultivation};
use crate::cultivation::dugu::SelfAntidoteIntent;
use crate::cultivation::forging::ForgeRequest;
use crate::cultivation::insight::{InsightChosen, InsightRequest};
use crate::cultivation::known_techniques::{technique_definition, TechniqueDefinition};
use crate::cultivation::lifespan::LifespanExtensionIntent;
use crate::cultivation::meridian_open::MeridianTarget;
use crate::cultivation::possession::{DuoSheRequestEvent, UseLifeCoreEvent};
use crate::cultivation::skill_registry::{CastResult, SkillRegistry};
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
    fully_repair_weapon_instance, inventory_item_by_instance_borrow, pickup_dropped_loot_instance,
    DroppedLootRegistry, InventoryDurabilityChangedEvent, InventoryInstanceIdAllocator,
    InventoryMoveOutcome, ItemInstance, PlayerInventory, FRONT_SATCHEL_CONTAINER_ID,
    MAIN_PACK_CONTAINER_ID, SMALL_POUCH_CONTAINER_ID,
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
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::alchemy_bridge::alchemy_session_id;
use crate::network::alchemy_snapshot_emit;
use crate::network::audio_event_emit::{AudioRecipient, PlaySoundRecipeRequest};
use crate::network::cast_emit::{
    apply_item_effect, current_unix_millis, push_cast_sync, CAST_INTERRUPT_COOLDOWN_TICKS,
};
// dropped_loot_sync is emitted by dropped_loot_sync_emit.
use crate::identity::PlayerIdentities;
use crate::network::inventory_snapshot_emit::send_inventory_snapshot_to_client;
use crate::network::npc_metadata::{
    display_name as npc_display_name, greeting_text_for_archetype,
    reputation_to_player_score_for_client,
};
use crate::network::qi_color_observed_emit::QiColorInspectRequest;
use crate::network::send_server_data_payload;
use crate::network::skill_config_emit::send_skill_config_snapshot_to_client;
use crate::network::skill_snapshot_emit::send_skill_snapshot_to_client;
use crate::network::{
    gameplay_vfx, redis_bridge::RedisOutbound, vfx_event_emit::VfxEventRequest, RedisBridgeResource,
};
use crate::npc::faction::FactionMembership;
use crate::npc::lifecycle::NpcArchetype;
use crate::npc::spawn::NpcMarker;
use crate::player::gameplay::{GameplayAction, GameplayActionQueue, GatherAction};
use crate::player::state::{
    canonical_player_id, update_player_ui_prefs, PlayerState, PlayerStatePersistence,
};
use crate::qi_physics::constants::QI_TARGETED_ITEM_WEAR_WEIGHT_THRESHOLD;
use crate::qi_physics::qi_targeted_item_wear_fraction;
use crate::qi_physics::AnqiContainerKind;
use crate::schema::alchemy::{AlchemyInterventionResultV1, AlchemySessionStartV1};
use crate::schema::client_request::{ClientRequestV1, SkillBarBindingV1};
use crate::schema::combat_hud::{CastOutcomeV1, CastPhaseV1, CastSyncV1};
use crate::schema::inventory::{ContainerIdV1, EquipSlotV1, InventoryEventV1, InventoryLocationV1};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
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
use crate::social::events::{
    SparringInviteResponseEvent, SparringInviteResponseKind, SpiritNicheActivateGuardianRequest,
    SpiritNicheCoordinateRevealRequest, SpiritNichePlaceRequest, SpiritNicheRevealSource,
    TradeOfferRequest, TradeOfferResponseEvent,
};
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
use crate::world::zone::{ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};
use crate::zhenfa::{ZhenfaDisarmRequest, ZhenfaPlaceRequest, ZhenfaTriggerRequest};

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
    pub spoil_warnings: Option<ResMut<'w, Events<SpoilConsumeWarning>>>,
    pub age_bonus_rolls: Option<ResMut<'w, Events<AgeBonusRoll>>>,
    pub season_state: Option<Res<'w, WorldSeasonState>>,
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
    pub place_furnace_tx: EventWriter<'w, PlaceFurnaceRequest>,
    pub outcome_tx: Option<ResMut<'w, Events<crate::alchemy::AlchemyOutcomeEvent>>>,
    pub item_registry: Res<'w, ItemRegistry>,
    pub instance_allocator: Option<ResMut<'w, InventoryInstanceIdAllocator>>,
    pub redis: Option<Res<'w, RedisBridgeResource>>,
    pub zones: Option<Res<'w, ZoneRegistry>>,
    pub vfx_events: Option<ResMut<'w, Events<VfxEventRequest>>>,
}

#[derive(SystemParam)]
pub struct ClientRequestDispatchParams<'w> {
    pub gameplay_queue: Option<valence::prelude::ResMut<'w, GameplayActionQueue>>,
    pub breakthrough_tx: EventWriter<'w, BreakthroughRequest>,
    pub start_du_xu_tx: Option<ResMut<'w, Events<StartDuXuRequest>>>,
    pub void_action_tx: Option<ResMut<'w, Events<VoidActionIntent>>>,
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
    pub spirit_niche_coordinate_reveal_tx:
        Option<ResMut<'w, Events<SpiritNicheCoordinateRevealRequest>>>,
    pub spirit_niche_activate_guardian_tx:
        Option<ResMut<'w, Events<SpiritNicheActivateGuardianRequest>>>,
    pub coffin_open_tx: Option<ResMut<'w, Events<CoffinOpenRequest>>>,
    pub sparring_invite_response_tx: Option<ResMut<'w, Events<SparringInviteResponseEvent>>>,
    pub trade_offer_request_tx: Option<ResMut<'w, Events<TradeOfferRequest>>>,
    pub trade_offer_response_tx: Option<ResMut<'w, Events<TradeOfferResponseEvent>>>,
    pub zhenfa_place_tx: Option<ResMut<'w, Events<ZhenfaPlaceRequest>>>,
    pub zhenfa_trigger_tx: Option<ResMut<'w, Events<ZhenfaTriggerRequest>>>,
    pub zhenfa_disarm_tx: Option<ResMut<'w, Events<ZhenfaDisarmRequest>>>,
    pub charge_carrier_tx: Option<ResMut<'w, Events<ChargeCarrierIntent>>>,
    pub throw_carrier_tx: Option<ResMut<'w, Events<ThrowCarrierIntent>>>,
    // ─── plan-craft-v1 P2：通用手搓 intent ──────────────────
    pub craft_start_tx: Option<ResMut<'w, Events<crate::craft::CraftStartIntent>>>,
    pub craft_cancel_tx: Option<ResMut<'w, Events<crate::craft::CraftCancelIntent>>>,
}

#[derive(SystemParam)]
pub struct SkillScrollRequestParams<'w, 's> {
    pub skill_xp_tx: Option<ResMut<'w, Events<SkillXpGain>>>,
    pub skill_scroll_used_tx: Option<ResMut<'w, Events<SkillScrollUsed>>>,
    pub mineral_probe_tx: Option<ResMut<'w, Events<MineralProbeIntent>>>,
    pub skill_sets: Query<'w, 's, &'static mut SkillSet>,
    pub learned_blueprints: Query<'w, 's, &'static mut LearnedBlueprints>,
    pub cultivations: Query<'w, 's, &'static Cultivation>,
    pub positions: Query<'w, 's, &'static valence::prelude::Position>,
    pub dimensions: Query<'w, 's, &'static CurrentDimension>,
    pub inscription_scroll_tx: Option<ResMut<'w, Events<InscriptionScrollSubmit>>>,
    pub forge_sessions: Option<Res<'w, ForgeSessions>>,
}

type NpcEngagementItem = (
    &'static valence::prelude::Position,
    &'static NpcArchetype,
    Option<&'static FactionMembership>,
    Option<&'static Cultivation>,
    Option<&'static Lifecycle>,
);

#[derive(SystemParam)]
pub struct NpcEngagementRequestParams<'w, 's> {
    pub npcs: Query<'w, 's, NpcEngagementItem, With<NpcMarker>>,
    pub positions: Query<'w, 's, &'static valence::prelude::Position>,
    pub dimensions: Query<'w, 's, &'static CurrentDimension>,
    pub identities: Query<'w, 's, &'static PlayerIdentities, With<Client>>,
    pub audio_events: Option<ResMut<'w, Events<PlaySoundRecipeRequest>>>,
}

const CHANNEL: &str = "bong:client_request";
const SUPPORTED_VERSION: u8 = 1;
const QI_COLOR_INSPECT_MAX_DISTANCE: f64 = 6.0;
const NPC_INTERACTION_MAX_DISTANCE: f64 = 6.0;
/// plan-cultivation-v1 §3.1：服用突破辅助丹药的 buff 持续时间（5 分钟）。
/// 20 tick/s × 60 s × 5 = 6000。
const BREAKTHROUGH_BOOST_DURATION_TICKS: u64 = 6_000;

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
            | ClientRequestV1::AlchemyTakePill { v, .. }
            | ClientRequestV1::AlchemyFurnacePlace { v, .. }
            | ClientRequestV1::CoffinOpen { v, .. }
            | ClientRequestV1::SpiritNichePlace { v, .. }
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
            | ClientRequestV1::LearnSkillScroll { v, .. }
            | ClientRequestV1::InventoryMoveIntent { v, .. }
            | ClientRequestV1::EquipFalseSkin { v, .. }
            | ClientRequestV1::ForgeFalseSkin { v, .. }
            | ClientRequestV1::InventoryDiscardItem { v, .. }
            | ClientRequestV1::DropWeaponIntent { v, .. }
            | ClientRequestV1::RepairWeaponIntent { v, .. }
            | ClientRequestV1::PickupDroppedItem { v, .. }
            | ClientRequestV1::MineralProbe { v, .. }
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
            | ClientRequestV1::CraftCancel { v } => *v,
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
                let price = match crate::npc::scattered_cultivator::trade_price_for_reputation(
                    base_price,
                    target.reputation_to_player,
                ) {
                    Ok(price) => price,
                    Err(_) => {
                        let attack_hint =
                            if crate::npc::scattered_cultivator::should_attack_for_reputation(
                                target.reputation_to_player,
                            ) {
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
            ClientRequestV1::LearnSkillScroll { instance_id, .. } => {
                handle_learn_skill_scroll(
                    ev.client,
                    instance_id,
                    &mut inventories,
                    &mut clients,
                    &player_states,
                    &mut skill_scroll_params,
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
                );
            }
            ClientRequestV1::EquipFalseSkin {
                slot,
                item_instance_id,
                ..
            } => {
                if slot != EquipSlotV1::FalseSkin {
                    tracing::warn!(
                        "[bong][network][tuike] equip_false_skin rejected: slot={slot:?} item_instance_id={item_instance_id}"
                    );
                    continue;
                }
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
                        slot: EquipSlotV1::FalseSkin,
                    },
                    &combat_params.item_registry,
                    &mut inventories,
                    &mut clients,
                    &player_states,
                    &skill_scroll_params.cultivations,
                    karma_weights.as_deref(),
                    durability_changed_tx.as_deref_mut(),
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
                    &mut clients,
                    &player_states,
                    &skill_scroll_params.cultivations,
                    &dropped_loot_params.positions,
                    &skill_scroll_params.dimensions,
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
                    &mut clients,
                    &player_states,
                    &skill_scroll_params.cultivations,
                    &dropped_loot_params.positions,
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
                start_extract_tx.send(StartExtractRequestEvent {
                    player: ev.client,
                    portal: Entity::from_bits(portal_entity_id),
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
                start_search_tx.send(StartSearchRequestEvent {
                    player: ev.client,
                    container: Entity::from_bits(container_entity_id),
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
) {
    let Some((skill, scroll_id, xp_grant)) = ({
        let inventory = match inventories.get(entity) {
            Ok(inv) => inv,
            Err(_) => return,
        };
        let instance = match inventory_item_by_instance_borrow(inventory, instance_id) {
            Some(instance) => instance,
            None => return,
        };
        skill_scroll_spec(instance.template_id.as_str())
            .map(|(skill, xp_grant)| (skill, ScrollId::new(instance.template_id.clone()), xp_grant))
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
    for item in inventory.equipped.values() {
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
    use crate::skill::components::SkillSet;
    use valence::prelude::{
        ident, App, DVec3, EventReader, IntoSystemConfigs, Position, ResMut, Update,
    };
    use valence::protocol::packets::play::CustomPayloadS2c;
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

    #[derive(Default)]
    struct CapturedSpiritNichePlaces(Vec<SpiritNichePlaceRequest>);

    impl valence::prelude::Resource for CapturedSpiritNichePlaces {}

    #[derive(Default)]
    struct CapturedSpiritNicheCoordinateReveals(Vec<SpiritNicheCoordinateRevealRequest>);

    impl valence::prelude::Resource for CapturedSpiritNicheCoordinateReveals {}

    #[derive(Default)]
    struct CapturedCoffinOpenRequests(Vec<CoffinOpenRequest>);

    impl valence::prelude::Resource for CapturedCoffinOpenRequests {}

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
                },
            ),
            (
                "inscription_scroll_sharp_v0".to_string(),
                ItemTemplate {
                    id: "inscription_scroll_sharp_v0".to_string(),
                    display_name: "锐意铭文残卷".to_string(),
                    category: ItemCategory::Misc,
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
                },
            ),
        ]))
    }

    fn inventory_with_skill_scroll(item: ItemInstance) -> PlayerInventory {
        PlayerInventory {
            revision: InventoryRevision(0),
            containers: vec![ContainerState {
                id: "main_pack".into(),
                name: "main_pack".into(),
                rows: 5,
                cols: 7,
                items: vec![PlacedItemState {
                    row: 0,
                    col: 0,
                    instance: item,
                }],
            }],
            equipped: Default::default(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 50.0,
        }
    }

    fn inventory_with_stack(template_id: &str, count: u32) -> PlayerInventory {
        PlayerInventory {
            revision: InventoryRevision(0),
            containers: vec![ContainerState {
                id: "main_pack".into(),
                name: "main_pack".into(),
                rows: 5,
                cols: 7,
                items: vec![PlacedItemState {
                    row: 0,
                    col: 0,
                    instance: ItemInstance {
                        instance_id: 9001,
                        template_id: template_id.to_string(),
                        display_name: template_id.to_string(),
                        grid_w: 1,
                        grid_h: 1,
                        weight: 0.1,
                        rarity: ItemRarity::Common,
                        description: String::new(),
                        stack_count: count,
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
                }],
            }],
            equipped: Default::default(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 50.0,
        }
    }

    fn empty_inventory() -> PlayerInventory {
        PlayerInventory {
            revision: InventoryRevision(0),
            containers: vec![ContainerState {
                id: "main_pack".into(),
                name: "main_pack".into(),
                rows: 5,
                cols: 7,
                items: Vec::new(),
            }],
            equipped: Default::default(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 50.0,
        }
    }

    fn inventory_with_item(item: ItemInstance) -> PlayerInventory {
        PlayerInventory {
            revision: InventoryRevision(0),
            containers: vec![ContainerState {
                id: "main_pack".into(),
                name: "main_pack".into(),
                rows: 5,
                cols: 7,
                items: vec![PlacedItemState {
                    row: 0,
                    col: 0,
                    instance: item,
                }],
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
        app.add_event::<SpiritNichePlaceRequest>();
        app.add_event::<SpiritNicheCoordinateRevealRequest>();
        app.add_event::<CoffinOpenRequest>();
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
        app.add_event::<SkillXpGain>();
        app.add_event::<SkillScrollUsed>();
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
    fn inventory_move_applies_hidden_targeted_wear_to_spiritual_item() {
        let mut app = App::new();
        register_request_app(&mut app);
        app.insert_resource(ItemRegistry::from_map(HashMap::from([(
            "spiritual_ore".to_string(),
            ItemTemplate {
                id: "spiritual_ore".to_string(),
                display_name: "灵矿".to_string(),
                category: ItemCategory::Misc,
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

        let (client_bundle, _helper) = create_mock_client("Azure");
        let mut skill_bar = SkillBarBindings::default();
        assert!(skill_bar.set(
            0,
            SkillSlot::Skill {
                skill_id: "burst_meridian.tie_shan_kao".to_string(),
            },
        ));
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut().entity_mut(entity).insert((
            Position::new([0.0, 0.0, 0.0]),
            skill_bar,
            QuickSlotBindings::default(),
            empty_inventory(),
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
        assert_eq!(casting.duration_ticks, 10);
        assert_eq!(casting.complete_cooldown_ticks, 70);
        assert_eq!(
            casting.skill_id.as_deref(),
            Some("burst_meridian.tie_shan_kao")
        );
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
        app.world_mut().entity_mut(entity).insert((
            Position::new([0.0, 0.0, 0.0]),
            skill_bar,
            QuickSlotBindings::default(),
            empty_inventory(),
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
        .and_then(|inv| {
            for c in &inv.containers {
                if let Some(p) = c
                    .items
                    .iter()
                    .find(|p| p.instance.instance_id == instance_id)
                {
                    return Some(p.instance.template_id.clone());
                }
            }
            inv.hotbar
                .iter()
                .flatten()
                .find(|i| i.instance_id == instance_id)
                .map(|i| i.template_id.clone())
        })
        .and_then(|template_id| combat_params.item_registry.get(&template_id).cloned())
        .map(|t| (t.cast_duration_ms, t.cooldown_ms))
        .unwrap_or((TEMPLATE_DEFAULT_CAST_MS, TEMPLATE_DEFAULT_COOLDOWN_MS));
    // 50ms / tick；进 1 至少跑 1 tick，避免 0 时长 cast。
    let duration_ticks = u64::from(duration_ms).div_ceil(50).max(1);
    let complete_cooldown_ticks = u64::from(cooldown_ms).div_ceil(50).max(1);
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
        .any(|item| item.instance_id == instance_id)
    {
        return true;
    }
    inv.hotbar
        .iter()
        .flatten()
        .any(|item| item.instance_id == instance_id)
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
        Some(template) => inventories.get(entity).ok().and_then(|inv| {
            for c in &inv.containers {
                if let Some(p) = c.items.iter().find(|p| p.instance.template_id == template) {
                    return Some(p.instance.instance_id);
                }
            }
            inv.hotbar
                .iter()
                .flatten()
                .find(|i| i.template_id == template)
                .map(|i| i.instance_id)
        }),
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
    let duration_ms = definition.cast_ticks.saturating_mul(50);
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
    id.parse::<u64>().ok().map(Entity::from_bits)
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
    archetype: NpcArchetype,
    reputation_to_player: i32,
    display_name: String,
    greeting_text: String,
    position: DVec3,
}

impl NpcEngagementTarget {
    fn can_trade(&self) -> bool {
        matches!(self.archetype, NpcArchetype::Rogue | NpcArchetype::Commoner)
            && self.reputation_to_player >= -30
    }
}

fn resolve_npc_engagement_target(
    player: Entity,
    npc_entity_id: i32,
    combat_params: &CombatRequestParams,
    npc_params: &NpcEngagementRequestParams,
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
    let (npc_position, archetype, membership, cultivation, lifecycle) =
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
    let realm = cultivation
        .map(|cultivation| cultivation.realm)
        .unwrap_or(crate::cultivation::components::Realm::Awaken);
    Some(NpcEngagementTarget {
        archetype: *archetype,
        reputation_to_player: reputation_to_player_score_for_client(membership, player_identities),
        display_name: npc_display_name(*archetype, realm, membership),
        greeting_text: greeting_text_for_archetype(*archetype).to_string(),
        position: npc_position,
    })
}

fn npc_trade_catalog_entry(
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

fn handle_skill_bar_bind(
    entity: valence::prelude::Entity,
    slot: u8,
    binding: Option<SkillBarBindingV1>,
    bindings_q: &mut Query<&mut SkillBarBindings>,
    inventories: &Query<&mut PlayerInventory>,
    clients: &Query<(&Username, &mut Client)>,
    persistence: Option<&PlayerStatePersistence>,
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

    for (slot, item) in &inventory.equipped {
        if item.instance_id == instance_id {
            return equip_slot_v1_for_runtime(slot).map(|slot| InventoryLocationV1::Equip { slot });
        }
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
    match id {
        MAIN_PACK_CONTAINER_ID => Some(ContainerIdV1::MainPack),
        SMALL_POUCH_CONTAINER_ID => Some(ContainerIdV1::SmallPouch),
        FRONT_SATCHEL_CONTAINER_ID => Some(ContainerIdV1::FrontSatchel),
        _ => None,
    }
}

fn equip_slot_v1_for_runtime(slot: &str) -> Option<EquipSlotV1> {
    match slot {
        crate::inventory::EQUIP_SLOT_HEAD => Some(EquipSlotV1::Head),
        crate::inventory::EQUIP_SLOT_CHEST => Some(EquipSlotV1::Chest),
        crate::inventory::EQUIP_SLOT_LEGS => Some(EquipSlotV1::Legs),
        crate::inventory::EQUIP_SLOT_FEET => Some(EquipSlotV1::Feet),
        crate::inventory::EQUIP_SLOT_FALSE_SKIN => Some(EquipSlotV1::FalseSkin),
        crate::inventory::EQUIP_SLOT_MAIN_HAND => Some(EquipSlotV1::MainHand),
        crate::inventory::EQUIP_SLOT_OFF_HAND => Some(EquipSlotV1::OffHand),
        crate::inventory::EQUIP_SLOT_TWO_HAND => Some(EquipSlotV1::TwoHand),
        crate::inventory::EQUIP_SLOT_TREASURE_BELT_0 => Some(EquipSlotV1::TreasureBelt0),
        crate::inventory::EQUIP_SLOT_TREASURE_BELT_1 => Some(EquipSlotV1::TreasureBelt1),
        crate::inventory::EQUIP_SLOT_TREASURE_BELT_2 => Some(EquipSlotV1::TreasureBelt2),
        crate::inventory::EQUIP_SLOT_TREASURE_BELT_3 => Some(EquipSlotV1::TreasureBelt3),
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
        slot: EquipSlotV1::FalseSkin,
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
                tracing::warn!(
                    "[bong][network][tuike] rejected false_skin equip entity={entity:?} instance={instance_id}: realm too low for {:?}",
                    kind
                );
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
    clients: &mut Query<(&Username, &mut Client)>,
    player_states: &Query<&PlayerState>,
    cultivations: &Query<&Cultivation>,
    positions: &Query<&valence::prelude::Position>,
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
            for _ in 0..delta.unsigned_abs() {
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
        if !inventory_has_template_count(&inventory, material.as_str(), count) {
            send_alchemy_error(
                &mut client,
                &player_id,
                format!("材料不足：{material}×{count}"),
            );
            return;
        }
        if let Err(error) =
            session.feed_stage(recipe, slot_idx as usize, &[(material.clone(), count, 1.0)])
        {
            send_alchemy_error(&mut client, &player_id, format!("投料失败：{error}"));
            return;
        }
        for _ in 0..count {
            let consumed = consume_one_by_template(&mut inventory, material.as_str());
            debug_assert!(
                consumed,
                "inventory_has_template_count checked availability first"
            );
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
            let resolved = crate::alchemy::resolver::resolve_with_meta(&ended, recipe, registry);
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
        ItemEffect::MeridianHeal { .. } | ItemEffect::ContaminationCleanse { .. } => {
            let meridians = combat_params.meridians.get_mut(entity).ok();
            let contamination = combat_params.contaminations.get_mut(entity).ok();
            apply_item_effect(
                &effect,
                None,
                meridians,
                contamination,
                pill_item_id,
                entity,
            );
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

/// 扣除一颗 template 匹配的 item（优先 hotbar → containers → equipped）。
/// stack_count > 1 时减 1；否则移除整个 slot/placement。成功返回 true。
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
    let equipped_key = inventory
        .equipped
        .iter()
        .find(|(_, v)| v.template_id == template_id)
        .map(|(k, _)| k.clone());
    if let Some(k) = equipped_key {
        if let Some(slot) = inventory.equipped.get_mut(&k) {
            if slot.stack_count > 1 {
                slot.stack_count -= 1;
            } else {
                inventory.equipped.remove(&k);
            }
            inventory.revision.0 = inventory.revision.0.saturating_add(1);
            return true;
        }
    }
    false
}

fn inventory_has_template_count(
    inventory: &PlayerInventory,
    template_id: &str,
    required: u32,
) -> bool {
    let mut total = 0u32;
    for item in inventory.hotbar.iter().flatten() {
        if item.template_id == template_id {
            total = total.saturating_add(item.stack_count);
        }
    }
    for container in &inventory.containers {
        for placed in &container.items {
            if placed.instance.template_id == template_id {
                total = total.saturating_add(placed.instance.stack_count);
            }
        }
    }
    for item in inventory.equipped.values() {
        if item.template_id == template_id {
            total = total.saturating_add(item.stack_count);
        }
    }
    total >= required
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
            revision: InventoryRevision(0),
            containers: vec![ContainerState {
                id: "main".into(),
                name: "main".into(),
                rows: 4,
                cols: 4,
                items: Vec::new(),
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
