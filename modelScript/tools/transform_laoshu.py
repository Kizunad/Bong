#!/usr/bin/env python3
"""把用户手绘的 laoshu.bbmodel（89 cube 大老鼠，无贴图/无骨骼名/超大）
缩到合适大小 + 区域上色 + 命名/重组骨骼 + 加 idle 动画，输出可审 bbmodel。
噬元鼠（末法残土 醒灵级 小妖兽）：脏灰褐皮 + 淡腹 + 暗爪 + 肉色尾 + 黑眼。"""
import base64
import io
import json
import math
import uuid
from pathlib import Path

from PIL import Image, ImageDraw

SRC = (Path(__file__).resolve().parents[1] / "models" / "laoshu_src.bbmodel")
OUT = (Path(__file__).resolve().parents[1] / "models" / "devour_rat_v2.bbmodel")
TARGET_LEN = 26.0  # 目标体长(Z, 单位u; 16u=1格)——比旧 devour_rat(25u)略长，仍是小妖兽

# ── 调色板（脏灰褐末法鼠；提亮+拉开背/腹对比，避免暗成一团）──────────
PALETTE = {
    "back":  (124, 104, 80, 255),  # 背/主体 褐（提亮）
    "belly": (182, 166, 138, 255), # 腹 淡土（更亮拉对比）
    "foot":  (58, 44, 34, 255),    # 爪 暗褐
    "tail":  (194, 164, 154, 255), # 尾 肉褐
    "eye":   (14, 10, 8, 255),     # 眼 近黑
    "ear":   (162, 120, 116, 255), # 耳 淡肉
    "tooth": (214, 206, 184, 255), # 牙 灰白
}
ZONE_ORDER = list(PALETTE.keys())


def png_data_url(img):
    buf = io.BytesIO()
    img.save(buf, format="PNG")
    return "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode()


# 贴图布局：7 个纯色色区，每区 >= 最大 box-uv 展开(19.5x8.5) → 24x12，4列grid，128x32 图集。
TEX_W, TEX_H = 128, 32
ZONE_W, ZONE_H = 24, 12
PITCH_X, PITCH_Y = 30, 15


def build_texture():
    """7 个纯色色区（box-uv 展开落在纯色区内 → 整 cube 单色；renderer 靠法线补 3D 明暗）。"""
    im = Image.new("RGBA", (TEX_W, TEX_H), (0, 0, 0, 0))
    dr = ImageDraw.Draw(im)
    origins = {}
    for i, key in enumerate(ZONE_ORDER):
        zx = (i % 4) * PITCH_X
        zy = (i // 4) * PITCH_Y
        dr.rectangle([zx, zy, zx + ZONE_W - 1, zy + ZONE_H - 1], fill=PALETTE[key])
        origins[key] = (zx, zy)
    return im, origins


def main():
    d = json.loads(SRC.read_text())
    els = {e["uuid"]: e for e in d["elements"]}
    root = d["outliner"][0]

    def cubes_under(node):
        acc = []
        stack = list(node.get("children", []))
        while stack:
            n = stack.pop()
            if isinstance(n, str):
                if n in els:
                    acc.append(els[n])
            elif isinstance(n, dict):
                stack += n.get("children", [])
        return acc

    parts = root["children"]  # 7 body parts (顺序: head,body,legFL,legFR,legBL,legBR,tail)
    part_cubes = [cubes_under(p) for p in parts]
    labels = ["head", "body", "leg_fl", "leg_fr", "leg_bl", "leg_br", "tail"]
    # 本脚本按 outliner **下标**硬绑部位（同批的 build_rat_assets / make_variants 用的是
    # 几何质心自动识别）。少一个组或顺序变一次就会把腿当尾、把尾当头，且全程不报错，
    # 只在最终渲染时表现为上色/枢轴错乱——先把这条顺序契约断言出来。
    if len(parts) != len(labels):
        raise SystemExit(
            f"outliner 顶层组数为 {len(parts)}，本脚本按固定顺序 {labels} 硬绑部位，"
            f"需恰好 {len(labels)} 个；请检查输入 bbmodel 的分组"
        )

    # ── 计算全局 bbox → scale + recenter ─────────────────────────
    allc = [c for pc in part_cubes for c in pc]
    xs = [v for c in allc for v in (c["from"][0], c["to"][0])]
    ys = [v for c in allc for v in (c["from"][1], c["to"][1])]
    zs = [v for c in allc for v in (c["from"][2], c["to"][2])]
    lenZ = max(zs) - min(zs)
    if lenZ <= 0:
        raise SystemExit("输入模型 Z 跨度为 0（退化的单薄片/空 cube 集），无法按体长缩放")
    S = TARGET_LEN / lenZ
    # recenter: 缩放后 脚落 Y=0, X 居中, Z 居中
    cx = (min(xs) + max(xs)) / 2
    minY = min(ys)
    cz = (min(zs) + max(zs)) / 2

    def tf(p):
        return [
            round((p[0] - cx) * S, 3),
            round((p[1] - minY) * S, 3),
            round((p[2] - cz) * S, 3),
        ]

    # ── 分类每个 cube → 色区 ─────────────────────────────────────
    def classify(cube, part_idx):
        f, t = cube["from"], cube["to"]
        cyx = (f[0] + t[0]) / 2
        cyy = (f[1] + t[1]) / 2
        czz = (f[2] + t[2]) / 2
        vol = abs((t[0] - f[0]) * (t[1] - f[1]) * (t[2] - f[2]))
        lab = labels[part_idx]
        if lab == "tail":
            return "tail"
        if lab.startswith("leg"):
            # 该腿最低的 cube = 爪
            legmin = min(cc["from"][1] for cc in part_cubes[part_idx])
            return "foot" if f[1] <= legmin + 1.5 else "back"
        if lab == "head":
            # 眼: 小体积 + 偏离中线 + 偏上
            if vol < 12 and abs(cyx) > 1.6 and cyy > 18:
                return "eye"
            # 牙: 靠前(最小Z附近) + 偏下 + 小
            if czz < -24 and cyy < 12 and vol < 20:
                return "tooth"
            # 耳: 很高(顶部) + 薄
            if cyy > 25 and (t[0] - f[0]) * (t[2] - f[2]) < 20:
                return "ear"
            return "back"
        # body: 下半 = 腹（阈值提到 0.5 让腹更明显、和背拉开对比）
        return "belly" if cyy < minY + (max(ys) - minY) * 0.5 else "back"

    tex, zone_origin = build_texture()

    # pass 1: 先用**原始坐标**分类全部 cube（避免边分类边缩放导致 legmin 混坐标系）
    zones = {}
    for pi, pc in enumerate(part_cubes):
        for c in pc:
            zones[c["uuid"]] = classify(c, pi)

    # pass 2: 缩放 + 上色。**box_uv=True**（modded_entity 原生，Blockbench 用 uv_offset 展开）：
    # uv_offset 指到该色纯色区左上，cube 展开(<=24x12)整块落在纯色区 → 单色。
    # 同时写 per-face uv（指向同色区内 2px）供 render_bbmodel.py（它只读 faces uv）。
    for pi, pc in enumerate(part_cubes):
        for c in pc:
            zx, zy = zone_origin[zones[c["uuid"]]]
            c["from"] = tf(c["from"])
            c["to"] = tf(c["to"])
            if "origin" in c:
                c["origin"] = tf(c["origin"])
            c["box_uv"] = True
            c["uv_offset"] = [zx, zy]
            uv = [zx + 2, zy + 2, zx + 4, zy + 4]  # 色区内 2px 纯色（给 headless renderer）
            c["faces"] = {fn: {"uv": list(uv), "texture": 0}
                          for fn in ("north", "south", "east", "west", "up", "down")}

    # ── 命名 + 重组骨骼: body 为根, 其余为子 ──────────────────────
    def part_bbox(pi):
        pcs = part_cubes[pi]
        xs = [v for c in pcs for v in (c["from"][0], c["to"][0])]
        ys = [v for c in pcs for v in (c["from"][1], c["to"][1])]
        zs = [v for c in pcs for v in (c["from"][2], c["to"][2])]
        return xs, ys, zs

    def child_uuids(pi):
        # 骨骼直挂的 cube uuid（保持原有子组结构）
        return parts[pi].get("children", [])

    # pivot(枢轴): cube 已在 pass2 变换过 → part_bbox 返回变换后坐标, pivot 直接用它
    def pivot_head():
        xs, ys, zs = part_bbox(0)
        return [0, round(min(ys), 3), round(max(zs), 3)]

    def pivot_body():
        xs, ys, zs = part_bbox(1)
        return [0, round((min(ys) + max(ys)) / 2, 3), round((min(zs) + max(zs)) / 2, 3)]

    def pivot_leg(pi):
        xs, ys, zs = part_bbox(pi)
        return [round((min(xs) + max(xs)) / 2, 3), round(max(ys), 3), round((min(zs) + max(zs)) / 2, 3)]

    def pivot_tail():
        xs, ys, zs = part_bbox(6)
        return [0, round((min(ys) + max(ys)) / 2, 3), round(min(zs), 3)]

    pivots = {
        "head": pivot_head(), "body": pivot_body(),
        "leg_fl": pivot_leg(2), "leg_fr": pivot_leg(3),
        "leg_bl": pivot_leg(4), "leg_br": pivot_leg(5), "tail": pivot_tail(),
    }
    bone_uuid = {name: str(uuid.uuid4()) for name in labels}

    def mk_bone(name, pi, children=None):
        return {
            "name": name, "origin": pivots[name], "uuid": bone_uuid[name],
            "color": labels.index(name) % 8, "export": True, "mirror_uv": False,
            "isOpen": True, "locked": False, "visibility": True, "autouv": 0,
            "children": (child_uuids(pi) + (children or [])),
        }

    head = mk_bone("head", 0)
    legfl = mk_bone("leg_fl", 2)
    legfr = mk_bone("leg_fr", 3)
    legbl = mk_bone("leg_bl", 4)
    legbr = mk_bone("leg_br", 5)
    tail = mk_bone("tail", 6)
    body = mk_bone("body", 1, children=[head, legfl, legfr, legbl, legbr, tail])
    d["outliner"] = [body]

    # ── idle 动画: 呼吸 bob + 尾摆 + 头微动 ────────────────────────
    def kf(channel, time, x, y, z, interp="catmullrom"):
        return {"channel": channel, "data_points": [{"x": x, "y": y, "z": z}],
                "uuid": str(uuid.uuid4()), "time": time, "color": -1,
                "interpolation": interp, "bezier_linked": True,
                "bezier_left_time": [-0.1, -0.1, -0.1], "bezier_left_value": [0, 0, 0],
                "bezier_right_time": [0.1, 0.1, 0.1], "bezier_right_value": [0, 0, 0]}

    animators = {
        bone_uuid["body"]: {"name": "body", "type": "bone", "keyframes": [
            kf("position", 0.0, 0, 0, 0), kf("position", 1.0, 0, 0.35, 0), kf("position", 2.0, 0, 0, 0),
        ]},
        bone_uuid["head"]: {"name": "head", "type": "bone", "keyframes": [
            kf("rotation", 0.0, 0, 0, 0), kf("rotation", 1.0, -3, 2, 0), kf("rotation", 2.0, 0, 0, 0),
        ]},
        bone_uuid["tail"]: {"name": "tail", "type": "bone", "keyframes": [
            kf("rotation", 0.0, 0, 8, 0), kf("rotation", 0.7, 0, -8, 0),
            kf("rotation", 1.4, 0, 8, 0), kf("rotation", 2.0, 0, 8, 0),
        ]},
    }
    d["animations"] = [{
        "uuid": str(uuid.uuid4()), "name": "idle", "loop": "loop", "override": False,
        "length": 2.0, "snapping": 24, "selected": False, "saved": True,
        "path": "", "anim_time_update": "", "blend_weight": "", "start_delay": "",
        "loop_delay": "", "animators": animators,
    }]

    # ── 贴图 + resolution ────────────────────────────────────────
    d["resolution"] = {"width": TEX_W, "height": TEX_H}
    d["textures"] = [{
        "name": "devour_rat", "folder": "entity/fauna", "namespace": "bong",
        "id": "0", "width": TEX_W, "height": TEX_H, "uv_width": TEX_W, "uv_height": TEX_H,
        "particle": False, "use_as_default": True, "layers_enabled": False,
        "render_mode": "default", "render_sides": "auto", "frame_time": 1,
        "frame_order_type": "loop", "frame_order": "", "frame_interpolate": False,
        "visible": True, "internal": True, "relative_path": "", "mode": "bitmap",
        "saved": False, "uuid": str(uuid.uuid4()), "source": png_data_url(tex),
    }]
    d["name"] = "devour_rat"
    d.setdefault("meta", {})["box_uv"] = True

    OUT.write_text(json.dumps(d, ensure_ascii=False))
    # 报告
    xs = [v for c in allc for v in (c["from"][0], c["to"][0])]
    ys = [v for c in allc for v in (c["from"][1], c["to"][1])]
    zs = [v for c in allc for v in (c["from"][2], c["to"][2])]
    print(f"scale={S:.3f}  new size = ({(max(xs)-min(xs)):.1f} x {(max(ys)-min(ys)):.1f} x {(max(zs)-min(zs)):.1f})u "
          f"= ({(max(xs)-min(xs))/16:.2f} x {(max(ys)-min(ys))/16:.2f} x {(max(zs)-min(zs))/16:.2f}) blocks")
    print(f"bones: body(root) > head,4legs,tail  | 89 cubes colored | idle anim")
    print(f"wrote {OUT}")


if __name__ == "__main__":
    main()
