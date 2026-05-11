//! Audio recipe schema and S2C CustomPayload payloads (`bong:audio/play`, `bong:audio/stop`).
//!
//! Audio v1 intentionally uses vanilla Minecraft sound ids only. The server keeps
//! the authoritative JSON recipe registry and includes the resolved recipe snapshot
//! in each play payload so the client can play hot-reloaded recipes without a
//! parallel resource-pack registry.

use serde::{Deserialize, Serialize};

use super::common::MAX_PAYLOAD_BYTES;

pub const AUDIO_EVENT_VERSION: u8 = 1;
pub const AUDIO_VOLUME_MIN: f32 = 0.0;
pub const AUDIO_VOLUME_MAX: f32 = 4.0;
pub const AUDIO_PITCH_MIN: f32 = 0.1;
pub const AUDIO_PITCH_MAX: f32 = 2.0;
pub const AUDIO_PRIORITY_MAX: u8 = 100;

#[derive(Debug)]
pub enum AudioEventBuildError {
    Json(serde_json::Error),
    Oversize { size: usize, max: usize },
    InvalidRecipe(String),
    InvalidPlayPayload(String),
    InvalidStopPayload(String),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AudioAttenuation {
    PlayerLocal,
    #[serde(rename = "world_3d")]
    World3d,
    GlobalHint,
    ZoneBroadcast,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AudioSoundCategory {
    Master,
    Players,
    Hostile,
    Ambient,
    Voice,
    Blocks,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SoundLayer {
    pub sound: String,
    pub volume: f32,
    pub pitch: f32,
    pub delay_ticks: u32,
}

impl SoundLayer {
    pub fn validate(&self) -> Result<(), String> {
        validate_identifier(&self.sound)
            .map_err(|error| format!("sound `{}` {error}", self.sound))?;
        if !self.volume.is_finite()
            || self.volume < AUDIO_VOLUME_MIN
            || self.volume > AUDIO_VOLUME_MAX
        {
            return Err(format!(
                "layer sound `{}` volume must be finite in [{AUDIO_VOLUME_MIN}, {AUDIO_VOLUME_MAX}], got {}",
                self.sound, self.volume
            ));
        }
        if !self.pitch.is_finite() || self.pitch < AUDIO_PITCH_MIN || self.pitch > AUDIO_PITCH_MAX {
            return Err(format!(
                "layer sound `{}` pitch must be finite in [{AUDIO_PITCH_MIN}, {AUDIO_PITCH_MAX}], got {}",
                self.sound, self.pitch
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoopConfig {
    pub interval_ticks: u32,
    pub while_flag: String,
}

impl LoopConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.interval_ticks == 0 {
            return Err("loop.interval_ticks must be > 0".to_string());
        }
        if self.while_flag.trim().is_empty() {
            return Err("loop.while_flag must not be blank".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SoundRecipe {
    pub id: String,
    pub layers: Vec<SoundLayer>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "loop")]
    pub loop_cfg: Option<LoopConfig>,
    #[serde(default)]
    pub priority: u8,
    pub attenuation: AudioAttenuation,
    pub category: AudioSoundCategory,
}

impl SoundRecipe {
    pub fn validate(&self) -> Result<(), String> {
        validate_recipe_id(&self.id)?;
        if self.layers.is_empty() {
            return Err(format!(
                "recipe `{}` must contain at least one layer",
                self.id
            ));
        }
        if self.priority > AUDIO_PRIORITY_MAX {
            return Err(format!(
                "recipe `{}` priority must be <= {AUDIO_PRIORITY_MAX}, got {}",
                self.id, self.priority
            ));
        }
        for layer in &self.layers {
            layer
                .validate()
                .map_err(|error| format!("recipe `{}` invalid layer: {error}", self.id))?;
        }
        if let Some(loop_cfg) = &self.loop_cfg {
            loop_cfg
                .validate()
                .map_err(|error| format!("recipe `{}` invalid loop: {error}", self.id))?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlaySoundRecipePayload {
    pub recipe_id: String,
    pub instance_id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pos: Option<[i32; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flag: Option<String>,
    pub volume_mul: f32,
    pub pitch_shift: f32,
    pub recipe: SoundRecipe,
}

impl PlaySoundRecipePayload {
    pub fn validate(&self) -> Result<(), AudioEventBuildError> {
        validate_recipe_id(&self.recipe_id).map_err(AudioEventBuildError::InvalidPlayPayload)?;
        if self.recipe_id != self.recipe.id {
            return Err(AudioEventBuildError::InvalidPlayPayload(format!(
                "recipe_id `{}` does not match recipe.id `{}`",
                self.recipe_id, self.recipe.id
            )));
        }
        if let Some(flag) = &self.flag {
            if flag.trim().is_empty() {
                return Err(AudioEventBuildError::InvalidPlayPayload(
                    "flag must not be blank when present".to_string(),
                ));
            }
        }
        if !self.volume_mul.is_finite()
            || self.volume_mul < 0.0
            || self.volume_mul > AUDIO_VOLUME_MAX
        {
            return Err(AudioEventBuildError::InvalidPlayPayload(format!(
                "volume_mul must be finite in [0, {AUDIO_VOLUME_MAX}], got {}",
                self.volume_mul
            )));
        }
        if !self.pitch_shift.is_finite() || self.pitch_shift < -1.0 || self.pitch_shift > 1.0 {
            return Err(AudioEventBuildError::InvalidPlayPayload(format!(
                "pitch_shift must be finite in [-1, 1], got {}",
                self.pitch_shift
            )));
        }
        self.recipe
            .validate()
            .map_err(AudioEventBuildError::InvalidRecipe)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StopSoundRecipePayload {
    pub instance_id: u64,
    pub fade_out_ticks: u32,
}

impl StopSoundRecipePayload {
    pub fn validate(&self) -> Result<(), AudioEventBuildError> {
        if self.instance_id == 0 {
            return Err(AudioEventBuildError::InvalidStopPayload(
                "instance_id must be > 0".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AmbientZoneS2c {
    pub v: u8,
    pub zone_name: String,
    pub ambient_recipe_id: String,
    pub music_state: String,
    pub is_night: bool,
    pub season: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tsy_depth: Option<String>,
    pub fade_ticks: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pos: Option<[i32; 3]>,
    pub volume_mul: f32,
    pub pitch_shift: f32,
    pub recipe: SoundRecipe,
}

impl AmbientZoneS2c {
    pub fn to_json_bytes_checked(&self) -> Result<Vec<u8>, AudioEventBuildError> {
        if self.v != AUDIO_EVENT_VERSION {
            return Err(AudioEventBuildError::InvalidPlayPayload(format!(
                "ambient_zone version must be {AUDIO_EVENT_VERSION}, got {}",
                self.v
            )));
        }
        if self.zone_name.trim().is_empty() {
            return Err(AudioEventBuildError::InvalidPlayPayload(
                "zone_name must not be blank".to_string(),
            ));
        }
        validate_recipe_id(&self.ambient_recipe_id)
            .map_err(AudioEventBuildError::InvalidPlayPayload)?;
        if self.ambient_recipe_id != self.recipe.id {
            return Err(AudioEventBuildError::InvalidPlayPayload(format!(
                "ambient_recipe_id `{}` does not match recipe.id `{}`",
                self.ambient_recipe_id, self.recipe.id
            )));
        }
        if !matches!(
            self.music_state.as_str(),
            "AMBIENT" | "COMBAT" | "CULTIVATION" | "TSY" | "TRIBULATION"
        ) {
            return Err(AudioEventBuildError::InvalidPlayPayload(format!(
                "unsupported music_state `{}`",
                self.music_state
            )));
        }
        if !matches!(
            self.season.as_str(),
            "summer" | "summer_to_winter" | "winter" | "winter_to_summer"
        ) {
            return Err(AudioEventBuildError::InvalidPlayPayload(format!(
                "unsupported season `{}`",
                self.season
            )));
        }
        if !self.volume_mul.is_finite()
            || self.volume_mul < 0.0
            || self.volume_mul > AUDIO_VOLUME_MAX
        {
            return Err(AudioEventBuildError::InvalidPlayPayload(format!(
                "volume_mul must be finite in [0, {AUDIO_VOLUME_MAX}], got {}",
                self.volume_mul
            )));
        }
        if !self.pitch_shift.is_finite() || self.pitch_shift < -1.0 || self.pitch_shift > 1.0 {
            return Err(AudioEventBuildError::InvalidPlayPayload(format!(
                "pitch_shift must be finite in [-1, 1], got {}",
                self.pitch_shift
            )));
        }
        self.recipe
            .validate()
            .map_err(AudioEventBuildError::InvalidRecipe)?;
        json_bytes_checked(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlaySoundRecipeEventV1 {
    pub v: u8,
    #[serde(flatten)]
    pub payload: PlaySoundRecipePayload,
}

impl PlaySoundRecipeEventV1 {
    pub fn new(payload: PlaySoundRecipePayload) -> Self {
        Self {
            v: AUDIO_EVENT_VERSION,
            payload,
        }
    }

    pub fn to_json_bytes_checked(&self) -> Result<Vec<u8>, AudioEventBuildError> {
        self.payload.validate()?;
        json_bytes_checked(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StopSoundRecipeEventV1 {
    pub v: u8,
    #[serde(flatten)]
    pub payload: StopSoundRecipePayload,
}

impl StopSoundRecipeEventV1 {
    pub fn new(payload: StopSoundRecipePayload) -> Self {
        Self {
            v: AUDIO_EVENT_VERSION,
            payload,
        }
    }

    pub fn to_json_bytes_checked(&self) -> Result<Vec<u8>, AudioEventBuildError> {
        self.payload.validate()?;
        json_bytes_checked(self)
    }
}

fn json_bytes_checked<T: Serialize>(payload: &T) -> Result<Vec<u8>, AudioEventBuildError> {
    let bytes = serde_json::to_vec(payload).map_err(AudioEventBuildError::Json)?;
    if bytes.len() > MAX_PAYLOAD_BYTES {
        return Err(AudioEventBuildError::Oversize {
            size: bytes.len(),
            max: MAX_PAYLOAD_BYTES,
        });
    }
    Ok(bytes)
}

pub fn validate_recipe_id(id: &str) -> Result<(), String> {
    if id.is_empty() {
        return Err("must not be empty".to_string());
    }
    if id.len() > 128 {
        return Err("must be <= 128 characters".to_string());
    }
    if id
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        Ok(())
    } else {
        Err("must match [a-z0-9_]+".to_string())
    }
}

pub fn validate_identifier(id: &str) -> Result<(), String> {
    let Some((namespace, path)) = id.split_once(':') else {
        return Err("must be namespace:path".to_string());
    };
    if namespace.is_empty() || path.is_empty() {
        return Err("must have non-empty namespace and path".to_string());
    }
    if !namespace
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-' || c == '.')
    {
        return Err("namespace has invalid characters".to_string());
    }
    if !path.chars().all(|c| {
        c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-' || c == '.' || c == '/'
    }) {
        return Err("path has invalid characters".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_recipe() -> SoundRecipe {
        SoundRecipe {
            id: "pill_consume".to_string(),
            layers: vec![
                SoundLayer {
                    sound: "minecraft:entity.generic.drink".to_string(),
                    volume: 0.4,
                    pitch: 1.0,
                    delay_ticks: 0,
                },
                SoundLayer {
                    sound: "minecraft:block.brewing_stand.brew".to_string(),
                    volume: 0.3,
                    pitch: 1.2,
                    delay_ticks: 5,
                },
            ],
            loop_cfg: None,
            priority: 40,
            attenuation: AudioAttenuation::PlayerLocal,
            category: AudioSoundCategory::Voice,
        }
    }

    #[test]
    fn sound_recipe_validates_vanilla_layers() {
        sample_recipe()
            .validate()
            .expect("sample recipe should validate");
    }

    #[test]
    fn sound_category_players_roundtrips() {
        let json = serde_json::to_string(&AudioSoundCategory::Players).expect("serialize category");
        assert_eq!(json, "\"PLAYERS\"");
        let back: AudioSoundCategory = serde_json::from_str(&json).expect("deserialize category");
        assert_eq!(back, AudioSoundCategory::Players);
    }

    #[test]
    fn sound_recipe_rejects_bad_pitch() {
        let mut recipe = sample_recipe();
        recipe.layers[0].pitch = 2.5;
        let error = recipe.validate().expect_err("pitch above max should fail");
        assert!(
            error.contains("pitch"),
            "error should mention pitch: {error}"
        );
    }

    #[test]
    fn play_payload_roundtrips_with_inline_recipe() {
        let payload = PlaySoundRecipeEventV1::new(PlaySoundRecipePayload {
            recipe_id: "pill_consume".to_string(),
            instance_id: 7,
            pos: Some([1, 64, -2]),
            flag: None,
            volume_mul: 0.8,
            pitch_shift: 0.0,
            recipe: sample_recipe(),
        });

        let bytes = payload.to_json_bytes_checked().expect("serialize");
        let back: PlaySoundRecipeEventV1 = serde_json::from_slice(&bytes).expect("deserialize");

        assert_eq!(back.v, AUDIO_EVENT_VERSION);
        assert_eq!(back.payload.recipe_id, "pill_consume");
        assert_eq!(back.payload.recipe.layers.len(), 2);
    }

    #[test]
    fn stop_payload_rejects_zero_instance_id() {
        let payload = StopSoundRecipeEventV1::new(StopSoundRecipePayload {
            instance_id: 0,
            fade_out_ticks: 0,
        });
        assert!(matches!(
            payload.to_json_bytes_checked(),
            Err(AudioEventBuildError::InvalidStopPayload(_))
        ));
    }

    #[test]
    fn ambient_zone_rejects_wrong_version() {
        let payload = AmbientZoneS2c {
            v: AUDIO_EVENT_VERSION + 1,
            zone_name: "spawn".to_string(),
            ambient_recipe_id: "pill_consume".to_string(),
            music_state: "AMBIENT".to_string(),
            is_night: false,
            season: "summer".to_string(),
            tsy_depth: None,
            fade_ticks: 60,
            pos: Some([1, 64, -2]),
            volume_mul: 1.0,
            pitch_shift: 0.0,
            recipe: sample_recipe(),
        };

        assert!(matches!(
            payload.to_json_bytes_checked(),
            Err(AudioEventBuildError::InvalidPlayPayload(error))
                if error.contains("version")
        ));
    }

    #[test]
    fn ambient_zone_rejects_unknown_season() {
        let payload = AmbientZoneS2c {
            v: AUDIO_EVENT_VERSION,
            zone_name: "spawn".to_string(),
            ambient_recipe_id: "pill_consume".to_string(),
            music_state: "AMBIENT".to_string(),
            is_night: false,
            season: "monsoon".to_string(),
            tsy_depth: None,
            fade_ticks: 60,
            pos: Some([1, 64, -2]),
            volume_mul: 1.0,
            pitch_shift: 0.0,
            recipe: sample_recipe(),
        };

        assert!(matches!(
            payload.to_json_bytes_checked(),
            Err(AudioEventBuildError::InvalidPlayPayload(error))
                if error.contains("season")
        ));
    }
}
