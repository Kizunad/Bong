"""worldgen-v4 P3 — multi-layer cave network carver.

Carves several stacked air galleries below the surface so a column folds into
multiple solid segments separated by void — a vertically connected cave system
rather than the single cave void P0 already folds:

    span[0] = surface cap (walkable ground — query_surface unchanged)
    span[1] = an intermediate solid floor between two galleries
    span[2] = a lower solid floor
    span[3] = bedrock floor remnant

The galleries are carved by a 3D-noise *density* field: blocks below the surface
whose ``fbm_3d`` value falls in a carve band become air, so the caverns wind and
connect in 3D instead of being flat slabs.  Because the framework budgets to
MAX_SPANS=4 by merging the thinnest gaps, deep columns with many noise pockets
collapse gracefully to the four most prominent solid layers — never an illegal
column.
"""

from __future__ import annotations

import numpy as np

from .base import SEED_SALT, BaseCarver, SolidColumn, validate_spans
from ..fields import SPAN_MIN_Y
from ..noise import fbm_3d


class CaveNetworkCarver(BaseCarver):
    """Carve stacked 3D-noise galleries below the surface.

    Parameters
    ----------
    surface_cap:
        Solid blocks kept directly under the surface so the ground never opens
        straight to sky (the cave roof).
    floor_buffer:
        Solid blocks kept above bedrock so the bottom gallery has a floor.
    carve_band:
        ``(low, high)`` density window — noise values inside the window become
        air.  A wider window = more open caves.
    scale:
        3D-noise feature size — smaller = tighter, more numerous galleries.
    layers:
        Number of independent noise octave-bands stacked to create multiple
        vertically separated galleries.
    """

    name = "cave_network"
    salt = SEED_SALT["cave_network"]

    def __init__(
        self,
        surface_cap: int = 5,
        floor_buffer: int = 6,
        carve_band: tuple[float, float] = (-0.06, 0.06),
        scale: float = 30.0,
        layers: int = 2,
        octaves: int = 2,
    ) -> None:
        self.surface_cap = max(2, surface_cap)
        self.floor_buffer = max(2, floor_buffer)
        self.carve_band = carve_band
        self.scale = scale
        self.layers = max(1, layers)
        # Cave density only needs a couple of octaves to wind convincingly; 2
        # (vs the fbm default 4) roughly halves the noise cost over the full
        # cave volume — the dominant export expense — with no visible loss in
        # the gallery shapes.  Scalar + vectorized paths share this so they stay
        # byte-identical.
        self.octaves = max(1, octaves)

    def _carve(self, col: SolidColumn, wx: int, wz: int, seed: int) -> None:
        surface_y = col.surface_y
        if surface_y is None:
            return
        carve_top = surface_y - self.surface_cap
        carve_bottom = SPAN_MIN_Y + self.floor_buffer
        if carve_top <= carve_bottom + 2:
            return  # column too shallow to host caves

        lo, hi = self.carve_band
        # Vectorized over the whole cave band: sample the layered 3D noise for
        # every Y at once (one fbm_3d call per layer over a Y vector) instead of
        # a per-block Python loop — the per-block loop is ~500×layers scalar
        # noise evals per column and made full-tile carving impractically slow.
        # The result is identical: a block carves to air where ANY layer's noise
        # falls inside the density window.
        ys = np.arange(carve_bottom, carve_top + 1, dtype=np.float64)
        wx_v = np.full(ys.shape, float(wx))
        wz_v = np.full(ys.shape, float(wz))
        carve_mask = np.zeros(ys.shape, dtype=bool)
        for layer in range(self.layers):
            # Each layer uses a different vertical frequency so the galleries
            # appear at different depths, then merge where they overlap.
            v = fbm_3d(
                wx_v,
                ys * (1.0 + 0.35 * layer),
                wz_v,
                scale=self.scale * (1.0 + 0.25 * layer),
                octaves=self.octaves,
                seed=seed + 41 + layer * 7,
            )
            carve_mask |= (v >= lo) & (v <= hi)

        # Apply the carve only where the column is currently solid.
        lo_idx = carve_bottom - SPAN_MIN_Y
        hi_idx = carve_top - SPAN_MIN_Y
        band = col.mask[lo_idx : hi_idx + 1]
        band[carve_mask] = False

    def carve_tile(self, columns, wx, wz, seed):
        """Fully-vectorized tile carve — identical output to the scalar loop.

        Samples each layer's density noise once over a global ``(Y, column)``
        grid (the carve band is ``[floor_buffer .. max_surface - surface_cap]``),
        then masks each column to its own ``[carve_bottom .. surface - cap]`` band
        and carves air where any layer's noise falls in the density window.  The
        noise coords + window are identical to the per-column ``_carve``, so the
        result is byte-identical — just ``layers`` vectorized fbm calls instead
        of 40k × layers scalar ones.
        """
        salted = self._seed_for(seed)
        wx = np.asarray(wx, dtype=np.float64)
        wz = np.asarray(wz, dtype=np.float64)
        n = len(columns)
        surface = np.full(n, -1, dtype=np.int64)
        for i, col in enumerate(columns):
            s = col.surface_ceiling_y
            if s is not None:
                surface[i] = s

        carve_bottom = SPAN_MIN_Y + self.floor_buffer
        carve_top_per = surface - self.surface_cap
        # Columns deep enough to host caves (same guard as _carve).
        carveable = (surface >= 0) & (carve_top_per > carve_bottom + 2)
        if not np.any(carveable):
            return list(columns)

        # Only build the noise grid over the carveable columns (the rest pass
        # through untouched) — for a cave zone that is most columns, but skipping
        # the shallow rim/edge columns keeps the grid from spanning dead space.
        sel = np.flatnonzero(carveable)
        wx_sel = wx[sel]
        wz_sel = wz[sel]
        m = sel.shape[0]

        global_top = int(carve_top_per[carveable].max())
        if global_top <= carve_bottom:
            return list(columns)
        ys = np.arange(carve_bottom, global_top + 1, dtype=np.float64)
        depth = ys.shape[0]

        # Grid shape (depth, m): broadcast wx/wz across Y, ys across columns.
        ys_g = ys[:, None]
        wx_g = wx_sel[None, :]
        wz_g = wz_sel[None, :]
        lo, hi = self.carve_band
        carve_grid = np.zeros((depth, m), dtype=bool)
        for layer in range(self.layers):
            v = fbm_3d(
                np.broadcast_to(wx_g, (depth, m)),
                np.broadcast_to(ys_g * (1.0 + 0.35 * layer), (depth, m)),
                np.broadcast_to(wz_g, (depth, m)),
                scale=self.scale * (1.0 + 0.25 * layer),
                octaves=self.octaves,
                seed=salted + 41 + layer * 7,
            )
            carve_grid |= (v >= lo) & (v <= hi)

        # Map selected-column index → its grid column for the carve loop.
        grid_col_of = {int(col_idx): j for j, col_idx in enumerate(sel)}
        out: list = []
        for i, column in enumerate(columns):
            if not carveable[i]:
                out.append(column)
                continue
            j = grid_col_of[i]
            carve_top = int(carve_top_per[i])
            # This column's band within the global grid (its top may be below
            # the global top — slice the grid to its own [bottom, top]).
            n_band = carve_top - carve_bottom + 1
            carve_mask = carve_grid[:n_band, j]
            col = SolidColumn.from_spans(column)
            lo_idx = carve_bottom - SPAN_MIN_Y
            hi_idx = carve_top - SPAN_MIN_Y
            band = col.mask[lo_idx : hi_idx + 1]
            band[carve_mask] = False
            out.append(validate_spans(col.to_spans()))
        return out
