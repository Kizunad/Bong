"""末世生物建模生成器的共用底座：建模辅助、门禁、骨骼树、审阅图（接触表 / 同取景对照 / check.png）。

每只生物一个目录 ``modelScript/creatures/<id>/``，里面的 ``gen_<id>.py`` 只写自己的东西：
材质表、骨骼树、分部件函数（``part_*``），然后交给 :func:`run_cli` 统一跑门禁、写 bbmodel、出审阅图。
几何约定同游商傀儡：1 block = 16px，原点在两脚（四足为四蹄）中间的地面，+y 向上，
正面朝 -z（GeckoLib 约定），+x 是生物的右侧（骨名 ``*_r``）。

审阅输出目录：环境变量 ``BONG_MODEL_REVIEW_DIR`` 指定时写那里（调度的审阅页读这个目录），
否则写 ``modelScript/out/review``（已 gitignore）。参考图 / 上一轮模型缺失时只跳过对应的图，
生成器照常退出 0（CI 的 golden 沙箱里没有 model-review）。
"""

from __future__ import annotations

import argparse
import copy
import importlib.util
import json
import os
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Callable

import numpy as np
from PIL import Image, ImageDraw

from bbmodel_maker.gates import gatekit
from bbmodel_maker.render.render_bbmodel import render
from bbmodel_maker.rig.rigkit import Rig

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "modelScript" / "tools"))
from cjk_font import load_font  # noqa: E402

# 仓库内的共面 (Z-fighting) 门禁：modelScript/bbmodel_maker 与已安装的同名库同名，按路径单独加载
_spec = importlib.util.spec_from_file_location("repo_coplanar", REPO / "modelScript" / "bbmodel_maker" / "gates" / "coplanar.py")
coplanar = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(coplanar)

BG = (119, 119, 119)
Bone = tuple[str, "str | None", tuple[float, float, float], str]   # (名, 父, 枢轴 px, 说明)

# 六个诚实命名的视角：名字写的是相机所在位置（生物正面朝 -z，右侧 = +x）。
# render() 的 yaw=180 时相机在 -z 看正面，yaw=0 在 +z 看背面，yaw=90 在 -x 看左侧。
SIX_VIEWS = (
    ("FRONT 正视（相机在 -z）", 180.0, 0.0),
    ("3/4 前右斜视（相机在 -z / +x 上方）", 150.0, 18.0),
    ("LEFT 左侧视（相机在 -x，头朝画面左）", 90.0, 0.0),
    ("RIGHT 右侧视（相机在 +x，头朝画面右）", -90.0, 0.0),
    ("TOP 俯视（相机在 +y，头朝画面下方）", 180.0, 89.9),
    ("BACK 背视（相机在 +z）", 0.0, 0.0),
)
SAME_FRAMING_VIEWS = (("FRONT 正", 180.0, 0.0), ("LEFT 侧（头朝左）", 90.0, 0.0), ("BACK 背", 0.0, 0.0))


def review_root() -> Path:
    override = os.environ.get("BONG_MODEL_REVIEW_DIR")
    return Path(override) if override else REPO / "modelScript" / "out" / "review"


@dataclass
class CreatureSpec:
    """一只生物的全部输入。parts 的顺序 = 建模顺序；骨骼数下限见任务卡「骨骼数下限」。"""

    model_id: str                     # 如 sui_yan_yuan，也是审阅目录名
    title: str                        # 如「碎岩猿」
    class_name: str                   # bbmodel 名 / 文件名，如 SuiYanYuan
    creature_dir: Path
    mats: dict[str, tuple[int, int, int]]
    bones: list[Bone]
    parts: dict[str, Callable[["Builder"], None]]
    min_bones: int
    soft_over: frozenset = frozenset()
    asym_bones: frozenset = frozenset()
    ref_prompt: str = ""              # 参考图（CPA 文生图）的主体描述，英文
    round_label: str = "round 1/3"
    # 侧视比例量尺（见 proportion_check.py）：缺省不出这一行。键：ref_crop 参考图里整只生物侧视的裁剪框、
    # ref_torso_x 裁剪后参考图上人点的（肩前缘, 臀后缘）像素列、ref_nose_x 可选的鼻尖列、
    # front_bones / rear_bones 模型里躯干前缘 / 后缘所在的骨（取它们立方体的 z 范围）
    proportion: dict | None = None
    gates: gatekit.AssetGates = field(init=False)

    def __post_init__(self) -> None:
        self.gates = EntityGates(f"{self.title} / {self.model_id}", self.mats, asym=self.asym_bones, soft_over=self.soft_over)

    @property
    def review_dir(self) -> Path:
        return review_root() / "img" / "creatures" / self.model_id


class EntityGates(gatekit.AssetGates):
    """实体模型的门禁：去掉「越出 0..16 方块空间」一道 —— 生物不是方块，GeckoLib 实体不受它限制。"""

    def specs(self):
        return tuple(spec for spec in super().specs() if spec[0] != "overflow")


class _Offset:
    def __init__(self, builder, delta):
        self.builder, self.delta = builder, delta

    def __enter__(self):
        self.saved = self.builder.shift
        self.builder.shift = tuple(self.saved[k] + self.delta[k] for k in range(3))

    def __exit__(self, *exc):
        self.builder.shift = self.saved


class Builder:
    """Rig 的薄封装：左右镜像成对建件。"""

    def __init__(self, spec: CreatureSpec) -> None:
        self.shift = (0.0, 0.0, 0.0)
        self.spec = spec
        self.rig = Rig(spec.mats, tex=64, swatch=8)
        for name, parent, pivot, _ in spec.bones:
            self.rig.bone(name, pivot, parent)

    def offset(self, delta):
        """上下文管理器：块内建的所有件整体平移 delta=(dx, dy, dz)（含旋转中心），用来整组前倾 / 下沉（含胸驼背）。
        对应骨的枢轴要在骨骼树里同步改。"""
        return _Offset(self, delta)

    def box(self, bone, name, f, t, mat, rot=None, org=None):
        d = self.shift
        f, t = tuple(f[k] + d[k] for k in range(3)), tuple(t[k] + d[k] for k in range(3))
        org = None if org is None else tuple(org[k] + d[k] for k in range(3))
        return self.rig.cube(bone, name, f, t, mat=mat, rot=rot, org=org)

    def pair(self, bone, name, f, t, mat, rot=None, org=None, sided=True):
        """f/t 按右侧（+x）坐标给；左侧自动镜像（x 取反，旋转的 y/z 分量取反）。

        sided=True：右件挂 ``bone_r``、左件挂 ``bone_l``（bone 是骨名词干）；
        sided=False：左右两件都挂在 bone（中线骨上的成对件，如肋骨）。
        """
        bone_r = f"{bone}_r" if sided else bone
        bone_l = f"{bone}_l" if sided else bone
        self.box(bone_r, f"{name}_r", f, t, mat, rot=rot, org=org)
        fl = (-t[0], f[1], f[2])
        tl = (-f[0], t[1], t[2])
        rl = None if rot is None else (rot[0], -rot[1], -rot[2])
        ol = None if org is None else (-org[0], org[1], org[2])
        self.box(bone_l, f"{name}_l", fl, tl, mat, rot=rl, org=ol)

    def put(self, bone, name, f, t, mat, side=None, rot=None, org=None):
        """统一入口：side=None 单件；"lr" 左右骨成对（bone_l / bone_r）；"mid" 中线骨上成对。"""
        if side is None:
            return self.box(bone, name, f, t, mat, rot=rot, org=org)
        return self.pair(bone, name, f, t, mat, rot=rot, org=org, sided=(side == "lr"))

    def total(self) -> int:
        return len(self.rig.elements)


def _stem(name: str) -> str:
    return name[:-2] if name.endswith(("_l", "_r")) else name


COPLANAR_RE = re.compile(r"共面冲突: (\S+) 与 (\S+) 在 ([+-])([XYZ]) 面共面")
NUDGE = 0.05


def nudge_coplanar(b: Builder, max_rounds: int = 60) -> int:
    """层层叠毛 / 叠岩时两件的面常恰好落在同一平面（Z-fighting）。逐轮把冲突对里后一件的那一面
    向内缩 0.05px 错开，直到共面门不再报；返回被缩的面数。几何意图（谁压着谁）不变。"""
    moved = 0
    by_name = {e["name"]: e for e in b.rig.elements}
    for _ in range(max_rounds):
        hits = coplanar_violations(b)
        if not hits:
            return moved
        shrinks: set[tuple[str, int, str]] = set()
        for line in hits:
            m = COPLANAR_RE.search(line)
            if not m:
                continue
            first, second, sign, axis = m.groups()
            victim = second if _stem(second) >= _stem(first) else first    # 左右镜像件按同一规则选，保证两侧缩得一样
            k = "XYZ".index(axis)
            shrinks.add((victim, k, sign))
            twin = mirror_twin(victim)
            if twin in by_name:                                            # 镜像件一并缩（x 轴上正负面对调），保持左右对称
                shrinks.add((twin, k, ("-" if sign == "+" else "+") if k == 0 else sign))
        for victim, k, sign in sorted(shrinks):
            el = by_name[victim]
            if sign == "+":
                el["to"][k] = round(el["to"][k] - NUDGE, 4)
            else:
                el["from"][k] = round(el["from"][k] + NUDGE, 4)
            moved += 1
    return moved


def mirror_twin(name: str) -> str:
    """左右镜像件的名字：``_l`` ↔ ``_r``；没有后缀的中线件返回自己。"""
    if name.endswith("_l"):
        return name[:-2] + "_r"
    if name.endswith("_r"):
        return name[:-2] + "_l"
    return name


def build(spec: CreatureSpec, part_names: list[str] | None = None) -> Builder:
    b = Builder(spec)
    for name in part_names or list(spec.parts):
        spec.parts[name](b)
    assert_unique_names(b)
    nudge_coplanar(b)
    return b


def assert_unique_names(b: Builder) -> None:
    """方块名必须唯一：共面微调、门禁报告、镜像配对都是按名字找件的，重名会让它们改错件或漏报。"""
    seen: set[str] = set()
    dup = sorted({e["name"] for e in b.rig.elements if e["name"] in seen or seen.add(e["name"])})
    if dup:
        raise SystemExit(f"方块重名 {len(dup)} 个（共面微调 / 门禁按名字找件，重名会改错）：{dup[:8]}")


# ───────────────────────────────────────────────────────────── 门禁
def gate_rig(spec: CreatureSpec, b: Builder) -> Rig:
    """门禁用副本：rigkit 把 color 写成「材质序号 % 8」，材质多于 8 种时会撞号，
    门禁按材质名判同材质 / 软覆盖，所以副本里按贴图色块 uv 反推完整序号。"""
    rig = copy.deepcopy(b.rig)
    per_row = rig.tex // rig.swatch
    for el in rig.elements:
        uv = el["faces"]["north"]["uv"]
        el["color"] = int(uv[1] // rig.swatch) * per_row + int(uv[0] // rig.swatch)
    spec.gates.colors = {i: n for i, n in enumerate(spec.mats)}
    return rig


def coplanar_violations(b: Builder, tol: float = 1e-4) -> list[str]:
    """共面（Z-fighting）门：只查没有旋转的件（旋转件的 from/to 不是真实包围盒）。

    判据与 ``coplanar.check_coplanar_faces`` 完全相同（同轴同向面坐标差 < tol 且另两轴投影有正面积重叠），
    输出文案也一致；区别是先按「轴 + 面坐标」分桶，只在同桶里两两比较。几千个小块的生物用原版两两全比要几十秒，
    nudge_coplanar 还要反复调用，所以这里提速而不改语义。
    """
    flat = [e for e in b.rig.elements if not any(e["rotation"])]
    buckets: dict[tuple[int, str, int], list[int]] = {}
    for idx, el in enumerate(flat):
        for axis in range(3):
            buckets.setdefault((axis, "-", round(el["from"][axis] / tol)), []).append(idx)
            buckets.setdefault((axis, "+", round(el["to"][axis] / tol)), []).append(idx)
    found: set[tuple[int, int, int, str]] = set()
    for (axis, sign, _), members in buckets.items():
        if len(members) < 2:
            continue
        other = [a for a in range(3) if a != axis]
        for ai, i in enumerate(members):
            for j in members[ai + 1:]:
                lo, hi = (i, j) if i < j else (j, i)
                a, c = flat[lo], flat[hi]
                face = "from" if sign == "-" else "to"
                if abs(a[face][axis] - c[face][axis]) >= tol:
                    continue
                ov = [min(a["to"][o], c["to"][o]) - max(a["from"][o], c["from"][o]) for o in other]
                if ov[0] > tol and ov[1] > tol:
                    found.add((lo, hi, axis, sign))
    lines = []
    for lo, hi, axis, sign in sorted(found):
        a, c = flat[lo], flat[hi]
        other = [x for x in range(3) if x != axis]
        ov = [min(a["to"][o], c["to"][o]) - max(a["from"][o], c["from"][o]) for o in other]
        coord = a["from"][axis] if sign == "-" else a["to"][axis]
        lines.append(f"共面冲突: {a['name']} 与 {c['name']} 在 {sign}{'XYZ'[axis]} 面共面 ({coord:.4f}), 重叠区域 ({ov[0]:.3f}x{ov[1]:.3f})")
    return lines


def moving_bone_count(b: Builder) -> int:
    """真正带几何、能独立动的骨（不含只做分组的骨）。"""
    return sum(1 for bone in b.rig.bones.values() if bone["children"])


def run_gates(spec: CreatureSpec, b: Builder) -> int:
    total = spec.gates.report(gate_rig(spec, b), px=16.0, note="（实体不受 16px 方块空间限制，已去掉越界一道）")
    cop = coplanar_violations(b)
    print(f"  {'✓' if not cop else '✗'} 共面 (Z-fighting): {len(cop)}")
    for line in cop[:80]:
        print(f"      - {line}")
    moving = moving_bone_count(b)
    ok = moving >= spec.min_bones
    print(f"  {'✓' if ok else '✗'} 骨骼数: 共 {len(b.rig.bones)}，带几何的可动骨 {moving}（下限 {spec.min_bones}）")
    return total + len(cop) + (0 if ok else 1)


def self_test(spec: CreatureSpec, b: Builder) -> None:
    """差分自证：先在干净模型上跑一遍（须全绿），再给每道门注入缺陷，须被抓到。"""
    broken = spec.gates.self_test(gate_rig(spec, b))
    clean = coplanar_violations(b)
    flat = [e for e in b.rig.elements if not any(e["rotation"])]
    victim = flat[0]
    twin = copy.deepcopy(victim)
    twin["name"] = victim["name"] + "__twin"
    hits = coplanar.check_coplanar_faces(flat + [twin])
    if clean or not hits:
        broken += 1
        print(f"  ✗ 共面: 干净 {len(clean)} / 注入后 {len(hits)}（须 0 / >0）")
    else:
        print(f"  ✓ 共面: 干净 0 → 注入同位复制件 {victim['name']} 后 {len(hits)}")
    first = next(iter(spec.parts))
    small = build(spec, [first])
    if moving_bone_count(small) >= spec.min_bones:
        broken += 1
        print("  ✗ 骨骼数门没有鉴别力")
    else:
        print(f"  ✓ 骨骼数: 完整 {moving_bone_count(b)} ≥ {spec.min_bones}；只含 {first} 的件 {moving_bone_count(small)} 被拦下")
    if broken:
        raise SystemExit(f"自证未通过：{broken} 道门失效")
    print("✓ 差分自证全绿")


# ───────────────────────────────────────────────────────────── 参考图（CPA 文生图）
REF_STYLE = (
    "Minecraft voxel style character design reference sheet, every surface built from chunky cuboid voxel blocks "
    "with visible 16px-texel texture, dharma-ending xianxia wasteland aesthetic: weathered, desaturated, bone, rust, "
    "dry earth and old wood, no glowing fantasy gloss. Layout: the top 70% shows THREE full-body orthographic views "
    "side by side — front view, side view (facing LEFT), back view — same scale, standing on the ground; "
    "the bottom 30% shows five detail close-ups of the most characteristic parts. "
    "plain neutral grey studio background, soft shadow, no text, no labels, no watermark"
)


def ensure_reference(spec: CreatureSpec) -> Path | None:
    """缺参考图时用 CPA 生成一张（key 取 ~/cpa/config.yaml，不打印不落盘）；失败返回 None，不影响建模。"""
    path = spec.review_dir / f"{spec.model_id}_ref.png"
    if path.exists() or not spec.ref_prompt:
        return path if path.exists() else None
    try:
        from gen_exploded_ref import cpa_client
        data = cpa_client().text_to_image(f"{REF_STYLE}. Subject: {spec.ref_prompt}", size="1024x1536")
    except Exception as exc:  # 网络 / 鉴权问题只影响审阅图
        print(f"参考图未生成（{type(exc).__name__}）：{exc}")
        return None
    spec.review_dir.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)
    print(f"✓ 参考图 → {path}")
    return path


# ───────────────────────────────────────────────────────────── 审阅图
def write_model(b: Builder, path: Path, name: str) -> None:
    path.write_text(json.dumps(b.rig.bbmodel(name), ensure_ascii=False, indent=1), encoding="utf-8")


def render_model(path: Path, yaw: float, pitch: float, size: int) -> Image.Image:
    return render(path, yaw=yaw, pitch=pitch, size=size, bg=BG)[0]


def render_part_previews(spec: CreatureSpec, tmp: Path) -> None:
    """逐件单独预览：每个 part_*() 只放自己的立方体，出正 / 3/4 / 侧三视角。"""
    parts_dir = spec.review_dir / "parts"
    parts_dir.mkdir(parents=True, exist_ok=True)
    font_t, font_s = load_font(15), load_font(12)
    for idx, (key, fn) in enumerate(spec.parts.items(), start=1):
        part = build(spec, [key])
        write_model(part, tmp, f"{spec.class_name}_{key}")
        w = 300
        sheet = Image.new("RGB", (w * 3, w + 44), (32, 33, 36))
        d = ImageDraw.Draw(sheet)
        d.text((8, 5), f"part_{key}: {fn.__doc__.splitlines()[0]}  ({part.total()} cubes)", fill=(235, 235, 235), font=font_t)
        for k, (label, yaw, pitch) in enumerate((("FRONT -z", 180.0, 0.0), ("3/4 前右", 150.0, 18.0), ("LEFT -x", 90.0, 0.0))):
            sheet.paste(render_model(tmp, yaw, pitch, w), (k * w, 24))
            d.text((k * w + 6, w + 26), label, fill=(200, 200, 200), font=font_s)
        sheet.save(parts_dir / f"part_{idx:02d}_{key}.png")
    print(f"✓ 分件预览 {len(spec.parts)} 张 → {parts_dir}")


def render_six_views(model_path: Path, title: str, cell: int = 380) -> Image.Image:
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


def figure_box(im: Image.Image, bg_tol: float = 38.0) -> tuple[int, int, int, int]:
    """图里「不是背景」的包围盒：背景取四条边缘像素的中位色。"""
    a = np.asarray(im.convert("RGB"), dtype=float)
    edge = np.concatenate([a[0], a[-1], a[:, 0], a[:, -1]])
    mask = np.abs(a - np.median(edge, axis=0)).sum(axis=2) > bg_tol
    ys, xs = np.where(mask)
    return int(xs.min()), int(ys.min()), int(xs.max()) + 1, int(ys.max()) + 1


def fit_height(im: Image.Image, h: int) -> Image.Image:
    box = figure_box(im)
    pad = int(0.03 * (box[3] - box[1]))
    crop = im.crop((max(0, box[0] - pad), max(0, box[1] - pad), min(im.width, box[2] + pad), min(im.height, box[3] + pad)))
    return crop.resize((max(1, round(crop.width * h / crop.height)), h), Image.Resampling.LANCZOS)


def same_framing_row(model_path: Path, ref_path: Path, h: int = 340) -> Image.Image:
    """参考图上排三张（正 / 侧 / 背，按宽度三等分）与本模型同视角，各自裁到包围盒后缩到同一高度并排：上行参考，下行模型。"""
    font_h, font_l = load_font(17), load_font(12)
    ref = Image.open(ref_path).convert("RGB")
    ref_top = ref.crop((0, 0, ref.width, int(ref.height * 0.70)))
    third = ref_top.width // 3
    ref_tiles, mdl_tiles = [], []
    for i, (_, yaw, pitch) in enumerate(SAME_FRAMING_VIEWS):
        ref_tiles.append(fit_height(ref_top.crop((i * third, 0, (i + 1) * third, ref_top.height)), h))
        mdl_tiles.append(fit_height(render_model(model_path, yaw, pitch, 700), h))
    col_w = [max(a.width, b.width) + 12 for a, b in zip(ref_tiles, mdl_tiles)]
    canvas = Image.new("RGB", (sum(col_w) + 10, 2 * h + 96), (32, 33, 36))
    d = ImageDraw.Draw(canvas)
    d.text((10, 6), "同取景对照：上 参考，下 模型（裁到包围盒、同高）", fill=(235, 235, 235), font=font_h)
    x = 5
    for (label, _, _), rt, mt, cw in zip(SAME_FRAMING_VIEWS, ref_tiles, mdl_tiles, col_w):
        d.text((x + 4, 30), f"参考 · {label}", fill=(215, 205, 180), font=font_l)
        canvas.paste(rt, (x + (cw - rt.width) // 2, 48))
        d.text((x + 4, 54 + h), f"模型 · {label}", fill=(150, 230, 180), font=font_l)
        canvas.paste(mt, (x + (cw - mt.width) // 2, 72 + h))
        x += cw
    return canvas


def prev_version_row(model_path: Path, prev_path: Path, h: int = 340) -> Image.Image:
    """返工对照：上行是上一版模型，下行是本版，同样三个视角，各自裁到包围盒后缩到同一高度。"""
    font_h, font_l = load_font(17), load_font(12)
    prev_tiles, new_tiles = [], []
    for _, yaw, pitch in SAME_FRAMING_VIEWS:
        prev_tiles.append(fit_height(render_model(prev_path, yaw, pitch, 700), h))
        new_tiles.append(fit_height(render_model(model_path, yaw, pitch, 700), h))
    col_w = [max(a.width, b.width) + 12 for a, b in zip(prev_tiles, new_tiles)]
    canvas = Image.new("RGB", (sum(col_w) + 10, 2 * h + 96), (32, 33, 36))
    d = ImageDraw.Draw(canvas)
    d.text((10, 6), "返工对照：上 上一版，下 本版（同视角，裁到包围盒、同高）", fill=(235, 235, 235), font=font_h)
    x = 5
    for (label, _, _), pt, nt, cw in zip(SAME_FRAMING_VIEWS, prev_tiles, new_tiles, col_w):
        d.text((x + 4, 30), f"上一版 · {label}", fill=(215, 205, 180), font=font_l)
        canvas.paste(pt, (x + (cw - pt.width) // 2, 48))
        d.text((x + 4, 54 + h), f"本版 · {label}", fill=(150, 230, 180), font=font_l)
        canvas.paste(nt, (x + (cw - nt.width) // 2, 72 + h))
        x += cw
    return canvas


def proportion_row(spec: CreatureSpec, model_path: Path, ref_path: Path, prev_path: Path | None) -> Image.Image | None:
    """check.png 底部的比例量尺行：参考 / 上一版 / 当前版的 躯干长÷肩高、头颈长÷躯干长，差超过 10% 会标「还要改」。"""
    cfg = spec.proportion
    if not cfg:
        return None
    import proportion_check as pc

    def torso_z(path: Path) -> tuple[float, float]:
        return pc.bone_z_extent(path, cfg["front_bones"])[0], pc.bone_z_extent(path, cfg["rear_bones"])[1]

    return pc.side_ratio_row(load_font(14), ref_path, cfg["ref_crop"], cfg["ref_torso_x"], model_path, torso_z(model_path),
                             prev_path, torso_z(prev_path) if prev_path else None, ref_nose_x=cfg.get("ref_nose_x"))


def bone_tree_lines(spec: CreatureSpec, b: Builder) -> list[str]:
    children: dict[str | None, list[str]] = {}
    for name, parent, _, _ in spec.bones:
        children.setdefault(parent, []).append(name)
    desc = {name: d for name, _, _, d in spec.bones}
    lines: list[str] = []

    def walk(name: str, prefix: str, last: bool, root: bool) -> None:
        n = len(b.rig.bones[name]["children"]) if name in b.rig.bones else 0
        branch = "" if root else ("└─ " if last else "├─ ")
        lines.append(f"{prefix}{branch}{name}  [{n}]  {desc[name]}")
        kids = children.get(name, [])
        for i, k in enumerate(kids):
            walk(k, prefix + ("" if root else ("   " if last else "│  ")), i == len(kids) - 1, False)

    walk(spec.bones[0][0], "", True, True)
    return lines


def bone_tree_panel(spec: CreatureSpec, b: Builder, height: int) -> Image.Image:
    lines = bone_tree_lines(spec, b)
    font_h, font_s, font_t = load_font(17), load_font(13), load_font(12)
    step, w = 17, 760
    h = max(height, 110 + step * len(lines) + 20)
    panel = Image.new("RGB", (w, h), (32, 33, 37))
    d = ImageDraw.Draw(panel)
    d.rectangle([0, 0, w - 1, h - 1], outline=(70, 75, 85), width=2)
    d.text((14, 8), "骨骼层级树 BONE HIERARCHY（[n] = 该骨自带的立方体数）", fill=(180, 235, 200), font=font_h)
    d.text((14, 34), f"总骨骼 {len(b.rig.bones)}（含根）｜带几何的可动骨 {moving_bone_count(b)}｜任务卡下限 ≥ {spec.min_bones}",
           fill=(240, 210, 150), font=font_s)
    d.text((14, 54), f"立方体 {b.total()}｜材质 {len(spec.mats)} 种", fill=(190, 210, 235), font=font_s)
    d.line([(12, 78), (w - 12, 78)], fill=(65, 70, 80), width=1)
    y = 86
    for ln in lines:
        d.text((14, y), ln, fill=(228, 228, 234), font=font_t)
        y += step
    return panel


def make_check(spec: CreatureSpec, b: Builder, model_path: Path, gate_total: int, prev_path: Path | None = None) -> None:
    """check.png：左 参考图 | 中 六视角接触表 + 同取景对照（+ 与上一版的返工对照） | 右 骨骼层级树。缺参考图只出接触表和骨骼树。"""
    spec.review_dir.mkdir(parents=True, exist_ok=True)
    font_h = load_font(18)
    six = render_six_views(model_path, f"{spec.model_id}（{spec.title}）{spec.round_label} · {b.total()} cubes · "
                                       f"{len(b.rig.bones)} 骨（可动 {moving_bone_count(b)}）· 门禁违例 {gate_total}")
    six.save(spec.review_dir / "contact_sheet.png")
    ref_p = ensure_reference(spec)
    if ref_p is None:
        tree = bone_tree_panel(spec, b, six.height)
        tree.save(spec.review_dir / "bones_tree.png")
        print("跳过 check.png：没有参考图（审阅用，非必需）；接触表与骨骼树已出")
        return
    row = same_framing_row(model_path, ref_p)
    prev_row = prev_version_row(model_path, prev_path) if prev_path else None
    ratio_row = proportion_row(spec, model_path, ref_p, prev_path)
    mid_h = six.height + row.height + 10 + (prev_row.height + 10 if prev_row else 0) + (ratio_row.height + 10 if ratio_row else 0)
    tree = bone_tree_panel(spec, b, mid_h)
    tree.save(spec.review_dir / "bones_tree.png")
    ref = Image.open(ref_p).convert("RGB")
    lw = 640
    ref_s = ref.resize((lw, round(ref.height * lw / ref.width)), Image.Resampling.LANCZOS)
    mid_w = max(six.width, row.width, prev_row.width if prev_row else 0, ratio_row.width if ratio_row else 0)
    total_h = max(ref_s.height + 60, mid_h + 60, tree.height + 44)
    canvas = Image.new("RGB", (lw + mid_w + tree.width + 50, total_h), (25, 26, 29))
    d = ImageDraw.Draw(canvas)
    d.text((10, 8), "参考图（CPA 生图：上三视图 / 下细节）", fill=(215, 205, 180), font=font_h)
    canvas.paste(ref_s, (10, 38))
    x = lw + 25
    d.text((x, 8), f"本模型（{spec.round_label} 几何：无贴图 / 动画 / 接线 / manifest）", fill=(140, 230, 180), font=font_h)
    canvas.paste(six, (x, 38))
    canvas.paste(row, (x, 38 + six.height + 10))
    if prev_row:
        canvas.paste(prev_row, (x, 38 + six.height + row.height + 20))
    if ratio_row:
        canvas.paste(ratio_row, (x, 38 + six.height + row.height + 20 + (prev_row.height + 10 if prev_row else 0)))
    x += mid_w + 15
    d.text((x, 8), f"骨骼树（{len(b.rig.bones)} 骨）", fill=(180, 220, 240), font=font_h)
    canvas.paste(tree, (x, 38))
    out = spec.review_dir / "check.png"
    canvas.save(out)
    print(f"✓ check.png → {out} ({canvas.width}×{canvas.height})")


# ───────────────────────────────────────────────────────────── 命令行
def full_run(spec: CreatureSpec) -> None:
    """全量：自证 → 全套门禁 → 写 bbmodel → 分件预览 → 接触表 / 骨骼树 / check.png。"""
    b = build(spec)
    self_test(spec, b)
    total = run_gates(spec, b)
    if total:
        raise SystemExit(f"门禁仍有 {total} 处违例，不出图")
    model = spec.creature_dir / f"{spec.class_name}.bbmodel"
    prev = keep_prev_version(spec, model)
    write_model(b, model, spec.class_name)
    print(f"✓ {model}: {b.total()} cubes / {len(b.rig.bones)} bones")
    tmp = spec.creature_dir / "_tmp_part.bbmodel"
    try:
        render_part_previews(spec, tmp)
    finally:
        tmp.unlink(missing_ok=True)
    make_check(spec, b, model, total, prev)


def keep_prev_version(spec: CreatureSpec, model: Path) -> Path | None:
    """第一次覆盖前把已提交的旧模型存进审阅目录当「上一版」；已存过就沿用（想换基线就删掉它）。"""
    prev = spec.review_dir / f"{spec.model_id}_prev.bbmodel"
    if not prev.exists() and model.exists():
        prev.parent.mkdir(parents=True, exist_ok=True)
        prev.write_bytes(model.read_bytes())
    return prev if prev.exists() else None


def run_cli(spec: CreatureSpec) -> None:
    ap = argparse.ArgumentParser(description=f"{spec.title} 建模生成器")
    ap.add_argument("--part", choices=sorted(spec.parts), help="只生成单件预览 bbmodel")
    ap.add_argument("--self-test", action="store_true", help="门禁差分自证")
    ap.add_argument("--gates", action="store_true", help="只跑门禁")
    args = ap.parse_args()
    if not (args.part or args.self_test or args.gates):
        full_run(spec)
        return
    b = build(spec, [args.part] if args.part else None)
    if args.self_test:
        self_test(spec, b)
    elif args.gates:
        run_gates(spec, b)
    else:
        out = spec.creature_dir / f"_tmp_{args.part}.bbmodel"
        write_model(b, out, spec.class_name)
        print(f"{b.total()} cubes → {out}")
