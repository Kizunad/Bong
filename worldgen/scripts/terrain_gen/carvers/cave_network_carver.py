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

from .base import SEED_SALT, BaseCarver, SolidColumn
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
    ) -> None:
        self.surface_cap = max(2, surface_cap)
        self.floor_buffer = max(2, floor_buffer)
        self.carve_band = carve_band
        self.scale = scale
        self.layers = max(1, layers)

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
                seed=seed + 41 + layer * 7,
            )
            carve_mask |= (v >= lo) & (v <= hi)

        # Apply the carve only where the column is currently solid.
        lo_idx = carve_bottom - SPAN_MIN_Y
        hi_idx = carve_top - SPAN_MIN_Y
        band = col.mask[lo_idx : hi_idx + 1]
        band[carve_mask] = False
