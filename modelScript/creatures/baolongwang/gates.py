"""暴龙王附加装饰的资产契约与缺陷注入。"""

from __future__ import annotations

import copy
import json

import numpy as np

from bbmodel_maker.gates.gatekit import GateResult
from bbmodel_maker.rig.rigkit import _rotmat3
from modelScript.creatures.baolongwang import gen_decorations as gen


def build():
    return json.loads((gen.MODELS / "BaolongwangDecorated_round3.bbmodel").read_text())


def ownership(doc):
    parents, owners = {}, {}
    definitions = {g["uuid"]: g["name"] for g in doc["groups"]}

    def visit(node, parent):
        if isinstance(node, str):
            owners.setdefault(node, []).append(parent)
            return
        name = definitions[node["uuid"]]
        parents[name] = parent
        for child in node.get("children", []):
            visit(child, name)

    for node in doc["outliner"]:
        visit(node, None)
    return parents, owners


def original_preserved(doc):
    base = gen.load_base()
    old_elements = {e["uuid"]: e for e in base["elements"]}
    elements = {e["uuid"]: e for e in doc["elements"]}
    errors = ["原几何/UV 被更改或丢失" for uid, el in old_elements.items() if elements.get(uid) != el]
    groups = {g["uuid"]: g for g in doc["groups"]}
    errors += ["原骨骼枢轴/旋转被更改" for g in base["groups"] if groups.get(g["uuid"]) != g]
    parents, owners = ownership(doc)
    old_parents, old_owners = ownership(base)
    if any(parents.get(n) != p for n, p in old_parents.items()) or any(owners.get(u) != p for u, p in old_owners.items()):
        errors.append("原绑定层级被更改")
    return errors


def animations_preserved(doc):
    return [] if doc["animations"] == gen.load_base()["animations"] else ["原动画关键帧或轨道被更改"]


def texture_preserved(doc):
    base = gen.load_base()
    old, new = gen.texture_image(base["textures"][0]), gen.texture_image(doc["textures"][0])
    errors = []
    if not np.array_equal(np.array(old), np.array(new.crop((0, 0, old.width, old.height)))):
        errors.append("原贴图像素区被更改")
    for axis, pixels in (("width", new.width), ("height", new.height)):
        old_pixels = getattr(old, axis)
        if abs(doc["resolution"][axis] / pixels - base["resolution"][axis] / old_pixels) > 1e-9:
            errors.append("原 UV 与贴图像素比例漂移")
    return errors


def attachment_contract(doc):
    parents, owners = ownership(doc)
    errors = []
    for name in ("organ_furnace", "organ_fusion", "human_a_upper_arm", "human_b_upper_arm"):
        if parents.get(name) != "bdk_body":
            errors.append(f"{name} 没有随躯干绑定")
    for side in ("a", "b"):
        if parents.get(f"human_{side}_forearm") != f"human_{side}_upper_arm" or parents.get(f"human_{side}_hand") != f"human_{side}_forearm":
            errors.append(f"human_{side} 肩肘腕链断开")
    ids = [e["uuid"] for e in doc["elements"]]
    if len(set(ids)) != len(ids) or any(len(owners.get(uid, [])) != 1 for uid in ids):
        errors.append("元素存在重复或缺失绑定")
    return errors


def valid_geometry(doc):
    old_ids = {e["uuid"] for e in gen.load_base()["elements"]}
    errors = []
    for e in doc["elements"]:
        if e["uuid"] in old_ids:
            continue
        size = np.array(e["to"]) - e["from"]
        if not np.isfinite(size).all() or min(size) < .3:
            errors.append(f"{e['name']} 存在退化尺寸")
        if not np.isfinite(e["rotation"] + e["origin"]).all():
            errors.append(f"{e['name']} 旋转或枢轴无效")
        for face in e["faces"].values():
            uv = np.array(face["uv"])
            if not np.isfinite(uv).all() or min(uv) < 0 or max(uv[[0, 2]]) > doc["resolution"]["width"] or max(uv[[1, 3]]) > doc["resolution"]["height"]:
                errors.append(f"{e['name']} UV 越界")
    return errors


def embedded_furnace(doc):
    shell = next((e for e in doc["elements"] if e["name"] == "furnace_buried_shell"), None)
    if shell is None:
        return ["炉体缺失"]
    # 在炉体局部空间均匀采样，再测原胸腹旋转盒的并集；AABB 会高估嵌入深度。
    low, high = np.array(shell["from"]), np.array(shell["to"])
    axes = [np.linspace(a + (b-a)/24, b - (b-a)/24, 12) for a, b in zip(low, high)]
    points = np.array(np.meshgrid(*axes)).reshape(3, -1).T
    origin = np.array(shell["origin"])
    points = (points-origin) @ np.array(_rotmat3(shell["rotation"])).T + origin
    base = gen.load_base()
    _, owners = ownership(base)
    inside = np.zeros(len(points), dtype=bool)
    for e in base["elements"]:
        if owners.get(e["uuid"]) != ["bdk_body"]:
            continue
        origin = np.array(e["origin"])
        local = (points-origin) @ np.array(_rotmat3(e["rotation"])) + origin
        inside |= np.all((local >= e["from"]) & (local <= e["to"]), axis=1)
    fraction = inside.mean()
    return [] if fraction >= 2/3 else [f"炉体嵌入仅 {fraction:.1%}，应至少 2/3"]


class DecorationGates:
    checks = (("original", "原几何与骨树保留", original_preserved),
              ("animation", "五条原动画保留", animations_preserved),
              ("texture", "原贴图与 UV 比例保留", texture_preserved),
              ("attachment", "躯干与人臂绑定", attachment_contract),
              ("geometry", "新增几何与 UV 有效", valid_geometry),
              ("embedded", "炉体至少三分之二嵌入", embedded_furnace))

    def run_all(self, doc):
        return [GateResult(key, label, check(doc)) for key, label, check in self.checks]

    def report(self, doc):
        results = self.run_all(doc)
        for result in results:
            print(f"{'OK' if result.ok else 'FAIL'} {result.label}: {result.violations}")
        return sum(not result.ok for result in results)

    def self_test(self, doc):
        broken = 0
        for key, label, check in self.checks:
            bad = copy.deepcopy(doc)
            if key == "original":
                bad["elements"][0]["from"][0] += 1
            elif key == "animation":
                bad["animations"][0]["length"] += 1
            elif key == "texture":
                bad["resolution"]["width"] /= 2
            elif key == "attachment":
                # 实际从 outliner 脱离炉体，而非改一个辅助标记。
                uid = gen.stable_id("organ_furnace")
                def detach(node):
                    if isinstance(node, dict):
                        node["children"] = [c for c in node.get("children", []) if not isinstance(c, dict) or c["uuid"] != uid]
                        for child in node["children"]:
                            detach(child)
                for node in bad["outliner"]:
                    detach(node)
            elif key == "geometry":
                bad["elements"][-1]["to"][0] = bad["elements"][-1]["from"][0]
            else:
                shell = next(e for e in bad["elements"] if e["name"] == "furnace_buried_shell")
                for field in ("from", "to", "origin"):
                    shell[field][2] -= 50
            valid = not check(doc) and bool(check(bad))
            broken += not valid
            print(f"{'OK' if valid else 'FAIL'} 差分自证 {label}")
        return broken


GATES = DecorationGates()

if __name__ == "__main__":
    model = build()
    raise SystemExit(GATES.report(model) + GATES.self_test(model))
