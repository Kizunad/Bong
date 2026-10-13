#!/usr/bin/env python3
"""锈甲鳖虫贴图版：几何沿用 gen_xiu_jia_bie.build_rig，材质换成画笔库的像素贴图。

输出 XiuJiaBie_tex.bbmodel（本目录）与 check_tex.png（参考图 | 平色 | 贴图）。
不接线、不写 manifest、不装进 client 资源树。

    ~/.venvs/bong-modelscript/bin/python modelScript/creatures/xiu_jia_bie/gen_xiu_jia_bie_tex.py
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

from PIL import Image, ImageDraw

CREATURE_DIR = Path(__file__).resolve().parent
REPO = CREATURE_DIR.parents[2]
sys.path.insert(0, str(CREATURE_DIR))
sys.path.insert(0, str(REPO / "modelScript" / "generators"))
sys.path.insert(0, str(REPO / "modelScript" / "tools"))

import gen_xiu_jia_bie as base  # noqa: E402
from cjk_font import load_font  # noqa: E402
from creature_texture import rigged_textured_bbmodel  # noqa: E402

# parts.MATS 的材质名 -> handheld_texture 画笔库的材质名
LIBRARY_MATERIAL = {
    "rust_iron": "rust_iron",
    "rust_dark": "rust_heavy",
    "gold_flake": "gold",
    "gold_dull": "gold_dull",
    "copper_orange": "copper_edge",
    "copper_patina": "copper_green",
    "rivet": "rivet",
    "chitin_dark": "shell_dark",
    "chitin_mid": "shell_mid",
    "belly_pale": "belly_pale",
    "belly_seg": "belly_seg",
    "eye": "eye",
    "jaw_gold": "gold_dull",
    "jaw_dark": "rust_heavy",
}


def main() -> None:
    rig = base.build_rig("all")
    mats = {uid: LIBRARY_MATERIAL[m] for uid, m in rig.cube_mats.items()}

    tex_path = CREATURE_DIR / "XiuJiaBie_tex.bbmodel"
    flat_path = CREATURE_DIR / "_tmp_flat.bbmodel"
    tex_path.write_text(json.dumps(rigged_textured_bbmodel(rig, mats, "XiuJiaBie_tex")), encoding="utf-8")
    flat_path.write_text(json.dumps(rigged_textured_bbmodel(rig, mats, "XiuJiaBie_flat", flat=True)), encoding="utf-8")
    print(f"✓ {tex_path}（{len(rig.elements)} cubes）")

    if not (base.REVIEW_DIR / "xiu_jia_bie_ref.png").exists():
        print("跳过 check_tex.png：缺参考图（审阅用，非必需）")
        return
    sheet_tex, _ = base.render_six_views(tex_path, "贴图版")
    sheet_flat, _ = base.render_six_views(flat_path, "平色对照")
    flat_path.unlink(missing_ok=True)

    ref = Image.open(base.REVIEW_DIR / "xiu_jia_bie_ref.png").convert("RGB")
    ref = ref.resize((int(ref.width * sheet_tex.height / ref.height), sheet_tex.height))
    canvas = Image.new("RGB", (ref.width + sheet_flat.width + sheet_tex.width + 40, sheet_tex.height + 40), (25, 26, 29))
    draw = ImageDraw.Draw(canvas)
    font = load_font(18)
    x = 10
    for label, im in (("参考图", ref), ("平色（每材质一色）", sheet_flat), ("像素贴图", sheet_tex)):
        draw.text((x + 4, 8), label, fill=(225, 225, 225), font=font)
        canvas.paste(im, (x, 34))
        x += im.width + 10
    out = base.REVIEW_DIR / "check_tex.png"
    canvas.save(out)
    print(f"✓ {out} {canvas.size}")


if __name__ == "__main__":
    main()
