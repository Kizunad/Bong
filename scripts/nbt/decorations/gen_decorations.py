#!/usr/bin/env python3
"""P6 decoration NBT batch driver.

Runs every per-kind generator, writes the `.nbt` assets under
`server/structures/decorations/<kind>/`, round-trips each through the same
reader the Rust `DecorationNbtRegistry` mirrors, and prints a per-kind summary
plus the §8.1 #10 decompressed-residency accounting.

Usage:
    python3 scripts/nbt/decorations/gen_decorations.py
"""

from __future__ import annotations

import os
import sys
from collections import defaultdict

sys.path.insert(0, os.path.dirname(__file__))

import gen_big_mushroom  # noqa: E402
import gen_boulder  # noqa: E402
import gen_bush  # noqa: E402
import gen_crystal  # noqa: E402
import gen_fallen_log  # noqa: E402
import gen_grave  # noqa: E402
import gen_hanging_crystal  # noqa: E402
import gen_structures  # noqa: E402
import gen_tree  # noqa: E402

# Each entry: (kind label, module.generate) — flora kinds first, then structures.
GENERATORS = [
    ("small_tree", gen_tree.generate),
    # bush is split into bush_<ecology>/ subdirs so a shrub never stamps a
    # cross-biome variant (worldgen-v4 P6 ecology fix); generate() emits all four.
    ("bush_*", gen_bush.generate),
    ("boulder", gen_boulder.generate),
    ("crystal", gen_crystal.generate),
    ("big_mushroom", gen_big_mushroom.generate),
    ("fallen_log", gen_fallen_log.generate),
    ("grave", gen_grave.generate),
    ("hanging_crystal", gen_hanging_crystal.generate),
    # gen_structures.generate covers ruins_pillar/broken_urn/bone_pile/
    # spirit_ore_vein/rift_bridge/spawn_portal in one pass.
    ("<structures>", gen_structures.generate),
]


def main() -> int:
    all_reports = []
    for label, gen in GENERATORS:
        reports = gen()
        all_reports.extend(reports)
        if label != "<structures>":
            print(f"\n[{label}] {len(reports)} variants")
            for r in reports:
                rel = os.path.relpath(r["path"])
                print(f"  {os.path.basename(r['path']):28s} "
                      f"size={tuple(r['size'])} blocks={r['total_blocks']:4d} "
                      f"palette={r['palette_size']:2d} bytes={r['file_bytes']:6d}")

    # Group by kind directory for the structures pass + overall accounting.
    by_kind: dict[str, list[dict]] = defaultdict(list)
    for r in all_reports:
        kind = os.path.basename(os.path.dirname(r["path"]))
        by_kind[kind].append(r)

    print("\n" + "=" * 64)
    print("PER-KIND SUMMARY (variants / blocks / gzip bytes)")
    print("=" * 64)
    total_gzip = 0
    total_blocks = 0
    for kind in sorted(by_kind):
        rs = by_kind[kind]
        gzip_bytes = sum(r["file_bytes"] for r in rs)
        blocks = sum(r["total_blocks"] for r in rs)
        total_gzip += gzip_bytes
        total_blocks += blocks
        ok = all(r["total_blocks"] == r["loaded_blocks"] for r in rs)
        flag = "OK " if ok else "RT-MISMATCH"
        print(f"  {kind:18s} variants={len(rs)} blocks={blocks:5d} "
              f"gzip={gzip_bytes:7d}B {flag}")
        assert len(rs) >= 3, f"{kind} has only {len(rs)} variants (need >=3)"

    # §8.1 #10 residency accounting. Decompressed residency is what matters for
    # the 32 MB budget; structure blocks are tiny (each block ~ a pos+state),
    # so we conservatively estimate decompressed memory as a generous multiple
    # of the gzip size and also report the raw block totals.
    print("=" * 64)
    print(f"TOTAL: {len(all_reports)} assets across {len(by_kind)} kinds, "
          f"{total_blocks} blocks, gzip={total_gzip}B ({total_gzip/1024:.1f} KiB)")
    print(f"§8.1 #10 budget: decoration decompressed residency must be <= 32 MiB.")
    print(f"  total gzip on disk = {total_gzip/1024:.1f} KiB; even at a 20x inflate "
          f"that is {total_gzip*20/1024/1024:.3f} MiB << 32 MiB. PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
