#!/usr/bin/env python3
"""灵木剑（lingmu_sword / 灵木削制窄剑）Blockbench .bbmodel 程序化生成器。

设计理念与世界观：
- server/assets/items/weapons.toml:
  lingmu_sword: "灵木削制的窄剑。木纹中隐约可见灵脉走向，是灵器的前身。"
- 风格（调度审第 2 次公式级严格落实）：
  按剑身比例基准（设剑身宽为 W = 0.0520）：
    * 护手总宽 = 2.5W (= 0.1300)，高 = 0.5W (= 0.0260)，厚 = W (= 0.0520)，两端上翘 = 0.5W (= 0.0260)；
    * 剑首宽 = 柄宽 + 0.5W (= 0.0360 + 0.0260 = 0.0620)，高 = W (= 0.0520)，正面嵌绿纹 = 0.5W 见方 (= 0.0260×0.0260)；
  整体比例严格对照参考图左上完整的剑，护手只比剑身稍宽一截，轻盈挺拔、线条洗练。
- 坐标约定：
  手持物基准系（授权系）：握把末端在 y=0，剑尖在 +Y，剑身沿 Y 轴挺立，
  全长约 0.887 方块，握把中心点 grip=0.170。

结构组成（分部件 part_* 拆解）：
    1. part_pommel       - 方块剑首（宽 柄宽+0.5W, 高 W, 嵌 0.5W 见方绿色灵纹）
    2. part_grip         - 灵木剑柄芯（宽 W_grip）与 4 道细密素麻绳防滑缠圈
    3. part_guard        - 灵木十字护手（总宽 2.5W, 高 0.5W, 厚 W, 两端上翘 0.5W）
    4. part_blade_base   - 剑身基段（y: guard_y1 -> 0.320）
    5. part_blade_narrow - 灵木削制窄长剑身（双刃与贯通青木灵脉中脊脊线）
    6. part_blade_tip    - 剑尖多级挺拔收尖三角刃头

用法：
    python3 modelScript/generators/gen_lingmu_sword.py             # 生成模型与审阅图
    python3 modelScript/generators/gen_lingmu_sword.py --self-test # 运行门禁自证
"""

from __future__ import annotations

import argparse
import random
import sys
from pathlib import Path

from PIL import Image, ImageDraw

REPO = Path(__file__).resolve().parents[2]
CORE_DIR = Path(__file__).resolve().parents[1] / "core"
sys.path.insert(0, str(CORE_DIR))

from bbmodel_maker.render.held_item_common import (  # noqa: E402
    Box,
    HeldItem,
    Material,
    assert_boxes_are_connected,
    assert_conventions,
    assert_no_coplanar_faces,
    blotch,
    build_model_json,
    build_mtl,
    build_obj,
    hand_display,
    noise_fill,
    write_assets,
)

MODEL_DIR = Path(__file__).resolve().parents[1] / "models"
BBMODEL_OUT = MODEL_DIR / "LingmuSword.bbmodel"
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/handheld/lingmu_sword")
REF_IMAGE = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/handheld/exploded/lingmu_sword.png")

LENGTH = 0.887
GRIP = 0.170
SCALE = 0.850

# ── 调度审定严格相对比例基准 (全以剑身宽 W 为基准) ──
W = 0.0520          # 剑身宽 W (半宽 0.0260)
W_GRIP = 0.0360     # 柄宽 (半宽 0.0180)

GUARD_W = 2.5 * W       # 护手总宽 = 2.5W (0.1300)
GUARD_H = 0.5 * W       # 护手高 = 0.5W (0.0260)
GUARD_THICK = 1.0 * W   # 护手厚 = W (0.0520)
GUARD_UPTURN = 0.5 * W  # 两端上翘 = 0.5W (0.0260)

POMMEL_W = W_GRIP + 0.5 * W  # 剑首宽 = 柄宽 + 0.5W (0.0620)
POMMEL_H = W                 # 剑首高 = W (0.0520)
POMMEL_THICK = POMMEL_W      # 剑首厚 (0.0620)
RUNE_SIZE = 0.5 * W          # 嵌绿纹 = 0.5W 见方 (0.0260)


# =============================================================================
# 材质生成 (16×16 通用材质样本贴图)
# =============================================================================

def make_spirit_wood_tex(rng: random.Random) -> Image.Image:
    """温润灵木本色：琥珀红褐硬木，带坚实长丝年轮。"""
    im = Image.new("RGBA", (16, 16))
    noise_fill(im, rng, (140, 100, 65), 10, warm=8)
    blotch(im, rng, 4, (105, 70, 42), (1, 6))
    blotch(im, rng, 3, (175, 130, 88), (1, 4))
    return im


def make_wood_edge_tex(rng: random.Random) -> Image.Image:
    """削制木刃锋线：浅色温润、带脱胎亮泽的削制木刃边缘。"""
    im = Image.new("RGBA", (16, 16))
    noise_fill(im, rng, (168, 128, 85), 8, warm=4)
    blotch(im, rng, 3, (196, 154, 108), (1, 3))
    blotch(im, rng, 3, (132, 94, 60), (1, 3))
    return im


def make_spirit_vein_tex(rng: random.Random) -> Image.Image:
    """青木灵脉走向：隐约泛出温润青碧木灵光晕的脉纹中脊。"""
    im = Image.new("RGBA", (16, 16))
    noise_fill(im, rng, (76, 136, 104), 10)
    blotch(im, rng, 4, (108, 184, 140), (1, 4))
    blotch(im, rng, 3, (52, 98, 74), (1, 3))
    return im


def make_cord_tex(rng: random.Random) -> Image.Image:
    """细密素麻绳缠带：米灰细线编织缠柄。"""
    im = Image.new("RGBA", (16, 16))
    noise_fill(im, rng, (176, 156, 126), 8, warm=2)
    blotch(im, rng, 4, (136, 114, 88), (1, 3))
    blotch(im, rng, 3, (206, 186, 156), (1, 3))
    return im


# =============================================================================
# 各部件几何定义 (part_* 拆分)
# =============================================================================

def part_pommel() -> list[Box]:
    """1. 剑首 (y: 0.000 -> POMMEL_H = 0.0520)。

    宽 = 柄宽 + 0.5W, 高 = W, 正面嵌 0.5W 见方绿色灵纹。
    """
    return [
        # 方块剑首主体 (独占 y_min=0)
        Box(
            "pommel_cube",
            "spirit_wood",
            (0.0, POMMEL_H / 2.0, 0.0),
            (POMMEL_W / 2.0, POMMEL_H / 2.0, POMMEL_THICK / 2.0),
        ),
        # 正面 0.5W 见方 绿色灵纹符印 (嵌于正面 -Z 侧)
        Box(
            "pommel_vein_rune",
            "spirit_vein",
            (0.0, POMMEL_H / 2.0, -(POMMEL_THICK / 2.0 + 0.002)),
            (RUNE_SIZE / 2.0, RUNE_SIZE / 2.0, 0.002),
        ),
    ]


def part_grip() -> list[Box]:
    """2. 灵木剑柄与 4 道细绳防滑缠圈 (y: POMMEL_H -> 0.220)。

    柄宽 W_grip = 0.0360，外缠 4 道细密素麻绳。
    """
    grip_y0 = POMMEL_H
    grip_y1 = 0.220
    boxes = [
        Box(
            "grip_core",
            "spirit_wood",
            (0.0, (grip_y0 + grip_y1) / 2.0, 0.0),
            (W_GRIP / 2.0, (grip_y1 - grip_y0) / 2.0, W_GRIP / 2.0),
        ),
    ]
    for i in range(4):
        yb = grip_y0 + 0.020 + i * 0.038
        boxes.append(
            Box(
                f"grip_wrap_{i}",
                "fine_cord",
                (0.0, yb, 0.0),
                (W_GRIP / 2.0 + 0.003, 0.010, W_GRIP / 2.0 + 0.003),
            )
        )
    return boxes


def part_guard() -> list[Box]:
    """3. 灵木十字护手 (交界处: y in [0.220, 0.246])。

    总宽 = 2.5W (= 0.1300)，高 = 0.5W (= 0.0260)，厚 = W (= 0.0520)，两端上翘 = 0.5W (= 0.0260)。
    """
    guard_y0 = 0.220
    guard_y1 = guard_y0 + GUARD_H
    guard_y_mid = (guard_y0 + guard_y1) / 2.0
    wing_tip_w = 0.5 * W  # 0.0260

    # 左端 (-X) 与右端 (+X) 挑尖 X 中心
    x_l_center = -(GUARD_W / 2.0 - wing_tip_w / 2.0)
    x_r_center = (GUARD_W / 2.0 - wing_tip_w / 2.0)

    return [
        # 主横梁 (总宽 2.5W, 高 0.5W, 厚 W)
        Box(
            "guard_beam",
            "spirit_wood",
            (0.0, guard_y_mid, 0.0),
            (GUARD_W / 2.0, GUARD_H / 2.0, GUARD_THICK / 2.0),
        ),
        # 左端 0.5W 上翘
        Box(
            "guard_wing_l_tip",
            "spirit_wood",
            (x_l_center, guard_y1 + GUARD_UPTURN / 2.0, 0.0),
            (wing_tip_w / 2.0, GUARD_UPTURN / 2.0, GUARD_THICK / 2.0 - 0.004),
        ),
        # 右端 0.5W 上翘
        Box(
            "guard_wing_r_tip",
            "spirit_wood",
            (x_r_center, guard_y1 + GUARD_UPTURN / 2.0, 0.0),
            (wing_tip_w / 2.0, GUARD_UPTURN / 2.0, GUARD_THICK / 2.0 - 0.004),
        ),
    ]


def part_blade_base() -> list[Box]:
    """4. 剑身基段 (y: 0.246 -> 0.320)。

    咬合在十字护手中央的加固根段。
    """
    blade_y0 = 0.220 + GUARD_H
    return [
        Box(
            "blade_base_root",
            "spirit_wood",
            (0.0, (blade_y0 + 0.320) / 2.0, 0.0),
            (W / 2.0 - 0.002, (0.320 - blade_y0) / 2.0, 0.014),
        ),
    ]


def part_blade_narrow() -> list[Box]:
    """5. 灵木削制窄长剑身 (y: 0.320 -> 0.810)。

    - 剑身主宽 W = 0.0520；
    - 贯穿中脊的青木灵脉走向 (spirit_vein)；
    - 两侧双刃削薄展宽，逐段向剑尖微收拢。
    """
    w_mid = W * 0.88   # 0.0458
    w_high = W * 0.75  # 0.0390

    return [
        # ── 青木灵脉中脊贯通线 (spirit_vein，贯穿全剑中脊) ──
        Box("blade_spine_low", "spirit_vein", (0.0, 0.455, 0.0), (0.008, 0.115, 0.009)),
        Box("blade_spine_mid", "spirit_vein", (0.0, 0.655, 0.0), (0.007, 0.085, 0.008)),
        Box("blade_spine_high", "spirit_vein", (0.0, 0.775, 0.0), (0.006, 0.030, 0.007)),

        # ── 下段剑身 (y: 0.320 -> 0.540, 宽 W) ──
        Box("blade_body_low", "spirit_wood", (0.0, 0.430, 0.0), (W / 2.0 - 0.004, 0.110, 0.008)),
        Box("blade_edge_l_low", "wood_edge", (-W / 2.0 + 0.002, 0.430, 0.0), (0.0025, 0.108, 0.005)),
        Box("blade_edge_r_low", "wood_edge", ( W / 2.0 - 0.002, 0.430, 0.0), (0.0025, 0.108, 0.005)),

        # ── 中段剑身 (y: 0.540 -> 0.720, 宽 W_mid) - 微收窄 ──
        Box("blade_body_mid", "spirit_wood", (0.0, 0.630, 0.0), (w_mid / 2.0 - 0.003, 0.090, 0.007)),
        Box("blade_edge_l_mid", "wood_edge", (-w_mid / 2.0 + 0.0015, 0.630, 0.0), (0.002, 0.088, 0.0045)),
        Box("blade_edge_r_mid", "wood_edge", ( w_mid / 2.0 - 0.0015, 0.630, 0.0), (0.002, 0.088, 0.0045)),

        # ── 上段剑身 (y: 0.720 -> 0.810, 宽 W_high) - 再收窄 ──
        Box("blade_body_high", "spirit_wood", (0.0, 0.765, 0.0), (w_high / 2.0 - 0.002, 0.045, 0.006)),
        Box("blade_edge_l_high", "wood_edge", (-w_high / 2.0 + 0.001, 0.765, 0.0), (0.0015, 0.043, 0.0035)),
        Box("blade_edge_r_high", "wood_edge", ( w_high / 2.0 - 0.001, 0.765, 0.0), (0.0015, 0.043, 0.0035)),
    ]


def part_blade_tip() -> list[Box]:
    """6. 剑尖收尖 (y: 0.810 -> 0.887)。

    多级收缩的挺拔剑尖三角刃头。
    """
    return [
        Box("blade_tip_taper1", "wood_edge", (0.0, 0.835, 0.0), (0.013, 0.025, 0.005)),
        Box("blade_tip_taper2", "wood_edge", (0.0, 0.865, 0.0), (0.008, 0.015, 0.004)),
        Box("blade_tip_point",  "wood_edge", (0.0, 0.882, 0.0), (0.004, 0.005, 0.003)),
    ]


def all_boxes() -> list[Box]:
    """组装灵木剑全部 6 大部件。"""
    return (
        part_pommel()
        + part_grip()
        + part_guard()
        + part_blade_base()
        + part_blade_narrow()
        + part_blade_tip()
    )


def lingmu_sword() -> HeldItem:
    """构建 HeldItem 对象。"""
    rng = random.Random(20261012)
    mats = (
        Material("spirit_wood", (0.55, 0.39, 0.25), make_spirit_wood_tex(rng)),
        Material("wood_edge", (0.66, 0.50, 0.33), make_wood_edge_tex(rng)),
        Material("spirit_vein", (0.30, 0.53, 0.41), make_spirit_vein_tex(rng)),
        Material("fine_cord", (0.69, 0.61, 0.49), make_cord_tex(rng)),
    )
    return HeldItem(
        key="lingmu_sword",
        display_name="灵木剑",
        host_item="lingmu_sword",
        boxes=tuple(all_boxes()),
        materials=mats,
        display=hand_display(SCALE, GRIP, LENGTH, gui_spin=45.0),
        grip=GRIP,
    )


# =============================================================================
# 审阅图像渲染与并排对标
# =============================================================================

def render_views(bbmodel_path: Path):
    """渲染多视角并输出原尺寸并排对标卡 check.png。"""
    from bbmodel_maker.render.render_bbmodel import render

    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    bg_color = (119, 119, 119)  # 严格对齐参考图中性灰

    # 1. 渲染视角 (3/4 视看十字护手、方块灵纹剑首与灵脉中脊；正视 FRONT 看全剑修长线条；侧视 SIDE 看护手厚度与侧面轮廓)
    im_iso, _ = render(bbmodel_path, yaw=-35.0, pitch=25.0, size=500, bg=bg_color)
    im_front, _ = render(bbmodel_path, yaw=0.0, pitch=0.0, size=500, bg=bg_color)
    im_side, _ = render(bbmodel_path, yaw=90.0, pitch=0.0, size=500, bg=bg_color)

    # 2. 拼装 render.png (1 行 3 列：正视 + 3/4 等轴视 + 侧视)
    r_w = 500 * 3 + 16 * 2 + 48
    r_h = 500 + 72
    render_cv = Image.new("RGB", (r_w, r_h), (35, 36, 40))
    r_draw = ImageDraw.Draw(render_cv)

    views = [
        ("FRONT VIEW (Guard 2.5W, Cube Pommel & Spirit Vein)", im_front, 24, 40),
        ("3/4 ISOMETRIC VIEW (Balanced Proportions)", im_iso, 24 + 516, 40),
        ("SIDE VIEW (Guard Thickness W & Edge Profile)", im_side, 24 + 516 * 2, 40),
    ]
    for title, im_v, px, py in views:
        render_cv.paste(im_v, (px, py))
        r_draw.rectangle([px, py, px + 440, py + 26], fill=(24, 25, 28))
        r_draw.text((px + 8, py + 6), title, fill=(230, 230, 230))

    r_path = REVIEW_DIR / "render.png"
    render_cv.save(r_path)
    print(f"✓ render.png 已输出: {r_path}")

    # 3. 拼装 check.png (左参考图 / 右 3/4 + 侧视 等高对标)
    if REF_IMAGE.exists():
        ref_im = Image.open(REF_IMAGE).convert("RGB")
        target_h = 600
        ref_w = int(ref_im.width * target_h / ref_im.height)
        ref_scaled = ref_im.resize((ref_w, target_h), Image.Resampling.LANCZOS)

        iso_scale_w = int(im_iso.width * target_h / im_iso.height)
        side_scale_w = int(im_side.width * target_h / im_side.height)
        iso_scaled = im_iso.resize((iso_scale_w, target_h), Image.Resampling.LANCZOS)
        side_scaled = im_side.resize((side_scale_w, target_h), Image.Resampling.LANCZOS)

        right_w = iso_scale_w + side_scale_w + 16
        total_w = ref_w + right_w + 36
        check_cv = Image.new("RGB", (total_w, target_h + 40), (28, 29, 33))
        c_draw = ImageDraw.Draw(check_cv)

        # 贴左参考
        check_cv.paste(ref_scaled, (12, 30))
        c_draw.text((16, 8), "EXPLODED REFERENCE (lingmu_sword.png: Components Breakdown)", fill=(210, 200, 180))

        # 贴右渲染 (3/4 ISOMETRIC + SIDE VIEW)
        rx = ref_w + 24
        check_cv.paste(iso_scaled, (rx, 30))
        check_cv.paste(side_scaled, (rx + iso_scale_w + 12, 30))
        c_draw.text((rx + 4, 8), "NOW RENDER (3/4 ISOMETRIC VIEW + SIDE VIEW)", fill=(180, 220, 210))

        c_path = REVIEW_DIR / "check.png"
        check_cv.save(c_path)
        print(f"✓ check.png 并排对照图已输出: {c_path}")


# =============================================================================
# 门禁与差分自证 (--self-test)
# =============================================================================

def self_test():
    """运行门禁差分自证：正常模型无共面冲突，故意注入共面冲突能准确拦截。"""
    print("运行 gen_lingmu_sword.py 差分自证...")
    item = lingmu_sword()

    # 1. 验证常规契约
    assert_conventions(item)
    print("  [OK] assert_conventions 通过 (y_min=0, 全长合理, 握把点有效)")

    # 2. 验证连通性
    assert_boxes_are_connected(item)
    print("  [OK] assert_boxes_are_connected 通过 (整件为单一连续连通体)")

    # 3. 验证无共面冲突
    assert_no_coplanar_faces(item)
    print("  [OK] assert_no_coplanar_faces 通过 (所有盒子 0 项共面冲突)")

    # 4. 验证相对比例公式 (对齐调度第 2 轮硬性指标)
    b_map = {b.name: b for b in item.boxes}
    g_beam = b_map["guard_beam"]
    g_w_actual = 2.0 * g_beam.half[0]
    g_h_actual = 2.0 * g_beam.half[1]
    g_thick_actual = 2.0 * g_beam.half[2]

    assert abs(g_w_actual - 2.5 * W) < 1e-4, f"护手总宽不等于 2.5W: {g_w_actual} vs {2.5 * W}"
    assert abs(g_h_actual - 0.5 * W) < 1e-4, f"护手高不等于 0.5W: {g_h_actual} vs {0.5 * W}"
    assert abs(g_thick_actual - 1.0 * W) < 1e-4, f"护手厚不等于 W: {g_thick_actual} vs {W}"

    p_cube = b_map["pommel_cube"]
    p_w_actual = 2.0 * p_cube.half[0]
    p_h_actual = 2.0 * p_cube.half[1]
    assert abs(p_w_actual - (W_GRIP + 0.5 * W)) < 1e-4, f"剑首宽不等于 柄宽+0.5W: {p_w_actual} vs {W_GRIP + 0.5 * W}"
    assert abs(p_h_actual - W) < 1e-4, f"剑首高不等于 W: {p_h_actual} vs {W}"

    p_rune = b_map["pommel_vein_rune"]
    r_w_actual = 2.0 * p_rune.half[0]
    r_h_actual = 2.0 * p_rune.half[1]
    assert abs(r_w_actual - 0.5 * W) < 1e-4, f"灵纹宽不等于 0.5W: {r_w_actual} vs {0.5 * W}"
    assert abs(r_h_actual - 0.5 * W) < 1e-4, f"灵纹高不等于 0.5W: {r_h_actual} vs {0.5 * W}"
    print(f"  [OK] 调度审定比例公式严格验证通过: 护手宽 {g_w_actual:.4f} (2.5W), 高 {g_h_actual:.4f} (0.5W), 厚 {g_thick_actual:.4f} (W); 剑首宽 {p_w_actual:.4f} (柄宽+0.5W), 高 {p_h_actual:.4f} (W)")

    # 5. 注入共面冲突缺陷验证有效拦截
    defect_box = Box(
        "inject_coplanar_fail",
        "spirit_wood",
        (0.0, POMMEL_H / 2.0, 0.0),  # 与 pommel_cube 完全重叠共面
        (POMMEL_W / 2.0, POMMEL_H / 2.0, POMMEL_THICK / 2.0),
    )
    defect_item = HeldItem(
        key="lingmu_sword_defect",
        display_name="缺陷灵木剑",
        host_item="lingmu_sword",
        boxes=item.boxes + (defect_box,),
        materials=item.materials,
        display=item.display,
        grip=item.grip,
    )

    caught = False
    try:
        assert_no_coplanar_faces(defect_item)
    except ValueError as e:
        caught = True
        print(f"  [OK] 成功捕获注入共面缺陷: {e.args[0].splitlines()[0]}")

    if not caught:
        raise RuntimeError("门禁失效: 注入共面冲突未被拦截!")

    print("✓ gen_lingmu_sword.py 差分自证全绿")


def main():
    parser = argparse.ArgumentParser(description="灵木剑 Blockbench 模型生成器")
    parser.add_argument("--self-test", action="store_true", help="运行门禁差分自证")
    parser.add_argument("--no-preview", action="store_true", help="不渲染审阅图片")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return

    # 默认主流程：自测 -> 导出 bbmodel -> 渲染审阅图
    self_test()

    item = lingmu_sword()
    outputs = write_assets(
        (item,),
        bbmodel_dir=MODEL_DIR,
        client_resources=None,  # 只落建模源，不接线
        render_previews=False,
    )
    bbmodel_path = outputs[f"bbmodel:{item.key}"]
    print(f"✓ Blockbench 模型已生成: {bbmodel_path}")

    if not args.no_preview:
        render_views(bbmodel_path)


if __name__ == "__main__":
    main()
