//! plan-mineral-v2 P1 — mining session primitives.

use valence::prelude::{BlockPos, Entity};

use super::types::MineralRarity;
use crate::world::dimension::DimensionKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MiningSessionState {
    Running,
    Finished,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MiningSession {
    pub player: Entity,
    pub dimension: DimensionKind,
    pub ore_pos: BlockPos,
    pub started_at_tick: u64,
    pub ticks_total: u32,
    pub elapsed_ticks: u32,
    pub state: MiningSessionState,
    pub origin_position: Option<[f64; 3]>,
    pub tool_instance_id: Option<u64>,
}

impl MiningSession {
    pub fn new(
        player: Entity,
        dimension: DimensionKind,
        ore_pos: BlockPos,
        started_at_tick: u64,
        rarity: MineralRarity,
    ) -> Self {
        Self {
            player,
            dimension,
            ore_pos,
            started_at_tick,
            ticks_total: ticks_total_for_rarity(rarity),
            elapsed_ticks: 0,
            state: MiningSessionState::Running,
            origin_position: None,
            tool_instance_id: None,
        }
    }

    pub fn tick(&mut self) {
        if self.state != MiningSessionState::Running {
            return;
        }
        self.elapsed_ticks = self.elapsed_ticks.saturating_add(1);
        if self.elapsed_ticks >= self.ticks_total {
            self.state = MiningSessionState::Finished;
        }
    }

    pub fn cancel(&mut self) {
        if self.state == MiningSessionState::Running {
            self.state = MiningSessionState::Cancelled;
        }
    }

    pub fn progress_percent(&self) -> u8 {
        if self.ticks_total == 0 {
            return 100;
        }
        ((self.elapsed_ticks as f32 / self.ticks_total as f32) * 100.0)
            .round()
            .clamp(0.0, 100.0) as u8
    }
}

pub fn ticks_total_for_rarity(rarity: MineralRarity) -> u32 {
    match rarity {
        MineralRarity::Fan => 20,
        MineralRarity::Ling => 60,
        MineralRarity::Xi => 120,
        MineralRarity::Yi => 240,
    }
}

#[cfg(test)]
mod tests {
    use super::super::types::MineralRarity;
    use super::*;

    #[test]
    fn mining_session_duration_ladder_matches_plan() {
        assert_eq!(ticks_total_for_rarity(MineralRarity::Fan), 20);
        assert_eq!(ticks_total_for_rarity(MineralRarity::Ling), 60);
        assert_eq!(ticks_total_for_rarity(MineralRarity::Xi), 120);
        assert_eq!(ticks_total_for_rarity(MineralRarity::Yi), 240);
    }

    #[test]
    fn mining_session_progress_and_finish() {
        let mut session = MiningSession::new(
            Entity::from_raw(1),
            DimensionKind::Overworld,
            BlockPos::new(1, 64, 1),
            7,
            MineralRarity::Fan,
        );
        for _ in 0..10 {
            session.tick();
        }
        assert_eq!(session.progress_percent(), 50);
        assert_eq!(session.state, MiningSessionState::Running);
        for _ in 0..10 {
            session.tick();
        }
        assert_eq!(session.state, MiningSessionState::Finished);
        assert_eq!(session.progress_percent(), 100);
    }
}
