#!/usr/bin/env python3
"""背篓（back_basket）Blockbench .bbmodel 生成器。

worldview.md §十三 L558「背篓负于背」的背负式主包。与既有胸挂/背挂**破草包**
（GrassPouchBack/Front.bbmodel，6×6×4px 小件）形制区分：本件是竹架 + 草编身的
方箱背篓，带兽皮盖、骨扣、左右不对称背带。

视觉语言（对齐生图参考，末法残土穷酸调）：
  竹架 bamboo   —— 4 立柱 + 上下横箍，pale 死竹带暗绿
  草编身 weave  —— 4 壁 + 底，横向粗编带
  兽皮盖 hide   —— 旧血锈色硬皮，压得略歪（不是布，不垂坠）
  草绳 cord     —— 捆盖两道 + 骨扣拴绳
  骨扣 bone     —— 泛黄骨栓代替搭扣
  背带 strap    —— 左原生草编 / 右灰布替换（左右不对称是刻意的）
  补丁 patch    —— 灰布方补，粗针脚缝在低处破洞上

**不对称是设计意图**：右背带是后补的灰布、破口重编在左上角、盖压歪。所以
`check()` 只查左右对称件（竹架/编身/底箍），刻意的不对称件走白名单排除。

坐标：建模用居中空间（x/z ∈ [-8,8]，地面 y=0），写盘前平移进 MC 方块空间
0..16。这样 rigkit.mirror_violations() 的 x=0 中轴判据可直接用。

用法:
    python3 modelScript/generators/gen_back_basket.py            # 生成 + 自检
    python3 modelScript/generators/gen_back_basket.py --check    # 只自检
    python3 modelScript/generators/gen_back_basket.py --part lid # 只出单件预览
    python3 modelScript/generators/gen_back_basket.py --list
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "core"))
from rigkit import Rig, element_bounds  # noqa: E402

REPO = Path(__file__).resolve().parents[2]
OUT_DIR = Path(__file__).resolve().parents[1] / "models" / "containers"

PX = 16.0

# ── 材质（swatch=8，64×64 贴图最多放 64 种，这里 7 种）──────────────
MATS = {
    "bamboo": (150, 152, 126),   # 死竹：灰绿，无生气
    "weave":  (150, 134, 100),   # 草编：枯草黄压灰
    "seam":   (116, 102, 74),    # 编缝暗带
    "hide":   (110, 68, 52),     # 兽皮：旧血锈
    "cord":   (138, 126, 96),    # 草绳
    "bone":   (206, 198, 170),   # 骨栓：泛黄骨白
    "patch":  (126, 126, 122),   # 灰布补丁
    "stitch": (74, 58, 44),      # 针脚：深褐，必须比 cord 暗才读得出「缝」
}

# ── 主体尺寸（px，居中空间）─────────────────────────────────────────
HALF_W = 5.6          # 宽 11.2px ≈ 0.70 格
HALF_D = 3.4          # 深 6.8px ≈ 0.43 格（背篓扁，贴背）
H_BODY = 12.0         # 编身顶
FLOOR = 1.2           # 底板厚
WALL = 1.0            # 壁厚
POST = 1.1            # 竹立柱边长

# 刻意不对称件：不参与左右镜像自检
ASYM = ("lid", "cord", "strap_l", "strap_r", "patch", "rework")


def part_frame(rig: Rig) -> None:
    """竹架：4 立柱 + 上下横箍。立柱略外凸于编身，读作承重骨架。"""
    rig.bone("frame", (0.0, 0.0, 0.0))
    px, pz = HALF_W - 0.15, HALF_D - 0.15
    for sx in (-1, 1):
        for sz in (-1, 1):
            cx, cz = sx * px, sz * pz
            # 命名带 _l_/_r_ 中缀，供 rigkit.mirror_violations 左右配对
            side = "r" if sx > 0 else "l"
            face = "f" if sz > 0 else "b"
            # 立柱用 shaft：端点精确落在地面与篓口，不手写 rotation
            # 柱头止于篓口：原先到 H_BODY+1.4 会从皮盖（12.0..12.9）里穿出来，
            # 盖面四角露出四块柱头 = 穿模。竹架到口，皮盖压在其上，相切不互穿。
            rig.shaft(
                "frame", f"post_{side}_{face}",
                (cx, 0.0, cz), (cx, H_BODY, cz),
                POST / 2, POST / 2, mat="bamboo",
            )
    # 上下横箍（前后各一道，绕 x 走）
    for y in (1.8, H_BODY - 0.8):
        for sz in (-1, 1):
            rig.shaft(
                "frame", f"hoop_{'f' if sz > 0 else 'b'}_{int(y)}",
                (-HALF_W - 0.1, y, sz * (HALF_D + 0.05)),
                (HALF_W + 0.1, y, sz * (HALF_D + 0.05)),
                0.45, 0.45, mat="bamboo",
            )
    # 底部承重横箍（左右各一道，绕 z 走，读作可着地）
    for sx in (-1, 1):
        rig.shaft(
            "frame", f"skid_{'r' if sx > 0 else 'l'}",
            (sx * (HALF_W - 0.4), 0.5, -HALF_D - 0.1),
            (sx * (HALF_W - 0.4), 0.5, HALF_D + 0.1),
            0.5, 0.5, mat="bamboo",
        )


def part_basket(rig: Rig) -> None:
    """草编身：底 + 4 壁 + 横向编带。开顶（盖单独一件）。"""
    rig.bone("basket", (0.0, 0.0, 0.0))
    rig.cube("basket", "floor",
             (-HALF_W, 0.0, -HALF_D), (HALF_W, FLOOR, HALF_D), mat="weave")
    rig.cube("basket", "wall_front",
             (-HALF_W, FLOOR, HALF_D - WALL), (HALF_W, H_BODY, HALF_D), mat="weave")
    rig.cube("basket", "wall_back",
             (-HALF_W, FLOOR, -HALF_D), (HALF_W, H_BODY, -HALF_D + WALL), mat="weave")
    rig.cube("basket", "wall_side_l",
             (-HALF_W, FLOOR, -HALF_D), (-HALF_W + WALL, H_BODY, HALF_D), mat="weave")
    rig.cube("basket", "wall_side_r",
             (HALF_W - WALL, FLOOR, -HALF_D), (HALF_W, H_BODY, HALF_D), mat="weave")
    # 横向编带：满壁一道压一道。参考图的核心质感就是「粗编」——
    # 三道浮带中间大片平面会读作木板箱，必须密到明暗交替连续。
    # 交替 seam/weave 两色、逐道微调深度，才有编条压叠的起伏。
    y = FLOOR + 0.35
    i = 0
    while y < H_BODY - 0.9:
        mat = "seam" if i % 2 == 0 else "weave"
        bulge = 0.34 if i % 2 == 0 else 0.20
        for sz in (-1, 1):
            z0 = sz * HALF_D
            z1 = z0 + sz * bulge
            rig.cube("basket", f"band_{'f' if sz > 0 else 'b'}_{i}",
                     (-HALF_W + 0.25, y, min(z0, z1)),
                     (HALF_W - 0.25, y + 0.78, max(z0, z1)), mat=mat)
        # 侧壁同步铺，转角处编带才不断
        for sx in (-1, 1):
            x0 = sx * HALF_W
            x1 = x0 + sx * bulge
            rig.cube("basket", f"bandside_{'r' if sx > 0 else 'l'}_{i}",
                     (min(x0, x1), y, -HALF_D + 0.25),
                     (max(x0, x1), y + 0.78, HALF_D - 0.25), mat=mat)
        y += 1.06
        i += 1


def part_rework(rig: Rig) -> None:
    """破口重编：左上角一块用错色粗绳补过，编距对不上原件。"""
    rig.bone("rework", (0.0, H_BODY, 0.0))
    for i in range(3):
        y = H_BODY - 2.6 + i * 1.0
        rig.cube("rework", f"rework_{i}",
                 (-HALF_W - 0.2, y, HALF_D - WALL - 0.1),
                 (-HALF_W + 3.0 - i * 0.4, y + 0.7, HALF_D + 0.25), mat="cord")


def part_lid(rig: Rig) -> None:
    """兽皮盖：硬皮板，压得略歪（绕 y 偏 4°），前檐下折。"""
    rig.bone("lid", (0.0, H_BODY, 0.0))
    ROT, ORG = (0.0, 4.0, 0.0), (0.0, H_BODY, 0.0)
    # 顶板：厚 0.9px。1.3px 时盖组占编身高 51%，正面四成画面被皮子吞掉，
    # 读作厚木箱盖压在篓上；参考图的皮是软的、薄的，罩下来而不是架上去。
    rig.cube("lid", "lid_top",
             (-HALF_W - 0.9, H_BODY, -HALF_D - 0.8),
             (HALF_W + 0.9, H_BODY + 0.9, HALF_D + 0.9),
             rot=ROT, org=ORG, mat="hide")
    # 前檐：垂到篓口下 3.6px（参考图里皮子盖过前壁上半）
    rig.cube("lid", "lid_flap",
             (-HALF_W - 0.7, H_BODY - 3.6, HALF_D + 0.5),
             (HALF_W + 0.8, H_BODY + 0.6, HALF_D + 1.15),
             rot=ROT, org=ORG, mat="hide")
    # 侧檐：左右各垂一段，皮子才「罩」住而不是「架」在上面。
    # 内沿必须让开立柱外沿（HALF_W - 0.15 + POST/2 = 6.0），否则皮和竹互穿。
    for sx, side in ((-1, "l"), (1, "r")):
        x0 = sx * (HALF_W + 0.45)
        x1 = sx * (HALF_W + 1.05)
        rig.cube("lid", f"lid_skirt_{side}",
                 (min(x0, x1), H_BODY - 2.0, -HALF_D - 0.6),
                 (max(x0, x1), H_BODY + 0.6, HALF_D + 0.8),
                 rot=ROT, org=ORG, mat="hide")
    # 后檐略短
    rig.cube("lid", "lid_back",
             (-HALF_W - 0.7, H_BODY - 1.8, -HALF_D - 1.05),
             (HALF_W + 0.7, H_BODY + 0.9, -HALF_D - 0.45),
             rot=ROT, org=ORG, mat="hide")


def part_cord(rig: Rig) -> None:
    """捆绳两道 + 骨扣。绳跨盖顶压住兽皮，骨栓横拴在前壁。"""
    rig.bone("cord", (0.0, H_BODY, 0.0))
    # 绳半径 ≥0.45：16px 空间里 0.28 不足一像素，渲出来直接消失
    for i, x in enumerate((-2.6, 2.2)):
        # 跨顶（压在皮盖上方，y 高于 lid_top 顶面）
        rig.shaft("cord", f"lash_top_{i}",
                  (x, H_BODY + 1.55, -HALF_D - 0.7),
                  (x, H_BODY + 1.55, HALF_D + 1.05),
                  0.48, 0.48, mat="cord")
        # 前垂：贴在皮檐外侧
        rig.shaft("cord", f"lash_front_{i}",
                  (x, H_BODY + 1.55, HALF_D + 1.1),
                  (x, H_BODY - 4.6, HALF_D + 1.3),
                  0.45, 0.45, mat="cord")
    # 骨扣：**横置**小骨，中段被绳缠死当搭扣用。竖置 4.45px 长时占编身宽 40%，
    # 读作「挂了根大骨头」而不是扣件；参考图的骨横躺、长约编身宽 28%。
    # taper 分 4 段：两端骨节膨大、中段收细，才是骨头而不是白棍。
    py = H_BODY - 4.6
    pz = HALF_D + 1.45
    rig.taper("cord", "bone_peg",
              [(-1.6, py, pz), (-0.7, py + 0.1, pz),
               (0.7, py + 0.1, pz), (1.6, py, pz)],
              # 中段 0.55（=1.1px）必须粗于拴绳 0.8px，否则腰身被绳整条挡住，
              # 骨扣渲成两块分离的白方块（16px 空间的亚像素陷阱，同 lash 半径注释）。
              [0.72, 0.55, 0.55, 0.72], mat="bone", flat=0.85)
    # 拴绳绕骨扣中段一道（竖向绕，把骨压在绳下）
    rig.shaft("cord", "peg_tie",
              (0.0, py + 1.5, pz + 0.1),
              (0.0, py - 1.5, pz + 0.1),
              0.4, 0.4, mat="cord")


def _flat_band(rig: Rig, bone: str, prefix: str, pts, half_w: float,
               thick: float, mat: str) -> None:
    """沿折线铺一条**宽面恒定朝 -z** 的等宽扁带。

    为什么不用 shaft：shaft 按段方向自动定向，而背带是「先外扩再收回」的 S 形，
    dx/dz 符号会翻转，atan2 出的 yaw 在相邻段间跳近 180°。扁板绕 y 一转，宽面就
    从贴背翻成侧对镜头 —— 一段看着宽、下一段看着窄，拼起来是把尖角刀片（实测
    strap_l_1 yaw=-153.4° 紧接 strap_l_2 yaw=+26.6°）。
    这里只给 pitch（绕 x 倾），x 宽度全程恒定，于是每段都是平摆贴背的扁板。
    """
    for i, (a, b) in enumerate(zip(pts, pts[1:])):
        cx = (a[0] + b[0]) / 2
        dy, dz = b[1] - a[1], b[2] - a[2]
        pitch = math.degrees(math.atan2(dz, -dy))
        my, mz = (a[1] + b[1]) / 2, (a[2] + b[2]) / 2
        # 段长取 |dy| 而非 hypot(dy, dz)：斜长再绕 x 转会让端面在 z 上外扩，
        # 下一段起点仍按折线算，接缝就裂开（实测 z 起伏最大处裂 1.77px）。
        # 只吃 y 跨度，z 的走向交给 pitch 扫过，端面 y 便精确落在折线节点上。
        seg = abs(dy)
        # 段间留一点重叠，斜接的外棱不至于露出楔形缺口
        seg += 0.5 if 0 < i < len(pts) - 2 else 0.25
        rig.cube(bone, f"{prefix}_{i}",
                 (cx - half_w, my - seg / 2, mz - thick / 2),
                 (cx + half_w, my + seg / 2, mz + thick / 2),
                 rot=(pitch, 0.0, 0.0), org=(cx, my, mz), mat=mat)


def part_strap_l(rig: Rig) -> None:
    """左背带：原生草编，宽扁平板（不垂坠，靠分段折线读出弯）。"""
    rig.bone("strap_l", (-2.6, H_BODY, -HALF_D))
    # z 起伏必须压在板厚（0.84）量级内：段是直的，拐点在 z 上的错位无法被
    # 斜接吃掉，起伏一大就是肉眼可见的断层（原折线 z 摆幅 1.7px，实测裂开）。
    # 「绕过肩」的外扩交给 x 表达，z 只留极轻的离背弧度。
    pts = [(-2.6, H_BODY + 0.4, -HALF_D - 0.2),
           (-3.4, H_BODY - 2.6, -HALF_D - 0.5),
           (-3.7, H_BODY - 6.4, -HALF_D - 0.7),
           (-3.4, H_BODY - 9.6, -HALF_D - 0.5),
           (-2.8, 1.6, -HALF_D - 0.2)]
    _flat_band(rig, "strap_l", "strap_l", pts, 1.15, 0.84, "cord")


def part_strap_r(rig: Rig) -> None:
    """右背带：后补的灰布条，比左边窄、走线更直（刻意不匹配）。"""
    rig.bone("strap_r", (2.6, H_BODY, -HALF_D))
    # z 必须绕到后箍（外沿 -3.9）和后檐 lid_back（-4.45..-3.85）之外，否则灰布
    # 从竹箍/皮檐里穿出去。带子是绕过、贴在外面，不是扎进去。
    pts = [(2.6, H_BODY + 0.4, -HALF_D - 1.35),
           (3.2, H_BODY - 3.0, -HALF_D - 1.6),
           (3.4, H_BODY - 7.2, -HALF_D - 1.7),
           (3.0, 1.6, -HALF_D - 1.3)]
    _flat_band(rig, "strap_r", "strap_r", pts, 0.92, 0.72, "patch")
    # 粗针脚：接到篓身那截缝了一排十字线
    for i in range(3):
        y = H_BODY - 1.0 - i * 1.1
        rig.cube("strap_r", f"stitch_{i}",
                 (2.2, y, -HALF_D - 2.25), (3.9, y + 0.34, -HALF_D - 2.0), mat="stitch")


def part_patch(rig: Rig) -> None:
    """灰布补丁：正面主补 + 右侧壁副补，四边粗针脚。

    参考图有两块：**正面**左下一块显眼方补（主角），右侧带上一块小补。原先只做了
    侧面那块，正面视角完全看不到补丁 —— 缺了「穷酸缝补」最该被看见的一处。
    """
    rig.bone("patch", (0.0, 4.0, 0.0))
    # 正面主补：前壁左下，压在编带之上
    x0, x1 = -3.9, -0.5
    y0, y1 = 2.5, 5.9
    zf = HALF_D + 0.42
    rig.cube("patch", "patch_front",
             (x0, y0, HALF_D + 0.05), (x1, y1, zf), mat="patch")
    for i in range(4):
        x = x0 + 0.35 + i * 0.9
        rig.cube("patch", f"patch_fs_b{i}",
                 (x, y0 - 0.32, zf - 0.05), (x + 0.36, y0 + 0.16, zf + 0.2), mat="stitch")
        rig.cube("patch", f"patch_fs_t{i}",
                 (x, y1 - 0.16, zf - 0.05), (x + 0.36, y1 + 0.32, zf + 0.2), mat="stitch")
    for i in range(3):
        y = y0 + 0.5 + i * 0.95
        for sx, tag in ((x0, "l"), (x1 - 0.36, "r")):
            rig.cube("patch", f"patch_fs_{tag}{i}",
                     (sx, y, zf - 0.05), (sx + 0.36, y + 0.36, zf + 0.2), mat="stitch")
    # 右侧壁副补
    rig.cube("patch", "patch_cloth",
             (HALF_W - 0.05, 3.0, -1.9), (HALF_W + 0.4, 6.6, 1.5), mat="patch")
    for i, z in enumerate((-1.7, -0.6, 0.5, 1.3)):
        rig.cube("patch", f"patch_stitch_{i}",
                 (HALF_W + 0.1, 2.7, z), (HALF_W + 0.55, 3.2, z + 0.34), mat="stitch")
        rig.cube("patch", f"patch_stitch_t{i}",
                 (HALF_W + 0.1, 6.5, z), (HALF_W + 0.55, 7.0, z + 0.34), mat="stitch")


PARTS = {
    "frame": part_frame,
    "basket": part_basket,
    "rework": part_rework,
    "lid": part_lid,
    "cord": part_cord,
    "strap_l": part_strap_l,
    "strap_r": part_strap_r,
    "patch": part_patch,
}
ORDER = ["frame", "basket", "rework", "lid", "cord", "strap_l", "strap_r", "patch"]


def build(parts: list[str] | None = None) -> Rig:
    rig = Rig(MATS)
    for key in (parts or ORDER):
        PARTS[key](rig)
    return rig


def _shift_to_block_space(model: dict) -> None:
    """居中空间 → MC 方块空间：x/z +8，y 不动（地面已是 0）。"""
    for el in model["elements"]:
        for k in ("from", "to", "origin"):
            el[k][0] += 8.0
            el[k][2] += 8.0
    for node in model["outliner"]:
        node["origin"][0] += 8.0
        node["origin"][2] += 8.0


def _bone_of(rig: Rig, eid: str) -> str:
    for name, b in rig.bones.items():
        if eid in b["children"]:
            return name
    return "?"


def _overflow(rig: Rig) -> list[str]:
    """越出 0..16 方块空间的件（平移后会被 MC 裁掉）。"""
    bad = []
    for el in rig.elements:
        lo, hi = element_bounds([el])
        if lo[0] + 8 < -0.01 or hi[0] + 8 > 16.01 or lo[1] < -0.01 or hi[1] > 16.01 \
                or lo[2] + 8 < -0.01 or hi[2] + 8 > 16.01:
            bad.append(f"{el['name']}: {tuple(round(v, 2) for v in lo)}→"
                       f"{tuple(round(v, 2) for v in hi)}")
    return bad


def _orphans(rig: Rig) -> list[str]:
    """没被任何骨骼收养的 element（渲染时会丢）。"""
    owned = {eid for b in rig.bones.values() for eid in b["children"]}
    return [e["name"] for e in rig.elements if e["uuid"] not in owned]


def _degenerate(rig: Rig) -> list[str]:
    """任一轴薄于 0.2px 的件 —— 生图提示词里的「不细于二十分之一」下限。"""
    bad = []
    for el in rig.elements:
        d = [el["to"][i] - el["from"][i] for i in range(3)]
        if min(d) < 0.2:
            bad.append(f"{el['name']}: {tuple(round(v, 2) for v in d)}")
    return bad


def _floating(rig: Rig) -> list[str]:
    """悬空件：与其它件无任何面接触（≥2 轴重叠 > 0.15）。

    背带下端刻意脱离篓身（挂在空中读作可套肩），故白名单排除。
    """
    strap_tail = {"strap_l_3", "strap_r_2"}
    boxes = []
    for el in rig.elements:
        lo, hi = element_bounds([el])
        boxes.append((el["name"], lo, hi))
    bad = []
    for i, (name, lo, hi) in enumerate(boxes):
        if name in strap_tail:
            continue
        touch = False
        for j, (_, lo2, hi2) in enumerate(boxes):
            if i == j:
                continue
            ov = sum(1 for k in range(3)
                     if min(hi[k], hi2[k]) - max(lo[k], lo2[k]) > 0.15)
            if ov >= 2:
                touch = True
                break
        if not touch:
            bad.append(name)
    return bad


def _interpenetrating(rig: Rig) -> list[str]:
    """穿模：跨 bone 的两件在三轴上都实体重叠且体积可观。

    **必须区分「搭接」和「穿模」**：编带压壁、绳压皮盖、补丁贴壁、针脚咬补丁
    都是贴合，本来就该有薄重叠。判据取两条 ——
      1. 只查跨 bone 组合（同 bone 内是同一构件的分段，如 taper/shaft 相邻段）；
      2. 三轴同时重叠、且最小重叠深度 > MIN_BITE（不是薄贴而是真扎进去）。
    薄件贴合最小轴重叠通常 ≤0.5px；柱头穿盖那种是整段扎进去。

    材质对白名单：软件覆盖硬件是设计意图（皮盖罩竹架、绳捆皮盖、布补压编身），
    只有「硬件互穿」才是缺陷。
    """
    MIN_BITE = 0.55
    soft_over = {
        # 竹架嵌入编身壁是正常构造（立柱本就埋在壁里，实测 0.70px）。
        frozenset(("bamboo", "weave")), frozenset(("bamboo", "seam")),
        # hide×bamboo **不放行**：竹柱头扎穿皮盖正是要抓的穿模。
        frozenset(("cord", "hide")),
        frozenset(("cord", "bamboo")), frozenset(("cord", "weave")),
        frozenset(("cord", "seam")), frozenset(("cord", "bone")),
        frozenset(("patch", "weave")), frozenset(("patch", "seam")),
        frozenset(("stitch", "patch")), frozenset(("stitch", "weave")),
        frozenset(("stitch", "seam")), frozenset(("hide", "weave")),
        frozenset(("hide", "seam")), frozenset(("seam", "weave")),
        frozenset(("cord", "patch")),
    }
    items = []
    for el in rig.elements:
        lo, hi = element_bounds([el])
        items.append((el["name"], _bone_of(rig, el["uuid"]),
                      MATS_BY_COLOR.get(el["color"], "?"), lo, hi))
    bad = []
    for i, (n1, b1, m1, lo1, hi1) in enumerate(items):
        for n2, b2, m2, lo2, hi2 in items[i + 1:]:
            if b1 == b2:
                continue
            if frozenset((m1, m2)) in soft_over or m1 == m2:
                continue
            bite = min(min(hi1[k], hi2[k]) - max(lo1[k], lo2[k])
                       for k in range(3))
            if bite > MIN_BITE:
                bad.append(f"{n1}({m1}) × {n2}({m2}) 互穿 {bite:.2f}px")
    return bad


def check(rig: Rig) -> int:
    """五道门：孤儿 / 越界 / 退化薄片 / 悬空 / 穿模。外加对称件镜像。

    刻意不对称件（盖歪、右带替换、补丁、重编）走 ASYM 白名单排除 —— 那是
    设计意图，不是缺陷。
    """
    print("背篓 / back_basket 自检:")
    lo, hi = rig.bounds()
    dims = tuple(hi[i] - lo[i] for i in range(3))
    print(f"  bbox   : {dims[0]:.1f}×{dims[1]:.1f}×{dims[2]:.1f}px = "
          f"{dims[0]/PX:.2f}W × {dims[1]/PX:.2f}H × {dims[2]/PX:.2f}D 格")
    print(f"  cubes  : {len(rig.elements)}  bones: {len(rig.bones)}")
    used = {}
    for el in rig.elements:
        m = MATS_BY_COLOR.get(el["color"], "?")
        used[m] = used.get(m, 0) + 1
    print(f"  材质   : {len(used)}/{len(MATS)} 种在用 — "
          + ", ".join(f"{k}:{v}" for k, v in used.items()))

    total = 0
    sym_els = [e for e in rig.elements if _bone_of(rig, e["uuid"]) not in ASYM]
    from rigkit import mirror_violations as _mv
    gates = [
        ("孤儿 element", _orphans(rig)),
        ("越出 0..16 方块空间", _overflow(rig)),
        ("退化薄片 (<0.2px)", _degenerate(rig)),
        ("悬空无接触", _floating(rig)),
        ("硬件互穿（穿模）", _interpenetrating(rig)),
        ("对称件左右不镜像", _mv(sym_els)),
    ]
    for label, bad in gates:
        total += len(bad)
        mark = "✓" if not bad else "✗"
        print(f"  {mark} {label}: {len(bad)}")
        for b in bad[:6]:
            print(f"      - {b}")
    print(f"  → 共 {total} 处违例")
    print("  注：立体感/比例/不对称是否好看，自检量不出 —— 必须人眼看 "
          "render_bbmodel.py 三视图定夺。")
    return total


MATS_BY_COLOR = {i % 8: name for i, name in enumerate(MATS)}


def main() -> int:
    ap = argparse.ArgumentParser(description="背篓 bbmodel 生成器")
    ap.add_argument("--part", help="只生成单件（调试用）")
    ap.add_argument("--check", action="store_true", help="只跑自检，不写盘")
    ap.add_argument("--list", action="store_true", help="列出所有部件")
    args = ap.parse_args()

    if args.list:
        for k in ORDER:
            doc = (PARTS[k].__doc__ or "").strip().splitlines()[0]
            print(f"  {k:9s} {doc}")
        return 0

    parts = [args.part] if args.part else None
    if args.part and args.part not in PARTS:
        print(f"未知部件 {args.part}；可选：{', '.join(ORDER)}")
        return 2

    rig = build(parts)
    bad = check(rig)
    if args.check:
        return 1 if bad else 0

    name = f"BackBasket_{args.part}" if args.part else "BackBasket"
    model = rig.bbmodel(name)
    _shift_to_block_space(model)
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = OUT_DIR / f"{name}.bbmodel"
    out.write_text(json.dumps(model, ensure_ascii=False, indent=1))
    print(f"  → {out.relative_to(REPO)} ({out.stat().st_size} B)")
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main())
