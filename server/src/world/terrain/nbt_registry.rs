//! Runtime NBT decoration template registry (plan-worldgen-v4 P6 §8.1 #10).
//!
//! The §8.1 #10 performance contract:
//!
//! * **Full residency** — every authored decoration / structure `.nbt` template
//!   under `server/structures/decorations/**` is gzip-decompressed exactly once,
//!   at server startup, and the resulting [`StructureNbt`] is held in memory for
//!   the whole process lifetime (the 11 existing P5 assets are ~4.3 MB
//!   decompressed; the decoration increment budget is ≤32 MB).
//! * **memcpy-level stamp** — placing a decoration during chunk generation must
//!   never gzip-decompress on the hot path (the 4 MB `dan_zong_great_hall`
//!   decompresses in ~50 ms, which alone blows the 30 ms single-chunk budget).
//!   [`DecorationNbtRegistry::stamp`] only walks the already-resident block list
//!   and lowers each palette entry to a runtime [`BlockState`] — no IO, no
//!   inflate.
//! * **gzip on disk** — assets are gzip-mandatory ([`super::nbt_io`] rejects
//!   bare NBT). So the lifecycle is: gzip on disk → decompress once at startup →
//!   memcpy at runtime.
//!
//! This is the runtime consumer of the front-loaded [`super::nbt_io`] capability
//! and the [`crate::cmd::dev::gallery::structure_placements`] lowering helper.
//! Stage 1 (this module) locks the schema + registry + stamp API; the flora /
//! structures wiring that *calls* `stamp` lands in a later stage. Until then the
//! public surface has no in-binary caller, so it is `allow(dead_code)` —
//! mirroring the `#[allow(dead_code)] mod schema` convention in `main.rs`. The
//! test suite below exercises every public entry point regardless.
#![allow(dead_code)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use valence::prelude::{BlockPos, Resource};

use super::nbt_io::{self, StructureNbt};
use crate::cmd::dev::gallery::{structure_placements, StampPlacement};

/// Sub-directory (relative to `server/structures/`) that holds decoration
/// templates. Kept distinct from the existing `dan_zong/` and `wangyintai/`
/// large-layout structures so decoration loading never sweeps those in.
pub const DECORATIONS_SUBDIR: &str = "decorations";

/// How a stamped NBT template is positioned relative to the column surface.
///
/// Mirrors the Python `DECORATION_ANCHORS` literal in
/// `worldgen/scripts/terrain_gen/profiles/base.py`. The manifest carries the
/// lowercase string form; [`DecorationAnchor::from_manifest`] parses it (unknown
/// / empty → [`DecorationAnchor::Ground`], the backward-compatible default so an
/// old manifest with no `anchor` field keeps placing on the surface).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DecorationAnchor {
    /// Stamp origin sits on the ground surface (the surface column `top_y + 1`).
    /// The default for every procedural / legacy spec.
    #[default]
    Ground,
    /// Stamp sinks one block into the surface (e.g. a grave-mound dome whose
    /// base replaces the top soil block). The surface column is the dome top.
    Embedded,
    /// Stamp hangs *below* a sky-isle underside (the segment `bottom_y - 1`),
    /// growing downward (hanging crystals / vines). The placement Y is the
    /// highest block of the template; lower template rows hang further down.
    Hanging,
}

impl DecorationAnchor {
    /// Parse the manifest string form. Unknown or empty → [`Ground`] so old
    /// manifests (no `anchor` key) deserialize into the surface-placement path
    /// rather than panicking.
    ///
    /// [`Ground`]: DecorationAnchor::Ground
    pub fn from_manifest(raw: &str) -> Self {
        match raw {
            "embedded" => DecorationAnchor::Embedded,
            "hanging" => DecorationAnchor::Hanging,
            // "ground", "" and any unrecognised value map to the safe default.
            _ => DecorationAnchor::Ground,
        }
    }

    /// The canonical lowercase manifest string for this variant (inverse of
    /// [`DecorationAnchor::from_manifest`] over the three real variants).
    pub fn as_manifest(self) -> &'static str {
        match self {
            DecorationAnchor::Ground => "ground",
            DecorationAnchor::Embedded => "embedded",
            DecorationAnchor::Hanging => "hanging",
        }
    }
}

/// A quarter-turn rotation about the world Y axis applied to a stamp, so a
/// single authored template (e.g. a fallen log lying along +X) can be placed in
/// four orientations deterministically without authoring four `.nbt` files.
///
/// Rotation is applied to block **positions** about the template's local origin
/// (the structure's own `[0,0,0]` corner). Block-state `facing` properties are
/// *not* rotated by Stage 1 — that lives with the placement wiring that knows
/// each decoration's directional semantics; templates whose look does not depend
/// on a facing property (logs, boulders, mounds) rotate correctly with positions
/// alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Rotation {
    /// No rotation — positions stamped verbatim.
    #[default]
    None,
    /// 90° clockwise (viewed from above): `(x, z) -> (-z, x)`.
    Cw90,
    /// 180°: `(x, z) -> (-x, -z)`.
    Cw180,
    /// 270° clockwise / 90° counter-clockwise: `(x, z) -> (z, -x)`.
    Cw270,
}

impl Rotation {
    /// All four variants, in turn order. Useful for picking one by index.
    pub const ALL: [Rotation; 4] = [
        Rotation::None,
        Rotation::Cw90,
        Rotation::Cw180,
        Rotation::Cw270,
    ];

    /// Pick a rotation deterministically from a hash value (`hash % 4`).
    pub fn from_index(index: u32) -> Self {
        Self::ALL[(index % 4) as usize]
    }

    /// Rotate a template-local `(dx, dy, dz)` offset about the Y axis. `dy` is
    /// unchanged; the horizontal plane turns clockwise viewed from above.
    pub fn apply(self, dx: i32, dy: i32, dz: i32) -> (i32, i32, i32) {
        match self {
            Rotation::None => (dx, dy, dz),
            Rotation::Cw90 => (-dz, dy, dx),
            Rotation::Cw180 => (-dx, dy, -dz),
            Rotation::Cw270 => (dz, dy, -dx),
        }
    }
}

/// Errors surfaced when loading the decoration template registry.
#[derive(Debug)]
pub enum RegistryError {
    /// A specific template file failed to read / decompress / parse. Carries
    /// the relative template id and the underlying NBT IO error string.
    Template { id: String, source: String },
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegistryError::Template { id, source } => {
                write!(f, "decoration template '{id}' failed to load: {source}")
            }
        }
    }
}

impl std::error::Error for RegistryError {}

/// In-memory store of every decoration NBT template, decompressed once at
/// startup and held for the process lifetime so runtime stamps are memcpy-level.
///
/// Templates are keyed by their **relative path** under `server/structures/`
/// (forward-slash normalised, e.g. `decorations/tree/small_tree_v1.nbt`) — the
/// exact string a [`super::raster::Decoration::nbt_templates`] entry carries, so
/// a placement looks up `registry.get(&deco.nbt_templates[i])` with no extra
/// path munging.
#[derive(Debug, Default)]
pub struct DecorationNbtRegistry {
    templates: HashMap<String, StructureNbt>,
}

impl Resource for DecorationNbtRegistry {}

impl DecorationNbtRegistry {
    /// An empty registry (no templates). Used when no `decorations/` directory
    /// exists yet (the asset directory is authored in a later P6 stage) — the
    /// server must boot fine with zero templates and every NBT-driven decoration
    /// silently falls back to its procedural path.
    pub fn empty() -> Self {
        Self::default()
    }

    /// The compile-time `server/structures` directory (mirrors the `/gallery`
    /// `structures_root`). The production load location for decoration templates.
    pub fn default_structures_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("structures")
    }

    /// Load from the default `server/structures/decorations/**` location.
    ///
    /// On a malformed asset this logs the error and returns an **empty** registry
    /// rather than propagating — startup must not panic just because one
    /// decoration template is corrupt (every NBT-driven decoration then falls
    /// back to its procedural path). The hard-error form is [`load`] for callers
    /// that want to fail loudly (e.g. tests / asset-build validation).
    ///
    /// [`load`]: DecorationNbtRegistry::load
    pub fn load_default() -> Self {
        let dir = Self::default_structures_dir();
        match Self::load(&dir) {
            Ok(reg) => reg,
            Err(error) => {
                tracing::error!(
                    "[bong][world] decoration NBT registry failed to load from {}: {error}; \
                     continuing with an empty registry (NBT-driven decorations fall back to \
                     procedural placement)",
                    dir.display()
                );
                Self::empty()
            }
        }
    }

    /// Scan `<structures_dir>/decorations/**/*.nbt`, decompress each once, and
    /// build the resident registry. Keys are relative paths under
    /// `structures_dir` (so they start with `decorations/`).
    ///
    /// **Graceful on a missing / empty directory** — returns an empty registry
    /// (never errors, never panics) when `decorations/` is absent or holds no
    /// `.nbt` files, so the binary boots before any decoration asset is authored.
    /// A *present but malformed* file is a hard error (a corrupt asset must trip
    /// loudly, not be silently dropped from placement).
    pub fn load(structures_dir: &Path) -> Result<Self, RegistryError> {
        let deco_dir = structures_dir.join(DECORATIONS_SUBDIR);
        let mut paths: Vec<PathBuf> = Vec::new();
        collect_nbt_files(&deco_dir, &mut paths);
        // Deterministic load order so logging / iteration is stable across runs.
        paths.sort();

        let mut templates = HashMap::with_capacity(paths.len());
        for path in paths {
            let id = relative_template_id(structures_dir, &path);
            let structure =
                nbt_io::read_structure_nbt(&path).map_err(|e| RegistryError::Template {
                    id: id.clone(),
                    source: e.to_string(),
                })?;
            templates.insert(id, structure);
        }
        Ok(Self { templates })
    }

    /// Number of resident templates.
    pub fn len(&self) -> usize {
        self.templates.len()
    }

    /// Whether the registry holds no templates (e.g. asset dir not authored yet).
    pub fn is_empty(&self) -> bool {
        self.templates.is_empty()
    }

    /// Look up a resident template by its relative id (e.g.
    /// `"decorations/tree/small_tree_v1.nbt"`). `None` when no such template was
    /// loaded — callers fall back to the procedural placement path.
    pub fn get(&self, template_id: &str) -> Option<&StructureNbt> {
        self.templates.get(template_id)
    }

    /// Whether a template id resolves to a resident template.
    pub fn contains(&self, template_id: &str) -> bool {
        self.templates.contains_key(template_id)
    }

    /// Iterate `(template_id, &StructureNbt)` over every resident template.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &StructureNbt)> {
        self.templates.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Compute the world placements for stamping `template_id` so its anchor
    /// reference point lands at `surface_pos`, applying `rotation` about the Y
    /// axis. **memcpy-level**: this only walks the resident block list and lowers
    /// palette entries — no IO, no gzip inflate (the §8.1 #10 hot-path contract).
    ///
    /// Returns `(placements, unresolved)`:
    /// * `placements` — `(world_pos, block_state, block_entity_nbt)` triples to
    ///   write into the chunk (the same [`StampPlacement`] shape `/gallery` uses).
    /// * `unresolved` — palette block names that did not resolve via
    ///   `block_from_name` (skipped, not written as holes); callers should warn.
    ///
    /// `None` when `template_id` is not resident (caller falls back to procedural).
    ///
    /// Anchor semantics relative to `surface_pos` (the column surface block):
    /// * [`DecorationAnchor::Ground`] — template `[0,0,0]` sits at `surface_pos`
    ///   `+ (0, 1, 0)` (one block above the surface block, i.e. on top of it).
    /// * [`DecorationAnchor::Embedded`] — template `[0,0,0]` sits *at*
    ///   `surface_pos` (sinks one block, replacing the surface block).
    /// * [`DecorationAnchor::Hanging`] — the template's *top* row aligns to
    ///   `surface_pos - (0,1,0)` (one block below the underside `surface_pos`),
    ///   so the structure grows downward from there.
    pub fn stamp(
        &self,
        template_id: &str,
        surface_pos: BlockPos,
        anchor: DecorationAnchor,
        rotation: Rotation,
    ) -> Option<(Vec<StampPlacement>, Vec<String>)> {
        let structure = self.templates.get(template_id)?;
        // Anchor sets the world Y the template's reference plane maps to.
        let y_offset = match anchor {
            DecorationAnchor::Ground => 1,
            DecorationAnchor::Embedded => 0,
            // For Hanging, the template's *top* (size_y - 1) row sits at
            // surface_pos.y - 1, so template y=0 lands further below.
            DecorationAnchor::Hanging => -(structure.size[1].max(1)),
        };
        let base_origin = [surface_pos.x, surface_pos.y + y_offset, surface_pos.z];

        if rotation == Rotation::None {
            // Fast path: stamp positions verbatim from base_origin.
            return Some(structure_placements(structure, base_origin));
        }

        // Rotated path: rotate each block's template-local offset about Y, then
        // place at base_origin. We rebuild placements directly (rather than via
        // structure_placements at one origin) because the rotation is per-block.
        let unresolved = structure.unresolved_palette_blocks();
        let mut placements = Vec::with_capacity(structure.blocks.len());
        for block in &structure.blocks {
            let Some(entry) = structure.palette.get(block.state as usize) else {
                continue;
            };
            let Some(state) = entry.block_state() else {
                continue;
            };
            let (rx, ry, rz) = rotation.apply(block.pos[0], block.pos[1], block.pos[2]);
            let pos = BlockPos::new(
                base_origin[0] + rx,
                base_origin[1] + ry,
                base_origin[2] + rz,
            );
            placements.push((pos, state, block.block_nbt.clone()));
        }
        Some((placements, unresolved))
    }
}

/// Recursively collect every `*.nbt` file under `dir` into `out`. Missing dir is
/// a silent no-op (graceful empty-directory handling).
fn collect_nbt_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_nbt_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("nbt") {
            out.push(path);
        }
    }
}

/// The forward-slash relative id of `path` under `base` (e.g.
/// `decorations/tree/small_tree_v1.nbt`). Falls back to the file name when the
/// path is not under `base` (shouldn't happen for scanned files).
fn relative_template_id(base: &Path, path: &Path) -> String {
    let rel = path.strip_prefix(base).unwrap_or(path);
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::terrain::nbt_io::{
        write_structure_nbt, PaletteEntry, StructureBlockEntry, StructureNbt, DATA_VERSION,
    };
    use std::fs;

    // ── helpers ─────────────────────────────────────────────────────────────

    /// A tiny but non-trivial structure: a 3×1×1 row of distinct blocks at
    /// template-local x=0,1,2. Distinct blocks so a rotated stamp's per-position
    /// mapping is observable (not all-same).
    fn row_structure() -> StructureNbt {
        StructureNbt {
            data_version: DATA_VERSION,
            size: [3, 1, 1],
            palette: vec![
                PaletteEntry {
                    name: "minecraft:stone".into(),
                    properties: vec![],
                },
                PaletteEntry {
                    name: "minecraft:cobblestone".into(),
                    properties: vec![],
                },
                PaletteEntry {
                    name: "minecraft:mossy_cobblestone".into(),
                    properties: vec![],
                },
            ],
            blocks: vec![
                StructureBlockEntry {
                    pos: [0, 0, 0],
                    state: 0,
                    block_nbt: None,
                },
                StructureBlockEntry {
                    pos: [1, 0, 0],
                    state: 1,
                    block_nbt: None,
                },
                StructureBlockEntry {
                    pos: [2, 0, 0],
                    state: 2,
                    block_nbt: None,
                },
            ],
            entities: vec![],
        }
    }

    /// A 1×3×1 vertical column (y=0,1,2) for anchor tests.
    fn column_structure() -> StructureNbt {
        StructureNbt {
            data_version: DATA_VERSION,
            size: [1, 3, 1],
            palette: vec![PaletteEntry {
                name: "minecraft:amethyst_block".into(),
                properties: vec![],
            }],
            blocks: (0..3)
                .map(|y| StructureBlockEntry {
                    pos: [0, y, 0],
                    state: 0,
                    block_nbt: None,
                })
                .collect(),
            entities: vec![],
        }
    }

    /// Write a structure as a gzip `.nbt` under `dir/rel` (creating parent dirs).
    fn write_template(dir: &Path, rel: &str, structure: &StructureNbt) {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create template parent dir");
        }
        write_structure_nbt(structure, &path).expect("write template nbt");
    }

    fn temp_structures_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "bong_nbt_registry_{}_{}_{:p}",
            tag,
            std::process::id(),
            &tag as *const _
        ));
        // Fresh dir each time.
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp structures dir");
        dir
    }

    // ── ① anchor enum: every variant + manifest round-trip + default ─────────

    #[test]
    fn anchor_from_manifest_maps_every_known_variant() {
        assert_eq!(
            DecorationAnchor::from_manifest("ground"),
            DecorationAnchor::Ground,
            "'ground' must map to Ground"
        );
        assert_eq!(
            DecorationAnchor::from_manifest("embedded"),
            DecorationAnchor::Embedded,
            "'embedded' must map to Embedded"
        );
        assert_eq!(
            DecorationAnchor::from_manifest("hanging"),
            DecorationAnchor::Hanging,
            "'hanging' must map to Hanging"
        );
    }

    #[test]
    fn anchor_unknown_and_empty_default_to_ground() {
        // Backward-compat: an old manifest with no anchor (empty string) or a
        // typo must NOT panic — it must fall back to the surface default so the
        // decoration still places somewhere sane.
        for raw in ["", "ground", "floating", "GROUND", "sky_isle_top"] {
            let got = DecorationAnchor::from_manifest(raw);
            if raw == "ground" {
                assert_eq!(got, DecorationAnchor::Ground);
            } else if raw != "embedded" && raw != "hanging" {
                assert_eq!(
                    got,
                    DecorationAnchor::Ground,
                    "anchor {raw:?} should default to Ground, got {got:?}"
                );
            }
        }
        assert_eq!(DecorationAnchor::default(), DecorationAnchor::Ground);
    }

    #[test]
    fn anchor_as_manifest_round_trips_through_from_manifest() {
        for anchor in [
            DecorationAnchor::Ground,
            DecorationAnchor::Embedded,
            DecorationAnchor::Hanging,
        ] {
            let s = anchor.as_manifest();
            assert_eq!(
                DecorationAnchor::from_manifest(s),
                anchor,
                "as_manifest({anchor:?})={s:?} must re-parse to the same variant"
            );
        }
    }

    // ── ② rotation: every variant transform + index mapping + composition ────

    #[test]
    fn rotation_apply_matches_clockwise_quarter_turns() {
        // A point at template-local (2, 5, 0) — on the +X arm, y=5 height.
        let (dx, dy, dz) = (2, 5, 0);
        assert_eq!(
            Rotation::None.apply(dx, dy, dz),
            (2, 5, 0),
            "None leaves the point unchanged"
        );
        assert_eq!(
            Rotation::Cw90.apply(dx, dy, dz),
            (0, 5, 2),
            "Cw90: (x,z)->(-z,x); +X arm should swing to +Z, y unchanged"
        );
        assert_eq!(
            Rotation::Cw180.apply(dx, dy, dz),
            (-2, 5, 0),
            "Cw180: (x,z)->(-x,-z); +X arm flips to -X"
        );
        assert_eq!(
            Rotation::Cw270.apply(dx, dy, dz),
            (0, 5, -2),
            "Cw270: (x,z)->(z,-x); +X arm swings to -Z"
        );
    }

    #[test]
    fn rotation_y_is_never_touched() {
        for rot in Rotation::ALL {
            let (_, ry, _) = rot.apply(7, 42, -3);
            assert_eq!(
                ry, 42,
                "{rot:?} must leave y unchanged (Y-axis rotation only)"
            );
        }
    }

    #[test]
    fn rotation_four_turns_returns_to_origin() {
        // Applying Cw90 four times must be identity (composition pin).
        let start = (3, 9, -5);
        let mut p = start;
        for _ in 0..4 {
            p = Rotation::Cw90.apply(p.0, p.1, p.2);
        }
        assert_eq!(
            p, start,
            "four Cw90 turns must compose to identity, got {p:?}"
        );
    }

    #[test]
    fn rotation_from_index_wraps_mod_four() {
        assert_eq!(Rotation::from_index(0), Rotation::None);
        assert_eq!(Rotation::from_index(1), Rotation::Cw90);
        assert_eq!(Rotation::from_index(2), Rotation::Cw180);
        assert_eq!(Rotation::from_index(3), Rotation::Cw270);
        assert_eq!(
            Rotation::from_index(4),
            Rotation::None,
            "index 4 wraps back to None (mod 4)"
        );
        assert_eq!(Rotation::from_index(7), Rotation::Cw270);
    }

    // ── ③ registry load: empty dir / missing dir / present templates ─────────

    #[test]
    fn empty_registry_has_no_templates_and_does_not_panic() {
        let reg = DecorationNbtRegistry::empty();
        assert!(reg.is_empty(), "empty() must report is_empty");
        assert_eq!(reg.len(), 0);
        assert!(reg.get("decorations/anything.nbt").is_none());
        assert!(!reg.contains("decorations/anything.nbt"));
    }

    #[test]
    fn load_missing_decorations_dir_yields_empty_registry() {
        // structures_dir exists but has no `decorations/` subdir — the asset
        // directory is authored in a later P6 stage. Must boot empty, not error.
        let dir = temp_structures_dir("missing_subdir");
        let reg = DecorationNbtRegistry::load(&dir).expect("missing subdir must not error");
        assert!(
            reg.is_empty(),
            "no decorations/ dir must give an empty registry, got {} templates",
            reg.len()
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_empty_decorations_dir_yields_empty_registry() {
        let dir = temp_structures_dir("empty_subdir");
        fs::create_dir_all(dir.join(DECORATIONS_SUBDIR)).unwrap();
        let reg = DecorationNbtRegistry::load(&dir).expect("empty subdir must not error");
        assert!(reg.is_empty(), "empty decorations/ dir → empty registry");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_collects_nested_templates_keyed_by_relative_path() {
        let dir = temp_structures_dir("nested");
        write_template(&dir, "decorations/tree/small_tree_v1.nbt", &row_structure());
        write_template(
            &dir,
            "decorations/crystal/crystal_v1.nbt",
            &column_structure(),
        );
        // A stray non-.nbt file must be ignored.
        fs::write(dir.join("decorations/README.txt"), b"ignore me").unwrap();

        let reg = DecorationNbtRegistry::load(&dir).expect("load nested templates");
        assert_eq!(
            reg.len(),
            2,
            "exactly the two .nbt files must be registered"
        );
        assert!(
            reg.contains("decorations/tree/small_tree_v1.nbt"),
            "nested tree template must key by its forward-slash relative path"
        );
        assert!(
            reg.contains("decorations/crystal/crystal_v1.nbt"),
            "nested crystal template must be present"
        );
        // The loaded structure must equal what we wrote (decompressed residency).
        let loaded = reg.get("decorations/tree/small_tree_v1.nbt").unwrap();
        assert_eq!(
            loaded,
            &row_structure(),
            "resident template must equal the authored structure verbatim"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_does_not_sweep_sibling_structure_dirs() {
        // dan_zong/ and wangyintai/ live alongside decorations/ — they must NOT
        // be swept into the decoration registry.
        let dir = temp_structures_dir("siblings");
        write_template(&dir, "decorations/tree/t.nbt", &row_structure());
        write_template(&dir, "dan_zong/great_hall.nbt", &column_structure());
        write_template(&dir, "wangyintai/disc.nbt", &column_structure());

        let reg = DecorationNbtRegistry::load(&dir).expect("load");
        assert_eq!(
            reg.len(),
            1,
            "only files under decorations/ must register; dan_zong/wangyintai excluded"
        );
        assert!(reg.contains("decorations/tree/t.nbt"));
        assert!(!reg.contains("dan_zong/great_hall.nbt"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_malformed_template_is_a_hard_error() {
        // A present-but-corrupt .nbt (bare, non-gzip) must trip loudly so a
        // broken asset is never silently dropped from placement.
        let dir = temp_structures_dir("malformed");
        let deco = dir.join(DECORATIONS_SUBDIR);
        fs::create_dir_all(&deco).unwrap();
        fs::write(deco.join("broken.nbt"), b"this is not gzip nbt").unwrap();

        let err = DecorationNbtRegistry::load(&dir)
            .expect_err("a corrupt template must be a hard error, not silently skipped");
        match err {
            RegistryError::Template { id, source } => {
                assert!(
                    id.contains("broken.nbt"),
                    "error must name the offending template, got id={id:?}"
                );
                assert!(
                    !source.is_empty(),
                    "error must carry the underlying nbt_io reason"
                );
            }
        }
        let _ = fs::remove_dir_all(&dir);
    }

    // ── ④ stamp: lookup miss / determinism / anchors / rotation contract ─────

    #[test]
    fn stamp_unknown_template_returns_none() {
        let reg = DecorationNbtRegistry::empty();
        assert!(
            reg.stamp(
                "decorations/nope.nbt",
                BlockPos::new(0, 64, 0),
                DecorationAnchor::Ground,
                Rotation::None,
            )
            .is_none(),
            "stamping a non-resident template must return None so callers fall back"
        );
    }

    #[test]
    fn stamp_ground_anchor_places_one_above_surface() {
        let dir = temp_structures_dir("stamp_ground");
        write_template(&dir, "decorations/row.nbt", &row_structure());
        let reg = DecorationNbtRegistry::load(&dir).unwrap();

        let surface = BlockPos::new(100, 64, -50);
        let (placements, unresolved) = reg
            .stamp(
                "decorations/row.nbt",
                surface,
                DecorationAnchor::Ground,
                Rotation::None,
            )
            .expect("known template stamps");
        assert!(unresolved.is_empty(), "row palette fully resolves");
        assert_eq!(placements.len(), 3, "all three row blocks placed");
        // Ground: template y=0 sits at surface.y + 1 (on top of the surface block).
        // template-local (0,0,0) → world (100, 65, -50).
        let first = placements.iter().find(|(p, _, _)| p.x == 100).unwrap();
        assert_eq!(
            first.0,
            BlockPos::new(100, 65, -50),
            "Ground anchor: template [0,0,0] must land one block above the surface"
        );
        // x=2 block → world x=102, same y.
        assert!(
            placements
                .iter()
                .any(|(p, _, _)| *p == BlockPos::new(102, 65, -50)),
            "template +X end must extend along +X at the same (ground+1) y"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn stamp_embedded_anchor_sinks_to_surface_level() {
        let dir = temp_structures_dir("stamp_embedded");
        write_template(&dir, "decorations/row.nbt", &row_structure());
        let reg = DecorationNbtRegistry::load(&dir).unwrap();

        let surface = BlockPos::new(0, 70, 0);
        let (placements, _) = reg
            .stamp(
                "decorations/row.nbt",
                surface,
                DecorationAnchor::Embedded,
                Rotation::None,
            )
            .unwrap();
        // Embedded: template y=0 sits AT surface.y (replaces the surface block).
        assert!(
            placements
                .iter()
                .any(|(p, _, _)| *p == BlockPos::new(0, 70, 0)),
            "Embedded anchor: template [0,0,0] must land at the surface block level"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn stamp_hanging_anchor_places_top_below_underside() {
        let dir = temp_structures_dir("stamp_hanging");
        // 1×3×1 column, size_y = 3.
        write_template(&dir, "decorations/col.nbt", &column_structure());
        let reg = DecorationNbtRegistry::load(&dir).unwrap();

        let underside = BlockPos::new(5, 200, 5);
        let (placements, _) = reg
            .stamp(
                "decorations/col.nbt",
                underside,
                DecorationAnchor::Hanging,
                Rotation::None,
            )
            .unwrap();
        assert_eq!(placements.len(), 3, "all three column blocks placed");
        // Hanging: base_origin.y = underside.y - size_y = 200 - 3 = 197.
        // template y=0 → 197; y=2 (top) → 199 = underside.y - 1 (one below underside).
        let ys: Vec<i32> = {
            let mut v: Vec<i32> = placements.iter().map(|(p, _, _)| p.y).collect();
            v.sort_unstable();
            v
        };
        assert_eq!(
            ys,
            vec![197, 198, 199],
            "Hanging: the column top (y=2) must sit at underside-1 (199), growing downward"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn stamp_is_deterministic_same_input_same_output() {
        let dir = temp_structures_dir("determinism");
        write_template(&dir, "decorations/row.nbt", &row_structure());
        let reg = DecorationNbtRegistry::load(&dir).unwrap();

        let surface = BlockPos::new(12, 80, 34);
        let a = reg
            .stamp(
                "decorations/row.nbt",
                surface,
                DecorationAnchor::Ground,
                Rotation::Cw90,
            )
            .unwrap();
        let b = reg
            .stamp(
                "decorations/row.nbt",
                surface,
                DecorationAnchor::Ground,
                Rotation::Cw90,
            )
            .unwrap();
        assert_eq!(
            a.0, b.0,
            "same (template, pos, anchor, rotation) must yield identical placements (memcpy determinism)"
        );
        assert_eq!(a.1, b.1, "unresolved list must be deterministic too");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn stamp_rotation_cw90_turns_the_x_row_into_a_z_row() {
        let dir = temp_structures_dir("rotate_row");
        write_template(&dir, "decorations/row.nbt", &row_structure());
        let reg = DecorationNbtRegistry::load(&dir).unwrap();

        let surface = BlockPos::new(0, 64, 0);
        // Cw90 maps (x,z) -> (-z, x); the +X row at z=0 becomes a +Z column at x=0.
        let (rotated, _) = reg
            .stamp(
                "decorations/row.nbt",
                surface,
                DecorationAnchor::Embedded,
                Rotation::Cw90,
            )
            .unwrap();
        // Embedded origin → (0,64,0). template (0,0,0)->(0,0,0); (1,0,0)->(0,0,1); (2,0,0)->(0,0,2).
        let mut zs: Vec<i32> = rotated.iter().map(|(p, _, _)| p.z).collect();
        zs.sort_unstable();
        assert_eq!(
            zs,
            vec![0, 1, 2],
            "Cw90 must turn the +X row into a +Z row (positions along z)"
        );
        assert!(
            rotated.iter().all(|(p, _, _)| p.x == 0),
            "after Cw90 every block sits on x=0 (the row no longer extends along x)"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn stamp_rotation_none_matches_gallery_structure_placements() {
        // Contract pin: Rotation::None + Ground must equal calling
        // structure_placements directly at (surface + ground offset). This ties
        // the fast path to the shared /gallery lowering helper.
        let dir = temp_structures_dir("rot_none_matches_gallery");
        write_template(&dir, "decorations/row.nbt", &row_structure());
        let reg = DecorationNbtRegistry::load(&dir).unwrap();
        let structure = row_structure();

        let surface = BlockPos::new(7, 64, 9);
        let (via_stamp, _) = reg
            .stamp(
                "decorations/row.nbt",
                surface,
                DecorationAnchor::Ground,
                Rotation::None,
            )
            .unwrap();
        let (via_gallery, _) =
            structure_placements(&structure, [surface.x, surface.y + 1, surface.z]);
        assert_eq!(
            via_stamp, via_gallery,
            "Rotation::None Ground stamp must equal structure_placements at the ground origin"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn stamp_skips_unresolved_palette_blocks_and_reports_them() {
        // A template referencing an unknown block must skip that block (no hole)
        // and report it in `unresolved` so the wiring stage can warn.
        let dir = temp_structures_dir("unresolved");
        let structure = StructureNbt {
            data_version: DATA_VERSION,
            size: [2, 1, 1],
            palette: vec![
                PaletteEntry {
                    name: "minecraft:stone".into(),
                    properties: vec![],
                },
                PaletteEntry {
                    name: "minecraft:totally_not_a_real_block".into(),
                    properties: vec![],
                },
            ],
            blocks: vec![
                StructureBlockEntry {
                    pos: [0, 0, 0],
                    state: 0,
                    block_nbt: None,
                },
                StructureBlockEntry {
                    pos: [1, 0, 0],
                    state: 1,
                    block_nbt: None,
                },
            ],
            entities: vec![],
        };
        write_template(&dir, "decorations/mixed.nbt", &structure);
        let reg = DecorationNbtRegistry::load(&dir).unwrap();

        // Both the rotated and non-rotated paths must agree on skip+report.
        for rot in [Rotation::None, Rotation::Cw90] {
            let (placements, unresolved) = reg
                .stamp(
                    "decorations/mixed.nbt",
                    BlockPos::new(0, 64, 0),
                    DecorationAnchor::Ground,
                    rot,
                )
                .unwrap();
            assert_eq!(
                placements.len(),
                1,
                "{rot:?}: only the resolvable stone block is placed (unknown block skipped, no hole)"
            );
            assert_eq!(
                unresolved,
                vec!["minecraft:totally_not_a_real_block".to_string()],
                "{rot:?}: the unknown palette block must be reported, not silently dropped"
            );
        }
        let _ = fs::remove_dir_all(&dir);
    }

    // ── ⑥ real authored decoration assets (P6 Stage 2) ──────────────────────
    //
    // These exercise the actual `server/structures/decorations/**/*.nbt` files
    // the gen scripts produce, not synthetic fixtures, so a broken / renamed /
    // missing asset trips the suite. They are the runtime counterpart to the
    // Python `gen_decorations.py` round-trip report.

    /// The 14 NBT-ised decoration kinds, each with the >=3-variant contract.
    /// Mirrors the `<kind>` directories under `server/structures/decorations/`.
    const DECORATION_KINDS: &[&str] = &[
        "small_tree",
        "bush",
        "boulder",
        "crystal",
        "big_mushroom",
        "fallen_log",
        "grave",
        "hanging_crystal",
        "ruins_pillar",
        "broken_urn",
        "bone_pile",
        "spirit_ore_vein",
        "rift_bridge",
        "spawn_portal",
    ];

    #[test]
    fn real_decoration_assets_load_and_round_trip() {
        let reg = DecorationNbtRegistry::load_default();
        assert!(
            !reg.is_empty(),
            "the authored decoration assets must load; if this is empty either \
             server/structures/decorations/ is missing or every .nbt failed to parse \
             (run scripts/nbt/decorations/gen_decorations.py)"
        );

        // Each resident template must be a well-formed MC 1.20.1 structure:
        // correct DataVersion, positive size, a palette, and at least one block.
        for (id, structure) in reg.iter() {
            assert_eq!(
                structure.data_version,
                nbt_io::DATA_VERSION,
                "template '{id}' must carry MC 1.20.1 DataVersion {} (got {})",
                nbt_io::DATA_VERSION,
                structure.data_version,
            );
            assert!(
                structure.size.iter().all(|&d| d > 0),
                "template '{id}' has a non-positive size dimension: {:?}",
                structure.size,
            );
            assert!(
                !structure.palette.is_empty(),
                "template '{id}' has an empty palette (no block types)"
            );
            assert!(
                !structure.blocks.is_empty(),
                "template '{id}' has no blocks — an empty decoration would stamp nothing"
            );
            // Every block index must be in palette range (no dangling state ref).
            for block in &structure.blocks {
                assert!(
                    (block.state as usize) < structure.palette.len(),
                    "template '{id}' block at {:?} references palette index {} \
                     but palette has only {} entries",
                    block.pos,
                    block.state,
                    structure.palette.len(),
                );
            }
        }
    }

    #[test]
    fn real_decoration_assets_have_no_unresolved_palette_blocks() {
        // The §contract: every authored palette block must resolve via
        // `blocks::block_from_name`, else the stamp silently drops it (a hole).
        // This is the runtime mirror of the Python name<->blocks.rs cross-check.
        let reg = DecorationNbtRegistry::load_default();
        assert!(!reg.is_empty(), "decoration assets must be present");

        let mut offenders: Vec<(String, Vec<String>)> = Vec::new();
        for (id, structure) in reg.iter() {
            let unresolved = structure.unresolved_palette_blocks();
            if !unresolved.is_empty() {
                offenders.push((id.to_string(), unresolved));
            }
        }
        assert!(
            offenders.is_empty(),
            "these decoration templates contain palette blocks that do NOT resolve \
             in block_from_name (they would stamp as holes). Add each block name to \
             server/src/world/terrain/blocks.rs::block_from_name:\n{offenders:#?}"
        );
    }

    #[test]
    fn every_decoration_kind_has_at_least_three_variants() {
        // §6.1 hard requirement: each NBT-ised kind ships >=3 form variants.
        let reg = DecorationNbtRegistry::load_default();
        assert!(!reg.is_empty(), "decoration assets must be present");

        for kind in DECORATION_KINDS {
            let prefix = format!("decorations/{kind}/");
            let count = reg.iter().filter(|(id, _)| id.starts_with(&prefix)).count();
            assert!(
                count >= 3,
                "decoration kind '{kind}' has only {count} variant(s) under {prefix}; \
                 the §6.1 contract requires >=3 distinct form variants"
            );
        }
    }

    #[test]
    fn decoration_variants_within_a_kind_differ() {
        // Variants must be genuinely different shapes, not copies — compare the
        // (size, block-count, palette) signature within each kind and require
        // at least two distinct signatures (so we never ship 3 identical files).
        let reg = DecorationNbtRegistry::load_default();
        assert!(!reg.is_empty(), "decoration assets must be present");

        for kind in DECORATION_KINDS {
            let prefix = format!("decorations/{kind}/");
            let mut signatures = std::collections::HashSet::new();
            for (_, s) in reg.iter().filter(|(id, _)| id.starts_with(&prefix)) {
                signatures.insert((s.size, s.blocks.len(), s.palette.len()));
            }
            assert!(
                signatures.len() >= 2,
                "decoration kind '{kind}' variants are not distinct enough: only \
                 {} unique (size, block_count, palette_size) signature(s) across all \
                 variants — variants must differ in height/density/completeness",
                signatures.len()
            );
        }
    }

    #[test]
    fn fallen_log_and_rift_bridge_ship_orientation_variants() {
        // Directional kinds bake explicit orientation variants so the runtime
        // can place them along either world axis without re-authoring.
        let reg = DecorationNbtRegistry::load_default();
        assert!(!reg.is_empty(), "decoration assets must be present");

        // Fallen log: an X-axis run and a Z-axis run have transposed bounding
        // boxes (size_x vs size_z dominant). Confirm both exist.
        let log_x = reg.get("decorations/fallen_log/oak_x_v1.nbt");
        let log_z = reg.get("decorations/fallen_log/spruce_z_v2.nbt");
        assert!(log_x.is_some(), "fallen_log X-axis variant must be present");
        assert!(log_z.is_some(), "fallen_log Z-axis variant must be present");
        let lx = log_x.unwrap();
        let lz = log_z.unwrap();
        assert!(
            lx.size[0] > lx.size[2],
            "oak_x_v1 must be longer along X (got size {:?})",
            lx.size
        );
        assert!(
            lz.size[2] > lz.size[0],
            "spruce_z_v2 must be longer along Z (got size {:?})",
            lz.size
        );

        // Rift bridge: X-span and Z-span variants present (transposed footprints).
        let bridge_x = reg.get("decorations/rift_bridge/x_v1.nbt");
        let bridge_z = reg.get("decorations/rift_bridge/z_v2.nbt");
        assert!(bridge_x.is_some(), "rift_bridge X variant must be present");
        assert!(bridge_z.is_some(), "rift_bridge Z variant must be present");
        let bx = bridge_x.unwrap();
        let bz = bridge_z.unwrap();
        assert!(
            bx.size[0] > bx.size[2] && bz.size[2] > bz.size[0],
            "rift_bridge x_v1 must span X (size {:?}) and z_v2 must span Z (size {:?})",
            bx.size,
            bz.size
        );
    }

    #[test]
    fn grave_assets_are_authored_for_embedded_stamping() {
        // Grave mounds use the Embedded anchor (sink one block). The dome must
        // start at template y=0 so the lowest row replaces the surface soil.
        let reg = DecorationNbtRegistry::load_default();
        let grave = reg
            .get("decorations/grave/small_v1.nbt")
            .expect("grave small_v1 must be present");
        let min_y = grave.blocks.iter().map(|b| b.pos[1]).min().unwrap();
        assert_eq!(
            min_y, 0,
            "grave dome must start at template y=0 so an Embedded stamp sinks the \
             base row into the surface (got min y={min_y})"
        );
        // Stamping with Embedded at a surface puts the base row AT the surface.
        let (placements, unresolved) = reg
            .stamp(
                "decorations/grave/small_v1.nbt",
                BlockPos::new(10, 70, 10),
                DecorationAnchor::Embedded,
                Rotation::None,
            )
            .expect("grave stamp resolves");
        assert!(unresolved.is_empty(), "grave palette must fully resolve");
        let base_row = placements.iter().filter(|(p, _, _)| p.y == 70).count();
        assert!(
            base_row > 0,
            "Embedded grave must place blocks at the surface y (70) — the sunken base row"
        );
    }

    #[test]
    fn hanging_crystal_assets_grow_downward_under_hanging_anchor() {
        // Hanging crystals attach at the template top and the tip is at y=0, so
        // a Hanging stamp puts the whole structure BELOW the underside surface.
        let reg = DecorationNbtRegistry::load_default();
        let id = "decorations/hanging_crystal/amethyst_stalactite_v1.nbt";
        let crystal = reg.get(id).expect("hanging crystal v1 must be present");
        let (placements, unresolved) = reg
            .stamp(
                id,
                BlockPos::new(0, 200, 0),
                DecorationAnchor::Hanging,
                Rotation::None,
            )
            .expect("hanging crystal stamp resolves");
        assert!(
            unresolved.is_empty(),
            "hanging crystal palette must fully resolve"
        );
        let max_y = placements.iter().map(|(p, _, _)| p.y).max().unwrap();
        assert!(
            max_y < 200,
            "a Hanging stamp must place every block strictly below the underside \
             surface (y=200); got top block at y={max_y}. size={:?}",
            crystal.size
        );
    }
}
