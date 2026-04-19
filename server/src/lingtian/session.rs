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

use super::hoe::HoeKind;

/// plan §1.2.2 — 手动 2s / 自动 5s。tick = 1/20 s（valence 默认）。
pub const TILL_MANUAL_TICKS: u32 = 40;
pub const TILL_AUTO_TICKS: u32 = 100;

/// plan §1.6 — 翻新 5s。
pub const RENEW_TICKS: u32 = 100;

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
    pub mode: SessionMode,
    pub elapsed_ticks: u32,
    pub state: SessionState,
}

impl TillSession {
    pub fn new(pos: BlockPos, hoe: HoeKind, mode: SessionMode) -> Self {
        Self {
            pos,
            hoe,
            mode,
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
    pub elapsed_ticks: u32,
    pub state: SessionState,
}

impl RenewSession {
    pub fn new(pos: BlockPos, hoe: HoeKind) -> Self {
        Self {
            pos,
            hoe,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn pos() -> BlockPos {
        BlockPos::new(1, 64, 1)
    }

    #[test]
    fn till_manual_finishes_at_40_ticks() {
        let mut s = TillSession::new(pos(), HoeKind::Iron, SessionMode::Manual);
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
        let manual = TillSession::new(pos(), HoeKind::Iron, SessionMode::Manual).target_ticks();
        let auto = TillSession::new(pos(), HoeKind::Iron, SessionMode::Auto).target_ticks();
        assert!(auto > manual);
    }

    #[test]
    fn till_tick_after_finish_is_noop() {
        let mut s = TillSession::new(pos(), HoeKind::Iron, SessionMode::Manual);
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
        let mut s = TillSession::new(pos(), HoeKind::Iron, SessionMode::Manual);
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
        let mut s = RenewSession::new(pos(), HoeKind::Xuantie);
        for _ in 0..RENEW_TICKS - 1 {
            s.tick();
            assert!(!s.is_finished());
        }
        s.tick();
        assert!(s.is_finished());
    }

    #[test]
    fn renew_cancel_blocks_finish() {
        let mut s = RenewSession::new(pos(), HoeKind::Iron);
        s.cancel();
        for _ in 0..200 {
            s.tick();
        }
        assert!(!s.is_finished());
    }
}
