#!/usr/bin/env python3
"""Shared helpers for P6 decoration NBT generators.

worldgen-v4 P6 §6.1 / §8.1 #10 — these generators replace the procedural
geometry that used to live in `server/src/world/terrain/flora.rs` and
`structures.rs` with authored `.nbt` templates that the runtime stamps
(`DecorationNbtRegistry::stamp`).

Design rules carried over from the retired procedural geometry:

* **Block palettes** mirror the `DecorationSpec.blocks` tuples declared in
  `worldgen/scripts/terrain_gen/profiles/*.py` and the `BlockState::*` used in
  the retired `flora.rs` / `structures.rs` functions, so the NBT look matches
  what worldgen already promised for each `kind`.
* **Anchor**: trees / shrubs / boulders / crystals / mushrooms / logs grow up
  from template `y=0` (the registry's `Ground` anchor stamps `y=0` one block
  above the surface). Hanging crystals are authored growing **downward** from
  the template top (`Hanging` anchor aligns the template top to the underside).
  Grave mounds are authored to sink one block (`Embedded` anchor).
* **Variants**: every NBT-ised kind ships >=3 variants differing in
  height / density / completeness / species, and where orientation matters
  (logs, bridges) we bake explicit rotation variants so the runtime can also
  pick `Rotation::{None,Cw90,Cw180,Cw270}` without re-authoring.

All generators are deterministic (`random.seed(...)`) so re-running produces
byte-stable assets.
"""

from __future__ import annotations

import os
import sys

# Make `nbt_builder` importable whether run as a module or a script.
_NBT_DIR = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
if _NBT_DIR not in sys.path:
    sys.path.insert(0, _NBT_DIR)

from nbt_builder import StructureBuilder, load_structure  # noqa: E402

# Production asset root: server/structures/decorations/<kind>/<variant>.nbt
_REPO_ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
ASSET_ROOT = os.path.join(_REPO_ROOT, "server", "structures", "decorations")


def asset_path(kind: str, variant: str) -> str:
    """Absolute path for `<ASSET_ROOT>/<kind>/<variant>.nbt`, dir created."""
    kind_dir = os.path.join(ASSET_ROOT, kind)
    os.makedirs(kind_dir, exist_ok=True)
    return os.path.join(kind_dir, f"{variant}.nbt")


def rel_template_id(kind: str, variant: str) -> str:
    """The `decorations/<kind>/<variant>.nbt` id the Rust registry keys on."""
    return f"decorations/{kind}/{variant}.nbt"


# ---------------------------------------------------------------------------
# Geometry primitives (shared by several kinds)
# ---------------------------------------------------------------------------


def sphere_offsets(radius: int, *, y_min: int | None = None, y_max: int | None = None):
    """Yield (dx, dy, dz) offsets inside a sphere of `radius` (centred at 0).

    `y_min` / `y_max` clamp the vertical band (defaults: full sphere).
    """
    ymin = -radius if y_min is None else y_min
    ymax = radius if y_max is None else y_max
    rr = radius * radius
    for dy in range(ymin, ymax + 1):
        for dx in range(-radius, radius + 1):
            for dz in range(-radius, radius + 1):
                if dx * dx + dy * dy + dz * dz <= rr:
                    yield dx, dy, dz


def disc_offsets(radius: int):
    """Yield (dx, dz) offsets inside a flat disc of `radius`."""
    rr = radius * radius
    for dx in range(-radius, radius + 1):
        for dz in range(-radius, radius + 1):
            if dx * dx + dz * dz <= rr:
                yield dx, dz


def hemisphere_offsets(radius: int):
    """Yield (dx, dy, dz) for the upper hemisphere (dy in 0..radius)."""
    yield from sphere_offsets(radius, y_min=0, y_max=radius)


# ---------------------------------------------------------------------------
# Variant validation helper (used by the generator __main__ blocks)
# ---------------------------------------------------------------------------


def save_and_report(sb: StructureBuilder, path: str, *, preview_y: int | None = None) -> dict:
    """Save `sb` to `path`, then re-load to confirm a clean round-trip.

    Returns a stats dict the batch driver aggregates for accounting.
    """
    sb.save(path)
    stats = sb.get_stats()
    file_bytes = os.path.getsize(path)
    # Round-trip via the same reader the Rust registry mirrors.
    loaded = load_structure(path)
    out = {
        "path": path,
        "size": stats["size"],
        "total_blocks": stats["total_blocks"],
        "palette_size": stats["palette_size"],
        "file_bytes": file_bytes,
        "loaded_blocks": len(loaded),
        "block_counts": stats["block_counts"],
    }
    if preview_y is not None:
        out["preview"] = sb.ascii_top_view(y_level=preview_y)
    return out
