#!/usr/bin/env python3
"""劫雷鸦贴图：JieLeiYa.bbmodel（平色 rig）→ JieLeiYa_tex.bbmodel（像素贴图，骨骼层级不变）+ check_tex.png。

画笔库是 ``generators/handheld_texture.py``（焦羽 crow_*、雷痕 bolt_blue、眼白 eye_white 补在里面），
本文件只负责「本生物的材质名 → 画笔库材质名」的映射和取景。雷痕与眼里的白光只靠对比，不做发光层。

用法：先跑 gen_jie_lei_ya.py 出平色 bbmodel，再::

    ~/.venvs/bong-modelscript/bin/python modelScript/creatures/jie_lei_ya/gen_tex.py
"""

from __future__ import annotations

import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[1] / "tools"))

import gen_jie_lei_ya as geo  # noqa: E402
from creature_tex import texture_bbmodel, write_check_tex  # noqa: E402

LIBRARY_MATS = {
    "feather_black": "crow_black",
    "feather_dark": "crow_dark",
    "char_brown": "crow_char",
    "ash_grey": "crow_ash",
    "bolt_blue": "bolt_blue",
    "eye_white": "eye_white",
    "horn": "crow_beak",
    "horn_dark": "crow_claw",
}

TEX_OUT = HERE / "JieLeiYa_tex.bbmodel"
TEX_COPY = geo.MODELS_DIR / "JieLeiYa_tex.bbmodel"


def views():
    return [
        ("左前上 3/4（-X -Z +Y）", 135.0, 25.0, ((0.0, 17.0, 0.0), 120.0)),
        ("头吻面（-Z）", 180.0, 6.0, ((0.0, 20.0, -16.0), 24.0)),
        ("右翼特写（+X +Z +Y）", -35.0, 28.0, ((26.0, 19.0, -2.0), 62.0)),
        ("躯干与尾（左侧 -X）", 90.0, 8.0, ((0.0, 15.0, 4.0), 56.0)),
    ]


def main() -> None:
    atlas = texture_bbmodel(geo.BBMODEL_OUT, TEX_OUT, tuple(geo.MATS), LIBRARY_MATS)
    TEX_COPY.write_bytes(TEX_OUT.read_bytes())
    print(f"✓ {TEX_OUT.name}（图集 {atlas.width}×{atlas.height}）")
    check = write_check_tex(geo.BBMODEL_OUT, TEX_OUT, geo.REVIEW_DIR / "check_tex.png", views(), geo.EXPLODED_REF)
    print(f"✓ check_tex.png: {check}")


if __name__ == "__main__":
    main()
