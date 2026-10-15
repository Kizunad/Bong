#!/usr/bin/env python3
"""剥皮豺贴图：BoPiChai.bbmodel（平色 rig）→ BoPiChai_tex.bbmodel（像素贴图，骨骼层级不变）+ check_tex.png。

画笔库是 ``generators/handheld_texture.py``：红毛 / 深毛（dhole_fur_*，带零星脱毛）、剥皮后苍白的硬皮、
裸露的红肉与肌腱（muscle_*，顺肌束的纤维）、骨白脊突（spine_bone）是为剥皮豺补进同一个库的材质；
爪用 claw_dark，琥珀眼用 eye_amber。发光只靠对比，不做发光层。

用法：先跑 gen_bo_pi_chai.py 出平色 bbmodel，再::

    ~/.venvs/bong-modelscript/bin/python modelScript/creatures/bo_pi_chai/gen_tex.py
"""

from __future__ import annotations

import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[1] / "tools"))

import gen_bo_pi_chai as geo  # noqa: E402
from creature_tex import texture_bbmodel, write_check_tex  # noqa: E402

LIBRARY_MATS = {
    "fur_red": "dhole_fur_red",
    "fur_dark": "dhole_fur_dark",
    "fur_pale": "dhole_skin_pale",
    "muscle_raw": "muscle_raw",
    "muscle_dark": "muscle_dark",
    "bone_white": "spine_bone",
    "claw_dark": "claw_dark",
    "eye_amber": "eye_amber",
}

TEX_OUT = HERE / "BoPiChai_tex.bbmodel"
TEX_COPY = geo.MODELS_DIR / "BoPiChai_tex.bbmodel"


def views():
    whole = ((0.0, 10.0, 2.0), 54.0)
    return [
        ("左前上 3/4（-X -Z +Y）", 135.0, 22.0, whole),
        ("左肋面（-X）", 90.0, 0.0, whole),
        ("头部特写（-X -Z）", 125.0, 15.0, ((0.0, 12.0, -13.0), 20.0)),
        ("肩背特写（-X -Z +Y）", 140.0, 30.0, ((0.0, 12.0, -1.0), 24.0)),
    ]


def main() -> None:
    atlas = texture_bbmodel(geo.BBMODEL_OUT, TEX_OUT, tuple(geo.MATS), LIBRARY_MATS)
    TEX_COPY.write_bytes(TEX_OUT.read_bytes())
    print(f"✓ {TEX_OUT.name}（图集 {atlas.width}×{atlas.height}）")
    check = write_check_tex(geo.BBMODEL_OUT, TEX_OUT, geo.REVIEW_DIR / "check_tex.png", views(), geo.EXPLODED_REF)
    print(f"✓ check_tex.png: {check}")


if __name__ == "__main__":
    main()
