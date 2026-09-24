#!/usr/bin/env python3
"""拟态灰烬蛛 —— 动画生成：程序化步态 + 恐惧动作，写回 bbmodel 关键帧。

原则与狮子一致：**逐层派生，别凭手感拧角度**。移动类只给「脚该踩在哪」，
关节角全由 spider_rig.solve_leg 逆解——支撑相脚锁死世界坐标，不滑步。

恐惧设计参数（每条动画的存在理由）：
  · ambush_burst 5 tick——快到眼睛跟不上才是暴起；先 1 tick 压缩蓄势（squash），
    炸开过冲 15% 再弹回（弹性过冲是"活物"和"机关"的区别）
  · bite 蓄 0.42 / 刺 0.14——威吓后仰亮腹面慢，突刺比蓄力快三倍
  · walk/run 逐腿相位噪声——步频高而步幅小的 scuttle，不是机械阅兵
  · idle 永不全静——触肢 3.5Hz 微颤 + 螯肢无征兆开合一次
  · fold 从容——暴起的 2 倍时长逐对收腿，咬完当着你的面重新叠回一块方块
  · retreat 低身位急窜（freeze-and-stare 由引擎切 idle 插入，见 check_anim 注记）
  · death 八腿向腹面蜷缩（真实蜘蛛死态）+ 两次递减抽搐

输出:
  modelScript/models/mimic_spider/MimicSpiderRig.bbmodel     带动画（可直接拖进 Blockbench）
  modelScript/models/mimic_spider/mimic_spider.animation.json GeckoLib（参考/兜底，
      正经资产走 bbmodel_to_geckolib.py 官方 codec 导出）

源模型 MimicSpiderShell.bbmodel 只读不写。
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import uuid
from pathlib import Path

import numpy as np

from spider_rig import (
    LEG_KEYS,
    SHELL,
    Channel,
    Pose,
    SpiderRig,
    fold_pose,
    rest_targets,
)

OUT_BB = SHELL.parent / "MimicSpiderRig.bbmodel"
OUT_JSON = SHELL.parent / "mimic_spider.animation.json"
NAMESPACE = "bong"
MODEL_ID = "mimic_spider"


# ---------------------------------------------------------------- 曲线工具
def wrap(u: float) -> float:
    return u - math.floor(u)


def smooth(s: float) -> float:
    s = min(1.0, max(0.0, s))
    return s * s * (3.0 - 2.0 * s)


def ease_out(s: float, p: float = 2.0) -> float:
    return 1.0 - (1.0 - min(1.0, max(0.0, s))) ** p


def keyed(t: float, pts: list[tuple[float, float]]) -> float:
    """分段线性关键曲线（各段 smoothstep）。"""
    if t <= pts[0][0]:
        return pts[0][1]
    for (t0, v0), (t1, v1) in zip(pts, pts[1:]):
        if t0 <= t <= t1:
            return v0 + (v1 - v0) * smooth((t - t0) / (t1 - t0) if t1 > t0 else 1.0)
    return pts[-1][1]


def leg_noise(pair: int, side: str, scale: float) -> float:
    """逐腿确定性相位噪声：步态不许整排阅兵，也不许每次生成不一样。"""
    h = hashlib.md5(f"{pair}{side}".encode()).digest()[0]
    return (h / 255.0 * 2.0 - 1.0) * scale


def leg_group(pair: int, side: str) -> float:
    """交替四足组：奇数对右侧 + 偶数对左侧同相。"""
    return 0.0 if (pair % 2 == 1) == (side == "r") else 0.5


# ---------------------------------------------------------------- 姿态积木
def palp_tremor(pose: Pose, t: float, hz: float, amp: float, length: float) -> None:
    """触肢高频低幅微颤——idle 的"永不全静"。左右异相。

    周期数取整：sin 里必须是整数个循环，否则 t=0 与 t=1 不等——循环接缝跳变
    （check_anim 抓的第一批 bug 之一就是 3.5Hz×1.31×6s = 27.51 个非整周期）。"""
    n1 = max(1, round(hz * length))
    n2 = max(1, round(hz * 1.31 * length))
    for side, ph in (("l", 0.0), ("r", 0.37)):
        w = math.sin(2.0 * math.pi * (n1 * t + ph))
        w2 = math.sin(2.0 * math.pi * (n2 * t + ph * 0.65))
        pose[f"palp1_{side}"].rot[0] += amp * w
        pose[f"palp2_{side}"].rot[0] += amp * 1.4 * w2
        pose[f"palp2_{side}"].rot[2] += amp * 0.6 * w


def chelicera_spread(pose: Pose, spread: float, unsheathe: float) -> None:
    """螯肢外张 + 螯牙出鞘。spread/unsheathe ∈ [0,1]。"""
    for side, sx in (("l", -1.0), ("r", 1.0)):
        pose[f"chelicera_{side}"].rot[0] += 18.0 * spread
        pose[f"chelicera_{side}"].rot[2] += sx * -38.0 * spread
        pose[f"fang_{side}"].rot[0] += -32.0 * unsheathe


def gait_targets(t: float, *, stride: float, duty: float, lift: float,
                 noise: float, widen: float = 1.0):
    """一帧的八爪世界落点 + 支撑相判定。前进方向 = −z（头朝向）。"""
    rest = rest_targets()
    out: dict[tuple[int, str], np.ndarray] = {}
    stance: dict[tuple[int, str], bool] = {}
    for pair, side in LEG_KEYS:
        u = wrap(t + leg_group(pair, side) + leg_noise(pair, side, noise))
        tgt = rest[(pair, side)].copy()
        tgt[0] *= widen
        if u < duty:                       # 支撑：脚从前极限匀速后移（世界系锁死）
            frac = u / duty
            tgt[2] += stride * (frac - 0.5)
            stance[(pair, side)] = True
        else:                              # 摆动：抬脚前摆
            frac = (u - duty) / (1.0 - duty)
            tgt[2] += stride * (0.5 - frac)
            tgt[1] += lift * math.sin(math.pi * frac)
            stance[(pair, side)] = False
        out[(pair, side)] = tgt
    return out, stance


def body_bob(pose: Pose, t: float, bob: float, sway: float, pitch: float = 0.0) -> None:
    pose["root"].pos[1] += bob * math.sin(4.0 * math.pi * t)
    pose["root"].rot[1] += sway * math.sin(2.0 * math.pi * t)
    pose["prosoma"].rot[0] += pitch
    pose["abdomen"].rot[0] += -pitch * 0.5 + 1.5 * math.sin(4.0 * math.pi * t + 1.2)


# ================================================================ 各动画
def anim_idle(rig: SpiderRig, t: float) -> Pose:
    """伏击待机：只有触肢在颤、螯肢无征兆开合一次。腿一动不动——盯着才吓人。"""
    p = Pose()
    length = ANIMS["idle"][0]
    p["root"].pos[1] = 0.22 * math.sin(2.0 * math.pi * t)          # 呼吸（一周期）
    p["abdomen"].rot[0] = 1.2 * math.sin(2.0 * math.pi * t + 0.8)
    palp_tremor(p, t, hz=3.5, amp=2.6, length=length)
    clack = keyed(t, [(0.60, 0.0), (0.63, 1.0), (0.67, 0.0)])       # 无征兆开合
    chelicera_spread(p, clack * 0.35, clack * 0.5)
    rig.plant(p, rest_targets())                                    # 呼吸由腿逆解吸收
    return p


def anim_walk(rig: SpiderRig, t: float) -> Pose:
    p = Pose()
    body_bob(p, t, bob=0.45, sway=1.4)
    palp_tremor(p, t, hz=2.2, amp=1.4, length=ANIMS["walk"][0])
    tgts, _ = gait_targets(t, stride=6.0, duty=0.58, lift=2.4, noise=0.045)
    rig.plant(p, tgts)
    return p


def anim_run(rig: SpiderRig, t: float) -> Pose:
    """追击 scuttle：步频高、身体前倾、噪声更大——爬得"太快了"才对。"""
    p = Pose()
    body_bob(p, t, bob=0.85, sway=2.2, pitch=-4.0)
    chelicera_spread(p, 0.3, 0.4)                                   # 追着你时牙已半张
    tgts, _ = gait_targets(t, stride=8.0, duty=0.44, lift=3.0, noise=0.06)
    rig.plant(p, tgts)
    return p


def anim_retreat(rig: SpiderRig, t: float) -> Pose:
    """低身位急窜。freeze-and-stare 由引擎在循环间切 idle 插入，不在本条内。"""
    p = Pose()
    p["root"].pos[1] = -1.4
    p["root"].rot[2] = 2.0 * math.sin(2.0 * math.pi * t)
    body_bob(p, t, bob=0.6, sway=1.6)
    tgts, _ = gait_targets(t, stride=7.0, duty=0.40, lift=2.2, noise=0.07, widen=1.12)
    rig.plant(p, tgts)
    return p


def _mix_fold(p: Pose, fold: Pose, s: float, bones: list[str]) -> None:
    """把 fold 姿按比例 s 混入 p（s=1 完全折叠；s<0 = 反向过冲伸展）。"""
    for n in bones:
        f = fold[n] if n in fold else Channel()
        ch = p[n]
        ch.rot = [f.rot[i] * s for i in range(3)]
        ch.pos = [f.pos[i] * s for i in range(3)]


_FOLD = fold_pose()
_LEG_BONES = [f"{pre}{pair}_{side}" for pair, side in LEG_KEYS
              for pre in ("coxa", "femur", "tibia", "tarsus")]
_BODY_BONES = ["root", "abdomen", "chelicera_l", "chelicera_r",
               "palp1_l", "palp1_r", "palp2_l", "palp2_r"]


def anim_ambush_burst(rig: SpiderRig, t: float) -> Pose:
    """暴起：1 tick 压缩蓄势 → 3 tick 炸开过冲 15% → 1 tick 弹定。
    首帧 = 折叠姿（方块渲染切换的交界帧），末帧 = 站姿。全程无转身——
    从方块里出来的第一帧就已经正对着你。"""
    p = Pose()
    # s: 折叠比例。1.04 = 先再压 4%（anticipation squash），−0.15 = 过冲伸展
    s = keyed(t, [(0.0, 1.0), (0.10, 1.04), (0.55, -0.15), (0.78, 0.04), (1.0, 0.0)])
    _mix_fold(p, _FOLD, s, _LEG_BONES + _BODY_BONES)
    # 身体弹射：蹲底 → 腾起过冲 → 落回
    hop = keyed(t, [(0.10, 0.0), (0.50, 1.0), (0.72, -0.18), (1.0, 0.0)])
    p["root"].pos[1] += 1.6 * hop
    # 落地前螯牙先张——威吓在脚落地之前到位
    thr = keyed(t, [(0.30, 0.0), (0.62, 1.0), (1.0, 0.35)])
    chelicera_spread(p, thr, thr)
    return p


def anim_fold(rig: SpiderRig, t: float) -> Pose:
    """收拢：暴起的 2 倍时长，逐对收腿（4→1 对波次），身体最后沉底。
    末帧精确 = 折叠姿——下一帧就切方块渲染，超一丝就穿帮。"""
    p = Pose()
    for pair, side in LEG_KEYS:
        t0 = (4 - pair) * 0.09                                     # 后腿先收
        s = keyed(t, [(t0, 0.0), (t0 + 0.52, 1.0)])
        bones = [f"{pre}{pair}_{side}" for pre in ("coxa", "femur", "tibia", "tarsus")]
        _mix_fold(p, _FOLD, s, bones)
    body_s = keyed(t, [(0.30, 0.0), (0.92, 1.0)])
    _mix_fold(p, _FOLD, body_s, _BODY_BONES)
    return p


def anim_bite(rig: SpiderRig, t: float) -> Pose:
    """咬：后仰威吓亮腹面（慢）→ 突刺（快 3 倍）→ 复位。命中后引擎接 fold。"""
    p = Pose()
    rear = keyed(t, [(0.0, 0.0), (0.42, 1.0), (0.56, 0.0), (1.0, 0.0)])
    strike = keyed(t, [(0.42, 0.0), (0.50, 1.0), (0.70, 0.2), (1.0, 0.0)])

    p["prosoma"].rot[0] = -26.0 * rear + 18.0 * strike
    p["root"].pos[1] = -0.6 * rear - 0.9 * strike
    p["root"].pos[2] = 1.2 * rear - 2.6 * strike                   # 后坐 → 前扑
    p["abdomen"].rot[0] = 10.0 * rear - 6.0 * strike
    chelicera_spread(p, rear * 1.0 + strike * 0.1, rear * 0.8)
    for side in ("l", "r"):
        p[f"fang_{side}"].rot[0] += 40.0 * strike                  # 合牙
        p[f"palp1_{side}"].rot[0] += -30.0 * rear

    rest = rest_targets()
    for pair, side in LEG_KEYS:
        tgt = rest[(pair, side)].copy()
        if pair == 1:                                              # 前对腿举起亮爪
            tgt[1] += 7.5 * rear
            tgt[2] += -3.0 * rear + 1.0 * strike
            tgt[0] *= 1.0 + 0.25 * rear
        elif pair == 2:
            tgt[1] += 2.5 * rear
        rig.solve_leg(p, pair, side, tgt)
    return p


def anim_hurt(rig: SpiderRig, t: float) -> Pose:
    """受击：硬顿挫 + 递减抖动；腿外splay一瞬——像被踩了一脚的真蛛。"""
    p = Pose()
    hit = keyed(t, [(0.0, 0.0), (0.10, 1.0), (0.34, 0.4), (1.0, 0.0)])
    shake = math.sin(2.0 * math.pi * 8.0 * t) * math.exp(-4.0 * t)
    p["root"].pos[1] = -1.1 * hit
    p["root"].rot[2] = 6.0 * hit + 1.2 * shake
    p["prosoma"].rot[0] = 5.0 * hit + 1.5 * shake
    p["abdomen"].rot[0] = -8.0 * hit
    chelicera_spread(p, 0.5 * hit, 0.3 * hit)
    rest = rest_targets()
    for pair, side in LEG_KEYS:
        tgt = rest[(pair, side)].copy()
        tgt[0] *= 1.0 + 0.14 * hit
        rig.solve_leg(p, pair, side, tgt)
    return p


def anim_death(rig: SpiderRig, t: float) -> Pose:
    """死亡：腿失去支撑 → 身体沉底 → 八腿向腹面蜷缩（真实蜘蛛死态）→
    两次递减抽搐 → 静止。"""
    p = Pose()
    buckle = keyed(t, [(0.0, 0.0), (0.30, 1.0)])
    sink = keyed(t, [(0.12, 0.0), (0.48, 1.0)])
    curl = keyed(t, [(0.40, 0.0), (0.82, 1.0)])
    spasm = (keyed(t, [(0.62, 0.0), (0.66, 1.0), (0.72, 0.0)]) * 0.10
             + keyed(t, [(0.80, 0.0), (0.83, 1.0), (0.88, 0.0)]) * 0.05)

    p["root"].pos[1] = -4.5 * sink
    p["prosoma"].rot[0] = 6.0 * sink
    p["abdomen"].rot[0] = -6.0 * sink + 10.0 * curl
    for side in ("l", "r"):
        p[f"chelicera_{side}"].rot[0] = -30.0 * curl
        p[f"palp1_{side}"].rot[0] = -40.0 * curl

    rest = rest_targets()
    if curl < 1e-4:                        # 失稳段：脚内滑，身体跟着塌
        for pair, side in LEG_KEYS:
            tgt = rest[(pair, side)].copy()
            tgt[0] *= 1.0 - 0.30 * buckle
            tgt[2] *= 1.0 - 0.15 * buckle
            rig.solve_leg(p, pair, side, tgt)
    else:                                  # 蜷缩段：直接向折叠腿姿收（死蜷）
        s = min(1.0, curl + spasm)
        _mix_fold(p, _FOLD, s, _LEG_BONES)
        if curl < 1.0:                     # 蜷缩前半仍有脚拖地，与失稳段衔接
            pass
    low = rig.lowest(p)                    # 沉底夹持：别穿进地面
    if low < 0.0:
        p["root"].pos[1] -= low
    return p


# name → (时长秒, 是否循环, 采样数, 生成函数)
ANIMS = {
    "idle":         (6.00, True, 40, anim_idle),
    "walk":         (1.10, True, 32, anim_walk),
    "run":          (0.55, True, 28, anim_run),
    "retreat":      (0.50, True, 26, anim_retreat),
    "ambush_burst": (0.25, False, 14, anim_ambush_burst),
    "fold":         (0.55, False, 22, anim_fold),
    "bite":         (0.55, False, 24, anim_bite),
    "hurt":         (0.40, False, 18, anim_hurt),
    "death":        (1.80, False, 32, anim_death),
}


def sample(rig: SpiderRig, name: str, t01: float) -> Pose:
    return ANIMS[name][3](rig, t01)


# ---------------------------------------------------------------- 导出
def _uuid(seed: str) -> str:
    """确定性合法 v4 uuid（md5 熵 + 版本位修正，防关键帧撞车）。"""
    return str(uuid.UUID(bytes=hashlib.md5(seed.encode()).digest(), version=4))


def _kf(channel: str, time: float, vec, idx: int, aname: str) -> dict:
    return {
        "channel": channel,
        "data_points": [{"x": f"{vec[0]:.4f}", "y": f"{vec[1]:.4f}", "z": f"{vec[2]:.4f}"}],
        "uuid": _uuid(f"{aname}{channel}{idx}"),
        "time": round(time, 4),
        "color": -1,
        "interpolation": "linear",
        "bezier_linked": True,
        "bezier_left_time": [-0.1, -0.1, -0.1],
        "bezier_left_value": [0, 0, 0],
        "bezier_right_time": [0.1, 0.1, 0.1],
        "bezier_right_value": [0, 0, 0],
    }


def build_tracks(rig: SpiderRig, name: str) -> tuple[float, bool, dict[str, dict[str, list]]]:
    length, loop, n, _ = ANIMS[name]
    frames = []
    for i in range(n + 1):
        t = i / n
        if loop and i == n:
            frames.append((length, frames[0][1]))       # 循环末帧 = 首帧，接缝为零
            break
        frames.append((t * length, sample(rig, name, t)))

    tracks: dict[str, dict[str, list]] = {}
    for bone in rig.order:
        for chan, attr, default in (("rotation", "rot", 0.0), ("position", "pos", 0.0),
                                    ("scale", "scale", 1.0)):
            vals = [(tt, list(getattr(pz[bone], attr)) if bone in pz else [default] * 3)
                    for tt, pz in frames]
            if all(abs(v[k] - default) < 1e-4 for _, v in vals for k in range(3)):
                continue
            tracks.setdefault(bone, {})[chan] = vals
    return length, loop, tracks


def _bone_uuids_410(doc: dict) -> dict[str, str]:
    """4.10 格式骨骼 uuid：从 outliner 内联树里走（没有 5.0 的 groups[]）。"""
    out: dict[str, str] = {}

    def walk(node):
        if isinstance(node, str):
            return
        out[node["name"]] = node["uuid"]
        for c in node.get("children", []):
            walk(c)

    for root in doc["outliner"]:
        walk(root)
    return out


def write_bbmodel(rig: SpiderRig, names: list[str], out: Path) -> None:
    doc = json.loads(SHELL.read_text())
    uuids = _bone_uuids_410(doc)
    anims = []
    for name in names:
        length, loop, tracks = build_tracks(rig, name)
        animators = {}
        for bone, chans in tracks.items():
            kfs = []
            for chan, vals in chans.items():
                for i, (tt, v) in enumerate(vals):
                    kfs.append(_kf(chan, tt, v, i, f"{name}{bone}{chan}"))
            animators[uuids[bone]] = {"name": bone, "type": "bone", "keyframes": kfs}
        anims.append({
            "uuid": _uuid(f"anim:{name}"),
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
        })
    doc["animations"] = anims
    doc["name"] = "MimicSpiderRig"
    doc["model_identifier"] = "MimicSpiderRig"
    out.write_text(json.dumps(doc, ensure_ascii=False))


def write_geckolib(rig: SpiderRig, names: list[str], out: Path) -> None:
    """直出 GeckoLib animation.json——**参考/兜底**。正经资产路径是把
    MimicSpiderRig.bbmodel 交给 bbmodel_to_geckolib.py（官方 codec 导出），
    旋转符号约定由 codec 负责。"""
    animations = {}
    for name in names:
        length, loop, tracks = build_tracks(rig, name)
        bones = {}
        for bone, chans in tracks.items():
            entry = {}
            for chan, vals in chans.items():
                entry[chan] = {str(round(tt, 4)): [round(v[0], 4), round(v[1], 4), round(v[2], 4)]
                               for tt, v in vals}
            bones[bone] = entry
        animations[f"animation.{NAMESPACE}.{MODEL_ID}.{name}"] = {
            "loop": bool(loop),
            "animation_length": round(length, 4),
            "bones": bones,
        }
    out.write_text(json.dumps({"format_version": "1.8.0", "animations": animations},
                              indent="\t", ensure_ascii=False))


def main() -> int:
    ap = argparse.ArgumentParser(description="拟态灰烬蛛动画生成")
    ap.add_argument("--only", nargs="*", help="只生成这些动画")
    args = ap.parse_args()
    names = args.only or list(ANIMS)
    rig = SpiderRig()
    write_bbmodel(rig, names, OUT_BB)
    write_geckolib(rig, names, OUT_JSON)
    total = 0
    for name in names:
        length, loop, tracks = build_tracks(rig, name)
        kf = sum(len(v) for c in tracks.values() for v in c.values())
        total += kf
        print(f"  {name:<13} {length:4.2f}s {'循环' if loop else '单次'}  骨 {len(tracks):2d}  关键帧 {kf}")
    print(f"→ {OUT_BB.name} / {OUT_JSON.name}  共 {total} 关键帧")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
