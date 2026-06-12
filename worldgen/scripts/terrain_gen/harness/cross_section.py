"""worldgen-v4 P3 §6.1 — PIL cross-section render tool.

Renders a **vertical slice** of a span column line to a PNG so the P3 carve
shapes (overhang / floating isle / arch / cave galleries) can be eyeballed: does
the overhang actually overhang? does the isle float free with a drippy bottom?
This is the §6.1 三轮打磨 evidence tool — each refinement round renders a slice
to ``worldgen/generated/p3_preview/`` (gitignored) and the geometry is checked
by eye.

Output convention
-----------------
Pixel ``(px, py)``: ``px`` indexes the column along the cut line (left→right);
``py`` is world height with **y_max at the top** (so the image reads like a
side-on photo).  Colours:

  * solid  → stone grey
  * air    → sky blue
  * water  → deep blue (when a water level is supplied)
  * bedrock line, surface markers → subtle tints

Uses Pillow (``PIL``) per the P3 task; if Pillow is unavailable the renderer
raises a clear ImportError rather than silently skipping evidence.
"""

from __future__ import annotations

from pathlib import Path
from typing import Sequence

from ..fields import SPAN_MAX_Y, SPAN_MIN_Y, ColumnSpans

# Palette (RGB).
COLOR_AIR = (150, 196, 232)      # sky blue
COLOR_SOLID = (111, 114, 118)    # stone grey
COLOR_SURFACE = (120, 168, 96)   # grass-ish cap on the walkable surface
COLOR_WATER = (50, 90, 150)      # deep blue
COLOR_BEDROCK = (54, 52, 58)     # near-black bedrock band
COLOR_GRID = (96, 128, 156)      # faint altitude gridline over air


def _column_color_stack(
    column: ColumnSpans,
    y_min: int,
    y_max: int,
    water_level: int | None,
) -> list[tuple[int, int, int]]:
    """Bottom→top list of RGB colours, one per world Y in ``[y_min, y_max]``."""
    surface_y = column.surface_ceiling_y
    height = y_max - y_min + 1
    # Start as air (or water below the water level).
    stack: list[tuple[int, int, int]] = []
    for y in range(y_min, y_max + 1):
        if water_level is not None and y <= water_level:
            stack.append(COLOR_WATER)
        else:
            stack.append(COLOR_AIR)
    # Paint solids.
    for floor_y, ceiling_y in column.spans:
        lo = max(floor_y, y_min)
        hi = min(ceiling_y, y_max)
        for y in range(lo, hi + 1):
            stack[y - y_min] = COLOR_SOLID
    # Bedrock band at the very bottom (visual anchor).
    if y_min <= SPAN_MIN_Y <= y_max:
        stack[SPAN_MIN_Y - y_min] = COLOR_BEDROCK
    # Surface cap marker — the walkable top of span[0].
    if surface_y is not None and y_min <= surface_y <= y_max:
        stack[surface_y - y_min] = COLOR_SURFACE
    assert len(stack) == height
    return stack


def render_cross_section(
    columns: Sequence[ColumnSpans],
    output_path: str | Path,
    *,
    y_min: int = SPAN_MIN_Y,
    y_max: int = SPAN_MAX_Y,
    water_level: int | None = None,
    pixel_scale: int = 2,
    title: str | None = None,
) -> Path:
    """Render ``columns`` (a cut line of span columns) to a vertical-slice PNG.

    ``columns`` is left→right along the cut.  Returns the written path.  The
    image is ``len(columns) * pixel_scale`` wide and ``(y_max-y_min+1) *
    pixel_scale`` tall, with ``y_max`` at the top.
    """
    try:
        from PIL import Image
    except ImportError as exc:  # pragma: no cover - environment guard
        raise ImportError(
            "render_cross_section needs Pillow (PIL); install it to produce "
            "P3 §6.1 cross-section evidence PNGs"
        ) from exc

    if not columns:
        raise ValueError("render_cross_section needs at least one column")
    if y_max <= y_min:
        raise ValueError(f"y_max={y_max} must be > y_min={y_min}")

    width = len(columns)
    height = y_max - y_min + 1
    img = Image.new("RGB", (width, height), COLOR_AIR)
    px = img.load()

    for col_x, column in enumerate(columns):
        stack = _column_color_stack(column, y_min, y_max, water_level)
        for y_idx, color in enumerate(stack):
            # Flip vertically so y_max is at the top of the image.
            py = (height - 1) - y_idx
            px[col_x, py] = color

    if pixel_scale > 1:
        img = img.resize(
            (width * pixel_scale, height * pixel_scale), Image.NEAREST
        )

    output_path = Path(output_path)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    img.save(output_path)
    return output_path


def render_carved_line(
    base_column: ColumnSpans,
    carvers: Sequence,
    output_path: str | Path,
    *,
    wz: int = 0,
    x_start: int = 0,
    length: int = 256,
    seed: int = 1337,
    **render_kwargs,
) -> Path:
    """Convenience: carve a horizontal line and render it in one call.

    Applies the ``carvers`` chain across ``x ∈ [x_start, x_start+length)`` at a
    fixed ``wz`` over ``base_column`` and renders the resulting slice.  Used by
    the §6.1 round-by-round evidence scripts.
    """
    line: list[ColumnSpans] = []
    for wx in range(x_start, x_start + length):
        col = base_column
        for carver in carvers:
            col = carver.carve_column(col, wx, wz, seed)
        line.append(col)
    return render_cross_section(line, output_path, **render_kwargs)
