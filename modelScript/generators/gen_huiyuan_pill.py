#!/usr/bin/env python3
"""回元丹（HuiyuanPill）Blockbench .bbmodel 生成器 - 追求极致还原版。

物品来源：
  `client/src/main/resources/assets/bong-client/textures/gui/items/huiyuan_pill.png`
  `server/assets/items/pills.toml` (id = "huiyuan_pill", name = "回元丹")

参考原画设计解析：
  1. 白瓷丹罐（Porcelain Jar）：
     - 极具古典韵味的扁圆白瓷钵，圆鼓腹、宽大敞口。
     - 采用正交多阶梯体素（Voxel Stepped Circle）逼近饱满正圆，杜绝突出的星角与锯齿感。
     - 结构细节：
       - 底足圈：内收、沉稳
       - 下腹弧：优雅向上外展
       - 中腹最大鼓腹：浑圆饱满、弧线柔和
       - 束颈凹圈：形成收束颈部
       - 罐口厚卷唇：微微外侈的开敞大口，带温暖的陶褐烧结边
       - 罐内深腔：中空深底，托住盛满的丹药
  2. 满盛回元丹丸（Pills）：
     - 罐内堆满温润饱满的赤褐色/赭红回元丹丸。
     - 顶峰主丹居中高挺，周围环绕 4~5 颗丹丸错落簇拥，呈现药丸颗粒饱满、圆润堆叠的视觉冲击。
     - 每颗丹丸采用三轴平滑体素圆球（核心体 + 6面圆弧贴片），真正圆润无死角。
  3. 斜靠瓷盖（Porcelain Lid）：
     - 优雅斜靠在罐身右下侧地面，紧贴罐壁。
     - 结构：圆形薄盘盖沿 + 微拱穹面 + 宝珠形球头盖钮。
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "core"))
from bbmodel_maker.rig.rigkit import Rig
from bbmodel_maker import workspace

_WS = workspace.Workspace.discover(start=Path(__file__))
OUT_DIR = _WS.models
RENDER_OUT = _WS.out

PX = 16.0

# ── 材质色板 ──────────────────────────────────────────────────────────
# 严格提炼自 huiyuan_pill.png 原版像素颜色：
MATS = {
    "porcelain":       (240, 236, 228),  # 瓷罐主身：柔润象牙白瓷
    "porcelain_side":  (220, 214, 204),  # 瓷罐侧向过渡面（环境散射）
    "porcelain_dark":  (182, 172, 158),  # 瓷罐底足与束颈暗部
    "porcelain_rim":   (200, 176, 140),  # 罐口与盖沿的陶黄釉边
    "pill_base":       (128, 54, 36),    # 丹丸主体赤赭色
    "pill_lit":        (178, 92, 66),    # 丹丸向光面（顶部）
    "pill_shadow":     (76, 28, 18),     # 丹丸阴影面（底部与夹缝）
    "jar_inner":       (54, 46, 40),     # 罐内深底衬托
}


def add_voxel_circle(rig: Rig, bone: str, prefix: str,
                     y0: float, y1: float, r: float,
                     mat_main: str = "porcelain", mat_side: str = "porcelain_side",
                     cx: float = 0.0, cz: float = 0.0):
    """使用 3 块轴对齐正交长方体复合逼近极致正圆（12边形平滑圆轮廓）。
    没有任何倾角旋转，在 MC 体素中呈现最浑厚柔和的圆柱曲面。
    """
    # 尺寸比例：
    # 块1：横向最宽，前后缩进 0.28*r
    # 块2：纵向最长，左右缩进 0.28*r
    # 块3：正方形过渡块，各缩进 0.12*r
    r_wide = r
    r_narrow = r * 0.72
    r_mid = r * 0.88

    # 块1（横向宽）
    rig.cube(bone, f"{prefix}_c1",
             (cx - r_wide, y0, cz - r_narrow),
             (cx + r_wide, y1, cz + r_narrow),
             mat=mat_main)
    # 块2（前后长）
    rig.cube(bone, f"{prefix}_c2",
             (cx - r_narrow, y0, cz - r_wide),
             (cx + r_narrow, y1, cz + r_wide),
             mat=mat_main)
    # 块3（四角圆滑过渡）
    rig.cube(bone, f"{prefix}_c3",
             (cx - r_mid, y0, cz - r_mid),
             (cx + r_mid, y1, cz + r_mid),
             mat=mat_side)


def add_voxel_hollow_ring(rig: Rig, bone: str, prefix: str,
                          y0: float, y1: float, r_out: float, r_in: float,
                          mat: str = "porcelain_rim"):
    """构建中空大圆环（罐口卷唇与束颈）。
    由 8 块轴对齐环形厚块拼合而成，中间完全中空通透，外轮廓平滑圆润。
    """
    # 外径与内径的关键分段
    # 外圆四正端点
    w_out = r_out * 0.72
    w_in = r_in * 0.72

    # 前后唇沿主块
    rig.cube(bone, f"{prefix}_f",
             (-w_out, y0, r_in), (w_out, y1, r_out),
             mat=mat)
    rig.cube(bone, f"{prefix}_b",
             (-w_out, y0, -r_out), (w_out, y1, -r_in),
             mat=mat)
    # 左右唇沿主块
    rig.cube(bone, f"{prefix}_l",
             (-r_out, y0, -w_out), (-r_in, y1, w_out),
             mat=mat)
    rig.cube(bone, f"{prefix}_r",
             (r_in, y0, -w_out), (r_out, y1, w_out),
             mat=mat)

    # 四角圆滑封边块（保证外缘圆润，内圈不阻挡）
    c_out = r_out * 0.88
    c_in = r_in * 0.75
    for sx, sz, tag in [(-1, 1, "fl"), (1, 1, "fr"), (-1, -1, "bl"), (1, -1, "br")]:
        x0, x1 = sorted([sx * c_in, sx * c_out])
        z0, z1 = sorted([sz * c_in, sz * c_out])
        rig.cube(bone, f"{prefix}_{tag}",
                 (x0, y0, z0), (x1, y1, z1),
                 mat=mat)


def add_smooth_pill(rig: Rig, bone: str, prefix: str,
                    cx: float, cy: float, cz: float, r: float):
    """构建一颗精致饱满的体素圆球丹药。
    由核心体 + 顶冠向光面 + 底托暗面 + 四向微圆弧贴片组成。
    """
    # 核心块
    rc = r * 0.78
    rig.cube(bone, f"{prefix}_core",
             (cx - rc, cy - rc, cz - rc),
             (cx + rc, cy + rc, cz + rc),
             mat="pill_base")

    # 顶冠向光面（微凸亮色）
    rt = r * 0.58
    rig.cube(bone, f"{prefix}_top",
             (cx - rt, cy + rc, cz - rt),
             (cx + rt, cy + r, cz + rt),
             mat="pill_lit")

    # 底托阴影面（深沉底色）
    rig.cube(bone, f"{prefix}_bot",
             (cx - rt, cy - r, cz - rt),
             (cx + rt, cy - rc, cz + rt),
             mat="pill_shadow")

    # 前后侧面微凸薄片（形成球体圆弧）
    rig.cube(bone, f"{prefix}_f",
             (cx - rt, cy - rt, cz + rc),
             (cx + rt, cy + rt, cz + r),
             mat="pill_base")
    rig.cube(bone, f"{prefix}_b",
             (cx - rt, cy - rt, cz - r),
             (cx + rt, cy + rt, cz - rc),
             mat="pill_shadow")
    rig.cube(bone, f"{prefix}_l",
             (cx - r, cy - rt, cz - rt),
             (cx - rc, cy + rt, cz + rt),
             mat="pill_base")
    rig.cube(bone, f"{prefix}_r",
             (cx + rc, cy - rt, cz - rt),
             (cx + r, cy + rt, cz + rt),
             mat="pill_base")


def part_jar(rig: Rig) -> None:
    """圆润白瓷罐身：
    圈足 -> 下腹微弧渐起 -> 浑圆大鼓腹 -> 上腹平滑收束 -> 束颈环 -> 外翻圆润陶黄卷沿。
    """
    rig.bone("jar", (0.0, 0.0, 0.0))

    # 1. 圈足 (y: 0.0 ~ 0.8, r=3.3)
    add_voxel_circle(rig, "jar", "foot", 0.0, 0.8, r=3.3,
                     mat_main="porcelain_dark", mat_side="porcelain_dark")

    # 2. 腹部下段渐起 (y: 0.8 ~ 2.2, r=4.5)
    add_voxel_circle(rig, "jar", "belly_low", 0.8, 2.2, r=4.5,
                     mat_main="porcelain", mat_side="porcelain_side")

    # 3. 腹部中段最大鼓腹 (y: 2.2 ~ 4.8, r=5.3) - 饱满浑厚的瓷罐肚子
    add_voxel_circle(rig, "jar", "belly_mid", 2.2, 4.8, r=5.3,
                     mat_main="porcelain", mat_side="porcelain_side")

    # 4. 腹部上段圆滑收束 (y: 4.8 ~ 6.0, r=4.5)
    add_voxel_circle(rig, "jar", "belly_up", 4.8, 6.0, r=4.5,
                     mat_main="porcelain", mat_side="porcelain_side")

    # 5. 罐内盛丹暗底 (y: 4.8 ~ 5.4, 深色底托)
    rig.cube("jar", "inner_bed",
             (-3.2, 4.8, -3.2), (3.2, 5.4, 3.2),
             mat="jar_inner")

    # 6. 束颈凹环 (y: 6.0 ~ 6.7, r_out=3.9, r_in=3.0)
    add_voxel_hollow_ring(rig, "jar", "neck", 6.0, 6.7,
                          r_out=3.9, r_in=3.0,
                          mat="porcelain_dark")

    # 7. 罐口卷唇厚沿 (y: 6.7 ~ 7.6, r_out=4.5, r_in=3.2) - 陶黄色外侈卷沿
    add_voxel_hollow_ring(rig, "jar", "rim", 6.7, 7.6,
                          r_out=4.5, r_in=3.2,
                          mat="porcelain_rim")


def part_pills(rig: Rig) -> None:
    """罐内满盛的回元丹丸。
    罐口通径内半径约 3.2，可容纳多颗直径约 2.6~2.9 的饱满药丸。
    中央高耸顶峰主丹 + 四周围绕 4 颗紧密簇拥丹丸，金字塔状层叠，视觉极度充实！
    """
    rig.bone("pills", (0.0, 6.6, 0.0))

    r_main = 1.45
    r_sub = 1.30

    # 1. 顶峰核心主丹（位于中心微偏前，高出罐口，向光面饱满）
    add_smooth_pill(rig, "pills", "p_main", 0.0, 7.9, 0.2, r_main)

    # 2. 前排偏左丹丸（探出罐口边缘）
    add_smooth_pill(rig, "pills", "p_fl", -1.25, 7.0, 1.45, r_sub)

    # 3. 前排偏右丹丸
    add_smooth_pill(rig, "pills", "p_fr", 1.35, 7.0, 1.30, r_sub)

    # 4. 后排偏左丹丸
    add_smooth_pill(rig, "pills", "p_bl", -1.35, 7.1, -1.25, r_sub)

    # 5. 后排偏右丹丸
    add_smooth_pill(rig, "pills", "p_br", 1.25, 7.2, -1.35, r_sub)


def part_lid(rig: Rig) -> None:
    """斜靠在瓷罐右侧地面的瓷盖。
    圆盖身 + 圆拱穹顶 + 宝珠形盖纽。
    """
    rig.bone("lid", (5.2, 1.4, 0.8))

    org = (5.2, 0.4, 0.8)
    rot = (8.0, -10.0, -42.0)

    # 1. 盖沿大圆盘（直径 5.4，厚 0.6，陶黄边）
    r_rim = 2.7
    r_narrow = r_rim * 0.72
    r_mid = r_rim * 0.88
    rig.cube("lid", "lid_rim_1",
             (5.2 - r_rim, 0.0, 0.8 - r_narrow),
             (5.2 + r_rim, 0.6, 0.8 + r_narrow),
             rot=rot, org=org, mat="porcelain_rim")
    rig.cube("lid", "lid_rim_2",
             (5.2 - r_narrow, 0.0, 0.8 - r_rim),
             (5.2 + r_narrow, 0.6, 0.8 + r_rim),
             rot=rot, org=org, mat="porcelain_rim")
    rig.cube("lid", "lid_rim_3",
             (5.2 - r_mid, 0.0, 0.8 - r_mid),
             (5.2 + r_mid, 0.6, 0.8 + r_mid),
             rot=rot, org=org, mat="porcelain_rim")

    # 2. 盖顶圆拱穹面（直径 4.0，厚 0.7，米白瓷）
    r_dome = 2.0
    rd_narrow = r_dome * 0.72
    rd_mid = r_dome * 0.88
    rig.cube("lid", "lid_dome_1",
             (5.2 - r_dome, 0.6, 0.8 - rd_narrow),
             (5.2 + r_dome, 1.3, 0.8 + rd_narrow),
             rot=rot, org=org, mat="porcelain")
    rig.cube("lid", "lid_dome_2",
             (5.2 - rd_narrow, 0.6, 0.8 - r_dome),
             (5.2 + rd_narrow, 1.3, 0.8 + r_dome),
             rot=rot, org=org, mat="porcelain")
    rig.cube("lid", "lid_dome_3",
             (5.2 - rd_mid, 0.6, 0.8 - rd_mid),
             (5.2 + rd_mid, 1.3, 0.8 + rd_mid),
             rot=rot, org=org, mat="porcelain_side")

    # 3. 盖纽立颈
    rig.cube("lid", "lid_knob_neck",
             (5.2 - 0.45, 1.3, 0.8 - 0.45),
             (5.2 + 0.45, 1.8, 0.8 + 0.45),
             rot=rot, org=org, mat="porcelain_dark")

    # 4. 宝珠圆纽（微型体素圆球）
    rk = 0.8
    rk_n = rk * 0.72
    rig.cube("lid", "lid_knob_1",
             (5.2 - rk, 1.8, 0.8 - rk_n),
             (5.2 + rk, 2.9, 0.8 + rk_n),
             rot=rot, org=org, mat="porcelain")
    rig.cube("lid", "lid_knob_2",
             (5.2 - rk_n, 1.8, 0.8 - rk),
             (5.2 + rk_n, 2.9, 0.8 + rk),
             rot=rot, org=org, mat="porcelain")


def build_rig() -> Rig:
    rig = Rig(MATS, swatch=8)
    part_jar(rig)
    part_pills(rig)
    part_lid(rig)
    return rig


def main():
    parser = argparse.ArgumentParser(description="回元丹 bbmodel 生成器")
    parser.add_argument("--part", choices=["jar", "pills", "lid"], help="仅预览单个部件")
    parser.add_argument("--out", type=Path, default=OUT_DIR / "HuiyuanPill.bbmodel", help="输出路径")
    args = parser.parse_args()

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    RENDER_OUT.mkdir(parents=True, exist_ok=True)

    rig = Rig(MATS, swatch=8)
    if args.part == "jar":
        part_jar(rig)
    elif args.part == "pills":
        part_pills(rig)
    elif args.part == "lid":
        part_lid(rig)
    else:
        part_jar(rig)
        part_pills(rig)
        part_lid(rig)

    bb_json = rig.bbmodel("HuiyuanPill")
    args.out.write_text(json.dumps(bb_json, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"✓ 写入模型: {args.out}")


if __name__ == "__main__":
    main()
