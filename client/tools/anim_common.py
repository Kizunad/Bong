"""Shared emitter for Bong player_animation JSON (Emotecraft v3, degrees=false).

Usage inside a per-animation generator (`gen_<name>.py`):

    from anim_common import emit_json, render

    POSE = {
        0: dict(easing="INOUTSINE",
            body=dict(x=0.0, y=0.0, z=0.0),
            rightArm=dict(pitch=-30, bend=15, axis=180),
            ...),
        5: dict(easing="OUTQUAD", ...),
        ...
    }

    emit_json(POSE,
        name="meditate_sit",
        description="...",
        end_tick=40, stop_tick=43, is_loop=True)

Conventions:
  - Angles in DEGREES (converted to radians at emit time since emote.degrees=false).
  - Linear xyz in MC "meters" (model pixels × 1/16 for body, raw for part offsets).
  - "axis" is the JSON key for bendDirection (NOT "bendDirection" — see
    docs/player-animation-conventions.md §7.4).
  - For looped animations: tick 0 and tick end_tick MUST have matching values on
    every axis used, or KeyframeAnimationPlayer.Axis.findAfter fabricates a
    virtual endTick+1 frame pointing to defaultValue and you get "fade to T-pose"
    mid-loop. We assert this.
  - Default bend axis for folding forearm toward player FRONT (punching /
    holding / meditating): axis=180°. Default (axis=0°) folds toward back.
"""

from __future__ import annotations

import json
import math
from pathlib import Path
from typing import Dict, Iterable, Optional

d = math.radians

ANGLE_AXES = frozenset({"pitch", "yaw", "roll", "bend", "axis"})
LINEAR_AXES = frozenset({"x", "y", "z"})
BODY_PARTS = frozenset(
    {"body", "head", "torso", "leftArm", "rightArm", "leftLeg", "rightLeg"}
)
# ── 手持物自己那根骨头 ──────────────────────────────────────────────────────
#
# PlayerAnimator 的 emote 除了七个身体部件，还认 `rightItem` / `leftItem`：
# `HeldItemMixin.changeItemLocation` 在 `HeldItemFeatureRenderer` 调 renderItem 之前
# 把它乘进手持物矩阵（`AnimationJson.getOrCreatePart` 照单全收，`KeyframeAnimation`
# 里 `bodyParts` 本来就有这两项）。
#
# **没有它，正握 / 反握是表达不出来的。** 手持物被 display 变换焊死在前臂上，刃相对
# 前臂的朝向按定义恒定；转手腕只能连着整条前臂一起转，读出来是"胳膊拧了"，不是
# "刀在手里掉了个头"。2026-08-31 之前本仓所有匕首动画都没有这根骨头，于是
# `dagger_grip_switch`（换握）根本没在换握，`knife_anim_gates.gate_flip` 只好退而
# 量"世界刃向转过多少度"——那个量对着一次普通挥砍也能读出 87°，分辨不了。
#
# 单位与身体部件**不一样**，别混：
#   pitch/yaw/roll  弧度，作用序 Rz(roll)·Ry(yaw)·Rx(pitch)，与身体部件同构；
#   x/y/z           **像素**（mixin 里 `pos.scale(1/16)` 之后才 translate），
#                   而身体部件的 x/y/z 是方块。为免这个陷阱静默生效，暂不放行——
#                   真要用先在这里写清单位再开。
ITEM_PARTS = frozenset({"rightItem", "leftItem"})
ITEM_AXES = frozenset({"pitch", "yaw", "roll"})
VALID_PARTS = BODY_PARTS | ITEM_PARTS
RESERVED_KEYS = frozenset({"easing"})


# ── 手持物在手里转（正握 ↔ 反握） ──────────────────────────────────────────
#
# `rightItem` 的三个角是在**手持物 display 变换之前**那个系里作用的，而人想说的是
# 「刀在自己的局部系里绕刃口轴转 180°」。两者差一个共轭：
#
#     渲染链   O = R_ATTACH · R_item · R_disp
#     静止时   O0 = R_ATTACH · R_disp
#     要 O = O0 · R_extra（R_extra 写在刀自己的局部系里）
#     => R_item = R_disp · R_extra · R_disp^-1
#
# 即：绕 `R_disp · 局部轴` 转同样的角度。下面两个函数就是这条式子，别在生成器里
# 各写一份魔数——theta 一改三个角全变，抄过去的数会悄悄对不上。


def _rot_axis(axis, deg: float):
    """Rodrigues：绕单位轴转 deg 度的 3x3（行优先嵌套 list）。"""
    n = math.sqrt(sum(v * v for v in axis))
    x, y, z = (v / n for v in axis)
    c, s_, t = math.cos(d(deg)), math.sin(d(deg)), 1.0 - math.cos(d(deg))
    return [[t * x * x + c, t * x * y - s_ * z, t * x * z + s_ * y],
            [t * x * y + s_ * z, t * y * y + c, t * y * z - s_ * x],
            [t * x * z - s_ * y, t * y * z + s_ * x, t * z * z + c]]


def _matmul(a, b):
    return [[sum(a[i][k] * b[k][j] for k in range(3)) for j in range(3)] for i in range(3)]


def item_spin(display_rotation, local_axis, theta_deg: float) -> dict:
    """手持物在**自己的局部系**里绕 `local_axis` 转 `theta_deg` -> `rightItem` 的三个角。

    `display_rotation` 就是物品模型 `display.thirdperson_righthand.rotation`
    （JOML `Quaternionf.rotationXYZ` => Rx·Ry·Rz）。返回值按 `Rz(roll)·Ry(yaw)·Rx(pitch)`
    分解，与 `HeldItemMixin.changeItemLocation` 的作用序一致。

    匕首的局部 +Y 是刃向、+X 是刃口方向（刃宽那一轴）、+Z 是刀面法线。绕 **X** 转
    180 度 = 刃口朝向不变、刀身整个倒转 = 正握 <-> 反握；绕 Z 转 180 度也能把刃倒过来，
    但会同时把刃口翻到另一侧，是另一个动作。
    """
    rx, ry, rz = (float(v) for v in display_rotation)
    r_disp = _matmul(_matmul(_rot_axis((1, 0, 0), rx), _rot_axis((0, 1, 0), ry)),
                     _rot_axis((0, 0, 1), rz))
    axis = [sum(r_disp[i][k] * local_axis[k] for k in range(3)) for i in range(3)]
    m = _rot_axis(axis, theta_deg)
    # R = Rz(roll)·Ry(yaw)·Rx(pitch) 的逆解；|sin(yaw)| 在本用法下 <= 0.82，不触万向锁
    yaw = math.degrees(math.asin(max(-1.0, min(1.0, -m[2][0]))))
    roll = math.degrees(math.atan2(m[1][0], m[0][0]))
    pitch = math.degrees(math.atan2(m[2][1], m[2][2]))
    return {"pitch": round(pitch, 4), "yaw": round(yaw, 4), "roll": round(roll, 4)}


def item_spin_series(display_rotation, local_axis, thetas) -> list[dict]:
    """一串 theta -> 一串 `rightItem` 三元组，**逐轴连续化**。

    为什么不能逐帧各调各的 `item_spin`：欧拉分解取的是主值，theta 越过 ±180 时某一轴
    会跳 360°。emote 是**逐轴线性插值**的，跳过去之后那一轴会朝反方向绕整整一圈 ——
    渲出来是刀在半路猛地反转一圈再转回来，而每一帧单独看都对。

    连续化只给某一轴加减 360°k。`Rz(roll)·Ry(yaw)·Rx(pitch)` 里任何一个因子加 360°
    都是同一个旋转，所以姿态一帧不变，变的只是插值走哪条路。
    """
    out = []
    for theta in thetas:
        axes = item_spin(display_rotation, local_axis, theta)
        if out:
            prev = out[-1]
            axes = {k: round(v + 360.0 * round((prev[k] - v) / 360.0), 4)
                    for k, v in axes.items()}
        out.append(axes)
    return out


def item_spin_angle(display_rotation, local_axis, item_axes: dict) -> tuple[float, float]:
    """`item_spin` 的逆：`rightItem` 的三个角 -> (theta, 偏轴角)，度。

    烘 bbmodel 时要把出料 JSON 里的三个角还原成「刀绕刃口轴转了多少」，才能写进
    `dagger_right_pitch` 那一层。同时报出偏轴角：绕别的轴转出来的旋转还原不成一个
    纯 X 自转，静默取 theta 会把它烘丢。
    """
    rx, ry, rz = (float(v) for v in display_rotation)
    r_disp = _matmul(_matmul(_rot_axis((1, 0, 0), rx), _rot_axis((0, 1, 0), ry)),
                     _rot_axis((0, 0, 1), rz))
    r_disp_t = [[r_disp[j][i] for j in range(3)] for i in range(3)]
    p_, y_, r_ = (float(item_axes.get(k, 0.0)) for k in ("pitch", "yaw", "roll"))
    r_item = _matmul(_matmul(_rot_axis((0, 0, 1), r_), _rot_axis((0, 1, 0), y_)),
                     _rot_axis((1, 0, 0), p_))
    m = _matmul(r_disp_t, _matmul(r_item, r_disp))
    theta = math.degrees(math.atan2(m[2][1], m[1][1]))
    x_img = [m[i][0] for i in range(3)]
    n = math.sqrt(sum(v * v for v in x_img))
    off = math.degrees(math.acos(max(-1.0, min(1.0, x_img[0] / n))))
    # local_axis 目前只支持 X（刃口轴）——别的轴要先想清楚 bb 那一层该挂在哪
    if tuple(local_axis) != (1.0, 0.0, 0.0):
        raise NotImplementedError(f"item_spin_angle 只解 X 轴自转，收到 {local_axis}")
    return round(theta, 4), round(off, 4)


# ── 关节解剖朝向 ────────────────────────────────────────────────────────────
# 肘只能往身前折（手够向脸/胸），膝只能往身后折（脚跟够向臀）。这不是风格偏好，
# 是骨骼约束——折反了就是断肢，而渲染出来往往只让人觉得"姿势有点别扭"，很难一眼
# 认定是 bug。所以在源码里硬拦，不靠肉眼审图。
#
# 判据：bendy-lib 把弯折轴取为 (cos(axis), 0, sin(axis))，绕它转 bend 后，末端相对
# 静止位的**前后**位移是
#
#     Δz = L · sin(bend) · cos(axis)          （MC 空间，+Z 是身后）
#
# 于是只需看 sin(bend)·cos(axis) 的符号，负 bend 和任意 axis 都自动处理：
#     肘 → 必须 < 0（往前）
#     膝 → 必须 > 0（往后）
# sin(bend) ≈ 0 时肢体本就是直的，axis 无意义，跳过。

_ARM_PARTS = frozenset({"leftArm", "rightArm"})
_LEG_PARTS = frozenset({"leftLeg", "rightLeg"})
# 低于这个折角就认为肢体是直的（axis 此时纯属声明，不产生朝向）
_FOLD_EPSILON_DEG = 1.0


def joint_fold_z(bend_deg: float, axis_deg: float) -> float:
    """末端前后位移的符号量（正=往身后，负=往身前）。单位无关，只看符号与大小。"""
    return math.sin(math.radians(bend_deg)) * math.cos(math.radians(axis_deg))


def assert_joint_fold_is_anatomical(part: str, bend_deg: float, axis_deg: float,
                                    where: str = "") -> None:
    """折向不合解剖就抛 ValueError。非可弯部位与近乎伸直的肢体直接放行。"""
    if part not in _ARM_PARTS and part not in _LEG_PARTS:
        return
    if abs(math.sin(math.radians(bend_deg))) < math.sin(math.radians(_FOLD_EPSILON_DEG)):
        return
    z = joint_fold_z(bend_deg, axis_deg)
    is_arm = part in _ARM_PARTS
    ok = z < 0 if is_arm else z > 0
    if ok:
        return
    joint, want, got = ("肘", "身前", "身后") if is_arm else ("膝", "身后", "身前")
    loc = f"{where}: " if where else ""
    raise ValueError(
        f"{loc}{part} 的{joint}折反了——bend={bend_deg:g}° axis={axis_deg:g}° 会让末端往"
        f"{got}折，{joint}只能往{want}折。\n"
        f"    修法：axis 加减 180°（本仓约定 手臂 axis=180、腿 axis=0），"
        f"或把 bend 取反。\n"
        f"    判据 sin(bend)·cos(axis) = {z:+.3f}，"
        f"手臂要求 < 0、腿要求 > 0。"
    )


def _validate_pose_table(pose_table: Dict[int, dict]) -> None:
    for tick, pose in pose_table.items():
        if not isinstance(tick, int):
            raise TypeError(f"pose tick must be int, got {type(tick).__name__}={tick!r}")
        for key, value in pose.items():
            if key in RESERVED_KEYS:
                continue
            if key not in VALID_PARTS:
                raise ValueError(f"tick {tick}: unknown part '{key}' (valid: {sorted(VALID_PARTS)})")
            if not isinstance(value, dict):
                raise TypeError(f"tick {tick}: part '{key}' must be dict, got {type(value).__name__}")
            for axis in value:
                if key in ITEM_PARTS:
                    if axis not in ITEM_AXES:
                        raise ValueError(
                            f"tick {tick}, part {key}: axis '{axis}' 不放行 —— 手持物骨头"
                            f"只开 {sorted(ITEM_AXES)}；x/y/z 的单位是像素而不是方块"
                            f"（见 ITEM_PARTS 注释），bend 在 runtime 侧对它是关闭的"
                        )
                    continue
                if axis not in ANGLE_AXES and axis not in LINEAR_AXES:
                    raise ValueError(
                        f"tick {tick}, part {key}: unknown axis '{axis}' "
                        f"(valid angles {sorted(ANGLE_AXES)}, linear {sorted(LINEAR_AXES)})"
                    )
            if "bend" in value:
                assert_joint_fold_is_anatomical(
                    key, float(value["bend"]), float(value.get("axis", 0.0)),
                    where=f"tick {tick}")


def _check_loop_closure(pose_table: Dict[int, dict], end_tick: int) -> None:
    """For looped animations, tick 0 and end_tick must match on every axis mentioned.

    Why: PlayerAnimator's Axis.findAfter synthesizes a virtual (endTick+1,
    defaultValue) frame when looping, so an axis keyed only at tick 0 linearly
    fades to 0 over the loop. See conventions doc §2 rule 8.
    """
    if 0 not in pose_table or end_tick not in pose_table:
        raise ValueError(f"looped anim must define both tick 0 and tick {end_tick}")
    pose0 = pose_table[0]
    pose_end = pose_table[end_tick]
    # Union of axes mentioned on either boundary.
    parts = (set(pose0.keys()) | set(pose_end.keys())) - RESERVED_KEYS
    problems = []
    for part in parts:
        axes0 = pose0.get(part, {})
        axesE = pose_end.get(part, {})
        all_axes = set(axes0.keys()) | set(axesE.keys())
        for axis in all_axes:
            v0 = axes0.get(axis)
            vE = axesE.get(axis)
            if v0 is None or vE is None or abs(float(v0) - float(vE)) > 1e-6:
                problems.append(f"  {part}.{axis}: tick 0 = {v0}, tick {end_tick} = {vE}")
    if problems:
        raise AssertionError(
            "loop boundary mismatch (tick 0 must equal tick {}):\n".format(end_tick)
            + "\n".join(problems)
        )


def build_doc(
    pose_table: Dict[int, dict],
    *,
    name: str,
    description: str,
    end_tick: int,
    stop_tick: int,
    is_loop: bool = False,
    return_tick: int = 0,
) -> dict:
    """Convert a POSE dict to an Emotecraft v3 JSON dict."""
    _validate_pose_table(pose_table)
    if is_loop:
        _check_loop_closure(pose_table, end_tick)
    if stop_tick < end_tick:
        raise ValueError(f"stop_tick ({stop_tick}) must be >= end_tick ({end_tick})")

    moves = []
    for tick in sorted(pose_table.keys()):
        pose = pose_table[tick]
        easing = pose.get("easing", "linear")
        for part_name, axes in pose.items():
            if part_name in RESERVED_KEYS:
                continue
            for axis_name, value in axes.items():
                val_out = d(float(value)) if axis_name in ANGLE_AXES else float(value)
                moves.append(
                    {
                        "tick": tick,
                        "easing": easing,
                        part_name: {axis_name: round(val_out, 7)},
                    }
                )

    return {
        "version": 3,
        "author": "Bong",
        "name": name,
        "description": description,
        "emote": {
            "beginTick": 0,
            "endTick": int(end_tick),
            "stopTick": int(stop_tick),
            "isLoop": bool(is_loop),
            "returnTick": int(return_tick),
            "nsfw": False,
            "degrees": False,
            "moves": moves,
        },
    }


# ---------------------------------------------------------------------------
# 重定时（把一条已经设计好的动画整体拉长 / 压缩）
# ---------------------------------------------------------------------------
#
# **tick 是整数，这是运行时的硬约束，不是本仓的洁癖**：PlayerAnimator 读 JSON 时
# `int tick = obj.get("tick").getAsInt()`（AnimationJson.java:123），存储层也是
# `findAtTick(int)` / `addKeyFrame(int, ...)`（KeyframeAnimation.java:451/469）。写
# 小数进去不会报错——会被截断，然后和相邻整数帧**撞成同一帧**，静默丢关键帧。
#
# 于是"把动画拉长 1.2 倍"这件事在整数网格上根本没有精确解：本来只有 1 tick 的段乘
# 1.2 之后只能落回 1 或 2，也就是 ×1.0 或 ×2.0。能做到的最好情况是让**每一帧的时间
# 位置**误差都不超过半 tick，办法是对累计位置取整（段长 = 相邻累计值之差），而不是逐段
# 取整再累加——后者的误差会一路攒下去，末帧能偏出好几 tick。
#
# **算出落位之后，把 POSE 表直接改写到新 tick 上，不要在出料时现搬。** 全仓每个动画
# 生成器都满足「POSE 的键 == 出料 JSON 的 tick」，`bbmodel_to_pose`（把 Blockbench 里
# 手改的姿态读回成 POSE 表）就是靠这条等式才能把读到的帧号原样当作 POSE 键用。谁在
# 出料一步搬帧，谁就单方面废掉这条回程：工具读出来的是出料 tick，贴回生成器却成了另一
# 套编号，贴一次就把整条动画的节奏改掉。`PoseTickContractTest` 逐个扫过去钉住这条等式。
#
# 所以这里只提供**求落位**，不提供搬表：拿 `integer_retime` 算出 {旧: 新}，照着它把
# 生成器里的 POSE 键改成新 tick，再把设计骨架和倍率作为常量留在生成器里（`gen_club_sweep`
# 的 `DESIGN_TICKS` / `TIME_SCALE` / `KEEP_GAP`），由测试反过来核验落位仍然对得上。
# 姿态本身一个数都不用改——这正是「拉长 = 搬帧，不是重采样」的全部含义：每一段走过的
# 姿态集合 `{lerp(v0, v1, ease(α)) : α ∈ [0,1]}` 与段长无关，贴棍距离、挡不挡脸、包围盒
# 这些几何判据逐字成立，变的只有速度。重采样做不到——倍率不是整数时，设计好的极值帧会
# 落在两个新整数 tick 之间，LOAD / IMPACT 的峰值被插值削掉。


def integer_retime(ticks: Iterable[int], scale: float, *,
                   keep_gap: Iterable[int] = ()) -> Dict[int, int]:
    """{原 tick: 新 tick}，按 `scale` 拉长到整数网格，累计时间误差 ≤ 0.5 tick。

    `keep_gap` 里的帧与**上一帧**的间隔保持原长。给的是那些"必须紧跟"的段：
    overshoot 就得贴着 impact 后一 tick（conventions §2.6），被拉成 2 tick 就不再是
    弹性过冲，而是"到位之后又慢慢挪了一下"。
    """
    src = sorted(int(t) for t in ticks)
    if not src or src[0] != 0:
        raise ValueError(f"重定时要求首帧是 tick 0，收到 {src[:1]}")
    tight = {int(t) for t in keep_gap}
    unknown = tight - set(src)
    if unknown:
        raise ValueError(f"keep_gap 里有不存在的帧 {sorted(unknown)}")

    out: Dict[int, int] = {src[0]: 0}
    prev = 0
    for i in range(1, len(src)):
        t = src[i]
        if t in tight:
            nxt = prev + (t - src[i - 1])
        else:
            nxt = int(math.floor(t * scale + 0.5))   # 半数向上，不要 banker's rounding
        if nxt <= prev:
            raise ValueError(
                f"tick {src[i - 1]}→{t} 在 scale={scale:g} 下压成了同一帧"
                f"（{prev}→{nxt}）——整数网格装不下，改 scale 或合并这两帧")
        out[t] = nxt
        prev = nxt
    return out


def resolve_output_path(name: str) -> Path:
    """Write into the Fabric resource tree regardless of CWD."""
    here = Path(__file__).resolve().parent
    return here.parent / "src/main/resources/assets/bong/player_animation" / f"{name}.json"


def write_json(doc: dict, out_path: Optional[Path] = None) -> Path:
    if out_path is None:
        out_path = resolve_output_path(doc["name"])
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(doc, ensure_ascii=False, indent=2))
    return out_path


def emit_json(
    pose_table: Dict[int, dict],
    *,
    name: str,
    description: str,
    end_tick: int,
    stop_tick: int,
    is_loop: bool = False,
    return_tick: int = 0,
) -> Path:
    """One-shot: build + write + print a small summary."""
    doc = build_doc(
        pose_table,
        name=name,
        description=description,
        end_tick=end_tick,
        stop_tick=stop_tick,
        is_loop=is_loop,
        return_tick=return_tick,
    )
    path = write_json(doc)
    print(
        f"wrote {path.name}  "
        f"ticks={sorted(pose_table.keys())}  "
        f"moves={len(doc['emote']['moves'])}  "
        f"loop={is_loop}"
    )
    return path


# ---------------------------------------------------------------------------
# Pose inheritance helpers
# ---------------------------------------------------------------------------


def inherit(base_pose: dict, **overrides: dict) -> dict:
    """Shallow-merge a base pose with per-part overrides.

    Useful for guard-return frames (copy the guard pose) or mirror poses.
    Each override value is merged INTO the corresponding base part dict, so
    you can tweak just `rightArm.pitch` without retyping the other axes.
    """
    out: dict = {}
    for k, v in base_pose.items():
        if isinstance(v, dict):
            out[k] = dict(v)
        else:
            out[k] = v
    for part, axes in overrides.items():
        if part in RESERVED_KEYS or not isinstance(axes, dict):
            out[part] = axes
            continue
        merged = dict(out.get(part, {}))
        merged.update(axes)
        out[part] = merged
    return out


def mirror_pose(pose: dict, *, exclude_parts: Iterable[str] = ()) -> dict:
    """Mirror left/right of a pose in place — swap arms/legs and flip signs on
    symmetric axes (yaw, roll, body.x, body.yaw, head.yaw, torso.yaw, bend axis).

    Bend MAGNITUDE is preserved (pitch is preserved for the corresponding arm).
    Call for left-handed cross from a right-handed cross, etc.
    """
    exclude = set(exclude_parts)
    out: dict = {}
    for k, v in pose.items():
        if k in exclude:
            out[k] = v
            continue
        if isinstance(v, dict):
            out[k] = dict(v)
        else:
            out[k] = v

    # swap arms
    if "rightArm" in out or "leftArm" in out:
        ra = out.pop("rightArm", {})
        la = out.pop("leftArm", {})
        out["rightArm"] = _flip_axes(la)
        out["leftArm"] = _flip_axes(ra)
    # swap legs
    if "rightLeg" in out or "leftLeg" in out:
        rl = out.pop("rightLeg", {})
        ll = out.pop("leftLeg", {})
        out["rightLeg"] = _flip_axes(ll)
        out["leftLeg"] = _flip_axes(rl)

    # flip symmetric axes on central parts
    for part in ("body", "head", "torso"):
        axes = out.get(part)
        if not axes:
            continue
        flipped = dict(axes)
        if "x" in flipped:
            flipped["x"] = -flipped["x"]
        if "yaw" in flipped:
            flipped["yaw"] = -flipped["yaw"]
        if "roll" in flipped:
            flipped["roll"] = -flipped["roll"]
        out[part] = flipped
    return out


def _flip_axes(axes: dict) -> dict:
    """Flip yaw / roll signs. Flip bend axis around π (axis → 360°-axis).

    pitch and bend MAGNITUDE are preserved.
    """
    out = dict(axes)
    if "yaw" in out:
        out["yaw"] = -out["yaw"]
    if "roll" in out:
        out["roll"] = -out["roll"]
    if "axis" in out:
        # bend axis in degrees; mirror around vertical plane = 360 - axis (mod 360)
        out["axis"] = (360.0 - out["axis"]) % 360.0
    return out
