#!/usr/bin/env python3
"""腐沼蟾（fu_zhao_chan）Blockbench .bbmodel 程序化生成器 —— Round 3/3，几何。

正典依据：docs/worldview.md §十三 灵泉湿地「异变兽」。任务卡：末世生物批次 #11（.task-wasteland-creatures.md）：
肥大的泥沼蟾蜍，背上鼓包长着灵草和苔，吸饱灵气时整个身子鼓胀（动画要有鼓胀），长舌，眼睛突出。

外形：一只比人膝盖还高的蹲伏大蟾蜍（体长约 26px、宽 20px、背高 15px）。身体是一个扁圆的体腔（浅色腹面带褐色菱斑），
背上一粒粒疣鼓包（深浅两色），顶面长着灰绿苔斑和几丛细小的灵草（白色小花）、一根冒出来的枯枝；头宽扁，嘴微张
（上颌内侧肉红口腔、下颌内侧同色），一条粉红长舌从嘴里垂出来、尖端下卷；两只金黄色的大凸眼立在头顶（横向黑瞳）；
喉下一个浅色鼓囊（吸饱灵气时鼓胀）。四肢：前肢粗短、四指撑地；后肢折叠，大腿粗壮、小腿折回、五趾张开。

做法：**体素场**（creature_kit.Field）——所有 1px 体素格进同一张表，先到先得，所以件与件之间没有共面和重叠；
只算右半（x ≥ 0）再镜像。体块 / 肢体用椭球和胶囊，疣、灵草、枯枝是单独的小体素。每只骨都带几何。
骨骼（≥28）见 BONES；层级树随接触表输出。

用法：
    python3 modelScript/creatures/fu_zhao_chan/gen_fu_zhao_chan.py --self-test   # 门禁差分自证
    python3 modelScript/creatures/fu_zhao_chan/gen_fu_zhao_chan.py --part legs    # 单件预览
    python3 modelScript/creatures/fu_zhao_chan/gen_fu_zhao_chan.py                # 全量：模型 + 接触表 + check.png
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO / "modelScript" / "creatures" / "_kit"))
import creature_kit as kit  # noqa: E402
from creature_kit import Field, Puppet, blotch, pick  # noqa: E402

CREATURE_DIR = Path(__file__).resolve().parent
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/creatures/fu_zhao_chan")
PARTS_DIR = REVIEW_DIR / "parts"
MODEL_ID = "fu_zhao_chan"

# ───────────────────────────────────────────────────────────── 材质（RGB，每种占贴图一个 8×8 色块）
MATS = {
    "skin_dk":    (70, 62, 40),       # 蟾皮：橄榄褐三档
    "skin":       (96, 86, 54),
    "skin_lt":    (122, 110, 70),
    "moss":       (78, 92, 48),       # 背上灰绿苔
    "moss_lt":    (104, 120, 64),
    "belly":      (196, 184, 150),    # 腹面浅色
    "belly_spot": (150, 120, 84),     # 腹面褐菱斑
    "wart":       (108, 84, 60),      # 疣
    "wart_lt":    (138, 112, 80),
    "eye":        (212, 176, 80),     # 金黄凸眼
    "eye_ring":   (96, 78, 40),
    "pupil":      (18, 14, 12),
    "mouth":      (150, 72, 76),      # 口腔
    "tongue":     (206, 120, 124),
    "tongue_dk":  (168, 90, 98),
    "toe":        (84, 72, 46),
    "toe_pad":    (150, 130, 90),
    "herb":       (96, 140, 70),      # 灵草
    "herb_flower": (222, 214, 190),
    "twig":       (80, 60, 44),
}

# ───────────────────────────────────────────────────────────── 骨骼树
# (name, parent, pivot[px, 世界坐标], 说明)。先父后子。+x = 右侧（*_r）。
BONES: list[tuple[str, str | None, tuple[float, float, float], str]] = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("body_hips", "root", (0.0, 8.0, 6.0), "臀（后肢根）：疣 + 苔"),
    ("body_mid", "body_hips", (0.0, 8.0, -1.0), "腰（背上灵草的根）"),
    ("body_chest", "body_mid", (0.0, 8.0, -8.0), "胸（前肢根）"),
    ("head", "body_chest", (0.0, 9.0, -11.5), "头：上颌 + 口腔顶"),
    ("jaw", "head", (0.0, 6.5, -10.5), "下颌：口腔底"),
    ("tongue_1", "jaw", (0.0, 6.4, -17.5), "舌 1（根）"),
    ("tongue_2", "tongue_1", (0.0, 6.2, -21.5), "舌 2"),
    ("tongue_3", "tongue_2", (0.0, 5.2, -25.0), "舌 3（尖，下卷）"),
    ("throat", "head", (0.0, 4.5, -12.5), "喉鼓囊（吸灵时鼓胀）"),
    ("eye_r", "head", (4.6, 15.0, -13.8), "右凸眼"),
    ("eye_l", "head", (-4.6, 15.0, -13.8), "左凸眼"),
    ("herb_a_r", "body_mid", (3.5, 15.0, -4.5), "右灵草丛 a"),
    ("herb_a_l", "body_mid", (-3.5, 15.0, -4.5), "左灵草丛 a"),
    ("herb_b_r", "body_hips", (5.5, 14.0, 4.0), "右灵草丛 b"),
    ("herb_b_l", "body_hips", (-5.5, 14.0, 4.0), "左灵草丛 b"),
    ("herb_c_r", "body_hips", (1.5, 14.5, 7.5), "右灵草丛 c"),
    ("herb_c_l", "body_hips", (-1.5, 14.5, 7.5), "左灵草丛 c"),
    ("twig_r", "body_mid", (6.5, 14.0, -1.0), "右背枯枝"),
    ("twig_l", "body_mid", (-6.5, 14.0, -1.0), "左背枯枝"),
    ("leg_f1_r", "body_chest", (8.2, 7.4, -8.2), "右前肢·上臂"),
    ("leg_f1_l", "body_chest", (-8.2, 7.4, -8.2), "左前肢·上臂"),
    ("leg_f2_r", "leg_f1_r", (10.6, 3.6, -10.6), "右前肢·前臂"),
    ("leg_f2_l", "leg_f1_l", (-10.6, 3.6, -10.6), "左前肢·前臂"),
    ("leg_f3_r", "leg_f2_r", (9.4, 1.2, -14.2), "右前肢·手掌"),
    ("leg_f3_l", "leg_f2_l", (-9.4, 1.2, -14.2), "左前肢·手掌"),
    ("toe_f_r", "leg_f3_r", (9.2, 0.7, -16.0), "右前肢·四指"),
    ("toe_f_l", "leg_f3_l", (-9.2, 0.7, -16.0), "左前肢·四指"),
    ("leg_h1_r", "body_hips", (8.5, 8.0, 7.0), "右后肢·大腿"),
    ("leg_h1_l", "body_hips", (-8.5, 8.0, 7.0), "左后肢·大腿"),
    ("leg_h2_r", "leg_h1_r", (13.0, 5.0, 0.0), "右后肢·小腿"),
    ("leg_h2_l", "leg_h1_l", (-13.0, 5.0, 0.0), "左后肢·小腿"),
    ("leg_h3_r", "leg_h2_r", (12.6, 2.0, 9.2), "右后肢·跗跖"),
    ("leg_h3_l", "leg_h2_l", (-12.6, 2.0, 9.2), "左后肢·跗跖"),
    ("toe_h_r", "leg_h3_r", (11.6, 0.9, 3.2), "右后肢·五趾"),
    ("toe_h_l", "leg_h3_l", (-11.6, 0.9, 3.2), "左后肢·五趾"),
]
MIN_BONES = 28                      # 任务卡「骨骼数下限」：四足 ≥ 28


def new_puppet() -> Puppet:
    return Puppet(MATS, BONES)


def new_field(p: Puppet) -> Field:
    f = Field(p)

    def body_bone(bone, i, j, k):
        if bone != "body":
            return bone
        return "body_chest" if k < -5 else ("body_hips" if k >= 3 else "body_mid")

    f.bone_of = body_bone
    return f


# ───────────────────────────────────────────────────────────── 蟾皮材质函数
def skin_fn(i: int, j: int, k: int) -> str:
    """蟾皮：腹面浅色带菱斑；背面橄榄褐三档斑块；顶面苔斑。"""
    if j <= 5:
        return "belly_spot" if blotch(i, j, k, 3, 5) < 16 else "belly"
    if j >= 12 and blotch(i, j, k, 3, 7) < 26:
        return "moss" if blotch(i, j, k, 2, 9) < 62 else "moss_lt"
    return pick(blotch(i, j, k, 2, 1), (("skin_dk", 36), ("skin", 40), ("skin_lt", 24)))


def limb_fn(i: int, j: int, k: int) -> str:
    if j >= 4 and blotch(i, j, k, 3, 11) < 22:
        return "moss"
    return pick(blotch(i, j, k, 2, 3), (("skin_dk", 40), ("skin", 40), ("skin_lt", 20)))


def toe_fn(i: int, j: int, k: int) -> str:
    return "toe_pad" if blotch(i, j, k, 2, 13) < 22 else "toe"


def tongue_fn(i: int, j: int, k: int) -> str:
    return "tongue_dk" if blotch(i, j, k, 2, 15) < 28 else "tongue"


# ───────────────────────────────────────────────────────────── 分部件
def part_legs(p: Puppet, f: Field) -> None:
    """四肢：前肢粗短（上臂 / 前臂 / 手掌 / 四指），后肢折叠（粗壮大腿 / 折回的小腿 / 跗跖 / 五趾张开）。"""
    f.capsule("leg_f1_r", (8.2, 7.4, -8.2), (10.6, 3.6, -10.6), 3.0, limb_fn, r_end=2.4, surface=True)
    f.capsule("leg_f2_r", (10.6, 3.6, -10.6), (9.4, 1.2, -14.2), 2.3, limb_fn, r_end=1.9)
    f.capsule("leg_f3_r", (9.4, 1.2, -14.2), (9.2, 0.7, -16.0), 1.7, limb_fn)
    for tip in ((6.8, 0.5, -19.4), (8.6, 0.5, -20.0), (10.4, 0.5, -20.0), (12.0, 0.5, -19.0)):
        f.capsule("toe_f_r", (9.2, 0.7, -16.0), tip, 1.0, toe_fn)
    f.capsule("leg_h1_r", (8.5, 8.0, 7.0), (13.0, 5.0, 0.0), 4.4, limb_fn, r_end=3.4, surface=True)
    f.capsule("leg_h2_r", (13.0, 5.0, 0.0), (12.6, 2.0, 9.2), 3.0, limb_fn, r_end=2.0, surface=True)
    f.capsule("leg_h3_r", (12.6, 2.0, 9.2), (11.6, 0.9, 3.2), 1.9, limb_fn, r_end=1.5)
    for tip in ((8.4, 0.5, -0.6), (10.2, 0.5, -1.6), (12.2, 0.5, -2.0), (14.0, 0.5, -1.4), (15.4, 0.5, 0.2)):
        f.capsule("toe_h_r", (11.6, 0.9, 3.2), tip, 1.0, toe_fn)


def eye_fn(center):
    cx, cy, cz = center

    def fn(i, j, k):
        dx, dy, dz = i + 0.5 - cx, j + 0.5 - cy, k + 0.5 - cz
        if dz < -2.0 and abs(dy) < 0.9 and abs(dx) < 1.7:
            return "pupil"                                    # 横向黑瞳（朝前）
        return "eye_ring" if (dx * dx + dy * dy + dz * dz) ** 0.5 > 3.2 else "eye"

    return fn


def part_head(p: Puppet, f: Field) -> None:
    """头：扁宽的上颌（口腔顶肉红）+ 下颌（口腔底肉红）+ 两只金黄凸眼（横向黑瞳）+ 喉下鼓囊。"""
    f.ellipsoid("head", (0.0, 10.4, -14.5), (8.4, 5.4, 6.2), skin_fn, clip=lambda i, j, k: j >= 8, cut_mat="mouth")
    f.ellipsoid("jaw", (0.0, 5.8, -13.5), (6.8, 2.6, 5.0), skin_fn, clip=lambda i, j, k: j <= 5, cut_mat="mouth")
    f.ellipsoid("eye_r", (4.8, 15.4, -14.0), (3.5, 3.5, 3.5), eye_fn((4.8, 15.4, -14.0)), surface=False, overwrite=True)
    f.ellipsoid("throat", (0.0, 3.6, -12.2), (5.2, 3.4, 4.0), lambda i, j, k: "belly_spot" if blotch(i, j, k, 3, 5) < 16 else "belly")
    # 眉棱：盖在凸眼上缘的一道皮褶，让眼睛像从皮里鼓出来
    f.ellipsoid("head", (4.6, 16.6, -13.0), (3.8, 1.6, 3.4), lambda i, j, k: "skin_dk" if blotch(i, j, k, 2, 21) < 45 else "skin")
    # 耳后腺：眼后一对又大又鼓的疣包（蟾蜍的标志），顶上一圈浅色
    f.ellipsoid("head", (7.0, 13.2, -9.0), (3.2, 2.6, 4.0), lambda i, j, k: "wart_lt" if blotch(i, j, k, 2, 23) < 40 else "wart")
    # 上颌唇缘：嘴线上一排细小的疣
    for z in (-19, -17, -15, -13, -11):
        f.ellipsoid("head", (6.0 + 0.2 * (z + 19) / 2, 8.4, float(z)), (1.2, 1.0, 1.1), lambda i, j, k: "wart")


def part_tongue(p: Puppet, f: Field) -> None:
    """长舌：粉红舌三段，从口腔里垂出来，逐段变细、尖端下卷。"""
    f.capsule("tongue_1", (0.0, 6.5, -17.5), (0.0, 6.2, -21.5), 1.3, tongue_fn)
    f.capsule("tongue_2", (0.0, 6.2, -21.5), (0.0, 5.2, -25.0), 1.2, tongue_fn)
    f.capsule("tongue_3", (0.0, 5.2, -25.0), (0.0, 3.6, -28.0), 1.0, tongue_fn)


def part_body(p: Puppet, f: Field) -> None:
    """体腔：扁圆的体块（腹面浅色带菱斑、背面橄榄褐斑块、顶面苔斑），按 z 分胸 / 腰 / 臀三骨。"""
    f.ellipsoid("body", (0.0, 8.4, 0.5), (11.6, 8.4, 13.2), skin_fn)
    f.ellipsoid("body", (0.0, 5.0, 1.5), (10.4, 4.6, 11.4), skin_fn)          # 鼓出来下坠的肚子：腹面更低更宽，贴地


WARTS = (  # (x, z, 半径)：右半坐标；中线上的 x 取 1 以内
    (2.5, -6.5, 2.2), (6.0, -3.5, 2.6), (3.0, 0.0, 2.4), (7.2, 2.5, 2.2), (2.0, 5.5, 2.6), (5.5, 7.5, 2.0),
    (8.4, -0.5, 1.8), (0.8, -2.5, 1.6), (0.8, 3.2, 1.8), (4.4, -9.0, 1.6), (8.0, 6.0, 1.8), (6.4, 10.0, 1.6),
    (9.4, -6.0, 1.5), (4.0, 3.2, 1.4), (1.6, 9.5, 1.5), (7.6, 9.0, 1.4), (3.6, -4.2, 1.2), (9.6, 2.5, 1.3), (5.0, 12.0, 1.3),
    (2.0, -10.5, 1.4), (7.4, -8.2, 1.4), (10.0, 6.5, 1.2),
)


def part_warts(p: Puppet, f: Field) -> None:
    """背上的疣：十几粒半埋进背里的鼓包（深浅两色，个别带苔），分属胸 / 腰 / 臀三骨。"""
    for n, (x, z, r) in enumerate(WARTS):
        top = f.top_y(int(x), int(z), ("body",))
        if top is None:
            continue
        fn = (lambda i, j, k, n=n: "moss" if blotch(i, j, k, 2, 17 + n) < 20 else ("wart_lt" if blotch(i, j, k, 2, 19 + n) < 38 else "wart"))
        f.ellipsoid("body", (x, top + 0.2, z), (r, r * 0.85, r), fn)


HERBS = (("herb_a_r", 3, -5, 4), ("herb_b_r", 5, 4, 5), ("herb_c_r", 1, 8, 4))     # (骨, x, z, 高)


def part_herbs(p: Puppet, f: Field) -> None:
    """灵草丛：每丛三根细茎（1px）从背上长出来，顶端一朵小白花（十字 5 格）；背上另有一根冒出来的枯枝（斜着向外上方）。"""
    for bone, x, z, h in HERBS:
        for dx, dz, dh in ((0, 0, 0), (1, 1, -1), (0, 2, -2)):
            top = f.top_y(x + dx, z + dz, ("body",))
            if top is None:
                continue
            for dy in range(1, h + dh + 1):
                f.put(x + dx, top + dy, z + dz, bone, "herb")
            ty = top + h + dh + 1
            f.put(x + dx, ty, z + dz, bone, "herb_flower")
            for ox, oz in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                f.put(x + dx + ox, ty, z + dz + oz, bone, "herb_flower")
    for x in range(0, 11, 2):                       # 苔丛：背上一团团 1~3 格高的毛绒苔，疣顶和背脊上最密
        for z in range(-14, 13, 2):
            if blotch(x, 0, z, 2, 31) >= 34:
                continue
            top = f.top_y(x, z, ("body",))
            if top is None or top < 12:
                continue
            for dy in range(1, 1 + 1 + blotch(x, 0, z, 2, 33) // 40):
                f.put(x, top + dy, z, "body", "moss_lt" if dy > 1 else "moss")
    base = f.top_y(6, -1, ("body",))
    if base is not None:
        for step in range(6):
            f.put(6 + step // 2, base + 1 + step, -1, "twig_r", "twig")


PART_FUNCS = {"legs": part_legs, "head": part_head, "tongue": part_tongue, "body": part_body, "warts": part_warts, "herbs": part_herbs}
BUILD_ORDER = ("legs", "head", "tongue", "body", "warts", "herbs")           # 先肢体和头，再体腔（先到先得），最后背上的小件


def build(parts) -> Puppet:
    p = new_puppet()
    f = new_field(p)
    for fn in sorted(parts, key=lambda fn: BUILD_ORDER.index(fn.__name__.removeprefix("part_"))):
        fn(p, f)
    f.emit()
    return p


GATES = kit.Gates("腐沼蟾 / fu_zhao_chan", MATS, asym=frozenset(), soft_over=frozenset(), min_bones=MIN_BONES,
                  build_small=lambda: build([part_tongue]))

VIEWS = (
    ("FRONT 正视（相机在 -z，看脸 / 凸眼 / 长舌）", 180.0, 8.0),
    ("3/4 前右斜视（相机在 -z / +x 上方）", 150.0, 22.0),
    ("LEFT 左侧视（相机在 -x，头朝画面左）", 90.0, 0.0),
    ("RIGHT 右侧视（相机在 +x，头朝画面右）", -90.0, 0.0),
    ("TOP 俯视（相机在 +y，头朝画面下方，看背上疣 / 苔 / 灵草）", 180.0, 89.9),
    ("BACK 背视（相机在 +z，看后肢 / 臀）", 0.0, 8.0),
)
PART_VIEWS = (("FRONT -z", 180.0, 8.0), ("3/4 前右", 150.0, 22.0), ("TOP +y", 180.0, 89.9))
FRAMING_VIEWS = (("3/4 左前（yaw120）", 120.0, 18.0), ("LEFT 侧（头朝左）", 90.0, 4.0))


def full_run() -> None:
    p = build(list(PART_FUNCS.values()))
    GATES.self_test(p)
    total = GATES.run(p)
    if total:
        raise SystemExit(f"门禁仍有 {total} 处违例，不出图")
    model = CREATURE_DIR / "FuZhaoChan.bbmodel"
    kit.write_model(p, model, "FuZhaoChan")
    print(f"✓ {model}: {p.total()} cubes / {len(p.rig.bones)} bones")
    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    tmp = CREATURE_DIR / "_tmp_part.bbmodel"
    try:
        kit.render_part_previews(PART_FUNCS, lambda fns: build(fns + ([part_body] if fns[0] in (part_warts, part_herbs) else [])),
                                  tmp, PARTS_DIR, "FuZhaoChan", PART_VIEWS)
    finally:
        tmp.unlink(missing_ok=True)
    kit.make_check(p, BONES, model, REVIEW_DIR, MODEL_ID, "fu_zhao_chan（腐沼蟾）round 3/3", VIEWS, MIN_BONES, total,
                   ref_crop=(0.0, 0.0, 1.0, 0.52), framing_views=FRAMING_VIEWS, tree_columns=2)


# ───────────────────────────────────────────────────────────── 像素贴图（画笔库 handheld_texture：toad_* / wet_* 是本只补的）
TEXTURE_MATERIAL = {
    "skin_dk": "toad_skin_dk", "skin": "toad_skin", "skin_lt": "toad_skin_lt",
    "moss": "moss", "moss_lt": "moss_light",
    "belly": "belly_pale", "belly_spot": "belly_seg",
    "wart": "toad_wart", "wart_lt": "toad_wart_lt",
    "eye": "toad_eye", "eye_ring": "hide_dark", "pupil": "void",
    "mouth": "wet_mouth", "tongue": "wet_tongue", "tongue_dk": "wet_tongue_dk",
    "toe": "hide_dark", "toe_pad": "hide_light",
    "herb": "spirit_green", "herb_flower": "moss_flower", "twig": "wood_dark",
}
TEX_VIEWS = (("3/4 FRONT-RIGHT", 150.0, 22.0), ("FRONT -z（脸 / 凸眼 / 舌）", 180.0, 8.0), ("TOP +y（疣 / 苔 / 灵草）", 180.0, 89.9))


def textured_run() -> None:
    p = build(list(PART_FUNCS.values()))
    kit.render_check_tex(p, BONES, MATS, REVIEW_DIR, CREATURE_DIR, MODEL_ID, MODEL_ID, TEXTURE_MATERIAL, TEX_VIEWS, "FuZhaoChan_tex.bbmodel")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--part", choices=sorted(PART_FUNCS))
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--gates", action="store_true")
    ap.add_argument("--tex", action="store_true", help="只出像素贴图版 FuZhaoChan_tex.bbmodel 与 check_tex.png")
    args = ap.parse_args()
    if args.tex:
        textured_run()
        return
    if not (args.part or args.self_test or args.gates):
        full_run()
        return
    p = build([PART_FUNCS[args.part]] if args.part else list(PART_FUNCS.values()))
    if args.self_test:
        GATES.self_test(p)
    elif args.gates:
        GATES.run(p)
    else:
        out = CREATURE_DIR / f"_tmp_{args.part}.bbmodel"
        out.write_text(json.dumps(p.rig.bbmodel("FuZhaoChan")), encoding="utf-8")
        print(f"{p.total()} cubes → {out}")


if __name__ == "__main__":
    main()
