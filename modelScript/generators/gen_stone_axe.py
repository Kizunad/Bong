#!/usr/bin/env python3
"""石斧（stone_axe / 碎石夹木柄石斧）Blockbench .bbmodel 程序化生成器。Round 3/3 (PROMISE 担保轮)。

设计理念与世界观：
- server/assets/items/workbench_materials.toml:
  stone_axe: "碎石夹木柄，伐木入门。"
- 风格（Round 3 终轮精修）：
  末法时代早期实用砍伐工具，质朴粗犷、风化残损。
  天然打制燧石/玄武岩斧头，双面打击剥离崩口与阶梯贝壳状崩面，夹在经过削整劈开的硬木柄端部（带木夹爪端部收口倒角），
  用浸油生麻绳/皮绳层层十字交叉紧紧捆扎勒紧，柄尾带有天然防脱木节与缠绳。
- 坐标约定：
  手持物基准系（授权系）：握把末端在 y=0，斧头在 +Y，斧刃朝向 -X 侧（与 axe_bone/axe_iron 一致），
  全长约 0.81 方块，握把中心点 grip=0.18。

结构组成（分部件 part_* 拆解）：
    1. part_butt         - 柄尾防脱木节与底部收口
    2. part_grip         - 握把段与 4 道生麻绳防滑缠圈
    3. part_haft         - 天然原木主干中段柄身
    4. part_fork_socket  - 劈开夹持木槽（双木夹爪与端部受压削面）
    5. part_stone_head   - 打制碎石斧头（粗石刃：斧背、石身、扇形崩面凸棱、阶梯剥离崩口锋刃）
    6. part_bindings     - 紧勒木夹与石刃的十字交叉粗麻绳捆扎带

用法：
    python3 modelScript/generators/gen_stone_axe.py             # 生成模型与审阅图
    python3 modelScript/generators/gen_stone_axe.py --self-test # 运行门禁自证
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
BBMODEL_OUT = MODEL_DIR / "StoneAxe.bbmodel"
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/handheld/stone_axe")
REF_IMAGE = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/handheld/exploded/stone_axe.png")

LENGTH = 0.810
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
    """粗磨碎石表面：灰黑玄武岩/打制燧石基底，带敲击剥离崩痕与微杂斑。"""
    im = Image.new("RGBA", (16, 16))
    noise_fill(im, rng, (66, 68, 72), 10)
    blotch(im, rng, 4, (46, 47, 50), (2, 4))
    blotch(im, rng, 3, (102, 106, 112), (1, 3))
    return im


def make_edge_tex(rng: random.Random) -> Image.Image:
    """打制锋刃边缘：较亮冷灰崩口刃线，展现敲击锋面反光。"""
    im = Image.new("RGBA", (16, 16))
    noise_fill(im, rng, (136, 142, 150), 8)
    blotch(im, rng, 3, (160, 166, 174), (1, 2))
    blotch(im, rng, 3, (100, 104, 110), (1, 3))
    return im


def make_binding_tex(rng: random.Random) -> Image.Image:
    """粗麻绳/生皮绑带：米褐编织粗纤维，带油脂浸润与风化暗调。"""
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

    天然膨大的木节柄端，提供挥动砍伐时的防脱手支点。
    """
    return [
        Box("butt_knob", "weathered_wood", (0.0, 0.0225, 0.0), (0.024, 0.0225, 0.024)),
    ]


def part_grip() -> list[Box]:
    """2. 握把段与 4 道生麻绳防滑缠圈 (y: 0.045 -> 0.320)。

    下段握持区，四道粗麻绳箍紧密包覆，防止出汗打滑。
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
    """3. 天然原木主干中段柄身 (y: 0.320 -> 0.650)。

    坚固韧性的风干杂木杆身，直立承力核心。
    """
    return [
        Box("haft_mid", "weathered_wood", (0.0, 0.485, 0.0), (0.021, 0.165, 0.021)),
    ]


def part_fork_socket() -> list[Box]:
    """4. 劈开夹持木槽 (y: 0.650 -> 0.785)。

    木柄上端劈开形成的夹持叉：底座过渡与前后两片坚韧木夹爪，加设端部收紧削面。
    """
    return [
        # 夹持座底部承托
        Box("fork_base", "weathered_wood", (0.0, 0.665, 0.0), (0.022, 0.015, 0.022)),
        # 后夹爪 (Z+)
        Box("fork_claw_rear", "weathered_wood", (0.0, 0.7285, 0.023), (0.020, 0.0565, 0.007)),
        # 前夹爪 (Z-)
        Box("fork_claw_front", "weathered_wood", (0.0, 0.7285, -0.023), (0.020, 0.0565, 0.007)),
        # R3 收口加固倒角: 夹爪端部微收紧削面
        Box("fork_tip_wedge_r", "weathered_wood", (0.0, 0.771, 0.021), (0.016, 0.011, 0.006)),
        Box("fork_tip_wedge_f", "weathered_wood", (0.0, 0.771, -0.021), (0.016, 0.011, 0.006)),
    ]


def part_stone_head() -> list[Box]:
    """5. 打制碎石斧头 (粗石刃，y: 0.580 -> 0.810，X 向展布)。

    - 刃锋向 -X 侧扇形展开（砍劈方向）；
    - 斧背向 +X 侧微凸，钝厚平衡；
    - R3 细化：增设打击剥落贝壳崩面凹凸与阶梯开刃崩齿。
    """
    return [
        # 斧背尾端 (+X)
        Box("stone_butt", "rough_stone", (0.038, 0.700, 0.0), (0.022, 0.065, 0.016)),
        # 嵌于木槽内的石核中段
        Box("stone_core", "rough_stone", (-0.010, 0.700, 0.0), (0.026, 0.080, 0.015)),
        # 向 -X 展开的斧身过渡
        Box("stone_body", "rough_stone", (-0.062, 0.700, 0.0), (0.026, 0.095, 0.013)),
        # 扇形展开石斧大身
        Box("stone_flare", "rough_stone", (-0.110, 0.700, 0.0), (0.022, 0.105, 0.010)),
        # R3 打制贝壳崩面 (flaking facets)
        Box("stone_flake_upper", "rough_stone", (-0.088, 0.745, 0.0), (0.016, 0.032, 0.012)),
        Box("stone_flake_lower", "rough_stone", (-0.088, 0.645, 0.0), (0.016, 0.032, 0.012)),
        # 锋利开刃主中段 (-X 最前端)
        Box("stone_edge_mid", "stone_edge", (-0.142, 0.700, 0.0), (0.010, 0.090, 0.006)),
        # 上刃尖角
        Box("stone_edge_top", "stone_edge", (-0.134, 0.768, 0.0), (0.008, 0.023, 0.005)),
        # 下刃尖角
        Box("stone_edge_bot", "stone_edge", (-0.134, 0.632, 0.0), (0.008, 0.023, 0.005)),
        # R3 刃缘微挑小崩齿 (阶梯崩口，形成真实打制石器崩缺)
        Box("stone_edge_notch_upper", "stone_edge", (-0.147, 0.730, 0.0), (0.004, 0.016, 0.004)),
        Box("stone_edge_notch_lower", "stone_edge", (-0.147, 0.670, 0.0), (0.004, 0.016, 0.004)),
    ]


def part_bindings() -> list[Box]:
    """6. 十字交叉粗麻绳捆扎带 (y: 0.660 -> 0.760)。

    层层交叉勒紧木爪与碎石斧身，防止砍伐冲击震脱碎石。
    """
    return [
        # 底套圈箍
        Box("lashing_collar", "hemp_binding", (0.0, 0.675, 0.0), (0.025, 0.018, 0.027)),
        # 交叉绑带 1 (斜拉紧固)
        Box("lashing_diag_1", "hemp_binding", (-0.005, 0.715, 0.0), (0.027, 0.016, 0.029)),
        # 交叉绑带 2 (上部收口)
        Box("lashing_diag_2", "hemp_binding", (-0.004, 0.745, 0.0), (0.026, 0.014, 0.027)),
        # 后背十字锁结
        Box("lashing_cross", "hemp_binding", (0.022, 0.710, 0.0), (0.019, 0.035, 0.025)),
    ]


def all_boxes() -> list[Box]:
    """组装石斧全部 6 大部件。"""
    return (
        part_butt()
        + part_grip()
        + part_haft()
        + part_fork_socket()
        + part_stone_head()
        + part_bindings()
    )


def stone_axe() -> HeldItem:
    """构建 HeldItem 对象。"""
    rng = random.Random(20261012)
    mats = (
        Material("weathered_wood", (0.47, 0.35, 0.24), make_wood_tex(rng)),
        Material("rough_stone", (0.26, 0.27, 0.28), make_stone_tex(rng)),
        Material("stone_edge", (0.53, 0.56, 0.59), make_edge_tex(rng)),
        Material("hemp_binding", (0.65, 0.55, 0.41), make_binding_tex(rng)),
    )
    return HeldItem(
        key="stone_axe",
        display_name="石斧",
        host_item="stone_axe",
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

    # 1. 渲染视角 (3/4 视看斧刃与捆扎全貌、正视 FRONT 看斧刃展开弧面、侧视 SIDE 看木柄厚度)
    im_iso, _ = render(bbmodel_path, yaw=-35.0, pitch=25.0, size=500, bg=bg_color)
    im_front, _ = render(bbmodel_path, yaw=0.0, pitch=0.0, size=500, bg=bg_color)
    im_side, _ = render(bbmodel_path, yaw=90.0, pitch=0.0, size=500, bg=bg_color)

    # 2. 拼装 render.png (1 行 3 列：正视 + 3/4 等轴视 + 侧视)
    r_w = 500 * 3 + 16 * 2 + 48
    r_h = 500 + 72
    render_cv = Image.new("RGB", (r_w, r_h), (35, 36, 40))
    r_draw = ImageDraw.Draw(render_cv)

    views = [
        ("FRONT VIEW (Blade Spread & Bindings)", im_front, 24, 40),
        ("3/4 ISOMETRIC VIEW (Full Tool Structure)", im_iso, 24 + 516, 40),
        ("SIDE VIEW (Haft Profile & Stone Thickness)", im_side, 24 + 516 * 2, 40),
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
        c_draw.text((16, 8), "EXPLODED REFERENCE (stone_axe.png: Components Breakdown)", fill=(210, 200, 180))

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
    print("运行 gen_stone_axe.py 差分自证...")
    item = stone_axe()

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
        key="stone_axe_defect",
        display_name="缺陷石斧",
        host_item="stone_axe",
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

    print("✓ gen_stone_axe.py 差分自证全绿")


def main():
    parser = argparse.ArgumentParser(description="石斧 Blockbench 模型生成器")
    parser.add_argument("--self-test", action="store_true", help="运行门禁差分自证")
    parser.add_argument("--no-preview", action="store_true", help="不渲染审阅图片")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return

    # 默认主流程：自测 -> 导出 bbmodel -> 渲染审阅图
    self_test()

    item = stone_axe()
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
