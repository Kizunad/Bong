use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::common::{GameEventType, NpcStateKind, PlayerTrend};

pub type Vec3 = [f64; 3];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerPowerBreakdown {
    pub combat: f64,
    pub wealth: f64,
    pub social: f64,
    pub karma: f64,
    pub territory: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerProfile {
    pub uuid: String,
    pub name: String,
    pub realm: String,
    pub composite_power: f64,
    pub breakdown: PlayerPowerBreakdown,
    pub trend: PlayerTrend,
    pub active_hours: f64,
    pub zone: String,
    pub pos: Vec3,
    pub recent_kills: u32,
    pub recent_deaths: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpcSnapshot {
    pub id: String,
    pub kind: String,
    pub pos: Vec3,
    pub state: NpcStateKind,
    pub blackboard: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneSnapshot {
    pub name: String,
    pub spirit_qi: f64,
    pub danger_level: u8,
    pub active_events: Vec<String>,
    pub player_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameEvent {
    #[serde(rename = "type")]
    pub event_type: GameEventType,
    pub tick: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub player: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldStateV1 {
    pub v: u8,
    pub ts: u64,
    pub tick: u64,
    pub players: Vec<PlayerProfile>,
    pub npcs: Vec<NpcSnapshot>,
    pub zones: Vec<ZoneSnapshot>,
    pub recent_events: Vec<GameEvent>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_world_state_sample() {
        let json = include_str!("../../../agent/packages/schema/samples/world-state.sample.json");
        let state: WorldStateV1 = serde_json::from_str(json)
            .expect("world-state.sample.json should deserialize into WorldStateV1");

        assert_eq!(state.v, 1);
        assert_eq!(state.tick, 84000);
        assert_eq!(state.players.len(), 2);
        assert_eq!(state.players[0].name, "Steve");
        assert_eq!(state.players[0].pos, [128.5, 66.0, 200.3]);
        assert_eq!(state.npcs.len(), 1);
        assert_eq!(state.npcs[0].id, "npc_001");
        assert_eq!(state.zones.len(), 2);
        assert_eq!(state.recent_events.len(), 2);
    }

    #[test]
    fn roundtrip_world_state() {
        let json = include_str!("../../../agent/packages/schema/samples/world-state.sample.json");
        let state: WorldStateV1 = serde_json::from_str(json).unwrap();
        let re_json = serde_json::to_string(&state).unwrap();
        let state2: WorldStateV1 = serde_json::from_str(&re_json).unwrap();
        assert_eq!(state.v, state2.v);
        assert_eq!(state.tick, state2.tick);
        assert_eq!(state.players.len(), state2.players.len());
    }
}
