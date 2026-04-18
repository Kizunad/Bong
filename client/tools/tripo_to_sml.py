#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = [
#   "pymeshlab",
#   "numpy",
#   "pillow",
#   "scipy",
# ]
# ///
"""Tripo OBJ → Special Model Loader 资产管线。

将 Tripo 导出的高面数 OBJ（Blender 扩展顶点色、无 UV、无 MTL）自动处理成
Bong client (Fabric 1.20.1 + SML) 能消费的完整资产包。

流水：
  1. 读原始 OBJ 提取顶点色均值（或 k-means 主色）→ 主色
  2. pymeshlab 加载 → Quadric Edge Collapse 减面到 target-faces
  3. 为减面后 mesh 加 trivial per-wedge UV（1×1 贴图管线，UV 质量无关）
  4. 居中 + 缩放到 target_length MC 单位（沿最长轴）
  5. 导出标准 Wavefront OBJ（无顶点色，SML/de.javagl.obj 兼容）
  6. 生成 1×1 主色 PNG
  7. 生成 MTL（引用 bong:item/<id>）
  8. 生成 SML 顶层 item JSON，handheld display transform

产出结构（<client-root>/src/main/resources）：
  assets/bong/models/item/<id>/<id>.obj
  assets/bong/models/item/<id>/<id>.mtl
  assets/bong/textures/item/<id>.png
  assets/<override_ns>/models/item/<override_path>.json  (默认：minecraft:iron_sword)

用法：
  uv run client/tools/tripo_to_sml.py tmp/xxx.obj --id iron_sword
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import dataclass
from pathlib import Path

import numpy as np
import pymeshlab
from PIL import Image

CLIENT_ROOT = Path(__file__).resolve().parents[1]
RESOURCES_ROOT = CLIENT_ROOT / "src" / "main" / "resources"
ASSET_CONFIG_DIR = Path(__file__).resolve().parent / "asset_configs"

# 3D OBJ mesh 版 handheld display —— vanilla item/handheld 的 translation (1.13, 3.2, 1.13)
# 是为 16×16 2D sprite（中心在 (8, 8, 8)）设计的，目的是把 sprite 中心推到手心位置。
# Bong 的 OBJ 握柄原点在 (0, 0, 0)，用 vanilla translation 会把整把剑推离手部 1 block 左右。
# 所以 first/third person 的 translation 归零，让握柄直接落在手心；rotation 保留 vanilla
# 的持握角度。具体角度后续按 WeaponKind 精调（见 plan-weapon-v1.md §5.4）。
#
# ━━━ 校准陷阱（真实踩过的坑，后续调整者请务必读完） ━━━
#
# 1) rotation Z 方向符号反直觉：
#    绕 Z 轴 (rotation[2]) 正值让**剑尖朝下**（不是朝上）。要剑尖朝上用**负值**或小值。
#    例：rotation=[0,-90,55] 时剑"下垂"明显；rotation=[0,-90,10] 剑更接近水平。
#
# 2) rotation 和 translation **相互耦合**：
#    translation 在 rotation 之前应用到 matrix stack，但 rotation 会把每个轴重新映射到
#    world 方向。所以改了 rotation 之后，translation.x/y/z 对应的视觉"左右/上下/前后"
#    会变化。不能假定"Y 永远是上"——这只在某个固定 rotation 下成立。
#    thirdperson_righthand 实测表（rotation[1]=-90 固定）：
#      rotation[2]=55 → X- 左, Y+ 上, Z± 混合(偏高度)
#      rotation[2]=10 → X- 左, Y+ 前（不是上！）, Z± 垂直分量大
#      rotation[2]=0  → X- 左, Y+ 前, **Z+ 才是上**（非 Y+）
#    改 rotation 后**必须重新做轴方向实测**，不要套上一次的"Y+=上"印象。
#
# 3) rotation 改动后 **剑的位置也会漂移**：
#    rotation 绕 model origin (0,0,0) 旋转，非原点顶点位置随之变化。改 rotation 常伴
#    随剑的整体"沉下去"或"升上去"的视觉效果，要重新 translation 补偿。
#
# 4) 调参正确顺序：**先定 rotation，再调 translation**。
#    反过来（先调好位置再改角度）会让每次 rotation 变化都推翻位置。
#    每次迭代只改一个字段（translation.y 或 rotation.z 等），便于归因。
#
# 5) per-asset 定制放在 client/tools/asset_configs/<id>.json 里覆盖此默认；
#    不要把某把武器的 translation 固化到这里，会污染其他武器。
HANDHELD_DISPLAY: dict[str, dict[str, list[float]]] = {
    "thirdperson_righthand": {
        "rotation": [0, -90, 55],
        "translation": [0, 0, 0],
        "scale": [1.0, 1.0, 1.0],
    },
    "thirdperson_lefthand": {
        "rotation": [0, 90, -55],
        "translation": [0, 0, 0],
        "scale": [1.0, 1.0, 1.0],
    },
    "firstperson_righthand": {
        "rotation": [0, -90, 25],
        "translation": [0, 0, 0],
        "scale": [1.0, 1.0, 1.0],
    },
    "firstperson_lefthand": {
        "rotation": [0, 90, -25],
        "translation": [0, 0, 0],
        "scale": [1.0, 1.0, 1.0],
    },
    "ground": {
        "rotation": [0, 0, 0],
        "translation": [0, 2, 0],
        "scale": [0.5, 0.5, 0.5],
    },
    "gui": {
        "rotation": [0, 0, 0],
        "translation": [0, 0, 0],
        "scale": [1, 1, 1],
    },
    "fixed": {
        "rotation": [0, 0, 0],
        "translation": [0, 0, 0],
        "scale": [1, 1, 1],
    },
}


@dataclass
class PipelineArgs:
    input_obj: Path
    asset_id: str
    namespace: str
    override: str | None
    target_faces: int
    target_length: float
    colors: int
    texture_max_size: int
    flip_y: bool
    flip_x: bool
    flip_z: bool
    no_winding_invert: bool
    dry_run: bool


def parse_args(argv: list[str]) -> PipelineArgs:
    p = argparse.ArgumentParser(description="Tripo OBJ → SML assets pipeline")
    p.add_argument("input_obj", type=Path, help="Tripo 导出的 OBJ 源文件")
    p.add_argument("--id", default="placeholder_sword", help="asset id（产出目录名 + 贴图名）")
    p.add_argument("--namespace", default="bong", help="Bong 资源命名空间（贴图+mesh 所在）")
    p.add_argument(
        "--override",
        default="minecraft:iron_sword",
        help="用 override JSON 劫持的 vanilla item (namespace:path)。设 'none' 跳过。",
    )
    p.add_argument("--target-faces", type=int, default=2500, help="减面目标三角形数（500 粗 / 2500 常规 / 5000 精）")
    p.add_argument("--target-length", type=float, default=12.0, help="沿最长轴缩放到此 MC 单位（vanilla 铁剑 ~12）")
    p.add_argument("--colors", type=int, default=4, help="从原顶点色聚类出 N 个主色 → N 张 1×1 贴图 + N 段 material")
    p.add_argument(
        "--texture-max-size",
        type=int,
        default=1024,
        help="纹理路径下 basecolor PNG 最大边长（Tripo 常给 4K，缩到 1024 省 90% 体积）",
    )
    p.add_argument("--flip-y", action="store_true", help="Y 轴翻转：剑尖从上翻到下（或反之）。用于修正 Tripo 生成时握柄方向")
    p.add_argument("--flip-x", action="store_true", help="X 轴翻转")
    p.add_argument("--flip-z", action="store_true", help="Z 轴翻转（前后互换）")
    p.add_argument(
        "--no-winding-invert",
        action="store_true",
        help="即使奇数次翻转也不 invert face winding（诊断：对比有/无 winding 反向哪个显示更接近原模型）",
    )
    p.add_argument("--dry-run", action="store_true", help="只打印计划，不落盘")
    ns = p.parse_args(argv)
    return PipelineArgs(
        input_obj=ns.input_obj.resolve(),
        asset_id=ns.id,
        namespace=ns.namespace,
        override=None if ns.override == "none" else ns.override,
        target_faces=ns.target_faces,
        target_length=ns.target_length,
        colors=max(1, ns.colors),
        texture_max_size=max(16, ns.texture_max_size),
        flip_y=ns.flip_y,
        flip_x=ns.flip_x,
        flip_z=ns.flip_z,
        no_winding_invert=ns.no_winding_invert,
        dry_run=ns.dry_run,
    )


def extract_dominant_color(obj_path: Path) -> tuple[float, float, float]:
    """从 OBJ 的 `v x y z r g b` 顶点色扩展里提取均值主色（日志/fallback 用）。"""
    _, colors = load_source_verts_and_colors(obj_path)
    if colors.shape[0] == 0:
        return (0.75, 0.75, 0.78)
    return tuple(float(v) for v in colors.mean(axis=0))  # type: ignore[return-value]


def load_source_verts_and_colors(obj_path: Path) -> tuple[np.ndarray, np.ndarray]:
    """原 OBJ 解析：返回 (verts [N,3], colors [N,3])。无顶点色时 colors 为空 (0,3)。"""
    pattern = re.compile(r"^v\s+(\S+)\s+(\S+)\s+(\S+)(?:\s+(\S+)\s+(\S+)\s+(\S+))?")
    verts: list[tuple[float, float, float]] = []
    colors: list[tuple[float, float, float]] = []
    with obj_path.open("r", encoding="utf-8", errors="replace") as f:
        for line in f:
            m = pattern.match(line)
            if not m:
                continue
            try:
                x, y, z = (float(v) for v in m.groups()[:3])
            except (ValueError, TypeError):
                continue
            verts.append((x, y, z))
            if m.group(4) is not None:
                try:
                    r, g, b = (float(v) for v in m.groups()[3:6])
                    colors.append((r, g, b))
                except (ValueError, TypeError):
                    pass
    return np.array(verts, dtype=np.float64), np.array(colors, dtype=np.float64)


def sample_face_colors(
    src_verts: np.ndarray,
    src_colors: np.ndarray,
    face_centers: np.ndarray,
) -> np.ndarray:
    """在原顶点集上最近邻查询 face_centers 对应颜色。两者必须在同一坐标系下。"""
    from scipy.spatial import cKDTree

    if src_verts.shape[0] == 0 or src_colors.shape[0] != src_verts.shape[0]:
        # 无顶点色：退化成银色
        n = face_centers.shape[0]
        return np.full((n, 3), fill_value=0.75, dtype=np.float64)
    tree = cKDTree(src_verts)
    _, nearest = tree.query(face_centers, k=1)
    return src_colors[nearest]


def cluster_face_colors(face_colors: np.ndarray, k: int) -> tuple[np.ndarray, np.ndarray]:
    """PIL.Image.quantize 把 face 色聚类到 k 个 palette。
    返回 (face_cluster_ids [N], palette [K, 3] 0..1)。

    PIL quantize 用 median-cut + 颜色感知算法，效果可接受且零额外依赖。
    返回的 palette index 天然和 quantize 出的 P-mode 像素值对齐。
    """
    if k <= 1 or face_colors.shape[0] == 0:
        mean = face_colors.mean(axis=0) if face_colors.shape[0] > 0 else np.array([0.75, 0.75, 0.75])
        return (np.zeros(face_colors.shape[0], dtype=np.int32), mean.reshape(1, 3))

    rgb_u8 = np.clip(face_colors * 255.0, 0, 255).astype(np.uint8)
    n = rgb_u8.shape[0]
    img = Image.fromarray(rgb_u8.reshape(n, 1, 3), mode="RGB")
    quantized = img.quantize(colors=k, method=Image.Quantize.MEDIANCUT)
    face_ids = np.array(quantized).reshape(-1).astype(np.int32)
    palette_flat = quantized.getpalette()
    actual_k = int(face_ids.max()) + 1 if face_ids.size > 0 else 1
    palette = np.array(palette_flat[: actual_k * 3], dtype=np.float64).reshape(actual_k, 3) / 255.0
    return face_ids, palette


DISPLAY_MODE_KEYS: set[str] = {
    "thirdperson_righthand",
    "thirdperson_lefthand",
    "firstperson_righthand",
    "firstperson_lefthand",
    "head",
    "gui",
    "ground",
    "fixed",
}


def load_asset_config(asset_id: str) -> dict:
    """读 client/tools/asset_configs/<asset_id>.json 整个 JSON（过滤 _ 开头的注释字段）。
    无文件返回 {}。
    """
    path = ASSET_CONFIG_DIR / f"{asset_id}.json"
    if not path.exists():
        return {}
    try:
        raw = json.loads(path.read_text(encoding="utf-8"))
    except Exception as e:
        print(f"WARN: asset config 读取失败 {path}: {e}", file=sys.stderr)
        return {}
    return {k: v for k, v in raw.items() if not k.startswith("_")}


def load_display_override(asset_id: str) -> dict | None:
    """读 asset config 里的 display transform 覆盖（仅 DISPLAY_MODE_KEYS 对应的字段）。

    JSON 结构与 MC display 一致，但只需列出要覆盖的视角 + 要改的字段；
    未列出的字段沿用脚本里的 HANDHELD_DISPLAY 默认。例:

        {
          "thirdperson_righthand": { "translation": [-8, 7, 0] },
          "firstperson_righthand": { "rotation": [0, -90, 30] }
        }
    """
    cfg = load_asset_config(asset_id)
    display = {k: v for k, v in cfg.items() if k in DISPLAY_MODE_KEYS}
    return display if display else None


def apply_tint(palette: np.ndarray, tint: list[float] | None) -> np.ndarray:
    """把 tint (R, G, B multiplier) 乘到 palette 上，clip 到 [0, 1]。

    灰色顶点色 (0.5, 0.5, 0.5) × tint (0.3, 0.7, 1.3) → (0.15, 0.35, 0.65) 偏蓝。
    保留明暗层次（聚类的 K 个色阶按比例被同色调染色）。
    """
    if tint is None:
        return palette
    tint_arr = np.array(tint, dtype=np.float64)
    if tint_arr.shape != (3,):
        print(f"WARN: tint 必须是 3 元素 [r, g, b]，收到 {tint}，忽略", file=sys.stderr)
        return palette
    return np.clip(palette * tint_arr, 0.0, 1.0)


def merge_display(default: dict[str, dict], override: dict | None) -> dict[str, dict]:
    """把 override（可能部分）合并到 default 上，返回新 dict（不 mutate 输入）。"""
    if not override:
        return {k: dict(v) for k, v in default.items()}
    merged = {k: dict(v) for k, v in default.items()}
    for mode, transform in override.items():
        if mode not in merged:
            merged[mode] = dict(transform)
            continue
        # 逐字段覆盖（translation / rotation / scale 各自独立）
        for field, value in transform.items():
            merged[mode][field] = list(value) if isinstance(value, (list, tuple)) else value
    return merged


def load_obj_manual(
    path: Path,
) -> tuple[list[tuple[float, float, float]], list[tuple[float, float]], list[list[tuple[int, int]]]]:
    """纯文本 OBJ parser，不走 pymeshlab（避免其 load 阶段把 wedge UV 和 face 索引关系打乱）。

    返回 (verts, uvs, faces)：
      - verts: [(x, y, z)]
      - uvs: [(u, v)]
      - faces: 每 face 是 [(vi, uvi), ...] 1-indexed（OBJ 原生 1-based）；0=缺省。
    多边形 face 自动三角化为 fan。
    """
    verts: list[tuple[float, float, float]] = []
    uvs: list[tuple[float, float]] = []
    faces: list[list[tuple[int, int]]] = []
    with path.open("r", encoding="utf-8", errors="replace") as f:
        for line in f:
            parts = line.split()
            if not parts:
                continue
            tag = parts[0]
            if tag == "v" and len(parts) >= 4:
                verts.append((float(parts[1]), float(parts[2]), float(parts[3])))
            elif tag == "vt" and len(parts) >= 3:
                uvs.append((float(parts[1]), float(parts[2])))
            elif tag == "f" and len(parts) >= 4:
                corners: list[tuple[int, int]] = []
                for token in parts[1:]:
                    ids = token.split("/")
                    vi = int(ids[0]) if ids[0] else 0
                    uvi = int(ids[1]) if len(ids) > 1 and ids[1] else 0
                    corners.append((vi, uvi))
                if len(corners) == 3:
                    faces.append(corners)
                elif len(corners) >= 4:
                    # fan triangulate
                    for i in range(1, len(corners) - 1):
                        faces.append([corners[0], corners[i], corners[i + 1]])
    return verts, uvs, faces


def apply_verts_transform(
    verts: list[tuple[float, float, float]],
    flip_x: bool,
    flip_y: bool,
    flip_z: bool,
    target_length: float,
) -> list[tuple[float, float, float]]:
    """顶点变换：flip 轴 + normalize（Y-min 对齐 0，居中 X/Z，最长轴缩到 target_length）。"""
    V = np.array(verts, dtype=np.float64)
    if flip_x:
        V[:, 0] = -V[:, 0]
    if flip_y:
        V[:, 1] = -V[:, 1]
    if flip_z:
        V[:, 2] = -V[:, 2]
    bbox_min = V.min(axis=0)
    bbox_max = V.max(axis=0)
    extent = bbox_max - bbox_min
    longest = float(extent.max())
    if longest > 1e-9:
        scale = target_length / longest
        cx = (bbox_min[0] + bbox_max[0]) * 0.5
        cz = (bbox_min[2] + bbox_max[2]) * 0.5
        ty = bbox_min[1]
        V[:, 0] = (V[:, 0] - cx) * scale
        V[:, 1] = (V[:, 1] - ty) * scale
        V[:, 2] = (V[:, 2] - cz) * scale
    return [(float(v[0]), float(v[1]), float(v[2])) for v in V]


def write_obj_with_uvs(
    path: Path,
    verts: list[tuple[float, float, float]],
    uvs: list[tuple[float, float]],
    faces: list[list[tuple[int, int]]],
    mtl_filename: str,
    material_name: str,
    invert_winding: bool,
) -> None:
    """手写 OBJ，单 material + real UV + per-face normal。

    SML 的 de.javagl.obj 解析下，face 格式 `v/vt/vn` 三字段识别最稳；
    没 vn 的情况下 MC BakedModel 构建可能退化成 missing texture（观测结果）。
    """
    V = np.array(verts, dtype=np.float64)
    face_normals: list[tuple[float, float, float]] = []
    for face in faces:
        vi0, vi1, vi2 = face[0][0] - 1, face[1][0] - 1, face[2][0] - 1
        if vi0 < 0 or vi1 < 0 or vi2 < 0 or max(vi0, vi1, vi2) >= len(V):
            face_normals.append((0.0, 1.0, 0.0))
            continue
        edge1 = V[vi1] - V[vi0]
        edge2 = V[vi2] - V[vi0]
        n = np.cross(edge1, edge2)
        length = float(np.linalg.norm(n))
        if length < 1e-12:
            face_normals.append((0.0, 1.0, 0.0))
        else:
            n = n / length
            if invert_winding:
                n = -n
            face_normals.append((float(n[0]), float(n[1]), float(n[2])))

    with path.open("w", encoding="utf-8") as f:
        f.write("# Generated by tripo_to_sml.py (texture path, manual OBJ writer)\n")
        f.write(f"mtllib {mtl_filename}\n")
        for v in verts:
            f.write(f"v {v[0]:.6f} {v[1]:.6f} {v[2]:.6f}\n")
        for uv in uvs:
            f.write(f"vt {uv[0]:.6f} {uv[1]:.6f}\n")
        for nrm in face_normals:
            f.write(f"vn {nrm[0]:.6f} {nrm[1]:.6f} {nrm[2]:.6f}\n")
        f.write(f"usemtl {material_name}\n")
        for fi, face in enumerate(faces):
            corners = [face[0], face[2], face[1]] if invert_winding else face
            n_idx = fi + 1
            tokens = []
            for vi, uvi in corners:
                if uvi:
                    tokens.append(f"{vi}/{uvi}/{n_idx}")
                else:
                    tokens.append(f"{vi}//{n_idx}")
            f.write(f"f {' '.join(tokens)}\n")


def run_texture_pipeline(args: PipelineArgs, mtl_path: Path, texture_path: Path) -> None:
    """纹理路径：手工 OBJ parser + 1:1 face/UV 保留 + 复制贴图。完全不碰 pymeshlab。"""
    print(f"[1/5] 手工解析 OBJ: {args.input_obj}")
    verts, uvs, faces = load_obj_manual(args.input_obj)
    print(f"      v={len(verts)} vt={len(uvs)} f={len(faces)}")
    if verts:
        V = np.array(verts)
        print(f"      [raw] bbox min={V.min(axis=0).tolist()} max={V.max(axis=0).tolist()}")

    invert_winding = False
    if args.flip_x or args.flip_y or args.flip_z:
        flips_label = "+".join(a for a, v in [("x", args.flip_x), ("y", args.flip_y), ("z", args.flip_z)] if v)
        print(f"[2/5] 轴翻转 {flips_label} + 归一化 target_length={args.target_length}")
        n_flips = sum(int(b) for b in (args.flip_x, args.flip_y, args.flip_z))
        invert_winding = (n_flips % 2 == 1) and not args.no_winding_invert
        if invert_winding:
            print(f"      奇数次翻转 → face winding 反向（写 OBJ 时 swap 顶点顺序）")
        elif n_flips % 2 == 1 and args.no_winding_invert:
            print(f"      奇数次翻转但 --no-winding-invert 强制保持原 winding")
    else:
        print(f"[2/5] 无轴翻转，仅归一化 target_length={args.target_length}")
    verts = apply_verts_transform(verts, args.flip_x, args.flip_y, args.flip_z, args.target_length)
    V = np.array(verts)
    print(f"      [final] bbox min={V.min(axis=0).tolist()} max={V.max(axis=0).tolist()}")

    # 输出路径
    mesh_dir = RESOURCES_ROOT / "assets" / args.namespace / "models" / "item" / args.asset_id
    obj_out = mesh_dir / f"{args.asset_id}.obj"
    mtl_out = mesh_dir / f"{args.asset_id}.mtl"
    tex_dir = RESOURCES_ROOT / "assets" / args.namespace / "textures" / "item" / args.asset_id
    basecolor_out = tex_dir / "basecolor.png"
    if args.override:
        ov_ns, ov_path = args.override.split(":", 1)
        item_json_out = RESOURCES_ROOT / "assets" / ov_ns / "models" / "item" / f"{ov_path}.json"
    else:
        item_json_out = RESOURCES_ROOT / "assets" / args.namespace / "models" / "item" / f"{args.asset_id}.json"

    print(f"[3/5] 输出目标:")
    print(f"  obj     {obj_out}")
    print(f"  mtl     {mtl_out}")
    print(f"  tex     {basecolor_out}")
    print(f"  json    {item_json_out}")
    if args.dry_run:
        print("DRY RUN: 不落盘。")
        return

    mesh_dir.mkdir(parents=True, exist_ok=True)
    tex_dir.mkdir(parents=True, exist_ok=True)
    item_json_out.parent.mkdir(parents=True, exist_ok=True)
    # 清理旧顶点色路径残留
    for k in range(16):
        old = tex_dir / f"{k}.png"
        if old.exists():
            old.unlink()
    old_single = RESOURCES_ROOT / "assets" / args.namespace / "textures" / "item" / f"{args.asset_id}.png"
    if old_single.exists():
        old_single.unlink()

    material_name = f"{args.asset_id}_mat_0"
    tex_ref = f"{args.namespace}:item/{args.asset_id}/basecolor"

    print(f"[4/5] 写 OBJ + MTL + basecolor PNG")
    write_obj_with_uvs(obj_out, verts, uvs, faces, mtl_out.name, material_name, invert_winding)
    mtl_out.write_text(build_textured_mtl(material_name, tex_ref), encoding="utf-8")
    img = Image.open(texture_path)
    if img.mode != "RGB":
        img = img.convert("RGB")
    original_size = img.size
    if max(img.size) > args.texture_max_size:
        img.thumbnail((args.texture_max_size, args.texture_max_size), Image.Resampling.LANCZOS)
    img.save(basecolor_out, "PNG", optimize=True)
    size_tag = (
        f"{img.size[0]}×{img.size[1]}"
        if img.size == original_size
        else f"{original_size[0]}×{original_size[1]} → {img.size[0]}×{img.size[1]}"
    )
    print(f"      basecolor.png {basecolor_out.stat().st_size / 1024:.1f} KB ← {texture_path.name} ({size_tag})")

    print(f"[5/5] 写 SML item JSON")
    override = load_display_override(args.asset_id)
    display_final = merge_display(HANDHELD_DISPLAY, override)
    item_json = {
        "parent": "sml:builtin/obj",
        "model": f"{args.namespace}:models/item/{args.asset_id}/{args.asset_id}.obj",
        "display": display_final,
    }
    item_json_out.write_text(json.dumps(item_json, indent=2), encoding="utf-8")

    print("DONE. Asset breakdown:")
    for p in (obj_out, mtl_out, item_json_out, basecolor_out):
        size_kb = p.stat().st_size / 1024
        print(f"  {size_kb:7.1f} KB  {p.relative_to(CLIENT_ROOT)}")


def detect_texture_source(obj_path: Path) -> tuple[Path | None, Path | None]:
    """探测 OBJ 是否引用 MTL + 外部贴图。返回 (mtl_path, texture_path)。
    两者都有 → 纹理路径可用；任一 None → 退回顶点色路径。
    """
    mtl_ref: str | None = None
    with obj_path.open("r", encoding="utf-8", errors="replace") as f:
        for line in f:
            if line.startswith("mtllib "):
                mtl_ref = line.split(None, 1)[1].strip()
                break
    if not mtl_ref:
        return None, None
    mtl_path = obj_path.parent / mtl_ref
    if not mtl_path.exists():
        return None, None
    tex_ref: str | None = None
    with mtl_path.open("r", encoding="utf-8", errors="replace") as f:
        for line in f:
            s = line.strip()
            if s.startswith("map_Kd "):
                tex_ref = s.split(None, 1)[1].strip()
                break
    if not tex_ref:
        return mtl_path, None
    tex_path = obj_path.parent / tex_ref
    return (mtl_path, tex_path) if tex_path.exists() else (mtl_path, None)


def compute_face_normals(verts: np.ndarray, faces: np.ndarray) -> np.ndarray:
    """每面法向量（归一化）。verts (Nv,3), faces (Nf,3) int。"""
    a = verts[faces[:, 0]]
    b = verts[faces[:, 1]]
    c = verts[faces[:, 2]]
    n = np.cross(b - a, c - a)
    norm = np.linalg.norm(n, axis=1, keepdims=True)
    return n / np.maximum(norm, 1e-12)


def _log_bbox(ms: pymeshlab.MeshSet, tag: str) -> None:
    vmatrix = ms.current_mesh().vertex_matrix()
    bbox_min = vmatrix.min(axis=0)
    bbox_max = vmatrix.max(axis=0)
    extent = bbox_max - bbox_min
    print(f"      [{tag}] bbox min={bbox_min.tolist()} max={bbox_max.tolist()} extent={extent.tolist()}")


def apply_axis_flips(ms: pymeshlab.MeshSet, flip_x: bool, flip_y: bool, flip_z: bool) -> bool:
    """做轴翻转；返回 needs_winding_invert：是否需要在 OBJ 写出时反 face winding。"""
    if not (flip_x or flip_y or flip_z):
        return False
    mat = np.eye(4, dtype=np.float64)
    if flip_x:
        mat[0, 0] = -1.0
    if flip_y:
        mat[1, 1] = -1.0
    if flip_z:
        mat[2, 2] = -1.0
    ms.apply_filter(
        "set_matrix",
        transformmatrix=mat.astype(np.float32),
        compose=False,
        freeze=True,
        alllayers=False,
    )
    # 奇数次轴翻转反转 winding，偶数次不反。由下游 OBJ 写出时处理，不再依赖 pymeshlab filter。
    flips = sum(1 for f in (flip_x, flip_y, flip_z) if f)
    return flips % 2 == 1


def normalize_mesh(ms: pymeshlab.MeshSet, target_length: float) -> None:
    """居中 + 沿最长轴缩放到 target_length。原点移到几何底部中心（Y-min 对齐 Y=0，即 item/handheld 握柄位置）。"""
    m = ms.current_mesh()
    vmatrix = m.vertex_matrix()
    bbox_min = vmatrix.min(axis=0)
    bbox_max = vmatrix.max(axis=0)
    extent = bbox_max - bbox_min
    longest = float(extent.max())
    if longest <= 1e-9:
        print("WARN: mesh degenerate (zero extent), skipping normalization", file=sys.stderr)
        return
    scale = target_length / longest
    cx = (bbox_min[0] + bbox_max[0]) * 0.5
    cz = (bbox_min[2] + bbox_max[2]) * 0.5
    ty = bbox_min[1]
    mat = np.eye(4, dtype=np.float64)
    mat[0, 0] = mat[1, 1] = mat[2, 2] = scale
    mat[0, 3] = -cx * scale
    mat[1, 3] = -ty * scale
    mat[2, 3] = -cz * scale
    ms.apply_filter(
        "set_matrix",
        transformmatrix=mat.astype(np.float32),
        compose=False,
        freeze=True,
        alllayers=False,
    )


def run_pipeline(args: PipelineArgs) -> None:
    if not args.input_obj.exists():
        raise FileNotFoundError(args.input_obj)

    # 纹理路径优先：检测到 MTL + 贴图直接走手工 OBJ parser（完全不用 pymeshlab），
    # 这样 Tripo 原 face/UV 索引关系 100% 保留，避免 pymeshlab load 阶段打乱 wedge 顺序。
    textured_mtl, textured_tex = detect_texture_source(args.input_obj)
    if textured_mtl is not None and textured_tex is not None:
        print(f"检测到纹理 OBJ (MTL={textured_mtl.name}, 贴图={textured_tex.name})")
        print(f"走手工纹理路径 —— 绕过 pymeshlab 保留原 UV")
        run_texture_pipeline(args, textured_mtl, textured_tex)
        return

    is_textured = False  # 后面代码保持引用但不会再走 texture 分支
    print(f"[1/8] 读源: {args.input_obj}")
    if is_textured:
        src_verts = np.empty((0, 3), dtype=np.float64)
        src_colors = np.empty((0, 3), dtype=np.float64)
    else:
        src_verts, src_colors = load_source_verts_and_colors(args.input_obj)
        if src_colors.shape[0] == 0:
            print("      WARN: 原 OBJ 无顶点色也无纹理，聚类降级为单色银灰", file=sys.stderr)
        else:
            mean_col = src_colors.mean(axis=0)
            print(f"      顶点色路径 src verts={src_verts.shape[0]} 均值色 RGB={tuple(float(c) for c in mean_col)}")

    ms = pymeshlab.MeshSet()
    ms.load_new_mesh(str(args.input_obj))
    m0 = ms.current_mesh()
    print(f"      初始 verts={m0.vertex_number()} faces={m0.face_number()}")
    _log_bbox(ms, "raw")

    # 减面：如果当前面数已 ≤ target，直接跳过（保证 UV / wedge 1:1 原样）。
    # 纹理路径优先用 with_texture 版本保留 UV（参数集和基础版不同，去掉 autoclean/planarquadric）；
    # 失败则退回基础版但会丢 UV —— 所以此时会 warn。
    print(f"[2/8] 减面目标: {args.target_faces} (纹理={'是' if is_textured else '否'})")
    current_faces = ms.current_mesh().face_number()
    if current_faces <= args.target_faces:
        print(f"      当前 {current_faces} 面 ≤ target {args.target_faces}，跳过减面（保留原 UV）")
    else:
        base_kwargs = dict(
            targetfacenum=args.target_faces,
            preserveboundary=True,
            preservenormal=True,
            optimalplacement=True,
            planarquadric=False,
            autoclean=True,
        )
        if is_textured:
            tex_kwargs = dict(
                targetfacenum=args.target_faces,
                preserveboundary=True,
                preservenormal=True,
                optimalplacement=True,
            )
            try:
                ms.apply_filter("meshing_decimation_quadric_edge_collapse_with_texture", **tex_kwargs)
            except Exception as e:
                print(
                    f"      WARN: with_texture 减面失败 ({e})，退回基础版（UV 可能失真）",
                    file=sys.stderr,
                )
                ms.apply_filter("meshing_decimation_quadric_edge_collapse", **base_kwargs)
        else:
            ms.apply_filter("meshing_decimation_quadric_edge_collapse", **base_kwargs)
    m1 = ms.current_mesh()
    print(f"      处理后 verts={m1.vertex_number()} faces={m1.face_number()}")

    # asset config 可能提供 tint（仅顶点色路径用）
    asset_config = load_asset_config(args.asset_id)
    tint = asset_config.get("tint")

    if is_textured:
        print(f"[3/8] 纹理路径：单 material + 原 UV，跳过采样/聚类")
        reduced_faces = ms.current_mesh().face_matrix().copy()
        n_faces = int(reduced_faces.shape[0])
        face_cluster_ids = np.zeros(n_faces, dtype=np.int32)
        palette = np.array([[1.0, 1.0, 1.0]], dtype=np.float64)
        actual_k = 1
        if tint is not None:
            print(f"      WARN: 纹理路径下 tint 被忽略（tint 只用于顶点色灰度染色）", file=sys.stderr)
    else:
        # 注意：face color 采样必须在减面后、任何坐标变换前进行，
        # 此时 pymeshlab mesh 坐标仍和源 OBJ 一致，可以用 KD-tree 最近邻回查 src_colors。
        print(f"[3/8] 采样 face 色 + 聚类 K={args.colors}")
        reduced_verts_raw = ms.current_mesh().vertex_matrix().copy()
        reduced_faces = ms.current_mesh().face_matrix().copy()
        face_centers_raw = reduced_verts_raw[reduced_faces].mean(axis=1)
        face_colors = sample_face_colors(src_verts, src_colors, face_centers_raw)
        face_cluster_ids, palette = cluster_face_colors(face_colors, args.colors)
        actual_k = int(palette.shape[0])
        if tint is not None:
            print(f"      应用 tint {tint} 到 palette（聚类原色 × tint）")
            palette = apply_tint(palette, tint)
        print(f"      聚类实际 K={actual_k}，palette:")
        for k in range(actual_k):
            r, g, b = palette[k]
            n_in_cluster = int((face_cluster_ids == k).sum())
            print(f"        [{k}] RGB=({r:.3f}, {g:.3f}, {b:.3f})  面数={n_in_cluster}")

    needs_winding_invert = False
    if args.flip_x or args.flip_y or args.flip_z:
        flips = [a for a, v in [("x", args.flip_x), ("y", args.flip_y), ("z", args.flip_z)] if v]
        print(f"[4/8] 轴翻转: {'+'.join(flips)}")
        needs_winding_invert = apply_axis_flips(ms, args.flip_x, args.flip_y, args.flip_z)
        _log_bbox(ms, "after-flip")
        if needs_winding_invert:
            print(f"      奇数次翻转 → OBJ 写出时反 face winding")
    else:
        print(f"[4/8] 无轴翻转")

    print(f"[5/8] 归一化：target_length={args.target_length} (MC 单位)")
    normalize_mesh(ms, args.target_length)
    _log_bbox(ms, "final")

    # 输出目录
    mesh_dir = RESOURCES_ROOT / "assets" / args.namespace / "models" / "item" / args.asset_id
    obj_out = mesh_dir / f"{args.asset_id}.obj"
    mtl_out = mesh_dir / f"{args.asset_id}.mtl"
    tex_dir = RESOURCES_ROOT / "assets" / args.namespace / "textures" / "item" / args.asset_id

    if args.override:
        ov_ns, ov_path = args.override.split(":", 1)
        item_json_out = RESOURCES_ROOT / "assets" / ov_ns / "models" / "item" / f"{ov_path}.json"
    else:
        item_json_out = RESOURCES_ROOT / "assets" / args.namespace / "models" / "item" / f"{args.asset_id}.json"

    tex_desc = "1 张 basecolor.png（Tripo 原贴图转 PNG）" if is_textured else f"{actual_k} 张 1×1 纯色 PNG"
    print(f"[6/8] 输出目标:")
    print(f"  obj     {obj_out}")
    print(f"  mtl     {mtl_out}")
    print(f"  tex dir {tex_dir}/*.png  ({tex_desc})")
    print(f"  json    {item_json_out}")

    if args.dry_run:
        print("DRY RUN: 不落盘。")
        return

    mesh_dir.mkdir(parents=True, exist_ok=True)
    tex_dir.mkdir(parents=True, exist_ok=True)
    item_json_out.parent.mkdir(parents=True, exist_ok=True)
    # 清理同目录下可能遗留的旧单色贴图（旧版本脚本把 <id>.png 放在 textures/item/ 下）
    old_single = RESOURCES_ROOT / "assets" / args.namespace / "textures" / "item" / f"{args.asset_id}.png"
    if old_single.exists():
        old_single.unlink()

    branch_label = "纹理" if is_textured else "顶点色"
    print(f"[7/8] 写 OBJ + MTL + PNG（{branch_label}路径）")
    # 在写 OBJ 前拿最终（变换后）坐标
    final_verts = ms.current_mesh().vertex_matrix().copy()
    final_faces = ms.current_mesh().face_matrix().copy()
    # face normals 在 winding invert 前计算（invert 只影响 OBJ 里的索引顺序，不改正面朝向语义，
    # 因为 invert 就是把反向 winding 修正回正面）；我们根据当前 face indices 算出当前几何法线。
    face_normals = compute_face_normals(final_verts, final_faces)
    # 奇数轴翻转后，几何上法线会反向指向，需手动反转 normal 向量保持正面朝外
    if needs_winding_invert:
        face_normals = -face_normals

    material_prefix = f"{args.asset_id}_mat"
    tex_ref_prefix = f"{args.namespace}:item/{args.asset_id}"

    wedge_uvs: np.ndarray | None = None
    if is_textured:
        try:
            raw_uvs = ms.current_mesh().wedge_tex_coord_matrix()
            if raw_uvs.shape[0] == final_faces.shape[0] * 3:
                wedge_uvs = raw_uvs.copy()
            else:
                print(
                    f"      WARN: wedge UV shape {raw_uvs.shape} 和 faces {final_faces.shape} 对不上，"
                    "退回 trivial UV（贴图会错位）",
                    file=sys.stderr,
                )
        except Exception as e:
            print(f"      WARN: 读取 wedge UV 失败 ({e})，退回 trivial", file=sys.stderr)

    write_multimaterial_obj(
        path=obj_out,
        verts=final_verts,
        faces=final_faces,
        face_normals=face_normals,
        face_clusters=face_cluster_ids,
        n_clusters=actual_k,
        material_prefix=material_prefix,
        mtl_filename=mtl_out.name,
        invert_winding=needs_winding_invert,
        wedge_uvs=wedge_uvs,
    )

    if is_textured:
        # 复制 Tripo 原贴图 → PNG（JPEG/其他格式由 Pillow 自动转换），超尺寸时下采样
        basecolor_out = tex_dir / "basecolor.png"
        img = Image.open(textured_tex)
        if img.mode != "RGB":
            img = img.convert("RGB")
        original_size = img.size
        max_side = max(img.size)
        if max_side > args.texture_max_size:
            img.thumbnail((args.texture_max_size, args.texture_max_size), Image.Resampling.LANCZOS)
        img.save(basecolor_out, "PNG", optimize=True)
        size_tag = (
            f"{img.size[0]}×{img.size[1]}"
            if img.size == original_size
            else f"{original_size[0]}×{original_size[1]} → {img.size[0]}×{img.size[1]}"
        )
        print(f"      basecolor.png {basecolor_out.stat().st_size / 1024:.1f} KB "
              f"← {textured_tex.name} ({size_tag})")

        mtl_content = build_textured_mtl(
            material_name=f"{material_prefix}_0",
            tex_ref=f"{tex_ref_prefix}/basecolor",
        )
    else:
        # K 张 1×1 纯色 PNG
        for k in range(actual_k):
            rgb255 = tuple(max(0, min(255, int(round(c * 255)))) for c in palette[k])
            Image.new("RGB", (1, 1), rgb255).save(tex_dir / f"{k}.png")
        mtl_content = build_multi_mtl(
            n_clusters=actual_k,
            palette=palette,
            material_prefix=material_prefix,
            tex_ref_prefix=tex_ref_prefix,
        )
    mtl_out.write_text(mtl_content, encoding="utf-8")

    print(f"[8/8] 写 SML item JSON")
    override = load_display_override(args.asset_id)
    if override:
        override_modes = ", ".join(override.keys())
        print(f"      合并 asset config: {ASSET_CONFIG_DIR / (args.asset_id + '.json')} (覆盖 {override_modes})")
    else:
        print(f"      无 asset config；全套用 HANDHELD_DISPLAY 默认")
    display_final = merge_display(HANDHELD_DISPLAY, override)
    item_json = {
        "parent": "sml:builtin/obj",
        "model": f"{args.namespace}:models/item/{args.asset_id}/{args.asset_id}.obj",
        "display": display_final,
    }
    item_json_out.write_text(json.dumps(item_json, indent=2), encoding="utf-8")

    print("DONE. Asset breakdown:")
    if is_textured:
        breakdown = [obj_out, mtl_out, item_json_out, tex_dir / "basecolor.png"]
    else:
        breakdown = [obj_out, mtl_out, item_json_out] + [tex_dir / f"{k}.png" for k in range(actual_k)]
    for p in breakdown:
        size_kb = p.stat().st_size / 1024
        print(f"  {size_kb:7.1f} KB  {p.relative_to(CLIENT_ROOT)}")


def write_multimaterial_obj(
    path: Path,
    verts: np.ndarray,
    faces: np.ndarray,
    face_normals: np.ndarray,
    face_clusters: np.ndarray,
    n_clusters: int,
    material_prefix: str,
    mtl_filename: str,
    invert_winding: bool,
    wedge_uvs: np.ndarray | None = None,
) -> None:
    """手写多 material OBJ。

    wedge_uvs=None → trivial (0,0)(1,0)(0,1) 三 UV 共用（1×1 纯色贴图路径够用）。
    wedge_uvs=(Nf*3, 2) 数组 → 每 face 三 wedge 独立 UV（纹理路径保留 Tripo 原 UV）。

    - verts / vt / vn 都 1-indexed
    - invert_winding 时 face (a,b,c) 写成 (a,c,b)；wedge UV 也同步 swap（对应顶点顺序）
    """
    n_faces = faces.shape[0]
    use_real_uv = wedge_uvs is not None
    with path.open("w", encoding="utf-8") as f:
        f.write("# Generated by tripo_to_sml.py\n")
        f.write(f"mtllib {mtl_filename}\n")
        for v in verts:
            f.write(f"v {v[0]:.6f} {v[1]:.6f} {v[2]:.6f}\n")
        if use_real_uv:
            for uv in wedge_uvs:
                f.write(f"vt {float(uv[0]):.6f} {float(uv[1]):.6f}\n")
        else:
            f.write("vt 0.000000 0.000000\n")
            f.write("vt 1.000000 0.000000\n")
            f.write("vt 0.000000 1.000000\n")
        for n in face_normals:
            f.write(f"vn {n[0]:.6f} {n[1]:.6f} {n[2]:.6f}\n")
        for k in range(n_clusters):
            face_idx = np.where(face_clusters == k)[0]
            if face_idx.size == 0:
                continue
            f.write(f"usemtl {material_prefix}_{k}\n")
            for fi in face_idx:
                tri = faces[fi]
                if invert_winding:
                    a, b, c = int(tri[0]) + 1, int(tri[2]) + 1, int(tri[1]) + 1
                    if use_real_uv:
                        uv_a, uv_b, uv_c = fi * 3 + 1, fi * 3 + 3, fi * 3 + 2
                    else:
                        uv_a, uv_b, uv_c = 1, 3, 2
                else:
                    a, b, c = int(tri[0]) + 1, int(tri[1]) + 1, int(tri[2]) + 1
                    if use_real_uv:
                        uv_a, uv_b, uv_c = fi * 3 + 1, fi * 3 + 2, fi * 3 + 3
                    else:
                        uv_a, uv_b, uv_c = 1, 2, 3
                nrm = int(fi) + 1
                f.write(f"f {a}/{uv_a}/{nrm} {b}/{uv_b}/{nrm} {c}/{uv_c}/{nrm}\n")


def build_textured_mtl(material_name: str, tex_ref: str) -> str:
    """纹理路径的单 material MTL。map_Kd 指向 Tripo 原贴图转 PNG 后的 `<ns>:item/<id>/basecolor`。"""
    return (
        "# Generated by tripo_to_sml.py (textured)\n"
        f"newmtl {material_name}\n"
        "Ka 1.000000 1.000000 1.000000\n"
        "Kd 1.000000 1.000000 1.000000\n"
        "Ks 0.000000 0.000000 0.000000\n"
        "Ns 10.000000\n"
        "d 1.000000\n"
        "illum 1\n"
        f"map_Kd {tex_ref}\n"
    )


def build_multi_mtl(
    n_clusters: int,
    palette: np.ndarray,
    material_prefix: str,
    tex_ref_prefix: str,
) -> str:
    """每 cluster 一段 material，map_Kd 指向 <ns>:item/<asset_id>/<k>（SML 的 namespace:path 约定）。"""
    lines = ["# Generated by tripo_to_sml.py\n"]
    for k in range(n_clusters):
        r, g, b = palette[k]
        lines.extend(
            [
                f"newmtl {material_prefix}_{k}",
                "Ka 1.000000 1.000000 1.000000",
                f"Kd {r:.6f} {g:.6f} {b:.6f}",
                "Ks 0.000000 0.000000 0.000000",
                "Ns 10.000000",
                "d 1.000000",
                "illum 1",
                f"map_Kd {tex_ref_prefix}/{k}",
                "",
            ]
        )
    return "\n".join(lines)


def main() -> None:
    args = parse_args(sys.argv[1:])
    run_pipeline(args)


if __name__ == "__main__":
    main()
