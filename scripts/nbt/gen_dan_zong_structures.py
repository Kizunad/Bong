"""
gen_dan_zong_structures.py — Generate all 9 丹宗遗园 NBT structure files.

Run from repo root:
    python3 scripts/nbt/gen_dan_zong_structures.py

Output: server/structures/dan_zong/*.nbt
"""

from __future__ import annotations

import os
import sys
import random

# Ensure scripts/ is on path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from nbt.nbt_builder import StructureBuilder

OUT_DIR = os.path.join(os.path.dirname(__file__), "..", "..", "server", "structures", "dan_zong")

# Deterministic seed for reproducible ruins
random.seed(42)


def _ruin_block(primary: str, mossy: str, cracked: str | None = None, ratio: float = 0.3) -> str:
    """Pick a ruin variant with some probability."""
    r = random.random()
    if cracked and r < ratio * 0.4:
        return cracked
    if r < ratio:
        return mossy
    return primary


def _maybe_skip(ratio: float = 0.1) -> bool:
    """Should we skip this block (for ruin gaps)?"""
    return random.random() < ratio


# ════════════════════════════════════════════════════════════════
# 1. 倒塌丹方碑 — fallen_recipe_stele — 1×3×2
# ════════════════════════════════════════════════════════════════

def build_fallen_recipe_stele() -> StructureBuilder:
    """A toppled stone stele lying on its side, 1 wide × 3 tall × 2 deep.
    Represents a recipe tablet that fell face-down over centuries."""
    sb = StructureBuilder(1, 3, 2)

    # Base debris — cracked stone on ground
    sb.set_block(0, 0, 0, "minecraft:mossy_cobblestone")
    sb.set_block(0, 0, 1, "minecraft:gravel")

    # The fallen stele body — chiseled blackstone (the "inscription" surface)
    sb.set_block(0, 1, 0, "minecraft:chiseled_polished_blackstone")
    sb.set_block(0, 1, 1, "minecraft:polished_blackstone")

    # Top — cracked / overgrown
    sb.set_block(0, 2, 0, "minecraft:cracked_stone_bricks")

    return sb


# ════════════════════════════════════════════════════════════════
# 2. 丹师枯骨 — fallen_alchemist_bone — 2×1×3
# ════════════════════════════════════════════════════════════════

def build_fallen_alchemist_bone() -> StructureBuilder:
    """Skeletal remains of a Dan sect alchemist, 2x1x3.
    Bone blocks form the body, skull at one end, a flower pot with dead bush
    represents the pill they were holding."""
    sb = StructureBuilder(2, 1, 3)

    # Torso / spine — bone blocks
    sb.set_block(0, 0, 0, "minecraft:bone_block", {"axis": "z"})
    sb.set_block(0, 0, 1, "minecraft:bone_block", {"axis": "z"})

    # Skull
    sb.set_block(0, 0, 2, "minecraft:skeleton_skull", {"rotation": "0"})

    # Arm reaching out with "pill" (flower pot + dead bush = withered pill herb)
    sb.set_block(1, 0, 1, "minecraft:bone_block", {"axis": "x"})
    sb.set_block(1, 0, 0, "minecraft:flower_pot")

    return sb


# ════════════════════════════════════════════════════════════════
# 3. 蒸气毒泉·小 — vapor_poison_spring_small — 3×2×3
# ════════════════════════════════════════════════════════════════

def build_vapor_poison_spring_small() -> StructureBuilder:
    """Small corner poison spring, 3x2x3.
    Soul sand base with water/bubble column, purple stained glass rim."""
    sb = StructureBuilder(3, 2, 3)

    # Layer 0 (ground) — soul sand basin
    sb.set_block(0, 0, 0, "minecraft:mossy_cobblestone")
    sb.set_block(1, 0, 0, "minecraft:mossy_cobblestone")
    sb.set_block(2, 0, 0, "minecraft:mossy_cobblestone")
    sb.set_block(0, 0, 1, "minecraft:mossy_cobblestone")
    sb.set_block(1, 0, 1, "minecraft:soul_sand")  # center = bubble source
    sb.set_block(2, 0, 1, "minecraft:mossy_cobblestone")
    sb.set_block(0, 0, 2, "minecraft:mossy_cobblestone")
    sb.set_block(1, 0, 2, "minecraft:mossy_cobblestone")
    sb.set_block(2, 0, 2, "minecraft:mossy_cobblestone")

    # Layer 1 — rim of purple glass + center water
    sb.set_block(0, 1, 0, "minecraft:purple_stained_glass")
    sb.set_block(1, 1, 0, "minecraft:purple_stained_glass")
    sb.set_block(2, 1, 0, "minecraft:purple_stained_glass")
    sb.set_block(0, 1, 1, "minecraft:purple_stained_glass")
    sb.set_block(1, 1, 1, "minecraft:water")  # toxic water
    sb.set_block(2, 1, 1, "minecraft:purple_stained_glass")
    sb.set_block(0, 1, 2, "minecraft:purple_stained_glass")
    sb.set_block(1, 1, 2, "minecraft:purple_stained_glass")
    sb.set_block(2, 1, 2, "minecraft:purple_stained_glass")

    return sb


# ════════════════════════════════════════════════════════════════
# 4. 药圃石篱 6×6 — herb_garden_pen_6x6 — 6×2×6
# ════════════════════════════════════════════════════════════════

def build_herb_garden_pen_6x6() -> StructureBuilder:
    """Inner herb garden enclosure, 6x2x6.
    Stone wall perimeter 1 block high, podzol interior for herb planting."""
    sb = StructureBuilder(6, 2, 6)

    # Layer 0 — floor
    for x in range(6):
        for z in range(6):
            if x == 0 or x == 5 or z == 0 or z == 5:
                # Wall foundation
                block = _ruin_block("minecraft:stone_bricks", "minecraft:mossy_stone_bricks",
                                    "minecraft:cracked_stone_bricks", 0.3)
                sb.set_block(x, 0, z, block)
            else:
                # Interior soil
                sb.set_block(x, 0, z, "minecraft:podzol")

    # Layer 1 — wall tops (with ruin gaps)
    for x in range(6):
        for z in range(6):
            if x == 0 or x == 5 or z == 0 or z == 5:
                if not _maybe_skip(0.2):  # 20% chance of missing top block (ruin)
                    block = _ruin_block("minecraft:stone_bricks", "minecraft:mossy_stone_bricks",
                                        "minecraft:cracked_stone_bricks", 0.4)
                    sb.set_block(x, 1, z, block)

    return sb


# ════════════════════════════════════════════════════════════════
# 5. 药圃石篱 8×8 — herb_garden_pen_8x8 — 8×2×8
# ════════════════════════════════════════════════════════════════

def build_herb_garden_pen_8x8() -> StructureBuilder:
    """Outer herb garden enclosure, 8x2x8. Larger version of 6x6."""
    sb = StructureBuilder(8, 2, 8)

    # Layer 0 — floor
    for x in range(8):
        for z in range(8):
            if x == 0 or x == 7 or z == 0 or z == 7:
                block = _ruin_block("minecraft:stone_bricks", "minecraft:mossy_stone_bricks",
                                    "minecraft:cracked_stone_bricks", 0.3)
                sb.set_block(x, 0, z, block)
            else:
                # Interior soil, with occasional gravel debris
                if random.random() < 0.15:
                    sb.set_block(x, 0, z, "minecraft:gravel")
                else:
                    sb.set_block(x, 0, z, "minecraft:podzol")

    # Layer 1 — wall tops with more ruin gaps than 6x6
    for x in range(8):
        for z in range(8):
            if x == 0 or x == 7 or z == 0 or z == 7:
                if not _maybe_skip(0.25):  # 25% gaps
                    block = _ruin_block("minecraft:stone_bricks", "minecraft:mossy_stone_bricks",
                                        "minecraft:cracked_stone_bricks", 0.4)
                    sb.set_block(x, 1, z, block)

    return sb


# ════════════════════════════════════════════════════════════════
# 6. 蒸气毒泉·主 — vapor_poison_spring_main — 5×3×5
# ════════════════════════════════════════════════════════════════

def build_vapor_poison_spring_main() -> StructureBuilder:
    """Main poison spring, 5x3x5. Larger basin with soul sand, water,
    purple glass walls, and a soul campfire for vapor effect."""
    sb = StructureBuilder(5, 3, 5)

    # Layer 0 — foundation
    for x in range(5):
        for z in range(5):
            if x == 0 or x == 4 or z == 0 or z == 4:
                sb.set_block(x, 0, z, "minecraft:mossy_cobblestone")
            else:
                sb.set_block(x, 0, z, "minecraft:soul_sand")

    # Layer 1 — basin walls + water interior
    for x in range(5):
        for z in range(5):
            if x == 0 or x == 4 or z == 0 or z == 4:
                block = _ruin_block("minecraft:polished_blackstone", "minecraft:mossy_cobblestone",
                                    ratio=0.2)
                sb.set_block(x, 1, z, block)
            else:
                sb.set_block(x, 1, z, "minecraft:water")

    # Layer 2 — rim decorations + vapor source
    # Corner accents
    sb.set_block(0, 2, 0, "minecraft:purple_stained_glass")
    sb.set_block(4, 2, 0, "minecraft:purple_stained_glass")
    sb.set_block(0, 2, 4, "minecraft:purple_stained_glass")
    sb.set_block(4, 2, 4, "minecraft:purple_stained_glass")

    # Center vapor (soul campfire for smoke particles)
    sb.set_block(2, 2, 2, "minecraft:soul_campfire", {"lit": "true"})

    # Magenta glass accents on edges
    sb.set_block(2, 2, 0, "minecraft:magenta_stained_glass")
    sb.set_block(2, 2, 4, "minecraft:magenta_stained_glass")
    sb.set_block(0, 2, 2, "minecraft:magenta_stained_glass")
    sb.set_block(4, 2, 2, "minecraft:magenta_stained_glass")

    return sb


# ════════════════════════════════════════════════════════════════
# 7. 露天炼丹大鼎 — ruined_open_furnace — 5×5×5
# ════════════════════════════════════════════════════════════════

def build_ruined_open_furnace() -> StructureBuilder:
    """Half-buried bronze cauldron / open furnace, 5x5x5.
    A ritual alchemy furnace that has partially collapsed."""
    sb = StructureBuilder(5, 5, 5)

    # Layer 0 — buried base, mostly underground rubble
    for x in range(5):
        for z in range(5):
            if 1 <= x <= 3 and 1 <= z <= 3:
                sb.set_block(x, 0, z, "minecraft:polished_blackstone")
            else:
                if not _maybe_skip(0.3):
                    sb.set_block(x, 0, z, "minecraft:mossy_cobblestone")

    # Layer 1 — furnace body base ring
    for x in range(5):
        for z in range(5):
            if (x in (0, 4) and 1 <= z <= 3) or (z in (0, 4) and 1 <= x <= 3):
                block = _ruin_block("minecraft:polished_blackstone_bricks",
                                    "minecraft:mossy_stone_bricks",
                                    "minecraft:cracked_stone_bricks", 0.3)
                sb.set_block(x, 1, z, block)
            elif x in (0, 4) and z in (0, 4):
                # corners — partial collapse
                if not _maybe_skip(0.4):
                    sb.set_block(x, 1, z, "minecraft:mossy_cobblestone")

    # Layer 1 interior — the cauldron + campfire
    sb.set_block(2, 1, 2, "minecraft:cauldron")  # central cauldron
    sb.set_block(1, 1, 2, "minecraft:campfire", {"lit": "false"})  # extinguished fire
    sb.set_block(3, 1, 2, "minecraft:soul_soil")
    sb.set_block(2, 1, 1, "minecraft:soul_soil")
    sb.set_block(2, 1, 3, "minecraft:soul_soil")

    # Layer 2 — walls rising, more broken
    for x in range(5):
        for z in range(5):
            if (x in (0, 4) and 1 <= z <= 3) or (z in (0, 4) and 1 <= x <= 3):
                if not _maybe_skip(0.25):
                    block = _ruin_block("minecraft:polished_blackstone_bricks",
                                        "minecraft:mossy_stone_bricks",
                                        "minecraft:cracked_stone_bricks", 0.35)
                    sb.set_block(x, 2, z, block)

    # Layer 2 interior — ash and debris
    sb.set_block(1, 2, 1, "minecraft:cobweb")
    sb.set_block(3, 2, 3, "minecraft:cobweb")

    # Layer 3 — only partial walls remain (heavy ruin)
    sb.set_block(0, 3, 2, "minecraft:polished_blackstone_bricks")
    sb.set_block(4, 3, 2, "minecraft:cracked_stone_bricks")
    sb.set_block(2, 3, 0, "minecraft:mossy_stone_bricks")

    # Layer 4 — single remaining pillar fragment
    sb.set_block(0, 4, 2, "minecraft:mossy_stone_bricks")

    return sb


# ════════════════════════════════════════════════════════════════
# 8. 地下室·宗主石棺 — master_sarcophagus — 3×2×5
# ════════════════════════════════════════════════════════════════

def build_master_sarcophagus() -> StructureBuilder:
    """Sect master's sarcophagus in the basement, 3x2x5.
    Polished blackstone slab construction with the 师叔之印 placement point."""
    sb = StructureBuilder(3, 2, 5)

    # Layer 0 — sarcophagus base (solid blackstone slab platform)
    for x in range(3):
        for z in range(5):
            sb.set_block(x, 0, z, "minecraft:polished_blackstone_slab",
                        {"type": "bottom"})

    # Layer 1 — coffin walls + lid
    # Side walls
    for z in range(5):
        sb.set_block(0, 1, z, "minecraft:polished_blackstone_slab",
                    {"type": "top"})
        sb.set_block(2, 1, z, "minecraft:polished_blackstone_slab",
                    {"type": "top"})

    # Head and foot
    sb.set_block(1, 1, 0, "minecraft:chiseled_polished_blackstone")  # head end — decorated
    sb.set_block(1, 1, 4, "minecraft:polished_blackstone_slab",
                {"type": "top"})  # foot end

    # Center of coffin — "印" placement point (use a chiseled block as marker)
    sb.set_block(1, 1, 2, "minecraft:chiseled_polished_blackstone")  # 师叔之印 placement

    # Decorative elements on lid
    sb.set_block(1, 1, 1, "minecraft:polished_blackstone_slab",
                {"type": "top"})
    sb.set_block(1, 1, 3, "minecraft:polished_blackstone_slab",
                {"type": "top"})

    return sb


# ════════════════════════════════════════════════════════════════
# 9. 百草丹殿主体 — dan_zong_great_hall — 50×18×30
# ════════════════════════════════════════════════════════════════

def build_dan_zong_great_hall() -> StructureBuilder:
    """The Great Hall of Hundred Herbs (百草丹殿), 50x18x30.

    Layout:
    - Main hall: center 30x18x20 (grand vaulted space)
    - East wing: right side 10x10x10 (collapsed storage)
    - West wing: left side 10x10x10 (herb processing room)
    - Basement passage: center floor leads down (3x3 stairwell)

    Architectural style: ancient Chinese sect hall, thousand-year ruins
    - Stone brick walls with heavy moss/crack damage
    - Purple terracotta accents (Dan sect color)
    - Pillars of polished blackstone
    - "百草" chiseled blocks at entrance
    """
    sb = StructureBuilder(50, 18, 30)

    # ── Constants ──
    # Main hall bounds: x[10..39], z[5..24], full height
    MH_X1, MH_X2 = 10, 39  # 30 wide
    MH_Z1, MH_Z2 = 5, 24   # 20 deep
    MH_H = 14               # main hall interior height

    # East wing: x[40..49], z[5..14]
    EW_X1, EW_X2 = 40, 49
    EW_Z1, EW_Z2 = 5, 14

    # West wing: x[0..9], z[5..14]
    WW_X1, WW_X2 = 0, 9
    WW_Z1, WW_Z2 = 5, 14

    def wall_block() -> str:
        return _ruin_block("minecraft:stone_bricks", "minecraft:mossy_stone_bricks",
                          "minecraft:cracked_stone_bricks", 0.35)

    def floor_block() -> str:
        r = random.random()
        if r < 0.15:
            return "minecraft:mossy_cobblestone"
        if r < 0.25:
            return "minecraft:cracked_stone_bricks"
        return "minecraft:stone_bricks"

    # ════════ MAIN HALL ════════

    # Floor (y=0)
    for x in range(MH_X1, MH_X2 + 1):
        for z in range(MH_Z1, MH_Z2 + 1):
            sb.set_block(x, 0, z, floor_block())

    # Walls (y=1..MH_H)
    for y in range(1, MH_H + 1):
        # Higher walls have more damage
        extra_skip = 0.02 * max(0, y - 8)  # above y=8, start losing blocks
        for x in range(MH_X1, MH_X2 + 1):
            for z in [MH_Z1, MH_Z2]:
                if not _maybe_skip(0.05 + extra_skip):
                    sb.set_block(x, y, z, wall_block())
        for z in range(MH_Z1, MH_Z2 + 1):
            for x in [MH_X1, MH_X2]:
                if not _maybe_skip(0.05 + extra_skip):
                    sb.set_block(x, y, z, wall_block())

    # Entrance wall (z=MH_Z1) — leave a 4-wide doorway at center
    door_x_center = (MH_X1 + MH_X2) // 2
    for y in range(1, 6):
        for x in range(door_x_center - 2, door_x_center + 3):
            # Remove door blocks (they were placed by wall loop, just skip them)
            pass
    # Actually, let's selectively NOT place wall blocks at the door
    # We need to re-do the front wall with a gap. Let's post-process:
    # The wall loop already placed them, but StructureBuilder appends, so
    # we can't remove. Let's build walls more carefully.

    # We'll rebuild the structure more carefully. Clear and restart approach:
    sb = StructureBuilder(50, 18, 30)

    # ── FLOOR ──

    # Main hall floor
    for x in range(MH_X1, MH_X2 + 1):
        for z in range(MH_Z1, MH_Z2 + 1):
            sb.set_block(x, 0, z, floor_block())

    # East wing floor
    for x in range(EW_X1, EW_X2 + 1):
        for z in range(EW_Z1, EW_Z2 + 1):
            sb.set_block(x, 0, z, floor_block())

    # West wing floor
    for x in range(WW_X1, WW_X2 + 1):
        for z in range(WW_Z1, WW_Z2 + 1):
            sb.set_block(x, 0, z, floor_block())

    # ── MAIN HALL WALLS ──
    door_cx = (MH_X1 + MH_X2) // 2  # x=24

    for y in range(1, MH_H + 1):
        extra_skip = 0.03 * max(0, y - 10)

        # Front wall (z=MH_Z1) with doorway
        for x in range(MH_X1, MH_X2 + 1):
            # Doorway: 5-wide gap at center, 5 blocks tall
            if (door_cx - 2) <= x <= (door_cx + 2) and y <= 5:
                continue  # doorway gap
            if not _maybe_skip(0.05 + extra_skip):
                sb.set_block(x, y, MH_Z1, wall_block())

        # Back wall (z=MH_Z2)
        for x in range(MH_X1, MH_X2 + 1):
            if not _maybe_skip(0.05 + extra_skip):
                sb.set_block(x, y, MH_Z2, wall_block())

        # Left wall (x=MH_X1) — has opening to west wing
        for z in range(MH_Z1 + 1, MH_Z2):
            if z in range(WW_Z1 + 2, WW_Z2 - 2) and y <= 4:
                continue  # passage to west wing
            if not _maybe_skip(0.05 + extra_skip):
                sb.set_block(MH_X1, y, z, wall_block())

        # Right wall (x=MH_X2) — has opening to east wing
        for z in range(MH_Z1 + 1, MH_Z2):
            if z in range(EW_Z1 + 2, EW_Z2 - 2) and y <= 4:
                continue  # passage to east wing
            if not _maybe_skip(0.05 + extra_skip):
                sb.set_block(MH_X2, y, z, wall_block())

    # ── MAIN HALL PILLARS — 4 pairs along the hall ──
    pillar_xs = [MH_X1 + 4, MH_X1 + 10, MH_X2 - 10, MH_X2 - 4]
    pillar_zs = [MH_Z1 + 4, MH_Z2 - 4]
    for px in pillar_xs:
        for pz in pillar_zs:
            for y in range(1, MH_H - 1):
                if not _maybe_skip(0.02):
                    sb.set_block(px, y, pz, "minecraft:polished_blackstone")

    # ── ENTRANCE "百草" CHISELED SIGNAGE ──
    # Above doorway, chiseled stone bricks as "inscription"
    for dx in range(-1, 2):
        sb.set_block(door_cx + dx, 6, MH_Z1, "minecraft:chiseled_stone_bricks")
    # Purple terracotta accent line
    for dx in range(-2, 3):
        sb.set_block(door_cx + dx, 7, MH_Z1, "minecraft:purple_terracotta")

    # ── MAIN HALL ROOF (partial — ruined) ──
    for x in range(MH_X1, MH_X2 + 1):
        for z in range(MH_Z1, MH_Z2 + 1):
            # Partial roof — more holes toward center (collapsed)
            dist_from_wall = min(x - MH_X1, MH_X2 - x, z - MH_Z1, MH_Z2 - z)
            collapse_chance = 0.1 + 0.06 * dist_from_wall  # center more collapsed
            if not _maybe_skip(collapse_chance):
                sb.set_block(x, MH_H, z, wall_block())

    # ── INTERIOR DETAILS ──

    # Altar platform at back of hall
    altar_cx = (MH_X1 + MH_X2) // 2
    altar_z = MH_Z2 - 3
    for dx in range(-3, 4):
        for dz in range(-1, 2):
            sb.set_block(altar_cx + dx, 1, altar_z + dz,
                        "minecraft:polished_blackstone_slab", {"type": "bottom"})
    # Altar top — chiseled centerpiece
    sb.set_block(altar_cx, 2, altar_z, "minecraft:chiseled_polished_blackstone")
    sb.set_block(altar_cx - 1, 2, altar_z, "minecraft:soul_lantern")
    sb.set_block(altar_cx + 1, 2, altar_z, "minecraft:soul_lantern")

    # Cobwebs in corners
    for (cx, cz) in [(MH_X1 + 1, MH_Z1 + 1), (MH_X2 - 1, MH_Z1 + 1),
                     (MH_X1 + 1, MH_Z2 - 1), (MH_X2 - 1, MH_Z2 - 1)]:
        for cy in range(MH_H - 3, MH_H):
            if not _maybe_skip(0.3):
                sb.set_block(cx, cy, cz, "minecraft:cobweb")

    # Scattered debris on floor
    for _ in range(25):
        dx = random.randint(MH_X1 + 2, MH_X2 - 2)
        dz = random.randint(MH_Z1 + 2, MH_Z2 - 2)
        if random.random() < 0.5:
            sb.set_block(dx, 1, dz, "minecraft:cobweb")
        else:
            sb.set_block(dx, 1, dz, "minecraft:dead_bush")

    # ── EAST WING (collapsed storage) ──
    for y in range(1, 8):
        extra_skip = 0.04 * max(0, y - 5)
        # Walls
        for x in range(EW_X1, EW_X2 + 1):
            for z in [EW_Z1, EW_Z2]:
                if not _maybe_skip(0.08 + extra_skip):
                    sb.set_block(x, y, z, wall_block())
        for z in range(EW_Z1, EW_Z2 + 1):
            if not _maybe_skip(0.08 + extra_skip):
                sb.set_block(EW_X2, y, z, wall_block())
        # Don't build inner wall (shared with main hall, already has opening)

    # East wing interior — storage shelves (oak planks + slabs)
    for sz in range(EW_Z1 + 1, EW_Z2, 2):
        for sx in range(EW_X1 + 2, EW_X2 - 1, 3):
            if not _maybe_skip(0.3):
                sb.set_block(sx, 1, sz, "minecraft:dark_oak_planks")
                if not _maybe_skip(0.4):
                    sb.set_block(sx, 2, sz, "minecraft:dark_oak_slab",
                                {"type": "bottom"})

    # Rubble pile in east wing (collapsed roof)
    for _ in range(15):
        rx = random.randint(EW_X1 + 1, EW_X2 - 1)
        rz = random.randint(EW_Z1 + 1, EW_Z2 - 1)
        ry = random.randint(1, 3)
        sb.set_block(rx, ry, rz, random.choice([
            "minecraft:cobblestone", "minecraft:gravel",
            "minecraft:mossy_cobblestone"
        ]))

    # ── WEST WING (herb processing) ──
    for y in range(1, 8):
        extra_skip = 0.04 * max(0, y - 5)
        for x in range(WW_X1, WW_X2 + 1):
            for z in [WW_Z1, WW_Z2]:
                if not _maybe_skip(0.08 + extra_skip):
                    sb.set_block(x, y, z, wall_block())
        for z in range(WW_Z1, WW_Z2 + 1):
            if not _maybe_skip(0.08 + extra_skip):
                sb.set_block(WW_X1, y, z, wall_block())

    # West wing interior — herb processing tables
    for sz in [WW_Z1 + 2, WW_Z1 + 5, WW_Z2 - 2]:
        for sx in range(WW_X1 + 1, WW_X1 + 4):
            if not _maybe_skip(0.2):
                sb.set_block(sx, 1, sz, "minecraft:dark_oak_planks")

    # Cauldrons in west wing (herb boiling)
    sb.set_block(WW_X1 + 2, 1, WW_Z1 + 4, "minecraft:cauldron")
    sb.set_block(WW_X1 + 5, 1, WW_Z1 + 3, "minecraft:cauldron")

    # ── BASEMENT STAIRWELL ──
    # 3x3 hole in main hall floor near the altar, leading down
    stair_cx = altar_cx
    stair_cz = altar_z - 4
    # Dig down — remove floor blocks and create stairs
    # Layer -1 (y=0 level, but we mark the opening)
    for dx in range(-1, 2):
        for dz in range(-1, 2):
            # The stairwell is represented by deepslate at floor level
            sb.set_block(stair_cx + dx, 0, stair_cz + dz,
                        "minecraft:deepslate_bricks")

    # Stair blocks going down (symbolic — actual underground is separate)
    sb.set_block(stair_cx, 0, stair_cz - 1, "minecraft:polished_blackstone_stairs",
                {"facing": "south", "half": "bottom"})
    sb.set_block(stair_cx, 0, stair_cz + 1, "minecraft:polished_blackstone_stairs",
                {"facing": "north", "half": "bottom"})

    # Iron bars around stairwell opening
    for dx in range(-2, 3):
        for dz in range(-2, 3):
            if abs(dx) == 2 or abs(dz) == 2:
                if not (abs(dx) <= 1 and abs(dz) <= 1):
                    if not _maybe_skip(0.15):
                        sb.set_block(stair_cx + dx, 1, stair_cz + dz,
                                    "minecraft:iron_bars")

    # ── LANTERNS ──
    # Hang soul lanterns from remaining ceiling
    lantern_positions = [
        (MH_X1 + 5, MH_H - 1, MH_Z1 + 5),
        (MH_X2 - 5, MH_H - 1, MH_Z1 + 5),
        (MH_X1 + 5, MH_H - 1, MH_Z2 - 5),
        (MH_X2 - 5, MH_H - 1, MH_Z2 - 5),
        (door_cx, MH_H - 1, (MH_Z1 + MH_Z2) // 2),
    ]
    for (lx, ly, lz) in lantern_positions:
        if not _maybe_skip(0.2):
            sb.set_block(lx, ly, lz, "minecraft:chain")
            sb.set_block(lx, ly - 1, lz, "minecraft:soul_lantern")

    return sb


# ════════════════════════════════════════════════════════════════
# Main generation + stats
# ════════════════════════════════════════════════════════════════

STRUCTURES = [
    ("fallen_recipe_stele", build_fallen_recipe_stele, (1, 3, 2)),
    ("fallen_alchemist_bone", build_fallen_alchemist_bone, (2, 1, 3)),
    ("vapor_poison_spring_small", build_vapor_poison_spring_small, (3, 2, 3)),
    ("herb_garden_pen_6x6", build_herb_garden_pen_6x6, (6, 2, 6)),
    ("herb_garden_pen_8x8", build_herb_garden_pen_8x8, (8, 2, 8)),
    ("vapor_poison_spring_main", build_vapor_poison_spring_main, (5, 3, 5)),
    ("ruined_open_furnace", build_ruined_open_furnace, (5, 5, 5)),
    ("master_sarcophagus", build_master_sarcophagus, (3, 2, 5)),
    ("dan_zong_great_hall", build_dan_zong_great_hall, (50, 18, 30)),
]


def generate_all(out_dir: str, print_stats: bool = True) -> dict[str, dict]:
    """Generate all structures and return stats."""
    os.makedirs(out_dir, exist_ok=True)
    all_stats = {}

    for name, builder_fn, expected_size in STRUCTURES:
        # Reset seed per structure for reproducibility
        random.seed(42 + hash(name) % 10000)

        sb = builder_fn()
        path = os.path.join(out_dir, f"{name}.nbt")
        sb.save(path)

        stats = sb.get_stats()
        file_size = os.path.getsize(path)
        stats["file_size_bytes"] = file_size
        stats["expected_size"] = expected_size

        # Verify size matches spec
        actual = stats["size"]
        if actual != expected_size:
            print(f"  WARNING: {name} size mismatch! expected={expected_size}, actual={actual}")

        all_stats[name] = stats

        if print_stats:
            print(f"\n{'='*60}")
            print(f"  {name}")
            print(f"  Size: {actual[0]}x{actual[1]}x{actual[2]} (expected {expected_size[0]}x{expected_size[1]}x{expected_size[2]})")
            print(f"  Blocks: {stats['total_blocks']}, Palette: {stats['palette_size']}")
            print(f"  File: {file_size} bytes")
            print(f"  Block types:")
            for btype, count in stats["block_counts"].items():
                print(f"    {btype}: {count}")

    return all_stats


def review_all(out_dir: str) -> None:
    """Print ASCII top views for all structures (review aid)."""
    for name, builder_fn, _ in STRUCTURES:
        random.seed(42 + hash(name) % 10000)
        sb = builder_fn()
        print(f"\n{'='*60}")
        print(f"  {name} — Top View")
        print(f"{'='*60}")
        print(sb.ascii_top_view())
        if sb.size[1] > 1:
            print(f"\n  Floor level (y=0):")
            print(sb.ascii_top_view(y_level=0))
            print(f"\n  Level 1 (y=1):")
            print(sb.ascii_top_view(y_level=1))


if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser(description="Generate 丹宗遗园 NBT structures")
    parser.add_argument("--review", action="store_true", help="Print ASCII review views")
    parser.add_argument("--out", default=OUT_DIR, help="Output directory")
    args = parser.parse_args()

    if args.review:
        review_all(args.out)
    else:
        print("Generating 丹宗遗园 structures...")
        stats = generate_all(args.out)
        print(f"\n{'='*60}")
        print(f"Generated {len(stats)} structures in {args.out}")
        print(f"{'='*60}")
