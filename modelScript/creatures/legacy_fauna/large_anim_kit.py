"""大型旧生物（黑虎 / 活柱 / 毒龙 / 骨龙 / 黑武士）补动画用的小工具。

只做两件事：
1. ``Clip`` 把「每根骨头一串关键帧」写成 GeckoLib 1.8 的 JSON 形状；
2. ``append_clips`` 往已安装的 ``<id>.animation.json`` **追加**新段，已有段一律不碰
   （名字撞了直接报错，而不是悄悄覆盖）。

关键帧数值全部在各 creature 的脚本里从零写，这里不放任何生物的具体姿态。
"""
from __future__ import annotations

import json
import math
from pathlib import Path
from typing import Callable, Iterable, Sequence

ASSETS = Path(__file__).resolve().parents[3] / "client/src/main/resources/assets/bong"
CLIP_PREFIX = "animation.bong."

Vec = Sequence[float]


def tau(phase: float) -> float:
    """相位 0..1 → 弧度，写循环波形时少打一串 2π。"""

    return phase * 2.0 * math.pi


def key(time: float) -> str:
    return str(round(time, 3))


class Clip:
    """一段动画。``loop=True`` 的段要求每条轨道首尾同值，``wave`` 自动保证。"""

    def __init__(self, length: float, loop: bool):
        self.length = length
        self.loop = loop
        self.bones: dict[str, dict[str, dict[str, list[float]]]] = {}

    def track(self, bone: str, channel: str, frames: dict[float, Vec]) -> "Clip":
        """手写关键帧。一次性动画要自己保证 0 秒与终点都有帧。"""

        self.bones.setdefault(bone, {})[channel] = {
            key(time): [round(float(v), 3) for v in value] for time, value in sorted(frames.items())
        }
        return self

    def rot(self, bone: str, frames: dict[float, Vec]) -> "Clip":
        return self.track(bone, "rotation", frames)

    def pos(self, bone: str, frames: dict[float, Vec]) -> "Clip":
        return self.track(bone, "position", frames)

    def scale(self, bone: str, frames: dict[float, Vec]) -> "Clip":
        return self.track(bone, "scale", frames)

    def wave(self, bone: str, channel: str, fn: Callable[[float], Vec], steps: int = 8) -> "Clip":
        """按相位 0..1 采样 ``steps`` 段并闭合首尾（循环动画专用）。fn 必须以 1 为周期。"""

        frames = {self.length * i / steps: fn(i / steps) for i in range(steps)}
        frames[self.length] = fn(0.0)
        return self.track(bone, channel, frames)

    def to_json(self) -> dict:
        return {"loop": self.loop, "animation_length": self.length, "bones": self.bones}

    def bone_names(self) -> Iterable[str]:
        return self.bones.keys()


def append_clips(creature: str, clips: dict[str, Clip], existing: set[str]) -> list[str]:
    """把 ``clips`` 追加进 ``animations/<creature>.animation.json``。

    ``existing`` 是开工前该文件里已有的段短名；新段名与它重名就报错——已有段一帧都不改。
    重复运行本脚本是幂等的：只会覆写上一次自己追加的同名段。
    返回追加的完整段名，供调用方打印。
    """

    path = ASSETS / "animations" / f"{creature}.animation.json"
    text = path.read_text(encoding="utf-8")
    document = json.loads(text)
    geo = json.loads((ASSETS / "geo" / f"{creature}.geo.json").read_text(encoding="utf-8"))
    bones = {bone["name"] for bone in geo["minecraft:geometry"][0]["bones"]}

    written = []
    for short, clip in clips.items():
        if short in existing:
            raise ValueError(f"{creature}.{short} 是已有动画段，不允许改写")
        unknown = set(clip.bone_names()) - bones
        if unknown:
            raise ValueError(f"{creature}.{short} 引用了 geo 里没有的骨：{sorted(unknown)}")
        full = f"{CLIP_PREFIX}{creature}.{short}"
        document["animations"][full] = clip.to_json()
        written.append(full)

    path.write_text(json.dumps(document, indent=2, ensure_ascii=False) + ("\n" if text.endswith("\n") else ""),
                    encoding="utf-8")
    return written
