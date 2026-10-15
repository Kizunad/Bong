#!/usr/bin/env python3
"""碎岩猿贴图：SuiYanYuan.bbmodel（平色 rig）→ SuiYanYuan_tex.bbmodel（像素贴图，骨骼层级不变）+ check_tex.png。

画笔库是 ``generators/handheld_texture.py``：灰褐长毛（ape_fur*，竖直毛束）、崩口灰岩（rock_grey*）、拧股草绳（straw_rope*）、
深色猿脸与手掌裸皮（ape_skin*）是为碎岩猿补进同一个库的材质；岩上的灰绿苔用库里已有的 lichen，牙用 bone_main，爪用 claw_dark，
眼用 eye_amber（只靠对比，不做发光层）。

碎岩猿有 18 种材质，超过 rigkit 的 8 种（color = 材质序号 % 8 会撞号），所以先按每个立方体北面 UV 所在的色块
（8px 一格，每行 8 格）反推材质序号写进临时平色稿的 color，再交给 creature_tex 上贴图。

用法：先跑 gen_sui_yan_yuan.py 出平色 bbmodel，再::

    ~/.venvs/bong-modelscript/bin/python modelScript/creatures/sui_yan_yuan/gen_tex.py
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[1] / "tools"))

import gen_sui_yan_yuan as geo  # noqa: E402
from creature_tex import texture_bbmodel, write_check_tex  # noqa: E402

LIBRARY_MATS = {
    "fur_dark": "ape_fur_dark",
    "fur": "ape_fur",
    "fur_lt": "ape_fur_lit",
    "fur_grey": "ape_fur_grey",
    "cloth": "cloth_tan",
    "skin": "ape_skin",
    "skin_lt": "ape_skin_lit",
    "rock": "rock_grey",
    "rock_dk": "rock_grey_dark",
    "rock_lt": "rock_grey_lit",
    "rock_crack": "rock_grey_crack",
    "lichen": "lichen",
    "rope": "straw_rope",
    "rope_dk": "straw_rope_dark",
    "bone": "bone_main",
    "claw": "claw_dark",
    "void": "void",
    "eye": "eye_amber",
}

FLAT = HERE / "SuiYanYuan.bbmodel"
TEX_OUT = HERE / "SuiYanYuan_tex.bbmodel"
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
    whole = ((0.0, 15.0, -2.0), 44.0)
    return [
        ("左前上 3/4（-X -Z +Y）", 140.0, 18.0, whole),
        ("正面（-Z）", 180.0, 4.0, whole),
        ("猿脸（-Z 偏左）", 160.0, 8.0, ((0.0, 26.0, -13.0), 16.0)),
        ("右手与投掷石（+X -Z）", -150.0, 10.0, ((11.0, 8.0, -8.0), 18.0)),
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
