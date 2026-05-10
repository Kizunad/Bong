from __future__ import annotations

from .ascension_pit import XUJIE_CANXIE_ITEM_ID, ascension_pits_for_zone
from .corpse_mound import corpse_mound_loot_pool, corpse_mounds_for_zone
from .whale_fossil import fossil_bboxes_for_zone, rasterize_whale_fossil_mask

__all__ = [
    "XUJIE_CANXIE_ITEM_ID",
    "ascension_pits_for_zone",
    "corpse_mound_loot_pool",
    "corpse_mounds_for_zone",
    "fossil_bboxes_for_zone",
    "rasterize_whale_fossil_mask",
]
