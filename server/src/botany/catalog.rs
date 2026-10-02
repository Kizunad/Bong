//! 三端共享的静态植物目录；单株状态仍归各玩法系统管理。
use std::collections::{BTreeMap, HashMap};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use super::registry::{BotanyPlantId, BotanyPlantKind};

pub const CATALOG_JSON: &str = include_str!("../../../shared/botany/plants.json");

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlantDefinition {
    pub id: BotanyPlantId,
    pub name: String,
    pub description: String,
    pub aliases: Vec<String>,
    pub visual: PlantVisual,
    pub wild: Option<BotanyPlantKind>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlantVisual {
    pub tint_rgb: u32,
    pub stages: BTreeMap<String, PlantStageModel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PlantStageModel {
    Billboard {
        texture: String,
        scale: f32,
        offset: [f32; 3],
    },
    Geo {
        geometry: String,
        texture: String,
        scale: f32,
        offset: [f32; 3],
        animation: Option<PlantAnimation>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlantAnimation {
    pub resource: String,
    pub idle: String,
}

#[derive(Debug, Clone)]
pub struct PlantCatalog {
    plants: BTreeMap<String, PlantDefinition>,
    aliases: HashMap<String, String>,
}

impl PlantCatalog {
    /// 内嵌同一份源数据，二进制不依赖开发机目录。数据变更随构建发布。
    pub fn builtin() -> &'static Self {
        static CATALOG: OnceLock<PlantCatalog> = OnceLock::new();
        CATALOG.get_or_init(|| {
            Self::from_json(CATALOG_JSON)
                .unwrap_or_else(|error| panic!("shared/botany/plants.json: {error}"))
        })
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Document {
            schema_version: u32,
            plants: Vec<serde_json::Value>,
        }
        let document: Document = serde_json::from_str(json).map_err(|error| error.to_string())?;
        if document.schema_version != 1 || document.plants.is_empty() {
            return Err("expected schema_version=1 and nonempty plants".into());
        }
        let mut plants = BTreeMap::new();
        let mut identifiers = HashMap::new();
        let mut aliases = HashMap::new();
        for raw in document.plants {
            let label = raw
                .get("id")
                .and_then(|id| id.as_str())
                .unwrap_or("<missing id>");
            let mut plant: PlantDefinition =
                serde_json::from_value(raw.clone()).map_err(|error| format!("{label}: {error}"))?;
            validate_plant(&plant).map_err(|error| format!("{label}: {error}"))?;
            for id in
                std::iter::once(plant.id.as_str()).chain(plant.aliases.iter().map(String::as_str))
            {
                if let Some(previous) = identifiers.insert(id.to_owned(), plant.id.clone()) {
                    return Err(format!(
                        "{label}: identifier `{id}` already belongs to {}",
                        previous.as_str()
                    ));
                }
            }
            for alias in &plant.aliases {
                aliases.insert(alias.clone(), plant.id.as_str().to_owned());
            }
            if let Some(wild) = &mut plant.wild {
                wild.id = plant.id.clone();
            }
            plants.insert(plant.id.as_str().to_owned(), plant);
        }
        Ok(Self { plants, aliases })
    }

    pub fn get(&self, id: &str) -> Option<&PlantDefinition> {
        self.plants.get(id)
    }

    pub fn resolve(&self, id: &str) -> Option<&PlantDefinition> {
        self.get(self.aliases.get(id).map(String::as_str).unwrap_or(id))
    }

    pub fn iter(&self) -> impl Iterator<Item = &PlantDefinition> {
        self.plants.values()
    }
}

pub(super) fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.as_bytes()[0].is_ascii_alphanumeric()
        && id
            .bytes()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || b"_.-".contains(&ch))
}

fn valid_resource(path: &str) -> bool {
    let Some((namespace, path)) = path.split_once(':') else {
        return false;
    };
    valid_id(namespace)
        && !path.is_empty()
        && path
            .split('/')
            .all(|part| valid_id(part) && part != "." && part != "..")
}

fn validate_plant(plant: &PlantDefinition) -> Result<(), String> {
    if !valid_id(plant.id.as_str()) || plant.aliases.iter().any(|id| !valid_id(id)) {
        return Err(
            "id/aliases must use lowercase letters, digits, dots, hyphens or underscores".into(),
        );
    }
    if plant.name.trim().is_empty() || plant.visual.tint_rgb > 0xffffff {
        return Err("name is empty or visual.tint_rgb is outside RGB range".into());
    }
    if !plant.visual.stages.contains_key("mature") {
        return Err("visual.stages.mature is required".into());
    }
    for (stage, model) in &plant.visual.stages {
        if !matches!(stage.as_str(), "seedling" | "growing" | "mature" | "wilted") {
            return Err(format!("visual.stages.{stage}: unknown growth stage"));
        }
        let (texture, scale, offset) = match model {
            PlantStageModel::Billboard {
                texture,
                scale,
                offset,
            } => (texture, scale, offset),
            PlantStageModel::Geo {
                texture,
                scale,
                offset,
                geometry,
                animation,
            } => {
                if !valid_resource(geometry) || !geometry.ends_with(".geo.json") {
                    return Err(format!("visual.stages.{stage}.geometry: invalid resource"));
                }
                if let Some(animation) = animation {
                    if !valid_resource(&animation.resource)
                        || !animation.resource.ends_with(".animation.json")
                        || animation.idle.trim().is_empty()
                    {
                        return Err(format!(
                            "visual.stages.{stage}.animation: invalid resource or idle name"
                        ));
                    }
                }
                (texture, scale, offset)
            }
        };
        if !valid_resource(texture)
            || !texture.ends_with(".png")
            || !scale.is_finite()
            || *scale <= 0.0
            || offset.iter().any(|value| !value.is_finite())
        {
            return Err(format!(
                "visual.stages.{stage}: invalid texture/scale/offset"
            ));
        }
    }
    if let Some(wild) = &plant.wild {
        wild.validate()?;
    }
    Ok(())
}
