use std::fs;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component, Resource};

use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
use crate::schema::world_state::PlayerPowerBreakdown;

pub const DEFAULT_PLAYER_DATA_DIR: &str = "data/players";
pub const PLAYER_STATE_AUTOSAVE_INTERVAL_TICKS: u64 = 1_200;

const DEFAULT_REALM: &str = "mortal";
const DEFAULT_SPIRIT_QI_MAX: f64 = 100.0;
const REALM_SCORE_QI_REFINING_DIVISOR: f64 = 12.0;
const REALM_SCORE_FOUNDATION_BASE: f64 = 0.7;
const REALM_SCORE_FOUNDATION_STEP: f64 = 0.08;
const EXPERIENCE_SCORE_DIVISOR: f64 = 10_000.0;

#[derive(Clone, Debug, Component, Serialize, Deserialize, PartialEq)]
pub struct PlayerState {
    pub realm: String,
    pub spirit_qi: f64,
    pub spirit_qi_max: f64,
    pub karma: f64,
    pub experience: u64,
    pub inventory_score: f64,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            realm: DEFAULT_REALM.to_string(),
            spirit_qi: 0.0,
            spirit_qi_max: DEFAULT_SPIRIT_QI_MAX,
            karma: 0.0,
            experience: 0,
            inventory_score: 0.0,
        }
    }
}

impl PlayerState {
    pub fn normalized(&self) -> Self {
        let spirit_qi_max = self.spirit_qi_max.max(1.0);
        let realm = if self.realm.trim().is_empty() {
            DEFAULT_REALM.to_string()
        } else {
            self.realm.trim().to_string()
        };

        Self {
            realm,
            spirit_qi: self.spirit_qi.clamp(0.0, spirit_qi_max),
            spirit_qi_max,
            karma: self.karma.clamp(-1.0, 1.0),
            experience: self.experience,
            inventory_score: clamp_unit(self.inventory_score),
        }
    }

    pub fn power_breakdown(&self) -> PlayerPowerBreakdown {
        let normalized = self.normalized();
        let realm_score = realm_progress_score(normalized.realm.as_str());
        let qi_ratio = ratio_score(normalized.spirit_qi, normalized.spirit_qi_max);
        let experience_score =
            (normalized.experience as f64 / EXPERIENCE_SCORE_DIVISOR).clamp(0.0, 1.0);
        let wealth = clamp_unit(normalized.inventory_score);
        let karma_alignment = ((normalized.karma + 1.0) * 0.5).clamp(0.0, 1.0);
        let karma_influence = normalized.karma.abs().clamp(0.0, 1.0);

        PlayerPowerBreakdown {
            combat: clamp_unit(realm_score * 0.6 + qi_ratio * 0.4),
            wealth,
            social: clamp_unit(experience_score * 0.6 + karma_alignment * 0.4),
            karma: karma_influence,
            territory: clamp_unit(experience_score * 0.5 + wealth * 0.5),
        }
    }

    pub fn composite_power(&self) -> f64 {
        let breakdown = self.power_breakdown();

        clamp_unit(
            breakdown.combat * 0.4
                + breakdown.wealth * 0.15
                + breakdown.social * 0.15
                + breakdown.karma * 0.15
                + breakdown.territory * 0.15,
        )
    }

    pub fn server_payload(&self, player: Option<String>, zone: impl Into<String>) -> ServerDataV1 {
        let normalized = self.normalized();

        ServerDataV1::new(ServerDataPayloadV1::PlayerState {
            player,
            realm: normalized.realm.clone(),
            spirit_qi: normalized.spirit_qi,
            karma: normalized.karma,
            composite_power: normalized.composite_power(),
            breakdown: normalized.power_breakdown(),
            zone: zone.into(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct PlayerStatePersistence {
    data_dir: PathBuf,
}

impl Default for PlayerStatePersistence {
    fn default() -> Self {
        Self::new(DEFAULT_PLAYER_DATA_DIR)
    }
}

impl Resource for PlayerStatePersistence {}

impl PlayerStatePersistence {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
        }
    }

    pub fn data_dir(&self) -> &std::path::Path {
        self.data_dir.as_path()
    }

    pub fn path_for_username(&self, username: &str) -> PathBuf {
        let player_key = canonical_player_id(username);
        self.data_dir.join(format!("{player_key}.json"))
    }
}

#[derive(Debug, Default)]
pub struct PlayerStateAutosaveTimer {
    pub ticks: u64,
}

impl Resource for PlayerStateAutosaveTimer {}

pub fn canonical_player_id(username: &str) -> String {
    format!("offline:{username}")
}

pub fn load_player_state(persistence: &PlayerStatePersistence, username: &str) -> PlayerState {
    let path = persistence.path_for_username(username);

    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            tracing::warn!(
                "[bong][player] no saved PlayerState for `{}` at {}; using default state",
                canonical_player_id(username),
                path.display()
            );
            return PlayerState::default();
        }
        Err(error) => {
            tracing::warn!(
                "[bong][player] failed to read PlayerState for `{}` from {}: {error}; using default state",
                canonical_player_id(username),
                path.display()
            );
            return PlayerState::default();
        }
    };

    match serde_json::from_str::<PlayerState>(&contents) {
        Ok(state) => state.normalized(),
        Err(error) => {
            tracing::warn!(
                "[bong][player] failed to parse PlayerState for `{}` from {}: {error}; using default state",
                canonical_player_id(username),
                path.display()
            );
            PlayerState::default()
        }
    }
}

pub fn save_player_state(
    persistence: &PlayerStatePersistence,
    username: &str,
    state: &PlayerState,
) -> io::Result<PathBuf> {
    let path = persistence.path_for_username(username);
    let normalized = state.normalized();
    let serialized = serde_json::to_vec_pretty(&normalized)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

    fs::create_dir_all(persistence.data_dir())?;
    fs::write(&path, serialized)?;

    Ok(path)
}

fn ratio_score(value: f64, max: f64) -> f64 {
    if max <= 0.0 {
        0.0
    } else {
        (value / max).clamp(0.0, 1.0)
    }
}

fn clamp_unit(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

fn realm_progress_score(realm: &str) -> f64 {
    let normalized = realm.trim().to_ascii_lowercase();

    if normalized == DEFAULT_REALM {
        return 0.05;
    }

    if let Some(stage) = normalized
        .strip_prefix("qi_refining_")
        .and_then(|value| value.parse::<u8>().ok())
    {
        return clamp_unit(stage as f64 / REALM_SCORE_QI_REFINING_DIVISOR);
    }

    if let Some(stage) = normalized
        .strip_prefix("foundation_establishment_")
        .or_else(|| normalized.strip_prefix("foundation_"))
        .and_then(|value| value.parse::<u8>().ok())
    {
        return clamp_unit(
            REALM_SCORE_FOUNDATION_BASE + stage as f64 * REALM_SCORE_FOUNDATION_STEP,
        );
    }

    match normalized.as_str() {
        "golden_core" => 0.92,
        "nascent_soul" => 1.0,
        _ => 0.0,
    }
}

#[cfg(test)]
mod player_state_tests {
    use super::*;
    use crate::network::agent_bridge::serialize_server_data_payload;
    use crate::schema::server_data::{ServerDataPayloadV1, SERVER_DATA_VERSION};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(test_name: &str) -> PathBuf {
        let unique_suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();

        std::env::temp_dir().join(format!(
            "bong-player-state-{test_name}-{}-{unique_suffix}",
            std::process::id()
        ))
    }

    fn approx_eq(left: f64, right: f64) {
        assert!(
            (left - right).abs() < 1e-9,
            "expected {left} to be approximately equal to {right}"
        );
    }

    #[test]
    fn loads_and_saves_offline_player_state() {
        let data_dir = unique_temp_dir("load-save");
        let persistence = PlayerStatePersistence::new(&data_dir);
        let default_state = load_player_state(&persistence, "Azure");

        assert_eq!(default_state, PlayerState::default());

        let persisted = PlayerState {
            realm: "qi_refining_3".to_string(),
            spirit_qi: 78.0,
            spirit_qi_max: 100.0,
            karma: 0.2,
            experience: 1_200,
            inventory_score: 0.4,
        };

        let save_path = save_player_state(&persistence, "Azure", &persisted)
            .expect("saving PlayerState should succeed");
        let reloaded = load_player_state(&persistence, "Azure");

        assert!(
            save_path.ends_with("offline:Azure.json"),
            "save path should use canonical offline:{{username}} key"
        );
        assert_eq!(reloaded, persisted.normalized());

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn computes_composite_power() {
        let state = PlayerState {
            realm: "qi_refining_3".to_string(),
            spirit_qi: 60.0,
            spirit_qi_max: 100.0,
            karma: 0.25,
            experience: 2_000,
            inventory_score: 0.4,
        };

        let breakdown = state.power_breakdown();
        approx_eq(breakdown.combat, 0.39);
        approx_eq(breakdown.wealth, 0.4);
        approx_eq(breakdown.social, 0.37);
        approx_eq(breakdown.karma, 0.25);
        approx_eq(breakdown.territory, 0.3);
        approx_eq(state.composite_power(), 0.354);
    }

    #[test]
    fn serializes_player_state_payload() {
        let state = PlayerState {
            realm: "qi_refining_3".to_string(),
            spirit_qi: 78.0,
            spirit_qi_max: 100.0,
            karma: 0.2,
            experience: 1_200,
            inventory_score: 0.4,
        };
        let payload = state.server_payload(Some(canonical_player_id("Steve")), "blood_valley");
        let bytes =
            serialize_server_data_payload(&payload).expect("PlayerState payload should serialize");
        let json: serde_json::Value =
            serde_json::from_slice(&bytes).expect("serialized payload should decode as JSON value");

        assert_eq!(json.get("v"), Some(&serde_json::json!(SERVER_DATA_VERSION)));
        assert_eq!(json.get("type"), Some(&serde_json::json!("player_state")));
        assert_eq!(
            json.get("player"),
            Some(&serde_json::json!("offline:Steve"))
        );
        assert_eq!(json.get("realm"), Some(&serde_json::json!("qi_refining_3")));
        assert_eq!(json.get("spirit_qi"), Some(&serde_json::json!(78.0)));
        assert_eq!(json.get("karma"), Some(&serde_json::json!(0.2)));
        assert_eq!(json.get("zone"), Some(&serde_json::json!("blood_valley")));

        match payload.payload {
            ServerDataPayloadV1::PlayerState {
                composite_power,
                breakdown,
                ..
            } => {
                approx_eq(composite_power, state.composite_power());
                approx_eq(breakdown.combat, state.power_breakdown().combat);
                approx_eq(breakdown.wealth, state.power_breakdown().wealth);
                approx_eq(breakdown.social, state.power_breakdown().social);
                approx_eq(breakdown.karma, state.power_breakdown().karma);
                approx_eq(breakdown.territory, state.power_breakdown().territory);
            }
            other => panic!("expected PlayerState payload, got {other:?}"),
        }
    }

    #[test]
    fn corrupt_save_uses_default_state() {
        let data_dir = unique_temp_dir("corrupt");
        let persistence = PlayerStatePersistence::new(&data_dir);
        let save_path = persistence.path_for_username("CorruptCultivator");

        fs::create_dir_all(&data_dir).expect("test data dir should be creatable");
        fs::write(&save_path, b"{not valid json")
            .expect("corrupt PlayerState fixture should be writable");

        let recovered = load_player_state(&persistence, "CorruptCultivator");
        assert_eq!(recovered, PlayerState::default());

        let _ = fs::remove_dir_all(&data_dir);
    }
}
