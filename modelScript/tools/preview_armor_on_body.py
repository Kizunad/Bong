#!/usr/bin/env python3
"""把护甲件戴在一具**真 MC 玩家模型**上出正交视图，裸体/着甲两行对照。

`armor_model_common.write_material_assets` 出的三视图是**裸甲**——甲片悬空、
看不出和头/躯干的关系，前视还会从脸洞里看穿到后颈帘，读成一堵墙。判"戴上去
对不对"必须有身体，而且得是有脸、有头发、有衣服的身体：灰方块看不出脸洞框没框
住脸、护耳有没有盖住耳朵、后帘有没有压到领口。

玩家模型按原版 biped 尺寸摆（头 8³ 在 y24~32、躯干 8×12×4、四肢 4×12×4），
贴图用本文件里程序生成的 64×64 原版布局皮肤（末法残土的流民相：晒糙的脸、
乱发、粗麻衣、旧裤靴），不依赖外部素材。

做法：临时 bbmodel 把 resolution 提到 128x128，左半贴甲贴图（UV 数值不变即仍落
在原内容上），右半贴玩家皮肤（UV 原点整体 +64）。只是预览工具，不写任何游戏资产。
"""

from __future__ import annotations

import argparse
import base64
import io
import json
import random
import sys
import tempfile
import uuid
from pathlib import Path

from PIL import Image, ImageDraw

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "core"))
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "generators"))

from armor_model_common import ArmorPart, TEXTURE_SIZE  # noqa: E402
from render_bbmodel import render  # noqa: E402

import workspace  # noqa: E402

_WS = workspace.Workspace.discover(start=Path(__file__))
REPO = _WS.root
PREVIEW_RES = 128
SKIN_ORIGIN = (64, 0)  # 玩家皮肤在预览图集里的原点，UV 全部平移这个量
MODEL_NAMESPACE = uuid.uuid5(uuid.NAMESPACE_URL, "https://bong.local/armor-preview")

# 原版 biped 摆位（Bedrock 坐标，脚在 y=0）+ 原版 64x64 皮肤的 UV 原点。
# 这套 uv 和 _faces 的 box-uv 排布对得上原版：头脸落在 (8,8)-(16,16)，
# 躯干正面 (20,20)-(28,32)，右臂正面 (44,20)-(48,32)，左臂 (36,52)-(40,64)。
# 第 4 列是原版骨骼枢轴：导出的 bbmodel 里当 element/group origin 用，
# 在 Blockbench 里直接拖旋转就能摆姿势，不用自己找轴心。
PLAYER = (
    ("head", (-4, 24, -4), (8, 8, 8), (0, 0), (0, 24, 0)),
    ("body", (-4, 12, -2), (8, 12, 4), (16, 16), (0, 24, 0)),
    ("arm_right", (-8, 12, -2), (4, 12, 4), (40, 16), (-5, 22, 0)),
    ("arm_left", (4, 12, -2), (4, 12, 4), (32, 48), (5, 22, 0)),
    # 腿按**原版真实摆位**放：rightLeg 枢轴 (-1.9,12,0) + cuboid(-2,0,-2,4,12,4)
    # → 世界 x -3.9~0.1，左腿 -0.1~3.9（原版两条腿在中线本来就叠 0.2）。这里以前
    # 图省事写成 ±4 的理想布局，和 MOUNT_X=±1.9 的甲件差 0.1，判腿甲贴合度会假。
    ("leg_right", (-3.9, 0, -2), (4, 12, 4), (0, 16), (-1.9, 12, 0)),
    ("leg_left", (-0.1, 0, -2), (4, 12, 4), (16, 48), (1.9, 12, 0)),
)

# 灰模特（--grey 用）：不带脸，只判轮廓关系时更干净。
GREY_UV = (72, 72)
GREY = (150, 150, 152)
GREY_DARK = (120, 120, 123)

# 固定取景：两行必须用同一 focus，否则自动取景会因包围盒不同把两行错开，
# "戴上去高了/矮了"就成了取景假象。
FRAME = {
    "head": ((0.0, 27.5, 0.0), 26.0),
    "chest": ((0.0, 19.0, 0.0), 24.0),
    "legs": ((0.0, 8.0, 0.0), 21.0),
    "feet": ((0.0, 3.4, 0.0), 12.5),
}

# 视角约定与 render_bbmodel.THREE_VIEW_ANGLES 一致：背面剔除只留法线朝 +z 的面，
# 故 yaw=180 才是正面，yaw=0 看到的是背面。写反会把"脸洞被挡住"误读成建模错误。
VIEWS = (("FRONT", 180.0, 0.0), ("SIDE", 90.0, 0.0), ("BACK", 0.0, 0.0), ("3/4", 145.0, 12.0))


# ─── 皮肤 ─────────────────────────────────────────────────────────────────

SKIN = (176, 140, 108)
SKIN_SHADE = (146, 113, 86)
SKIN_LIT = (198, 162, 128)
HAIR = (46, 39, 34)
HAIR_LIT = (68, 57, 48)
CLOTH = (112, 104, 86)
CLOTH_DARK = (86, 79, 64)
TROUSER = (72, 64, 55)
BOOT = (52, 46, 40)


def _fill(draw, box, color):
    x, y, w, h = box
    draw.rectangle((x, y, x + w - 1, y + h - 1), fill=color)


def _grain(image, box, rng, amount=7):
    """加低频颗粒——纯色面在 MC 着色下会糊成一片，分不出朝向。"""
    x0, y0, w, h = box
    pixels = image.load()
    for y in range(y0, y0 + h):
        for x in range(x0, x0 + w):
            jitter = rng.randint(-amount, amount)
            pixels[x, y] = tuple(max(0, min(255, c + jitter)) for c in pixels[x, y])


def make_player_skin() -> Image.Image:
    """程序生成一张原版布局 64x64 皮肤：末法残土的流民相。

    仓库里没有任何玩家皮肤素材（`*skin*.png` 只有水囊图标），所以这里自己画。
    只要 face/hair/cloth 的分区对，就足够判"盔戴上去脸露多少、护耳压不压耳朵"。
    """
    rng = random.Random(0x5EED)
    skin = Image.new("RGB", (64, 64), (0, 0, 0))
    draw = ImageDraw.Draw(skin)

    # ── 头：上/下/西/北(脸)/东/南(后脑) ──
    _fill(draw, (8, 0, 8, 8), HAIR)            # 头顶
    _fill(draw, (16, 0, 8, 8), SKIN_SHADE)     # 下巴底
    _fill(draw, (0, 8, 8, 8), SKIN)            # 左侧脸
    _fill(draw, (0, 8, 8, 4), HAIR)            # 左鬓角
    _fill(draw, (16, 8, 8, 8), SKIN)           # 右侧脸
    _fill(draw, (16, 8, 8, 4), HAIR)           # 右鬓角
    _fill(draw, (24, 8, 8, 8), HAIR)           # 后脑
    _fill(draw, (24, 13, 8, 3), SKIN_SHADE)    # 后颈
    _fill(draw, (8, 8, 8, 8), SKIN)            # 正脸
    _fill(draw, (8, 8, 8, 2), HAIR)            # 刘海
    for x in range(8, 16):                      # 刘海参差，不要一条平边
        if rng.random() < 0.55:
            draw.point((x, 10), fill=HAIR)
    draw.point((9, 11), fill=HAIR_LIT)          # 眉
    draw.point((10, 11), fill=HAIR_LIT)
    draw.point((13, 11), fill=HAIR_LIT)
    draw.point((14, 11), fill=HAIR_LIT)
    draw.rectangle((9, 12, 10, 12), fill=(52, 44, 40))   # 左眼
    draw.rectangle((13, 12, 14, 12), fill=(52, 44, 40))  # 右眼
    draw.point((9, 12), fill=(226, 222, 214))
    draw.point((13, 12), fill=(226, 222, 214))
    draw.point((11, 13), fill=SKIN_SHADE)       # 鼻
    draw.point((12, 13), fill=SKIN_SHADE)
    draw.rectangle((10, 15, 13, 15), fill=(126, 92, 78))  # 嘴
    draw.point((15, 13), fill=SKIN_SHADE)       # 颧骨阴影/风霜
    draw.point((8, 14), fill=SKIN_SHADE)
    for box in ((8, 0, 8, 8), (24, 8, 8, 5)):   # 乱发
        for _ in range(14):
            x = rng.randrange(box[0], box[0] + box[2])
            y = rng.randrange(box[1], box[1] + box[3])
            draw.point((x, y), fill=HAIR_LIT)

    # ── 躯干：粗麻衣，领口露一截皮肤，腰上一条布带 ──
    for box in ((16, 16, 8, 4), (24, 16, 8, 4), (16, 20, 4, 12),
                (20, 20, 8, 12), (28, 20, 4, 12), (32, 20, 8, 12)):
        _fill(draw, box, CLOTH)
    _fill(draw, (20, 20, 8, 2), CLOTH_DARK)     # 前领口
    _fill(draw, (22, 20, 4, 2), SKIN_SHADE)     # 敞着的胸口
    _fill(draw, (20, 27, 8, 1), CLOTH_DARK)     # 腰带
    _fill(draw, (32, 27, 8, 1), CLOTH_DARK)
    for _ in range(26):                          # 补丁与磨损
        x, y = rng.randrange(20, 28), rng.randrange(22, 32)
        draw.point((x, y), fill=CLOTH_DARK)

    # ── 四肢：袖子挽到小臂，裤腿塞进旧靴 ──
    for u, v in ((40, 16), (32, 48)):
        _fill(draw, (u, v, 16, 4), CLOTH)                 # 肩顶/袖口底
        _fill(draw, (u, v + 4, 16, 12), SKIN)             # 整圈先铺皮肤
        _fill(draw, (u, v + 4, 16, 6), CLOTH)             # 上半截袖子
        _fill(draw, (u, v + 9, 16, 1), CLOTH_DARK)        # 挽起来的袖边
        _grain(skin, (u, v, 16, 16), rng, 6)
    for u, v in ((0, 16), (16, 48)):
        _fill(draw, (u, v, 16, 4), TROUSER)
        _fill(draw, (u, v + 4, 16, 12), TROUSER)
        _fill(draw, (u, v + 13, 16, 3), BOOT)             # 靴筒
        _fill(draw, (u, v + 12, 16, 1), (40, 35, 30))     # 靴口
        _grain(skin, (u, v, 16, 16), rng, 6)

    for box in ((8, 0, 24, 16), (16, 16, 24, 16), (32, 20, 8, 12)):
        _grain(skin, box, rng, 5)
    return skin


def make_grey_skin() -> Image.Image:
    skin = Image.new("RGB", (64, 64), GREY)
    draw = ImageDraw.Draw(skin)
    for y in range(0, 64, 3):
        draw.line((0, y, 64, y), fill=GREY_DARK)
    return skin


# ─── 模型装配 ──────────────────────────────────────────────────────────────


def _faces(size, uv):
    u, v = uv
    sx, sy, sz = size
    return {
        "west": {"uv": [u, v + sz, u + sz, v + sz + sy], "texture": 0},
        "north": {"uv": [u + sz, v + sz, u + sz + sx, v + sz + sy], "texture": 0},
        "east": {"uv": [u + sz + sx, v + sz, u + 2 * sz + sx, v + sz + sy], "texture": 0},
        "south": {"uv": [u + 2 * sz + sx, v + sz, u + 2 * (sz + sx), v + sz + sy], "texture": 0},
        "up": {"uv": [u + sz, v, u + sz + sx, v + sz], "texture": 0},
        "down": {"uv": [u + sz + sx, v, u + sz + 2 * sx, v + sz], "texture": 0},
    }


def build_preview_texture(armor_texture: Image.Image, skin: Image.Image) -> Image.Image:
    """左上角甲贴图（UV 不动），右上角玩家皮肤（UV 平移 SKIN_ORIGIN）。"""
    if armor_texture.size != (TEXTURE_SIZE, TEXTURE_SIZE):
        raise ValueError(f"armor texture must be {TEXTURE_SIZE}x{TEXTURE_SIZE}")
    canvas = Image.new("RGB", (PREVIEW_RES, PREVIEW_RES), (0, 0, 0))
    canvas.paste(armor_texture, (0, 0))
    canvas.paste(skin.convert("RGB"), SKIN_ORIGIN)
    return canvas


def _cube(name, origin, size, uv, pivot=(0, 0, 0)):
    end = tuple(origin[i] + size[i] for i in range(3))
    return {
        "name": name,
        "type": "cube",
        "uuid": str(uuid.uuid4()),
        "from": [round(v, 3) for v in origin],
        "to": [round(v, 3) for v in end],
        "origin": list(pivot),
        "faces": _faces(size, uv),
    }


def build_preview_model(part: ArmorPart | None, skin: Image.Image,
                        armor_texture: Image.Image) -> dict:
    from armor_model_common import MOUNT_PIVOT, MOUNT_X

    elements = []
    if part is not None:
        for cube in part.cubes:
            origin = (cube.origin[0] + MOUNT_X[cube.mount], cube.origin[1], cube.origin[2])
            elements.append(_cube(cube.name, origin, cube.size, cube.uv,
                                  MOUNT_PIVOT[cube.mount]))
    for name, origin, size, uv, pivot in PLAYER:
        elements.append(_cube(f"player_{name}", origin, size,
                              (uv[0] + SKIN_ORIGIN[0], uv[1] + SKIN_ORIGIN[1]), pivot))

    texture = build_preview_texture(armor_texture, skin)
    buf = io.BytesIO()
    texture.save(buf, format="PNG")
    return {
        "meta": {"format_version": "4.10", "model_format": "free", "box_uv": False},
        "name": f"{part.key if part else 'player'}_on_body",
        "resolution": {"width": PREVIEW_RES, "height": PREVIEW_RES},
        "elements": elements,
        "outliner": [e["uuid"] for e in elements],
        "textures": [{
            "id": "0", "name": "preview.png", "width": PREVIEW_RES, "height": PREVIEW_RES,
            "uv_width": PREVIEW_RES, "uv_height": PREVIEW_RES, "mode": "bitmap",
            "source": "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode("ascii"),
        }],
    }


def _group(name, origin, color, key, children):
    return {
        "name": name,
        "origin": [round(float(v), 3) for v in origin],
        "color": color,
        "uuid": str(uuid.uuid5(MODEL_NAMESPACE, f"group/{key}")),
        "export": True,
        "mirror_uv": False,
        "isOpen": True,
        "locked": False,
        "visibility": True,
        "autouv": 0,
        "children": children,
    }


def build_player_bbmodel(part: ArmorPart, skin: Image.Image,
                         armor_texture: Image.Image) -> dict:
    """导出可在 Blockbench 里直接打开的"甲 + 玩家"合模。

    和渲染用的临时模型同源（同一 build_preview_model 的几何与 UV），区别只在
    这份带稳定 uuid、分好组、枢轴按原版骨骼摆好——重新生成不会把 Blockbench 里
    的选择和展开状态洗掉。

    甲件按名字第一段分组（crown / brow / ear / side / curtain），和
    gen_*.py 里的 _helmet_* 子装配一一对应；玩家每个部位单独一组、组心取原版
    骨骼枢轴，拖旋转就能摆姿势看动态下会不会穿帮。
    """
    from armor_model_common import MOUNT_PIVOT, MOUNT_X

    elements: list[dict] = []
    # 挂点 → 装配名 → uuid。leggings / boots 的每个装配都同时含左右两侧，
    # 只按装配名分组会让右侧跟着左侧的枢轴转，所以挂点必须是更外面的一级。
    assemblies: dict[str, dict[str, list[str]]] = {}
    for cube in part.cubes:
        origin = (cube.origin[0] + MOUNT_X[cube.mount], cube.origin[1], cube.origin[2])
        element = _cube(cube.name, origin, cube.size, cube.uv, MOUNT_PIVOT[cube.mount])
        element["uuid"] = str(uuid.uuid5(MODEL_NAMESPACE, f"{part.key}/{cube.name}"))
        element["box_uv"] = False
        element["autouv"] = 0
        element["rescale"] = False
        element["locked"] = False
        elements.append(element)
        assembly = cube.name.split("_", 1)[0]
        assemblies.setdefault(cube.mount, {}).setdefault(assembly, []).append(element["uuid"])

    player_groups = []
    for index, (name, origin, size, uv, pivot) in enumerate(PLAYER):
        element = _cube(f"player_{name}", origin, size,
                        (uv[0] + SKIN_ORIGIN[0], uv[1] + SKIN_ORIGIN[1]), pivot)
        element["uuid"] = str(uuid.uuid5(MODEL_NAMESPACE, f"player/{name}"))
        element["box_uv"] = False
        element["autouv"] = 0
        elements.append(element)
        player_groups.append(_group(name, pivot, 6 + index % 2, f"player/{name}",
                                    [element["uuid"]]))

    def _assembly_groups(mount: str, groups: dict[str, list[str]], prefix: str):
        return [
            _group(assembly, MOUNT_PIVOT[mount], index % 8,
                   f"{prefix}/{assembly}", ids)
            for index, (assembly, ids) in enumerate(groups.items())
        ]

    if len(assemblies) == 1:
        # 单挂点（helmet / chestplate）：装配直接挂在甲件下，不多套一层
        mount, groups = next(iter(assemblies.items()))
        armor_children = _assembly_groups(mount, groups, part.key)
        armor_pivot = MOUNT_PIVOT[mount]
    else:
        # 跨挂点（leggings / boots）：先按挂点分，各自带自己的枢轴
        armor_children = [
            _group(mount, MOUNT_PIVOT[mount], index % 8, f"{part.key}/{mount}",
                   _assembly_groups(mount, groups, f"{part.key}/{mount}"))
            for index, (mount, groups) in enumerate(assemblies.items())
        ]
        # 混挂点时没有单一正确的枢轴，取身体中心，别假装是左腿
        armor_pivot = (0.0, 24.0, 0.0)

    texture = build_preview_texture(armor_texture, skin)
    buf = io.BytesIO()
    texture.save(buf, format="PNG")
    return {
        "meta": {"format_version": "4.10", "model_format": "free", "box_uv": False},
        "name": f"{part.key}_on_player",
        "model_identifier": f"geometry.bong.{part.key}_on_player",
        "visible_box": [3, 3, 0.5],
        "resolution": {"width": PREVIEW_RES, "height": PREVIEW_RES},
        "elements": elements,
        "outliner": [
            _group(part.key, armor_pivot, 0, part.key, armor_children),
            _group("player", (0, 24, 0), 7, "player", player_groups),
        ],
        "textures": [{
            "path": "",
            "name": f"{part.key}_on_player.png",
            "folder": "armor",
            "namespace": "bong",
            "id": "0",
            "width": PREVIEW_RES, "height": PREVIEW_RES,
            "uv_width": PREVIEW_RES, "uv_height": PREVIEW_RES,
            "particle": False,
            "render_mode": "default",
            "visible": True,
            "mode": "bitmap",
            "saved": False,
            "uuid": str(uuid.uuid5(MODEL_NAMESPACE, f"{part.key}/atlas")),
            "source": "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode("ascii"),
        }],
    }


def write_player_bbmodel(part: ArmorPart, skin: Image.Image, armor_texture: Image.Image,
                         material: str, out_dir: Path | None = None) -> Path:
    directory = out_dir or (_WS.models / "armor" / material)
    directory.mkdir(parents=True, exist_ok=True)
    name = "".join(word.capitalize() for word in part.key.split("_")) + "OnPlayer.bbmodel"
    path = directory / name
    path.write_text(
        json.dumps(build_player_bbmodel(part, skin, armor_texture),
                   ensure_ascii=False, indent=1) + "\n",
        encoding="utf-8",
    )
    return path



def coverage_report(part: ArmorPart) -> list[tuple[str, int, int, int]]:
    """静止姿下逐面采样，报每个身体部位有多少外表面**没被甲盖住**。

    加这个是因为 hide_chestplate 栽过一次：肩片顶边照参考图斜下去，正视三视图
    全看不出问题，一俯视才发现手臂顶面（y=24 那个硬平面）整片露在甲外。参考图
    的灰模特肩是圆的、没有这个平面，照抄剖面必然漏——**这类洞只有俯视或逐面
    采样才抓得到**。

    统计口径：被别的身体部位挡住的面（躯干顶面在脑袋底下、手臂内侧贴着躯干）
    不计入分母，它们本来就看不见。返回 (部位, 露出, 应盖, 总采样)。
    """
    from armor_model_common import MOUNT_X

    def armor_boxes() -> list[tuple[tuple[float, ...], tuple[float, ...]]]:
        out = []
        for cube in part.cubes:
            offset = MOUNT_X[cube.mount]
            low = (cube.origin[0] + offset, cube.origin[1], cube.origin[2])
            out.append((low, tuple(low[i] + cube.size[i] for i in range(3))))
        return out

    boxes = armor_boxes()
    bodies = [(name, origin, tuple(origin[i] + size[i] for i in range(3)))
              for name, origin, size, _, _ in PLAYER]

    def inside(point, low, high, slack=0.0):
        return all(low[i] - slack <= point[i] <= high[i] + slack for i in range(3))

    step = 0.25
    report = []
    for name, low, high in bodies:
        exposed = expected = total = 0
        for axis in range(3):
            others = [k for k in range(3) if k != axis]
            spans = [
                [low[k] + step / 2 + i * step
                 for i in range(max(1, int((high[k] - low[k]) / step)))]
                for k in others
            ]
            for face_value, normal in ((low[axis], -1.0), (high[axis], 1.0)):
                for a in spans[0]:
                    for b in spans[1]:
                        point = [0.0, 0.0, 0.0]
                        point[axis] = face_value + normal * 0.02
                        point[others[0]] = a
                        point[others[1]] = b
                        total += 1
                        if any(other != name and inside(point, lo, hi)
                               for other, lo, hi in bodies):
                            continue          # 被别的身体部位挡着，本来就看不见
                        expected += 1
                        if not any(inside(point, lo, hi) for lo, hi in boxes):
                            exposed += 1
        report.append((name, exposed, expected, total))
    return report


def _render_row(model: dict, focus, size: int, shading: str):
    with tempfile.NamedTemporaryFile("w", suffix=".bbmodel", delete=False) as handle:
        json.dump(model, handle)
        tmp = Path(handle.name)
    try:
        return [(label, render(tmp, yaw=yaw, pitch=pitch, size=size,
                               focus=focus, shading=shading)[0])
                for label, yaw, pitch in VIEWS]
    finally:
        tmp.unlink(missing_ok=True)


def render_on_body(part: ArmorPart, slot: str, armor_texture: Image.Image, out_path: Path,
                   size: int = 300, shading: str = "mc", grey: bool = False,
                   full_body: bool = False, zoom: float = 1.0) -> Path:
    skin = make_grey_skin() if grey else make_player_skin()
    center, span = ((0.0, 16.0, 0.0), 42.0) if full_body else FRAME[slot]
    focus = (center, span / zoom)

    rows = (
        ("裸 BARE", _render_row(build_preview_model(None, skin, armor_texture),
                                focus, size, shading)),
        (f"戴 {part.key}", _render_row(build_preview_model(part, skin, armor_texture),
                                       focus, size, shading)),
    )

    gap, label_h = 10, 16
    width = size * len(VIEWS) + gap * (len(VIEWS) + 1)
    height = (size + label_h + gap) * len(rows) + gap
    canvas = Image.new("RGB", (width, height), (14, 15, 17))
    draw = ImageDraw.Draw(canvas)
    y = gap
    for row_label, tiles in rows:
        x = gap
        for label, tile in tiles:
            draw.text((x + 4, y), f"{row_label}  {label}", fill=(220, 220, 212))
            canvas.paste(tile, (x, y + label_h))
            x += size + gap
        y += size + label_h + gap
    out_path.parent.mkdir(parents=True, exist_ok=True)
    canvas.save(out_path)
    return out_path


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("module", help="生成器模块名，如 gen_hide_armor")
    parser.add_argument("--slot", default="head", choices=sorted(FRAME))
    parser.add_argument("--part", help="只渲染这个 part.key（默认全渲）")
    parser.add_argument("--size", type=int, default=300)
    parser.add_argument("--shading", default="mc", choices=["mc", "lambert"])
    parser.add_argument("--grey", action="store_true", help="退回无脸灰模特，只看轮廓")
    parser.add_argument("--full-body", action="store_true", help="全身取景而非该槽特写")
    parser.add_argument("--zoom", type=float, default=1.0, help="取景倍率，>1 拉近看接缝")
    parser.add_argument("--no-bbmodel", action="store_true",
                        help="不导出可在 Blockbench 打开的甲+玩家合模")
    parser.add_argument("--no-render", action="store_true", help="只导出 bbmodel，不出图")
    parser.add_argument("--dump-skin", action="store_true", help="另存一张生成的皮肤便于查看")
    parser.add_argument("--coverage", action="store_true",
                        help="逐面采样报哪个身体部位还露在甲外面")
    parser.add_argument("--set", dest="as_set", action="store_true",
                        help="把该材质的所有件**合成一件**穿上（判各件之间的搭接，"
                             "如腿甲下摆压不压得住靴筒口）；件名取 <material>_set")
    args = parser.parse_args()

    out_dir = _WS.out
    out_dir.mkdir(parents=True, exist_ok=True)
    if args.dump_skin:
        path = out_dir / "preview_player_skin.png"
        make_player_skin().save(path)
        print(path)

    module = __import__(args.module)
    texture = module.make_texture()
    material = getattr(module, "MATERIAL", "preview")
    skin = make_grey_skin() if args.grey else make_player_skin()

    # --set：整套一起穿。单件预览判不出**件与件**的关系——腿甲的下摆和靴筒口
    # 谁压谁、绑腿的穗有没有把鞋的绑绳整个埋掉，这些只有合起来才看得见，而它们
    # 恰恰是"穿上以后好不好看"的大头。合成件只用于预览，不写任何游戏资产。
    if args.as_set:
        merged: tuple = ()
        for part in module.parts():
            merged += part.cubes
        parts = (ArmorPart(f"{material}_set", f"{material.upper()} SET", merged),)
    else:
        parts = module.parts()

    for part in parts:
        if args.part and part.key != args.part:
            continue
        if args.coverage:
            print(f"[coverage] {part.key}")
            for name, exposed, expected, total in coverage_report(part):
                mark = "  " if exposed == 0 else "!!"
                pct = 100.0 * (expected - exposed) / expected if expected else 100.0
                print(f"  {mark} {name:10s} 盖住 {pct:5.1f}%  露出 {exposed:4d}/{expected}"
                      f"（另有 {total - expected} 个采样点被身体自己挡住）")
        if not args.no_bbmodel:
            print(write_player_bbmodel(part, skin, texture, material))
        if args.no_render:
            continue
        suffix = "_full" if args.full_body else ("_zoom" if args.zoom != 1.0 else "")
        out = out_dir / f"{part.key}_on_player{suffix}.png"
        print(render_on_body(part, args.slot, texture, out, args.size,
                             args.shading, args.grey, args.full_body, args.zoom))


if __name__ == "__main__":
    main()
