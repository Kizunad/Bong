#!/usr/bin/env python3
"""校准动画的离线预览：骨骼正向运动学摆姿 + bbmodel_maker 渲染，输出三视角关键帧图和 GIF。

姿态矩阵与模组一致：M = 父 · T(位移) · T(枢轴) · Rz·Ry·Rx · T(−枢轴)，数值为导出前的面板值
（读 JSON 后把旋转 X/Y、位移 X 反号还原）。零姿态之外的骨按预览底姿（默认全零）。

渲染使用「预览模板」：切口面用同一方块的一个像素填色，关节弯折时不露洞；
这只是离线观感，游戏内由模组蒙皮连接，不代表实机弯折外观。
"""

from __future__ import annotations

import argparse
import copy
import json
import math
import tempfile
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

from bbmodel_maker.render import framing, render_bbmodel

from . import anim, bbmodel, compare, rig

SIZE = 300


def _rot(deg, axis):
    a = math.radians(deg)
    c, s = math.cos(a), math.sin(a)
    m = np.eye(4)
    i, j = [(1, 2), (2, 0), (0, 1)][axis]
    m[i, i], m[i, j], m[j, i], m[j, j] = c, -s, s, c
    return m


def _translate(v):
    m = np.eye(4)
    m[:3, 3] = v
    return m


def sample(channel, t):
    """线性采样 Bedrock 通道，返回导出值向量。"""
    if isinstance(channel, list):
        return list(channel)
    frames = sorted((float(k), v) for k, v in channel.items())
    if t <= frames[0][0]:
        return list(frames[0][1])
    for (t0, v0), (t1, v1) in zip(frames, frames[1:]):
        if t < t1:
            f = (t - t0) / (t1 - t0)
            return [a + (b - a) * f for a, b in zip(v0, v1)]
    return list(frames[-1][1])


def panel_pose(document: dict, t: float, mirror: bool = False) -> dict:
    """某时刻 骨名 -> (位移, 旋转)，面板数值；mirror 按契约镜像左右。"""
    body = next(iter(document["animations"].values()))
    pose = {}
    for bone, tracks in body["bones"].items():
        pos = sample(tracks["position"], t) if "position" in tracks else [0, 0, 0]
        rot = sample(tracks["rotation"], t) if "rotation" in tracks else [0, 0, 0]
        pos = [-pos[0], pos[1], pos[2]]  # 还原 Blockbench 面板值
        rot = [-rot[0], -rot[1], rot[2]]
        if mirror:
            pos[0] = -pos[0]
            rot[1], rot[2] = -rot[1], -rot[2]
            bone = bone.replace("r_", "§").replace("l_", "r_").replace("§", "l_") if bone[:2] in ("r_", "l_") else bone
        pose[bone] = (pos, rot)
    return pose


def bone_matrices(pivots: dict, pose: dict) -> dict:
    mats = {}
    for name in rig.BONE_ORDER:
        pos, rot = pose.get(name, ([0, 0, 0], [0, 0, 0]))
        pivot = pivots[name]
        m = _translate(pos) @ _translate(pivot) @ (_rot(rot[2], 2) @ _rot(rot[1], 1) @ _rot(rot[0], 0)) @ _translate(-np.array(pivot, float))
        parent = rig.PARENT[name]
        mats[name] = m if parent is None else mats[parent] @ m
    return mats


def preview_template(variant: str) -> tuple[dict, dict, dict]:
    """渲染用模板：外层展开，切口面补一个像素色。返回 (模型, 元素->骨, 枢轴)。"""
    model = compare.render_ready_template(variant)
    bones = rig.build(variant == "slim")
    owner = {}
    for bone in bones:
        for cube in bone.cubes:
            uid = bbmodel._id(variant, bone.name, "outer" if cube.outer else "inner")
            owner[uid] = bone.name
            element = next(e for e in model["elements"] if e["uuid"] == uid)
            fill = next(iter(cube.faces.values()))
            texel = [fill[0], fill[1], fill[0] + math.copysign(1, fill[2] - fill[0]) * 0.99,
                     fill[1] + math.copysign(1, fill[3] - fill[1]) * 0.99]
            for face in ("north", "south", "east", "west", "up", "down"):
                element["faces"].setdefault(face, {"uv": [round(v, 3) for v in texel], "texture": 0})
    return model, owner, {b.name: b.pivot for b in bones}


class Previewer:
    def __init__(self, variant: str = "classic"):
        self.model, self.owner, self.pivots = preview_template(variant)
        self.tmp = Path(tempfile.mkdtemp()) / "preview.bbmodel"
        self.tmp.write_text(json.dumps(self.model))
        self.views = framing.views_for(framing.LEGACY_FACING, ("FRONT", "SIDE_R", "3/4"))

    def frame(self, pose: dict, view, focus=((0, 16, 0), 46), size=SIZE):
        mats = bone_matrices(self.pivots, pose)
        xform = {uid: mats[bone] for uid, bone in self.owner.items()}
        image, _ = render_bbmodel.render(self.tmp, yaw=view.yaw, pitch=view.pitch, size=size, focus=focus,
                                         xform=xform, shading="mc", bg=(34, 36, 42))
        return image

    def sheet(self, rows, out: Path, title: str):
        """rows: [(标签, pose)]，每行三视角。"""
        gap, label = 8, 22
        width = SIZE * 3 + gap * 4
        canvas = Image.new("RGB", (width, label + (SIZE + gap) * len(rows) + gap), (14, 15, 17))
        draw = ImageDraw.Draw(canvas)
        draw.text((gap, 4), title, fill=(220, 220, 220))
        for r, (text, pose) in enumerate(rows):
            y = label + r * (SIZE + gap)
            for c, view in enumerate(self.views):
                canvas.paste(self.frame(pose, view), (gap + c * (SIZE + gap), y))
            draw.text((gap + 4, y + 4), text, fill=(255, 230, 120))
        out.parent.mkdir(parents=True, exist_ok=True)
        canvas.save(out)

    def gif(self, document: dict, out: Path, step: float, duration_ms: int, base=None, mirror=False):
        length = next(iter(document["animations"].values()))["animation_length"]
        frames = []
        t = 0.0
        while t < length - 1e-9:
            pose = dict(base or {})
            pose.update(panel_pose(document, t, mirror))
            frames.append(self.frame(pose, self.views[2], size=260))
            t += step
        out.parent.mkdir(parents=True, exist_ok=True)
        frames[0].save(out, save_all=True, append_images=frames[1:], duration=duration_ms, loop=0)
        return len(frames)


WALK = {"r_thigh": ([0, 0, 0], [25, 0, 0]), "l_thigh": ([0, 0, 0], [-25, 0, 0]),
        "r_shin": ([0, 0, 0], [-10, 0, 0]), "l_shin": ([0, 0, 0], [0, 0, 0])}


def run(out_root: Path) -> None:
    pv = Previewer("classic")
    docs = {name: build() for name, (build, *_r) in anim.ANIMATIONS.items()}

    def at(name, t, mirror=False, base=None):
        pose = dict(base or {})
        pose.update(panel_pose(docs[name], t, mirror))
        return pose

    key_times = {
        "calib_zero": [0.0],
        "calib_item": [0.0, 0.75, 1.5, 2.25],
        "calib_mirror": [0.0, 0.3, 0.5, 0.9],
        "calib_layer": [0.0, 0.3, 0.6, 0.9],
    }
    for name, times in key_times.items():
        base = WALK if name == "calib_layer" else None
        rows = [(f"{name}  t={t:g}s", at(name, t, base=base)) for t in times]
        pv.sheet(rows, out_root / name / "keyframes.png", f"{name}（三视角：正面 / 右侧 / 3/4）")
    # 左主手：按契约镜像（换骨名、位移 X 与旋转 Y/Z 反号）
    pv.sheet([(f"calib_mirror 左主手  t={t:g}s", at("calib_mirror", t, mirror=True)) for t in key_times["calib_mirror"]],
             out_root / "calib_mirror" / "keyframes_left_hand.png", "calib_mirror 左主手（契约镜像规则）")
    # calib_axes：25 根骨各取 X/Y/Z 峰值
    block = 3 * 2 * anim.STEP
    rows = []
    for index, bone in enumerate(anim.JOINTS):
        for axis, letter in enumerate("XYZ"):
            t = index * block + axis * 2 * anim.STEP + anim.STEP
            rows.append((f"{bone} {letter}+{anim.SWING}°  t={t:g}s", at("calib_axes", t)))
    # 总表按每 5 根骨一张，避免单图过长
    for part in range(0, len(anim.JOINTS), 5):
        chunk = rows[part * 3:(part + 5) * 3]
        pv.sheet(chunk, out_root / "calib_axes" / f"keyframes_{part // 5 + 1}.png",
                 f"calib_axes：{anim.JOINTS[part]} … {anim.JOINTS[min(part + 4, len(anim.JOINTS) - 1)]}")
    # GIF
    pv.gif(docs["calib_zero"], out_root / "calib_zero" / "anim.gif", 0.5, 500)
    pv.gif(docs["calib_item"], out_root / "calib_item" / "anim.gif", 0.1, 100)
    pv.gif(docs["calib_mirror"], out_root / "calib_mirror" / "anim.gif", 0.05, 50)
    pv.gif(docs["calib_mirror"], out_root / "calib_mirror" / "anim_left_hand.gif", 0.05, 50, mirror=True)
    pv.gif(docs["calib_layer"], out_root / "calib_layer" / "anim.gif", 0.05, 50, base=WALK)
    pv.gif(docs["calib_axes"], out_root / "calib_axes" / "anim.gif", 0.25, 50)  # 5 倍速


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", type=Path, required=True)
    run(parser.parse_args().out)


if __name__ == "__main__":
    main()
