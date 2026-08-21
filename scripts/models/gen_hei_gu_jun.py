#!/usr/bin/env python3
"""生成黑骨菌 (HeiGuJun) Blockbench .bbmodel 与三视图预览 (Round 2 结构精进版)。

【世界观正典与设定依据】：
- 正典出处：《末法药材十七种》毒性五味之十五。
  “生于死人之骨缝。毒入经脉，三息内化骨为粉。此物炼丹师亦不敢动。”
- 视觉特征与 Round 2 拓扑优化：
  1. 风化发黑残骨基床 (part_decayed_bone)：
     - 底部为一节粗壮断裂、向侧向倾斜摆放的猛兽/修士发黑残骨 (断裂骨节、髓腔凹槽、骨裂深缝与风化碎骨)。
     - 彻底破除方盒感，增加骨质斜切断口与深黑色髓腔内嵌槽。
  2. 多向曲折柔性菌柄群 (part_stalks)：
     - 6 株自残骨裂缝中破骨钻出的细长扭曲菌柄 (高度 4.5~12.8 格)，菌柄具有更自然的柔韧弧度与根部聚拢。
  3. 八角平滑多面黑油菌伞 (part_caps)：
     - 采用【主伞面 + 45° 切角斜檐 + 顶端微凸死气斑】三重体素复合结构，呈现圆润而坚硬的黑油菌盖。
  4. 悬浮死气孢子 (part_spores)：
     - 周围微悬浮惨白死气孢子与黑色剧毒微粒。
"""

from __future__ import annotations

import base64
import io
import json
import math
import uuid
from dataclasses import dataclass
from pathlib import Path

import numpy as np
from PIL import Image

REPO = Path(__file__).resolve().parents[2]
LOCAL_MODELS = REPO / "local_models"
PREVIEW_DIR = REPO / "scripts" / "models"
TEXTURE_RES = 64
UUID_NAMESPACE = uuid.UUID("f912c443-6789-4b12-9832-1122aabb0006")


@dataclass(frozen=True)
class Cube:
    bone: str
    name: str
    origin: tuple[float, float, float]
    target: tuple[float, float, float]
    uv_preset: str
    rotation: tuple[float, float, float] = (0.0, 0.0, 0.0)
    rot_origin: tuple[float, float, float] = (0.0, 0.0, 0.0)


def stable_uuid(*parts: str) -> str:
    return str(uuid.uuid5(UUID_NAMESPACE, ":".join(parts)))


def make_directed_segment(
    bone: str,
    name: str,
    p1: tuple[float, float, float],
    p2: tuple[float, float, float],
    thickness: float = 0.55,
    uv_preset: str = "stalk_dark",
) -> Cube:
    """在空间两点 p1, p2 之间创建一节真实倾斜旋转的菌柄。"""
    x1, y1, z1 = p1
    x2, y2, z2 = p2
    v = np.array([x2 - x1, y2 - y1, z2 - z1], dtype=float)
    L = float(np.linalg.norm(v))
    if L < 1e-4:
        return Cube(
            bone,
            name,
            (x1 - thickness / 2, y1, z1 - thickness / 2),
            (x1 + thickness / 2, y1 + 0.1, z1 + thickness / 2),
            uv_preset,
        )

    u = v / L
    ux, uy, uz = u

    rx_rad = math.acos(max(-1.0, min(1.0, uy)))
    sin_rx = math.sin(rx_rad)
    if sin_rx > 1e-5:
        ry_rad = math.atan2(ux, uz)
    else:
        ry_rad = 0.0

    rx_deg = math.degrees(rx_rad)
    ry_deg = math.degrees(ry_rad)
    rz_deg = 0.0

    ht = thickness / 2.0
    orig = (x1 - ht, y1, z1 - ht)
    targ = (x1 + ht, y1 + L, z1 + ht)
    rot_origin = (x1, y1, z1)

    return Cube(
        bone,
        name,
        orig,
        targ,
        uv_preset,
        rotation=(rx_deg, ry_deg, rz_deg),
        rot_origin=rot_origin,
    )


def build_stalk_chain(
    bone: str,
    prefix: str,
    path_points: list[tuple[float, float, float]],
    thickness_start: float = 0.65,
    thickness_end: float = 0.45,
    uv_preset: str = "stalk_dark",
) -> list[Cube]:
    """生成一条连续曲折扭曲的黑菌柄。"""
    cubes: list[Cube] = []
    n = len(path_points)
    for i in range(n - 1):
        p1 = path_points[i]
        p2 = path_points[i + 1]
        progress = i / max(1, n - 2)
        t = thickness_start + (thickness_end - thickness_start) * progress
        cubes.append(
            make_directed_segment(bone, f"{prefix}_{i:02d}", p1, p2, t, uv_preset)
        )
    return cubes


def make_mushroom_cap(
    bone: str,
    prefix: str,
    center: tuple[float, float, float],
    size: tuple[float, float, float] = (2.4, 0.9, 2.4),
    tilt: tuple[float, float, float] = (0.0, 0.0, 0.0),
) -> list[Cube]:
    """生成多面切角、黑油光泽与惨白死气斑点的有机菌伞。"""
    cx, cy, cz = center
    w, h, d = size
    cubes = []

    # 1. 菌盖主伞身 (Cap Main Dome)
    cubes.append(
        Cube(
            bone,
            f"{prefix}_dome",
            (cx - w / 2, cy, cz - d / 2),
            (cx + w / 2, cy + h * 0.85, cz + d / 2),
            "cap_oily_black",
            rotation=tilt,
            rot_origin=center,
        )
    )

    # 2. 45° 交错圆润切角斜檐 (Cap Octagonal Rim)
    rw, rd = w * 0.88, d * 0.88
    cubes.append(
        Cube(
            bone,
            f"{prefix}_rim",
            (cx - rw / 2, cy - 0.05, cz - rd / 2),
            (cx + rw / 2, cy + h * 0.75, cz + rd / 2),
            "cap_oily_black",
            rotation=(tilt[0], tilt[1] + 45.0, tilt[2]),
            rot_origin=center,
        )
    )

    # 3. 菌盖顶端微凸死气斑 (Pale Rot Crown)
    pw, ph, pd = w * 0.48, h * 0.45, d * 0.48
    cubes.append(
        Cube(
            bone,
            f"{prefix}_rot_spot",
            (cx - pw / 2, cy + h * 0.65, cz - pd / 2),
            (cx + pw / 2, cy + h + ph, cz + pd / 2),
            "cap_pale_rot",
            rotation=tilt,
            rot_origin=center,
        )
    )

    # 4. 菌盖下缘阴影内褶 (Underside Gills AO)
    uw, uh, ud = w * 0.68, h * 0.35, d * 0.68
    cubes.append(
        Cube(
            bone,
            f"{prefix}_gills",
            (cx - uw / 2, cy - uh, cz - ud / 2),
            (cx + uw / 2, cy, cz + ud / 2),
            "bone_marrow",
            rotation=tilt,
            rot_origin=center,
        )
    )

    return cubes


def part_decayed_bone() -> list[Cube]:
    """部件1：风化发黑残骨基床 (Decayed Bone Bed)。

    形成断裂的猛兽粗骨节、纵向骨缝与骨髓深槽。
    """
    cubes: list[Cube] = []

    # 1. 残骨底层地台 (Y: 0.0 ~ 0.8)
    cubes.append(
        Cube(
            "bone_bed",
            "bone_ground_slab",
            (3.4, 0.0, 3.4),
            (12.6, 0.8, 12.6),
            "bone_marrow",
        )
    )

    # 2. 粗壮断裂残骨主干 (斜向摆放，宽 4.4, 长 8.8, 倾斜 10°)
    cubes.append(
        Cube(
            "bone_bed",
            "bone_shaft_main",
            (4.4, 0.4, 4.0),
            (11.6, 2.6, 7.8),
            "bone_decayed",
            rotation=(8.0, 25.0, -6.0),
            rot_origin=(8.0, 0.4, 6.0),
        )
    )

    # 3. 骨关节膨大端与劈裂骨片 (Epiphysis & Splinters)
    cubes.append(
        Cube(
            "bone_bed",
            "bone_joint_head",
            (2.8, 0.4, 3.2),
            (5.8, 3.4, 6.8),
            "bone_decayed",
            rotation=(-10.0, 30.0, 15.0),
            rot_origin=(4.3, 0.4, 5.0),
        )
    )
    # 右侧断裂碎骨尖
    cubes.append(
        Cube(
            "bone_bed",
            "bone_splinter_e",
            (10.2, 0.4, 6.8),
            (13.2, 2.8, 10.4),
            "bone_decayed",
            rotation=(12.0, -15.0, -18.0),
            rot_origin=(11.7, 0.4, 8.6),
        )
    )

    # 4. 骨缝凹槽与腐蚀空腔 (Fissures & Cavity)
    cubes.append(
        Cube(
            "bone_bed",
            "bone_fissure_trough",
            (5.6, 0.8, 6.2),
            (10.4, 2.2, 10.2),
            "bone_marrow",
            rotation=(6.0, 20.0, -4.0),
            rot_origin=(8.0, 0.8, 8.2),
        )
    )

    # 5. 散落风化碎骨粒
    cubes.append(
        Cube(
            "bone_bed",
            "bone_debris_s",
            (5.4, 0.2, 11.0),
            (8.2, 1.2, 12.8),
            "bone_decayed",
            rotation=(5.0, -10.0, 0.0),
            rot_origin=(6.8, 0.2, 11.9),
        )
    )

    return cubes


def part_stalks_and_caps() -> list[Cube]:
    """部件2与部件3：多向曲折菌柄与漆黑油亮菌盖群 (Stalks & Caps)。

    共 6 株大小高低错落的黑骨菌，自骨缝中曲折生长：
    - Mushroom 1 (主柱菌): 高度 12.8, 位于中心偏后
    - Mushroom 2 (高挑侧菌): 高度 10.8, 位于西北骨端
    - Mushroom 3 (中位侧菌): 高度 9.2, 位于东南骨缝
    - Mushroom 4 (探出斜菌): 高度 7.8, 位于正东斜伸
    - Mushroom 5 (低位幼菌): 高度 6.0, 位于西南骨裂
    - Mushroom 6 (微型初生菌): 高度 4.5, 位于正南浅缝
    """
    cubes: list[Cube] = []

    # ─────────────────────────────────────────────────────────────
    # 1. Mushroom 1 (中心主柱大黑菌, Height ~12.8)
    m1_path = [
        (7.8, 1.8, 7.8),
        (7.6, 4.8, 7.6),
        (8.1, 8.0, 7.4),
        (7.9, 11.2, 7.2),
    ]
    cubes.extend(
        build_stalk_chain(
            "mushrooms",
            "m1_stalk",
            m1_path,
            thickness_start=0.8,
            thickness_end=0.55,
        )
    )
    cubes.extend(
        make_mushroom_cap(
            "mushrooms",
            "m1_cap",
            center=(7.9, 11.8, 7.2),
            size=(2.6, 1.2, 2.6),
            tilt=(5.0, 25.0, -6.0),
        )
    )

    # ─────────────────────────────────────────────────────────────
    # 2. Mushroom 2 (西北高挑侧菌, Height ~10.8)
    m2_path = [
        (5.8, 2.2, 5.8),
        (5.2, 4.8, 5.4),
        (4.8, 7.6, 5.2),
        (4.6, 9.6, 5.0),
    ]
    cubes.extend(
        build_stalk_chain(
            "mushrooms",
            "m2_stalk",
            m2_path,
            thickness_start=0.65,
            thickness_end=0.45,
        )
    )
    cubes.extend(
        make_mushroom_cap(
            "mushrooms",
            "m2_cap",
            center=(4.5, 10.2, 4.9),
            size=(2.1, 1.0, 2.1),
            tilt=(-12.0, -35.0, 15.0),
        )
    )

    # ─────────────────────────────────────────────────────────────
    # 3. Mushroom 3 (东南中位曲折菌, Height ~9.2)
    m3_path = [
        (9.4, 1.6, 8.6),
        (10.2, 4.2, 8.8),
        (10.6, 6.8, 8.5),
        (10.4, 8.2, 8.2),
    ]
    cubes.extend(
        build_stalk_chain(
            "mushrooms",
            "m3_stalk",
            m3_path,
            thickness_start=0.6,
            thickness_end=0.42,
        )
    )
    cubes.extend(
        make_mushroom_cap(
            "mushrooms",
            "m3_cap",
            center=(10.3, 8.7, 8.1),
            size=(1.9, 0.95, 1.9),
            tilt=(10.0, -20.0, -12.0),
        )
    )

    # ─────────────────────────────────────────────────────────────
    # 4. Mushroom 4 (正东外探菌, Height ~7.8)
    m4_path = [
        (8.8, 1.6, 6.8),
        (9.8, 3.8, 6.4),
        (10.8, 5.8, 6.2),
        (11.2, 6.8, 6.0),
    ]
    cubes.extend(
        build_stalk_chain(
            "mushrooms",
            "m4_stalk",
            m4_path,
            thickness_start=0.55,
            thickness_end=0.4,
        )
    )
    cubes.extend(
        make_mushroom_cap(
            "mushrooms",
            "m4_cap",
            center=(11.3, 7.3, 5.9),
            size=(1.7, 0.85, 1.7),
            tilt=(-8.0, 45.0, -18.0),
        )
    )

    # ─────────────────────────────────────────────────────────────
    # 5. Mushroom 5 (西南骨裂幼菌, Height ~6.2)
    m5_path = [
        (6.6, 1.4, 8.8),
        (6.0, 3.2, 9.4),
        (5.6, 4.8, 9.8),
    ]
    cubes.extend(
        build_stalk_chain(
            "mushrooms",
            "m5_stalk",
            m5_path,
            thickness_start=0.5,
            thickness_end=0.38,
        )
    )
    cubes.extend(
        make_mushroom_cap(
            "mushrooms",
            "m5_cap",
            center=(5.5, 5.3, 9.9),
            size=(1.5, 0.8, 1.5),
            tilt=(18.0, 20.0, 12.0),
        )
    )

    # ─────────────────────────────────────────────────────────────
    # 6. Mushroom 6 (微型初生芽菌, Height ~4.6)
    m6_path = [
        (8.2, 1.4, 9.8),
        (8.4, 2.8, 10.4),
        (8.5, 3.8, 10.8),
    ]
    cubes.extend(
        build_stalk_chain(
            "mushrooms",
            "m6_stalk",
            m6_path,
            thickness_start=0.45,
            thickness_end=0.35,
        )
    )
    cubes.extend(
        make_mushroom_cap(
            "mushrooms",
            "m6_cap",
            center=(8.5, 4.2, 10.9),
            size=(1.3, 0.7, 1.3),
            tilt=(22.0, 0.0, 0.0),
        )
    )

    return cubes


def part_spores() -> list[Cube]:
    """部件4：悬浮死气孢子与微粒 (Ambient Spores)。"""
    cubes: list[Cube] = []

    spores = [
        ("spore_1", (5.8, 9.2, 7.8), (0.4, 0.4, 0.4), (15.0, 35.0, 20.0)),
        ("spore_2", (9.6, 10.6, 6.2), (0.35, 0.35, 0.35), (-20.0, 45.0, -15.0)),
        ("spore_3", (6.8, 12.8, 5.8), (0.35, 0.35, 0.35), (25.0, -20.0, 10.0)),
        ("spore_4", (11.0, 8.4, 9.4), (0.3, 0.3, 0.3), (10.0, -35.0, -25.0)),
    ]
    for name, (sx, sy, sz), (sw, sh, sd), srot in spores:
        cubes.append(
            Cube(
                "spores",
                name,
                (sx - sw / 2, sy, sz - sd / 2),
                (sx + sw / 2, sy + sh, sz + sd / 2),
                "spore_toxic",
                rotation=srot,
                rot_origin=(sx, sy, sz),
            )
        )

    return cubes


def all_cubes() -> list[Cube]:
    return part_decayed_bone() + part_stalks_and_caps() + part_spores()


# ─────────────────────────────────────────────────────────────────────────────
# 材质调色板 (Palettes: 4~6 色，绝无纯黑纯白)
# ─────────────────────────────────────────────────────────────────────────────

# 1. 风化发黑残骨 (Decayed Bone)
BONE_PALETTE = {
    "light":  (196, 190, 174),  # #C4BEAE - 风化骨质向光面
    "mid":    (138, 130, 122),  # #8A827A - 骨质主体 (60% 干净面)
    "rot":    (82, 74, 76),     # #524A4C - 发黑侵蚀过渡面
    "dark":   (44, 38, 44),     # #2C262C - 严重炭化发黑面
    "shadow": (22, 18, 24),     # #161218 - 骨腔深部 AO
}

# 2. 漆黑油亮菌盖 (Oily Black Cap)
CAP_PALETTE = {
    "high":   (124, 116, 138),  # #7C748A - 黑油冷灰紫反光棱
    "light":  (74, 66, 82),     # #4A4252 - 向光深油色
    "mid":    (38, 32, 44),     # #26202C - 漆黑菌伞主体 (65% 干净面)
    "dark":   (20, 16, 26),     # #14101A - 菌伞背光深暗
    "shadow": (12, 10, 16),     # #0C0A10 - 菌褶接触 AO
}

# 3. 顶端惨白死气斑 (Pale Rot Spots)
PALE_ROT_PALETTE = {
    "pale_high": (224, 218, 206),  # #E0DACE - 惨白死气中心
    "pale_mid":  (168, 160, 150),  # #A8A096 - 死气光晕
    "pale_edge": (96, 88, 86),     # #605856 - 腐蚀坏死边缘
}

# 4. 细长扭曲深灰菌柄 (Dark Stalk)
STALK_PALETTE = {
    "light":  (78, 70, 84),     # #4E4654 - 菌柄向光侧
    "mid":    (52, 46, 58),     # #342E3A - 菌柄主体
    "dark":   (30, 26, 36),     # #1E1A24 - 菌柄背光与根部
}


def create_texture() -> Image.Image:
    """生成 64x64 UV Atlas 高清黑骨菌贴图。"""
    img = Image.new("RGBA", (TEXTURE_RES, TEXTURE_RES), (0, 0, 0, 0))

    # ─────────────────────────────────────────────────────────────
    # 1. 风化发黑残骨面 (bone_decayed: 0,0~32,24)
    for x in range(32):
        for y in range(24):
            u = x
            v = y

            decay_grad = (u * 0.8 + v * 1.2) / 45.0

            if decay_grad < 0.25:
                col = BONE_PALETTE["light"]
            elif decay_grad < 0.55:
                col = BONE_PALETTE["mid"]
            elif decay_grad < 0.80:
                col = BONE_PALETTE["rot"]
            else:
                col = BONE_PALETTE["dark"]

            # 微小骨裂缝隙
            if (u == 14 and 4 <= v <= 12) or (v == 16 and 8 <= u <= 20):
                col = BONE_PALETTE["shadow"]

            img.putpixel((x, y), (*col, 255))

    # ─────────────────────────────────────────────────────────────
    # 2. 骨髓深处腐黑面 (bone_marrow: 0,24~32,40)
    for x in range(32):
        for y in range(24, 40):
            u = x
            v = y - 24
            edge_dist = min(u, 31 - u, v, 15 - v)
            if edge_dist <= 1:
                col = BONE_PALETTE["shadow"]
            elif v <= 6:
                col = BONE_PALETTE["dark"]
            else:
                col = BONE_PALETTE["rot"]
            img.putpixel((x, y), (*col, 255))

    # ─────────────────────────────────────────────────────────────
    # 3. 漆黑油亮菌盖主面 (cap_oily_black: 32,0~64,24)
    for x in range(32, 64):
        for y in range(24):
            u = x - 32
            v = y

            dist_to_top = math.sqrt((u - 16) ** 2 + v ** 2)

            if v <= 2 and 6 <= u <= 25:
                col = CAP_PALETTE["high"]
            elif dist_to_top <= 8:
                col = CAP_PALETTE["light"]
            elif v <= 18:
                col = CAP_PALETTE["mid"]
            else:
                col = CAP_PALETTE["dark"]

            if v >= 22:
                col = CAP_PALETTE["shadow"]

            img.putpixel((x, y), (*col, 255))

    # ─────────────────────────────────────────────────────────────
    # 4. 顶端惨白死气斑点 (cap_pale_rot: 32,24~48,40)
    for x in range(32, 48):
        for y in range(24, 40):
            u = x - 32
            v = y - 24
            center_d = math.sqrt((u - 8) ** 2 + (v - 8) ** 2)

            if center_d <= 2.2:
                col = PALE_ROT_PALETTE["pale_high"]
            elif center_d <= 5.0:
                col = PALE_ROT_PALETTE["pale_mid"]
            elif center_d <= 7.0:
                col = PALE_ROT_PALETTE["pale_edge"]
            else:
                col = CAP_PALETTE["dark"]

            img.putpixel((x, y), (*col, 255))

    # ─────────────────────────────────────────────────────────────
    # 5. 细长扭曲深灰菌柄 (stalk_dark: 48,24~64,48)
    for x in range(48, 64):
        for y in range(24, 48):
            u = x - 48
            v = y - 24

            if u <= 4:
                col = STALK_PALETTE["light"]
            elif u <= 11:
                col = STALK_PALETTE["mid"]
            else:
                col = STALK_PALETTE["dark"]

            if v <= 1 or v >= 22:
                col = CAP_PALETTE["shadow"]

            img.putpixel((x, y), (*col, 255))

    # ─────────────────────────────────────────────────────────────
    # 6. 悬浮死气微粒 (spore_toxic: 0,40~32,64)
    for x in range(32):
        for y in range(40, 64):
            u = x
            v = y - 40
            cd = math.sqrt((u - 16) ** 2 + (v - 12) ** 2)
            if cd <= 3.5:
                col = PALE_ROT_PALETTE["pale_high"]
            elif cd <= 7.5:
                col = PALE_ROT_PALETTE["pale_mid"]
            else:
                col = CAP_PALETTE["dark"]
            img.putpixel((x, y), (*col, 255))

    return img


def build_bbmodel_dict() -> dict:
    cubes = all_cubes()
    tex_img = create_texture()
    buf = io.BytesIO()
    tex_img.save(buf, format="PNG")
    tex_base64 = base64.b64encode(buf.getvalue()).decode("ascii")

    uv_presets = {
        "bone_decayed": [0, 0, 32, 24],
        "bone_marrow": [0, 24, 32, 40],
        "cap_oily_black": [32, 0, 64, 24],
        "cap_pale_rot": [32, 24, 48, 40],
        "stalk_dark": [48, 24, 64, 48],
        "spore_toxic": [0, 40, 32, 64],
    }

    elements = []
    outliner_bones: dict[str, list[str]] = {}

    for c in cubes:
        elem_uuid = stable_uuid("elem", c.name)
        uv = uv_presets.get(c.uv_preset, [0, 0, 16, 16])

        faces = {}
        for fname in ("north", "east", "south", "west", "up", "down"):
            faces[fname] = {
                "uv": uv,
                "texture": 0,
            }

        elem = {
            "name": c.name,
            "box_uv": False,
            "type": "cube",
            "from": [round(c.origin[0], 2), round(c.origin[1], 2), round(c.origin[2], 2)],
            "to": [round(c.target[0], 2), round(c.target[1], 2), round(c.target[2], 2)],
            "faces": faces,
            "uuid": elem_uuid,
        }
        if any(c.rotation):
            elem["rotation"] = [round(r, 2) for r in c.rotation]
            elem["origin"] = [round(o, 2) for o in c.rot_origin]

        elements.append(elem)

        if c.bone not in outliner_bones:
            outliner_bones[c.bone] = []
        outliner_bones[c.bone].append(elem_uuid)

    outliner = []
    for bname, children in outliner_bones.items():
        outliner.append(
            {
                "name": bname,
                "origin": [8, 0, 8],
                "color": 0,
                "uuid": stable_uuid("bone", bname),
                "export": True,
                "isOpen": True,
                "locked": False,
                "visibility": True,
                "children": children,
            }
        )

    return {
        "meta": {
            "format_version": "4.5",
            "model_format": "free",
            "box_uv": False,
        },
        "name": "HeiGuJun",
        "model_identifier": "hei_gu_jun",
        "resolution": {"width": TEXTURE_RES, "height": TEXTURE_RES},
        "elements": elements,
        "outliner": outliner,
        "textures": [
            {
                "path": "hei_gu_jun.png",
                "name": "hei_gu_jun",
                "folder": "block",
                "namespace": "bong",
                "id": "0",
                "particle": False,
                "render_mode": "default",
                "saved": True,
                "uuid": stable_uuid("texture", "main"),
                "source": f"data:image/png;base64,{tex_base64}",
            }
        ],
    }


def main():
    LOCAL_MODELS.mkdir(parents=True, exist_ok=True)
    PREVIEW_DIR.mkdir(parents=True, exist_ok=True)

    bb_dict = build_bbmodel_dict()
    out_bbmodel = LOCAL_MODELS / "HeiGuJun.bbmodel"
    out_bbmodel.write_text(json.dumps(bb_dict, indent=2))
    print(f"Generated BBModel: {out_bbmodel} (Elements: {len(bb_dict['elements'])})")

    # 保存 UV 贴图预览
    tex = create_texture()
    tex.save(PREVIEW_DIR / "hei_gu_jun_texture.png")
    print(f"Saved texture atlas: {PREVIEW_DIR / 'hei_gu_jun_texture.png'}")


if __name__ == "__main__":
    main()
