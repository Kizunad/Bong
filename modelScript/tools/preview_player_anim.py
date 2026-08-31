#!/usr/bin/env python3
"""把 PlayerAnimator (Emotecraft v3) 关键帧动画**渲在真 bbmodel 上**，可带手持物。

## 为什么要有这个工具

`client/tools/render_animation.py` 画的是**火柴人**（它自己的 docstring 第一句就是
"Render ... as stick-figure views"，几何是硬编码的 `CUBOIDS`），而且**手里没有东西**。
拿它去调「匕首横划时肘不能伸直、刀要贴身」这类判据等于闭着眼睛做：看不到刀，就
判不出刀尖扫过哪里、会不会插进自己大腿、guard 位在 FPV 里遮不遮视线。

这里换成 bbmodel 全链路：

    真玩家 bbmodel（分段肢体）+ 手持物 bbmodel → 逐 tick 骨骼变换 → render_bbmodel

## 「适配 bbmodel 的动画机制」具体指什么

bendy-lib 在运行时把肢体 cuboid **从 bend_center 劈成两半、只转下半**
（`docs/player-animation-conventions.md §10.2`）。所以 bbmodel 这边**肢体必须建成
两段骨**：上臂 / 前臂各一个 element，前臂绕肘转。整条手臂做成一个 element 画不出
弯肘——只会整条一起转，那就退回成火柴人的表达力了。腿同理（膝）。头/躯干不可弯，
单段。这也正是 Blockbench 里给这套模型做动画时该有的骨架。

## 坐标系

动画 JSON 用 MC ModelPart 空间（+X = 玩家左、**+Y = 下**、+Z = 后）；
bbmodel/Bedrock 用 y 朝上、脚在 0。映射是 `bedrock = (x, 24 - y, z)`。

逐点验过：头枢轴 ModelPart (0,0,0) → Bedrock (0,24,0)；腿 (1.9,12,0) → (1.9,12,0)；
臂 (-5,2,0) → (-5,22,0)——与 `preview_armor_on_body.PLAYER` 的枢轴表完全吻合。

y 轴翻转**翻手性**，所以 ModelPart 空间的旋转矩阵搬过来要夹成 `S·R·S`
（`S = diag(1,-1,1)`）。漏掉这层的症状是 pitch 方向全反：抬手渲成垂手。

## 已知近似

手持物摆位用 model JSON 的 `thirdperson_righthand` rotation/translation/scale
**近似**还原，不是 MC `ItemRenderer` 的精确 display transform（那套还要过
`MatrixStack` 一串 push/pop 和 `HeldItemRenderer` 自己的手臂补偿）。用途是判
「刀在不在该在的地方、扫过哪里」，不是像素级对位；真机手位以 `/anim test <id>` 为准。

## 用法

    python3 modelScript/tools/preview_player_anim.py \\
        client/src/main/resources/assets/bong/player_animation/dagger_slash.json \\
        --hold modelScript/models/StoneKnife.bbmodel --ticks 0,3,5,6,8
"""

from __future__ import annotations

import argparse
import base64
import io
import json
import math
import sys
import uuid
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

LIB = Path(__file__).resolve().parents[1]
from bbmodel_maker import workspace  # noqa: E402

_WS = workspace.Workspace.discover(start=Path(__file__))
REPO = _WS.root
for _d in (LIB / "tools", REPO / "client" / "tools"):
    sys.path.insert(0, str(_d))

import anim_common as AC  # noqa: E402  关节解剖判据的唯一定义处
import render_animation as RA  # noqa: E402  复用它已验证的 PlayerAnimator/bendy 数学
from bbmodel_maker.workbench.preview_armor_on_body import make_player_skin  # noqa: E402
from bbmodel_maker.render.render_bbmodel import _load_texture, load_bbmodel, render  # noqa: E402

NS = uuid.UUID("6b1d0f3a-2c47-4e58-9a10-77c3f9e0b542")
S_FLIP = np.diag([1.0, -1.0, 1.0])        # ModelPart(y↓) ↔ Bedrock(y↑) 的手性夹层
ATLAS = 128                               # 合并图集：玩家皮肤 64² + 手持物贴图右移 64
VIEWS = (("FRONT", 180.0, 6.0), ("SIDE", 96.0, 6.0), ("3/4", 146.0, 14.0))

# 分段肢体：每肢拆上/下两段，切点 = bendy-lib 的 bend_center（肢长 12 的一半）。
# (element 名, ModelPart 枢轴, from(相对枢轴), size, 皮肤 uv)
SEGMENTS = (
    ("head", (0.0, 0.0, 0.0), (-4, -8, -4), (8, 8, 8), (0, 0)),
    ("torso", (0.0, 0.0, 0.0), (-4, 0, -2), (8, 12, 4), (16, 16)),
    ("rightArm_up", (-5.0, 2.0, 0.0), (-3, -2, -2), (4, 6, 4), (40, 16)),
    ("rightArm_lo", (-5.0, 2.0, 0.0), (-3, 4, -2), (4, 6, 4), (40, 22)),
    ("leftArm_up", (5.0, 2.0, 0.0), (-1, -2, -2), (4, 6, 4), (32, 48)),
    ("leftArm_lo", (5.0, 2.0, 0.0), (-1, 4, -2), (4, 6, 4), (32, 54)),
    ("rightLeg_up", (-1.9, 12.0, 0.0), (-2, 0, -2), (4, 6, 4), (0, 16)),
    ("rightLeg_lo", (-1.9, 12.0, 0.0), (-2, 6, -2), (4, 6, 4), (0, 22)),
    ("leftLeg_up", (1.9, 12.0, 0.0), (-2, 0, -2), (4, 6, 4), (16, 48)),
    ("leftLeg_lo", (1.9, 12.0, 0.0), (-2, 6, -2), (4, 6, 4), (16, 54)),
)
PIVOT_OF = {s[0]: s[1] for s in SEGMENTS}
PART_OF = {s[0]: s[0].split("_")[0] for s in SEGMENTS}
BODY_PIVOT = np.array([0.0, 12.0, 0.0])   # 腰点：body.* 的整体旋转绕它


# ── 基础变换 ──────────────────────────────────────────────────────────────


def _pt(p) -> np.ndarray:
    """ModelPart 点 → Bedrock 点。"""
    return np.array([p[0], 24.0 - p[1], p[2]], dtype=float)


def _rot(R: np.ndarray) -> np.ndarray:
    """ModelPart 旋转 → Bedrock 旋转（y 翻转翻手性，夹 S·R·S）。"""
    return S_FLIP @ R @ S_FLIP


def _aff(R: np.ndarray, t: np.ndarray) -> np.ndarray:
    m = np.eye(4)
    m[:3, :3] = R
    m[:3, 3] = t
    return m


def _about(R: np.ndarray, pivot: np.ndarray) -> np.ndarray:
    """绕定点旋转：T(p)·R·T(-p)。"""
    return _aff(R, pivot - R @ pivot)


def _axis_rot(axis: np.ndarray, angle: float) -> np.ndarray:
    axis = axis / (np.linalg.norm(axis) or 1.0)
    x, y, z = axis
    c, s, C = np.cos(angle), np.sin(angle), 1.0 - np.cos(angle)
    return np.array([
        [x * x * C + c, x * y * C - z * s, x * z * C + y * s],
        [y * x * C + z * s, y * y * C + c, y * z * C - x * s],
        [z * x * C - y * s, z * y * C + x * s, z * z * C + c],
    ])


def body_matrix(kfs, tick: float, body_disp_scale: float = 1.0) -> np.ndarray:
    """`body.*` 的整体变换，Bedrock 空间。肢体和手持物都要左乘它。"""
    body = RA.sample_part(kfs, "body", tick)
    body_R = _rot(RA.part_rotation_matrix(body["pitch"], body["yaw"], body["roll"]))
    # body.x/y/z 是位移不是点，只翻 y 的符号
    body_t = np.array([body["x"], -body["y"], body["z"]], float) * body_disp_scale
    return _aff(np.eye(3), body_t) @ _about(body_R, BODY_PIVOT)


def segment_transforms(kfs, tick: float, body_disp_scale: float = 1.0) -> dict[str, np.ndarray]:
    """每个分段 element 在 Bedrock 世界空间的 4×4 刚体变换。

    组合顺序照 `render_animation.solve_skeleton` 的口径（body → part → bend），
    区别是那边只留关节点坐标，这里保留整条旋转链——摆 bbmodel 需要姿态不只是位置。
    """
    body_m = body_matrix(kfs, tick, body_disp_scale)

    out: dict[str, np.ndarray] = {}
    for name in PIVOT_OF:
        part = PART_OF[name]
        p = RA.sample_part(kfs, part, tick)
        pivot_b = _pt(np.array(PIVOT_OF[name], float)
                      + np.array([p["x"], p["y"], p["z"]], float))
        R_part = _rot(RA.part_rotation_matrix(p["pitch"], p["yaw"], p["roll"]))
        seg = _about(R_part, pivot_b)

        if name.endswith("_lo"):
            # 下段：上段之后再绕 bend_center 转 bend 角。bendy-lib 的轴向量在
            # cuboid 局部是 (cos(axis), 0, sin(axis))，只作用于靠 hand/foot 那半。
            centre_b = _pt(np.array(PIVOT_OF[name], float) + RA.bend_center(part))
            # bend/axis 已经是**弧度**（emote 头 degrees:false，`sample_axis` 原样返回，
            # 上面的 part_rotation_matrix 也是直接吃原值）。这里曾多套一层 np.radians，
            # 把角度缩掉约 57 倍 —— 肘和膝在预览里等于焊死，"肘不伸直"这类判据全失效。
            a = float(p["axis"])
            axis_b = S_FLIP @ np.array([np.cos(a), 0.0, np.sin(a)], float)
            # **角度取负**。`_pt` 把 y 翻过来（MC 的 +Y 朝下 → Bedrock 的 +Y 朝上），
            # 这是个反射，会反转旋转的旋向：S·R(a,θ)·S = R(S·a, −θ)。part 旋转那边
            # 用 `_rot` 做了完整共轭所以自带这个负号；bend 这条是直接在 Bedrock 空间
            # 建轴的，而该轴 y 分量恒为 0、S_FLIP 对它不起作用，负号就这么丢了。
            # 症状：肘和膝**朝反方向折**——前臂往身后翻、小腿往身前踢。
            # 由 test_anim_preview_fidelity 与参考实现 `RA.bent_end_local` 逐点对拍锁死。
            # 渲染侧也拦一道。授权侧（anim_common）拦的是"动画写错了"，这里拦的是
            # "变换算错了"——本文件历史上就把旋向做反过（见上面那段注释），当时肘往
            # 身后翻、膝往身前踢，两个关节同时反，而图上只表现为"姿势别扭"。
            AC.assert_joint_fold_is_anatomical(
                part, math.degrees(float(p["bend"])), math.degrees(a),
                where=f"预览 tick {tick:g} / {name}")
            seg = seg @ _about(_axis_rot(axis_b, -float(p["bend"])), centre_b)

        out[name] = body_m @ seg
    return out


# ── MC 手持物挂点 ─────────────────────────────────────────────────────────
# 下面三个常数逐字抄自运行时，不是估的：
#   R_ATTACH / HAND_OFFSET_PX  ← `HeldItemFeatureRenderer.renderItem`（1.20.1）
#       setArmAngle(arm) → Rx(-90) → Ry(180) → translate(±1/16, 0.125, -0.625)
#   ITEM_BEND_PIVOT_PX         ← PlayerAnimator `HeldItemMixin`（同一方法的 mixin，
#       注在 ordinal=0 的 mulPose 之前，即 Rx(-90) 之前）：
#       translate(0, 0.25, 0) → rotateAxis(bend, (cos(-axis), 0, sin(-axis)))
#       → translate(0, -0.25, 0)。手持物**跟着肘弯走**，不是钉在直臂手位。
#       注意它取的枢轴是 (0,4,0)，比 cuboid 真 bend_center 的 (-1,4,0) 差 1px —— 这
#       是库自己的近似，照抄，别"修正"，否则预览和游戏对不上。
#   BLOCK_CENTRE_PX            ← `ItemRenderer.renderItem` 在 display 变换之后的
#       translate(-0.5,-0.5,-0.5)。SML 的 `ObjUnbakedModelModel.emitVertex` 只做
#       「-0.5 → blockstate 旋转 → +0.5」，**不重定心**，所以 OBJ/bbmodel 的
#       (0,0,0) 落在方块角，display 变换的枢轴是 (8,8,8)px。
R_ATTACH = RA.rot_x(np.radians(-90.0)) @ RA.rot_y(np.radians(180.0))
HAND_OFFSET_PX = np.array([1.0, 2.0, -10.0])      # 右手；左手 x 取负
ITEM_BEND_PIVOT_PX = np.array([0.0, 4.0, 0.0])
BLOCK_CENTRE_PX = np.array([8.0, 8.0, 8.0])
# `_pt` 的仿射形式。手持物这条链**整条在 ModelPart 空间里算完**再用它过桥，
# 就不会重蹈「bend 轴 y 分量为 0、S_FLIP 对它不起作用 → 旋向静默丢负号」那个坑。
A_TO_BEDROCK = np.array([[1.0, 0.0, 0.0, 0.0],
                         [0.0, -1.0, 0.0, 24.0],
                         [0.0, 0.0, 1.0, 0.0],
                         [0.0, 0.0, 0.0, 1.0]])


ITEM_PART_OF = {True: "rightItem", False: "leftItem"}


def collect_keyframes(emote: dict) -> dict:
    """`RA.collect_keyframes` + **手持物那两根骨头**。

    库里的 `bbmodel_maker.rig.emote_anim.collect_keyframes` 按 `BODY_PART_NAMES`
    七个身体部件过滤，`rightItem` / `leftItem` 被静默丢掉——而 PlayerAnimator 认它们
    （`HeldItemMixin.changeItemLocation`）。丢掉的后果不是"少一点细节"：**正握 /
    反握的区别整个消失**，预览与门禁都会把一条根本没换握的动画读成合格的换握
    （见 `anim_common.ITEM_PARTS` 注释）。

    返回值是同一个 kfs 结构，只是多两个键。`RA.sample_axis` 查不到键就回落到
    `default_axis_value` = 0，所以对没有手持物骨头的另外 150 条动画完全等价。
    """
    kfs = RA.collect_keyframes(emote)
    for move in emote["moves"]:
        tick = int(move["tick"])
        easing = move.get("easing", "linear")
        for part, axes in move.items():
            if part not in AC.ITEM_PARTS or not isinstance(axes, dict):
                continue
            for axis, value in axes.items():
                if axis not in AC.ITEM_AXES:
                    continue
                kfs.setdefault(part, {}).setdefault(axis, []).append(
                    (tick, float(value), easing))
    for part in AC.ITEM_PARTS:
        for axis_list in kfs.get(part, {}).values():
            axis_list.sort(key=lambda row: row[0])
    return kfs


def item_bone_matrix(kfs, tick: float, right: bool = True) -> np.ndarray:
    """`rightItem` / `leftItem` 那根骨头在手持物局部系里的 3×3。

    逐字对齐 `HeldItemMixin.changeItemLocation`：

        mulPose(Axis.ZP.rotation(rot.z))   // roll
        mulPose(Axis.YP.rotation(rot.y))   // yaw
        mulPose(Axis.XP.rotation(rot.x))   // pitch

    合成即 `Rz(roll)·Ry(yaw)·Rx(pitch)`，与身体部件的
    `part_rotation_matrix` 同构（`get3DTransform(..., ROTATION, ...)` 返回的
    x/y/z 就是 pitch/yaw/roll，见 `KeyframeAnimationPlayer.get3DTransform`）。
    """
    part = ITEM_PART_OF[bool(right)]
    return RA.part_rotation_matrix(
        RA.sample_axis(kfs, part, "pitch", tick),
        RA.sample_axis(kfs, part, "yaw", tick),
        RA.sample_axis(kfs, part, "roll", tick),
    )


def item_attach_modelpart(kfs, tick: float, display: dict, right: bool = True) -> np.ndarray:
    """手持物模型 px 坐标 → ModelPart 空间（含肩枢轴平移），逐步对齐 MC 调用序。

    完整链（每个 translate 都在它前面那些旋转之后的局部系里）：

        T(肩枢轴) · R_arm
        · T(0,4,0) · R_bend · T(0,-4,0)       PlayerAnimator HeldItemMixin
        · R_ATTACH · T(±1, 2, -10)            HeldItemFeatureRenderer
        · R_item                               PlayerAnimator HeldItemMixin（rightItem）
        · T(display.translation) · R_disp · S  ItemRenderer / Transformation.apply
        · T(-8,-8,-8)                          方块中心重定心

    历史上这里错了四处，合起来让刀飘在拳头外 5.8px（一个拳头才 4px 宽）：
    ① 整条 `R_ATTACH` 没有 —— 刀的朝向从根上就不是游戏里那个；
    ② display 的 translation 被当成 `R_disp · t` 来加，而 MC 是**先平移再旋转**；
    ③ 挂点用 `limb_end_local`（-1,10,0）近似，真值是 R_ATTACH·(1,2,-10)=(-1,10,-2)；
    ④ 少了 `T(-8,-8,-8)`，等于默认模型原点就是 display 枢轴。
    这四条都由 `test_anim_preview_fidelity.HeldItemAttachTest` 钉死。
    """
    part = RA.sample_part(kfs, "rightArm" if right else "leftArm", tick)
    pivot = (np.array(PIVOT_OF["rightArm_lo" if right else "leftArm_lo"], float)
             + np.array([part["x"], part["y"], part["z"]], float))
    R_arm = RA.part_rotation_matrix(part["pitch"], part["yaw"], part["roll"])

    axis = float(part["axis"])
    R_bend = RA.rotate_about_axis(
        np.array([np.cos(-axis), 0.0, np.sin(-axis)]), float(part["bend"]))

    rx, ry, rz = display.get("rotation", [0, -90, 55])
    # JOML `Quaternionf.rotationXYZ(x,y,z)` = Rx·Ry·Rz（见 render_held_item.py）。
    R_disp = (RA.rot_x(np.radians(rx)) @ RA.rot_y(np.radians(ry))
              @ RA.rot_z(np.radians(rz)))
    scale = np.diag(display.get("scale", [0.8, 0.8, 0.8]))
    # display.translation 的 JSON 数值就是 px（解析时 ×1/16 转方块，这里全程 px）
    trans = np.array(display.get("translation", [0, 4, 0]), float)
    hand = HAND_OFFSET_PX * (1.0 if right else np.array([-1.0, 1.0, 1.0]))

    return (_aff(np.eye(3), pivot)
            @ _aff(R_arm, np.zeros(3))
            @ _about(R_bend, ITEM_BEND_PIVOT_PX)
            @ _aff(R_ATTACH, np.zeros(3))
            @ _aff(np.eye(3), hand)
            @ _aff(item_bone_matrix(kfs, tick, right), np.zeros(3))
            @ _aff(np.eye(3), trans)
            @ _aff(R_disp @ scale, np.zeros(3))
            @ _aff(np.eye(3), -BLOCK_CENTRE_PX))


def hand_transform(kfs, tick: float, display: dict,
                   body_disp_scale: float = 1.0) -> np.ndarray:
    """右手手持物在 Bedrock 世界空间的 4×4。

    `body_m · A · M_MP`：`A` 是 ModelPart→Bedrock 的过桥仿射，而 `body_m` 已经是
    Bedrock 空间的共轭形式（`body_m = A·M_body·A⁻¹`），所以它留在最左边。
    """
    return (body_matrix(kfs, tick, body_disp_scale)
            @ A_TO_BEDROCK
            @ item_attach_modelpart(kfs, tick, display))


# ── bbmodel 组装 ──────────────────────────────────────────────────────────


def _faces(size, uv, u_off=0):
    """面 UV 排布逐字对齐 `preview_armor_on_body._faces`（那份已验证过）。

    注意 **必须吃 size 而不是 hi-lo**：y 翻转后 hi/lo 在 y 上会互换，逐分量取
    min/max 之前 hi-lo 是负的，拿去算 UV 会得到一堆负宽矩形——症状是整块渲成黑。
    """
    u, v = uv[0] + u_off, uv[1]
    dx, dy, dz = size
    return {
        "north": {"uv": [u + dz, v + dz, u + dz + dx, v + dz + dy], "texture": 0},
        "south": {"uv": [u + 2 * dz + dx, v + dz, u + 2 * (dz + dx), v + dz + dy], "texture": 0},
        "west": {"uv": [u, v + dz, u + dz, v + dz + dy], "texture": 0},
        "east": {"uv": [u + dz + dx, v + dz, u + 2 * dz + dx, v + dz + dy], "texture": 0},
        "up": {"uv": [u + dz, v, u + dz + dx, v + dz], "texture": 0},
        "down": {"uv": [u + dz + dx, v, u + dz + 2 * dx, v + dz], "texture": 0},
    }


def build_scene(out_path: Path, held: Path | None) -> tuple[Path, dict[str, str], list[str]]:
    """写一份「分段玩家 + 可选手持物」的 bbmodel。

    `load_bbmodel` 只读 `textures[0]`，所以两份贴图并进一张 128² 图集：玩家皮肤占
    左上 64²，手持物贴图整体右移 64（沿用 `preview_armor_on_body` 的同一手法）。
    """
    atlas = Image.new("RGBA", (ATLAS, ATLAS), (0, 0, 0, 0))
    atlas.paste(make_player_skin().convert("RGBA"), (0, 0))

    elements, ids, held_ids = [], {}, []
    for name, pivot, frm, size, uv in SEGMENTS:
        lo_mp = [pivot[i] + frm[i] for i in range(3)]
        hi_mp = [lo_mp[i] + size[i] for i in range(3)]
        a, b = _pt(lo_mp), _pt(hi_mp)
        # **逐分量** min/max：_pt 只翻 y，整体互换会让 x/z 变成负尺寸盒，
        # 而负 size 的 cube 在 render_bbmodel 里会翻面（渲成黑块）。
        lo, hi = np.minimum(a, b), np.maximum(a, b)
        eid = str(uuid.uuid5(NS, f"player/{name}"))
        ids[name] = eid
        elements.append({
            "name": name, "box_uv": False, "type": "cube", "uuid": eid,
            "from": [round(v, 4) for v in lo], "to": [round(v, 4) for v in hi],
            "autouv": 0, "color": 0, "origin": [0.0, 0.0, 0.0],
            "rescale": False, "locked": False, "render_order": "default",
            "allow_mirror_modeling": True,
            "faces": _faces(size, uv),
        })

    if held is not None:
        doc = json.loads(held.read_text(encoding="utf-8"))
        # **必须走 `_load_texture`**，不能无条件 base64 解码。bbmodel 的贴图有两种存法：
        # 内嵌 data URI，或指向磁盘的相对路径（Blockbench 的 "linked" 贴图）。仓库 55 个
        # bbmodel 里 11 个是后者，硬解 base64 会报 `binascii.Error: Incorrect padding`
        # ——一个和"贴图找不到"毫无关系的错。`render_bbmodel` 那边早就修了，这里曾是
        # 同一个 bug 的**第二个调用点**，漏修的后果是 `--hold` 挂在那 11 个模型上。
        tex = Image.fromarray(_load_texture(doc["textures"][0], held).astype(np.uint8), "RGBA")
        atlas.paste(tex, (64, 0))
        for e in doc["elements"]:
            e = dict(e)
            e["uuid"] = str(uuid.uuid5(NS, f"held/{e['name']}"))
            e["faces"] = {
                k: {"uv": [f["uv"][0] + 64, f["uv"][1], f["uv"][2] + 64, f["uv"][3]],
                    "texture": 0}
                for k, f in e["faces"].items()
            }
            held_ids.append(e["uuid"])
            elements.append(e)

    buf = io.BytesIO()
    atlas.save(buf, format="PNG")
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps({
        "meta": {"format_version": "4.10", "model_format": "free", "box_uv": False},
        "name": "player_anim_preview",
        "model_identifier": "geometry.bong.player_anim_preview",
        "visible_box": [4, 4, 2],
        "resolution": {"width": ATLAS, "height": ATLAS},
        "elements": elements,
        "outliner": [e["uuid"] for e in elements],
        "textures": [{
            "path": "", "name": "scene.png", "folder": "entity", "namespace": "bong",
            "id": "0", "width": ATLAS, "height": ATLAS,
            "uv_width": ATLAS, "uv_height": ATLAS,
            "particle": False, "render_mode": "default", "visible": True,
            "mode": "bitmap", "saved": False,
            "uuid": str(uuid.uuid5(NS, "scene/tex")),
            "source": "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode(),
        }],
    }, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    return out_path, ids, held_ids


def _fit_focus(kfs, display, scene, ids, held_ids, end, samples=17, margin=1.10):
    """扫全段取联合 AABB。分开算每帧会抖；只看首末帧会在挥砍中段裁掉刀尖。"""
    lo = np.full(3, np.inf)
    hi = np.full(3, -np.inf)
    for i in range(samples):
        tick = end * i / max(1, samples - 1)
        seg = segment_transforms(kfs, tick)
        xform = {ids[n]: m for n, m in seg.items()}
        if held_ids:
            hm = hand_transform(kfs, tick, display)
            for hid in held_ids:
                xform[hid] = hm
        tris, _, _, _ = load_bbmodel(scene, xform=xform)
        v = np.array([p for vs, _, _ in tris for p in vs])
        lo = np.minimum(lo, v.min(0))
        hi = np.maximum(hi, v.max(0))
    center = (lo + hi) / 2
    # span 取 xy 里较大的那个：render 按单一 scale 等比缩放，取 max 才不会裁掉。
    span = float(max(hi[0] - lo[0], hi[1] - lo[1])) * margin
    return center, span


def _frame(args, kfs, display, scene, ids, held_ids, focus, tick):
    """一个 tick 的三视图横排。GIF 和静态网格共用，保证两者画的是同一套变换。"""
    seg = segment_transforms(kfs, tick)
    xform = {ids[n]: m for n, m in seg.items()}
    if held_ids:
        hm = hand_transform(kfs, tick, display)
        for hid in held_ids:
            xform[hid] = hm
    return [(label, render(scene, yaw=yaw, pitch=pitch, size=args.size,
                           xform=xform, focus=focus, shading="mc")[0])
            for label, yaw, pitch in VIEWS]


def _end_tick(emote) -> float:
    """emote 的 `endTick`。**必须传剥好的 emote 子对象**，不是整份文档。

    没有这个键就抛，不给默认值：静默兜底正是上面那个 bug 能活这么久的原因。
    """
    if "endTick" not in emote:
        raise KeyError(
            "emote 里没有 endTick —— 多半是把整份 JSON 文档当 emote 传进来了，"
            "应当先 `doc.get(\"emote\", doc)` 剥一层")
    return float(emote["endTick"])


def _write_gif(args, emote, kfs, display, scene, ids, held_ids, focus):
    end = _end_tick(emote)
    n = max(2, int(round(end * args.subdiv)))
    gap, lab = 8, 16
    w = args.size * len(VIEWS) + gap * (len(VIEWS) + 1) + 54
    h = args.size + lab + gap * 2

    frames = []
    for i in range(n):
        tick = end * i / n            # 不含 end：末帧==首帧时循环会顿一拍
        tiles = _frame(args, kfs, display, scene, ids, held_ids, focus, tick)
        canvas = Image.new("RGB", (w, h), (16, 17, 20))
        draw = ImageDraw.Draw(canvas)
        draw.text((6, h // 2), f"t{tick:4.1f}", fill=(232, 232, 224))
        x = 54
        for label, img in tiles:
            draw.text((x + 3, gap), label, fill=(198, 198, 190))
            canvas.paste(img, (x, gap + lab))
            x += args.size + gap
        frames.append(canvas.convert("P", palette=Image.ADAPTIVE, colors=192))

    per = max(20, int(round(50.0 / args.subdiv / args.speed)))
    out = args.out or (LIB / "out" / f"{args.json.stem}.gif")
    out.parent.mkdir(parents=True, exist_ok=True)
    frames[0].save(out, save_all=True, append_images=frames[1:],
                   duration=per, loop=0, disposal=2, optimize=False)
    print(f"{out}  {n} 帧 / {per}ms 每帧 / 循环 {n * per}ms（原速 {end * 50:.0f}ms）")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("json", type=Path, help="player_animation JSON")
    ap.add_argument("--hold", type=Path, default=None, help="手持物 bbmodel（挂右手）")
    ap.add_argument("--display", type=Path, default=None,
                    help="手持物的 model JSON，取 thirdperson_righthand；缺省用 axe_bone 基线")
    ap.add_argument("--ticks", default="0,3,5,6,8")
    ap.add_argument("--size", type=int, default=260)
    ap.add_argument("--out", type=Path, default=None)
    ap.add_argument("--gif", action="store_true",
                    help="出逐帧 GIF 而不是静态网格。静态图看得出姿势对不对，"
                         "看不出动作节奏——错峰、overshoot、收势是否连得上只有动起来才判得了")
    ap.add_argument("--subdiv", type=int, default=3,
                    help="每 tick 插几帧（GIF 用）。MC 一 tick 50ms，只按整 tick 采样"
                         "会把缓动曲线抽没")
    ap.add_argument("--speed", type=float, default=0.35,
                    help="GIF 播放速度倍率（GIF 用）。默认 0.35 倍慢放：原速 8 tick 只有"
                         "400ms，且多数看图器把 <50ms 的帧延迟钳到 100ms，原速反而失真")
    args = ap.parse_args()

    # **先剥到 emote 再用**。这里曾经拿整份文档当 emote 使：`collect_keyframes` 那行
    # 自己做了 `.get("emote", ...)` 所以关键帧是对的，但后面两处 `endTick` 取的是**文档
    # 顶层**——那儿没有这个键，于是永远吃默认值 8。症状极隐蔽：仓库里绝大多数动画正好
    # 就是 8 tick，图看着完全正常；只有 endTick ≠ 8 的（`club_smash` 是 12）会被**悄悄
    # 截断**，GIF 只播到 t8，整段收势看不见，而工具还理直气壮地打印"原速 400ms"。
    doc = json.loads(args.json.read_text(encoding="utf-8"))
    emote = doc.get("emote", doc)
    kfs = collect_keyframes(emote)
    ticks = [float(t) for t in args.ticks.split(",")]

    display = {"rotation": [0, -90, 55], "translation": [0, 4, 0], "scale": [0.8] * 3}
    if args.display and args.display.is_file():
        display = json.loads(args.display.read_text(encoding="utf-8"))["display"].get(
            "thirdperson_righthand", display)

    scene, ids, held_ids = build_scene(LIB / "out" / "_player_anim_scene.bbmodel", args.hold)

    # 固定取景：逐帧自动取景会让整段动画抖（render_bbmodel.render 的 docstring 明说）。
    # 但硬编码 span 会留一圈死边——扫一遍整段动画取**联合**包围盒，既贴身又照样恒定。
    focus = _fit_focus(kfs, display, scene, ids, held_ids, _end_tick(emote))

    if args.gif:
        return _write_gif(args, emote, kfs, display, scene, ids, held_ids, focus)

    rows = []
    for tick in ticks:
        seg = segment_transforms(kfs, tick)
        xform = {ids[n]: m for n, m in seg.items()}
        if held_ids:
            hm = hand_transform(kfs, tick, display)
            for hid in held_ids:
                xform[hid] = hm
        tiles = []
        for label, yaw, pitch in VIEWS:
            img, _ = render(scene, yaw=yaw, pitch=pitch, size=args.size,
                            xform=xform, focus=focus, shading="mc")
            tiles.append((label, img))
        rows.append((tick, tiles))

    gap, lab = 8, 16
    w = args.size * len(VIEWS) + gap * (len(VIEWS) + 1) + 54
    h = (args.size + lab + gap) * len(rows) + gap
    canvas = Image.new("RGB", (w, h), (16, 17, 20))
    draw = ImageDraw.Draw(canvas)
    y = gap
    for tick, tiles in rows:
        draw.text((6, y + args.size // 2), f"t{tick:g}", fill=(232, 232, 224))
        x = 54
        for label, img in tiles:
            draw.text((x + 3, y), label, fill=(198, 198, 190))
            canvas.paste(img, (x, y + lab))
            x += args.size + gap
        y += args.size + lab + gap

    out = args.out or (LIB / "out" / f"{args.json.stem}_on_player.png")
    out.parent.mkdir(parents=True, exist_ok=True)
    canvas.save(out)
    print(out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
