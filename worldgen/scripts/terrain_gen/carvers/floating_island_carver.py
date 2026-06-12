"""worldgen-v4 P3 — floating island carver.

Lifts a detached solid island body high above the ground and sculpts its
underside into hanging stalactite垂坠 (irregular drip-points) with 3D noise.  A
column under an island folds to:

    span[0] = ground (the original surface — query_surface stays on the ground)
    span[1] = the floating island body, far above with open air between

Where the island silhouette (a warped 3D-noise blob anchored at a target
altitude) is solid, the column grows an isle span; its **floor** is pushed up
unevenly by a second 3D-noise field so the bottom hangs in irregular drips
rather than a flat slab.  Columns outside the silhouette keep only their ground
span — the island is sparse, an archipelago in the sky, not a ceiling.
"""

from __future__ import annotations

import numpy as np

from .base import SEED_SALT, BaseCarver, SolidColumn, clamp_y, validate_spans
from ..fields import SPAN_MAX_Y, SPAN_MIN_Y
from ..noise import fbm_3d, warped_fbm_3d


class FloatingIslandCarver(BaseCarver):
    """Grow a detached island span above ground with a stalactite underside.

    Parameters
    ----------
    altitude:
        Target world-Y of the island core (island bodies cluster around here).
    body_thickness:
        Nominal vertical thickness of the island body.
    silhouette_threshold:
        Minimum island-noise value (after [0,1] remap) for a column to host an
        island — controls how sparse the archipelago is.
    drip_depth:
        Maximum extra blocks the stalactite underside can hang below the body's
        nominal floor.
    """

    name = "floating_island"
    salt = SEED_SALT["floating_island"]

    def __init__(
        self,
        altitude: int = 250,
        body_thickness: int = 22,
        silhouette_threshold: float = 0.55,
        drip_depth: int = 14,
        scale: float = 130.0,
        drip_scale: float = 44.0,
        drip_sharpness: float = 1.0,
    ) -> None:
        self.altitude = clamp_y(altitude)
        self.body_thickness = max(6, body_thickness)
        self.silhouette_threshold = silhouette_threshold
        self.drip_depth = max(2, drip_depth)
        self.scale = scale
        # Underside stalactite feel: ``drip_scale`` is the 3D-noise feature size
        # of the hanging drips (smaller = tighter, more numerous stalactites);
        # ``drip_sharpness`` is the exponent on the [0,1] drip field — >1 biases
        # most columns to hang a little and a few to hang a lot, so the underside
        # reads as spiky 钟乳 points instead of a smooth wavy slab.
        self.drip_scale = max(8.0, drip_scale)
        self.drip_sharpness = max(0.25, drip_sharpness)

    def _silhouette(self, wx: int, wz: int, seed: int) -> float:
        # Warped 2.5D-ish blob sampled at the island altitude → island plan view.
        raw = warped_fbm_3d(
            float(wx), float(self.altitude), float(wz),
            scale=self.scale, warp_scale=180.0, warp_strength=40.0, seed=seed + 5,
        )
        return float((raw + 1.0) * 0.5)

    def _gate_mask(self, wx, surface_y, wz, seed, active):
        # Islands can grow over open sky too, so the gate ignores ``active`` and
        # keys purely on the vectorized silhouette — the exact same field the
        # scalar ``_carve`` thresholds, so the gate is an exact (==) carve-able
        # superset, not merely a heuristic.
        raw = warped_fbm_3d(
            np.asarray(wx, dtype=np.float64),
            np.full(np.shape(wx), float(self.altitude)),
            np.asarray(wz, dtype=np.float64),
            scale=self.scale, warp_scale=180.0, warp_strength=40.0, seed=seed + 5,
        )
        silhouette = (raw + 1.0) * 0.5
        return silhouette >= self.silhouette_threshold

    def _carve(self, col: SolidColumn, wx: int, wz: int, seed: int) -> None:
        silhouette = self._silhouette(wx, wz, seed)
        if silhouette < self.silhouette_threshold:
            return  # open sky here — no island

        # Island top wobbles with altitude noise so islands sit at staggered
        # heights instead of one flat shelf.
        top_noise = fbm_3d(
            float(wx), float(self.altitude), float(wz), scale=200.0, seed=seed + 17
        )
        island_top = clamp_y(self.altitude + int(round(top_noise * 24.0)))
        # Body thickness scales with how deep inside the silhouette we are
        # (centre thick, edge thin → tapered isle).
        core = (silhouette - self.silhouette_threshold) / max(
            1.0 - self.silhouette_threshold, 1e-6
        )
        thickness = int(round(self.body_thickness * (0.4 + 0.6 * core)))
        nominal_floor = island_top - thickness

        # Stalactite underside: a 3D drip field hangs the floor down unevenly.
        # A higher-frequency tight field + a sharpness exponent turns the smooth
        # wave into spiky 钟乳 points — most columns hang a little, a few hang a
        # lot — so the bottom face reads as irregular drips, not a flat slab.
        drip = fbm_3d(
            float(wx), float(nominal_floor), float(wz),
            scale=self.drip_scale, seed=seed + 29,
        )
        drip01 = ((drip + 1.0) * 0.5) ** self.drip_sharpness
        drip_amount = int(round(drip01 * self.drip_depth))
        island_floor = clamp_y(nominal_floor - drip_amount)

        surface_y = col.surface_y
        # Guarantee a real air gap between ground and island; if the ground is
        # somehow already up near the island, skip (no detached isle possible).
        if surface_y is not None and island_floor <= surface_y + 2:
            return
        if island_top <= island_floor:
            return
        if island_top > SPAN_MAX_Y:
            island_top = SPAN_MAX_Y

        col.fill_range(island_floor, island_top, True)

    def carve_tile(self, columns, wx, wz, seed):
        """Fully-vectorized tile carve — identical output to the scalar loop.

        Computes the silhouette / top-wobble / drip noise once over the whole
        tile (3 vectorized ``fbm`` calls instead of 40k scalar ones), derives
        ``island_floor`` / ``island_top`` arrays with the *same* arithmetic +
        ``np.rint`` (banker's rounding, matching Python ``round``) as ``_carve``,
        then builds SolidColumns only for the columns that pass every gate.  The
        per-column work that remains is just the cheap span rasterize/refold.
        """
        salted = self._seed_for(seed)
        wx = np.asarray(wx, dtype=np.float64)
        wz = np.asarray(wz, dtype=np.float64)
        n = len(columns)
        alt = np.full(n, float(self.altitude))

        raw_sil = warped_fbm_3d(
            wx, alt, wz, scale=self.scale,
            warp_scale=180.0, warp_strength=40.0, seed=salted + 5,
        )
        silhouette = (raw_sil + 1.0) * 0.5
        has_isle = silhouette >= self.silhouette_threshold

        top_noise = fbm_3d(wx, alt, wz, scale=200.0, seed=salted + 17)
        island_top = np.clip(
            self.altitude + np.rint(top_noise * 24.0).astype(np.int64),
            SPAN_MIN_Y, SPAN_MAX_Y,
        )
        core = (silhouette - self.silhouette_threshold) / max(
            1.0 - self.silhouette_threshold, 1e-6
        )
        thickness = np.rint(self.body_thickness * (0.4 + 0.6 * core)).astype(np.int64)
        nominal_floor = island_top - thickness

        drip = fbm_3d(
            wx, nominal_floor.astype(np.float64), wz,
            scale=self.drip_scale, seed=salted + 29,
        )
        drip01 = ((drip + 1.0) * 0.5) ** self.drip_sharpness
        drip_amount = np.rint(drip01 * self.drip_depth).astype(np.int64)
        island_floor = np.clip(
            nominal_floor - drip_amount, SPAN_MIN_Y, SPAN_MAX_Y
        )
        island_top = np.minimum(island_top, SPAN_MAX_Y)

        out: list = []
        for i, column in enumerate(columns):
            if not has_isle[i]:
                out.append(column)
                continue
            ifloor = int(island_floor[i])
            itop = int(island_top[i])
            surface_y = column.surface_ceiling_y
            # Same skip guards as the scalar _carve (air gap + valid body).
            if surface_y is not None and ifloor <= surface_y + 2:
                out.append(column)
                continue
            if itop <= ifloor:
                out.append(column)
                continue
            col = SolidColumn.from_spans(column)
            col.fill_range(ifloor, itop, True)
            out.append(validate_spans(col.to_spans()))
        return out
