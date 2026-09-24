#!/usr/bin/env python3
"""珂珂达（kekeda_goose）外观层 —— 最终成品那一层。原版口径。

**造型原则：少量大方块，形靠比例不靠堆叠。**

走过两条弯路，都记在这里：

  一版把绒羽做成上百簇朝各方向的**旋转小方块**（"羽簇从体表长到外壳"，解剖上讲得
  通）。渲出来是个刺球：每簇的边角各自吃光，表面全是噪点。

  二版改成分带切片堆椭球，一路加密到 27 带 × 10 环 = 481 块、台阶 0.86 单位。
  是圆了，但那是拿几百个小台阶去逼近曲面 —— 体素本来就不该这么用，代价也不合理。

  这一版按 MC 原版生物的做法：整只四十块。躯干五块（主体 + 上下前后四块倒角）、
  头两块、颈四片、喙三块 + 眼四块、翼各三片平板、尾两片、腿脚各七块。台阶少而大，
  是**设计出来的面**，不是逼近曲面时漏出来的锯齿 —— 后者才是"块状堆叠"难看的原因。

  颈之所以是四片而不是一片：动画层要把它从弦长 4.39 伸到 7.0（威吓、鸣叫），
  一整块跟不住两头，会脱节成悬空的方块。详见 part_neck。

配色也按原版逻辑：白身 + 灰翼灰尾 + 橙喙橙蹼。翼尾压一档灰不只是好看，是**可读性**
——通体全白时远处只剩一个白团，分不出朝向也分不出翅膀在哪。

翅膀挂在自己的 wing_l/r 骨上，是体侧独立的三片平板：静止时贴着身体，骨骼一转整片
掀起，底下躯干是完整的。

分部件（逐件可单独预览）：
  body 躯干 · neck 颈 · head 头 · bill 喙 + 眼 · wing 收翼 · tail 尾羽 · legs 腿脚

用法:
  python3 modelScript/creatures/kekeda_goose/gen_plume.py                  # 成品
  python3 modelScript/creatures/kekeda_goose/gen_plume.py --part body
  python3 modelScript/creatures/kekeda_goose/gen_plume.py --with-anatomy   # 半剖，看外皮多厚
  python3 modelScript/creatures/kekeda_goose/gen_plume.py --list
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "core"))
sys.path.insert(0, str(HERE))

import gen_muscle as MU  # noqa: E402
import gen_skeleton as SK  # noqa: E402
from bbmodel_maker.rig.voxel_rig import Palette, Rig  # noqa: E402

OUT_DIR = SK.OUT_DIR

PLUME_MATS = {
    "down": (241, 238, 231),
    "down_shade": (219, 214, 203),   # 腹侧压一档
    "wing": (197, 196, 191),         # 翼覆羽：比身体灰一档，远处才分得出翅膀在哪
    "wing_dark": (159, 158, 153),    # 飞羽 / 尾羽
    "bill_h": (240, 158, 66),
    "bill_dark": (203, 120, 44),
    "eye_h": (26, 24, 22),
    "eye_light": (250, 250, 248),
}
# noise 调到 2：默认 ±4 的抖动在这种大面积平色上会读成木纹
PALETTE = Palette({**SK.MATS, **MU.MUSCLE_MATS, **PLUME_MATS}, noise=2)

# ================================================================ 关键尺度
# 单位 = MC 像素，16 = 1 格。地面 y=0，头朝 -Z。站高约 15.7 ≈ 0.98 m。
BODY_HALF_W = 4.00
BODY_TOP = 11.05
BODY_BOTTOM = 4.70
HEAD_C_Y = 14.30
LEG_X = 1.88


# ================================================================ 躯干
def part_body(rig: Rig) -> None:
    """躯干：主体一块 + 上下前后四块倒角。

    倒角块是让方盒子读成"胖鹅"的全部秘密：每块都比主体窄一圈、各自只朝一个方向
    伸出去，八个角就被切掉了。五块，没有一处阶梯 —— 比用几百个小台阶去逼近椭球
    干净得多。
    """
    rig.bone("plume_body", (0.0, 7.9, 0.0), parent="trunk_back")
    for name, (x, y0, y1, z0, z1, mat) in {
        "body_core": (BODY_HALF_W, BODY_BOTTOM, BODY_TOP, -4.10, 4.00, "down"),
        # 背冠：x 和 z 都要收，只收 x 的话它在背上就是一块盖子（顶面那圈亮边一眼看得见）
        "body_back": (3.15, BODY_TOP - 0.10, 11.85, -3.00, 2.85, "down"),
        "body_belly": (3.15, 3.80, BODY_BOTTOM + 0.15, -3.30, 3.00, "down_shade"),
        "body_breast": (3.25, 5.55, 10.15, -5.35, -3.95, "down"),
        "body_rump": (3.00, 5.85, 10.25, 3.90, 5.28, "down"),
    }.items():
        rig.cube("plume_body", name, (-x, y0, z0), (x, y1, z1), mat=mat)


#: 露在外面那截颈：从躯干顶面钻出来到头底，四片自下而上略微收窄。
#: (挂靠骨, y0, y1, 半宽)
#:
#: 相邻片在静止姿要**重叠 0.75**，不是刚好接上。颈全伸时弦长从 4.39 涨到 7.04
#: （+60%），片心间距 0.9 会被拉开约 0.55 —— 重叠给到 0.75 才留得住。实测重叠 0.40
#: 时引颈高鸣那一帧裂开 0.21，方块之间能看见黑缝。裂缝实测出现在**最下面**那道
#: （身→颈），颈一伸整条被从躯干里拔出来，所以最底那片要往下扎得足够深 ——
#: 反正埋在身体里，多长都不花钱。
NECK_SLICES = (
    ("neck_8", 9.20, 12.00, 1.72),
    ("neck_11", 11.25, 12.85, 1.66),
    ("neck_14", 12.10, 13.45, 1.60),
    ("skull", 12.70, 13.75, 1.54),
)


def part_neck(rig: Rig) -> None:
    """颈：沿颈链分四片。真颈 17 节折成 S 整条埋在身体里，露出来就这一小截。

    **一块是不行的。** 首版就挂一块在 neck_12 上，静止姿看着没问题；可是威吓时颈要
    从弦长 4.39 伸到 7.0（+60%），那一块跟着单根椎骨走，上头跟不上颅骨、下头追不上
    躯干，渲出来是几块白方块**悬在空中**。分片之后每片各跟自己那节椎骨，伸展时片与
    片之间只是重叠变少，链子不会断 —— 相邻片在静止姿留足重叠就是为这个。
    check_anim 的颈连续性项会实测所有动画里最坏的那道缝，别只信这段注释。

    颈要比头窄一圈，且躯干顶面到头底之间得留出净空，否则头直接坐在身体上，
    侧看是一整团白，看不出这是脖子。
    """
    for i, (bone, y0, y1, w) in enumerate(NECK_SLICES):
        rig.cube(bone, f"neck_{i}", (-w, y0, -5.05 + i * 0.10), (w, y1, -2.55 - i * 0.06),
                 mat="down")


def part_head(rig: Rig) -> None:
    """头：一块 + 一小块额头收口。"""
    rig.cube("skull", "head", (-1.82, 12.80, -6.10), (1.82, 15.60, -3.00), mat="down")
    rig.cube("skull", "crown", (-1.38, 15.50, -5.60), (1.38, 15.98, -3.40), mat="down")


# ================================================================ 喙 / 眼
def part_bill(rig: Rig) -> None:
    """喙：上喙 + 喙甲 + 下喙，三块。鸭雁的喙宽而扁，别做成尖锥。

    下喙的**后端要一直插进头里**（z 到 −4.60，不是到 −5.90 就停）。jaw 骨的 pivot 在
    z=−3.40，比喙的实际铰链靠后 2.5 单位；下喙只做到 −5.90 的话，张嘴超过 20° 整根就
    被从头里拽出来，渲出来是一根**悬空的橙条**（实测 26° 时缝隙 +0.33）。多出来这截
    平时埋在头里看不见，张嘴时正好当口腔内壁。
    """
    rig.cube("bill_upper", "bill", (-1.25, 13.50, -8.30), (1.25, 14.45, -5.90), mat="bill_h")
    rig.cube("bill_upper", "bill_nail", (-1.05, 13.42, -8.72), (1.05, 14.08, -8.20), mat="bill_dark")
    rig.cube("jaw", "bill_lower", (-1.05, 13.12, -8.20), (1.05, 13.55, -4.60), mat="bill_dark")


def part_eyes(rig: Rig) -> None:
    """眼：一颗小黑豆 + 一粒高光。

    深度关系是这里唯一的坑：轴对齐盒子没有"贴面"概念，谁在前谁把谁盖死。
    眼必须在**每个要被看到的方向**上都比头凸出去（前 -z、外 ±x），高光再比眼凸一点。
    栽过两次：眼圈前缘比眼珠靠前 → 正面只剩两块白片；眼下缘落在比眼中心更宽的
    头切片里 → 半颗眼被自己的脸埋掉。
    """
    for sx, side in ((-1, "l"), (1, "r")):
        rig.cube("skull", f"eye_{side}",
                 (sx * 1.30, 14.35, -6.24), (sx * 1.92, 15.00, -5.20), mat="eye_h")
        rig.cube("skull", f"eye_light_{side}",
                 (sx * 1.42, 14.72, -6.36), (sx * 1.66, 14.96, -5.90), mat="eye_light")


# ================================================================ 翼 / 尾
def part_wing(rig: Rig, sx: int, side: str) -> None:
    """收起的翅膀：体侧三片平板，尖端逐级后退变窄，搭过尾根。

    原版的翅膀就是贴在体侧的板，不做立体；分三片是为了尖端能"阶梯式"收窄 ——
    那几级台阶正是折叠飞羽的读点，而且是**设计出来的**台阶，不是逼近曲面漏出的锯齿。
    整组挂在 wing_l/r 骨上，转起来整片掀。
    """
    for i, (x0, x1, y0, y1, z0, z1, mat) in enumerate((
        (3.95, 4.62, 6.35, 10.55, -3.35, 3.45, "wing"),        # 覆羽主片
        (3.90, 4.48, 6.60, 8.60, 3.30, 5.80, "wing_dark"),     # 初级飞羽
        (3.85, 4.36, 6.95, 8.05, 5.70, 7.15, "wing_dark"),     # 飞羽尖
    )):
        rig.cube(f"wing_{side}", f"wing_{side}_{i}",
                 (sx * x0, y0, z0), (sx * x1, y1, z1), mat=mat)


def part_tail(rig: Rig) -> None:
    """尾羽：两片，短而上翘。鸭雁的尾很短，够判朝向就行。"""
    rig.cube("tail_base", "tail_0", (-2.20, 9.00, 5.10), (2.20, 10.40, 7.00), mat="wing")
    rig.cube("tail_base", "tail_1", (-1.60, 9.30, 6.90), (1.60, 10.20, 8.10), mat="wing_dark")


# ================================================================ 腿脚
def part_legs(rig: Rig) -> None:
    """腿：两段橙柱；脚：三级递宽的蹼板 + 两道趾缝。"""
    for sx, side in ((-1, "l"), (1, "r")):
        for i, (w, y0, y1, z0, z1) in enumerate((
            (0.66, 2.40, 4.30, -0.62, 0.62),
            (0.58, 0.60, 2.50, -0.54, 0.58),
        )):
            rig.cube(f"tarsus_{side}", f"shank_{side}_{i}",
                     (sx * LEG_X - w, y0, z0), (sx * LEG_X + w, y1, z1), mat="bill_h")
        # 最宽那级不能超过 LEG_X（1.88）—— 超了两只脚在正面就连成一条横板
        for i, (w, z0, z1) in enumerate(((0.92, -0.40, 0.85), (1.34, -1.60, -0.35), (1.66, -2.70, -1.55))):
            rig.cube(f"foot_{side}", f"web_{side}_{i}",
                     (sx * LEG_X - w, 0.0, z0), (sx * LEG_X + w, 0.58, z1), mat="bill_h")
        for tag, k in (("a", -1), ("b", 1)):
            # 偏移量要连 sx 一起乘：只乘 k 的话左脚的两道口子是平移不是镜像
            rig.cube(f"foot_{side}", f"web_notch_{side}_{tag}",
                     (sx * (LEG_X + k * 0.42), 0.02, -2.70), (sx * (LEG_X + k * 0.74), 0.56, -1.95),
                     mat="bill_dark")


# ================================================================ 装配
PARTS = {
    "body": ("躯干", part_body),
    "neck": ("颈", part_neck),
    "head": ("头", part_head),
    "bill": ("喙 + 眼", lambda r: (part_bill(r), part_eyes(r))),
    "wing": ("收翼", lambda r: [part_wing(r, sx, s) for sx, s in ((-1, "l"), (1, "r"))]),
    "tail": ("尾羽", part_tail),
    "legs": ("腿脚", part_legs),
}


def build(only: str | None = None, with_anatomy: bool = False, cutaway: bool = False) -> Rig:
    rig = Rig(PALETTE)
    # 前缀对拍要覆盖**所有会被搬进来的层**，不能只查骨架：漏查哪层，那层的 element
    # 就会静默地渲成别的颜色（半剖图里整片胸肌曾被渲成白的，看着还挺像羽毛）
    for src_pal, who in ((SK.PALETTE, "骨架"), (MU.PALETTE, "肌肉")):
        assert PALETTE.names[: len(src_pal.names)] == src_pal.names, (
            f"调色板前缀与{who}层不一致，搬过来的 element uv 会指错色块"
        )
    src = MU.build() if with_anatomy else SK.build_full()
    for name in src.bone_order:
        b = src.bones[name]
        rig.bone(name, tuple(b["pivot"]), b["parent"])
    if with_anatomy:
        rig.elements.extend(src.elements)
        for name in src.bone_order:
            rig.bones[name]["children"] = list(src.bones[name]["children"])

    n_anat = len(rig.elements)
    for key, (_label, fn) in PARTS.items():
        if only is None or key == only:
            fn(rig)  # type: ignore[operator]

    if cutaway:
        # 半剖：外观层只留右半，露出底下的骨与肌。直接两层叠起来是白叠 —— 外观把
        # 一切盖死，出来的图和成品图一模一样，没有诊断价值。
        # 外观层全是轴对齐块，所以**裁 x 范围**而不是整块丢
        dropped = set()
        for e in rig.elements[n_anat:]:
            if any(e["rotation"]):
                if e["origin"][0] < -0.15:
                    dropped.add(e["uuid"])
            elif e["to"][0] <= 0.0:
                dropped.add(e["uuid"])
            elif e["from"][0] < 0.0:
                e["from"][0] = 0.0
        rig.elements = [e for e in rig.elements if e["uuid"] not in dropped]
        for b in rig.bones.values():
            b["children"] = [c for c in b["children"] if c not in dropped]
    return rig


# 原版口径的生物大多在 10~40 块。超出就说明又在拿小方块逼近曲面了 —— 这个模型
# 走过两次那条路（羽簇 355 块、分带椭球 481 块），都得推倒重来，所以把上限焊进自检
CUBE_BUDGET = 44


def report(rig: Rig, *, symmetric: bool = True) -> int:
    # 半剖模型左右本来就不对称，别拿镜像自检去量它
    problems = rig.mirror_problems() if symmetric else []
    lo, hi = rig.bounds()
    height = hi[1]
    body_w = 2 * BODY_HALF_W
    head_w = 3.60
    shank = (BODY_BOTTOM - 0.95) / height          # 腹底（含倒角块）/ 站高

    if not 0.44 <= body_w / height <= 0.60:
        problems.append(f"体宽/站高 = {body_w / height:.2f}（应 0.44~0.60）")
    if not 0.34 <= head_w / body_w <= 0.52:
        problems.append(f"头宽/体宽 = {head_w / body_w:.2f}（应 0.34~0.52）")
    if not 0.13 <= shank <= 0.28:
        problems.append(f"露腿/站高 = {shank:.2f}（应 0.13~0.28）")
    if lo[1] < -0.05 or lo[1] > 0.30:
        problems.append(
            f"贴地异常：最低点 y={lo[1]:.2f}；最低几件："
            f"{', '.join(f'{n}@{y:.2f}' for y, n in rig.lowest(4))}"
        )
    if symmetric and len(rig.elements) > CUBE_BUDGET:
        problems.append(f"块数 {len(rig.elements)} 超预算 {CUBE_BUDGET} —— "
                        f"多半是又在用小方块逼近曲面了，回去看模块头部那两条弯路")

    print(f"cube {len(rig.elements)} 个（预算 {CUBE_BUDGET}）· 骨骼 {len(rig.bones)} 根")
    print(f"站高 {height:.2f} = {height / 16:.2f} m · 体宽 {body_w:.2f}（占站高 {body_w / height:.0%}）"
          f" · 全宽 {hi[0] - lo[0]:.2f} · 全长 {hi[2] - lo[2]:.2f} = {(hi[2] - lo[2]) / 16:.2f} m")
    print(f"头宽 {head_w:.2f}（体宽的 {head_w / body_w:.0%}）· 露腿 {shank:.0%} · 最低点 {lo[1]:.2f}")
    if problems:
        print(f"\n✗ {len(problems)} 处违例：")
        for p in problems[:12]:
            print(f"   {p}")
    else:
        print("\n✓ 镜像 / 体宽比 / 头身比 / 露腿比例 / 贴地 / 块数预算 全部通过")
    return len(problems)


def main() -> int:
    ap = argparse.ArgumentParser(description="珂珂达外观层")
    ap.add_argument("--part", choices=sorted(PARTS))
    ap.add_argument("--with-anatomy", action="store_true", help="半剖：左半留骨+肌，右半留外观")
    ap.add_argument("--list", action="store_true")
    ap.add_argument("--check", action="store_true", help="只报告，不写文件")
    ap.add_argument("--out", type=Path)
    args = ap.parse_args()

    if args.list:
        for k, (label, _) in PARTS.items():
            print(f"  {k:5s} {label}")
        return 0
    if args.check:
        return 1 if report(build()) else 0

    rig = build(only=args.part, with_anatomy=args.with_anatomy, cutaway=args.with_anatomy)
    name = "KekedaPlume"
    if args.part:
        name += f"_{args.part}"
    if args.with_anatomy:
        name += "_anatomy"
    out = rig.save(args.out or (OUT_DIR / f"{name}.bbmodel"), name)
    print(f"→ {out}")
    bad = report(rig, symmetric=not args.with_anatomy)
    return 1 if bad and not args.part else 0


if __name__ == "__main__":
    raise SystemExit(main())
