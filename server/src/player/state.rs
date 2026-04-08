use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component, Resource, Uuid};

pub const DEFAULT_PLAYER_DATA_DIR: &str = "data/players";
pub const PLAYER_STATE_AUTOSAVE_INTERVAL_TICKS: u64 = 1_200;

const DEFAULT_REALM: &str = "mortal";
const DEFAULT_SPIRIT_QI_MAX: f64 = 100.0;
const EXPERIENCE_SCORE_DIVISOR: f64 = 10_000.0;
const REALM_SCORE_QI_REFINING_DIVISOR: f64 = 12.0;
const REALM_SCORE_FOUNDATION_BASE: f64 = 0.7;
const REALM_SCORE_FOUNDATION_STEP: f64 = 0.08;

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

    pub fn power_breakdown(&self) -> crate::schema::world_state::PlayerPowerBreakdown {
        let normalized = self.normalized();
        let realm_score = realm_progress_score(normalized.realm.as_str());
        let qi_ratio = ratio_score(normalized.spirit_qi, normalized.spirit_qi_max);
        let experience_score =
            (normalized.experience as f64 / EXPERIENCE_SCORE_DIVISOR).clamp(0.0, 1.0);
        let wealth = clamp_unit(normalized.inventory_score);
        let karma_alignment = ((normalized.karma + 1.0) * 0.5).clamp(0.0, 1.0);
        let karma_influence = normalized.karma.abs().clamp(0.0, 1.0);

        crate::schema::world_state::PlayerPowerBreakdown {
            combat: clamp_unit(realm_score * 0.6 + qi_ratio * 0.4),
            wealth,
            social: clamp_unit(experience_score * 0.6 + karma_alignment * 0.4),
            karma: karma_influence,
            territory: clamp_unit(experience_score * 0.5 + wealth * 0.5),
        }
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

    pub fn data_dir(&self) -> &Path {
        self.data_dir.as_path()
    }

    pub fn path_for_uuid(&self, uuid: Uuid) -> PathBuf {
        self.data_dir.join(format!("{uuid}.json"))
    }
}

#[derive(Debug, Default)]
pub struct PlayerStateAutosaveTimer {
    pub ticks: u64,
}

impl Resource for PlayerStateAutosaveTimer {}

pub fn load_or_init_player_state(persistence: &PlayerStatePersistence, uuid: Uuid) -> PlayerState {
    let path = persistence.path_for_uuid(uuid);
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return PlayerState::default();
        }
        Err(error) => {
            tracing::warn!(
                "[bong][player] failed to read PlayerState for {uuid} from {}: {error}; using default state",
                path.display()
            );
            return PlayerState::default();
        }
    };

    match serde_json::from_str::<PlayerState>(&contents) {
        Ok(state) => state.normalized(),
        Err(error) => {
            tracing::warn!(
                "[bong][player] failed to parse PlayerState for {uuid} from {}: {error}; using default state",
                path.display()
            );
            PlayerState::default()
        }
    }
}

pub fn save_player_state(
    persistence: &PlayerStatePersistence,
    uuid: Uuid,
    state: &PlayerState,
) -> io::Result<PathBuf> {
    let path = persistence.path_for_uuid(uuid);
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
mod tests {
    use super::*;
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

    #[test]
    fn load_or_init_uses_default_when_file_missing() {
        let data_dir = unique_temp_dir("load-init-missing");
        let persistence = PlayerStatePersistence::new(&data_dir);
        let uuid = Uuid::parse_str("123e4567-e89b-12d3-a456-426614174000")
            .expect("uuid literal should parse");

        let state = load_or_init_player_state(&persistence, uuid);

        assert_eq!(state, PlayerState::default());
    }

    #[test]
    fn save_and_load_roundtrip_by_uuid() {
        let data_dir = unique_temp_dir("save-load-roundtrip");
        let persistence = PlayerStatePersistence::new(&data_dir);
        let uuid = Uuid::parse_str("123e4567-e89b-12d3-a456-426614174111")
            .expect("uuid literal should parse");
        let source = PlayerState {
            realm: "qi_refining_3".to_string(),
            spirit_qi: 78.0,
            spirit_qi_max: 100.0,
            karma: 0.2,
            experience: 1_200,
            inventory_score: 0.4,
        };

        let save_path = save_player_state(&persistence, uuid, &source)
            .expect("save should create player state file");
        let loaded = load_or_init_player_state(&persistence, uuid);

        assert!(
            save_path.ends_with("123e4567-e89b-12d3-a456-426614174111.json"),
            "save path should be data/players/{{uuid}}.json"
        );
        assert_eq!(loaded, source.normalized());

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn save_json_roundtrip_preserves_fields() {
        let state = PlayerState {
            realm: "qi_refining_2".to_string(),
            spirit_qi: 56.0,
            spirit_qi_max: 120.0,
            karma: -0.35,
            experience: 778,
            inventory_score: 0.61,
        };

        let json = serde_json::to_string_pretty(&state).expect("PlayerState should serialize");
        let decoded: PlayerState =
            serde_json::from_str(&json).expect("PlayerState should deserialize");

        assert_eq!(decoded.realm, "qi_refining_2");
        assert_eq!(decoded.spirit_qi, 56.0);
        assert_eq!(decoded.spirit_qi_max, 120.0);
        assert_eq!(decoded.karma, -0.35);
        assert_eq!(decoded.experience, 778);
        assert_eq!(decoded.inventory_score, 0.61);
    }

    #[test]
    fn normalized_clamps_invalid_values() {
        let state = PlayerState {
            realm: "   ".to_string(),
            spirit_qi: 999.0,
            spirit_qi_max: 0.0,
            karma: 5.0,
            experience: 7,
            inventory_score: 5.0,
        };

        let normalized = state.normalized();

        assert_eq!(normalized.realm, "mortal");
        assert_eq!(normalized.spirit_qi_max, 1.0);
        assert_eq!(normalized.spirit_qi, 1.0);
        assert_eq!(normalized.karma, 1.0);
        assert_eq!(normalized.inventory_score, 1.0);
    }
}
