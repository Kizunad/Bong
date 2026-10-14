#!/usr/bin/env python3
"""末土穿戴件（背包 / 腰挂 / 斜挎）生成器的共用部件：身体占位、穿模门、check.png 渲染。

穿戴约定（任务卡 ``.task-wasteland-gear.md`` §穿戴约定，先例 ``grass_pouch_back.geo.json`` +
``WornPackModel`` 类注释）：

  · 建模直接用 Bedrock geo 系：脚在 y=0，躯干 y∈[12,24]、x∈[-4,4]、z∈[-2,2]，**+z 是背面**
    （头朝 -z，正脸在 -z）。**不平移进 0..16 方块空间** —— 穿戴件的 cube origin 是绝对 model-space
    坐标，转 vanilla ModelPart 时按 ``vanilla_y = 24 - bedrock_y`` 直接换算。
  · 不能穿进玩家身体：躯干、双腿、头、双臂（x=±4..±8 的**静止姿态**体积）都是禁区。
    摆臂/走路时的扫掠体积没有仿真，所以靠留出静止姿态下的 x 间隔来给 —— 这是该门量不出的部分。
  · 按挂点分 bone（``pack_*`` / ``strap_*`` / ``hang_*`` …），挂件单独一骨，方便以后做晃动。

check.png = 灰色占位人体穿着该件的 正 / 侧 / 背 三面（全身 + 挂点特写两排），玩家用 8×12×4 躯干、
4×12×4 四肢、8³ 头，和原版 biped 尺寸一致。灰模只在渲染用的 rig 里出现，不进门禁、不进交付 bbmodel。
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path

from bbmodel_maker.gates import gatekit
from bbmodel_maker.render import framing
from bbmodel_maker.render.render_bbmodel import render
from bbmodel_maker.rig.rigkit import Rig, element_bounds

REPO = Path(__file__).resolve().parents[2]
REVIEW_ROOT = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/gear")
MODELS_DIR = REPO / "modelScript" / "models"

PX = 16.0

# ── 灰色占位人体（渲染专用）──────────────────────────────────────────
PLAYER_MATS = {
    "_pl_body": (158, 158, 162),
    "_pl_limb": (126, 126, 132),   # 手臂 / 腿比躯干暗一档，灰模里才分得清体块边界
    "_pl_face": (84, 84, 90),
}
# (名字, from, to, 材质)；左右按「穿戴者自己的左右」：右臂/右腿在 -x（与 vanilla biped 一致）
PLAYER_BOXES = (
    ("head", (-4, 24, -4), (4, 32, 4), "_pl_body"),
    ("body", (-4, 12, -2), (4, 24, 2), "_pl_body"),
    ("arm_r", (-8, 12, -2), (-4, 24, 2), "_pl_limb"),
    ("arm_l", (4, 12, -2), (8, 24, 2), "_pl_limb"),
    ("leg_r", (-4, 0, -2), (0, 12, 2), "_pl_limb"),
    ("leg_l", (0, 0, -2), (4, 12, 2), "_pl_limb"),
    # 脸：-z 面一条深色眼带，让「哪边是正面」一眼可辨
    ("face", (-3, 27.5, -4.2), (3, 29.0, -4.0), "_pl_face"),
)

# ── 穿模禁区（静止姿态）──────────────────────────────────────────────
ZONES = (
    ("躯干", (-4, 12, -2), (4, 24, 2)),
    ("右臂", (-8, 12, -2), (-4, 24, 2)),
    ("左臂", (4, 12, -2), (8, 24, 2)),
    ("右腿", (-4, 0, -2), (0, 12, 2)),
    ("左腿", (0, 0, -2), (4, 12, 2)),
    ("头", (-4, 24, -4), (4, 32, 4)),
)
CLIP_BITE = 0.15  # 三轴同时重叠超过它才算扎进去；贴着体表（重叠 ≈ 0）是合法的

# 穿戴件整体包络：脚下不许有、不许离身体一臂以外
ENVELOPE = ((-14.0, 0.0, -12.0), (14.0, 36.0, 14.0))


def gate_body_clip(rig: Rig, zones=ZONES, bite: float = CLIP_BITE) -> list[str]:
    """穿戴件的任一 cube 扎进玩家身体/手臂/腿/头的静止体积。"""
    bad = []
    for el in rig.elements:
        lo, hi = element_bounds([el])
        for zone, zlo, zhi in zones:
            depth = min(min(hi[k], zhi[k]) - max(lo[k], zlo[k]) for k in range(3))
            if depth > bite:
                bad.append(f"{el['name']} 扎进{zone} {depth:.2f}px")
    return bad


def gate_envelope(rig: Rig, envelope=ENVELOPE) -> list[str]:
    """越出穿戴包络（脚下 / 离人太远）—— 转 ModelPart 后会被视锥或挂点吃掉。"""
    lo_e, hi_e = envelope
    bad = []
    for el in rig.elements:
        lo, hi = element_bounds([el])
        if any(lo[k] < lo_e[k] - 0.01 or hi[k] > hi_e[k] + 0.01 for k in range(3)):
            bad.append(f"{el['name']}: {tuple(round(v, 2) for v in lo)}→{tuple(round(v, 2) for v in hi)}")
    return bad


def inject_body_clip(rig: Rig, **_):
    """把第一件挪到躯干正中 —— 穿进身体。"""
    import copy

    r = copy.deepcopy(rig)
    el = r.elements[0]
    lo, hi = element_bounds([el])
    for axis, target in enumerate((0.0, 18.0, 0.0)):
        delta = target - (lo[axis] + hi[axis]) / 2
        gatekit._translate(el, axis, delta)
    return r, el["name"], f"把 {el['name']} 挪进躯干正中"


def inject_envelope(rig: Rig, **_):
    import copy

    r = copy.deepcopy(rig)
    el = max(r.elements, key=lambda e: element_bounds([e])[1][0])
    gatekit._translate(el, 0, 30.0)
    return r, el["name"], f"把 {el['name']} 沿 x 推出包络 30px"


@dataclass
class GearGates(gatekit.AssetGates):
    """穿戴件门禁：在通用七道门上把「越出 0..16 方块空间」换成穿戴包络，并加一道「扎进身体」。

    symmetric=False 用于单侧挂件（腰侧袋、斜挎）：左右镜像对它没有意义，这道门整条拿掉，
    而不是靠 asym 白名单把所有件都排除 —— 排空之后差分注入器造不出缺陷，自证会误报失效。
    """

    symmetric: bool = True

    def specs(self):
        out = []
        for key, label, fn, injector in super().specs():
            if key == "mirror" and not self.symmetric:
                continue
            if key == "overflow":
                out.append(("envelope", "越出穿戴包络（脚下 / 离人过远）",
                            lambda r: gate_envelope(r), inject_envelope))
                continue
            out.append((key, label, fn, injector))
        out.append(("body_clip", "扎进玩家身体 / 手臂 / 腿 / 头",
                    lambda r: gate_body_clip(r), inject_body_clip))
        return tuple(out)


# ── 渲染 ─────────────────────────────────────────────────────────────
def render_mats(mats: dict) -> dict:
    """gear 材质 + 灰模材质。渲染 rig 不进门禁，所以材质总数可以超过 8。"""
    return {**mats, **PLAYER_MATS}


def add_player(rig: Rig) -> None:
    rig.bone("_player", (0.0, 12.0, 0.0))
    for name, a, b, mat in PLAYER_BOXES:
        rig.cube("_player", f"player_{name}", a, b, mat=mat)


def write_bbmodel(rig: Rig, name: str, path: Path) -> Path:
    model = rig.bbmodel(name)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(model, ensure_ascii=False, indent=1))
    return path


def _tile(path: Path, view, focus, size: int):
    img, _ = render(path, yaw=view.yaw, pitch=view.pitch, size=size, focus=focus,
                    shading="mc", bg=(34, 35, 40))
    return img


def render_check(build, mats: dict, gear_id: str, title: str, scratch: Path,
                 notes=(), close_pad: float = 6.0, close_bias=(0.0, 0.0, 0.0)) -> Path:
    """出 check.png：灰模穿着该件的 正/侧/背（全身 + 挂点特写）。

    build(rig=...) 把穿戴件造进给定 rig。第一排全身（公共取景，三张可比），第二排以穿戴件
    包围盒为中心的特写（同一取景）。视角名由 facing='-z' 派生：FRONT 看到 -z 脸，BACK 看到 +z 背。
    """
    scratch.mkdir(parents=True, exist_ok=True)
    gear_only = Rig(mats)
    build(rig=gear_only)
    lo, hi = gear_only.bounds()

    worn = Rig(render_mats(mats))
    build(rig=worn)
    add_player(worn)
    worn_path = write_bbmodel(worn, f"{gear_id}_worn", scratch / f"{gear_id}_worn.bbmodel")

    views = framing.views_for("-z", ("FRONT", "SIDE_R", "BACK"))
    full_focus = ((0.0, 16.0, 0.0), 40.0)
    center = tuple((lo[k] + hi[k]) / 2 + close_bias[k] for k in range(3))
    span = max(hi[k] - lo[k] for k in range(3)) + close_pad * 2
    close_focus = (center, max(span, 14.0))

    tiles = []
    for view in views:
        tiles.append((f"{view.label}  [full body]", _tile(worn_path, view, full_focus, 360)))
    for view in views:
        tiles.append((f"{view.label}  [gear close-up]", _tile(worn_path, view, close_focus, 360)))
    sheet = framing.contact_sheet(tiles, title=title, notes=notes, columns=3)

    out_dir = REVIEW_ROOT / gear_id
    out_dir.mkdir(parents=True, exist_ok=True)
    out = out_dir / "check.png"
    sheet.save(out)
    return out


def render_parts(build, parts: list[str], mats: dict, gear_id: str, scratch: Path) -> Path:
    """逐件预览：每个 part 单独造一遍，3/4 视角自动取景，拼成一张 parts.png。"""
    from bbmodel_maker.render.render_bbmodel import render as _render

    tiles = []
    view = framing.views_for("-z", ("3/4",))[0]
    for key in parts:
        rig = Rig(mats)
        build([key], rig=rig)
        p = write_bbmodel(rig, f"{gear_id}_{key}", scratch / f"{gear_id}_part_{key}.bbmodel")
        img, _ = _render(p, yaw=view.yaw, pitch=view.pitch, size=300, shading="mc", bg=(34, 35, 40))
        tiles.append((key, img))
    sheet = framing.contact_sheet(tiles, title=f"{gear_id} parts (3/4)", columns=4)
    out_dir = REVIEW_ROOT / gear_id
    out_dir.mkdir(parents=True, exist_ok=True)
    out = out_dir / "parts.png"
    sheet.save(out)
    return out
