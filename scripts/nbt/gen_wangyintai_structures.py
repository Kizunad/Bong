"""
gen_wangyintai_structures.py — Generate all 4 王印台 NBT structure files.

Run from repo root:
    python3 scripts/nbt/gen_wangyintai_structures.py

Output: server/structures/wangyintai/*.nbt

Architectural style: 末法残土 · 忘音台 · 观天台废墟
  - Weathered / crumbling: mossy stone bricks, cracked stone bricks, deepslate
  - Calcite and smooth basalt accents (astral observatory aesthetic)
  - Heavy ruin damage: missing blocks, cobwebs, gravel debris
  - Roofless or partial roof (thousand-year sky exposure)

Structures:
  1. jingxuguan_side_hall  — 静虚馆配殿 (12×8×12)  side hall ruins at cardinal
  2. corridor_fragment     — 碎廊残段   (4×4×10)   connecting corridor fragment
  3. fallen_vortex_disc    — 坠落旋涡盘  (5×2×5)    fallen stone vortex disc
  4. guantiantai_ruins     — 观天台主体  (20×14×20) central observatory ruins
"""

from __future__ import annotations

import os
import random
import sys

# Ensure scripts/ is on path for nbt_builder import
sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from nbt.nbt_builder import StructureBuilder

OUT_DIR = os.path.join(
    os.path.dirname(__file__), "..", "..", "server", "structures", "wangyintai"
)

# Deterministic seed for reproducible ruins
random.seed(0xABCD)


# ---------------------------------------------------------------------------
# Helper utilities
# ---------------------------------------------------------------------------


def _ruin_block(
    primary: str,
    mossy: str,
    cracked: str | None = None,
    ratio: float = 0.3,
) -> str:
    """Pick a ruin variant with weathering probability."""
    r = random.random()
    if cracked and r < ratio * 0.4:
        return cracked
    if r < ratio:
        return mossy
    return primary


def _skip(ratio: float = 0.1) -> bool:
    """Return True if this block should be omitted (ruin gap)."""
    return random.random() < ratio


def _wall(extra: float = 0.0) -> str:
    return _ruin_block(
        "minecraft:stone_bricks",
        "minecraft:mossy_stone_bricks",
        "minecraft:cracked_stone_bricks",
        0.35 + extra,
    )


def _deep() -> str:
    return _ruin_block(
        "minecraft:deepslate_bricks",
        "minecraft:mossy_cobblestone",
        "minecraft:cracked_deepslate_bricks",
        0.25,
    )


# ════════════════════════════════════════════════════════════════
# 1. 静虚馆配殿 — jingxuguan_side_hall — 12×8×12
# ════════════════════════════════════════════════════════════════


def build_jingxuguan_side_hall() -> StructureBuilder:
    """Ruined side hall of the Quiet-Void Hall (静虚馆), 12x8x12.

    A small ceremonial hall with perimeter walls, an arched entrance on the
    south face (z=11), and a collapsed partial roof.  Deepslate pillar stubs
    mark the inner corners.  Calcite floor patches survive near the altar slab.
    """
    sb = StructureBuilder(12, 8, 12)

    # ── Floor (y=0) ──
    for x in range(12):
        for z in range(12):
            if x == 0 or x == 11 or z == 0 or z == 11:
                sb.set_block(x, 0, z, _deep())
            else:
                b = random.random()
                if b < 0.12:
                    sb.set_block(x, 0, z, "minecraft:gravel")
                elif b < 0.22:
                    sb.set_block(x, 0, z, "minecraft:calcite")
                else:
                    sb.set_block(x, 0, z, "minecraft:smooth_basalt")

    # ── Perimeter walls (y=1..5) ──
    entrance_z = 11      # south face, door opening x=[4..7] y=1..3
    for y in range(1, 6):
        extra_skip = 0.04 * max(0, y - 3)
        for x in range(12):
            # South wall (z=11) — entrance opening
            if z == entrance_z:
                pass  # handled below
            # North wall (z=0)
            if not _skip(0.06 + extra_skip):
                sb.set_block(x, y, 0, _wall(extra_skip))
            # South wall with entrance gap
            if not (4 <= x <= 7 and y <= 3):
                if not _skip(0.06 + extra_skip):
                    sb.set_block(x, y, 11, _wall(extra_skip))
        # East/West walls
        for z in range(1, 11):
            if not _skip(0.06 + extra_skip):
                sb.set_block(0, y, z, _wall(extra_skip))
            if not _skip(0.06 + extra_skip):
                sb.set_block(11, y, z, _wall(extra_skip))

    # ── Inner corner pillar stubs (deepslate) ──
    for (px, pz) in [(2, 2), (9, 2), (2, 9), (9, 9)]:
        for y in range(1, 7):
            if not _skip(0.03):
                sb.set_block(px, y, pz, "minecraft:deepslate_bricks")

    # ── Altar slab at back of hall ──
    altar_x, altar_z = 5, 2
    for dx in range(-2, 3):
        if not _skip(0.1):
            sb.set_block(altar_x + dx, 1, altar_z,
                         "minecraft:polished_deepslate_slab", {"type": "bottom"})
    sb.set_block(altar_x, 2, altar_z, "minecraft:chiseled_deepslate")

    # ── Partial collapsed roof (y=6) ──
    for x in range(12):
        for z in range(12):
            dist = min(x, 11 - x, z, 11 - z)
            collapse = 0.08 + 0.10 * max(0, 3 - dist)
            if not _skip(collapse):
                sb.set_block(x, 6, z, _wall(0.2))

    # ── Cobweb decay ──
    for _ in range(8):
        cx = random.randint(1, 10)
        cz = random.randint(1, 10)
        sb.set_block(cx, 5, cz, "minecraft:cobweb")

    return sb


# ════════════════════════════════════════════════════════════════
# 2. 碎廊残段 — corridor_fragment — 4×4×10
# ════════════════════════════════════════════════════════════════


def build_corridor_fragment() -> StructureBuilder:
    """A 4-wide corridor fragment connecting center to side halls, 4x4x10.

    Floor: smooth basalt with gravel gaps.
    Side walls: 3 blocks tall, mossy/cracked stone bricks, many gaps.
    No roof (collapsed).  Calcite threshold stones at each end (z=0, z=9).
    """
    sb = StructureBuilder(4, 4, 10)

    # Floor (y=0)
    for z in range(10):
        for x in range(4):
            if _skip(0.12):
                sb.set_block(x, 0, z, "minecraft:gravel")
            else:
                sb.set_block(x, 0, z, "minecraft:smooth_basalt")

    # Threshold accents at corridor ends
    for x in range(4):
        sb.set_block(x, 0, 0, "minecraft:calcite")
        sb.set_block(x, 0, 9, "minecraft:calcite")

    # Side walls (x=0 and x=3), y=1..2, heavy decay
    for z in range(10):
        for y in range(1, 3):
            extra = 0.03 * y
            if not _skip(0.20 + extra):
                sb.set_block(0, y, z, _wall(extra))
            if not _skip(0.20 + extra):
                sb.set_block(3, y, z, _wall(extra))

    # Occasional low arch remnant (deepslate brick over center)
    for z in [2, 5, 8]:
        if not _skip(0.35):
            sb.set_block(0, 3, z, "minecraft:deepslate_brick_slab",
                         {"type": "bottom"})
            sb.set_block(3, 3, z, "minecraft:deepslate_brick_slab",
                         {"type": "bottom"})

    # Rubble scatter
    for _ in range(6):
        rz = random.randint(1, 8)
        rx = random.randint(1, 2)
        sb.set_block(rx, 1, rz,
                     random.choice(["minecraft:cobblestone", "minecraft:gravel",
                                    "minecraft:mossy_cobblestone"]))

    return sb


# ════════════════════════════════════════════════════════════════
# 3. 坠落旋涡盘 — fallen_vortex_disc — 5×2×5
# ════════════════════════════════════════════════════════════════


def build_fallen_vortex_disc() -> StructureBuilder:
    """A fallen stone vortex disc (law array fragment), 5x2x5.

    Layer 0: circular stone disc (3x3 core + corner nubs) made of
             polished deepslate and smooth basalt, some cracked.
    Layer 1: disc stands on its edge (leaning), represented as a
             single upright ring of deepslate brick slabs — the top
             surface, partly crumbled.  A chiseled center stone at (2,1,2).

    Lore: fragments of the 王印台 vortex formation that collapsed in the
    末法 age.  Used as loot/lore markers in the layout.
    """
    sb = StructureBuilder(5, 2, 5)

    # Layer 0 — disc base lying flat
    disc_map = [
        [0, 1, 1, 1, 0],
        [1, 1, 1, 1, 1],
        [1, 1, 1, 1, 1],
        [1, 1, 1, 1, 1],
        [0, 1, 1, 1, 0],
    ]
    for x in range(5):
        for z in range(5):
            if disc_map[z][x]:
                if _skip(0.08):
                    sb.set_block(x, 0, z, "minecraft:gravel")
                elif random.random() < 0.3:
                    sb.set_block(x, 0, z, "minecraft:smooth_basalt")
                elif random.random() < 0.15:
                    sb.set_block(x, 0, z, "minecraft:cracked_deepslate_bricks")
                else:
                    sb.set_block(x, 0, z, "minecraft:polished_deepslate")

    # Layer 1 — raised ring (disc edge / vortex groove)
    ring_map = [
        [0, 1, 1, 1, 0],
        [1, 0, 0, 0, 1],
        [1, 0, 0, 0, 1],
        [1, 0, 0, 0, 1],
        [0, 1, 1, 1, 0],
    ]
    for x in range(5):
        for z in range(5):
            if ring_map[z][x] and not _skip(0.18):
                sb.set_block(x, 1, z, "minecraft:deepslate_brick_slab",
                             {"type": "bottom"})

    # Center carved stone — vortex inscription
    sb.set_block(2, 1, 2, "minecraft:chiseled_deepslate")

    return sb


# ════════════════════════════════════════════════════════════════
# 4. 观天台主体 — guantiantai_ruins — 20×14×20
# ════════════════════════════════════════════════════════════════


def build_guantiantai_ruins() -> StructureBuilder:
    """Central observatory ruins of 观天台 (Heaven-Watching Platform), 20x14x20.

    Layout:
    - Outer platform ring (20x20, 2 blocks tall) — polished deepslate / smooth basalt
    - Inner sanctuary walls (10x10 inside, y=2..8) — mossy/cracked stone brick
    - 4 corner tower stubs (2x2 footprint each corner, up to y=12)
    - Central altar raised 1 block (3x3 polished deepslate at y=3)
    - South entrance gap (z=19 face, x=8..11) in inner wall y=2..4
    - Collapsed roof (partial coverage at y=8..10)
    - Calcite astral inlay on outer platform
    - Cobweb / gravel ruin decay throughout
    """
    sb = StructureBuilder(20, 14, 20)

    # ── Outer platform (y=0..1) ──
    for x in range(20):
        for z in range(20):
            if not _skip(0.04):
                block = (
                    "minecraft:polished_deepslate"
                    if random.random() < 0.6
                    else "minecraft:smooth_basalt"
                )
                sb.set_block(x, 0, z, block)
                # Platform raised edge ring
                if x == 0 or x == 19 or z == 0 or z == 19:
                    if not _skip(0.10):
                        sb.set_block(x, 1, z, _deep())

    # Calcite astral inlay — radial cross pattern on outer platform
    for i in range(20):
        if not _skip(0.15):
            sb.set_block(i, 0, 9, "minecraft:calcite")
            sb.set_block(i, 0, 10, "minecraft:calcite")
        if not _skip(0.15):
            sb.set_block(9, 0, i, "minecraft:calcite")
            sb.set_block(10, 0, i, "minecraft:calcite")

    # ── Inner sanctuary walls (x=5..14, z=5..14) y=2..8 ──
    # South entrance gap: z=14 face, x=8..11, y=2..4 left open
    for y in range(2, 9):
        extra_skip = 0.04 * max(0, y - 5)
        # North wall (z=5)
        for x in range(5, 15):
            if not _skip(0.07 + extra_skip):
                sb.set_block(x, y, 5, _wall(extra_skip * 0.5))
        # South wall (z=14) — entrance gap x=[8..11], y=[2..4]
        for x in range(5, 15):
            if 8 <= x <= 11 and y <= 4:
                continue  # entrance opening — skip
            if not _skip(0.07 + extra_skip):
                sb.set_block(x, y, 14, _wall(extra_skip * 0.5))
        # West wall (x=5) and East wall (x=14)
        for z in range(5, 15):
            if not _skip(0.07 + extra_skip):
                sb.set_block(5, y, z, _wall(extra_skip * 0.5))
            if not _skip(0.07 + extra_skip):
                sb.set_block(14, y, z, _wall(extra_skip * 0.5))

    # ── 4 corner tower stubs (2×2 each corner) ──
    corners = [(0, 0), (0, 18), (18, 0), (18, 18)]
    for (cx, cz) in corners:
        for y in range(1, 12):
            h_skip = 0.03 * max(0, y - 8)
            for dx in range(2):
                for dz in range(2):
                    if not _skip(0.04 + h_skip):
                        sb.set_block(cx + dx, y, cz + dz, "minecraft:deepslate_bricks")

    # ── Central altar (y=3, 3×3 polished deepslate) ──
    for ax in range(8, 12):
        for az in range(8, 12):
            sb.set_block(ax, 3, az,
                         "minecraft:polished_deepslate" if random.random() < 0.8
                         else "minecraft:calcite")
    # Altar centerpiece
    sb.set_block(9, 4, 9, "minecraft:chiseled_deepslate")
    sb.set_block(10, 4, 10, "minecraft:chiseled_deepslate")
    # Soul lanterns at altar corners (lore: still lit by residual qi)
    for (lx, lz) in [(8, 8), (11, 8), (8, 11), (11, 11)]:
        if not _skip(0.2):
            sb.set_block(lx, 4, lz, "minecraft:soul_lantern")

    # ── Collapsed partial roof (y=8..10) ──
    for x in range(5, 15):
        for z in range(5, 15):
            dist = min(x - 5, 14 - x, z - 5, 14 - z)
            collapse = 0.10 + 0.08 * max(0, 2 - dist)
            if not _skip(collapse):
                y = 8 if dist > 1 else (9 if random.random() < 0.4 else 8)
                sb.set_block(x, y, z, _wall(0.25))
    # Partial roof at y=9..10 near walls (heaviest surviving section)
    for x in range(5, 8):
        for z in range(5, 8):
            if not _skip(0.35):
                sb.set_block(x, 9, z, _wall(0.1))

    # ── Cobweb decay in inner sanctuary corners ──
    for (cx, cz) in [(6, 6), (13, 6), (6, 13), (13, 13)]:
        for cy in range(6, 9):
            if not _skip(0.4):
                sb.set_block(cx, cy, cz, "minecraft:cobweb")

    # ── Gravel / rubble scatter on outer platform ──
    for _ in range(18):
        gx = random.randint(1, 18)
        gz = random.randint(1, 18)
        sb.set_block(gx, 1, gz, "minecraft:gravel")

    # ── Dead bushes / bones ──
    for _ in range(5):
        dx = random.randint(6, 13)
        dz = random.randint(6, 13)
        sb.set_block(dx, 2, dz, "minecraft:dead_bush")

    return sb


# ════════════════════════════════════════════════════════════════
# Main generation + stats
# ════════════════════════════════════════════════════════════════


STRUCTURES = [
    ("jingxuguan_side_hall", build_jingxuguan_side_hall, (12, 8, 12)),
    ("corridor_fragment",    build_corridor_fragment,    (4, 4, 10)),
    ("fallen_vortex_disc",   build_fallen_vortex_disc,   (5, 2, 5)),
    ("guantiantai_ruins",    build_guantiantai_ruins,    (20, 14, 20)),
]


def generate_all(out_dir: str, print_stats: bool = True) -> dict[str, dict]:
    """Generate all structures and return stats."""
    os.makedirs(out_dir, exist_ok=True)
    all_stats: dict[str, dict] = {}

    for name, builder_fn, expected_size in STRUCTURES:
        # Reset seed per structure for reproducibility
        random.seed(0xABCD + hash(name) % 10000)

        sb = builder_fn()
        path = os.path.join(out_dir, f"{name}.nbt")
        sb.save(path)

        stats = sb.get_stats()
        file_size = os.path.getsize(path)
        stats["file_size_bytes"] = file_size
        stats["expected_size"] = expected_size

        actual = stats["size"]
        if actual != expected_size:
            print(
                f"  WARNING: {name} size mismatch! "
                f"expected={expected_size}, actual={actual}"
            )

        all_stats[name] = stats

        if print_stats:
            print(f"\n{'=' * 60}")
            print(f"  {name}")
            print(
                f"  Size: {actual[0]}x{actual[1]}x{actual[2]} "
                f"(expected {expected_size[0]}x{expected_size[1]}x{expected_size[2]})"
            )
            print(
                f"  Blocks: {stats['total_blocks']}, "
                f"Palette: {stats['palette_size']}"
            )
            print(f"  File: {file_size} bytes")
            print(f"  Block types:")
            for btype, count in stats["block_counts"].items():
                print(f"    {btype}: {count}")

    return all_stats


def review_all(out_dir: str) -> None:
    """Print ASCII top views for all structures (review aid)."""
    for name, builder_fn, _ in STRUCTURES:
        random.seed(0xABCD + hash(name) % 10000)
        sb = builder_fn()
        print(f"\n{'=' * 60}")
        print(f"  {name} — Top View (y=highest non-air)")
        print(f"{'=' * 60}")
        print(sb.ascii_top_view())
        if sb.size[1] > 1:
            print(f"\n  Floor level (y=0):")
            print(sb.ascii_top_view(y_level=0))
            if sb.size[1] > 2:
                print(f"\n  Level 1 (y=1):")
                print(sb.ascii_top_view(y_level=1))
            if sb.size[1] > 3:
                print(f"\n  Level 2 (y=2):")
                print(sb.ascii_top_view(y_level=2))


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Generate 王印台 NBT structures")
    parser.add_argument("--review", action="store_true",
                        help="Print ASCII review views")
    parser.add_argument("--out", default=OUT_DIR, help="Output directory")
    args = parser.parse_args()

    if args.review:
        review_all(args.out)
    else:
        print("Generating 王印台 structures... (round 3/3)")
        stats = generate_all(args.out)
        print(f"\n{'=' * 60}")
        print(f"Generated {len(stats)} structures in {args.out}")
        print(f"{'=' * 60}")
