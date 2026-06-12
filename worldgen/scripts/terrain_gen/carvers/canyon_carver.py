"""worldgen-v4 P3 — canyon overhang carver.

Sculpts the wall of a canyon so it is not a clean vertical cliff but a 3D
overhanging wall: an upper **rock shelf** juts out over the void, with open air
below it, then the canyon floor far below.  A wall column therefore folds to a
multi-span列:

    span[0] = upper shelf (the overhang lip — its ceiling is the original
              surface, so query_surface still lands on the rim walk)
    span[1] = canyon floor remnant below the void

The void band between them is the air a creature would fall through if it
stepped off the overhang.  Columns away from the wall (low ``wall_strength``)
are left as a single solid span — only the steep canyon flank overhangs.

The overhang depth + lip thickness are driven by 3D noise (``fbm_3d`` keyed on
world X/Y/Z) so the wall undulates vertically into genuine overhangs instead of
a flat ledge at one height.
"""

from __future__ import annotations

import numpy as np

from .base import SEED_SALT, BaseCarver, SolidColumn, clamp_y, validate_spans
from ..fields import SPAN_MIN_Y
from ..noise import fbm_3d, warped_fbm_3d


class CanyonCarver(BaseCarver):
    """Carve an overhanging shelf + void + floor into canyon-wall columns.

    Parameters
    ----------
    wall_threshold:
        Minimum ``wall_strength`` (0..1, passed per column via the carver chain
        or sampled from noise) for a column to overhang at all.
    void_height:
        Nominal vertical thickness of the air band under the shelf.
    shelf_thickness:
        Nominal vertical thickness of the rock shelf (overhang lip).
    scale:
        3D-noise feature size controlling how fast the overhang depth varies
        with height.
    """

    name = "canyon"
    salt = SEED_SALT["canyon"]

    def __init__(
        self,
        wall_threshold: float = 0.35,
        void_height: int = 14,
        shelf_thickness: int = 8,
        scale: float = 70.0,
        wall_depth_band: tuple[int, int] | None = None,
    ) -> None:
        self.wall_threshold = wall_threshold
        self.void_height = max(3, void_height)
        self.shelf_thickness = max(2, shelf_thickness)
        self.scale = scale
        # Optional [lo, hi] surface-Y gate: a profile sets this to the valley
        # *wall* band (between floor and rim) so overhangs hug the slope and
        # never appear on the flat plateau top or the valley floor.  None (the
        # default, used by the standalone carver tests) means "any surface Y".
        if wall_depth_band is not None:
            lo, hi = wall_depth_band
            if lo > hi:
                lo, hi = hi, lo
            wall_depth_band = (int(lo), int(hi))
        self.wall_depth_band = wall_depth_band

    def _wall_strength(self, wx: int, wz: int, surface_y: int, seed: int) -> float:
        # 2.5D wall-likelihood from warped 3D noise sampled at the surface; the
        # canyon flank is where this rises above the threshold.  In standalone
        # (no-profile) use the noise stands in for "near the rift wall"; once a
        # profile drives it, it can pass valley_strength in directly.
        raw = warped_fbm_3d(
            float(wx), float(surface_y), float(wz), scale=160.0, seed=seed + 11
        )
        return float((raw + 1.0) * 0.5)  # → [0,1]

    def _gate_mask(self, wx, surface_y, wz, seed, active):
        # Vectorize the two cheap gates ``_carve`` applies before any expensive
        # punch: (1) surface inside the optional wall depth band, (2) wall
        # strength >= threshold.  Same fields/comparisons as the scalar path, so
        # the gate is an exact carve-able superset (only surface-bearing columns
        # can overhang, hence the ``active`` factor).
        gate = np.asarray(active, dtype=bool)
        if self.wall_depth_band is not None:
            lo, hi = self.wall_depth_band
            gate = gate & (surface_y >= lo) & (surface_y <= hi)
        raw = warped_fbm_3d(
            np.asarray(wx, dtype=np.float64),
            np.asarray(surface_y, dtype=np.float64),
            np.asarray(wz, dtype=np.float64),
            scale=160.0, seed=seed + 11,
        )
        strength = (raw + 1.0) * 0.5
        return gate & (strength >= self.wall_threshold)

    def _carve(self, col: SolidColumn, wx: int, wz: int, seed: int) -> None:
        surface_y = col.surface_y
        if surface_y is None:
            return  # void column — nothing to overhang
        if self.wall_depth_band is not None:
            lo, hi = self.wall_depth_band
            if not (lo <= surface_y <= hi):
                return  # plateau top or valley floor — not the wall, no overhang
        strength = self._wall_strength(wx, wz, surface_y, seed)
        if strength < self.wall_threshold:
            return  # not a wall — leave the solid column alone

        # How deep below the surface the overhang lip sits, modulated by 3D
        # noise so the shelf rises and dips along the wall.
        #
        # worldgen-v4 P3 §6.1 悬壁自然化: the depth/void noise is read at the
        # column's *surface_y*, which varies smoothly along the wall, so adjacent
        # columns used to sample nearly-identical noise → the cut depth barely
        # changed column-to-column and the overhang read as a flat horizontal
        # WorldEdit-style ledge (同高列切口趋同成水平条纹).  Fix: (1) bigger noise
        # amplitude (±9 lip / ±11 void instead of ±4 / ±5) so the depth swings
        # harder, and (2) a high-frequency lateral PHASE added to the noise's
        # vertical sample coordinate — ``lat_phase`` decorrelates neighbouring
        # columns (it shifts fast along x/z) so same-surface-height columns no
        # longer align on one cut depth.  The cut now staggers high/low into a
        # natural broken cliff.  ``lat_phase`` is a pure function of (wx, wz) so
        # the scalar and vectorized paths stay byte-identical.
        lat_phase = wx * 11.0 - wz * 7.0
        depth_noise = fbm_3d(
            float(wx), float(surface_y) + lat_phase, float(wz),
            scale=self.scale, seed=seed + 23,
        )
        lip_drop = self.shelf_thickness + int(round((depth_noise + 1.0) * 9.0))
        # The shelf occupies [shelf_floor, surface_y]; the void is the band of
        # air immediately under it; the floor remnant is everything below.
        shelf_floor = surface_y - lip_drop
        void_noise = fbm_3d(
            float(wx), float(surface_y - lip_drop) + lat_phase, float(wz),
            scale=self.scale, seed=seed + 37,
        )
        void_h = self.void_height + int(round((void_noise + 1.0) * 11.0))
        floor_ceiling = shelf_floor - void_h

        # Only carve if there's room for a real floor remnant below the void —
        # otherwise the column would lose its ground entirely.
        if floor_ceiling <= SPAN_MIN_Y + 4:
            return
        if shelf_floor <= floor_ceiling + 1:
            return  # void collapsed — keep the solid column

        # Punch the air band: clear [floor_ceiling+1, shelf_floor-1].
        col.fill_range(clamp_y(floor_ceiling + 1), clamp_y(shelf_floor - 1), False)

    def carve_tile(self, columns, wx, wz, seed):
        """Fully-vectorized tile carve — identical output to the scalar loop.

        Computes wall-strength / lip-depth / void noise once over the tile and
        derives ``shelf_floor`` / ``floor_ceiling`` arrays with the *same*
        arithmetic + ``np.rint`` as ``_carve``, then punches the air band only on
        gated, geometrically-valid columns.  The scalar path and this share the
        gate (``_gate_mask``), so the wall-strength comparison is identical.
        """
        salted = self._seed_for(seed)
        wx = np.asarray(wx, dtype=np.float64)
        wz = np.asarray(wz, dtype=np.float64)
        n = len(columns)
        surface = np.full(n, -1, dtype=np.int64)
        active = np.zeros(n, dtype=bool)
        for i, col in enumerate(columns):
            s = col.surface_ceiling_y
            if s is not None:
                surface[i] = s
                active[i] = True
        gate = self._gate_mask(wx, surface.astype(np.float64), wz, salted, active)

        # Mirror the scalar lateral-phase decorrelation + larger amplitude (P3
        # §6.1 悬壁自然化) bit-for-bit so the vectorized carve stays byte-identical.
        lat_phase = wx * 11.0 - wz * 7.0
        surf_f = surface.astype(np.float64)
        depth_noise = fbm_3d(
            wx, surf_f + lat_phase, wz, scale=self.scale, seed=salted + 23
        )
        lip_drop = self.shelf_thickness + np.rint(
            (depth_noise + 1.0) * 9.0
        ).astype(np.int64)
        shelf_floor = surface - lip_drop
        void_noise = fbm_3d(
            wx, (surface - lip_drop).astype(np.float64) + lat_phase, wz,
            scale=self.scale, seed=salted + 37,
        )
        void_h = self.void_height + np.rint(
            (void_noise + 1.0) * 11.0
        ).astype(np.int64)
        floor_ceiling = shelf_floor - void_h
        valid = (
            gate
            & (floor_ceiling > SPAN_MIN_Y + 4)
            & (shelf_floor > floor_ceiling + 1)
        )

        out: list = []
        for i, column in enumerate(columns):
            if not valid[i]:
                out.append(column)
                continue
            col = SolidColumn.from_spans(column)
            col.fill_range(
                clamp_y(int(floor_ceiling[i]) + 1),
                clamp_y(int(shelf_floor[i]) - 1),
                False,
            )
            out.append(validate_spans(col.to_spans()))
        return out
