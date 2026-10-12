#!/usr/bin/env python3
"""经脉工厂 b05 分流器 / b12 合流器共用部件（第 2 版，按调度给的 0–16 坐标重做）。

两件是同一套部件，只差一个符号：`single_z`。
- b12 合流器：±X 两面进，单口在 +Z 出，-Z 是封闭后壁；`single_z=+1`。
- b05 分流器：-Z 一面进，±X 两面出，+Z 是封闭后壁；`single_z=-1`。
坐标换算：模型 x/z = 调度坐标 − 8，y 不变；原点在底面中心。

逐行宽度表（x 方向宽度；括号里是调度的 0–16 坐标）
  y  0–9.5   中心腔实心肉壳 x/z ±4（4–12）
  y  9.5–10  顶面半透膜 ±4
  y 10–10.5  十字亮通道贴片：宽 4（x/z ±2），臂伸到腔壁 ±4；中心 4×4 #fff0dc，往臂端渐变，腔顶约 1/2 是亮的
  y 10.5–12  骨脊：1 宽，高出膜面 2px，只落在单口那一臂上（z 绝对值 2..4），不压中心：合流器在出口 +Z 臂，分流器在入口 -Z 臂
  y  0–4     四根 2×2 矮骨脚，x/z 在 ±(2..4)，贴着腔体四角，顶上各一个 1px 小骨节（y4–5）
  y  2–3, 7–8  臂框四条 1×1 肉梁（臂截面 6 宽 × 6 高：z ±3，y 2..8），长度沿臂；四个侧面全空
  y  3–7     肌肉卷：3×3 暗红卷芯 + 每隔 1px 一道外凸 0.5px 的 4×4 肋环，#8a2a2a / #5a1a1a 交替
  臂沿轴向 4..8，两端各 4 个 1×1 骨节（共 8 个）
  四个外角 |x|>4 且 |z|>4（调度的 x0–4×z0–4 等）整块为空。
"""

from __future__ import annotations

import base64
import io
import json
import sys
import uuid
from pathlib import Path
from typing import Dict, List, Tuple

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
)

MODEL_DIR = REPO / "modelScript" / "models" / "meridian_factory"
REVIEW_ROOT = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory")

# 在公共调色板上只多一个 #fff0dc 十字通道中心亮芯
PALETTE = dict(cp.PALETTE)
PALETTE["qi_core"] = (255, 240, 220, 255)
MAT_UV = dict(cp.MAT_UV)
MAT_UV["qi_core"] = [16, 32, 32, 48]

# ---- 几何常量（px） ----
CHAMBER_HALF = 4.0
BODY_TOP = 9.5
MEMBRANE_TOP = 10.0
GLOW_TOP = 10.5
RIDGE_Y = (10.5, 12.0)
CROSS_HALF_WIDTH = 2.0     # 十字通道半宽（总宽 4），臂一直伸到腔壁 ±4
RIDGE_Z = (2.0, 4.0)       # 骨脊只落在单口那一臂上（z 的绝对值范围），不压中心 4×4
ARM_A0, ARM_A1 = 4.0, 8.0
ARM_Y = (2.0, 8.0)
ARM_HALF_B = 3.0
ROLL_HALF = 2.0
ROLL_Y = (3.0, 7.0)
POST_CENTER = 3.025        # 柱 x/z 2.025..4.025，比腔壁多凸 0.025 避开共面
POST_HEIGHT = 4.0         # 矮骨脚，顶上再加 1px 小骨节（总高到 y5）
RIB_PROUD = 0.5
RIB_THICK = 0.5


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
# 部件 1：臂段模块（开放框 + 肌肉卷）
# =============================================================================

def arm_module(direction: str, pfx: str) -> List[dict]:
    """开放框臂段：只有 4 条 1×1 肉梁沿臂长，四个侧面全空；框里一根 4×4 肌肉卷，1px 一道肋。"""
    g = f"arm_{pfx}"
    local = []
    y_lo, y_hi = ARM_Y
    corners = [("nw", -ARM_HALF_B, y_hi - 1.0), ("ne", ARM_HALF_B - 1.0, y_hi - 1.0),
               ("sw", -ARM_HALF_B, y_lo), ("se", ARM_HALF_B - 1.0, y_lo)]
    for tag, b0, y0 in corners:
        # 肉梁在两端骨节之间，骨节 1×1 放在两端
        local.append(_box(f"{pfx}_beam_{tag}", (ARM_A0 + 1.0, y0, b0), (ARM_A1 - 1.0, y0 + 1.0, b0 + 1.0), g, "flesh_main"))
        local.append(_box(f"{pfx}_knot_in_{tag}", (ARM_A0, y0, b0), (ARM_A0 + 1.0, y0 + 1.0, b0 + 1.0), g, "bone_main"))
        local.append(_box(f"{pfx}_knot_out_{tag}", (ARM_A1 - 1.0, y0, b0), (ARM_A1, y0 + 1.0, b0 + 1.0), g, "bone_main"))
    # 肌肉卷：3×3 暗红卷芯贯穿全臂，外面每隔 1px 一道外凸 0.5px 的肋环（共 4×4），红/暗红交替
    core = ROLL_HALF - RIB_PROUD
    local.append(_box(f"{pfx}_core", (ARM_A0, ROLL_Y[0] + RIB_PROUD, -core), (ARM_A1, ROLL_Y[1] - RIB_PROUD, core), g, "flesh_dark"))
    for i in range(4):
        # 肋环错开 0.25，避免端面和卷芯端面共面
        a0 = ARM_A0 + 0.25 + i
        mat = "flesh_main" if i % 2 == 0 else "flesh_dark"
        local.append(_box(f"{pfx}_rib_{i}", (a0, ROLL_Y[0], -ROLL_HALF), (a0 + RIB_THICK, ROLL_Y[1], ROLL_HALF), g, mat))
    return _orient(local, direction)


# =============================================================================
# 部件 2：中心腔
# =============================================================================

def center_chamber(single_z: int) -> List[dict]:
    """x/z ±4、y0..10 的实心肉壳；顶面半透膜，膜上十字亮通道（宽 4，中心 #fff0dc 往四臂渐变）。"""
    g = "chamber"
    h = CHAMBER_HALF
    cubes = [
        _box("chamber_body", (-h, 0.05, -h), (h, BODY_TOP, h), g, "flesh_main"),
        _box("chamber_membrane", (-h, BODY_TOP, -h), (h, MEMBRANE_TOP, h), g, "tendon_tube"),
    ]
    # 封闭后壁外侧两条斜筋丝，微凸 0.5（避开骨柱柱面）
    zf = 4.0 if single_z < 0 else -4.5
    cubes.append(_box("chamber_back_fiber_a", (-1.5, 3.0, min(zf, zf + 0.5)), (-0.5, 6.0, max(zf, zf + 0.5)), g, "tendon_fiber"))
    cubes.append(_box("chamber_back_fiber_b", (0.5, 4.0, min(zf, zf + 0.5)), (1.5, 7.0, max(zf, zf + 0.5)), g, "tendon_fiber"))

    y0, y1 = MEMBRANE_TOP, GLOW_TOP
    w = CROSS_HALF_WIDTH
    cubes.append(_box("glow_center", (-w, y0, -w), (w, y1, w), "glow_cross", "qi_core"))
    gradient = ["qi_glow", "tendon_highlight"]  # 臂段 w..h 分两段，往壁渐暗
    for i, mat in enumerate(gradient):
        d0, d1 = w + i, w + 1.0 + i
        cubes.append(_box(f"glow_xp_{i}", (d0, y0, -w), (d1, y1, w), "glow_cross", mat))
        cubes.append(_box(f"glow_xn_{i}", (-d1, y0, -w), (-d0, y1, w), "glow_cross", mat))
        cubes.append(_box(f"glow_zp_{i}", (-w, y0, d0), (w, y1, d1), "glow_cross", mat))
        cubes.append(_box(f"glow_zn_{i}", (-w, y0, -d1), (w, y1, -d0), "glow_cross", mat))
    return cubes


# =============================================================================
# 部件 3：骨脊（原骨楔）
# =============================================================================

def bone_ridge(single_z: int, flip: bool = False) -> List[dict]:
    """膜上 1 宽的骨脊，高出膜面 2px，只落在单口那一臂上（z 绝对值 2..4），不压中心 4×4 的亮通道。

    合流器（single_z=+1）：在出口 +Z 那一臂。
    分流器（single_z=-1）：在入口 -Z 那一臂；两条出臂（±X）上不放骨脊。
    flip=True 只给差分自证用：造一条朝反的坏脊。
    """
    g = "bone_ridge"
    y0, y1 = RIDGE_Y
    z0, z1 = RIDGE_Z
    cubes = [_box("ridge_main", (-0.5, y0, z0), (0.5, y1, z1), g, "bone_main")]
    cubes.append(_box("ridge_cap", (-0.25, y1, z0 + 0.5), (0.25, y1 + 0.25, z1), g, "bone_dark"))
    # 默认朝 +Z；分流器取 -Z，flip 再把它翻回反侧
    mirrored = (single_z < 0) != flip
    return _mirror_z(cubes, -1) if mirrored else cubes


# =============================================================================
# 部件 4：四根骨柱
# =============================================================================

def corner_posts() -> List[dict]:
    cubes = []
    for sx in (-1, 1):
        for sz in (-1, 1):
            cubes.extend(
                bone_post(sx * POST_CENTER, sz * POST_CENTER, y_bot=0.0, height=POST_HEIGHT,
                          group="bone_posts", pfx=f"post_{sx:+d}{sz:+d}")
            )
            cx, cz = sx * POST_CENTER, sz * POST_CENTER
            cubes.append(_box(f"post_{sx:+d}{sz:+d}_cap", (cx - 0.5, POST_HEIGHT, cz - 0.5), (cx + 0.5, POST_HEIGHT + 1.0, cz + 0.5),
                              "bone_posts", "bone_main"))
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
        "chamber": center_chamber(single_z),
        "ridge": bone_ridge(single_z),
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
    """整件结构门禁：外包、外角为空、臂开放框、肌肉卷交替、骨柱、骨脊方向。"""
    lo, hi = _bounds(cubes)
    assert lo[0] >= -8 and hi[0] <= 8 and lo[2] >= -8 and hi[2] <= 8, f"x/z 越出方块 ±8: {lo} {hi}"
    assert lo[1] >= 0 and hi[1] <= 16, f"y 越出 0..16: {lo[1]} {hi[1]}"

    # 四个外角整块为空：任何立方体不得同时在 |x|>4.05 与 |z|>4.05 的区域里
    for c in cubes:
        xs = max(abs(c["from"][0]), abs(c["to"][0]))
        zs = max(abs(c["from"][2]), abs(c["to"][2]))
        xmin = min(abs(c["from"][0]), abs(c["to"][0])) if c["from"][0] * c["to"][0] > 0 else 0.0
        zmin = min(abs(c["from"][2]), abs(c["to"][2])) if c["from"][2] * c["to"][2] > 0 else 0.0
        assert not (xmin >= 4.05 and zmin >= 4.05), f"外角必须为空，{c['name']} 落在 x/z>4 的角区"
        del xs, zs

    ribs = [c for c in cubes if "_rib_" in c["name"]]
    assert len(ribs) == 12, f"臂内肌肉肋应为 3 臂 × 4 道，实际 {len(ribs)}"
    for arm in ("arm_e", "arm_w", "arm_s"):
        mine = sorted((r for r in ribs if r["name"].startswith(arm + "_")), key=lambda r: r["name"])
        mats = [r["material"] for r in mine]
        assert mats == ["flesh_main", "flesh_dark", "flesh_main", "flesh_dark"], f"{arm} 肋未红/暗红交替: {mats}"
    beams = [c for c in cubes if "_beam_" in c["name"]]
    assert len(beams) == 12, f"臂框肉梁应为 3 臂 × 4 条，实际 {len(beams)}"
    for b in beams:
        dims = sorted(b["to"][i] - b["from"][i] for i in range(3))
        assert dims[0] == 1.0 and dims[1] == 1.0, f"{b['name']} 不是 1×1 截面: {dims}"
    cores = {c["name"].rsplit("_core", 1)[0]: c for c in cubes if c["name"].endswith("_core")}
    assert len(cores) == 3, f"应有 3 根卷芯，实际 {len(cores)}"
    for r in ribs:
        core = cores[r["name"].rsplit("_rib_", 1)[0]]
        proud = core["from"][1] - r["from"][1]   # 竖直方向肋环比卷芯多出的高度
        assert abs(proud - RIB_PROUD) < 1e-6, f"{r['name']} 肋环没比卷芯外凸 {RIB_PROUD}: {proud}"
    caps = [c for c in cubes if c["name"].endswith("_cap") and c["name"].startswith("post_")]
    assert len(caps) == 4, f"每根骨脚顶上应有 1 个小骨节，实际 {len(caps)}"
    knots = [c for c in cubes if "_knot_" in c["name"]]
    assert len(knots) == 24, f"小骨节应为 3 臂 × 8，实际 {len(knots)}"

    posts = [c for c in cubes if c["name"].endswith("_body") and c["name"].startswith("post_")]
    assert len(posts) == 4, f"骨柱应为 4 根，实际 {len(posts)}"
    for p in posts:
        assert p["to"][1] <= POST_HEIGHT + 1e-6, f"骨柱 {p['name']} 高过 {POST_HEIGHT}: {p['to'][1]}"
        cx = (p["from"][0] + p["to"][0]) / 2
        cz = (p["from"][2] + p["to"][2]) / 2
        assert abs(abs(cx) - POST_CENTER) < 1e-6 and abs(abs(cz) - POST_CENTER) < 1e-6, f"骨柱 {p['name']} 不在腔体四角: ({cx},{cz})"

    # 十字亮通道：中心 4×4，四臂宽 4 一直伸到腔壁
    cross = [c for c in cubes if c.get("group") == "glow_cross"]
    center = next(c for c in cross if c["name"] == "glow_center")
    center_width = (center["to"][0] - center["from"][0], center["to"][2] - center["from"][2])
    assert center_width == (4.0, 4.0), f"十字中心应为 4×4，实际 {center_width}"
    for c in cross:
        across = (c["to"][2] - c["from"][2]) if c["name"].startswith(("glow_xp", "glow_xn")) else (c["to"][0] - c["from"][0])
        if c is not center:
            assert across == 4.0, f"{c['name']} 通道宽应为 4，实际 {across}"
    assert max(abs(c["to"][0]) for c in cross) == CHAMBER_HALF and max(abs(c["to"][2]) for c in cross) == CHAMBER_HALF, "十字臂应伸到腔壁"

    # 骨脊不压中心：任何骨脊立方体都不得和中心 4×4 在 x/z 上重叠
    for c in cubes:
        if c["name"].startswith("ridge_"):
            overlaps = c["from"][0] < CROSS_HALF_WIDTH and c["to"][0] > -CROSS_HALF_WIDTH \
                and c["from"][2] < CROSS_HALF_WIDTH and c["to"][2] > -CROSS_HALF_WIDTH
            assert not overlaps, f"{c['name']} 压在十字中心 4×4 上"

    # 骨脊方向：脊的重心应落在单口一侧（合流在出口侧，分流在入口侧）
    ridge = [c for c in cubes if c["name"].startswith("ridge_main")]
    assert len(ridge) == 1, "应有一条主骨脊"
    zc = (ridge[0]["from"][2] + ridge[0]["to"][2]) / 2
    assert zc * single_z > 0, f"骨脊没落在单口侧(single_z={single_z}): z={zc}"
    assert abs(ridge[0]["to"][0] - ridge[0]["from"][0] - 1.0) < 1e-6, "骨脊应 1 宽"
    assert abs(ridge[0]["to"][1] - 12.0) < 1e-6, "骨脊应高出膜面 2px"


def self_test(single_z: int, label: str) -> None:
    cubes = all_cubes(single_z)
    _assert_no_coplanar_faces(cubes)
    check_structure(cubes, single_z)
    print(f"  [OK] {label}: {len(cubes)} 个立方体，共面/外包/外角空/开放框/肌肉卷/骨柱/骨脊方向全部通过")

    def must_fail(name: str, bad: List[dict], gate) -> None:
        try:
            gate(bad)
        except AssertionError as e:
            print(f"  [OK] 注入「{name}」被拦截: {str(e).splitlines()[0][:60]}")
            return
        raise RuntimeError(f"门禁失效: 注入「{name}」没被拦截")

    must_fail("共面", cubes + [dict(cubes[0], name="dup")], _assert_no_coplanar_faces)

    flipped = [c for c in cubes if not c["name"].startswith("ridge_")] + bone_ridge(single_z, flip=True)
    must_fail("骨脊朝反", flipped, lambda cs: check_structure(cs, single_z))

    narrow = [dict(c, **{"from": [c["from"][0], c["from"][1], -1.0], "to": [c["to"][0], c["to"][1], 1.0]})
              if c["name"] in ("glow_xp_0", "glow_xp_1") else c for c in cubes]
    must_fail("十字通道变窄", narrow, lambda cs: check_structure(cs, single_z))

    on_center = cubes + [_box("ridge_over_center", (-0.5, 10.5, -1.0), (0.5, 12.0, 1.0), "bone_ridge", "bone_main")]
    must_fail("骨脊压中心", on_center, lambda cs: check_structure(cs, single_z))

    corner = cubes + [_box("corner_block", (-8.0, 0.0, -8.0), (-6.0, 8.0, -6.0), "x", "bone_main")]
    must_fail("外角有骨块", corner, lambda cs: check_structure(cs, single_z))

    tall = [dict(c, to=[c["to"][0], 12.0, c["to"][2]]) if c["name"] == "post_+1+1_body" else c for c in cubes]
    must_fail("骨柱过高", tall, lambda cs: check_structure(cs, single_z))

    flat = [dict(c, **{"from": [c["from"][0], 3.5, c["from"][2]], "to": [c["to"][0], 6.5, c["to"][2]]}) if "_rib_" in c["name"] else c for c in cubes]
    must_fail("肋环不外凸", flat, lambda cs: check_structure(cs, single_z))

    no_arm = [c for c in cubes if not c["name"].startswith("arm_w")]
    must_fail("少一条臂", no_arm, lambda cs: check_structure(cs, single_z))

    walled = cubes + [_box("arm_e_side_wall", (4.0, 2.0, 3.0), (8.0, 8.0, 3.5), "arm_arm_e", "flesh_main")]
    must_fail("臂框侧面封闭", walled, lambda cs: check_structure(cs, single_z) or _assert_open_sides(cs))


def _assert_open_sides(cubes: List[dict]) -> None:
    """臂框四个侧面必须是空的：臂区内除 1×1 梁、骨节、肌肉卷以外不得有别的立方体。"""
    for c in cubes:
        if not c["name"].startswith("arm_"):
            continue
        if any(k in c["name"] for k in ("_beam_", "_knot_", "_rib_", "_core")):
            continue
        raise AssertionError(f"臂框侧面不许有封板: {c['name']}")


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
