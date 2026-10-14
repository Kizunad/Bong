"""末土装备（背包 / 斗篷 / 腰带等穿戴件）Blockbench .bbmodel 生成器的共用工具。

每件装备的生成器只写自己的材质、骨骼树和 part_*() 分部件函数，再交给 ``main(spec)``：
它负责 --part 单件预览、--self-test 差分自证、--gates 门禁、全量出模型 + check.png。

坐标约定（与 docs 里的穿戴约定、grass_pouch_back.geo.json 一致）：

* 1 方块 = 16px，脚底 y=0，+y 向上；玩家面朝 -z，**+z 是背面**；
* 玩家躯干 x∈[-4,4]、y∈[12,24]、z∈[-2,2]；头 y∈[24,32]；双臂 x=±4..±8、y∈[12,24]；腿 y∈[0,12]；
* 本工具里 **+x = 穿戴者的右侧**（和游商傀儡等 GeckoLib 生物的 *_r 一致）；转 Bedrock geo 时 x 取反；
* 装备直接按「穿在玩家身上」的坐标建模，不再另做平移。

穿戴件特有的门禁 ``clearance``：任何立方体不得扎进玩家的躯干 / 头 / 双臂 / 双腿（只许贴面，咬入 ≤0.05px）。
这是「不能穿进玩家身体」的可执行版本，旁边带缺陷注入器，``--self-test`` 时必须能被抓到。
"""

from __future__ import annotations

import argparse
import copy
import importlib.util
import json
import math
import os
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Callable

from PIL import Image, ImageDraw

from bbmodel_maker.gates import gatekit
from bbmodel_maker.render.render_bbmodel import render
from bbmodel_maker.rig.rigkit import Rig, element_bounds

REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO / "modelScript" / "tools"))
from cjk_font import load_font  # noqa: E402

# 仓库内的共面 (Z-fighting) 门禁：modelScript/bbmodel_maker 与已安装的同名库同名，按路径单独加载
_spec = importlib.util.spec_from_file_location(
    "repo_coplanar", REPO / "modelScript" / "bbmodel_maker" / "gates" / "coplanar.py")
coplanar = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(coplanar)

REVIEW_ROOT = Path(os.environ.get(
    "BONG_MODEL_REVIEW_DIR", "/home/serverkizuna/Code/Bong/.agent-worktrees/model-review"))

# ───────────────────────────────────────────────────────────── 标准玩家（灰色占位人体）
Vec = tuple[float, float, float]
PLAYER_BOXES: dict[str, tuple[Vec, Vec]] = {
    "头": ((-4.0, 24.0, -4.0), (4.0, 32.0, 4.0)),
    "躯干": ((-4.0, 12.0, -2.0), (4.0, 24.0, 2.0)),
    "右臂": ((4.0, 12.0, -2.0), (8.0, 24.0, 2.0)),
    "左臂": ((-8.0, 12.0, -2.0), (-4.0, 24.0, 2.0)),
    "右腿": ((0.0, 0.0, -2.0), (4.0, 12.0, 2.0)),
    "左腿": ((-4.0, 0.0, -2.0), (0.0, 12.0, 2.0)),
}
CLEARANCE_TOL = 0.05      # 贴面允许的咬入深度；再深就是穿进身体
PLAYER_MATS = {
    "wearer_body": (150, 150, 152),
    "wearer_limb": (122, 122, 126),
    "wearer_face": (70, 70, 76),
}
_INSET = 0.03             # 渲染时占位人体各面内缩，免得与贴面的装备共面闪烁

BG = (119, 119, 119)


# ───────────────────────────────────────────────────────────── 建模辅助
class Gear:
    """Rig 的薄封装：按骨骼树建件，左右镜像成对，并带每根骨的说明供骨骼树面板用。"""

    def __init__(self, mats: dict, bones: list[tuple[str, str | None, Vec, str]]) -> None:
        self.rig = Rig(mats, tex=64, swatch=8)
        self.bones = bones
        for name, parent, pivot, _ in bones:
            self.rig.bone(name, pivot, parent)

    def box(self, bone, name, f, t, mat, rot=None, org=None):
        return self.rig.cube(bone, name, f, t, mat=mat, rot=rot, org=org)

    def pair(self, bone, name, f, t, mat, rot=None, org=None, sided=False):
        """f/t 按右侧（+x）坐标给，左侧自动镜像（x 取反，旋转的 y/z 分量取反）。

        sided=True：右件挂 bone_r、左件挂 bone_l（成对的左右骨）；False：两件都挂在 bone。
        """
        bone_r = f"{bone}_r" if sided else bone
        bone_l = f"{bone}_l" if sided else bone
        self.rig.cube(bone_r, f"{name}_r", f, t, mat=mat, rot=rot, org=org)
        fl = (-t[0], f[1], f[2])
        tl = (-f[0], t[1], t[2])
        rl = None if rot is None else (rot[0], -rot[1], -rot[2])
        ol = None if org is None else (-org[0], org[1], org[2])
        self.rig.cube(bone_l, f"{name}_l", fl, tl, mat=mat, rot=rl, org=ol)

    def rod(self, bone, name, a, b, r, mat, rz=None):
        """端点精确落在 a、b 上的斜杆（骨条 / 铁条 / 斜挎带）。r 为半宽。"""
        self.rig.shaft(bone, name, a, b, r, rz, mat=mat)

    def ring(self, bone, name, c, r, w, t, mat, axis="z", n=8):
        """一圈环（n 段短杆首尾相接）。c 为圆心；r 为中心线半径；w 为环面宽；t 为环厚。

        axis="z"：环在 xy 平面里，厚度沿 z（竖在背上的圈）；axis="x"：环在 yz 平面里，厚度沿 x（侧挂的绳圈）。
        """
        pts = []
        for k in range(n):
            a = 2 * math.pi * k / n
            if axis == "z":
                pts.append((c[0] + r * math.sin(a), c[1] + r * math.cos(a), c[2]))
            else:
                pts.append((c[0], c[1] + r * math.cos(a), c[2] + r * math.sin(a)))
        for k in range(n):
            a, b = pts[k], pts[(k + 1) % n]
            self.rod(bone, f"{name}_{k}", a, b, w / 2, mat, rz=t / 2)

    def total(self) -> int:
        return len(self.rig.elements)


# ───────────────────────────────────────────────────────────── 门禁
def gate_clearance(rig: Rig, tol: float = CLEARANCE_TOL) -> list[str]:
    """装备立方体扎进玩家身体（躯干 / 头 / 双臂 / 双腿）的深度超过 tol。"""
    bad = []
    for el in rig.elements:
        lo, hi = element_bounds([el])
        for label, (klo, khi) in PLAYER_BOXES.items():
            depth = min(min(hi[k], khi[k]) - max(lo[k], klo[k]) for k in range(3))
            if depth > tol:
                bad.append(f"{el['name']} 扎进{label} {depth:.2f}px")
                break
    return bad


def inject_clearance(rig: Rig, **_):
    """把一个无旋转件搬进躯干正中，必须被 clearance 抓到。"""
    r = copy.deepcopy(rig)
    for el in r.elements:
        if any(el["rotation"]):
            continue
        size = [t - f for f, t in zip(el["from"], el["to"])]
        centre = (0.0, 18.0, 0.0)
        el["from"] = [round(c - s / 2, 3) for c, s in zip(centre, size)]
        el["to"] = [round(c + s / 2, 3) for c, s in zip(centre, size)]
        el["origin"] = list(centre)
        return r, el["name"], f"把 {el['name']} 搬进躯干正中"
    raise gatekit.InjectionImpossible("没有无旋转件可搬")


class WornGates(gatekit.AssetGates):
    """穿戴件门禁：去掉「越出 0..16 方块空间」（穿戴件不受方块限制），加上 clearance。"""

    def specs(self):
        base = tuple(s for s in super().specs() if s[0] != "overflow")
        return base + (("clearance", "穿进玩家身体",
                        lambda r: gate_clearance(r), inject_clearance),)


def soft_pairs(soft: tuple[str, ...], over: tuple[str, ...]) -> frozenset:
    """软件（绳 / 布 / 皮）允许薄压在哪些材质上：返回 gatekit 要的无序材质对集合。"""
    return frozenset(frozenset((s, o)) for s in soft for o in over if s != o)


# ───────────────────────────────────────────────────────────── 单件的规格
@dataclass
class GearSpec:
    gear_id: str                     # gu_jia_bei_lou
    model_name: str                  # GuJiaBeiLou
    title: str                       # 骨架背篓 / gu_jia_bei_lou
    mats: dict
    bones: list
    parts: dict[str, Callable]       # {"frame": part_frame, ...}，按顺序拼
    out_dir: Path                    # 生成器所在目录（bbmodel 落这里）
    asym: frozenset = frozenset()
    soft_over: frozenset = frozenset()
    free_floating: frozenset = frozenset()
    min_bones: int = 6
    wear_note: str = ""              # 写进 check.png 标题的挂点说明
    # 穿戴渲染的固定取景 ((cx, cy, cz), 跨度 px)；小件（腰带）整个人放进画面太小，可指定只看腰部
    worn_focus: tuple | None = None
    gates: WornGates = field(init=False)

    def __post_init__(self) -> None:
        self.gates = WornGates(self.title, self.mats, asym=self.asym,
                               soft_over=self.soft_over, free_floating=self.free_floating)

    @property
    def review_dir(self) -> Path:
        return REVIEW_ROOT / "img" / "gear" / self.gear_id

    def build(self, parts: list[Callable] | None = None) -> Gear:
        g = Gear(self.mats, self.bones)
        for fn in parts if parts is not None else self.parts.values():
            fn(g)
        return g


def gate_rig(spec: GearSpec, g: Gear) -> Rig:
    """门禁用副本：rigkit 把 color 写成「材质序号 % 8」，材质多于 8 种会撞号，这里改回完整序号。"""
    rig = copy.deepcopy(g.rig)
    per_row = rig.tex // rig.swatch
    for el in rig.elements:
        uv = el["faces"]["north"]["uv"]
        el["color"] = int(uv[1] // rig.swatch) * per_row + int(uv[0] // rig.swatch)
    spec.gates.colors = {i: n for i, n in enumerate(spec.mats)}
    return rig


def moving_bone_count(g: Gear) -> int:
    return sum(1 for b in g.rig.bones.values() if b["children"])


def coplanar_violations(g: Gear) -> list[str]:
    flat = [e for e in g.rig.elements if not any(e["rotation"])]
    return coplanar.check_coplanar_faces(flat)


def run_gates(spec: GearSpec, g: Gear) -> int:
    rig = gate_rig(spec, g)
    total = spec.gates.report(rig, px=16.0, note="（穿戴件不受 16px 方块限制，已去掉越界一道，加上「穿进玩家身体」）")
    cop = coplanar_violations(g)
    print(f"  {'✓' if not cop else '✗'} 共面 (Z-fighting): {len(cop)}")
    shown: set[str] = set()
    for line in cop:
        # 左右镜像的两处是同一个问题，只列一次（_l / _r 后缀抹掉后去重）
        key = line.replace("_l ", "_r ").replace("_l_", "_r_")
        if key not in shown and len(shown) < 40:
            shown.add(key)
            print(f"      - {line}")
    moving = moving_bone_count(g)
    ok = moving >= spec.min_bones
    print(f"  {'✓' if ok else '✗'} 骨骼数: 共 {len(g.rig.bones)}（含 root），带几何的可动骨 {moving}（下限 {spec.min_bones}）")
    return total + len(cop) + (0 if ok else 1)


def self_test(spec: GearSpec, g: Gear) -> None:
    """差分自证：干净模型须全绿；每道门注入缺陷后须抓到。"""
    rig = gate_rig(spec, g)
    broken = spec.gates.self_test(rig)
    clean = coplanar_violations(g)
    victim = next(e for e in g.rig.elements if not any(e["rotation"]))
    twin = copy.deepcopy(victim)
    twin["name"] = victim["name"] + "__twin"
    hits = coplanar.check_coplanar_faces([e for e in g.rig.elements if not any(e["rotation"])] + [twin])
    if clean or not hits:
        broken += 1
        print(f"  ✗ 共面: 干净 {len(clean)} / 注入后 {len(hits)}（须 0 / >0）")
    else:
        print(f"  ✓ 共面: 干净 0 → 注入同位复制件 {victim['name']} 后 {len(hits)}")
    first = next(iter(spec.parts.values()))
    small = spec.build([first])
    if len(spec.parts) > 1 and moving_bone_count(small) >= moving_bone_count(g):
        broken += 1
        print("  ✗ 骨骼数门没有鉴别力：只含首件的预览模型骨数不比完整模型少")
    elif moving_bone_count(g) < spec.min_bones or moving_bone_count(small) >= spec.min_bones:
        # 下限本身须能把残缺件拦下，否则这道门形同虚设
        broken += 1
        print(f"  ✗ 骨骼数: 完整 {moving_bone_count(g)}、首件 {moving_bone_count(small)}，下限 {spec.min_bones} 拦不住残缺件")
    else:
        print(f"  ✓ 骨骼数: 完整 {moving_bone_count(g)} ≥ {spec.min_bones}；残缺件 {moving_bone_count(small)} 被拦下")
    if broken:
        raise SystemExit(f"自证未通过：{broken} 道门失效")
    print("✓ 差分自证全绿")


# ───────────────────────────────────────────────────────────── 穿在灰色占位人体上
def worn_doc(g: Gear, name: str) -> dict:
    """装备 + 灰色占位玩家合成一个 bbmodel（只用于渲染看比例，不是交付物）。"""
    mats = {**g.rig.mats, **PLAYER_MATS}
    w = Rig(mats, tex=g.rig.tex, swatch=g.rig.swatch)
    for bone in g.rig.bone_order:
        w.bones[bone] = copy.deepcopy(g.rig.bones[bone])
        w.bone_order.append(bone)
    w.elements = copy.deepcopy(g.rig.elements)
    w.bone("wearer", (0.0, 0.0, 0.0))
    for label, (lo, hi) in PLAYER_BOXES.items():
        mat = "wearer_body" if label in ("头", "躯干") else "wearer_limb"
        f = tuple(v + _INSET for v in lo)
        t = tuple(v - _INSET for v in hi)
        w.cube("wearer", f"player_{label}", f, t, mat=mat)
    # 脸：头前面（-z）两只眼睛，让正面一眼可辨
    for sx in (-2.0, 1.0):
        w.cube("wearer", "player_eye", (sx, 27.0, -4.0 - 0.1), (sx + 1.0, 28.0, -4.0 + _INSET), mat="wearer_face")
    return w.bbmodel(name)


WORN_VIEWS = (
    ("FRONT 正面（相机在 -z）", 180.0, 0.0),
    ("RIGHT 右侧（相机在 +x）", -90.0, 0.0),
    ("BACK 背面（相机在 +z）", 0.0, 0.0),
    ("3/4 背右斜（相机在 +z / +x 上方）", 28.0, 14.0),
)


def _focus(doc_path: Path) -> tuple[tuple[float, float, float], float]:
    doc = json.loads(doc_path.read_text(encoding="utf-8"))
    lo, hi = element_bounds(doc["elements"])
    centre = tuple((lo[i] + hi[i]) / 2 for i in range(3))
    return centre, max(hi[i] - lo[i] for i in range(3)) * 1.08


def render_worn(spec: GearSpec, g: Gear, tmp: Path, cell: int = 420) -> Image.Image:
    tmp.write_text(json.dumps(worn_doc(g, "Worn"), ensure_ascii=False), encoding="utf-8")
    focus = spec.worn_focus or _focus(tmp)
    font_h, font_l = load_font(17), load_font(12)
    sheet = Image.new("RGB", (cell * len(WORN_VIEWS) + 5 * (len(WORN_VIEWS) + 1), cell + 62), (32, 33, 36))
    d = ImageDraw.Draw(sheet)
    d.text((8, 6), f"穿在标准玩家（灰色占位人体，身高 32px）身上 · {spec.wear_note}", fill=(235, 235, 235), font=font_h)
    for i, (label, yaw, pitch) in enumerate(WORN_VIEWS):
        x = 5 + i * (cell + 5)
        d.rectangle([x, 32, x + cell - 1, 53], fill=(22, 23, 26))
        d.text((x + 6, 35), label, fill=(235, 235, 235), font=font_l)
        sheet.paste(render(tmp, yaw=yaw, pitch=pitch, size=cell, bg=BG, focus=focus)[0], (x, 56))
    return sheet


# ───────────────────────────────────────────────────────────── 单件预览 / 六视图 / check.png
def write_model(g: Gear, path: Path, name: str) -> None:
    path.write_text(json.dumps(g.rig.bbmodel(name), ensure_ascii=False, indent=1), encoding="utf-8")


def render_model(path: Path, yaw: float, pitch: float, size: int) -> Image.Image:
    return render(path, yaw=yaw, pitch=pitch, size=size, bg=BG)[0]


SIX_VIEWS = (
    ("BACK 背面（相机在 +z，装备主视面）", 0.0, 0.0),
    ("3/4 背右斜（相机在 +z / +x 上方）", 28.0, 18.0),
    ("RIGHT 右侧（相机在 +x）", -90.0, 0.0),
    ("LEFT 左侧（相机在 -x）", 90.0, 0.0),
    ("FRONT 正面（相机在 -z，贴身一侧）", 180.0, 0.0),
    ("TOP 俯视（相机在 +y，脸朝画面下方）", 180.0, 89.9),
)


def render_part_previews(spec: GearSpec, tmp: Path) -> None:
    parts_dir = spec.review_dir / "parts"
    parts_dir.mkdir(parents=True, exist_ok=True)
    font_t, font_s = load_font(15), load_font(12)
    for idx, (key, fn) in enumerate(spec.parts.items(), start=1):
        part = spec.build([fn])
        write_model(part, tmp, f"{spec.model_name}_{key}")
        w = 300
        sheet = Image.new("RGB", (w * 3, w + 44), (32, 33, 36))
        d = ImageDraw.Draw(sheet)
        d.text((8, 5), f"part_{key}: {fn.__doc__.splitlines()[0]}  ({part.total()} cubes)", fill=(235, 235, 235), font=font_t)
        for k, (label, yaw, pitch) in enumerate((("BACK +z", 0.0, 0.0), ("3/4 背右", 28.0, 18.0), ("RIGHT +x", -90.0, 0.0))):
            sheet.paste(render_model(tmp, yaw, pitch, w), (k * w, 24))
            d.text((k * w + 6, w + 26), label, fill=(200, 200, 200), font=font_s)
        sheet.save(parts_dir / f"part_{idx:02d}_{key}.png")
    print(f"✓ 分件预览 {len(spec.parts)} 张 → {parts_dir}")


def render_six_views(model_path: Path, title: str, cell: int = 340) -> Image.Image:
    font_h, font_l = load_font(17), load_font(12)
    sheet = Image.new("RGB", (cell * 3 + 20, cell * 2 + 70), (32, 33, 36))
    d = ImageDraw.Draw(sheet)
    d.text((10, 6), title, fill=(235, 235, 235), font=font_h)
    for i, (label, yaw, pitch) in enumerate(SIX_VIEWS):
        x, y = 5 + (i % 3) * (cell + 5), 32 + (i // 3) * (cell + 28)
        sheet.paste(render_model(model_path, yaw, pitch, cell), (x, y + 22))
        d.rectangle([x, y, x + cell - 1, y + 21], fill=(22, 23, 26))
        d.text((x + 6, y + 3), label, fill=(235, 235, 235), font=font_l)
    return sheet


def bone_tree_panel(g: Gear, min_bones: int, height: int) -> Image.Image:
    children: dict[str | None, list[str]] = {}
    for name, parent, _, _ in g.bones:
        children.setdefault(parent, []).append(name)
    desc = {name: d for name, _, _, d in g.bones}
    lines: list[str] = []

    def walk(name: str, prefix: str, last: bool, root: bool) -> None:
        n = len(g.rig.bones[name]["children"])
        branch = "" if root else ("└─ " if last else "├─ ")
        lines.append(f"{prefix}{branch}{name}  [{n}]  {desc[name]}")
        kids = children.get(name, [])
        for i, k in enumerate(kids):
            walk(k, prefix + ("" if root else ("   " if last else "│  ")), i == len(kids) - 1, False)

    walk(g.bones[0][0], "", True, True)
    font_h, font_s, font_t = load_font(17), load_font(13), load_font(12)
    step, w = 17, 800
    h = max(height, 110 + step * len(lines) + 20)
    panel = Image.new("RGB", (w, h), (32, 33, 37))
    d = ImageDraw.Draw(panel)
    d.rectangle([0, 0, w - 1, h - 1], outline=(70, 75, 85), width=2)
    d.text((14, 8), "骨骼层级树 BONE HIERARCHY（[n] = 该骨自带的立方体数；按挂点分骨）", fill=(180, 235, 200), font=font_h)
    d.text((14, 34), f"总骨骼 {len(g.rig.bones)}（含 root）｜带几何的可动骨 {moving_bone_count(g)}｜下限 ≥ {min_bones} ✓",
           fill=(240, 210, 150), font=font_s)
    d.text((14, 54), f"立方体 {g.total()}｜材质 {len(g.rig.mats)} 种｜坐标 0.05px 的整数倍", fill=(190, 210, 235), font=font_s)
    d.line([(12, 78), (w - 12, 78)], fill=(65, 70, 80), width=1)
    y = 86
    for ln in lines:
        d.text((14, y), ln, fill=(228, 228, 234), font=font_t)
        y += step
    return panel


def make_check(spec: GearSpec, g: Gear, model_path: Path, gate_total: int) -> None:
    rd = spec.review_dir
    font_h = load_font(18)
    six = render_six_views(model_path, f"{spec.title} round 1/3 · {g.total()} cubes · {len(g.rig.bones)} 骨"
                                       f"（可动 {moving_bone_count(g)}）· 门禁违例 {gate_total}")
    worn_tmp = spec.out_dir / "_tmp_worn.bbmodel"
    try:
        worn = render_worn(spec, g, worn_tmp)
    finally:
        worn_tmp.unlink(missing_ok=True)
    mid_w = max(six.width, worn.width)
    tree = bone_tree_panel(g, spec.min_bones, six.height + worn.height + 10)
    refs = []
    for kind, label in (("ref", "参考图（生图，gen_gear_ref.py）"), ("exploded", "爆炸分解图")):
        path = rd / f"{spec.gear_id}_{kind}.png"
        refs.append((label, Image.open(path).convert("RGB") if path.exists() else None))
    lw = 640
    scaled = [(lab, im.resize((lw, round(im.height * lw / im.width)), Image.Resampling.LANCZOS) if im else None)
              for lab, im in refs]
    ref_h = sum((im.height if im else 200) + 40 for _, im in scaled)
    total_h = max(ref_h + 20, six.height + worn.height + 70, tree.height + 44)
    canvas = Image.new("RGB", (lw + mid_w + tree.width + 50, total_h), (25, 26, 29))
    d = ImageDraw.Draw(canvas)
    y = 8
    for lab, im in scaled:
        d.text((10, y), lab, fill=(215, 205, 180), font=font_h)
        if im:
            canvas.paste(im, (10, y + 30))
        y += (im.height if im else 200) + 40
    x = lw + 25
    d.text((x, 8), "本模型（Round 1 几何：无贴图 / 动画 / 接线 / manifest）", fill=(140, 230, 180), font=font_h)
    canvas.paste(six, (x, 38))
    canvas.paste(worn, (x, 38 + six.height + 10))
    x += mid_w + 15
    d.text((x, 8), f"骨骼树（{len(g.rig.bones)} 骨）", fill=(180, 220, 240), font=font_h)
    canvas.paste(tree, (x, 38))
    rd.mkdir(parents=True, exist_ok=True)
    out = rd / "check.png"
    canvas.save(out)
    print(f"✓ check.png → {out} ({canvas.width}×{canvas.height})")


def full_run(spec: GearSpec) -> None:
    """全量：自证 → 全套门禁 → 写 bbmodel → 分件预览 → check.png。"""
    g = spec.build()
    self_test(spec, g)
    total = run_gates(spec, g)
    if total:
        raise SystemExit(f"门禁仍有 {total} 处违例，不出图")
    model = spec.out_dir / f"{spec.model_name}.bbmodel"
    write_model(g, model, spec.model_name)
    print(f"✓ {model}: {g.total()} cubes / {len(g.rig.bones)} bones")
    spec.review_dir.mkdir(parents=True, exist_ok=True)
    tmp = spec.out_dir / "_tmp_part.bbmodel"
    try:
        render_part_previews(spec, tmp)
    finally:
        tmp.unlink(missing_ok=True)
    make_check(spec, g, model, total)


def main(spec: GearSpec) -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--part", choices=sorted(spec.parts))
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--gates", action="store_true")
    ap.add_argument("--worn", action="store_true", help="只出穿戴三面预览到 /tmp，不跑全量")
    args = ap.parse_args()
    if not (args.part or args.self_test or args.gates or args.worn):
        full_run(spec)
        return
    g = spec.build([spec.parts[args.part]] if args.part else None)
    if args.self_test:
        self_test(spec, g)
    elif args.gates:
        run_gates(spec, g)
    elif args.worn:
        tmp = spec.out_dir / "_tmp_worn.bbmodel"
        try:
            sheet = render_worn(spec, g, tmp)
        finally:
            tmp.unlink(missing_ok=True)
        out = Path("/tmp/claude-1000") / f"{spec.gear_id}_worn.png"
        sheet.save(out)
        print(f"✓ {out}")
    else:
        out = spec.out_dir / f"_tmp_{args.part}.bbmodel"
        write_model(g, out, spec.model_name)
        print(f"{g.total()} cubes → {out}")
