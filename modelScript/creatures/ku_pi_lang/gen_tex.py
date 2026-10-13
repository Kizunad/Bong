#!/usr/bin/env python3
"""枯皮狼贴图：KuPiLang.bbmodel（平色 rig）→ KuPiLang_tex.bbmodel（像素贴图，骨骼层级不变）+ check_tex.png。

画笔库是 ``generators/handheld_texture.py``（毛 / 结痂肉 / 骨 / 琥珀眼都在里面），
本文件只负责「本生物的材质名 → 画笔库材质名」的映射和取景。发光只靠对比（眼睛），不做发光层。

用法：先跑 gen_ku_pi_lang.py 出平色 bbmodel，再::

    python3 modelScript/creatures/ku_pi_lang/gen_tex.py
"""

from __future__ import annotations

import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[1] / "tools"))

import gen_ku_pi_lang as geo  # noqa: E402
from creature_tex import texture_bbmodel, write_check_tex  # noqa: E402

# 生物材质 → 画笔库材质。bone_dark 在生物里是近黑（爪、胸腔暗衬、鼻头），库里的 bone_dark 是淡骨色，所以走 void。
LIBRARY_MATS = {
    "pelt_dark": "fur_dark",
    "pelt_mangy": "fur_mangy",
    "pelt_rust": "fur_rust",
    "flesh_scab": "scab_flesh",
    "bone_white": "bone_main",
    "bone_dark": "void",
    "fang_white": "bone_lit",
    "eye_amber": "eye_amber",
}

TEX_OUT = HERE / "KuPiLang_tex.bbmodel"
TEX_COPY = geo.MODELS_DIR / "KuPiLang_tex.bbmodel"


def views():
    whole = ((0.0, 10.0, 0.7), 42.0)
    return [
        ("左前上 3/4（-X -Z +Y）", 135.0, 25.0, whole),
        ("左肋面（-X）", 90.0, 0.0, whole),
        ("头吻特写（-Z 偏左）", 150.0, 12.0, ((0.0, 14.5, -12.5), 17.0)),
        ("后躯特写（+Z 偏右）", -35.0, 20.0, ((2.0, 10.0, 6.0), 24.0)),
    ]


def main() -> None:
    atlas = texture_bbmodel(geo.BBMODEL_OUT, TEX_OUT, tuple(geo.MATS), LIBRARY_MATS)
    TEX_COPY.write_bytes(TEX_OUT.read_bytes())
    print(f"✓ {TEX_OUT.name}（图集 {atlas.width}×{atlas.height}）")
    check = write_check_tex(geo.BBMODEL_OUT, TEX_OUT, geo.REVIEW_DIR / "check_tex.png", views(), geo.EXPLODED_REF)
    print(f"✓ check_tex.png: {check}")


if __name__ == "__main__":
    main()
