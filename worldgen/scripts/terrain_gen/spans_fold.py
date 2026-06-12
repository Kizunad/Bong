"""worldgen-v4 §8.1 #1/#54 — 2D → spans 统一折算 (canonical fold).

This is the single, canonical place where a tile's blended 2D layers are folded
into the span column representation that ships to the Rust runtime.  Every
terrain profile (all DSL-driven after P2) emits the 2D layers — height + the
carve-driving masks rift_axis_sdf / rim_edge_mask / fracture_mask / neg_pressure
/ entrance_mask + the vertical patch layers cave_mask / ceiling_height /
sky_island_base_y / sky_island_thickness — and the export folds them here.

History: this module started life in P0 as a compatibility *shim* — its job was
to keep the v3 landscape byte-identical while the 19 hand-written numpy profiles
were rewritten into the declarative DSL.  P2 finished that rewrite, so the shim
framing is retired: the fold below is the permanent 2D→spans converter, not a
temporary bridge.  The carve formulas it applies are mirrored by the dsl.py
carve ops (rift_carve / fracture_carve / neg_pressure_carve), so a profile
authored in pure DSL and a profile folded here describe the same surface —
"换表示不换景观".

CRITICAL (worldgen-v4 BLOCKER invariant): the v3 final landscape was NOT
``round(height)``.  v3 baked the surface in Rust ``column.rs::resolve_column``
by taking ``top_y = clamp_world_y(round(height))`` and then carving it down with
rift / fracture / neg_pressure / entrance sculpting (up to ~90 blocks) before
the cave void was even considered.  v4 dropped that Rust sculpting, so this fold
MUST reproduce the full v3 ``top_y`` here or the rendered surface of every
rift / fracture / neg / entrance zone floats 14~90 blocks above where v3 put it
("换表示不换景观" 被破坏).

v3 surface ``top_y`` (origin/main server/src/world/terrain/column.rs:96-211),
applied in this exact order on top of ``round(height)``:
  bedrock_y = MIN_Y (-64)
  top_y = clamp_world_y(round(height))               # clamp to [MIN_Y, MIN_Y+H-3]
  ① rift:    if rift_axis_sdf < 0.9:
                top_y -= round((1-rift_axis_sdf)*22 + rim_edge_mask*4)
  ② fracture:if fracture_mask > 0.7:
                top_y = max(bedrock_y+6, top_y - int((fracture_mask-0.7)*300))
                                                       # int() truncates, NOT round
  ③ neg:     if neg_pressure > 0.18:
                top_y -= round(neg_pressure*14)
  ④ entrance:if entrance_mask > 0.16:
                top_y -= round(entrance_mask*10)
  ⑥ final:   top_y = clamp(bedrock_y+2, MIN_Y+H-2)

Then the surface span and (optional) cave void / sky-isle are built ON TOP of
this carved ``top_y``:
  ⑤ cave:    if cave_mask > 0.58:
                carve_floor   = max(top_y - round(ceiling_height), bedrock_y+8)
                carve_ceiling = max(top_y - 2, carve_floor + 4)
             → surface span = (carve_ceiling+1, top_y); floor remnant below.
The sky-isle gate matches the old ``sky_island_span_for_sample``:
    mask >= 0.2 AND base_y < 9000 AND thickness >= 4.

Whatever the inputs, the result is always a legal ``ColumnSpans`` (sorted,
non-overlapping, in world range); degenerate carves collapse back to a single
solid span rather than producing an illegal column.
"""

from __future__ import annotations

import math

import numpy as np

from .fields import (
    MAX_SPANS,
    SPAN_MAX_Y,
    SPAN_MIN_Y,
    ColumnSpans,
    TileFieldBuffer,
)

# Bedrock floor of every solid column (matches Rust column.rs bedrock_y = min_y).
BEDROCK_Y = SPAN_MIN_Y

# World-Y geometry, mirrored from server/src/world/terrain/mod.rs.
#   MIN_Y = -64, WORLD_HEIGHT = 496.
# clamp_world_y caps the initial round(height) at MIN_Y + WORLD_HEIGHT - 3.
# The final v3 clamp is [bedrock_y + 2, MIN_Y + WORLD_HEIGHT - 2].
WORLD_HEIGHT = 496
CLAMP_WORLD_CEIL = SPAN_MIN_Y + WORLD_HEIGHT - 3  # 429 — clamp_world_y ceiling
FINAL_TOP_CEIL = SPAN_MIN_Y + WORLD_HEIGHT - 2  # 430 — final resolve clamp ceiling
FINAL_TOP_FLOOR = SPAN_MIN_Y + 2  # -62 — final resolve clamp floor

# Carve gate constants — kept in lockstep with column.rs::resolve_column.
RIFT_SDF_THRESHOLD = 0.9
FRACTURE_MASK_THRESHOLD = 0.7
NEG_PRESSURE_THRESHOLD = 0.18
ENTRANCE_MASK_THRESHOLD = 0.16
CAVE_MASK_THRESHOLD = 0.58
SKY_ISLE_MASK_THRESHOLD = 0.2
SKY_ISLE_SENTINEL = 9000.0
SKY_ISLE_MIN_THICKNESS = 4.0


def _round_half_away(value: float) -> int:
    """Reproduce Rust ``f32::round`` (round half away from zero).

    Python ``round`` / ``numpy.round`` use banker's rounding (half to even),
    which can differ from Rust by ±1 on exact ``.5`` boundaries.  Every carve
    depth in v3 went through ``f32::round`` (or ``round() as i32``), so the fold
    must round identically or the folded surface drifts off the v3 byte value.
    """
    if value >= 0.0:
        return math.floor(value + 0.5)
    return math.ceil(value - 0.5)


def v3_surface_top_y(
    height: float,
    rift_axis_sdf: float,
    rim_edge_mask: float,
    fracture_mask: float,
    neg_pressure: float,
    entrance_mask: float,
) -> int:
    """Reproduce v3 ``column.rs::resolve_column`` carved ``top_y`` (steps ①~⑥).

    This is the walkable surface BEFORE the cave void / sky-isle are layered on.
    Returns the integer block Y, fully clamped to the v3 world range, so the
    span fold and the v3-equivalence golden can both anchor to it.

    All carve arithmetic runs in **float32** (``np.float32``) because the v3 Rust
    column.rs did it in f32, and f32 vs f64 truncation diverges by ±1 at the
    fracture step (e.g. ``(0.90 - 0.7) * 300`` is ``59.999…`` in f32 → 59 but
    ``60.000…`` in f64 → 60).  Mirroring f32 is what keeps the folded surface
    byte-identical to v3, not merely "close".
    """
    f32 = np.float32
    bedrock_y = SPAN_MIN_Y
    # clamp_world_y(round(height)) — initial cap at CLAMP_WORLD_CEIL.
    # Rust: sample.height (f32) .round() as i32.
    top_y = max(SPAN_MIN_Y, min(CLAMP_WORLD_CEIL, _round_half_away(float(f32(height)))))

    rift = f32(rift_axis_sdf)
    # ① rift sculpt: ((1 - sdf) * 22 + rim * 4).round() as i32, all f32.
    if rift < f32(RIFT_SDF_THRESHOLD):
        carve_depth = _round_half_away(
            float((f32(1.0) - rift) * f32(22.0) + f32(rim_edge_mask) * f32(4.0))
        )
        top_y -= carve_depth

    # ② fracture crack — Rust ((mask - 0.7) * 300.0) as i32 (truncate, NOT round).
    fracture = f32(fracture_mask)
    if fracture > f32(FRACTURE_MASK_THRESHOLD):
        crack_depth = int((fracture - f32(FRACTURE_MASK_THRESHOLD)) * f32(300.0))
        top_y = max(bedrock_y + 6, top_y - crack_depth)

    # ③ negative-pressure sink: (neg * 14.0).round() as i32, f32.
    neg = f32(neg_pressure)
    if neg > f32(NEG_PRESSURE_THRESHOLD):
        top_y -= _round_half_away(float(neg * f32(14.0)))

    # ④ entrance sink: (entrance * 10.0).round() as i32, f32.
    entrance = f32(entrance_mask)
    if entrance > f32(ENTRANCE_MASK_THRESHOLD):
        top_y -= _round_half_away(float(entrance * f32(10.0)))

    # ⑥ final clamp
    return max(FINAL_TOP_FLOOR, min(FINAL_TOP_CEIL, top_y))


def _clamp_y(value: int) -> int:
    return max(SPAN_MIN_Y, min(SPAN_MAX_Y, value))


def _layer_or_default(
    buffer: TileFieldBuffer, name: str, area: int, default: float
) -> np.ndarray:
    """Return the layer as a float ndarray, or *default*-filled if omitted."""
    if name in buffer.layers:
        return np.asarray(buffer.layers[name], dtype=np.float64).reshape(area)
    return np.full(area, default, dtype=np.float64)


def _layer_or_zero(buffer: TileFieldBuffer, name: str, area: int) -> np.ndarray:
    """Return the layer as a float ndarray, or zeros if the profile omitted it."""
    return _layer_or_default(buffer, name, area, 0.0)


def column_spans_for_index(
    surface_y: int,
    cave_mask: float,
    ceiling_height: float,
    sky_mask: float,
    sky_base_y: float,
    sky_thickness: float,
    suppress_fold_isle: bool = False,
) -> ColumnSpans:
    """Fold one column's scalar 2D values into a ColumnSpans.

    ``surface_y`` MUST already be the carved v3 ``top_y`` (the output of
    :func:`v3_surface_top_y`), because the v3 cave carve in column.rs operates on
    the rift/fracture/neg/entrance-sculpted ``top_y`` — feeding it a raw
    ``round(height)`` would re-float the cave floor remnant off the v3 value.

    worldgen-v4 P3 §6.1 双源收口: ``suppress_fold_isle`` skips the 2D
    sky_island_base_y/thickness → floating-span fold.  A profile that declares a
    ``floating_island`` carver (currently only ``sky_isle``) gets its floating
    geometry from that carver — a richer 3D, staggered-altitude, drippy-underside
    island — so letting the flat 2D fold *also* emit an isle span double-sources
    the same column (the carver stacks its body on top of the fold slab → 3~4
    redundant spans).  The export sets this True whenever the tile's carver chain
    contains a floating_island carver, making the carver the **sole** geometric
    isle source; the sky_island_mask SEMANTIC layer is untouched (botany 五灵草
    env-locks + the carver gate still key off it).  Default False keeps the pure
    fold (and the P2 换表示不换景观 baseline + the fold unit tests) intact.

    Pure + deterministic so it can be unit-tested without building a tile.
    """
    surface_y = _clamp_y(surface_y)

    # --- ground span(s): span[0] is ALWAYS the walkable surface cap ---
    # A cave carves a void *below* the surface, leaving a floor remnant.  We
    # keep span[0] = (…, surface_y) so query_surface stays on the walkable top
    # and append the floor remnant as span[1] (§8.1 #2).
    spans: list[tuple[int, int]] = []
    carved = False
    if cave_mask > CAVE_MASK_THRESHOLD and ceiling_height > 0.0:
        # Rust: carve_floor = (top_y - ceiling_height.round()).max(bedrock_y + 8),
        # ceiling_height is f32 so mirror f32::round.
        carve_floor = max(
            surface_y - _round_half_away(float(np.float32(ceiling_height))),
            BEDROCK_Y + 8,
        )
        carve_ceiling = max(surface_y - 2, carve_floor + 4)
        lower_ceiling = carve_floor - 1  # top of the floor remnant below void
        upper_floor = carve_ceiling + 1  # bottom of the walkable surface cap
        # Need a genuine air gap (upper_floor > lower_ceiling + 1) AND both a
        # real floor remnant and a real surface cap; otherwise keep 1 span.
        if (
            lower_ceiling >= BEDROCK_Y
            and upper_floor <= surface_y
            and upper_floor > lower_ceiling + 1
        ):
            spans.append((_clamp_y(upper_floor), surface_y))  # span[0] = surface
            spans.append((BEDROCK_Y, _clamp_y(lower_ceiling)))  # span[1] = floor
            carved = True
    if not carved:
        spans.append((BEDROCK_Y, surface_y))

    # --- sky-isle span: a floating solid block above the ground ---
    # Suppressed when a floating_island carver owns the isle geometry (P3 §6.1
    # 双源收口) — see the docstring.  The mask/base_y/thickness layers still
    # exist (mask is exported as a semantic layer), they just don't fold a
    # redundant flat slab under the carver's 3D island.
    if (
        not suppress_fold_isle
        and sky_mask >= SKY_ISLE_MASK_THRESHOLD
        and sky_base_y < SKY_ISLE_SENTINEL
        and sky_thickness >= SKY_ISLE_MIN_THICKNESS
    ):
        base_y = _clamp_y(int(round(sky_base_y)))
        top_y = _clamp_y(base_y + int(round(sky_thickness)))
        # The isle must sit above the surface with a real air gap, and we must
        # have a free slot.
        if base_y > surface_y + 1 and top_y > base_y and len(spans) < MAX_SPANS:
            spans.append((base_y, top_y))

    return ColumnSpans(tuple(spans))


def spans_for_tile(
    buffer: TileFieldBuffer, suppress_fold_isle: bool = False
) -> list[ColumnSpans]:
    """Fold every column of *buffer* into a ColumnSpans list (column order).

    ``suppress_fold_isle`` (worldgen-v4 P3 §6.1 双源收口) forwards to
    :func:`column_spans_for_index`: when the tile's carver chain owns the
    floating-island geometry, the 2D sky_island_base_y/thickness fold is skipped
    so the carver is the sole isle source.  Default False keeps the canonical
    fold for callers without a carver context (the P2 equivalence baseline, the
    incremental console path).
    """
    area = buffer.tile_size * buffer.tile_size
    if "height" not in buffer.layers:
        raise KeyError("tile buffer has no 'height' layer to fold into spans")

    height = np.asarray(buffer.layers["height"], dtype=np.float64).reshape(area)
    # Carve-driving masks — the layers v3 column.rs used to sculpt top_y. Absent
    # layers default to the LAYER_REGISTRY "no effect" value: rift_axis_sdf=99
    # (>= 0.9 → no rift), everything else 0.
    rift_axis_sdf = _layer_or_default(buffer, "rift_axis_sdf", area, 99.0)
    rim_edge_mask = _layer_or_zero(buffer, "rim_edge_mask", area)
    fracture_mask = _layer_or_zero(buffer, "fracture_mask", area)
    neg_pressure = _layer_or_zero(buffer, "neg_pressure", area)
    entrance_mask = _layer_or_zero(buffer, "entrance_mask", area)
    cave_mask = _layer_or_zero(buffer, "cave_mask", area)
    ceiling_height = _layer_or_zero(buffer, "ceiling_height", area)
    sky_mask = _layer_or_zero(buffer, "sky_island_mask", area)
    # base_y absent → treat as sentinel (no isle); thickness absent → 0.
    if "sky_island_base_y" in buffer.layers:
        sky_base_y = np.asarray(
            buffer.layers["sky_island_base_y"], dtype=np.float64
        ).reshape(area)
    else:
        sky_base_y = np.full(area, SKY_ISLE_SENTINEL, dtype=np.float64)
    sky_thickness = _layer_or_zero(buffer, "sky_island_thickness", area)

    columns: list[ColumnSpans] = []
    for idx in range(area):
        # Reproduce the full v3 carved surface (rift/fracture/neg/entrance) BEFORE
        # folding the cave void + sky-isle on top of it.
        carved_top_y = v3_surface_top_y(
            height=float(height[idx]),
            rift_axis_sdf=float(rift_axis_sdf[idx]),
            rim_edge_mask=float(rim_edge_mask[idx]),
            fracture_mask=float(fracture_mask[idx]),
            neg_pressure=float(neg_pressure[idx]),
            entrance_mask=float(entrance_mask[idx]),
        )
        columns.append(
            column_spans_for_index(
                surface_y=carved_top_y,
                cave_mask=float(cave_mask[idx]),
                ceiling_height=float(ceiling_height[idx]),
                sky_mask=float(sky_mask[idx]),
                sky_base_y=float(sky_base_y[idx]),
                sky_thickness=float(sky_thickness[idx]),
                suppress_fold_isle=suppress_fold_isle,
            )
        )
    return columns
