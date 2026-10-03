//! plan-forge-v1 §1.3 四步进程 Session 状态机。

use std::collections::{hash_map::Entry, HashMap};

use serde::{Deserialize, Serialize};
use valence::prelude::{Entity, Resource};

use super::adapter::ForgeSessionAdapter;
use super::blueprint::{BlueprintId, StepKind};
use super::steps::{ConsecrationResult, InscriptionResult, TemperingResult};
use crate::cultivation::components::ColorKind;
use crate::world::dimension::DimensionKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ForgeSessionId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ForgeStep {
    Billet,
    Tempering,
    Inscription,
    Consecration,
    Done,
}

impl ForgeStep {
    pub fn from_kind(kind: StepKind) -> Self {
        match kind {
            StepKind::Billet => ForgeStep::Billet,
            StepKind::Tempering => ForgeStep::Tempering,
            StepKind::Inscription => ForgeStep::Inscription,
            StepKind::Consecration => ForgeStep::Consecration,
        }
    }
}

/// 每步独立状态。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StepState {
    Billet(BilletState),
    Tempering(TemperingState),
    Inscription(InscriptionState),
    Consecration(ConsecrationState),
    None,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BilletState {
    /// 投入物料：material -> count。
    pub materials_in: HashMap<String, u32>,
    /// 已确认的载体 material（决定 tier_cap）。
    pub active_carrier: Option<String>,
    pub resolved_tier_cap: u8,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TemperingState {
    /// 已处理到 pattern 的第几拍。
    pub beat_cursor: usize,
    pub hits: u32,
    pub misses: u32,
    /// 累积偏差（miss + 异键）。
    pub deviation: u32,
    /// 已消耗真元量（累计）。
    pub qi_spent: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InscriptionState {
    pub scrolls_in: Vec<String>,
    pub filled_slots: u8,
    pub failed: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConsecrationState {
    pub qi_injected: f64,
    pub qi_required: f64,
    pub color_imprint: Option<ColorKind>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgeSession {
    pub id: ForgeSessionId,
    pub blueprint: BlueprintId,
    pub station: Entity,
    /// 起炉时快照的锻炉方块坐标；结算时即使锻炉实体被清理，成品仍有可靠落点。
    #[serde(default)]
    pub station_pos: Option<(i32, i32, i32)>,
    /// 起炉时快照的锻炉维度；实体被清理或玩家换维度后仍能做权威范围校验。
    #[serde(default)]
    pub station_dimension: DimensionKind,
    pub caster: Entity,
    /// 当前步骤在图谱 steps[] 中的 index。
    pub step_index: usize,
    pub current_step: ForgeStep,
    pub step_state: StepState,
    /// 本次会话总偏差（跨步累积，决定最终 bucket）。
    pub total_deviation: u32,
    /// Billet 自身是否带 flaw（异物 / tolerance 内缺料）。
    pub billet_flawed: bool,
    /// Billet 决定的载体品阶上限；后续 step 计算 achieved_tier 不能再从 step_state 回溯。
    pub billet_carrier_cap: u8,
    /// 任一 step 是否已走 flawed 路径（仅作快速标记；最终 bucket 以各 step 实际结果为准）。
    pub flawed_marker: bool,
    /// 已锁定不可返还的材料（投入即消耗）。
    pub committed_materials: HashMap<String, u32>,
    /// 最终达成的 tier（坯料后刷新，后续可跳步下调）。
    pub achieved_tier: u8,
    /// 各步实际结算结果，供最终 bucket / tier 计算复用。
    pub tempering_result: Option<TemperingResult>,
    pub inscription_result: Option<InscriptionResult>,
    pub consecration_result: Option<ConsecrationResult>,
    /// 开光阶段累计注入真元。ForgeOutcomeEvent 需要它初始化法器铭纹深度。
    #[serde(default)]
    pub consecration_qi_injected: f64,
    /// Done 状态下已累积的 tick 数。
    /// cleanup_completed_sessions 系统每 tick 对 Done session 累加此值，
    /// 超过 DONE_SESSION_RETENTION_TICKS 后才从 ForgeSessions 移除。
    #[serde(default)]
    pub done_retention_ticks: u32,
}

impl ForgeSession {
    pub fn new(
        id: ForgeSessionId,
        blueprint: BlueprintId,
        station: Entity,
        caster: Entity,
    ) -> Self {
        Self {
            id,
            blueprint,
            station,
            station_pos: None,
            station_dimension: DimensionKind::Overworld,
            caster,
            step_index: 0,
            current_step: ForgeStep::Billet,
            step_state: StepState::None,
            total_deviation: 0,
            billet_flawed: false,
            billet_carrier_cap: 1,
            flawed_marker: false,
            committed_materials: HashMap::new(),
            achieved_tier: 0,
            tempering_result: None,
            inscription_result: None,
            consecration_result: None,
            consecration_qi_injected: 0.0,
            done_retention_ticks: 0,
        }
    }

    pub fn is_done(&self) -> bool {
        self.current_step == ForgeStep::Done
    }
}

/// Done session 在被移除前保留的 tick 数。
/// client_request_handler 仍可在这段窗口内读 Done session 来拒绝后续操作，
/// 超过此阈值后 cleanup_completed_sessions 系统每 tick 批量移除。
pub const DONE_SESSION_RETENTION_TICKS: u32 = 3;

/// 所有在炉 session 的总表。ForgeSessionId → ForgeSession。
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ForgeSessions {
    next_id: u64,
    sessions: HashMap<ForgeSessionId, ForgeSessionAdapter>,
}

impl Resource for ForgeSessions {}

impl ForgeSessions {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            sessions: HashMap::new(),
        }
    }

    pub fn allocate_id(&mut self) -> ForgeSessionId {
        let id = ForgeSessionId(self.next_id);
        self.next_id += 1;
        id
    }

    pub fn insert(&mut self, session: ForgeSession) -> bool {
        let placed_id = session
            .station_pos
            .map(|(x, y, z)| {
                format!(
                    "forge:station:{}:{x}:{y}:{z}",
                    session.station_dimension.ident_str()
                )
            })
            .unwrap_or_else(|| format!("forge:session:{}", session.id.0));
        let owner_key = format!("forge:session-owner:{}", session.id.0);
        let adapter = ForgeSessionAdapter::from_station(
            session,
            format!("forge:session:{}", placed_id),
            owner_key,
            placed_id,
        );
        self.insert_adapter(adapter)
    }

    /// 插入已经带有稳定工位身份的生产 adapter；生产起锻路径必须使用此入口。
    ///
    /// 会话 ID 是所有权键，重复发布必须拒绝而不能覆盖现有 adapter。返回 `true`
    /// 表示发布成功，`false` 表示调用方仍保有传入 adapter 的所有权并应回滚外部事务。
    pub fn insert_adapter(&mut self, adapter: ForgeSessionAdapter) -> bool {
        let id = adapter.session.id;
        match self.sessions.entry(id) {
            Entry::Vacant(entry) => {
                entry.insert(adapter);
                true
            }
            Entry::Occupied(_) => false,
        }
    }

    pub fn get(&self, id: ForgeSessionId) -> Option<&ForgeSessionAdapter> {
        self.sessions.get(&id)
    }

    pub fn get_mut(&mut self, id: ForgeSessionId) -> Option<&mut ForgeSessionAdapter> {
        self.sessions.get_mut(&id)
    }

    pub fn remove(&mut self, id: ForgeSessionId) -> Option<ForgeSessionAdapter> {
        self.sessions.remove(&id)
    }

    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    /// 每 tick 对 Done session 累加 retention 计数，超过阈值后移除。
    /// 返回本次移除的 session id 列表（供日志/测试断言）。
    ///
    /// # 约束
    /// 不在 finalize 时即时 remove——client_request_handler 仍需在短暂窗口内
    /// 读 Done session 来拒绝后续操作；延迟 DONE_SESSION_RETENTION_TICKS 后清理。
    pub fn cleanup_completed(&mut self) -> Vec<ForgeSessionId> {
        let mut to_remove: Vec<ForgeSessionId> = Vec::new();
        for (id, session) in self.sessions.iter_mut() {
            if session.current_step == ForgeStep::Done {
                session.done_retention_ticks += 1;
                if session.done_retention_ticks >= DONE_SESSION_RETENTION_TICKS {
                    to_remove.push(*id);
                }
            }
        }
        for id in &to_remove {
            self.sessions.remove(id);
        }
        to_remove
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_adapter_refuses_to_overwrite_existing_session_owner() {
        let id = ForgeSessionId(7);
        let first = ForgeSessionAdapter::from_station(
            ForgeSession::new(
                id,
                "iron_sword_v0".to_string(),
                Entity::from_raw(1),
                Entity::from_raw(2),
            ),
            "forge:session:first",
            "offline:alice",
            "forge:station:overworld:1:64:1",
        );
        let replacement = ForgeSessionAdapter::from_station(
            ForgeSession::new(
                id,
                "iron_sword_v0".to_string(),
                Entity::from_raw(3),
                Entity::from_raw(4),
            ),
            "forge:session:replacement",
            "offline:bob",
            "forge:station:overworld:2:64:2",
        );
        let mut sessions = ForgeSessions::new();

        assert!(sessions.insert_adapter(first));
        assert!(!sessions.insert_adapter(replacement));
        let stored = sessions
            .get(id)
            .expect("first adapter must remain registered");
        assert_eq!(stored.record.owner_key.as_str(), "offline:alice");
        assert_eq!(stored.durable_placed_id(), "forge:station:overworld:1:64:1");
    }
}
