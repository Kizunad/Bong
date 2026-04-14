use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Entity, Event};

use crate::player::gameplay::CombatAction;

#[derive(Debug, Clone, Event, Serialize, Deserialize)]
pub struct AttackIntent {
    pub attacker: Entity,
    pub target: Option<Entity>,
    pub issued_at_tick: u64,
    pub reach: f32,
    pub debug_command: Option<CombatAction>,
}

#[derive(Debug, Clone, Event, Serialize, Deserialize)]
pub struct CombatEvent {
    pub attacker: Entity,
    pub target: Entity,
    pub resolved_at_tick: u64,
    pub description: String,
}

#[derive(Debug, Clone, Event, Serialize, Deserialize)]
pub struct DeathEvent {
    pub target: Entity,
    pub cause: String,
    pub at_tick: u64,
}
