//! plan-craft-v1 P2 — Craft IPC bridge（server → client + intent → session）。
//!
//! 5 个系统：
//!   1. `apply_craft_start_intents` / `apply_craft_cancel_intents` — 读
//!      `CraftStartIntent` / `CraftCancelIntent`，跑 `start_craft` /
//!      `cancel_craft`，产生 `CraftStartedEvent` / `CraftFailedEvent`，并在
//!      caster 上 insert/remove `CraftSession` component
//!   2. `tick_craft_sessions` — 每 tick 推进所有在线玩家的 session；断线时与
//!      inventory 同事务持久化，重连恢复后继续推进
//!   3. `emit_craft_session_state` — 定期把当前 session 进度推到 client（每 20 tick
//!      一次 / 状态切换时立刻推一次）
//!   4. `emit_craft_outcome_payloads` — 监听 Completed/Failed → push `CraftOutcomeV1`
//!   5. `emit_recipe_list_on_join` / `emit_recipe_list_on_unlock` —
//!      初始全表 + 每次 unlock 增量
//!   6. `apply_material_discovery_unlock` —（plan-craft-material-discovery）
//!      每 tick 扫背包，持有任一原料即被动解锁空源配方 + 重推列表 + narration
//!
//! 守恒律：所有 qi 变更走 `start_craft` 内部封装的
//! `transfer_external_qi_to_ledger(QiTransferReason::Crafting)`。制作消耗统一进入
//! `pending_inflow_account()`，再由 heartbeat 按 zone 平衡规则回流；本模块**禁止**
//! 直接写 zone 或 `cultivation.qi_current`，否则会绕过全局守恒律。

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
    add_item_to_player_inventory, add_item_to_player_inventory_or_ground, DroppedLootEntry,
    DroppedLootRegistry, GrantOrGroundOutcome, InventoryInstanceIdAllocator, ItemRegistry,
    PlayerInventory,
};
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::player::gameplay::PendingGameplayNarrations;
use crate::player::state::{
    canonical_player_id, save_player_craft_checkpoint,
    save_player_inventory_and_craft_session_slices, PlayerState, PlayerStatePersistence,
};
use crate::qi_physics::ledger::WorldQiAccount;
use crate::schema::common::NarrationStyle;
use crate::schema::craft::{
    CraftCategoryV1, CraftFailureReasonV1, CraftOutcomeV1, CraftRecipeEntryV1, CraftRequirementsV1,
    CraftSessionStateV1, RecipeListV1, RecipeUnlockedV1, UnlockEventSourceV1,
};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
use crate::skill::components::SkillSet;
use crate::world::dimension::{CurrentDimension, DimensionKind};

const DEFAULT_REFUND_GROUND_POS: [f64; 3] = [0.0, 64.0, 0.0];

type CraftStarterQuery<'a> = (
    &'a mut PlayerInventory,
    &'a mut Cultivation,
    &'a QiColor,
    Option<&'a SkillSet>,
    Option<&'a CraftSession>,
);

/// 每隔 N tick 对在线 session 推一次进度（20 tick = 1 秒）。
const SESSION_STATE_PUSH_INTERVAL_TICKS: u64 = 20;

/// 标记某玩家本帧需要立刻推一次 SessionState（启动 / 取消 / 完成时打上）。
#[derive(Component, Default, Debug)]
pub struct CraftSessionStateDirty;

#[derive(Component, Default, Debug)]
pub struct CraftSessionPersistenceDirty;

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
    dropped_loot: Option<&mut DroppedLootRegistry>,
    refund_manifest: impl IntoIterator<Item = (String, u32)>,
    current_tick: u64,
    ground_target: RefundGroundTarget,
) -> RefundGrantSummary {
    let mut staged_inventory = inventory.clone();
    let mut staged_allocator = allocator.clone();
    let mut staged_dropped_loot = dropped_loot.as_deref().map(|registry| DroppedLootRegistry {
        entries: registry.entries.clone(),
    });
    let mut summary = RefundGrantSummary::default();
    for (template, count) in refund_manifest {
        if count == 0 {
            continue;
        }
        let outcome = add_item_to_player_inventory_or_ground(
            &mut staged_inventory,
            item_registry,
            &mut staged_allocator,
            staged_dropped_loot.as_mut(),
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
    if summary.errors.is_empty() {
        *inventory = staged_inventory;
        *allocator = staged_allocator;
        if let (Some(target), Some(staged)) = (dropped_loot, staged_dropped_loot) {
            target.entries = staged.entries;
        }
    } else {
        summary.material_returned = 0;
        summary.granted_count = 0;
        summary.dropped_count = 0;
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
    persistence: Option<Res<PlayerStatePersistence>>,
    clock: Res<CombatClock>,
    mut commands: Commands,
    names: Query<&Username>,
    player_contexts: Query<(&Position, Option<&CurrentDimension>)>,
    workbenches: Query<&Position, With<WorkbenchBlock>>,
    mut casters: Query<CraftStarterQuery<'_>>,
) {
    // ── start ───────────────────────────────────────────────
    let mut processed_start_casters = HashSet::new();
    for intent in start_intents.read() {
        if !processed_start_casters.insert(intent.caster) {
            tracing::debug!(
                "[bong][craft] duplicate start intent on caster {:?} in same frame — noop",
                intent.caster
            );
            continue;
        }
        let Ok((mut inventory, mut cultivation, qi_color, skill_set, existing)) =
            casters.get_mut(intent.caster)
        else {
            tracing::warn!(
                "[bong][craft] start intent caster {:?} missing inventory/cultivation",
                intent.caster
            );
            continue;
        };
        let username = names.get(intent.caster).ok();
        let player_id = username
            .map(|u| canonical_player_id(u.0.as_str()))
            .unwrap_or_else(|| format!("entity:{}", intent.caster.to_bits()));
        let mut staged_inventory = inventory.clone();
        let mut staged_cultivation = cultivation.clone();
        let mut staged_ledger = ledger.clone();
        let req = StartCraftRequest {
            caster: intent.caster,
            player_id: &player_id,
            recipe_id: &intent.recipe_id,
            current_tick: clock.tick,
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
            inventory: &mut staged_inventory,
            cultivation: &mut staged_cultivation,
            qi_color,
            ledger: &mut staged_ledger,
            existing_session: existing,
            skill_set,
            has_nearby_workbench,
        };

        match start_craft(req, deps) {
            Ok(success) => {
                if let Some(persistence) = persistence.as_deref() {
                    let Some(username) = username else {
                        tracing::error!(
                            "[bong][craft] refusing to persist start for {:?} without Username",
                            intent.caster
                        );
                        continue;
                    };
                    if let Err(error) = save_player_craft_checkpoint(
                        persistence,
                        username.0.as_str(),
                        Some(&staged_inventory),
                        Some(&success.session),
                        Some(&staged_cultivation),
                        Some(&staged_ledger),
                        &[],
                    ) {
                        tracing::error!(
                            "[bong][craft] start persistence failed player={} recipe={}: {error}",
                            player_id,
                            success.event.recipe_id
                        );
                        failed_tx.send(CraftFailedEvent {
                            caster: intent.caster,
                            recipe_id: intent.recipe_id.clone(),
                            reason: CraftFailureReason::InternalError,
                            material_returned: 0,
                            qi_refunded: 0.0,
                        });
                        commands
                            .entity(intent.caster)
                            .insert(CraftSessionStateDirty);
                        continue;
                    }
                }
                *inventory = staged_inventory;
                *cultivation = staged_cultivation;
                *ledger = staged_ledger;
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
    persistence: Option<Res<PlayerStatePersistence>>,
    clock: Res<CombatClock>,
    mut commands: Commands,
    names: Query<&Username>,
    player_contexts: Query<(&Position, Option<&CurrentDimension>)>,
    mut casters: Query<(&mut PlayerInventory, Option<&CraftSession>)>,
) {
    let mut processed_cancel_casters = HashSet::new();
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
        if !processed_cancel_casters.insert(intent.caster) {
            tracing::debug!(
                "[bong][craft] duplicate cancel intent on caster {:?} in same frame — noop",
                intent.caster
            );
            continue;
        }
        let Some(recipe) = registry.get(&session.recipe_id) else {
            tracing::warn!(
                "[bong][craft] cancel intent recipe `{}` missing — preserving session",
                session.recipe_id
            );
            commands
                .entity(intent.caster)
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
        let mut staged_inventory = inventory.clone();
        let mut staged_allocator = allocator.clone();
        let mut staged_dropped_loot = dropped_loot.as_deref().cloned();
        let ground_target = refund_ground_context(player_contexts.get(intent.caster).ok());
        let refund_summary = grant_refund_manifest_to_inventory_or_ground(
            &mut staged_inventory,
            &item_registry,
            &mut staged_allocator,
            staged_dropped_loot.as_mut(),
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
            commands
                .entity(intent.caster)
                .insert(CraftSessionStateDirty);
            continue;
        }
        let durable_drops: Vec<DroppedLootEntry> = staged_dropped_loot
            .as_ref()
            .into_iter()
            .flat_map(|staged| staged.entries.values())
            .filter(|entry| {
                dropped_loot
                    .as_deref()
                    .is_none_or(|current| !current.entries.contains_key(&entry.instance_id))
            })
            .cloned()
            .collect();
        if let Some(persistence) = persistence.as_deref() {
            let Ok(username) = names.get(intent.caster) else {
                tracing::error!(
                    "[bong][craft] refusing to persist cancel for {:?} without Username",
                    intent.caster
                );
                commands
                    .entity(intent.caster)
                    .insert(CraftSessionStateDirty);
                continue;
            };
            if let Err(error) = save_player_craft_checkpoint(
                persistence,
                username.0.as_str(),
                Some(&staged_inventory),
                None,
                None,
                None,
                &durable_drops,
            ) {
                tracing::error!(
                    "[bong][craft] cancel persistence failed player={} recipe={}: {error}",
                    username.0,
                    event.recipe_id
                );
                commands
                    .entity(intent.caster)
                    .insert(CraftSessionStateDirty);
                continue;
            }
        }
        *inventory = staged_inventory;
        *allocator = staged_allocator;
        if let (Some(current), Some(staged)) = (dropped_loot.as_deref_mut(), staged_dropped_loot) {
            current.entries = staged.entries;
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
    persistence: Option<Res<PlayerStatePersistence>>,
    clock: Res<CombatClock>,
    mut commands: Commands,
    mut completed_tx: EventWriter<CraftCompletedEvent>,
    mut failed_tx: EventWriter<CraftFailedEvent>,
    mut dropped_loot: Option<ResMut<DroppedLootRegistry>>,
    names: Query<&Username>,
    player_contexts: Query<(&Position, Option<&CurrentDimension>)>,
    mut sessions: Query<(Entity, &mut CraftSession, &mut PlayerInventory), With<Client>>,
) {
    for (entity, mut session, mut inventory) in sessions.iter_mut() {
        let mut staged_session = session.clone();
        if tick_session(&mut staged_session, 1) {
            // session 完成
            let Some(recipe) = registry.get(&staged_session.recipe_id) else {
                tracing::error!(
                    "[bong][craft] tick finalize: recipe `{}` missing; preserving completed session for recovery",
                    staged_session.recipe_id
                );
                *session = staged_session;
                commands
                    .entity(entity)
                    .insert((CraftSessionStateDirty, CraftSessionPersistenceDirty));
                continue;
            };
            let FinalizeCraftOutcome {
                event,
                output_manifest,
            } = finalize_craft(&staged_session, recipe, entity, clock.tick);
            let (template, count) = output_manifest;
            let mut staged_inventory = inventory.clone();
            let mut staged_allocator = allocator.clone();
            let mut staged_dropped_loot = dropped_loot.as_deref().cloned();
            // review fix (Codex P1)：产物入背包失败时不能静默——qi 已扣材料已耗，
            // 玩家必须知道任务失败而不是显示一条假"出炉成功"。改 emit Failed
            // (InternalError)，让 client 渲染失败 toast；不送 Completed 事件。
            match add_item_to_player_inventory(
                &mut staged_inventory,
                &item_registry,
                &mut staged_allocator,
                &template,
                count,
                clock.tick,
            ) {
                Ok(_) => {
                    let next_completed = staged_session.completed_count.saturating_add(1);
                    let has_more = next_completed < staged_session.quantity_total;
                    if has_more {
                        staged_session.completed_count = next_completed;
                        staged_session.remaining_ticks = staged_session.total_ticks;
                    }
                    if let Some(persistence) = persistence.as_deref() {
                        let Some(username) = names.get(entity).ok() else {
                            tracing::error!(
                                "[bong][craft] refusing to persist finalize for {entity:?} without Username"
                            );
                            continue;
                        };
                        if let Err(error) = save_player_craft_checkpoint(
                            persistence,
                            username.0.as_str(),
                            Some(&staged_inventory),
                            has_more.then_some(&staged_session),
                            None,
                            None,
                            &[],
                        ) {
                            tracing::error!(
                                "[bong][craft] finalize persistence failed player={} recipe={}: {error}",
                                username.0,
                                event.recipe_id
                            );
                            continue;
                        }
                    }
                    *inventory = staged_inventory;
                    *allocator = staged_allocator;
                    tracing::info!(
                        "[bong][craft] finalize caster={entity:?} recipe={} output={template} x{count} completed={}/{}",
                        event.recipe_id,
                        next_completed,
                        staged_session.quantity_total
                    );
                    completed_tx.send(event);
                    if has_more {
                        *session = staged_session;
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
                    } = cancel_craft(
                        &staged_session,
                        recipe,
                        entity,
                        CraftFailureReason::InternalError,
                    );
                    let ground_target = refund_ground_context(player_contexts.get(entity).ok());
                    let refund_summary = grant_refund_manifest_to_inventory_or_ground(
                        &mut staged_inventory,
                        &item_registry,
                        &mut staged_allocator,
                        staged_dropped_loot.as_mut(),
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
                        commands.entity(entity).insert(CraftSessionStateDirty);
                        continue;
                    }
                    let durable_drops: Vec<DroppedLootEntry> = staged_dropped_loot
                        .as_ref()
                        .into_iter()
                        .flat_map(|staged| staged.entries.values())
                        .filter(|entry| {
                            dropped_loot.as_deref().is_none_or(|current| {
                                !current.entries.contains_key(&entry.instance_id)
                            })
                        })
                        .cloned()
                        .collect();
                    if let Some(persistence) = persistence.as_deref() {
                        let Some(username) = names.get(entity).ok() else {
                            tracing::error!(
                                "[bong][craft] refusing to persist failed finalize for {entity:?} without Username"
                            );
                            continue;
                        };
                        if let Err(error) = save_player_craft_checkpoint(
                            persistence,
                            username.0.as_str(),
                            Some(&staged_inventory),
                            None,
                            None,
                            None,
                            &durable_drops,
                        ) {
                            tracing::error!(
                                "[bong][craft] failed-finalize persistence failed player={} recipe={}: {error}",
                                username.0,
                                event.recipe_id
                            );
                            continue;
                        }
                    }
                    *inventory = staged_inventory;
                    *allocator = staged_allocator;
                    if let (Some(current), Some(staged)) =
                        (dropped_loot.as_deref_mut(), staged_dropped_loot)
                    {
                        current.entries = staged.entries;
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
            *session = staged_session;
            // 每秒标脏一次让 emit 系统下一帧推 progress
            commands
                .entity(entity)
                .insert((CraftSessionStateDirty, CraftSessionPersistenceDirty));
        } else {
            *session = staged_session;
        }
    }
}

/// inventory 与 session 在同一 SQLite transaction 中保存；只有成功后才清持久化
/// dirty 标记。断线和进程退出另由 player flush 以相同事务再次兜底。
pub fn persist_dirty_craft_sessions(
    mut commands: Commands,
    persistence: Res<PlayerStatePersistence>,
    players: Query<
        (Entity, &Username, &PlayerInventory, Option<&CraftSession>),
        With<CraftSessionPersistenceDirty>,
    >,
) {
    for (entity, username, inventory, session) in players.iter() {
        match save_player_inventory_and_craft_session_slices(
            &persistence,
            username.0.as_str(),
            Some(inventory),
            session,
        ) {
            Ok(_) => {
                commands
                    .entity(entity)
                    .remove::<CraftSessionPersistenceDirty>();
            }
            Err(error) => tracing::error!(
                "[bong][craft] failed to persist inventory/session atomically for `{}`: {error}",
                username.0
            ),
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
#[allow(clippy::type_complexity)]
pub fn emit_recipe_list_on_join(
    registry: Res<CraftRegistry>,
    unlock_state: Res<RecipeUnlockState>,
    mut sent: Local<HashMap<Entity, String>>,
    mut clients: Query<
        (Entity, &Username, &mut Client, Option<&CraftSession>),
        (With<Client>, With<PlayerState>),
    >,
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
) {
    for intent in intents.read() {
        let player_id = intent.player_id.as_str();
        let Some(recipe) = registry.get(&intent.recipe_id) else {
            if let UnlockEventSource::Scroll { .. } = &intent.source {
                unlock_state.release_scroll_unlock_reservation(player_id, &intent.recipe_id);
            }
            tracing::warn!(
                "[bong][craft] unlock intent ignored: recipe `{}` not in registry",
                intent.recipe_id
            );
            continue;
        };
        let outcome = match &intent.source {
            UnlockEventSource::Scroll { item_template } => {
                unlock_via_scroll(&mut unlock_state, player_id, recipe, item_template)
            }
            UnlockEventSource::Mentor { npc_archetype } => {
                unlock_via_mentor(&mut unlock_state, player_id, recipe, npc_archetype)
            }
            UnlockEventSource::Insight { trigger } => {
                unlock_via_insight(&mut unlock_state, player_id, recipe, *trigger)
            }
        };
        if let UnlockEventSource::Scroll { .. } = &intent.source {
            unlock_state.release_scroll_unlock_reservation(player_id, &intent.recipe_id);
        }
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
#[path = "craft_emit_tests.rs"]
mod tests;
