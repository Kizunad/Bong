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
mod tests {
    use super::*;
    use crate::craft::{
        register_basic_processing_recipes, register_examples, CraftCategory, CraftRecipe,
        CraftRequirements, CraftSession, RecipeId, RecipeUnlockState, UnlockSource,
    };
    use crate::cultivation::tick::CultivationClock;
    use crate::inventory::{
        ContainerState, DroppedLootRegistry, InventoryInstanceIdAllocator, InventoryRevision,
        ItemCategory, ItemInstance, ItemRarity, ItemRegistry, ItemTemplate, PlacedItemState,
        JS_SAFE_INTEGER_MAX,
    };
    use crate::persistence::bootstrap_sqlite;
    use crate::player::state::{load_player_slices, save_player_state, PlayerState};
    use crate::qi_physics::constants::QI_ZONE_UNIT_CAPACITY;
    use crate::qi_physics::ledger::{pending_inflow_account, QiAccountId, QiTransferReason};
    use crate::world::events::ActiveEventsResource;
    use crate::world::heartbeat;
    use crate::world::zone::{Zone, ZoneRegistry};
    use crate::worldgen::pseudo_vein::TICKS_PER_MINUTE;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use valence::prelude::{App, DVec3, Events, Update};
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
                    wearer_race: crate::body_plan::types::RaceGateOwned::default(),
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

    fn craft_test_persistence(test_name: &str) -> (PlayerStatePersistence, PathBuf) {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let data_dir = std::env::temp_dir().join(format!(
            "bong-craft-emit-{test_name}-{}-{suffix}",
            std::process::id()
        ));
        let db_path = data_dir.join("bong.db");
        bootstrap_sqlite(&db_path, &format!("craft-emit-{test_name}"))
            .expect("sqlite bootstrap should succeed");
        let persistence = PlayerStatePersistence::with_db_path(&data_dir, &db_path);
        save_player_state(&persistence, "Azure", &PlayerState::default())
            .expect("test player should initialize");
        (persistence, data_dir)
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
    fn refund_ground_context_falls_back_without_position_or_dimension() {
        let absent = refund_ground_context(None);
        assert_eq!(
            absent.pos, DEFAULT_REFUND_GROUND_POS,
            "缺少 Position 时应使用固定退款落地点，不能依赖无效玩家坐标"
        );
        assert_eq!(
            absent.dimension,
            DimensionKind::default(),
            "缺少 Position 时 dimension 也应回退到默认维度"
        );

        let pos = Position::new([12.0, 66.0, -9.0]);
        let missing_dimension = refund_ground_context(Some((&pos, None)));
        assert_eq!(
            missing_dimension.pos,
            [12.0, 66.0, -9.0],
            "有 Position 时应保留玩家坐标作为退款落地点"
        );
        assert_eq!(
            missing_dimension.dimension,
            DimensionKind::default(),
            "缺少 CurrentDimension 时应使用默认维度"
        );

        let dimension = CurrentDimension(DimensionKind::Tsy);
        let populated = refund_ground_context(Some((&pos, Some(&dimension))));
        assert_eq!(
            populated.pos,
            [12.0, 66.0, -9.0],
            "Position 与 CurrentDimension 齐全时应原样使用玩家坐标作为退款落地点"
        );
        assert_eq!(
            populated.dimension,
            DimensionKind::Tsy,
            "Position 与 CurrentDimension 齐全时应原样使用玩家所在维度"
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
    fn refund_manifest_structural_error_rolls_back_earlier_grants_atomically() {
        let registry = registry_with_templates(&[("fan_tie", 64)]);
        let mut inventory = inv_with(&[("fan_tie", 63)]);
        clamp_main_pack_to_grid(&mut inventory, 1, 1);
        let original_revision = inventory.revision;
        let mut allocator = InventoryInstanceIdAllocator::new(60);
        let mut dropped_loot = DroppedLootRegistry::default();

        let summary = grant_refund_manifest_to_inventory_or_ground(
            &mut inventory,
            &registry,
            &mut allocator,
            Some(&mut dropped_loot),
            vec![
                ("fan_tie".to_string(), 1),
                ("missing_template".to_string(), 1),
            ],
            1,
            RefundGroundTarget {
                pos: [0.0, 64.0, 0.0],
                dimension: DimensionKind::Overworld,
            },
        );

        assert_eq!(
            summary.material_returned, 0,
            "manifest 后项结构错误时前项也不得提交，否则重试会复制已成功项"
        );
        assert_eq!(summary.errors.len(), 1, "应保留唯一结构错误供调用方诊断");
        assert_eq!(
            inventory.containers[0].items[0].instance.stack_count, 63,
            "事务失败必须回滚先前已合并到背包的退款"
        );
        assert_eq!(
            inventory.revision, original_revision,
            "事务失败不应留下虚假的 inventory revision 变化"
        );
        assert!(
            dropped_loot.entries.is_empty(),
            "事务失败不得留下部分地面掉落"
        );
        assert_eq!(
            allocator.next_id().unwrap(),
            60,
            "事务失败必须回滚 instance id allocator，避免无效退款消耗 id"
        );
    }

    #[test]
    fn refund_manifest_allocator_boundary_rolls_back_without_drop_id_collision() {
        let registry = registry_with_templates(&[("fan_tie", 64), ("zhu_pi", 64)]);
        let mut inventory = inv_with(&[("occupant", 1)]);
        clamp_main_pack_to_grid(&mut inventory, 1, 1);
        let mut allocator = InventoryInstanceIdAllocator::new(JS_SAFE_INTEGER_MAX);
        let mut dropped_loot = DroppedLootRegistry::default();

        let summary = grant_refund_manifest_to_inventory_or_ground(
            &mut inventory,
            &registry,
            &mut allocator,
            Some(&mut dropped_loot),
            vec![("fan_tie".to_string(), 1), ("zhu_pi".to_string(), 1)],
            1,
            RefundGroundTarget {
                pos: [0.0, 64.0, 0.0],
                dimension: DimensionKind::Overworld,
            },
        );

        assert_eq!(
            summary.material_returned, 0,
            "第二个掉落无法分配安全 ID 时整批退款必须回滚"
        );
        assert_eq!(summary.errors.len(), 1, "allocator 越界应作为结构错误暴露");
        assert!(
            dropped_loot.entries.is_empty(),
            "allocator 边界失败不得让相同 ID 的后项覆盖前项并虚报两份退款"
        );
        assert_eq!(
            allocator.next_id().unwrap(),
            JS_SAFE_INTEGER_MAX,
            "失败事务应回滚 allocator，保留原始可分配边界 ID"
        );
    }

    #[test]
    fn refund_manifest_rejects_existing_drop_id_collision_without_overwrite() {
        let registry = registry_with_templates(&[("fan_tie", 64), ("zhu_pi", 64)]);
        let mut inventory = inv_with(&[("occupant", 1)]);
        clamp_main_pack_to_grid(&mut inventory, 1, 1);
        let mut dropped_loot = DroppedLootRegistry::default();
        let target = RefundGroundTarget {
            pos: [0.0, 64.0, 0.0],
            dimension: DimensionKind::Overworld,
        };
        let mut allocator = InventoryInstanceIdAllocator::new(60);
        let seeded = grant_refund_manifest_to_inventory_or_ground(
            &mut inventory,
            &registry,
            &mut allocator,
            Some(&mut dropped_loot),
            vec![("fan_tie".to_string(), 1)],
            1,
            target,
        );
        assert_eq!(seeded.dropped_count, 1, "夹具应先落地 instance_id=60");

        let mut colliding_allocator = InventoryInstanceIdAllocator::new(60);
        let summary = grant_refund_manifest_to_inventory_or_ground(
            &mut inventory,
            &registry,
            &mut colliding_allocator,
            Some(&mut dropped_loot),
            vec![("zhu_pi".to_string(), 1)],
            2,
            target,
        );

        assert_eq!(
            summary.material_returned, 0,
            "已有掉落 ID 冲突时不能虚报新退款已返还"
        );
        assert!(
            summary
                .errors
                .iter()
                .any(|error| error.contains("instance id collision")),
            "碰撞错误应保留可诊断原因，实际={:?}",
            summary.errors
        );
        assert_eq!(
            dropped_loot.entries.len(),
            1,
            "碰撞不得新增或覆盖 registry 条目"
        );
        assert_eq!(
            dropped_loot.entries.get(&60).unwrap().item.template_id,
            "fan_tie",
            "新退款不得覆盖已有同 ID 掉落并吞掉旧物品"
        );
    }

    #[test]
    fn cancel_intent_missing_caster_is_noop_without_failed_event() {
        let recipe = make_recipe(
            "craft.test.cancel_missing_caster",
            &[("fan_tie", 2)],
            vec![],
        );
        let mut app = craft_refund_test_app(recipe, &[("fan_tie", 64)], 11);
        app.add_systems(Update, apply_craft_cancel_intents);

        let caster_without_inventory = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(CraftCancelIntent {
            caster: caster_without_inventory,
        });
        app.update();

        assert!(
            current_failed_events(&app).is_empty(),
            "caster 查找失败应只跳过本 intent，不能伪造 Failed 事件"
        );
        assert!(
            app.world()
                .get::<CraftSession>(caster_without_inventory)
                .is_none(),
            "缺少 inventory/session 的 caster 不应被 cancel 路径补写 CraftSession"
        );
    }

    #[test]
    fn production_skill_gate_reads_caster_skill_set() {
        let mut recipe = make_recipe("craft.skill.integration", &[("fan_tie", 1)], vec![]);
        recipe.requirements.skill_lv_min = Some(2);
        let mut app = craft_refund_test_app(recipe, &[("fan_tie", 64)], 10);
        app.add_systems(Update, apply_craft_start_intents);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let player = app
            .world_mut()
            .spawn(client_bundle)
            .insert(inv_with(&[("fan_tie", 1)]))
            .insert(Cultivation::default())
            .insert(QiColor::default())
            .insert(SkillSet::default())
            .insert(Position::new([0.0, 64.0, 0.0]))
            .id();
        app.world_mut()
            .resource_mut::<RecipeUnlockState>()
            .unlock("offline:Azure", RecipeId::new("craft.skill.integration"));
        app.world_mut()
            .get_mut::<SkillSet>(player)
            .unwrap()
            .skills
            .insert(
                crate::skill::components::SkillId::Alchemy,
                crate::skill::components::SkillEntry {
                    lv: 1,
                    ..Default::default()
                },
            );
        app.world_mut().send_event(CraftStartIntent {
            caster: player,
            recipe_id: RecipeId::new("craft.skill.integration"),
            quantity: 1,
        });

        app.update();

        assert!(
            app.world().get::<CraftSession>(player).is_none(),
            "production bridge must reject a caster below loaded skill requirement"
        );
        assert_eq!(
            count_template_in_inventory(
                app.world().get::<PlayerInventory>(player).unwrap(),
                "fan_tie"
            ),
            1,
            "production skill rejection must not consume materials"
        );
        assert!(
            !current_failed_events(&app).is_empty(),
            "production skill rejection must emit the observable craft failure"
        );
    }

    #[test]
    fn duplicate_start_intents_same_frame_consume_materials_only_once() {
        let recipe = make_recipe("craft.tool.workbench", &[("fan_tie", 2)], vec![]);
        let mut app = craft_refund_test_app(recipe, &[("fan_tie", 64)], 10);
        app.add_systems(Update, apply_craft_start_intents);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let player = app
            .world_mut()
            .spawn(client_bundle)
            .insert(inv_with(&[("fan_tie", 4)]))
            .insert(Cultivation::default())
            .insert(QiColor::default())
            .insert(Position::new([0.0, 64.0, 0.0]))
            .id();
        for _ in 0..2 {
            app.world_mut().send_event(CraftStartIntent {
                caster: player,
                recipe_id: RecipeId::new("craft.tool.workbench"),
                quantity: 1,
            });
        }

        app.update();

        let started: Vec<_> = app
            .world()
            .resource::<Events<CraftStartedEvent>>()
            .iter_current_update_events()
            .collect();
        assert_eq!(
            started.len(),
            1,
            "同帧重复 start 只能创建一个 session/Started 事件，实际={started:?}"
        );
        let inventory = app.world().get::<PlayerInventory>(player).unwrap();
        assert_eq!(
            count_template_in_inventory(inventory, "fan_tie"),
            2,
            "同帧重复 start 只能预扣一次 fan_tie x2，不能在 deferred insert 前重复扣料"
        );
        assert!(
            app.world().get::<CraftSession>(player).is_some(),
            "首个 start 成功后应保留唯一 CraftSession"
        );
    }

    #[test]
    fn apply_craft_start_intents_without_spatial_context_credits_pending_never_spawn() {
        let mut recipe = make_recipe("craft.tool.workbench", &[("fan_tie", 1)], vec![]);
        recipe.qi_cost = 5.0;
        let mut app = craft_refund_test_app(recipe, &[("fan_tie", 64)], 10);
        app.add_systems(Update, apply_craft_start_intents);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let mut cultivation = Cultivation::default();
        cultivation.qi_current = 10.0;
        cultivation.qi_max = cultivation.qi_max.max(10.0);
        let mut player_entity = app.world_mut().spawn(client_bundle);
        player_entity
            .insert(inv_with(&[("fan_tie", 1)]))
            .insert(cultivation)
            .insert(QiColor::default())
            .remove::<Position>();
        let player = player_entity.id();
        let player_account = QiAccountId::player(canonical_player_id("Azure"));
        let observed_before = app.world().get::<Cultivation>(player).unwrap().qi_current
            + app.world().resource::<WorldQiAccount>().total();

        app.world_mut().send_event(CraftStartIntent {
            caster: player,
            recipe_id: RecipeId::new("craft.tool.workbench"),
            quantity: 1,
        });
        app.update();

        let ledger = app.world().resource::<WorldQiAccount>();
        assert!(
            !ledger.has_account(&player_account),
            "在线玩家真元权威在 ECS，制作后不得留下长期 player ledger 镜像"
        );
        assert_eq!(
            ledger.balance(&pending_inflow_account()),
            5.0,
            "缺少空间组件时制作消耗仍应完整进入待分配池"
        );
        assert_eq!(
            ledger.balance(&QiAccountId::zone("spawn")),
            0.0,
            "制作消耗不得再落入陈旧的硬编码 spawn 账户"
        );
        let cultivation_after = app.world().get::<Cultivation>(player).unwrap();
        assert!(
            (cultivation_after.qi_current + ledger.total() - observed_before).abs() < 1e-9,
            "ECS player qi + ledger 在制作前后必须守恒"
        );
        let transfer = ledger
            .transfers()
            .last()
            .expect("正 qi_cost 制作必须留下审计 transfer");
        assert_eq!(
            transfer.from, player_account,
            "制作审计转账的来源必须是当前玩家账户"
        );
        assert_eq!(
            transfer.to,
            pending_inflow_account(),
            "制作审计转账的目标必须是待分配池"
        );
        assert_eq!(
            transfer.reason,
            QiTransferReason::Crafting,
            "制作审计转账必须标记为 Crafting"
        );
        assert_eq!(transfer.amount, 5.0, "制作审计金额必须等于配方 qi_cost");
        assert_eq!(
            cultivation_after.qi_current, 5.0,
            "制作后应从 ECS 玩家真元扣除 5 点"
        );
        assert!(
            app.world().get::<CraftSession>(player).is_some(),
            "成功预付真元后必须创建制作会话"
        );
        assert!(
            current_failed_events(&app).is_empty(),
            "成功制作起手不得发出 CraftFailedEvent"
        );
    }

    #[test]
    fn crafting_pending_then_heartbeat_zone_inflow_preserves_total_and_skips_full_zone() {
        let mut recipe = make_recipe("craft.tool.workbench", &[("fan_tie", 1)], vec![]);
        recipe.qi_cost = 5.0;
        let mut app = craft_refund_test_app(recipe, &[("fan_tie", 64)], 10);
        app.insert_resource(CultivationClock { tick: 0 });
        app.insert_resource(ActiveEventsResource::default());
        app.add_event::<crate::cultivation::breakthrough::BreakthroughOutcome>();
        app.add_event::<crate::world::events::ZoneCollapsedEvent>();
        heartbeat::register(&mut app);
        crate::network::register_craft_start_runtime_system(&mut app);
        let initial_sink_qi = 0.10;
        let expected_sink_qi = initial_sink_qi + 1.0 / QI_ZONE_UNIT_CAPACITY;
        app.insert_resource(ZoneRegistry {
            spatial_revision: 0,
            zones: vec![
                Zone {
                    name: "full_zone".to_string(),
                    dimension: DimensionKind::Overworld,
                    bounds: (DVec3::new(-50.0, 60.0, -50.0), DVec3::new(50.0, 90.0, 50.0)),
                    spirit_qi: 0.25,
                    danger_level: 0,
                    active_events: Vec::new(),
                    patrol_anchors: Vec::new(),
                    blocked_tiles: Vec::new(),
                    qi_equilibrium: 0.25,
                    qi_inflow_per_min: 100.0,
                },
                Zone {
                    name: "craft_sink".to_string(),
                    dimension: DimensionKind::Overworld,
                    bounds: (
                        DVec3::new(100.0, 60.0, -50.0),
                        DVec3::new(200.0, 90.0, 50.0),
                    ),
                    spirit_qi: initial_sink_qi,
                    danger_level: 0,
                    active_events: Vec::new(),
                    patrol_anchors: Vec::new(),
                    blocked_tiles: Vec::new(),
                    qi_equilibrium: 0.30,
                    qi_inflow_per_min: 1.0,
                },
            ],
        });
        let (client_bundle, _helper) = create_mock_client("Azure");
        let mut cultivation = Cultivation::default();
        cultivation.qi_current = 10.0;
        cultivation.qi_max = cultivation.qi_max.max(10.0);
        let mut player_entity = app.world_mut().spawn(client_bundle);
        player_entity
            .insert(inv_with(&[("fan_tie", 1)]))
            .insert(cultivation)
            .insert(QiColor::default())
            .remove::<Position>();
        let player = player_entity.id();
        let player_account = QiAccountId::player(canonical_player_id("Azure"));
        let full_account = QiAccountId::zone("full_zone");
        let sink_account = QiAccountId::zone("craft_sink");
        let observed_before = app.world().get::<Cultivation>(player).unwrap().qi_current
            + app.world().resource::<WorldQiAccount>().total()
            + app
                .world()
                .resource::<ZoneRegistry>()
                .zones
                .iter()
                .map(|zone| zone.spirit_qi * QI_ZONE_UNIT_CAPACITY)
                .sum::<f64>();

        app.world_mut().send_event(CraftStartIntent {
            caster: player,
            recipe_id: RecipeId::new("craft.tool.workbench"),
            quantity: 1,
        });
        app.update();

        {
            let ledger = app.world().resource::<WorldQiAccount>();
            assert!(
                !ledger.has_account(&player_account),
                "制作阶段不得留下在线玩家的长期 ledger 镜像"
            );
            assert_eq!(
                ledger.balance(&pending_inflow_account()),
                5.0,
                "heartbeat 前制作消耗应全部停留在待分配池"
            );
            assert!(
                !ledger.has_account(&full_account),
                "制作阶段不得为已满 zone 创建长期 ledger mirror"
            );
            assert!(
                !ledger.has_account(&sink_account),
                "制作阶段不得为目标 zone 创建长期 ledger mirror"
            );
            let cultivation_after = app.world().get::<Cultivation>(player).unwrap();
            let zone_qi = app
                .world()
                .resource::<ZoneRegistry>()
                .zones
                .iter()
                .map(|zone| zone.spirit_qi * QI_ZONE_UNIT_CAPACITY)
                .sum::<f64>();
            assert!(
                (cultivation_after.qi_current + ledger.total() + zone_qi - observed_before).abs()
                    < 1e-9,
                "ECS player qi + Zone field + pending 制作阶段必须保持观察总量守恒"
            );
            assert_eq!(
                ledger.transfers().len(),
                1,
                "heartbeat 前账本应只有一笔 Crafting 转账"
            );
            let crafting = &ledger.transfers()[0];
            assert_eq!(
                crafting.from, player_account,
                "第一笔转账必须从制作玩家账户发出"
            );
            assert_eq!(
                crafting.to,
                pending_inflow_account(),
                "第一笔转账必须写入待分配池"
            );
            assert_eq!(
                crafting.reason,
                QiTransferReason::Crafting,
                "第一笔转账必须标记为 Crafting"
            );
            assert_eq!(
                crafting.amount, 5.0,
                "Crafting 转账金额必须等于配方 qi_cost"
            );
        }
        assert!(
            app.world().get::<CraftSession>(player).is_some(),
            "制作阶段成功后必须保留 CraftSession"
        );
        assert!(
            current_failed_events(&app).is_empty(),
            "完整回流链的制作阶段不得发出 CraftFailedEvent"
        );

        app.world_mut().resource_mut::<CultivationClock>().tick = TICKS_PER_MINUTE;
        app.update();

        let zones = app.world().resource::<ZoneRegistry>();
        let full_zone = zones
            .find_zone_by_name("full_zone")
            .expect("full-zone fixture should remain registered");
        let sink_zone = zones
            .find_zone_by_name("craft_sink")
            .expect("sink-zone fixture should remain registered");
        assert_eq!(
            full_zone.spirit_qi, 0.25,
            "已达 equilibrium 的 zone 即使速率很高也不得消费 pending"
        );
        assert!(
            (sink_zone.spirit_qi - expected_sink_qi).abs() < 1e-9,
            "1 分钟 heartbeat 应把 1.0 绝对真元从 pending 滴灌到 craft_sink"
        );

        let ledger = app.world().resource::<WorldQiAccount>();
        assert_eq!(
            ledger.balance(&pending_inflow_account()),
            4.0,
            "一分钟 heartbeat 应从待分配池消费恰好 1 点真元"
        );
        assert!(
            !ledger.has_account(&full_account),
            "heartbeat 不得为已满 zone 创建长期 ledger mirror"
        );
        assert!(
            !ledger.has_account(&sink_account),
            "heartbeat 不得为目标 zone 创建长期 ledger mirror"
        );
        let zone_qi = app
            .world()
            .resource::<ZoneRegistry>()
            .zones
            .iter()
            .map(|zone| zone.spirit_qi * QI_ZONE_UNIT_CAPACITY)
            .sum::<f64>();
        assert!(
            (app.world().get::<Cultivation>(player).unwrap().qi_current + ledger.total() + zone_qi
                - observed_before)
                .abs()
                < 1e-9,
            "Crafting → pending → ZoneInflow 全链路必须保持 ECS、Zone field 与账本总量守恒"
        );
        assert_eq!(
            ledger.transfers().len(),
            2,
            "heartbeat 后账本应恰有 Crafting 与 ZoneInflow 两笔转账"
        );
        let inflow = &ledger.transfers()[1];
        assert_eq!(
            inflow.from,
            pending_inflow_account(),
            "ZoneInflow 必须从待分配池发出"
        );
        assert_eq!(
            inflow.to, sink_account,
            "ZoneInflow 必须写入未达 equilibrium 的目标 zone"
        );
        assert_eq!(
            inflow.reason,
            QiTransferReason::ZoneInflow,
            "heartbeat 回流转账必须标记为 ZoneInflow"
        );
        assert_eq!(
            inflow.amount, 1.0,
            "一分钟 heartbeat 应按 qi_inflow_per_min 转入 1 点真元"
        );
        assert!(
            ledger.transfers().iter().all(|transfer| {
                transfer.reason != QiTransferReason::ZoneInflow || transfer.to != full_account
            }),
            "容量门禁不得给已达 equilibrium 的 full_zone 生成 ZoneInflow"
        );
    }

    #[test]
    fn start_persistence_failure_keeps_inventory_qi_ledger_and_session_at_pre_state() {
        let mut recipe = make_recipe("craft.tool.workbench", &[("fan_tie", 2)], vec![]);
        recipe.qi_cost = 5.0;
        let mut app = craft_refund_test_app(recipe, &[("fan_tie", 64)], 10);
        app.add_systems(Update, apply_craft_start_intents);
        let (persistence, data_dir) = craft_test_persistence("start-rollback");
        let connection = rusqlite::Connection::open(persistence.db_path())
            .expect("sqlite should open for failure injection");
        connection
            .execute_batch(
                "
                CREATE TRIGGER fail_craft_session_insert
                BEFORE INSERT ON player_craft_sessions
                BEGIN
                    SELECT RAISE(FAIL, 'forced craft session failure');
                END;
                ",
            )
            .expect("failure trigger should install");
        app.insert_resource(persistence.clone());

        let (client_bundle, _helper) = create_mock_client("Azure");
        let mut cultivation = Cultivation::default();
        cultivation.qi_current = 10.0;
        cultivation.qi_max = cultivation.qi_max.max(10.0);
        let player = app
            .world_mut()
            .spawn(client_bundle)
            .insert(inv_with(&[("fan_tie", 2)]))
            .insert(cultivation)
            .insert(QiColor::default())
            .insert(Position::new([0.0, 64.0, 0.0]))
            .id();
        let player_account = QiAccountId::player(canonical_player_id("Azure"));
        app.world_mut().send_event(CraftStartIntent {
            caster: player,
            recipe_id: RecipeId::new("craft.tool.workbench"),
            quantity: 1,
        });

        app.update();

        let inventory = app.world().get::<PlayerInventory>(player).unwrap();
        assert_eq!(
            count_template_in_inventory(inventory, "fan_tie"),
            2,
            "failed durable start must not publish the staged material debit"
        );
        assert_eq!(
            app.world().get::<Cultivation>(player).unwrap().qi_current,
            10.0,
            "failed durable start must not publish the staged qi debit"
        );
        assert!(
            app.world().get::<CraftSession>(player).is_none(),
            "failed durable start must not create an in-memory session"
        );
        let ledger = app.world().resource::<WorldQiAccount>();
        assert!(
            !ledger.has_account(&player_account),
            "持久化拒绝后不得发布临时 player ledger 影子账户"
        );
        assert_eq!(
            ledger.balance(&pending_inflow_account()),
            0.0,
            "持久化拒绝后不得发布 staged pending 入账"
        );
        assert!(
            ledger.transfers().is_empty(),
            "failed durable start must not publish a transfer audit entry"
        );
        assert_eq!(
            current_failed_events(&app).len(),
            1,
            "persistence rejection should produce one client-visible failure"
        );
        let reloaded = load_player_slices(&persistence, "Azure");
        assert!(reloaded.inventory.is_none());
        assert!(reloaded.craft_session.is_none());
        std::fs::remove_dir_all(data_dir).ok();
    }

    #[test]
    fn cancel_intent_without_session_is_noop_without_failed_event() {
        let recipe = make_recipe(
            "craft.test.cancel_without_session",
            &[("fan_tie", 2)],
            vec![],
        );
        let mut app = craft_refund_test_app(recipe, &[("fan_tie", 64)], 12);
        app.add_systems(Update, apply_craft_cancel_intents);

        let player = app.world_mut().spawn(inv_with(&[("fan_tie", 1)])).id();
        app.world_mut()
            .send_event(CraftCancelIntent { caster: player });
        app.update();

        assert!(
            current_failed_events(&app).is_empty(),
            "无 CraftSession 的取消 intent 应 debug/noop，不应通知 client 失败"
        );
        let inventory = app.world().get::<PlayerInventory>(player).unwrap();
        assert_eq!(
            inventory.containers[0].items.len(),
            1,
            "无 session 的 cancel 不应改动玩家背包"
        );
        assert!(
            app.world().get::<CraftSession>(player).is_none(),
            "无 session 的 cancel 不应插入或保留 CraftSession"
        );
    }

    #[test]
    fn cancel_intent_unknown_recipe_preserves_session_without_terminal_event() {
        let recipe = make_recipe("craft.test.cancel_known_recipe", &[("fan_tie", 2)], vec![]);
        let mut app = craft_refund_test_app(recipe, &[("fan_tie", 64)], 13);
        app.add_systems(Update, apply_craft_cancel_intents);

        let player = app
            .world_mut()
            .spawn(inv_with(&[("fan_tie", 1)]))
            .insert(CraftSession {
                recipe_id: RecipeId::new("craft.test.cancel_missing_recipe"),
                started_at_tick: 10,
                remaining_ticks: 20,
                total_ticks: 40,
                owner_player_id: "offline:Azure".into(),
                qi_paid: 0.0,
                quantity_total: 1,
                completed_count: 0,
            })
            .id();

        app.world_mut()
            .send_event(CraftCancelIntent { caster: player });
        app.update();

        assert!(
            current_failed_events(&app).is_empty(),
            "未知 recipe 尚未终止 session，不能发布语义为终结的 CraftFailedEvent"
        );
        assert!(
            app.world().get::<CraftSession>(player).is_some(),
            "未知 recipe 时无法重建退款 manifest，必须保留 session 避免吞掉预扣材料"
        );
    }

    #[test]
    fn cancel_refund_missing_drop_registry_preserves_session_then_retries_once() {
        let recipe = make_recipe("craft.test.refund_retry", &[("fan_tie", 2)], vec![]);
        let mut app = craft_refund_test_app(recipe, &[("fan_tie", 64)], 455);
        app.add_systems(Update, apply_craft_cancel_intents);
        app.world_mut().remove_resource::<DroppedLootRegistry>();

        let mut inventory = inv_with(&[("occupant", 1)]);
        clamp_main_pack_to_grid(&mut inventory, 1, 1);
        let player = app
            .world_mut()
            .spawn(inventory)
            .insert(Position::new([3.0, 65.0, 4.0]))
            .insert(CraftSession {
                recipe_id: RecipeId::new("craft.test.refund_retry"),
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

        assert!(
            current_failed_events(&app).is_empty(),
            "退款事务未提交时不能下发已取消 outcome"
        );
        assert!(
            app.world().get::<CraftSession>(player).is_some(),
            "缺少落地 registry 时必须保留 session 作为可重试退款凭证"
        );

        app.world_mut()
            .insert_resource(DroppedLootRegistry::default());
        app.world_mut()
            .send_event(CraftCancelIntent { caster: player });
        app.update();

        let failed = current_failed_events(&app);
        assert_eq!(failed.len(), 1, "依赖恢复后重试应只产生一次已提交退款事件");
        assert_eq!(
            failed[0].material_returned, 1,
            "重试应返还配方约定的一个材料"
        );
        assert!(
            app.world().get::<CraftSession>(player).is_none(),
            "退款成功提交后才可移除 session"
        );
        let dropped = app.world().resource::<DroppedLootRegistry>();
        assert_eq!(
            dropped.entries.len(),
            1,
            "跨帧重试只能落地一次，不能复制首次失败的退款"
        );
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

    /// plan-bughunt-craft-refund-full-inventory-loss-v1 P4 — `no containers` 是配置/结构
    /// 错误（`carried_container_candidate_indices(...).is_empty()`），绝不能被
    /// `add_item_to_player_inventory_or_ground` 当作 `inventory full:` 满包成功 fallback 到
    /// 地面掉落。Part A 直接命中生产退款入口 `grant_refund_manifest_to_inventory_or_ground`
    /// 断言精确错误文案；Part B 走真实 `CraftCancelIntent → apply_craft_cancel_intents`
    /// 生产系统，与同批次真正满包的对照玩家一起处理，证明两种错误在同一入口下被正确区分。
    #[test]
    fn refund_structural_error_does_not_mask_config_bug() {
        // ── Part A：直接命中生产退款入口，锁死精确错误文案与整批回滚 ──────────
        let registry = registry_with_templates(&[("fan_tie", 64)]);
        let mut inventory_no_containers = inv_with(&[]);
        inventory_no_containers.containers.clear();
        let original_revision = inventory_no_containers.revision;
        let mut allocator = InventoryInstanceIdAllocator::new(70);
        let mut dropped_loot = DroppedLootRegistry::default();

        let summary = grant_refund_manifest_to_inventory_or_ground(
            &mut inventory_no_containers,
            &registry,
            &mut allocator,
            Some(&mut dropped_loot),
            vec![("fan_tie".to_string(), 1)],
            1,
            RefundGroundTarget {
                pos: [0.0, 64.0, 0.0],
                dimension: DimensionKind::Overworld,
            },
        );

        assert_eq!(
            summary.material_returned, 0,
            "no containers 是结构错误而非满包成功，绝不能虚报已返还，实际={}",
            summary.material_returned
        );
        assert_eq!(
            summary.errors.len(),
            1,
            "应保留唯一结构错误供调用方诊断，实际={:?}",
            summary.errors
        );
        assert!(
            summary.errors[0].contains("player inventory has no containers"),
            "no containers 必须原样透传为结构错误，不能被 `inventory full:` fallback 判据吞掉，实际={:?}",
            summary.errors
        );
        assert!(
            dropped_loot.entries.is_empty(),
            "no containers 结构错误绝不能产生 DroppedLootEntry——否则配置 bug 会被掩盖成『满包已掉地上』"
        );
        assert!(
            inventory_no_containers.containers.is_empty(),
            "no containers 分支不应凭空补写容器"
        );
        assert_eq!(
            inventory_no_containers.revision, original_revision,
            "结构错误必须整批回滚：clone staging 不得发布，revision 不能变化"
        );
        assert_eq!(
            allocator.next_id().unwrap(),
            70,
            "结构错误分支必须在触发前就失败，不能消耗有效 instance id"
        );

        // ── Part B：真实 CraftCancelIntent → apply_craft_cancel_intents 生产系统，
        //    no containers 玩家与真正满包玩家同批处理，验证两者被正确区分 ──────────
        let recipe = make_recipe(
            "craft.test.refund_structural_error",
            &[("fan_tie", 2)],
            vec![],
        );
        let mut app = craft_refund_test_app(recipe, &[("fan_tie", 64)], 789);
        app.add_systems(Update, apply_craft_cancel_intents);

        let mut inventory_no_containers_ecs = inv_with(&[]);
        inventory_no_containers_ecs.containers.clear();
        let ecs_original_revision = inventory_no_containers_ecs.revision;
        let player_no_containers = app
            .world_mut()
            .spawn(inventory_no_containers_ecs)
            .insert(Position::new([1.0, 65.0, 1.0]))
            .insert(CraftSession {
                recipe_id: RecipeId::new("craft.test.refund_structural_error"),
                started_at_tick: 700,
                remaining_ticks: 20,
                total_ticks: 40,
                owner_player_id: canonical_player_id("Azure"),
                qi_paid: 0.0,
                quantity_total: 1,
                completed_count: 0,
            })
            .id();

        // 对照组：真正的满包（有容器、格子占满），同一入口必须走地面掉落成功。
        let mut inventory_full = inv_with(&[("occupant", 1)]);
        clamp_main_pack_to_grid(&mut inventory_full, 1, 1);
        let player_full = app
            .world_mut()
            .spawn(inventory_full)
            .insert(Position::new([2.0, 65.0, 2.0]))
            .insert(CraftSession {
                recipe_id: RecipeId::new("craft.test.refund_structural_error"),
                started_at_tick: 700,
                remaining_ticks: 20,
                total_ticks: 40,
                owner_player_id: canonical_player_id("Bob"),
                qi_paid: 0.0,
                quantity_total: 1,
                completed_count: 0,
            })
            .id();

        app.world_mut().send_event(CraftCancelIntent {
            caster: player_no_containers,
        });
        app.world_mut().send_event(CraftCancelIntent {
            caster: player_full,
        });
        app.update();

        let failed = current_failed_events(&app);
        assert_eq!(
            failed.len(),
            1,
            "两个 caster 里只有真正满包的对照组应完成退款事务并发布 Failed(PlayerCancelled)，\
             no containers 那个绝不能被误判成功，实际事件={:?}",
            failed
        );
        assert_eq!(
            failed[0].caster, player_full,
            "唯一发布的 Failed 事件必须属于满包对照组玩家，不能是 no containers 结构错误玩家"
        );
        assert_eq!(
            failed[0].material_returned, 1,
            "满包对照组按 70% 取整应实际返还 1 个材料"
        );

        assert!(
            app.world().get::<CraftSession>(player_no_containers).is_some(),
            "no containers 结构错误退款事务未提交，必须保留 CraftSession 作为可重试凭证，不能被误删"
        );
        assert!(
            app.world().get::<CraftSession>(player_full).is_none(),
            "满包对照组退款成功提交后 session 应正常结束"
        );

        let inventory_after = app
            .world()
            .get::<PlayerInventory>(player_no_containers)
            .unwrap();
        assert_eq!(
            inventory_after.revision, ecs_original_revision,
            "no containers 分支的 clone staging 绝不能发布到真实 PlayerInventory"
        );
        assert!(
            inventory_after.containers.is_empty(),
            "no containers 分支不应被结构错误路径意外补写容器"
        );

        let dropped = app.world().resource::<DroppedLootRegistry>();
        assert_eq!(
            dropped.entries.len(),
            1,
            "只有满包对照组的退款才允许落地，no containers 绝不能贡献 DroppedLootEntry，实际={:?}",
            dropped.entries
        );
        let entry = dropped.entries.values().next().unwrap();
        assert_eq!(
            entry.world_pos,
            [2.0, 65.0, 2.0],
            "唯一的地面掉落必须来自满包对照组玩家坐标，证明 no containers 玩家没有偷偷贡献掉落"
        );
        assert_eq!(entry.item.template_id, "fan_tie");
    }

    #[test]
    fn duplicate_cancel_intents_same_frame_refund_only_once() {
        let recipe = make_recipe(
            "craft.test.refund_cancel_duplicate",
            &[("fan_tie", 2)],
            vec![],
        );
        let mut app = craft_refund_test_app(recipe, &[("fan_tie", 64)], 457);
        app.add_systems(Update, apply_craft_cancel_intents);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let mut inventory = inv_with(&[("occupant", 1)]);
        clamp_main_pack_to_grid(&mut inventory, 1, 1);
        let player = app
            .world_mut()
            .spawn(client_bundle)
            .insert(inventory)
            .insert(Position::new([11.0, 65.0, -2.0]))
            .insert(CurrentDimension(DimensionKind::Tsy))
            .insert(CraftSession {
                recipe_id: RecipeId::new("craft.test.refund_cancel_duplicate"),
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
        app.world_mut()
            .send_event(CraftCancelIntent { caster: player });
        app.update();

        let failed = current_failed_events(&app);
        assert_eq!(
            failed.len(),
            1,
            "同帧重复 cancel 只能产生一条失败事件，避免 deferred remove 前重复退款，实际={failed:?}"
        );
        assert_eq!(
            failed[0].material_returned, 1,
            "同帧重复 cancel 的 material_returned 只能统计第一次真实落地退款"
        );
        assert!(
            app.world().get::<CraftSession>(player).is_none(),
            "同帧重复 cancel 处理后 session 应结束"
        );

        let dropped = app.world().resource::<DroppedLootRegistry>();
        assert_eq!(
            dropped.entries.len(),
            1,
            "同帧重复 cancel 满包退款只能产生一个地面掉落，避免复制材料"
        );
        let entry = dropped.entries.values().next().unwrap();
        assert_eq!(entry.item.template_id, "fan_tie");
        assert_eq!(entry.item.stack_count, 1);
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
    fn finalize_missing_recipe_preserves_completed_session_without_terminal_event() {
        let recipe = make_recipe("craft.test.known", &[("fan_tie", 2)], vec![]);
        let mut app = craft_refund_test_app(recipe, &[("fan_tie", 64)], 790);
        app.add_systems(Update, tick_craft_sessions);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let player = app
            .world_mut()
            .spawn(client_bundle)
            .insert(inv_with(&[]))
            .insert(CraftSession {
                recipe_id: RecipeId::new("craft.test.missing"),
                started_at_tick: 700,
                remaining_ticks: 1,
                total_ticks: 1,
                owner_player_id: canonical_player_id("Azure"),
                qi_paid: 0.0,
                quantity_total: 2,
                completed_count: 0,
            })
            .id();

        app.update();

        assert!(
            current_failed_events(&app).is_empty(),
            "配方缺失时退款 manifest 不可重建，不能发布终止事件后吞掉预扣材料"
        );
        assert!(
            current_completed_events(&app).is_empty(),
            "配方缺失时不能伪造完成事件"
        );
        let session = app.world().get::<CraftSession>(player).unwrap();
        assert_eq!(
            session.remaining_ticks, 0,
            "完成边界应被持久化为可恢复的 remaining_ticks=0 session"
        );
        assert!(
            app.world()
                .get::<CraftSessionPersistenceDirty>(player)
                .is_some(),
            "缺失配方的完成 session 必须标脏等待持久化，不能只留易失 ECS 状态"
        );
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
        crate::craft::register_workbench_recipes(&mut registry).unwrap();

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
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut()
            .entity_mut(entity)
            .insert(PlayerState::default());
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
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut()
            .entity_mut(entity)
            .insert(PlayerState::default());
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
    fn emit_recipe_list_on_join_waits_for_hydration_then_sends_active_session_state() {
        let mut app = App::new();
        let mut registry = CraftRegistry::new();
        register_basic_processing_recipes(&mut registry).unwrap();
        let unlock_state = RecipeUnlockState::new();
        app.insert_resource(registry);
        app.insert_resource(unlock_state);
        app.add_systems(Update, emit_recipe_list_on_join);

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.update();
        flush_client_packets(&mut app);
        assert!(
            collect_recipe_lists(&mut helper).is_empty(),
            "PlayerState hydration 完成前不得发送 recipe/session 首包并缓存 idle 状态"
        );
        assert!(
            collect_craft_session_states(&mut helper).is_empty(),
            "PlayerState hydration 完成前不得发送错误的 idle craft_session_state"
        );

        app.world_mut().entity_mut(entity).insert((
            PlayerState::default(),
            CraftSession {
                recipe_id: RecipeId::new("basic.wood_handle"),
                started_at_tick: 0,
                remaining_ticks: 20,
                total_ticks: 60,
                owner_player_id: canonical_player_id("Azure"),
                qi_paid: 0.0,
                quantity_total: 3,
                completed_count: 1,
            },
        ));
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
        app.insert_resource(
            crate::inventory::load_item_registry()
                .expect("craft emission test requires ItemRegistry"),
        );
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
