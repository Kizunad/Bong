#!/usr/bin/env python3
"""P0: 用户 bbmodel → GeckoLib 资产。产出:
- geo/devour_rat.geo.json (共享几何; 尾脊 4 cube 各占独立 UV cell 以便贴图控制填充)
- textures/entity/fauna/devour_rat_q{0,1,2}.png (底图: 尾脊蓝填充 0/半/全)
- textures/entity/fauna/devour_rat_q{0,1,2}_glow.png (emissive: 蓝脊+红眼发光)
- animations/devour_rat.animation.json (idle/walk/run/peck/claw/pounce)
几何/动画取自用户最新 bbmodel; 骨骼原地命名(uuid不变)。"""
import base64
import io
import json
import sys
import uuid
from pathlib import Path

from PIL import Image, ImageDraw

SRC = Path("local_models/devour_rat_v3_user2.bbmodel")
CLIENT = Path("client/src/main/resources/assets/bong")
GEO = CLIENT / "geo/devour_rat.geo.json"
TEXDIR = CLIENT / "textures/entity/fauna"
ANIM = CLIENT / "animations/devour_rat.animation.json"

# 色板 (黑身/红眼/深蓝脊)
COL = {"back": (26, 26, 32), "belly": (46, 46, 56), "foot": (12, 12, 16),
       "eye": (232, 24, 24), "ear": (34, 30, 40), "tooth": (176, 172, 168),
       "blue": (42, 68, 240), "blk": (20, 20, 26)}
# 图集 128x48, cell 32x16 **填满无缝**（max box-uv 展开 19.5x8.5，留 12/7px 边距防 MC 过滤溢色）
TW, TH, CW, CH = 128, 48, 32, 16
# 固定 cell 坐标（4 列 × 3 行，无缝紧贴）
CELL = {"back": (0, 0), "belly": (32, 0), "foot": (64, 0), "eye": (96, 0),
        "ear": (0, 16), "tooth": (32, 16), "ridge0": (64, 16), "ridge1": (96, 16),
        "ridge2": (0, 32), "ridge3": (32, 32)}
OLD = {(0, 0): "back", (30, 0): "belly", (60, 0): "foot", (90, 0): "tail",
       (0, 15): "eye", (30, 15): "ear", (60, 15): "tooth"}


def bb(cs):
    xs = [v for c in cs for v in (c["from"][0], c["to"][0])]
    ys = [v for c in cs for v in (c["from"][1], c["to"][1])]
    zs = [v for c in cs for v in (c["from"][2], c["to"][2])]
    return xs, ys, zs


def main():
    # 输入 bbmodel 走 gitignored 的 local_models/（用户在 Blockbench 手雕后放这里）。
    # 缺文件时裸 FileNotFoundError 读不出"该把文件放到哪"，故给明确提示。
    # 允许用 CLI 参数覆盖，保留原路径为默认。
    src = Path(sys.argv[1]) if len(sys.argv) > 1 else SRC
    if not src.is_file():
        raise SystemExit(
            f"找不到输入 bbmodel：{src}\n"
            f"用法：python3 {Path(__file__).name} [输入.bbmodel]（默认 {SRC}）\n"
            f"该目录是 gitignored 的本地工作区——请先把 Blockbench 手雕的模型放进去。"
        )
    d = json.loads(src.read_text())
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

    # 识别 7 部位 + 命名/枢轴 (原地)
    groups = [c for c in root["children"] if isinstance(c, dict)]
    info = [(g, bb(under(g))) for g in groups]
    head = min(info, key=lambda t: min(t[1][2]))[0]
    rest = [t for t in info if t[0] is not head]
    tail = max(rest, key=lambda t: max(t[1][2]))[0]
    legs = sorted([t for t in rest if t[0] is not tail], key=lambda t: (min(t[1][2]) + max(t[1][2])) / 2)
    front, back = legs[:2], legs[2:]

    def cx(t): return (min(t[1][0]) + max(t[1][0])) / 2
    fl, fr = min(front, key=cx)[0], max(front, key=cx)[0]
    bl, br = min(back, key=cx)[0], max(back, key=cx)[0]

    def setname(g, nm, piv):
        g["name"] = nm
        g["origin"] = [round(p, 3) for p in piv]
    _, gy, gz = bb(list(els.values()))
    setname(root, "body", [0, (min(gy) + max(gy)) / 2, (min(gz) + max(gz)) / 2])
    hx, hy, hz = bb(under(head)); setname(head, "head", [0, min(hy), max(hz)])
    tx, ty, tz = bb(under(tail)); setname(tail, "tail", [0, (min(ty) + max(ty)) / 2, min(tz)])
    for nm, g in (("leg_fl", fl), ("leg_fr", fr), ("leg_bl", bl), ("leg_br", br)):
        xs, ys, zs = bb(under(g)); setname(g, nm, [(min(xs) + max(xs)) / 2, max(ys), (min(zs) + max(zs)) / 2])

    # 每 cube 的 UV cell: 尾脊 4 段各独立 cell; 其余按色区; 尾核黑
    ridge = sorted([e for e in d["elements"] if e.get("name") == "tail_glow"],
                   key=lambda e: (e["from"][2] + e["to"][2]) / 2)
    ridge_cell = {r["uuid"]: f"ridge{i}" for i, r in enumerate(ridge)}
    cube_cell = {}
    for c in els.values():
        if c["uuid"] in ridge_cell:
            cube_cell[c["uuid"]] = ridge_cell[c["uuid"]]
        else:
            off = tuple(c.get("uv_offset", [])[:2]) if isinstance(c.get("uv_offset"), list) else None
            z = OLD.get(off, "back")
            cube_cell[c["uuid"]] = "back" if z == "tail" else z

    # ── geo.json: 只建 7 个命名骨骼; 每骨收集其下所有 cube(跳过命名子骨骼) ──
    NAMED = {"body", "head", "tail", "leg_fl", "leg_fr", "leg_bl", "leg_br"}

    def cubes_of_bone(g):
        acc = []
        for ch in g.get("children", []):
            if isinstance(ch, str):
                if ch in els:
                    acc.append(els[ch])
            elif isinstance(ch, dict) and ch.get("name") not in NAMED:
                acc += cubes_of_bone(ch)  # 未命名子组: 递归收集; 命名子骨骼: 跳过(单独成骨)
        return acc

    def geo_cube(c):
        f, t = c["from"], c["to"]
        zx, zy = CELL[cube_cell[c["uuid"]]]
        # from/to 归一化：Blockbench 允许某轴 to < from（用户手雕时很容易出现），直接
        # `to - from` 会得到**负 size**。Bedrock geometry 约定 size 非负，负宽度会让该 cube
        # 退化/翻面（背面剔除后从外面看不见）。取 min 作 origin、取绝对值作 size —— 得到的是
        # 几何上完全相同的盒子，只是端点顺序摆正。
        # （实测命中：用户 bbmodel 的尾脊末段 ridge3 是 to.x < from.x，size.x = -0.186，
        #  而它正是 q2 满档才点亮的第 4 段蓝脊；不修则"吃饱"态的最后一段可能整段看不见。）
        lo = [min(f[i], t[i]) for i in range(3)]
        size = [abs(t[i] - f[i]) for i in range(3)]
        gc = {"origin": [round(v, 3) for v in lo],
              "size": [round(v, 3) for v in size], "uv": [zx + 3, zy + 3]}
        if any(c.get("rotation", [0, 0, 0])):
            gc["rotation"] = c["rotation"]
            gc["pivot"] = c.get("origin", [0, 0, 0])
        return gc

    def mk_bone(g, name, parent):
        b = {"name": name, "pivot": g.get("origin", [0, 0, 0])}
        if parent:
            b["parent"] = parent
        cubes = [geo_cube(c) for c in cubes_of_bone(g)]
        if cubes:
            b["cubes"] = cubes
        return b

    bones = [
        mk_bone(root, "body", None),
        mk_bone(head, "head", "body"),
        mk_bone(tail, "tail", "body"),
        mk_bone(fl, "leg_fl", "body"), mk_bone(fr, "leg_fr", "body"),
        mk_bone(bl, "leg_bl", "body"), mk_bone(br, "leg_br", "body"),
    ]
    geo = {"format_version": "1.12.0", "minecraft:geometry": [{
        "description": {"identifier": "geometry.bong.devour_rat",
                        "texture_width": TW, "texture_height": TH,
                        "visible_bounds_width": 3, "visible_bounds_height": 2,
                        "visible_bounds_offset": [0, 0.6, 0]},
        "bones": bones}]}
    GEO.write_text(json.dumps(geo, ensure_ascii=False, indent=1))

    # ── 3 组贴图 (底图 + emissive), 尾脊蓝填充 0/2/4 ──
    # glow 底**必须透明**：client `FaunaEmissiveGlowLayer` 走
    # `RenderLayer.getEntityTranslucentEmissive` 整模重绘一遍 glow 贴图，非透明像素一律
    # 全亮。若 glow 底是不透明黑（历史 bug），整只鼠会被涂成"全亮黑"而不是只有蓝脊+红眼发光。
    def make_tex(fill):
        base = Image.new("RGBA", (TW, TH), (0, 0, 0, 0))
        glow = Image.new("RGBA", (TW, TH), (0, 0, 0, 0))
        db, dg = ImageDraw.Draw(base), ImageDraw.Draw(glow)
        for name, (zx, zy) in CELL.items():
            if name.startswith("ridge"):
                idx = int(name[-1])
                blue = idx < fill
                c = COL["blue"] if blue else COL["blk"]
                db.rectangle([zx, zy, zx + CW - 1, zy + CH - 1], fill=c + (255,))
                if blue:
                    dg.rectangle([zx, zy, zx + CW - 1, zy + CH - 1], fill=COL["blue"] + (255,))
            else:
                db.rectangle([zx, zy, zx + CW - 1, zy + CH - 1], fill=COL[name] + (255,))
                if name == "eye":
                    dg.rectangle([zx, zy, zx + CW - 1, zy + CH - 1], fill=COL["eye"] + (255,))
        return base, glow

    TEXDIR.mkdir(parents=True, exist_ok=True)
    for q, fill in ((0, 0), (1, 2), (2, 4)):
        base, glow = make_tex(fill)
        base.save(TEXDIR / f"devour_rat_q{q}.png")
        glow.save(TEXDIR / f"devour_rat_q{q}_glow.png")
        if q == 0:
            # `FaunaVisualKind.DEVOUR_RAT.textureId()` 的物种缺省贴图 = q0 别名。
            # 正常路径 `FaunaModel.selectTexture` 恒返回 q0/q1/q2，这两张只在缺省兜底
            # 路径被取到；缺了会渲染 missing texture（紫黑格），故一并产出。
            base.save(TEXDIR / "devour_rat.png")
            glow.save(TEXDIR / "devour_rat_glow.png")

    # ── animations → geckolib .animation.json ──
    anims = {}
    for a in d.get("animations", []):
        bones = {}
        for av in a["animators"].values():
            nm = av["name"]
            chans = {}
            for kf in av["keyframes"]:
                ch = "position" if kf["channel"] == "position" else "rotation"
                dp = kf["data_points"][0]
                chans.setdefault(ch, {})[str(round(kf["time"], 4))] = [dp.get("x", 0), dp.get("y", 0), dp.get("z", 0)]
            bones[nm] = chans
        entry = {"animation_length": round(a["length"], 4), "bones": bones}
        if a.get("loop") == "loop":
            entry["loop"] = True
        anims[f"animation.bong.devour_rat.{a['name']}"] = entry
    ANIM.parent.mkdir(parents=True, exist_ok=True)
    ANIM.write_text(json.dumps({"format_version": "1.8.0", "animations": anims}, ensure_ascii=False, indent=1))

    nbones = len(geo["minecraft:geometry"][0]["bones"])
    ncubes = sum(len(b.get("cubes", [])) for b in geo["minecraft:geometry"][0]["bones"])
    print(f"geo: {nbones} bones / {ncubes} cubes → {GEO}")
    print(f"textures: devour_rat_q0/1/2(.png +_glow) → {TEXDIR}")
    print(f"anims: {list(anims.keys())}")
    print(f"        → {ANIM}")


if __name__ == "__main__":
    main()
