use std::collections::HashMap;

use valence::prelude::{bevy_ecs, Component, Entity, Resource};

use super::registry::BotanyPlantId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotanyHarvestMode {
    Manual,
    Auto,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotanyPhase {
    Pending,
    InProgress,
    Completed,
    Interrupted,
    Trampled,
}

#[derive(Debug, Clone, Component, PartialEq)]
pub struct Plant {
    pub id: BotanyPlantId,
    pub zone_name: String,
    pub planted_at_tick: u64,
    pub wither_progress: u32,
    pub source_point: Option<u64>,
    pub harvested: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub struct PlantStaticPoint {
    pub id: u64,
    pub zone_name: String,
    pub preferred_plant: BotanyPlantId,
    pub last_spawn_tick: Option<u64>,
    pub regen_ticks: u64,
    pub bound_entity: Option<Entity>,
}

#[derive(Debug, Default)]
pub struct PlantStaticPointStore {
    initialized: bool,
    points_by_id: HashMap<u64, PlantStaticPoint>,
}

impl Resource for PlantStaticPointStore {}

impl PlantStaticPointStore {
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    pub fn mark_initialized(&mut self) {
        self.initialized = true;
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.points_by_id.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.points_by_id.is_empty()
    }

    pub fn upsert(&mut self, point: PlantStaticPoint) {
        self.points_by_id.insert(point.id, point);
    }

    pub fn get_mut(&mut self, id: u64) -> Option<&mut PlantStaticPoint> {
        self.points_by_id.get_mut(&id)
    }

    #[allow(dead_code)]
    pub fn iter(&self) -> impl Iterator<Item = &PlantStaticPoint> {
        self.points_by_id.values()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut PlantStaticPoint> {
        self.points_by_id.values_mut()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct HarvestSession {
    pub player_id: String,
    pub client_entity: Entity,
    pub target_entity: Option<Entity>,
    pub target_plant: BotanyPlantId,
    pub mode: BotanyHarvestMode,
    pub started_at_tick: u64,
    pub duration_ticks: u64,
    pub phase: BotanyPhase,
    pub last_progress: f32,
}

impl HarvestSession {
    pub fn progress_at(&self, tick: u64) -> f32 {
        if self.duration_ticks == 0 {
            return 1.0;
        }

        let elapsed = tick.saturating_sub(self.started_at_tick);
        (elapsed as f32 / self.duration_ticks as f32).clamp(0.0, 1.0)
    }

    #[allow(dead_code)]
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.phase,
            BotanyPhase::Completed | BotanyPhase::Interrupted | BotanyPhase::Trampled
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BotanySkillState {
    pub level: u8,
    pub xp: u64,
    pub auto_unlock_level: u8,
}

impl Default for BotanySkillState {
    fn default() -> Self {
        Self {
            level: 1,
            xp: 0,
            auto_unlock_level: 3,
        }
    }
}

#[derive(Debug, Default)]
pub struct HarvestSessionStore {
    sessions_by_player: HashMap<String, HarvestSession>,
    skills_by_player: HashMap<String, BotanySkillState>,
}

impl Resource for HarvestSessionStore {}

impl HarvestSessionStore {
    pub fn session_for(&self, player_id: &str) -> Option<&HarvestSession> {
        self.sessions_by_player.get(player_id)
    }

    #[allow(dead_code)]
    pub fn session_for_mut(&mut self, player_id: &str) -> Option<&mut HarvestSession> {
        self.sessions_by_player.get_mut(player_id)
    }

    pub fn upsert_session(&mut self, session: HarvestSession) {
        self.sessions_by_player
            .insert(session.player_id.clone(), session);
    }

    pub fn remove_session(&mut self, player_id: &str) -> Option<HarvestSession> {
        self.sessions_by_player.remove(player_id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &HarvestSession> {
        self.sessions_by_player.values()
    }

    pub fn skill_for(&self, player_id: &str) -> BotanySkillState {
        self.skills_by_player
            .get(player_id)
            .copied()
            .unwrap_or_default()
    }

    pub fn add_skill_xp(&mut self, player_id: &str, delta: u64) -> BotanySkillState {
        let mut next = self.skill_for(player_id);
        next.xp = next.xp.saturating_add(delta);

        let next_level = ((next.xp / 100).min(u64::from(u8::MAX - 1)) as u8).saturating_add(1);
        next.level = next_level.max(1);

        self.skills_by_player.insert(player_id.to_string(), next);
        next
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlantLifecycleClock {
    pub tick: u64,
}

impl Resource for PlantLifecycleClock {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InventorySnapshotPushTarget {
    pub client_entity: Entity,
}

#[derive(Debug, Default)]
pub struct InventorySnapshotPushQueue {
    pending: Vec<InventorySnapshotPushTarget>,
}

impl Resource for InventorySnapshotPushQueue {}

impl InventorySnapshotPushQueue {
    pub fn enqueue(&mut self, client_entity: Entity) {
        self.pending
            .push(InventorySnapshotPushTarget { client_entity });
    }

    pub fn drain(&mut self) -> Vec<InventorySnapshotPushTarget> {
        std::mem::take(&mut self.pending)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn harvest_session_progress_clamps() {
        let session = HarvestSession {
            player_id: "offline:Azure".to_string(),
            client_entity: Entity::from_raw(1),
            target_entity: Some(Entity::from_raw(2)),
            target_plant: BotanyPlantId::CiSheHao,
            mode: BotanyHarvestMode::Manual,
            started_at_tick: 10,
            duration_ticks: 20,
            phase: BotanyPhase::InProgress,
            last_progress: 0.0,
        };

        assert_eq!(session.progress_at(0), 0.0);
        assert!((session.progress_at(20) - 0.5).abs() < f32::EPSILON);
        assert_eq!(session.progress_at(35), 1.0);
    }

    #[test]
    fn skill_progression_unlocks_auto_level() {
        let mut store = HarvestSessionStore::default();
        let state = store.add_skill_xp("offline:Azure", 250);
        assert_eq!(state.level, 3);
        assert_eq!(state.auto_unlock_level, 3);
    }
}
