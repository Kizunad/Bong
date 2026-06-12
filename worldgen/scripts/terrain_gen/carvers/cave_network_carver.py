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

Performance — vertical coarse sampling
--------------------------------------
The dominant cost is sampling the layered 3D density over the *whole* cave
volume (``depth × columns × layers`` gradient-noise evaluations).  At the cave
feature ``scale`` (≈28 blocks) the density varies smoothly along Y, so we sample
it only every ``y_step`` blocks on an **absolute world-Y grid** and linearly
interpolate the in-between blocks.  Keying the sample grid on absolute world Y
(``carve_bottom`` + k·step) — not a per-column relative index — is what keeps the
scalar ``_carve`` and the vectorized ``carve_tile`` byte-identical: both reach
for the same world-Y sample anchors regardless of how deep their own column runs.
With ``y_step=4`` this drops the Y noise work ~4× (and a single density octave
halves it again) for a visually indistinguishable gallery silhouette — the carve
band is a threshold on a smooth field, so sub-``y_step`` wobble is below one
block of cave wall.
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
    octaves:
        fbm octaves per density layer.  A single octave winds convincingly at
        this feature size; the default keeps the cave volume cheap.
    y_step:
        Sample the density on an absolute world-Y grid every ``y_step`` blocks
        and linearly interpolate between anchors.  ``1`` = sample every block
        (exact, slowest).  Larger steps trade an imperceptible amount of cave
        wall wobble for a proportional drop in noise cost.
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
        y_step: int = 1,
        xz_step: int = 1,
    ) -> None:
        self.surface_cap = max(2, surface_cap)
        self.floor_buffer = max(2, floor_buffer)
        self.carve_band = carve_band
        self.scale = scale
        self.layers = max(1, layers)
        # Cave density only needs a couple of octaves to wind convincingly; the
        # default keeps the noise cost over the full cave volume — the dominant
        # export expense — low with no visible loss in the gallery shapes.
        # Scalar + vectorized paths share this so they stay byte-identical.
        self.octaves = max(1, octaves)
        self.y_step = max(1, y_step)
        # Coarse density sampling on an absolute world lattice + interpolation:
        # ``y_step`` strides the vertical axis, ``xz_step`` strides the two
        # horizontal axes.  Together they sample the density on a coarse 3D
        # lattice and trilinearly interpolate the in-between blocks.  Keying the
        # lattice on absolute world (x, y, z) — never a tile-relative index —
        # keeps the scalar single-column and vectorized whole-tile paths
        # byte-identical: both reach for the same lattice anchors around any
        # given world coordinate.
        self.xz_step = max(1, xz_step)

    @staticmethod
    def _axis_anchors(coords: np.ndarray, step: int) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
        """Absolute-lattice anchor bracket + lerp weight for each world coordinate.

        Returns ``(anchor_values, idx_into_unique, weight)`` where every input
        coordinate ``c`` is bracketed by lattice points ``a0 = floor(c/step)·step``
        and ``a1 = a0 + step`` with weight ``(c - a0)/step``.  ``anchor_values``
        is the sorted unique set of all bracketing lattice points (so the noise
        is sampled once per unique anchor, not once per column), and ``idx`` maps
        each coordinate to the position of its ``a0`` within ``anchor_values``
        (its ``a1`` is the next entry).  Because the lattice is the absolute
        world grid, a single column and a whole tile that contain the same world
        coordinate resolve to the identical anchors + weight.
        """
        a0 = np.floor(coords / step) * step
        weight = (coords - a0) / step
        # A CONTIGUOUS anchor lattice from the lowest to the highest bracket,
        # spaced by ``step``, so a0 and its partner a0+step are always adjacent
        # entries (idx, idx+1) — sampling only the lattice points the columns
        # actually bracket, but never letting a sparse gap split a bracket pair.
        lo_a = float(a0.min())
        hi_a = float(a0.max()) + step
        anchors = np.arange(lo_a, hi_a + 0.5 * step, step, dtype=np.float64)
        idx = np.rint((a0 - lo_a) / step).astype(np.int64)
        return anchors, idx, weight

    # ------------------------------------------------------------------
    # Shared density core — both the scalar and vectorized carve paths call
    # this so they are byte-identical by construction.
    # ------------------------------------------------------------------
    def _carve_band_mask(
        self,
        wx: np.ndarray,
        wz: np.ndarray,
        carve_bottom: int,
        carve_top: int,
        seed: int,
    ) -> np.ndarray:
        """Boolean ``(depth, ncols)`` air mask over world-Y ``[carve_bottom, carve_top]``.

        ``wx`` / ``wz`` are flat per-column coordinate arrays (length ``ncols``).
        Returns a mask whose row ``r`` is world-Y ``carve_bottom + r`` and whose
        column ``c`` is the input column ``c``.  The density is sampled on an
        **absolute** world-Y anchor grid (``carve_bottom`` stepped by
        ``y_step``) and linearly interpolated — keying on absolute Y (not a
        relative index) makes the scalar single-column and the vectorized
        whole-tile callers sample the identical anchors, so their carve masks
        match block-for-block.
        """
        depth = carve_top - carve_bottom + 1
        ncols = wx.shape[0]
        if depth <= 0 or ncols == 0:
            return np.zeros((max(depth, 0), ncols), dtype=bool)

        # --- Vertical anchors: carve_bottom + k·y_step, stepping PAST carve_top
        # so the final lerp segment always brackets the top row.  The anchor
        # OFFSETS ``{0, y_step, 2·y_step, …}`` do NOT depend on carve_top, so a
        # shorter column is an exact prefix of a taller one — that prefix
        # property is what makes the scalar ``_carve`` (per-column top) and the
        # vectorized ``carve_tile`` (global top, then sliced per column)
        # byte-identical for every column.
        last_off = ((depth - 1) // self.y_step) * self.y_step
        if last_off < depth - 1:
            last_off += self.y_step
        y_off = np.arange(0, last_off + 1, self.y_step)
        ys_anchor = (carve_bottom + y_off).astype(np.float64)
        ny = y_off.shape[0]
        rows = np.arange(depth)
        yseg = np.clip(np.searchsorted(y_off, rows, side="right") - 1, 0, ny - 2)
        yw = ((rows - y_off[yseg]) / np.maximum(y_off[yseg + 1] - y_off[yseg], 1)).astype(
            np.float64
        )

        # --- Horizontal anchors on the absolute world lattice (floor(c/step)·step
        # bracket per coordinate, with the noise sampled once per UNIQUE anchor).
        xa, xidx, xw = self._axis_anchors(wx, self.xz_step)
        za, zidx, zw = self._axis_anchors(wz, self.xz_step)
        nxa = xa.shape[0]
        nza = za.shape[0]

        lo, hi = self.carve_band
        mask = np.zeros((depth, ncols), dtype=bool)
        # Lattice grid for the noise sample: (ny, nza, nxa).
        xa_g = xa[None, None, :]
        za_g = za[None, :, None]
        for layer in range(self.layers):
            ys_layer = (ys_anchor * (1.0 + 0.35 * layer))[:, None, None]
            sampled = fbm_3d(
                np.broadcast_to(xa_g, (ny, nza, nxa)),
                np.broadcast_to(ys_layer, (ny, nza, nxa)),
                np.broadcast_to(za_g, (ny, nza, nxa)),
                scale=self.scale * (1.0 + 0.25 * layer),
                octaves=self.octaves,
                seed=seed + 41 + layer * 7,
            )
            # Trilinear gather → (ny, ncols): bilinear over the X/Z lattice per
            # column, then it stays on the coarse Y anchors for the Y lerp below.
            s00 = sampled[:, zidx, xidx]
            s10 = sampled[:, zidx, xidx + 1]
            s01 = sampled[:, zidx + 1, xidx]
            s11 = sampled[:, zidx + 1, xidx + 1]
            sx0 = s00 * (1.0 - xw)[None, :] + s10 * xw[None, :]
            sx1 = s01 * (1.0 - xw)[None, :] + s11 * xw[None, :]
            anchor_col = sx0 * (1.0 - zw)[None, :] + sx1 * zw[None, :]  # (ny, ncols)
            # Linearly interpolate the coarse-Y anchors back to every world-Y row.
            v = anchor_col[yseg] * (1.0 - yw)[:, None] + anchor_col[yseg + 1] * yw[:, None]
            mask |= (v >= lo) & (v <= hi)
        return mask

    def _carve(self, col: SolidColumn, wx: int, wz: int, seed: int) -> None:
        surface_y = col.surface_y
        if surface_y is None:
            return
        carve_top = surface_y - self.surface_cap
        carve_bottom = SPAN_MIN_Y + self.floor_buffer
        if carve_top <= carve_bottom + 2:
            return  # column too shallow to host caves

        mask = self._carve_band_mask(
            np.array([float(wx)]),
            np.array([float(wz)]),
            carve_bottom,
            carve_top,
            seed,
        )[:, 0]

        # Apply the carve only where the column is currently solid.
        lo_idx = carve_bottom - SPAN_MIN_Y
        hi_idx = carve_top - SPAN_MIN_Y
        band = col.mask[lo_idx : hi_idx + 1]
        band[mask] = False

    def carve_tile(self, columns, wx, wz, seed):
        """Fully-vectorized tile carve — identical output to the scalar loop.

        Samples each layer's density once over a global ``(Y, column)`` grid (the
        carve band is ``[floor_buffer .. max_surface - surface_cap]``) via the
        shared :meth:`_carve_band_mask`, then masks each column to its own
        ``[carve_bottom .. surface - cap]`` band and carves air where any layer's
        density falls in the window.  Both paths anchor the coarse-Y sampling on
        absolute world Y, so the result is byte-identical to the scalar
        ``_carve`` — just ``layers`` vectorized fbm calls instead of 40k × layers
        scalar ones.
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

        global_top = int(carve_top_per[carveable].max())
        if global_top <= carve_bottom:
            return list(columns)

        carve_grid = self._carve_band_mask(
            wx_sel, wz_sel, carve_bottom, global_top, salted
        )

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
