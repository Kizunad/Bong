"""旧静态生物的作者流水线：读取已导入 bbmodel，拆分左右肢体、设置关节，再烘焙动作。

sources 是导入前几何的可编辑快照；不从安装后的 client 反复导入，避免分组越跑越多。
"""
from __future__ import annotations

import copy
import json
import math
from pathlib import Path
from uuid import NAMESPACE_URL, uuid5

HERE = Path(__file__).resolve().parent
OUT = HERE.parents[1] / "models" / "legacy_fauna"
SPECIES = ("void_distorted", "daoxiang", "zhinian", "tsy_sentinel", "fuya", "skull_fiend")


def build(source: dict, name: str) -> dict:
    doc = copy.deepcopy(source)
    elements = {element["uuid"]: element for element in doc["elements"]}
    bones = {}

    def regroup(group):
        bones[group["name"]] = group
        cubes = [elements[child] for child in group["children"] if isinstance(child, str)]
        if group["name"] in ("Arms", "Legs", "Tendrils", "Pauldrons") and len(cubes) == 2:
            group["children"] = [child for child in group["children"] if not isinstance(child, str)]
            for cube in cubes:
                part = group["name"] + ("_L" if cube["from"][0] + cube["to"][0] > 0 else "_R")
                joint = [(cube["from"][i] + cube["to"][i]) / 2 for i in range(3)]
                joint[1] = cube["to"][1] - 0.5
                group["children"].append({"uuid": str(uuid5(NAMESPACE_URL, name + "/" + part)),
                                          "name": part, "origin": joint, "rotation": [0, 0, 0],
                                          "children": [cube["uuid"]]})
        elif cubes and not group["name"].endswith(("_L", "_R")):
            lo = [min(cube["from"][i] for cube in cubes) for i in range(3)]
            hi = [max(cube["to"][i] for cube in cubes) for i in range(3)]
            group["origin"] = [(a + b) / 2 for a, b in zip(lo, hi)]
            if group["name"] == "Head": group["origin"][1] = lo[1]
        for child in group["children"]:
            if isinstance(child, dict): regroup(child)

    for group in doc["outliner"]: regroup(group)
    animations = []

    def clip(label, duration, loop, tracks):
        animators = {}
        for bone, channels in tracks.items():
            if bone not in bones: raise ValueError(f"{name}/{label}: 未知骨 {bone}")
            keys = []
            for channel, sample in channels.items():
                for frame in range(17):
                    vector = sample(frame / 16)
                    keys.append({"channel": channel, "time": round(duration * frame / 16, 5),
                                 "interpolation": "linear", "data_points": [dict(zip("xyz", vector))]})
            animators[bones[bone]["uuid"]] = {"name": bone, "type": "bone", "keyframes": keys}
        animations.append({"uuid": str(uuid5(NAMESPACE_URL, name + "/" + label)), "name": label,
                           "length": duration, "loop": "loop" if loop else "once", "animators": animators})

    def wave(t): return math.sin(t * math.tau)

    # 地面躯体呼吸、漂浮生物起伏，分别保留自己的幅度与节律。
    hover = name in ("zhinian", "fuya", "skull_fiend")
    idle = {"Body": {"position": lambda t: [0, (0.5 if hover else 0.12) * (1 - math.cos(t * math.tau)), 0]},
            "Head": {"rotation": lambda t: [1.5 * wave(t), 3 * wave(t), 0]}}
    if name == "void_distorted":
        idle["VoidCore"] = {"scale": lambda t: [1 + 0.045 * wave(t)] * 3}
    if name == "fuya":
        idle["CollapseCore"] = {"scale": lambda t: [1 + 0.06 * wave(t)] * 3}
    if name == "zhinian":
        idle["Mist"] = {"scale": lambda t: [1 + 0.025 * wave(t), 1, 1 + 0.025 * wave(t)]}
    if name == "skull_fiend":
        idle["LeftSkull"] = {"rotation": lambda t: [0, 8 * wave(t), 3 * wave(t)]}
        idle["RightSkull"] = {"rotation": lambda t: [0, -8 * wave(t), -3 * wave(t)]}
    for bone in bones:
        if bone.startswith("Tendrils_"):
            sign = 1 if bone.endswith("L") else -1
            idle[bone] = {"rotation": lambda t, sign=sign: [5 * wave(t), 0, sign * 4 * wave(t)]}
    clip("idle", 4.2 if hover else 3.0, True, idle)

    walking = copy.copy(idle)
    walking["Body"] = {"position": lambda t: [0, 0.3 * (1 - math.cos(t * math.tau * 2)), 0],
                       "rotation": lambda t: [3 if hover else 0, 0, 2 * wave(t)]}
    for bone in bones:
        if bone.startswith(("Arms_", "Legs_", "Tendrils_", "Pauldrons_")):
            sign = (1 if bone.endswith("L") else -1) * (-1 if bone.startswith("Arms") else 1)
            amplitude = 23 if bone.startswith("Legs") else 12 if bone.startswith("Arms") else 6
            walking[bone] = {"rotation": lambda t, a=amplitude, s=sign: [a * s * wave(t), 0, 0]}
    clip("walk", 1.15 if name == "tsy_sentinel" else 0.95, True, walking)
    clip("hurt", 0.4, False, {"Body": {"rotation": lambda t: [-8 * math.sin(t * math.pi), 0, 0]},
                              "Head": {"rotation": lambda t: [0, 0, 9 * math.sin(t * math.pi)]}})
    clip("death", 1.4, False, {"Body": {"rotation": lambda t: [0, 0, 75 * t * t],
                                       "position": lambda t: [0, -3 * t, 0]}})
    doc["animations"] = animations
    return doc


if __name__ == "__main__":
    OUT.mkdir(parents=True, exist_ok=True)
    for name in SPECIES:
        source = json.loads((HERE / "sources" / f"{name}.bbmodel").read_text())
        model = build(source, name)
        (OUT / f"{name}Rig.bbmodel").write_text(json.dumps(model, indent=2) + "\n")
        print(f"{name}: 分组与 idle/walk/hurt/death 已烘焙")
