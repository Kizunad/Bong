"""worldgen-v4 P3 §8.1 #1 — span carvers package.

3D-noise span sculptors that turn the 2.5D folded span columns into奇幻 3D
地貌 (overhang / floating isle / arch / cave network) **without adding any new
raster layer** — every carver only ever mutates ``ColumnSpans``.

Public surface:
  * framework — ``Carver`` protocol, ``BaseCarver``, ``SolidColumn``,
    ``apply_carver_chain`` (the post-fold pipeline hook), ``validate_spans``;
  * carvers — ``CanyonCarver`` / ``FloatingIslandCarver`` / ``ArchCarver`` /
    ``CaveNetworkCarver``;
  * ``CARVER_REGISTRY`` / ``build_carver`` — name → constructor, so a terrain
    style can declare its carver chain by ``kind`` string (wired into profiles
    in the next P3 segment).
"""

from __future__ import annotations

from typing import Any, Mapping

from .arch_carver import ArchCarver
from .base import (
    BaseCarver,
    Carver,
    SolidColumn,
    apply_carver_chain,
    clamp_y,
    validate_spans,
)
from .canyon_carver import CanyonCarver
from .cave_network_carver import CaveNetworkCarver
from .floating_island_carver import FloatingIslandCarver

CARVER_REGISTRY: dict[str, type[BaseCarver]] = {
    CanyonCarver.name: CanyonCarver,
    FloatingIslandCarver.name: FloatingIslandCarver,
    ArchCarver.name: ArchCarver,
    CaveNetworkCarver.name: CaveNetworkCarver,
}


def build_carver(kind: str, params: Mapping[str, Any] | None = None) -> BaseCarver:
    """Construct a carver by registry ``kind`` with optional kwargs.

    Raises ``KeyError`` with the available kinds if ``kind`` is unknown — fail
    loud at chain-assembly time rather than silently dropping a carver.
    """
    if kind not in CARVER_REGISTRY:
        available = ", ".join(sorted(CARVER_REGISTRY))
        raise KeyError(
            f"unknown carver kind {kind!r}; available: {available}"
        )
    return CARVER_REGISTRY[kind](**(dict(params) if params else {}))


__all__ = [
    "ArchCarver",
    "BaseCarver",
    "CARVER_REGISTRY",
    "CanyonCarver",
    "Carver",
    "CaveNetworkCarver",
    "FloatingIslandCarver",
    "SolidColumn",
    "apply_carver_chain",
    "build_carver",
    "clamp_y",
    "validate_spans",
]
