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
#[cfg(windows)]
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};

use valence::prelude::{BlockPos, Resource};

use super::nbt_io::{self, StructureNbt};
use crate::cmd::dev::gallery::{structure_placements, StampPlacement};

/// Sub-directory (relative to `server/structures/`) that holds decoration
/// templates. Kept distinct from the existing `dan_zong/` and `wangyintai/`
/// large-layout structures so decoration loading never sweeps those in.
pub const DECORATIONS_SUBDIR: &str = "decorations";
/// Maximum aggregate resident storage admitted for all decoration templates.
pub const MAX_RESIDENT_TEMPLATE_MEMORY_BYTES: usize = 32 * 1024 * 1024;

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryError {
    diagnostics: Vec<String>,
}

impl RegistryError {
    fn new(mut diagnostics: Vec<String>) -> Self {
        diagnostics.sort();
        diagnostics.dedup();
        Self { diagnostics }
    }

    pub fn diagnostics(&self) -> &[String] {
        &self.diagnostics
    }
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "decoration NBT registry failed validation:\n- {}",
            self.diagnostics.join("\n- ")
        )
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

/// Startup-only candidate plus every NBT admission diagnostic. Valid templates
/// remain queryable so raster manifests can validate their template references
/// in the same preflight even when another authored template is broken. The
/// candidate cannot become a runtime resource until
/// [`DecorationNbtPreflight::into_registry`] proves the diagnostic set is empty.
pub(crate) struct DecorationNbtPreflight {
    candidate: DecorationNbtRegistry,
    diagnostics: Vec<String>,
}

impl DecorationNbtPreflight {
    #[cfg(test)]
    pub(crate) fn from_parts_for_tests(
        candidate: DecorationNbtRegistry,
        diagnostics: Vec<String>,
    ) -> Self {
        let diagnostics = RegistryError::new(diagnostics).diagnostics;
        Self {
            candidate,
            diagnostics,
        }
    }

    pub(crate) fn candidate(&self) -> &DecorationNbtRegistry {
        &self.candidate
    }

    pub(crate) fn diagnostics(&self) -> &[String] {
        &self.diagnostics
    }

    pub(crate) fn into_registry(self) -> Result<DecorationNbtRegistry, RegistryError> {
        if self.diagnostics.is_empty() {
            Ok(self.candidate)
        } else {
            Err(RegistryError::new(self.diagnostics))
        }
    }
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
    /// A missing decorations directory remains compatible and returns an empty
    /// registry. A present but malformed template is fatal and propagates to
    /// startup; corrupt authored assets must never degrade into a silent empty
    /// registry.
    pub fn load_default() -> Result<Self, RegistryError> {
        Self::prepare_default().into_registry()
    }

    pub(crate) fn prepare_default() -> DecorationNbtPreflight {
        Self::prepare(&Self::default_structures_dir())
    }

    /// Scan `<structures_dir>/decorations/**/*.nbt`, decompress each once, and
    /// build the resident registry. Keys are relative paths under
    /// `structures_dir` (so they start with `decorations/`).
    ///
    /// **Graceful on a missing / empty directory** — returns an empty registry when `decorations/` is absent or holds no
    /// `.nbt` files, so the binary boots before any decoration asset is authored.
    /// A *present but malformed* file is a hard error (a corrupt asset must trip
    /// loudly, not be silently dropped from placement).
    pub fn load(structures_dir: &Path) -> Result<Self, RegistryError> {
        Self::prepare(structures_dir).into_registry()
    }

    pub(crate) fn prepare(structures_dir: &Path) -> DecorationNbtPreflight {
        Self::prepare_with_resident_limit(structures_dir, MAX_RESIDENT_TEMPLATE_MEMORY_BYTES)
    }

    fn prepare_with_resident_limit(
        structures_dir: &Path,
        resident_limit: usize,
    ) -> DecorationNbtPreflight {
        let deco_dir = structures_dir.join(DECORATIONS_SUBDIR);
        let root_metadata = match std::fs::symlink_metadata(structures_dir) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return DecorationNbtPreflight {
                    candidate: Self::empty(),
                    diagnostics: vec![format!(
                        "structures directory {} must be a real directory, not a symlink or special file",
                        structures_dir.display()
                    )],
                };
            }
            Ok(metadata) => Some(metadata),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => {
                return DecorationNbtPreflight {
                    candidate: Self::empty(),
                    diagnostics: vec![format!(
                        "failed to inspect structures directory {}: {error}",
                        structures_dir.display()
                    )],
                };
            }
        };
        if root_metadata.is_none()
            && std::fs::symlink_metadata(&deco_dir)
                .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
        {
            return DecorationNbtPreflight {
                candidate: Self::empty(),
                diagnostics: Vec::new(),
            };
        }
        let trusted_root = match std::fs::canonicalize(structures_dir) {
            Ok(root) => root,
            Err(error) => {
                return DecorationNbtPreflight {
                    candidate: Self::empty(),
                    diagnostics: vec![format!(
                        "failed to anchor structures directory {}: {error}",
                        structures_dir.display()
                    )],
                };
            }
        };
        if let Some(expected) = root_metadata {
            let anchored = match std::fs::metadata(&trusted_root) {
                Ok(metadata) => metadata,
                Err(error) => {
                    return DecorationNbtPreflight {
                        candidate: Self::empty(),
                        diagnostics: vec![format!(
                            "failed to recheck structures directory {}: {error}",
                            structures_dir.display()
                        )],
                    };
                }
            };
            #[cfg(unix)]
            let same_root = {
                use std::os::unix::fs::MetadataExt;
                (expected.dev(), expected.ino()) == (anchored.dev(), anchored.ino())
            };
            #[cfg(windows)]
            let same_root = (expected.volume_serial_number(), expected.file_index())
                == (anchored.volume_serial_number(), anchored.file_index());
            #[cfg(not(any(unix, windows)))]
            let same_root = true;
            if !same_root {
                return DecorationNbtPreflight {
                    candidate: Self::empty(),
                    diagnostics: vec![format!(
                        "structures directory {} changed while being anchored",
                        structures_dir.display()
                    )],
                };
            }
        }
        let NbtFileScan {
            mut paths,
            mut diagnostics,
        } = collect_nbt_files(&deco_dir, &trusted_root);
        // Deterministic load order so diagnostics and iteration are stable.
        paths.sort();

        let mut templates = HashMap::with_capacity(paths.len());
        let mut resident_bytes = 0usize;
        for path in paths {
            let id = relative_template_id(structures_dir, &path);
            match nbt_io::open_regular_file_under_root(&path, &trusted_root)
                .map_err(nbt_io::NbtIoError::Io)
                .and_then(nbt_io::read_structure_nbt_file)
            {
                Ok(structure) => {
                    let invalid_palette = structure.palette_diagnostics();
                    if invalid_palette.is_empty() {
                        let template_bytes = structure.resident_memory_bytes();
                        if resident_bytes.saturating_add(template_bytes) > resident_limit {
                            diagnostics.push(format!(
                                "decoration template '{id}' would exceed the {resident_limit}-byte aggregate resident memory limit (current {resident_bytes}, template {template_bytes})"
                            ));
                        } else if templates.insert(id.clone(), structure).is_some() {
                            diagnostics.push(format!("duplicate decoration template id '{id}'"));
                        } else {
                            resident_bytes += template_bytes;
                        }
                    } else {
                        diagnostics.extend(invalid_palette.into_iter().map(|reason| {
                            format!("decoration template '{id}' has invalid palette entry {reason}")
                        }));
                    }
                }
                Err(error) => diagnostics.push(format!(
                    "decoration template '{id}' failed to load: {error}"
                )),
            }
        }

        diagnostics.sort();
        diagnostics.dedup();
        DecorationNbtPreflight {
            candidate: Self { templates },
            diagnostics,
        }
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
    /// * `unresolved` — always empty for a registry admitted by startup validation.
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
        let mut placements = Vec::with_capacity(structure.blocks.len());
        for block in &structure.blocks {
            let Some(entry) = structure.palette.get(block.state as usize) else {
                continue;
            };
            let Ok(state) = entry.block_state() else {
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
        Some((placements, Vec::new()))
    }

    /// The sorted list of resident template ids that live directly under
    /// `decorations/<kind>/` (e.g. `kind = "small_tree"` →
    /// `["decorations/small_tree/birch_tall_v2.nbt",
    ///   "decorations/small_tree/oak_round_v1.nbt", …]`).
    ///
    /// Sorted so a deterministic `index % len` pick is stable across runs (the
    /// `HashMap` iteration order is not). Empty when no variants are resident for
    /// the kind (the caller then falls back to its procedural geometry path).
    ///
    /// This is how the flora / structures wiring resolves "give me the variant
    /// pool for this decoration kind" without each profile having to enumerate
    /// the authored filenames — the registry is the single source of truth for
    /// which `.nbt` assets exist.
    pub fn variants_for_kind(&self, kind: &str) -> Vec<&str> {
        let prefix = format!("decorations/{kind}/");
        let mut ids: Vec<&str> = self
            .templates
            .keys()
            .filter(|id| {
                // Direct children of the kind dir only — `decorations/small_tree/x.nbt`
                // matches, `decorations/small_tree/sub/x.nbt` does not (no nested
                // kinds today, but keep the contract tight).
                id.strip_prefix(&prefix)
                    .is_some_and(|rest| !rest.contains('/'))
            })
            .map(|id| id.as_str())
            .collect();
        ids.sort_unstable();
        ids
    }

    /// Deterministically pick one of `kind`'s resident variant ids using a hash
    /// value (typically a per-placement `decoration_hash`). `None` when the kind
    /// has no resident variants (caller falls back to procedural).
    ///
    /// Picking from the sorted [`variants_for_kind`] list keeps the choice stable
    /// for a given `(kind, hash)` regardless of `HashMap` iteration order — the
    /// determinism the §8.1 #10 stamp contract requires.
    ///
    /// [`variants_for_kind`]: DecorationNbtRegistry::variants_for_kind
    pub fn pick_variant(&self, kind: &str, hash: u32) -> Option<String> {
        let variants = self.variants_for_kind(kind);
        if variants.is_empty() {
            return None;
        }
        Some(variants[(hash as usize) % variants.len()].to_string())
    }

    /// Whether the registry holds at least one variant under `decorations/<kind>/`.
    /// The flora / structures wiring uses this to decide NBT-stamp vs. procedural
    /// without allocating the full variant list.
    pub fn has_kind(&self, kind: &str) -> bool {
        let prefix = format!("decorations/{kind}/");
        self.templates.keys().any(|id| {
            id.strip_prefix(&prefix)
                .is_some_and(|rest| !rest.contains('/'))
        })
    }
}

/// Paths that remain safe to inspect plus every filesystem admission error.
/// Keeping valid paths alongside diagnostics lets startup validate their NBT
/// contents and manifest references in the same failure instead of losing the
/// rest of the scan after one bad node.
struct NbtFileScan {
    paths: Vec<PathBuf>,
    diagnostics: Vec<String>,
}

/// Recursively collect every regular `*.nbt` file under `dir`. A genuinely
/// missing decorations root remains the backward-compatible empty registry;
/// unreadable entries, symlinks, and non-regular `*.nbt` nodes fail closed while
/// other valid files remain available to the startup candidate.
const MAX_SCAN_ENTRIES: usize = 100_000;
const MAX_SCAN_DIAGNOSTICS: usize = 256;

fn push_scan_diagnostic(diagnostics: &mut Vec<String>, diagnostic: String) {
    if diagnostics.len() < MAX_SCAN_DIAGNOSTICS {
        diagnostics.push(diagnostic);
    }
}

fn collect_nbt_files(dir: &Path, trusted_root: &Path) -> NbtFileScan {
    let metadata = match std::fs::symlink_metadata(dir) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return NbtFileScan {
                paths: Vec::new(),
                diagnostics: Vec::new(),
            };
        }
        Err(error) => {
            return NbtFileScan {
                paths: Vec::new(),
                diagnostics: vec![format!(
                    "failed to inspect decoration directory {}: {error}",
                    dir.display()
                )],
            };
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return NbtFileScan {
            paths: Vec::new(),
            diagnostics: vec![format!(
                "decoration path {} must be a real directory, not a symlink or special file",
                dir.display()
            )],
        };
    }

    let mut pending = vec![dir.to_path_buf()];
    let mut paths = Vec::new();
    let mut diagnostics = Vec::new();
    let mut scanned_entries = 0usize;
    while let Some(current) = pending.pop() {
        let canonical_current = match std::fs::canonicalize(&current) {
            Ok(path) if path.starts_with(trusted_root) => path,
            Ok(_) => {
                push_scan_diagnostic(
                    &mut diagnostics,
                    format!(
                        "decoration directory {} escaped trusted root during traversal",
                        current.display()
                    ),
                );
                continue;
            }
            Err(error) => {
                push_scan_diagnostic(
                    &mut diagnostics,
                    format!(
                        "failed to anchor decoration directory {}: {error}",
                        current.display()
                    ),
                );
                continue;
            }
        };
        let current_metadata = match std::fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => metadata,
            Ok(_) => {
                push_scan_diagnostic(
                    &mut diagnostics,
                    format!(
                        "decoration directory {} changed during traversal",
                        current.display()
                    ),
                );
                continue;
            }
            Err(error) => {
                push_scan_diagnostic(
                    &mut diagnostics,
                    format!(
                        "failed to inspect decoration directory {}: {error}",
                        current.display()
                    ),
                );
                continue;
            }
        };
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let canonical_metadata = match std::fs::metadata(&canonical_current) {
                Ok(metadata) => metadata,
                Err(error) => {
                    push_scan_diagnostic(
                        &mut diagnostics,
                        format!(
                            "failed to inspect anchored decoration directory {}: {error}",
                            current.display()
                        ),
                    );
                    continue;
                }
            };
            if (current_metadata.dev(), current_metadata.ino())
                != (canonical_metadata.dev(), canonical_metadata.ino())
            {
                push_scan_diagnostic(
                    &mut diagnostics,
                    format!(
                        "decoration directory {} changed during traversal",
                        current.display()
                    ),
                );
                continue;
            }
        }

        let read_dir = match std::fs::read_dir(&current) {
            Ok(entries) => entries,
            Err(error) => {
                push_scan_diagnostic(
                    &mut diagnostics,
                    format!(
                        "failed to read decoration directory {}: {error}",
                        current.display()
                    ),
                );
                continue;
            }
        };
        let post_open_path = match std::fs::canonicalize(&current) {
            Ok(path) if path == canonical_current => path,
            Ok(_) => {
                push_scan_diagnostic(
                    &mut diagnostics,
                    format!(
                        "decoration directory {} changed while being opened",
                        current.display()
                    ),
                );
                continue;
            }
            Err(error) => {
                push_scan_diagnostic(
                    &mut diagnostics,
                    format!(
                        "failed to recheck decoration directory {}: {error}",
                        current.display()
                    ),
                );
                continue;
            }
        };
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let post_open_metadata = match std::fs::metadata(&post_open_path) {
                Ok(metadata) => metadata,
                Err(error) => {
                    push_scan_diagnostic(
                        &mut diagnostics,
                        format!(
                            "failed to recheck anchored decoration directory {}: {error}",
                            current.display()
                        ),
                    );
                    continue;
                }
            };
            if (current_metadata.dev(), current_metadata.ino())
                != (post_open_metadata.dev(), post_open_metadata.ino())
            {
                push_scan_diagnostic(
                    &mut diagnostics,
                    format!(
                        "decoration directory {} changed while being opened",
                        current.display()
                    ),
                );
                continue;
            }
        }
        for entry_result in read_dir {
            scanned_entries += 1;
            if scanned_entries > MAX_SCAN_ENTRIES {
                push_scan_diagnostic(
                    &mut diagnostics,
                    format!(
                        "decoration scan exceeded {MAX_SCAN_ENTRIES} filesystem entries under {}",
                        dir.display()
                    ),
                );
                pending.clear();
                break;
            }
            let entry = match entry_result {
                Ok(entry) => entry,
                Err(error) => {
                    push_scan_diagnostic(
                        &mut diagnostics,
                        format!(
                            "failed to enumerate decoration directory {}: {error}",
                            current.display()
                        ),
                    );
                    continue;
                }
            };
            let path = entry.path();
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(error) => {
                    push_scan_diagnostic(
                        &mut diagnostics,
                        format!(
                            "failed to inspect decoration path {}: {error}",
                            path.display()
                        ),
                    );
                    continue;
                }
            };
            if file_type.is_symlink() {
                push_scan_diagnostic(
                    &mut diagnostics,
                    format!("decoration path {} must not be a symlink", path.display()),
                );
            } else if path.extension().and_then(|extension| extension.to_str()) == Some("nbt") {
                if file_type.is_file() {
                    paths.push(path);
                } else {
                    push_scan_diagnostic(
                        &mut diagnostics,
                        format!(
                            "decoration template {} must be a regular file",
                            path.display()
                        ),
                    );
                }
            } else if file_type.is_dir() {
                pending.push(path);
            }
        }
    }

    diagnostics.sort();
    diagnostics.dedup();
    NbtFileScan { paths, diagnostics }
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
    fn load_missing_structures_root_yields_empty_registry() {
        let dir = temp_structures_dir("missing_root");
        fs::remove_dir_all(&dir).expect("remove structures fixture root");
        let reg = DecorationNbtRegistry::load(&dir).expect("missing optional root must not error");
        assert!(reg.is_empty());
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

    #[cfg(unix)]
    #[test]
    fn load_rejects_root_and_nested_symlinks() {
        use std::os::unix::fs::symlink;

        let root_link = temp_structures_dir("structures_root_symlink");
        let root_target = root_link.with_file_name("structures_root_symlink_target");
        fs::create_dir_all(root_target.join(DECORATIONS_SUBDIR)).unwrap();
        fs::remove_dir_all(&root_link).unwrap();
        symlink(&root_target, &root_link).unwrap();
        let root_error = DecorationNbtRegistry::load(&root_link)
            .expect_err("the structures directory itself must not be a symlink");
        assert!(
            root_error.to_string().contains("real directory"),
            "structures-root symlink diagnostic must explain the trust boundary: {root_error}"
        );
        let _ = fs::remove_file(&root_link);
        let _ = fs::remove_dir_all(&root_target);

        let root_case = temp_structures_dir("root_symlink");
        let external = root_case.join("external");
        fs::create_dir_all(&external).unwrap();
        symlink(&external, root_case.join(DECORATIONS_SUBDIR)).unwrap();
        let root_error = DecorationNbtRegistry::load(&root_case)
            .expect_err("a symlinked decorations root must fail closed");
        assert!(
            root_error.to_string().contains("real directory"),
            "root symlink diagnostic must explain the real-directory contract: {root_error}"
        );
        let _ = fs::remove_dir_all(&root_case);

        let nested_case = temp_structures_dir("nested_symlink");
        let decorations = nested_case.join(DECORATIONS_SUBDIR);
        let external = nested_case.join("external");
        fs::create_dir_all(&decorations).unwrap();
        fs::create_dir_all(&external).unwrap();
        symlink(&external, decorations.join("linked-kind")).unwrap();
        let nested_error = DecorationNbtRegistry::load(&nested_case)
            .expect_err("a nested symlink must fail closed instead of being followed or skipped");
        assert!(
            nested_error.to_string().contains("must not be a symlink"),
            "nested symlink diagnostic must name the forbidden node type: {nested_error}"
        );
        let _ = fs::remove_dir_all(&nested_case);
    }

    #[cfg(unix)]
    #[test]
    fn load_rejects_non_regular_nbt_node() {
        use std::os::unix::net::UnixListener;

        let dir = temp_structures_dir("non_regular_nbt");
        let decorations = dir.join(DECORATIONS_SUBDIR);
        fs::create_dir_all(&decorations).unwrap();
        let socket_path = decorations.join("not-a-template.nbt");
        let listener = UnixListener::bind(&socket_path).expect("bind unix socket fixture");

        let error = DecorationNbtRegistry::load(&dir)
            .expect_err("a special node ending in .nbt must fail before any read is attempted");
        assert!(
            error.to_string().contains("must be a regular file"),
            "special .nbt node diagnostic must explain the regular-file contract: {error}"
        );

        drop(listener);
        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn load_rejects_fresh_nbt_symlink_input() {
        use std::os::unix::fs::symlink;

        let dir = temp_structures_dir("fresh_nbt_symlink");
        let decorations = dir.join(DECORATIONS_SUBDIR);
        fs::create_dir_all(&decorations).expect("create decorations fixture");
        let external = dir.join("external.nbt");
        fs::write(&external, b"not gzip nbt").expect("write external NBT target");
        symlink(&external, decorations.join("linked.nbt")).expect("create NBT symlink fixture");

        let error = DecorationNbtRegistry::load(&dir)
            .expect_err("the discovery and open path must reject a symlinked NBT input");
        assert!(
            error.to_string().contains("must not be a symlink"),
            "fresh NBT symlink admission must name the forbidden node type: {error}"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn load_reads_the_opened_nbt_descriptor_after_path_replacement() {
        use std::os::unix::fs::symlink;

        let dir = temp_structures_dir("opened_descriptor_race");
        let template_path = dir.join("decorations/tree/original.nbt");
        write_template(&dir, "decorations/tree/original.nbt", &row_structure());
        let replacement = dir.join("replacement.nbt");
        fs::write(&replacement, b"not gzip nbt").expect("write bad replacement target");

        let hook_path = template_path.clone();
        let hook_replacement = replacement.clone();
        super::nbt_io::set_open_regular_file_after_open_test_hook(move || {
            fs::remove_file(&hook_path).expect("unlink opened NBT path");
            symlink(&hook_replacement, &hook_path).expect("replace NBT path with symlink");
        });

        let registry = DecorationNbtRegistry::load(&dir).expect(
            "registry must decode the opened original descriptor, not the path replacement",
        );
        assert!(registry.contains("decorations/tree/original.nbt"));
        assert!(
            fs::symlink_metadata(&template_path)
                .expect("replacement path should exist")
                .file_type()
                .is_symlink(),
            "test must actually replace the path after descriptor open"
        );

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
        assert_eq!(
            err.diagnostics().len(),
            1,
            "one corrupt template must produce one deterministic diagnostic"
        );
        assert!(
            err.diagnostics()[0].contains("broken.nbt"),
            "error must name the offending template: {err}"
        );
        assert!(
            err.diagnostics()[0].contains("failed to load"),
            "error must carry the underlying nbt_io reason: {err}"
        );
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

    #[cfg(unix)]
    #[test]
    fn preflight_keeps_valid_candidate_when_sibling_nbt_node_is_invalid() {
        use std::os::unix::net::UnixListener;

        let dir = temp_structures_dir("candidate_with_special_node");
        write_template(&dir, "decorations/tree/valid.nbt", &column_structure());
        let socket_path = dir.join("decorations/tree/broken.nbt");
        let listener = UnixListener::bind(&socket_path).expect("bind special NBT socket fixture");

        let preflight = DecorationNbtRegistry::prepare(&dir);
        assert!(
            preflight.candidate().contains("decorations/tree/valid.nbt"),
            "a bad sibling node must not hide valid templates from cross-source reference preflight"
        );
        assert!(
            preflight
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.contains("broken.nbt")
                    && diagnostic.contains("regular file")),
            "special-node admission failure must remain fatal and identify the path"
        );
        assert!(
            preflight.into_registry().is_err(),
            "filesystem diagnostics must prevent the partial candidate becoming a runtime resource"
        );

        drop(listener);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn aggregate_resident_budget_rejects_excess_templates_before_commit() {
        let dir = temp_structures_dir("resident_budget");
        write_template(&dir, "decorations/tree/first.nbt", &row_structure());
        write_template(&dir, "decorations/tree/second.nbt", &column_structure());

        let first_bytes = row_structure().resident_memory_bytes();
        let second_bytes = column_structure().resident_memory_bytes();
        let limit = first_bytes.max(second_bytes);
        assert!(
            first_bytes + second_bytes > limit,
            "fixture must require the aggregate budget to reject one template"
        );

        let preflight = DecorationNbtRegistry::prepare_with_resident_limit(&dir, limit);
        assert_eq!(
            preflight.candidate().len(),
            1,
            "only the first template within the aggregate budget may enter the candidate"
        );
        assert!(
            preflight
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.contains("aggregate resident memory limit")),
            "aggregate overflow must be surfaced as a startup diagnostic: {:?}",
            preflight.diagnostics()
        );
        assert!(
            preflight.into_registry().is_err(),
            "aggregate overflow must fail closed instead of committing a partial runtime registry"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_invalid_palette_entries_is_atomic_and_aggregated() {
        let dir = temp_structures_dir("invalid_palette");
        write_template(&dir, "decorations/tree/valid.nbt", &column_structure());

        let mut unknown_block = row_structure();
        unknown_block.palette[0].name = "minecraft:totally_not_a_real_block".into();
        write_template(&dir, "decorations/tree/unknown_block.nbt", &unknown_block);

        let mut invalid_property = row_structure();
        invalid_property.palette[0].name = "minecraft:oak_log".into();
        invalid_property.palette[0].properties = vec![("axis".into(), "north".into())];
        write_template(
            &dir,
            "decorations/tree/invalid_property.nbt",
            &invalid_property,
        );

        let preflight = DecorationNbtRegistry::prepare(&dir);
        assert!(
            preflight.candidate().contains("decorations/tree/valid.nbt"),
            "valid templates stay queryable during the same startup preflight"
        );
        assert!(
            !preflight
                .candidate()
                .contains("decorations/tree/unknown_block.nbt"),
            "invalid templates must never enter the candidate registry"
        );
        let error = preflight
            .into_registry()
            .expect_err("a candidate with diagnostics must not become a runtime registry");
        assert_eq!(
            error.diagnostics().len(),
            2,
            "both invalid templates must be reported in one startup failure: {error}"
        );
        assert!(
            error
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.contains("unknown_block.nbt")
                    && diagnostic.contains("totally_not_a_real_block")),
            "unknown block diagnostic must name both template and block: {error}"
        );
        assert!(
            error
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.contains("invalid_property.nbt")
                    && diagnostic.contains("axis")
                    && diagnostic.contains("north")),
            "invalid property diagnostic must name template, property, and value: {error}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_rejects_negative_and_out_of_range_palette_state_references() {
        let dir = temp_structures_dir("invalid_palette_state");

        let mut negative = row_structure();
        negative.blocks[0].state = -1;
        write_template(&dir, "decorations/tree/negative.nbt", &negative);

        let mut out_of_range = row_structure();
        out_of_range.blocks[1].state = out_of_range.palette.len() as i32;
        write_template(&dir, "decorations/tree/out_of_range.nbt", &out_of_range);

        let error = DecorationNbtRegistry::load(&dir)
            .expect_err("dangling palette state references must reject the whole registry");
        assert_eq!(
            error.diagnostics().len(),
            2,
            "negative and upper-bound state references must both be reported: {error}"
        );
        assert!(
            error.diagnostics().iter().any(|diagnostic| {
                diagnostic.contains("negative.nbt")
                    && diagnostic.contains("state -1")
                    && diagnostic.contains("palette length 3")
            }),
            "negative state diagnostic must identify template, state, and palette length: {error}"
        );
        assert!(
            error.diagnostics().iter().any(|diagnostic| {
                diagnostic.contains("out_of_range.nbt")
                    && diagnostic.contains("state 3")
                    && diagnostic.contains("palette length 3")
            }),
            "state == palette.len() diagnostic must identify the upper-bound failure: {error}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    // ── ⑥ real authored decoration assets (P6 Stage 2) ──────────────────────
    //
    // These exercise the actual `server/structures/decorations/**/*.nbt` files
    // the gen scripts produce, not synthetic fixtures, so a broken / renamed /
    // missing asset trips the suite. They are the runtime counterpart to the
    // Python `gen_decorations.py` round-trip report.

    /// The NBT-ised decoration kind dirs, each with the >=3-variant contract.
    /// Mirrors the `<kind>` directories under `server/structures/decorations/`.
    /// `bush` is split into four ecology pools (`bush_temperate` / `bush_cold` /
    /// `bush_marsh` / `bush_nether`) so a `kind="shrub"` decoration only ever
    /// stamps variants from its own biome — see `profiles/base.py`
    /// `_SHRUB_ECOLOGY` and `gen_bush.py`.
    const DECORATION_KINDS: &[&str] = &[
        "small_tree",
        "bush_temperate",
        "bush_cold",
        "bush_marsh",
        "bush_nether",
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
        let reg = DecorationNbtRegistry::load_default().expect("real decoration assets must load");
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
        // Every authored palette block/property passed the same strict load-time
        // validation production startup uses. This mirrors the Python catalog check.
        let reg = DecorationNbtRegistry::load_default().expect("real decoration assets must load");
        assert!(!reg.is_empty(), "decoration assets must be present");

        let mut offenders: Vec<(String, Vec<String>)> = Vec::new();
        for (id, structure) in reg.iter() {
            let unresolved = structure.palette_diagnostics();
            if !unresolved.is_empty() {
                offenders.push((id.to_string(), unresolved));
            }
        }
        assert!(
            offenders.is_empty(),
            "these decoration templates contain palette entries that do NOT resolve \
             in server/assets/worldgen/block_catalog.toml:\n{offenders:#?}"
        );
    }

    #[test]
    fn every_decoration_kind_has_at_least_three_variants() {
        // §6.1 hard requirement: each NBT-ised kind ships >=3 form variants.
        let reg = DecorationNbtRegistry::load_default().expect("real decoration assets must load");
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
        let reg = DecorationNbtRegistry::load_default().expect("real decoration assets must load");
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
        let reg = DecorationNbtRegistry::load_default().expect("real decoration assets must load");
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
        let reg = DecorationNbtRegistry::load_default().expect("real decoration assets must load");
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
        let reg = DecorationNbtRegistry::load_default().expect("real decoration assets must load");
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
