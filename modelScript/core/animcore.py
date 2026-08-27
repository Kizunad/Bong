#!/usr/bin/env python3
"""动画层的公共底座：旋转 / 曲线 / 关键帧落盘。

历史上 `animkit.py`（腐羽鹫用）和 `anim_rig.py`（可可哒鹅 + 缝合兽用）各自带了一份
`rotmat` / `euler` / `affine` / `wrap` / `smooth` / `pulse` / `keyed` / 关键帧写盘 ——
函数体逐字相同，纯负债：改一边另一边不会跟着改，而两边都在被真实流水线调用。

本模块只收**真正重复**的那部分。两边**行为不同**的东西不合并：
  · `build_tracks` 的裁剪策略：animkit 走力臂加权的共线裁剪（Ramer–Douglas–Peucker）+
    必须落点 + 解缠；anim_rig 走「恒定但非默认的通道只留首末两帧」。两种策略各自解决
    的问题不一样（一个是慢通道冗余，一个是十几条芽的恒定姿），硬合成一个带开关的函数
    只会把两边的回归风险搅在一起。这里只抽出它们共用的**采样骨架**。
  · 关键帧 uuid 的种子拼法：animkit 是 `名+骨+通道+序号`，anim_rig 是
    `名+骨+通道+通道+序号`（多一遍通道名）。差别没有意义，但改它会让所有既有产物的
    uuid 全变 —— 所以 `keyframe()` 收**完整种子串**，拼法留在各自的调用点。

`creatures/dainu_lion/rig.py` 与 `creatures/horse/rig.py` 里还各有一份
`rotmat`/`euler`/`affine`（拟态灰烬蛛复用狮子那份）。它们是 creature 本地绑定层，
不在本轮合并范围内 —— 但那三个函数体同样是逐字相同的，是已知的剩余重复。
"""

from __future__ import annotations

import hashlib
import math
import uuid as _uuidlib
import zlib

import numpy as np

# ================================================================ 旋转


def rotmat(deg: float, axis: int) -> np.ndarray:
    a = math.radians(deg)
    c, s = math.cos(a), math.sin(a)
    if axis == 0:
        return np.array([[1, 0, 0], [0, c, -s], [0, s, c]])
    if axis == 1:
        return np.array([[c, 0, s], [0, 1, 0], [-s, 0, c]])
    return np.array([[c, -s, 0], [s, c, 0], [0, 0, 1]])


def euler(rot) -> np.ndarray:
    """Blockbench 顺序 Rz·Ry·Rx。"""
    if not any(rot):
        return np.eye(3)
    return rotmat(rot[2], 2) @ rotmat(rot[1], 1) @ rotmat(rot[0], 0)


# 万向锁判据的门限。两份历史实现写的是同一个量的两种写法 ——
# animkit 判 `sqrt(1 − sin²y) < 1e-6`、anim_rig 判 `|cos y| < 1e-7`，而
# `sqrt(1 − sin²y) ≡ |cos y|`，差的只是数量级。合并取 1e-6（更早进入退化分支）：
# 两者的分歧窗口是俯仰角落在距 ±90° 约 6e-5 度以内，实际动画到不了那里，
# 三个 creature 的产物逐字节回归也证实没变。
GIMBAL_EPS = 1e-6


def euler_xyz(R: np.ndarray) -> tuple[float, float, float]:
    """`euler()` 的逆：旋转矩阵 → Blockbench 顺序（Rz·Ry·Rx）的三个角（度）。

    需要它是因为有些姿态**天然是用轴角描述的**（绕某条切向倒过去多少度），而骨骼通道
    只吃欧拉角。自己按分量反解，别拿三个角去凑 —— 凑出来的解在万向锁附近会跳。

    R = Rz(c)·Ry(b)·Rx(a) 展开后第三行是 (−sb, sa·cb, ca·cb)，第一列是
    (cb·cc, cb·sc, −sb)，于是 b = −asin(R₂₀)、a = atan2(R₂₁, R₂₂)、c = atan2(R₁₀, R₀₀)。
    cb→0 时（俯仰 ±90°）a 与 c 简并，**把 c 固定为 0 再解 a** —— 该处两者本就简并，
    硬解出来的那一对角度会在相邻帧之间乱跳。
    """
    sy = max(-1.0, min(1.0, -float(R[2, 0])))
    if math.sqrt(max(0.0, 1.0 - sy * sy)) < GIMBAL_EPS:
        return (math.degrees(math.atan2(-R[1, 2], R[1, 1])), math.degrees(math.asin(sy)), 0.0)
    return (math.degrees(math.atan2(R[2, 1], R[2, 2])),
            math.degrees(math.asin(sy)),
            math.degrees(math.atan2(R[1, 0], R[0, 0])))


def affine(R: np.ndarray, t: np.ndarray) -> np.ndarray:
    M = np.eye(4)
    M[:3, :3] = R
    M[:3, 3] = t
    return M


def align(u, v) -> np.ndarray:
    """把向量 u 转到 v 的**最小**旋转（罗德里格斯）。反向时挑一个垂直轴。"""
    u = np.asarray(u, float)
    v = np.asarray(v, float)
    u = u / (np.linalg.norm(u) or 1.0)
    v = v / (np.linalg.norm(v) or 1.0)
    c = float(np.dot(u, v))
    if c > 1.0 - 1e-12:
        return np.eye(3)
    if c < -1.0 + 1e-12:
        axis = np.cross(u, [1.0, 0.0, 0.0])
        if np.linalg.norm(axis) < 1e-6:
            axis = np.cross(u, [0.0, 1.0, 0.0])
        axis /= np.linalg.norm(axis)
        K = np.array([[0, -axis[2], axis[1]], [axis[2], 0, -axis[0]], [-axis[1], axis[0], 0]])
        return np.eye(3) + 2.0 * K @ K
    w = np.cross(u, v)
    K = np.array([[0, -w[2], w[1]], [w[2], 0, -w[0]], [-w[1], w[0], 0]])
    return np.eye(3) + K + K @ K / (1.0 + c)


def to_quat(R: np.ndarray) -> np.ndarray:
    t = float(np.trace(R))
    if t > 0.0:
        s = math.sqrt(t + 1.0) * 2.0
        return np.array([(R[2, 1] - R[1, 2]) / s, (R[0, 2] - R[2, 0]) / s,
                         (R[1, 0] - R[0, 1]) / s, 0.25 * s])
    i = int(np.argmax(np.diag(R)))
    j, k = (i + 1) % 3, (i + 2) % 3
    s = math.sqrt(1.0 + R[i, i] - R[j, j] - R[k, k]) * 2.0
    q = np.zeros(4)
    q[3] = (R[k, j] - R[j, k]) / s
    q[i], q[j], q[k] = 0.25 * s, (R[j, i] + R[i, j]) / s, (R[k, i] + R[i, k]) / s
    return q


def from_quat(q: np.ndarray) -> np.ndarray:
    x, y, z, w = q / (np.linalg.norm(q) or 1.0)
    return np.array([
        [1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)],
        [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)],
        [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)],
    ])


def slerp(R0: np.ndarray, R1: np.ndarray, w: float) -> np.ndarray:
    """两个旋转之间的**球面**插值。

    别对欧拉角做线性插值：三个分量各走各的直线，合成出来的姿态既不是最短弧、中途也不
    在两端姿态的"之间"。转角小的时候看不出来，收翼→展翼每根羽要转近 90°，线性插值下
    整片翼中途会散成互不相连的板（实测 t=0.3~0.45 两帧最明显）。
    """
    q0, q1 = to_quat(R0), to_quat(R1)
    if float(np.dot(q0, q1)) < 0.0:
        q1 = -q1                      # 取近端：不翻符号会绕远路转 360° 减去夹角
    d = float(np.clip(np.dot(q0, q1), -1.0, 1.0))
    if d > 0.9995:
        return from_quat(q0 + (q1 - q0) * w)
    th = math.acos(d)
    s = math.sin(th)
    return from_quat((math.sin((1 - w) * th) * q0 + math.sin(w * th) * q1) / s)


# ================================================================ 曲线
def wrap(u: float) -> float:
    return u - math.floor(u)


def clamp01(s: float) -> float:
    return min(1.0, max(0.0, s))


def smooth(s: float) -> float:
    s = clamp01(s)
    return s * s * (3.0 - 2.0 * s)


def ease_out(s: float, p: float = 2.0) -> float:
    return 1.0 - (1.0 - clamp01(s)) ** p


def pulse(u: float, center: float, width: float) -> float:
    """环形高斯脉冲：做"大部分时间不动、某一刻抽一下"。"""
    d = abs(wrap(u - center + 0.5) - 0.5)
    return math.exp(-((d / width) ** 2))


def soft_clamp(x: float, lo: float, hi: float, knee: float) -> float:
    """夹进 [lo, hi]，但在边界前 knee 的宽度里**平滑**收口，永不真正贴死。

    硬夹的代价不在越界，在"冻住"：逆解一旦顶到可达上界，关节角就一动不动，等目标退回
    可达域再突然复活 —— 值是连续的、导数不是，看着就是肢体先卡住再"啪"地弹一下。实测
    落地那条动作里跗跖角连着六个子步纹丝不动（−48.79），紧接着 1/480 个周期内跳 9.5°。

    x ≤ hi−knee 时原样返回（导数为 1，与未夹区无缝接上），越界越多越贴近 hi 但取不到。
    """
    if knee <= 1e-9:
        return min(hi, max(lo, x))
    if x > hi - knee:
        return hi - knee * math.exp(-(x - (hi - knee)) / knee)
    if x < lo + knee:
        return lo + knee * math.exp((x - (lo + knee)) / knee)
    return x


def keyed(t: float, keys) -> float:
    """按 (时间, 值) 列表做平滑插值（段内 smoothstep，不像线性那样留折角）。"""
    if t <= keys[0][0]:
        return keys[0][1]
    for (t0, v0), (t1, v1) in zip(keys, keys[1:]):
        if t <= t1:
            return v0 + (v1 - v0) * smooth((t - t0) / (t1 - t0)) if t1 > t0 else v1
    return keys[-1][1]


def jitter(name: str, i: int) -> float:
    """稳定扰动（crc32 不用内置 hash——后者每进程加盐，两次跑出的动画会不一样）。"""
    return (((zlib.crc32(f"{name}{i}".encode()) >> 3) & 1023) / 1023.0) * 2.0 - 1.0


def decay_shake(t: float, freq: float, tau: float) -> float:
    """指数衰减抖动。受击/抖毛/努责余震都是这个形状，别用等幅正弦。"""
    return math.sin(2.0 * math.pi * freq * t) * math.exp(-tau * t)


# ================================================================ 采样骨架
CHANNELS = (("rotation", "rot", 0.0), ("position", "pos", 0.0), ("scale", "scale", 1.0))


def sample_frames(sampler, length: float, loop: bool, times) -> list:
    """按给定的 t01 序列采样，返回 [(秒, Pose)]。

    循环动画的末帧直接复用首帧的姿态对象（接缝严格为零），而不是再采一次 t=1.0 ——
    采样器在 t=0 和 t=1 上未必给出**逐位相同**的浮点数，那点差就是循环时的一跳。
    """
    frames: list = []
    for t in times:
        if loop and t >= 1.0:
            frames.append((length, frames[0][1]))
            break
        frames.append((t * length, sampler(t)))
    return frames


def channel_values(bone: str, attr: str, default: float, frames) -> list:
    return [(tt, list(getattr(pz[bone], attr)) if bone in pz else [default] * 3)
            for tt, pz in frames]


def is_constant_default(vals, default: float, tol: float = 1e-4) -> bool:
    return all(abs(v[k] - default) < tol for _, v in vals for k in range(3))


def unwrap_degrees(vals) -> None:
    """就地解缠旋转通道。

    euler(θ) 与 euler(θ±360) 是同一个姿态，但导出的关键帧走线性插值 —— 相邻两帧一个
    +179 一个 −179，播出来是整整转一圈。姿态由旋转矩阵解出来时（球面插值那条路）必然
    会在 ±180 处翻面，所以这一步不是可选的。
    """
    for i in range(1, len(vals)):
        prev, cur = vals[i - 1][1], vals[i][1]
        for k in range(3):
            cur[k] -= 360.0 * round((cur[k] - prev[k]) / 360.0)


# ================================================================ 关键帧落盘
def stable_uuid(seed: str) -> str:
    """确定性 v4 uuid。

    别用 `uuid.UUID(int=crc32(seed))` 那种拼法：熵只有 32 位（上万关键帧撞车概率约
    1%），版本/变体位也不合法。Blockbench 拿 uuid 当索引键，不值得冒这个险。
    """
    return str(_uuidlib.UUID(bytes=hashlib.md5(seed.encode()).digest(), version=4))


def keyframe(channel: str, time: float, vec, seed: str) -> dict:
    """一个关键帧。字段照用户手上能正常打开的带动画工程逐项对齐：data_points 用**字符串**、
    bezier 四件套即使走 linear 也写全 —— 缺字段的默认值随 Blockbench 版本变，不值得赌。

    seed 收**完整种子串**（含序号）而不是自己拼：两个调用点的历史拼法不同，
    统一它会让所有既有产物的 uuid 全变，而 uuid 只是索引键，没有换的收益。
    """
    return {
        "channel": channel,
        "data_points": [{"x": f"{vec[0]:.4f}", "y": f"{vec[1]:.4f}", "z": f"{vec[2]:.4f}"}],
        "uuid": stable_uuid(seed),
        "time": round(time, 4),
        "color": -1,
        "interpolation": "linear",
        "bezier_linked": True,
        "bezier_left_time": [-0.1, -0.1, -0.1],
        "bezier_left_value": [0, 0, 0],
        "bezier_right_time": [0.1, 0.1, 0.1],
        "bezier_right_value": [0, 0, 0],
    }


def animators_of(tracks: dict, uuid_of, seed_of) -> dict:
    """tracks → Blockbench 的 animators 表。

    uuid_of(骨名) 交出该骨的 uuid；seed_of(骨名, 通道, 序号) 交出关键帧 uuid 的种子串。
    种子拼法由调用点给，见 `keyframe()` 的说明 —— 两处历史拼法不同，统一它只会让所有
    既有产物的 uuid 全变。
    """
    animators = {}
    for bone, chans in tracks.items():
        kfs = []
        for chan, vals in chans.items():
            for i, (tt, v) in enumerate(vals):
                kfs.append(keyframe(chan, tt, v, seed_of(bone, chan, i)))
        animators[uuid_of(bone)] = {"name": bone, "type": "bone", "keyframes": kfs}
    return animators


def animation_entry(model_name: str, name: str, length: float, loop: bool,
                    animators: dict) -> dict:
    """bbmodel `animations` 里的一条。"""
    return {
        "uuid": stable_uuid(f"anim:{model_name}:{name}"),
        "name": name,
        "loop": "loop" if loop else "once",
        "override": False,
        "length": round(length, 4),
        "snapping": 24,
        "selected": False,
        "saved": True,
        "path": "",
        "anim_time_update": "",
        "blend_weight": "",
        "start_delay": "",
        "loop_delay": "",
        "animators": animators,
    }


def geckolib_document(entries, namespace: str, model_id: str) -> dict:
    """entries: [(名字, 时长秒, 是否循环, tracks)] → GeckoLib animation.json 的整份文档。

    **参考用，未经引擎侧验证，别直接当资产提交。** Bedrock 动画的旋转符号约定与
    Blockbench 面板显示是否一致（X/Y 是否取反），仓库里没有可对拍的同源实例。正经路径
    是把带动画的 .bbmodel 交给 `bbmodel_to_geckolib.py`（驱动 Blockbench 官方 codec
    导出），由 codec 负责这层约定；本函数只用于人眼查曲线和兜底。
    """
    animations = {}
    for name, length, loop, tracks in entries:
        bones = {}
        for bone, chans in tracks.items():
            bones[bone] = {
                chan: {str(round(tt, 4)): [round(v[0], 4), round(v[1], 4), round(v[2], 4)]
                       for tt, v in vals}
                for chan, vals in chans.items()
            }
        animations[f"animation.{namespace}.{model_id}.{name}"] = {
            "loop": bool(loop),
            "animation_length": round(length, 4),
            "bones": bones,
        }
    return {"format_version": "1.8.0", "animations": animations}
