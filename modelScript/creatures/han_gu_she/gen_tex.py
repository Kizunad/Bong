#!/usr/bin/env python3
"""旱骨蛇贴图：HanGuShe.bbmodel（平色 rig）→ HanGuShe_tex.bbmodel（像素贴图，骨骼层级不变）+ check_tex.png。

画笔库是 ``generators/handheld_texture.py``：晒干的赭黄纸皮 / 碎鳞 / 旧血痂 / 裂纹 / 干舌（dry_*）是为旱骨蛇补进同一个库的材质；
骨 / 毒牙用库里已有的 bone_main / bone_dark，眼窝里那点枯黄用 eye_amber（只靠对比，不做发光层）。

旱骨蛇有 11 种材质，超过 rigkit 的 8 种（color = 材质序号 % 8 会撞号），所以先按每个立方体北面 UV 所在的色块
（8px 一格，每行 8 格）反推材质序号写进临时平色稿的 color，再交给 creature_tex 上贴图。

用法：先跑 gen_han_gu_she.py 出平色 bbmodel，再::

    ~/.venvs/bong-modelscript/bin/python modelScript/creatures/han_gu_she/gen_tex.py
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[1] / "tools"))

import gen_han_gu_she as geo  # noqa: E402
from creature_tex import texture_bbmodel, write_check_tex  # noqa: E402

LIBRARY_MATS = {
    "skin": "dry_scale",
    "skin_dk": "dry_scale_dark",
    "skin_lt": "dry_scale_lit",
    "flake": "dry_flake",
    "rust": "dry_rust",
    "crack": "dry_crack",
    "bone": "bone_main",
    "bone_dk": "bone_dark",
    "tongue": "dry_tongue",
    "void": "void",
    "eye": "eye_amber",
}

FLAT = HERE / "HanGuShe.bbmodel"
TEX_OUT = HERE / "HanGuShe_tex.bbmodel"
SWATCH, TEX = 8, 64             # 和 creature_kit 里 Rig(tex=64, swatch=8) 一致


def indexed_flat(path: Path) -> Path:
    """临时平色稿：color 改写成材质序号（由北面 UV 色块反推）。"""
    doc = json.loads(path.read_text(encoding="utf-8"))
    per_row = TEX // SWATCH
    for element in doc["elements"]:
        uv = element["faces"]["north"]["uv"]
        element["color"] = int(uv[1] // SWATCH) * per_row + int(uv[0] // SWATCH)
    tmp = HERE / "_tmp_indexed.bbmodel"
    tmp.write_text(json.dumps(doc, ensure_ascii=False), encoding="utf-8")
    return tmp


def views():
    whole = ((0.0, 20.0, -4.0), 64.0)
    return [
        ("左前上 3/4（-X -Z +Y）", 135.0, 22.0, whole),
        ("左侧面（-X）", 90.0, 0.0, whole),
        ("头骨与毒牙（-Z 偏左）", 150.0, 10.0, ((0.0, 38.0, -26.0), 20.0)),
        ("盘圈与颈根（+X +Z +Y）", -40.0, 30.0, ((2.0, 8.0, 0.0), 40.0)),
    ]


def main() -> None:
    tmp = indexed_flat(FLAT)
    try:
        atlas = texture_bbmodel(tmp, TEX_OUT, tuple(geo.MATS), LIBRARY_MATS)
        print(f"✓ {TEX_OUT.name}（图集 {atlas.width}×{atlas.height}）")
        review = geo.SPEC.review_dir
        ref = review / f"{geo.SPEC.model_id}_ref.png"
        check = write_check_tex(tmp, TEX_OUT, review / "check_tex.png", views(), ref if ref.exists() else None)
        print(f"✓ check_tex.png: {check}")
    finally:
        tmp.unlink(missing_ok=True)


if __name__ == "__main__":
    main()
