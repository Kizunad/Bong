#!/usr/bin/env python3
"""残背驮兽贴图版：几何沿用 gen_can_bei_tuo.build_rig，材质换成画笔库的像素贴图。

输出 CanBeiTuo_tex.bbmodel（本目录）与 check_tex.png（参考图 | 平色 | 贴图）。
不接线、不写 manifest、不装进 client 资源树。

    ~/.venvs/bong-modelscript/bin/python modelScript/creatures/can_bei_tuo/gen_can_bei_tuo_tex.py
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

import gen_can_bei_tuo as base  # noqa: E402
from cjk_font import load_font  # noqa: E402
from creature_texture import rigged_textured_bbmodel  # noqa: E402

# parts.MATS 的材质名 -> handheld_texture 画笔库的材质名
LIBRARY_MATERIAL = {
    "hide_base": "hide",
    "hide_dark": "hide_dark",
    "hide_belly": "hide_light",
    "skin_fold": "hide_fold",
    "moss_layer1": "moss_dark",
    "moss_layer2": "moss",
    "moss_crust": "moss_light",
    "moss_flower": "moss_flower",
    "horn_bone": "horn",
    "snout_horn": "horn_dark",
    "hoof_horn": "hoof",
    "eye": "eye",
    "socket": "void",
}


def main() -> None:
    rig = base.build_rig("all")
    mats = {uid: LIBRARY_MATERIAL[m] for uid, m in rig.cube_mats.items()}

    tex_path = CREATURE_DIR / "CanBeiTuo_tex.bbmodel"
    flat_path = CREATURE_DIR / "_tmp_flat.bbmodel"
    tex_path.write_text(json.dumps(rigged_textured_bbmodel(rig, mats, "CanBeiTuo_tex")), encoding="utf-8")
    flat_path.write_text(json.dumps(rigged_textured_bbmodel(rig, mats, "CanBeiTuo_flat", flat=True)), encoding="utf-8")
    print(f"✓ {tex_path}（{len(rig.elements)} cubes）")

    sheet_tex, _ = base.render_six_views(tex_path, "贴图版")
    sheet_flat, _ = base.render_six_views(flat_path, "平色对照")
    flat_path.unlink(missing_ok=True)

    ref = Image.open(base.REVIEW_DIR / "can_bei_tuo_ref.png").convert("RGB")
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
