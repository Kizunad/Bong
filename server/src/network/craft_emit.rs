//! plan-craft-v1 P2 — Craft IPC bridge（server → client + intent → session）。
//!
//! 5 个系统：
//!   1. `apply_craft_start_intents` / `apply_craft_cancel_intents` — 读
//!      `CraftStartIntent` / `CraftCancelIntent`，跑 `start_craft` /
//!      `cancel_craft`，产生 `CraftStartedEvent` / `CraftFailedEvent`，并在
//!      caster 上 insert/remove `CraftSession` component
//!   2. `tick_craft_sessions` — 每 tick 推进所有在线玩家的 session（worldview §九
//!      "玩家在场是基本要求"，下线 Entity 自动清空，session 随之消失）
//!   3. `emit_craft_session_state` — 定期把当前 session 进度推到 client（每 20 tick
//!      一次 / 状态切换时立刻推一次）
//!   4. `emit_craft_outcome_payloads` — 监听 Completed/Failed → push `CraftOutcomeV1`
//!   5. `emit_recipe_list_on_join` / `emit_recipe_list_on_unlock` —
//!      初始全表 + 每次 unlock 增量
//!   6. `apply_material_discovery_unlock` —（plan-craft-material-discovery）
//!      每 tick 扫背包，持有任一原料即被动解锁空源配方 + 重推列表 + narration
//!
//! 守恒律：所有 qi 变更走 `start_craft`/`cancel_craft` 内部已封装的
//! `WorldQiAccount::transfer(QiTransferReason::Crafting)` —— 本模块**禁止**
//! 直接写 `cultivation.qi_current`，否则破坏全局守恒律。

use std::{
    collections::{HashMap, HashSet},
    time::{SystemTime, UNIX_EPOCH},
};

use valence::prelude::{
    bevy_ecs, Changed, Client, Commands, Component, Entity, EventReader, EventWriter, Local,
    Position, Query, Res, ResMut, Username, With,
};

use crate::combat::CombatClock;
use crate::craft::{
    cancel_craft, count_template_in_inventory, finalize_craft, is_within_workbench_range,
    start_craft, tick_session, unlock_via_insight, unlock_via_material, unlock_via_mentor,
    unlock_via_scroll, CancelCraftOutcome, CraftCancelIntent, CraftCompletedEvent,
    CraftFailedEvent, CraftFailureReason, CraftRegistry, CraftSession, CraftStartIntent,
    CraftStartedEvent, CraftUnlockIntent, FinalizeCraftOutcome, MaterialUnlockOutcome,
    RecipeUnlockState, RecipeUnlockedEvent, StartCraftDeps, StartCraftError, StartCraftRequest,
    UnlockEventSource, UnlockOutcome, WorkbenchBlock,
};
use crate::cultivation::components::{Cultivation, QiColor};
use crate::inventory::{
    add_item_to_player_inventory, add_item_to_player_inventory_or_ground, DroppedLootRegistry,
    GrantOrGroundOutcome, InventoryInstanceIdAllocator, ItemRegistry, PlayerInventory,
};
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::player::gameplay::PendingGameplayNarrations;
use crate::player::state::canonical_player_id;
use crate::qi_physics::ledger::WorldQiAccount;
use crate::schema::common::NarrationStyle;
use crate::schema::craft::{
    CraftCategoryV1, CraftFailureReasonV1, CraftOutcomeV1, CraftRecipeEntryV1, CraftRequirementsV1,
    CraftSessionStateV1, RecipeListV1, RecipeUnlockedV1, UnlockEventSourceV1,
};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
use crate::world::dimension::{CurrentDimension, DimensionKind};

/// inventory 内手搓默认绑定的 zone 账户（暂时统一用 "spawn"，与现有
/// `cultivation` 守恒模型一致；后续 plan-zone-v2 可按 `Position → ZoneRegistry`
/// 解析真实 zone）。
const DEFAULT_CRAFT_ZONE_ID: &str = "spawn";

const DEFAULT_REFUND_GROUND_POS: [f64; 3] = [0.0, 64.0, 0.0];

/// 每隔 N tick 对在线 session 推一次进度（20 tick = 1 秒）。
const SESSION_STATE_PUSH_INTERVAL_TICKS: u64 = 20;

/// 标记某玩家本帧需要立刻推一次 SessionState（启动 / 取消 / 完成时打上）。
#[derive(Component, Default, Debug)]
pub struct CraftSessionStateDirty;

fn current_unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn map_failure_reason(reason: CraftFailureReason) -> CraftFailureReasonV1 {
    reason.into()
}

fn build_session_state_payload(
    player_id: &str,
    session: Option<&CraftSession>,
) -> CraftSessionStateV1 {
    if let Some(session) = session {
        let elapsed = session.total_ticks.saturating_sub(session.remaining_ticks);
        CraftSessionStateV1 {
            v: 1,
            player_id: player_id.to_string(),
            active: true,
            recipe_id: Some(session.recipe_id.as_str().to_string()),
            elapsed_ticks: elapsed,
            total_ticks: session.total_ticks,
            completed_count: session.completed_count,
            total_count: session.quantity_total,
            ts: current_unix_millis(),
        }
    } else {
        CraftSessionStateV1 {
            v: 1,
            player_id: player_id.to_string(),
            active: false,
            recipe_id: None,
            elapsed_ticks: 0,
            total_ticks: 0,
            completed_count: 0,
            total_count: 0,
            ts: current_unix_millis(),
        }
    }
}

fn send_payload(client: &mut Client, payload: ServerDataPayloadV1, debug_tag: &str) -> bool {
    let envelope = ServerDataV1::new(payload);
    let label = payload_type_label(envelope.payload_type());
    let bytes = match serialize_server_data_payload(&envelope) {
        Ok(b) => b,
        Err(err) => {
            log_payload_build_error(label, &err);
            return false;
        }
    };
    send_server_data_payload(client, bytes.as_slice());
    tracing::debug!(
        "[bong][network][craft] sent {} {} {}",
        SERVER_DATA_CHANNEL,
        label,
        debug_tag
    );
    true
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct RefundGrantSummary {
    material_returned: u32,
    granted_count: u32,
    dropped_count: u32,
    errors: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct RefundGroundTarget {
    pos: [f64; 3],
    dimension: DimensionKind,
}

fn refund_ground_context(
    player_context: Option<(&Position, Option<&CurrentDimension>)>,
) -> RefundGroundTarget {
    player_context
        .map(|(pos, dimension)| RefundGroundTarget {
            pos: [pos.0.x, pos.0.y, pos.0.z],
            dimension: dimension.map(|dimension| dimension.0).unwrap_or_default(),
        })
        .unwrap_or(RefundGroundTarget {
            pos: DEFAULT_REFUND_GROUND_POS,
            dimension: DimensionKind::default(),
        })
}

fn grant_refund_manifest_to_inventory_or_ground(
    inventory: &mut PlayerInventory,
    item_registry: &ItemRegistry,
    allocator: &mut InventoryInstanceIdAllocator,
    mut dropped_loot: Option<&mut DroppedLootRegistry>,
    refund_manifest: impl IntoIterator<Item = (String, u32)>,
    current_tick: u64,
    ground_target: RefundGroundTarget,
) -> RefundGrantSummary {
    let mut summary = RefundGrantSummary::default();
    for (template, count) in refund_manifest {
        if count == 0 {
            continue;
        }
        let outcome = add_item_to_player_inventory_or_ground(
            inventory,
            item_registry,
            allocator,
            dropped_loot.as_deref_mut(),
            &template,
            count,
            current_tick,
            ground_target.pos,
            ground_target.dimension,
            None,
        );
        match outcome {
            Ok(GrantOrGroundOutcome::Granted(_)) => {
                summary.material_returned = summary.material_returned.saturating_add(count);
                summary.granted_count = summary.granted_count.saturating_add(count);
            }
            Ok(GrantOrGroundOutcome::DroppedToGround(_)) => {
                summary.material_returned = summary.material_returned.saturating_add(count);
                summary.dropped_count = summary.dropped_count.saturating_add(count);
            }
            Err(err) => {
                summary.errors.push(format!("{template} x{count}: {err}"));
            }
        }
    }
    summary
}

/// §1a — 处理客户端发来的 Start intent。
///
/// 命中失败时（材料不足 / qi 不足 / 已有 session / 配方未解锁等）→ emit
/// `CraftFailedEvent { reason: InternalError }` 让 client 收到 Outcome::Failed
/// 通知（client 可据此弹错误 toast）；P2 暂不实装更细分的失败 reason。
#[allow(clippy::too_many_arguments)]
pub fn apply_craft_start_intents(
    mut start_intents: EventReader<CraftStartIntent>,
    mut started_tx: EventWriter<CraftStartedEvent>,
    mut failed_tx: EventWriter<CraftFailedEvent>,
    registry: Res<CraftRegistry>,
    unlock_state: Res<RecipeUnlockState>,
    mut ledger: ResMut<WorldQiAccount>,
    clock: Res<CombatClock>,
    mut commands: Commands,
    names: Query<&Username>,
    player_contexts: Query<(&Position, Option<&CurrentDimension>)>,
    workbenches: Query<&Position, With<WorkbenchBlock>>,
    mut casters: Query<(
        &mut PlayerInventory,
        &mut Cultivation,
        &QiColor,
        Option<&CraftSession>,
    )>,
) {
    // ── start ───────────────────────────────────────────────
    for intent in start_intents.read() {
        let Ok((mut inventory, mut cultivation, qi_color, existing)) =
            casters.get_mut(intent.caster)
        else {
            tracing::warn!(
                "[bong][craft] start intent caster {:?} missing inventory/cultivation",
                intent.caster
            );
            continue;
        };
        let player_id = names
            .get(intent.caster)
            .map(|u| canonical_player_id(u.0.as_str()))
            .unwrap_or_else(|_| format!("entity:{}", intent.caster.to_bits()));

        let req = StartCraftRequest {
            caster: intent.caster,
            player_id: &player_id,
            recipe_id: &intent.recipe_id,
            current_tick: clock.tick,
            zone_id: DEFAULT_CRAFT_ZONE_ID,
            quantity: intent.quantity,
        };
        // §P2.4：检查玩家 Chebyshev 3 格内是否有 WorkbenchBlock entity。
        let has_nearby_workbench = player_contexts
            .get(intent.caster)
            .map(|(pos, _dimension)| {
                let player_pos = [pos.0.x, pos.0.y, pos.0.z];
                workbenches.iter().any(|wb_pos| {
                    let block_pos = [
                        wb_pos.0.x.floor() as i32,
                        wb_pos.0.y.floor() as i32,
                        wb_pos.0.z.floor() as i32,
                    ];
                    is_within_workbench_range(player_pos, block_pos)
                })
            })
            .unwrap_or(false);

        let deps = StartCraftDeps {
            registry: &registry,
            unlock_state: &unlock_state,
            inventory: &mut inventory,
            cultivation: &mut cultivation,
            qi_color,
            ledger: &mut ledger,
            existing_session: existing,
            has_nearby_workbench,
        };

        match start_craft(req, deps) {
            Ok(success) => {
                tracing::info!(
                    "[bong][craft] start ok player={} recipe={} ticks={} quantity={}",
                    player_id,
                    success.event.recipe_id,
                    success.event.total_ticks,
                    intent.quantity
                );
                started_tx.send(success.event);
                commands
                    .entity(intent.caster)
                    .insert(success.session)
                    .insert(CraftSessionStateDirty);
            }
            Err(err) => {
                tracing::info!(
                    "[bong][craft] start rejected player={} recipe={}: {:?}",
                    player_id,
                    intent.recipe_id,
                    err
                );
                // Outcome::Failed 给 client，让它知道开始失败 → 取消按钮态恢复
                failed_tx.send(CraftFailedEvent {
                    caster: intent.caster,
                    recipe_id: intent.recipe_id.clone(),
                    reason: match err {
                        StartCraftError::AlreadyHasSession => CraftFailureReason::PlayerCancelled,
                        _ => CraftFailureReason::InternalError,
                    },
                    material_returned: 0,
                    qi_refunded: 0.0,
                });
                commands
                    .entity(intent.caster)
                    .insert(CraftSessionStateDirty);
            }
        }
    }
}

/// §1b — 处理客户端发来的 Cancel intent。
#[allow(clippy::too_many_arguments)]
pub fn apply_craft_cancel_intents(
    mut cancel_intents: EventReader<CraftCancelIntent>,
    mut failed_tx: EventWriter<CraftFailedEvent>,
    registry: Res<CraftRegistry>,
    item_registry: Res<ItemRegistry>,
    mut allocator: ResMut<InventoryInstanceIdAllocator>,
    mut dropped_loot: Option<ResMut<DroppedLootRegistry>>,
    clock: Res<CombatClock>,
    mut commands: Commands,
    player_contexts: Query<(&Position, Option<&CurrentDimension>)>,
    mut casters: Query<(&mut PlayerInventory, Option<&CraftSession>)>,
) {
    for intent in cancel_intents.read() {
        let Ok((mut inventory, existing)) = casters.get_mut(intent.caster) else {
            continue;
        };
        let Some(session) = existing else {
            tracing::debug!(
                "[bong][craft] cancel intent on caster {:?} without session — noop",
                intent.caster
            );
            continue;
        };
        let Some(recipe) = registry.get(&session.recipe_id) else {
            tracing::warn!(
                "[bong][craft] cancel intent recipe `{}` missing — emitting InternalError",
                session.recipe_id
            );
            failed_tx.send(CraftFailedEvent {
                caster: intent.caster,
                recipe_id: session.recipe_id.clone(),
                reason: CraftFailureReason::InternalError,
                material_returned: 0,
                qi_refunded: 0.0,
            });
            commands
                .entity(intent.caster)
                .remove::<CraftSession>()
                .insert(CraftSessionStateDirty);
            continue;
        };
        let CancelCraftOutcome {
            mut event,
            refund_manifest,
        } = cancel_craft(
            session,
            recipe,
            intent.caster,
            CraftFailureReason::PlayerCancelled,
        );
        let ground_target = refund_ground_context(player_contexts.get(intent.caster).ok());
        let refund_summary = grant_refund_manifest_to_inventory_or_ground(
            &mut inventory,
            &item_registry,
            &mut allocator,
            dropped_loot.as_deref_mut(),
            refund_manifest,
            clock.tick,
            ground_target,
        );
        if !refund_summary.errors.is_empty() {
            tracing::warn!(
                "[bong][craft] cancel refund had structural grant errors caster={:?} recipe={} errors={:?}",
                intent.caster,
                event.recipe_id,
                refund_summary.errors
            );
        }
        event.material_returned = refund_summary.material_returned;
        tracing::info!(
            "[bong][craft] cancel ok caster={:?} recipe={} returned={} granted={} dropped={}",
            intent.caster,
            event.recipe_id,
            event.material_returned,
            refund_summary.granted_count,
            refund_summary.dropped_count
        );
        failed_tx.send(event);
        commands
            .entity(intent.caster)
            .remove::<CraftSession>()
            .insert(CraftSessionStateDirty);
        // 完成事件不发，cancel 走 Failed 通道（reason=PlayerCancelled）
    }
}

/// §2 — 推进 in-game tick；只对在线玩家（Entity 持有 Client）的 session 推进。
/// `tick_session` 返回 true 则当 tick 结束，本系统执行 finalize_craft。
#[allow(clippy::too_many_arguments)]
pub fn tick_craft_sessions(
    registry: Res<CraftRegistry>,
    item_registry: Res<ItemRegistry>,
    mut allocator: ResMut<InventoryInstanceIdAllocator>,
    clock: Res<CombatClock>,
    mut commands: Commands,
    mut completed_tx: EventWriter<CraftCompletedEvent>,
    mut failed_tx: EventWriter<CraftFailedEvent>,
    mut dropped_loot: Option<ResMut<DroppedLootRegistry>>,
    player_contexts: Query<(&Position, Option<&CurrentDimension>)>,
    mut sessions: Query<(Entity, &mut CraftSession, &mut PlayerInventory), With<Client>>,
) {
    for (entity, mut session, mut inventory) in sessions.iter_mut() {
        if tick_session(&mut session, 1) {
            // session 完成
            let Some(recipe) = registry.get(&session.recipe_id) else {
                tracing::warn!(
                    "[bong][craft] tick finalize: recipe `{}` missing in registry",
                    session.recipe_id
                );
                failed_tx.send(CraftFailedEvent {
                    caster: entity,
                    recipe_id: session.recipe_id.clone(),
                    reason: CraftFailureReason::InternalError,
                    material_returned: 0,
                    qi_refunded: 0.0,
                });
                commands
                    .entity(entity)
                    .remove::<CraftSession>()
                    .insert(CraftSessionStateDirty);
                continue;
            };
            let FinalizeCraftOutcome {
                event,
                output_manifest,
            } = finalize_craft(&session, recipe, entity, clock.tick);
            let (template, count) = output_manifest;
            // review fix (Codex P1)：产物入背包失败时不能静默——qi 已扣材料已耗，
            // 玩家必须知道任务失败而不是显示一条假"出炉成功"。改 emit Failed
            // (InternalError)，让 client 渲染失败 toast；不送 Completed 事件。
            match add_item_to_player_inventory(
                &mut inventory,
                &item_registry,
                &mut allocator,
                &template,
                count,
                clock.tick,
            ) {
                Ok(_) => {
                    let next_completed = session.completed_count.saturating_add(1);
                    tracing::info!(
                        "[bong][craft] finalize caster={entity:?} recipe={} output={template} x{count} completed={}/{}",
                        event.recipe_id,
                        next_completed,
                        session.quantity_total
                    );
                    completed_tx.send(event);
                    if next_completed < session.quantity_total {
                        session.completed_count = next_completed;
                        session.remaining_ticks = session.total_ticks;
                        commands.entity(entity).insert(CraftSessionStateDirty);
                        continue;
                    }
                }
                Err(err) => {
                    tracing::error!(
                        "[bong][craft] finalize FAILED: recipe={} output={template} x{count} grant_err={err} — cancel remaining batch and refund materials",
                        event.recipe_id
                    );
                    let CancelCraftOutcome {
                        mut event,
                        refund_manifest,
                    } = cancel_craft(&session, recipe, entity, CraftFailureReason::InternalError);
                    let ground_target = refund_ground_context(player_contexts.get(entity).ok());
                    let refund_summary = grant_refund_manifest_to_inventory_or_ground(
                        &mut inventory,
                        &item_registry,
                        &mut allocator,
                        dropped_loot.as_deref_mut(),
                        refund_manifest,
                        clock.tick,
                        ground_target,
                    );
                    if !refund_summary.errors.is_empty() {
                        tracing::error!(
                            "[bong][craft] refund had structural grant errors after finalize failure: recipe={} errors={:?}",
                            event.recipe_id,
                            refund_summary.errors
                        );
                    }
                    event.material_returned = refund_summary.material_returned;
                    failed_tx.send(event);
                }
            }
            commands
                .entity(entity)
                .remove::<CraftSession>()
                .insert(CraftSessionStateDirty);
        } else if clock.tick.is_multiple_of(SESSION_STATE_PUSH_INTERVAL_TICKS) {
            // 每秒标脏一次让 emit 系统下一帧推 progress
            commands.entity(entity).insert(CraftSessionStateDirty);
        }
    }
}

/// §3 — 推 SessionState payload。包含两条路径：
///   * dirty 标记：状态切换瞬间立刻推一次（启动 / 取消 / 完成 / 拒绝）
///   * 周期推送：每 SESSION_STATE_PUSH_INTERVAL_TICKS tick 一次（进度同步）
pub fn emit_craft_session_state(
    mut commands: Commands,
    names: Query<&Username>,
    mut clients: Query<&mut Client>,
    sessions_with_dirty: Query<(Entity, Option<&CraftSession>), With<CraftSessionStateDirty>>,
) {
    for (entity, session) in sessions_with_dirty.iter() {
        let player_id = match names.get(entity) {
            Ok(u) => canonical_player_id(u.0.as_str()),
            Err(_) => continue,
        };
        let Ok(mut client) = clients.get_mut(entity) else {
            commands.entity(entity).remove::<CraftSessionStateDirty>();
            continue;
        };
        let payload = ServerDataPayloadV1::CraftSessionState(build_session_state_payload(
            &player_id, session,
        ));
        send_payload(&mut client, payload, &format!("session_state {entity:?}"));
        commands.entity(entity).remove::<CraftSessionStateDirty>();
    }
}

/// §4 — 监听 CraftCompleted/CraftFailed → push CraftOutcome 给 caster。
pub fn emit_craft_outcome_payloads(
    mut completed: EventReader<CraftCompletedEvent>,
    mut failed: EventReader<CraftFailedEvent>,
    names: Query<&Username>,
    mut clients: Query<&mut Client>,
) {
    for event in completed.read() {
        let player_id = match names.get(event.caster) {
            Ok(u) => canonical_player_id(u.0.as_str()),
            Err(_) => continue,
        };
        let Ok(mut client) = clients.get_mut(event.caster) else {
            continue;
        };
        let outcome = CraftOutcomeV1::Completed {
            v: 1,
            player_id: player_id.clone(),
            recipe_id: event.recipe_id.as_str().to_string(),
            output_template: event.output_template.clone(),
            output_count: event.output_count,
            completed_at_tick: event.completed_at_tick,
            ts: current_unix_millis(),
        };
        send_payload(
            &mut client,
            ServerDataPayloadV1::CraftOutcome(outcome),
            "outcome::completed",
        );
    }
    for event in failed.read() {
        let player_id = match names.get(event.caster) {
            Ok(u) => canonical_player_id(u.0.as_str()),
            Err(_) => continue,
        };
        let Ok(mut client) = clients.get_mut(event.caster) else {
            continue;
        };
        let outcome = CraftOutcomeV1::Failed {
            v: 1,
            player_id: player_id.clone(),
            recipe_id: event.recipe_id.as_str().to_string(),
            reason: map_failure_reason(event.reason),
            material_returned: event.material_returned,
            qi_refunded: event.qi_refunded,
            ts: current_unix_millis(),
        };
        send_payload(
            &mut client,
            ServerDataPayloadV1::CraftOutcome(outcome),
            "outcome::failed",
        );
    }
}

/// §5 — 监听 RecipeUnlockedEvent → push RecipeUnlockedV1 给 caster。
pub fn emit_recipe_unlocked_payloads(
    mut events: EventReader<RecipeUnlockedEvent>,
    registry: Res<CraftRegistry>,
    unlock_state: Res<RecipeUnlockState>,
    names: Query<&Username>,
    mut clients: Query<&mut Client>,
) {
    for event in events.read() {
        let player_id = match names.get(event.caster) {
            Ok(u) => canonical_player_id(u.0.as_str()),
            Err(_) => continue,
        };
        let Ok(mut client) = clients.get_mut(event.caster) else {
            continue;
        };
        let payload = RecipeUnlockedV1 {
            v: 1,
            player_id: player_id.clone(),
            recipe_id: event.recipe_id.as_str().to_string(),
            source: UnlockEventSourceV1::from(event.source.clone()),
            unlocked_at_tick: event.unlocked_at_tick,
            ts: current_unix_millis(),
        };
        send_payload(
            &mut client,
            ServerDataPayloadV1::RecipeUnlocked(payload),
            "recipe_unlocked",
        );
        let list = build_recipe_list_payload(&player_id, &registry, &unlock_state);
        send_payload(
            &mut client,
            ServerDataPayloadV1::CraftRecipeList(Box::new(list)),
            "recipe_list::unlock_refresh",
        );
    }
}

/// §6 — 玩家上线 / 解锁后推 `RecipeListV1` 全表（含解锁状态）。
///
/// P2 简化：每个在线玩家成功推一次。不能只查 `Added<Client>`，因为
/// `Username` / inventory 等组件可能在 join 后续系统才挂上，单帧查询会漏发。
/// 后续 unlock 增量靠 `RecipeUnlockedV1` 单条推。
pub fn emit_recipe_list_on_join(
    registry: Res<CraftRegistry>,
    unlock_state: Res<RecipeUnlockState>,
    mut sent: Local<HashMap<Entity, String>>,
    mut clients: Query<(Entity, &Username, &mut Client, Option<&CraftSession>), With<Client>>,
) {
    let mut active_clients = HashSet::new();
    for (entity, username, mut client, session) in clients.iter_mut() {
        active_clients.insert(entity);
        let player_id = canonical_player_id(username.0.as_str());
        if sent
            .get(&entity)
            .is_some_and(|cached_player_id| cached_player_id == &player_id)
        {
            continue;
        }
        let payload = build_recipe_list_payload(&player_id, &registry, &unlock_state);
        if send_payload(
            &mut client,
            ServerDataPayloadV1::CraftRecipeList(Box::new(payload)),
            "recipe_list::join",
        ) && send_payload(
            &mut client,
            ServerDataPayloadV1::CraftSessionState(build_session_state_payload(
                &player_id, session,
            )),
            "session_state::join",
        ) {
            sent.insert(entity, player_id);
        }
    }
    sent.retain(|entity, _| active_clients.contains(entity));
}

/// §7 — plan-craft-v1 P3 三渠道解锁 intent 处理。
///
/// 各 source plan 按自身条件触发时 emit `CraftUnlockIntent`，本系统统一
/// 把它们路由到对应的 `unlock_via_*` 函数 + emit `RecipeUnlockedEvent`。
/// SourceMismatch / Already 都视为 noop（不广播，不影响业务）。
///
/// 出现的 narration 由后续 `emit_recipe_unlocked_payloads` 给 client，
/// `craft_event_bridge` 给 agent。
pub fn apply_unlock_intents(
    mut intents: EventReader<CraftUnlockIntent>,
    mut unlocked_tx: EventWriter<RecipeUnlockedEvent>,
    mut unlock_state: ResMut<RecipeUnlockState>,
    registry: Res<CraftRegistry>,
    clock: Res<CombatClock>,
    names: Query<&Username>,
) {
    for intent in intents.read() {
        let player_id = match names.get(intent.caster) {
            Ok(u) => canonical_player_id(u.0.as_str()),
            Err(_) => format!("entity:{}", intent.caster.to_bits()),
        };
        let Some(recipe) = registry.get(&intent.recipe_id) else {
            tracing::warn!(
                "[bong][craft] unlock intent ignored: recipe `{}` not in registry",
                intent.recipe_id
            );
            continue;
        };
        let outcome = match &intent.source {
            UnlockEventSource::Scroll { item_template } => {
                unlock_via_scroll(&mut unlock_state, &player_id, recipe, item_template)
            }
            UnlockEventSource::Mentor { npc_archetype } => {
                unlock_via_mentor(&mut unlock_state, &player_id, recipe, npc_archetype)
            }
            UnlockEventSource::Insight { trigger } => {
                unlock_via_insight(&mut unlock_state, &player_id, recipe, *trigger)
            }
        };
        match outcome {
            UnlockOutcome::Newly { source } => {
                tracing::info!(
                    "[bong][craft] unlock newly player={} recipe={} source={:?}",
                    player_id,
                    recipe.id,
                    source
                );
                unlocked_tx.send(RecipeUnlockedEvent {
                    caster: intent.caster,
                    recipe_id: recipe.id.clone(),
                    source,
                    unlocked_at_tick: clock.tick,
                });
            }
            UnlockOutcome::Already => {
                tracing::debug!(
                    "[bong][craft] unlock already-known player={} recipe={}",
                    player_id,
                    recipe.id
                );
            }
            UnlockOutcome::SourceMismatch => {
                tracing::debug!(
                    "[bong][craft] unlock source mismatch player={} recipe={} (intent source did not match recipe.unlock_sources)",
                    player_id,
                    recipe.id
                );
            }
        }
    }
}

/// 构造 `RecipeListV1` payload（按 `grouped_for_ui` 排序，含解锁状态）。
pub fn build_recipe_list_payload(
    player_id: &str,
    registry: &CraftRegistry,
    unlock_state: &RecipeUnlockState,
) -> RecipeListV1 {
    let entries: Vec<CraftRecipeEntryV1> = registry
        .grouped_for_ui()
        .into_iter()
        .flat_map(|(_, recipes)| recipes.into_iter())
        // plan-craft-material-discovery：未解锁配方一律不下发，客户端只展示已解锁列表。
        // 空 unlock_sources 不再"默认解锁"——基础配方须经材料发现写入 unlock_state
        // 后才出现（apply_material_discovery_unlock）。若以后改为灰显锁定配方，
        // 需同步扩展 payload 与客户端交互。
        .filter(|r| unlock_state.is_unlocked(player_id, &r.id))
        .map(|r| CraftRecipeEntryV1 {
            id: r.id.as_str().to_string(),
            category: CraftCategoryV1::from(r.category),
            display_name: r.display_name.clone(),
            materials: r.materials.clone(),
            qi_cost: r.qi_cost,
            time_ticks: r.time_ticks,
            output: r.output.clone(),
            requirements: CraftRequirementsV1 {
                realm_min: r.requirements.realm_min,
                qi_color_min: r.requirements.qi_color_min,
                skill_lv_min: r.requirements.skill_lv_min,
            },
            // 过滤后此处恒为 true；保留显式赋值作 safeguard——若未来移除上面的
            // filter（改为灰显锁定配方），这一行能继续如实反映解锁态而不至于语义破裂。
            unlocked: unlock_state.is_unlocked(player_id, &r.id),
            // 下发 station 让客户端分流手搓台 / 制作台（此前漏发 → workbench 配方泄漏到
            // 手搓台、点制作 StationOutOfRange 静默失败）。None=手搓配方。
            station: r.station.map(|s| s.as_str().to_string()),
        })
        .collect();
    RecipeListV1 {
        v: 1,
        player_id: player_id.to_string(),
        recipes: entries,
        ts: current_unix_millis(),
    }
}

/// plan-craft-material-discovery — 被动材料发现解锁。
///
/// 对【无显式解锁来源】且【原料含背包中任一物品】的配方，自动解锁、刷新该玩家
/// 配方列表、推一条 narration。残卷/师承/顿悟门控的秘传配方不受影响
/// （`unlock_via_material` 内部跳过）。
///
/// 性能：Query 用 `Changed<PlayerInventory>` 过滤 —— 只在玩家背包**有变动**的
/// tick 才扫描该玩家（背包不变时整玩家跳过，稳态零成本）。`Added` ⊆ `Changed`，
/// 故玩家进场 / 持久化加载那 tk 背包被 attach 时也会命中一次，覆盖"开局已有原料
/// 的初始解锁"。单次扫描复杂度 O(|配方|)，对已解锁/秘传配方提前 `continue` 短路；
/// 仅确有新解锁时才构造并下发一次 `RecipeListV1`。
pub fn apply_material_discovery_unlock(
    registry: Res<CraftRegistry>,
    mut unlock_state: ResMut<RecipeUnlockState>,
    mut narrations: Option<ResMut<PendingGameplayNarrations>>,
    mut players: Query<(&Username, &PlayerInventory, &mut Client), Changed<PlayerInventory>>,
) {
    for (username, inventory, mut client) in players.iter_mut() {
        let player_id = canonical_player_id(username.0.as_str());
        let mut newly: Vec<String> = Vec::new();
        for recipe in registry.iter() {
            // 秘传配方 + 已解锁配方提前短路（稳态零成本）。
            if !recipe.unlock_sources.is_empty() || unlock_state.is_unlocked(&player_id, &recipe.id)
            {
                continue;
            }
            // 持有任一原料即可发现（.find 命中即停）。
            let Some(template) = recipe
                .materials
                .iter()
                .map(|(t, _)| t.as_str())
                .find(|t| count_template_in_inventory(inventory, t) > 0)
            else {
                continue;
            };
            if unlock_via_material(&mut unlock_state, &player_id, recipe, template)
                == MaterialUnlockOutcome::Newly
            {
                tracing::info!(
                    "[bong][craft] material-discovery unlock player={} recipe={} via={}",
                    player_id,
                    recipe.id,
                    template
                );
                newly.push(recipe.display_name.clone());
            }
        }
        if newly.is_empty() {
            continue;
        }
        // 重推配方列表，client 把新解锁配方加进 CraftScreen。
        let list = build_recipe_list_payload(&player_id, &registry, &unlock_state);
        send_payload(
            &mut client,
            ServerDataPayloadV1::CraftRecipeList(Box::new(list)),
            "recipe_list::material_discovery",
        );
        if let Some(ref mut narr) = narrations {
            let msg = if newly.len() == 1 {
                format!("悟得【{}】的制法。", newly[0])
            } else {
                format!("悟得 {} 种新制法：{}。", newly.len(), newly.join("、"))
            };
            narr.push_player(username.0.as_str(), &msg, NarrationStyle::Perception);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::craft::{
        register_basic_processing_recipes, register_examples, CraftCategory, CraftRecipe,
        CraftRequirements, CraftSession, RecipeId, RecipeUnlockState, UnlockSource,
    };
    use crate::inventory::{
        ContainerState, DroppedLootRegistry, InventoryInstanceIdAllocator, InventoryRevision,
        ItemCategory, ItemInstance, ItemRarity, ItemRegistry, ItemTemplate, PlacedItemState,
    };
    use std::collections::HashMap;
    use valence::prelude::{App, Events, Update};
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::{create_mock_client, MockClientHelper};

    /// 造一个最小 PlayerInventory，main_pack 内放指定 (template, count)。
    fn inv_with(items: &[(&str, u32)]) -> PlayerInventory {
        let placed: Vec<PlacedItemState> = items
            .iter()
            .enumerate()
            .map(|(idx, (template, n))| PlacedItemState {
                row: idx as u8,
                col: 0,
                instance: ItemInstance {
                    instance_id: idx as u64 + 1,
                    template_id: (*template).into(),
                    display_name: (*template).into(),
                    grid_w: 1,
                    grid_h: 1,
                    weight: 1.0,
                    rarity: ItemRarity::Common,
                    description: String::new(),
                    stack_count: *n,
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
                },
            })
            .collect();
        PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(1),
            containers: vec![ContainerState {
                quick_access: false,
                id: "main_pack".into(),
                name: "main".into(),
                rows: 16,
                cols: 1,
                items: placed,
                owner_instance_id: None,
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 100.0,
        }
    }

    fn registry_with_templates(templates: &[(&str, u32)]) -> ItemRegistry {
        let templates = templates
            .iter()
            .map(|(id, max_stack_count)| {
                let template = ItemTemplate {
                    id: (*id).to_string(),
                    display_name: (*id).to_string(),
                    category: ItemCategory::Misc,
                    placeable: None,
                    max_stack_count: *max_stack_count,
                    grid_w: 1,
                    grid_h: 1,
                    base_weight: 1.0,
                    rarity: ItemRarity::Common,
                    spirit_quality_initial: 0.0,
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
                    shield_spec: None,
                    shelflife_profile: None,
                    shelflife_track: None,
                };
                ((*id).to_string(), template)
            })
            .collect();
        ItemRegistry::from_map(templates)
    }

    fn clamp_main_pack_to_grid(inventory: &mut PlayerInventory, rows: u8, cols: u8) {
        inventory.containers[0].rows = rows;
        inventory.containers[0].cols = cols;
    }

    fn craft_refund_test_app(
        recipe: CraftRecipe,
        templates: &[(&str, u32)],
        clock_tick: u64,
    ) -> App {
        let mut app = App::new();
        let mut registry = CraftRegistry::new();
        registry.register(recipe).unwrap();
        app.insert_resource(registry);
        app.insert_resource(RecipeUnlockState::new());
        app.insert_resource(WorldQiAccount::default());
        app.insert_resource(registry_with_templates(templates));
        app.insert_resource(InventoryInstanceIdAllocator::new(100));
        app.insert_resource(DroppedLootRegistry::default());
        app.insert_resource(CombatClock { tick: clock_tick });
        app.add_event::<CraftStartIntent>();
        app.add_event::<CraftCancelIntent>();
        app.add_event::<CraftStartedEvent>();
        app.add_event::<CraftCompletedEvent>();
        app.add_event::<CraftFailedEvent>();
        app
    }

    fn current_failed_events(app: &App) -> Vec<CraftFailedEvent> {
        app.world()
            .resource::<Events<CraftFailedEvent>>()
            .iter_current_update_events()
            .cloned()
            .collect()
    }

    fn current_completed_events(app: &App) -> Vec<CraftCompletedEvent> {
        app.world()
            .resource::<Events<CraftCompletedEvent>>()
            .iter_current_update_events()
            .cloned()
            .collect()
    }

    /// 造一个 craft 配方，指定 id / 原料 / 解锁来源（空 vec = 材料发现路径）。
    fn make_recipe(id: &str, materials: &[(&str, u32)], sources: Vec<UnlockSource>) -> CraftRecipe {
        CraftRecipe {
            id: RecipeId::new(id),
            category: CraftCategory::Tool,
            display_name: id.into(),
            materials: materials
                .iter()
                .map(|(t, c)| ((*t).to_string(), *c))
                .collect(),
            qi_cost: 0.0,
            time_ticks: 60,
            output: ("out".into(), 1),
            requirements: CraftRequirements::default(),
            unlock_sources: sources,
            station: None,
        }
    }

    /// 解锁 registry 内全部配方（构造"全解锁"基线，用于列表排序 / 体积上限测试）。
    fn unlock_all(unlock_state: &mut RecipeUnlockState, player: &str, registry: &CraftRegistry) {
        for r in registry.iter() {
            unlock_state.unlock(player.to_string(), r.id.clone());
        }
    }

    fn flush_client_packets(app: &mut App) {
        let world = app.world_mut();
        let mut query = world.query::<&mut Client>();
        for mut client in query.iter_mut(world) {
            client
                .flush_packets()
                .expect("mock client packets should flush");
        }
    }

    fn collect_recipe_lists(helper: &mut MockClientHelper) -> Vec<RecipeListV1> {
        let mut out = Vec::new();
        for frame in helper.collect_received().0 {
            let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                continue;
            };
            if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                continue;
            }
            let payload = serde_json::from_slice::<serde_json::Value>(packet.data.0 .0)
                .expect("server_data payload should decode as JSON");
            if payload.get("type").and_then(|v| v.as_str()) == Some("craft_recipe_list") {
                let mut list_payload = payload;
                if let Some(object) = list_payload.as_object_mut() {
                    object.remove("type");
                }
                let list = serde_json::from_value::<RecipeListV1>(list_payload)
                    .expect("craft_recipe_list payload should decode");
                out.push(list);
            }
        }
        out
    }

    fn collect_craft_session_states(helper: &mut MockClientHelper) -> Vec<CraftSessionStateV1> {
        let mut out = Vec::new();
        for frame in helper.collect_received().0 {
            let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                continue;
            };
            if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                continue;
            }
            let payload = serde_json::from_slice::<serde_json::Value>(packet.data.0 .0)
                .expect("server_data payload should decode as JSON");
            if payload.get("type").and_then(|v| v.as_str()) == Some("craft_session_state") {
                let mut state_payload = payload;
                if let Some(object) = state_payload.as_object_mut() {
                    object.remove("type");
                }
                let state = serde_json::from_value::<CraftSessionStateV1>(state_payload)
                    .expect("craft_session_state payload should decode");
                out.push(state);
            }
        }
        out
    }

    #[test]
    fn build_session_state_inactive() {
        let state = build_session_state_payload("offline:Alice", None);
        assert!(!state.active);
        assert!(state.recipe_id.is_none());
        assert_eq!(state.elapsed_ticks, 0);
        assert_eq!(state.total_ticks, 0);
    }

    #[test]
    fn build_session_state_active_reflects_elapsed() {
        let session = CraftSession {
            recipe_id: RecipeId::new("craft.test.x"),
            started_at_tick: 0,
            remaining_ticks: 30,
            total_ticks: 100,
            owner_player_id: "offline:Alice".into(),
            qi_paid: 5.0,
            quantity_total: 1,
            completed_count: 0,
        };
        let state = build_session_state_payload("offline:Alice", Some(&session));
        assert!(state.active);
        assert_eq!(state.recipe_id.as_deref(), Some("craft.test.x"));
        assert_eq!(state.elapsed_ticks, 70);
        assert_eq!(state.total_ticks, 100);
        assert_eq!(state.completed_count, 0);
        assert_eq!(state.total_count, 1);
    }

    #[test]
    fn build_session_state_completed_session_shows_full_elapsed() {
        let session = CraftSession {
            recipe_id: RecipeId::new("craft.test.y"),
            started_at_tick: 0,
            remaining_ticks: 0,
            total_ticks: 100,
            owner_player_id: "offline:Bob".into(),
            qi_paid: 5.0,
            quantity_total: 1,
            completed_count: 0,
        };
        let state = build_session_state_payload("offline:Bob", Some(&session));
        assert_eq!(state.elapsed_ticks, 100);
        assert_eq!(state.total_ticks, 100);
        assert_eq!(state.completed_count, 0);
        assert_eq!(state.total_count, 1);
    }

    #[test]
    fn map_failure_reason_covers_all_variants() {
        assert_eq!(
            map_failure_reason(CraftFailureReason::PlayerCancelled),
            CraftFailureReasonV1::PlayerCancelled
        );
        assert_eq!(
            map_failure_reason(CraftFailureReason::PlayerDied),
            CraftFailureReasonV1::PlayerDied
        );
        assert_eq!(
            map_failure_reason(CraftFailureReason::InternalError),
            CraftFailureReasonV1::InternalError
        );
    }

    #[test]
    fn refund_manifest_full_inventory_drops_to_ground() {
        let registry = registry_with_templates(&[("fan_tie", 64)]);
        let mut inventory = inv_with(&[("occupant", 1)]);
        clamp_main_pack_to_grid(&mut inventory, 1, 1);
        let mut allocator = InventoryInstanceIdAllocator::new(20);
        let mut dropped_loot = DroppedLootRegistry::default();
        let ground_pos = [7.0, 65.0, -3.0];

        let summary = grant_refund_manifest_to_inventory_or_ground(
            &mut inventory,
            &registry,
            &mut allocator,
            Some(&mut dropped_loot),
            vec![("fan_tie".to_string(), 1)],
            123,
            RefundGroundTarget {
                pos: ground_pos,
                dimension: DimensionKind::Tsy,
            },
        );

        assert_eq!(summary.material_returned, 1);
        assert_eq!(summary.granted_count, 0);
        assert_eq!(summary.dropped_count, 1);
        assert!(
            summary.errors.is_empty(),
            "满包但 DroppedLootRegistry 可用时不应出现结构性错误：{:?}",
            summary.errors
        );
        assert_eq!(dropped_loot.entries.len(), 1);
        let entry = dropped_loot.entries.values().next().unwrap();
        assert_eq!(entry.item.template_id, "fan_tie");
        assert_eq!(entry.item.stack_count, 1);
        assert_eq!(entry.world_pos, ground_pos);
        assert_eq!(entry.dimension, DimensionKind::Tsy);
        assert!(
            inventory.containers[0]
                .items
                .iter()
                .all(|placed| placed.instance.template_id != "fan_tie"),
            "满包退款应落地，不应写进已满容器"
        );
    }

    #[test]
    fn refund_manifest_mixed_grant_and_drop_counts_actual_returned() {
        let registry = registry_with_templates(&[("fan_tie", 64), ("zhu_pi", 64)]);
        let mut inventory = inv_with(&[("fan_tie", 63), ("occupant", 1)]);
        clamp_main_pack_to_grid(&mut inventory, 2, 1);
        let mut allocator = InventoryInstanceIdAllocator::new(30);
        let mut dropped_loot = DroppedLootRegistry::default();

        let summary = grant_refund_manifest_to_inventory_or_ground(
            &mut inventory,
            &registry,
            &mut allocator,
            Some(&mut dropped_loot),
            vec![("fan_tie".to_string(), 1), ("zhu_pi".to_string(), 1)],
            77,
            RefundGroundTarget {
                pos: [0.0, 70.0, 0.0],
                dimension: DimensionKind::Overworld,
            },
        );

        assert_eq!(
            summary.material_returned, 2,
            "实际返还数必须统计成功入包 + 成功落地"
        );
        assert_eq!(summary.granted_count, 1);
        assert_eq!(summary.dropped_count, 1);
        assert!(summary.errors.is_empty());
        let fan_tie_stack = inventory.containers[0]
            .items
            .iter()
            .find(|placed| placed.instance.template_id == "fan_tie")
            .expect("fan_tie refund should merge into existing stack");
        assert_eq!(fan_tie_stack.instance.stack_count, 64);
        assert_eq!(dropped_loot.entries.len(), 1);
        let dropped = dropped_loot.entries.values().next().unwrap();
        assert_eq!(dropped.item.template_id, "zhu_pi");
        assert_eq!(dropped.item.stack_count, 1);
    }

    #[test]
    fn refund_manifest_full_inventory_without_registry_reports_error_without_counting_returned() {
        let registry = registry_with_templates(&[("fan_tie", 64)]);
        let mut inventory = inv_with(&[("occupant", 1)]);
        clamp_main_pack_to_grid(&mut inventory, 1, 1);
        let mut allocator = InventoryInstanceIdAllocator::new(40);

        let summary = grant_refund_manifest_to_inventory_or_ground(
            &mut inventory,
            &registry,
            &mut allocator,
            None,
            vec![("fan_tie".to_string(), 1)],
            1,
            RefundGroundTarget {
                pos: [0.0, 64.0, 0.0],
                dimension: DimensionKind::Overworld,
            },
        );

        assert_eq!(
            summary.material_returned, 0,
            "无 DroppedLootRegistry 且背包满时不能虚报已返还"
        );
        assert_eq!(summary.granted_count, 0);
        assert_eq!(summary.dropped_count, 0);
        assert_eq!(summary.errors.len(), 1);
        assert!(
            summary.errors[0].contains("no DroppedLootRegistry"),
            "错误必须暴露缺少落地兜底的结构问题，实际={:?}",
            summary.errors
        );
    }

    #[test]
    fn refund_manifest_unknown_template_does_not_create_ground_drop() {
        let registry = registry_with_templates(&[("fan_tie", 64)]);
        let mut inventory = inv_with(&[]);
        clamp_main_pack_to_grid(&mut inventory, 1, 1);
        let mut allocator = InventoryInstanceIdAllocator::new(50);
        let mut dropped_loot = DroppedLootRegistry::default();

        let summary = grant_refund_manifest_to_inventory_or_ground(
            &mut inventory,
            &registry,
            &mut allocator,
            Some(&mut dropped_loot),
            vec![("missing_template".to_string(), 1)],
            1,
            RefundGroundTarget {
                pos: [0.0, 64.0, 0.0],
                dimension: DimensionKind::Overworld,
            },
        );

        assert_eq!(summary.material_returned, 0);
        assert_eq!(summary.granted_count, 0);
        assert_eq!(summary.dropped_count, 0);
        assert_eq!(summary.errors.len(), 1);
        assert!(
            summary.errors[0].contains("unknown item template id"),
            "unknown template 是配置错误，不应伪装成满包落地：{:?}",
            summary.errors
        );
        assert!(
            dropped_loot.entries.is_empty(),
            "结构性错误不能产生 DroppedLootRegistry 条目"
        );
        assert!(inventory.containers[0].items.is_empty());
    }

    #[test]
    fn cancel_refund_full_inventory_drops_to_ground_and_reports_actual_returned() {
        let recipe = make_recipe("craft.test.refund_cancel", &[("fan_tie", 2)], vec![]);
        let mut app = craft_refund_test_app(recipe, &[("fan_tie", 64)], 456);
        app.add_systems(Update, apply_craft_cancel_intents);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let mut inventory = inv_with(&[("occupant", 1)]);
        clamp_main_pack_to_grid(&mut inventory, 1, 1);
        inventory.bone_coins = 77;
        let player = app
            .world_mut()
            .spawn(client_bundle)
            .insert(inventory)
            .insert(Cultivation::default())
            .insert(QiColor::default())
            .insert(Position::new([11.0, 65.0, -2.0]))
            .insert(CurrentDimension(DimensionKind::Tsy))
            .insert(CraftSession {
                recipe_id: RecipeId::new("craft.test.refund_cancel"),
                started_at_tick: 400,
                remaining_ticks: 20,
                total_ticks: 40,
                owner_player_id: canonical_player_id("Azure"),
                qi_paid: 0.0,
                quantity_total: 1,
                completed_count: 0,
            })
            .id();

        app.world_mut()
            .send_event(CraftCancelIntent { caster: player });
        app.update();

        let failed = current_failed_events(&app);
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].reason, CraftFailureReason::PlayerCancelled);
        assert_eq!(
            failed[0].material_returned, 1,
            "material_returned 必须按实际入包 + 落地成功数统计，不能沿用预估数虚报"
        );
        assert_eq!(failed[0].qi_refunded, 0.0, "craft 取消不退真元");
        assert!(
            app.world().get::<CraftSession>(player).is_none(),
            "退款落地成功后 session 应正常结束"
        );

        let inventory = app.world().get::<PlayerInventory>(player).unwrap();
        assert_eq!(inventory.bone_coins, 77, "退款材料不得改动骨币");
        assert!(
            inventory.containers[0]
                .items
                .iter()
                .all(|placed| placed.instance.template_id != "fan_tie"),
            "背包满时 fan_tie 不应被伪造进背包"
        );

        let dropped = app.world().resource::<DroppedLootRegistry>();
        assert_eq!(dropped.entries.len(), 1);
        let entry = dropped.entries.values().next().unwrap();
        assert_eq!(entry.item.template_id, "fan_tie");
        assert_eq!(entry.item.stack_count, 1);
        assert_eq!(entry.world_pos, [11.0, 65.0, -2.0]);
        assert_eq!(entry.dimension, DimensionKind::Tsy);
    }

    #[test]
    fn finalize_failure_refund_full_inventory_drops_to_ground_without_bone_coin_drift() {
        let recipe = make_recipe("craft.test.refund_finalize", &[("fan_tie", 2)], vec![]);
        let mut app = craft_refund_test_app(recipe, &[("fan_tie", 64), ("out", 64)], 789);
        app.add_systems(Update, tick_craft_sessions);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let mut inventory = inv_with(&[("occupant", 1)]);
        clamp_main_pack_to_grid(&mut inventory, 1, 1);
        inventory.bone_coins = 88;
        let player = app
            .world_mut()
            .spawn(client_bundle)
            .insert(inventory)
            .insert(Position::new([-4.0, 66.0, 9.0]))
            .insert(CurrentDimension(DimensionKind::Overworld))
            .insert(CraftSession {
                recipe_id: RecipeId::new("craft.test.refund_finalize"),
                started_at_tick: 700,
                remaining_ticks: 1,
                total_ticks: 1,
                owner_player_id: canonical_player_id("Azure"),
                qi_paid: 0.0,
                quantity_total: 1,
                completed_count: 0,
            })
            .id();

        app.update();

        assert!(
            current_completed_events(&app).is_empty(),
            "产物入包失败不能发 Completed"
        );
        let failed = current_failed_events(&app);
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].reason, CraftFailureReason::InternalError);
        assert_eq!(
            failed[0].material_returned, 1,
            "产物入包失败后的剩余批次退款也必须统计实际落地成功数"
        );
        assert!(
            app.world().get::<CraftSession>(player).is_none(),
            "finalize 失败退款落地后 session 应结束，不能保留僵尸状态"
        );

        let inventory = app.world().get::<PlayerInventory>(player).unwrap();
        assert_eq!(inventory.bone_coins, 88, "材料退款不得增减骨币");
        assert!(
            inventory.containers[0]
                .items
                .iter()
                .all(|placed| placed.instance.template_id != "fan_tie"
                    && placed.instance.template_id != "out"),
            "满包时产物和退款材料都不应被写进已满容器"
        );

        let dropped = app.world().resource::<DroppedLootRegistry>();
        assert_eq!(dropped.entries.len(), 1);
        let entry = dropped.entries.values().next().unwrap();
        assert_eq!(
            entry.item.template_id, "fan_tie",
            "落地兜底只用于退款材料；产物 grant 失败仍按失败事件处理"
        );
        assert_eq!(entry.item.stack_count, 1);
        assert_eq!(entry.world_pos, [-4.0, 66.0, 9.0]);
        assert_eq!(entry.dimension, DimensionKind::Overworld);
    }

    #[test]
    fn build_recipe_list_payload_hides_empty_source_recipes_until_unlocked() {
        // plan-craft-material-discovery：空源配方不再"默认下发"。空 unlock_state 下
        // 列表应为空；写入 unlock_state（模拟材料发现）后对应配方才出现且 unlocked。
        let mut registry = CraftRegistry::new();
        register_examples(&mut registry).unwrap();

        let empty = RecipeUnlockState::new();
        let payload = build_recipe_list_payload("offline:Alice", &registry, &empty);
        assert_eq!(payload.player_id, "offline:Alice");
        assert!(
            payload.recipes.is_empty(),
            "未解锁任何配方时列表应为空（空源配方不再默认下发），实际={:?}",
            payload.recipes.iter().map(|r| &r.id).collect::<Vec<_>>()
        );

        let mut unlock_state = RecipeUnlockState::new();
        unlock_state.unlock(
            "offline:Alice",
            RecipeId::new("craft.example.eclipse_needle.iron"),
        );
        let payload = build_recipe_list_payload("offline:Alice", &registry, &unlock_state);
        assert_eq!(payload.recipes.len(), 1);
        assert!(payload
            .recipes
            .iter()
            .any(|r| r.id == "craft.example.eclipse_needle.iron" && r.unlocked));
        // 未解锁的其它配方仍不下发
        assert!(payload
            .recipes
            .iter()
            .all(|r| r.id != "craft.example.poison_decoction.fan"));
    }

    #[test]
    fn build_recipe_list_payload_reflects_partial_unlocks() {
        let mut registry = CraftRegistry::new();
        register_examples(&mut registry).unwrap();
        let mut unlock_state = RecipeUnlockState::new();
        unlock_state.unlock(
            "offline:Alice",
            RecipeId::new("craft.example.fake_skin.light"),
        );
        let payload = build_recipe_list_payload("offline:Alice", &registry, &unlock_state);
        let unlocked = payload
            .recipes
            .iter()
            .find(|r| r.id == "craft.example.fake_skin.light")
            .expect("fake skin recipe should be included");
        assert!(unlocked.unlocked);
    }

    #[test]
    fn build_recipe_list_payload_hides_empty_source_basic_recipe_until_material_unlock() {
        let mut registry = CraftRegistry::new();
        register_basic_processing_recipes(&mut registry).unwrap();

        // 空 unlock_state：空源基础配方不下发。
        let empty = RecipeUnlockState::new();
        let payload = build_recipe_list_payload("offline:Alice", &registry, &empty);
        assert!(
            payload.recipes.iter().all(|r| r.id != "basic.wood_handle"),
            "材料发现解锁前，空源 basic.wood_handle 不应下发"
        );

        // 模拟材料发现解锁后：出现且 unlocked，字段正确。
        let mut unlock_state = RecipeUnlockState::new();
        unlock_state.unlock("offline:Alice", RecipeId::new("basic.wood_handle"));
        let payload = build_recipe_list_payload("offline:Alice", &registry, &unlock_state);
        let wood_handle = payload
            .recipes
            .iter()
            .find(|r| r.id == "basic.wood_handle")
            .expect("解锁后 basic.wood_handle 应出现在列表");
        assert!(wood_handle.unlocked);
        assert_eq!(wood_handle.display_name, "削木柄");
        assert_eq!(wood_handle.materials, vec![("crude_wood".to_string(), 2)]);
        assert_eq!(wood_handle.output, ("wood_handle".to_string(), 2));
    }

    #[test]
    fn build_recipe_list_payload_always_includes_baseline_workbench_recipe() {
        // 基线常显豁免（unlock::BASELINE_RECIPES）：制作台自身配方对空 unlock
        // state 的新玩家必须直接出现在列表里且 unlocked=true —— 它是 workbench
        // 配方树的入口，被材料发现藏住会让玩家不知道有制作台这条路。
        let mut registry = CraftRegistry::new();
        crate::craft::workbench_recipes::register_workbench_recipes(&mut registry).unwrap();

        let empty = RecipeUnlockState::new();
        let payload = build_recipe_list_payload("offline:Newbie", &registry, &empty);
        let workbench = payload
            .recipes
            .iter()
            .find(|r| r.id == "craft.tool.workbench")
            .unwrap_or_else(|| {
                panic!(
                    "期望空 unlock state 下 craft.tool.workbench 仍被下发（基线常显），\
                     实际下发列表={:?}",
                    payload.recipes.iter().map(|r| &r.id).collect::<Vec<_>>()
                )
            });
        assert!(
            workbench.unlocked,
            "基线配方下发时 unlocked 字段必须为 true"
        );
        assert_eq!(
            workbench.station, None,
            "制作台自身是手搓配方（station=None），客户端应把它分流到手搓台"
        );
        // 豁免不外溢：注册表里其余 100+ 空源 workbench 配方在空 state 下仍全部隐藏。
        assert_eq!(
            payload.recipes.len(),
            1,
            "空 unlock state 下应只下发基线配方本身，实际={:?}",
            payload.recipes.iter().map(|r| &r.id).collect::<Vec<_>>()
        );
    }

    #[test]
    fn emit_recipe_list_sends_once_to_online_client() {
        let mut app = App::new();
        let mut registry = CraftRegistry::new();
        register_basic_processing_recipes(&mut registry).unwrap();
        // 预先解锁 wood_handle（模拟材料发现已完成），使 join 列表非空可断言。
        let mut unlock_state = RecipeUnlockState::new();
        unlock_state.unlock(
            canonical_player_id("Azure"),
            RecipeId::new("basic.wood_handle"),
        );
        app.insert_resource(registry);
        app.insert_resource(unlock_state);
        app.add_systems(Update, emit_recipe_list_on_join);

        let (client_bundle, mut helper) = create_mock_client("Azure");
        app.world_mut().spawn(client_bundle);
        app.update();
        flush_client_packets(&mut app);

        let lists = collect_recipe_lists(&mut helper);
        assert_eq!(lists.len(), 1);
        assert!(lists[0].recipes.iter().any(|r| r.id == "basic.wood_handle"));

        app.update();
        flush_client_packets(&mut app);
        assert!(collect_recipe_lists(&mut helper).is_empty());
    }

    #[test]
    fn emit_recipe_list_on_join_also_sends_idle_session_state_without_active_session() {
        let mut app = App::new();
        let mut registry = CraftRegistry::new();
        register_basic_processing_recipes(&mut registry).unwrap();
        let unlock_state = RecipeUnlockState::new();
        app.insert_resource(registry);
        app.insert_resource(unlock_state);
        app.add_systems(Update, emit_recipe_list_on_join);

        let (client_bundle, mut helper) = create_mock_client("Azure");
        app.world_mut().spawn(client_bundle);
        app.update();
        flush_client_packets(&mut app);

        let states = collect_craft_session_states(&mut helper);
        assert_eq!(
            states.len(),
            1,
            "玩家 join 首包必须包含 idle craft_session_state；否则客户端 stale active session 只能靠自身断线清理自愈"
        );
        let state = &states[0];
        assert_eq!(state.player_id, canonical_player_id("Azure"));
        assert!(
            !state.active,
            "无 CraftSession 的新连接必须收到 active=false，实际 state={state:?}"
        );
        assert!(
            state.recipe_id.is_none(),
            "idle session state 不应携带 recipe_id，实际 state={state:?}"
        );
        assert_eq!(state.elapsed_ticks, 0);
        assert_eq!(state.total_ticks, 0);

        app.update();
        flush_client_packets(&mut app);
        assert!(
            collect_craft_session_states(&mut helper).is_empty(),
            "join hydration 已完成后不应每 tick 重复推 idle session state"
        );
    }

    #[test]
    fn emit_recipe_list_on_join_sends_active_session_state_when_session_exists() {
        let mut app = App::new();
        let mut registry = CraftRegistry::new();
        register_basic_processing_recipes(&mut registry).unwrap();
        let unlock_state = RecipeUnlockState::new();
        app.insert_resource(registry);
        app.insert_resource(unlock_state);
        app.add_systems(Update, emit_recipe_list_on_join);

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut().entity_mut(entity).insert(CraftSession {
            recipe_id: RecipeId::new("basic.wood_handle"),
            started_at_tick: 0,
            remaining_ticks: 20,
            total_ticks: 60,
            owner_player_id: canonical_player_id("Azure"),
            qi_paid: 0.0,
            quantity_total: 3,
            completed_count: 1,
        });
        app.update();
        flush_client_packets(&mut app);

        let states = collect_craft_session_states(&mut helper);
        assert_eq!(
            states.len(),
            1,
            "玩家 join 首包必须携带当前 active craft session，避免 client 等待下一次 dirty/progress tick"
        );
        let state = &states[0];
        assert!(
            state.active,
            "已有 CraftSession 时 join hydration 必须 active=true"
        );
        assert_eq!(state.recipe_id.as_deref(), Some("basic.wood_handle"));
        assert_eq!(state.elapsed_ticks, 40);
        assert_eq!(state.total_ticks, 60);
        assert_eq!(state.completed_count, 1);
        assert_eq!(state.total_count, 3);
    }

    #[test]
    fn material_discoverable_recipe_list_fits_server_data_budget() {
        // 本 PR 关心的不变式：材料发现可达集合 = 全部空源配方。一个靠采集解锁了
        // 所有 gather-able 配方的玩家，其 CraftRecipeList 仍能单包下发（不超预算）。
        //
        // 注（既有限制，不在本 PR 范围）：把残卷/师承/顿悟门控配方也全部解锁后的
        // "终态全表"目前约 41KB，超过单包 MAX_PAYLOAD_BYTES(32KB)，需要 CraftRecipeList
        // 分页 / 增量下发来根治。这是 plan-craft-v1 既有的 payload 设计待办，材料发现
        // 改动并未抬高这一上限（终态全表集合与改动前一致）。
        let mut app = App::new();
        crate::craft::register(&mut app);
        let registry = app.world().resource::<CraftRegistry>();
        let mut unlock_state = RecipeUnlockState::new();
        for r in registry.iter() {
            if r.unlock_sources.is_empty() {
                unlock_state.unlock("offline:Alice", r.id.clone());
            }
        }
        let payload = ServerDataV1::new(ServerDataPayloadV1::CraftRecipeList(Box::new(
            build_recipe_list_payload("offline:Alice", registry, &unlock_state),
        )));

        let bytes = serialize_server_data_payload(&payload)
            .expect("material-discoverable craft recipe list must fit server_data budget");
        assert!(
            bytes.len() <= crate::schema::common::MAX_PAYLOAD_BYTES,
            "空源（材料发现）配方全解锁后的列表应在单包预算内，实际 {} 字节 > {}",
            bytes.len(),
            crate::schema::common::MAX_PAYLOAD_BYTES
        );
    }

    #[test]
    fn build_recipe_list_payload_grouped_by_category_for_ui_order() {
        let mut registry = CraftRegistry::new();
        register_examples(&mut registry).unwrap();
        // 全解锁后才能看到跨多类别的完整列表，验证类别分组连续（同类别不交错出现）。
        let mut unlock_state = RecipeUnlockState::new();
        unlock_all(&mut unlock_state, "offline:Charlie", &registry);
        let payload = build_recipe_list_payload("offline:Charlie", &registry, &unlock_state);
        let cats: Vec<CraftCategoryV1> = payload.recipes.iter().map(|r| r.category).collect();
        assert!(cats.len() >= 2, "示例配方应覆盖多个类别");
        // grouped_for_ui 不变式：同一 category 必须连续成段（一旦离开某类别不再回来）。
        let mut seen = std::collections::HashSet::new();
        let mut prev: Option<CraftCategoryV1> = None;
        for cat in &cats {
            if prev != Some(*cat) {
                assert!(
                    seen.insert(*cat),
                    "类别 {cat:?} 非连续出现，违反 grouped_for_ui 分组不变式：{cats:?}"
                );
                prev = Some(*cat);
            }
        }
    }

    #[test]
    fn build_recipe_list_payload_preserves_requirements_qi_color_gate() {
        let mut registry = CraftRegistry::new();
        register_examples(&mut registry).unwrap();
        let mut unlock_state = RecipeUnlockState::new();
        unlock_state.unlock(
            "offline:Y",
            RecipeId::new("craft.example.eclipse_needle.iron"),
        );
        let payload = build_recipe_list_payload("offline:Y", &registry, &unlock_state);
        let needle = payload
            .recipes
            .iter()
            .find(|r| r.id == "craft.example.eclipse_needle.iron")
            .expect("eclipse_needle entry");
        assert!(needle.requirements.qi_color_min.is_some());
    }

    #[test]
    fn requirements_v1_default_omits_optional_fields_in_payload() {
        let r = CraftRequirementsV1 {
            realm_min: None,
            qi_color_min: None,
            skill_lv_min: None,
        };
        let s = serde_json::to_string(&r).unwrap();
        assert!(!s.contains("realm_min"));
        assert!(!s.contains("qi_color_min"));
        assert!(!s.contains("skill_lv_min"));
        // sanity：requirements 即使全 None 也应该序列化干净
        let _: CraftRequirementsV1 = serde_json::from_str(&s).unwrap();
        // unused 静默
        let _ = CraftRequirements::default;
    }

    // ── plan-craft-material-discovery — apply_material_discovery_unlock 系统 ──

    #[test]
    fn material_discovery_unlocks_empty_source_recipe_and_repushes_list() {
        let mut app = App::new();
        let mut registry = CraftRegistry::new();
        // basic.wood_handle：空源、原料 crude_wood
        register_basic_processing_recipes(&mut registry).unwrap();
        app.insert_resource(registry);
        app.insert_resource(RecipeUnlockState::new());
        app.add_systems(Update, apply_material_discovery_unlock);

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut()
            .entity_mut(entity)
            .insert(inv_with(&[("crude_wood", 3)]));
        app.update();
        flush_client_packets(&mut app);

        let player_id = canonical_player_id("Azure");
        let unlock_state = app.world().resource::<RecipeUnlockState>();
        assert!(
            unlock_state.is_unlocked(&player_id, &RecipeId::new("basic.wood_handle")),
            "持有 crude_wood 应被动解锁空源 basic.wood_handle"
        );
        let lists = collect_recipe_lists(&mut helper);
        assert!(
            lists.iter().any(|l| l
                .recipes
                .iter()
                .any(|r| r.id == "basic.wood_handle" && r.unlocked)),
            "材料发现后应重推一次含已解锁 wood_handle 的 CraftRecipeList，实际 lists={lists:?}"
        );
    }

    #[test]
    fn material_discovery_skips_player_without_relevant_ingredient() {
        let mut app = App::new();
        let mut registry = CraftRegistry::new();
        register_basic_processing_recipes(&mut registry).unwrap();
        app.insert_resource(registry);
        app.insert_resource(RecipeUnlockState::new());
        app.add_systems(Update, apply_material_discovery_unlock);

        let (client_bundle, mut helper) = create_mock_client("Bob");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut()
            .entity_mut(entity)
            .insert(inv_with(&[("unobtainium_xyz", 1)]));
        app.update();
        flush_client_packets(&mut app);

        let player_id = canonical_player_id("Bob");
        let unlock_state = app.world().resource::<RecipeUnlockState>();
        assert_eq!(
            unlock_state.unlocked_count(&player_id),
            0,
            "背包无任何配方原料时不应解锁配方"
        );
        assert!(
            collect_recipe_lists(&mut helper).is_empty(),
            "无新解锁时不应重推配方列表"
        );
    }

    #[test]
    fn material_discovery_does_not_unlock_explicit_source_recipe() {
        // 同一原料 fan_tie：空源配方解锁，scroll 门控的秘传配方不解锁（worldview §九）。
        let mut app = App::new();
        let mut registry = CraftRegistry::new();
        registry
            .register(make_recipe("craft.open.tool", &[("fan_tie", 1)], vec![]))
            .unwrap();
        registry
            .register(make_recipe(
                "craft.secret.tool",
                &[("fan_tie", 1)],
                vec![UnlockSource::Scroll {
                    item_template: "scroll_secret".into(),
                }],
            ))
            .unwrap();
        app.insert_resource(registry);
        app.insert_resource(RecipeUnlockState::new());
        app.add_systems(Update, apply_material_discovery_unlock);

        let (client_bundle, _helper) = create_mock_client("Cleo");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut()
            .entity_mut(entity)
            .insert(inv_with(&[("fan_tie", 5)]));
        app.update();

        let player_id = canonical_player_id("Cleo");
        let unlock_state = app.world().resource::<RecipeUnlockState>();
        assert!(
            unlock_state.is_unlocked(&player_id, &RecipeId::new("craft.open.tool")),
            "空源配方应被材料发现解锁"
        );
        assert!(
            !unlock_state.is_unlocked(&player_id, &RecipeId::new("craft.secret.tool")),
            "scroll 门控的秘传配方不应因持有原料而解锁"
        );
    }

    #[test]
    fn material_discovery_idempotent_across_ticks_and_narrates_once() {
        // A→A 转移：每 tick 跑的系统，首帧解锁并推列表 + 一条 narration；
        // 第二帧已解锁，不应重复推 CraftRecipeList、也不应重复写 narration。
        let mut app = App::new();
        let mut registry = CraftRegistry::new();
        register_basic_processing_recipes(&mut registry).unwrap();
        app.insert_resource(registry);
        app.insert_resource(RecipeUnlockState::new());
        app.insert_resource(PendingGameplayNarrations::default());
        app.add_systems(Update, apply_material_discovery_unlock);

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut()
            .entity_mut(entity)
            .insert(inv_with(&[("crude_wood", 3)]));

        // ── tick 1：首次解锁 ──
        app.update();
        flush_client_packets(&mut app);
        let lists_1 = collect_recipe_lists(&mut helper);
        assert_eq!(lists_1.len(), 1, "首帧应推一次 CraftRecipeList");
        let narr_1 = app
            .world_mut()
            .resource_mut::<PendingGameplayNarrations>()
            .drain();
        assert_eq!(narr_1.len(), 1, "首次解锁应恰好产生一条 narration");
        assert_eq!(
            narr_1[0].target.as_deref(),
            Some("Azure"),
            "narration 应定向到该玩家"
        );
        assert!(
            narr_1[0].text.contains("削木柄"),
            "narration 应点名解锁的配方，实际={:?}",
            narr_1[0].text
        );

        // ── tick 2：A→A，已解锁不重复 ──
        app.update();
        flush_client_packets(&mut app);
        assert!(
            collect_recipe_lists(&mut helper).is_empty(),
            "第二帧不应重复推送 CraftRecipeList"
        );
        let narr_2 = app
            .world_mut()
            .resource_mut::<PendingGameplayNarrations>()
            .drain();
        assert!(narr_2.is_empty(), "第二帧不应重复写 narration");
    }

    #[test]
    fn material_discovery_isolates_unlocks_per_player() {
        // 两个在线玩家持不同原料，各自只解锁与自己原料匹配的空源配方，互不污染。
        let mut app = App::new();
        let mut registry = CraftRegistry::new();
        registry
            .register(make_recipe("craft.open.alpha", &[("fan_tie", 1)], vec![]))
            .unwrap();
        registry
            .register(make_recipe("craft.open.beta", &[("zhu_pi", 1)], vec![]))
            .unwrap();
        app.insert_resource(registry);
        app.insert_resource(RecipeUnlockState::new());
        app.add_systems(Update, apply_material_discovery_unlock);

        let (alice_bundle, _h1) = create_mock_client("Alice");
        let alice = app.world_mut().spawn(alice_bundle).id();
        app.world_mut()
            .entity_mut(alice)
            .insert(inv_with(&[("fan_tie", 2)]));
        let (bob_bundle, _h2) = create_mock_client("Bob");
        let bob = app.world_mut().spawn(bob_bundle).id();
        app.world_mut()
            .entity_mut(bob)
            .insert(inv_with(&[("zhu_pi", 2)]));

        app.update();

        let unlock_state = app.world().resource::<RecipeUnlockState>();
        let alice_id = canonical_player_id("Alice");
        let bob_id = canonical_player_id("Bob");
        assert!(
            unlock_state.is_unlocked(&alice_id, &RecipeId::new("craft.open.alpha")),
            "Alice 持 fan_tie 应解锁 alpha"
        );
        assert!(
            !unlock_state.is_unlocked(&alice_id, &RecipeId::new("craft.open.beta")),
            "Alice 无 zhu_pi 不应解锁 beta"
        );
        assert!(
            unlock_state.is_unlocked(&bob_id, &RecipeId::new("craft.open.beta")),
            "Bob 持 zhu_pi 应解锁 beta"
        );
        assert!(
            !unlock_state.is_unlocked(&bob_id, &RecipeId::new("craft.open.alpha")),
            "Bob 无 fan_tie 不应解锁 alpha"
        );
    }

    #[test]
    fn material_discovery_narrates_multiple_recipes_in_one_frame() {
        // 边界：同帧背包同时持有多种空源配方原料 → 命中 newly.len() > 1 的
        // 多配方 narration 分支「悟得 N 种新制法：…」，合并为一条定向 narration。
        let mut app = App::new();
        let mut registry = CraftRegistry::new();
        registry
            .register(make_recipe("craft.open.alpha", &[("fan_tie", 1)], vec![]))
            .unwrap();
        registry
            .register(make_recipe("craft.open.beta", &[("zhu_pi", 1)], vec![]))
            .unwrap();
        app.insert_resource(registry);
        app.insert_resource(RecipeUnlockState::new());
        app.insert_resource(PendingGameplayNarrations::default());
        app.add_systems(Update, apply_material_discovery_unlock);

        let (client_bundle, _helper) = create_mock_client("Duke");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut()
            .entity_mut(entity)
            .insert(inv_with(&[("fan_tie", 1), ("zhu_pi", 1)]));

        app.update();

        let narr = app
            .world_mut()
            .resource_mut::<PendingGameplayNarrations>()
            .drain();
        assert_eq!(narr.len(), 1, "同帧多配方解锁应合并为恰好一条 narration");
        assert_eq!(
            narr[0].target.as_deref(),
            Some("Duke"),
            "narration 应定向到该玩家"
        );
        let text = &narr[0].text;
        assert!(
            text.starts_with("悟得 2 种新制法："),
            "应走多配方分支并带数量，实际={text:?}"
        );
        // registry 迭代顺序不定，故只断言两个配方名都在（不绑定顺序）。
        assert!(
            text.contains("craft.open.alpha") && text.contains("craft.open.beta"),
            "应列出两个解锁配方名，实际={text:?}"
        );
        assert!(
            text.ends_with("。"),
            "narration 文案应以句号收尾，实际={text:?}"
        );
    }
}
