#!/usr/bin/env python3
"""灰鳞穴蜥贴图：HuiLinXueXi.bbmodel（平色 rig）→ HuiLinXueXi_tex.bbmodel（像素贴图，骨骼层级不变）+ check_tex.png。

画笔库是 ``generators/handheld_texture.py``。灰白穴鳞（cave_scale*）、腹鳞、眼位瘢痕是为地穴蜥补进同一个库的材质；
骨瘤 / 齿用库里已有的 bone_lit，爪用 claw_dark，口腔 / 舌用 gum。发光只靠对比，不做发光层。

用法：先跑 gen_hui_lin_xue_xi.py 出平色 bbmodel，再::

    python3 modelScript/creatures/hui_lin_xue_xi/gen_tex.py
"""

from __future__ import annotations

import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[1] / "tools"))

import gen_hui_lin_xue_xi as geo  # noqa: E402
from creature_tex import texture_bbmodel, write_check_tex  # noqa: E402

LIBRARY_MATS = {
    "scale_grey": "cave_scale",
    "scale_pale": "cave_scale_pale",
    "scale_dark": "cave_scale_dark",
    "belly": "cave_belly",
    "nodule": "bone_lit",
    "claw_dark": "claw_dark",
    "mouth": "gum",
    "scar": "cave_scar",
}

TEX_OUT = HERE / "HuiLinXueXi_tex.bbmodel"
TEX_COPY = geo.MODELS_DIR / "HuiLinXueXi_tex.bbmodel"


def views():
    whole = ((0.0, 5.0, -2.0), 44.0)
    return [
        ("左前上 3/4（-X -Z +Y）", 135.0, 28.0, whole),
        ("左肋面（-X）", 90.0, 0.0, whole),
        ("头部特写（-Z 偏左）", 150.0, 18.0, ((0.0, 6.0, -13.0), 16.0)),
        ("前爪特写（-X -Z）", 125.0, 25.0, ((-6.5, 2.0, -7.5), 12.0)),
    ]


def main() -> None:
    atlas = texture_bbmodel(geo.BBMODEL_OUT, TEX_OUT, tuple(geo.MATS), LIBRARY_MATS)
    TEX_COPY.write_bytes(TEX_OUT.read_bytes())
    print(f"✓ {TEX_OUT.name}（图集 {atlas.width}×{atlas.height}）")
    check = write_check_tex(geo.BBMODEL_OUT, TEX_OUT, geo.REVIEW_DIR / "check_tex.png", views(), geo.EXPLODED_REF)
    print(f"✓ check_tex.png: {check}")


if __name__ == "__main__":
    main()
