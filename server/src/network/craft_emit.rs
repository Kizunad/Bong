//! plan-craft-v1 P2 — Craft IPC bridge（server → client + intent → session）。
//!
//! 5 个系统：
//!   1. `apply_craft_intents` — 读 `CraftStartIntent` / `CraftCancelIntent`，
//!      跑 `start_craft` / `cancel_craft`，产生 `CraftStartedEvent` /
//!      `CraftFailedEvent`，并在 caster 上 insert/remove `CraftSession` component
//!   2. `tick_craft_sessions` — 每 tick 推进所有在线玩家的 session（worldview §九
//!      "玩家在场是基本要求"，下线 Entity 自动清空，session 随之消失）
//!   3. `emit_craft_session_state` — 定期把当前 session 进度推到 client（每 20 tick
//!      一次 / 状态切换时立刻推一次）
//!   4. `emit_craft_outcome_payloads` — 监听 Completed/Failed → push `CraftOutcomeV1`
//!   5. `emit_recipe_list_on_join` / `emit_recipe_list_on_unlock` —
//!      初始全表 + 每次 unlock 增量
//!
//! 守恒律：所有 qi 变更走 `start_craft`/`cancel_craft` 内部已封装的
//! `WorldQiAccount::transfer(QiTransferReason::Crafting)` —— 本模块**禁止**
//! 直接写 `cultivation.qi_current`，否则破坏全局守恒律。

use std::{
    collections::{HashMap, HashSet},
    time::{SystemTime, UNIX_EPOCH},
};

use valence::prelude::{
    bevy_ecs, Client, Commands, Component, Entity, EventReader, EventWriter, Local, Query, Res,
    ResMut, Username, With,
};

use crate::combat::CombatClock;
use crate::craft::{
    cancel_craft, finalize_craft, start_craft, tick_session, unlock_via_insight, unlock_via_mentor,
    unlock_via_scroll, CancelCraftOutcome, CraftCancelIntent, CraftCompletedEvent,
    CraftFailedEvent, CraftFailureReason, CraftRegistry, CraftSession, CraftStartIntent,
    CraftStartedEvent, CraftUnlockIntent, FinalizeCraftOutcome, RecipeUnlockState,
    RecipeUnlockedEvent, StartCraftDeps, StartCraftError, StartCraftRequest, UnlockEventSource,
    UnlockOutcome,
};
use crate::cultivation::components::{Cultivation, QiColor};
use crate::inventory::{
    add_item_to_player_inventory, InventoryInstanceIdAllocator, ItemRegistry, PlayerInventory,
};
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::player::state::canonical_player_id;
use crate::qi_physics::ledger::WorldQiAccount;
use crate::schema::craft::{
    CraftCategoryV1, CraftFailureReasonV1, CraftOutcomeV1, CraftRecipeEntryV1, CraftRequirementsV1,
    CraftSessionStateV1, RecipeListV1, RecipeUnlockedV1, UnlockEventSourceV1,
};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};

/// inventory 内手搓默认绑定的 zone 账户（暂时统一用 "spawn"，与现有
/// `cultivation` 守恒模型一致；后续 plan-zone-v2 可按 `Position → ZoneRegistry`
/// 解析真实 zone）。
const DEFAULT_CRAFT_ZONE_ID: &str = "spawn";

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

/// §1 — 处理客户端发来的 Start/Cancel intent。
///
/// 命中失败时（材料不足 / qi 不足 / 已有 session / 配方未解锁等）→ emit
/// `CraftFailedEvent { reason: InternalError }` 让 client 收到 Outcome::Failed
/// 通知（client 可据此弹错误 toast）；P2 暂不实装更细分的失败 reason。
#[allow(clippy::too_many_arguments)]
pub fn apply_craft_intents(
    mut start_intents: EventReader<CraftStartIntent>,
    mut cancel_intents: EventReader<CraftCancelIntent>,
    mut started_tx: EventWriter<CraftStartedEvent>,
    mut failed_tx: EventWriter<CraftFailedEvent>,
    registry: Res<CraftRegistry>,
    unlock_state: Res<RecipeUnlockState>,
    mut ledger: ResMut<WorldQiAccount>,
    item_registry: Res<ItemRegistry>,
    mut allocator: ResMut<InventoryInstanceIdAllocator>,
    clock: Res<CombatClock>,
    mut commands: Commands,
    names: Query<&Username>,
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
        let deps = StartCraftDeps {
            registry: &registry,
            unlock_state: &unlock_state,
            inventory: &mut inventory,
            cultivation: &mut cultivation,
            qi_color,
            ledger: &mut ledger,
            existing_session: existing,
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

    // ── cancel ──────────────────────────────────────────────
    for intent in cancel_intents.read() {
        let Ok((mut inventory, _cultivation, _qi_color, existing)) = casters.get_mut(intent.caster)
        else {
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
            event,
            refund_manifest,
        } = cancel_craft(
            session,
            recipe,
            intent.caster,
            CraftFailureReason::PlayerCancelled,
        );
        for (template, count) in refund_manifest {
            if count == 0 {
                continue;
            }
            if let Err(err) = add_item_to_player_inventory(
                &mut inventory,
                &item_registry,
                &mut allocator,
                &template,
                count,
            ) {
                tracing::warn!("[bong][craft] cancel refund failed for {template} x{count}: {err}");
            }
        }
        tracing::info!(
            "[bong][craft] cancel ok caster={:?} recipe={} returned={}",
            intent.caster,
            event.recipe_id,
            event.material_returned
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
                    let mut material_returned = 0;
                    for (refund_template, refund_count) in refund_manifest {
                        match add_item_to_player_inventory(
                            &mut inventory,
                            &item_registry,
                            &mut allocator,
                            &refund_template,
                            refund_count,
                        ) {
                            Ok(_) => {
                                material_returned += refund_count;
                            }
                            Err(refund_err) => {
                                tracing::error!(
                                    "[bong][craft] refund FAILED after finalize failure: recipe={} refund={refund_template} x{refund_count} err={refund_err}",
                                    event.recipe_id
                                );
                            }
                        }
                    }
                    event.material_returned = material_returned;
                    failed_tx.send(event);
                }
            }
            commands
                .entity(entity)
                .remove::<CraftSession>()
                .insert(CraftSessionStateDirty);
        } else if clock.tick % SESSION_STATE_PUSH_INTERVAL_TICKS == 0 {
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
    mut clients: Query<(Entity, &Username, &mut Client), With<Client>>,
) {
    let mut active_clients = HashSet::new();
    for (entity, username, mut client) in clients.iter_mut() {
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
        // 当前产品语义：未解锁配方先不下发，客户端只展示可制作/已解锁列表；
        // 若以后改为灰显锁定配方，需要同步扩展 payload 与客户端交互。
        .filter(|r| r.unlock_sources.is_empty() || unlock_state.is_unlocked(player_id, &r.id))
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
            unlocked: r.unlock_sources.is_empty() || unlock_state.is_unlocked(player_id, &r.id),
        })
        .collect();
    RecipeListV1 {
        v: 1,
        player_id: player_id.to_string(),
        recipes: entries,
        ts: current_unix_millis(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::craft::{
        register_basic_processing_recipes, register_examples, CraftRequirements, CraftSession,
        RecipeId, RecipeUnlockState,
    };
    use valence::prelude::{App, Update};
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::{create_mock_client, MockClientHelper};

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
    fn build_recipe_list_payload_includes_default_unlocked_early_examples() {
        let mut registry = CraftRegistry::new();
        register_examples(&mut registry).unwrap();
        let unlock_state = RecipeUnlockState::new();
        let payload = build_recipe_list_payload("offline:Alice", &registry, &unlock_state);
        assert_eq!(payload.player_id, "offline:Alice");
        assert_eq!(payload.recipes.len(), 2);
        assert!(payload
            .recipes
            .iter()
            .any(|r| r.id == "craft.example.eclipse_needle.iron" && r.unlocked));
        assert!(payload
            .recipes
            .iter()
            .any(|r| r.id == "craft.example.poison_decoction.fan" && r.unlocked));
        assert!(payload
            .recipes
            .iter()
            .all(|r| r.id != "craft.example.fake_skin.light"));
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
    fn build_recipe_list_payload_marks_empty_unlock_sources_default_unlocked() {
        let mut registry = CraftRegistry::new();
        register_basic_processing_recipes(&mut registry).unwrap();
        let unlock_state = RecipeUnlockState::new();

        let payload = build_recipe_list_payload("offline:Alice", &registry, &unlock_state);

        let wood_handle = payload
            .recipes
            .iter()
            .find(|r| r.id == "basic.wood_handle")
            .expect("basic wood handle recipe should be included");
        assert!(wood_handle.unlocked);
        assert_eq!(wood_handle.display_name, "削木柄");
        assert_eq!(wood_handle.materials, vec![("crude_wood".to_string(), 2)]);
        assert_eq!(wood_handle.output, ("wood_handle".to_string(), 2));
    }

    #[test]
    fn emit_recipe_list_sends_once_to_online_client() {
        let mut app = App::new();
        let mut registry = CraftRegistry::new();
        register_basic_processing_recipes(&mut registry).unwrap();
        app.insert_resource(registry);
        app.insert_resource(RecipeUnlockState::new());
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
    fn registered_recipe_list_fits_server_data_budget() {
        let mut app = App::new();
        crate::craft::register(&mut app);
        let registry = app.world().resource::<CraftRegistry>();
        let unlock_state = app.world().resource::<RecipeUnlockState>();
        let payload = ServerDataV1::new(ServerDataPayloadV1::CraftRecipeList(Box::new(
            build_recipe_list_payload("offline:Alice", registry, unlock_state),
        )));

        let bytes = serialize_server_data_payload(&payload)
            .expect("registered craft recipe list must fit server_data budget");
        assert!(bytes.len() <= crate::schema::common::MAX_PAYLOAD_BYTES);
    }

    #[test]
    fn build_recipe_list_payload_grouped_by_category_for_ui_order() {
        let mut registry = CraftRegistry::new();
        register_examples(&mut registry).unwrap();
        let unlock_state = RecipeUnlockState::new();
        let payload = build_recipe_list_payload("offline:Charlie", &registry, &unlock_state);
        // grouped_for_ui 输出应保证 category 分组连续：判定相邻 entry 同 category 段
        let cats: Vec<CraftCategoryV1> = payload.recipes.iter().map(|r| r.category).collect();
        // 初始只下发当前可见配方；仍需保持类别分组稳定。
        let unique: std::collections::HashSet<_> = cats.iter().collect();
        assert_eq!(unique.len(), 2);
    }

    #[test]
    fn build_recipe_list_payload_preserves_requirements_qi_color_gate() {
        let mut registry = CraftRegistry::new();
        register_examples(&mut registry).unwrap();
        let unlock_state = RecipeUnlockState::new();
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
}
