#!/usr/bin/env python3
"""经脉工厂 b05 分流器 / b12 合流器共用部件（按爆炸图 x12_merger.png / x05_splitter.png 重做）。

两件是同一套部件，只差一个符号：`single_z`。
- b12 合流器：±X 两面进，单口在 +Z 出，-Z 是封闭后壁；`single_z=+1`。
- b05 分流器：-Z 一面进，±X 两面出，+Z 是封闭后壁；`single_z=-1`。
单口一侧永远是：臂段 + 烟囱口 + 骨楔尖朝向。四角那 4 块 L 形骨块已全部删除，换成 4 根 2×2 骨柱。

逐行宽度表（x 为左右宽度，单位 px；y 自下而上；原点在底面中心，方块 ±8）
  y    0–1  臂脚（臂段 x/z 5..8，横向 ±5）
  y    0–2  腔体地板 ±5（10 宽）
  y    1–2  臂框下梁（横向 ±5，高 1）
  y  2–8   臂框左右梁 ±(4..5) / 腔壁 ±(4..5)，窗口 8 宽 × 6 高（统一接口截面，底边离地 2）
  y  8–9   臂框上梁 ±5
  y  8–10  窗口过梁（开口侧）/ 封闭侧整面壁
  y  9–10  腔体顶面半透膜 ±4 × ±4
  y 10–10.5 十字亮通道贴片：中心 2×2 #fff0dc，往三条臂渐变（封闭后壁一臂被骨楔盖住，单口一臂落进烟囱）
  y 10–13  烟囱口：外 4×4（x ±2，z 1..5），内孔 2×2
  y 10–13  骨楔：6 宽 → 1 宽，0.5px 一级阶梯，尖朝单口一侧；脊线 y 13–13.5
  y  0–7   四根 2×2 骨柱，中心 (±6.9, ±6.9)，高度 7 < 腔体高 10

臂段 3 段肉肋（1px 一道）：x 5..6 #8a2a2a（7×5），6..7 #5a1a1a（6×4），7..8 #8a2a2a（7×5）。
臂框四条棱用 common_parts.flesh_collar；四个 2×2 小骨节凸出框外 0.5px，避开共面。
"""

from __future__ import annotations

import base64
import io
import json
import sys
import uuid
from pathlib import Path
from typing import Dict, List, Optional, Tuple

import numpy as np
from PIL import Image, ImageDraw

REPO = Path(__file__).resolve().parents[3]
if str(REPO) not in sys.path:
    sys.path.insert(0, str(REPO))

from modelScript.generators.meridian_factory import common_parts as cp  # noqa: E402
from modelScript.generators.meridian_factory.common_parts import (  # noqa: E402
    RES,
    _assert_no_coplanar_faces,
    bone_post,
    flesh_collar,
)

MODEL_DIR = REPO / "modelScript" / "models" / "meridian_factory"
REVIEW_ROOT = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory")

# 在公共调色板上只多一个 #fff0dc 十字通道中心亮芯
PALETTE = dict(cp.PALETTE)
PALETTE["qi_core"] = (255, 240, 220, 255)
MAT_UV = dict(cp.MAT_UV)
MAT_UV["qi_core"] = [16, 32, 32, 48]

# ---- 几何常量（px） ----
ARM_A0, ARM_A1 = 5.0, 8.0          # 臂段沿自身轴向的范围
CAVITY_HALF = 5.0                   # 腔体外廓 ±5，内壁 ±4
CAVITY_TOP = 10.0
FLOOR_TOP = 2.0
WINDOW_Y = (2.0, 8.0)               # 统一接口截面：8 宽 × 6 高，底边离地 2
MEMBRANE_Y = (9.0, 10.0)
GLOW_Y = (10.0, 10.5)
CHIMNEY_TOP = 13.0
POST_CENTER = 6.9
POST_HEIGHT = 7.0
WEDGE_Y = (10.0, 13.0)
WEDGE_Z_FAR, WEDGE_Z_NEAR = -4.0, -1.0   # 以 single_z=+1 计：宽端在后壁侧，尖在 z=-1
WEDGE_HALF_WIDTHS = [3.0, 2.5, 2.0, 1.5, 1.0, 0.5]   # 每 0.5px 一级


def _box(name, lo, hi, group, material) -> dict:
    return {"name": name, "from": list(lo), "to": list(hi), "group": group, "material": material}


def _mirror_z(cubes: List[dict], single_z: int) -> List[dict]:
    """single_z=-1 时把整组立方体沿 z 镜像（b05 就是 b12 的镜像）。"""
    if single_z > 0:
        return cubes
    out = []
    for c in cubes:
        f, t = c["from"], c["to"]
        n = dict(c)
        n["from"] = [f[0], f[1], -t[2]]
        n["to"] = [t[0], t[1], -f[2]]
        out.append(n)
    return out


def _orient(cubes: List[dict], direction: str) -> List[dict]:
    """把以 +X 臂为基准（a 沿 x，b 沿 z）的立方体转到 direction。"""
    out = []
    for c in cubes:
        f, t = c["from"], c["to"]
        n = dict(c)
        if direction == "+x":
            lo, hi = [f[0], f[1], f[2]], [t[0], t[1], t[2]]
        elif direction == "-x":
            lo, hi = [-t[0], f[1], f[2]], [-f[0], t[1], t[2]]
        elif direction == "+z":
            lo, hi = [f[2], f[1], f[0]], [t[2], t[1], t[0]]
        elif direction == "-z":
            lo, hi = [f[2], f[1], -t[0]], [t[2], t[1], -f[0]]
        else:
            raise ValueError(direction)
        n["from"], n["to"] = lo, hi
        out.append(n)
    return out


# =============================================================================
# 部件 1：臂段模块
# =============================================================================

def arm_module(direction: str, pfx: str) -> List[dict]:
    """8×6 截面的臂段：四条肉质棱梁 + 四个 2×2 小骨节 + 三道横向肌肉肋，沿 direction 伸出腔体。"""
    group = f"arm_{pfx}"
    axis = "x" if direction in ("+x", "-x") else "z"
    sign = 1.0 if direction in ("+x", "+z") else -1.0
    mid = sign * (ARM_A0 + ARM_A1) / 2.0

    # 四条棱肉梁：common_parts.flesh_collar（横向 ±5，y 1..9，窗口 8×6）
    # 只留上下两条横梁，左右两侧敞开，露出里面一圈圈肌肉肋
    collar = flesh_collar(center_pos=mid, length=ARM_A1 - ARM_A0, axis=axis, group=group, pfx=f"{pfx}_beam")
    cubes = [c for c in collar if c["name"].endswith(("_bot", "_top"))]

    local = []
    # 臂脚：托住下梁，免得臂悬空
    local.append(_box(f"{pfx}_foot", (ARM_A0, 0.0, -5.0), (ARM_A1, 1.0, 5.0), group, "flesh_dark"))
    # 三道横向肌肉肋：红 / 暗红 / 红
    local.append(_box(f"{pfx}_rib_1", (5.0, 2.5, -3.5), (6.0, 7.5, 3.5), group, "flesh_main"))
    local.append(_box(f"{pfx}_rib_2", (6.0, 3.0, -3.0), (7.0, 7.0, 3.0), group, "flesh_dark"))
    local.append(_box(f"{pfx}_rib_3", (7.0, 2.5, -3.5), (8.0, 7.5, 3.5), group, "flesh_main"))
    # 四个 2×2 小骨节，凸出框外 0.5px
    for name, (b0, b1), (y0, y1) in [
        ("knot_nw", (-5.5, -3.5), (7.5, 9.5)),
        ("knot_ne", (3.5, 5.5), (7.5, 9.5)),
        ("knot_sw", (-5.5, -3.5), (0.5, 2.5)),
        ("knot_se", (3.5, 5.5), (0.5, 2.5)),
    ]:
        local.append(_box(f"{pfx}_{name}", (5.5, y0, b0), (7.5, y1, b1), group, "bone_main"))
    cubes_local = _orient(local, direction)
    return cubes + cubes_local


# =============================================================================
# 部件 2：中心腔
# =============================================================================

def center_chamber(open_sides: Tuple[str, ...], single_z: int) -> List[dict]:
    """约 8×8 内腔、外廓 10×10、高到 y10；顶面半透膜 + 十字亮通道 + 单口侧小方烟囱。

    open_sides：开窗的方向（'+x' '-x' 以及单口一侧 '+z'/'-z'），其余为封闭壁。
    """
    g = "chamber"
    cubes = [_box("chamber_floor", (-5.0, 0.0, -5.0), (5.0, FLOOR_TOP, 5.0), g, "flesh_dark")]

    # 四个 1×1 角柱
    for sx in (-1, 1):
        for sz in (-1, 1):
            x0 = 4.0 if sx > 0 else -5.0
            z0 = 4.0 if sz > 0 else -5.0
            cubes.append(_box(f"chamber_corner_{sx:+d}{sz:+d}", (x0, FLOOR_TOP, z0), (x0 + 1.0, CAVITY_TOP, z0 + 1.0), g, "flesh_main"))

    # 四面壁板（夹在角柱之间，宽 8）；开口侧只剩窗口上方的过梁
    for d in ("+x", "-x", "+z", "-z"):
        y0 = WINDOW_Y[1] if d in open_sides else FLOOR_TOP
        sign = 1.0 if d[0] == "+" else -1.0
        if d[1] == "x":
            lo = (4.0 if sign > 0 else -5.0, y0, -4.0)
            hi = (lo[0] + 1.0, CAVITY_TOP, 4.0)
        else:
            lo = (-4.0, y0, 4.0 if sign > 0 else -5.0)
            hi = (4.0, CAVITY_TOP, lo[2] + 1.0)
        cubes.append(_box(f"chamber_wall_{d}", lo, hi, g, "flesh_main" if d in open_sides else "tendon_tube"))

    # 封闭后壁外侧两条斜筋丝
    back = -single_z
    zf = 5.0 if back > 0 else -5.5
    cubes.append(_box("chamber_back_fiber_a", (-3.0, 3.0, min(zf, zf + 0.5)), (-0.5, 5.0, max(zf, zf + 0.5)), g, "tendon_fiber"))
    cubes.append(_box("chamber_back_fiber_b", (0.5, 5.5, min(zf, zf + 0.5)), (3.0, 7.5, max(zf, zf + 0.5)), g, "tendon_fiber"))

    # 顶面半透膜
    cubes.append(_box("chamber_membrane", (-4.0, MEMBRANE_Y[0], -4.0), (4.0, MEMBRANE_Y[1], 4.0), g, "tendon_tube"))
    return cubes


def glow_cross(single_z: int) -> List[dict]:
    """膜上十字形亮通道：中心 #fff0dc，往三条臂依次 #f6dcc4 / #e8bca8 / #c07868。

    封闭后壁一臂被骨楔盖住，不画；单口一臂落在烟囱内孔（z 2..4）里。
    """
    g = "glow_cross"
    y0, y1 = GLOW_Y
    cubes = [_box("glow_center", (-1.0, y0, -1.0), (1.0, y1, 1.0), g, "qi_core")]
    gradient = ["qi_glow", "tendon_highlight", "tendon_fiber"]
    for i, mat in enumerate(gradient):
        d0, d1 = 1.0 + i, 2.0 + i
        cubes.append(_box(f"glow_xp_{i}", (d0, y0, -1.0), (d1, y1, 1.0), g, mat))
        cubes.append(_box(f"glow_xn_{i}", (-d1, y0, -1.0), (-d0, y1, 1.0), g, mat))
    # 单口一臂（以 +z 计），只在烟囱内孔里露出两段
    cubes.append(_box("glow_out_0", (-1.0, y0, 2.0), (1.0, y1, 3.0), g, "qi_glow"))
    cubes.append(_box("glow_out_1", (-1.0, y0, 3.0), (1.0, y1, 4.0), g, "tendon_highlight"))
    return _mirror_z(cubes, single_z)


def chimney(single_z: int) -> List[dict]:
    """单口一侧的小方形烟囱口：外 4×4（x ±2，z 1..5），内孔 2×2，高 y10..13。"""
    g = "chimney"
    y0, y1 = CHIMNEY_TOP - 3.0, CHIMNEY_TOP
    cubes = [
        _box("chimney_w", (-2.0, y0, 1.0), (-1.0, y1, 5.0), g, "flesh_main"),
        _box("chimney_e", (1.0, y0, 1.0), (2.0, y1, 5.0), g, "flesh_main"),
        _box("chimney_n", (-1.0, y0, 1.0), (1.0, y1, 2.0), g, "flesh_main"),
        _box("chimney_s", (-1.0, y0, 4.0), (1.0, y1, 5.0), g, "flesh_main"),
        # 口沿一圈亮肉，微凸 0.5px
        _box("chimney_lip_w", (-2.0, y1, 1.0), (-1.0, y1 + 0.5, 5.0), g, "flesh_lit"),
        _box("chimney_lip_e", (1.0, y1, 1.0), (2.0, y1 + 0.5, 5.0), g, "flesh_lit"),
        _box("chimney_lip_n", (-1.0, y1, 1.0), (1.0, y1 + 0.5, 2.0), g, "flesh_lit"),
        _box("chimney_lip_s", (-1.0, y1, 4.0), (1.0, y1 + 0.5, 5.0), g, "flesh_lit"),
    ]
    return _mirror_z(cubes, single_z)


# =============================================================================
# 部件 3：骨楔
# =============================================================================

def bone_wedge(single_z: int, flip: bool = False) -> List[dict]:
    """立在腔体顶膜上的米白骨楔：宽端靠封闭后壁，尖朝单口一侧（合流器朝出口、分流器朝入口）。

    flip=True 只给差分自证用：造一个尖朝反的坏楔。
    """
    g = "bone_wedge"
    y0, y1 = WEDGE_Y
    cubes = []
    step = 0.5
    for i, hw in enumerate(WEDGE_HALF_WIDTHS):
        z0 = WEDGE_Z_FAR + i * step
        cubes.append(_box(f"wedge_{i}", (-hw, y0, z0), (hw, y1, z0 + step), g, "bone_main"))
    cubes.append(_box("wedge_ridge", (-0.25, y1, WEDGE_Z_FAR), (0.25, y1 + 0.5, WEDGE_Z_NEAR), g, "bone_dark"))
    if flip:
        return _mirror_z(cubes, -single_z)
    return _mirror_z(cubes, single_z)


# =============================================================================
# 部件 4：四根骨柱（替代原来的 L 形骨块）
# =============================================================================

def corner_posts() -> List[dict]:
    cubes = []
    for sx in (-1, 1):
        for sz in (-1, 1):
            cubes.extend(
                bone_post(
                    sx * POST_CENTER,
                    sz * POST_CENTER,
                    y_bot=0.0,
                    height=POST_HEIGHT,
                    group="bone_posts",
                    pfx=f"post_{sx:+d}{sz:+d}",
                )
            )
    return cubes


# =============================================================================
# 整件拼装
# =============================================================================

def parts_for(single_z: int) -> Dict[str, List[dict]]:
    """返回按部件名分组的立方体，合流器 single_z=+1，分流器 -1。"""
    single_dir = "+z" if single_z > 0 else "-z"
    return {
        "arm_east": arm_module("+x", "arm_e"),
        "arm_west": arm_module("-x", "arm_w"),
        "arm_single": arm_module(single_dir, "arm_s"),
        "chamber": center_chamber(("+x", "-x", single_dir), single_z) + glow_cross(single_z) + chimney(single_z),
        "wedge": bone_wedge(single_z),
        "posts": corner_posts(),
    }


def all_cubes(single_z: int) -> List[dict]:
    cubes: List[dict] = []
    for part in parts_for(single_z).values():
        cubes.extend(part)
    return cubes


# =============================================================================
# 门禁（每道都配注入器，先造出缺陷确认报得出来）
# =============================================================================

def _bounds(cubes: List[dict]) -> Tuple[List[float], List[float]]:
    lo = [min(c["from"][i] for c in cubes) for i in range(3)]
    hi = [max(c["to"][i] for c in cubes) for i in range(3)]
    return lo, hi


def check_structure(cubes: List[dict], single_z: int) -> None:
    """整件结构门禁：外包、臂数、骨节数、骨柱高度与位置、骨楔尖朝向、肌肉肋交替。"""
    lo, hi = _bounds(cubes)
    assert lo[0] >= -8 and hi[0] <= 8 and lo[2] >= -8 and hi[2] <= 8, f"x/z 越出方块 ±8: {lo} {hi}"
    assert lo[1] >= 0 and hi[1] <= 16, f"y 越出 0..16: {lo[1]} {hi[1]}"

    ribs = [c for c in cubes if "_rib_" in c["name"]]
    assert len(ribs) == 9, f"臂段肌肉肋应为 3 臂 × 3 道，实际 {len(ribs)}"
    for arm in ("arm_e", "arm_w", "arm_s"):
        mats = [c["material"] for c in sorted((r for r in ribs if r["name"].startswith(arm)), key=lambda r: r["name"])]
        assert mats == ["flesh_main", "flesh_dark", "flesh_main"], f"{arm} 肋未红/暗红交替: {mats}"
    knots = [c for c in cubes if "_knot_" in c["name"]]
    assert len(knots) == 12, f"小骨节应为 3 臂 × 4，实际 {len(knots)}"

    posts = [c for c in cubes if c["name"].endswith("_body") and c["name"].startswith("post_")]
    assert len(posts) == 4, f"骨柱应为 4 根，实际 {len(posts)}"
    for p in posts:
        assert p["to"][1] <= CAVITY_TOP, f"骨柱 {p['name']} 高出腔体: {p['to'][1]}"
        cx = (p["from"][0] + p["to"][0]) / 2
        cz = (p["from"][2] + p["to"][2]) / 2
        assert abs(abs(cx) - POST_CENTER) < 1e-6 and abs(abs(cz) - POST_CENTER) < 1e-6, f"骨柱 {p['name']} 不在腔体四角: ({cx},{cz})"

    # 骨楔尖朝向：最窄层的 z 中心应朝向单口一侧
    wedge = sorted((c for c in cubes if c["name"].startswith("wedge_") and c["name"] != "wedge_ridge"), key=lambda c: c["from"][2])
    widest = max(wedge, key=lambda c: c["to"][0] - c["from"][0])
    narrow = min(wedge, key=lambda c: c["to"][0] - c["from"][0])
    wz = (widest["from"][2] + widest["to"][2]) / 2
    nz = (narrow["from"][2] + narrow["to"][2]) / 2
    assert (nz - wz) * single_z > 0, f"骨楔尖没朝单口侧(single_z={single_z}): 宽端 z={wz}, 尖 z={nz}"


def self_test(single_z: int, label: str) -> None:
    cubes = all_cubes(single_z)
    _assert_no_coplanar_faces(cubes)
    check_structure(cubes, single_z)
    print(f"  [OK] {label}: {len(cubes)} 个立方体，共面/外包/臂段/骨节/骨柱/骨楔朝向全部通过")

    def must_fail(name: str, bad: List[dict], gate) -> None:
        try:
            gate(bad)
        except AssertionError as e:
            print(f"  [OK] 注入「{name}」被拦截: {str(e).splitlines()[0][:60]}")
            return
        raise RuntimeError(f"门禁失效: 注入「{name}」没被拦截")

    must_fail("共面", cubes + [dict(cubes[0], name="dup")], _assert_no_coplanar_faces)

    flipped = [c for c in cubes if not c["name"].startswith("wedge_")] + bone_wedge(single_z, flip=True)
    must_fail("骨楔尖朝反", flipped, lambda cs: check_structure(cs, single_z))

    tall = [dict(c, to=[c["to"][0], 12.0, c["to"][2]]) if c["name"] == "post_+1+1_body" else c for c in cubes]
    must_fail("骨柱高过腔体", tall, lambda cs: check_structure(cs, single_z))

    no_arm = [c for c in cubes if not c["name"].startswith("arm_w")]
    must_fail("少一条臂", no_arm, lambda cs: check_structure(cs, single_z))

    outside = cubes + [_box("far", (7.0, 0.0, 7.0), (9.0, 1.0, 9.0), "x", "bone_main")]
    must_fail("越出方块", outside, lambda cs: check_structure(cs, single_z))


# =============================================================================
# 导出与渲染
# =============================================================================

def build_texture() -> Image.Image:
    im = Image.new("RGBA", (RES, RES), (0, 0, 0, 0))
    rng = np.random.RandomState(42)
    for mat, (u0, v0, u1, v1) in MAT_UV.items():
        w, h = u1 - u0, v1 - v0
        tile = np.zeros((h, w, 4), dtype=np.uint8)
        tile[:, :] = PALETTE[mat]
        noise = rng.randint(-8, 8, size=(h, w))
        for ch in range(3):
            tile[:, :, ch] = np.clip(tile[:, :, ch].astype(int) + noise, 0, 255)
        im.paste(Image.fromarray(tile, mode="RGBA"), (u0, v0))
    return im


def write_bbmodel(out_path: Path, name: str, cubes: List[dict]) -> Path:
    out_path.parent.mkdir(parents=True, exist_ok=True)
    buf = io.BytesIO()
    build_texture().save(buf, format="PNG")
    src = "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode("ascii")

    elements, groups = [], {}
    for c in cubes:
        uv = MAT_UV[c["material"]]
        elem = {
            "name": c["name"],
            "box_uv": False,
            "from": c["from"],
            "to": c["to"],
            "faces": {s: {"uv": uv, "texture": 0} for s in ("north", "south", "east", "west", "up", "down")},
            "uuid": str(uuid.uuid4()),
        }
        elements.append(elem)
        groups.setdefault(c.get("group", name), []).append(elem["uuid"])

    bb = {
        "meta": {"format_version": "4.10", "model_format": "free"},
        "name": name,
        "resolution": {"width": RES, "height": RES},
        "elements": elements,
        "outliner": [{"name": g, "origin": [0.0, 0.0, 0.0], "children": u} for g, u in groups.items()],
        "textures": [{"name": name, "folder": "block", "namespace": "bong", "id": 0, "source": src}],
    }
    out_path.write_text(json.dumps(bb, indent=2), encoding="utf-8")
    return out_path


def render_review(piece: str, single_z: int, ref_name: str, tmp_dir: Path) -> None:
    """出 render.png（四视图）、parts.png（分部件 3/4）、check.png（左爆炸参考 / 右 3/4 + 俯视）。"""
    from bbmodel_maker.render.render_bbmodel import render

    out_dir = REVIEW_ROOT / piece
    out_dir.mkdir(parents=True, exist_ok=True)
    tmp_dir.mkdir(parents=True, exist_ok=True)
    bg = (119, 119, 119)

    model = write_bbmodel(MODEL_DIR / f"{piece}.bbmodel", piece, all_cubes(single_z))
    iso, _ = render(model, yaw=-35.0, pitch=30.0, size=500, bg=bg)
    top, _ = render(model, yaw=0.0, pitch=89.9, size=500, bg=bg)
    side, _ = render(model, yaw=90.0, pitch=0.0, size=500, bg=bg)
    rear, _ = render(model, yaw=145.0, pitch=30.0, size=500, bg=bg)

    canvas = Image.new("RGB", (1030, 1030), (35, 36, 40))
    d = ImageDraw.Draw(canvas)
    for title, im, x, y in [("3/4", iso, 10, 10), ("TOP", top, 520, 10), ("SIDE", side, 10, 520), ("3/4 REAR (yaw 145)", rear, 520, 520)]:
        canvas.paste(im, (x, y))
        d.text((x + 8, y + 6), title, fill=(235, 235, 235))
    canvas.save(out_dir / "render.png")

    # 分部件：每件单独出 3/4
    thumbs = []
    for pname, pcubes in parts_for(single_z).items():
        p = write_bbmodel(tmp_dir / f"{piece}_{pname}.bbmodel", pname, pcubes)
        im, _ = render(p, yaw=-35.0, pitch=30.0, size=360, bg=bg)
        thumbs.append((pname, im))
    cols = 3
    rows = (len(thumbs) + cols - 1) // cols
    sheet = Image.new("RGB", (cols * 370 + 10, rows * 370 + 10), (35, 36, 40))
    sd = ImageDraw.Draw(sheet)
    for i, (pname, im) in enumerate(thumbs):
        x, y = 10 + (i % cols) * 370, 10 + (i // cols) * 370
        sheet.paste(im, (x, y))
        sd.text((x + 8, y + 6), pname, fill=(235, 235, 235))
    sheet.save(out_dir / "parts.png")

    ref_path = REVIEW_ROOT / "exploded" / ref_name
    ref = Image.open(ref_path).convert("RGB")
    h = 600
    ref = ref.resize((int(ref.width * h / ref.height), h), Image.Resampling.LANCZOS)
    iso_s = iso.resize((h, h), Image.Resampling.LANCZOS)
    top_s = top.resize((h, h), Image.Resampling.LANCZOS)
    check = Image.new("RGB", (ref.width + 2 * h + 36, h + 40), (28, 29, 33))
    cd = ImageDraw.Draw(check)
    check.paste(ref, (12, 30))
    check.paste(iso_s, (ref.width + 24, 30))
    check.paste(top_s, (ref.width + h + 36, 30))
    cd.text((16, 8), f"REFERENCE {ref_name}", fill=(210, 200, 180))
    cd.text((ref.width + 28, 8), "3/4", fill=(180, 220, 210))
    cd.text((ref.width + h + 40, 8), "TOP", fill=(180, 220, 210))
    check.save(out_dir / "check.png")
    print(f"✓ {piece}: render.png / parts.png / check.png -> {out_dir}")
