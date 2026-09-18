#!/usr/bin/env python3
"""生成灵布衫四件套的 bbmodel、64x64 贴图与预览。

这套护甲没有外部模型或旧生成器可迁移，形制依据只有 server 的物品描述：
淡青灵布、缠成的头巾、轻便不碍气、布面软靴。几何刻意用薄布片、搭接、缠带和
垂坠表达柔软，不使用硬壳、骨架或金属扣件。

Round 2 的人工判断由 manifests/SpiritCloth*.manifest.toml 点名；本文件只负责
生成事实和可计算门禁。运行时真相仍是 client 的 ArmorPartModel.CUBE_TABLES，
``--emit-java`` 用来生成接线所需的 Java 字面量，禁止手抄。
"""

from __future__ import annotations

import argparse
import copy
import random
from dataclasses import replace
from pathlib import Path

from PIL import Image, ImageDraw

import sys as _sys
from pathlib import Path as _Path

_sys.path.insert(0, str(_Path(__file__).resolve().parents[1] / "core"))

from bbmodel_maker.gates import gatekit
from bbmodel_maker.model.armor_model_common import (
    ArmorPart,
    Cube,
    MOUNT_X,
    TEXTURE_SIZE,
    write_material_assets,
)
from bbmodel_maker.rig.rigkit import Rig

REPO = Path(__file__).resolve().parents[2]
LOCAL_MODELS = Path(__file__).resolve().parents[1] / "models"
PREVIEW_ROOT = Path(__file__).resolve().parents[1] / "out"

MATERIAL = "spirit_cloth"
DRAFT_TEXTURE_ROOT = LOCAL_MODELS / "armor" / MATERIAL / "textures"
CLIENT_TEXTURE_ROOT = (
    REPO / "client" / "src" / "main" / "resources" / "assets" / "bong" / "textures" / "armor"
)

# 64x64 的四格灵布调色：主布淡青、阴影蓝青、月白缠带、灰青针脚。
# 每格 32x32，所有 cube 的 box-UV 都必须完整落在单格内。
UV_CLOTH_MAIN = (0, 0)
UV_CLOTH_SHADE = (32, 0)
UV_WRAP_LIGHT = (0, 32)
UV_STITCH = (32, 32)
UV_TILES = {
    UV_CLOTH_MAIN: (32, 32),
    UV_CLOTH_SHADE: (32, 32),
    UV_WRAP_LIGHT: (32, 32),
    UV_STITCH: (32, 32),
}

# hide 头巾的已验收前缘量级；灵布头巾更贴头，不能做成斗笠檐。
BROW_FRONT_Z_MIN = -5.10
CONTACT_TOL = 0.12


def c(
    mount: str,
    name: str,
    origin: tuple[float, float, float],
    size: tuple[float, float, float],
    uv: tuple[int, int] = UV_CLOTH_MAIN,
) -> Cube:
    return Cube(mount, name, origin, size, uv)


def _side_x(sign: float, inner: float, width: float) -> float:
    """把「朝外为正」的局部 x 变成左右挂载点的局部 origin。"""
    return inner if sign > 0 else -inner - width


# ─── 头巾 ────────────────────────────────────────────────────────────────────


def _helmet_crown() -> tuple[Cube, ...]:
    """贴头的多段缠布；横向不超过 ±4.7，前后用搭接而不是一块硬壳。"""
    return (
        c("HEAD", "wrap_crown_front", (-4.55, 31.15, -4.55), (9.10, 0.72, 2.48), UV_CLOTH_SHADE),
        c("HEAD", "wrap_crown_mid", (-4.50, 31.28, -2.30), (9.00, 0.70, 2.72), UV_CLOTH_MAIN),
        c("HEAD", "wrap_crown_back", (-4.45, 31.12, 0.22), (8.90, 0.72, 3.42), UV_CLOTH_MAIN),
        c("HEAD", "wrap_top_fold", (-4.35, 31.88, -1.72), (8.70, 0.48, 2.20), UV_WRAP_LIGHT),
    )


def _helmet_brow() -> tuple[Cube, ...]:
    """额前横缠布与两侧收口，前缘仅略过头盒。"""
    return (
        c("HEAD", "brow_wrap", (-4.88, 29.58, -4.78), (9.76, 1.02, 0.58), UV_CLOTH_SHADE),
        c("HEAD", "brow_fold", (-4.72, 30.40, -4.65), (9.44, 0.48, 0.38), UV_WRAP_LIGHT),
        c("HEAD", "temple_wrap_left", (-5.05, 29.35, -4.50), (0.90, 2.28, 4.16), UV_CLOTH_SHADE),
        c("HEAD", "temple_wrap_right", (4.15, 29.35, -4.50), (0.90, 2.28, 4.16), UV_CLOTH_SHADE),
    )


def _helmet_ear_flaps() -> tuple[Cube, ...]:
    """护耳只在 x=±5.25 两侧下落，前后角留空，底边收在 y≈24.3。"""
    return (
        c("HEAD", "ear_flap_left", (-5.25, 24.28, -4.00), (1.15, 5.45, 3.70), UV_CLOTH_MAIN),
        c("HEAD", "ear_flap_right", (4.10, 24.28, -4.00), (1.15, 5.45, 3.70), UV_CLOTH_MAIN),
        c("HEAD", "ear_band_left", (-5.33, 26.10, -3.72), (0.20, 0.38, 3.12), UV_WRAP_LIGHT),
        c("HEAD", "ear_band_right", (5.13, 26.10, -3.72), (0.20, 0.38, 3.12), UV_WRAP_LIGHT),
        c("HEAD", "ear_band_high_left", (-5.33, 28.38, -3.72), (0.20, 0.38, 3.12), UV_WRAP_LIGHT),
        c("HEAD", "ear_band_high_right", (5.13, 28.38, -3.72), (0.20, 0.38, 3.12), UV_WRAP_LIGHT),
    )


def _helmet_rear_and_ties() -> tuple[Cube, ...]:
    return (
        # 侧后缠布接住护耳、眉侧与后脑，填掉 y=26..31 的连续包覆区；
        # x 仍收在 ±5.04，不把轻薄头巾横向做宽。
        c("HEAD", "side_curtain_left", (-5.04, 26.00, -0.36), (0.88, 5.44, 4.38), UV_CLOTH_MAIN),
        c("HEAD", "side_curtain_right", (4.16, 26.00, -0.36), (0.88, 5.44, 4.38), UV_CLOTH_MAIN),
        c("HEAD", "rear_wrap", (-4.40, 27.25, 2.76), (8.80, 4.18, 1.02), UV_CLOTH_SHADE),
        c("HEAD", "rear_fold", (-4.22, 24.42, 3.02), (8.44, 2.88, 0.92), UV_CLOTH_MAIN),
        c("HEAD", "temple_tail_left", (-5.36, 26.92, -3.58), (0.32, 2.92, 0.38), UV_STITCH),
        c("HEAD", "temple_tail_right", (5.04, 26.92, -3.58), (0.32, 2.92, 0.38), UV_STITCH),
        c("HEAD", "tie_knot_left", (-5.56, 26.35, -3.70), (0.62, 0.72, 0.62), UV_WRAP_LIGHT),
        c("HEAD", "tie_knot_right", (4.94, 26.35, -3.70), (0.62, 0.72, 0.62), UV_WRAP_LIGHT),
        c("HEAD", "brow_center_stitch", (-0.18, 29.72, -5.02), (0.36, 1.05, 0.30), UV_STITCH),
    )


def part_helmet() -> ArmorPart:
    return ArmorPart(
        "spirit_cloth_helmet",
        "SPIRIT CLOTH HELMET",
        _helmet_crown() + _helmet_brow() + _helmet_ear_flaps() + _helmet_rear_and_ties(),
    )


# ─── 胸甲 ────────────────────────────────────────────────────────────────────


def _chest_core() -> tuple[Cube, ...]:
    return (
        c("BODY", "tunic_front", (-4.28, 12.42, -2.54), (8.56, 10.16, 0.58), UV_CLOTH_MAIN),
        c("BODY", "tunic_back", (-4.28, 12.42, 1.94), (8.56, 10.16, 0.58), UV_CLOTH_MAIN),
        c("BODY", "side_panel_left", (-4.46, 12.46, -2.06), (0.46, 10.08, 4.12), UV_CLOTH_SHADE),
        c("BODY", "side_panel_right", (4.00, 12.46, -2.06), (0.46, 10.08, 4.12), UV_CLOTH_SHADE),
        c("BODY", "shoulder_yoke_left", (-4.32, 22.64, -2.46), (2.10, 1.18, 4.94), UV_CLOTH_MAIN),
        c("BODY", "shoulder_yoke_right", (2.22, 22.64, -2.46), (2.10, 1.18, 4.94), UV_CLOTH_MAIN),
        c("BODY", "neck_fold", (-2.16, 23.18, -2.72), (4.32, 0.58, 0.62), UV_WRAP_LIGHT),
    )


def _chest_sash_and_wraps() -> tuple[Cube, ...]:
    return (
        # 阶梯式相互搭接，读成缠绕的交领而不是一块贴图上的斜线。
        c("BODY", "sash_upper", (-3.18, 20.22, -2.86), (6.36, 1.18, 0.42), UV_CLOTH_SHADE),
        c("BODY", "sash_mid", (-2.42, 17.82, -2.88), (4.84, 2.62, 0.42), UV_CLOTH_SHADE),
        c("BODY", "sash_lower", (-1.62, 15.60, -2.90), (3.24, 2.42, 0.42), UV_CLOTH_SHADE),
        c("BODY", "sash_edge_upper", (-3.24, 20.02, -3.04), (6.48, 0.30, 0.28), UV_WRAP_LIGHT),
        c("BODY", "sash_edge_lower", (-1.72, 15.42, -3.08), (3.44, 0.30, 0.28), UV_WRAP_LIGHT),
        c("BODY", "waist_wrap_front", (-4.56, 12.04, -3.04), (9.12, 0.66, 0.56), UV_WRAP_LIGHT),
        c("BODY", "waist_wrap_back", (-4.56, 12.04, 2.48), (9.12, 0.66, 0.56), UV_WRAP_LIGHT),
        c("BODY", "waist_wrap_left", (-4.82, 12.04, -2.52), (0.58, 0.66, 5.04), UV_WRAP_LIGHT),
        c("BODY", "waist_wrap_right", (4.24, 12.04, -2.52), (0.58, 0.66, 5.04), UV_WRAP_LIGHT),
        # 两条垂尾刻意一长一短，使用非镜像名称，避免把设计不对称藏进镜像门。
        c("BODY", "waist_tail_long", (-1.18, 10.44, -3.14), (0.62, 1.72, 0.48), UV_CLOTH_SHADE),
        c("BODY", "waist_tail_short", (0.42, 10.76, -3.12), (0.62, 1.40, 0.48), UV_CLOTH_MAIN),
    )


def _chest_sleeves() -> tuple[Cube, ...]:
    cubes: list[Cube] = []
    for side, sign in (("left", 1.0), ("right", -1.0)):
        def sx(inner: float, width: float) -> float:
            return _side_x(sign, inner, width)

        cubes.extend(
            (
                c("BODY", f"sleeve_shoulder_{side}", (sx(3.92, 4.12), 22.74, -2.40), (4.12, 0.92, 4.82), UV_CLOTH_MAIN),
                c("BODY", f"sleeve_front_{side}", (sx(4.04, 3.62), 17.04, -2.58), (3.62, 5.86, 0.52), UV_CLOTH_MAIN),
                c("BODY", f"sleeve_back_{side}", (sx(4.04, 3.62), 17.04, 2.04), (3.62, 5.86, 0.52), UV_CLOTH_MAIN),
                c("BODY", f"sleeve_outer_{side}", (sx(7.62, 0.58), 17.04, -2.06), (0.58, 5.86, 4.16), UV_CLOTH_SHADE),
                c("BODY", f"sleeve_cuff_{side}", (sx(3.98, 4.02), 16.58, -2.68), (4.02, 0.66, 5.36), UV_WRAP_LIGHT),
                c("BODY", f"sleeve_stitch_{side}", (sx(7.88, 0.30), 19.20, -2.35), (0.30, 0.38, 0.72), UV_STITCH),
            )
        )
    return tuple(cubes)


def part_chestplate() -> ArmorPart:
    return ArmorPart(
        "spirit_cloth_chestplate",
        "SPIRIT CLOTH CHESTPLATE",
        _chest_core() + _chest_sash_and_wraps() + _chest_sleeves(),
    )


# ─── 护腿 ────────────────────────────────────────────────────────────────────


def _leg_cubes(mount: str, sign: float) -> tuple[Cube, ...]:
    side = "left" if sign > 0 else "right"

    def lx(inner: float, width: float) -> float:
        return _side_x(sign, inner, width)

    def c2(name: str, origin: tuple[float, float, float], size: tuple[float, float, float], uv=UV_CLOTH_MAIN) -> Cube:
        return c(mount, f"{name}_{side}", origin, size, uv)

    return (
        c2("hip_front", (lx(-1.82, 3.74), 10.30, -2.42), (3.74, 1.64, 0.52), UV_CLOTH_MAIN),
        c2("hip_back", (lx(-1.82, 3.74), 10.30, 1.90), (3.74, 1.64, 0.52), UV_CLOTH_MAIN),
        c2("hip_outer", (lx(1.82, 0.48), 10.26, -1.98), (0.48, 1.68, 3.90), UV_CLOTH_SHADE),
        c2("thigh_front", (lx(-1.78, 3.66), 6.34, -2.46), (3.66, 4.12, 0.48), UV_CLOTH_MAIN),
        c2("thigh_back", (lx(-1.78, 3.66), 6.34, 1.96), (3.66, 4.12, 0.48), UV_CLOTH_MAIN),
        c2("thigh_outer", (lx(1.76, 0.46), 6.34, -1.96), (0.46, 4.12, 3.92), UV_CLOTH_SHADE),
        c2("calf_front", (lx(-1.80, 3.70), 1.16, -2.50), (3.70, 5.34, 0.50), UV_CLOTH_MAIN),
        c2("calf_back", (lx(-1.80, 3.70), 1.16, 1.92), (3.70, 5.34, 0.50), UV_CLOTH_MAIN),
        c2("calf_outer", (lx(1.78, 0.48), 1.18, -2.02), (0.48, 5.30, 4.00), UV_CLOTH_SHADE),
        c2("wrap_band_high", (lx(-1.88, 3.86), 5.12, -2.66), (3.86, 0.38, 0.54), UV_WRAP_LIGHT),
        c2("wrap_band_mid", (lx(-1.88, 3.86), 3.38, -2.66), (3.86, 0.38, 0.54), UV_WRAP_LIGHT),
        c2("wrap_band_low", (lx(-1.88, 3.86), 1.62, -2.66), (3.86, 0.38, 0.54), UV_WRAP_LIGHT),
        c2("wrap_side_high", (lx(1.96, 0.34), 5.12, -2.18), (0.34, 0.38, 4.30), UV_STITCH),
        c2("wrap_side_low", (lx(1.96, 0.34), 1.62, -2.18), (0.34, 0.38, 4.30), UV_STITCH),
        c2("cuff", (lx(-1.84, 3.78), 0.62, -2.48), (3.78, 0.72, 4.96), UV_WRAP_LIGHT),
    )


def part_leggings() -> ArmorPart:
    return ArmorPart(
        "spirit_cloth_leggings",
        "SPIRIT CLOTH LEGGINGS",
        _leg_cubes("LEFT_LEG", 1.0) + _leg_cubes("RIGHT_LEG", -1.0),
    )


# ─── 软靴 ────────────────────────────────────────────────────────────────────


def _boot_cubes(mount: str, sign: float) -> tuple[Cube, ...]:
    side = "left" if sign > 0 else "right"
    outward_clearance = 0.25

    def bx(inner: float, width: float) -> float:
        return (
            inner + outward_clearance
            if sign > 0
            else -inner - width - outward_clearance
        )

    def c2(name: str, origin: tuple[float, float, float], size: tuple[float, float, float], uv=UV_CLOTH_MAIN) -> Cube:
        return c(mount, f"{name}_{side}", origin, size, uv)

    return (
        # 鞋底分成前掌、中掌、后跟，-z 端明显多伸，避免读成两个箱子。
        c2("sole_toe", (bx(-2.00, 4.00), -0.48, -3.92), (4.00, 0.52, 2.38), UV_CLOTH_SHADE),
        c2("sole_mid", (bx(-1.99, 3.98), -0.46, -1.60), (3.98, 0.48, 2.18), UV_CLOTH_SHADE),
        c2("sole_heel", (bx(-1.78, 3.56), -0.43, 0.66), (3.56, 0.44, 1.96), UV_CLOTH_SHADE),
        c2("sole_front_wrap", (bx(-2.08, 4.16), -0.08, -4.12), (4.16, 0.38, 0.42), UV_WRAP_LIGHT),
        c2("sole_heel_wrap", (bx(-1.86, 3.72), -0.05, 2.56), (3.72, 0.36, 0.34), UV_WRAP_LIGHT),
        # 鞋面是柔软分片，不封成一个方盒；toe/vamp/heel 三段明确前后。
        c2("toe_cap", (bx(-1.92, 3.84), 0.02, -3.72), (3.84, 0.80, 1.26), UV_CLOTH_MAIN),
        c2("vamp_front", (bx(-1.88, 3.76), 0.24, -2.84), (3.76, 0.98, 1.34), UV_CLOTH_MAIN),
        c2("vamp_back", (bx(-1.84, 3.68), 0.22, -1.62), (3.68, 1.02, 1.12), UV_CLOTH_MAIN),
        c2("heel_panel", (bx(-1.76, 3.52), 0.10, 0.48), (3.52, 1.22, 1.62), UV_CLOTH_SHADE),
        c2("toe_fold", (bx(-1.74, 3.48), 0.78, -3.86), (3.48, 0.34, 0.74), UV_WRAP_LIGHT),
        c2("vamp_lace_front", (bx(-1.54, 3.08), 1.18, -2.48), (3.08, 0.30, 0.32), UV_STITCH),
        c2("vamp_lace_back", (bx(-1.44, 2.88), 1.38, -1.04), (2.88, 0.30, 0.32), UV_STITCH),
        # 鞋筒四片开口，保持软靴而不是硬箱。
        c2("shaft_front", (bx(-1.82, 3.64), 1.18, -1.86), (3.64, 3.18, 0.40), UV_CLOTH_SHADE),
        c2("shaft_back", (bx(-1.78, 3.56), 1.18, 1.48), (3.56, 3.18, 0.40), UV_CLOTH_SHADE),
        c2("shaft_outer", (bx(1.76, 0.40), 1.18, -1.46), (0.40, 3.18, 2.98), UV_CLOTH_SHADE),
        c2("shaft_inner", (bx(-1.72, 0.34), 1.18, -1.42), (0.34, 3.18, 2.90), UV_CLOTH_MAIN),
        c2("shaft_top_front", (bx(-1.88, 3.76), 4.42, -1.92), (3.76, 0.38, 0.38), UV_WRAP_LIGHT),
        c2("shaft_top_back", (bx(-1.84, 3.68), 4.42, 1.56), (3.68, 0.38, 0.36), UV_WRAP_LIGHT),
        c2("ankle_wrap_front", (bx(-1.94, 3.88), 1.36, -2.04), (3.88, 0.34, 0.30), UV_STITCH),
        c2("ankle_wrap_back", (bx(-1.90, 3.80), 1.40, 1.70), (3.80, 0.34, 0.28), UV_STITCH),
        c2("ankle_wrap_outer", (bx(1.84, 0.30), 1.38, -1.72), (0.30, 0.34, 3.54), UV_STITCH),
        c2("ankle_tie", (bx(2.06, 0.68), 2.18, -0.88), (0.68, 0.68, 0.70), UV_WRAP_LIGHT),
    )


def part_boots() -> ArmorPart:
    return ArmorPart(
        "spirit_cloth_boots",
        "SPIRIT CLOTH BOOTS",
        _boot_cubes("LEFT_FOOT", 1.0) + _boot_cubes("RIGHT_FOOT", -1.0),
    )


def parts() -> tuple[ArmorPart, ...]:
    return (part_helmet(), part_chestplate(), part_leggings(), part_boots())


# ─── 可计算几何门 ────────────────────────────────────────────────────────────


def _world_box(cube: Cube) -> tuple[tuple[float, float], ...]:
    offset = MOUNT_X[cube.mount]
    origin = (cube.origin[0] + offset, cube.origin[1], cube.origin[2])
    return tuple((origin[i], origin[i] + cube.size[i]) for i in range(3))


def _cube_bounds(cube: Cube) -> tuple[tuple[float, ...], tuple[float, ...]]:
    box = _world_box(cube)
    return tuple(axis[0] for axis in box), tuple(axis[1] for axis in box)


def _assert_no_coplanar_faces(all_parts: tuple[ArmorPart, ...]) -> None:
    """同一件内不允许有相交投影的共面外表面。"""
    for part in all_parts:
        cubes = part.cubes
        for i, first in enumerate(cubes):
            low_a, high_a = _cube_bounds(first)
            for second in cubes[i + 1:]:
                low_b, high_b = _cube_bounds(second)
                for axis in range(3):
                    projection = 1.0
                    for other in (k for k in range(3) if k != axis):
                        projection *= max(
                            0.0,
                            min(high_a[other], high_b[other]) - max(low_a[other], low_b[other]),
                        )
                    if projection <= 0.02:
                        continue
                    for side, a, b in (
                        ("min", low_a[axis], low_b[axis]),
                        ("max", high_a[axis], high_b[axis]),
                    ):
                        if abs(a - b) < 1e-6:
                            raise ValueError(
                                f"{part.key}: {first.name} 与 {second.name} 的 "
                                f"{'xyz'[axis]}-{side} 面共面于 {a}，投影相交 {projection:.2f}"
                            )


def _assert_uv_tiles(all_parts: tuple[ArmorPart, ...]) -> None:
    for part in all_parts:
        for cube in part.cubes:
            tile = UV_TILES.get(cube.uv)
            if tile is None:
                raise ValueError(f"{part.key}/{cube.name}: 未知 uv {cube.uv}")
            tile_w, tile_h = tile
            sx, sy, sz = cube.size
            used_w, used_h = 2 * (sx + sz), sy + sz
            if used_w > tile_w + 1e-6 or used_h > tile_h + 1e-6:
                raise ValueError(
                    f"{part.key}/{cube.name}: box-UV {used_w:.2f}×{used_h:.2f} "
                    f"超出 uv{cube.uv} 的 {tile_w}×{tile_h} 格"
                )


def _assert_mirror_symmetry(all_parts: tuple[ArmorPart, ...]) -> None:
    """所有以 _left/_right 命名的件必须严格关于世界 x=0 镜像。"""
    for part in all_parts:
        left = {cube.name[:-5]: cube for cube in part.cubes if cube.name.endswith("_left")}
        right = {cube.name[:-6]: cube for cube in part.cubes if cube.name.endswith("_right")}
        if set(left) != set(right):
            raise ValueError(f"{part.key}: 左右件名不成对 {set(left) ^ set(right)}")
        for name, left_cube in left.items():
            right_cube = right[name]
            left_low = left_cube.origin[0] + MOUNT_X[left_cube.mount]
            right_high = right_cube.origin[0] + MOUNT_X[right_cube.mount] + right_cube.size[0]
            if abs(left_low + right_high) > 1e-6:
                raise ValueError(
                    f"{part.key}/{name}: 左右不镜像（左 x0={left_low:.3f}，右 x1={right_high:.3f}）"
                )
            if left_cube.size != right_cube.size or left_cube.origin[1:] != right_cube.origin[1:]:
                raise ValueError(f"{part.key}/{name}: 左右 y/z/size 不一致")


def _touches(first: Cube, second: Cube, tolerance: float = CONTACT_TOL) -> bool:
    a, b = _world_box(first), _world_box(second)
    overlaps = [min(a[i][1], b[i][1]) - max(a[i][0], b[i][0]) for i in range(3)]
    if any(value < -tolerance for value in overlaps):
        return False
    # 至少两个轴要有实质投影，第三轴允许细小接缝；这不是「看起来靠近」。
    return sum(value > 0.05 for value in overlaps) >= 2


def _assert_no_isolated_cubes(all_parts: tuple[ArmorPart, ...]) -> None:
    """逐挂载点计算连通分量，防止薄缠带/结件漂浮。"""
    for part in all_parts:
        by_mount: dict[str, list[Cube]] = {}
        for cube in part.cubes:
            by_mount.setdefault(cube.mount, []).append(cube)
        for mount, cubes in by_mount.items():
            seen = {0}
            todo = [0]
            while todo:
                index = todo.pop()
                for other, candidate in enumerate(cubes):
                    if other not in seen and _touches(cubes[index], candidate):
                        seen.add(other)
                        todo.append(other)
            if len(seen) != len(cubes):
                names = [cube.name for index, cube in enumerate(cubes) if index not in seen]
                raise ValueError(f"{part.key}/{mount}: 孤立 cube {names}，必须与主体贴合")


def _assert_helmet_front_projection(all_parts: tuple[ArmorPart, ...]) -> None:
    helmet = next(part for part in all_parts if part.key == "spirit_cloth_helmet")
    brow = next(cube for cube in helmet.cubes if cube.name == "brow_wrap")
    if brow.origin[2] < BROW_FRONT_Z_MIN - 1e-6:
        raise ValueError(
            f"spirit_cloth_helmet/brow_wrap 前缘 z={brow.origin[2]:.2f}，"
            f"超过贴头头巾允许的 {BROW_FRONT_Z_MIN:.2f}"
        )


def _assert_shape_dimensions(all_parts: tuple[ArmorPart, ...]) -> None:
    """把「贴头」「前后有脚」这些容易被独立缩放骗过的判断钉成坐标门。"""
    helmet = next(part for part in all_parts if part.key == "spirit_cloth_helmet")
    crown = [cube for cube in helmet.cubes if cube.name.startswith("wrap_crown_")]
    crown_boxes = [_world_box(cube) for cube in crown]
    crown_extent = max(max(box[0][1], -box[0][0]) for box in crown_boxes)
    if crown_extent > 4.70 + 1e-6:
        raise ValueError(f"spirit_cloth_helmet 颅盖横向 {crown_extent:.2f} 超过 ±4.70")
    brow = next(cube for cube in helmet.cubes if cube.name == "brow_wrap")
    if brow.origin[2] < BROW_FRONT_Z_MIN - 1e-6:
        raise ValueError(
            f"spirit_cloth_helmet/brow_wrap 前缘 z={brow.origin[2]:.2f} 超过 {BROW_FRONT_Z_MIN:.2f}"
        )
    left_ear = next(cube for cube in helmet.cubes if cube.name == "ear_flap_left")
    right_ear = next(cube for cube in helmet.cubes if cube.name == "ear_flap_right")
    left_box, right_box = _world_box(left_ear), _world_box(right_ear)
    if abs(left_box[0][0] + 5.25) > 1e-6 or abs(right_box[0][1] - 5.25) > 1e-6:
        raise ValueError("spirit_cloth_helmet 护耳没有落在 x=±5.25")
    if abs(left_box[1][0] - 24.28) > 1e-6 or abs(right_box[1][0] - 24.28) > 1e-6:
        raise ValueError("spirit_cloth_helmet 护耳下沿没有收在 y≈24.3")

    boots = next(part for part in all_parts if part.key == "spirit_cloth_boots")
    for mount in ("LEFT_FOOT", "RIGHT_FOOT"):
        toe = next(cube for cube in boots.cubes if cube.mount == mount and cube.name.startswith("toe_cap_"))
        heel = next(cube for cube in boots.cubes if cube.mount == mount and cube.name.startswith("heel_panel_"))
        toe_min_z = _world_box(toe)[2][0]
        heel_min_z = _world_box(heel)[2][0]
        if toe_min_z >= heel_min_z - 2.0:
            raise ValueError(
                f"{boots.key}/{mount}: 鞋头 z={toe_min_z:.2f} 与后跟 z={heel_min_z:.2f} 前后差不足"
            )


def _helmet_side_coverage(helmet: ArmorPart, side: str) -> tuple[float, float, int]:
    """量 HEAD 两侧耳高区间的 y/z 覆盖，返回面积率、最大断口和断层数。

    这是「缠成的头巾」的几何契约：在 y=26..31 的每个水平薄层，
    z=-4..4 必须由同侧的布片连续覆盖。只看投影总长会把不同 y 的两段
    错当成相连，所以按所有 y 边界分层后再合并 z 区间。
    """
    if side not in {"left", "right"}:
        raise ValueError(f"未知头巾侧面 {side!r}")
    side_x = (-5.25, -4.0) if side == "left" else (4.0, 5.25)
    y_low, y_high = 26.0, 31.0
    z_low, z_high = -4.0, 4.0
    rectangles: list[tuple[float, float, float, float]] = []
    for cube in helmet.cubes:
        box = _world_box(cube)
        if min(box[0][1], side_x[1]) <= max(box[0][0], side_x[0]):
            continue
        y0 = max(box[1][0], y_low)
        y1 = min(box[1][1], y_high)
        z0 = max(box[2][0], z_low)
        z1 = min(box[2][1], z_high)
        if y1 > y0 and z1 > z0:
            rectangles.append((y0, y1, z0, z1))

    y_cuts = sorted({y_low, y_high, *(edge for rect in rectangles for edge in rect[:2])})
    covered_area = 0.0
    max_gap = 0.0
    broken_layers = 0
    for slab_low, slab_high in zip(y_cuts, y_cuts[1:]):
        if slab_high <= slab_low:
            continue
        intervals = sorted(
            (z0, z1)
            for y0, y1, z0, z1 in rectangles
            if y0 <= slab_low and y1 >= slab_high
        )
        merged: list[list[float]] = []
        for z0, z1 in intervals:
            if not merged or z0 > merged[-1][1] + 1e-9:
                merged.append([z0, z1])
            else:
                merged[-1][1] = max(merged[-1][1], z1)

        gaps: list[tuple[float, float]] = []
        cursor = z_low
        for z0, z1 in merged:
            if z0 > cursor + 1e-9:
                gaps.append((cursor, z0))
            cursor = max(cursor, z1)
        if cursor < z_high - 1e-9:
            gaps.append((cursor, z_high))
        if gaps:
            broken_layers += 1
            max_gap = max(max_gap, *(z1 - z0 for z0, z1 in gaps))
        covered_area += (slab_high - slab_low) * sum(z1 - z0 for z0, z1 in merged)

    area_ratio = covered_area / ((y_high - y_low) * (z_high - z_low))
    return area_ratio, max_gap, broken_layers


def _assert_helmet_side_coverage(all_parts: tuple[ArmorPart, ...]) -> None:
    helmet = next(part for part in all_parts if part.key == "spirit_cloth_helmet")
    for side in ("left", "right"):
        ratio, max_gap, broken_layers = _helmet_side_coverage(helmet, side)
        if ratio < 0.999999 or max_gap > 1e-9 or broken_layers:
            curtain = f"side_curtain_{side}"
            raise ValueError(
                f"{helmet.key}/{side}/{curtain}: 侧面覆盖不足，y=26..31 对 z=-4..4 覆盖率 {ratio:.4%}，"
                f"最大断口 {max_gap:.2f}，断层 {broken_layers}"
            )


# ─── gatekit 差分自证 ───────────────────────────────────────────────────────

GATE_MATS = {
    "main": (151, 198, 198),
    "shade": (86, 137, 143),
    "wrap": (211, 229, 222),
    "stitch": (104, 151, 158),
}


def _gate_material(cube: Cube) -> str:
    return {
        UV_CLOTH_MAIN: "main",
        UV_CLOTH_SHADE: "shade",
        UV_WRAP_LIGHT: "wrap",
        UV_STITCH: "stitch",
    }[cube.uv]


def _gate_rig(all_parts: tuple[ArmorPart, ...]) -> Rig:
    rig = Rig(GATE_MATS)
    rig._spirit_parts = tuple(all_parts)
    for part in all_parts:
        rig.bone(part.key, (0.0, 0.0, 0.0))
        for cube in part.cubes:
            low, high = _cube_bounds(cube)
            rig.cube(part.key, cube.name, low, high, mat=_gate_material(cube))
    return rig


def build() -> Rig:
    return _gate_rig(parts())


def _gate_violations(rig: Rig, check) -> list[str]:
    try:
        check(rig._spirit_parts)
    except ValueError as exc:
        return [str(exc)]
    return []


def _replace_gate_cube(rig: Rig, part_key: str, index: int, cube: Cube) -> Rig:
    updated = []
    for part in rig._spirit_parts:
        if part.key == part_key:
            cubes = list(part.cubes)
            cubes[index] = cube
            part = replace(part, cubes=tuple(cubes))
        updated.append(part)
    rig._spirit_parts = tuple(updated)
    return rig


def _inject_coplanar(rig: Rig, **_) -> tuple[Rig, str, str]:
    r = copy.deepcopy(rig)
    for part in r._spirit_parts:
        for first_index, first in enumerate(part.cubes):
            low_a, high_a = _cube_bounds(first)
            for second_index in range(first_index + 1, len(part.cubes)):
                second = part.cubes[second_index]
                low_b, high_b = _cube_bounds(second)
                for axis in range(3):
                    projection = 1.0
                    for other in (k for k in range(3) if k != axis):
                        projection *= max(0.0, min(high_a[other], high_b[other]) - max(low_a[other], low_b[other]))
                    if projection <= 0.02:
                        continue
                    origin = list(second.origin)
                    offset = MOUNT_X[second.mount] if axis == 0 else 0.0
                    origin[axis] = high_a[axis] - second.size[axis] - offset
                    _replace_gate_cube(r, part.key, second_index, replace(second, origin=tuple(origin)))
                    return r, second.name, f"把 {second.name} 的 {'xyz'[axis]} 面移到共面"
    raise gatekit.InjectionImpossible("找不到可造共面且投影相交的 cube 对")


def _inject_uv(rig: Rig, **_) -> tuple[Rig, str, str]:
    r = copy.deepcopy(rig)
    part = r._spirit_parts[0]
    cube = part.cubes[0]
    _replace_gate_cube(r, part.key, 0, replace(cube, uv=(TEXTURE_SIZE, TEXTURE_SIZE)))
    return r, cube.name, f"把 {cube.name} 的 uv 移出 64×64 贴图"


def _inject_mirror(rig: Rig, **_) -> tuple[Rig, str, str]:
    r = copy.deepcopy(rig)
    for part in r._spirit_parts:
        for index, cube in enumerate(part.cubes):
            if cube.name.endswith("_left"):
                _replace_gate_cube(r, part.key, index, replace(cube, origin=(cube.origin[0] + 0.9, *cube.origin[1:])))
                return r, cube.name[:-5], f"把 {cube.name} 单侧平移 0.9"
    raise gatekit.InjectionImpossible("没有参与镜像自检的件")


def _inject_isolated(rig: Rig, **_) -> tuple[Rig, str, str]:
    r = copy.deepcopy(rig)
    for part in r._spirit_parts:
        if len(part.cubes) < 2:
            continue
        index = len(part.cubes) - 1
        cube = part.cubes[index]
        moved = replace(cube, origin=(cube.origin[0] + 12.0, *cube.origin[1:]))
        _replace_gate_cube(r, part.key, index, moved)
        return r, cube.name, f"把 {cube.name} 沿 x 平移 12.0，制造孤立件"
    raise gatekit.InjectionImpossible("没有足够 cube 可注入孤立件")


def _inject_dimensions(rig: Rig, **_) -> tuple[Rig, str, str]:
    r = copy.deepcopy(rig)
    for part in r._spirit_parts:
        for index, cube in enumerate(part.cubes):
            if cube.name == "brow_wrap":
                moved = replace(cube, origin=(cube.origin[0], cube.origin[1], cube.origin[2] - 1.0))
                _replace_gate_cube(r, part.key, index, moved)
                return r, cube.name, "把 brow_wrap 前移 1.0，制造超出头巾前缘的尺寸违例"
    raise gatekit.InjectionImpossible("没有 brow_wrap 可注入尺寸违例")


def _inject_side_coverage(rig: Rig, **_) -> tuple[Rig, str, str]:
    r = copy.deepcopy(rig)
    for part in r._spirit_parts:
        for index, cube in enumerate(part.cubes):
            if cube.name == "side_curtain_left":
                moved = replace(cube, origin=(cube.origin[0], cube.origin[1], cube.origin[2] + 8.0))
                _replace_gate_cube(r, part.key, index, moved)
                return r, cube.name, "把左侧后缠布沿 z 移走，制造耳高区间断口"
    raise gatekit.InjectionImpossible("没有 side_curtain_left 可注入侧面覆盖违例")


class _SpiritClothGates(gatekit.AssetGates):
    def specs(self):
        return (
            ("coplanar", "单件共面 / z-fighting", lambda r: _gate_violations(r, _assert_no_coplanar_faces), _inject_coplanar),
            ("uv_tiles", "box-UV 越出指定色块", lambda r: _gate_violations(r, _assert_uv_tiles), _inject_uv),
            ("mirror", "对称件左右不镜像", lambda r: _gate_violations(r, _assert_mirror_symmetry), _inject_mirror),
            ("isolated_cube", "同挂载点孤立 cube", lambda r: _gate_violations(r, _assert_no_isolated_cubes), _inject_isolated),
            ("shape_dimensions", "贴头/鞋头前后尺寸契约", lambda r: _gate_violations(r, _assert_shape_dimensions), _inject_dimensions),
            ("side_coverage", "头巾耳高区间连续覆盖", lambda r: _gate_violations(r, _assert_helmet_side_coverage), _inject_side_coverage),
        )


GATES = _SpiritClothGates("灵布衫四件套", GATE_MATS)


# ─── 贴图 ────────────────────────────────────────────────────────────────────


def _mottle(image: Image.Image, rng: random.Random, box, count, dark, light, radius) -> None:
    x0, y0, x1, y1 = box
    pixels = image.load()
    for _ in range(count):
        cx, cy = rng.uniform(x0, x1), rng.uniform(y0, y1)
        rx, ry = rng.uniform(*radius), rng.uniform(*radius)
        tint = dark if rng.random() < 0.55 else light
        strength = rng.uniform(0.12, 0.30)
        for y in range(max(y0, int(cy - ry)), min(y1, int(cy + ry) + 1)):
            for x in range(max(x0, int(cx - rx)), min(x1, int(cx + rx) + 1)):
                distance = ((x - cx) / rx) ** 2 + ((y - cy) / ry) ** 2
                if distance > 1.0:
                    continue
                alpha = strength * (1.0 - distance)
                pixels[x, y] = tuple(
                    int(round(channel * (1.0 - alpha) + target * alpha))
                    for channel, target in zip(pixels[x, y], tint)
                )


def make_texture() -> Image.Image:
    """确定性生成淡青灵布、蓝青阴影、月白缠带与灰青针脚。"""
    rng = random.Random(0x535049524954)  # SPIRIT
    image = Image.new("RGB", (TEXTURE_SIZE, TEXTURE_SIZE), GATE_MATS["main"])
    pixels = image.load()

    bases = {
        UV_CLOTH_MAIN: GATE_MATS["main"],
        UV_CLOTH_SHADE: GATE_MATS["shade"],
        UV_WRAP_LIGHT: GATE_MATS["wrap"],
        UV_STITCH: GATE_MATS["stitch"],
    }
    for (u, v), base in bases.items():
        for y in range(v, v + 32):
            for x in range(u, u + 32):
                jitter = rng.randint(-5, 5)
                pixels[x, y] = tuple(max(0, min(255, channel + jitter)) for channel in base)

    _mottle(image, rng, (0, 0, 32, 32), 18, (113, 166, 169), (181, 218, 213), (3.0, 8.0))
    _mottle(image, rng, (32, 0, 64, 32), 14, (58, 105, 114), (111, 164, 167), (3.0, 8.0))
    _mottle(image, rng, (0, 32, 32, 64), 12, (171, 202, 198), (235, 241, 229), (3.0, 7.0))
    _mottle(image, rng, (32, 32, 64, 64), 12, (76, 124, 133), (137, 181, 181), (3.0, 7.0))

    draw = ImageDraw.Draw(image)
    # 主布用稀疏经纬线，不把柔布画成塑料格。
    for row in range(1, 32, 4):
        draw.line((0, row, 31, row), fill=(126, 178, 180), width=1)
    for col in range(2, 32, 5):
        draw.line((col, 0, col, 31), fill=(174, 213, 208), width=1)
    # 阴影象限用柔和斜纹，作为缠绕方向提示。
    for start in range(-28, 64, 6):
        draw.line((32 + start, 31, 32 + start + 30, 0), fill=(65, 112, 120), width=1)
    # 月白缠带用横向折线，针脚象限用短交错线。
    for row in range(33, 64, 4):
        draw.line((0, row, 31, row), fill=(190, 218, 211), width=1)
        for col in range((row // 4) % 2, 32, 5):
            draw.point((col, row), fill=(240, 245, 232))
    for row in range(33, 64, 5):
        for col in range(33, 64, 6):
            draw.line((col, row, min(63, col + 2), row + 2), fill=(161, 196, 193), width=1)
    return image


# ─── 输出 ────────────────────────────────────────────────────────────────────


def emit_java(all_parts: tuple[ArmorPart, ...] | None = None) -> str:
    """输出 ArmorPartModel.CUBE_TABLES 用的 Java 字面量。"""
    all_parts = parts() if all_parts is None else all_parts
    chunks = []
    for part in all_parts:
        method = "".join(word.capitalize() for word in part.key.split("_"))
        method = method[0].lower() + method[1:]
        lines = [f"    private static List<ArmorCube> {method}() {{", "        return List.of("]
        body = []
        for cube in part.cubes:
            ox, oy, oz = cube.origin
            sx, sy, sz = cube.size
            u, v = cube.uv
            body.append(
                f"            new ArmorCube(Mount.{cube.mount}, "
                f"{ox}f, {oy}f, {oz}f, {sx}f, {sy}f, {sz}f, {u}, {v})"
            )
        lines.append(",\n".join(body))
        lines.extend(("        );", "    }"))
        chunks.append("\n".join(lines))
    return "\n\n".join(chunks)


def cube_digest(part: ArmorPart) -> str:
    """复刻 ArmorPartModelTest.cubeDigest 的 FNV-1a。"""
    import struct

    def fnv1a(hash_value: int, value: int) -> int:
        for _ in range(4):
            hash_value ^= value & 0xFF
            hash_value = (hash_value * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
            value >>= 8
        return hash_value

    def bits(value: float) -> int:
        return struct.unpack("<I", struct.pack("<f", value))[0]

    mounts = ["HEAD", "BODY", "LEFT_LEG", "RIGHT_LEG", "LEFT_FOOT", "RIGHT_FOOT"]
    digest = 0xCBF29CE484222325
    for cube in part.cubes:
        digest = fnv1a(digest, mounts.index(cube.mount))
        for value in (*cube.origin, *cube.size):
            digest = fnv1a(digest, bits(value))
        digest = fnv1a(digest, cube.uv[0])
        digest = fnv1a(digest, cube.uv[1])
    return f"{digest:016x}"


def generate(render_previews: bool = True, install: bool = False) -> dict[str, Path]:
    all_parts = parts()
    _assert_no_coplanar_faces(all_parts)
    _assert_uv_tiles(all_parts)
    _assert_mirror_symmetry(all_parts)
    _assert_no_isolated_cubes(all_parts)
    _assert_helmet_front_projection(all_parts)
    _assert_helmet_side_coverage(all_parts)
    return write_material_assets(
        MATERIAL,
        all_parts,
        make_texture(),
        LOCAL_MODELS,
        CLIENT_TEXTURE_ROOT if install else DRAFT_TEXTURE_ROOT,
        PREVIEW_ROOT,
        render_previews,
    )


def main() -> None:
    parser = argparse.ArgumentParser(description="生成灵布衫四件套 3D 程序化资产")
    parser.add_argument("--no-preview", action="store_true", help="跳过三视图渲染")
    parser.add_argument("--emit-java", action="store_true", help="输出 ArmorPartModel Java 代码")
    parser.add_argument("--self-test", action="store_true", help="运行门禁差分自证")
    parser.add_argument("--install", action="store_true", help="写入客户端正式资源目录")
    args = parser.parse_args()

    if args.self_test:
        raise SystemExit(GATES.self_test(build()))
    if args.emit_java:
        print(emit_java())
        return
    outputs = generate(render_previews=not args.no_preview, install=args.install)
    for key, path in outputs.items():
        print(f"[{key}] {path.relative_to(REPO)}")


if __name__ == "__main__":
    main()
