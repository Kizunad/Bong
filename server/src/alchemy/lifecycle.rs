//! 炼丹 adapter 的生产生命周期接线。
//!
//! `AlchemySessionAdapter` 负责 reducer 与域状态；本模块负责把断线、关服和重连的
//! production 触发点接到 R3 的 checkpoint/guard 事务。持久化失败时恢复调用前的
//! adapter，生产 tick 继续沿用旧路径，避免把未落盘的 Suspended 当成权威状态。

use valence::prelude::{Added, AppExit, Client, EventReader, Query, RemovedComponents, Res};

use crate::persistence::{
    consume_reconnect_guard, load_suspended_session_bundle, persist_suspended_session_checkpoint,
    persist_suspended_session_checkpoints, SuspendedCheckpointPersistOutcome,
};
use crate::player::state::{canonical_player_id, PlayerStatePersistence};
use crate::session::{SessionEvent, SessionLifecycleCtx, SessionState};
use crate::world::dimension::DimensionKind;

use super::{AlchemyFurnace, AlchemySessionAdapter};

fn identity_matches(left: &str, right: &str) -> bool {
    left == right
        || left.strip_prefix("offline:").unwrap_or(left)
            == right.strip_prefix("offline:").unwrap_or(right)
}

fn session_belongs_to_player(session: &AlchemySessionAdapter, player_id: &str) -> bool {
    identity_matches(session.record.owner_key.as_str(), player_id)
        || identity_matches(&session.session.caster_id, player_id)
}

/// 断线时先通过 adapter reducer 进入 Suspended，再把 checkpoint 与 guard 原子落盘。
///
/// 该系统必须排在 `refund_alchemy_qi_on_disconnect` 之前：成功 checkpoint 的炉次保留
/// 真实账户余额，等 guarded restore 后继续使用；只有持久化失败回滚到 Running 的炉次
/// 才走旧的退款/overflow 兜底。
pub(crate) fn checkpoint_alchemy_sessions_on_disconnect(
    mut disconnected: RemovedComponents<Client>,
    usernames: Query<&valence::prelude::Username>,
    mut furnaces: Query<&mut AlchemyFurnace>,
    persistence: Option<Res<PlayerStatePersistence>>,
) {
    let Some(persistence) = persistence else {
        return;
    };
    for player in disconnected.read() {
        let Ok(username) = usernames.get(player) else {
            continue;
        };
        let player_id = canonical_player_id(username.0.as_str());
        for mut furnace in &mut furnaces {
            let Some(adapter) = furnace.session.as_mut() else {
                continue;
            };
            if !matches!(
                adapter.record.state,
                SessionState::Running | SessionState::Paused
            ) || !session_belongs_to_player(adapter, &player_id)
            {
                continue;
            }
            let previous = adapter.clone();
            let mut context = SessionLifecycleCtx::default();
            let Ok((guard, checkpoint)) = adapter.suspend_for_disconnect(&mut context) else {
                continue;
            };
            match persist_suspended_session_checkpoint(&persistence, &checkpoint, &guard) {
                Ok(
                    SuspendedCheckpointPersistOutcome::Inserted
                    | SuspendedCheckpointPersistOutcome::Updated,
                ) => {
                    tracing::info!(
                        "[bong][alchemy] disconnected furnace session suspended: {}",
                        checkpoint.session_key
                    );
                }
                Ok(SuspendedCheckpointPersistOutcome::IgnoredStale) => {
                    *adapter = previous;
                    tracing::warn!(
                        "[bong][alchemy] stale disconnect checkpoint ignored for {}",
                        checkpoint.session_key
                    );
                }
                Err(error) => {
                    *adapter = previous;
                    tracing::warn!(
                        "[bong][alchemy] disconnect checkpoint failed for {}: {error}",
                        checkpoint.session_key
                    );
                }
            }
        }
    }
}

struct PendingShutdownCheckpoint {
    furnace: valence::prelude::Entity,
    previous: AlchemySessionAdapter,
    checkpoint: crate::persistence::SuspendedSessionCheckpoint,
    guard: crate::persistence::ReconnectGuard,
}

/// 关服与断线共用 durable fence；成功的 Suspended 炉次不会被 shutdown qi flush 提前
/// 移入 overflow，下一次同一进程重连仍可用 guard 恢复。
pub(crate) fn checkpoint_alchemy_sessions_on_shutdown(
    mut app_exit: EventReader<AppExit>,
    mut furnaces: Query<(valence::prelude::Entity, &mut AlchemyFurnace)>,
    persistence: Option<Res<PlayerStatePersistence>>,
) {
    if app_exit.read().next().is_none() {
        return;
    }
    let Some(persistence) = persistence else {
        return;
    };
    let mut pending = Vec::new();
    for (furnace_entity, mut furnace) in &mut furnaces {
        let Some(adapter) = furnace.session.as_mut() else {
            continue;
        };
        if !matches!(
            adapter.record.state,
            SessionState::Running | SessionState::Paused
        ) {
            continue;
        }
        let previous = adapter.clone();
        let mut context = SessionLifecycleCtx::default();
        let Ok((guard, checkpoint)) = adapter.suspend_for_shutdown(&mut context) else {
            continue;
        };
        pending.push(PendingShutdownCheckpoint {
            furnace: furnace_entity,
            previous,
            checkpoint,
            guard,
        });
    }
    if pending.is_empty() {
        return;
    }

    let batch: Vec<_> = pending
        .iter()
        .map(|item| (&item.checkpoint, &item.guard))
        .collect();
    match persist_suspended_session_checkpoints(&persistence, &batch) {
        Ok(outcomes) => {
            for (item, outcome) in pending.iter().zip(outcomes) {
                if outcome == SuspendedCheckpointPersistOutcome::IgnoredStale {
                    if let Ok((_, mut furnace)) = furnaces.get_mut(item.furnace) {
                        if let Some(adapter) = furnace.session.as_mut() {
                            *adapter = item.previous.clone();
                        }
                    }
                    tracing::warn!(
                        "[bong][alchemy] stale shutdown checkpoint ignored for {}",
                        item.checkpoint.session_key
                    );
                }
            }
        }
        Err(error) => {
            for item in &pending {
                if let Ok((_, mut furnace)) = furnaces.get_mut(item.furnace) {
                    if let Some(adapter) = furnace.session.as_mut() {
                        *adapter = item.previous.clone();
                    }
                }
            }
            tracing::warn!(
                "[bong][alchemy] shutdown checkpoint batch failed; restored {} session(s): {error}",
                pending.len()
            );
        }
    }
}

/// 玩家重新连接时读取服务端 guard，恢复炉内域状态并用稳定 placed_id 绑定新实体。
///
/// 恢复完成前不消费 guard；`rebind_runtime`、显式 Resume 和 guard CAS 全部成功后才
/// 删除一次性凭证，任何一步失败都保留原 Suspended adapter 并 fail closed。
pub(crate) fn restore_suspended_alchemy_sessions_on_join(
    joined: Query<&valence::prelude::Username, Added<Client>>,
    mut furnaces: Query<(valence::prelude::Entity, &mut AlchemyFurnace)>,
    persistence: Option<Res<PlayerStatePersistence>>,
) {
    let Some(persistence) = persistence else {
        return;
    };
    for username in &joined {
        let player_id = canonical_player_id(username.0.as_str());
        for (furnace_entity, mut furnace) in &mut furnaces {
            let Some(placed_id) = furnace.stable_placed_id() else {
                continue;
            };
            let Some(adapter) = furnace.session.as_mut() else {
                continue;
            };
            if adapter.record.state != SessionState::Suspended
                || !session_belongs_to_player(adapter, &player_id)
            {
                continue;
            }
            let session_key = adapter.record.session_key.as_str().to_string();
            let bundle = match load_suspended_session_bundle(&persistence, &session_key) {
                Ok(Some(bundle)) => bundle,
                Ok(None) => continue,
                Err(error) => {
                    tracing::warn!(
                        "[bong][alchemy] guarded restore load failed for {session_key}: {error}"
                    );
                    continue;
                }
            };
            if bundle.checkpoint.placed_id.as_deref() != Some(placed_id.as_str()) {
                tracing::warn!(
                    "[bong][alchemy] guarded restore placed_id mismatch for {session_key}"
                );
                continue;
            }
            let restored_domain: super::session::AlchemySession = match serde_json::from_str(
                &bundle.checkpoint.checkpoint_json,
            ) {
                Ok(domain) => domain,
                Err(error) => {
                    tracing::warn!(
                        "[bong][alchemy] guarded restore domain decode failed for {session_key}: {error}"
                    );
                    continue;
                }
            };
            let previous = adapter.clone();
            adapter.session = restored_domain;
            let Some(restore_revision) = bundle.reconnect_guard.phase_revision.checked_add(1)
            else {
                adapter.clone_from(&previous);
                continue;
            };
            let mut context = SessionLifecycleCtx::default();
            let restored = adapter.apply_event(
                SessionEvent::Restore {
                    identity: adapter.record.identity(),
                    guard: bundle.reconnect_guard.clone(),
                    phase_revision: restore_revision,
                },
                &mut context,
            );
            if !restored.accepted
                || adapter
                    .rebind_runtime(furnace_entity, &placed_id, DimensionKind::Overworld)
                    .is_err()
            {
                *adapter = previous;
                continue;
            }
            let resumed = adapter.apply_event(
                SessionEvent::Resume {
                    identity: adapter.record.identity(),
                },
                &mut context,
            );
            if !resumed.accepted
                || !consume_reconnect_guard(&persistence, &bundle.reconnect_guard).unwrap_or(false)
            {
                *adapter = previous;
                continue;
            }
            tracing::info!(
                "[bong][alchemy] guarded restore rebound furnace session: {session_key}"
            );
        }
    }
}
