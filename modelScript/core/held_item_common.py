#!/usr/bin/env python3
"""手持物（武器 / 工具 / 盾）的 bbmodel + SML OBJ 双出公共层。

和护甲的 `armor_model_common` 对位，但目标链路不同：

    护甲   box 表 → bbmodel（离线设计）+ ArmorPartModel.CUBE_TABLES（运行时真相）
    手持物 box 表 → bbmodel（离线设计）+ OBJ/MTL/16² 贴图（运行时真相，SML 加载）

**一份 box 表同时出两种产物**，这是本模块存在的理由。此前手持物是两套各写一份：
bbmodel 在 `modelScript/generators/gen_*_shield.py`，OBJ 在
`client/tools/gen_shield_models.py`，两边的坐标靠人肉对齐——盾牌那两件至今
bbmodel 和 OBJ 的 boss 厚度就对不上。这里合成一个源头，从结构上去掉这个失同步点。

渲染链（见 `BongWeaponModelRegistry` / `WeaponRenderBootstrap`）：
    server 下发 template_id → client 合成宿主 vanilla item 的 fake stack
    → 该宿主的 item model JSON 被 SML 劫持到 `bong:models/item/<id>/<id>.obj`
    → 显示 3D 模型

## 坐标约定

模型空间 1.0 = 一个方块 = 16 px。**y=0 必须落在握把末端、尖端朝 +Y**，
`assert_conventions` 会查。这样一来所有手持物共用同一套 display 变换基线
（参照 `axe_bone`：TP rotation [0,-90,55] / translation [0,4,0]），不必每件
单独调 `client/tools/asset_configs/<id>.json`。

## UV 约定

OBJ 那条链是**每个面整张贴图铺满**（`_VT` 四角恒为 0,0..1,1），一个 material
一张 16² 图。bbmodel 这边为了长得一样，把各 material 的 16² 图拼成一张图集，
每个面的 uv 取该 material 的整块 tile。两边因此像素级一致。

副作用：贴图会按面拉伸，所以每张图必须画成**通用材质样本**（木纹 / 石片 /
锈斑 / 骨纹 / 绳纹），不能画成"某个面的具体图案"。
"""

from __future__ import annotations

import base64
import io
import json
import uuid
from dataclasses import dataclass
from pathlib import Path

from PIL import Image

TILE = 16                       # 每个 material 一张 16² 图
MODEL_NAMESPACE = uuid.UUID("2f0f1a7c-6b3e-4d21-9a55-0c9d7e4b8f13")

# ── OBJ 几何常量：和 axe_bone.obj / bone_shield.obj 同构 ────────────────────
# 共享 4 角 UV + 6 面法线，每 box 8 verts。
_VT = ((0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0))
_VN = (
    (0.0, 0.0, 1.0),    # 1 +Z front
    (0.0, 0.0, -1.0),   # 2 -Z back
    (-1.0, 0.0, 0.0),   # 3 -X left
    (1.0, 0.0, 0.0),    # 4 +X right
    (0.0, 1.0, 0.0),    # 5 +Y top
    (0.0, -1.0, 0.0),   # 6 -Y bottom
)
# 每面 4 个本地顶点序（CCW，外法线）+ 该面法线序号
_FACES = (
    ((4, 5, 6, 7), 1),
    ((1, 0, 3, 2), 2),
    ((0, 4, 7, 3), 3),
    ((5, 1, 2, 6), 4),
    ((7, 6, 2, 3), 5),
    ((0, 1, 5, 4), 6),
)
# bbmodel 的面名 → OBJ 法线序（用来给 bbmodel 排同一套朝向）
_BB_FACES = ("north", "south", "west", "east", "up", "down")


@dataclass(frozen=True)
class Box:
    """一个轴对齐盒。center/half 用模型空间单位（1.0 = 16px）。"""

    name: str
    material: str
    center: tuple[float, float, float]
    half: tuple[float, float, float]

    @property
    def low(self) -> tuple[float, float, float]:
        return tuple(self.center[i] - self.half[i] for i in range(3))

    @property
    def high(self) -> tuple[float, float, float]:
        return tuple(self.center[i] + self.half[i] for i in range(3))


@dataclass(frozen=True)
class Material:
    """一个 material = 一张 16² 贴图 + 一个 MTL 条目。"""

    name: str
    kd: tuple[float, float, float]      # MTL 漫反射（无贴图时的兜底色）
    texture: Image.Image


@dataclass(frozen=True)
class HeldItem:
    key: str                            # = template_id，也是资源目录名
    display_name: str
    host_item: str                      # 宿主 vanilla item 的注册名（其 model JSON 被劫持）
    boxes: tuple[Box, ...]
    materials: tuple[Material, ...]
    display: dict[str, dict[str, list]]


# ── 校验 ──────────────────────────────────────────────────────────────────


def assert_conventions(item: HeldItem) -> None:
    """坐标与材质约定。违反了不是"看着怪"，是 display 变换整套失准。"""
    if not item.boxes:
        raise ValueError(f"{item.key}: 没有 box")

    names: set[str] = set()
    for box in item.boxes:
        if box.name in names:
            raise ValueError(f"{item.key}: 重名 box {box.name}")
        names.add(box.name)
        if any(h <= 0.0 for h in box.half):
            raise ValueError(f"{item.key}/{box.name}: half 必须为正，得到 {box.half}")

    known = {m.name for m in item.materials}
    if len(known) != len(item.materials):
        raise ValueError(f"{item.key}: material 重名")
    for box in item.boxes:
        if box.material not in known:
            raise ValueError(f"{item.key}/{box.name}: 未知 material {box.material}")
    used = {b.material for b in item.boxes}
    if used != known:
        raise ValueError(
            f"{item.key}: material {known - used} 定义了但没有 box 用——"
            f"会白占一张 16² 贴图，且 MTL 里挂个死条目"
        )

    for material in item.materials:
        if material.texture.size != (TILE, TILE):
            raise ValueError(
                f"{item.key}/{material.name}: 贴图 {material.texture.size} 不是 {TILE}²"
            )

    y_min = min(b.low[1] for b in item.boxes)
    y_max = max(b.high[1] for b in item.boxes)
    if abs(y_min) > 1e-6:
        raise ValueError(
            f"{item.key}: 最低点 y={y_min:.4f} 不在 0。握把末端必须落在 y=0、"
            f"尖端朝 +Y，否则这件的 display 变换和 axe_bone 那套基线对不上，"
            f"手持时会插进手掌或飘在外面"
        )
    if not 0.3 <= y_max <= 1.2:
        raise ValueError(f"{item.key}: 全长 {y_max:.3f} 超出手持物合理区间 0.3~1.2")
    for axis, label in ((0, "x"), (2, "z")):
        span = max(b.high[axis] for b in item.boxes) - min(b.low[axis] for b in item.boxes)
        if span > 0.6:
            raise ValueError(f"{item.key}: {label} 向跨度 {span:.3f} 过大，不像手持物")


def assert_no_coplanar_faces(item: HeldItem) -> None:
    """揪出"两块外表面落在同一平面且投影相交"的 box 对——体素模型的经典 z-fighting。

    渲染器对同深度的两个面没有稳定取舍，逐像素乱选，渲出来是一片高频噪点，
    肉眼极易误判成"贴图脏"。刀这类件里最容易犯的是**刃分段**：相邻两段图省事
    写成同一个 x 半宽，两段的侧面就共面了。
    """
    boxes = item.boxes
    for i in range(len(boxes)):
        for j in range(i + 1, len(boxes)):
            first, second = boxes[i], boxes[j]
            for axis in range(3):
                overlap = 1.0
                for other in (k for k in range(3) if k != axis):
                    overlap *= max(0.0, min(first.high[other], second.high[other])
                                   - max(first.low[other], second.low[other]))
                if overlap <= 1e-4:      # 只擦到一条边不算，那是正常拼接
                    continue
                for face, a, b in (("max", first.high[axis], second.high[axis]),
                                   ("min", first.low[axis], second.low[axis])):
                    if abs(a - b) < 1e-9:
                        raise ValueError(
                            f"{item.key}: {first.name} 与 {second.name} 的 "
                            f"{'xyz'[axis]}-{face} 面共面于 {a}，投影相交 {overlap:.5f}"
                            f"——会 z-fighting，挪开一块"
                        )


# ── OBJ / MTL ─────────────────────────────────────────────────────────────


def build_obj(item: HeldItem) -> str:
    lines = [
        f"# {item.key}.obj -- generated by modelScript/core/held_item_common.py",
        "# 勿手改：改 gen_* 里的 box 表后重跑生成器。",
        f"mtllib {item.key}.mtl",
        f"o {item.key}",
    ]
    lines += [f"vt {u:.4f} {v:.4f}" for u, v in _VT]
    lines += [f"vn {x:.4f} {y:.4f} {z:.4f}" for x, y, z in _VN]

    base = 0
    for box in item.boxes:
        lo, hi = box.low, box.high
        corners = (
            (lo[0], lo[1], lo[2]), (hi[0], lo[1], lo[2]),
            (hi[0], hi[1], lo[2]), (lo[0], hi[1], lo[2]),
            (lo[0], lo[1], hi[2]), (hi[0], lo[1], hi[2]),
            (hi[0], hi[1], hi[2]), (lo[0], hi[1], hi[2]),
        )
        lines.append(f"# part: {box.name}")
        lines += [f"v {x:.4f} {y:.4f} {z:.4f}" for x, y, z in corners]
        lines.append(f"usemtl {box.material}")
        for order, normal in _FACES:
            lines.append("f " + " ".join(
                f"{base + k + 1}/{n + 1}/{normal}" for n, k in enumerate(order)
            ))
        base += 8
    return "\n".join(lines) + "\n"


def build_mtl(item: HeldItem) -> str:
    lines = [f"# {item.key} materials -- generated by held_item_common.py"]
    for index, material in enumerate(item.materials):
        r, g, b = material.kd
        lines += [
            "",
            f"newmtl {material.name}",
            "Ka 1.000000 1.000000 1.000000",
            f"Kd {r:.6f} {g:.6f} {b:.6f}",
            "Ks 0.000000 0.000000 0.000000",
            "Ns 10.000000",
            "d 1.000000",
            "illum 1",
            f"map_Kd bong:item/{item.key}/{index}",
        ]
    return "\n".join(lines) + "\n"


def build_model_json(item: HeldItem) -> str:
    return json.dumps(
        {
            "parent": "sml:builtin/obj",
            "model": f"bong:models/item/{item.key}/{item.key}.obj",
            "display": item.display,
        },
        ensure_ascii=False,
        indent=2,
    ) + "\n"


# ── bbmodel ───────────────────────────────────────────────────────────────


def build_atlas(item: HeldItem) -> Image.Image:
    """把各 material 的 16² 图横排成一张图集，供 bbmodel 用。

    只是为了让 Blockbench 里看到的和游戏里一致；游戏那条链读的是拆开的单张图。
    """
    count = len(item.materials)
    width = TILE * count
    atlas = Image.new("RGBA", (width, TILE), (0, 0, 0, 0))
    for index, material in enumerate(item.materials):
        atlas.paste(material.texture.convert("RGBA"), (index * TILE, 0))
    return atlas


def _data_url(image: Image.Image) -> str:
    buffer = io.BytesIO()
    image.save(buffer, format="PNG")
    return "data:image/png;base64," + base64.b64encode(buffer.getvalue()).decode("ascii")


def build_bbmodel(item: HeldItem) -> dict:
    """bbmodel 用**模型空间 ×16**（即 px）写坐标，Blockbench 的格子才对得上。

    uuid 全部走 uuid5：uuid4 会让每次重跑都产出一份 diff，git 上分不清"改了造型"
    和"只是重跑了一遍"（棺材那批生成器踩过）。
    """
    index_of = {m.name: i for i, m in enumerate(item.materials)}
    atlas_w = TILE * len(item.materials)
    elements = []
    for box in item.boxes:
        tile = index_of[box.material]
        u0, u1 = tile * TILE, (tile + 1) * TILE
        faces = {
            name: {"uv": [u0, 0, u1, TILE], "texture": 0}
            for name in _BB_FACES
        }
        elements.append({
            "name": box.name,
            "box_uv": False,
            "rescale": False,
            "locked": False,
            "render_order": "default",
            "allow_mirror_modeling": True,
            "type": "cube",
            "uuid": str(uuid.uuid5(MODEL_NAMESPACE, f"{item.key}/{box.name}")),
            "from": [round(v * 16.0, 4) for v in box.low],
            "to": [round(v * 16.0, 4) for v in box.high],
            "autouv": 0,
            "color": tile % 8,
            "origin": [0.0, 0.0, 0.0],
            "faces": faces,
        })

    groups: dict[str, list[str]] = {}
    for box, element in zip(item.boxes, elements):
        groups.setdefault(box.material, []).append(element["uuid"])
    outliner = [
        {
            "name": material,
            "origin": [0.0, 0.0, 0.0],
            "color": index_of[material] % 8,
            "uuid": str(uuid.uuid5(MODEL_NAMESPACE, f"{item.key}/group/{material}")),
            "export": True,
            "mirror_uv": False,
            "isOpen": True,
            "locked": False,
            "visibility": True,
            "autouv": 0,
            "children": children,
        }
        for material, children in groups.items()
    ]

    return {
        "meta": {"format_version": "4.10", "model_format": "free", "box_uv": False},
        "name": item.key,
        "model_identifier": f"geometry.bong.{item.key}",
        "visible_box": [2, 2, 1],
        "resolution": {"width": atlas_w, "height": TILE},
        "elements": elements,
        "outliner": outliner,
        "textures": [{
            "path": "",
            "name": f"{item.key}.png",
            "folder": "item",
            "namespace": "bong",
            "id": "0",
            "width": atlas_w,
            "height": TILE,
            "uv_width": atlas_w,
            "uv_height": TILE,
            "particle": False,
            "render_mode": "default",
            "visible": True,
            "mode": "bitmap",
            "saved": False,
            "uuid": str(uuid.uuid5(MODEL_NAMESPACE, f"{item.key}/texture")),
            "source": _data_url(build_atlas(item)),
        }],
    }


# ── 落盘 ──────────────────────────────────────────────────────────────────


def write_assets(
    items: tuple[HeldItem, ...],
    bbmodel_dir: Path,
    client_resources: Path | None,
    preview_dir: Path | None = None,
    render_previews: bool = True,
) -> dict[str, Path]:
    """bbmodel 恒写；OBJ/MTL/贴图/model JSON 只在给了 client_resources 时写。

    分开是为了让"改造型"和"装进游戏"能拆成两个 commit：前者只动 modelScript，
    后者才碰 client 资源树（那一步要连带同步资源包 sha1）。
    """
    for item in items:
        assert_conventions(item)
        assert_no_coplanar_faces(item)

    outputs: dict[str, Path] = {}
    bbmodel_dir.mkdir(parents=True, exist_ok=True)

    for item in items:
        name = "".join(word.capitalize() for word in item.key.split("_")) + ".bbmodel"
        path = bbmodel_dir / name
        path.write_text(
            json.dumps(build_bbmodel(item), ensure_ascii=False, indent=1) + "\n",
            encoding="utf-8",
        )
        outputs[f"bbmodel:{item.key}"] = path

        if client_resources is not None:
            model_dir = client_resources / "assets" / "bong" / "models" / "item" / item.key
            model_dir.mkdir(parents=True, exist_ok=True)
            (model_dir / f"{item.key}.obj").write_text(build_obj(item), encoding="utf-8")
            (model_dir / f"{item.key}.mtl").write_text(build_mtl(item), encoding="utf-8")
            (model_dir / f"{item.key}.json").write_text(build_model_json(item), encoding="utf-8")
            outputs[f"obj:{item.key}"] = model_dir / f"{item.key}.obj"

            # 劫持宿主 vanilla item 的 model JSON —— 内容与 bong 那份一致，指向同一 OBJ。
            host_dir = client_resources / "assets" / "minecraft" / "models" / "item"
            host_dir.mkdir(parents=True, exist_ok=True)
            host_path = host_dir / f"{item.host_item}.json"
            host_path.write_text(build_model_json(item), encoding="utf-8")
            outputs[f"host:{item.key}"] = host_path

            tex_dir = client_resources / "assets" / "bong" / "textures" / "item" / item.key
            tex_dir.mkdir(parents=True, exist_ok=True)
            for index, material in enumerate(item.materials):
                material.texture.save(tex_dir / f"{index}.png")
            outputs[f"tex:{item.key}"] = tex_dir

    if render_previews and preview_dir is not None:
        from render_bbmodel import render_three_view

        preview_dir.mkdir(parents=True, exist_ok=True)
        for item in items:
            preview, _ = render_three_view(outputs[f"bbmodel:{item.key}"], size=320)
            path = preview_dir / f"{item.key}_render_three_view.png"
            preview.save(path)
            outputs[f"preview:{item.key}"] = path

    return outputs
