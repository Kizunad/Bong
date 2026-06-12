"""worldgen-v4 P3 — stone arch carver.

Punches a doorway/arch through a solid wall: a sky-facing opening with a solid
arch roof spanning over it.  An arch column folds to:

    span[0] = ground beneath the opening (the threshold you walk on)
    span[1] = the arch roof above the opening

The opening is a 3D-noise blob (so the doorway has an organic, slightly
asymmetric mouth) carved out of the wall between the ground and a roof band kept
solid up to the original surface.  Columns where the opening noise stays below
the threshold remain a single solid span — the arch is a localized hole, the
wall around it intact.

The roof band thickness is height-modulated by 3D noise so the arch crown is not
a flat lintel but a curved span.
"""

from __future__ import annotations

from .base import SEED_SALT, BaseCarver, SolidColumn, clamp_y
from ..fields import SPAN_MIN_Y
from ..noise import fbm_3d, warped_fbm_3d


class ArchCarver(BaseCarver):
    """Carve a doorway opening + solid roof into wall columns.

    Parameters
    ----------
    opening_threshold:
        Minimum opening-noise value (after [0,1] remap) for a column to host an
        arch mouth.
    opening_height:
        Nominal vertical size of the opening band.
    base_clearance:
        Minimum solid threshold kept above bedrock so the column always retains
        a ground span to stand on.
    roof_thickness:
        Nominal solid roof band kept between the opening top and the surface.
    """

    name = "arch"
    salt = SEED_SALT["arch"]

    def __init__(
        self,
        opening_threshold: float = 0.54,
        opening_height: int = 20,
        roof_thickness: int = 8,
        scale: float = 70.0,
    ) -> None:
        self.opening_threshold = opening_threshold
        self.opening_height = max(4, opening_height)
        self.roof_thickness = max(2, roof_thickness)
        self.scale = scale

    def _opening(self, wx: int, wz: int, surface_y: int, seed: int) -> float:
        raw = warped_fbm_3d(
            float(wx), float(surface_y), float(wz),
            scale=self.scale, warp_scale=110.0, warp_strength=26.0, seed=seed + 3,
        )
        return float((raw + 1.0) * 0.5)

    def _carve(self, col: SolidColumn, wx: int, wz: int, seed: int) -> None:
        surface_y = col.surface_y
        if surface_y is None:
            return
        opening = self._opening(wx, wz, surface_y, seed)
        if opening < self.opening_threshold:
            return  # solid wall here — no doorway

        # A doorway/window punched through the UPPER wall: the lower wall stays
        # as the ground span you walk through, a solid roof band caps it at the
        # surface, and the air window is the band between them. Keeping the
        # opening near the surface (not from bedrock) makes a walk-through arch
        # rather than a column-tall slot.
        mouth = (opening - self.opening_threshold) / max(
            1.0 - self.opening_threshold, 1e-6
        )
        h_noise = fbm_3d(
            float(wx), float(surface_y), float(wz), scale=self.scale, seed=seed + 13
        )
        opening_h = self.opening_height + int(round(mouth * 14.0 + h_noise * 4.0))

        roof_floor = clamp_y(surface_y - self.roof_thickness)
        opening_top = roof_floor - 1
        opening_bottom = clamp_y(opening_top - opening_h)

        # Need a real opening AND both a ground span (below the opening) and a
        # roof span (the band [roof_floor, surface_y]) left.  By construction the
        # roof sits directly above opening_top (opening_top == roof_floor - 1),
        # so the roof span is valid as long as roof_floor <= surface_y.
        if opening_bottom <= SPAN_MIN_Y + 2:
            return  # opening would eat the ground — leave the wall solid
        if opening_top <= opening_bottom + 1:
            return  # opening collapsed
        if roof_floor > surface_y:
            return  # no roof band (roof_thickness ate past the surface)

        col.fill_range(opening_bottom, opening_top, False)
