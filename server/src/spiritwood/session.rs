use std::collections::{HashMap, HashSet};

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
    settling: HashSet<Entity>,
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
        self.settling.remove(&player);
        self.sessions.remove(&player)
    }

    pub fn claim_for_settlement(&mut self, player: Entity) -> Option<WoodSession> {
        let session = self.sessions.get(&player)?.clone();
        self.settling.insert(player).then_some(session)
    }

    pub fn finish_settlement(&mut self, expected: &WoodSession) -> Option<WoodSession> {
        if !self.settling.remove(&expected.player)
            || self.sessions.get(&expected.player) != Some(expected)
        {
            return None;
        }
        self.sessions.remove(&expected.player)
    }

    pub fn is_settling(&self, player: Entity) -> bool {
        self.settling.contains(&player)
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

    #[test]
    fn settlement_claim_is_single_consumer_and_keeps_session_visible() {
        let player = Entity::from_raw(8);
        let session = WoodSession::new(
            player,
            "offline:settling".to_string(),
            DimensionKind::Overworld,
            BlockPos::new(2, 80, 2),
            100,
            [2.0, 80.0, 2.0],
            Some(10),
        );
        let mut store = WoodSessionStore::default();
        store.upsert(session.clone());

        assert_eq!(store.claim_for_settlement(player), Some(session.clone()));
        assert_eq!(store.session_for(player), Some(&session));
        assert!(store.is_settling(player));
        assert_eq!(store.claim_for_settlement(player), None);
        assert_eq!(store.finish_settlement(&session), Some(session));
        assert!(store.session_for(player).is_none());
        assert!(!store.is_settling(player));
    }

    #[test]
    fn finishing_old_claim_does_not_remove_replacement_session() {
        let player = Entity::from_raw(9);
        let old = WoodSession::new(
            player,
            "offline:old".to_string(),
            DimensionKind::Overworld,
            BlockPos::new(3, 80, 3),
            100,
            [3.0, 80.0, 3.0],
            Some(11),
        );
        let replacement = WoodSession::new(
            player,
            "offline:replacement".to_string(),
            DimensionKind::Overworld,
            BlockPos::new(4, 80, 4),
            200,
            [4.0, 80.0, 4.0],
            Some(12),
        );
        let mut store = WoodSessionStore::default();
        store.upsert(old.clone());
        assert_eq!(store.claim_for_settlement(player), Some(old.clone()));

        store.upsert(replacement.clone());
        assert_eq!(store.finish_settlement(&old), None);
        assert_eq!(store.session_for(player), Some(&replacement));
        assert!(!store.is_settling(player));
    }
}
