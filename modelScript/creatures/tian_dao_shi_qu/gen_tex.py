#!/usr/bin/env python3
"""天道尸蛆贴图：TianDaoShiQu.bbmodel（平色 rig）→ TianDaoShiQu_tex.bbmodel（像素贴图，骨骼层级不变）+ check_tex.png。

画笔库是 ``generators/handheld_texture.py``（蛆皮 grub_*、脏器、灵光碎块、腐液都补在里面），
本文件只负责「本生物的材质名 → 画笔库材质名」的映射和取景。灵光碎块只靠对比，不做发光层。

用法：先跑 gen_tian_dao_shi_qu.py 出平色 bbmodel，再::

    ~/.venvs/bong-modelscript/bin/python modelScript/creatures/tian_dao_shi_qu/gen_tex.py
"""

from __future__ import annotations

import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[1] / "tools"))

import gen_tian_dao_shi_qu as geo  # noqa: E402
from creature_tex import texture_bbmodel, write_check_tex  # noqa: E402

LIBRARY_MATS = {
    "skin_cream": "grub_cream",
    "skin_pink": "grub_pink",
    "rot_dark": "rot_brown",
    "organ_red": "organ_red",
    "glint_amber": "glint_amber",
    "glint_core": "glint_core",
    "tooth_ivory": "bone_lit",
    "ichor_brown": "ichor",
}

TEX_OUT = HERE / "TianDaoShiQu_tex.bbmodel"
TEX_COPY = geo.MODELS_DIR / "TianDaoShiQu_tex.bbmodel"


def views():
    whole = ((0.0, 24.0, 0.0), 190.0)
    return [
        ("左前上 3/4（-X -Z +Y）", 135.0, 22.0, whole),
        ("口器正面（-Z）", 180.0, 5.0, ((0.0, 30.0, -62.0), 56.0)),
        ("中段透皮窗侧视（-X）", 90.0, 0.0, ((0.0, 26.0, -14.0), 76.0)),
        ("尾端呼吸管（+X +Z +Y）", -50.0, 22.0, ((0.0, 30.0, 52.0), 70.0)),
    ]


def main() -> None:
    atlas = texture_bbmodel(geo.BBMODEL_OUT, TEX_OUT, tuple(geo.MATS), LIBRARY_MATS)
    TEX_COPY.write_bytes(TEX_OUT.read_bytes())
    print(f"✓ {TEX_OUT.name}（图集 {atlas.width}×{atlas.height}）")
    check = write_check_tex(geo.BBMODEL_OUT, TEX_OUT, geo.REVIEW_DIR / "check_tex.png", views(), geo.EXPLODED_REF)
    print(f"✓ check_tex.png: {check}")


if __name__ == "__main__":
    main()
