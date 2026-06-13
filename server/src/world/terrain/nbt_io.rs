//! Gzip Minecraft Structure NBT read/write for the dev gallery review loop
//! (plan-worldgen-v4 P5).
//!
//! This is the Rust counterpart to `scripts/nbt/nbt_builder.py`: it reads and
//! writes MC 1.20.1 Structure Block `.nbt` files (DataVersion 3465, gzip
//! mandatory) using `valence_nbt` directly. The on-disk layout it produces is
//! byte-for-byte identical (after gzip decompression) to what the Python
//! builder emits, so the two implementations round-trip each other's files.
//!
//! ## Why a faithful structural mirror instead of `Vec<(BlockPos, BlockState)>`
//!
//! Round-tripping must be lossless: a `.nbt` read then written back must
//! produce the same NBT bytes (after decompression). To guarantee that we hold
//! the raw structure — palette entries keep their `minecraft:` namespace and
//! their blockstate `Properties` in original key order; block entries keep
//! their palette `state` index and any per-block `nbt` compound verbatim. We do
//! NOT lower into `BlockState` on read (that path is lossy for properties valence
//! does not model and re-orders the palette), so the gallery `/structure save`
//! command in a later P5 segment can edit-in-place without corrupting unrelated
//! blocks.
//!
//! Lowering to runtime `BlockState` (for `/gallery` stamping) is a *separate*
//! concern handled by [`PaletteEntry::block_state`], which strips the namespace
//! and applies properties through `blocks::block_from_name`.
//!
//! This module is the *front-loaded* NBT IO capability for plan-worldgen-v4 P5:
//! the `/gallery` stamp and `/structure save` dev commands (later P5 segments)
//! and the P6 NBT decoration pipeline are its runtime consumers. Until those
//! land, the public surface has no in-binary caller, so the whole module is
//! `allow(dead_code)` — mirroring the existing `#[allow(dead_code)] mod schema`
//! convention in `main.rs`. The full round-trip / cross-language test suite
//! below exercises every public entry point regardless.
#![allow(dead_code)]

use std::io::{self, Read, Write};
use std::path::Path;

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use valence::nbt::{from_binary, to_binary, Compound, List, Value};
use valence::prelude::{BlockState, PropName, PropValue};

/// MC 1.20.1 structure DataVersion. Must match
/// `scripts/nbt/nbt_builder.py::StructureBuilder.DATA_VERSION`.
pub const DATA_VERSION: i32 = 3465;

/// Gzip magic bytes (RFC 1952). All authored `.nbt` assets are gzip-compressed;
/// bare (uncompressed) NBT is rejected so we never silently mis-parse a file
/// written by a non-conforming tool.
const GZIP_MAGIC: [u8; 2] = [0x1f, 0x8b];

/// Errors surfaced by [`read_structure_nbt`] / [`write_structure_nbt`].
#[derive(Debug)]
pub enum NbtIoError {
    /// Underlying filesystem / gzip stream error.
    Io(io::Error),
    /// File did not begin with the gzip magic — bare NBT is unsupported.
    NotGzip,
    /// Decoded NBT but it did not match the structure schema (missing/typed
    /// field). The string explains which field and what was expected.
    Malformed(String),
    /// NBT binary decode/encode error from `valence_nbt`.
    Nbt(String),
}

impl std::fmt::Display for NbtIoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NbtIoError::Io(e) => write!(f, "structure nbt io error: {e}"),
            NbtIoError::NotGzip => write!(
                f,
                "structure nbt is not gzip-compressed (bare NBT unsupported; \
                 all authored .nbt assets must be gzip per scripts/nbt/nbt_builder.py)"
            ),
            NbtIoError::Malformed(msg) => write!(f, "malformed structure nbt: {msg}"),
            NbtIoError::Nbt(msg) => write!(f, "nbt codec error: {msg}"),
        }
    }
}

impl std::error::Error for NbtIoError {}

impl From<io::Error> for NbtIoError {
    fn from(e: io::Error) -> Self {
        NbtIoError::Io(e)
    }
}

/// One palette slot of a structure: a namespaced block name plus its
/// blockstate properties in original key order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteEntry {
    /// Namespaced block name as stored on disk, e.g. `"minecraft:stone_bricks"`.
    pub name: String,
    /// Blockstate properties (e.g. `facing=south`), preserved in the order they
    /// appeared in the file so re-serialization is byte-stable. Empty when the
    /// palette entry had no `Properties` compound.
    pub properties: Vec<(String, String)>,
}

impl PaletteEntry {
    /// The bare block name with the `minecraft:` namespace stripped, matching
    /// the keys understood by [`crate::world::terrain::blocks::block_from_name`].
    pub fn bare_name(&self) -> &str {
        self.name.strip_prefix("minecraft:").unwrap_or(&self.name)
    }

    /// Build a palette entry from a runtime [`BlockState`] — the inverse of
    /// [`PaletteEntry::block_state`]. Used by `/structure save` to serialize a
    /// gallery region back to a structure `.nbt`.
    ///
    /// The block name carries the `minecraft:` namespace (matching the on-disk
    /// authored assets and `nbt_builder.py` output), and every property the
    /// state actually has is captured in `PropName` declaration order so the
    /// resulting palette entry round-trips through `block_state()`.
    pub fn from_block_state(state: BlockState) -> Self {
        let name = format!("minecraft:{}", state.to_kind().to_str());
        let properties = state
            .to_kind()
            .props()
            .iter()
            .filter_map(|&prop_name| {
                state.get(prop_name).map(|prop_value| {
                    (
                        prop_name.to_str().to_string(),
                        prop_value.to_str().to_string(),
                    )
                })
            })
            .collect();
        PaletteEntry { name, properties }
    }

    /// Lower this palette entry into a runtime [`BlockState`], applying any
    /// properties valence recognises. Returns `None` when the bare block name
    /// is unknown to `block_from_name` (the caller decides whether to warn and
    /// skip or treat it as fatal).
    pub fn block_state(&self) -> Option<BlockState> {
        let mut state = super::blocks::block_from_name(self.bare_name())?;
        for (prop_name, prop_val) in &self.properties {
            if let (Some(pn), Some(pv)) =
                (PropName::from_str(prop_name), PropValue::from_str(prop_val))
            {
                if state.get(pn).is_some() {
                    state = state.set(pn, pv);
                }
            }
        }
        Some(state)
    }
}

/// One block placement: a position, the palette index it references, and an
/// optional per-block NBT compound (block entities — none of the current
/// authored assets carry one, but the format allows it).
#[derive(Debug, Clone, PartialEq)]
pub struct StructureBlockEntry {
    pub pos: [i32; 3],
    pub state: i32,
    /// Verbatim per-block `nbt` compound, if present. Held raw to round-trip
    /// block entities (signs, banners) without modelling each one.
    pub block_nbt: Option<Compound>,
}

/// In-memory mirror of an MC 1.20.1 Structure Block `.nbt` file. Faithful enough
/// to round-trip the original bytes (after gzip decompression).
#[derive(Debug, Clone, PartialEq)]
pub struct StructureNbt {
    pub data_version: i32,
    pub size: [i32; 3],
    pub palette: Vec<PaletteEntry>,
    pub blocks: Vec<StructureBlockEntry>,
}

impl StructureNbt {
    /// Collect every bare palette block name (namespace stripped) that does NOT
    /// resolve through `blocks::block_from_name`. An empty result means the
    /// whole palette is renderable; a non-empty result lists the offending
    /// names so callers can fail loudly instead of silently dropping blocks
    /// during a stamp.
    ///
    /// This is the runtime mirror of the `AUTHORED_STRUCTURE_BLOCKS` dual-pin
    /// test in `blocks.rs`: it enforces the same "every authored palette block
    /// must be resolvable" contract on data actually loaded from disk.
    pub fn unresolved_palette_blocks(&self) -> Vec<String> {
        self.palette
            .iter()
            .filter(|entry| super::blocks::block_from_name(entry.bare_name()).is_none())
            .map(|entry| entry.name.clone())
            .collect()
    }
}

/// Read a gzip-compressed structure `.nbt` file from disk.
///
/// Rejects bare (uncompressed) NBT with [`NbtIoError::NotGzip`].
pub fn read_structure_nbt(path: &Path) -> Result<StructureNbt, NbtIoError> {
    let raw = std::fs::read(path)?;
    read_structure_nbt_bytes(&raw)
}

/// Read a structure from in-memory file bytes (the contents of a `.nbt` file,
/// gzip header included). Used by [`read_structure_nbt`] and tests that load
/// fixtures via `include_bytes!`.
pub fn read_structure_nbt_bytes(file_bytes: &[u8]) -> Result<StructureNbt, NbtIoError> {
    if file_bytes.len() < 2 || file_bytes[0..2] != GZIP_MAGIC {
        return Err(NbtIoError::NotGzip);
    }

    let mut decoder = GzDecoder::new(file_bytes);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed)?;

    let mut slice: &[u8] = &decompressed;
    let (root, _root_name): (Compound, String) =
        from_binary(&mut slice).map_err(|e| NbtIoError::Nbt(e.to_string()))?;

    structure_from_compound(&root)
}

/// Write a structure to a gzip-compressed `.nbt` file, producing bytes that
/// `scripts/nbt/nbt_builder.py::load_structure` can read.
pub fn write_structure_nbt(structure: &StructureNbt, path: &Path) -> Result<(), NbtIoError> {
    let bytes = write_structure_nbt_bytes(structure)?;
    std::fs::write(path, bytes)?;
    Ok(())
}

/// Serialize a structure to gzip-compressed `.nbt` bytes (in memory).
pub fn write_structure_nbt_bytes(structure: &StructureNbt) -> Result<Vec<u8>, NbtIoError> {
    let root = compound_from_structure(structure);

    let mut raw = Vec::new();
    // Root name is the empty string, matching nbt_builder.py's `write_root("", ..)`.
    to_binary(&root, &mut raw, "").map_err(|e| NbtIoError::Nbt(e.to_string()))?;

    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&raw)?;
    let bytes = encoder.finish()?;
    Ok(bytes)
}

// ── Compound <-> StructureNbt ─────────────────────────────────────────────

fn structure_from_compound(root: &Compound) -> Result<StructureNbt, NbtIoError> {
    let data_version = match root.get("DataVersion") {
        Some(Value::Int(v)) => *v,
        Some(other) => {
            return Err(NbtIoError::Malformed(format!(
                "DataVersion must be TAG_Int, got {other:?}"
            )))
        }
        None => return Err(NbtIoError::Malformed("missing DataVersion".into())),
    };

    let size = read_int_triple(root, "size")?;

    let palette = match root.get("palette") {
        Some(Value::List(List::Compound(entries))) => entries
            .iter()
            .map(palette_entry_from_compound)
            .collect::<Result<Vec<_>, _>>()?,
        // An empty palette serializes as TAG_List of TAG_End in the Python
        // builder only when there are zero entries; accept that too.
        Some(Value::List(List::End)) => Vec::new(),
        Some(other) => {
            return Err(NbtIoError::Malformed(format!(
                "palette must be TAG_List<Compound>, got {other:?}"
            )))
        }
        None => return Err(NbtIoError::Malformed("missing palette".into())),
    };

    let blocks = match root.get("blocks") {
        Some(Value::List(List::Compound(entries))) => entries
            .iter()
            .map(block_entry_from_compound)
            .collect::<Result<Vec<_>, _>>()?,
        Some(Value::List(List::End)) => Vec::new(),
        Some(other) => {
            return Err(NbtIoError::Malformed(format!(
                "blocks must be TAG_List<Compound>, got {other:?}"
            )))
        }
        None => return Err(NbtIoError::Malformed("missing blocks".into())),
    };

    Ok(StructureNbt {
        data_version,
        size,
        palette,
        blocks,
    })
}

fn palette_entry_from_compound(comp: &Compound) -> Result<PaletteEntry, NbtIoError> {
    let name = match comp.get("Name") {
        Some(Value::String(s)) => s.clone(),
        Some(other) => {
            return Err(NbtIoError::Malformed(format!(
                "palette entry Name must be TAG_String, got {other:?}"
            )))
        }
        None => return Err(NbtIoError::Malformed("palette entry missing Name".into())),
    };

    let properties = match comp.get("Properties") {
        Some(Value::Compound(props)) => {
            let mut out = Vec::with_capacity(props.len());
            for (key, value) in props.iter() {
                match value {
                    Value::String(s) => out.push((key.clone(), s.clone())),
                    other => {
                        return Err(NbtIoError::Malformed(format!(
                            "palette property '{key}' must be TAG_String, got {other:?}"
                        )))
                    }
                }
            }
            out
        }
        None => Vec::new(),
        Some(other) => {
            return Err(NbtIoError::Malformed(format!(
                "palette Properties must be TAG_Compound, got {other:?}"
            )))
        }
    };

    Ok(PaletteEntry { name, properties })
}

fn block_entry_from_compound(comp: &Compound) -> Result<StructureBlockEntry, NbtIoError> {
    let pos = read_int_triple(comp, "pos")?;

    let state = match comp.get("state") {
        Some(Value::Int(v)) => *v,
        Some(other) => {
            return Err(NbtIoError::Malformed(format!(
                "block state must be TAG_Int, got {other:?}"
            )))
        }
        None => return Err(NbtIoError::Malformed("block entry missing state".into())),
    };

    let block_nbt = match comp.get("nbt") {
        Some(Value::Compound(c)) => Some(c.clone()),
        None => None,
        Some(other) => {
            return Err(NbtIoError::Malformed(format!(
                "block nbt must be TAG_Compound, got {other:?}"
            )))
        }
    };

    Ok(StructureBlockEntry {
        pos,
        state,
        block_nbt,
    })
}

fn read_int_triple(comp: &Compound, key: &str) -> Result<[i32; 3], NbtIoError> {
    match comp.get(key) {
        Some(Value::List(List::Int(list))) if list.len() == 3 => Ok([list[0], list[1], list[2]]),
        Some(Value::List(List::Int(list))) => Err(NbtIoError::Malformed(format!(
            "'{key}' must have 3 ints, got {}",
            list.len()
        ))),
        Some(other) => Err(NbtIoError::Malformed(format!(
            "'{key}' must be TAG_List<Int>, got {other:?}"
        ))),
        None => Err(NbtIoError::Malformed(format!("missing '{key}'"))),
    }
}

fn compound_from_structure(structure: &StructureNbt) -> Compound {
    let mut root = Compound::new();
    // Field order mirrors nbt_builder.py::_build_nbt_compound for byte-stable
    // round-tripping: DataVersion, size, palette, blocks, entities.
    root.insert("DataVersion", Value::Int(structure.data_version));
    root.insert("size", int_triple_value(structure.size));

    let palette: Vec<Compound> = structure
        .palette
        .iter()
        .map(palette_entry_to_compound)
        .collect();
    root.insert("palette", Value::List(List::Compound(palette)));

    let blocks: Vec<Compound> = structure
        .blocks
        .iter()
        .map(block_entry_to_compound)
        .collect();
    root.insert("blocks", Value::List(List::Compound(blocks)));

    // Empty entities list. The Python builder emits TAG_List<Compound> of length
    // zero here; valence serializes List::Compound(vec![]) the same way (element
    // tag = Compound, length = 0), so we keep that variant for parity rather
    // than List::End.
    root.insert("entities", Value::List(List::Compound(Vec::new())));

    root
}

fn palette_entry_to_compound(entry: &PaletteEntry) -> Compound {
    let mut comp = Compound::new();
    comp.insert("Name", Value::String(entry.name.clone()));
    if !entry.properties.is_empty() {
        let mut props = Compound::new();
        for (key, value) in &entry.properties {
            props.insert(key.clone(), Value::String(value.clone()));
        }
        comp.insert("Properties", Value::Compound(props));
    }
    comp
}

fn block_entry_to_compound(entry: &StructureBlockEntry) -> Compound {
    let mut comp = Compound::new();
    comp.insert("pos", int_triple_value(entry.pos));
    comp.insert("state", Value::Int(entry.state));
    if let Some(block_nbt) = &entry.block_nbt {
        comp.insert("nbt", Value::Compound(block_nbt.clone()));
    }
    comp
}

fn int_triple_value(triple: [i32; 3]) -> Value {
    Value::List(List::Int(triple.to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::terrain::blocks::tests::AUTHORED_STRUCTURE_BLOCKS;
    use std::path::PathBuf;

    /// The 11 authored `.nbt` assets, loaded at compile time so the round-trip
    /// pins run without touching the filesystem.
    const FIXTURES: &[(&str, &[u8])] = &[
        (
            "dan_zong/dan_zong_great_hall.nbt",
            include_bytes!("../../../structures/dan_zong/dan_zong_great_hall.nbt"),
        ),
        (
            "dan_zong/fallen_alchemist_bone.nbt",
            include_bytes!("../../../structures/dan_zong/fallen_alchemist_bone.nbt"),
        ),
        (
            "dan_zong/fallen_recipe_stele.nbt",
            include_bytes!("../../../structures/dan_zong/fallen_recipe_stele.nbt"),
        ),
        (
            "dan_zong/master_sarcophagus.nbt",
            include_bytes!("../../../structures/dan_zong/master_sarcophagus.nbt"),
        ),
        (
            "dan_zong/ruined_open_furnace.nbt",
            include_bytes!("../../../structures/dan_zong/ruined_open_furnace.nbt"),
        ),
        (
            "dan_zong/vapor_poison_spring_main.nbt",
            include_bytes!("../../../structures/dan_zong/vapor_poison_spring_main.nbt"),
        ),
        (
            "dan_zong/vapor_poison_spring_small.nbt",
            include_bytes!("../../../structures/dan_zong/vapor_poison_spring_small.nbt"),
        ),
        (
            "wangyintai/corridor_fragment.nbt",
            include_bytes!("../../../structures/wangyintai/corridor_fragment.nbt"),
        ),
        (
            "wangyintai/fallen_vortex_disc.nbt",
            include_bytes!("../../../structures/wangyintai/fallen_vortex_disc.nbt"),
        ),
        (
            "wangyintai/guantiantai_ruins.nbt",
            include_bytes!("../../../structures/wangyintai/guantiantai_ruins.nbt"),
        ),
        (
            "wangyintai/jingxuguan_side_hall.nbt",
            include_bytes!("../../../structures/wangyintai/jingxuguan_side_hall.nbt"),
        ),
    ];

    /// Decompress raw gzip file bytes into the underlying NBT payload, for
    /// byte-level comparison (gzip headers carry mtime/OS bytes that differ
    /// between writers, so we always compare the *decompressed* payload).
    fn decompress(file_bytes: &[u8]) -> Vec<u8> {
        let mut decoder = GzDecoder::new(file_bytes);
        let mut out = Vec::new();
        decoder.read_to_end(&mut out).expect("gzip decompress");
        out
    }

    fn repo_root() -> PathBuf {
        // CARGO_MANIFEST_DIR = <repo>/server ; structures live at <repo>/server/structures
        // and scripts at <repo>/scripts.
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("server dir has a parent")
            .to_path_buf()
    }

    // ── ① round-trip pin × 11: read → write → read structurally equal AND the
    // re-written decompressed NBT bytes equal the original decompressed bytes.

    #[test]
    fn all_authored_assets_round_trip_structurally_and_byte_identical() {
        for (name, original) in FIXTURES {
            let parsed = read_structure_nbt_bytes(original)
                .unwrap_or_else(|e| panic!("{name}: first read failed: {e}"));

            // DataVersion must be the pinned MC 1.20.1 value.
            assert_eq!(
                parsed.data_version, DATA_VERSION,
                "{name}: DataVersion should be {DATA_VERSION} (MC 1.20.1), got {}",
                parsed.data_version
            );

            let rewritten = write_structure_nbt_bytes(&parsed)
                .unwrap_or_else(|e| panic!("{name}: write failed: {e}"));

            let reparsed = read_structure_nbt_bytes(&rewritten)
                .unwrap_or_else(|e| panic!("{name}: second read failed: {e}"));

            assert_eq!(
                parsed, reparsed,
                "{name}: structure changed across read→write→read; \
                 palette/blocks/properties must round-trip losslessly"
            );

            // Strongest pin: decompressed NBT bytes are identical. This locks
            // field order, palette order, property order, and the empty-entities
            // encoding so any regression in the serializer trips immediately.
            // (Relies on valence_nbt's `preserve_order` feature so Compound keeps
            // insertion order, matching nbt_builder.py's field order.)
            assert_eq!(
                decompress(original),
                decompress(&rewritten),
                "{name}: re-serialized NBT payload differs from original byte-for-byte; \
                 field/palette/property ordering must be preserved"
            );
        }
    }

    // ── ② Rust↔Python cross check (both directions) ────────────────────────

    /// Run a small python snippet against scripts/nbt/nbt_builder.py and return
    /// stdout. Returns None (test skips) when python3 is unavailable so the
    /// suite stays green on machines without an interpreter, but asserts hard on
    /// any python-side error (so a real mismatch trips red, not skips).
    fn run_python(code: &str) -> Option<String> {
        use std::process::Command;
        let root = repo_root();
        let output = match Command::new("python3")
            .arg("-c")
            .arg(code)
            .current_dir(&root)
            .output()
        {
            Ok(o) => o,
            Err(_) => return None, // python3 not installed → skip cross-check
        };
        assert!(
            output.status.success(),
            "python cross-check failed:\n--- stdout ---\n{}\n--- stderr ---\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    #[test]
    fn rust_written_nbt_is_readable_by_python_builder() {
        // Build a structure in Rust covering: namespaced names, multi-property
        // palette entries (facing+half order), and an air block (which Python's
        // load_structure skips, exercising the air-filter path).
        let structure = StructureNbt {
            data_version: DATA_VERSION,
            size: [2, 1, 2],
            palette: vec![
                PaletteEntry {
                    name: "minecraft:stone_bricks".into(),
                    properties: vec![],
                },
                PaletteEntry {
                    name: "minecraft:stone_brick_stairs".into(),
                    properties: vec![
                        ("facing".into(), "south".into()),
                        ("half".into(), "bottom".into()),
                    ],
                },
                PaletteEntry {
                    name: "minecraft:air".into(),
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
                    pos: [1, 0, 1],
                    state: 2, // air — Python's load_structure drops this
                    block_nbt: None,
                },
            ],
        };

        let dir = std::env::temp_dir();
        let path = dir.join(format!("bong_nbt_io_rust_to_py_{}.nbt", std::process::id()));
        write_structure_nbt(&structure, &path).expect("write rust structure");

        let path_str = path.to_string_lossy().replace('\\', "\\\\");
        let code = format!(
            "import sys; sys.path.insert(0, 'scripts/nbt'); \
             from nbt_builder import load_structure; \
             blocks = load_structure('{path_str}'); \
             names = sorted(b.block_name for b in blocks); \
             stairs = [b for b in blocks if b.block_name == 'minecraft:stone_brick_stairs']; \
             print(len(blocks)); \
             print(','.join(names)); \
             print(dict(stairs[0].properties) if stairs else {{}})"
        );

        let Some(stdout) = run_python(&code) else {
            let _ = std::fs::remove_file(&path);
            eprintln!("python3 unavailable — skipping rust→python cross-check");
            return;
        };
        let _ = std::fs::remove_file(&path);

        let lines: Vec<&str> = stdout.lines().collect();
        assert_eq!(
            lines[0], "2",
            "python should read 2 non-air blocks (air filtered); got {stdout:?}"
        );
        assert_eq!(
            lines[1], "minecraft:stone_brick_stairs,minecraft:stone_bricks",
            "python should see exactly the two non-air block names; got {stdout:?}"
        );
        assert_eq!(
            lines[2], "{'facing': 'south', 'half': 'bottom'}",
            "python should recover the multi-property blockstate intact; got {stdout:?}"
        );
    }

    #[test]
    fn python_written_nbt_is_readable_by_rust_reader() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("bong_nbt_io_py_to_rust_{}.nbt", std::process::id()));
        let path_str = path.to_string_lossy().replace('\\', "\\\\");

        let code = format!(
            "import sys; sys.path.insert(0, 'scripts/nbt'); \
             from nbt_builder import StructureBuilder; \
             sb = StructureBuilder(3, 1, 1); \
             sb.set_block(0, 0, 0, 'minecraft:cobblestone'); \
             sb.set_block(1, 0, 0, 'minecraft:stone_brick_stairs', {{'facing': 'north', 'half': 'top'}}); \
             sb.set_block(2, 0, 0, 'minecraft:mossy_stone_bricks'); \
             sb.save('{path_str}'); \
             print('ok')"
        );

        let Some(stdout) = run_python(&code) else {
            eprintln!("python3 unavailable — skipping python→rust cross-check");
            return;
        };
        assert_eq!(stdout, "ok", "python build step should succeed");

        let parsed = read_structure_nbt(&path).expect("rust reads python-written nbt");
        let _ = std::fs::remove_file(&path);

        assert_eq!(parsed.data_version, DATA_VERSION, "DataVersion mismatch");
        assert_eq!(parsed.size, [3, 1, 1], "size mismatch");
        assert_eq!(parsed.palette.len(), 3, "expected 3 palette entries");
        assert_eq!(parsed.blocks.len(), 3, "expected 3 placed blocks");

        let stairs = parsed
            .palette
            .iter()
            .find(|p| p.name == "minecraft:stone_brick_stairs")
            .expect("stairs palette entry present");
        assert_eq!(
            stairs.properties,
            vec![
                ("facing".to_string(), "north".to_string()),
                ("half".to_string(), "top".to_string()),
            ],
            "rust should recover python-authored multi-property in order"
        );

        // The whole palette must be resolvable — Python authored only known blocks.
        assert!(
            parsed.unresolved_palette_blocks().is_empty(),
            "python-authored palette should fully resolve, unresolved: {:?}",
            parsed.unresolved_palette_blocks()
        );
    }

    // ── ③ gzip mandatory: bare (uncompressed) NBT is rejected ───────────────

    #[test]
    fn bare_uncompressed_nbt_is_rejected() {
        // Serialize a valid structure, then feed the *uncompressed* NBT bytes
        // (no gzip header) to the reader — it must reject, not mis-parse.
        let structure = StructureNbt {
            data_version: DATA_VERSION,
            size: [1, 1, 1],
            palette: vec![PaletteEntry {
                name: "minecraft:stone".into(),
                properties: vec![],
            }],
            blocks: vec![StructureBlockEntry {
                pos: [0, 0, 0],
                state: 0,
                block_nbt: None,
            }],
        };
        let root = compound_from_structure(&structure);
        let mut raw = Vec::new();
        to_binary(&root, &mut raw, "").expect("encode bare nbt");

        assert!(
            !raw.is_empty() && raw[0] != GZIP_MAGIC[0],
            "bare NBT should not start with gzip magic (sanity for the test setup)"
        );

        match read_structure_nbt_bytes(&raw) {
            Err(NbtIoError::NotGzip) => {}
            other => panic!(
                "bare NBT must be rejected with NotGzip; got {other:?}. \
                 gzip is mandatory per scripts/nbt/nbt_builder.py."
            ),
        }
    }

    #[test]
    fn empty_and_too_short_input_is_rejected_as_not_gzip() {
        for bytes in [&b""[..], &b"\x1f"[..]] {
            match read_structure_nbt_bytes(bytes) {
                Err(NbtIoError::NotGzip) => {}
                other => panic!("input {bytes:?} must be NotGzip, got {other:?}"),
            }
        }
    }

    // ── ④ empty structure (0 blocks) round-trips ────────────────────────────

    #[test]
    fn empty_structure_round_trips() {
        let structure = StructureNbt {
            data_version: DATA_VERSION,
            size: [0, 0, 0],
            palette: vec![],
            blocks: vec![],
        };
        let bytes = write_structure_nbt_bytes(&structure).expect("write empty");
        let reparsed = read_structure_nbt_bytes(&bytes).expect("read empty");
        assert_eq!(
            structure, reparsed,
            "empty (0-block, 0-palette) structure must round-trip identically"
        );
    }

    #[test]
    fn block_with_block_entity_nbt_round_trips() {
        // No authored asset carries per-block nbt today, but the format allows
        // it (signs/banners). Pin that we preserve it verbatim.
        let mut sign_text = Compound::new();
        sign_text.insert("Text1", Value::String("\"残碑\"".into()));
        let structure = StructureNbt {
            data_version: DATA_VERSION,
            size: [1, 1, 1],
            palette: vec![PaletteEntry {
                name: "minecraft:oak_sign".into(),
                properties: vec![("rotation".into(), "0".into())],
            }],
            blocks: vec![StructureBlockEntry {
                pos: [0, 0, 0],
                state: 0,
                block_nbt: Some(sign_text),
            }],
        };
        let bytes = write_structure_nbt_bytes(&structure).expect("write block-entity nbt");
        let reparsed = read_structure_nbt_bytes(&bytes).expect("read block-entity nbt");
        assert_eq!(
            structure, reparsed,
            "per-block nbt compound must round-trip verbatim"
        );
    }

    // ── ⑤ palette compliance: authored palettes resolve; illegal names flagged ─

    #[test]
    fn all_authored_asset_palettes_resolve_against_block_from_name() {
        for (name, original) in FIXTURES {
            let parsed = read_structure_nbt_bytes(original)
                .unwrap_or_else(|e| panic!("{name}: read failed: {e}"));
            let unresolved = parsed.unresolved_palette_blocks();
            assert!(
                unresolved.is_empty(),
                "{name}: palette blocks not resolvable via block_from_name: {unresolved:?}. \
                 Add each bare name to AUTHORED_STRUCTURE_BLOCKS and block_from_name."
            );
        }
    }

    #[test]
    fn authored_asset_palettes_are_subset_of_authored_structure_blocks_pin() {
        // Every bare palette name across the 11 assets must appear in the
        // AUTHORED_STRUCTURE_BLOCKS dual-pin list. This ties the runtime NBT
        // reader to the same contract `blocks.rs` enforces, so adding a new
        // block to an asset without updating the pin trips here.
        use std::collections::BTreeSet;
        let pinned: BTreeSet<&str> = AUTHORED_STRUCTURE_BLOCKS.iter().copied().collect();
        let mut missing: BTreeSet<String> = BTreeSet::new();
        for (name, original) in FIXTURES {
            let parsed = read_structure_nbt_bytes(original)
                .unwrap_or_else(|e| panic!("{name}: read failed: {e}"));
            for entry in &parsed.palette {
                let bare = entry.bare_name();
                // air is implicit and intentionally not in the authored pin.
                if bare == "air" {
                    continue;
                }
                if !pinned.contains(bare) {
                    missing.insert(bare.to_string());
                }
            }
        }
        assert!(
            missing.is_empty(),
            "asset palette blocks not in AUTHORED_STRUCTURE_BLOCKS pin: {missing:?}. \
             Update blocks.rs::AUTHORED_STRUCTURE_BLOCKS and block_from_name."
        );
    }

    #[test]
    fn unresolved_palette_blocks_flags_illegal_names() {
        // A palette entry referencing a block name unknown to block_from_name
        // must be reported (not silently dropped during a future stamp).
        let structure = StructureNbt {
            data_version: DATA_VERSION,
            size: [1, 1, 1],
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
            blocks: vec![],
        };
        let unresolved = structure.unresolved_palette_blocks();
        assert_eq!(
            unresolved,
            vec!["minecraft:totally_not_a_real_block".to_string()],
            "illegal palette block must be flagged by unresolved_palette_blocks; \
             known 'minecraft:stone' must not appear"
        );
    }

    #[test]
    fn palette_entry_lowers_to_block_state_with_properties() {
        let entry = PaletteEntry {
            name: "minecraft:stone_brick_stairs".into(),
            properties: vec![
                ("facing".into(), "south".into()),
                ("half".into(), "bottom".into()),
            ],
        };
        let state = entry
            .block_state()
            .expect("stairs must lower to a BlockState");
        assert_eq!(
            state.get(PropName::Facing),
            Some(PropValue::South),
            "facing property should be applied to the lowered BlockState"
        );
        assert_eq!(
            state.get(PropName::Half),
            Some(PropValue::Bottom),
            "half property should be applied to the lowered BlockState"
        );
    }

    // ── ⑥ from_block_state is the exact inverse of block_state() ────────────

    #[test]
    fn palette_entry_from_block_state_round_trips_back_to_state() {
        // Pick a state with non-default properties so the property capture path
        // is exercised, not just the bare-kind path.
        let original = BlockState::STONE_BRICK_STAIRS
            .set(PropName::Facing, PropValue::South)
            .set(PropName::Half, PropValue::Top);

        let entry = PaletteEntry::from_block_state(original);
        assert_eq!(
            entry.name, "minecraft:stone_brick_stairs",
            "from_block_state must emit the namespaced block name, got {}",
            entry.name
        );
        // Every captured property must reproduce the original state on re-lowering.
        let recovered = entry
            .block_state()
            .expect("entry built from a real state must lower back");
        assert_eq!(
            recovered.get(PropName::Facing),
            Some(PropValue::South),
            "facing must survive from_block_state → block_state round-trip"
        );
        assert_eq!(
            recovered.get(PropName::Half),
            Some(PropValue::Top),
            "half must survive from_block_state → block_state round-trip"
        );
        assert_eq!(
            recovered, original,
            "from_block_state(state).block_state() must equal the original state \
             (this is the /structure save serialization contract)"
        );
    }

    #[test]
    fn palette_entry_from_simple_block_state_has_no_spurious_properties() {
        let entry = PaletteEntry::from_block_state(BlockState::STONE);
        assert_eq!(entry.name, "minecraft:stone");
        assert!(
            entry.properties.is_empty(),
            "a property-less block must serialize with no Properties, got {:?}",
            entry.properties
        );
    }

    #[test]
    fn malformed_compound_missing_size_is_rejected() {
        // Build a structure compound missing `size`, gzip it, and confirm the
        // reader reports Malformed rather than panicking.
        let mut root = Compound::<String>::new();
        root.insert("DataVersion", Value::Int(DATA_VERSION));
        root.insert("palette", Value::List(List::Compound(Vec::new())));
        root.insert("blocks", Value::List(List::Compound(Vec::new())));
        let mut raw = Vec::new();
        to_binary(&root, &mut raw, "").expect("encode");
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&raw).unwrap();
        let bytes = encoder.finish().unwrap();

        match read_structure_nbt_bytes(&bytes) {
            Err(NbtIoError::Malformed(msg)) => assert!(
                msg.contains("size"),
                "error should name the missing 'size' field, got: {msg}"
            ),
            other => panic!("missing size must be Malformed, got {other:?}"),
        }
    }
}
