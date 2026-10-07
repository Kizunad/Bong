"""v2 生物绑定稿转向的回归测试。

保护的契约：整件绕 Y 转 180° 是刚体旋转，带旋转的 cube 转向后，它在世界里的 8 个角点
必须恰好是原来角点的 (x, z) -> (-x, -z)。执念的飘发、缠布有 50 个带旋转的 cube，
gen_rig.turn_element 原先遇到旋转直接报错；放开后若欧拉角符号写错，模型会在游戏里
悄悄歪掉而不报任何错，所以这里直接比对角点。

欧拉角的合成顺序在这里取 Z·Y·X。180° 绕 Y 的共轭对每个因子单独成立，
所以不依赖 Blockbench 实际用哪种顺序。
"""

from __future__ import annotations

import copy
import itertools
import math
import sys
import unittest
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "creatures" / "fauna_v2"))

import gen_rig  # noqa: E402


def _rotation_matrix(euler_deg: list[float]) -> np.ndarray:
    rx, ry, rz = (math.radians(v) for v in euler_deg)
    rot_x = np.array([[1, 0, 0], [0, math.cos(rx), -math.sin(rx)], [0, math.sin(rx), math.cos(rx)]])
    rot_y = np.array([[math.cos(ry), 0, math.sin(ry)], [0, 1, 0], [-math.sin(ry), 0, math.cos(ry)]])
    rot_z = np.array([[math.cos(rz), -math.sin(rz), 0], [math.sin(rz), math.cos(rz), 0], [0, 0, 1]])
    return rot_z @ rot_y @ rot_x


def _world_corners(element: dict) -> np.ndarray:
    start, end = element["from"], element["to"]
    corners = np.array([
        [x, y, z]
        for x, y, z in itertools.product(*zip(start, end))
    ])
    rotation = element.get("rotation") or [0.0, 0.0, 0.0]
    origin = np.array(element.get("origin", [0.0, 0.0, 0.0]))
    return origin + (corners - origin) @ _rotation_matrix(rotation).T


class TurnElementTest(unittest.TestCase):
    def test_rotated_cube_corners_are_turned_rigidly(self) -> None:
        element = {
            "name": "probe",
            "from": [1.0, 4.0, -2.0],
            "to": [2.5, 9.0, 0.5],
            "origin": [1.5, 8.0, -0.5],
            "rotation": [20.0, 35.0, -50.0],
            "faces": {side: {"uv": [0, 0, 4, 8]} for side in ("north", "south", "east", "west", "up", "down")},
        }
        before = _world_corners(element)
        turned = copy.deepcopy(element)
        gen_rig.turn_element(turned, lift=0.0)
        after = _world_corners(turned)

        expected = before * np.array([-1.0, 1.0, -1.0])
        key = lambda points: sorted(map(tuple, np.round(points, 6)))  # noqa: E731
        self.assertEqual(
            key(after), key(expected),
            "期望：转向后带旋转 cube 的世界角点 = 原角点 (x, z) 取反；"
            "实际不一致，说明欧拉角或旋转中心 origin 没有随整件一起转",
        )

    def test_rotation_without_origin_is_rejected(self) -> None:
        element = {"name": "bad", "from": [0, 0, 0], "to": [1, 1, 1], "rotation": [0, 0, 30], "faces": {}}
        with self.assertRaises(ValueError):
            gen_rig.turn_element(element, lift=0.0)


class ZhinianRigTest(unittest.TestCase):
    def test_every_cube_gets_a_bone(self) -> None:
        """骨表必须认领全部 cube，漏认领的 cube 会让导出直接报错。"""
        source, rig, bones = gen_rig.load_species("zhinian_v2")
        claimed = {uuid for group in _walk(rig["outliner"]) for uuid in group.get("children", []) if isinstance(uuid, str)}
        self.assertEqual(len(rig["elements"]), len(claimed), "期望：每个 cube 挂在恰好一根骨下")
        self.assertEqual(len(bones), len({bone.name for bone in bones}), "骨名必须唯一")


class TsySentinelRigTest(unittest.TestCase):
    def test_every_cube_gets_a_bone(self) -> None:
        """骨表必须认领全部 cube（含 1000 多块石碎片），漏认领会让导出直接报错。"""
        _, rig, bones = gen_rig.load_species("tsy_sentinel_v2")
        claimed = {uuid for group in _walk(rig["outliner"]) for uuid in group.get("children", []) if isinstance(uuid, str)}
        self.assertEqual(len(rig["elements"]), len(claimed), "期望：每个 cube 挂在恰好一根骨下")
        self.assertEqual(len(bones), len({bone.name for bone in bones}), "骨名必须唯一")

    def test_face_ends_up_on_minus_z(self) -> None:
        """建模源面朝 +Z（眉梁在 +Z 侧），绑定稿转向后必须朝游戏里的 -Z。"""
        source, rig, _ = gen_rig.load_species("tsy_sentinel_v2")
        brow_before = next(e for e in source["elements"] if e["name"] == "head_brow")
        brow_after = next(e for e in rig["elements"] if e["name"] == "head_brow")
        self.assertGreater(brow_before["from"][2], 0, "期望：终审稿眉梁在 +Z 侧（脸朝 +Z）")
        self.assertLess(brow_after["to"][2], 0, "期望：转向后眉梁在 -Z 侧（脸朝游戏里的 -Z）；实际没转过去")


class VoidDistortedRigTest(unittest.TestCase):
    def test_every_cube_gets_a_bone(self) -> None:
        """骨表必须认领全部 cube（1456 块），漏认领会让导出直接报错。"""
        _, rig, bones = gen_rig.load_species("void_distorted_v2")
        claimed = {uuid for group in _walk(rig["outliner"]) for uuid in group.get("children", []) if isinstance(uuid, str)}
        self.assertEqual(len(rig["elements"]), len(claimed), "期望：每个 cube 挂在恰好一根骨下")
        self.assertEqual(len(bones), len({bone.name for bone in bones}), "骨名必须唯一")

    def test_maw_ends_up_on_minus_z(self) -> None:
        """建模源面朝 +Z（虚空口在身体前端 +Z 侧），绑定稿转向后必须朝游戏里的 -Z。"""
        source, rig, _ = gen_rig.load_species("void_distorted_v2")
        before = next(e for e in source["elements"] if e["name"] == "maw_void")
        after = next(e for e in rig["elements"] if e["name"] == "maw_void")
        self.assertGreater(before["from"][2], 0, "期望：终审稿虚空口在 +Z 侧（面朝 +Z）")
        self.assertLess(after["to"][2], 0, "期望：转向后虚空口在 -Z 侧（面朝游戏里的 -Z）；实际没转过去")


def _walk(nodes: list) -> list[dict]:
    out = []
    for node in nodes:
        if isinstance(node, dict):
            out.append(node)
            out.extend(_walk(node.get("children", [])))
    return out


if __name__ == "__main__":
    unittest.main()
