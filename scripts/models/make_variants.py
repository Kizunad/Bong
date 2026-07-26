#!/usr/bin/env python3
"""基于用户最新几何生成 3 个尾巴蓝度变体（0 / 半 / 全，base→tip 填充），对应老鼠三态。
只改：骨骼名/枢轴(原地,uuid不变→动画仍绑)、颜色、尾脊 blue 填充量。几何+动画保留。
用法: python3 make_variants.py <fill 0..4> <out.bbmodel>"""
import base64
import io
import json
import sys
import uuid
from pathlib import Path

from PIL import Image, ImageDraw

SRC = Path("local_models/devour_rat_v3_user2.bbmodel")
PALETTE = {
    "back": (26, 26, 32, 255), "belly": (46, 46, 56, 255), "foot": (12, 12, 16, 255),
    "tail": (42, 68, 240, 255), "eye": (232, 24, 24, 255), "ear": (34, 30, 40, 255),
    "tooth": (176, 172, 168, 255),
}
GLOW = {"tail", "eye"}
ZONE_ORDER = list(PALETTE.keys())
TEX_W, TEX_H, ZW, ZH, PX, PY = 128, 32, 24, 12, 30, 15
OLD = {(0, 0): "back", (30, 0): "belly", (60, 0): "foot", (90, 0): "tail",
       (0, 15): "eye", (30, 15): "ear", (60, 15): "tooth"}


def png_url(img):
    b = io.BytesIO()
    img.save(b, format="PNG")
    return "data:image/png;base64," + base64.b64encode(b.getvalue()).decode()


def build_tex():
    base = Image.new("RGBA", (TEX_W, TEX_H), (0, 0, 0, 0))
    # glow 底必须**透明**：client `FaunaEmissiveGlowLayer` 走
    # `RenderLayer.getEntityTranslucentEmissive` 整模重绘 glow 贴图，非透明像素一律全亮。
    # 不透明黑底会把整只鼠涂成"全亮黑"而不是只有蓝脊+红眼发光（build_rat_assets.py 已修同款）。
    glow = Image.new("RGBA", (TEX_W, TEX_H), (0, 0, 0, 0))
    db, dg = ImageDraw.Draw(base), ImageDraw.Draw(glow)
    origin = {}
    for i, k in enumerate(ZONE_ORDER):
        zx, zy = (i % 4) * PX, (i // 4) * PY
        db.rectangle([zx, zy, zx + ZW - 1, zy + ZH - 1], fill=PALETTE[k])
        if k in GLOW:
            dg.rectangle([zx, zy, zx + ZW - 1, zy + ZH - 1], fill=PALETTE[k])
        origin[k] = (zx, zy)
    return base, glow, origin


def main():
    fill = int(sys.argv[1]) if len(sys.argv) > 1 else 4
    out = Path(sys.argv[2]) if len(sys.argv) > 2 else Path("local_models/devour_rat_var.bbmodel")
    d = json.loads(SRC.read_text())
    els = {e["uuid"]: e for e in d["elements"]}
    root = d["outliner"][0]

    def under(n):
        acc, st = [], list(n.get("children", []))
        while st:
            x = st.pop()
            if isinstance(x, str):
                if x in els:
                    acc.append(els[x])
            elif isinstance(x, dict):
                st += x.get("children", [])
        return acc

    def bb(cs):
        xs = [v for c in cs for v in (c["from"][0], c["to"][0])]
        ys = [v for c in cs for v in (c["from"][1], c["to"][1])]
        zs = [v for c in cs for v in (c["from"][2], c["to"][2])]
        return xs, ys, zs

    # 识别 7 部位 + 原地命名/枢轴（uuid 不变→动画仍绑）
    groups = [c for c in root["children"] if isinstance(c, dict)]
    info = [(g, bb(under(g))) for g in groups]
    head = min(info, key=lambda t: min(t[1][2]))[0]
    rest = [t for t in info if t[0] is not head]
    tail = max(rest, key=lambda t: max(t[1][2]))[0]
    legs = [t for t in rest if t[0] is not tail]
    legs.sort(key=lambda t: (min(t[1][2]) + max(t[1][2])) / 2)
    front, back = legs[:2], legs[2:]

    def cx(t): return (min(t[1][0]) + max(t[1][0])) / 2
    fl = min(front, key=cx)[0]; fr = max(front, key=cx)[0]
    bl = min(back, key=cx)[0]; br = max(back, key=cx)[0]
    parts = {"body": root, "head": head, "tail": tail, "leg_fl": fl, "leg_fr": fr, "leg_bl": bl, "leg_br": br}

    def setbone(g, nm, piv):
        g["name"] = nm
        g["origin"] = [round(p, 3) for p in piv]
    bx, by, bz = bb([e for e in els.values()])
    setbone(root, "body", [0, (min(by) + max(by)) / 2, (min(bz) + max(bz)) / 2])
    hx, hy, hz = bb(under(head)); setbone(head, "head", [0, min(hy), max(hz)])
    tx, ty, tz = bb(under(tail)); setbone(tail, "tail", [0, (min(ty) + max(ty)) / 2, min(tz)])
    for nm, g in (("leg_fl", fl), ("leg_fr", fr), ("leg_bl", bl), ("leg_br", br)):
        xs, ys, zs = bb(under(g)); setbone(g, nm, [(min(xs) + max(xs)) / 2, max(ys), (min(zs) + max(zs)) / 2])

    base_tex, glow_tex, origin = build_tex()

    def paint(c, zone):
        zx, zy = origin[zone]
        c["box_uv"] = True
        c["uv_offset"] = [zx, zy]
        uv = [zx + 2, zy + 2, zx + 4, zy + 4]
        c["faces"] = {fn: {"uv": list(uv), "texture": 0} for fn in ("north", "south", "east", "west", "up", "down")}

    # 尾脊 cube（tail_glow）按 z 排序 base→tip；前 fill 个染蓝(发光)，其余黑
    ridge = sorted([e for e in d["elements"] if e.get("name") == "tail_glow"],
                   key=lambda e: (e["from"][2] + e["to"][2]) / 2)
    ridge_ids = {e["uuid"] for e in ridge}
    blue_ids = {e["uuid"] for e in ridge[:fill]}

    for c in els.values():
        if c["uuid"] in ridge_ids:
            paint(c, "tail" if c["uuid"] in blue_ids else "back")
            continue
        off = tuple(c.get("uv_offset", [])[:2]) if isinstance(c.get("uv_offset"), list) else None
        zone = OLD.get(off, "back")
        if zone == "tail":  # 尾核恒黑
            zone = "back"
        paint(c, zone)

    d["resolution"] = {"width": TEX_W, "height": TEX_H}

    def texd(nm, img, emis):
        return {"name": nm, "folder": "entity/fauna", "namespace": "bong", "id": "0" if not emis else "1",
                "width": TEX_W, "height": TEX_H, "uv_width": TEX_W, "uv_height": TEX_H, "particle": False,
                "use_as_default": not emis, "layers_enabled": False,
                "render_mode": "emissive" if emis else "default", "render_sides": "auto", "frame_time": 1,
                "frame_order_type": "loop", "frame_order": "", "frame_interpolate": False, "visible": True,
                "internal": True, "relative_path": "", "mode": "bitmap", "saved": False,
                "uuid": str(uuid.uuid4()), "source": png_url(img)}
    d["textures"] = [texd("devour_rat", base_tex, False), texd("devour_rat_glow", glow_tex, True)]
    d["name"] = "devour_rat"
    d.setdefault("meta", {})["box_uv"] = True
    # 动画保留（用户文件里已有 idle/walk/run/peck/claw/pounce，绑的是原 group uuid，命名不改 uuid）

    out.write_text(json.dumps(d, ensure_ascii=False))
    print(f"fill={fill}/4 blue ridge → {out}  (anims: {[a['name'] for a in d.get('animations',[])]})")


if __name__ == "__main__":
    main()
