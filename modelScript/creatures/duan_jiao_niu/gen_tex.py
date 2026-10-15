#!/usr/bin/env python3
"""断角野牛贴图：DuanJiaoNiu.bbmodel（平色 rig）→ DuanJiaoNiu_tex.bbmodel（像素贴图，骨骼层级不变）+ check_tex.png。

画笔库是 ``generators/handheld_texture.py``：野牛垂毛（bison_fur*，竖直毛束）、角（bison_horn*）、旧伤疤（scar_tissue）
是为断角野牛补进同一个库的材质；皮 / 骨 / 蹄 / 眼用库里已有的。

野牛有 17 种材质，而 rigkit 把立方体的 color 写成「材质序号 % 8」会撞号，所以这里先按每个立方体北面 UV 所在的
色块（8px 一格，每行 8 格）反推材质序号，写进临时平色稿的 color 里（序号 < 17，不再取模），再交给 creature_tex 上贴图。

用法：先跑 gen_duan_jiao_niu.py 出平色 bbmodel，再::

    ~/.venvs/bong-modelscript/bin/python modelScript/creatures/duan_jiao_niu/gen_tex.py
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[1] / "tools"))

import gen_duan_jiao_niu as geo  # noqa: E402
from creature_tex import texture_bbmodel, write_check_tex  # noqa: E402

LIBRARY_MATS = {
    "hide": "hide",
    "hide_dk": "hide_dark",
    "hide_lt": "hide_light",
    "scar": "scar_tissue",
    "blood": "scab_flesh",
    "bone": "bone_main",
    "bone_dk": "bone_dark",
    "fur_dk": "bison_fur_dark",
    "fur": "bison_fur",
    "fur_lt": "bison_fur_lit",
    "fur_rust": "bison_fur_rust",
    "horn": "bison_horn",
    "horn_dk": "bison_horn_dark",
    "horn_fresh": "bison_horn_fresh",
    "hoof": "hoof",
    "void": "void",
    "eye": "eye_amber",
}

FLAT = HERE / "DuanJiaoNiu.bbmodel"
TEX_OUT = HERE / "DuanJiaoNiu_tex.bbmodel"
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
    whole = ((0.0, 15.0, 0.0), 54.0)
    return [
        ("左前上 3/4（-X -Z +Y）", 135.0, 22.0, whole),
        ("左侧面（-X）", 90.0, 0.0, whole),
        ("头与断角（-Z 偏左）", 150.0, 12.0, ((0.0, 14.0, -16.0), 20.0)),
        ("后半身露肋（-X +Z）", 60.0, 15.0, ((0.0, 15.0, 6.0), 26.0)),
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
