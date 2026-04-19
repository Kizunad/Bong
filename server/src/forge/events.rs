//! plan-forge-v1 §4 Events。

use valence::prelude::{bevy_ecs, Entity, Event};

use super::blueprint::{BlueprintId, TemperBeat};
use super::session::ForgeSessionId;
use crate::cultivation::components::ColorKind;

/// 客户端请求起炉 —— 需 station tier 达标且已学该图。
#[derive(Debug, Clone, Event)]
pub struct StartForgeRequest {
    pub station: Entity,
    pub caster: Entity,
    pub blueprint: BlueprintId,
    pub materials: Vec<(String, u32)>,
}

/// 淬炼按键上报（J=Light, K=Heavy, L=Fold）。
#[derive(Debug, Clone, Event)]
pub struct TemperingHit {
    pub session: ForgeSessionId,
    pub beat: TemperBeat,
    /// 窗口内剩余 ticks（用于 combo 精度），0 = 过窗。
    pub ticks_remaining: u32,
}

/// 铭文残卷投入（每次投一张）。
#[derive(Debug, Clone, Event)]
pub struct InscriptionScrollSubmit {
    pub session: ForgeSessionId,
    pub inscription_id: String,
}

/// 开光真元注入（客户端每 tick 上报注入量）。
#[derive(Debug, Clone, Event)]
pub struct ConsecrationInject {
    pub session: ForgeSessionId,
    pub qi_amount: f64,
}

/// 当前步骤完成，推进到下一步。
#[derive(Debug, Clone, Event)]
pub struct StepAdvance {
    pub session: ForgeSessionId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForgeBucket {
    Perfect,
    Good,
    Flawed,
    Waste,
    Explode,
}

#[derive(Debug, Clone, Event)]
pub struct ForgeOutcomeEvent {
    pub session: ForgeSessionId,
    pub blueprint: BlueprintId,
    pub bucket: ForgeBucket,
    pub weapon_item: Option<String>,
    pub quality: f32,
    pub color: Option<ColorKind>,
    pub side_effects: Vec<String>,
    pub achieved_tier: u8,
}
