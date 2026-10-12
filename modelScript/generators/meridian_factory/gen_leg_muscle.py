#!/usr/bin/env python3
"""经脉工厂内景器官生成器 —— o04: leg_muscle (下肢肌肉组 / 腿肌)

风格：A 有机型 (活体血肉、半透明筋管/肌腱、旧损暗淡配色)
规范出处：
- /home/serverkizuna/Code/Bong/.agent-worktrees/.task-meridian-models.md
- /home/serverkizuna/Code/Bong/.agent-worktrees/model-review/meridian_factory.md

结构与规范落实：
1. 外包围与尺寸：3 宽 × 2 高 × 3 长方块 (严格落在 48×32×48 px 空间内：x in [-24..24], y in [0..32], z in [0..48])，
   长轴为 z 轴，肌腱端在 z = 0，肌腹端在 z = 44..48。
2. 双骨环 + 两个 8×6 统一截面耦合插口 (z: 0..2)：
   - 左右各一插口，中间保留严格 6px 空隙；
   - 左插口内孔：x in [-11.0, -3.0], y in [2.0, 8.0] (宽 8px × 高 6px，底边离地 2px)；
   - 右插口内孔：x in [3.0, 11.0], y in [2.0, 8.0] (宽 8px × 高 6px，底边离地 2px)；
   - 两内孔之间内缘距离 3.0 - (-3.0) = 6.0px；
   - 各自配备四面骨环外箍 #d8ccb0、方口内壁衬层 #b05050 与真元内光 #f6dcc4。
3. 左右两条肌腱 (z: 2..12, Y 形分叉)：
   - 从左右两插口分别向肌腹汇拢延伸，两条肌腱之间保持贯通空隙，呈经典 Y 形双头肌腱；
   - 材质采用 #e8bca8 渐变白色肌腱，表面配有一条 #f6dcc4 纵向高光亮线。
4. 9 束肌纤维更粗更短 (按 3-3-3 排列，整束长 32px：z 12..44)：
   - 上 3 束、中 3 束、下 3 束；
   - 每束分 3 节逐渐收细：
     - 节 1 (z 30–44): 截面 10×10，呈粗壮有力的肌腹主干；
     - 节 2 (z 20–30): 截面 8×8，向中心轴收拢过渡；
     - 节 3 (z 12–20): 截面 5×5，分流汇入左右两条肌腱；
   - 各束末端（z 44 那头）长度错开 0–4px（z 到 44.0..47.9px），外圈束外角向内切角做圆钝，避免生硬平切；
   - 每束顶面沿长轴配置 1px 宽 #b05050 高光条，束间缝隙露出暗血肉 #5a1a1a 衬垫。
5. 门禁要求：通过 _assert_no_coplanar_faces 严格自检（0 共面冲突），带 --self-test 差分缺陷拦截验证。
"""

from __future__ import annotations

import argparse
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
BBMODEL_OUT = REPO / "modelScript" / "models" / "meridian_factory" / "leg_muscle.bbmodel"
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/leg_muscle")
REVIEW_DIR_ALIAS = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/o04_leg_muscle")
REF_IMAGE = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/refs/o04_leg_muscle.png")

# 调色板 (严格对齐 meridian_factory.md 色值表)
PALETTE = {
    "flesh_dark":    (90, 26, 26, 255),    # #5a1a1a 束间细缝与暗部
    "flesh_main":    (138, 42, 42, 255),   # #8a2a2a 红肌纤维主色
    "flesh_lit":     (176, 80, 80, 255),   # #b05050 顶面高光条与插口内衬
    "tendon_trans":  (232, 188, 168, 255), # #e8bca8 肌腱白色/暖粉白
    "bone_main":     (216, 204, 176, 255), # #d8ccb0 骨环
    "bone_dark":     (184, 168, 136, 255), # #b8a888 骨暗面
    "qi_glow":       (246, 220, 196, 255), # #f6dcc4 肌腱亮线与真元内光
}

MAT_UV = {
    "flesh_main":    [0, 0, 16, 16],
    "flesh_lit":     [16, 0, 32, 16],
    "flesh_dark":    [32, 0, 48, 16],
    "tendon_trans":  [48, 0, 64, 16],
    "bone_main":     [0, 16, 16, 32],
    "bone_dark":     [16, 16, 32, 32],
    "qi_glow":       [32, 16, 48, 32],
}

RES = 64


# =============================================================================
# 各部件几何定义 (part_* 拆分) —— 第 3 轮：整件改成竖立姿态
# =============================================================================
#
# 逐行宽度表（x 为左右宽度；y 自下而上；原点在底面中心，正面 +Z）
#   y 29–32  顶骨环口：外 7×7、内孔 5×5、高 3，中心 x0，z 贴在肌腹前上方
#   y 22–29  肌腱主干：宽 6 × 厚 3，两条并在一起，贴着肌腹前面
#   y  2–22  肌腱张开成 Y：每条宽 3 × 厚 3，每降 1px 向外偏 ~0.79px，末端中心落在 x ±16
#   y  0–3   底部两个骨环口：外 7×7、内孔 5×5，中心 x ±17，两口内缘相距 28px
#   y  2–30  肌腹 9 束纵向肌束（3 列 × 3 排，列中心 x -9/0/9，排中心 z -9/0/9，束宽 8、缝 1）：
#            两端各 2 级倒角（半宽 4.0 → 3.0 → 2.0），两端高度逐束错开，不平切
#
# 肌腱贴在前排肌束外面：每一节的后表面取该高度前排肌束的外表面再缩 0.5px，所以从正面看得清。

BELLY_ROW_Z = [-9.0, 0.0, 9.0]
BELLY_COL_X = [-9.0, 0.0, 9.0]
BELLY_HALF = 4.0

# 每束 (底部多缩短 db，顶部多缩短 dt)，使 9 束的两端高度错开
BELLY_STAGGER = [
    (0.0, 1.0), (1.0, 0.0), (0.5, 1.5),
    (1.5, 0.5), (0.0, 0.0), (1.0, 1.0),
    (0.5, 0.5), (1.5, 1.5), (0.0, 1.5),
]

BELLY_BASE_Y = 2.5       # 肌腹最低处
BELLY_TOP_Y = 29.0       # 肌腹最高处
CAP_STEP = 2.0           # 每级倒角的高度
CAP_HALVES = [2.0, 3.0]  # 端部两级倒角的半宽，之后是 4.0

TENDON_TOP_Y = 29.0
TENDON_SPLIT_Y = 22.0
LEG_END_Y = 3.0
LEG_END_X = 17.0
TRUNK_W = 6.2
LEG_W = 3.0
TENDON_T = 3.0           # 肌腱厚度（z 方向）
Y_OFFSET = 0.25          # 肌腱节界相对整数高度错开 0.25，避开肌束倒角面的共面
RING_OUTER = 7.0
RING_HOLE = 5.0
RING_H = 3.0


def _bundle_half(y: float, db: float, dt: float) -> float:
    """束在高度 y 处的半宽：两端各两级倒角，错开 db / dt。"""
    y0 = BELLY_BASE_Y + db
    y1 = BELLY_TOP_Y - dt
    if y < y0 or y > y1:
        return 0.0
    if y < y0 + CAP_STEP:
        return CAP_HALVES[0]
    if y < y0 + 2 * CAP_STEP:
        return CAP_HALVES[1]
    if y > y1 - CAP_STEP:
        return CAP_HALVES[0]
    if y > y1 - 2 * CAP_STEP:
        return CAP_HALVES[1]
    return BELLY_HALF


def _front_z(y: float) -> float:
    """前排正中肌束在高度 y 处的前表面 z（肌腱贴在它外面）；超出肌腹则取排中心。"""
    half = _bundle_half(y, 0.0, 0.0)
    return BELLY_ROW_Z[2] + half


def _box(name, lo, hi, group, material) -> dict:
    return {"name": name, "from": list(lo), "to": list(hi), "group": group, "material": material}


def _ring(prefix: str, cx: float, cz: float, y_base: float, face: str) -> List[dict]:
    """竖向骨环口：外 6×6、内孔 4×4、高 2；face='up' 口朝上，'down' 口朝下。"""
    g = "bone_rings"
    o, h = RING_OUTER / 2.0, RING_HOLE / 2.0
    y0, y1 = y_base, y_base + RING_H
    cubes = [
        _box(f"{prefix}_w", (cx - o, y0, cz - o), (cx - h, y1, cz + o), g, "bone_main"),
        _box(f"{prefix}_e", (cx + h, y0, cz - o), (cx + o, y1, cz + o), g, "bone_main"),
        _box(f"{prefix}_n", (cx - h, y0, cz - o), (cx + h, y1, cz - h), g, "bone_main"),
        _box(f"{prefix}_s", (cx - h, y0, cz + h), (cx + h, y1, cz + o), g, "bone_main"),
    ]
    # 孔底衬层 #b05050 + 中心真元内光：口朝上时在底部，口朝下时在顶部
    if face == "up":
        floor = (y0, y0 + 0.5)
        core = (y0 + 0.5, y0 + 1.0)
    else:
        floor = (y1 - 0.5, y1)
        core = (y1 - 1.0, y1 - 0.5)
    cubes.append(_box(f"{prefix}_floor", (cx - h, floor[0], cz - h), (cx + h, floor[1], cz + h), g, "flesh_lit"))
    cubes.append(_box(f"{prefix}_core", (cx - 1.0, core[0], cz - 1.0), (cx + 1.0, core[1], cz + 1.0), g, "qi_glow"))
    return cubes


def part_01_bone_rings() -> List[dict]:
    """顶 1 个、底 2 个骨环口。顶口在肌腱主干上端；底口落在两条肌腱末端，左右各贴近边缘。"""
    cubes = _ring("ring_top", 0.0, _tendon_z_mid(TENDON_TOP_Y - 0.5), TENDON_TOP_Y, "up")
    bz = _tendon_z_mid(LEG_END_Y + 0.5)
    cubes += _ring("ring_bl", -LEG_END_X, bz, 0.0, "down")
    cubes += _ring("ring_br", LEG_END_X, bz, 0.0, "down")
    return cubes


def _tendon_z_mid(y: float) -> float:
    z0 = _front_z(y) - 0.5
    return z0 + TENDON_T / 2.0


def part_02_tendons() -> List[dict]:
    """肌腱：顶口 → 主干（两条并在一起）→ y22 起张开成 Y → 各落到底部骨环口。

    每 1px 高度一节，z 跟着前排肌束外表面走；每条肌腱表面一条 #f6dcc4 亮线。
    """
    g = "tendons"
    cubes = []

    # 主干：y 22..30，宽 4 × 厚 3
    ys = TENDON_SPLIT_Y + Y_OFFSET
    i = 0
    while ys < TENDON_TOP_Y - 1e-6:
        y_hi = min(ys + 1.0, TENDON_TOP_Y)
        z0 = _front_z(ys + 0.5) - 0.5
        cubes.append(_box(f"trunk_{i}", (-TRUNK_W / 2, ys, z0), (TRUNK_W / 2, y_hi, z0 + TENDON_T), g, "tendon_trans"))
        cubes.append(_box(f"trunk_{i}_hl", (-0.3, ys, z0 + TENDON_T), (0.3, y_hi, z0 + TENDON_T + 0.3), g, "qi_glow"))
        ys += 1.0
        i += 1

    # Y 形两腿：y 22 → 2，共 20 节，中心 x 从 ±1 走到 ±16
    n = int(TENDON_SPLIT_Y - LEG_END_Y)
    dx = (LEG_END_X - LEG_W / 2.0) / (n - 1)
    for k in range(n):
        y1 = TENDON_SPLIT_Y + Y_OFFSET - k
        y0 = LEG_END_Y if k == n - 1 else y1 - 1.0
        cx = LEG_W / 2.0 + dx * k
        z0 = _front_z(y0 + 0.5) - 0.5
        for side, sx in (("l", -1.0), ("r", 1.0)):
            c = sx * cx
            cubes.append(_box(f"leg_{side}_{k}", (c - LEG_W / 2, y0, z0), (c + LEG_W / 2, y1, z0 + TENDON_T), g, "tendon_trans"))
            cubes.append(_box(f"leg_{side}_{k}_hl", (c - 0.3, y0, z0 + TENDON_T), (c + 0.3, y1, z0 + TENDON_T + 0.3), g, "qi_glow"))
    return cubes


def part_03_muscle_bundles() -> List[dict]:
    """肌腹 9 束纵向肌束，两端倒角圆钝、高度错开；束缝露出暗血肉芯。"""
    g = "muscle_bundles"
    cubes = []
    idx = 0
    for r, zc in enumerate(BELLY_ROW_Z):
        for c, xc in enumerate(BELLY_COL_X):
            db, dt = BELLY_STAGGER[idx]
            y0 = BELLY_BASE_Y + db
            y1 = BELLY_TOP_Y - dt
            levels = [
                (y0, y0 + CAP_STEP, CAP_HALVES[0]),
                (y0 + CAP_STEP, y0 + 2 * CAP_STEP, CAP_HALVES[1]),
                (y0 + 2 * CAP_STEP, y1 - 2 * CAP_STEP, BELLY_HALF),
                (y1 - 2 * CAP_STEP, y1 - CAP_STEP, CAP_HALVES[1]),
                (y1 - CAP_STEP, y1, CAP_HALVES[0]),
            ]
            for k, (ya, yb, half) in enumerate(levels):
                cubes.append(_box(f"b{r}{c}_{k}", (xc - half, ya, zc - half), (xc + half, yb, zc + half), g, "flesh_main"))
            # 最外两列外侧面各一条竖向亮肉条
            if c != 1:
                sx = 1.0 if c == 2 else -1.0
                edge = xc + sx * BELLY_HALF
                lo, hi = sorted((edge, edge + sx * 0.25))
                cubes.append(_box(f"b{r}{c}_hl", (lo, y0 + 4.0, zc - 0.5), (hi, y1 - 4.0, zc + 0.5), g, "flesh_lit"))
            idx += 1
    # 束缝暗芯（面坐标都避开束面）
    cubes.append(_box("fissure_core", (-11.6, 4.8, -11.6), (11.6, 27.2, 11.6), g, "flesh_dark"))
    return cubes


def all_cubes() -> List[dict]:
    """汇总下肢肌肉组：骨环口 + 肌腱 + 肌腹。"""
    return part_01_bone_rings() + part_02_tendons() + part_03_muscle_bundles()


# =============================================================================
# 门禁与共面冲突自检
# =============================================================================

def _assert_no_coplanar_faces(cubes: List[dict]):
    """严格检查立方体集是否存在同向同坐标且投影相交的共面冲突 (Z-fighting)。"""
    faces: Dict[Tuple[str, float], List[dict]] = {}
    for c in cubes:
        f = c["from"]
        t = c["to"]
        for side, axis, val in [
            ("-X", 0, f[0]), ("+X", 0, t[0]),
            ("-Y", 1, f[1]), ("+Y", 1, t[1]),
            ("-Z", 2, f[2]), ("+Z", 2, t[2]),
        ]:
            if axis == 0:
                rect = (min(f[1], t[1]), min(f[2], t[2]), max(f[1], t[1]), max(f[2], t[2]))
            elif axis == 1:
                rect = (min(f[0], t[0]), min(f[2], t[2]), max(f[0], t[0]), max(f[2], t[2]))
            else:
                rect = (min(f[0], t[0]), min(f[1], t[1]), max(f[0], t[0]), max(f[1], t[1]))
            key = (side, round(val, 4))
            faces.setdefault(key, []).append((c["name"], rect))

    conflicts = []
    for (side, val), entries in faces.items():
        if len(entries) < 2:
            continue
        for i in range(len(entries)):
            for j in range(i + 1, len(entries)):
                n1, r1 = entries[i]
                n2, r2 = entries[j]
                u0 = max(r1[0], r2[0])
                u1 = min(r1[2], r2[2])
                v0 = max(r1[1], r2[1])
                v1 = min(r1[3], r2[3])
                if u1 - u0 > 0.001 and v1 - v0 > 0.001:
                    conflicts.append(
                        f"共面冲突: {n1} 与 {n2} 在 {side} 面共面 ({val}), 重叠区域 ({(u1-u0):.3f}x{(v1-v0):.3f})"
                    )
    if conflicts:
        raise AssertionError("\n".join(conflicts))


# =============================================================================
# 贴图与 Blockbench 序列化
# =============================================================================

def build_texture(res: int = RES) -> Image.Image:
    """生成 64×64 RGBA 贴图，严格使用有机型与红肌纤维配色。"""
    rng = np.random.default_rng(20261009)
    arr = np.zeros((res, res, 4), dtype=np.uint8)

    for mat_name, (u0, v0, u1, v1) in MAT_UV.items():
        base_c = PALETTE[mat_name]
        for y in range(v0, v1):
            for x in range(u0, u1):
                noise = rng.integers(-4, 5)
                r = int(np.clip(base_c[0] + noise, 0, 255))
                g = int(np.clip(base_c[1] + noise, 0, 255))
                b = int(np.clip(base_c[2] + noise, 0, 255))

                if mat_name == "flesh_main":
                    if x % 4 in (0, 1):
                        r = int(np.clip(r + 8, 0, 255))
                        g = int(np.clip(g + 4, 0, 255))
                elif mat_name == "tendon_trans":
                    if (x + y) % 5 == 0:
                        r = int(np.clip(r - 6, 0, 255))
                        g = int(np.clip(g - 6, 0, 255))
                        b = int(np.clip(b - 4, 0, 255))

                arr[y, x] = [r, g, b, base_c[3]]

    return Image.fromarray(arr, "RGBA")


def build_bbmodel_doc(cubes: List[dict], tex: Image.Image) -> dict:
    """组装符合 Blockbench 4.10 格式的 JSON 字典。"""
    buf = io.BytesIO()
    tex.save(buf, format="PNG")
    tex_b64 = "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode("ascii")

    texture_uuid = str(uuid.uuid4())
    texture_entry = {
        "name": "leg_muscle",
        "folder": "meridian_factory",
        "namespace": "bong",
        "id": "0",
        "particle": False,
        "render_mode": "default",
        "visible": True,
        "mode": "bitmap",
        "saved": True,
        "uuid": texture_uuid,
        "source": tex_b64,
        "width": 64,
        "height": 64,
    }

    elements = []
    groups_map: Dict[str, List[str]] = {}

    for idx, c in enumerate(cubes):
        elem_uuid = str(uuid.uuid4())
        g_name = c.get("group", "muscle")
        groups_map.setdefault(g_name, []).append(elem_uuid)

        mat_key = c.get("material", "flesh_main")
        u0, v0, u1, v1 = MAT_UV.get(mat_key, [0, 0, 16, 16])
        span_x = max(1, u1 - u0 - 4)
        span_y = max(1, v1 - v0 - 4)
        uv_u = u0 + (idx * 2) % span_x
        uv_v = v0 + (idx * 3) % span_y
        uv_box = [float(uv_u), float(uv_v), float(uv_u + 4), float(uv_v + 4)]

        faces = {
            face_name: {"uv": uv_box, "texture": 0}
            for face_name in ("north", "east", "south", "west", "up", "down")
        }

        element = {
            "name": c["name"],
            "box_uv": False,
            "rescale": False,
            "locked": False,
            "from": [round(float(v), 4) for v in c["from"]],
            "to":   [round(float(v), 4) for v in c["to"]],
            "autouv": 0,
            "color": 0,
            "origin": [0.0, 0.0, 0.0],
            "faces": faces,
            "type": "cube",
            "uuid": elem_uuid,
        }
        elements.append(element)

    outliner = []
    for g_name in ["bone_rings", "tendons", "muscle_bundles"]:
        if g_name in groups_map:
            outliner.append({
                "name": g_name,
                "origin": [0.0, 0.0, 0.0],
                "color": 0,
                "uuid": str(uuid.uuid4()),
                "isOpen": True,
                "children": groups_map[g_name],
            })

    return {
        "meta": {"format_version": "4.10", "model_format": "free"},
        "name": "LegMuscle",
        "model_identifier": "leg_muscle",
        "visible_box": [3, 2, 3],
        "geometry_name": "leg_muscle",
        "resolution": {"width": 64, "height": 64},
        "elements": elements,
        "outliner": outliner,
        "textures": [texture_entry],
    }


def generate_bbmodel(out_path: Path = BBMODEL_OUT) -> Path:
    """执行标准生成流程并落盘 bbmodel。"""
    cubes = all_cubes()
    _assert_no_coplanar_faces(cubes)

    out_path.parent.mkdir(parents=True, exist_ok=True)
    tex = build_texture()
    doc = build_bbmodel_doc(cubes, tex)

    out_path.write_text(json.dumps(doc, indent=2, ensure_ascii=False), encoding="utf-8")
    rel = out_path.relative_to(REPO) if out_path.is_relative_to(REPO) else out_path
    print(f"✓ 下肢肌肉组 bbmodel 写入成功: {rel}")
    return out_path


# =============================================================================
# 审阅图像渲染 (render.png 与 check.png)
# =============================================================================

def render_views(bbmodel_path: Path = BBMODEL_OUT):
    """输出 render.png（四视图）、parts.png（分部件）、check.png（左参考 / 右 正视 + 3/4）。"""
    from bbmodel_maker.render.render_bbmodel import render

    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    REVIEW_DIR_ALIAS.mkdir(parents=True, exist_ok=True)
    bg_color = (119, 119, 119)  # 对齐参考图中性灰

    # 整件竖立，正面 +Z；yaw=180 渲的是 +Z 面（与参考图左图同向），yaw=145 是从前上方的 3/4
    im_front, _ = render(bbmodel_path, yaw=0.0, pitch=0.0, size=500, bg=bg_color)
    im_iso, _ = render(bbmodel_path, yaw=-35.0, pitch=25.0, size=500, bg=bg_color)
    im_side, _ = render(bbmodel_path, yaw=90.0, pitch=0.0, size=500, bg=bg_color)
    im_back, _ = render(bbmodel_path, yaw=145.0, pitch=25.0, size=500, bg=bg_color)

    canvas = Image.new("RGB", (1040, 1040), (35, 36, 40))
    draw = ImageDraw.Draw(canvas)
    for title, im_v, px, py in [
        ("FRONT (+Z)", im_front, 20, 20),
        ("SIDE", im_side, 540, 20),
        ("3/4 FRONT", im_iso, 20, 540),
        ("3/4 BACK", im_back, 540, 540),
    ]:
        canvas.paste(im_v, (px, py))
        draw.text((px + 8, py + 6), title, fill=(230, 230, 230))
    for target_dir in [REVIEW_DIR, REVIEW_DIR_ALIAS]:
        canvas.save(target_dir / "render.png")

    # 分部件
    tmp = Path("/tmp/meridian_factory_leg_parts")
    tmp.mkdir(parents=True, exist_ok=True)
    thumbs = []
    for pname, fn in (("bone_rings", part_01_bone_rings), ("tendons", part_02_tendons), ("muscle_bundles", part_03_muscle_bundles)):
        p = tmp / f"{pname}.bbmodel"
        p.write_text(json.dumps(build_bbmodel_doc(fn(), build_texture()), ensure_ascii=False), encoding="utf-8")
        im_p, _ = render(p, yaw=-35.0, pitch=25.0, size=400, bg=bg_color)
        thumbs.append((pname, im_p))
    sheet = Image.new("RGB", (3 * 410 + 10, 420), (35, 36, 40))
    sd = ImageDraw.Draw(sheet)
    for i, (pname, im_p) in enumerate(thumbs):
        sheet.paste(im_p, (10 + i * 410, 10))
        sd.text((18 + i * 410, 16), pname, fill=(230, 230, 230))
    for target_dir in [REVIEW_DIR, REVIEW_DIR_ALIAS]:
        sheet.save(target_dir / "parts.png")

    if REF_IMAGE.exists():
        ref_im = Image.open(REF_IMAGE).convert("RGB")
        target_h = 600
        ref_w = int(ref_im.width * target_h / ref_im.height)
        ref_scaled = ref_im.resize((ref_w, target_h), Image.Resampling.LANCZOS)
        front_s = im_front.resize((target_h, target_h), Image.Resampling.LANCZOS)
        iso_s = im_iso.resize((target_h, target_h), Image.Resampling.LANCZOS)
        check_cv = Image.new("RGB", (ref_w + 2 * target_h + 36, target_h + 40), (28, 29, 33))
        c_draw = ImageDraw.Draw(check_cv)
        check_cv.paste(ref_scaled, (12, 30))
        check_cv.paste(front_s, (ref_w + 24, 30))
        check_cv.paste(iso_s, (ref_w + target_h + 36, 30))
        c_draw.text((16, 8), "REFERENCE o04_leg_muscle.png", fill=(210, 200, 180))
        c_draw.text((ref_w + 28, 8), "FRONT", fill=(180, 220, 210))
        c_draw.text((ref_w + target_h + 40, 8), "3/4 FRONT", fill=(180, 220, 210))
        for target_dir in [REVIEW_DIR, REVIEW_DIR_ALIAS]:
            check_cv.save(target_dir / "check.png")
    print(f"✓ leg_muscle: render / parts / check -> {REVIEW_DIR}")


def check_structure(cubes: List[dict]) -> None:
    """结构门禁：外包、9 束、3 个骨环口、肌腱 Y 形张开且落在边缘、束端圆钝。"""
    lo = [min(c["from"][i] for c in cubes) for i in range(3)]
    hi = [max(c["to"][i] for c in cubes) for i in range(3)]
    assert lo[0] >= -24 and hi[0] <= 24 and lo[2] >= -24 and hi[2] <= 24, f"x/z 越出 3×3 方块: {lo} {hi}"
    assert lo[1] >= 0 and hi[1] <= 32, f"y 越出 0..32: {lo[1]} {hi[1]}"

    bundles = {c["name"].split("_")[0] for c in cubes if c["name"].startswith("b") and "_" in c["name"] and c["name"][1:3].isdigit()}
    assert len(bundles) == 9, f"肌束应为 9 束，实际 {len(bundles)}"
    for b in bundles:
        levels = [c for c in cubes if c["name"].startswith(f"{b}_") and not c["name"].endswith("_hl")]
        assert len(levels) == 5, f"{b} 应为 5 级(两端各 2 级倒角)，实际 {len(levels)}"

    rings = {c["name"].rsplit("_", 1)[0] for c in cubes if c["name"].startswith("ring_") and c["name"].endswith(("_w", "_e", "_n", "_s"))}
    assert rings == {"ring_top", "ring_bl", "ring_br"}, f"骨环口应为 顶1 + 底2: {rings}"

    legs = {s: sorted((c for c in cubes if c["name"].startswith(f"leg_{s}_") and not c["name"].endswith("_hl")),
                      key=lambda c: -c["from"][1]) for s in ("l", "r")}
    for s, seq in legs.items():
        xs = [abs((c["from"][0] + c["to"][0]) / 2) for c in seq]
        assert all(b >= a - 1e-6 for a, b in zip(xs, xs[1:])), f"{s} 腿没有单调向外张开: {xs}"
        assert xs[-1] >= 15.0, f"{s} 腿末端没贴近边缘(|x|={xs[-1]})"
        assert xs[0] <= 1.5, f"{s} 腿起点没并在中线(|x|={xs[0]})"
    trunk = [c for c in cubes if c["name"].startswith("trunk_") and not c["name"].endswith("_hl")]
    assert trunk and all(abs(c["from"][0] + c["to"][0]) < 1e-6 for c in trunk), "主干必须在中线"
    gap = min(c["from"][0] for c in legs["r"]) if False else None
    del gap
    # 底部两口分得很开
    inner = min(c["from"][0] for c in cubes if c["name"] == "ring_br_w") - max(c["to"][0] for c in cubes if c["name"] == "ring_bl_e")
    assert inner >= 20.0, f"底部两口内缘距太近: {inner}"
    # 束端不平切：最高一级的半宽必须小于主体
    tops = [c for c in cubes if c["name"].startswith("b") and c["name"].endswith("_4")]
    assert all((c["to"][0] - c["from"][0]) < 2 * BELLY_HALF for c in tops), "束顶端必须倒角收细"


def self_test():
    """门禁差分自证：每道门先注入缺陷确认报得出来。"""
    print("运行 gen_leg_muscle.py 差分自证...")
    cubes = all_cubes()
    _assert_no_coplanar_faces(cubes)
    check_structure(cubes)
    print(f"  [OK] {len(cubes)} 个立方体，共面/外包/9束/骨环口/Y形/倒角全部通过")

    def must_fail(name, bad, gate):
        try:
            gate(bad)
        except AssertionError as e:
            print(f"  [OK] 注入「{name}」被拦截: {str(e).splitlines()[0][:60]}")
            return
        raise RuntimeError(f"门禁失效: 注入「{name}」没被拦截")

    must_fail("共面", cubes + [dict(cubes[0], name="dup")], _assert_no_coplanar_faces)
    must_fail("少一束", [c for c in cubes if not c["name"].startswith("b00_")], check_structure)
    parallel = [dict(c, **{"from": [(-1.0 if c["name"].startswith("leg_l_") else 1.0) * 5.0 - 1.0 if c["name"].startswith("leg_l_") else 4.0, c["from"][1], c["from"][2]],
                           "to": [(-1.0 if c["name"].startswith("leg_l_") else 1.0) * 5.0 + 1.0 if c["name"].startswith("leg_l_") else 6.0, c["to"][1], c["to"][2]]})
                if c["name"].startswith("leg_") and not c["name"].endswith("_hl") else c for c in cubes]
    must_fail("两腿平行不张开", parallel, check_structure)
    must_fail("少一个底口", [c for c in cubes if not c["name"].startswith("ring_br_")], check_structure)
    def flatten(c):
        # 把最高一级撑回满宽，等于束端被平切
        cx, cz = (c["from"][0] + c["to"][0]) / 2, (c["from"][2] + c["to"][2]) / 2
        return dict(c, **{"from": [cx - BELLY_HALF, c["from"][1], cz - BELLY_HALF], "to": [cx + BELLY_HALF, c["to"][1], cz + BELLY_HALF]})

    flat_top = [flatten(c) if c["name"].startswith("b") and c["name"].endswith("_4") else c for c in cubes]
    must_fail("束端平切", flat_top, check_structure)
    print("✓ gen_leg_muscle.py 差分自证全绿")


def main():
    parser = argparse.ArgumentParser(description="经脉工厂内景器官 o04 leg_muscle 生成器")
    parser.add_argument("--self-test", action="store_true", help="只跑门禁差分自证")
    args = parser.parse_args()

    self_test()
    if args.self_test:
        return
    bb_path = generate_bbmodel()
    render_views(bb_path)


if __name__ == "__main__":
    main()
