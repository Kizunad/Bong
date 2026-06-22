#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = ["pillow"]
# ///
"""#3 手持盾无盾模型：生成木盾 / 骨盾的 SML OBJ + MTL + 16² 贴图。

渲染链（见 BongWeaponModelRegistry.SHIELD_TEMPLATE_IDS / WeaponRenderBootstrap）：
  EquippedShieldStore → MixinHeldItemRenderer / MixinPlayerEntityHeldItem 合成宿主 vanilla
  item 的 fake stack → 该 item model JSON 被 SML 劫持到此处生成的 bong OBJ → 显示 3D 盾。

每个盾 = 面板 slab（map_Kd 木纹/骨纹）+ 中央 boss 凸起（独立 material，金属/骨节）。
OBJ 约定对齐 axe_bone：8 verts/box、共享 4 角 vt（每面整张贴图）、f v/vt/vn、6 法线、usemtl 分部件。

  uv run client/tools/gen_shield_models.py
  uv run client/tools/render_held_item.py wooden_shield   # 预览
"""

from __future__ import annotations

from pathlib import Path

from PIL import Image

CLIENT_ROOT = Path(__file__).resolve().parents[1]
RES = CLIENT_ROOT / "src" / "main" / "resources"
MODELS = RES / "assets" / "bong" / "models" / "item"
TEX = RES / "assets" / "bong" / "textures" / "item"

# ── OBJ 几何 helpers ──────────────────────────────────────────────────────
# 共享 4 角 UV + 6 面法线，和 axe_bone.obj 同构。
_VT = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)]
_VN = [
    (0.0, 0.0, 1.0),   # 1 +Z front
    (0.0, 0.0, -1.0),  # 2 -Z back
    (-1.0, 0.0, 0.0),  # 3 -X left
    (1.0, 0.0, 0.0),   # 4 +X right
    (0.0, 1.0, 0.0),   # 5 +Y top
    (0.0, -1.0, 0.0),  # 6 -Y bottom
]
# 每面 4 个本地顶点序（CCW，外法线）+ 该面法线序号
_FACES = [
    ((4, 5, 6, 7), 1),  # front +Z
    ((1, 0, 3, 2), 2),  # back -Z
    ((0, 4, 7, 3), 3),  # left -X
    ((5, 1, 2, 6), 4),  # right +X
    ((7, 6, 2, 3), 5),  # top +Y
    ((0, 1, 5, 4), 6),  # bottom -Y
]


def _corners(c, h):
    cx, cy, cz = c
    hx, hy, hz = h
    return [
        (cx - hx, cy - hy, cz - hz),
        (cx + hx, cy - hy, cz - hz),
        (cx + hx, cy + hy, cz - hz),
        (cx - hx, cy + hy, cz - hz),
        (cx - hx, cy - hy, cz + hz),
        (cx + hx, cy - hy, cz + hz),
        (cx + hx, cy + hy, cz + hz),
        (cx - hx, cy + hy, cz + hz),
    ]


def build_obj(asset_id: str, parts: list[dict]) -> str:
    """parts: [{name, mtl, center, half}]，每个 part 一个 box + usemtl。"""
    lines = [f"# {asset_id}.obj -- shield held-model #3 (gen_shield_models.py)", f"mtllib {asset_id}.mtl", f"o {asset_id}"]
    for u, v in _VT:
        lines.append(f"vt {u:.4f} {v:.4f}")
    for nx, ny, nz in _VN:
        lines.append(f"vn {nx:.4f} {ny:.4f} {nz:.4f}")
    vbase = 0
    for part in parts:
        lines.append(f"# part: {part['name']}")
        for x, y, z in _corners(part["center"], part["half"]):
            lines.append(f"v {x:.4f} {y:.4f} {z:.4f}")
        lines.append(f"usemtl {part['mtl']}")
        for local, vn in _FACES:
            toks = []
            for i, li in enumerate(local):
                toks.append(f"{vbase + li + 1}/{i + 1}/{vn}")
            lines.append("f " + " ".join(toks))
        vbase += 8
    return "\n".join(lines) + "\n"


def build_mtl(asset_id: str, mats: list[dict]) -> str:
    """mats: [{name, kd, map}]，map=贴图序号(int)或 None(纯色 Kd)。"""
    out = [f"# {asset_id} materials -- gen_shield_models.py"]
    for m in mats:
        r, g, b = m["kd"]
        out.append("")
        out.append(f"newmtl {m['name']}")
        out.append("Ka 1.000000 1.000000 1.000000")
        out.append(f"Kd {r:.6f} {g:.6f} {b:.6f}")
        out.append("Ks 0.000000 0.000000 0.000000")
        out.append("Ns 10.000000")
        out.append("d 1.000000")
        out.append("illum 1")
        if m["map"] is not None:
            out.append(f"map_Kd bong:item/{asset_id}/{m['map']}")
    return "\n".join(out) + "\n"


# ── 16² 贴图绘制 ───────────────────────────────────────────────────────────
def _px(img, x, y, rgb):
    if 0 <= x < img.width and 0 <= y < img.height:
        img.putpixel((x, y), rgb)


def wood_face_tex() -> Image.Image:
    """木盾面板：竖向木板 + 暗边框 + 上下铁条。"""
    img = Image.new("RGBA", (16, 16), (139, 90, 43, 255))
    planks = [(122, 78, 38), (155, 101, 53), (134, 86, 41), (146, 95, 49)]
    for x in range(16):
        col = planks[(x // 4) % len(planks)]
        for y in range(16):
            _px(img, x, y, (*col, 255))
        # 板缝竖线
        if x % 4 == 0:
            for y in range(16):
                _px(img, x, y, (74, 47, 24, 255))
    # 暗边框
    for i in range(16):
        for (x, y) in [(i, 0), (i, 15), (0, i), (15, i)]:
            _px(img, x, y, (58, 36, 18, 255))
    # 上下铁包边
    for x in range(16):
        _px(img, x, 1, (120, 120, 128, 255))
        _px(img, x, 14, (120, 120, 128, 255))
    return img


def _dome_shade(img, base, hi):
    """以中心为亮点画一个圆顶高光，边缘渐暗——让 boss 读作金属/骨节凸起。"""
    cx, cy = 7.5, 7.5
    br, bg, bb = base
    hr, hg, hb = hi
    for x in range(16):
        for y in range(16):
            d = ((x - cx) ** 2 + (y - cy) ** 2) ** 0.5 / 10.6  # 0(center)..~1(corner)
            t = max(0.0, 1.0 - d)
            r = int(br + (hr - br) * t)
            g = int(bg + (hg - bg) * t)
            b = int(bb + (hb - bb) * t)
            _px(img, x, y, (r, g, b, 255))


def metal_boss_tex() -> Image.Image:
    """金属 boss：圆顶高光的灰铁。"""
    img = Image.new("RGBA", (16, 16), (150, 150, 158, 255))
    _dome_shade(img, base=(112, 112, 120), hi=(214, 214, 224))
    return img


def bone_face_tex() -> Image.Image:
    """骨盾面板：竖向骨片（4 条）以绳缚缝相连 + 骨纹 + 浅褐边框。无横向网格，读作骨而非窗格。"""
    img = Image.new("RGBA", (16, 16), (232, 224, 204, 255))
    seam_x = {0, 4, 8, 12}  # 4 条竖骨片之间的绳缚缝
    slat_tone = [(234, 226, 206), (222, 212, 188), (238, 230, 212), (226, 217, 197)]
    for x in range(16):
        col = slat_tone[(x // 4) % len(slat_tone)]
        for y in range(16):
            # 竖向骨纹：随 y 轻微明暗起伏
            jitter = ((x * 5 + y * 3) % 7) - 3
            _px(img, x, y, (col[0] + jitter, col[1] + jitter, col[2] + jitter, 255))
        if x in seam_x:
            for y in range(1, 15):
                _px(img, x, y, (120, 102, 74, 255))  # 绳缚缝（竖）
    # 顶/底各一道绑绳横结（细，仅 1px，不切成网格）
    for x in range(1, 15):
        _px(img, x, 2, (110, 92, 66, 255))
        _px(img, x, 13, (110, 92, 66, 255))
    # 浅褐边框
    for i in range(16):
        for (x, y) in [(i, 0), (i, 15), (0, i), (15, i)]:
            _px(img, x, y, (176, 160, 128, 255))
    return img


def bone_boss_tex() -> Image.Image:
    """骨盾 boss：圆顶高光的骨节。"""
    img = Image.new("RGBA", (16, 16), (216, 200, 168, 255))
    _dome_shade(img, base=(176, 160, 128), hi=(240, 232, 214))
    return img


def write(path: Path, text: str):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")
    print(f"  wrote {path.relative_to(CLIENT_ROOT)}")


def write_tex(asset_id: str, idx: int, img: Image.Image):
    p = TEX / asset_id / f"{idx}.png"
    p.parent.mkdir(parents=True, exist_ok=True)
    img.save(p)
    print(f"  wrote {p.relative_to(CLIENT_ROOT)}")


# ── 盾定义 ────────────────────────────────────────────────────────────────
SHIELDS = {
    "wooden_shield": {
        # heater 盾：偏高窄
        "parts": [
            {"name": "face", "mtl": "wooden_shield_face", "center": (0, 0, 0), "half": (0.32, 0.46, 0.035)},
            {"name": "boss", "mtl": "wooden_shield_boss", "center": (0, 0, 0.055), "half": (0.11, 0.11, 0.028)},
            {"name": "grip", "mtl": "wooden_shield_boss", "center": (0, 0, -0.05), "half": (0.045, 0.16, 0.02)},
        ],
        "mats": [
            {"name": "wooden_shield_face", "kd": (0.83, 0.77, 0.66), "map": 0},
            {"name": "wooden_shield_boss", "kd": (0.60, 0.60, 0.63), "map": 1},
        ],
        "tex": {0: wood_face_tex, 1: metal_boss_tex},
    },
    "bone_shield": {
        # round 盾：偏方正
        "parts": [
            {"name": "face", "mtl": "bone_shield_face", "center": (0, 0, 0), "half": (0.36, 0.38, 0.03)},
            {"name": "boss", "mtl": "bone_shield_boss", "center": (0, 0, 0.05), "half": (0.095, 0.095, 0.025)},
            {"name": "grip", "mtl": "bone_shield_boss", "center": (0, 0, -0.045), "half": (0.045, 0.14, 0.018)},
        ],
        "mats": [
            {"name": "bone_shield_face", "kd": (0.91, 0.88, 0.80), "map": 0},
            {"name": "bone_shield_boss", "kd": (0.85, 0.78, 0.66), "map": 1},
        ],
        "tex": {0: bone_face_tex, 1: bone_boss_tex},
    },
}

# 手持盾 display：盾立于前臂、面朝外。off_hand 默认左手，*_lefthand 为主视角。
# 初值参照 vanilla shield + axe_bone 比例，真机再微调（render 工具用 asset_config 覆盖预览）。
DISPLAY = {
    "thirdperson_righthand": {"rotation": [0, -90, 0], "translation": [4, 5, -1], "scale": [0.9, 0.9, 0.9]},
    "thirdperson_lefthand": {"rotation": [0, -90, 0], "translation": [4, 5, 1], "scale": [0.9, 0.9, 0.9]},
    "firstperson_righthand": {"rotation": [0, -90, 0], "translation": [3, 4, -4], "scale": [0.95, 0.95, 0.95]},
    "firstperson_lefthand": {"rotation": [0, -90, 0], "translation": [3, 4, -4], "scale": [0.95, 0.95, 0.95]},
    "ground": {"rotation": [0, 0, 0], "translation": [0, 2, 0], "scale": [0.5, 0.5, 0.5]},
    "gui": {"rotation": [0, 0, 0], "translation": [0, 0, 0], "scale": [1, 1, 1]},
    "fixed": {"rotation": [0, 0, 0], "translation": [0, 0, 0], "scale": [1, 1, 1]},
}


def build_model_json(asset_id: str) -> str:
    import json

    return json.dumps(
        {"parent": "sml:builtin/obj", "model": f"bong:models/item/{asset_id}/{asset_id}.obj", "display": DISPLAY},
        ensure_ascii=False,
        indent=2,
    ) + "\n"


def main():
    # 宿主 vanilla item model 路径（被 SML 劫持）— 与 BongWeaponModelRegistry 对齐
    vanilla_hosts = {"wooden_shield": "nautilus_shell", "bone_shield": "phantom_membrane"}
    minecraft_models = RES / "assets" / "minecraft" / "models" / "item"
    for asset_id, spec in SHIELDS.items():
        print(asset_id)
        obj = build_obj(asset_id, spec["parts"])
        mtl = build_mtl(asset_id, spec["mats"])
        write(MODELS / asset_id / f"{asset_id}.obj", obj)
        write(MODELS / asset_id / f"{asset_id}.mtl", mtl)
        write(MODELS / asset_id / f"{asset_id}.json", build_model_json(asset_id))
        # 劫持宿主 vanilla item model（内容与 bong JSON 一致，指向同一 OBJ）
        write(minecraft_models / f"{vanilla_hosts[asset_id]}.json", build_model_json(asset_id))
        for idx, fn in spec["tex"].items():
            write_tex(asset_id, idx, fn())
    print("done")


if __name__ == "__main__":
    main()
