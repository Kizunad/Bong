#!/usr/bin/env python3
"""马 —— 动画回读校验：只信**文件里的关键帧**，不信生成时的 Pose。

为什么必须有这一层：`render_anim.py` 渲的是 `rig.element_xform(pose)`，也就是生成器
内存里的姿态；Blockbench 播的是写进 `animations` 数组的关键帧。中间隔着采样、RDP
抽稀、四位小数字符串化三道有损环节，任何一处出错，预览全绿而 Blockbench 里散架——
这正是首版翻车的方式。这里从 bbmodel 反向读回骨树与关键帧，独立求值，再和生成器
逐骨对拍。

这一层只保证"文件 == 生成器"。**"生成器 == Blockbench"**由 `--probe` 打印的实测步骤
保证，两者合起来才闭环。

用法:
  python3 scripts/models/horse/verify_anim.py            # 全部模型 × 全部动画
  python3 scripts/models/horse/verify_anim.py --probe    # 打印 Blockbench 通道约定的复现步骤
"""

from __future__ import annotations

import argparse
import json
import sys
from bisect import bisect_left
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
sys.path.insert(0, str(HERE))

from rig import FINAL, Channel, Pose, Rig, affine, bb_pos, bb_rot, euler  # noqa: E402

CHANNELS = {"rotation": ("rot", 0.0), "position": ("pos", 0.0), "scale": ("scale", 1.0)}
# 与写盘同一对函数（对合），把文件里的 Blockbench 通道值转回几何约定
UNCONV = {"rotation": bb_rot, "position": bb_pos, "scale": list}


class Playback:
    """从 bbmodel 的 animations 数组独立求值——不碰生成器的任何函数。"""

    def __init__(self, path: Path):
        self.doc = json.loads(Path(path).read_text())
        self.origins: dict[str, np.ndarray] = {}
        self.parent: dict[str, str | None] = {}
        self.order: list[str] = []
        self.elements: dict[str, list[str]] = {}
        self.by_uuid: dict[str, str] = {}

        def walk(node, par):
            if isinstance(node, str):
                if par:
                    self.elements.setdefault(par, []).append(node)
                return
            name = node["name"]
            self.by_uuid[node["uuid"]] = name
            self.origins[name] = np.array(node.get("origin", [0, 0, 0]), float)
            self.parent[name] = par
            self.order.append(name)
            for c in node.get("children", []):
                walk(c, name)

        for r in self.doc["outliner"]:
            walk(r, None)

        self.anims: dict[str, dict] = {}
        for a in self.doc.get("animations", []):
            tracks: dict[str, dict[str, tuple[list[float], np.ndarray]]] = {}
            for uid, an in a["animators"].items():
                bone = self.by_uuid.get(uid)
                if bone is None:
                    raise KeyError(f"{a['name']}: animator uuid {uid} 不在骨树里（动画会静默不生效）")
                if bone != an.get("name"):
                    raise ValueError(f"{a['name']}: animator name={an['name']} 与 uuid 指向的 {bone} 不符")
                per: dict[str, list[tuple[float, list[float]]]] = {}
                for kf in an["keyframes"]:
                    dp = kf["data_points"][0]
                    per.setdefault(kf["channel"], []).append(
                        (float(kf["time"]), [float(dp[c]) for c in "xyz"])
                    )
                for ch, vals in per.items():
                    vals.sort(key=lambda kv: kv[0])
                    tracks.setdefault(bone, {})[ch] = (
                        [t for t, _ in vals],
                        np.array([v for _, v in vals], float),
                    )
            self.anims[a["name"]] = {
                "length": float(a["length"]),
                "loop": a["loop"] == "loop",
                "tracks": tracks,
            }

    def channel(self, name: str, bone: str, ch: str, time: float) -> list[float]:
        """求值后转回几何约定。**插值在 Blockbench 通道空间做**——Blockbench 就是这么插的，
        先转回来再插值会在符号翻转的分量上得到不同结果。"""
        default = [CHANNELS[ch][1]] * 3
        tr = self.anims[name]["tracks"].get(bone, {}).get(ch)
        if tr is None:
            return default
        ts, vs = tr
        if time <= ts[0]:
            v = vs[0]
        elif time >= ts[-1]:
            v = vs[-1]
        else:
            i = bisect_left(ts, time)
            t0, t1 = ts[i - 1], ts[i]
            s = (time - t0) / (t1 - t0) if t1 > t0 else 0.0
            v = vs[i - 1] + (vs[i] - vs[i - 1]) * s  # 关键帧写的是 linear
        return UNCONV[ch](list(v))

    def pose(self, name: str, time: float) -> Pose:
        p = Pose()
        for bone in self.order:
            ch = Channel()
            ch.rot = self.channel(name, bone, "rotation", time)
            ch.pos = self.channel(name, bone, "position", time)
            ch.scale = self.channel(name, bone, "scale", time)
            p[bone] = ch
        return p

    def world(self, name: str, time: float) -> dict[str, np.ndarray]:
        pose = self.pose(name, time)
        out: dict[str, np.ndarray] = {}
        for bone in self.order:
            c = pose[bone]
            R = euler(c.rot)
            if any(abs(s - 1.0) > 1e-6 for s in c.scale):
                R = R @ np.diag(c.scale)
            o = self.origins[bone]
            A = affine(R, o + np.array(c.pos, float) - R @ o)
            par = self.parent[bone]
            out[bone] = A if par is None else out[par] @ A
        return out


def compare(path: Path, profile_key: str, samples: int = 24) -> list[str]:
    """逐骨对拍：生成器姿态 vs 文件回读姿态，取骨上元素角点的最大世界位移偏差。"""
    import gen_anim as G
    from gen_skeleton import PROFILES

    rig = Rig(path)
    P = PROFILES[profile_key]
    pb = Playback(path)
    bad = []
    for name, L in G.clips(list(G.ANIMS), list(G.LOADS.values())):
        clip = L.clip(name)
        if clip not in pb.anims:  # 只出了部分负载档（--load）时照跑文件里有的
            continue
        length = pb.anims[clip]["length"]
        worst, worst_bone, worst_t = 0.0, "", 0.0
        for i in range(samples):
            t01 = i / samples
            Wa = rig.world(G.sample(rig, P, name, t01, L))
            Wb = pb.world(clip, t01 * length)
            for bone in rig.order:
                pts = rig.bone_points(bone)
                if not len(pts):
                    continue
                pa = pts @ Wa[bone][:3, :3].T + Wa[bone][:3, 3]
                pb_ = pts @ Wb[bone][:3, :3].T + Wb[bone][:3, 3]
                d = float(np.abs(pa - pb_).max())
                if d > worst:
                    worst, worst_bone, worst_t = d, bone, t01
        flag = "✓" if worst <= 0.35 else "✗"
        line = f"  {flag} {clip:14s} 最大回读偏差 {worst:5.2f} 单位 @ {worst_bone} t={worst_t:.2f}"
        print(line)
        if worst > 0.35:
            bad.append(line)
    return bad


PROBE_DOC = """Blockbench 骨动画通道约定 —— 实测复现步骤（2026-08-07 于 web.blockbench.net）

结论（`rig.bb_rot` / `rig.bb_pos` 就是这两行）：
    animator rotation (rx,ry,rz)  ≡  Rz(rz)·Ry(−ry)·Rx(−rx)      # x/y 取反、z 同号
    animator position (px,py,pz)  ≡  世界位移 (−px, +py, +pz)     # x 取反
    animator scale                ≡  绕 pivot 逐轴相乘，三轴同号
    element **自身**的静态 rotation ≡ Rz·Ry·Rx，三轴同号（与 rig.euler 一致）
最后一条是关键的不对称：**几何层不受影响，只有动画通道要转**。这是 Bedrock 动画格式
带进来的历史包袱，Blockbench 的 BoneAnimator 照单全收。

怎么测的（可重跑）：
  1. 无头 chromium 打开 https://web.blockbench.net/
  2. 页面里 `Codecs.project.load(model, {path:"probe.bbmodel"})` 直接塞一个 model_format
     "free" 的对象——比走文件选择器省事，且能程序化构造探针
  3. 探针模型：每根骨挂一个 1×1×1 标记块、块的 from 角就落在骨 pivot 上，于是
     `cube.mesh.localToWorld(0,0,0)` 读出来的就是该骨 pivot 的世界坐标
  4. `Modes.options.animate.select(); Animation.all[0].select();
     Timeline.setTime(t); Animator.preview();` 然后读各标记块的世界坐标
  5. 单轴各转 30° 定符号；再转 (30,40,50) 定顺序（三列基向量逐一对拍，只有
     Rz(50)·Ry(−40)·Rx(−30) 三列全中）
  6. 终验用**真交付物**的骨树 pivot + 真关键帧值（rear 峰值帧）跑 23 根骨全链，
     Blockbench 与 rig.world() 最大偏差 0.0005 单位

为什么必须实测：首版靠"想当然"认为通道值就是几何角度，预览（渲的是内存 Pose）全绿而
Blockbench 里俯仰相反——马低头吃草变成头卷到肩上、人立的四肢外翻散架。
"""


def main() -> int:
    ap = argparse.ArgumentParser(description="动画回读校验")
    ap.add_argument("--profile", default=None, help="只查这一档")
    ap.add_argument("--coat", default="rust")
    ap.add_argument("--probe", action="store_true", help="打印 Blockbench 通道约定的实测复现步骤")
    args = ap.parse_args()

    if args.probe:
        print(PROBE_DOC)
        return 0

    keys = [args.profile] if args.profile else ["small", "medium", "large"]
    bad = []
    for k in keys:
        p = FINAL / f"HorsePelt_{args.coat}_{k}.bbmodel"
        print(f"[{k}] {p.name}")
        bad += compare(p, k)
    if bad:
        print(f"\n✗ {len(bad)} 条动画回读与生成器不一致")
        return 1
    print("\n✓ 全部动画：文件关键帧回读 == 生成器姿态")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
