#!/usr/bin/env python3
"""经脉工厂内景方块生成器 —— b12: merger (合流器)，按爆炸图 x12_merger.png 重做。

±X 两面进、+Z 出，-Z 是封闭后壁；骨楔尖朝出口 +Z。
部件、逐行宽度表和门禁都在 flow_block_parts.py，与 b05 分流器共用（分流器是它沿 z 的镜像）。
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
if str(REPO) not in sys.path:
    sys.path.insert(0, str(REPO))

from modelScript.generators.meridian_factory import flow_block_parts as fb  # noqa: E402

PIECE = "merger"
SINGLE_Z = +1
REF_NAME = "x12_merger.png"
TMP_DIR = Path("/tmp/meridian_factory_merger_parts")


def all_cubes():
    return fb.all_cubes(SINGLE_Z)


def generate_bbmodel(out_path: Path) -> Path:
    return fb.write_bbmodel(out_path, PIECE, all_cubes())


def self_test() -> None:
    print("运行 gen_merger.py 差分自证...")
    fb.self_test(SINGLE_Z, PIECE)
    print("✓ gen_merger.py 差分自证全绿")


def main() -> None:
    parser = argparse.ArgumentParser(description="经脉工厂内景 b12 merger 生成器")
    parser.add_argument("--self-test", action="store_true", help="只跑门禁差分自证")
    args = parser.parse_args()

    self_test()
    if args.self_test:
        return
    fb.render_review(PIECE, SINGLE_Z, REF_NAME, TMP_DIR)


if __name__ == "__main__":
    main()
