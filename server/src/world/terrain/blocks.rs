//! Data-owned, intentionally closed worldgen block catalog.
//!
//! The checked-in TOML contains the historical 213 logical keys as a compatibility
//! baseline, but the declared TOML entries are the sole production allow-list.
//! Startup loads and validates that list once; callers still use [`block_from_name`]
//! and therefore cannot bypass it with an arbitrary vanilla block.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::Deserialize;
use valence::prelude::{BlockKind, BlockState, PropName, PropValue};

pub const DEFAULT_BLOCK_CATALOG_RELATIVE_PATH: &str = "assets/worldgen/block_catalog.toml";
const BLOCK_CATALOG_VERSION: u32 = 1;

static DEFAULT_BLOCK_CATALOG: OnceLock<BlockCatalog> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct BlockCatalog {
    states: HashMap<String, BlockState>,
    #[cfg(test)]
    source_order: Vec<String>,
    #[cfg(test)]
    direct_count: usize,
    #[cfg(test)]
    alias_count: usize,
}

impl BlockCatalog {
    pub fn load(path: &Path) -> Result<Self, BlockCatalogError> {
        let mut file = super::nbt_io::open_regular_file_no_follow(path).map_err(|error| {
            BlockCatalogError::Read {
                path: path.to_path_buf(),
                source: error.to_string(),
            }
        })?;
        let mut text = String::new();
        std::io::Read::read_to_string(&mut file, &mut text).map_err(|error| {
            BlockCatalogError::Read {
                path: path.to_path_buf(),
                source: error.to_string(),
            }
        })?;
        Self::from_toml(&text, path)
    }

    fn from_toml(text: &str, path: &Path) -> Result<Self, BlockCatalogError> {
        let raw: RawBlockCatalog =
            toml::from_str(text).map_err(|error| BlockCatalogError::Parse {
                path: path.to_path_buf(),
                source: error.to_string(),
            })?;
        Self::from_raw(raw, path)
    }

    fn from_raw(raw: RawBlockCatalog, path: &Path) -> Result<Self, BlockCatalogError> {
        let mut diagnostics = Vec::new();
        if raw.version != BLOCK_CATALOG_VERSION {
            diagnostics.push(format!(
                "unsupported catalog version {} (expected {BLOCK_CATALOG_VERSION})",
                raw.version
            ));
        }
        if raw.block.is_empty() {
            diagnostics.push("catalog must contain at least one block".to_string());
        }

        let mut seen = HashSet::with_capacity(raw.block.len());
        let mut source_order = Vec::with_capacity(raw.block.len());
        let mut direct_states = HashMap::with_capacity(raw.block.len());
        let mut alias_entries = Vec::new();
        let mut alias_names = HashSet::new();

        for (index, entry) in raw.block.into_iter().enumerate() {
            let position = index + 1;
            if entry.name.is_empty() {
                diagnostics.push(format!("block #{position} has an empty logical name"));
                continue;
            }
            if entry.name.contains(':') {
                diagnostics.push(format!(
                    "block #{position} logical name '{}' must be bare (namespaces are not allowed)",
                    entry.name
                ));
            }
            if !seen.insert(entry.name.clone()) {
                diagnostics.push(format!(
                    "duplicate logical block name '{}' at block #{position}",
                    entry.name
                ));
                continue;
            }
            source_order.push(entry.name.clone());

            match entry.alias_of {
                Some(target) => {
                    alias_names.insert(entry.name.clone());
                    alias_entries.push((entry.name, target));
                }
                None => match BlockKind::from_str(&entry.name) {
                    Some(kind) => {
                        direct_states.insert(entry.name, kind.to_state());
                    }
                    None => diagnostics.push(format!(
                        "direct logical key '{}' is not a Valence BlockKind",
                        entry.name
                    )),
                },
            }
        }

        let direct_names: HashSet<&str> = direct_states.keys().map(String::as_str).collect();
        let mut states = direct_states.clone();
        for (name, target) in &alias_entries {
            if target.is_empty() {
                diagnostics.push(format!("alias '{name}' has an empty target"));
                continue;
            }
            if target.contains(':') {
                diagnostics.push(format!(
                    "alias '{name}' target '{target}' must be bare (namespaces are not allowed)"
                ));
                continue;
            }
            if name == target {
                diagnostics.push(format!("alias '{name}' cannot target itself"));
                continue;
            }
            if alias_names.contains(target) {
                diagnostics.push(format!(
                    "alias '{name}' target '{target}' is another alias; alias chains are not supported"
                ));
                continue;
            }
            if !direct_names.contains(target.as_str()) {
                diagnostics.push(format!(
                    "alias '{name}' target '{target}' is not declared as a direct catalog key"
                ));
                continue;
            }
            let target_state = direct_states
                .get(target)
                .expect("validated direct catalog target must have a BlockState");
            states.insert(name.clone(), *target_state);
        }

        if states.len() != source_order.len() {
            diagnostics.push(format!(
                "only {} of {} unique catalog entries resolved to BlockState",
                states.len(),
                source_order.len()
            ));
        }

        if !diagnostics.is_empty() {
            diagnostics.sort();
            diagnostics.dedup();
            return Err(BlockCatalogError::Validation {
                path: path.to_path_buf(),
                diagnostics,
            });
        }

        Ok(Self {
            states,
            #[cfg(test)]
            source_order,
            #[cfg(test)]
            direct_count: direct_states.len(),
            #[cfg(test)]
            alias_count: alias_entries.len(),
        })
    }

    pub fn resolve(&self, name: &str) -> Option<BlockState> {
        self.states.get(name).copied()
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.source_order.len()
    }

    #[cfg(test)]
    pub fn source_order(&self) -> &[String] {
        &self.source_order
    }

    #[cfg(test)]
    pub fn direct_count(&self) -> usize {
        self.direct_count
    }

    #[cfg(test)]
    pub fn alias_count(&self) -> usize {
        self.alias_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockCatalogError {
    Read {
        path: PathBuf,
        source: String,
    },
    Parse {
        path: PathBuf,
        source: String,
    },
    Validation {
        path: PathBuf,
        diagnostics: Vec<String>,
    },
}

impl std::fmt::Display for BlockCatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read { path, source } => {
                write!(
                    f,
                    "failed to read block catalog {}: {source}",
                    path.display()
                )
            }
            Self::Parse { path, source } => {
                write!(
                    f,
                    "failed to parse block catalog {}: {source}",
                    path.display()
                )
            }
            Self::Validation { path, diagnostics } => write!(
                f,
                "block catalog {} failed validation:\n- {}",
                path.display(),
                diagnostics.join("\n- ")
            ),
        }
    }
}

impl std::error::Error for BlockCatalogError {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBlockCatalog {
    version: u32,
    block: Vec<RawBlockEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBlockEntry {
    name: String,
    #[serde(default)]
    alias_of: Option<String>,
}

fn default_catalog_path() -> PathBuf {
    crate::body_plan::resolve_assets_root().join(DEFAULT_BLOCK_CATALOG_RELATIVE_PATH)
}

fn load_catalog_once(
    cache: &'static OnceLock<BlockCatalog>,
    path: &Path,
) -> Result<&'static BlockCatalog, BlockCatalogError> {
    if let Some(catalog) = cache.get() {
        return Ok(catalog);
    }

    let catalog = BlockCatalog::load(path)?;
    let _ = cache.set(catalog);
    Ok(cache
        .get()
        .expect("a successful block catalog load must initialize the cache"))
}

fn default_catalog() -> Result<&'static BlockCatalog, BlockCatalogError> {
    load_catalog_once(&DEFAULT_BLOCK_CATALOG, &default_catalog_path())
}

/// Force the default catalog to load during app construction, before any world
/// bootstrap or chunk-generation consumer can resolve a block.
pub fn initialize_default_block_catalog() -> Result<(), BlockCatalogError> {
    default_catalog().map(|_| ())
}

/// Resolve a bare logical worldgen key. Unknown keys and namespaced strings are
/// rejected; this never opens the allow-list to every Valence `BlockKind`.
pub fn block_from_name(name: &str) -> Option<BlockState> {
    default_catalog().ok()?.resolve(name)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockStateResolveError {
    UnsupportedNamespace {
        name: String,
    },
    UnknownBlock {
        name: String,
    },
    UnknownPropertyName {
        block: String,
        property: String,
    },
    UnknownPropertyValue {
        block: String,
        property: String,
        value: String,
    },
    PropertyNotApplicable {
        block: String,
        property: String,
    },
    InvalidPropertyValue {
        block: String,
        property: String,
        value: String,
    },
}

impl std::fmt::Display for BlockStateResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedNamespace { name } => write!(
                f,
                "block '{name}' uses an unsupported namespace (only bare names or one 'minecraft:' prefix are accepted)"
            ),
            Self::UnknownBlock { name } => write!(f, "unknown catalog block '{name}'"),
            Self::UnknownPropertyName { block, property } => {
                write!(f, "block '{block}' has unknown property name '{property}'")
            }
            Self::UnknownPropertyValue {
                block,
                property,
                value,
            } => write!(
                f,
                "block '{block}' property '{property}' has unknown value '{value}'"
            ),
            Self::PropertyNotApplicable { block, property } => write!(
                f,
                "property '{property}' is not applicable to block '{block}'"
            ),
            Self::InvalidPropertyValue {
                block,
                property,
                value,
            } => write!(
                f,
                "value '{value}' is invalid for property '{property}' on block '{block}'"
            ),
        }
    }
}

impl std::error::Error for BlockStateResolveError {}

/// Lower a bare or once-`minecraft:`-prefixed catalog name plus ordered
/// properties. Every property is strict: unknown names, unknown values,
/// inapplicable properties, and values outside that block's property domain all
/// fail instead of being silently dropped.
pub fn block_state_with_properties<'a, I>(
    name: &str,
    properties: I,
) -> Result<BlockState, BlockStateResolveError>
where
    I: IntoIterator<Item = (&'a str, &'a str)>,
{
    let bare = if let Some(stripped) = name.strip_prefix("minecraft:") {
        if stripped.is_empty() || stripped.contains(':') {
            return Err(BlockStateResolveError::UnsupportedNamespace {
                name: name.to_string(),
            });
        }
        stripped
    } else if name.contains(':') {
        return Err(BlockStateResolveError::UnsupportedNamespace {
            name: name.to_string(),
        });
    } else {
        name
    };

    let mut state = block_from_name(bare).ok_or_else(|| BlockStateResolveError::UnknownBlock {
        name: name.to_string(),
    })?;

    for (property, value) in properties {
        let prop_name = PropName::from_str(property).ok_or_else(|| {
            BlockStateResolveError::UnknownPropertyName {
                block: name.to_string(),
                property: property.to_string(),
            }
        })?;
        let prop_value = PropValue::from_str(value).ok_or_else(|| {
            BlockStateResolveError::UnknownPropertyValue {
                block: name.to_string(),
                property: property.to_string(),
                value: value.to_string(),
            }
        })?;
        let current =
            state
                .get(prop_name)
                .ok_or_else(|| BlockStateResolveError::PropertyNotApplicable {
                    block: name.to_string(),
                    property: property.to_string(),
                })?;
        let next = state.set(prop_name, prop_value);
        if next == state && current != prop_value {
            return Err(BlockStateResolveError::InvalidPropertyValue {
                block: name.to_string(),
                property: property.to_string(),
                value: value.to_string(),
            });
        }
        state = next;
    }

    Ok(state)
}

#[cfg(test)]
#[path = "blocks_legacy_oracle.rs"]
mod legacy_oracle;
#[cfg(test)]
#[path = "raster_legacy_oracle.rs"]
mod raster_legacy_oracle;

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    use legacy_oracle::{legacy_block_state, LEGACY_BLOCK_ORACLE};
    use raster_legacy_oracle::LEGACY_RASTER_ORACLE;

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    /// Full list of distinct block names authored in dan_zong and wangyintai NBT
    /// structures. This legacy asset-specific pin remains independent of the new
    /// canonical catalog and catches accidental asset drift in either direction.
    pub const AUTHORED_STRUCTURE_BLOCKS: &[&str] = &[
        "amethyst_block",
        "amethyst_cluster",
        "andesite",
        "birch_pressure_plate",
        "blackstone",
        "bone_block",
        "bookshelf",
        "calcite",
        "campfire",
        "candle",
        "cauldron",
        "chain",
        "chiseled_deepslate",
        "chiseled_polished_blackstone",
        "chiseled_stone_bricks",
        "coal_block",
        "coal_ore",
        "coarse_dirt",
        "cobblestone",
        "cobblestone_wall",
        "cobweb",
        "cracked_deepslate_bricks",
        "cracked_polished_blackstone_bricks",
        "cracked_stone_bricks",
        "dark_oak_planks",
        "dark_oak_slab",
        "dark_oak_stairs",
        "dead_bush",
        "deepslate_brick_slab",
        "deepslate_bricks",
        "flower_pot",
        "gravel",
        "iron_nugget",
        "lectern",
        "mossy_cobblestone",
        "mossy_stone_bricks",
        "oak_fence",
        "oak_log",
        "podzol",
        "polished_blackstone",
        "polished_blackstone_bricks",
        "polished_blackstone_slab",
        "polished_blackstone_stairs",
        "polished_blackstone_wall",
        "polished_deepslate",
        "polished_deepslate_slab",
        "purple_glazed_terracotta",
        "purple_stained_glass",
        "purple_stained_glass_pane",
        "purple_terracotta",
        "red_mushroom",
        "skeleton_skull",
        "smooth_basalt",
        "soul_campfire",
        "soul_lantern",
        "soul_sand",
        "soul_soil",
        "stone_brick_slab",
        "stone_brick_stairs",
        "stone_bricks",
        "vine",
        "water",
        "white_banner",
    ];

    fn default_asset_text() -> String {
        fs::read_to_string(default_catalog_path()).expect("default block catalog must be readable")
    }

    fn temp_path(tag: &str) -> PathBuf {
        let serial = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "bong_block_catalog_{tag}_{}_{}.toml",
            std::process::id(),
            serial
        ))
    }

    fn write_and_load(tag: &str, text: &str) -> Result<BlockCatalog, BlockCatalogError> {
        let path = temp_path(tag);
        fs::write(&path, text).expect("write temporary block catalog");
        let result = BlockCatalog::load(&path);
        let _ = fs::remove_file(path);
        result
    }

    fn validation_diagnostics(error: BlockCatalogError) -> Vec<String> {
        match error {
            BlockCatalogError::Validation { diagnostics, .. } => diagnostics,
            other => panic!("expected validation error, got {other}"),
        }
    }

    fn assert_validation_contains(tag: &str, text: &str, expected: &str) {
        let error = write_and_load(tag, text).expect_err("catalog must fail validation");
        let diagnostics = validation_diagnostics(error);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.contains(expected)),
            "expected diagnostic containing {expected:?}, got {diagnostics:?}"
        );
    }

    #[test]
    fn default_catalog_preserves_legacy_entries_as_an_ordered_compatibility_subset() {
        let catalog = BlockCatalog::load(&default_catalog_path()).expect("default catalog loads");
        let legacy_names = LEGACY_BLOCK_ORACLE
            .iter()
            .map(|(name, _)| *name)
            .collect::<HashSet<_>>();
        let current_legacy_order = catalog
            .source_order()
            .iter()
            .map(String::as_str)
            .filter(|name| legacy_names.contains(name))
            .collect::<Vec<_>>();
        let expected_legacy_order = LEGACY_BLOCK_ORACLE
            .iter()
            .map(|(name, _)| *name)
            .collect::<Vec<_>>();

        assert_eq!(
            current_legacy_order, expected_legacy_order,
            "new data entries may be inserted, but historical entries must retain relative order"
        );
        for (name, target) in LEGACY_BLOCK_ORACLE {
            let actual = catalog
                .resolve(name)
                .unwrap_or_else(|| panic!("catalog must resolve legacy key '{name}'"));
            let expected = legacy_block_state(name)
                .unwrap_or_else(|| panic!("legacy oracle must resolve key '{name}'"));
            assert_eq!(
                actual, expected,
                "catalog key '{name}' -> '{target}' changed its BlockState"
            );
        }
    }

    #[test]
    fn loader_accepts_minimal_dynamic_direct_and_forward_alias_catalogs() {
        let catalog = write_and_load(
            "dynamic",
            r#"
version = 1

[[block]]
name = "custom_stone"
alias_of = "stone"

[[block]]
name = "stone"

[[block]]
name = "gold_block"

[[block]]
name = "second_stone"
alias_of = "stone"
"#,
        )
        .expect("forward aliases and arbitrary valid direct entries must be data-owned");

        assert_eq!(catalog.len(), 4);
        assert_eq!(catalog.direct_count(), 2);
        assert_eq!(catalog.alias_count(), 2);
        assert_eq!(catalog.resolve("stone"), Some(BlockState::STONE));
        assert_eq!(catalog.resolve("custom_stone"), Some(BlockState::STONE));
        assert_eq!(catalog.resolve("second_stone"), Some(BlockState::STONE));
        assert_eq!(
            catalog.resolve("gold_block"),
            BlockKind::from_str("gold_block").map(BlockKind::to_state)
        );
    }

    #[test]
    fn checked_in_catalog_can_be_extended_by_data_only() {
        let mut text = default_asset_text();
        text.push_str(
            r#"

[[block]]
name = "gold_block"

[[block]]
name = "data_only_gold"
alias_of = "gold_block"
"#,
        );
        let catalog = write_and_load("extended", &text)
            .expect("valid direct and alias additions must not require Rust changes");
        let gold = BlockKind::from_str("gold_block")
            .expect("gold_block is a vanilla block")
            .to_state();

        assert_eq!(catalog.resolve("gold_block"), Some(gold));
        assert_eq!(catalog.resolve("data_only_gold"), Some(gold));
        for (name, _) in LEGACY_BLOCK_ORACLE {
            assert!(
                catalog.resolve(name).is_some(),
                "extending the catalog must retain legacy key '{name}'"
            );
        }
    }

    #[test]
    fn a_synthetic_catalog_remains_closed_to_undeclared_vanilla_blocks() {
        let catalog = write_and_load(
            "closed",
            r#"
version = 1

[[block]]
name = "stone"
"#,
        )
        .expect("minimal non-empty catalog loads");

        assert_eq!(catalog.resolve("stone"), Some(BlockState::STONE));
        assert_eq!(
            catalog.resolve("gold_block"),
            None,
            "a valid vanilla block omitted from TOML must remain unavailable"
        );
    }

    #[test]
    fn raster_39_key_oracle_is_a_strict_catalog_subset_with_equal_states() {
        let catalog = BlockCatalog::load(&default_catalog_path()).expect("default catalog loads");
        assert_eq!(LEGACY_RASTER_ORACLE.len(), 39);
        assert!(LEGACY_RASTER_ORACLE.len() < catalog.len());
        for (name, expected) in LEGACY_RASTER_ORACLE {
            assert_eq!(
                catalog.resolve(name),
                Some(*expected),
                "legacy raster fast-path key '{name}' must resolve identically through the catalog"
            );
        }
    }

    #[test]
    fn default_resolver_preserves_historical_aliases_and_rejects_namespaces() {
        initialize_default_block_catalog().expect("default catalog initializes");
        assert_eq!(block_from_name("stone"), Some(BlockState::STONE));
        assert_eq!(block_from_name("glowshroom"), Some(BlockState::SHROOMLIGHT));
        assert_eq!(block_from_name("iron_nugget"), Some(BlockState::AIR));
        assert_eq!(block_from_name("minecraft:stone"), None);
        assert_eq!(block_from_name("not_a_real_block"), None);
    }

    #[test]
    fn all_authored_structure_blocks_resolve() {
        let failures = AUTHORED_STRUCTURE_BLOCKS
            .iter()
            .copied()
            .filter(|name| block_from_name(name).is_none())
            .collect::<Vec<_>>();
        assert!(
            failures.is_empty(),
            "authored NBT blocks missing from the canonical catalog: {failures:?}"
        );
    }

    #[test]
    fn strict_property_lowerer_accepts_bare_and_minecraft_names_and_valid_values() {
        let bare = block_state_with_properties("oak_log", [("axis", "x")])
            .expect("axis=x is valid for oak_log");
        let namespaced = block_state_with_properties(
            "minecraft:stone_brick_stairs",
            [("facing", "south"), ("half", "top")],
        )
        .expect("valid namespaced stair properties lower");
        assert_eq!(bare.get(PropName::Axis), Some(PropValue::X));
        assert_eq!(namespaced.get(PropName::Facing), Some(PropValue::South));
        assert_eq!(namespaced.get(PropName::Half), Some(PropValue::Top));
    }

    #[test]
    fn strict_property_lowerer_rejects_every_invalid_input_class() {
        assert!(matches!(
            block_state_with_properties("minecraft:", []),
            Err(BlockStateResolveError::UnsupportedNamespace { .. })
        ));
        assert!(matches!(
            block_state_with_properties("mod:stone", []),
            Err(BlockStateResolveError::UnsupportedNamespace { .. })
        ));
        assert!(matches!(
            block_state_with_properties("minecraft:minecraft:stone", []),
            Err(BlockStateResolveError::UnsupportedNamespace { .. })
        ));
        assert!(matches!(
            block_state_with_properties("minecraft:gold_block", []),
            Err(BlockStateResolveError::UnknownBlock { .. })
        ));
        assert!(matches!(
            block_state_with_properties("oak_log", [("not_a_property", "x")]),
            Err(BlockStateResolveError::UnknownPropertyName { .. })
        ));
        assert!(matches!(
            block_state_with_properties("oak_log", [("axis", "not_a_value")]),
            Err(BlockStateResolveError::UnknownPropertyValue { .. })
        ));
        assert!(matches!(
            block_state_with_properties("stone", [("axis", "x")]),
            Err(BlockStateResolveError::PropertyNotApplicable { .. })
        ));
        assert!(matches!(
            block_state_with_properties("oak_log", [("axis", "north")]),
            Err(BlockStateResolveError::InvalidPropertyValue { .. })
        ));
    }

    #[test]
    fn failed_default_style_load_does_not_poison_success_cache() {
        static CACHE: OnceLock<BlockCatalog> = OnceLock::new();

        let invalid = temp_path("recover_after_invalid");
        fs::write(&invalid, "version = 1\nblock = []\n").expect("write invalid catalog");
        assert!(matches!(
            load_catalog_once(&CACHE, &invalid),
            Err(BlockCatalogError::Validation { .. })
        ));
        assert!(
            CACHE.get().is_none(),
            "failed startup admission must not be cached permanently"
        );

        let valid = temp_path("recover_with_valid");
        fs::write(&valid, "version = 1\n\n[[block]]\nname = \"stone\"\n")
            .expect("write valid recovery catalog");
        let recovered = load_catalog_once(&CACHE, &valid)
            .expect("a later valid startup admission must recover after a failed attempt");
        assert_eq!(recovered.len(), 1);
        assert_eq!(recovered.resolve("stone"), Some(BlockState::STONE));
        fs::remove_file(&valid).expect("remove admitted catalog path");
        let reused = load_catalog_once(&CACHE, &valid)
            .expect("successful admission must be reused without reopening its path");
        assert!(std::ptr::eq(recovered, reused));
        assert_eq!(reused.resolve("stone"), Some(BlockState::STONE));
        let _ = fs::remove_file(invalid);
    }

    #[test]
    fn loader_rejects_missing_malformed_and_unknown_fields() {
        let missing = temp_path("missing");
        assert!(matches!(
            BlockCatalog::load(&missing),
            Err(BlockCatalogError::Read { .. })
        ));
        assert!(matches!(
            write_and_load("malformed", "version = ["),
            Err(BlockCatalogError::Parse { .. })
        ));

        let root_unknown =
            default_asset_text().replacen("version = 1", "version = 1\nextra = 7", 1);
        assert!(matches!(
            write_and_load("root_unknown", &root_unknown),
            Err(BlockCatalogError::Parse { .. })
        ));
        let entry_unknown =
            default_asset_text().replacen("name = \"stone\"", "name = \"stone\"\nextra = 7", 1);
        assert!(matches!(
            write_and_load("entry_unknown", &entry_unknown),
            Err(BlockCatalogError::Parse { .. })
        ));
    }

    #[test]
    fn catalog_validation_aggregates_and_sorts_simultaneous_errors() {
        let diagnostics = validation_diagnostics(
            write_and_load(
                "aggregate",
                r#"
version = 2
[[block]]
name = "stone"
[[block]]
name = "stone"
[[block]]
name = "alias"
alias_of = "missing"
"#,
            )
            .expect_err("all catalog errors must be reported together"),
        );
        assert!(diagnostics.windows(2).all(|pair| pair[0] <= pair[1]));
        for expected in [
            "unsupported catalog version 2",
            "duplicate logical block name 'stone'",
            "alias 'alias' target 'missing' is not declared",
            "only 1 of 2 unique catalog entries resolved",
        ] {
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.contains(expected)),
                "missing aggregate catalog diagnostic {expected:?}: {diagnostics:?}"
            );
        }
    }

    #[test]
    fn loader_rejects_version_empty_namespaced_and_unknown_direct_entries() {
        assert_validation_contains(
            "version",
            "version = 2\n\n[[block]]\nname = \"stone\"\n",
            "unsupported catalog version 2",
        );
        assert_validation_contains("empty", "version = 1\nblock = []\n", "at least one block");
        assert_validation_contains(
            "empty_name",
            "version = 1\n\n[[block]]\nname = \"\"\n",
            "empty logical name",
        );
        assert_validation_contains(
            "namespaced_name",
            "version = 1\n\n[[block]]\nname = \"minecraft:stone\"\n",
            "must be bare",
        );
        assert_validation_contains(
            "unknown_direct",
            "version = 1\n\n[[block]]\nname = \"not_a_valence_block\"\n",
            "is not a Valence BlockKind",
        );
    }

    #[test]
    fn loader_rejects_duplicates_across_all_entry_kind_pairs() {
        let cases = [
            (
                "direct_direct",
                r#"
version = 1
[[block]]
name = "stone"
[[block]]
name = "stone"
"#,
            ),
            (
                "alias_alias",
                r#"
version = 1
[[block]]
name = "stone"
[[block]]
name = "same"
alias_of = "stone"
[[block]]
name = "same"
alias_of = "stone"
"#,
            ),
            (
                "direct_alias",
                r#"
version = 1
[[block]]
name = "stone"
[[block]]
name = "same"
[[block]]
name = "same"
alias_of = "stone"
"#,
            ),
        ];

        for (tag, text) in cases {
            assert_validation_contains(tag, text, "duplicate logical block name");
        }
    }

    #[test]
    fn loader_rejects_every_invalid_alias_target_class() {
        let cases = [
            (
                "empty_target",
                r#"
version = 1
[[block]]
name = "stone"
[[block]]
name = "alias"
alias_of = ""
"#,
                "empty target",
            ),
            (
                "namespaced_target",
                r#"
version = 1
[[block]]
name = "stone"
[[block]]
name = "alias"
alias_of = "minecraft:stone"
"#,
                "must be bare",
            ),
            (
                "self_target",
                r#"
version = 1
[[block]]
name = "alias"
alias_of = "alias"
"#,
                "cannot target itself",
            ),
            (
                "missing_target",
                r#"
version = 1
[[block]]
name = "alias"
alias_of = "missing"
"#,
                "is not declared as a direct catalog key",
            ),
            (
                "undeclared_vanilla_target",
                r#"
version = 1
[[block]]
name = "stone"
[[block]]
name = "alias"
alias_of = "gold_block"
"#,
                "is not declared as a direct catalog key",
            ),
            (
                "alias_chain",
                r#"
version = 1
[[block]]
name = "stone"
[[block]]
name = "first"
alias_of = "stone"
[[block]]
name = "second"
alias_of = "first"
"#,
                "alias chains are not supported",
            ),
        ];

        for (tag, text, expected) in cases {
            assert_validation_contains(tag, text, expected);
        }
    }
}
