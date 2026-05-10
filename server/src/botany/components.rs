use std::collections::HashMap;

use valence::prelude::{bevy_ecs, Component, Entity, Event, Resource};

use super::registry::{BotanyPlantId, FaunaKind, PlantVariant};

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
    pub position: [f64; 3],
    pub planted_at_tick: u64,
    pub wither_progress: u32,
    pub source_point: Option<u64>,
    pub harvested: bool,
    pub trampled: bool,
    pub variant: PlantVariant,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub struct PlantStaticPoint {
    pub id: u64,
    pub zone_name: String,
    pub position: [f64; 3],
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
    pub origin_position: [f64; 3],
}

/// 踩踏概率的可注入骰子。`chance_inverse = 20` ⇒ 1/20 = 5% （plan §1.3）。
/// 测试里可覆盖（`chance_inverse = 1` 强制踩死；`= 0` 永不踩死）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BotanyTrampleRoll {
    pub chance_inverse: u32,
}

impl Default for BotanyTrampleRoll {
    fn default() -> Self {
        Self { chance_inverse: 20 }
    }
}

impl Resource for BotanyTrampleRoll {}

/// plan §7 植物变异概率（仅在 variant-qualifying zone 中掷）。
/// 默认 `chance_inverse = 3` → 1/3 符合条件 zone 产出变种；测试可覆盖为 1 强制 / 0 禁用。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BotanyVariantRoll {
    pub chance_inverse: u32,
}

impl Default for BotanyVariantRoll {
    fn default() -> Self {
        Self { chance_inverse: 3 }
    }
}

impl Resource for BotanyVariantRoll {}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// 记录上一 tick 的"玩家-植物"近邻关系，用于 edge-triggered 踩踏判定：
/// 仅在"本 tick 首次进入近邻范围"时掷骰子，避免玩家停留时每 tick 连掷。
#[derive(Debug, Default)]
pub struct PlantProximityTracker {
    pub in_range: std::collections::HashSet<(Entity, Entity)>,
}

impl Resource for PlantProximityTracker {}

impl Resource for PlantLifecycleClock {}

/// bevy Event：客户端背包 snapshot 需要重推（采集完成 / 其他 inventory 变更）。
/// 生产者 send；消费者 `emit_botany_inventory_snapshots` 在同一 tick 内读取发给 client。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Event)]
pub struct InventorySnapshotRequestEvent {
    pub client_entity: Entity,
}

/// bevy Event：plan §1.3 採集 session 终结（中止 / 完成）。
/// Session 在 enforce 或 tick 完成时已从 store 移除，故生产者在移除**前**发事件，
/// 保证携带植物信息（kind / target_pos / detail）。
#[derive(Debug, Clone, PartialEq, Event)]
pub struct HarvestTerminalEvent {
    pub client_entity: Entity,
    pub session_id: String,
    pub target_id: String,
    pub target_name: String,
    pub plant_kind: String,
    pub mode: BotanyHarvestMode,
    pub interrupted: bool,
    pub completed: bool,
    pub detail: String,
    pub target_pos: Option<[f64; 3]>,
    pub spirit_quality: f32,
}

/// botany-v2 `AttractsMobs` 真 spawn 请求。
///
/// 采集完成时由 harvest 写入，hazard 系统消费并按 fauna 的 `FaunaTag`
/// 生成 Beast NPC。这样采集结算不需要直接知道 layer / NPC bundle 细节。
#[derive(Debug, Clone, PartialEq, Event)]
pub struct BotanyAttractsMobsEvent {
    pub client_entity: Entity,
    pub plant_kind: BotanyPlantId,
    pub zone_name: String,
    pub target_pos: [f64; 3],
    pub mob_kind: FaunaKind,
    pub min_count: u8,
    pub max_count: u8,
    pub issued_at_tick: u64,
}

/// bevy Event：采药技能等级 / XP 变化（仅 add_skill_xp 路径发）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Event)]
pub struct BotanySkillChangedEvent {
    pub client_entity: Entity,
    pub state: BotanySkillState,
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
            origin_position: [0.0, 0.0, 0.0],
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
