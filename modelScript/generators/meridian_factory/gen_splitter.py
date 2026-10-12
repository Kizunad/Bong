#!/usr/bin/env python3
"""经脉工厂内景方块生成器 —— b05: splitter (分流器)，按爆炸图 x05_splitter.png 重做。

-Z 一面进、±X 两面出（沿用旧生成器的入口/出口约定），+Z 是封闭后壁；骨楔尖朝入口 -Z 把流劈开。
部件、逐行宽度表和门禁都在 flow_block_parts.py，与 b12 合流器共用（分流器是合流器沿 z 的镜像）。
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
if str(REPO) not in sys.path:
    sys.path.insert(0, str(REPO))

from modelScript.generators.meridian_factory import flow_block_parts as fb  # noqa: E402

PIECE = "splitter"
SINGLE_Z = -1
REF_NAME = "x05_splitter.png"
TMP_DIR = Path("/tmp/meridian_factory_splitter_parts")


def all_cubes():
    return fb.all_cubes(SINGLE_Z)


def generate_bbmodel(out_path: Path) -> Path:
    return fb.write_bbmodel(out_path, PIECE, all_cubes())


def self_test() -> None:
    print("运行 gen_splitter.py 差分自证...")
    fb.self_test(SINGLE_Z, PIECE)
    print("✓ gen_splitter.py 差分自证全绿")


def main() -> None:
    parser = argparse.ArgumentParser(description="经脉工厂内景 b05 splitter 生成器")
    parser.add_argument("--self-test", action="store_true", help="只跑门禁差分自证")
    args = parser.parse_args()

    self_test()
    if args.self_test:
        return
    fb.render_review(PIECE, SINGLE_Z, REF_NAME, TMP_DIR)


if __name__ == "__main__":
    main()
