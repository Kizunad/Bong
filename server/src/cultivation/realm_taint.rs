use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use valence::prelude::{
    bevy_ecs, App, Commands, Component, Entity, Event, EventReader, Query, Update,
};

use crate::combat::components::TICKS_PER_SECOND;

pub const NICHE_INTRUSION_WASH_TICKS: u64 = 8 * 60 * 60 * TICKS_PER_SECOND;
pub const NICHE_INTRUSION_SINGLE_DELTA: f32 = 0.20;
pub const NICHE_INTRUSION_MAIN_COLOR_THRESHOLD: f32 = 1.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RealmTaintedKind {
    NicheIntrusion,
}

#[derive(Debug, Clone, Component, Serialize, Deserialize, PartialEq)]
pub struct RealmTaintState {
    pub kind: RealmTaintedKind,
    pub qi_taint_severity: f32,
    pub last_tainted_at: u64,
    pub wash_available_at: u64,
}

impl Default for RealmTaintState {
    fn default() -> Self {
        Self {
            kind: RealmTaintedKind::NicheIntrusion,
            qi_taint_severity: 0.0,
            last_tainted_at: 0,
            wash_available_at: 0,
        }
    }
}

impl RealmTaintState {
    pub fn add_niche_intrusion(&mut self, delta: f32, now_tick: u64) {
        self.kind = RealmTaintedKind::NicheIntrusion;
        self.qi_taint_severity = (self.qi_taint_severity + delta.max(0.0)).clamp(0.0, 1.0);
        self.last_tainted_at = now_tick;
        self.wash_available_at = now_tick.saturating_add(NICHE_INTRUSION_WASH_TICKS);
    }

    pub fn is_main_color(&self) -> bool {
        self.kind == RealmTaintedKind::NicheIntrusion
            && self.qi_taint_severity >= NICHE_INTRUSION_MAIN_COLOR_THRESHOLD
    }

    pub fn wash_if_ready(&mut self, now_tick: u64) -> bool {
        if now_tick < self.wash_available_at || self.qi_taint_severity <= 0.0 {
            return false;
        }
        self.qi_taint_severity = 0.0;
        true
    }
}

#[derive(Debug, Clone, Event, Serialize, Deserialize)]
pub struct ApplyRealmTaint {
    pub target: valence::prelude::Entity,
    pub kind: RealmTaintedKind,
    pub delta: f32,
    pub tick: u64,
}

pub fn register(app: &mut App) {
    app.add_event::<ApplyRealmTaint>();
    app.add_systems(Update, apply_realm_taint_events);
}

pub fn apply_realm_taint_events(
    mut events: EventReader<ApplyRealmTaint>,
    mut commands: Commands,
    mut targets: Query<Option<&mut RealmTaintState>>,
) {
    let mut pending_inserts: HashMap<Entity, RealmTaintState> = HashMap::new();
    for event in events.read() {
        match targets.get_mut(event.target) {
            Ok(Some(mut state)) => apply_taint(&mut state, event),
            Ok(None) => {
                let state = pending_inserts.entry(event.target).or_default();
                apply_taint(state, event);
            }
            Err(_) => continue,
        }
    }
    for (target, state) in pending_inserts {
        commands.entity(target).insert(state);
    }
}

fn apply_taint(state: &mut RealmTaintState, event: &ApplyRealmTaint) {
    if event.kind == RealmTaintedKind::NicheIntrusion {
        state.add_niche_intrusion(event.delta, event.tick);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn niche_intrusion_taint_accumulates_to_main_color() {
        let mut state = RealmTaintState::default();
        for idx in 0..5 {
            state.add_niche_intrusion(NICHE_INTRUSION_SINGLE_DELTA, idx);
        }
        assert!(state.is_main_color());
        assert_eq!(state.qi_taint_severity, 1.0);
    }

    #[test]
    fn niche_intrusion_taint_requires_eight_hours_before_wash() {
        let mut state = RealmTaintState::default();
        state.add_niche_intrusion(NICHE_INTRUSION_SINGLE_DELTA, 10);
        assert!(!state.wash_if_ready(10 + NICHE_INTRUSION_WASH_TICKS - 1));
        assert!(state.wash_if_ready(10 + NICHE_INTRUSION_WASH_TICKS));
        assert_eq!(state.qi_taint_severity, 0.0);
    }

    #[test]
    fn apply_realm_taint_events_initializes_missing_state() {
        let mut app = App::new();
        app.add_event::<ApplyRealmTaint>();
        app.add_systems(Update, apply_realm_taint_events);
        let target = app.world_mut().spawn_empty().id();

        app.world_mut().send_event(ApplyRealmTaint {
            target,
            kind: RealmTaintedKind::NicheIntrusion,
            delta: NICHE_INTRUSION_SINGLE_DELTA,
            tick: 10,
        });
        app.update();

        let state = app
            .world()
            .get::<RealmTaintState>(target)
            .expect("taint event should attach missing state");
        assert_eq!(state.qi_taint_severity, NICHE_INTRUSION_SINGLE_DELTA);
        assert_eq!(state.last_tainted_at, 10);
    }

    #[test]
    fn apply_realm_taint_events_accumulates_same_frame_missing_state() {
        let mut app = App::new();
        app.add_event::<ApplyRealmTaint>();
        app.add_systems(Update, apply_realm_taint_events);
        let target = app.world_mut().spawn_empty().id();

        for tick in [10, 11] {
            app.world_mut().send_event(ApplyRealmTaint {
                target,
                kind: RealmTaintedKind::NicheIntrusion,
                delta: NICHE_INTRUSION_SINGLE_DELTA,
                tick,
            });
        }
        app.update();

        let state = app
            .world()
            .get::<RealmTaintState>(target)
            .expect("queued taints should attach one accumulated state");
        assert_eq!(state.qi_taint_severity, NICHE_INTRUSION_SINGLE_DELTA * 2.0);
        assert_eq!(state.last_tainted_at, 11);
    }
}
