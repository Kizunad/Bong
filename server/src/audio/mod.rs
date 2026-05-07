//! SoundRecipe registry: JSON-defined vanilla sound layers for audio v1.

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
            24,
            "plan §3 MVP recipe list plus narration and locust cues should contain 24 recipes"
        );
        assert!(registry.get("pill_consume").is_some());
        assert!(registry.get("locust_swarm_warning").is_some());
        assert!(registry.get("tribulation_thunder_distant").is_some());
        assert!(registry.get("skill_lv_up").is_some());
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
