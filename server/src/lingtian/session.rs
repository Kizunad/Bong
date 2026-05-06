//! plan-lingtian-v1 §1.2.2 / §1.6 — 开垦 (TillSession) + 翻新 (RenewSession)。
//!
//! 状态机风格仿 alchemy::session：纯 struct + 方法，**不依赖 ECS**；外层
//! 由 `Update` system 驱动 tick + 完成事件。
//!
//! 共用约束：
//!   * 单 plot 同时只能有一种 session（持有方在 ECS 侧加锁）
//!   * tick 计数到 `target_ticks` → `finished = true` → 等结算
//!   * `cancel()` 提前打断，不结算

use serde::{Deserialize, Serialize};
use valence::prelude::BlockPos;

use crate::alchemy::residue::PillResidueKind;
use crate::botany::PlantId;
pub use crate::qi_physics::constants::{
    LINGTIAN_DRAIN_PLAYER_RATIO as DRAIN_QI_TO_PLAYER_RATIO,
    LINGTIAN_DRAIN_ZONE_RATIO as DRAIN_QI_TO_ZONE_RATIO,
};

use super::environment::PlotEnvironment;
use super::hoe::HoeKind;

/// plan §1.2.2 — 手动 2s / 自动 5s。tick = 1/20 s（valence 默认）。
pub const TILL_MANUAL_TICKS: u32 = 40;
pub const TILL_AUTO_TICKS: u32 = 100;

/// plan §1.6 — 翻新 5s。
pub const RENEW_TICKS: u32 = 100;

/// plan §1.2.3 — 种植 1s。
pub const PLANTING_TICKS: u32 = 20;

/// plan §1.5 — 收获 manual 2.5s / auto 7s（与采集浮窗范式同档）。
pub const HARVEST_MANUAL_TICKS: u32 = 50;
pub const HARVEST_AUTO_TICKS: u32 = 140;

/// plan §1.4 + plan-alchemy-recycle-v1 — 补灵 5 来源。
/// 各档 amount / duration 见 [`ReplenishSource`] 方法。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplenishSource {
    /// 区域抽吸：免材料但慢；从 zone qi 扣等量。
    Zone,
    /// 骨币 1 枚 → +0.8。
    BoneCoin,
    /// 异变兽核 1 个 → +2.0（直接拉满）。
    BeastCore,
    /// 灵水 1 瓶 → +0.3。
    LingShui,
    /// 炼丹/炮制废料反哺；具体数值由 residue_kind 决定。
    PillResidue { residue_kind: PillResidueKind },
}

impl ReplenishSource {
    /// plan §1.4 — 一次补灵注入的 plot_qi 量（绝对值，可超 cap，溢出回馈环境）。
    pub fn plot_qi_amount(self) -> f32 {
        match self {
            Self::Zone => 0.5,
            Self::BoneCoin => 0.8,
            Self::BeastCore => 2.0,
            Self::LingShui => 0.3,
            Self::PillResidue { residue_kind } => residue_kind.spec().plot_qi_amount,
        }
    }

    /// session 时长（Bevy tick）：plan §1.4 "2-8s"。Zone 慢 8s = 160 tick；
    /// 其它有材料 2s = 40 tick。
    pub fn duration_ticks(self) -> u32 {
        match self {
            Self::Zone => 160,
            Self::PillResidue { residue_kind } => residue_kind.spec().duration_ticks,
            _ => 40,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ReplenishSession {
    pub pos: BlockPos,
    pub source: ReplenishSource,
    pub elapsed_ticks: u32,
    pub state: SessionState,
}

impl ReplenishSession {
    pub fn new(pos: BlockPos, source: ReplenishSource) -> Self {
        Self {
            pos,
            source,
            elapsed_ticks: 0,
            state: SessionState::Running,
        }
    }

    pub fn target_ticks(&self) -> u32 {
        self.source.duration_ticks()
    }

    pub fn tick(&mut self) {
        if self.state != SessionState::Running {
            return;
        }
        self.elapsed_ticks = self.elapsed_ticks.saturating_add(1);
        if self.elapsed_ticks >= self.target_ticks() {
            self.state = SessionState::Finished;
        }
    }

    pub fn cancel(&mut self) {
        if self.state == SessionState::Running {
            self.state = SessionState::Cancelled;
        }
    }

    pub fn is_finished(&self) -> bool {
        self.state == SessionState::Finished
    }
}

/// plan §1.4 — 同 plot 补灵冷却下限：72h 真实时间 = 72 × 60 = 4320 lingtian-tick。
/// 上限 168h = 10080；本切片用下限固定，未来可加随机扰动。
pub const REPLENISH_COOLDOWN_LINGTIAN_TICKS: u64 = 4320;

/// plan §1.7 — 偷灵 2s（与"普通材料补灵"同档）。
pub const DRAIN_QI_TICKS: u32 = 40;

#[derive(Debug, Clone)]
pub struct DrainQiSession {
    pub pos: BlockPos,
    pub elapsed_ticks: u32,
    pub state: SessionState,
}

impl DrainQiSession {
    pub fn new(pos: BlockPos) -> Self {
        Self {
            pos,
            elapsed_ticks: 0,
            state: SessionState::Running,
        }
    }

    pub fn target_ticks(&self) -> u32 {
        DRAIN_QI_TICKS
    }

    pub fn tick(&mut self) {
        if self.state != SessionState::Running {
            return;
        }
        self.elapsed_ticks = self.elapsed_ticks.saturating_add(1);
        if self.elapsed_ticks >= DRAIN_QI_TICKS {
            self.state = SessionState::Finished;
        }
    }

    pub fn cancel(&mut self) {
        if self.state == SessionState::Running {
            self.state = SessionState::Cancelled;
        }
    }

    pub fn is_finished(&self) -> bool {
        self.state == SessionState::Finished
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionMode {
    Manual,
    /// 自动模式（plan §1.2.2：herbalism Lv.3+ 解锁）。session 层不做权限校验，
    /// 由调用方在起 session 前确认。
    Auto,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionState {
    Running,
    Finished,
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct TillSession {
    pub pos: BlockPos,
    pub hoe: HoeKind,
    /// 会话开始时锁定的具体锄头 `ItemInstance.instance_id`；apply 路径按此扣耐久。
    pub hoe_instance_id: u64,
    pub mode: SessionMode,
    /// 起手时锁定的环境修饰（决定 plot_qi_cap）。
    pub environment: PlotEnvironment,
    pub elapsed_ticks: u32,
    pub state: SessionState,
}

impl TillSession {
    pub fn new(
        pos: BlockPos,
        hoe: HoeKind,
        hoe_instance_id: u64,
        mode: SessionMode,
        environment: PlotEnvironment,
    ) -> Self {
        Self {
            pos,
            hoe,
            hoe_instance_id,
            mode,
            environment,
            elapsed_ticks: 0,
            state: SessionState::Running,
        }
    }

    pub fn target_ticks(&self) -> u32 {
        match self.mode {
            SessionMode::Manual => TILL_MANUAL_TICKS,
            SessionMode::Auto => TILL_AUTO_TICKS,
        }
    }

    /// 推进一 tick；到时机就 mark Finished。`Finished` / `Cancelled` 之后调用 no-op。
    pub fn tick(&mut self) {
        if self.state != SessionState::Running {
            return;
        }
        self.elapsed_ticks = self.elapsed_ticks.saturating_add(1);
        if self.elapsed_ticks >= self.target_ticks() {
            self.state = SessionState::Finished;
        }
    }

    pub fn cancel(&mut self) {
        if self.state == SessionState::Running {
            self.state = SessionState::Cancelled;
        }
    }

    pub fn is_finished(&self) -> bool {
        self.state == SessionState::Finished
    }
}

#[derive(Debug, Clone)]
pub struct RenewSession {
    pub pos: BlockPos,
    pub hoe: HoeKind,
    pub hoe_instance_id: u64,
    pub elapsed_ticks: u32,
    pub state: SessionState,
}

impl RenewSession {
    pub fn new(pos: BlockPos, hoe: HoeKind, hoe_instance_id: u64) -> Self {
        Self {
            pos,
            hoe,
            hoe_instance_id,
            elapsed_ticks: 0,
            state: SessionState::Running,
        }
    }

    pub fn target_ticks(&self) -> u32 {
        RENEW_TICKS
    }

    pub fn tick(&mut self) {
        if self.state != SessionState::Running {
            return;
        }
        self.elapsed_ticks = self.elapsed_ticks.saturating_add(1);
        if self.elapsed_ticks >= RENEW_TICKS {
            self.state = SessionState::Finished;
        }
    }

    pub fn cancel(&mut self) {
        if self.state == SessionState::Running {
            self.state = SessionState::Cancelled;
        }
    }

    pub fn is_finished(&self) -> bool {
        self.state == SessionState::Finished
    }
}

#[derive(Debug, Clone)]
pub struct HarvestSession {
    pub pos: BlockPos,
    pub plant_id: PlantId,
    pub mode: SessionMode,
    pub elapsed_ticks: u32,
    pub state: SessionState,
}

impl HarvestSession {
    pub fn new(pos: BlockPos, plant_id: PlantId, mode: SessionMode) -> Self {
        Self {
            pos,
            plant_id,
            mode,
            elapsed_ticks: 0,
            state: SessionState::Running,
        }
    }

    pub fn target_ticks(&self) -> u32 {
        match self.mode {
            SessionMode::Manual => HARVEST_MANUAL_TICKS,
            SessionMode::Auto => HARVEST_AUTO_TICKS,
        }
    }

    pub fn tick(&mut self) {
        if self.state != SessionState::Running {
            return;
        }
        self.elapsed_ticks = self.elapsed_ticks.saturating_add(1);
        if self.elapsed_ticks >= self.target_ticks() {
            self.state = SessionState::Finished;
        }
    }

    pub fn cancel(&mut self) {
        if self.state == SessionState::Running {
            self.state = SessionState::Cancelled;
        }
    }

    pub fn is_finished(&self) -> bool {
        self.state == SessionState::Finished
    }
}

#[derive(Debug, Clone)]
pub struct PlantingSession {
    pub pos: BlockPos,
    pub plant_id: PlantId,
    pub elapsed_ticks: u32,
    pub state: SessionState,
}

impl PlantingSession {
    pub fn new(pos: BlockPos, plant_id: PlantId) -> Self {
        Self {
            pos,
            plant_id,
            elapsed_ticks: 0,
            state: SessionState::Running,
        }
    }

    pub fn target_ticks(&self) -> u32 {
        PLANTING_TICKS
    }

    pub fn tick(&mut self) {
        if self.state != SessionState::Running {
            return;
        }
        self.elapsed_ticks = self.elapsed_ticks.saturating_add(1);
        if self.elapsed_ticks >= PLANTING_TICKS {
            self.state = SessionState::Finished;
        }
    }

    pub fn cancel(&mut self) {
        if self.state == SessionState::Running {
            self.state = SessionState::Cancelled;
        }
    }

    pub fn is_finished(&self) -> bool {
        self.state == SessionState::Finished
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pos() -> BlockPos {
        BlockPos::new(1, 64, 1)
    }

    #[test]
    fn till_manual_finishes_at_40_ticks() {
        let mut s = TillSession::new(
            pos(),
            HoeKind::Iron,
            1,
            SessionMode::Manual,
            PlotEnvironment::base(),
        );
        for _ in 0..TILL_MANUAL_TICKS - 1 {
            s.tick();
            assert!(!s.is_finished());
        }
        s.tick();
        assert!(s.is_finished());
        assert_eq!(s.elapsed_ticks, TILL_MANUAL_TICKS);
    }

    #[test]
    fn till_auto_takes_longer_than_manual() {
        let manual = TillSession::new(
            pos(),
            HoeKind::Iron,
            1,
            SessionMode::Manual,
            PlotEnvironment::base(),
        )
        .target_ticks();
        let auto = TillSession::new(
            pos(),
            HoeKind::Iron,
            1,
            SessionMode::Auto,
            PlotEnvironment::base(),
        )
        .target_ticks();
        assert!(auto > manual);
    }

    #[test]
    fn till_tick_after_finish_is_noop() {
        let mut s = TillSession::new(
            pos(),
            HoeKind::Iron,
            1,
            SessionMode::Manual,
            PlotEnvironment::base(),
        );
        for _ in 0..TILL_MANUAL_TICKS {
            s.tick();
        }
        assert!(s.is_finished());
        let frozen = s.elapsed_ticks;
        s.tick();
        assert_eq!(s.elapsed_ticks, frozen);
    }

    #[test]
    fn till_cancel_blocks_finish() {
        let mut s = TillSession::new(
            pos(),
            HoeKind::Iron,
            1,
            SessionMode::Manual,
            PlotEnvironment::base(),
        );
        s.tick();
        s.cancel();
        for _ in 0..200 {
            s.tick();
        }
        assert!(!s.is_finished());
        assert_eq!(s.state, SessionState::Cancelled);
    }

    #[test]
    fn renew_finishes_at_100_ticks() {
        let mut s = RenewSession::new(pos(), HoeKind::Xuantie, 1);
        for _ in 0..RENEW_TICKS - 1 {
            s.tick();
            assert!(!s.is_finished());
        }
        s.tick();
        assert!(s.is_finished());
    }

    #[test]
    fn renew_cancel_blocks_finish() {
        let mut s = RenewSession::new(pos(), HoeKind::Iron, 1);
        s.cancel();
        for _ in 0..200 {
            s.tick();
        }
        assert!(!s.is_finished());
    }

    #[test]
    fn planting_finishes_at_20_ticks() {
        let mut s = PlantingSession::new(pos(), "ci_she_hao".into());
        for _ in 0..PLANTING_TICKS - 1 {
            s.tick();
            assert!(!s.is_finished());
        }
        s.tick();
        assert!(s.is_finished());
    }

    #[test]
    fn planting_cancel_blocks_finish() {
        let mut s = PlantingSession::new(pos(), "ci_she_hao".into());
        s.tick();
        s.cancel();
        for _ in 0..200 {
            s.tick();
        }
        assert!(!s.is_finished());
        assert_eq!(s.state, SessionState::Cancelled);
    }

    #[test]
    fn harvest_manual_finishes_at_50_ticks() {
        let mut s = HarvestSession::new(pos(), "ci_she_hao".into(), SessionMode::Manual);
        for _ in 0..HARVEST_MANUAL_TICKS - 1 {
            s.tick();
            assert!(!s.is_finished());
        }
        s.tick();
        assert!(s.is_finished());
    }

    #[test]
    fn harvest_auto_takes_longer_than_manual() {
        let m = HarvestSession::new(pos(), "ci_she_hao".into(), SessionMode::Manual).target_ticks();
        let a = HarvestSession::new(pos(), "ci_she_hao".into(), SessionMode::Auto).target_ticks();
        assert!(a > m);
    }

    #[test]
    fn harvest_cancel_blocks_finish() {
        let mut s = HarvestSession::new(pos(), "ci_she_hao".into(), SessionMode::Manual);
        s.cancel();
        for _ in 0..500 {
            s.tick();
        }
        assert!(!s.is_finished());
    }

    #[test]
    fn replenish_durations_match_plan() {
        assert_eq!(ReplenishSource::Zone.duration_ticks(), 160);
        for s in [
            ReplenishSource::BoneCoin,
            ReplenishSource::BeastCore,
            ReplenishSource::LingShui,
        ] {
            assert_eq!(s.duration_ticks(), 40);
        }
        assert_eq!(
            ReplenishSource::PillResidue {
                residue_kind: PillResidueKind::FailedPill,
            }
            .duration_ticks(),
            100
        );
    }

    #[test]
    fn replenish_amounts_match_plan() {
        assert_eq!(ReplenishSource::Zone.plot_qi_amount(), 0.5);
        assert_eq!(ReplenishSource::BoneCoin.plot_qi_amount(), 0.8);
        assert_eq!(ReplenishSource::BeastCore.plot_qi_amount(), 2.0);
        assert_eq!(ReplenishSource::LingShui.plot_qi_amount(), 0.3);
        assert_eq!(
            ReplenishSource::PillResidue {
                residue_kind: PillResidueKind::FailedPill,
            }
            .plot_qi_amount(),
            0.4
        );
    }

    #[test]
    fn replenish_session_ticks_to_finish() {
        let mut s = ReplenishSession::new(pos(), ReplenishSource::BoneCoin);
        for _ in 0..40 - 1 {
            s.tick();
            assert!(!s.is_finished());
        }
        s.tick();
        assert!(s.is_finished());
    }
}
