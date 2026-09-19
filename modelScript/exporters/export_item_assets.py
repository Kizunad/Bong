#!/usr/bin/env python3
"""把 Blockbench item ``.bbmodel`` 导出成 SML OBJ 运行时资产。

这条转换链和 ``export_coffin_assets.py`` 的 GeckoLib entity 导出完全不同：
item 的运行时模型是 OBJ/MTL，贴图是 bbmodel 内嵌的 atlas，model JSON 只负责
把 OBJ 交给 SML。转换器故意只接受 cube + 单张内嵌贴图；遇到不支持的输入要
直接报错，不能静默产出一个看起来完整但几何不对的资产。

用法示例::

    python3 modelScript/exporters/export_item_assets.py \
        --input modelScript/models/GuYuanPill.bbmodel --id guyuan_pill

``--all-pills`` 是本批八个丹药的可复现批量入口。默认输出到 Bong 的 client
资源树；``--output-root`` 可用于测试或先导出到临时目录。
"""

from __future__ import annotations

import argparse
import base64
import io
import json
import math
from dataclasses import dataclass, replace
from pathlib import Path
from typing import Iterable

from PIL import Image


REPO = Path(__file__).resolve().parents[2]
MODELS = Path(__file__).resolve().parents[1] / "models"
ASSETS = REPO / "client" / "src" / "main" / "resources" / "assets" / "bong"

FACE_ORDER = ("south", "north", "west", "east", "up", "down")
FACE_VERTICES = {
    "south": (4, 5, 6, 7),
    "north": (1, 0, 3, 2),
    "west": (0, 4, 7, 3),
    "east": (5, 1, 2, 6),
    "up": (7, 6, 2, 3),
    "down": (0, 1, 5, 4),
}
FACE_NORMALS = {
    "south": (0.0, 0.0, 1.0),
    "north": (0.0, 0.0, -1.0),
    "west": (-1.0, 0.0, 0.0),
    "east": (1.0, 0.0, 0.0),
    "up": (0.0, 1.0, 0.0),
    "down": (0.0, -1.0, 0.0),
}


@dataclass(frozen=True)
class ExportOptions:
    """一个 item 出料系的显式参数。

    bbmodel 的几何坐标是 px；OBJ 需要除以 16 后落进 MC 方块坐标。offset 是
    在 px 中加的出料平移。display 的 ``centre_px`` 是握持点到模型几何中心的
    标定距离，沿用仓库现有三件 item 资产的 hand-display 约定。
    """

    offset: tuple[float, float, float]
    y_rotation_sign: float = 1.0
    hand_scale: float = 0.8
    firstperson_scale: float | None = None
    gui_scale: float = 0.9
    centre_px: float = 0.0


# 这三个预设不是另造一套转换逻辑，而是三件已验收资产的 compatibility oracle。
#
# Beast/Herb 的 OBJ 直接按 bbmodel 的标准 Blockbench Euler 旋转即可。竹剑的历史
# OBJ 则对 ``BambooJianSingle`` 的 b 变体把 Y 旋转取了反号；它们是方形截面，
# 形体集合看起来仍相同，但面/UV 朝向不同。这里把这个已存在的字节事实锁成
# 明确预设，generic 路径仍使用标准旋转，既不修改主线产物也不把历史差异藏起来。
REFERENCE_PRESETS = {
    "bamboo_jian": ExportOptions(
        offset=(8.0, 5.75, 8.0),
        y_rotation_sign=-1.0,
        hand_scale=0.88,
        firstperson_scale=0.84,
        gui_scale=0.9,
        centre_px=8.7,
    ),
    "beast_spine_sword": ExportOptions(
        offset=(0.0, 0.0, 0.0),
        hand_scale=0.8,
        firstperson_scale=0.76,
        gui_scale=0.78,
        centre_px=10.5,
    ),
    "herb_sickle": ExportOptions(
        offset=(0.0, 0.0, 0.0),
        hand_scale=0.85,
        firstperson_scale=0.81,
        gui_scale=1.15,
        centre_px=2.7,
    ),
}

PILL_ASSETS = {
    "guyuan_pill": "GuYuanPill.bbmodel",
    "kaimai_dan": "KaimaiDan.bbmodel",
    "huiyuan_pill": "HuiyuanPill.bbmodel",
    "anti_spirit_pressure_pill": "AntiSpiritPressurePill.bbmodel",
    "huo_xue_dan": "HuoXueDan.bbmodel",
    "jin_zhong_dan": "JinZhongDan.bbmodel",
    "ji_feng_dan": "JiFengDan.bbmodel",
    "hui_li_dan": "HuiLiDan.bbmodel",
}


def _load(path: Path) -> dict:
    """读取 JSON，并给调用方一个统一的 UTF-8 入口。"""

    return json.loads(path.read_text(encoding="utf-8"))


def _bounds(bb: dict) -> tuple[list[float], list[float]]:
    elements = bb.get("elements") or []
    if not elements:
        raise ValueError("bbmodel 没有 elements，无法导出 item 几何")
    minimum = [float("inf")] * 3
    maximum = [float("-inf")] * 3
    for element in elements:
        for point_name in ("from", "to"):
            point = element.get(point_name)
            if not isinstance(point, list) or len(point) != 3:
                raise ValueError(f"元素 {element.get('name')!r} 缺少合法 {point_name}")
            for axis in range(3):
                minimum[axis] = min(minimum[axis], float(point[axis]))
                maximum[axis] = max(maximum[axis], float(point[axis]))
    return minimum, maximum


def _validate_bbmodel(bb: dict) -> None:
    """验证本导出器能表达的 bbmodel 子集，失败时给出修复线索。"""

    resolution = bb.get("resolution") or {}
    width, height = resolution.get("width"), resolution.get("height")
    if not isinstance(width, (int, float)) or not isinstance(height, (int, float)):
        raise ValueError("bbmodel 缺少 resolution.width/height")
    if width <= 0 or height <= 0:
        raise ValueError(f"非法贴图分辨率 {width}x{height}")

    textures = bb.get("textures") or []
    if not textures or not textures[0].get("source"):
        raise ValueError("item 导出要求 textures[0].source 内嵌贴图")

    for element in bb.get("elements") or []:
        if element.get("type", "cube") != "cube":
            raise ValueError(
                f"不支持非 cube 元素 {element.get('name')!r}；"
                "item OBJ 导出暂不支持 mesh"
            )
        rotation = element.get("rotation") or [0.0, 0.0, 0.0]
        if len(rotation) != 3:
            raise ValueError(f"元素 {element.get('name')!r} 的 rotation 不是三轴")
        if any(float(value) for value in rotation) and not element.get("origin"):
            raise ValueError(
                f"旋转元素 {element.get('name')!r} 没有 origin，"
                "不能安全计算旋转后的顶点"
            )
        faces = element.get("faces") or {}
        for face_name in FACE_ORDER:
            face = faces.get(face_name)
            if face is None or len(face.get("uv", [])) != 4:
                raise ValueError(f"元素 {element.get('name')!r} 缺少 {face_name} 面 UV")
            if face.get("texture", 0) != 0:
                raise ValueError(
                    f"元素 {element.get('name')!r}/{face_name} 使用 texture="
                    f"{face.get('texture')}；本导出器只接受单张 atlas"
                )
            if face.get("rotation", 0):
                raise ValueError(
                    f"元素 {element.get('name')!r}/{face_name} 有 texture rotation，"
                    "请先在 Blockbench 烘平 UV"
                )


def _embedded_texture(bb: dict) -> bytes:
    """取出内嵌 PNG 原字节；不重新编码，保证 atlas 可逐字节复现。"""

    source = bb["textures"][0]["source"]
    payload = source.split(",", 1)[1] if "," in source else source
    try:
        raw = base64.b64decode(payload, validate=True)
    except Exception as exc:  # binascii.Error 的 Python 版本名不稳定，统一报 ValueError
        raise ValueError("textures[0].source 不是合法 base64 data URI") from exc
    try:
        with Image.open(io.BytesIO(raw)) as image:
            actual = image.size
    except Exception as exc:
        raise ValueError("textures[0].source 不是可读 PNG") from exc
    expected = (int(bb["resolution"]["width"]), int(bb["resolution"]["height"]))
    if actual != expected:
        raise ValueError(
            f"内嵌 atlas 尺寸 {actual[0]}x{actual[1]} 与 bbmodel resolution "
            f"{expected[0]}x{expected[1]} 不一致"
        )
    return raw


def _rotate(vector: Iterable[float], rotation: Iterable[float]) -> tuple[float, float, float]:
    """按 Blockbench/OBJ 现有约定应用 Euler：先 Z，再 Y，再 X。"""

    x, y, z = (float(value) for value in vector)
    rx, ry, rz = (math.radians(float(value)) for value in rotation)

    cos_a, sin_a = math.cos(rz), math.sin(rz)
    x, y = x * cos_a - y * sin_a, x * sin_a + y * cos_a

    cos_a, sin_a = math.cos(ry), math.sin(ry)
    x, z = x * cos_a + z * sin_a, -x * sin_a + z * cos_a

    cos_a, sin_a = math.cos(rx), math.sin(rx)
    y, z = y * cos_a - z * sin_a, y * sin_a + z * cos_a

    # 避免 sin(0) 经负号传播成 -0.000000，保持生成资产的人类可读格式稳定。
    return tuple(0.0 if abs(value) < 1e-12 else value for value in (x, y, z))


def _effective_rotation(element: dict, y_rotation_sign: float) -> tuple[float, float, float]:
    rotation = list(element.get("rotation") or (0.0, 0.0, 0.0))
    rotation[1] = float(rotation[1]) * y_rotation_sign
    return tuple(float(value) for value in rotation)


def _fmt6(value: float) -> str:
    value = 0.0 if abs(value) < 0.0000005 else value
    return f"{value:.6f}"


def _source_label(path: Path) -> str:
    try:
        return path.resolve().relative_to(REPO.resolve()).as_posix()
    except ValueError:
        return path.name


def build_obj(bb: dict, identifier: str, source_path: Path, options: ExportOptions) -> str:
    """生成与 #2301 资产格式一致的 OBJ 文本。"""

    width = float(bb["resolution"]["width"])
    height = float(bb["resolution"]["height"])
    lines = [
        f"# {identifier}.obj -- generated from {_source_label(source_path)}",
        "# Keep this file in sync with the source bbmodel and its embedded atlas.",
        f"mtllib {identifier}.mtl",
        f"o {identifier}",
    ]

    for element_index, element in enumerate(bb["elements"]):
        name = element.get("name") or element.get("uuid") or f"element_{element_index}"
        lines.append(f"# part: {name}")
        frm = tuple(float(value) for value in element["from"])
        to = tuple(float(value) for value in element["to"])
        origin = tuple(float(value) for value in (element.get("origin") or (0.0, 0.0, 0.0)))
        rotation = _effective_rotation(element, options.y_rotation_sign)
        corners = (
            (frm[0], frm[1], frm[2]),
            (to[0], frm[1], frm[2]),
            (to[0], to[1], frm[2]),
            (frm[0], to[1], frm[2]),
            (frm[0], frm[1], to[2]),
            (to[0], frm[1], to[2]),
            (to[0], to[1], to[2]),
            (frm[0], to[1], to[2]),
        )
        for corner in corners:
            local = tuple(corner[axis] - origin[axis] for axis in range(3))
            rotated = _rotate(local, rotation)
            emitted = tuple(
                rotated[axis] + origin[axis] + options.offset[axis] for axis in range(3)
            )
            lines.append("v " + " ".join(_fmt6(value / 16.0) for value in emitted))

        base_vertex = element_index * 8
        for face_index, face_name in enumerate(FACE_ORDER):
            u1, v1, u2, v2 = (float(value) for value in element["faces"][face_name]["uv"])
            for u, v in ((u1, v1), (u2, v1), (u2, v2), (u1, v2)):
                lines.append(f"vt {_fmt6(u / width)} {_fmt6(1.0 - v / height)}")

            normal = _rotate(FACE_NORMALS[face_name], rotation)
            lines.append("vn " + " ".join(_fmt6(value) for value in normal))
            if face_index == 0:
                lines.append(f"usemtl {identifier}_atlas")

            vertex_order = FACE_VERTICES[face_name]
            vt_base = element_index * 24 + face_index * 4 + 1
            normal_index = element_index * 6 + face_index + 1
            lines.append(
                "f "
                + " ".join(
                    f"{base_vertex + vertex + 1}/{vt_base + corner}/{normal_index}"
                    for corner, vertex in enumerate(vertex_order)
                )
            )

    return "\n".join(lines) + "\n"


def build_mtl(identifier: str) -> str:
    return "\n".join(
        [
            f"# {identifier} materials -- source bbmodel atlas",
            "",
            f"newmtl {identifier}_atlas",
            "Ka 1.000000 1.000000 1.000000",
            "Kd 1.000000 1.000000 1.000000",
            "Ks 0.000000 0.000000 0.000000",
            "Ns 10.000000",
            "d 1.000000",
            "illum 1",
            f"map_Kd bong:item/{identifier}/atlas",
            "",
        ]
    )


def _centre_translation(
    rotation: tuple[float, float, float],
    scale: float,
    centre_px: float,
    target: tuple[float, float, float] = (0.0, 0.0, 0.0),
) -> list[float]:
    moved = _rotate((0.0, centre_px * scale, 0.0), rotation)
    return [round(target[index] - moved[index], 3) for index in range(3)]


def build_display(options: ExportOptions) -> dict:
    firstperson_scale = (
        options.firstperson_scale
        if options.firstperson_scale is not None
        else round(options.hand_scale - 0.04, 4)
    )
    return {
        "thirdperson_righthand": {
            "rotation": [-80, 90, 0],
            "translation": [0, -2.0, 1.5],
            "scale": [options.hand_scale] * 3,
        },
        "thirdperson_lefthand": {
            "rotation": [-80, -90, 0],
            "translation": [0, -2.0, 1.5],
            "scale": [options.hand_scale] * 3,
        },
        "firstperson_righthand": {
            "rotation": [-80, 90, 0],
            "translation": [0, -2.0, -4.0],
            "scale": [firstperson_scale] * 3,
        },
        "firstperson_lefthand": {
            "rotation": [-80, -90, 0],
            "translation": [0, -2.0, -4.0],
            "scale": [firstperson_scale] * 3,
        },
        "ground": {
            "rotation": [0, 0, 0],
            "scale": [0.45, 0.45, 0.45],
            "translation": _centre_translation(
                (0.0, 0.0, 0.0), 0.45, options.centre_px, (0.0, 2.0, 0.0)
            ),
        },
        "gui": {
            "rotation": [0, 0, 45],
            "scale": [options.gui_scale] * 3,
            "translation": _centre_translation(
                (0.0, 0.0, 45.0), options.gui_scale, options.centre_px
            ),
        },
        "fixed": {
            "rotation": [0, 180, 0],
            "scale": [1.0, 1.0, 1.0],
            "translation": _centre_translation(
                (0.0, 180.0, 0.0), 1.0, options.centre_px
            ),
        },
        "head": {
            "rotation": [0, 0, 0],
            "scale": [1.0, 1.0, 1.0],
            "translation": _centre_translation(
                (0.0, 0.0, 0.0), 1.0, options.centre_px, (0.0, 12.0, 0.0)
            ),
        },
    }


def build_model_json(identifier: str, options: ExportOptions) -> str:
    model = {
        "parent": "sml:builtin/obj",
        "model": f"bong:models/item/{identifier}/{identifier}.obj",
        "display": build_display(options),
    }
    return json.dumps(model, ensure_ascii=False, indent=2) + "\n"


def _default_options(bb: dict, identifier: str) -> ExportOptions:
    if identifier in REFERENCE_PRESETS:
        return REFERENCE_PRESETS[identifier]

    minimum, maximum = _bounds(bb)
    # 手工 pill bbmodel 以 (0, 0, 0) 为中心；已有 Beast/Herb 一类出料模型已经
    # 在 [0,16] px 方块系。只对越过负半轴的未知模型补 x/z=8，y 留给作者显式决定。
    offset = (
        8.0 if minimum[0] < 0.0 else 0.0,
        0.0,
        8.0 if minimum[2] < 0.0 else 0.0,
    )
    centre_px = (minimum[1] + maximum[1]) / 2.0
    return ExportOptions(offset=offset, centre_px=centre_px)


def _with_overrides(
    options: ExportOptions,
    *,
    offset: tuple[float, float, float] | None,
    y_rotation_sign: float | None,
    hand_scale: float | None,
    firstperson_scale: float | None,
    gui_scale: float | None,
    centre_px: float | None,
) -> ExportOptions:
    values = {}
    if offset is not None:
        values["offset"] = offset
    if y_rotation_sign is not None:
        values["y_rotation_sign"] = y_rotation_sign
    if hand_scale is not None:
        values["hand_scale"] = hand_scale
    if firstperson_scale is not None:
        values["firstperson_scale"] = firstperson_scale
    if gui_scale is not None:
        values["gui_scale"] = gui_scale
    if centre_px is not None:
        values["centre_px"] = centre_px
    return replace(options, **values)


def export_asset(
    source_path: Path,
    identifier: str,
    *,
    output_root: Path = ASSETS,
    options: ExportOptions | None = None,
) -> dict[str, Path]:
    """导出一件 item 的 JSON/MTL/OBJ/atlas，并返回实际写入路径。"""

    bb = _load(source_path)
    _validate_bbmodel(bb)
    selected = options or _default_options(bb, identifier)
    texture = _embedded_texture(bb)

    model_dir = output_root / "models" / "item" / identifier
    texture_dir = output_root / "textures" / "item" / identifier
    model_dir.mkdir(parents=True, exist_ok=True)
    texture_dir.mkdir(parents=True, exist_ok=True)

    outputs = {
        "json": model_dir / f"{identifier}.json",
        "mtl": model_dir / f"{identifier}.mtl",
        "obj": model_dir / f"{identifier}.obj",
        "atlas": texture_dir / "atlas.png",
    }
    outputs["obj"].write_text(
        build_obj(bb, identifier, source_path, selected), encoding="utf-8"
    )
    outputs["mtl"].write_text(build_mtl(identifier), encoding="utf-8")
    outputs["json"].write_text(
        build_model_json(identifier, selected), encoding="utf-8"
    )
    outputs["atlas"].write_bytes(texture)
    print(f"{identifier}: {len(bb['elements'])} cubes -> {model_dir}")
    return outputs


def _parse_offset(values: list[str] | None) -> tuple[float, float, float] | None:
    if values is None:
        return None
    return tuple(float(value) for value in values)  # type: ignore[return-value]


def main() -> None:
    parser = argparse.ArgumentParser(description="bbmodel → item OBJ/MTL/JSON + atlas exporter")
    parser.add_argument("--input", type=Path, help="输入 .bbmodel")
    parser.add_argument("--id", help="client/server 共用的 item id")
    parser.add_argument(
        "--all-pills",
        action="store_true",
        help="按 PILL_ASSETS 表导出八个丹药（逐项使用 server pills.toml 的 id）",
    )
    parser.add_argument(
        "--output-root",
        type=Path,
        default=ASSETS,
        help="bong 资源根目录，默认 client/src/main/resources/assets/bong",
    )
    parser.add_argument("--offset", nargs=3, metavar=("X", "Y", "Z"), help="px 出料平移")
    parser.add_argument(
        "--y-rotation-sign",
        type=float,
        choices=(-1.0, 1.0),
        help="Y 旋转符号；只应为历史兼容预设或明确的源模型约定使用",
    )
    parser.add_argument("--hand-scale", type=float)
    parser.add_argument("--firstperson-scale", type=float)
    parser.add_argument("--gui-scale", type=float)
    parser.add_argument("--centre-px", type=float)
    args = parser.parse_args()

    if args.all_pills:
        if args.input is not None or args.id is not None:
            parser.error("--all-pills 不能和 --input/--id 同时使用")
        if any(
            value is not None
            for value in (
                args.offset,
                args.y_rotation_sign,
                args.hand_scale,
                args.firstperson_scale,
                args.gui_scale,
                args.centre_px,
            )
        ):
            parser.error("--all-pills 使用每件模型的默认出料参数，不能混用单件覆盖参数")
        for identifier, filename in PILL_ASSETS.items():
            export_asset(MODELS / filename, identifier, output_root=args.output_root)
        return

    if args.input is None or args.id is None:
        parser.error("单件导出必须同时提供 --input 和 --id；或使用 --all-pills")
    options = _with_overrides(
        _default_options(_load(args.input), args.id),
        offset=_parse_offset(args.offset),
        y_rotation_sign=args.y_rotation_sign,
        hand_scale=args.hand_scale,
        firstperson_scale=args.firstperson_scale,
        gui_scale=args.gui_scale,
        centre_px=args.centre_px,
    )
    export_asset(args.input, args.id, output_root=args.output_root, options=options)


if __name__ == "__main__":
    main()
