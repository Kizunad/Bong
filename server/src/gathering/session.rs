use std::collections::{HashMap, HashSet};

use valence::prelude::{
    bevy_ecs, Component, Entity, Event, EventReader, EventWriter, Position, Query, Res, ResMut,
    Resource, With,
};

use super::quality::{quality_hint, roll_quality, GatheringQuality};
use super::tools::{
    damage_equipped_gathering_tool, gather_time_ticks, spec_for_item_id, GatheringMaterial,
    GatheringTargetKind, GatheringToolSpec,
};
use crate::combat::events::CombatEvent;
use crate::cultivation::components::Realm;
use crate::inventory::{InventoryDurabilityChangedEvent, PlayerInventory};
use crate::player::gameplay::GameplayTick;

pub const MOVEMENT_BREAK_DISTANCE_SQ: f64 = 0.3 * 0.3;
pub const PROGRESS_SYNC_INTERVAL_TICKS: u64 = 10;

#[derive(Debug, Clone, Component, PartialEq, Eq)]
pub struct Gatherable {
    pub target: GatheringTargetKind,
    pub base_time_ticks: u64,
    pub loot_table: String,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GatheringSession {
    pub player: Entity,
    pub session_id: String,
    pub target: GatheringTargetKind,
    pub target_name: String,
    pub started_at_tick: u64,
    pub total_ticks: u64,
    pub origin_position: [f64; 3],
    pub tool: Option<GatheringToolSpec>,
    pub realm: Realm,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GatheringSessionStart {
    pub player: Entity,
    pub session_id: String,
    pub target: GatheringTargetKind,
    pub target_name: String,
    pub started_at_tick: u64,
    pub origin_position: [f64; 3],
    pub tool: Option<GatheringToolSpec>,
    pub realm: Realm,
}

#[derive(Debug, Default)]
pub struct GatheringSessionStore {
    sessions: HashMap<Entity, GatheringSession>,
}

impl Resource for GatheringSessionStore {}

#[derive(Debug, Clone, PartialEq, Event)]
pub struct GatheringProgressFrame {
    pub player: Entity,
    pub session_id: String,
    pub origin_position: [f64; 3],
    pub progress_ticks: u64,
    pub total_ticks: u64,
    pub target_name: String,
    pub target_type: GatheringTargetKind,
    pub quality_hint: String,
    pub tool_used: Option<String>,
    pub interrupted: bool,
    pub completed: bool,
}

#[derive(Debug, Clone, PartialEq, Event)]
pub struct GatheringCompleteEvent {
    pub player: Entity,
    pub session_id: String,
    pub origin_position: [f64; 3],
    pub target_name: String,
    pub target_type: GatheringTargetKind,
    pub quality: GatheringQuality,
    pub tool_used: Option<String>,
}

impl GatheringSession {
    pub fn new(start: GatheringSessionStart) -> Self {
        Self {
            player: start.player,
            session_id: start.session_id,
            target: start.target,
            target_name: start.target_name,
            started_at_tick: start.started_at_tick,
            total_ticks: gather_time_ticks(start.target, start.tool, start.realm),
            origin_position: start.origin_position,
            tool: start.tool,
            realm: start.realm,
        }
    }

    pub fn progress_ticks_at(&self, now_tick: u64) -> u64 {
        now_tick
            .saturating_sub(self.started_at_tick)
            .min(self.total_ticks)
    }

    pub fn progress_ratio_at(&self, now_tick: u64) -> f64 {
        if self.total_ticks == 0 {
            return 1.0;
        }
        self.progress_ticks_at(now_tick) as f64 / self.total_ticks as f64
    }

    pub fn completed_at(&self, now_tick: u64) -> bool {
        self.progress_ticks_at(now_tick) >= self.total_ticks
    }

    pub fn progress_frame(
        &self,
        now_tick: u64,
        interrupted: bool,
        completed: bool,
    ) -> GatheringProgressFrame {
        GatheringProgressFrame {
            player: self.player,
            session_id: self.session_id.clone(),
            origin_position: self.origin_position,
            progress_ticks: if interrupted {
                0
            } else {
                self.progress_ticks_at(now_tick)
            },
            total_ticks: self.total_ticks,
            target_name: self.target_name.clone(),
            target_type: self.target,
            quality_hint: quality_hint(self.tool.map(|tool| tool.material), self.realm).to_string(),
            tool_used: self.tool.map(|tool| tool.item_id.to_string()),
            interrupted,
            completed,
        }
    }

    pub fn completion_event(&self, now_tick: u64) -> GatheringCompleteEvent {
        let seed = now_tick
            ^ self.player.to_bits().wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ self.total_ticks.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        let material: Option<GatheringMaterial> = self.tool.map(|tool| tool.material);
        GatheringCompleteEvent {
            player: self.player,
            session_id: self.session_id.clone(),
            origin_position: self.origin_position,
            target_name: self.target_name.clone(),
            target_type: self.target,
            quality: roll_quality(seed, material, self.realm),
            tool_used: self.tool.map(|tool| tool.item_id.to_string()),
        }
    }

    pub fn moved_too_far(&self, position: &Position) -> bool {
        let current = position.get();
        let dx = current.x - self.origin_position[0];
        let dy = current.y - self.origin_position[1];
        let dz = current.z - self.origin_position[2];
        dx * dx + dy * dy + dz * dz > MOVEMENT_BREAK_DISTANCE_SQ
    }
}

pub fn apply_gathering_tool_durability(
    mut completions: EventReader<GatheringCompleteEvent>,
    mut inventories: Query<&mut PlayerInventory, With<valence::prelude::Client>>,
    mut durability_events: EventWriter<InventoryDurabilityChangedEvent>,
) {
    for completion in completions.read() {
        let Some(tool_id) = completion.tool_used.as_deref() else {
            continue;
        };
        let Some(spec) = spec_for_item_id(tool_id) else {
            continue;
        };
        let Ok(mut inventory) = inventories.get_mut(completion.player) else {
            continue;
        };
        let _ = damage_equipped_gathering_tool(
            completion.player,
            &mut inventory,
            spec,
            &mut durability_events,
        );
    }
}

impl GatheringSessionStore {
    pub fn upsert(&mut self, session: GatheringSession) {
        self.sessions.insert(session.player, session);
    }

    pub fn remove(&mut self, player: Entity) -> Option<GatheringSession> {
        self.sessions.remove(&player)
    }

    pub fn session_for(&self, player: Entity) -> Option<&GatheringSession> {
        self.sessions.get(&player)
    }

    pub fn iter(&self) -> impl Iterator<Item = &GatheringSession> {
        self.sessions.values()
    }

    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }
}

pub fn enforce_gathering_session_constraints(
    gameplay_tick: Option<Res<GameplayTick>>,
    mut store: ResMut<GatheringSessionStore>,
    positions: Query<&Position, With<valence::prelude::Client>>,
    mut combat_events: EventReader<CombatEvent>,
    mut progress_events: EventWriter<GatheringProgressFrame>,
) {
    let now_tick = gameplay_tick.map(|tick| tick.current_tick()).unwrap_or(0);
    let hit_entities = combat_events
        .read()
        .map(|event| event.target)
        .collect::<HashSet<_>>();

    let interrupted = store
        .iter()
        .filter(|session| {
            hit_entities.contains(&session.player)
                || positions
                    .get(session.player)
                    .map(|position| session.moved_too_far(position))
                    .unwrap_or(false)
        })
        .map(|session| session.player)
        .collect::<Vec<_>>();

    for player in interrupted {
        if let Some(session) = store.remove(player) {
            progress_events.send(session.progress_frame(now_tick, true, false));
        }
    }
}

pub fn tick_gathering_sessions(
    gameplay_tick: Option<Res<GameplayTick>>,
    mut store: ResMut<GatheringSessionStore>,
    mut progress_events: EventWriter<GatheringProgressFrame>,
    mut complete_events: EventWriter<GatheringCompleteEvent>,
) {
    let now_tick = gameplay_tick.map(|tick| tick.current_tick()).unwrap_or(0);
    let completed = store
        .iter()
        .filter(|session| session.completed_at(now_tick))
        .map(|session| session.player)
        .collect::<Vec<_>>();

    for player in completed {
        if let Some(session) = store.remove(player) {
            progress_events.send(session.progress_frame(now_tick, false, true));
            complete_events.send(session.completion_event(now_tick));
        }
    }

    if now_tick % PROGRESS_SYNC_INTERVAL_TICKS != 0 {
        return;
    }
    for session in store.iter() {
        progress_events.send(session.progress_frame(now_tick, false, false));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gathering::tools::spec_for_item_id;
    use valence::prelude::Position;

    fn test_session(
        target: GatheringTargetKind,
        started_at_tick: u64,
        tool_id: Option<&str>,
        realm: Realm,
    ) -> GatheringSession {
        GatheringSession::new(GatheringSessionStart {
            player: Entity::from_raw(1),
            session_id: "s1".to_string(),
            target,
            target_name: "测试采集物".to_string(),
            started_at_tick,
            origin_position: [0.0, 64.0, 0.0],
            tool: tool_id.and_then(spec_for_item_id),
            realm,
        })
    }

    #[test]
    fn session_progress_ticks_are_clamped() {
        let session = test_session(
            GatheringTargetKind::Herb,
            10,
            Some("hoe_iron"),
            Realm::Awaken,
        );
        assert_eq!(session.progress_ticks_at(5), 0);
        assert_eq!(session.progress_ticks_at(20), 10);
        assert_eq!(session.progress_ticks_at(1000), session.total_ticks);
    }

    #[test]
    fn no_tool_is_slower_than_matching_tool() {
        let bare = test_session(GatheringTargetKind::Wood, 0, None, Realm::Awaken);
        let axe = test_session(
            GatheringTargetKind::Wood,
            0,
            Some("axe_iron"),
            Realm::Awaken,
        );

        assert_eq!(bare.total_ticks, 150);
        assert_eq!(axe.total_ticks, 50);
    }

    #[test]
    fn realm_reduces_time_to_void_floor() {
        let awaken = test_session(
            GatheringTargetKind::Ore,
            0,
            Some("pickaxe_iron"),
            Realm::Awaken,
        );
        let void = test_session(
            GatheringTargetKind::Ore,
            0,
            Some("pickaxe_iron"),
            Realm::Void,
        );

        assert_eq!(awaken.total_ticks, 60);
        assert_eq!(void.total_ticks, 45);
    }

    #[test]
    fn interrupt_on_move_uses_plan_threshold() {
        let session = test_session(
            GatheringTargetKind::Herb,
            0,
            Some("hoe_iron"),
            Realm::Awaken,
        );

        assert!(!session.moved_too_far(&Position::new([0.1, 64.0, 0.1])));
        assert!(session.moved_too_far(&Position::new([0.4, 64.0, 0.0])));
    }
}
