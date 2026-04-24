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
    bevy_ecs, ChunkLayer, Client, Commands, Entity, EventReader, EventWriter, Query, Res, ResMut,
    Resource, Username,
};

use crate::alchemy::{
    learned::LearnResult, AlchemyFurnace, AlchemySession, Intervention, LearnedRecipes,
    PlaceFurnaceRequest, RecipeRegistry,
};
use crate::combat::components::{Casting, QuickSlotBindings};
use crate::combat::events::{ApplyStatusEffectIntent, DefenseIntent, StatusEffectKind};
use crate::combat::CombatClock;
use crate::cultivation::breakthrough::BreakthroughRequest;
use crate::cultivation::components::Cultivation;
use crate::cultivation::forging::ForgeRequest;
use crate::cultivation::insight::InsightChosen;
use crate::cultivation::meridian_open::MeridianTarget;
use crate::inventory::{
    apply_inventory_move, consume_item_instance_once, inventory_item_by_instance_borrow,
    discard_inventory_item_to_dropped_loot, fully_repair_weapon_instance, pickup_dropped_loot_instance,
    DroppedLootRegistry, InventoryMoveOutcome, PlayerInventory,
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
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::alchemy_snapshot_emit;
use crate::network::cast_emit::{
    current_unix_millis, push_cast_sync, CAST_INTERRUPT_COOLDOWN_TICKS,
};
use crate::network::dropped_loot_sync_emit::send_dropped_loot_sync_to_client;
use crate::network::inventory_snapshot_emit::send_inventory_snapshot_to_client;
use crate::network::skill_snapshot_emit::send_skill_snapshot_to_client;
use crate::network::send_server_data_payload;
use crate::player::gameplay::{GameplayAction, GameplayActionQueue, GatherAction};
use crate::player::state::{canonical_player_id, PlayerState};
use crate::schema::client_request::ClientRequestV1;
use crate::schema::combat_hud::{CastOutcomeV1, CastPhaseV1, CastSyncV1};
use crate::schema::inventory::{InventoryEventV1, InventoryLocationV1};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
use crate::skill::components::{ScrollId, SkillId, SkillSet};
use crate::skill::events::{SkillScrollUsed, SkillXpGain, XpGainSource};

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
    pub positions: Query<'w, 's, &'static valence::prelude::Position>,
    pub item_registry: Res<'w, ItemRegistry>,
    pub buff_tx: EventWriter<'w, ApplyStatusEffectIntent>,
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
    pub layers: Query<'w, 's, &'static ChunkLayer>,
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
}

#[derive(SystemParam)]
pub struct SkillScrollRequestParams<'w, 's> {
    pub skill_xp_tx: EventWriter<'w, SkillXpGain>,
    pub skill_scroll_used_tx: EventWriter<'w, SkillScrollUsed>,
    pub skill_sets: Query<'w, 's, &'static mut SkillSet>,
    pub cultivations: Query<'w, 's, &'static Cultivation>,
}

const CHANNEL: &str = "bong:client_request";
const SUPPORTED_VERSION: u8 = 1;
/// plan-cultivation-v1 §3.1：服用突破辅助丹药的 buff 持续时间（5 分钟）。
/// 20 tick/s × 60 s × 5 = 6000。
const BREAKTHROUGH_BOOST_DURATION_TICKS: u64 = 6_000;

#[allow(clippy::too_many_arguments)] // Bevy system signature; one resource/query per gameplay area.
pub fn handle_client_request_payloads(
    mut events: EventReader<CustomPayloadEvent>,
    mut gameplay_queue: Option<valence::prelude::ResMut<GameplayActionQueue>>,
    mut breakthrough_tx: EventWriter<BreakthroughRequest>,
    mut forge_tx: EventWriter<ForgeRequest>,
    mut insight_tx: EventWriter<InsightChosen>,
    mut defense_tx: EventWriter<DefenseIntent>,
    combat_clock: Res<CombatClock>,
    mut commands: Commands,
    mut clients: Query<(&Username, &mut Client)>,
    mut alchemy_params: AlchemyRequestParams,
    mut inventories: Query<&mut PlayerInventory>,
    player_states: Query<&PlayerState>,
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
            | ClientRequestV1::ApplyPill { v, .. }
            | ClientRequestV1::Jiemai { v }
            | ClientRequestV1::UseQuickSlot { v, .. }
            | ClientRequestV1::QuickSlotBind { v, .. }
            | ClientRequestV1::LingtianStartTill { v, .. }
            | ClientRequestV1::LingtianStartRenew { v, .. }
            | ClientRequestV1::LingtianStartPlanting { v, .. }
            | ClientRequestV1::LingtianStartHarvest { v, .. }
            | ClientRequestV1::LingtianStartReplenish { v, .. }
            | ClientRequestV1::LingtianStartDrainQi { v, .. } => *v,
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
                breakthrough_tx.send(BreakthroughRequest {
                    entity: ev.client,
                    material_bonus: 0.0,
                });
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
                insight_tx.send(InsightChosen {
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
                forge_tx.send(ForgeRequest {
                    entity: ev.client,
                    meridian,
                    axis,
                });
            }
            ClientRequestV1::BotanyHarvestRequest {
                session_id, mode, ..
            } => {
                let Some(queue) = gameplay_queue.as_deref_mut() else {
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
                );
            }
            ClientRequestV1::AlchemyOpenFurnace { furnace_id, .. } => {
                // 当前 MVP:每玩家一个虚拟炉,furnace_id 仅作日志记录;触发一次完整 snapshot 重推。
                if let Ok((username, mut client)) = clients.get_mut(ev.client) {
                    let player_id = crate::player::state::canonical_player_id(username.0.as_str());
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
                    &combat_clock,
                    &mut inventories,
                    &mut clients,
                    &player_states,
                    &mut combat_params,
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
                    &dropped_loot_params.positions,
                );
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
                    &combat_clock,
                    &mut inventories,
                    &mut clients,
                    &player_states,
                    &mut combat_params,
                );
            }
            ClientRequestV1::Jiemai { .. } => {
                tracing::info!(
                    "[bong][network] client_request jiemai entity={:?} tick={}",
                    ev.client,
                    combat_clock.tick
                );
                defense_tx.send(DefenseIntent {
                    defender: ev.client,
                    issued_at_tick: combat_clock.tick,
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
        let instance = match inventory_item_by_instance_borrow(&inventory, instance_id) {
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
        skill_scroll_params.skill_scroll_used_tx.send(SkillScrollUsed {
            char_entity: entity,
            scroll_id,
            skill,
            xp_granted: 0,
            was_duplicate: true,
        });
        if let Ok(inventory) = inventories.get(entity) {
            resync_snapshot(
                entity,
                &inventory,
                clients,
                player_states,
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

    skill_scroll_params.skill_xp_tx.send(SkillXpGain {
        char_entity: entity,
        skill,
        amount: xp_grant,
        source: XpGainSource::Scroll {
            scroll_id: scroll_id.clone(),
            xp_grant,
        },
    });
    skill_scroll_params.skill_scroll_used_tx.send(SkillScrollUsed {
        char_entity: entity,
        scroll_id,
        skill,
        xp_granted: xp_grant,
        was_duplicate: false,
    });

    let Ok(player_state) = player_states.get(entity) else {
        return;
    };
    if let Ok((username, mut client)) = clients.get_mut(entity) {
        if let Ok(inventory) = inventories.get(entity) {
            send_inventory_snapshot_to_client(
                entity,
                &mut client,
                username.0.as_str(),
                &inventory,
                player_state,
                "skill_scroll_consumed",
            );
        }
        if let Ok(skill_set) = skill_scroll_params.skill_sets.get(entity) {
            let Ok(cultivation) = skill_scroll_params.cultivations.get(entity) else {
                return;
            };
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

    use crate::inventory::{ContainerState, InventoryRevision, ItemInstance, ItemRarity, PlacedItemState};
    use crate::skill::components::SkillSet;
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::prelude::{ident, App, EventReader, IntoSystemConfigs, ResMut, Update};
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
        }
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
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<PlaceFurnaceRequest>();
        app.add_event::<StartTillRequest>();
        app.add_event::<StartRenewRequest>();
        app.add_event::<StartPlantingRequest>();
        app.add_event::<StartHarvestRequest>();
        app.add_event::<StartReplenishRequest>();
        app.add_event::<StartDrainQiRequest>();
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
        app.add_event::<SkillXpGain>();
        app.add_event::<SkillScrollUsed>();
        app.add_systems(Update, handle_client_request_payloads);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn((
            client_bundle,
            inventory_with_skill_scroll(skill_scroll_item(42, "skill_scroll_herbalism_baicao_can")),
            SkillSet::default(),
            Cultivation::default(),
            PlayerState::default(),
            QuickSlotBindings::default(),
            DefenseStance::default(),
            UnlockedStyles::default(),
        )).id();
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
        assert!(skill_set.consumed_scrolls.contains(&ScrollId::new("skill_scroll_herbalism_baicao_can")));

        let xp_events: Vec<_> = app.world_mut().resource_mut::<valence::prelude::Events<SkillXpGain>>().drain().collect();
        assert_eq!(xp_events.len(), 1);
        assert_eq!(xp_events[0].skill, SkillId::Herbalism);
        assert_eq!(xp_events[0].amount, 500);
        let used_events: Vec<_> = app.world_mut().resource_mut::<valence::prelude::Events<SkillScrollUsed>>().drain().collect();
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
        app.add_event::<SkillXpGain>();
        app.add_event::<SkillScrollUsed>();
        app.add_systems(Update, handle_client_request_payloads);

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let mut skill_set = SkillSet::default();
        skill_set.consumed_scrolls.insert(ScrollId::new("skill_scroll_herbalism_baicao_can"));
        let entity = app.world_mut().spawn((
            client_bundle,
            inventory_with_skill_scroll(skill_scroll_item(42, "skill_scroll_herbalism_baicao_can")),
            skill_set,
            Cultivation::default(),
            PlayerState::default(),
            QuickSlotBindings::default(),
            DefenseStance::default(),
            UnlockedStyles::default(),
        )).id();
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
        let xp_events: Vec<_> = app.world_mut().resource_mut::<valence::prelude::Events<SkillXpGain>>().drain().collect();
        assert!(xp_events.is_empty());
        let used_events: Vec<_> = app.world_mut().resource_mut::<valence::prelude::Events<SkillScrollUsed>>().drain().collect();
        assert_eq!(used_events.len(), 1);
        assert!(used_events[0].was_duplicate);
        assert_eq!(used_events[0].xp_granted, 0);
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
    // plan §4.2: 已 cast 时——同 slot 静默忽略；不同 slot 视为 UserCancel + 启新 cast。
    if let Ok(prev) = combat_params.casting_q.get(entity) {
        if prev.slot == slot {
            tracing::debug!(
                "[bong][network] use_quick_slot entity={entity:?} slot={slot} ignored: same-slot during cast"
            );
            return;
        }
        // 不同 slot → 取消旧 cast。
        let prev_slot = prev.slot;
        let prev_duration_ms = prev.duration_ms;
        let prev_started_at_ms = prev.started_at_ms;
        commands.entity(entity).remove::<Casting>();
        if let Ok(mut bindings) = combat_params.bindings_q.get_mut(entity) {
            bindings.set_cooldown(
                prev_slot,
                clock.tick.saturating_add(CAST_INTERRUPT_COOLDOWN_TICKS),
            );
        }
        if let Ok((username, mut client)) = clients.get_mut(entity) {
            push_cast_sync(
                &mut client,
                CastSyncV1 {
                    phase: CastPhaseV1::Interrupt,
                    slot: prev_slot,
                    duration_ms: prev_duration_ms,
                    started_at_ms: prev_started_at_ms,
                    outcome: CastOutcomeV1::UserCancel,
                },
                username.0.as_str(),
                entity,
            );
        }
        tracing::info!(
            "[bong][network][cast] user_cancel entity={entity:?} prev_slot={prev_slot} → switching to slot={slot}"
        );
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
    let instance_id = match item_id.as_deref() {
        None | Some("") => None,
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
    tracing::info!(
        "[bong][network] quick_slot_bind entity={entity:?} slot={slot} item_id={:?} → instance={:?}",
        item_id,
        instance_id
    );
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
) {
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
            resync_snapshot(entity, &inventory, clients, player_states, "swap");
        }
        Err(reason) => {
            tracing::warn!(
                "[bong][network][inventory] rejected move_intent entity={entity:?} instance={instance_id}: {reason}"
            );
            // Client did optimistic update but server didn't move. Resync to
            // overwrite the diverged client state with authoritative truth.
            resync_snapshot(entity, &inventory, clients, player_states, "rejection");
        }
    }
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
    if let Ok((username, mut client)) = clients.get_mut(entity) {
        send_inventory_snapshot_to_client(
            entity,
            &mut client,
            username.0.as_str(),
            inventory,
            player_state,
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
        entity,
        player_pos,
        instance_id,
        &from,
    ) {
        Ok(outcome) => {
            tracing::info!(
                "[bong][network][inventory] discarded instance={instance_id} from {from:?} revision={}",
                outcome.revision.0
            );
            resync_snapshot(entity, &inventory, clients, player_states, "discard_item");
            if let Ok((_username, mut client)) = clients.get_mut(entity) {
                send_dropped_loot_sync_to_client(entity, &mut client, dropped_loot_registry);
            }
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
                "discard_rejection",
            );
        }
    }
}

fn handle_pickup_dropped_item(
    entity: Entity,
    instance_id: u64,
    inventories: &mut Query<&mut PlayerInventory>,
    dropped_loot_registry: &mut DroppedLootRegistry,
    clients: &mut Query<(&Username, &mut Client)>,
    player_states: &Query<&PlayerState>,
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
        entity,
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
                "pickup_dropped_item",
            );
            if let Ok((_username, mut client)) = clients.get_mut(entity) {
                send_dropped_loot_sync_to_client(entity, &mut client, dropped_loot_registry);
            }
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
                "pickup_rejection",
            );
        }
    }
}

fn handle_repair_weapon(
    entity: Entity,
    instance_id: u64,
    station_pos: [i32; 3],
    item_registry: &ItemRegistry,
    inventories: &mut Query<&mut PlayerInventory>,
    clients: &mut Query<(&Username, &mut Client)>,
    player_states: &Query<&PlayerState>,
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
            resync_snapshot(entity, &inventory, clients, player_states, "repair_weapon");
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
    clock: &CombatClock,
    inventories: &mut Query<&mut PlayerInventory>,
    clients: &mut Query<(&Username, &mut Client)>,
    player_states: &Query<&PlayerState>,
    combat_params: &mut CombatRequestParams,
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
        clock,
        inventories,
        clients,
        player_states,
        combat_params,
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
    let player_id = crate::player::state::canonical_player_id(username.0.as_str());
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
    let player_id = crate::player::state::canonical_player_id(username.0.as_str());
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
) {
    let Ok((username, mut client)) = clients.get_mut(entity) else {
        return;
    };
    let player_id = crate::player::state::canonical_player_id(username.0.as_str());
    let Ok(mut furnace) = furnaces.get_mut(entity) else {
        return;
    };
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

/// plan-cultivation-v1 §3.1：玩家服用 pill → 扣一颗 → 根据 ItemEffect 分派运行时效果。
/// 目前仅 `BreakthroughBonus` 有运行时接入（发 `ApplyStatusEffectIntent` 挂 buff）；
/// 其他 kind（MeridianHeal/ContaminationCleanse）待对应 tick 系统就位。
fn handle_alchemy_take_pill(
    entity: Entity,
    pill_item_id: &str,
    clock: &CombatClock,
    inventories: &mut Query<&mut PlayerInventory>,
    clients: &mut Query<(&Username, &mut Client)>,
    player_states: &Query<&PlayerState>,
    combat_params: &mut CombatRequestParams,
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
        ItemEffect::MeridianHeal { .. } | ItemEffect::ContaminationCleanse { .. } => {
            // 需对应 tick 系统（meridian_heal / contamination_cleanse）消费，当前尚未 wire。
            tracing::warn!(
                "[bong][network][alchemy] take_pill entity={entity:?} `{pill_item_id}` effect {:?} not yet wired to runtime",
                effect
            );
        }
    }

    resync_snapshot(entity, &inventory, clients, player_states, "take_pill");
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
