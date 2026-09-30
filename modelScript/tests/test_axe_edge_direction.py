"""两把 v2 斧的使用动画里，斧刃朝向必须领着挥砍方向。

保护的契约（用户在审阅页返工过两次）：
- 生铁斧竖劈：落斧那一刻刃口朝下（「斧头方向上下反了」）；
- 骨斧横砍：起手、发力、收势三帧刃口都朝左（「横批方向斧头也是反的，向下，应该向左」）。

用 preview_player_anim 的手持物变换（与预览图同一套数学）把「斧刃侧 - 握柄」这条模型
方向量到世界空间。坐标：+X = 角色左、+Y = 上、+Z = 身后。
"""

from __future__ import annotations

import json
import sys
import unittest
from pathlib import Path

import numpy as np

LIB_DIR = Path(__file__).resolve().parents[1]
REPO = LIB_DIR.parent
sys.path.insert(0, str(LIB_DIR / "tools"))

import preview_player_anim as preview  # noqa: E402

ASSETS = REPO / "client" / "src" / "main" / "resources" / "assets" / "bong"
# 两把斧的斧刃都建在建模稿 -X 侧，握柄在方块中心 x = 8
BLADE_POINT = np.array([1.5, 18.0, 8.0, 1.0])
HAFT_POINT = np.array([8.0, 18.0, 8.0, 1.0])
MIN_ALIGNMENT = 0.5  # 刃口方向在目标方向上的分量下限（单位向量点积）


def edge_direction(item: str, tick: float) -> np.ndarray:
    emote = json.loads((ASSETS / "player_animation" / f"{item}_use.json").read_text(encoding="utf-8"))["emote"]
    model = json.loads((ASSETS / "models" / "item" / item / f"{item}.json").read_text(encoding="utf-8"))
    hand = preview.hand_transform(preview.collect_keyframes(emote), tick,
                                  model["display"]["thirdperson_righthand"])
    direction = (hand @ BLADE_POINT - hand @ HAFT_POINT)[:3]
    return direction / np.linalg.norm(direction)


class AxeEdgeDirectionTest(unittest.TestCase):
    def test_iron_axe_edge_points_down_at_impact(self) -> None:
        impact_tick = 8
        edge = edge_direction("axe_iron_v2", impact_tick)
        self.assertLess(edge[1], -MIN_ALIGNMENT,
                        f"生铁斧落斧时刃口应朝下，实际方向 (左, 上, 后) = {np.round(edge, 2)}")

    def test_bone_axe_edge_leads_the_sweep_to_the_left(self) -> None:
        for label, tick in (("起手", 0), ("发力", 2), ("收势", 5)):
            with self.subTest(frame=label):
                edge = edge_direction("axe_bone_v2", tick)
                self.assertGreater(edge[0], MIN_ALIGNMENT,
                                   f"骨斧横砍{label}帧刃口应朝左，实际方向 (左, 上, 后) = {np.round(edge, 2)}")


if __name__ == "__main__":
    unittest.main()
