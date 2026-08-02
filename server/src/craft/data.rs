//! craft 配方 TOML 数据加载器。
//!
//! P0 仅承载原 `register_examples` 五条与 `register_workbench_recipes` 全量配方。
//! 流派/玩法各自 code-register 的配方仍由所属模块维护，绝不在这里隐式收编。

use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::cultivation::components::{ColorKind, Realm};
use crate::inventory::ItemRegistry;

use super::events::InsightTrigger;
use super::recipe::{
    CraftCategory, CraftRecipe, CraftRequirements, CraftStationKind, RecipeId,
    RecipeValidationError, UnlockSource,
};
use super::registry::CraftRegistry;

/// 配方数据文件相对 server crate 的默认目录。
pub const DEFAULT_CRAFT_RECIPES_DIR: &str = "assets/craft/recipes";
const LEGACY_EXAMPLES_DIR: &str = "legacy";
const WORKBENCH_RECIPES_DIR: &str = "workbench";

/// 服务器 gameplay tick 频率；TOML 的 `time_sec` 在转换时必须无损乘以该值。
const TICKS_PER_SECOND: u64 = 20;

/// 加载失败必须携带来源路径，以及可获得时的 recipe id / 精确字段。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CraftDataError {
    Directory {
        path: PathBuf,
        detail: String,
    },
    EmptyDirectory {
        path: PathBuf,
    },
    NoTomlFiles {
        path: PathBuf,
    },
    Read {
        path: PathBuf,
        detail: String,
    },
    Parse {
        path: PathBuf,
        recipe_id: Option<String>,
        field: String,
        detail: String,
    },
    DuplicateId {
        path: PathBuf,
        recipe_id: String,
        first_path: Option<PathBuf>,
    },
    Conversion {
        path: PathBuf,
        recipe_id: String,
        field: String,
        detail: String,
    },
    MissingItemReferences {
        references: Vec<MissingItemReference>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingItemReference {
    pub path: PathBuf,
    pub recipe_id: String,
    pub field: String,
    pub template_id: String,
}

impl fmt::Display for CraftDataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Directory { path, detail } => write!(
                formatter,
                "craft recipe data error at {} field `directory`: {detail}",
                path.display()
            ),
            Self::EmptyDirectory { path } => write!(
                formatter,
                "craft recipe data error at {} field `directory`: directory is empty",
                path.display()
            ),
            Self::NoTomlFiles { path } => write!(
                formatter,
                "craft recipe data error at {} field `directory`: contains no *.toml files",
                path.display()
            ),
            Self::Read { path, detail } => write!(
                formatter,
                "craft recipe data error at {} field `file`: failed to read TOML: {detail}",
                path.display()
            ),
            Self::Parse {
                path,
                recipe_id,
                field,
                detail,
            } => write_context(formatter, path, recipe_id.as_deref(), field, detail),
            Self::DuplicateId {
                path,
                recipe_id,
                first_path,
            } => {
                write!(
                    formatter,
                    "craft recipe data error at {} recipe `{recipe_id}` field `id`: duplicate id",
                    path.display()
                )?;
                if let Some(first_path) = first_path {
                    write!(formatter, "; first declared in {}", first_path.display())?;
                } else {
                    write!(formatter, "; id already exists in target CraftRegistry")?;
                }
                Ok(())
            }
            Self::Conversion {
                path,
                recipe_id,
                field,
                detail,
            } => write_context(formatter, path, Some(recipe_id), field, detail),
            Self::MissingItemReferences { references } => {
                write!(
                    formatter,
                    "craft recipe data contains {} missing ItemRegistry reference(s)",
                    references.len()
                )?;
                for reference in references {
                    write!(
                        formatter,
                        "\n- {} recipe `{}` field `{}`: ItemRegistry has no template `{}`",
                        reference.path.display(),
                        reference.recipe_id,
                        reference.field,
                        reference.template_id
                    )?;
                }
                Ok(())
            }
        }
    }
}

fn write_context(
    formatter: &mut fmt::Formatter<'_>,
    path: &Path,
    recipe_id: Option<&str>,
    field: &str,
    detail: &str,
) -> fmt::Result {
    match recipe_id {
        Some(recipe_id) if !recipe_id.is_empty() => write!(
            formatter,
            "craft recipe data error at {} recipe `{recipe_id}` field `{field}`: {detail}",
            path.display()
        ),
        _ => write!(
            formatter,
            "craft recipe data error at {} field `{field}`: {detail}",
            path.display()
        ),
    }
}

impl std::error::Error for CraftDataError {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CraftRecipeToml {
    id: String,
    category: String,
    display_name: String,
    materials: Vec<CraftItemStackToml>,
    qi_cost: f64,
    time_sec: u64,
    output: CraftItemStackToml,
    #[serde(default)]
    requirements: CraftRequirementsToml,
    #[serde(default)]
    unlock_sources: Vec<CraftUnlockSourceToml>,
    station: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CraftItemStackToml {
    template_id: String,
    count: u32,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct CraftRequirementsToml {
    #[serde(default)]
    realm_min: Option<String>,
    #[serde(default)]
    qi_color_min: Option<CraftQiColorMinToml>,
    #[serde(default)]
    skill_lv_min: Option<u8>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CraftQiColorMinToml {
    kind: String,
    min_share: f32,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum CraftUnlockSourceToml {
    Scroll { item_template: String },
    Mentor { npc_archetype: String },
    Insight { trigger: String },
}

#[derive(Debug)]
struct LocatedRecipe {
    path: PathBuf,
    recipe: CraftRecipe,
}

/// 从 server 自带的 craft 数据目录加载 P0 配方。
///
/// 调用方必须在 inventory 已注册后传入其 `ItemRegistry`，使材料、产出和配方残卷
/// 的全部引用都在启动期被验证。任何悬挂引用都会由严格生产入口聚合诊断后
/// fail fast；不得用近义 ID 或测试占位物绕过引用完整性。
pub fn load_default_craft_recipes(
    registry: &mut CraftRegistry,
    item_registry: &ItemRegistry,
) -> Result<(), CraftDataError> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join(DEFAULT_CRAFT_RECIPES_DIR);
    load_craft_recipes_from_dir(directory, registry, item_registry)
}

/// 只用于 canonical parity：读出全部 TOML 并绕过 ItemRegistry 引用门，证明
/// 数据搬运逐字段等于旧 registrar。生产代码不得调用。
#[cfg(test)]
fn load_default_craft_recipes_for_parity(
    registry: &mut CraftRegistry,
) -> Result<(), CraftDataError> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join(DEFAULT_CRAFT_RECIPES_DIR);
    load_craft_recipes_from_dir_without_item_validation(directory, registry)
}

/// 仅加载原 `register_examples` 对应的五条 legacy asset。
///
/// 这是测试和单模块使用的窄入口；生产启动使用 [`load_default_craft_recipes`]，
/// 因而两个目录始终一起预检并一次性提交。
pub fn load_legacy_example_recipes(
    registry: &mut CraftRegistry,
    item_registry: &ItemRegistry,
) -> Result<(), CraftDataError> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(DEFAULT_CRAFT_RECIPES_DIR)
        .join(LEGACY_EXAMPLES_DIR);
    load_craft_recipes_from_dir(directory, registry, item_registry)
}

/// 仅加载原 `register_workbench_recipes` 对应的全量 workbench asset。
///
/// 手搓制作台和三个石器仍由 TOML 的 `station = "none"` 直接表达，不保留 ID 特判。
pub fn load_workbench_recipes(
    registry: &mut CraftRegistry,
    item_registry: &ItemRegistry,
) -> Result<(), CraftDataError> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(DEFAULT_CRAFT_RECIPES_DIR)
        .join(WORKBENCH_RECIPES_DIR);
    load_craft_recipes_from_dir(directory, registry, item_registry)
}

/// 递归加载一个 craft 配方目录。
///
/// 过程分为 parse/convert/reference/duplicate 的完整预检阶段和 clone-then-commit 阶段。
/// 因此任一个坏文件或引用错误都会返回错误，且传入 registry 保持原状。
pub fn load_craft_recipes_from_dir(
    directory: impl AsRef<Path>,
    registry: &mut CraftRegistry,
    item_registry: &ItemRegistry,
) -> Result<(), CraftDataError> {
    let staged = stage_craft_recipes(directory.as_ref())?;

    let mut missing_references = Vec::new();
    for located in &staged {
        collect_missing_item_references(
            &located.recipe,
            &located.path,
            item_registry,
            &mut missing_references,
        );
    }
    if !missing_references.is_empty() {
        return Err(CraftDataError::MissingItemReferences {
            references: missing_references,
        });
    }

    commit_staged_recipes(staged, registry)
}

/// Test-only legacy fixture seam. Production must use the strict entry points above.
#[cfg(test)]
pub(crate) fn load_legacy_example_recipes_for_parity(
    registry: &mut CraftRegistry,
) -> Result<(), CraftDataError> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(DEFAULT_CRAFT_RECIPES_DIR)
        .join(LEGACY_EXAMPLES_DIR);
    load_craft_recipes_from_dir_without_item_validation(directory, registry)
}

/// Test-only workbench fixture seam. Production must use the strict entry points above.
#[cfg(test)]
pub(crate) fn load_workbench_recipes_for_parity(
    registry: &mut CraftRegistry,
) -> Result<(), CraftDataError> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(DEFAULT_CRAFT_RECIPES_DIR)
        .join(WORKBENCH_RECIPES_DIR);
    load_craft_recipes_from_dir_without_item_validation(directory, registry)
}

#[cfg(test)]
fn load_craft_recipes_from_dir_without_item_validation(
    directory: impl AsRef<Path>,
    registry: &mut CraftRegistry,
) -> Result<(), CraftDataError> {
    let staged = stage_craft_recipes(directory.as_ref())?;
    commit_staged_recipes(staged, registry)
}

fn stage_craft_recipes(directory: &Path) -> Result<Vec<LocatedRecipe>, CraftDataError> {
    let metadata = fs::symlink_metadata(directory).map_err(|error| CraftDataError::Directory {
        path: directory.to_path_buf(),
        detail: format!("cannot inspect directory: {error}"),
    })?;
    if metadata.file_type().is_symlink() {
        return Err(CraftDataError::Directory {
            path: directory.to_path_buf(),
            detail: "symbolic links are not allowed in craft recipe data".to_string(),
        });
    }
    if !metadata.is_dir() {
        return Err(CraftDataError::Directory {
            path: directory.to_path_buf(),
            detail: "expected a directory".to_string(),
        });
    }

    let mut paths = Vec::new();
    let mut saw_entry = false;
    collect_toml_paths(directory, &mut paths, &mut saw_entry)?;
    if !saw_entry {
        return Err(CraftDataError::EmptyDirectory {
            path: directory.to_path_buf(),
        });
    }
    if paths.is_empty() {
        return Err(CraftDataError::NoTomlFiles {
            path: directory.to_path_buf(),
        });
    }
    paths.sort();

    let mut staged = Vec::new();
    for path in paths {
        staged.extend(parse_recipe_file(&path)?);
    }
    Ok(staged)
}

fn commit_staged_recipes(
    staged: Vec<LocatedRecipe>,
    registry: &mut CraftRegistry,
) -> Result<(), CraftDataError> {
    // 全量预检：重复 id（同一批或既有 registry）在任何注册发生前失败。
    let mut first_paths: HashMap<String, PathBuf> = HashMap::new();
    for located in &staged {
        let id = located.recipe.id.as_str().to_owned();
        if let Some(first_path) = first_paths.insert(id.clone(), located.path.clone()) {
            return Err(CraftDataError::DuplicateId {
                path: located.path.clone(),
                recipe_id: id,
                first_path: Some(first_path),
            });
        }
        if registry.get(&located.recipe.id).is_some() {
            return Err(CraftDataError::DuplicateId {
                path: located.path.clone(),
                recipe_id: id,
                first_path: None,
            });
        }
    }

    let mut candidate = registry.cloned_recipes();
    for located in staged {
        let recipe_id = located.recipe.id.clone();
        if candidate
            .insert(recipe_id.clone(), located.recipe)
            .is_some()
        {
            return Err(CraftDataError::DuplicateId {
                path: located.path,
                recipe_id: recipe_id.as_str().to_owned(),
                first_path: None,
            });
        }
    }
    registry.replace_recipes(candidate);
    Ok(())
}

fn collect_toml_paths(
    directory: &Path,
    paths: &mut Vec<PathBuf>,
    saw_entry: &mut bool,
) -> Result<(), CraftDataError> {
    let entries = fs::read_dir(directory).map_err(|error| CraftDataError::Directory {
        path: directory.to_path_buf(),
        detail: format!("cannot read directory: {error}"),
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| CraftDataError::Directory {
            path: directory.to_path_buf(),
            detail: format!("cannot read directory entry: {error}"),
        })?;
        *saw_entry = true;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| CraftDataError::Directory {
                path: path.clone(),
                detail: format!("cannot inspect directory entry: {error}"),
            })?;
        let is_toml = path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("toml"));
        if file_type.is_symlink() {
            return Err(CraftDataError::Directory {
                path,
                detail: "symbolic links are not allowed in craft recipe data".to_string(),
            });
        }
        if is_toml && !file_type.is_file() {
            return Err(CraftDataError::Directory {
                path,
                detail: "*.toml recipe data must be a regular file".to_string(),
            });
        }
        if file_type.is_dir() {
            collect_toml_paths(&path, paths, saw_entry)?;
        } else if is_toml {
            paths.push(path);
        }
    }
    Ok(())
}

fn parse_recipe_file(path: &Path) -> Result<Vec<LocatedRecipe>, CraftDataError> {
    let content = fs::read_to_string(path).map_err(|error| CraftDataError::Read {
        path: path.to_path_buf(),
        detail: error.to_string(),
    })?;
    let document: toml::Value =
        toml::from_str(&content).map_err(|error| CraftDataError::Parse {
            path: path.to_path_buf(),
            recipe_id: None,
            field: "document".to_string(),
            detail: error.to_string(),
        })?;
    let table = document.as_table().ok_or_else(|| CraftDataError::Parse {
        path: path.to_path_buf(),
        recipe_id: None,
        field: "document".to_string(),
        detail: "expected a TOML table containing [[recipes]] entries".to_string(),
    })?;

    for field in table.keys() {
        if field != "recipes" {
            return Err(CraftDataError::Parse {
                path: path.to_path_buf(),
                recipe_id: None,
                field: field.clone(),
                detail: "unknown top-level field (only `recipes` is allowed)".to_string(),
            });
        }
    }

    let recipes = table
        .get("recipes")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| CraftDataError::Parse {
            path: path.to_path_buf(),
            recipe_id: None,
            field: "recipes".to_string(),
            detail: "expected one or more [[recipes]] tables".to_string(),
        })?;
    if recipes.is_empty() {
        return Err(CraftDataError::Parse {
            path: path.to_path_buf(),
            recipe_id: None,
            field: "recipes".to_string(),
            detail: "must contain at least one recipe".to_string(),
        });
    }

    recipes
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let recipe_id = recipe_id_hint(value);
            let raw: CraftRecipeToml =
                value
                    .clone()
                    .try_into()
                    .map_err(|error| CraftDataError::Parse {
                        path: path.to_path_buf(),
                        recipe_id: recipe_id.clone(),
                        field: format!("recipes[{index}]"),
                        detail: error.to_string(),
                    })?;
            let recipe = convert_recipe(raw, path)?;
            Ok(LocatedRecipe {
                path: path.to_path_buf(),
                recipe,
            })
        })
        .collect()
}

fn recipe_id_hint(value: &toml::Value) -> Option<String> {
    value
        .as_table()
        .and_then(|table| table.get("id"))
        .and_then(toml::Value::as_str)
        .map(ToOwned::to_owned)
}

fn convert_recipe(raw: CraftRecipeToml, path: &Path) -> Result<CraftRecipe, CraftDataError> {
    let recipe_id = raw.id.clone();
    let category = parse_category(&raw.category).ok_or_else(|| {
        conversion_error(
            path,
            &recipe_id,
            "category",
            format!("unknown category `{}`", raw.category),
        )
    })?;
    let station = parse_station(&raw.station).ok_or_else(|| {
        conversion_error(
            path,
            &recipe_id,
            "station",
            format!(
                "unknown station `{}` (expected `none` or `workbench`)",
                raw.station
            ),
        )
    })?;
    let realm_min = raw
        .requirements
        .realm_min
        .as_deref()
        .map(|realm| {
            parse_realm(realm).ok_or_else(|| {
                conversion_error(
                    path,
                    &recipe_id,
                    "requirements.realm_min",
                    format!("unknown realm `{realm}`"),
                )
            })
        })
        .transpose()?;
    let qi_color_min = raw
        .requirements
        .qi_color_min
        .map(|raw_color| {
            let color = parse_color(&raw_color.kind).ok_or_else(|| {
                conversion_error(
                    path,
                    &recipe_id,
                    "requirements.qi_color_min.kind",
                    format!("unknown color kind `{}`", raw_color.kind),
                )
            })?;
            Ok((color, raw_color.min_share))
        })
        .transpose()?;
    let time_ticks = raw.time_sec.checked_mul(TICKS_PER_SECOND).ok_or_else(|| {
        conversion_error(
            path,
            &recipe_id,
            "time_sec",
            format!(
                "{0} seconds overflows ticks when multiplied by {TICKS_PER_SECOND}",
                raw.time_sec
            ),
        )
    })?;

    let unlock_sources = raw
        .unlock_sources
        .into_iter()
        .enumerate()
        .map(|(index, source)| convert_unlock_source(source, path, &recipe_id, index))
        .collect::<Result<Vec<_>, _>>()?;

    let recipe = CraftRecipe {
        id: RecipeId::new(raw.id),
        category,
        display_name: raw.display_name,
        materials: raw
            .materials
            .into_iter()
            .map(|material| (material.template_id, material.count))
            .collect(),
        qi_cost: raw.qi_cost,
        time_ticks,
        output: (raw.output.template_id, raw.output.count),
        requirements: CraftRequirements {
            realm_min,
            qi_color_min,
            skill_lv_min: raw.requirements.skill_lv_min,
        },
        unlock_sources,
        station,
    };

    recipe
        .validate()
        .map_err(|error| CraftDataError::Conversion {
            path: path.to_path_buf(),
            recipe_id: recipe_id.clone(),
            field: validation_field(&error),
            detail: error.to_string(),
        })?;
    Ok(recipe)
}

fn convert_unlock_source(
    source: CraftUnlockSourceToml,
    path: &Path,
    recipe_id: &str,
    index: usize,
) -> Result<UnlockSource, CraftDataError> {
    match source {
        CraftUnlockSourceToml::Scroll { item_template } => {
            Ok(UnlockSource::Scroll { item_template })
        }
        CraftUnlockSourceToml::Mentor { npc_archetype } => {
            if npc_archetype.is_empty() {
                return Err(conversion_error(
                    path,
                    recipe_id,
                    &format!("unlock_sources[{index}].npc_archetype"),
                    "mentor unlock source must be non-empty".to_string(),
                ));
            }
            Ok(UnlockSource::Mentor { npc_archetype })
        }
        CraftUnlockSourceToml::Insight { trigger } => {
            let trigger = parse_insight_trigger(&trigger).ok_or_else(|| {
                conversion_error(
                    path,
                    recipe_id,
                    &format!("unlock_sources[{index}].trigger"),
                    format!("unknown insight trigger `{trigger}`"),
                )
            })?;
            Ok(UnlockSource::Insight { trigger })
        }
    }
}

fn collect_missing_item_references(
    recipe: &CraftRecipe,
    path: &Path,
    item_registry: &ItemRegistry,
    missing: &mut Vec<MissingItemReference>,
) {
    for (index, (template_id, _)) in recipe.materials.iter().enumerate() {
        if item_registry.get(template_id).is_none() {
            missing.push(missing_item_reference(
                path,
                recipe,
                format!("materials[{index}].template_id"),
                template_id,
            ));
        }
    }
    if item_registry.get(&recipe.output.0).is_none() {
        missing.push(missing_item_reference(
            path,
            recipe,
            "output.template_id".to_string(),
            &recipe.output.0,
        ));
    }
    for (index, source) in recipe.unlock_sources.iter().enumerate() {
        if let UnlockSource::Scroll { item_template } = source {
            if item_registry.get(item_template).is_none() {
                missing.push(missing_item_reference(
                    path,
                    recipe,
                    format!("unlock_sources[{index}].item_template"),
                    item_template,
                ));
            }
        }
    }
}

fn missing_item_reference(
    path: &Path,
    recipe: &CraftRecipe,
    field: String,
    template_id: &str,
) -> MissingItemReference {
    MissingItemReference {
        path: path.to_path_buf(),
        recipe_id: recipe.id.as_str().to_owned(),
        field,
        template_id: template_id.to_owned(),
    }
}

fn conversion_error(path: &Path, recipe_id: &str, field: &str, detail: String) -> CraftDataError {
    CraftDataError::Conversion {
        path: path.to_path_buf(),
        recipe_id: recipe_id.to_owned(),
        field: field.to_owned(),
        detail,
    }
}

fn validation_field(error: &RecipeValidationError) -> String {
    match error {
        RecipeValidationError::EmptyId => "id".to_string(),
        RecipeValidationError::NoMaterials { .. } => "materials".to_string(),
        RecipeValidationError::EmptyMaterialTemplate { .. } => {
            "materials[].template_id".to_string()
        }
        RecipeValidationError::ZeroCount { template, .. } => {
            format!("materials[{template}].count")
        }
        RecipeValidationError::EmptyOutputTemplate { .. } => "output.template_id".to_string(),
        RecipeValidationError::ZeroOutputCount { .. } => "output.count".to_string(),
        RecipeValidationError::InvalidQiCost { .. } => "qi_cost".to_string(),
        RecipeValidationError::ZeroTimeTicks { .. } => "time_sec".to_string(),
        RecipeValidationError::NoUnlockSources { .. } => "unlock_sources".to_string(),
        RecipeValidationError::InvalidQiColorMinShare { .. } => {
            "requirements.qi_color_min.min_share".to_string()
        }
        RecipeValidationError::EmptyUnlockSourceTemplate { kind, .. } => match *kind {
            "scroll" => "unlock_sources[].item_template".to_string(),
            "mentor" => "unlock_sources[].npc_archetype".to_string(),
            other => format!("unlock_sources[].{other}"),
        },
    }
}

fn parse_category(raw: &str) -> Option<CraftCategory> {
    match raw {
        "anqi_carrier" => Some(CraftCategory::AnqiCarrier),
        "dugu_potion" => Some(CraftCategory::DuguPotion),
        "tuike_skin" => Some(CraftCategory::TuikeSkin),
        "zhenfa_trap" => Some(CraftCategory::ZhenfaTrap),
        "tool" => Some(CraftCategory::Tool),
        "armor_craft" => Some(CraftCategory::ArmorCraft),
        "container" => Some(CraftCategory::Container),
        "poison_powder" => Some(CraftCategory::PoisonPowder),
        "misc" => Some(CraftCategory::Misc),
        _ => None,
    }
}

fn parse_station(raw: &str) -> Option<Option<CraftStationKind>> {
    match raw {
        "none" => Some(None),
        "workbench" => Some(Some(CraftStationKind::Workbench)),
        _ => None,
    }
}

fn parse_realm(raw: &str) -> Option<Realm> {
    match raw {
        "awaken" => Some(Realm::Awaken),
        "induce" => Some(Realm::Induce),
        "condense" => Some(Realm::Condense),
        "solidify" => Some(Realm::Solidify),
        "spirit" => Some(Realm::Spirit),
        "void" => Some(Realm::Void),
        _ => None,
    }
}

fn parse_color(raw: &str) -> Option<ColorKind> {
    match raw {
        "sharp" => Some(ColorKind::Sharp),
        "heavy" => Some(ColorKind::Heavy),
        "mellow" => Some(ColorKind::Mellow),
        "solid" => Some(ColorKind::Solid),
        "light" => Some(ColorKind::Light),
        "intricate" => Some(ColorKind::Intricate),
        "gentle" => Some(ColorKind::Gentle),
        "insidious" => Some(ColorKind::Insidious),
        "violent" => Some(ColorKind::Violent),
        "turbid" => Some(ColorKind::Turbid),
        _ => None,
    }
}

fn parse_insight_trigger(raw: &str) -> Option<InsightTrigger> {
    match raw {
        "breakthrough" => Some(InsightTrigger::Breakthrough),
        "near_death" => Some(InsightTrigger::NearDeath),
        "defeat_stronger" => Some(InsightTrigger::DefeatStronger),
        _ => None,
    }
}

#[cfg(test)]
#[path = "fixtures/legacy_p0_registrar.rs"]
mod legacy_p0_registrar;

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use serde::Deserialize;

    use super::*;

    static NEXT_TEMP_DIR: AtomicUsize = AtomicUsize::new(0);

    #[derive(Debug, Deserialize, PartialEq)]
    struct BaselineFixture {
        recipes: Vec<BaselineRecipe>,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    struct BaselineRecipe {
        id: String,
        category: String,
        display_name: String,
        materials: Vec<BaselineItemStack>,
        qi_cost: f64,
        time_ticks: u64,
        output: BaselineItemStack,
        requirements: BaselineRequirements,
        unlock_sources: Vec<BaselineUnlockSource>,
        station: String,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    struct BaselineItemStack {
        template_id: String,
        count: u32,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    struct BaselineRequirements {
        realm_min: Option<String>,
        qi_color_min: Option<BaselineQiColorMin>,
        skill_lv_min: Option<u8>,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    struct BaselineQiColorMin {
        kind: String,
        min_share: f32,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    #[serde(tag = "kind", rename_all = "snake_case")]
    enum BaselineUnlockSource {
        Scroll { item_template: String },
        Mentor { npc_archetype: String },
        Insight { trigger: String },
    }

    fn fixture() -> BaselineFixture {
        serde_json::from_str(include_str!(
            "fixtures/registry_datafication_p0_baseline.json"
        ))
        .expect("P0 baseline fixture must stay valid JSON")
    }

    fn item_registry() -> ItemRegistry {
        crate::inventory::load_item_registry()
            .expect("real ItemRegistry must load for craft data tests")
    }

    fn temp_dir(label: &str) -> PathBuf {
        let sequence = NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "bong-craft-data-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("test data directory must be creatable");
        path
    }

    fn write_toml(directory: &Path, name: &str, content: &str) -> PathBuf {
        let path = directory.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("test nested directory must be creatable");
        }
        fs::write(&path, content).expect("test craft TOML must be writable");
        path
    }

    fn clean(path: PathBuf) {
        fs::remove_dir_all(path).expect("test data directory must be removable");
    }

    fn minimal_recipe() -> String {
        r#"[[recipes]]
id = "fixture.recipe"
category = "misc"
display_name = "Fixture"
materials = [{ template_id = "iron_ingot", count = 1 }]
qi_cost = 0.0
time_sec = 1
output = { template_id = "iron_ingot", count = 1 }
station = "none"
"#
        .to_string()
    }

    fn replace_required_line(content: String, original: &str, replacement: &str) -> String {
        assert!(
            content.contains(original),
            "fixture must contain `{original}` before it is overridden"
        );
        content.replacen(original, replacement, 1)
    }

    fn append_recipe_requirements(content: &mut String, lines: &str) {
        content.push_str("[recipes.requirements]\n");
        content.push_str(lines);
        content.push('\n');
    }

    fn canonical(recipe: &CraftRecipe) -> BaselineRecipe {
        BaselineRecipe {
            id: recipe.id.as_str().to_owned(),
            category: recipe.category.as_str().to_owned(),
            display_name: recipe.display_name.clone(),
            materials: recipe
                .materials
                .iter()
                .map(|(template_id, count)| BaselineItemStack {
                    template_id: template_id.clone(),
                    count: *count,
                })
                .collect(),
            qi_cost: recipe.qi_cost,
            time_ticks: recipe.time_ticks,
            output: BaselineItemStack {
                template_id: recipe.output.0.clone(),
                count: recipe.output.1,
            },
            requirements: BaselineRequirements {
                realm_min: recipe
                    .requirements
                    .realm_min
                    .map(realm_name)
                    .map(str::to_owned),
                qi_color_min: recipe.requirements.qi_color_min.map(|(kind, min_share)| {
                    BaselineQiColorMin {
                        kind: color_name(kind).to_owned(),
                        min_share,
                    }
                }),
                skill_lv_min: recipe.requirements.skill_lv_min,
            },
            unlock_sources: recipe
                .unlock_sources
                .iter()
                .map(|source| match source {
                    UnlockSource::Scroll { item_template } => BaselineUnlockSource::Scroll {
                        item_template: item_template.clone(),
                    },
                    UnlockSource::Mentor { npc_archetype } => BaselineUnlockSource::Mentor {
                        npc_archetype: npc_archetype.clone(),
                    },
                    UnlockSource::Insight { trigger } => BaselineUnlockSource::Insight {
                        trigger: trigger.as_str().to_owned(),
                    },
                })
                .collect(),
            station: match recipe.station {
                None => "none".to_string(),
                Some(CraftStationKind::Workbench) => "workbench".to_string(),
            },
        }
    }

    fn realm_name(realm: Realm) -> &'static str {
        match realm {
            Realm::Awaken => "awaken",
            Realm::Induce => "induce",
            Realm::Condense => "condense",
            Realm::Solidify => "solidify",
            Realm::Spirit => "spirit",
            Realm::Void => "void",
        }
    }

    fn color_name(color: ColorKind) -> &'static str {
        match color {
            ColorKind::Sharp => "sharp",
            ColorKind::Heavy => "heavy",
            ColorKind::Mellow => "mellow",
            ColorKind::Solid => "solid",
            ColorKind::Light => "light",
            ColorKind::Intricate => "intricate",
            ColorKind::Gentle => "gentle",
            ColorKind::Insidious => "insidious",
            ColorKind::Violent => "violent",
            ColorKind::Turbid => "turbid",
        }
    }

    fn assert_error_context(error: CraftDataError, path: &Path, id: &str, field: &str) {
        let rendered = error.to_string();
        assert!(
            rendered.contains(&path.display().to_string()),
            "error must retain source path; actual={rendered}"
        );
        assert!(
            rendered.contains(id),
            "error must retain recipe id `{id}`; actual={rendered}"
        );
        assert!(
            rendered.contains(field),
            "error must retain field `{field}`; actual={rendered}"
        );
    }

    fn assert_conversion_error_field(
        error: CraftDataError,
        path: &Path,
        recipe_id: &str,
        field: &str,
    ) {
        match error {
            CraftDataError::Conversion {
                path: actual_path,
                recipe_id: actual_recipe_id,
                field: actual_field,
                ..
            } => {
                assert_eq!(
                    actual_path, path,
                    "conversion error must retain its source file"
                );
                assert_eq!(
                    actual_recipe_id, recipe_id,
                    "conversion error must retain the parsed recipe id"
                );
                assert_eq!(
                    actual_field, field,
                    "conversion error must report the documented schema field path"
                );
            }
            other => panic!("expected CraftDataError::Conversion for `{field}`, got {other:?}"),
        }
    }

    #[test]
    fn default_assets_match_pinned_legacy_registrars_field_for_field() {
        let mut legacy_registry = CraftRegistry::new();
        super::legacy_p0_registrar::register_examples(&mut legacy_registry)
            .expect("pinned legacy example registrar must stay executable");
        super::legacy_p0_registrar::register_workbench_recipes(&mut legacy_registry)
            .expect("pinned legacy workbench registrar must stay executable");
        let mut expected: Vec<_> = legacy_registry.iter().map(canonical).collect();
        expected.sort_by(|left, right| left.id.cmp(&right.id));

        let mut registry = CraftRegistry::new();
        load_default_craft_recipes_for_parity(&mut registry)
            .expect("default P0 craft assets must parse for canonical parity");
        let mut actual: Vec<_> = registry.iter().map(canonical).collect();
        actual.sort_by(|left, right| left.id.cmp(&right.id));

        assert_eq!(
            actual, expected,
            "P0 TOML loader output must equal the pinned pre-migration Rust registrars field-for-field"
        );
        assert_eq!(
            registry.len(),
            legacy_registry.len(),
            "recipe count must derive from the pinned pre-migration registrars"
        );
    }

    #[test]
    fn checked_in_p0_baseline_was_generated_from_pinned_legacy_registrars() {
        let baseline = fixture();
        let mut legacy_registry = CraftRegistry::new();
        super::legacy_p0_registrar::register_examples(&mut legacy_registry)
            .expect("pinned legacy example registrar must stay executable");
        super::legacy_p0_registrar::register_workbench_recipes(&mut legacy_registry)
            .expect("pinned legacy workbench registrar must stay executable");
        let mut generated: Vec<_> = legacy_registry.iter().map(canonical).collect();
        generated.sort_by(|left, right| left.id.cmp(&right.id));

        assert_eq!(
            baseline.recipes, generated,
            "checked-in P0 fixture must remain a one-way canonical dump of commit 6a6a262c's Rust registrars"
        );
    }

    #[test]
    fn default_assets_keep_handcraft_station_and_unlock_semantics() {
        let baseline = fixture();
        let mut registry = CraftRegistry::new();
        load_default_craft_recipes_for_parity(&mut registry).unwrap();
        for expected in &baseline.recipes {
            let actual = registry
                .get(&RecipeId::new(expected.id.clone()))
                .expect("fixture recipe must load");
            assert_eq!(
                &canonical(actual),
                expected,
                "full field contract must survive TOML conversion"
            );
        }
    }

    #[test]
    fn loads_nested_toml_files_without_changing_material_order() {
        let directory = temp_dir("nested-material-order");
        let second = replace_required_line(
            replace_required_line(
                minimal_recipe(),
                "id = \"fixture.recipe\"",
                "id = \"fixture.z\"",
            ),
            "materials = [{ template_id = \"iron_ingot\", count = 1 }]",
            "materials = [{ template_id = \"iron_ingot\", count = 1 }, { template_id = \"iron_needle\", count = 2 }]",
        );
        write_toml(&directory, "z/second.toml", &second);
        let first = replace_required_line(
            minimal_recipe(),
            "id = \"fixture.recipe\"",
            "id = \"fixture.a\"",
        );
        write_toml(&directory, "a/first.toml", &first);

        let mut registry = CraftRegistry::new();
        load_craft_recipes_from_dir(&directory, &mut registry, &item_registry()).unwrap();
        assert_eq!(registry.len(), 2);
        assert_eq!(
            registry.get(&RecipeId::new("fixture.z")).unwrap().materials,
            vec![
                ("iron_ingot".to_string(), 1),
                ("iron_needle".to_string(), 2)
            ],
            "material vector order is an observable craft consumption contract"
        );
        clean(directory);
    }

    #[test]
    fn rejects_missing_directory_non_directory_empty_directory_and_directory_without_toml() {
        let missing = std::env::temp_dir().join(format!(
            "bong-craft-data-missing-{}",
            NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed)
        ));
        let mut registry = CraftRegistry::new();
        let error =
            load_craft_recipes_from_dir(&missing, &mut registry, &item_registry()).unwrap_err();
        assert!(matches!(error, CraftDataError::Directory { .. }));
        assert!(error.to_string().contains("directory"));

        let directory = temp_dir("directory-cases");
        let file = write_toml(&directory, "not_a_directory", "x");
        let error =
            load_craft_recipes_from_dir(&file, &mut registry, &item_registry()).unwrap_err();
        assert!(matches!(error, CraftDataError::Directory { .. }));
        let empty = directory.join("empty");
        fs::create_dir_all(&empty).unwrap();
        let error =
            load_craft_recipes_from_dir(&empty, &mut registry, &item_registry()).unwrap_err();
        assert!(matches!(error, CraftDataError::EmptyDirectory { .. }));
        let no_toml = directory.join("no-toml");
        fs::create_dir_all(&no_toml).unwrap();
        fs::write(no_toml.join("note.txt"), "not recipe data").unwrap();
        let error =
            load_craft_recipes_from_dir(&no_toml, &mut registry, &item_registry()).unwrap_err();
        assert!(matches!(error, CraftDataError::NoTomlFiles { .. }));
        clean(directory);
    }

    #[test]
    fn rejects_directory_named_toml_without_mutating_registry() {
        let directory = temp_dir("directory-toml");
        let directory_toml_path = directory.join("named-directory.toml");
        fs::create_dir_all(&directory_toml_path).unwrap();
        write_toml(&directory_toml_path, "recipe.toml", &minimal_recipe());

        let mut registry = CraftRegistry::new();
        let error =
            load_craft_recipes_from_dir(&directory, &mut registry, &item_registry()).unwrap_err();
        assert!(matches!(error, CraftDataError::Directory { .. }));
        assert!(error.to_string().contains("must be a regular file"));
        assert!(error
            .to_string()
            .contains(&directory_toml_path.display().to_string()));
        assert!(
            registry.is_empty(),
            "a directory named *.toml must fail before nested recipes can commit"
        );
        clean(directory);
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinks_and_non_regular_toml_without_mutating_registry() {
        use std::os::unix::fs::symlink;

        let symlink_directory = temp_dir("symlink");
        write_toml(&symlink_directory, "valid.toml", &minimal_recipe());
        let symlink_target = symlink_directory.join("outside");
        fs::create_dir_all(&symlink_target).unwrap();
        write_toml(&symlink_target, "hidden.toml", &minimal_recipe());
        let symlink_path = symlink_directory.join("linked-recipes");
        symlink(&symlink_target, &symlink_path).unwrap();

        let mut registry = CraftRegistry::new();
        let error =
            load_craft_recipes_from_dir(&symlink_directory, &mut registry, &item_registry())
                .unwrap_err();
        assert!(matches!(error, CraftDataError::Directory { .. }));
        assert!(error.to_string().contains("symbolic links are not allowed"));
        assert!(error
            .to_string()
            .contains(&symlink_path.display().to_string()));
        assert!(
            registry.is_empty(),
            "a symlinked recipe subtree must fail closed before registry commit"
        );

        let symlink_root = temp_dir("symlink-root");
        write_toml(&symlink_root, "recipe.toml", &minimal_recipe());
        let symlink_root_path = symlink_directory.join("recipe-root");
        symlink(&symlink_root, &symlink_root_path).unwrap();
        let mut registry = CraftRegistry::new();
        let error =
            load_craft_recipes_from_dir(&symlink_root_path, &mut registry, &item_registry())
                .unwrap_err();
        assert!(matches!(error, CraftDataError::Directory { .. }));
        assert!(error.to_string().contains("symbolic links are not allowed"));
        assert!(error
            .to_string()
            .contains(&symlink_root_path.display().to_string()));
        assert!(
            registry.is_empty(),
            "a symlinked recipe root must fail closed before registry commit"
        );
        clean(symlink_root);
        clean(symlink_directory);

        let fifo_directory = temp_dir("non-regular-toml");
        write_toml(&fifo_directory, "valid.toml", &minimal_recipe());
        let fifo_path = fifo_directory.join("named-pipe.toml");
        let status = std::process::Command::new("mkfifo")
            .arg(&fifo_path)
            .status()
            .expect("mkfifo must be available for the Unix loader regression test");
        assert!(status.success(), "mkfifo fixture creation must succeed");

        let mut registry = CraftRegistry::new();
        let error = load_craft_recipes_from_dir(&fifo_directory, &mut registry, &item_registry())
            .unwrap_err();
        assert!(matches!(error, CraftDataError::Directory { .. }));
        assert!(error.to_string().contains("must be a regular file"));
        assert!(error.to_string().contains(&fifo_path.display().to_string()));
        assert!(
            registry.is_empty(),
            "a named-pipe *.toml entry must fail before any blocking read or registry commit"
        );
        clean(fifo_directory);
    }

    #[test]
    fn rejects_read_and_document_parse_failures_with_path_and_field() {
        let directory = temp_dir("read-parse");
        let unreadable = write_toml(&directory, "invalid-utf8.toml", "");
        fs::write(&unreadable, [0xff_u8]).unwrap();
        let error =
            load_craft_recipes_from_dir(&directory, &mut CraftRegistry::new(), &item_registry())
                .unwrap_err();
        assert!(matches!(error, CraftDataError::Read { .. }));
        assert!(error
            .to_string()
            .contains(&unreadable.display().to_string()));
        fs::remove_file(&unreadable).unwrap();

        let malformed = write_toml(&directory, "malformed.toml", "[[recipes]\nid = \"broken\"");
        let error =
            load_craft_recipes_from_dir(&directory, &mut CraftRegistry::new(), &item_registry())
                .unwrap_err();
        assert!(matches!(error, CraftDataError::Parse { .. }));
        assert!(error.to_string().contains(&malformed.display().to_string()));
        assert!(error.to_string().contains("document"));
        clean(directory);
    }

    #[test]
    fn rejects_unknown_fields_at_document_recipe_and_nested_levels() {
        let directory = temp_dir("unknown-fields");
        let top = write_toml(&directory, "top.toml", "unknown = true");
        let error =
            load_craft_recipes_from_dir(&directory, &mut CraftRegistry::new(), &item_registry())
                .unwrap_err();
        assert!(error.to_string().contains(&top.display().to_string()));
        assert!(error.to_string().contains("unknown"));
        fs::remove_file(&top).unwrap();

        let mut recipe_content = minimal_recipe();
        recipe_content.push_str("unexpected = true\n");
        let recipe = write_toml(&directory, "recipe.toml", &recipe_content);
        let error =
            load_craft_recipes_from_dir(&directory, &mut CraftRegistry::new(), &item_registry())
                .unwrap_err();
        assert_error_context(error, &recipe, "fixture.recipe", "recipes[0]");
        fs::remove_file(&recipe).unwrap();

        let nested_content = replace_required_line(
            minimal_recipe(),
            "materials = [{ template_id = \"iron_ingot\", count = 1 }]",
            "materials = [{ template_id = \"iron_ingot\", count = 1, unexpected = true }]",
        );
        let nested = write_toml(&directory, "nested.toml", &nested_content);
        let error =
            load_craft_recipes_from_dir(&directory, &mut CraftRegistry::new(), &item_registry())
                .unwrap_err();
        assert_error_context(error, &nested, "fixture.recipe", "recipes[0]");
        clean(directory);
    }

    #[test]
    fn accepts_every_category_station_realm_color_and_unlock_tag() {
        let categories = [
            "anqi_carrier",
            "dugu_potion",
            "tuike_skin",
            "zhenfa_trap",
            "tool",
            "armor_craft",
            "container",
            "poison_powder",
            "misc",
        ];
        let stations = ["none", "workbench"];
        let realms = ["awaken", "induce", "condense", "solidify", "spirit", "void"];
        let colors = [
            "sharp",
            "heavy",
            "mellow",
            "solid",
            "light",
            "intricate",
            "gentle",
            "insidious",
            "violent",
            "turbid",
        ];
        let unlocks = [
            "{ kind = \"mentor\", npc_archetype = \"fixture_mentor\" }",
            "{ kind = \"insight\", trigger = \"breakthrough\" }",
            "{ kind = \"insight\", trigger = \"near_death\" }",
            "{ kind = \"insight\", trigger = \"defeat_stronger\" }",
            "{ kind = \"scroll\", item_template = \"scroll_bronze_coffin\" }",
        ];
        let entry_count = [
            categories.len(),
            stations.len(),
            realms.len(),
            colors.len(),
            unlocks.len(),
        ]
        .into_iter()
        .max()
        .expect("variant dimensions are non-empty");
        let directory = temp_dir("all-tags");
        let mut entries = String::new();
        for index in 0..entry_count {
            entries.push_str(&format!(
                r#"[[recipes]]
id = "fixture.enum.{index}"
category = "{}"
display_name = "Fixture {index}"
materials = [{{ template_id = "iron_ingot", count = 1 }}]
qi_cost = 0.0
time_sec = 1
output = {{ template_id = "iron_ingot", count = 1 }}
station = "{}"
unlock_sources = [{}]
[recipes.requirements]
realm_min = "{}"
qi_color_min = {{ kind = "{}", min_share = 0.5 }}
skill_lv_min = 1

"#,
                categories[index % categories.len()],
                stations[index % stations.len()],
                unlocks[index % unlocks.len()],
                realms[index % realms.len()],
                colors[index % colors.len()],
            ));
        }
        write_toml(&directory, "variants.toml", &entries);
        let mut registry = CraftRegistry::new();
        load_craft_recipes_from_dir(&directory, &mut registry, &item_registry()).unwrap();
        assert_eq!(registry.len(), entry_count);
        for category in CraftCategory::ALL {
            assert!(
                registry.by_category(category).next().is_some(),
                "all CraftCategory variants must deserialize from TOML"
            );
        }
        clean(directory);
    }

    #[test]
    fn rejects_unknown_enum_tags_and_invalid_unlock_payloads_with_context() {
        enum InvalidCase {
            ReplaceLine {
                original: &'static str,
                replacement: &'static str,
            },
            Requirements(&'static str),
        }

        let cases = [
            (
                InvalidCase::ReplaceLine {
                    original: "category = \"misc\"",
                    replacement: "category = \"unknown\"",
                },
                "category",
            ),
            (
                InvalidCase::ReplaceLine {
                    original: "station = \"none\"",
                    replacement: "station = \"forge\"",
                },
                "station",
            ),
            (
                InvalidCase::Requirements("realm_min = \"unknown\""),
                "requirements.realm_min",
            ),
            (
                InvalidCase::Requirements(
                    "qi_color_min = { kind = \"unknown\", min_share = 0.5 }",
                ),
                "requirements.qi_color_min.kind",
            ),
            (
                InvalidCase::ReplaceLine {
                    original: "station = \"none\"",
                    replacement: "unlock_sources = [{ kind = \"unknown\" }]\nstation = \"none\"",
                },
                "recipes[0]",
            ),
            (
                InvalidCase::ReplaceLine {
                    original: "station = \"none\"",
                    replacement: "unlock_sources = [{ kind = \"insight\", trigger = \"unknown\" }]\nstation = \"none\"",
                },
                "unlock_sources[0].trigger",
            ),
            (
                InvalidCase::ReplaceLine {
                    original: "station = \"none\"",
                    replacement: "unlock_sources = [{ kind = \"mentor\", npc_archetype = \"\" }]\nstation = \"none\"",
                },
                "unlock_sources[0].npc_archetype",
            ),
        ];
        for (index, (case, field)) in cases.into_iter().enumerate() {
            let directory = temp_dir("invalid-enum");
            let mut content = minimal_recipe();
            match case {
                InvalidCase::ReplaceLine {
                    original,
                    replacement,
                } => content = replace_required_line(content, original, replacement),
                InvalidCase::Requirements(lines) => append_recipe_requirements(&mut content, lines),
            }
            let path = write_toml(&directory, &format!("case-{index}.toml"), &content);
            let error = load_craft_recipes_from_dir(
                &directory,
                &mut CraftRegistry::new(),
                &item_registry(),
            )
            .unwrap_err();
            assert_error_context(error, &path, "fixture.recipe", field);
            clean(directory);
        }
    }

    #[test]
    fn validation_conversion_errors_report_exact_empty_value_field_paths() {
        let cases = [
            (
                "id = \"fixture.recipe\"",
                "id = \"\"",
                "",
                "id",
            ),
            (
                "materials = [{ template_id = \"iron_ingot\", count = 1 }]",
                "materials = [{ template_id = \"\", count = 1 }]",
                "fixture.recipe",
                "materials[].template_id",
            ),
            (
                "output = { template_id = \"iron_ingot\", count = 1 }",
                "output = { template_id = \"\", count = 1 }",
                "fixture.recipe",
                "output.template_id",
            ),
            (
                "station = \"none\"",
                "unlock_sources = [{ kind = \"scroll\", item_template = \"\" }]\nstation = \"none\"",
                "fixture.recipe",
                "unlock_sources[].item_template",
            ),
        ];

        for (index, (original, replacement, recipe_id, field)) in cases.into_iter().enumerate() {
            let directory = temp_dir("empty-validation-field");
            let path = write_toml(
                &directory,
                &format!("case-{index}.toml"),
                &replace_required_line(minimal_recipe(), original, replacement),
            );
            let error = load_craft_recipes_from_dir(
                &directory,
                &mut CraftRegistry::new(),
                &item_registry(),
            )
            .unwrap_err();
            assert_conversion_error_field(error, &path, recipe_id, field);
            clean(directory);
        }
    }

    #[test]
    fn rejects_checked_time_overflow_and_validation_boundaries() {
        let cases = [
            ("time_sec = 922337203685477581", "time_sec", "time_sec = 1"),
            ("qi_cost = -1.0", "qi_cost", "qi_cost = 0.0"),
            (
                "output = { template_id = \"iron_ingot\", count = 0 }",
                "output.count",
                "output = { template_id = \"iron_ingot\", count = 1 }",
            ),
            (
                "materials = []",
                "materials",
                "materials = [{ template_id = \"iron_ingot\", count = 1 }]",
            ),
            ("time_sec = 0", "time_sec", "time_sec = 1"),
            (
                "[recipes.requirements]\nqi_color_min = { kind = \"sharp\", min_share = 1.1 }",
                "requirements.qi_color_min.min_share",
                "",
            ),
        ];
        for (index, (replacement, field, needle)) in cases.into_iter().enumerate() {
            let directory = temp_dir("validation");
            let mut content = minimal_recipe();
            if needle.is_empty() {
                content.push_str(replacement);
                content.push('\n');
            } else {
                content = content.replace(needle, replacement);
            }
            let path = write_toml(&directory, &format!("case-{index}.toml"), &content);
            let error = load_craft_recipes_from_dir(
                &directory,
                &mut CraftRegistry::new(),
                &item_registry(),
            )
            .unwrap_err();
            assert_error_context(error, &path, "fixture.recipe", field);
            clean(directory);
        }
    }

    #[test]
    fn default_assets_pass_strict_reference_validation_and_commit_atomically() {
        let baseline = fixture();
        let mut registry = CraftRegistry::new();
        registry
            .register(CraftRecipe {
                id: RecipeId::new("existing"),
                category: CraftCategory::Misc,
                display_name: "Existing".to_string(),
                materials: vec![("iron_ingot".to_string(), 1)],
                qi_cost: 0.0,
                time_ticks: 20,
                output: ("iron_ingot".to_string(), 1),
                requirements: CraftRequirements::default(),
                unlock_sources: vec![],
                station: None,
            })
            .unwrap();

        load_default_craft_recipes(&mut registry, &item_registry())
            .expect("default craft assets must pass strict ItemRegistry validation");

        assert!(
            registry.get(&RecipeId::new("existing")).is_some(),
            "successful commit must preserve recipes already present in the target registry"
        );
        assert_eq!(
            registry.len(),
            baseline.recipes.len() + 1,
            "strict default load must atomically add every canonical recipe exactly once"
        );
    }

    #[test]
    fn rejects_missing_material_output_and_scroll_references_before_mutating_registry() {
        let cases = [
            (
                "materials = [{ template_id = \"missing_material\", count = 1 }]",
                "materials[0].template_id",
            ),
            (
                "output = { template_id = \"missing_output\", count = 1 }",
                "output.template_id",
            ),
            (
                "unlock_sources = [{ kind = \"scroll\", item_template = \"missing_scroll\" }]",
                "unlock_sources[0].item_template",
            ),
        ];
        for (index, (replacement, field)) in cases.into_iter().enumerate() {
            let directory = temp_dir("missing-reference");
            let mut content = minimal_recipe();
            if field.starts_with("materials") {
                content = content.replace(
                    "materials = [{ template_id = \"iron_ingot\", count = 1 }]",
                    replacement,
                );
            } else if field.starts_with("output") {
                content = content.replace(
                    "output = { template_id = \"iron_ingot\", count = 1 }",
                    replacement,
                );
            } else {
                content.push_str(replacement);
                content.push('\n');
            }
            let path = write_toml(&directory, &format!("case-{index}.toml"), &content);
            let mut registry = CraftRegistry::new();
            registry
                .register(CraftRecipe {
                    id: RecipeId::new("existing"),
                    category: CraftCategory::Misc,
                    display_name: "Existing".to_string(),
                    materials: vec![("iron_ingot".to_string(), 1)],
                    qi_cost: 0.0,
                    time_ticks: 20,
                    output: ("iron_ingot".to_string(), 1),
                    requirements: CraftRequirements::default(),
                    unlock_sources: vec![],
                    station: None,
                })
                .unwrap();
            let error = load_craft_recipes_from_dir(&directory, &mut registry, &item_registry())
                .unwrap_err();
            assert_error_context(error, &path, "fixture.recipe", field);
            assert_eq!(
                registry.len(),
                1,
                "a failed preflight must leave target registry unchanged"
            );
            clean(directory);
        }
    }

    #[test]
    fn rejects_duplicate_ids_with_both_paths_before_mutating_registry() {
        let directory = temp_dir("duplicate");
        let first = write_toml(&directory, "a/first.toml", &minimal_recipe());
        let second = write_toml(&directory, "b/second.toml", &minimal_recipe());
        let mut registry = CraftRegistry::new();
        let error =
            load_craft_recipes_from_dir(&directory, &mut registry, &item_registry()).unwrap_err();
        let rendered = error.to_string();
        assert!(rendered.contains(&first.display().to_string()));
        assert!(rendered.contains(&second.display().to_string()));
        assert!(rendered.contains("fixture.recipe"));
        assert!(registry.is_empty());
        clean(directory);
    }

    #[test]
    fn rejects_id_already_in_target_registry_without_mutating_existing_entries() {
        let directory = temp_dir("duplicate-existing");
        let path = write_toml(&directory, "recipe.toml", &minimal_recipe());
        let existing = CraftRecipe {
            id: RecipeId::new("fixture.recipe"),
            category: CraftCategory::Tool,
            display_name: "Existing".to_string(),
            materials: vec![("wood_handle".to_string(), 1)],
            qi_cost: 0.0,
            time_ticks: 20,
            output: ("wood_handle".to_string(), 1),
            requirements: CraftRequirements::default(),
            unlock_sources: vec![],
            station: None,
        };
        let mut registry = CraftRegistry::new();
        registry.register(existing.clone()).unwrap();

        let error =
            load_craft_recipes_from_dir(&directory, &mut registry, &item_registry()).unwrap_err();
        assert!(matches!(
            &error,
            CraftDataError::DuplicateId {
                first_path: None,
                ..
            }
        ));
        assert_error_context(error, &path, "fixture.recipe", "id");
        assert_eq!(registry.len(), 1);
        assert_eq!(
            registry.get(&RecipeId::new("fixture.recipe")),
            Some(&existing),
            "duplicate preflight must retain the exact existing recipe"
        );
        clean(directory);
    }
}
