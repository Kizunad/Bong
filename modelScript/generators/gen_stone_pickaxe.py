#!/usr/bin/env python3
"""石镐（stone_pickaxe / 碎石绑木柄石镐）Blockbench .bbmodel 程序化生成器。

设计理念与世界观：
- server/assets/items/workbench_materials.toml:
  stone_pickaxe: "碎石绑木柄，挖矿入门。"
- 风格（调度审第 1 次落实）：
  两头向下弯曲的弧形打制浅灰双头石镐（一端扁凿刃，一端锐尖锥）：
  两臂各长 6px，从中心往外每 2px 向下弯 1px；截面从中心 2×2 逐段收束到尖端 1×1；
  颜色采用浅灰石色（#9a9a94 / #b8b8b0 斑驳燧石）；
  镐头与木柄交接处配有醒目的十字交叉粗生麻绳捆扎带；保留质朴原木手柄。
- 坐标约定：
  手持物基准系（授权系）：握把末端在 y=0，镐头在 +Y，两臂沿 X 轴对称展开并向下弧弯（与 pickaxe_bone/pickaxe_iron 一致），
  全长约 0.805 方块，握把中心点 grip=0.18。

结构组成（分部件 part_* 拆解）：
    1. part_butt         - 柄尾防脱木节与底端收口
    2. part_grip         - 握把段与 4 道生麻绳防滑缠圈
    3. part_haft         - 天然原木主干中段柄身
    4. part_socket       - 木柄顶端套穿与插槽（底承托座与穿出木顶盖）
    5. part_stone_head   - 双头弧形打制浅灰碎石镐头（中心石核、-X 侧下弯扁凿臂、+X 侧下弯尖锥臂）
    6. part_bindings     - 镐头与木柄交接处的十字交叉粗生麻绳捆扎带

用法：
    python3 modelScript/generators/gen_stone_pickaxe.py             # 生成模型与审阅图
    python3 modelScript/generators/gen_stone_pickaxe.py --self-test # 运行门禁自证
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
BBMODEL_OUT = MODEL_DIR / "StonePickaxe.bbmodel"
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/handheld/stone_pickaxe")
REF_IMAGE = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/handheld/exploded/stone_pickaxe.png")

LENGTH = 0.805
GRIP = 0.180
SCALE = 0.850


# =============================================================================
# 材质生成 (16×16 通用材质样本贴图)
# =============================================================================

def make_wood_tex(rng: random.Random) -> Image.Image:
    """风干老木纹理：灰褐色基底，带风化暗纹与浅色木丝。"""
    im = Image.new("RGBA", (16, 16))
    noise_fill(im, rng, (116, 88, 58), 12, warm=6)
    blotch(im, rng, 4, (88, 62, 38), (1, 5))
    blotch(im, rng, 3, (148, 114, 78), (1, 4))
    return im


def make_stone_tex(rng: random.Random) -> Image.Image:
    """浅灰石色表面：#9a9a94 / #b8b8b0 斑驳浅灰燧石，带有敲击剥离崩痕。"""
    im = Image.new("RGBA", (16, 16))
    noise_fill(im, rng, (154, 154, 148), 10)
    blotch(im, rng, 4, (184, 184, 176), (2, 4))
    blotch(im, rng, 3, (122, 122, 114), (1, 3))
    return im


def make_edge_tex(rng: random.Random) -> Image.Image:
    """浅亮石质锋刃与锥尖：#c8c8c0 / #e0e0d8 坚硬石英/燧石锋面高光。"""
    im = Image.new("RGBA", (16, 16))
    noise_fill(im, rng, (200, 200, 192), 8)
    blotch(im, rng, 3, (224, 224, 216), (1, 2))
    blotch(im, rng, 3, (160, 160, 152), (1, 3))
    return im


def make_binding_tex(rng: random.Random) -> Image.Image:
    """粗生麻绳绑带：米褐编织粗纤维，带油脂浸润与风化暗调。"""
    im = Image.new("RGBA", (16, 16))
    noise_fill(im, rng, (166, 140, 104), 10, warm=4)
    blotch(im, rng, 4, (122, 96, 66), (2, 3))
    blotch(im, rng, 3, (196, 170, 130), (1, 3))
    return im


# =============================================================================
# 各部件几何定义 (part_* 拆分)
# =============================================================================

def part_butt() -> list[Box]:
    """1. 柄尾木盖 / 防脱木节 (y: 0.000 -> 0.045)。

    天然膨大的木节柄端，提供采矿敲击时的防脱手支点。
    """
    return [
        Box("butt_knob", "weathered_wood", (0.0, 0.0225, 0.0), (0.024, 0.0225, 0.024)),
    ]


def part_grip() -> list[Box]:
    """2. 握把段与 4 道生麻绳防滑缠圈 (y: 0.045 -> 0.320)。

    下段握持区，四道粗麻绳箍紧密包覆。
    """
    boxes = [
        Box("haft_grip", "weathered_wood", (0.0, 0.1825, 0.0), (0.019, 0.1375, 0.019)),
    ]
    for i in range(4):
        y_b = 0.075 + i * 0.055
        boxes.append(
            Box(f"grip_wrap_{i}", "hemp_binding", (0.0, y_b, 0.0), (0.022, 0.015, 0.022))
        )
    return boxes


def part_haft() -> list[Box]:
    """3. 天然原木主干中段柄身 (y: 0.320 -> 0.670)。

    坚固韧性的原木杆身，直立承力核心。
    """
    return [
        Box("haft_mid", "weathered_wood", (0.0, 0.495, 0.0), (0.021, 0.175, 0.021)),
    ]


def part_socket() -> list[Box]:
    """4. 木柄顶端套穿与插槽 (y: 0.670 -> 0.805)。

    包含石镐下托座与穿过石核上方的木顶盖。
    """
    return [
        # 下托座
        Box("haft_socket_base", "weathered_wood", (0.0, 0.685, 0.0), (0.023, 0.015, 0.023)),
        # 穿顶木冠
        Box("haft_socket_crown", "weathered_wood", (0.0, 0.793, 0.0), (0.019, 0.012, 0.019)),
    ]


def part_stone_head() -> list[Box]:
    """5. 双头打制浅灰弧形碎石镐头（两臂各长 6px，从中心往外每 2px 向下弯 1px；截面从 2×2 逐段收到 1×1）。

    - 中心石核：截面 2×2（x in [-0.035, 0.035], y in [0.705, 0.785], z in [-0.038, 0.038]）；
    - 右臂 (+X: 尖锥镐尖，长 6px，每 2px 下弯 1px)：
      * 段 1 (长 2px: x in [0.035, 0.115], 截面 1.8×1.8)
      * 段 2 (长 2px: x in [0.115, 0.195], 下弯 1px, 截面 1.4×1.4)
      * 段 3 (长 2px: x in [0.195, 0.265], 再下弯 1px, 截面 1.0×1.0)
      * 尖锥 (x in [0.265, 0.294], 锋利尖锐镐锥，浅冷灰高光)
    - 左臂 (-X: 凿形扁刃，长 6px，每 2px 下弯 1px)：
      * 段 1 (长 2px: x in [-0.115, -0.035], 截面 1.8×1.8)
      * 段 2 (长 2px: x in [-0.195, -0.115], 下弯 1px, 截面 1.4×1.4)
      * 段 3 (长 2px: x in [-0.265, -0.195], 再下弯 1px, 截面 1.0×1.0)
      * 扁凿刃 (x in [-0.294, -0.265], 石器凿锋，浅冷灰高光)
    """
    return [
        # 中心石核 (截面 2x2)
        Box("stone_core", "rough_stone", (0.0, 0.745, 0.0), (0.035, 0.040, 0.038)),

        # ── 右臂 (+X: 尖锥镐尖) ──
        # 段 1 (长 2px)
        Box("pick_arm_r_1", "rough_stone", (0.075, 0.736, 0.0), (0.040, 0.036, 0.034)),
        # 段 2 (长 2px, 下弯 1px)
        Box("pick_arm_r_2", "rough_stone", (0.155, 0.690, 0.0), (0.040, 0.035, 0.028)),
        # 段 3 (长 2px, 再下弯 1px)
        Box("pick_arm_r_3", "rough_stone", (0.230, 0.640, 0.0), (0.035, 0.035, 0.021)),
        # 尖锥 (末端尖锥镐尖)
        Box("pick_cone_tip", "stone_edge", (0.2765, 0.595, 0.0), (0.0115, 0.030, 0.014)),
        Box("pick_cone_point", "stone_edge", (0.291, 0.575, 0.0), (0.003, 0.020, 0.008)),

        # ── 左臂 (-X: 凿形扁刃) ──
        # 段 1 (长 2px)
        Box("pick_arm_l_1", "rough_stone", (-0.075, 0.736, 0.0), (0.040, 0.036, 0.034)),
        # 段 2 (长 2px, 下弯 1px)
        Box("pick_arm_l_2", "rough_stone", (-0.155, 0.690, 0.0), (0.040, 0.035, 0.028)),
        # 段 3 (长 2px, 再下弯 1px)
        Box("pick_arm_l_3", "rough_stone", (-0.230, 0.640, 0.0), (0.035, 0.035, 0.021)),
        # 扁凿刃 (末端石器凿锋)
        Box("chisel_edge_tip", "stone_edge", (-0.2765, 0.595, 0.0), (0.0115, 0.030, 0.012)),
        Box("chisel_edge_blade", "stone_edge", (-0.291, 0.575, 0.0), (0.003, 0.020, 0.006)),
    ]


def part_bindings() -> list[Box]:
    """6. 镐头与木柄交接处的十字交叉粗生麻绳捆扎带。

    具有真实十字交叉缠绑与前后打结扣环，紧密固定石镐与木柄。
    """
    return [
        # 下部紧固底圈 (紧箍木柄与石镐下缘)
        Box("binding_collar_low", "hemp_binding", (0.0, 0.695, 0.0), (0.027, 0.010, 0.031)),
        # 横向围束宽带 (中央主绳带)
        Box("binding_belt_main", "hemp_binding", (0.0, 0.745, 0.0), (0.030, 0.020, 0.042)),
        # 十字斜交叉带 1 (左上 -> 右下斜向紧勒)
        Box("binding_diag_1", "hemp_binding", (0.005, 0.743, 0.0), (0.024, 0.037, 0.040)),
        # 十字斜交叉带 2 (右上 -> 左下斜向紧勒)
        Box("binding_diag_2", "hemp_binding", (-0.005, 0.747, 0.0), (0.027, 0.039, 0.044)),
        # 正面绳结凸块
        Box("binding_knot_front", "hemp_binding", (0.0, 0.745, -0.045), (0.014, 0.014, 0.005)),
        # 上部收口顶圈 (紧箍木柄顶冠)
        Box("binding_collar_top", "hemp_binding", (0.0, 0.785, 0.0), (0.023, 0.006, 0.026)),
    ]


def all_boxes() -> list[Box]:
    """组装石镐全部 6 大部件。"""
    return (
        part_butt()
        + part_grip()
        + part_haft()
        + part_socket()
        + part_stone_head()
        + part_bindings()
    )


def stone_pickaxe() -> HeldItem:
    """构建 HeldItem 对象。"""
    rng = random.Random(20261012)
    mats = (
        Material("weathered_wood", (0.47, 0.35, 0.24), make_wood_tex(rng)),
        Material("rough_stone", (0.60, 0.60, 0.58), make_stone_tex(rng)),
        Material("stone_edge", (0.78, 0.78, 0.75), make_edge_tex(rng)),
        Material("hemp_binding", (0.65, 0.55, 0.41), make_binding_tex(rng)),
    )
    return HeldItem(
        key="stone_pickaxe",
        display_name="石镐",
        host_item="stone_pickaxe",
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

    # 1. 渲染视角 (3/4 视看两臂下弯弧度与十字捆扎、正视 FRONT 看两臂横向展宽与下弯、侧视 SIDE 看木柄厚度)
    im_iso, _ = render(bbmodel_path, yaw=-35.0, pitch=25.0, size=500, bg=bg_color)
    im_front, _ = render(bbmodel_path, yaw=0.0, pitch=0.0, size=500, bg=bg_color)
    im_side, _ = render(bbmodel_path, yaw=90.0, pitch=0.0, size=500, bg=bg_color)

    # 2. 拼装 render.png (1 行 3 列：正视 + 3/4 等轴视 + 侧视)
    r_w = 500 * 3 + 16 * 2 + 48
    r_h = 500 + 72
    render_cv = Image.new("RGB", (r_w, r_h), (35, 36, 40))
    r_draw = ImageDraw.Draw(render_cv)

    views = [
        ("FRONT VIEW (Curved Pick Head & Bindings)", im_front, 24, 40),
        ("3/4 ISOMETRIC VIEW (Full Curved Structure)", im_iso, 24 + 516, 40),
        ("SIDE VIEW (Haft Profile & Stone Joint)", im_side, 24 + 516 * 2, 40),
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
        c_draw.text((16, 8), "EXPLODED REFERENCE (stone_pickaxe.png: Components Breakdown)", fill=(210, 200, 180))

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
    print("运行 gen_stone_pickaxe.py 差分自证...")
    item = stone_pickaxe()

    # 1. 验证常规契约
    assert_conventions(item)
    print("  [OK] assert_conventions 通过 (y_min=0, 全长合理, 握把点有效)")

    # 2. 验证连通性
    assert_boxes_are_connected(item)
    print("  [OK] assert_boxes_are_connected 通过 (整件为单一连续连通体)")

    # 3. 验证无共面冲突
    assert_no_coplanar_faces(item)
    print("  [OK] assert_no_coplanar_faces 通过 (所有盒子 0 项共面冲突)")

    # 4. 注入共面冲突缺陷验证有效拦截
    defect_box = Box(
        "inject_coplanar_fail",
        "weathered_wood",
        (0.0, 0.0225, 0.0),  # 与 butt_knob 完全重叠共面
        (0.024, 0.0225, 0.024),
    )
    defect_item = HeldItem(
        key="stone_pickaxe_defect",
        display_name="缺陷石镐",
        host_item="stone_pickaxe",
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

    print("✓ gen_stone_pickaxe.py 差分自证全绿")


def main():
    parser = argparse.ArgumentParser(description="石镐 Blockbench 模型生成器")
    parser.add_argument("--self-test", action="store_true", help="运行门禁差分自证")
    parser.add_argument("--no-preview", action="store_true", help="不渲染审阅图片")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return

    # 默认主流程：自测 -> 导出 bbmodel -> 渲染审阅图
    self_test()

    item = stone_pickaxe()
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
