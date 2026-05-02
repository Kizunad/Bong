use std::collections::HashMap;

use valence::prelude::{BlockPos, Entity, Resource};

use crate::world::dimension::DimensionKind;

pub const WOOD_SESSION_TICKS_TOTAL: u64 = 240;
pub const MOVEMENT_BREAK_DISTANCE_SQ: f64 = 1.5 * 1.5;

#[derive(Debug, Clone, PartialEq)]
pub struct WoodSession {
    pub player: Entity,
    pub player_id: String,
    pub dimension: DimensionKind,
    pub log_pos: BlockPos,
    pub started_at_tick: u64,
    pub ticks_total: u64,
    pub origin_position: [f64; 3],
    pub tool_instance_id: Option<u64>,
}

impl WoodSession {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        player: Entity,
        player_id: String,
        dimension: DimensionKind,
        log_pos: BlockPos,
        started_at_tick: u64,
        origin_position: [f64; 3],
        tool_instance_id: Option<u64>,
    ) -> Self {
        Self {
            player,
            player_id,
            dimension,
            log_pos,
            started_at_tick,
            ticks_total: WOOD_SESSION_TICKS_TOTAL,
            origin_position,
            tool_instance_id,
        }
    }

    pub fn progress_at(&self, tick: u64) -> f64 {
        if self.ticks_total == 0 {
            return 1.0;
        }
        let elapsed = tick.saturating_sub(self.started_at_tick);
        (elapsed as f64 / self.ticks_total as f64).clamp(0.0, 1.0)
    }

    pub fn completed_at(&self, tick: u64) -> bool {
        self.progress_at(tick) >= 1.0
    }
}

#[derive(Debug, Default)]
pub struct WoodSessionStore {
    sessions: HashMap<Entity, WoodSession>,
}

impl Resource for WoodSessionStore {}

impl WoodSessionStore {
    pub fn session_for(&self, player: Entity) -> Option<&WoodSession> {
        self.sessions.get(&player)
    }

    pub fn has_session_at(&self, dimension: DimensionKind, log_pos: BlockPos) -> bool {
        self.sessions
            .values()
            .any(|session| session.dimension == dimension && session.log_pos == log_pos)
    }

    pub fn upsert(&mut self, session: WoodSession) {
        self.sessions.insert(session.player, session);
    }

    pub fn remove(&mut self, player: Entity) -> Option<WoodSession> {
        self.sessions.remove(&player)
    }

    pub fn iter(&self) -> impl Iterator<Item = &WoodSession> {
        self.sessions.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wood_session_progress_clamps() {
        let session = WoodSession::new(
            Entity::from_raw(7),
            "offline:kiz".to_string(),
            DimensionKind::Overworld,
            BlockPos::new(1, 80, 2),
            100,
            [0.0, 64.0, 0.0],
            Some(9),
        );

        assert_eq!(session.progress_at(99), 0.0);
        assert_eq!(session.progress_at(100), 0.0);
        assert!((session.progress_at(220) - 0.5).abs() < f64::EPSILON);
        assert_eq!(session.progress_at(999), 1.0);
    }

    #[test]
    fn store_blocks_duplicate_session_for_same_log() {
        let mut store = WoodSessionStore::default();
        let log_pos = BlockPos::new(1, 80, 2);
        store.upsert(WoodSession::new(
            Entity::from_raw(7),
            "offline:a".to_string(),
            DimensionKind::Overworld,
            log_pos,
            100,
            [0.0, 64.0, 0.0],
            Some(9),
        ));

        assert!(store.has_session_at(DimensionKind::Overworld, log_pos));
        assert!(!store.has_session_at(DimensionKind::Overworld, BlockPos::new(2, 80, 2)));
        assert!(!store.has_session_at(DimensionKind::Tsy, log_pos));
    }
}
