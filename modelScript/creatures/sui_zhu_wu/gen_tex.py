#!/usr/bin/env python3
"""髓蛀蜈贴图：SuiZhuWu.bbmodel（平色 rig）→ SuiZhuWu_tex.bbmodel（像素贴图，骨骼层级不变）+ check_tex.png。

画笔库是 ``generators/handheld_texture.py``（骨质甲壳 chitin_*、髓渍、骨粉都在里面），
本文件只负责「本生物的材质名 → 画笔库材质名」的映射和取景。发光 / 髓渍只靠对比，不做发光层。

用法：先跑 gen_sui_zhu_wu.py 出平色 bbmodel，再::

    python3 modelScript/creatures/sui_zhu_wu/gen_tex.py
"""

from __future__ import annotations

import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[1] / "tools"))

import gen_sui_zhu_wu as geo  # noqa: E402
from creature_tex import texture_bbmodel, write_check_tex  # noqa: E402

LIBRARY_MATS = {
    "bone_white": "chitin_white",
    "bone_ochre": "chitin_ochre",
    "bone_grey": "chitin_grey",
    "bone_dark": "chitin_dark",
    "dust_white": "bone_dust",
    "rust_brown": "claw_rust",
    "tooth_ivory": "bone_lit",
    "marrow_amber": "marrow_amber",
}

TEX_OUT = HERE / "SuiZhuWu_tex.bbmodel"
TEX_COPY = geo.MODELS_DIR / "SuiZhuWu_tex.bbmodel"


def views():
    whole = ((0.0, 5.0, 4.0), 86.0)
    return [
        ("左前上 3/4（-X -Z +Y）", 135.0, 25.0, whole),
        ("钻头正面（-Z）", 180.0, 8.0, ((0.0, 5.5, -22.0), 24.0)),
        ("脊背特写（-X -Z +Y）", 140.0, 30.0, ((0.0, 5.0, 2.0), 30.0)),
        ("尾钳特写（+X +Z +Y）", -40.0, 25.0, ((0.0, 5.0, 42.0), 28.0)),
    ]


def main() -> None:
    atlas = texture_bbmodel(geo.BBMODEL_OUT, TEX_OUT, tuple(geo.MATS), LIBRARY_MATS)
    TEX_COPY.write_bytes(TEX_OUT.read_bytes())
    print(f"✓ {TEX_OUT.name}（图集 {atlas.width}×{atlas.height}）")
    check = write_check_tex(geo.BBMODEL_OUT, TEX_OUT, geo.REVIEW_DIR / "check_tex.png", views(), geo.EXPLODED_REF)
    print(f"✓ check_tex.png: {check}")


if __name__ == "__main__":
    main()
