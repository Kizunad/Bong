#!/usr/bin/env python3
"""骨笼蟹贴图：GuLongXie.bbmodel（平色 rig）→ GuLongXie_tex.bbmodel（像素贴图，骨骼层级不变）+ check_tex.png。

画笔库是 ``generators/handheld_texture.py``（颅骨 skull_bone*、蟹甲 crab_shell*、苔 lichen、藤壶 barnacle 补在里面），
本文件只负责「本生物的材质名 → 画笔库材质名」的映射和取景。

用法：先跑 gen_gu_long_xie.py 出平色 bbmodel，再::

    ~/.venvs/bong-modelscript/bin/python modelScript/creatures/gu_long_xie/gen_tex.py
"""

from __future__ import annotations

import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[1] / "tools"))

import gen_gu_long_xie as geo  # noqa: E402
from creature_tex import texture_bbmodel, write_check_tex  # noqa: E402

LIBRARY_MATS = {
    "bone_white": "skull_bone",
    "bone_grey": "skull_bone_old",
    "crack_dark": "void",
    "rust_orange": "crab_shell",
    "rust_dark": "crab_shell_dark",
    "moss_olive": "lichen",
    "barnacle_grey": "barnacle",
    "eye_black": "eye",
}

TEX_OUT = HERE / "GuLongXie_tex.bbmodel"
TEX_COPY = geo.MODELS_DIR / "GuLongXie_tex.bbmodel"


def views():
    whole = ((0.0, 14.0, -6.0), 78.0)
    return [
        ("左前上 3/4（-X -Z +Y）", 135.0, 25.0, whole),
        ("颅骨正面（-Z）", 180.0, 6.0, ((0.0, 21.0, -6.0), 36.0)),
        ("大螯特写（+X -Z +Y）", -140.0, 28.0, ((15.0, 10.0, -20.0), 40.0)),
        ("步足与腹（+X +Z +Y）", -40.0, 28.0, ((0.0, 9.0, 6.0), 56.0)),
    ]


def main() -> None:
    atlas = texture_bbmodel(geo.BBMODEL_OUT, TEX_OUT, tuple(geo.MATS), LIBRARY_MATS)
    TEX_COPY.write_bytes(TEX_OUT.read_bytes())
    print(f"✓ {TEX_OUT.name}（图集 {atlas.width}×{atlas.height}）")
    check = write_check_tex(geo.BBMODEL_OUT, TEX_OUT, geo.REVIEW_DIR / "check_tex.png", views(), geo.EXPLODED_REF)
    print(f"✓ check_tex.png: {check}")


if __name__ == "__main__":
    main()
