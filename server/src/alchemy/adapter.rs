//! 炼丹会话到 R1 `InteractionSession` 的适配层。
//!
//! 这里只负责把既有的 `AlchemySession` 域状态放进统一生命周期 reducer；炼丹的配方
//! 结算、物品扣除和真元转账仍由 alchemy/qi 与 delivery owner 负责。适配层不直接写
//! `Cultivation.qi_current`，也不把运行期 Entity 写进 checkpoint。

use serde::{Deserialize, Serialize};
use valence::prelude::Entity;

use crate::persistence::{ReconnectGuard, SuspendedSessionCheckpoint};
use crate::session::{
    BusyClaim, InteractionSession, PlayerKey, RuntimeBinding, SessionDecision, SessionDurability,
    SessionEvent, SessionKey, SessionLifecycleCtx, SessionRecord, SessionState,
};
use crate::world::dimension::DimensionKind;

use super::session::AlchemySession;

/// 炼丹域的 checkpointed session adapter。
///
/// `durable_placed_id` 是炉的持久身份；`record.runtime_binding` 只保存当前进程的 Entity
/// locator。断线的 durable checkpoint 提交后，adapter 会清掉 locator，直到 guarded restore
/// 成功并由调用方显式 `rebind_runtime` 才恢复 tick。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlchemySessionAdapter {
    /// R1 唯一生命周期状态与 facility claim。
    pub record: SessionRecord,
    /// 炼丹域自己的火候、投料和香座状态。
    pub session: AlchemySession,
    /// 炉的稳定放置身份；不随 ECS Entity 重建而改变。
    #[serde(default)]
    durable_placed_id: Option<String>,
}

impl AlchemySessionAdapter {
    /// 用已经构造好的 R1 record 包装一个既有炼丹 session。
    pub fn new(
        record: SessionRecord,
        session: AlchemySession,
        durable_placed_id: Option<String>,
    ) -> Self {
        let durable_placed_id = durable_placed_id.or_else(|| {
            record
                .runtime_binding
                .as_ref()
                .and_then(|binding| binding.placed_id.clone())
        });
        Self {
            record,
            session,
            durable_placed_id,
        }
    }

    /// 从一个当前挂在炉实体上的 session 建立标准 facility claim。
    ///
    /// `furnace_key` 必须是跨重启稳定的业务键（通常由放置坐标或 R3 的 `placed_id`
    /// 派生），不能使用 `Entity` 的 Debug 字符串。
    pub fn from_furnace(
        session: AlchemySession,
        session_key: impl Into<SessionKey>,
        furnace_key: impl Into<String>,
        furnace_entity: Entity,
        durable_placed_id: Option<String>,
        dimension: DimensionKind,
    ) -> Self {
        let furnace_key = furnace_key.into();
        let mut record = SessionRecord::new(
            session_key,
            PlayerKey::new(session.caster_id.clone()),
            SessionDurability::Checkpointed,
            BusyClaim::facility(format!("alchemy:furnace:{furnace_key}")),
        );
        record.runtime_binding = Some(RuntimeBinding {
            entity_id: furnace_entity.to_bits(),
            placed_id: durable_placed_id.clone(),
            dimension: dimension.ident_str().to_string(),
        });
        Self::new(record, session, durable_placed_id)
    }

    /// 应用一个 R1 生命周期事件，并在 durable fence 后解绑运行期炉实体。
    pub fn apply_event(
        &mut self,
        event: SessionEvent,
        context: &mut SessionLifecycleCtx,
    ) -> SessionDecision {
        let unbind_after_checkpoint = matches!(
            &event,
            SessionEvent::Disconnect { .. } | SessionEvent::Shutdown { .. }
        );
        let unbind_after_handoff = matches!(&event, SessionEvent::HandoffCommitted { .. });
        let decision = self.record.reduce(event, context);
        if decision.accepted
            && ((unbind_after_checkpoint && decision.next_state == SessionState::Suspended)
                || (unbind_after_handoff && decision.next_state == SessionState::Ended))
        {
            self.record.runtime_binding = None;
        }
        decision
    }

    /// 只有 Running 状态推进火候；Paused、Suspended 和终态都禁止后台 tick。
    pub fn tick(&mut self) -> bool {
        if self.record.state != SessionState::Running {
            return false;
        }
        self.session.tick();
        true
    }

    /// 以新的 ECS Entity 重新绑定已经通过 guarded restore 的炉。
    pub fn rebind_runtime(
        &mut self,
        furnace_entity: Entity,
        durable_placed_id: Option<&str>,
        dimension: DimensionKind,
    ) -> Result<(), String> {
        if !matches!(
            self.record.state,
            SessionState::Running | SessionState::Paused
        ) {
            return Err("alchemy session is not attachable in its current state".to_string());
        }
        if self.durable_placed_id.as_deref() != durable_placed_id {
            return Err("alchemy placed_id does not match the suspended session".to_string());
        }
        self.record.runtime_binding = Some(RuntimeBinding {
            entity_id: furnace_entity.to_bits(),
            placed_id: durable_placed_id.map(str::to_string),
            dimension: dimension.ident_str().to_string(),
        });
        Ok(())
    }

    /// 将 Suspended session 投影为 R3 使用的稳定 checkpoint。
    pub fn checkpoint(&self) -> Result<SuspendedSessionCheckpoint, String> {
        if self.record.state != SessionState::Suspended {
            return Err("alchemy checkpoint requires a suspended session".to_string());
        }
        if self.durable_placed_id.is_none() {
            return Err("alchemy checkpoint requires a stable furnace placed_id".to_string());
        }
        let checkpoint_json = serde_json::to_string(&self.session)
            .map_err(|error| format!("serialize alchemy checkpoint: {error}"))?;
        Ok(SuspendedSessionCheckpoint {
            owner_key: self.record.owner_key.as_str().to_string(),
            session_key: self.record.session_key.as_str().to_string(),
            generation: self.record.generation,
            phase_revision: self.record.phase_revision,
            placed_id: self.durable_placed_id.clone(),
            checkpoint_json,
        })
    }

    /// 返回当前暂停快照对应的一次性恢复凭证。
    pub fn reconnect_guard(&self) -> Option<ReconnectGuard> {
        self.record
            .restore_token
            .as_ref()
            .map(|restore_token| ReconnectGuard {
                owner_key: self.record.owner_key.as_str().to_string(),
                session_key: self.record.session_key.as_str().to_string(),
                generation: self.record.generation,
                phase_revision: self.record.phase_revision,
                restore_token: restore_token.clone(),
            })
    }

    /// 稳定炉身份，供 restore/rebind 与 registry 对拍。
    pub fn durable_placed_id(&self) -> Option<&str> {
        self.durable_placed_id.as_deref()
    }
}

impl InteractionSession for AlchemySessionAdapter {
    fn session_key(&self) -> SessionKey {
        self.record.session_key.clone()
    }

    fn owner_key(&self) -> &PlayerKey {
        &self.record.owner_key
    }

    fn durability(&self) -> SessionDurability {
        self.record.durability
    }

    fn busy_claim(&self) -> BusyClaim {
        self.record.busy_claim.clone()
    }

    fn reduce(
        &mut self,
        event: SessionEvent,
        context: &mut SessionLifecycleCtx,
    ) -> SessionDecision {
        self.apply_event(event, context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::SuspensionResult;

    const RESTORE_TOKEN: &str = "alchemy-restore-token-012345678901";

    fn adapter() -> AlchemySessionAdapter {
        AlchemySessionAdapter::from_furnace(
            AlchemySession::new("dan.tui".to_string(), "offline:alice".to_string()),
            "alchemy-session-1",
            "placed-furnace-1",
            Entity::from_raw(7),
            Some("placed-furnace-1".to_string()),
            DimensionKind::Overworld,
        )
    }

    fn committed_guard(adapter: &AlchemySessionAdapter) -> ReconnectGuard {
        ReconnectGuard {
            owner_key: adapter.record.owner_key.as_str().to_string(),
            session_key: adapter.record.session_key.as_str().to_string(),
            generation: adapter.record.generation,
            // The reducer bumps the phase revision when it enters Suspended.  The
            // durable fence therefore carries the revision that will be committed,
            // rather than the pre-disconnect revision.
            phase_revision: adapter.record.phase_revision + 1,
            restore_token: RESTORE_TOKEN.to_string(),
        }
    }

    #[test]
    fn disconnect_checkpoint_stops_tick_and_restores_by_stable_furnace_identity() {
        let mut adapter = adapter();
        assert!(adapter.tick());
        let before_disconnect = adapter.session.elapsed_ticks;
        let identity = adapter.record.identity();
        let guard = committed_guard(&adapter);
        let mut context = SessionLifecycleCtx::default();
        let decision = adapter.apply_event(
            SessionEvent::Disconnect {
                identity,
                suspension: SuspensionResult::Committed(guard.clone()),
            },
            &mut context,
        );

        assert!(decision.accepted);
        assert_eq!(adapter.record.state, SessionState::Suspended);
        assert!(adapter.record.runtime_binding.is_none());
        assert!(!adapter.tick());
        assert_eq!(adapter.session.elapsed_ticks, before_disconnect);

        let checkpoint = adapter
            .checkpoint()
            .expect("suspended checkpoint should serialize");
        assert_eq!(checkpoint.placed_id.as_deref(), Some("placed-furnace-1"));
        let restored_session: AlchemySession =
            serde_json::from_str(&checkpoint.checkpoint_json).expect("session state is restorable");
        assert_eq!(restored_session.recipe, adapter.session.recipe);
        assert_eq!(restored_session.elapsed_ticks, before_disconnect);

        let restore_identity = adapter.record.identity();
        let restore_revision = adapter.record.phase_revision + 1;
        let restored = adapter.apply_event(
            SessionEvent::Restore {
                identity: restore_identity,
                guard,
                phase_revision: restore_revision,
            },
            &mut context,
        );
        assert!(restored.accepted);
        assert_eq!(adapter.record.state, SessionState::Paused);
        adapter
            .rebind_runtime(
                Entity::from_raw(70),
                Some("placed-furnace-1"),
                DimensionKind::Overworld,
            )
            .expect("restore should rebind a rebuilt furnace entity");
        assert!(adapter.record.runtime_binding.is_some());
        assert!(adapter.reconnect_guard().is_none());
        assert!(
            !adapter.tick(),
            "restore remains paused until an explicit resume"
        );
    }

    #[test]
    fn suspension_expiry_handoffs_without_resuming_or_rebinding() {
        let mut adapter = adapter();
        let identity = adapter.record.identity();
        let guard = committed_guard(&adapter);
        let mut context = SessionLifecycleCtx::default();
        adapter.apply_event(
            SessionEvent::Disconnect {
                identity,
                suspension: SuspensionResult::Committed(guard),
            },
            &mut context,
        );
        let decision = adapter.apply_event(
            SessionEvent::SuspensionExpired {
                identity: adapter.record.identity(),
            },
            &mut context,
        );
        assert!(decision.accepted);
        assert_eq!(adapter.record.state, SessionState::HandoffPreparing);
        assert!(!adapter.tick());
        assert!(adapter.record.runtime_binding.is_none());
    }
}
