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
use valence::prelude::{
    bevy_ecs, ChunkLayer, Client, Commands, DVec3, Entity, EventReader, EventWriter, Events, Query,
    Res, ResMut, Resource, Username, With,
};

use crate::alchemy::{
    learned::LearnResult, AlchemyFurnace, AlchemySession, Intervention, LearnedRecipes,
    PlaceFurnaceRequest, RecipeRegistry,
};
use crate::combat::components::{
    CastSource, Casting, QuickSlotBindings, SkillBarBindings, SkillSlot,
};
use crate::combat::events::{
    ApplyStatusEffectIntent, DefenseIntent, RevivalActionIntent, RevivalActionKind,
    StatusEffectKind,
};
use crate::combat::CombatClock;
use crate::cultivation::breakthrough::BreakthroughRequest;
use crate::cultivation::components::{recover_current_qi, Cultivation};
use crate::cultivation::forging::ForgeRequest;
use crate::cultivation::insight::InsightChosen;
use crate::cultivation::known_techniques::technique_definition;
use crate::cultivation::lifespan::LifespanExtensionIntent;
use crate::cultivation::meridian_open::MeridianTarget;
use crate::cultivation::possession::{DuoSheRequestEvent, UseLifeCoreEvent};
use crate::cultivation::tribulation::{HeartDemonChoiceSubmitted, StartDuXuRequest};
use crate::forge::blueprint::TemperBeat;
use crate::forge::events::{
    ConsecrationInject, InscriptionScrollSubmit, StepAdvance, TemperingHit,
};
use crate::forge::learned::LearnedBlueprints;
use crate::forge::session::{ForgeSessionId, ForgeSessions, ForgeStep};
use crate::forge::station::PlaceForgeStationRequest;
use crate::inventory::{
    apply_inventory_move, apply_item_spiritual_wear, consume_item_instance_once,
    discard_inventory_item_to_dropped_loot, fully_repair_weapon_instance,
    inventory_item_by_instance_borrow, pickup_dropped_loot_instance, DroppedLootRegistry,
    InventoryDurabilityChangedEvent, InventoryMoveOutcome, ItemInstance, PlayerInventory,
};
use crate::inventory::{
    ItemEffect, ItemRegistry, DEFAULT_CAST_DURATION_MS as TEMPLATE_DEFAULT_CAST_MS,
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
use crate::network::alchemy_snapshot_emit;
use crate::network::cast_emit::{
    current_unix_millis, push_cast_sync, CAST_INTERRUPT_COOLDOWN_TICKS,
};
// dropped_loot_sync is emitted by dropped_loot_sync_emit.
use crate::network::inventory_snapshot_emit::send_inventory_snapshot_to_client;
use crate::network::send_server_data_payload;
use crate::network::skill_snapshot_emit::send_skill_snapshot_to_client;
use crate::player::gameplay::{GameplayAction, GameplayActionQueue, GatherAction};
use crate::player::state::{
    canonical_player_id, update_player_ui_prefs, PlayerState, PlayerStatePersistence,
};
use crate::schema::client_request::{ClientRequestV1, SkillBarBindingV1};
use crate::schema::combat_hud::{CastOutcomeV1, CastPhaseV1, CastSyncV1};
use crate::schema::inventory::{InventoryEventV1, InventoryLocationV1};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
use crate::skill::components::{ScrollId, SkillId, SkillSet};
use crate::skill::events::{SkillScrollUsed, SkillXpGain, XpGainSource};
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::events::EVENT_REALM_COLLAPSE;
use crate::world::extract_system::{
    CancelExtractRequest as CancelExtractRequestEvent,
    StartExtractRequest as StartExtractRequestEvent,
};
use crate::world::karma::KarmaWeightStore;
use crate::world::zone::ZoneRegistry;

const TARGETED_ITEM_WEAR_MIN_FRACTION: f64 = 0.01;
const TARGETED_ITEM_WEAR_MAX_FRACTION: f64 = 0.05;
const TARGETED_ITEM_WEAR_WEIGHT_THRESHOLD: f32 = 0.01;

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
    pub item_registry: Res<'w, ItemRegistry>,
    pub buff_tx: EventWriter<'w, ApplyStatusEffectIntent>,
    pub start_extract_tx: Option<ResMut<'w, Events<StartExtractRequestEvent>>>,
    pub cancel_extract_tx: Option<ResMut<'w, Events<CancelExtractRequestEvent>>>,
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
    pub furnaces: Query<'w, 's, &'static mut AlchemyFurnace>,
    pub learned: Query<'w, 's, &'static mut LearnedRecipes>,
    pub recipe_registry: Res<'w, RecipeRegistry>,
    pub place_furnace_tx: EventWriter<'w, PlaceFurnaceRequest>,
    pub zone_registry: Option<Res<'w, ZoneRegistry>>,
}

#[derive(SystemParam)]
pub struct ClientRequestDispatchParams<'w> {
    pub gameplay_queue: Option<valence::prelude::ResMut<'w, GameplayActionQueue>>,
    pub breakthrough_tx: EventWriter<'w, BreakthroughRequest>,
    pub start_du_xu_tx: Option<ResMut<'w, Events<StartDuXuRequest>>>,
    pub heart_demon_choice_tx: Option<ResMut<'w, Events<HeartDemonChoiceSubmitted>>>,
    pub forge_tx: EventWriter<'w, ForgeRequest>,
    pub insight_tx: EventWriter<'w, InsightChosen>,
    pub lifespan_extension_tx: Option<ResMut<'w, Events<LifespanExtensionIntent>>>,
    pub duo_she_tx: Option<ResMut<'w, Events<DuoSheRequestEvent>>>,
    pub life_core_tx: Option<ResMut<'w, Events<UseLifeCoreEvent>>>,
    pub defense_tx: Option<ResMut<'w, Events<DefenseIntent>>>,
    pub revival_tx: Option<ResMut<'w, Events<RevivalActionIntent>>>,
    pub place_forge_station_tx: Option<ResMut<'w, Events<PlaceForgeStationRequest>>>,
    pub tempering_hit_tx: Option<ResMut<'w, Events<TemperingHit>>>,
    pub consecration_inject_tx: Option<ResMut<'w, Events<ConsecrationInject>>>,
    pub step_advance_tx: Option<ResMut<'w, Events<StepAdvance>>>,
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

const CHANNEL: &str = "bong:client_request";
const SUPPORTED_VERSION: u8 = 1;
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
            | ClientRequestV1::LearnSkillScroll { v, .. }
            | ClientRequestV1::InventoryMoveIntent { v, .. }
            | ClientRequestV1::InventoryDiscardItem { v, .. }
            | ClientRequestV1::DropWeaponIntent { v, .. }
            | ClientRequestV1::RepairWeaponIntent { v, .. }
            | ClientRequestV1::PickupDroppedItem { v, .. }
            | ClientRequestV1::MineralProbe { v, .. }
            | ClientRequestV1::ApplyPill { v, .. }
            | ClientRequestV1::DuoSheRequest { v, .. }
            | ClientRequestV1::UseLifeCore { v, .. }
            | ClientRequestV1::Jiemai { v }
            | ClientRequestV1::UseQuickSlot { v, .. }
            | ClientRequestV1::QuickSlotBind { v, .. }
            | ClientRequestV1::SkillBarCast { v, .. }
            | ClientRequestV1::SkillBarBind { v, .. }
            | ClientRequestV1::CombatReincarnate { v }
            | ClientRequestV1::CombatTerminate { v }
            | ClientRequestV1::CombatCreateNewCharacter { v }
            | ClientRequestV1::StartExtractRequest { v, .. }
            | ClientRequestV1::CancelExtractRequest { v }
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
            | ClientRequestV1::ForgeStationPlace { v, .. } => *v,
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
            ClientRequestV1::AlchemyIntervention { intervention, .. } => {
                handle_alchemy_intervention(
                    ev.client,
                    intervention.into(),
                    &mut clients,
                    &mut alchemy_params.furnaces,
                    alchemy_params.zone_registry.as_deref(),
                );
            }
            ClientRequestV1::AlchemyOpenFurnace { furnace_id, .. } => {
                // 当前 MVP:每玩家一个虚拟炉,furnace_id 仅作日志记录;触发一次完整 snapshot 重推。
                if let Ok((username, mut client)) = clients.get_mut(ev.client) {
                    let player_id = canonical_player_id(username.0.as_str());
                    if let Ok(learned) = alchemy_params.learned.get(ev.client) {
                        alchemy_snapshot_emit::send_recipe_book_from_learned(
                            &mut client,
                            &player_id,
                            learned,
                        );
                    }
                    tracing::info!(
                        "[bong][network][alchemy] open_furnace `{furnace_id}` for `{player_id}`"
                    );
                }
            }
            ClientRequestV1::AlchemyTakePill { pill_item_id, .. } => {
                handle_alchemy_take_pill(
                    ev.client,
                    &pill_item_id,
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
            // 涉及 inventory 联动的请求暂保留 stub(plan-inventory-v1 接入后再做)
            other @ (ClientRequestV1::AlchemyFeedSlot { .. }
            | ClientRequestV1::AlchemyTakeBack { .. }
            | ClientRequestV1::AlchemyIgnite { .. }) => {
                tracing::debug!(
                    "[bong][network][alchemy] received {other:?} from {:?}; awaiting inventory wiring (plan-inventory-v1)",
                    ev.client
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
            ClientRequestV1::DuoSheRequest { target_id, .. } => {
                if let Some(duo_she_tx) = dispatch.duo_she_tx.as_deref_mut() {
                    duo_she_tx.send(DuoSheRequestEvent {
                        host: ev.client,
                        target_id,
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
    use crate::combat::components::UnlockedStyles;
    use crate::forge::session::{ForgeSession, StepState};
    use crate::inventory::{
        BlueprintScrollSpec, ContainerState, InscriptionScrollSpec, InventoryRevision,
        ItemCategory, ItemInstance, ItemRarity, ItemTemplate, PlacedItemState,
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
    struct CapturedInsightChoices(Vec<InsightChosen>);

    impl valence::prelude::Resource for CapturedInsightChoices {}

    #[derive(Default)]
    struct CapturedMineralProbes(Vec<MineralProbeIntent>);

    impl valence::prelude::Resource for CapturedMineralProbes {}

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
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(
            Update,
            (
                handle_client_request_payloads,
                crate::network::inventory_event_emit::emit_durability_changed_inventory_events,
            )
                .chain(),
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
        app.world_mut()
            .entity_mut(entity)
            .insert(AlchemyFurnace::placed(
                valence::prelude::BlockPos::new(8, 66, 8),
                1,
            ));
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data:
                    br#"{"type":"alchemy_intervention","v":1,"intervention":{"kind":"inject_qi","qi":5.0}}"#
                        .to_vec()
                        .into_boxed_slice(),
            });

        app.update();

        let furnace = app.world().entity(entity).get::<AlchemyFurnace>().unwrap();
        assert!(furnace.session.is_none());
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
    fn inventory_move_applies_hidden_targeted_wear_to_spiritual_item() {
        let mut app = App::new();
        register_request_app(&mut app);
        app.insert_resource(ItemRegistry::from_map(HashMap::from([(
            "spiritual_ore".to_string(),
            ItemTemplate {
                id: "spiritual_ore".to_string(),
                display_name: "灵矿".to_string(),
                category: ItemCategory::Misc,
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
                data: br#"{"type":"skill_bar_bind","v":1,"slot":0,"binding":{"kind":"skill","skill_id":"burst_meridian.beng_quan"}}"#
                    .to_vec()
                    .into_boxed_slice(),
            });
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: entity,
                channel: ident!("bong:client_request").into(),
                data: br#"{"type":"skill_bar_cast","v":1,"slot":0,"target":"npc:1"}"#
                    .to_vec()
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

    let duration_ticks = u64::from(definition.cast_ticks).max(1);
    let complete_cooldown_ticks = u64::from(definition.cooldown_ticks).max(1);
    let duration_ms = definition.cast_ticks.saturating_mul(50);
    let started_at_ms = current_unix_millis();
    let start_position = combat_params
        .positions
        .get(entity)
        .map(|position| position.get())
        .unwrap_or(valence::prelude::DVec3::ZERO);
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
        "[bong][network] skill cast started entity={entity:?} slot={slot} skill={skill_id} target={target:?} duration_ticks={} cooldown_ticks={} tick={}",
        definition.cast_ticks,
        definition.cooldown_ticks,
        clock.tick
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
    if weight < TARGETED_ITEM_WEAR_WEIGHT_THRESHOLD {
        return None;
    }

    let wear_fraction = targeted_item_wear_fraction(item.instance_id, username, weight);
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

fn targeted_item_wear_fraction(instance_id: u64, username: &str, karma_weight: f32) -> f64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    instance_id.hash(&mut hasher);
    username.hash(&mut hasher);
    karma_weight.to_bits().hash(&mut hasher);
    let bucket = hasher.finish() % 10_000;
    let unit = bucket as f64 / 9_999.0;
    TARGETED_ITEM_WEAR_MIN_FRACTION
        + (TARGETED_ITEM_WEAR_MAX_FRACTION - TARGETED_ITEM_WEAR_MIN_FRACTION) * unit
}

fn send_moved_event(
    entity: valence::prelude::Entity,
    clients: &mut Query<(&Username, &mut Client)>,
    instance_id: u64,
    from: InventoryLocationV1,
    to: InventoryLocationV1,
    revision: u64,
) {
    let payload = ServerDataV1::new(ServerDataPayloadV1::InventoryEvent(
        InventoryEventV1::Moved {
            revision,
            instance_id,
            from,
            to,
        },
    ));
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
) {
    let player_pos = client_position(positions, entity);
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
        }
        alchemy_snapshot_emit::send_recipe_book_from_learned(&mut client, &player_id, &learned);
    }
}

fn handle_alchemy_intervention(
    entity: valence::prelude::Entity,
    intervention: Intervention,
    clients: &mut Query<(&Username, &mut Client)>,
    furnaces: &mut Query<&mut AlchemyFurnace>,
    zone_registry: Option<&ZoneRegistry>,
) {
    let Ok((username, mut client)) = clients.get_mut(entity) else {
        return;
    };
    let player_id = canonical_player_id(username.0.as_str());
    let Ok(mut furnace) = furnaces.get_mut(entity) else {
        return;
    };
    if matches!(intervention, Intervention::InjectQi(_))
        && furnace_zone_is_collapsed(&furnace, zone_registry)
    {
        tracing::debug!(
            "[bong][network][alchemy] `{player_id}` inject_qi ignored: furnace is in collapsed zone"
        );
        return;
    }
    let session = match furnace.session.as_mut() {
        Some(s) => s,
        None => {
            // 没起炉 — 创建空 session 让干预可见(诊断/调试)。生产路径应先 ignite。
            let s = AlchemySession::new("__none__".into(), player_id.clone());
            furnace.session = Some(s);
            furnace.session.as_mut().unwrap()
        }
    };
    session.apply_intervention(intervention.clone());
    tracing::info!(
        "[bong][network][alchemy] `{player_id}` intervention {intervention:?} → temp={:.2} qi={:.2}",
        session.temp_current, session.qi_injected
    );
    alchemy_snapshot_emit::send_session_from_furnace(&mut client, &player_id, &furnace);
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
/// 目前仅 `BreakthroughBonus` 有运行时接入（发 `ApplyStatusEffectIntent` 挂 buff）；
/// 其他 kind（MeridianHeal/ContaminationCleanse）待对应 tick 系统就位。
#[allow(clippy::too_many_arguments)]
fn handle_alchemy_take_pill(
    entity: Entity,
    pill_item_id: &str,
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
    if !consume_one_by_template(&mut inventory, pill_item_id) {
        tracing::warn!(
            "[bong][network][alchemy] take_pill entity={entity:?} `{pill_item_id}` not in inventory"
        );
        return;
    }

    let mut cultivation_snapshot_override = None;
    match effect {
        ItemEffect::BreakthroughBonus { magnitude } => {
            combat_params.buff_tx.send(ApplyStatusEffectIntent {
                target: entity,
                kind: StatusEffectKind::BreakthroughBoost,
                magnitude: magnitude as f32,
                duration_ticks: BREAKTHROUGH_BOOST_DURATION_TICKS,
                issued_at_tick: clock.tick,
            });
            tracing::info!(
                "[bong][network][alchemy] take_pill entity={entity:?} `{pill_item_id}` → BreakthroughBoost +{magnitude:.3} for {BREAKTHROUGH_BOOST_DURATION_TICKS} ticks"
            );
        }
        ItemEffect::QiRecovery { amount } => {
            if let Ok(current) = cultivations.get(entity) {
                let mut cultivation = current.clone();
                let qi_max_before = cultivation.qi_max;
                let recovered = recover_current_qi(&mut cultivation, amount);
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
                lifespan_extension_tx.send(LifespanExtensionIntent {
                    entity,
                    requested_years: years,
                    source: source.clone(),
                });
            }
            tracing::info!(
                "[bong][network][alchemy] take_pill entity={entity:?} lifespan extension {years} years source={source}"
            );
        }
        ItemEffect::MeridianHeal { .. } | ItemEffect::ContaminationCleanse { .. } => {
            // 需对应 tick 系统（meridian_heal / contamination_cleanse）消费，当前尚未 wire。
            tracing::warn!(
                "[bong][network][alchemy] take_pill entity={entity:?} `{pill_item_id}` effect {:?} not yet wired to runtime",
                effect
            );
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
}
