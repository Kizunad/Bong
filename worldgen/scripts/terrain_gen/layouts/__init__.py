"""Deterministic building layout system for terrain generation.

Provides LayoutSpec / Placement dataclasses and a runner that places
structures relative to POI center coordinates from BlueprintZone.pois.

COMPOUND_LAYOUT_REGISTRY maps the ``architectural_layout`` string used in
terrain-profiles.example.json / zones.worldview.example.json to the
corresponding LayoutSpec.  Keys must match exactly.  The registry is
intentionally named to avoid collision with the existing ``LAYER_REGISTRY``
in ``scripts.terrain_gen.fields``.
"""

from .base import LayoutResult, LayoutSpec, Placement, PlacedStructure
from .dan_zong_compound import DAN_ZONG_COMPOUND_LAYOUT
from .runner import NbtPasteResult, export_placement_manifest, run_layout
from .wangyintai_compound import WANGYINTAI_COMPOUND_LAYOUT

# Registry: architectural_layout name  →  LayoutSpec
# Add new compound layouts here as they are created.
COMPOUND_LAYOUT_REGISTRY: dict[str, LayoutSpec] = {
    "dan_zong_compound": DAN_ZONG_COMPOUND_LAYOUT,
    "wangyintai_compound": WANGYINTAI_COMPOUND_LAYOUT,
}

__all__ = [
    "COMPOUND_LAYOUT_REGISTRY",
    "LayoutResult",
    "LayoutSpec",
    "NbtPasteResult",
    "Placement",
    "PlacedStructure",
    "export_placement_manifest",
    "run_layout",
]
