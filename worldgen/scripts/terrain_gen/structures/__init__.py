from __future__ import annotations

from .corpse_mound import corpse_mound_loot_pool, corpse_mounds_for_zone
from .whale_fossil import fossil_bboxes_for_zone, rasterize_whale_fossil_mask

__all__ = [
    "corpse_mound_loot_pool",
    "corpse_mounds_for_zone",
    "fossil_bboxes_for_zone",
    "rasterize_whale_fossil_mask",
]
