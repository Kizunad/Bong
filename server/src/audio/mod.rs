//! SoundRecipe registry: JSON-defined vanilla sound layers for audio v1.

pub mod ambient;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use valence::prelude::{App, Resource};

use crate::schema::audio::SoundRecipe;

pub const DEFAULT_AUDIO_RECIPES_DIR: &str = "assets/audio/recipes";

pub type RecipeId = String;

#[derive(Debug, Default)]
pub struct SoundRecipeRegistry {
    recipes: HashMap<RecipeId, SoundRecipe>,
}

impl Resource for SoundRecipeRegistry {}

#[derive(Debug)]
pub enum SoundRecipeLoadError {
    Io(std::io::Error),
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
    Duplicate(RecipeId),
    Invalid {
        path: PathBuf,
        recipe_id: RecipeId,
        reason: String,
    },
    Empty(PathBuf),
}

impl std::fmt::Display for SoundRecipeLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "io: {error}"),
            Self::Json { path, source } => write!(f, "json: {}: {source}", path.display()),
            Self::Duplicate(id) => write!(f, "duplicate sound recipe id {id}"),
            Self::Invalid {
                path,
                recipe_id,
                reason,
            } => write!(
                f,
                "invalid sound recipe `{recipe_id}` at {}: {reason}",
                path.display()
            ),
            Self::Empty(path) => write!(
                f,
                "audio recipe directory {} contains no *.json files",
                path.display()
            ),
        }
    }
}

impl std::error::Error for SoundRecipeLoadError {}

impl From<std::io::Error> for SoundRecipeLoadError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl SoundRecipeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, recipe: SoundRecipe) -> Result<(), SoundRecipeLoadError> {
        recipe
            .validate()
            .map_err(|reason| SoundRecipeLoadError::Invalid {
                path: PathBuf::new(),
                recipe_id: recipe.id.clone(),
                reason,
            })?;
        if self.recipes.contains_key(&recipe.id) {
            return Err(SoundRecipeLoadError::Duplicate(recipe.id));
        }
        self.recipes.insert(recipe.id.clone(), recipe);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&SoundRecipe> {
        self.recipes.get(id)
    }

    pub fn len(&self) -> usize {
        self.recipes.len()
    }

    pub fn load_default() -> Result<Self, SoundRecipeLoadError> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(DEFAULT_AUDIO_RECIPES_DIR);
        Self::load_dir(path)
    }

    pub fn load_dir(path: impl AsRef<Path>) -> Result<Self, SoundRecipeLoadError> {
        let dir = path.as_ref();
        let mut json_paths: Vec<PathBuf> = fs::read_dir(dir)?
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                path.extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
                    .then_some(path)
            })
            .collect();
        json_paths.sort();

        if json_paths.is_empty() {
            return Err(SoundRecipeLoadError::Empty(dir.to_path_buf()));
        }

        let mut registry = Self::new();
        for path in json_paths {
            let text = fs::read_to_string(&path)?;
            let recipe: SoundRecipe =
                serde_json::from_str(&text).map_err(|source| SoundRecipeLoadError::Json {
                    path: path.clone(),
                    source,
                })?;
            let id = recipe.id.clone();
            recipe
                .validate()
                .map_err(|reason| SoundRecipeLoadError::Invalid {
                    path: path.clone(),
                    recipe_id: id,
                    reason,
                })?;
            registry.insert(recipe).map_err(|error| match error {
                SoundRecipeLoadError::Invalid {
                    reason, recipe_id, ..
                } => SoundRecipeLoadError::Invalid {
                    path: path.clone(),
                    recipe_id,
                    reason,
                },
                other => other,
            })?;
        }
        Ok(registry)
    }
}

pub fn register(app: &mut App) {
    let registry = SoundRecipeRegistry::load_default().unwrap_or_else(|error| {
        panic!("[bong][audio] failed to load sound recipe registry: {error}");
    });
    tracing::info!(
        "[bong][audio] loaded {} sound recipe(s) from {}",
        registry.len(),
        DEFAULT_AUDIO_RECIPES_DIR
    );
    app.insert_resource(registry);
    ambient::register(app);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_default_audio_recipes() {
        let registry =
            SoundRecipeRegistry::load_default().expect("default audio recipes should load");
        assert_eq!(
            registry.len(),
            105,
            "audio registry should include MVP cues plus JueBi, botany/fauna visual cues, TSY experience, woliu-v2/v3, dugu-v2, baomai-v3, tuike-v2, NPC engagement cues, calamity arsenal cues, audio-world ambient/music loops, armor break cue, movement-v1 action cues, and coffin lifecycle cues"
        );
        assert!(registry.get("coffin_enter").is_some());
        assert!(registry.get("coffin_exit").is_some());
        assert!(registry.get("coffin_ambient").is_some());
        assert!(registry.get("coffin_break").is_some());
        assert!(registry.get("pill_consume").is_some());
        assert!(registry.get("locust_swarm_warning").is_some());
        assert!(registry.get("tribulation_thunder_distant").is_some());
        assert!(registry.get("skill_lv_up").is_some());
        assert!(registry.get("yidao_meridian_repair").is_some());
        assert!(registry.get("zhenmai_parry_thud").is_some());
        assert!(registry.get("zhenmai_neutralize_hiss").is_some());
        assert!(registry.get("zhenmai_shield_hum").is_some());
        assert!(registry.get("zhenmai_sever_crack").is_some());
        assert!(registry.get("vortex_low_hum").is_some());
        assert!(registry.get("vortex_qi_siphon").is_some());
        assert!(registry.get("lingtian_plant_seed").is_some());
        assert!(registry.get("lingtian_drain").is_some());
        assert!(registry.get("don_skin_low_thud").is_some());
        assert!(registry.get("shed_skin_burst").is_some());
        assert!(registry.get("contam_transfer_hum").is_some());
        assert!(registry.get("ground_crack_rumble").is_some());
        assert!(registry.get("pillar_eruption_boom").is_some());
        assert!(registry.get("pressure_collapse_whoosh").is_some());
        assert!(registry.get("aftershock_wind").is_some());
        assert!(registry.get("tsy_race_out_alarm").is_some());
        assert!(registry.get("tsy_collapse_rumble").is_some());
        assert!(registry.get("tsy_extract_success").is_some());
        assert!(registry.get("tsy_search_scrape").is_some());
        assert!(registry.get("fauna_rat_squeal").is_some());
        assert!(registry.get("fauna_rat_death").is_some());
        assert!(registry.get("fauna_fuya_pressure_hum").is_some());
        assert!(registry.get("fauna_fuya_charge").is_some());
        assert!(registry.get("fauna_ash_spider_attack").is_some());
        assert!(registry.get("fauna_hybrid_beast_death").is_some());
        assert!(registry.get("fauna_void_distorted_ambient").is_some());
        assert!(registry.get("dugu_needle_hiss").is_some());
        assert!(registry.get("dugu_self_cure_drink").is_some());
        assert!(registry.get("dugu_curse_cackle").is_some());
        assert!(registry.get("mountain_shake_rumble").is_some());
        assert!(registry.get("blood_burn_sizzle").is_some());
        assert!(registry.get("transcendence_thunder").is_some());
        assert!(registry.get("woliu_vacuum_palm").is_some());
        assert!(registry.get("woliu_vortex_shield").is_some());
        assert!(registry.get("woliu_vacuum_lock").is_some());
        assert!(registry.get("woliu_vortex_resonance").is_some());
        assert!(registry.get("woliu_turbulence_burst").is_some());
        assert!(registry.get("npc_refuse").is_some());
        assert!(registry.get("npc_greeting_cultivator").is_some());
        assert!(registry.get("npc_greeting_commoner").is_some());
        assert!(registry.get("npc_hurt").is_some());
        assert!(registry.get("npc_death").is_some());
        assert!(registry.get("npc_aggro").is_some());
        assert!(registry.get("ambient_spawn_plain").is_some());
        assert!(registry.get("ambient_tsy").is_some());
        assert!(registry.get("combat_music").is_some());
        assert!(registry.get("cultivation_meditate").is_some());
        assert!(registry.get("meridian_open_chime").is_some());
        assert!(registry.get("tribulation_atmosphere").is_some());
        assert!(registry.get("calamity_thunder").is_some());
        assert!(registry.get("calamity_miasma").is_some());
        assert!(registry.get("calamity_meridian_seal").is_some());
        assert!(registry.get("calamity_daoxiang_spawn").is_some());
        assert!(registry.get("calamity_heavenly_fire").is_some());
        assert!(registry.get("calamity_pressure_invert").is_some());
        assert!(registry.get("calamity_all_wither").is_some());
        assert!(registry.get("calamity_realm_collapse").is_some());
        assert!(registry.get("armor_break").is_some());
        assert!(registry.get("movement_dash").is_some());
        assert!(registry.get("movement_slide").is_some());
        assert!(registry.get("movement_double_jump").is_some());
    }

    #[test]
    fn duplicate_id_is_rejected() {
        let recipe = SoundRecipeRegistry::load_default()
            .expect("default recipes should load")
            .get("pill_consume")
            .expect("fixture recipe exists")
            .clone();
        let mut registry = SoundRecipeRegistry::new();
        registry.insert(recipe.clone()).expect("first insert ok");
        assert!(matches!(
            registry.insert(recipe),
            Err(SoundRecipeLoadError::Duplicate(id)) if id == "pill_consume"
        ));
    }
}
