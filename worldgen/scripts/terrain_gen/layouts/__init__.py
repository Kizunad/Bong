"""Deterministic building layout system for terrain generation.

Provides LayoutSpec / Placement dataclasses and a runner that places
structures relative to POI center coordinates from BlueprintZone.pois.
"""

from .base import LayoutResult, LayoutSpec, Placement, PlacedStructure
from .runner import run_layout

__all__ = [
    "LayoutResult",
    "LayoutSpec",
    "Placement",
    "PlacedStructure",
    "run_layout",
]
