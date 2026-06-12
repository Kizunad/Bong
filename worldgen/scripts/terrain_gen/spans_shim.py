"""worldgen-v4 P0 §8.1 #1/#54 — 2D → spans 兼容折算 shim.

The 19 existing profiles still emit the legacy 2D layers (height + the now-
unexported vertical patch layers cave_mask / ceiling_height / entrance_mask /
sky_island_base_y / sky_island_thickness).  Rather than rewrite every profile
(that is P2's job), this shim folds those layers into the span representation:

  * normal column  → 1 span: (bedrock, round(height))
  * sky-isle column → + a high span: (round(base_y), round(base_y + thickness))
  * cave column     → the surface span is carved open by one void, yielding a
                      lower (bedrock, carve_floor-1) and upper (carve_ceiling+1,
                      surface) span.

The carve geometry mirrors the Rust ``column.rs::resolve_column`` cave branch
byte-for-byte so the rendered landscape is unchanged ("先换表示不换景观"):
    cave_mask > 0.58 →
        carve_floor   = max(round(surface - ceiling_height), bedrock + 8)
        carve_ceiling = max(surface - 2, carve_floor + 4)
The sky-isle gate matches ``sky_island_span_for_sample``:
    mask >= 0.2 AND base_y < 9000 AND thickness >= 4.

Whatever the inputs, the result is always a legal ``ColumnSpans`` (sorted,
non-overlapping, in world range); degenerate carves collapse back to a single
solid span rather than producing an illegal column.
"""

from __future__ import annotations

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

# Cave carve gate + sky-isle gate constants — kept in lockstep with column.rs.
CAVE_MASK_THRESHOLD = 0.58
SKY_ISLE_MASK_THRESHOLD = 0.2
SKY_ISLE_SENTINEL = 9000.0
SKY_ISLE_MIN_THICKNESS = 4.0


def _clamp_y(value: int) -> int:
    return max(SPAN_MIN_Y, min(SPAN_MAX_Y, value))


def _layer_or_zero(buffer: TileFieldBuffer, name: str, area: int) -> np.ndarray:
    """Return the layer as a float ndarray, or zeros if the profile omitted it."""
    if name in buffer.layers:
        return np.asarray(buffer.layers[name], dtype=np.float64).reshape(area)
    return np.zeros(area, dtype=np.float64)


def column_spans_for_index(
    surface_y: int,
    cave_mask: float,
    ceiling_height: float,
    sky_mask: float,
    sky_base_y: float,
    sky_thickness: float,
) -> ColumnSpans:
    """Fold one column's scalar 2D values into a ColumnSpans.

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
        carve_floor = max(
            surface_y - int(round(ceiling_height)), BEDROCK_Y + 8
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
    if (
        sky_mask >= SKY_ISLE_MASK_THRESHOLD
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


def spans_for_tile(buffer: TileFieldBuffer) -> list[ColumnSpans]:
    """Fold every column of *buffer* into a ColumnSpans list (column order)."""
    area = buffer.tile_size * buffer.tile_size
    if "height" not in buffer.layers:
        raise KeyError("tile buffer has no 'height' layer to fold into spans")

    height = np.asarray(buffer.layers["height"], dtype=np.float64).reshape(area)
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
        columns.append(
            column_spans_for_index(
                surface_y=int(round(float(height[idx]))),
                cave_mask=float(cave_mask[idx]),
                ceiling_height=float(ceiling_height[idx]),
                sky_mask=float(sky_mask[idx]),
                sky_base_y=float(sky_base_y[idx]),
                sky_thickness=float(sky_thickness[idx]),
            )
        )
    return columns
