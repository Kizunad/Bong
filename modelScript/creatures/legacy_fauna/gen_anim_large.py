"""大型旧生物 / Boss 补动画：黑虎、活柱、毒龙、骨龙、黑武士（Round 1）。

只往 client 的 ``<id>.animation.json`` 追加缺的段，已有段（含黑武士技能）一帧不改。
骨骼名与枢轴取自 ``geo/<id>.geo.json``；关键帧数值在这里从零写，不抄别的生物。

用法::

    python3 modelScript/creatures/legacy_fauna/gen_anim_large.py            # 全部
    python3 modelScript/creatures/legacy_fauna/gen_anim_large.py dark_tiger # 只一只
"""
from __future__ import annotations

import math
import sys

from large_anim_kit import Clip, append_clips, tau

# 开工前各文件已有的段短名；新段与之重名即报错。
EXISTING = {
    "dark_tiger": {"idle", "melee_lion1", "walk"},
    "living_pillar": {"idle", "attack", "big1.5"},
    "poison_dragon": {"idle", "attack", "walk"},
    "bone_dragon": {"idle", "attack", "fly"},
    "heiwushi": {"dark_barrage", "dark_vortex", "transform", "skill1", "skill2", "skill3", "skill4", "idle"},
}

S = math.sin
C = math.cos


# ───────────────────────── 黑虎 dark_tiger ─────────────────────────
# 骨架：ALL（地面原点，不旋转）> Air_origin（躯干，静止旋转 X90）> Body_back（后躯：后腿、尾）
#                                                          └ Body_front > Chest（颈肩：头、鬃毛、前腿）
# 所有腿/脊柱/尾巴的静止旋转都只绕 X，因此动画里绕 X 的旋转 = 绕世界 X 轴的俯仰。
# 朝向 -Z（头在 -Z）。ALL 的 position 是世界系位移，所以整体前冲/起伏都写在 ALL 上。

def tiger_run() -> Clip:
    """奔跑：四蹄腾空-收拢的二拍跳跃步态，脊柱一屈一伸，尾巴向后拖直。"""

    clip = Clip(0.56, loop=True)
    clip.wave("ALL", "position", lambda p: (0, 2.2 * (1 + S(tau(p) + 0.6)), 0), 12)
    clip.wave("Air_origin", "rotation", lambda p: (5 * S(tau(p) - 0.4), 0, 0), 12)
    clip.wave("Body_front", "rotation", lambda p: (7 * S(tau(p) + 0.9), 0, 0), 12)
    clip.wave("Chest", "rotation", lambda p: (-4 * S(tau(p) + 0.9), 0, 0), 12)
    clip.wave("head2", "rotation", lambda p: (-4 + 3 * S(tau(p) + 2.0), 0, 0), 12)
    # 奔跑时嘴微张，随腾空收拢一开一合（绕 X 正方向 = 下颌尖往下 = 张嘴）
    clip.wave("jaw", "rotation", lambda p: (7 + 4 * S(tau(p) + 1.0), 0, 0), 12)
    for side, lag in (("R", 0.04), ("L", -0.04)):
        # 后腿成对蹬地（相位 0.0），前腿成对前伸（相位 0.5 之后），左右只差一点点
        clip.wave(f"Leg_{side}_back", "rotation", lambda p, g=lag: (-8 + 46 * S(tau(p + g)), 0, 0), 12)
        clip.wave(f"Leg_{side}_back_lower", "rotation", lambda p, g=lag: (14 + 26 * S(tau(p + g) - 1.3), 0, 0), 12)
        clip.wave(f"Foot_{side}_back", "rotation", lambda p, g=lag: (8 + 18 * S(tau(p + g) - 2.2), 0, 0), 12)
        clip.wave(f"Leg_{side}_front", "rotation", lambda p, g=lag: (-4 + 48 * S(tau(p + g + 0.42)), 0, 0), 12)
        clip.wave(f"Leg_{side}_front_lower", "rotation",
                  lambda p, g=lag: (10 + 30 * S(tau(p + g + 0.42) - 1.6), 0, 0), 12)
        clip.wave(f"Foot_{side}_front", "rotation",
                  lambda p, g=lag: (-2 + 14 * S(tau(p + g + 0.42) - 2.4), 0, 0), 12)
    for index, name in enumerate(("Tail", "Tail_lower", "Tail_lower_2", "Tail_tip")):
        clip.wave(name, "rotation", lambda p, i=index: (-6 + 7 * S(tau(p) - 0.8 - 0.5 * i), 0, 0), 12)
    clip.wave("Ear_L", "rotation", lambda p: (0, 0, -10 + 3 * S(tau(p))), 8)
    clip.wave("Ear_R", "rotation", lambda p: (0, 0, 10 - 3 * S(tau(p))), 8)
    return clip


def tiger_attack() -> Clip:
    """扑咬：后坐蓄力（前身抬起、双爪高举、张嘴）→ 整体前冲 + 双爪下劈 + 合颌 → 收势。

    正 X = 腿向后摆 / 头颈向下；ALL 的 -Z 是前方。
    """

    clip = Clip(0.9, loop=False)
    clip.pos("ALL", {0: (0, 0, 0), 0.26: (0, 2.2, 7), 0.4: (0, 2.5, -15), 0.5: (0, 1.5, -13), 0.7: (0, 2.2, -6.5), 0.9: (0, 0, 0)})
    clip.rot("Air_origin", {0: (0, 0, 0), 0.26: (-8, 0, 0), 0.4: (9, 0, 0), 0.9: (0, 0, 0)})
    clip.rot("Body_front", {0: (0, 0, 0), 0.26: (-16, 0, 0), 0.4: (14, 0, 0), 0.9: (0, 0, 0)})
    clip.rot("Chest", {0: (0, 0, 0), 0.26: (-14, 0, 0), 0.4: (20, 0, 0), 0.9: (0, 0, 0)})
    clip.rot("head2", {0: (0, 0, 0), 0.26: (-10, 0, 0), 0.4: (12, 0, 0), 0.9: (0, 0, 0)})
    clip.rot("jaw", {0: (0, 0, 0), 0.2: (28, 0, 0), 0.4: (38, 0, 0), 0.47: (0, 0, 0), 0.9: (0, 0, 0)})
    # 双爪：蓄力时高举在胸前（负=向前上），0.4 秒劈到身下（正=向后下），左右爪错开 0.03 秒
    for side, lag in (("L", 0.0), ("R", 0.03)):
        clip.rot(f"Leg_{side}_front", {0: (0, 0, 0), 0.26: (-62, 0, 0), 0.4 + lag: (38, 0, 0), 0.55: (6, 0, 0), 0.9: (0, 0, 0)})
        clip.rot(f"Leg_{side}_front_lower", {0: (0, 0, 0), 0.26: (-20, 0, 0), 0.4 + lag: (24, 0, 0), 0.9: (0, 0, 0)})
        clip.rot(f"Foot_{side}_front", {0: (0, 0, 0), 0.26: (12, 0, 0), 0.4 + lag: (-18, 0, 0), 0.9: (0, 0, 0)})
        clip.rot(f"Leg_{side}_back", {0: (0, 0, 0), 0.26: (12, 0, 0), 0.4: (-16, 0, 0), 0.9: (0, 0, 0)})
        clip.rot(f"Leg_{side}_back_lower", {0: (0, 0, 0), 0.26: (-8, 0, 0), 0.4: (10, 0, 0), 0.9: (0, 0, 0)})
    clip.rot("Ear_L", {0: (0, 0, 0), 0.26: (-14, 0, 0), 0.9: (0, 0, 0)})
    clip.rot("Ear_R", {0: (0, 0, 0), 0.26: (-14, 0, 0), 0.9: (0, 0, 0)})
    clip.rot("Tail", {0: (0, 0, 0), 0.26: (-22, 0, 0), 0.4: (14, 0, 0), 0.9: (0, 0, 0)})
    clip.rot("Tail_lower", {0: (0, 0, 0), 0.26: (-12, 0, 0), 0.45: (10, 0, 0), 0.9: (0, 0, 0)})
    clip.rot("Tail_tip", {0: (0, 0, 0), 0.3: (-14, 0, 0), 0.5: (12, 0, 0), 0.9: (0, 0, 0)})
    return clip


def tiger_hurt() -> Clip:
    """受击：被打得头颈后仰、整体往后一挫，张嘴吃痛，尾巴甩一下，0.45 秒回到静止。"""

    clip = Clip(0.45, loop=False)
    clip.pos("ALL", {0: (0, 0, 0), 0.08: (0, 1.2, 4.5), 0.45: (0, 0, 0)})
    clip.rot("Air_origin", {0: (0, 0, 0), 0.08: (-3, 0, 5), 0.25: (1, 0, -3), 0.45: (0, 0, 0)})
    clip.rot("Body_front", {0: (0, 0, 0), 0.08: (-9, 0, 0), 0.45: (0, 0, 0)})
    clip.rot("Chest", {0: (0, 0, 0), 0.08: (-12, 0, 0), 0.45: (0, 0, 0)})
    clip.rot("head2", {0: (0, 0, 0), 0.08: (-14, 0, 0), 0.3: (-4, 0, 0), 0.45: (0, 0, 0)})
    clip.rot("jaw", {0: (0, 0, 0), 0.08: (26, 0, 0), 0.3: (10, 0, 0), 0.45: (0, 0, 0)})
    clip.rot("Ear_L", {0: (0, 0, 0), 0.08: (-16, 0, 0), 0.45: (0, 0, 0)})
    clip.rot("Ear_R", {0: (0, 0, 0), 0.08: (-16, 0, 0), 0.45: (0, 0, 0)})
    for side in ("L", "R"):
        clip.rot(f"Leg_{side}_front", {0: (0, 0, 0), 0.08: (14, 0, 0), 0.45: (0, 0, 0)})
        clip.rot(f"Leg_{side}_back", {0: (0, 0, 0), 0.08: (-8, 0, 0), 0.45: (0, 0, 0)})
    clip.rot("Tail", {0: (0, 0, 0), 0.1: (-20, 0, 0), 0.3: (6, 0, 0), 0.45: (0, 0, 0)})
    clip.rot("Tail_tip", {0: (0, 0, 0), 0.12: (-18, 0, 0), 0.32: (10, 0, 0), 0.45: (0, 0, 0)})
    return clip


def tiger_death() -> Clip:
    """死亡：挨一下后前腿先软，身体晃两晃，侧身砸倒在地，下颌松开、尾巴瘫下，终态保持。"""

    clip = Clip(2.0, loop=False)
    # position 的 X 与世界 X 反号（GeckoLib 约定）：+X 把身体往世界 -X 拉，抵消侧倒时质心外移的 ~23px
    clip.pos("ALL", {0: (0, 0, 0), 0.15: (0, 0, 4), 0.55: (0, 1.5, 3), 1.0: (10, 6, 1), 1.3: (23, 14, 0),
                     1.45: (23, 13, 0), 2.0: (23, 13, 0)})
    clip.rot("ALL", {0: (0, 0, 0), 0.15: (0, 0, 2), 0.55: (0, 0, -6), 0.85: (0, 0, 5), 1.3: (0, 0, -90),
                     1.45: (0, 0, -86), 2.0: (0, 0, -88)})
    clip.rot("Chest", {0: (0, 0, 0), 0.15: (-12, 0, 0), 0.7: (18, 0, 0), 1.3: (28, 0, 0), 2.0: (30, 0, 0)})
    clip.rot("head2", {0: (0, 0, 0), 0.15: (-14, 0, 0), 0.7: (16, 0, 0), 1.3: (24, 0, 0), 2.0: (22, 0, 0)})
    clip.rot("jaw", {0: (0, 0, 0), 0.15: (28, 0, 0), 0.7: (14, 0, 0), 1.3: (30, 0, 0), 2.0: (36, 0, 0)})
    clip.rot("Ear_L", {0: (0, 0, 0), 0.7: (-12, 0, 0), 2.0: (-20, 0, 0)})
    clip.rot("Ear_R", {0: (0, 0, 0), 0.7: (-12, 0, 0), 2.0: (-20, 0, 0)})
    for side in ("L", "R"):
        clip.rot(f"Leg_{side}_front", {0: (0, 0, 0), 0.55: (-26, 0, 0), 1.0: (-34, 0, 0), 1.45: (-22, 0, 0), 2.0: (-24, 0, 0)})
        clip.rot(f"Leg_{side}_front_lower", {0: (0, 0, 0), 0.55: (34, 0, 0), 1.0: (40, 0, 0), 2.0: (32, 0, 0)})
        clip.rot(f"Leg_{side}_back", {0: (0, 0, 0), 0.55: (12, 0, 0), 1.0: (26, 0, 0), 1.45: (14, 0, 0), 2.0: (16, 0, 0)})
        clip.rot(f"Leg_{side}_back_lower", {0: (0, 0, 0), 0.55: (14, 0, 0), 1.0: (22, 0, 0), 2.0: (16, 0, 0)})
    clip.rot("Tail", {0: (0, 0, 0), 0.4: (-18, 0, 0), 1.3: (10, 0, 0), 2.0: (16, 0, 0)})
    clip.rot("Tail_lower", {0: (0, 0, 0), 0.5: (-10, 0, 0), 1.4: (12, 0, 0), 2.0: (14, 0, 0)})
    clip.rot("Tail_tip", {0: (0, 0, 0), 0.6: (-8, 0, 0), 1.5: (10, 0, 0), 2.0: (12, 0, 0)})
    return clip


def tiger_clips() -> dict[str, Clip]:
    return {"run": tiger_run(), "attack": tiger_attack(), "hurt": tiger_hurt(), "death": tiger_death()}


# ───────────────────────── 活柱 living_pillar ─────────────────────────
# 一根约 300 单位高的活体肉柱。骨架是一条竖直的链，**没有静止旋转**，所以：
#   绕 X = 前后弯（正 = 向 -Z 前倾），绕 Z = 左右弯，绕 Y = 扭转。
# bone2 是整根柱子的底座（地面枢轴），bone14/15/16 是挂在底座两侧的须根，bone13 是柱身下段向前探出的口器。
PILLAR_SEGMENTS = ("bone4", "bone3", "bone5", "bone6", "bone7", "bone8", "bone9", "r", "bone10",
                   "bone11", "bone17", "bone12")
PILLAR_ROOTS = ("bone14", "bone15", "bone16")


def pillar_walk() -> Clip:
    """原地蠕动：底座一缩一伸，一道弯折从根部沿柱身向上滚（相位逐节后移），须根交替扒地。

    活柱不移动，``walk`` 只是被客户端按水平速度触发时的「挪动」姿态，所以位置不动、只做体内的波。
    """

    clip = Clip(1.6, loop=True)
    steps = 16
    clip.wave("bone2", "position", lambda p: (0, 1.4 + 2.4 * (1 + S(tau(2 * p))), 0), steps)  # 只往上抬，不压进地面
    clip.wave("bone2", "scale", lambda p: (1 + 0.025 * S(tau(2 * p) + 1.0), 1 - 0.02 * S(tau(2 * p)), 1 + 0.025 * S(tau(2 * p) + 1.0)), steps)
    clip.wave("bone2", "rotation", lambda p: (2.5 * S(tau(p)), 0, 0), steps)
    for index, name in enumerate(PILLAR_SEGMENTS):
        reach = 2.2 + 0.45 * index  # 越往上摆幅越大
        clip.wave(name, "rotation",
                  lambda p, i=index, a=reach: (a * S(tau(p) - 0.5 * i), 0.8 * S(tau(p) - 0.5 * i + 1.0), 0.45 * a * S(tau(p) - 0.5 * i + 1.6)),
                  steps)
    clip.wave("bone13", "rotation", lambda p: (9 * S(tau(2 * p) - 0.4) + 6, 0, 0), steps)
    for index, name in enumerate(PILLAR_ROOTS):
        clip.wave(name, "rotation", lambda p, i=index: (0, 0, 8 * S(tau(p) + 2.1 * i)), steps)
    return clip


def pillar_hurt() -> Clip:
    """受击：一记冲击波从根部沿柱身向上传，每节先被顶弯再反弹，顶端甩得最狠；口器抽搐张开。"""

    clip = Clip(0.6, loop=False)
    clip.rot("bone2", {0: (0, 0, 0), 0.07: (-4, 0, 2), 0.25: (1.5, 0, -1), 0.6: (0, 0, 0)})
    clip.pos("bone2", {0: (0, 0, 0), 0.07: (0, 3.5, 0), 0.25: (0, 1, 0), 0.6: (0, 0, 0)})
    for index, name in enumerate(PILLAR_SEGMENTS):
        hit = 0.05 + 0.016 * index  # 冲击波到达该节的时刻
        kick = 4.0 + 0.9 * index
        clip.rot(name, {0: (0, 0, 0), hit: (0, 0, 0), hit + 0.08: (-kick, 1.5, 0.35 * kick),
                        hit + 0.2: (0.45 * kick, -1, -0.2 * kick), hit + 0.34: (-0.12 * kick, 0, 0.05 * kick),
                        0.6: (0, 0, 0)})
    clip.rot("bone13", {0: (0, 0, 0), 0.06: (-22, 0, 0), 0.22: (10, 0, 0), 0.6: (0, 0, 0)})
    for index, name in enumerate(PILLAR_ROOTS):
        clip.rot(name, {0: (0, 0, 0), 0.08: (0, 0, 12 * (1 - 2 * (index % 2))), 0.3: (0, 0, -4 * (1 - 2 * (index % 2))), 0.6: (0, 0, 0)})
    return clip


def pillar_death() -> Clip:
    """死亡：先在原地剧烈抖动，根部一软整根柱子向前倒下，顶端被甩得最远，砸地后各节瘫软松开，终态保持。"""

    clip = Clip(2.6, loop=False)
    shake = {0: 0, 0.15: 4, 0.3: -5, 0.45: 6, 0.6: -6, 0.75: 7, 0.9: -4}
    clip.rot("bone2", {0: (0, 0, 0), **{t: (0, 0, v * 0.5) for t, v in shake.items()},
                       1.0: (6, 0, 0), 1.2: (28, 0, 0), 1.45: (78, 0, 0), 1.6: (92, 0, 0), 1.75: (86, 0, 0),
                       1.95: (89, 0, 0), 2.6: (88, 0, 0)})
    clip.pos("bone2", {0: (0, 0, 0), 1.0: (0, 1, 0), 1.2: (0, 7, 0), 1.45: (0, 18, 0), 1.6: (0, 18.5, 0), 2.6: (0, 18.5, 0)})
    for index, name in enumerate(PILLAR_SEGMENTS):
        # 抖动时每节只带一点侧摆，12 节叠起来才是柱顶的大幅晃动；摔地后各节的前后弯要小，
        # 否则累计角度会把柱顶卷进地里
        sway = 0.25 * (0.6 + 0.1 * index)
        lag = 0.02 * index
        clip.rot(name, {0: (0, 0, 0),
                        0.15: (0, 0, shake[0.15] * sway), 0.45: (0, 0, shake[0.45] * sway),
                        0.75: (0, 0, shake[0.75] * sway), 0.9: (0, 0, 0),
                        1.2 + lag: (1.0 + 0.2 * index, 0, 0), 1.6 + lag: (-1.5 - 0.15 * index, 0, 0.8 * (index % 3 - 1)),
                        1.95 + lag: (-0.3 - 0.05 * index, 0, 0), 2.6: (-0.7 - 0.05 * index, 0, 0.6 * (index % 2 * 2 - 1))})
    clip.rot("bone13", {0: (0, 0, 0), 0.5: (14, 0, 0), 1.0: (-18, 0, 0), 1.6: (30, 0, 0), 2.6: (34, 0, 0)})
    for index, name in enumerate(PILLAR_ROOTS):
        side = 1 - 2 * (index % 2)
        clip.rot(name, {0: (0, 0, 0), 0.6: (0, 0, 10 * side), 1.2: (0, 0, -16 * side), 1.8: (0, 0, 30 * side),
                        2.6: (0, 0, 34 * side)})
    return clip


def pillar_clips() -> dict[str, Clip]:
    return {"walk": pillar_walk(), "hurt": pillar_hurt(), "death": pillar_death()}


# ───────────────────────── 毒龙 poison_dragon / 骨龙 bone_dragon ─────────────────────────
# 两只龙共用同一套骨架（geo 骨名、枢轴逐一相同），所以共用一套构造函数，只在参数上区分。
# 链：bone > pelvis（腰胯，同时是后腿、尾、腰的根）> waist > waist2 > chest（前臂、双翼、颈的根）> neck > neck2 > neck_3 > head > chin
# 主链没有静止旋转：绕 X = 俯仰（正 = 前端下压 / 腿脚向后），绕 Y = 偏航，绕 Z = 侧倾。朝向 -Z。
# 例外：chin 的静止旋转是 X+60（嘴默认张开 60°），所以「闭嘴」要写成 chin 绕 X 约 -50。
# 双翼静止是水平展开的 T 字：翼根绕 Y 向后扫 = 收拢，绕 Z（左 -、右 +）= 抬翼。
DRAGON_TAIL = ("tail", "tail_1", "tail_2", "tail_3", "tail_4", "tail_5", "tail_6", "tail_7", "tail_8")
DRAGON_NECK = ("neck", "neck2", "neck_3", "head")
DRAGON_SIDES = {
    "R": dict(hip="right_leg_4", knee="right_leg_5", ankle="right_leg_6", foot="right_foot",
              shoulder="right_arm_4", elbow="right_arm_5", wrist="right_arm_6", hand="right_hand",
              wings=("right_wings_4", "right_wings_5", "right_wings_6"), sign=-1),
    "L": dict(hip="left_leg_1", knee="left_leg_2", ankle="left_leg_3", foot="left_foot",
              shoulder="left_arm_1", elbow="left_arm_2", wrist="left_arm_3", hand="left_hand",
              wings=("left_wings_1", "left_wings_2", "left_wings_3"), sign=1),
}
DRAGON_JAW_CLOSED = -48.0  # chin 绕 X：闭嘴
DRAGON_JAW_OPEN = 4.0


def wing_values(side: dict, fold: float, lift: float) -> tuple[tuple, tuple, tuple]:
    """翼三节在「收拢程度 fold（1 = 完全折在背上，0 = 全展）」和抬翼角 lift 下的 (X, Y, Z) 旋转。"""

    s = side["sign"]
    # 实测：左翼绕 Z 取负才是抬翼（与直觉相反），所以这里 lift 为正 = 抬起 = Z 取负
    return ((0, -64 * s * fold, -(lift + 16 * fold) * s),
            (0, -74 * s * fold, 0),
            (0, -52 * s * fold, 0))


def dragon_wings(clip: Clip, fold, lift, steps: int | None = None, once_times: tuple = ()) -> None:
    """给双翼三节写轨道。循环段传 ``steps``（fold/lift 为相位函数），一次性段传 ``once_times``（fold/lift 为按时刻查的字典）。"""

    for side in DRAGON_SIDES.values():
        for index, bone in enumerate(side["wings"]):
            if steps:
                clip.wave(bone, "rotation", lambda p, i=index, sd=side: wing_values(sd, fold(p), lift(p))[i], steps)
            else:
                clip.rot(bone, {t: wing_values(side, fold[t], lift[t])[index] for t in once_times})


def dragon_walk() -> Clip:
    """地面行走：对角步态（右后腿与左前臂同相），腰脊反向扭、尾巴逐节后摆成行波，翼收在背上。"""

    clip = Clip(0.96, loop=True)
    steps = 16
    clip.wave("pelvis", "position", lambda p: (0, 3.6 + 3.4 * (1 + S(tau(2 * p) + 1.2)), 0), steps)
    clip.wave("pelvis", "rotation", lambda p: (0.8 * S(tau(2 * p)), 0, 2.2 * S(tau(p))), steps)
    clip.wave("waist", "rotation", lambda p: (0, -3.0 * S(tau(p)), 0), steps)
    clip.wave("waist2", "rotation", lambda p: (0, -2.4 * S(tau(p)), 0), steps)
    clip.wave("chest", "rotation", lambda p: (0.6 * S(tau(2 * p) + 1.0), -2.0 * S(tau(p)), -2.0 * S(tau(p))), steps)
    for hind, fore in (("R", "L"), ("L", "R")):
        phase = 0.0 if hind == "R" else 0.5
        h, f = DRAGON_SIDES[hind], DRAGON_SIDES[fore]

        def swing(p, ph=phase):
            return S(tau(p + ph)), max(0.0, -C(tau(p + ph)))  # 髋摆动、抬腿（只在前摆段）

        clip.wave(h["hip"], "rotation", lambda p, sw=swing: (21 * sw(p)[0], 0, 0), steps)
        clip.wave(h["knee"], "rotation", lambda p, sw=swing: (7 + 30 * sw(p)[1], 0, 0), steps)
        clip.wave(h["ankle"], "rotation", lambda p, sw=swing: (-4 - 16 * sw(p)[1], 0, 0), steps)
        clip.wave(h["foot"], "rotation", lambda p, sw=swing: (-11 * sw(p)[0] - 10 * sw(p)[1], 0, 0), steps)
        clip.wave(f["shoulder"], "rotation", lambda p, sw=swing: (-20 * sw(p)[0], 0, 0), steps)  # 前臂与同侧后腿反相
        clip.wave(f["elbow"], "rotation", lambda p, sw=swing: (6 - 26 * sw(p)[1], 0, 0), steps)
        clip.wave(f["wrist"], "rotation", lambda p, sw=swing: (-4 + 12 * sw(p)[1], 0, 0), steps)
        clip.wave(f["hand"], "rotation", lambda p, sw=swing: (10 * sw(p)[0] + 8 * sw(p)[1], 0, 0), steps)
    for index, name in enumerate(DRAGON_TAIL):
        clip.wave(name, "rotation", lambda p, i=index: (1.4 * S(tau(2 * p) - 0.4 * i), (3.0 + 0.55 * i) * S(tau(p) - 0.42 * i), 0), steps)
    for index, name in enumerate(DRAGON_NECK):
        clip.wave(name, "rotation",
                  lambda p, i=index: ((-1.5 if i < 3 else 2.5) + 2.6 * S(tau(2 * p) + 0.9 - 0.5 * i), 3.0 * S(tau(p) - 0.3 * i + 0.6), 0), steps)
    clip.wave("chin", "rotation", lambda p: (DRAGON_JAW_CLOSED + 2.5 * S(tau(2 * p)), 0, 0), steps)
    clip.scale("tongue", {0: (1, 1, 0.2), 0.96: (1, 1, 0.2)})
    dragon_wings(clip, lambda p: 0.92 + 0.04 * S(tau(2 * p)), lambda p: 2.0 * S(tau(2 * p) + 0.4), steps=steps)
    return clip


def dragon_hurt(rattle: float) -> Clip:
    """受击：整条龙被打得往后一挫，脖子与头猛地后仰、张嘴，前臂抬起，双翼被惊得张开一下再收拢，尾巴甩开。

    ``rattle`` > 0 时（骨龙）再叠一串逐渐衰减的左右抖动——骨架被打散架的碰撞感。
    """

    clip = Clip(0.55, loop=False)
    hit, back = 0.08, 0.55
    clip.pos("pelvis", {0: (0, 0, 0), hit: (0, 8, 5), 0.25: (0, 3, 1.5), back: (0, 0, 0)})
    clip.rot("pelvis", {0: (0, 0, 0), hit: (-2, 0, 1.5), back: (0, 0, 0)})
    clip.rot("chest", {0: (0, 0, 0), hit: (-9, 0, 0), 0.3: (-2, 0, 0), back: (0, 0, 0)})
    waist2 = {0: (0, 0, 0), back: (0, 0, 0)}
    if rattle:
        for index in range(1, 10):
            waist2[0.04 * index] = (0, 0, rattle * (1 - 0.1 * index) * (1 if index % 2 else -1))
    clip.rot("waist2", waist2)
    for index, name in enumerate(DRAGON_NECK):
        clip.rot(name, {0: (0, 0, 0), hit: (-(15 + 3 * index), 2 * (index % 2), 0), 0.3: (-(4 + index), 0, 0), back: (0, 0, 0)})
    clip.rot("chin", {0: (DRAGON_JAW_CLOSED, 0, 0), hit: (DRAGON_JAW_OPEN + 6, 0, 0),
                      0.35: (DRAGON_JAW_CLOSED + 16, 0, 0), back: (DRAGON_JAW_CLOSED, 0, 0)})
    clip.scale("tongue", {0: (1, 1, 0.2), hit: (1, 1, 0.9), 0.35: (1, 1, 0.3), back: (1, 1, 0.2)})
    for side in DRAGON_SIDES.values():
        clip.rot(side["shoulder"], {0: (0, 0, 0), hit: (-20, 0, 0), back: (0, 0, 0)})
        clip.rot(side["elbow"], {0: (0, 0, 0), hit: (24, 0, 0), back: (0, 0, 0)})
        clip.rot(side["hip"], {0: (0, 0, 0), hit: (-6, 0, 0), back: (0, 0, 0)})
    folds = {0: 0.92, hit: 0.4, 0.3: 0.7, back: 0.92}
    lifts = {0: 0.0, hit: 16.0, 0.3: 6.0, back: 0.0}
    dragon_wings(clip, folds, lifts, once_times=tuple(folds))
    for index, name in enumerate(DRAGON_TAIL):
        lash = 5 + 1.6 * index
        clip.rot(name, {0: (0, 0, 0), 0.1 + 0.012 * index: (0, lash, 0), 0.28 + 0.012 * index: (0, -0.5 * lash, 0), back: (0, 0, 0)})
    return clip


def dragon_death() -> Clip:
    """死亡：先被打得后仰嘶吼，前腿先塌、后腿坐下，胸腹砸地，头颈砸在地上，双翼松垮摊开，尾巴瘫软，终态保持。"""

    clip = Clip(3.2, loop=False)
    end = 3.2
    clip.pos("pelvis", {0: (0, 0, 0), 0.25: (0, 3, 5), 0.9: (0, 1, 3), 1.5: (0, -22, 0), 2.2: (0, -25, 0), end: (0, -25, 0)})
    clip.rot("pelvis", {0: (0, 0, 0), 0.25: (-2, 0, 1.5), 0.9: (-3, 0, 3), 1.5: (-4, 0, 2), end: (-4, 0, 2)})
    clip.rot("chest", {0: (0, 0, 0), 0.25: (-10, 0, 0), 0.9: (6, 0, -3), 1.5: (8, 0, -4), end: (8, 0, -4)})
    clip.rot("waist", {0: (0, 0, 0), 0.9: (3, 4, 0), 1.5: (4, 7, 0), end: (4, 7, 0)})
    for index, name in enumerate(DRAGON_NECK):
        clip.rot(name, {0: (0, 0, 0), 0.25: (-(16 + 3 * index), 0, 0), 0.9: (2 + index, 6 * (index % 2), 0),
                        1.5: (-3 + 0.5 * index, 8 + 2 * index, 0), 2.2: (-3.5 + 0.5 * index, 10 + 2 * index, 0),
                        end: (-3.5 + 0.5 * index, 10 + 2 * index, 0)})
    clip.rot("chin", {0: (DRAGON_JAW_CLOSED, 0, 0), 0.25: (DRAGON_JAW_OPEN + 8, 0, 0), 0.9: (-22, 0, 0),
                      1.5: (-36, 0, 0), end: (-34, 0, 0)})  # 终态下颌松垮半张，下颌尖搭在地上
    clip.scale("tongue", {0: (1, 1, 0.2), 0.25: (1, 1, 0.9), 1.5: (1, 1, 1.0), end: (1, 1, 1.0)})
    for side in DRAGON_SIDES.values():
        sign = side["sign"]
        clip.rot(side["shoulder"], {0: (0, 0, 0), 0.25: (-18, 0, 0), 0.9: (-34, 0, -6 * sign), 1.5: (-52, 0, -12 * sign), end: (-52, 0, -12 * sign)})
        clip.rot(side["elbow"], {0: (0, 0, 0), 0.25: (22, 0, 0), 0.9: (70, 0, 0), 1.5: (122, 0, 0), end: (122, 0, 0)})
        clip.rot(side["wrist"], {0: (0, 0, 0), 0.9: (-14, 0, 0), 1.5: (-60, 0, 0), end: (-60, 0, 0)})
        clip.rot(side["hip"], {0: (0, 0, 0), 0.25: (-6, 0, 0), 0.9: (-30, 0, 0), 1.5: (-62, 0, 0), end: (-62, 0, 0)})
        clip.rot(side["knee"], {0: (0, 0, 0), 0.9: (26, 0, 0), 1.5: (92, 0, 0), end: (92, 0, 0)})
        clip.rot(side["ankle"], {0: (0, 0, 0), 0.9: (-8, 0, 0), 1.5: (-52, 0, 0), end: (-52, 0, 0)})
        clip.rot(side["foot"], {0: (0, 0, 0), 1.5: (20, 0, 0), end: (20, 0, 0)})
    folds = {0: 0.92, 0.25: 0.3, 0.9: 0.38, 1.5: 0.12, 2.2: 0.1, end: 0.1}
    lifts = {0: 0.0, 0.25: 20.0, 0.9: 6.0, 1.5: -2.0, 2.2: -4.0, end: -4.0}
    dragon_wings(clip, folds, lifts, once_times=tuple(folds))
    for index, name in enumerate(DRAGON_TAIL):
        side = 1 if index % 2 else -1
        clip.rot(name, {0: (0, 0, 0), 0.4 + 0.02 * index: (0, 7 + index, 0), 1.5: (0.4, 6 + 1.2 * index, 0),
                        end: (0.4, 8 + 1.4 * index, 0)})
    return clip


def poison_clips() -> dict[str, Clip]:
    return {"hurt": dragon_hurt(0.0), "death": dragon_death()}


def bone_clips() -> dict[str, Clip]:
    return {"walk": dragon_walk(), "hurt": dragon_hurt(5.0), "death": dragon_death()}


# ───────────────────────── 黑武士 heiwushi ─────────────────────────
# 类人骨架：global（脚底原点）> body（躯干：头、双臂、背后剑）；双腿直接挂在 global 下。
# 双臂 / 双腿绕 X：负 = 向前（抬臂 / 迈腿），正 = 向后。绕 Z 是外展，绕 Y 是扭转。
# 只写 walk / attack / hurt / death，已有的 dark_barrage / dark_vortex / transform / skill1-4 一律不碰；
# 背后的剑（beiSword*）与臂上的剑刃（rightArmSword / leftArmSword 链）保持静止姿态，不进新段。

def heiwushi_walk() -> Clip:
    """行走：四肢对摆，躯干随步伐轻微上下起伏和扭转，头部反向稳住。"""

    clip = Clip(0.8, loop=True)
    steps = 16
    clip.wave("body", "position", lambda p: (0, 0.9 * (1 + abs(S(tau(p)))), 0), steps)
    # 腿绕髋摆动时方形脚底的后角会下沉 ~1px，整体抬 1.2 抵消
    clip.track("global", "position", {0: (0, 1.2, 0), 0.8: (0, 1.2, 0)})
    clip.wave("body", "rotation", lambda p: (2.0, 4.0 * S(tau(p)), 0), steps)
    clip.wave("head", "rotation", lambda p: (-1.5, -3.0 * S(tau(p)), 0), steps)
    clip.wave("rightLeg", "rotation", lambda p: (32 * S(tau(p)), 0, 0), steps)
    clip.wave("leftLeg", "rotation", lambda p: (-32 * S(tau(p)), 0, 0), steps)
    clip.wave("rightArm", "rotation", lambda p: (-28 * S(tau(p)), 0, 2), steps)
    clip.wave("leftArm", "rotation", lambda p: (28 * S(tau(p)), 0, -2), steps)
    return clip


def heiwushi_attack() -> Clip:
    """斩击：右臂高举蓄力、身体后仰拧转 → 前踏一步、右臂自上而下劈落、身体前压 → 收势。"""

    clip = Clip(0.8, loop=False)
    clip.rot("body", {0: (0, 0, 0), 0.26: (-10, 18, 0), 0.4: (16, -14, 0), 0.58: (8, -6, 0), 0.8: (0, 0, 0)})
    clip.rot("head", {0: (0, 0, 0), 0.26: (6, -10, 0), 0.4: (-8, 8, 0), 0.8: (0, 0, 0)})
    clip.rot("rightArm", {0: (0, 0, 0), 0.26: (-165, 0, -10), 0.4: (-22, 0, 4), 0.58: (-12, 0, 2), 0.8: (0, 0, 0)})
    clip.rot("leftArm", {0: (0, 0, 0), 0.26: (-35, 0, -22), 0.4: (18, 0, -12), 0.8: (0, 0, 0)})
    clip.rot("leftLeg", {0: (0, 0, 0), 0.26: (14, 0, 0), 0.4: (-34, 0, 0), 0.58: (-26, 0, 0), 0.8: (0, 0, 0)})
    clip.rot("rightLeg", {0: (0, 0, 0), 0.26: (-8, 0, 0), 0.4: (26, 0, 0), 0.58: (20, 0, 0), 0.8: (0, 0, 0)})
    clip.pos("body", {0: (0, 0, 0), 0.26: (0, 0.5, 1.5), 0.4: (0, -1.5, -3), 0.8: (0, 0, 0)})
    clip.pos("global", {0: (0, 0, 0), 0.26: (0, 1.2, 0), 0.58: (0, 1.2, 0), 0.8: (0, 0, 0)})
    return clip


def heiwushi_hurt() -> Clip:
    """受击：上身被打得后仰，头甩向一侧，双臂本能外张，腿跟着踉跄一步。"""

    clip = Clip(0.4, loop=False)
    clip.rot("body", {0: (0, 0, 0), 0.07: (-13, 5, 3), 0.22: (-4, -2, 0), 0.4: (0, 0, 0)})
    clip.rot("head", {0: (0, 0, 0), 0.07: (-16, 14, 0), 0.4: (0, 0, 0)})
    clip.rot("rightArm", {0: (0, 0, 0), 0.07: (-24, 0, -26), 0.4: (0, 0, 0)})
    clip.rot("leftArm", {0: (0, 0, 0), 0.07: (-18, 0, 24), 0.4: (0, 0, 0)})
    clip.rot("rightLeg", {0: (0, 0, 0), 0.07: (16, 0, 0), 0.4: (0, 0, 0)})
    clip.rot("leftLeg", {0: (0, 0, 0), 0.07: (-12, 0, 0), 0.4: (0, 0, 0)})
    clip.pos("body", {0: (0, 0, 0), 0.07: (0, 0, 2), 0.4: (0, 0, 0)})
    clip.pos("global", {0: (0, 0, 0), 0.07: (0, 1.2, 0), 0.4: (0, 0, 0)})
    return clip


def heiwushi_death() -> Clip:
    """死亡：踉跄两步，膝一软整个人仰面倒下，双臂摊开、双腿微分，终态保持。"""

    clip = Clip(1.7, loop=False)
    clip.rot("global", {0: (0, 0, 0), 0.2: (-6, 0, 0), 0.6: (4, 0, 3), 0.95: (-24, 0, 0), 1.25: (-92, 0, 0),
                        1.4: (-88, 0, 0), 1.7: (-90, 0, 0)})
    clip.pos("global", {0: (0, 0, 0), 0.6: (0, 1.6, 1), 0.95: (0, 0, 6), 1.25: (0, 4.6, 14), 1.7: (0, 4.6, 14)})
    clip.rot("body", {0: (0, 0, 0), 0.2: (-12, 6, 0), 0.6: (8, -6, 0), 1.25: (0, 0, 0), 1.7: (0, 0, 0)})
    clip.rot("head", {0: (0, 0, 0), 0.2: (-18, 10, 0), 0.95: (12, 0, 0), 1.25: (-14, 6, 0), 1.7: (-10, 12, 0)})
    clip.rot("rightArm", {0: (0, 0, 0), 0.2: (-26, 0, -26), 0.95: (-40, 0, -30), 1.25: (-10, 0, -62), 1.7: (-6, 0, -66)})
    clip.rot("leftArm", {0: (0, 0, 0), 0.2: (-20, 0, 24), 0.95: (-34, 0, 30), 1.25: (-14, 0, 58), 1.7: (-8, 0, 62)})
    clip.rot("rightLeg", {0: (0, 0, 0), 0.2: (12, 0, 0), 0.6: (-24, 0, 0), 0.95: (-34, 0, 0), 1.25: (-4, 0, -8), 1.7: (-2, 0, -10)})
    clip.rot("leftLeg", {0: (0, 0, 0), 0.2: (-8, 0, 0), 0.6: (28, 0, 0), 0.95: (22, 0, 0), 1.25: (2, 0, 8), 1.7: (4, 0, 11)})
    return clip


def heiwushi_clips() -> dict[str, Clip]:
    return {"walk": heiwushi_walk(), "attack": heiwushi_attack(), "hurt": heiwushi_hurt(), "death": heiwushi_death()}


BUILDERS = {
    "heiwushi": heiwushi_clips,
    "dark_tiger": tiger_clips,
    "living_pillar": pillar_clips,
    "poison_dragon": poison_clips,
    "bone_dragon": bone_clips,
}


def main() -> None:
    names = sys.argv[1:] or list(BUILDERS)
    for creature in names:
        written = append_clips(creature, BUILDERS[creature](), EXISTING[creature])
        print(f"{creature}: 追加 {len(written)} 段 → {', '.join(w.rsplit('.', 1)[-1] for w in written)}")


if __name__ == "__main__":
    main()
