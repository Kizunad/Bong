//! 锻造会话到 R1 `InteractionSession` 的适配层。
//!
//! 锻造原有 `ForgeSession` 仍保存步骤与结算域状态；本模块只接管生命周期、facility
//! claim、checkpoint 和 runtime rebind。checkpoint 明确排除 `station`/`caster` 的 ECS
//! Entity，恢复时必须用稳定 `placed_id` 找到新实体。材料消耗和开光真元仍由既有锻造
//! 系统通过统一 ledger / delivery owner 处理。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use valence::prelude::Entity;

use crate::persistence::{ReconnectGuard, SuspendedSessionCheckpoint};
use crate::session::{
    BusyClaim, InteractionSession, PlayerKey, RuntimeBinding, SessionDecision, SessionDurability,
    SessionEvent, SessionKey, SessionLifecycleCtx, SessionRecord, SessionState,
};

use super::blueprint::BlueprintId;
use super::session::{ForgeSession, ForgeSessionId, ForgeStep, StepState};
use super::steps::{ConsecrationResult, InscriptionResult, TemperingResult};
use crate::world::dimension::DimensionKind;

/// 锻造 checkpoint 中允许出现的字段。
///
/// 这里不复用 `ForgeSession` 的 serde 形状，因为后者包含当前进程的 `station` 与
/// `caster` Entity；把它们写入 durable 数据会让重启后的实体重建无法恢复到同一工位。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgeSessionCheckpoint {
    pub id: ForgeSessionId,
    pub blueprint: BlueprintId,
    pub station_pos: Option<(i32, i32, i32)>,
    pub station_dimension: DimensionKind,
    pub step_index: usize,
    pub current_step: ForgeStep,
    pub step_state: StepState,
    pub total_deviation: u32,
    pub billet_flawed: bool,
    pub billet_carrier_cap: u8,
    pub flawed_marker: bool,
    pub committed_materials: HashMap<String, u32>,
    pub achieved_tier: u8,
    pub tempering_result: Option<TemperingResult>,
    pub inscription_result: Option<InscriptionResult>,
    pub consecration_result: Option<ConsecrationResult>,
    pub consecration_qi_injected: f64,
    pub done_retention_ticks: u32,
}

impl From<&ForgeSession> for ForgeSessionCheckpoint {
    fn from(session: &ForgeSession) -> Self {
        Self {
            id: session.id,
            blueprint: session.blueprint.clone(),
            station_pos: session.station_pos,
            station_dimension: session.station_dimension,
            step_index: session.step_index,
            current_step: session.current_step,
            step_state: session.step_state.clone(),
            total_deviation: session.total_deviation,
            billet_flawed: session.billet_flawed,
            billet_carrier_cap: session.billet_carrier_cap,
            flawed_marker: session.flawed_marker,
            committed_materials: session.committed_materials.clone(),
            achieved_tier: session.achieved_tier,
            tempering_result: session.tempering_result,
            inscription_result: session.inscription_result,
            consecration_result: session.consecration_result,
            consecration_qi_injected: session.consecration_qi_injected,
            done_retention_ticks: session.done_retention_ticks,
        }
    }
}

/// 锻造域的 checkpointed session adapter。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgeSessionAdapter {
    /// R1 生命周期与 station facility claim。
    pub record: SessionRecord,
    /// 锻造步骤域状态；runtime Entity 只在当前进程有效。
    pub session: ForgeSession,
    /// R3 restore 使用的稳定工位身份。
    durable_placed_id: String,
}

impl ForgeSessionAdapter {
    /// 用已经构造好的 R1 record 包装既有锻造 session。
    pub fn new(
        record: SessionRecord,
        session: ForgeSession,
        durable_placed_id: impl Into<String>,
    ) -> Self {
        Self {
            record,
            session,
            durable_placed_id: durable_placed_id.into(),
        }
    }

    /// 从当前工位建立稳定 station claim；`placed_id` 不得由 runtime Entity 派生。
    pub fn from_station(
        session: ForgeSession,
        session_key: impl Into<SessionKey>,
        owner_key: impl Into<PlayerKey>,
        placed_id: impl Into<String>,
    ) -> Self {
        let placed_id = placed_id.into();
        let mut record = SessionRecord::new(
            session_key,
            owner_key,
            SessionDurability::Checkpointed,
            BusyClaim::facility(format!("forge:station:{placed_id}")),
        );
        record.runtime_binding = Some(RuntimeBinding {
            entity_id: session.station.to_bits(),
            placed_id: Some(placed_id.clone()),
            dimension: session.station_dimension.ident_str().to_string(),
        });
        Self::new(record, session, placed_id)
    }

    /// 应用 R1 生命周期事件；durable fence 成功后解除当前进程的 Entity 绑定。
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

    /// 锻造步骤是事件驱动的；只有 Running session 才能接受步骤输入。
    pub fn can_process_step(&self) -> bool {
        self.record.state == SessionState::Running && !self.session.is_done()
    }

    /// 用新的 ECS 工位和施术者实体恢复 Suspended session。
    pub fn rebind_runtime(
        &mut self,
        station: Entity,
        caster: Entity,
        durable_placed_id: &str,
    ) -> Result<(), String> {
        if !matches!(
            self.record.state,
            SessionState::Running | SessionState::Paused
        ) {
            return Err("forge session is not attachable in its current state".to_string());
        }
        if self.durable_placed_id != durable_placed_id {
            return Err("forge placed_id does not match the suspended session".to_string());
        }
        self.session.station = station;
        self.session.caster = caster;
        self.record.runtime_binding = Some(RuntimeBinding {
            entity_id: station.to_bits(),
            placed_id: Some(durable_placed_id.to_string()),
            dimension: self.session.station_dimension.ident_str().to_string(),
        });
        Ok(())
    }

    /// 将 Suspended session 投影为不含 runtime Entity 的 R3 checkpoint。
    pub fn checkpoint(&self) -> Result<SuspendedSessionCheckpoint, String> {
        if self.record.state != SessionState::Suspended {
            return Err("forge checkpoint requires a suspended session".to_string());
        }
        let snapshot = ForgeSessionCheckpoint::from(&self.session);
        let checkpoint_json = serde_json::to_string(&snapshot)
            .map_err(|error| format!("serialize forge checkpoint: {error}"))?;
        Ok(SuspendedSessionCheckpoint {
            owner_key: self.record.owner_key.as_str().to_string(),
            session_key: self.record.session_key.as_str().to_string(),
            generation: self.record.generation,
            phase_revision: self.record.phase_revision,
            placed_id: Some(self.durable_placed_id.clone()),
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

    /// 稳定工位身份，供 R3 checkpoint 与 runtime rebind 对拍。
    pub fn durable_placed_id(&self) -> &str {
        &self.durable_placed_id
    }
}

impl InteractionSession for ForgeSessionAdapter {
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

    const RESTORE_TOKEN: &str = "forge-restore-token-012345678901";

    fn adapter() -> ForgeSessionAdapter {
        let mut session = ForgeSession::new(
            ForgeSessionId(9),
            "qing.feng".to_string(),
            Entity::from_raw(7),
            Entity::from_raw(8),
        );
        session.station_pos = Some((4, 65, -2));
        ForgeSessionAdapter::from_station(
            session,
            "forge-session-9",
            "offline:alice",
            "forge-placed-9",
        )
    }

    fn committed_guard(adapter: &ForgeSessionAdapter) -> ReconnectGuard {
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
    fn forge_checkpoint_omits_entities_and_rebinds_by_placed_id() {
        let mut adapter = adapter();
        let guard = committed_guard(&adapter);
        let mut context = SessionLifecycleCtx::default();
        let disconnected = adapter.apply_event(
            SessionEvent::Disconnect {
                identity: adapter.record.identity(),
                suspension: SuspensionResult::Committed(guard.clone()),
            },
            &mut context,
        );
        assert!(disconnected.accepted);
        assert!(adapter.record.runtime_binding.is_none());
        assert!(!adapter.can_process_step());

        let checkpoint = adapter
            .checkpoint()
            .expect("forge checkpoint should serialize");
        let value: serde_json::Value = serde_json::from_str(&checkpoint.checkpoint_json)
            .expect("forge checkpoint must be valid JSON");
        assert!(value.get("station").is_none());
        assert!(value.get("caster").is_none());
        assert_eq!(checkpoint.placed_id.as_deref(), Some("forge-placed-9"));
        let restored: ForgeSessionCheckpoint =
            serde_json::from_str(&checkpoint.checkpoint_json).expect("forge state is restorable");
        assert_eq!(restored.id, adapter.session.id);
        assert_eq!(restored.station_pos, adapter.session.station_pos);

        let restored = adapter.apply_event(
            SessionEvent::Restore {
                identity: adapter.record.identity(),
                guard,
                phase_revision: adapter.record.phase_revision + 1,
            },
            &mut context,
        );
        assert!(restored.accepted);
        assert_eq!(adapter.record.state, SessionState::Paused);
        assert!(adapter
            .rebind_runtime(Entity::from_raw(70), Entity::from_raw(80), "wrong-id")
            .is_err());
        adapter
            .rebind_runtime(Entity::from_raw(70), Entity::from_raw(80), "forge-placed-9")
            .expect("rebuilt station should bind with the stable placed_id");
        assert_eq!(adapter.session.station, Entity::from_raw(70));
        assert_eq!(adapter.session.caster, Entity::from_raw(80));
    }

    #[test]
    fn suspension_expiry_prepares_handoff_without_rebinding() {
        let mut adapter = adapter();
        let guard = committed_guard(&adapter);
        let mut context = SessionLifecycleCtx::default();
        let disconnected = adapter.apply_event(
            SessionEvent::Disconnect {
                identity: adapter.record.identity(),
                suspension: SuspensionResult::Committed(guard),
            },
            &mut context,
        );
        assert!(disconnected.accepted);
        assert_eq!(adapter.record.state, SessionState::Suspended);
        assert!(adapter.record.runtime_binding.is_none());

        let expired = adapter.apply_event(
            SessionEvent::SuspensionExpired {
                identity: adapter.record.identity(),
            },
            &mut context,
        );
        assert!(expired.accepted);
        assert_eq!(adapter.record.state, SessionState::HandoffPreparing);
        assert!(!adapter.can_process_step());
        assert!(adapter.record.runtime_binding.is_none());
    }

    #[test]
    fn forge_completion_handoff_ends_session_and_releases_runtime_binding() {
        let mut adapter = adapter();
        let mut context = SessionLifecycleCtx::default();
        let identity = adapter.record.identity();
        let prepared = adapter.apply_event(SessionEvent::Completed { identity }, &mut context);
        assert!(prepared.accepted);
        assert_eq!(adapter.record.state, SessionState::HandoffPreparing);
        assert!(!adapter.can_process_step());

        let committed = adapter.apply_event(
            SessionEvent::HandoffCommitted {
                identity: adapter.record.identity(),
            },
            &mut context,
        );
        assert!(committed.accepted);
        assert_eq!(adapter.record.state, SessionState::Ended);
        assert!(adapter.record.runtime_binding.is_none());
    }
}
