#!/usr/bin/env python3
"""浮灵母贴图：FuLingMu.bbmodel（平色 rig）→ FuLingMu_tex.bbmodel（像素贴图，骨骼层级不变）+ check_tex.png。

画笔库是 ``generators/handheld_texture.py``（半透胶质 bell_*、脉纹、灵核 spirit_* 补在里面），
本文件只负责「本生物的材质名 → 画笔库材质名」的映射和取景。灵核与光点只靠对比，不做发光层。

用法：先跑 gen_fu_ling_mu.py 出平色 bbmodel，再::

    ~/.venvs/bong-modelscript/bin/python modelScript/creatures/fu_ling_mu/gen_tex.py
"""

from __future__ import annotations

import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[1] / "tools"))

import gen_fu_ling_mu as geo  # noqa: E402
from creature_tex import texture_bbmodel, write_check_tex  # noqa: E402

LIBRARY_MATS = {
    "jelly_cream": "bell_cream",
    "jelly_grey": "bell_grey",
    "vein_dark": "bell_vein",
    "vein_slate": "bell_vein_slate",
    "frill_tan": "bell_frill",
    "glow_teal": "spirit_core",
    "glow_white": "spirit_mote",
}

TEX_OUT = HERE / "FuLingMu_tex.bbmodel"
TEX_COPY = geo.MODELS_DIR / "FuLingMu_tex.bbmodel"


def views():
    whole = ((0.0, 22.0, 0.0), 62.0)
    return [
        ("左前上 3/4（-X -Z +Y）", 135.0, 20.0, whole),
        ("正面（-Z）", 180.0, 0.0, whole),
        ("伞下仰视（灵核与口腕）", 160.0, -35.0, ((0.0, 24.0, 0.0), 36.0)),
        ("伞盖俯视（+Y）", 0.0, 80.0, ((0.0, 36.0, 0.0), 50.0)),
    ]


def main() -> None:
    atlas = texture_bbmodel(geo.BBMODEL_OUT, TEX_OUT, tuple(geo.MATS), LIBRARY_MATS)
    TEX_COPY.write_bytes(TEX_OUT.read_bytes())
    print(f"✓ {TEX_OUT.name}（图集 {atlas.width}×{atlas.height}）")
    check = write_check_tex(geo.BBMODEL_OUT, TEX_OUT, geo.REVIEW_DIR / "check_tex.png", views(), geo.EXPLODED_REF)
    print(f"✓ check_tex.png: {check}")


if __name__ == "__main__":
    main()
