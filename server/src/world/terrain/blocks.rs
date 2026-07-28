//! Data-owned, intentionally closed worldgen block catalog.
//!
//! The checked-in TOML preserves the historical 213 logical keys. Production
//! startup loads and validates it once; callers still use [`block_from_name`]
//! and therefore cannot bypass the catalog with an arbitrary vanilla block.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::Deserialize;
use valence::prelude::{BlockKind, BlockState, PropName, PropValue};

pub const DEFAULT_BLOCK_CATALOG_RELATIVE_PATH: &str = "assets/worldgen/block_catalog.toml";
const BLOCK_CATALOG_VERSION: u32 = 1;
const CANONICAL_BLOCK_COUNT: usize = 213;
const CANONICAL_DIRECT_COUNT: usize = 211;
const CANONICAL_ALIAS_COUNT: usize = 2;
const CANONICAL_KEY_SET_FINGERPRINT: u64 = 0xa83c_4d95_f677_9648;
const CANONICAL_ALIASES: [(&str, &str); CANONICAL_ALIAS_COUNT] =
    [("glowshroom", "shroomlight"), ("iron_nugget", "air")];

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
        let text = std::fs::read_to_string(path).map_err(|error| BlockCatalogError::Read {
            path: path.to_path_buf(),
            source: error.to_string(),
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
        if raw.block.len() != CANONICAL_BLOCK_COUNT {
            diagnostics.push(format!(
                "catalog contains {} logical keys (expected exactly {CANONICAL_BLOCK_COUNT})",
                raw.block.len()
            ));
        }

        let mut seen = HashSet::with_capacity(raw.block.len());
        let mut source_order = Vec::with_capacity(raw.block.len());
        let mut states = HashMap::with_capacity(raw.block.len());
        let mut aliases = Vec::new();
        let mut direct_count = 0;

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
                    aliases.push((entry.name.clone(), target.clone()));
                    match BlockKind::from_str(&target) {
                        Some(kind) => {
                            states.insert(entry.name, kind.to_state());
                        }
                        None => diagnostics.push(format!(
                            "alias '{}' targets unknown vanilla block '{target}'",
                            entry.name
                        )),
                    }
                }
                None => {
                    direct_count += 1;
                    match BlockKind::from_str(&entry.name) {
                        Some(kind) => {
                            states.insert(entry.name, kind.to_state());
                        }
                        None => diagnostics.push(format!(
                            "direct logical key '{}' is not a Valence BlockKind",
                            entry.name
                        )),
                    }
                }
            }
        }

        if direct_count != CANONICAL_DIRECT_COUNT {
            diagnostics.push(format!(
                "catalog contains {direct_count} direct entries (expected exactly {CANONICAL_DIRECT_COUNT})"
            ));
        }
        if aliases.len() != CANONICAL_ALIAS_COUNT {
            diagnostics.push(format!(
                "catalog contains {} aliases (expected exactly {CANONICAL_ALIAS_COUNT})",
                aliases.len()
            ));
        }

        let actual_aliases: HashSet<(&str, &str)> = aliases
            .iter()
            .map(|(name, target)| (name.as_str(), target.as_str()))
            .collect();
        let expected_aliases: HashSet<(&str, &str)> = CANONICAL_ALIASES.into_iter().collect();
        for (name, target) in expected_aliases.difference(&actual_aliases) {
            diagnostics.push(format!("missing required alias '{name}' -> '{target}'"));
        }
        for (name, target) in actual_aliases.difference(&expected_aliases) {
            diagnostics.push(format!("unexpected alias '{name}' -> '{target}'"));
        }
        for (name, target) in &aliases {
            if !seen.contains(target) {
                diagnostics.push(format!(
                    "alias '{name}' target '{target}' is not itself declared as a direct catalog key"
                ));
            }
        }

        let actual_key_set_fingerprint = canonical_key_set_fingerprint(&seen);
        if actual_key_set_fingerprint != CANONICAL_KEY_SET_FINGERPRINT {
            diagnostics.push(format!(
                "catalog logical key set fingerprint is {actual_key_set_fingerprint:#018x} (expected {CANONICAL_KEY_SET_FINGERPRINT:#018x})"
            ));
        }

        if states.len() != raw_len_without_duplicates(&source_order) {
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
            direct_count,
            #[cfg(test)]
            alias_count: aliases.len(),
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

fn canonical_key_set_fingerprint(keys: &HashSet<String>) -> u64 {
    let mut keys = keys.iter().map(String::as_str).collect::<Vec<_>>();
    keys.sort_unstable();
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for key in keys {
        for byte in key.bytes().chain(std::iter::once(0)) {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    hash
}

fn raw_len_without_duplicates(source_order: &[String]) -> usize {
    source_order.len()
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

    #[test]
    fn default_catalog_matches_pinned_legacy_table_field_for_field() {
        let catalog = BlockCatalog::load(&default_catalog_path()).expect("default catalog loads");
        assert_eq!(catalog.len(), CANONICAL_BLOCK_COUNT);
        assert_eq!(catalog.direct_count(), CANONICAL_DIRECT_COUNT);
        assert_eq!(catalog.alias_count(), CANONICAL_ALIAS_COUNT);
        assert_eq!(
            catalog.source_order(),
            &LEGACY_BLOCK_ORACLE
                .iter()
                .map(|(name, _)| (*name).to_string())
                .collect::<Vec<_>>(),
            "catalog source order is a runtime contract and must match the pre-migration table"
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
    fn default_resolver_is_closed_bare_only_and_preserves_two_aliases() {
        initialize_default_block_catalog().expect("default catalog initializes");
        assert_eq!(block_from_name("stone"), Some(BlockState::STONE));
        assert_eq!(block_from_name("glowshroom"), Some(BlockState::SHROOMLIGHT));
        assert_eq!(block_from_name("iron_nugget"), Some(BlockState::AIR));
        assert_eq!(block_from_name("minecraft:stone"), None);
        assert_eq!(block_from_name("gold_block"), None);
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

        let missing = temp_path("recover_after_missing");
        assert!(matches!(
            load_catalog_once(&CACHE, &missing),
            Err(BlockCatalogError::Read { .. })
        ));
        assert!(
            CACHE.get().is_none(),
            "failed startup admission must not be cached permanently"
        );

        let valid = temp_path("recover_with_valid");
        fs::write(&valid, default_asset_text()).expect("write valid recovery catalog");
        let recovered = load_catalog_once(&CACHE, &valid)
            .expect("a later valid startup admission must recover after a failed attempt");
        assert_eq!(recovered.len(), CANONICAL_BLOCK_COUNT);
        assert!(CACHE.get().is_some());
        let _ = fs::remove_file(valid);
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
    fn loader_rejects_version_count_duplicate_direct_and_alias_drift() {
        let version = default_asset_text().replacen("version = 1", "version = 2", 1);
        assert!(matches!(
            write_and_load("version", &version),
            Err(BlockCatalogError::Validation { .. })
        ));

        let text = default_asset_text();
        let truncated = text
            .rsplit_once("[[block]]")
            .expect("catalog has entries")
            .0;
        assert!(matches!(
            write_and_load("count", truncated),
            Err(BlockCatalogError::Validation { .. })
        ));

        let duplicate = text.replacen("name = \"smooth_stone\"", "name = \"stone\"", 1);
        assert!(matches!(
            write_and_load("duplicate", &duplicate),
            Err(BlockCatalogError::Validation { .. })
        ));

        let valid_replacement =
            text.replacen("name = \"smooth_stone\"", "name = \"gold_block\"", 1);
        assert!(matches!(
            write_and_load("valid_replacement", &valid_replacement),
            Err(BlockCatalogError::Validation { .. })
        ));

        let invalid_direct = text.replacen(
            "name = \"smooth_stone\"",
            "name = \"not_a_valence_block\"",
            1,
        );
        assert!(matches!(
            write_and_load("invalid_direct", &invalid_direct),
            Err(BlockCatalogError::Validation { .. })
        ));

        let alias_drift = text.replacen(
            "name = \"glowshroom\"\nalias_of = \"shroomlight\"",
            "name = \"glowshroom\"\nalias_of = \"stone\"",
            1,
        );
        assert!(matches!(
            write_and_load("alias_drift", &alias_drift),
            Err(BlockCatalogError::Validation { .. })
        ));
    }
}
