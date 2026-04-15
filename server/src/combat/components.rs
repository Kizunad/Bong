use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component};

const DEFAULT_HEALTH_MAX: f32 = 100.0;
const DEFAULT_STAMINA_MAX: f32 = 100.0;
const DEFAULT_STAMINA_RECOVER_PER_SEC: f32 = 5.0;
const DEFAULT_FORTUNE_REMAINING: u8 = 3;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wound {
    pub severity: f32,
    pub bleeding_per_sec: f32,
    pub created_at_tick: u64,
    pub inflicted_by: Option<String>,
}

#[derive(Debug, Clone, Component, Serialize, Deserialize)]
pub struct Wounds {
    pub entries: Vec<Wound>,
    pub health_current: f32,
    pub health_max: f32,
}

impl Default for Wounds {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            health_current: DEFAULT_HEALTH_MAX,
            health_max: DEFAULT_HEALTH_MAX,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StaminaState {
    Idle,
    Walking,
    Jogging,
    Sprinting,
    Combat,
    Exhausted,
}

#[derive(Debug, Clone, Component, Serialize, Deserialize)]
pub struct Stamina {
    pub current: f32,
    pub max: f32,
    pub recover_per_sec: f32,
    pub last_drain_tick: Option<u64>,
    pub state: StaminaState,
}

impl Default for Stamina {
    fn default() -> Self {
        Self {
            current: DEFAULT_STAMINA_MAX,
            max: DEFAULT_STAMINA_MAX,
            recover_per_sec: DEFAULT_STAMINA_RECOVER_PER_SEC,
            last_drain_tick: None,
            state: StaminaState::Idle,
        }
    }
}

impl Stamina {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn normalized(&self) -> Self {
        let max = self.max.max(1.0);

        let mut normalized = self.clone();
        normalized.max = max;
        normalized.current = self.current.clamp(0.0, max);
        normalized.recover_per_sec = self.recover_per_sec.max(0.0);

        if normalized.current <= 0.0 && normalized.state == StaminaState::Sprinting {
            normalized.state = StaminaState::Exhausted;
        }

        normalized
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefenseWindow {
    pub opened_at_tick: u64,
    pub duration_ms: u32,
}

#[derive(Debug, Clone, Component, Default, Serialize, Deserialize)]
pub struct CombatState {
    pub in_combat_until_tick: Option<u64>,
    pub last_attack_at_tick: Option<u64>,
    pub incoming_window: Option<DefenseWindow>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LifecycleState {
    Alive,
    NearDeath,
    AwaitingRevival,
    Terminated,
}

#[derive(Debug, Clone, Component, Serialize, Deserialize)]
pub struct Lifecycle {
    pub character_id: String,
    pub death_count: u32,
    pub fortune_remaining: u8,
    pub last_death_tick: Option<u64>,
    pub last_revive_tick: Option<u64>,
    pub weakened_until_tick: Option<u64>,
    pub state: LifecycleState,
}

impl Default for Lifecycle {
    fn default() -> Self {
        Self {
            character_id: "unbound:character".to_string(),
            death_count: 0,
            fortune_remaining: DEFAULT_FORTUNE_REMAINING,
            last_death_tick: None,
            last_revive_tick: None,
            weakened_until_tick: None,
            state: LifecycleState::Alive,
        }
    }
}

#[derive(Debug, Clone, Component, Serialize, Deserialize)]
pub struct DerivedAttrs {
    pub attack_power: f32,
    pub defense_power: f32,
    pub move_speed_multiplier: f32,
}

impl Default for DerivedAttrs {
    fn default() -> Self {
        Self {
            attack_power: 1.0,
            defense_power: 1.0,
            move_speed_multiplier: 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_character_id_supports_canonical_string_ids() {
        let player_lifecycle = Lifecycle {
            character_id: "offline:Alice".to_string(),
            ..Default::default()
        };
        let npc_lifecycle = Lifecycle {
            character_id: "npc_42v7".to_string(),
            ..Default::default()
        };

        assert_eq!(player_lifecycle.character_id, "offline:Alice");
        assert_eq!(npc_lifecycle.character_id, "npc_42v7");
    }

    #[test]
    fn stamina_normalized_clamps_values_and_exhausts_invalid_sprint() {
        let stamina = Stamina {
            current: -8.0,
            max: 0.0,
            recover_per_sec: -2.0,
            last_drain_tick: Some(12),
            state: StaminaState::Sprinting,
        };

        let normalized = stamina.normalized();

        assert_eq!(normalized.max, 1.0);
        assert_eq!(normalized.current, 0.0);
        assert_eq!(normalized.recover_per_sec, 0.0);
        assert_eq!(normalized.last_drain_tick, Some(12));
        assert_eq!(normalized.state, StaminaState::Exhausted);
    }
}
