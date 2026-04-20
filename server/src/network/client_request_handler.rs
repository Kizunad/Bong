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
    bevy_ecs, Client, Commands, EventReader, EventWriter, Query, Res, ResMut, Resource, Username,
};

use crate::alchemy::{
    learned::LearnResult, AlchemyFurnace, AlchemySession, Intervention, LearnedRecipes,
    RecipeRegistry,
};
use crate::combat::components::{
    Casting, DefenseStance, DefenseStanceKind, QuickSlotBindings, UnlockedStyles,
};
use crate::combat::events::DefenseIntent;
use crate::combat::CombatClock;
use crate::cultivation::breakthrough::{add_pending_material_bonus, BreakthroughRequest};
use crate::cultivation::components::{Contamination, Cultivation, MeridianId, MeridianSystem};
use crate::cultivation::forging::ForgeRequest;
use crate::cultivation::insight::InsightChosen;
use crate::cultivation::meridian_open::MeridianTarget;
use crate::inventory::{
    apply_inventory_move, consume_item_instance_once, discard_inventory_item_to_dropped_loot,
    inventory_item_by_instance, pickup_dropped_loot_instance, DroppedLootRegistry,
    InventoryMoveOutcome, ItemEffect, PlayerInventory,
};
use crate::inventory::{
    ItemRegistry, DEFAULT_CAST_DURATION_MS as TEMPLATE_DEFAULT_CAST_MS,
    DEFAULT_COOLDOWN_MS as TEMPLATE_DEFAULT_COOLDOWN_MS,
};
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::alchemy_snapshot_emit;
use crate::network::cast_emit::{
    current_unix_millis, push_cast_sync, CAST_INTERRUPT_COOLDOWN_TICKS,
};
use crate::network::dropped_loot_sync_emit::send_dropped_loot_sync_to_client;
use crate::network::inventory_snapshot_emit::send_inventory_snapshot_to_client;
use crate::network::send_server_data_payload;
use crate::player::state::PlayerState;
use crate::schema::client_request::{ApplyPillTargetV1, ClientRequestV1};
use crate::schema::combat_hud::{CastOutcomeV1, CastPhaseV1, CastSyncV1};
use crate::schema::inventory::{InventoryEventV1, InventoryLocationV1};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};

/// per-client alchemy mock 状态，让 client→server 操作（翻页/学方）有可观察的回响。
/// 真实数据流（ECS 接入后）会替换掉本 resource。
#[derive(Default, Resource, Debug)]
pub struct AlchemyMockState {
    /// player_id → current recipe-book index
    pub recipe_index: HashMap<String, i32>,
}

/// 把 cast / quickslot / 防御姿态相关查询打包，避免 `handle_client_request_payloads`
/// 顶部参数 tuple 超出 Bevy 0.14 SystemParam 16-tuple 上限。
#[derive(SystemParam)]
pub struct CombatRequestParams<'w, 's> {
    pub casting_q: Query<'w, 's, &'static Casting>,
    pub bindings_q: Query<'w, 's, &'static mut QuickSlotBindings>,
    pub defense_stance_q: Query<'w, 's, &'static mut DefenseStance>,
    pub unlocked_q: Query<'w, 's, &'static UnlockedStyles>,
    pub positions: Query<'w, 's, &'static valence::prelude::Position>,
    pub item_registry: Res<'w, ItemRegistry>,
}

#[derive(SystemParam)]
pub struct InventoryApplyParams<'w, 's> {
    pub inventories: Query<'w, 's, &'static mut PlayerInventory>,
    pub cultivations: Query<'w, 's, &'static mut Cultivation>,
    pub meridians: Query<'w, 's, &'static mut MeridianSystem>,
    pub contaminations: Query<'w, 's, &'static mut Contamination>,
    pub positions: Query<'w, 's, &'static valence::prelude::Position>,
    pub player_states: Query<'w, 's, &'static PlayerState>,
}

const CHANNEL: &str = "bong:client_request";
const SUPPORTED_VERSION: u8 = 1;

#[allow(clippy::too_many_arguments)] // Bevy system signature; one resource/query per gameplay area.
pub fn handle_client_request_payloads(
    mut events: EventReader<CustomPayloadEvent>,
    mut breakthrough_tx: EventWriter<BreakthroughRequest>,
    mut forge_tx: EventWriter<ForgeRequest>,
    mut insight_tx: EventWriter<InsightChosen>,
    mut defense_tx: EventWriter<DefenseIntent>,
    combat_clock: Res<CombatClock>,
    mut commands: Commands,
    mut clients: Query<(&Username, &mut Client)>,
    mut alchemy_state: ResMut<AlchemyMockState>,
    mut alchemy_furnaces: Query<&mut AlchemyFurnace>,
    mut alchemy_learned: Query<&mut LearnedRecipes>,
    mut inventory_apply: InventoryApplyParams,
    mut dropped_loot_registry: ResMut<DroppedLootRegistry>,
    mut combat_params: CombatRequestParams,
    recipe_registry: Res<RecipeRegistry>,
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
            | ClientRequestV1::AlchemyOpenFurnace { v, .. }
            | ClientRequestV1::AlchemyFeedSlot { v, .. }
            | ClientRequestV1::AlchemyTakeBack { v, .. }
            | ClientRequestV1::AlchemyIgnite { v, .. }
            | ClientRequestV1::AlchemyIntervention { v, .. }
            | ClientRequestV1::AlchemyTurnPage { v, .. }
            | ClientRequestV1::AlchemyLearnRecipe { v, .. }
            | ClientRequestV1::AlchemyTakePill { v, .. }
            | ClientRequestV1::InventoryMoveIntent { v, .. }
            | ClientRequestV1::InventoryDiscardItem { v, .. }
            | ClientRequestV1::ApplyPill { v, .. }
            | ClientRequestV1::PickupDroppedItem { v, .. }
            | ClientRequestV1::Jiemai { v }
            | ClientRequestV1::UseQuickSlot { v, .. }
            | ClientRequestV1::QuickSlotBind { v, .. }
            | ClientRequestV1::SwitchDefenseStance { v, .. } => *v,
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
                // 当前阶段固定 material_bonus=0.0，等价于无灵材加成突破；
                // 保持该占位行为以稳定既有 ClientRequestV1 语义。
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
            // ── 炼丹请求 ECS dispatch (plan-alchemy-v1 §4) ──────────────────
            ClientRequestV1::AlchemyTurnPage { delta, .. } => {
                handle_alchemy_turn_page(
                    ev.client,
                    delta,
                    &mut clients,
                    &mut alchemy_learned,
                    &mut alchemy_state,
                );
            }
            ClientRequestV1::AlchemyLearnRecipe { recipe_id, .. } => {
                handle_alchemy_learn(
                    ev.client,
                    recipe_id,
                    &mut clients,
                    &mut alchemy_learned,
                    &recipe_registry,
                );
            }
            ClientRequestV1::AlchemyIntervention { intervention, .. } => {
                handle_alchemy_intervention(
                    ev.client,
                    intervention.into(),
                    &mut clients,
                    &mut alchemy_furnaces,
                );
            }
            ClientRequestV1::AlchemyOpenFurnace { furnace_id, .. } => {
                // 当前 MVP:每玩家一个虚拟炉,furnace_id 仅作日志记录;触发一次完整 snapshot 重推。
                if let Ok((username, mut client)) = clients.get_mut(ev.client) {
                    let player_id = crate::player::state::canonical_player_id(username.0.as_str());
                    if let Ok(learned) = alchemy_learned.get(ev.client) {
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
            // 涉及 inventory 联动的请求暂保留 stub(plan-inventory-v1 接入后再做)
            other @ (ClientRequestV1::AlchemyFeedSlot { .. }
            | ClientRequestV1::AlchemyTakeBack { .. }
            | ClientRequestV1::AlchemyIgnite { .. }
            | ClientRequestV1::AlchemyTakePill { .. }) => {
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
                    &mut inventory_apply.inventories,
                    &mut clients,
                    &inventory_apply.player_states,
                );
            }
            ClientRequestV1::ApplyPill {
                instance_id,
                target,
                ..
            } => {
                let mut inventory = match inventory_apply.inventories.get_mut(ev.client) {
                    Ok(inv) => inv,
                    Err(_) => {
                        tracing::warn!(
                            "[bong][network][inventory] apply_pill entity={:?} has no PlayerInventory",
                            ev.client
                        );
                        continue;
                    }
                };
                let mut cultivation = match inventory_apply.cultivations.get_mut(ev.client) {
                    Ok(c) => c,
                    Err(_) => {
                        tracing::warn!(
                            "[bong][network][inventory] apply_pill entity={:?} has no Cultivation component",
                            ev.client
                        );
                        continue;
                    }
                };
                let mut meridian_system = match inventory_apply.meridians.get_mut(ev.client) {
                    Ok(m) => m,
                    Err(_) => {
                        tracing::warn!(
                            "[bong][network][inventory] apply_pill entity={:?} has no MeridianSystem component",
                            ev.client
                        );
                        continue;
                    }
                };
                let mut contamination = match inventory_apply.contaminations.get_mut(ev.client) {
                    Ok(c) => c,
                    Err(_) => {
                        tracing::warn!(
                            "[bong][network][inventory] apply_pill entity={:?} has no Contamination component",
                            ev.client
                        );
                        continue;
                    }
                };

                match apply_pill_to_state(
                    &mut inventory,
                    &mut cultivation,
                    &mut meridian_system,
                    &mut contamination,
                    &combat_params.item_registry,
                    instance_id,
                    &target,
                ) {
                    Ok(applied) => {
                        tracing::info!(
                            "[bong][network][inventory] apply_pill entity={:?} instance={} template=`{}` magnitude={:.3} total_bonus={:.3} remaining_stack={} revision={}",
                            ev.client,
                            instance_id,
                            applied.template_id,
                            applied.magnitude,
                            applied.total_bonus,
                            applied.remaining_stack,
                            applied.revision,
                        );
                        resync_snapshot(
                            ev.client,
                            &inventory,
                            &mut clients,
                            &inventory_apply.player_states,
                            "apply_pill",
                        );
                    }
                    Err(reason) => {
                        tracing::debug!(
                            "[bong][network][inventory] apply_pill entity={:?} instance={} ignored: {}",
                            ev.client,
                            instance_id,
                            reason
                        );
                    }
                }
            }
            ClientRequestV1::InventoryDiscardItem {
                instance_id, from, ..
            } => {
                let mut inventory = match inventory_apply.inventories.get_mut(ev.client) {
                    Ok(inv) => inv,
                    Err(_) => continue,
                };
                let player_pos = match inventory_apply.positions.get(ev.client) {
                    Ok(pos) => [pos.0.x, pos.0.y, pos.0.z],
                    Err(_) => continue,
                };

                match discard_inventory_item_to_dropped_loot(
                    &mut inventory,
                    &mut dropped_loot_registry,
                    ev.client,
                    player_pos,
                    instance_id,
                    &from,
                ) {
                    Ok(_) => {
                        resync_snapshot(
                            ev.client,
                            &inventory,
                            &mut clients,
                            &inventory_apply.player_states,
                            "inventory_discard_item",
                        );
                        if let Ok((_username, mut client)) = clients.get_mut(ev.client) {
                            send_dropped_loot_sync_to_client(
                                ev.client,
                                &mut client,
                                &dropped_loot_registry,
                            );
                        }
                    }
                    Err(reason) => {
                        tracing::debug!(
                            "[bong][network][inventory] inventory_discard_item entity={:?} instance={} ignored: {}",
                            ev.client,
                            instance_id,
                            reason
                        );
                        resync_snapshot(
                            ev.client,
                            &inventory,
                            &mut clients,
                            &inventory_apply.player_states,
                            "inventory_discard_rejection",
                        );
                    }
                }
            }
            ClientRequestV1::PickupDroppedItem { instance_id, .. } => {
                let mut inventory = match inventory_apply.inventories.get_mut(ev.client) {
                    Ok(inv) => inv,
                    Err(_) => continue,
                };
                let player_pos = match inventory_apply.positions.get(ev.client) {
                    Ok(pos) => [pos.0.x, pos.0.y, pos.0.z],
                    Err(_) => continue,
                };

                match pickup_dropped_loot_instance(
                    &mut inventory,
                    &mut dropped_loot_registry,
                    ev.client,
                    player_pos,
                    instance_id,
                ) {
                    Ok(_) => {
                        resync_snapshot(
                            ev.client,
                            &inventory,
                            &mut clients,
                            &inventory_apply.player_states,
                            "pickup_dropped_item",
                        );
                        if let Ok((_username, mut client)) = clients.get_mut(ev.client) {
                            send_dropped_loot_sync_to_client(
                                ev.client,
                                &mut client,
                                &dropped_loot_registry,
                            );
                        }
                    }
                    Err(reason) => {
                        tracing::debug!(
                            "[bong][network][inventory] pickup_dropped_item entity={:?} instance={} ignored: {}",
                            ev.client,
                            instance_id,
                            reason
                        );
                    }
                }
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
                    &inventory_apply.inventories,
                );
            }
            ClientRequestV1::QuickSlotBind { slot, item_id, .. } => {
                handle_quick_slot_bind(
                    ev.client,
                    slot,
                    item_id,
                    &mut combat_params.bindings_q,
                    &inventory_apply.inventories,
                );
            }
            ClientRequestV1::SwitchDefenseStance { stance, .. } => {
                handle_switch_defense_stance(
                    ev.client,
                    &stance,
                    &mut combat_params.defense_stance_q,
                    &combat_params.unlocked_q,
                );
            }
        }
    }
}

fn handle_switch_defense_stance(
    entity: valence::prelude::Entity,
    stance_str: &str,
    defense_stance_q: &mut Query<&mut DefenseStance>,
    unlocked_q: &Query<&UnlockedStyles>,
) {
    let new_stance = match stance_str.to_ascii_uppercase().as_str() {
        "NONE" => DefenseStanceKind::None,
        "JIEMAI" => DefenseStanceKind::Jiemai,
        "TISHI" => DefenseStanceKind::Tishi,
        "JUELING" => DefenseStanceKind::Jueling,
        other => {
            tracing::warn!(
                "[bong][network] switch_defense_stance entity={entity:?} ignored: unknown stance '{other}'"
            );
            return;
        }
    };
    let unlocked = unlocked_q.get(entity).copied().unwrap_or(UnlockedStyles {
        jiemai: false,
        tishi: false,
        jueling: false,
    });
    let allowed = match new_stance {
        DefenseStanceKind::None => true,
        DefenseStanceKind::Jiemai => unlocked.jiemai,
        DefenseStanceKind::Tishi => unlocked.tishi,
        DefenseStanceKind::Jueling => unlocked.jueling,
    };
    if !allowed {
        tracing::debug!(
            "[bong][network] switch_defense_stance entity={entity:?} ignored: stance {new_stance:?} not unlocked"
        );
        return;
    }
    let Ok(mut stance) = defense_stance_q.get_mut(entity) else {
        tracing::warn!(
            "[bong][network] switch_defense_stance entity={entity:?} has no DefenseStance Component"
        );
        return;
    };
    if stance.stance == new_stance {
        // Bevy 不会触发 Changed，但也不报错；client 已经知道状态。
        return;
    }
    stance.stance = new_stance;
    tracing::info!("[bong][network] switch_defense_stance entity={entity:?} -> {new_stance:?}");
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

    match apply_inventory_move(&mut inventory, instance_id, &from, &to) {
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

#[derive(Debug, Clone, PartialEq)]
struct ApplyPillApplied {
    template_id: String,
    magnitude: f64,
    total_bonus: f64,
    remaining_stack: u32,
    revision: u64,
}

fn apply_meridian_heal(meridians: &mut MeridianSystem, meridian_id: MeridianId, magnitude: f64) {
    let meridian = meridians.get_mut(meridian_id);
    if !meridian.opened {
        return;
    }

    let mut remaining = magnitude.clamp(0.0, 1.0);
    for crack in &mut meridian.cracks {
        if remaining <= 0.0 {
            break;
        }
        let missing = (crack.severity - crack.healing_progress).max(0.0);
        let applied = remaining.min(missing);
        crack.healing_progress += applied;
        remaining -= applied;
    }

    let healed_count = meridian
        .cracks
        .iter()
        .filter(|crack| crack.healing_progress >= crack.severity)
        .count();
    meridian
        .cracks
        .retain(|crack| crack.healing_progress < crack.severity);
    if healed_count > 0 {
        meridian.integrity = (meridian.integrity + 0.05 * healed_count as f64).min(1.0);
    }
    if remaining > 0.0 {
        meridian.integrity = (meridian.integrity + remaining * 0.1).min(1.0);
    }
}

fn apply_contamination_cleanse(contamination: &mut Contamination, magnitude: f64) {
    let mut remaining = magnitude.clamp(0.0, 1.0);
    contamination.entries.sort_by(|a, b| {
        b.amount
            .partial_cmp(&a.amount)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for entry in &mut contamination.entries {
        if remaining <= 0.0 {
            break;
        }
        let applied = remaining.min(entry.amount);
        entry.amount = (entry.amount - applied).max(0.0);
        remaining -= applied;
    }
    contamination.entries.retain(|entry| entry.amount > 1e-9);
}

fn apply_pill_to_state(
    inventory: &mut PlayerInventory,
    cultivation: &mut Cultivation,
    meridians: &mut MeridianSystem,
    contamination: &mut Contamination,
    item_registry: &ItemRegistry,
    instance_id: u64,
    target: &ApplyPillTargetV1,
) -> Result<ApplyPillApplied, String> {
    let item = inventory_item_by_instance(inventory, instance_id)
        .ok_or_else(|| format!("missing inventory instance {instance_id}"))?;

    let template = item_registry
        .get(&item.template_id)
        .ok_or_else(|| format!("unknown inventory template `{}`", item.template_id))?;

    let (magnitude, total_bonus) = match (target, template.effect.as_ref()) {
        (ApplyPillTargetV1::SelfTarget, Some(ItemEffect::BreakthroughBonus { magnitude })) => {
            let magnitude = *magnitude;
            let total_bonus = add_pending_material_bonus(cultivation, magnitude);
            (magnitude, total_bonus)
        }
        (
            ApplyPillTargetV1::Meridian { meridian_id },
            Some(ItemEffect::MeridianHeal { magnitude, .. }),
        ) => {
            apply_meridian_heal(meridians, *meridian_id, *magnitude);
            (*magnitude, cultivation.pending_material_bonus)
        }
        (ApplyPillTargetV1::SelfTarget, Some(ItemEffect::ContaminationCleanse { magnitude })) => {
            apply_contamination_cleanse(contamination, *magnitude);
            (*magnitude, cultivation.pending_material_bonus)
        }
        (_, Some(effect)) => {
            return Err(format!("effect {effect:?} not supported in this slice"));
        }
        (_, None) => {
            return Err(format!("template `{}` has no effect", item.template_id));
        }
    };

    let consume_outcome = consume_item_instance_once(inventory, instance_id)?;

    Ok(ApplyPillApplied {
        template_id: item.template_id,
        magnitude,
        total_bonus,
        remaining_stack: consume_outcome.remaining_stack,
        revision: consume_outcome.revision.0,
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::{
        ColorKind, ContamSource, Contamination, CrackCause, Cultivation, MeridianCrack,
        MeridianSystem, Realm,
    };
    use crate::inventory::{
        ContainerState, InventoryRevision, ItemCategory, ItemInstance, ItemRarity, ItemRegistry,
        ItemTemplate, PlayerInventory, FRONT_SATCHEL_CONTAINER_ID, MAIN_PACK_CONTAINER_ID,
        SMALL_POUCH_CONTAINER_ID,
    };
    use std::collections::HashMap;

    fn breakthrough_pill_template() -> ItemTemplate {
        ItemTemplate {
            id: "guyuan_pill".to_string(),
            display_name: "固元丹".to_string(),
            category: ItemCategory::Pill,
            grid_w: 1,
            grid_h: 1,
            base_weight: 0.2,
            rarity: ItemRarity::Rare,
            spirit_quality_initial: 1.0,
            description: "温补真元，服后可加速恢复灵力。".to_string(),
            effect: Some(ItemEffect::BreakthroughBonus { magnitude: 0.12 }),
            cast_duration_ms: 1500,
            cooldown_ms: 1500,
            weapon_spec: None,
        }
    }

    fn unsupported_pill_template() -> ItemTemplate {
        ItemTemplate {
            id: "ningmai_powder".to_string(),
            display_name: "凝脉散".to_string(),
            category: ItemCategory::Pill,
            grid_w: 1,
            grid_h: 1,
            base_weight: 0.3,
            rarity: ItemRarity::Uncommon,
            spirit_quality_initial: 1.0,
            description: "外敷经脉，缓解走火入魔。".to_string(),
            effect: Some(ItemEffect::MeridianHeal {
                magnitude: 0.2,
                target: "any_meridian".to_string(),
            }),
            cast_duration_ms: 1500,
            cooldown_ms: 1500,
            weapon_spec: None,
        }
    }

    fn registry_with(template: ItemTemplate) -> ItemRegistry {
        let mut map = HashMap::new();
        map.insert(template.id.clone(), template);
        ItemRegistry::from_map(map)
    }

    fn inventory_with_item(instance: ItemInstance) -> PlayerInventory {
        PlayerInventory {
            revision: InventoryRevision(7),
            containers: vec![
                ContainerState {
                    id: MAIN_PACK_CONTAINER_ID.to_string(),
                    name: "主背包".to_string(),
                    rows: 5,
                    cols: 7,
                    items: vec![crate::inventory::PlacedItemState {
                        row: 0,
                        col: 0,
                        instance,
                    }],
                },
                ContainerState {
                    id: SMALL_POUCH_CONTAINER_ID.to_string(),
                    name: "小口袋".to_string(),
                    rows: 3,
                    cols: 3,
                    items: Vec::new(),
                },
                ContainerState {
                    id: FRONT_SATCHEL_CONTAINER_ID.to_string(),
                    name: "前挂包".to_string(),
                    rows: 3,
                    cols: 4,
                    items: Vec::new(),
                },
            ],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 45.0,
        }
    }

    #[test]
    fn apply_pill_to_state_consumes_breakthrough_pill_and_sets_pending_bonus() {
        let registry = registry_with(breakthrough_pill_template());
        let mut inventory = inventory_with_item(ItemInstance {
            instance_id: 1001,
            template_id: "guyuan_pill".to_string(),
            display_name: "固元丹".to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.2,
            rarity: ItemRarity::Rare,
            description: "温补真元，服后可加速恢复灵力。".to_string(),
            stack_count: 2,
            spirit_quality: 1.0,
            durability: 1.0,
        });
        let mut cultivation = Cultivation {
            realm: Realm::Awaken,
            pending_material_bonus: 0.05,
            ..Default::default()
        };
        let mut meridians = MeridianSystem::default();

        let out = apply_pill_to_state(
            &mut inventory,
            &mut cultivation,
            &mut meridians,
            &mut Contamination::default(),
            &registry,
            1001,
            &ApplyPillTargetV1::SelfTarget,
        )
        .expect("breakthrough pill should apply");

        assert_eq!(out.template_id, "guyuan_pill");
        assert!((out.magnitude - 0.12).abs() < 1e-9);
        assert!((out.total_bonus - 0.17).abs() < 1e-9);
        assert_eq!(out.remaining_stack, 1);
        assert_eq!(out.revision, 8);
        assert!((cultivation.pending_material_bonus - 0.17).abs() < 1e-9);
        assert_eq!(inventory.containers[0].items[0].instance.stack_count, 1);
    }

    #[test]
    fn apply_pill_to_state_rejects_unsupported_effect_without_mutation() {
        let registry = registry_with(unsupported_pill_template());
        let mut inventory = inventory_with_item(ItemInstance {
            instance_id: 1002,
            template_id: "ningmai_powder".to_string(),
            display_name: "凝脉散".to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.3,
            rarity: ItemRarity::Uncommon,
            description: "外敷经脉，缓解走火入魔。".to_string(),
            stack_count: 2,
            spirit_quality: 1.0,
            durability: 1.0,
        });
        let mut cultivation = Cultivation {
            pending_material_bonus: 0.05,
            ..Default::default()
        };
        let mut meridians = MeridianSystem::default();

        let err = apply_pill_to_state(
            &mut inventory,
            &mut cultivation,
            &mut meridians,
            &mut Contamination::default(),
            &registry,
            1002,
            &ApplyPillTargetV1::SelfTarget,
        )
        .unwrap_err();

        assert!(err.contains("not supported in this slice"));
        assert_eq!(inventory.revision, InventoryRevision(7));
        assert_eq!(inventory.containers[0].items[0].instance.stack_count, 2);
        assert!((cultivation.pending_material_bonus - 0.05).abs() < 1e-9);
    }

    #[test]
    fn apply_pill_to_state_heals_selected_meridian_and_consumes_powder() {
        let registry = registry_with(unsupported_pill_template());
        let mut inventory = inventory_with_item(ItemInstance {
            instance_id: 1002,
            template_id: "ningmai_powder".to_string(),
            display_name: "凝脉散".to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.3,
            rarity: ItemRarity::Uncommon,
            description: "外敷经脉，缓解走火入魔。".to_string(),
            stack_count: 2,
            spirit_quality: 1.0,
            durability: 1.0,
        });
        let mut cultivation = Cultivation::default();
        let mut meridians = MeridianSystem::default();
        let lung = meridians.get_mut(MeridianId::Lung);
        lung.opened = true;
        lung.integrity = 0.6;
        lung.cracks.push(MeridianCrack {
            severity: 0.15,
            healing_progress: 0.05,
            cause: CrackCause::Attack,
            created_at: 1,
        });

        let out = apply_pill_to_state(
            &mut inventory,
            &mut cultivation,
            &mut meridians,
            &mut Contamination::default(),
            &registry,
            1002,
            &ApplyPillTargetV1::Meridian {
                meridian_id: MeridianId::Lung,
            },
        )
        .expect("meridian heal pill should apply");

        assert_eq!(out.template_id, "ningmai_powder");
        assert!((out.magnitude - 0.2).abs() < 1e-9);
        assert_eq!(out.remaining_stack, 1);
        assert_eq!(out.revision, 8);
        assert_eq!(cultivation.pending_material_bonus, 0.0);
        assert!(meridians.get(MeridianId::Lung).cracks.is_empty());
        assert!((meridians.get(MeridianId::Lung).integrity - 0.66).abs() < 1e-9);
    }

    #[test]
    fn apply_pill_to_state_cleanses_contamination_and_consumes_forbidden_pill() {
        let registry = registry_with(ItemTemplate {
            id: "huiyuan_pill_forbidden".to_string(),
            display_name: "回元丹·禁药".to_string(),
            category: ItemCategory::Pill,
            grid_w: 1,
            grid_h: 1,
            base_weight: 0.2,
            rarity: ItemRarity::Legendary,
            spirit_quality_initial: 1.0,
            description: "禁药版回元丹，可瞬间排尽异种真元，然代价为反噬经脉。".to_string(),
            effect: Some(ItemEffect::ContaminationCleanse { magnitude: 0.6 }),
            cast_duration_ms: 2500,
            cooldown_ms: 8000,
            weapon_spec: None,
        });
        let mut inventory = inventory_with_item(ItemInstance {
            instance_id: 1003,
            template_id: "huiyuan_pill_forbidden".to_string(),
            display_name: "回元丹·禁药".to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.2,
            rarity: ItemRarity::Legendary,
            description: "禁药版回元丹，可瞬间排尽异种真元，然代价为反噬经脉。".to_string(),
            stack_count: 1,
            spirit_quality: 1.0,
            durability: 1.0,
        });
        let mut cultivation = Cultivation::default();
        let mut meridians = MeridianSystem::default();
        let mut contamination = Contamination {
            entries: vec![
                ContamSource {
                    amount: 0.4,
                    color: ColorKind::Sharp,
                    attacker_id: None,
                    introduced_at: 1,
                },
                ContamSource {
                    amount: 0.3,
                    color: ColorKind::Turbid,
                    attacker_id: None,
                    introduced_at: 2,
                },
            ],
        };

        let out = apply_pill_to_state(
            &mut inventory,
            &mut cultivation,
            &mut meridians,
            &mut contamination,
            &registry,
            1003,
            &ApplyPillTargetV1::SelfTarget,
        )
        .expect("forbidden pill should cleanse contamination");

        assert_eq!(out.template_id, "huiyuan_pill_forbidden");
        assert!((out.magnitude - 0.6).abs() < 1e-9);
        assert_eq!(out.remaining_stack, 0);
        assert_eq!(out.revision, 8);
        assert_eq!(cultivation.pending_material_bonus, 0.0);
        assert_eq!(contamination.entries.len(), 1);
        assert!((contamination.entries[0].amount - 0.1).abs() < 1e-9);
    }
}
