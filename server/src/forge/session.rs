//! plan-forge-v1 §1.3 四步进程 Session 状态机。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use valence::prelude::{Entity, Resource};

use super::blueprint::{BlueprintId, StepKind};
use crate::cultivation::components::ColorKind;

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
#[derive(Debug, Clone)]
pub enum StepState {
    Billet(BilletState),
    Tempering(TemperingState),
    Inscription(InscriptionState),
    Consecration(ConsecrationState),
    None,
}

#[derive(Debug, Clone, Default)]
pub struct BilletState {
    /// 投入物料：material -> count。
    pub materials_in: HashMap<String, u32>,
    /// 已确认的载体 material（决定 tier_cap）。
    pub active_carrier: Option<String>,
    pub resolved_tier_cap: u8,
}

#[derive(Debug, Clone, Default)]
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

#[derive(Debug, Clone, Default)]
pub struct InscriptionState {
    pub scrolls_in: Vec<String>,
    pub filled_slots: u8,
    pub failed: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ConsecrationState {
    pub qi_injected: f64,
    pub qi_required: f64,
    pub color_imprint: Option<ColorKind>,
}

#[derive(Debug, Clone)]
pub struct ForgeSession {
    pub id: ForgeSessionId,
    pub blueprint: BlueprintId,
    pub station: Entity,
    pub caster: Entity,
    /// 当前步骤在图谱 steps[] 中的 index。
    pub step_index: usize,
    pub current_step: ForgeStep,
    pub step_state: StepState,
    /// 本次会话总偏差（跨步累积，决定最终 bucket）。
    pub total_deviation: u32,
    /// 任一 step 是否已走 flawed 路径。
    pub flawed_marker: bool,
    /// 已锁定不可返还的材料（投入即消耗）。
    pub committed_materials: HashMap<String, u32>,
    /// 最终达成的 tier（坯料后刷新，后续可跳步下调）。
    pub achieved_tier: u8,
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
            caster,
            step_index: 0,
            current_step: ForgeStep::Billet,
            step_state: StepState::None,
            total_deviation: 0,
            flawed_marker: false,
            committed_materials: HashMap::new(),
            achieved_tier: 0,
        }
    }

    pub fn is_done(&self) -> bool {
        self.current_step == ForgeStep::Done
    }
}

/// 所有在炉 session 的总表。ForgeSessionId → ForgeSession。
#[derive(Debug, Default)]
pub struct ForgeSessions {
    next_id: u64,
    sessions: HashMap<ForgeSessionId, ForgeSession>,
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

    pub fn insert(&mut self, session: ForgeSession) {
        self.sessions.insert(session.id, session);
    }

    pub fn get(&self, id: ForgeSessionId) -> Option<&ForgeSession> {
        self.sessions.get(&id)
    }

    pub fn get_mut(&mut self, id: ForgeSessionId) -> Option<&mut ForgeSession> {
        self.sessions.get_mut(&id)
    }

    pub fn remove(&mut self, id: ForgeSessionId) -> Option<ForgeSession> {
        self.sessions.remove(&id)
    }

    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }
}
