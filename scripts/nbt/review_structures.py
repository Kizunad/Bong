"""
review_structures.py — Round 2 review tool for 丹宗 NBT structures.

Checks:
1. Size matches spec
2. Block palette uses appropriate ruin-themed materials
3. Symmetry check for symmetric structures
4. Block density (not too sparse, not too full)
5. ASCII top/side views for visual inspection
"""

from __future__ import annotations

import os
import sys
import random

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from nbt.nbt_builder import StructureBuilder
from nbt.gen_dan_zong_structures import STRUCTURES, build_fallen_recipe_stele

# Expected ruin-themed blocks
RUIN_BLOCKS = {
    "minecraft:stone_bricks", "minecraft:mossy_stone_bricks",
    "minecraft:cracked_stone_bricks", "minecraft:chiseled_stone_bricks",
    "minecraft:mossy_cobblestone", "minecraft:cobblestone",
    "minecraft:polished_blackstone", "minecraft:chiseled_polished_blackstone",
    "minecraft:polished_blackstone_bricks", "minecraft:polished_blackstone_slab",
    "minecraft:polished_blackstone_stairs",
    "minecraft:purple_terracotta", "minecraft:podzol",
    "minecraft:soul_sand", "minecraft:soul_soil",
    "minecraft:gravel", "minecraft:deepslate_bricks", "minecraft:deepslate_tiles",
}

ACCENT_BLOCKS = {
    "minecraft:purple_stained_glass", "minecraft:magenta_stained_glass",
    "minecraft:cauldron", "minecraft:campfire", "minecraft:soul_campfire",
    "minecraft:bone_block", "minecraft:skeleton_skull",
    "minecraft:water", "minecraft:bubble_column",
    "minecraft:iron_bars", "minecraft:chain",
    "minecraft:lantern", "minecraft:soul_lantern",
    "minecraft:flower_pot", "minecraft:dead_bush",
    "minecraft:cobweb", "minecraft:dark_oak_planks", "minecraft:dark_oak_slab",
    "minecraft:oak_planks", "minecraft:spruce_planks",
}

ALL_ALLOWED = RUIN_BLOCKS | ACCENT_BLOCKS


def check_symmetry(sb: StructureBuilder, axis: str = "x") -> float:
    """Check mirror symmetry along given axis. Returns ratio 0..1."""
    sx, sy, sz = sb.size
    block_map: dict[tuple[int, int, int], int] = {}
    for blk in sb._blocks:
        pos = tuple(blk["pos"])
        block_map[pos] = blk["state"]

    total = 0
    matched = 0
    for pos, state in block_map.items():
        x, y, z = pos
        if axis == "x":
            mirror = (sx - 1 - x, y, z)
        elif axis == "z":
            mirror = (x, y, sz - 1 - z)
        else:
            continue
        total += 1
        if mirror in block_map:
            matched += 1

    return matched / total if total > 0 else 0.0


def review_structure(name: str, builder_fn, expected_size: tuple[int, int, int]) -> list[str]:
    """Review a single structure. Returns list of issues found."""
    random.seed(42 + hash(name) % 10000)
    sb = builder_fn()
    issues = []

    # 1. Size check
    if sb.size != expected_size:
        issues.append(f"SIZE MISMATCH: got {sb.size}, expected {expected_size}")

    stats = sb.get_stats()

    # 2. Block count sanity
    volume = expected_size[0] * expected_size[1] * expected_size[2]
    density = stats["total_blocks"] / volume
    if density < 0.05:
        issues.append(f"TOO SPARSE: {density:.1%} fill ({stats['total_blocks']}/{volume})")
    # Small constructed objects (sarcophagus, springs) can be dense; only flag large structures
    if density > 0.95 and volume > 30:
        issues.append(f"TOO DENSE: {density:.1%} fill — ruins should have gaps")

    # 3. Block palette check
    for block_name in stats["block_counts"]:
        if block_name not in ALL_ALLOWED:
            issues.append(f"UNEXPECTED BLOCK: {block_name}")

    # 4. Ruin material ratio
    ruin_count = sum(stats["block_counts"].get(b, 0) for b in RUIN_BLOCKS)
    total = stats["total_blocks"]
    if total > 10:
        ruin_ratio = ruin_count / total
        if ruin_ratio < 0.3:
            issues.append(f"LOW RUIN RATIO: {ruin_ratio:.1%} — needs more stone/moss blocks")

    # 5. Symmetry for structures that should be symmetric
    symmetric_structures = {
        "vapor_poison_spring_small", "vapor_poison_spring_main",
        "master_sarcophagus",
    }
    if name in symmetric_structures:
        sym_x = check_symmetry(sb, "x")
        sym_z = check_symmetry(sb, "z")
        if sym_x < 0.7:
            issues.append(f"LOW X-SYMMETRY: {sym_x:.1%}")
        if sym_z < 0.7:
            issues.append(f"LOW Z-SYMMETRY: {sym_z:.1%}")

    # 6. Specific structure checks
    if name == "fallen_alchemist_bone":
        has_skull = any("skeleton_skull" in sb._palette[b["state"]]["Name"]
                       for b in sb._blocks)
        has_bone = any("bone_block" in sb._palette[b["state"]]["Name"]
                      for b in sb._blocks)
        if not has_skull:
            issues.append("MISSING skull block")
        if not has_bone:
            issues.append("MISSING bone blocks")

    if name == "master_sarcophagus":
        has_chiseled = any("chiseled_polished_blackstone" in sb._palette[b["state"]]["Name"]
                         for b in sb._blocks)
        if not has_chiseled:
            issues.append("MISSING chiseled_polished_blackstone (印 placement)")

    if name == "dan_zong_great_hall":
        has_purple_terra = any("purple_terracotta" in sb._palette[b["state"]]["Name"]
                             for b in sb._blocks)
        has_chiseled_sb = any("chiseled_stone_bricks" in sb._palette[b["state"]]["Name"]
                            for b in sb._blocks)
        has_pillars = sum(1 for b in sb._blocks
                        if sb._palette[b["state"]]["Name"] == "minecraft:polished_blackstone"
                        and b["pos"][1] > 3) > 10
        if not has_purple_terra:
            issues.append("MISSING purple_terracotta accent")
        if not has_chiseled_sb:
            issues.append("MISSING chiseled_stone_bricks (百草 signage)")
        if not has_pillars:
            issues.append("INSUFFICIENT pillars at height")

    if name == "ruined_open_furnace":
        has_cauldron = any("cauldron" in sb._palette[b["state"]]["Name"]
                         for b in sb._blocks)
        has_campfire = any("campfire" in sb._palette[b["state"]]["Name"]
                          for b in sb._blocks)
        if not has_cauldron:
            issues.append("MISSING cauldron (the 鼎)")
        if not has_campfire:
            issues.append("MISSING campfire (extinguished fire)")

    if name in ("herb_garden_pen_6x6", "herb_garden_pen_8x8"):
        has_podzol = any("podzol" in sb._palette[b["state"]]["Name"]
                        for b in sb._blocks)
        if not has_podzol:
            issues.append("MISSING podzol interior (herb soil)")

    # Print report
    print(f"\n{'='*60}")
    print(f"  REVIEW: {name}")
    print(f"  Size: {sb.size} (spec: {expected_size}) {'OK' if sb.size == expected_size else 'MISMATCH!'}")
    print(f"  Blocks: {stats['total_blocks']} / {volume} vol = {density:.1%} fill")
    print(f"  Palette: {stats['palette_size']} block types")
    if issues:
        print(f"  ISSUES ({len(issues)}):")
        for issue in issues:
            print(f"    - {issue}")
    else:
        print(f"  No issues found.")

    # ASCII views
    print(f"\n  Top view (highest):")
    for line in sb.ascii_top_view().split("\n"):
        print(f"  {line}")
    print(f"\n  Floor (y=0):")
    for line in sb.ascii_top_view(y_level=0).split("\n"):
        print(f"  {line}")

    return issues


def main():
    all_issues: dict[str, list[str]] = {}
    for name, builder_fn, expected_size in STRUCTURES:
        issues = review_structure(name, builder_fn, expected_size)
        if issues:
            all_issues[name] = issues

    print(f"\n{'='*60}")
    print(f"  REVIEW SUMMARY")
    print(f"{'='*60}")
    if all_issues:
        print(f"  Structures with issues: {len(all_issues)}")
        for name, issues in all_issues.items():
            print(f"  - {name}: {len(issues)} issue(s)")
    else:
        print(f"  All structures passed review!")
    return all_issues


if __name__ == "__main__":
    all_issues = main()
    sys.exit(1 if all_issues else 0)
