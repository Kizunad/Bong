#!/usr/bin/env python3
"""在**用户几何(devour_rat_v2_user.bbmodel)**基础上：重建骨骼名+枢轴（不动 from/to/rotation）、
按位置重新上色（黑身+红眼+深蓝glow尾）、加多个动画。geometry 一律保留。"""
import base64
import io
import json
import uuid
from pathlib import Path

from PIL import Image, ImageDraw

SRC = Path("local_models/devour_rat_v2_user.bbmodel")
OUT = Path("local_models/devour_rat_v3.bbmodel")

# 黑身 + 红眼 + 深蓝glow尾
PALETTE = {
    "back":  (26, 26, 32, 255),    # 背/主体 近黑
    "belly": (46, 46, 56, 255),    # 腹 深灰（比背亮拉出形体）
    "foot":  (12, 12, 16, 255),    # 爪 黑
    "tail":  (42, 68, 240, 255),   # 尾 深蓝(glow)
    "eye":   (232, 24, 24, 255),   # 眼 红
    "ear":   (34, 30, 40, 255),    # 耳 暗
    "tooth": (176, 172, 168, 255), # 牙 灰
}
GLOW = {"tail", "eye"}  # 发光区（用于生成 emissive mask）
ZONE_ORDER = list(PALETTE.keys())
TEX_W, TEX_H = 128, 32
ZONE_W, ZONE_H = 24, 12
PITCH_X, PITCH_Y = 30, 15


def png_data_url(img):
    buf = io.BytesIO()
    img.save(buf, format="PNG")
    return "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode()


def build_textures():
    base = Image.new("RGBA", (TEX_W, TEX_H), (0, 0, 0, 0))
    glow = Image.new("RGBA", (TEX_W, TEX_H), (0, 0, 0, 255))  # emissive mask: 黑底
    db, dg = ImageDraw.Draw(base), ImageDraw.Draw(glow)
    origins = {}
    for i, key in enumerate(ZONE_ORDER):
        zx, zy = (i % 4) * PITCH_X, (i // 4) * PITCH_Y
        db.rectangle([zx, zy, zx + ZONE_W - 1, zy + ZONE_H - 1], fill=PALETTE[key])
        if key in GLOW:  # 发光区在 emissive mask 上填亮色，其余保持黑
            dg.rectangle([zx, zy, zx + ZONE_W - 1, zy + ZONE_H - 1], fill=PALETTE[key])
        origins[key] = (zx, zy)
    return base, glow, origins


def main():
    d = json.loads(SRC.read_text())
    els = {e["uuid"]: e for e in d["elements"]}
    root = d["outliner"][0]

    def cubes_under(node):
        acc, stack = [], list(node.get("children", []))
        while stack:
            n = stack.pop()
            if isinstance(n, str):
                if n in els:
                    acc.append(els[n])
            elif isinstance(n, dict):
                stack += n.get("children", [])
        return acc

    def direct_cubes(node):
        return [els[k] for k in node.get("children", []) if isinstance(k, str) and k in els]

    def bbox(cubes):
        xs = [v for c in cubes for v in (c["from"][0], c["to"][0])]
        ys = [v for c in cubes for v in (c["from"][1], c["to"][1])]
        zs = [v for c in cubes for v in (c["from"][2], c["to"][2])]
        return xs, ys, zs

    # ── 识别 7 部位：root=body(直挂 cube)，6 子组按质心分 head/legs/tail ──
    child_groups = [c for c in root["children"] if isinstance(c, dict)]
    parts = {}  # name -> group node
    parts["body"] = root
    # 按质心分类子组
    scored = []
    for g in child_groups:
        cc = cubes_under(g)
        xs, ys, zs = bbox(cc)
        scored.append((g, sum(xs) / len(xs) / 2, (min(ys) + max(ys)) / 2, (min(zs) + max(zs)) / 2, min(zs), max(zs), len(cc)))
    # head = 最靠前(min z 最小) 且质心高
    heads = sorted(scored, key=lambda s: (s[4]))  # 按 min z 升序
    head = heads[0][0]
    parts["head"] = head
    rest = [s for s in scored if s[0] is not head]
    # tail = 最靠后(max z 最大)
    rest_sorted = sorted(rest, key=lambda s: -s[5])
    tail = rest_sorted[0][0]
    parts["tail"] = tail
    legs = [s for s in rest if s[0] is not tail]
    # 4 条腿: 前(z 小)/后(z 大) × 左(x<0)/右(x>0)
    def cx(s): return s[1]
    def cz(s): return s[3]
    legs_sorted = sorted(legs, key=lambda s: cz(s))
    front = legs_sorted[:2]
    back = legs_sorted[2:]
    fl = min(front, key=lambda s: cx(s))[0]
    fr = max(front, key=lambda s: cx(s))[0]
    bl = min(back, key=lambda s: cx(s))[0]
    br = max(back, key=lambda s: cx(s))[0]
    parts.update({"leg_fl": fl, "leg_fr": fr, "leg_bl": bl, "leg_br": br})

    # ── 命名 + 枢轴（bone metadata；不碰 cube 几何）──────────────
    def set_bone(g, name, pivot):
        g["name"] = name
        g["origin"] = [round(p, 3) for p in pivot]

    bxs, bys, bzs = bbox(direct_cubes(root) or cubes_under(root))
    set_bone(root, "body", [0, (min(bys) + max(bys)) / 2, (min(bzs) + max(bzs)) / 2])
    hxs, hys, hzs = bbox(cubes_under(head))
    set_bone(head, "head", [0, min(hys), max(hzs)])  # 颈部
    txs, tys, tzs = bbox(cubes_under(tail))
    set_bone(tail, "tail", [0, (min(tys) + max(tys)) / 2, min(tzs)])  # 尾根
    for nm in ("leg_fl", "leg_fr", "leg_bl", "leg_br"):
        g = parts[nm]
        xs, ys, zs = bbox(cubes_under(g))
        set_bone(g, nm, [(min(xs) + max(xs)) / 2, max(ys), (min(zs) + max(zs)) / 2])  # 髋部
    bone_uuid = {nm: parts[nm]["uuid"] for nm in
                 ("body", "head", "tail", "leg_fl", "leg_fr", "leg_bl", "leg_br")}

    # ── 每 cube 属于哪个部位（用于分类）──────────────────────────
    cube_part = {}
    for nm in ("head", "tail", "leg_fl", "leg_fr", "leg_bl", "leg_br"):
        for c in cubes_under(parts[nm]):
            cube_part[c["uuid"]] = nm
    for c in direct_cubes(root):
        cube_part[c["uuid"]] = "body"
    for c in els.values():
        cube_part.setdefault(c["uuid"], "body")

    # 分类**沿用用户文件里已有的 UV 色区**（eye/ear/tail/belly/foot 都被 Blockbench 保留了，
    # 只有 60 个 back cube 的 offset 被自动重算成杂值 → 归 back）。避免按位置重检测把耳/脸
    # 误判成眼（缩放后 cube 都很小，体积阈值不可靠）。
    OLD_ZONES = {(0, 0): "back", (30, 0): "belly", (60, 0): "foot", (90, 0): "tail",
                 (0, 15): "eye", (30, 15): "ear", (60, 15): "tooth"}

    def classify(c):
        off = c.get("uv_offset")
        off = tuple(off) if isinstance(off, list) else None
        return OLD_ZONES.get(off, "back")

    base_tex, glow_tex, zone_origin = build_textures()

    def paint(c, zone):
        zx, zy = zone_origin[zone]
        c["box_uv"] = True
        c["uv_offset"] = [zx, zy]
        uv = [zx + 2, zy + 2, zx + 4, zy + 4]
        c["faces"] = {fn: {"uv": list(uv), "texture": 0}
                      for fn in ("north", "south", "east", "west", "up", "down")}

    # 上色。**尾巴双层**：原尾 cube → 黑核；每段顶部加一条细蓝 glow 脊(emissive) = 黑中带蓝。
    tail_cubes = []
    for c in list(els.values()):
        zone = classify(c)
        if zone == "tail":
            tail_cubes.append(c)
            zone = "back"  # 尾核变黑
        paint(c, zone)

    import copy as _copy
    ridge_uuids = []
    for c in tail_cubes:
        f, t = c["from"], c["to"]
        r = _copy.deepcopy(c)
        r["uuid"] = str(uuid.uuid4())
        r["name"] = "tail_glow"
        # 细蓝脊：跨该段顶面(local +Y)、x 内缩、贴顶略凸；沿用同 rotation+origin(跟着尾巴弯)
        r["from"] = [round(f[0] + 0.25, 3), round(t[1] - 0.12, 3), round(f[2], 3)]
        r["to"] = [round(t[0] - 0.25, 3), round(t[1] + 0.5, 3), round(t[2], 3)]
        paint(r, "tail")
        d["elements"].append(r)
        ridge_uuids.append(r["uuid"])
    parts["tail"].setdefault("children", []).extend(ridge_uuids)
    els = {e["uuid"]: e for e in d["elements"]}

    # ── 动画 ────────────────────────────────────────────────────
    def kf(channel, time, x, y, z, interp="catmullrom"):
        return {"channel": channel, "data_points": [{"x": x, "y": y, "z": z}],
                "uuid": str(uuid.uuid4()), "time": round(time, 3), "color": -1,
                "interpolation": interp}

    def animator(bone, name, keyframes):
        return {"name": name, "type": "bone", "keyframes": keyframes}

    def anim(name, length, loop, animators):
        return {"uuid": str(uuid.uuid4()), "name": name, "loop": loop, "override": False,
                "length": length, "snapping": 24, "selected": False, "saved": True,
                "path": "", "anim_time_update": "", "blend_weight": "", "start_delay": "",
                "loop_delay": "", "animators": animators}

    U = bone_uuid
    animations = []

    # idle: 呼吸 bob + 尾摆 + 头微动
    animations.append(anim("idle", 2.4, "loop", {
        U["body"]: animator(U["body"], "body", [kf("position", 0, 0, 0, 0), kf("position", 1.2, 0, 0.4, 0), kf("position", 2.4, 0, 0, 0)]),
        U["head"]: animator(U["head"], "head", [kf("rotation", 0, 0, 0, 0), kf("rotation", 1.2, -3, 2, 0), kf("rotation", 2.4, 0, 0, 0)]),
        U["tail"]: animator(U["tail"], "tail", [kf("rotation", 0, 0, 10, 0), kf("rotation", 0.8, 0, -10, 0), kf("rotation", 1.6, 0, 10, 0), kf("rotation", 2.4, 0, 10, 0)]),
    }))

    # walk: 对角步态(FL+BR / FR+BL) + 身体上下 + 尾巴反向摆
    A = 32  # 腿摆幅度
    animations.append(anim("walk", 0.8, "loop", {
        U["body"]: animator(U["body"], "body", [kf("position", 0, 0, 0, 0), kf("position", 0.2, 0, 0.5, 0), kf("position", 0.4, 0, 0, 0), kf("position", 0.6, 0, 0.5, 0), kf("position", 0.8, 0, 0, 0)]),
        U["leg_fl"]: animator(U["leg_fl"], "leg_fl", [kf("rotation", 0, A, 0, 0), kf("rotation", 0.4, -A, 0, 0), kf("rotation", 0.8, A, 0, 0)]),
        U["leg_br"]: animator(U["leg_br"], "leg_br", [kf("rotation", 0, A, 0, 0), kf("rotation", 0.4, -A, 0, 0), kf("rotation", 0.8, A, 0, 0)]),
        U["leg_fr"]: animator(U["leg_fr"], "leg_fr", [kf("rotation", 0, -A, 0, 0), kf("rotation", 0.4, A, 0, 0), kf("rotation", 0.8, -A, 0, 0)]),
        U["leg_bl"]: animator(U["leg_bl"], "leg_bl", [kf("rotation", 0, -A, 0, 0), kf("rotation", 0.4, A, 0, 0), kf("rotation", 0.8, -A, 0, 0)]),
        U["tail"]: animator(U["tail"], "tail", [kf("rotation", 0, 0, 14, 0), kf("rotation", 0.4, 0, -14, 0), kf("rotation", 0.8, 0, 14, 0)]),
    }))

    # run: 更快更大幅 + 前后腿聚拢(gallop 感)
    R = 46
    animations.append(anim("run", 0.44, "loop", {
        U["body"]: animator(U["body"], "body", [kf("position", 0, 0, 0, 0), kf("position", 0.11, 0, 1.1, 0), kf("position", 0.22, 0, 0, 0), kf("position", 0.33, 0, 0.9, 0), kf("position", 0.44, 0, 0, 0),
                                                kf("rotation", 0, 6, 0, 0), kf("rotation", 0.22, -6, 0, 0), kf("rotation", 0.44, 6, 0, 0)]),
        U["leg_fl"]: animator(U["leg_fl"], "leg_fl", [kf("rotation", 0, R, 0, 0), kf("rotation", 0.22, -R, 0, 0), kf("rotation", 0.44, R, 0, 0)]),
        U["leg_fr"]: animator(U["leg_fr"], "leg_fr", [kf("rotation", 0, R, 0, 0), kf("rotation", 0.22, -R, 0, 0), kf("rotation", 0.44, R, 0, 0)]),
        U["leg_bl"]: animator(U["leg_bl"], "leg_bl", [kf("rotation", 0, -R, 0, 0), kf("rotation", 0.22, R, 0, 0), kf("rotation", 0.44, -R, 0, 0)]),
        U["leg_br"]: animator(U["leg_br"], "leg_br", [kf("rotation", 0, -R, 0, 0), kf("rotation", 0.22, R, 0, 0), kf("rotation", 0.44, -R, 0, 0)]),
        U["tail"]: animator(U["tail"], "tail", [kf("rotation", 0, -18, 0, 0), kf("rotation", 0.44, -18, 0, 0)]),
    }))

    # 约定(已 pose 验证): 头咬下=head -X, 头抬(蓄)=+X; 前进=-Z; 腿前扫=+X, 后勾(蓄)=-X;
    # 前扑抬身=body +X（绝不用 -X 那是屁股翘）。三招都: 蓄力(慢,ease-in)→爆发(快,大幅)→收势。

    # ① 啄咬 peck —— 头主导快啄: 抬头蓄 → 猛地下啄 → 咬住顿 → 收
    animations.append(anim("peck", 0.5, "once", {
        U["head"]: animator(U["head"], "head", [kf("rotation", 0, 0, 0, 0), kf("rotation", 0.12, 24, 0, 0), kf("rotation", 0.24, -34, 0, 0), kf("rotation", 0.3, -34, 0, 0), kf("rotation", 0.5, 0, 0, 0)]),
        U["body"]: animator(U["body"], "body", [kf("position", 0, 0, 0, 0), kf("position", 0.12, 0, 0.3, 0.4), kf("position", 0.24, 0, -0.2, -0.8), kf("position", 0.5, 0, 0, 0)]),
        U["tail"]: animator(U["tail"], "tail", [kf("rotation", 0, 0, 0, 0), kf("rotation", 0.12, -14, 0, 0), kf("rotation", 0.5, 0, 0, 0)]),
    }))

    # ② 抓 claw —— 前起身(蓄) → 右前爪由后上猛扒到前下(发力) → 收
    animations.append(anim("claw", 0.6, "once", {
        U["body"]: animator(U["body"], "body", [kf("rotation", 0, 0, 0, 0), kf("rotation", 0.18, 16, 0, 0), kf("rotation", 0.34, 3, 0, 0), kf("rotation", 0.6, 0, 0, 0),
                                                kf("position", 0, 0, 0, 0), kf("position", 0.18, 0, 0.8, 0), kf("position", 0.34, 0, 0, -0.6), kf("position", 0.6, 0, 0, 0)]),
        U["leg_fr"]: animator(U["leg_fr"], "leg_fr", [kf("rotation", 0, 0, 0, 0), kf("rotation", 0.18, -50, 0, 0), kf("rotation", 0.32, 56, 0, 0), kf("rotation", 0.44, 10, 0, 0), kf("rotation", 0.6, 0, 0, 0)]),
        U["leg_fl"]: animator(U["leg_fl"], "leg_fl", [kf("rotation", 0, 0, 0, 0), kf("rotation", 0.18, -18, 0, 0), kf("rotation", 0.34, 14, 0, 0), kf("rotation", 0.6, 0, 0, 0)]),
        U["head"]: animator(U["head"], "head", [kf("rotation", 0, 0, 0, 0), kf("rotation", 0.18, 10, -8, 0), kf("rotation", 0.34, -14, 6, 0), kf("rotation", 0.6, 0, 0, 0)]),
    }))

    # ③ 扑 pounce —— 蹲伏聚力(慢) → 后腿蹬地前上爆冲(快) → 空中前爪伸+空咬 → 落地收
    animations.append(anim("pounce", 0.78, "once", {
        U["body"]: animator(U["body"], "body", [kf("position", 0, 0, 0, 0), kf("position", 0.22, 0, -1.8, 2.6), kf("position", 0.4, 0, 4.2, -6.5), kf("position", 0.52, 0, 1.2, -3), kf("position", 0.78, 0, 0, 0),
                                                kf("rotation", 0, 0, 0, 0), kf("rotation", 0.22, 9, 0, 0), kf("rotation", 0.4, -9, 0, 0), kf("rotation", 0.78, 0, 0, 0)]),
        U["leg_bl"]: animator(U["leg_bl"], "leg_bl", [kf("rotation", 0, 0, 0, 0), kf("rotation", 0.22, 32, 0, 0), kf("rotation", 0.4, -44, 0, 0), kf("rotation", 0.78, 0, 0, 0)]),
        U["leg_br"]: animator(U["leg_br"], "leg_br", [kf("rotation", 0, 0, 0, 0), kf("rotation", 0.22, 32, 0, 0), kf("rotation", 0.4, -44, 0, 0), kf("rotation", 0.78, 0, 0, 0)]),
        U["leg_fl"]: animator(U["leg_fl"], "leg_fl", [kf("rotation", 0, 0, 0, 0), kf("rotation", 0.22, -14, 0, 0), kf("rotation", 0.4, 44, 0, 0), kf("rotation", 0.52, 22, 0, 0), kf("rotation", 0.78, 0, 0, 0)]),
        U["leg_fr"]: animator(U["leg_fr"], "leg_fr", [kf("rotation", 0, 0, 0, 0), kf("rotation", 0.22, -14, 0, 0), kf("rotation", 0.4, 44, 0, 0), kf("rotation", 0.52, 22, 0, 0), kf("rotation", 0.78, 0, 0, 0)]),
        U["head"]: animator(U["head"], "head", [kf("rotation", 0, 0, 0, 0), kf("rotation", 0.22, -14, 0, 0), kf("rotation", 0.4, -2, 0, 0), kf("rotation", 0.48, -24, 0, 0), kf("rotation", 0.78, 0, 0, 0)]),
        U["tail"]: animator(U["tail"], "tail", [kf("rotation", 0, 0, 0, 0), kf("rotation", 0.22, 20, 0, 0), kf("rotation", 0.4, -22, 0, 0), kf("rotation", 0.78, 0, 0, 0)]),
    }))

    d["animations"] = animations

    # ── 贴图: 主贴图 + emissive glow mask ────────────────────────
    d["resolution"] = {"width": TEX_W, "height": TEX_H}

    def tex_dict(name, img, folder):
        return {"name": name, "folder": folder, "namespace": "bong", "id": "0" if name == "devour_rat" else "1",
                "width": TEX_W, "height": TEX_H, "uv_width": TEX_W, "uv_height": TEX_H,
                "particle": False, "use_as_default": name == "devour_rat", "layers_enabled": False,
                "render_mode": "emissive" if name.endswith("glow") else "default",
                "render_sides": "auto", "frame_time": 1, "frame_order_type": "loop", "frame_order": "",
                "frame_interpolate": False, "visible": True, "internal": True, "relative_path": "",
                "mode": "bitmap", "saved": False, "uuid": str(uuid.uuid4()), "source": png_data_url(img)}

    d["textures"] = [
        tex_dict("devour_rat", base_tex, "entity/fauna"),
        tex_dict("devour_rat_glow", glow_tex, "entity/fauna"),
    ]
    d["name"] = "devour_rat"
    d.setdefault("meta", {})["box_uv"] = True

    OUT.write_text(json.dumps(d, ensure_ascii=False))
    print(f"bones: body(root) > head,4legs,tail  | recolored 黑身+红眼+深蓝glow尾")
    print(f"animations: {[a['name'] for a in animations]}")
    print(f"emissive mask: 尾+眼 亮 (devour_rat_glow, render_mode=emissive)")
    print(f"wrote {OUT}")


if __name__ == "__main__":
    main()
