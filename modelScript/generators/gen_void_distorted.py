#!/usr/bin/env python3
"""渊空畸变体的逐部件 Blockbench 生成器。

本轮只实现建造计划的 ``01_spine_ribcage``：驼背骨脊、短骨刺和外露
肋笼。后续部件会在调度确认本轮形状后逐件加入，避免用一个整件模型掩盖
接缝、比例或材质的问题。

门禁：
  - ``_assert_no_coplanar_faces`` 拦截同平面重叠，避免渲染时闪烁；
  - ``--self-test`` 注入一个共面骨片，证明门禁确实能拒绝缺陷；
  - ``--part 01`` 生成单件正面、侧面、3/4 渲染和参考图对照卡。
"""

from __future__ import annotations

import argparse
import base64
import io
import json
from pathlib import Path
from typing import Iterable
import uuid

import numpy as np
from PIL import Image, ImageDraw

REPO = Path(__file__).resolve().parents[2]
BBMODEL_OUT = Path(__file__).resolve().parents[1] / "models" / "VoidDistorted.bbmodel"
PARTS_DIR = Path(
    "/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/"
    "void_distorted/parts"
)
PARTS_REF_DIR = Path(
    "/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/"
    "void_distorted/parts_ref"
)

RESOLUTION = 64
PALETTE = {
    "bone": [216, 204, 176],
    "bone_shadow": [184, 168, 136],
    "rust_dark": [74, 64, 56],
    "rust_light": [106, 90, 72],
    "rust_spot": [138, 96, 64],
    "flesh_red": [138, 42, 42],
    "flesh_dark": [90, 26, 26],
    "void_black": [14, 12, 12],
}

# 每种材质占贴图上的一个 16×16 区块。材质名和坐标都集中在这里，后续
# 部件只引用 PALETTE 中的名字，避免生成器里出现未审定的颜色字面量。
MATERIAL_UV = {
    "bone": [0, 0, 16, 16],
    "bone_shadow": [16, 0, 32, 16],
    "rust_dark": [32, 0, 48, 16],
    "rust_light": [48, 0, 64, 16],
    "rust_spot": [0, 16, 16, 32],
    "flesh_red": [16, 16, 32, 32],
    "flesh_dark": [32, 16, 48, 32],
    "void_black": [48, 16, 64, 32],
}


def _cube(
    name: str,
    low: tuple[float, float, float],
    high: tuple[float, float, float],
    material: str,
) -> dict:
    """创建一个带有语义名称的轴对齐骨片。"""
    if material not in PALETTE:
        raise ValueError(f"未审定的材质：{material}")
    if any(a >= b for a, b in zip(low, high)):
        raise ValueError(f"部件 {name} 的 from/to 无效：{low} -> {high}")
    return {
        "name": name,
        "from": list(low),
        "to": list(high),
        "group": "spine_ribcage",
        "material": material,
    }


def part_spine_ribcage() -> list[dict]:
    """生成 01 部件：拱起骨脊、短骨刺和下挂的外露肋笼。

    坐标约定为 X 左右、Y 向上、Z 朝向生物正面。骨脊从后方低点沿 Z
    轴拱到中段高点，再向前下倾；肋骨从脊柱两侧向外、向下分层，中央留
    出虚空黑腔，使正面和侧面都能读出“骨架悬空”的形状。
    """
    cubes: list[dict] = []

    # 背部骨脊：每节略微改变高度和厚度，形成驼背弧线，而不是一根直梁。
    vertebrae = [
        ("rear", -5.6, -3.9, 10.6, 11.8, 1.45),
        ("rear_mid", -4.3, -2.6, 11.5, 12.9, 1.55),
        ("mid_low", -3.0, -1.3, 12.6, 14.2, 1.7),
        ("crest", -1.7, 0.1, 13.8, 15.7, 1.9),
        ("front_crest", -0.4, 1.3, 14.2, 15.6, 1.75),
        ("front_slope", 0.9, 2.6, 13.2, 14.6, 1.6),
        ("front_tip", 2.2, 3.9, 12.0, 13.4, 1.45),
    ]
    for index, (label, z0, z1, y0, y1, half_width) in enumerate(vertebrae):
        material = "bone" if index % 2 == 0 else "bone_shadow"
        cubes.append(
            _cube(
                f"spine_vertebra_{label}",
                (-half_width, y0, z0),
                (half_width, y1, z1),
                material,
            )
        )

    # 脊椎之间的窄连接节不与相邻顶面共面，保持弧线连续但不产生闪面。
    connector_specs = [
        ("rear", -4.05, -3.72, 11.55, 12.45, 1.0),
        ("rear_mid", -2.75, -2.42, 12.55, 13.55, 1.05),
        ("mid", -1.45, -1.12, 13.65, 14.75, 1.1),
        ("crest", -0.18, 0.18, 14.85, 15.95, 1.15),
        ("front", 1.12, 1.48, 13.85, 14.75, 1.0),
        ("tip", 2.52, 2.86, 12.65, 13.55, 0.9),
    ]
    for label, z0, z1, y0, y1, half_width in connector_specs:
        cubes.append(
            _cube(
                f"spine_connector_{label}",
                (-half_width, y0, z0),
                (half_width, y1, z1),
                "bone_shadow",
            )
        )

    # 脊顶短刺：一排分散、长短不一的细骨条，不合并成块。
    spike_specs = [
        ("rear", -4.95, 11.82, 0.72),
        ("rear_mid", -3.65, 12.95, 0.92),
        ("mid_low", -2.35, 14.25, 1.05),
        ("crest_a", -1.15, 15.72, 1.32),
        ("crest_b", 0.05, 15.68, 1.12),
        ("front_crest", 1.05, 15.64, 0.86),
        ("front_slope", 2.05, 14.7, 0.7),
        ("front_tip", 3.15, 13.52, 0.55),
    ]
    for index, (label, z, bottom, height) in enumerate(spike_specs):
        width = 0.34 if index % 2 else 0.42
        depth = 0.58 if index % 2 else 0.68
        cubes.append(
            _cube(
                f"spine_short_spike_{label}",
                (-width, bottom, z - depth / 2),
                (width, bottom + height, z + depth / 2),
                "bone",
            )
        )

    # 虚空黑腔是肋笼后的退让面；它只负责衬出骨条，不冒充另一件部件。
    cubes.append(
        _cube(
            "ribcage_void_cavity",
            (-3.25, 5.25, -0.1),
            (3.25, 11.12, 0.72),
            "void_black",
        )
    )

    # 四层肋骨，每层由左右外弧、下坠端节和中央胸骨构成。各层高度错开，
    # 既保留间隙，又避免门禁把相邻肋条判为共面重叠。
    rib_levels = [
        ("lower", 5.75, 3.75, 1.3),
        ("low_mid", 7.0, 4.05, 1.48),
        ("upper_mid", 8.35, 4.3, 1.66),
        ("upper", 9.75, 4.0, 1.86),
        ("top", 10.92, 3.55, 2.04),
    ]
    for index, (label, y, radius, front_z) in enumerate(rib_levels):
        thickness = 0.46 if index < 3 else 0.42
        outer_x = radius + 0.35
        inner_x = 0.9 + index * 0.04
        material = "bone_shadow" if index % 2 else "bone"
        cubes.extend(
            [
                _cube(
                    f"rib_{label}_left_arc",
                    (-outer_x, y, front_z - 0.82),
                    (-inner_x, y + thickness, front_z + 0.1),
                    material,
                ),
                _cube(
                    f"rib_{label}_right_arc",
                    (inner_x, y, front_z - 0.82),
                    (outer_x, y + thickness, front_z + 0.1),
                    material,
                ),
                _cube(
                    f"rib_{label}_left_drop",
                    (-outer_x - 0.34, y - 0.62, front_z - 0.22),
                    (-outer_x + 0.05, y + 0.02, front_z + 0.52),
                    "bone_shadow",
                ),
                _cube(
                    f"rib_{label}_right_drop",
                    (outer_x - 0.05, y - 0.62, front_z - 0.22),
                    (outer_x + 0.34, y + 0.02, front_z + 0.52),
                    "bone_shadow",
                ),
                _cube(
                    f"rib_{label}_sternum",
                    (-inner_x + 0.1, y + 0.18, front_z + 0.13),
                    (inner_x - 0.1, y + 0.18 + thickness, front_z + 0.58),
                    "bone",
                ),
            ]
        )

    return cubes


def all_cubes() -> list[dict]:
    """返回当前已审定的部件；后续部件按调度反馈逐步追加。"""
    return part_spine_ribcage()


def _assert_no_coplanar_faces(cubes: Iterable[dict]) -> None:
    """拒绝同一朝向、同一平面上有面积重叠的两个面。"""
    faces = {"+X": [], "-X": [], "+Y": [], "-Y": [], "+Z": [], "-Z": []}
    for cube in cubes:
        low = cube["from"]
        high = cube["to"]
        name = cube["name"]
        faces["-X"].append((low[0], (low[1], high[1], low[2], high[2]), name))
        faces["+X"].append((high[0], (low[1], high[1], low[2], high[2]), name))
        faces["-Y"].append((low[1], (low[0], high[0], low[2], high[2]), name))
        faces["+Y"].append((high[1], (low[0], high[0], low[2], high[2]), name))
        faces["-Z"].append((low[2], (low[0], high[0], low[1], high[1]), name))
        faces["+Z"].append((high[2], (low[0], high[0], low[1], high[1]), name))

    for axis, plane_faces in faces.items():
        for index, (position, (u0, u1, v0, v1), name) in enumerate(plane_faces):
            for other_position, (other_u0, other_u1, other_v0, other_v1), other_name in plane_faces[index + 1 :]:
                if abs(position - other_position) >= 1e-4:
                    continue
                overlap_u = max(0.0, min(u1, other_u1) - max(u0, other_u0))
                overlap_v = max(0.0, min(v1, other_v1) - max(v0, other_v0))
                if overlap_u > 1e-3 and overlap_v > 1e-3:
                    raise ValueError(
                        f"共面冲突: {name} 与 {other_name} 在 {axis} 面共面 "
                        f"({position:.4f}), 重叠区域 ({overlap_u:.3f}x{overlap_v:.3f})"
                    )


def build_texture() -> Image.Image:
    """生成只含计划配色的 64×64 材质图集。"""
    texture = Image.new("RGBA", (RESOLUTION, RESOLUTION), tuple(PALETTE["void_black"] + [255]))
    draw = ImageDraw.Draw(texture)
    for material, (x0, y0, x1, y1) in MATERIAL_UV.items():
        draw.rectangle((x0, y0, x1 - 1, y1 - 1), fill=tuple(PALETTE[material] + [255]))
    return texture


def generate_bbmodel(out_path: Path = BBMODEL_OUT, cubes_override: list[dict] | None = None) -> Path:
    """写出当前部件集合的 Blockbench free-format 模型。"""
    out_path.parent.mkdir(parents=True, exist_ok=True)
    cubes = list(all_cubes() if cubes_override is None else cubes_override)
    _assert_no_coplanar_faces(cubes)

    texture = build_texture()
    buffer = io.BytesIO()
    texture.save(buffer, format="PNG")
    texture_base64 = "data:image/png;base64," + base64.b64encode(buffer.getvalue()).decode("ascii")

    elements = []
    for cube in cubes:
        uv = MATERIAL_UV[cube["material"]]
        faces = {side: {"uv": uv, "texture": 0} for side in ("north", "south", "east", "west", "up", "down")}
        elements.append(
            {
                "name": cube["name"],
                "box_uv": False,
                "from": cube["from"],
                "to": cube["to"],
                "faces": faces,
                "uuid": str(uuid.uuid4()),
            }
        )

    model = {
        "meta": {"format_version": "4.10", "model_format": "free"},
        "name": "void_distorted",
        "resolution": {"width": RESOLUTION, "height": RESOLUTION},
        "elements": elements,
        "outliner": [
            {
                "name": "spine_ribcage",
                "origin": [0.0, 10.0, 0.0],
                "children": [element["uuid"] for element in elements],
            }
        ],
        "textures": [
            {
                "name": "void_distorted",
                "folder": "entity",
                "namespace": "bong",
                "id": 0,
                "source": texture_base64,
            }
        ],
    }
    out_path.write_text(json.dumps(model, indent=2), encoding="utf-8")
    print(f"✓ VoidDistorted 01_spine_ribcage 写入成功: {out_path.relative_to(REPO)}")
    return out_path


def _crop_tight(image: Image.Image, threshold: float = 15.0) -> Image.Image:
    """按渲染背景裁掉空白，只保留当前部件。"""
    pixels = np.asarray(image.convert("RGB"))
    background = pixels[0, 0].astype(float)
    changed = np.linalg.norm(pixels.astype(float) - background, axis=2) > threshold
    ys, xs = np.where(changed)
    if len(xs) == 0:
        return image.convert("RGB")
    return image.convert("RGB").crop((xs.min(), ys.min(), xs.max() + 1, ys.max() + 1))


def _crop_reference_panels(path: Path) -> tuple[Image.Image, Image.Image]:
    """从左正面、右侧面的参考图中裁出部件本身，去掉灰色背景。"""
    image = Image.open(path).convert("RGB")
    array = np.asarray(image).astype(float)
    midpoint = image.width // 2
    panels = []
    for start, end in ((0, midpoint), (midpoint, image.width)):
        panel = array[:, start:end]
        border = np.concatenate(
            [
                panel[:20].reshape(-1, 3),
                panel[-20:].reshape(-1, 3),
                panel[:, :20].reshape(-1, 3),
                panel[:, -20:].reshape(-1, 3),
            ]
        )
        background = np.median(border, axis=0)
        changed = np.linalg.norm(panel - background, axis=2) > 20
        ys, xs = np.where(changed)
        if len(xs) == 0:
            raise ValueError(f"参考图 {path} 的面板 {start}:{end} 没有可裁剪部件")
        panels.append(image.crop((xs.min() + start, ys.min(), xs.max() + start + 1, ys.max() + 1)))
    return panels[0], panels[1]


def _scale_to_height(image: Image.Image, height: int) -> Image.Image:
    width = max(1, int(image.width * height / image.height))
    return image.resize((width, height), Image.Resampling.LANCZOS)


def render_step_01() -> None:
    """输出 01 单件三视图与参考图并排对照卡。"""
    from bbmodel_maker.render.render_bbmodel import render

    PARTS_DIR.mkdir(parents=True, exist_ok=True)
    temporary_model = Path("/tmp/VoidDistorted_step_01_spine_ribcage.bbmodel")
    generate_bbmodel(temporary_model, cubes_override=part_spine_ribcage())

    front, _ = render(str(temporary_model), yaw=0, pitch=0, size=600)
    side, _ = render(str(temporary_model), yaw=90, pitch=0, size=600)
    three_quarter, _ = render(str(temporary_model), yaw=-35, pitch=20, size=600)
    temporary_model.unlink(missing_ok=True)

    rendered = [_scale_to_height(_crop_tight(image), 600) for image in (front, side, three_quarter)]
    reference_path = PARTS_REF_DIR / "01_spine_ribcage.png"
    if not reference_path.exists():
        raise FileNotFoundError(f"未找到参考图：{reference_path}")
    reference = [_scale_to_height(image, 600) for image in _crop_reference_panels(reference_path)]

    gap = 20
    card_width = sum(image.width for image in reference + rendered) + gap * 5 + 40
    card_height = 680
    card = Image.new("RGB", (card_width, card_height), (28, 30, 34))
    draw = ImageDraw.Draw(card)
    x = 20
    draw.text((25, 20), "REF: 01_spine_ribcage (Front / Side)", fill=(216, 204, 176))
    for image in reference:
        card.paste(image, (x, 60))
        x += image.width + gap
    draw.line((x - gap // 2, 20, x - gap // 2, card_height - 20), fill=(80, 84, 92), width=2)
    draw.text((x + 10, 20), "NOW: part_spine_ribcage() (Front / Side / 3/4)", fill=(230, 230, 230))
    for image in rendered:
        card.paste(image, (x, 60))
        x += image.width + gap

    check_path = PARTS_DIR / "check_01_spine_ribcage.png"
    card.save(check_path)
    print(f"✓ 01_spine_ribcage 并排对照图已输出: {check_path}")

    sheet_width = sum(image.width for image in rendered) + gap * 4
    sheet = Image.new("RGB", (sheet_width, 640), (28, 30, 34))
    sheet_draw = ImageDraw.Draw(sheet)
    sheet_draw.text((20, 10), "01_spine_ribcage (Front / Side / 3/4)", fill=(230, 230, 230))
    x = gap
    for image in rendered:
        sheet.paste(image, (x, 30))
        x += image.width + gap
    single_path = PARTS_DIR / "01_spine_ribcage.png"
    sheet.save(single_path)
    print(f"✓ 01_spine_ribcage 单部件渲染图已输出: {single_path}")


def self_test() -> None:
    """验证正常部件通过，并验证注入共面骨片会被拒绝。"""
    cubes = part_spine_ribcage()
    _assert_no_coplanar_faces(cubes)
    print("  [OK] 01_spine_ribcage 无共面冲突")

    defective = list(cubes)
    defective.append(
        _cube(
            "inject_coplanar_fail",
            (-1.0, 11.8, -5.0),
            (1.0, 12.7, -3.5),
            "bone",
        )
    )
    try:
        _assert_no_coplanar_faces(defective)
    except ValueError as error:
        print(f"  [OK] 成功捕获注入缺陷: {error}")
    else:
        raise RuntimeError("FAIL: 缺陷注入未被共面检查拦截")


def main() -> None:
    parser = argparse.ArgumentParser(description="生成渊空畸变体 01_spine_ribcage 部件")
    parser.add_argument("--self-test", action="store_true", help="运行共面门禁差分自证")
    parser.add_argument("--part", default="01", help="当前只支持 01")
    parser.add_argument("--out", type=Path, default=BBMODEL_OUT, help="输出 .bbmodel 路径")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return
    if args.part != "01":
        raise SystemExit("当前调度只允许生成 01_spine_ribcage")
    generate_bbmodel(args.out)
    render_step_01()


if __name__ == "__main__":
    main()
