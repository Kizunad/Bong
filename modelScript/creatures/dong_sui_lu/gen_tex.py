#!/usr/bin/env python3
"""冻髓鹿贴图：DongSuiLu.bbmodel（平色 rig）→ DongSuiLu_tex.bbmodel（像素贴图，骨骼层级不变）+ check_tex.png。

画笔库是 ``generators/handheld_texture.py``。霜毛（frost_fur_*）、骨灰（bone_grey）、冰晶（ice_*）、
白雾（mist）是为冻髓鹿补进同一个库的材质；蹄 / 暗衬直接用库里已有的 hoof。发光只靠对比（冰晶棱线最亮），
不做发光层。

用法：先跑 gen_dong_sui_lu.py 出平色 bbmodel，再::

    python3 modelScript/creatures/dong_sui_lu/gen_tex.py
"""

from __future__ import annotations

import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[1] / "tools"))

import gen_dong_sui_lu as geo  # noqa: E402
from creature_tex import texture_bbmodel, write_check_tex  # noqa: E402

# 生物材质 → 画笔库材质。hoof_dark 在生物里兼作鼻头 / 眼窝 / 胸腔暗衬，统一走近黑的 hoof。
LIBRARY_MATS = {
    "fur_ash": "frost_fur_ash",
    "fur_frost": "frost_fur_pale",
    "fur_dark": "frost_fur_dark",
    "bone_grey": "bone_grey",
    "hoof_dark": "hoof",
    "ice_pale": "ice_pale",
    "ice_deep": "ice_deep",
    "mist_white": "mist",
}

TEX_OUT = HERE / "DongSuiLu_tex.bbmodel"
TEX_COPY = geo.MODELS_DIR / "DongSuiLu_tex.bbmodel"


def views():
    whole = ((0.0, 22.0, -6.0), 46.0)
    return [
        ("左前上 3/4（-X -Z +Y）", 135.0, 22.0, whole),
        ("左肋面（-X）", 90.0, 0.0, whole),
        ("头角特写（-Z 偏左）", 150.0, 10.0, ((0.0, 33.0, -12.0), 26.0)),
        ("肋骨与结晶簇（+X 偏后）", -40.0, 25.0, ((0.0, 20.0, 1.0), 30.0)),
    ]


def main() -> None:
    atlas = texture_bbmodel(geo.BBMODEL_OUT, TEX_OUT, tuple(geo.MATS), LIBRARY_MATS)
    TEX_COPY.write_bytes(TEX_OUT.read_bytes())
    print(f"✓ {TEX_OUT.name}（图集 {atlas.width}×{atlas.height}）")
    check = write_check_tex(geo.BBMODEL_OUT, TEX_OUT, geo.REVIEW_DIR / "check_tex.png", views(), geo.EXPLODED_REF)
    print(f"✓ check_tex.png: {check}")


if __name__ == "__main__":
    main()
