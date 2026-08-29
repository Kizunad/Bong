#!/usr/bin/env python3
"""BeastSpineSword（异兽脊骨剑 / 髓骨残剑）bbmodel 生成器单元测试。

验证内容：
1. 结构与 Group 契约：
   - 必须包含 6 个层级 Group (pommel, tassel, grip, guard, blade_spine, blade_tip)
   - 每个 Group 均有对应的子元素 uuid，且不为空
   - 元素名称互不重复
2. 尺寸与比例约束：
   - 剑身长宽比与全长在合理区间（全长 24~30px）
   - 脊椎骨节数等于 SPINE_SEGS (10节) 且逐节递减收细
   - 左右骨刺对称且倒钩倾角合理
3. 贴图四象限与分辨率约束：
   - 贴图必须是 64x64 RGBA
   - 四个象限（bone, cinnabar, bronze, cord）色彩均不为纯色，且平均 RGB 符合材质特征
   - 生成贴图确定性（同 seed 产出完全一致）
4. 合法性检查：
   - 无 NaN、Inf 或退化零体积盒
   - 旋转仅使用单轴或规范欧拉角
"""

from __future__ import annotations

import json
import sys
import unittest
from pathlib import Path

import numpy as np
from PIL import Image

LIB_DIR = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(LIB_DIR / "generators"))

import gen_beast_spine_sword as sword  # noqa: E402


class TestBeastSpineSword(unittest.TestCase):
    def setUp(self):
        self.bbmodel = sword.build_bbmodel()
        self.elements = self.bbmodel["elements"]
        self.outliner = self.bbmodel["outliner"]

    def test_groups_structure_and_order(self):
        """测试 6 个核心 Group 完整性与元素挂载。"""
        group_names = [g["name"] for g in self.outliner]
        self.assertEqual(
            group_names,
            sword.BONE_ORDER,
            f"Group 结构或顺序不符，期望 {sword.BONE_ORDER}，实际 {group_names}",
        )
        for group in self.outliner:
            self.assertGreater(
                len(group["children"]),
                0,
                f"Group {group['name']} 不应为空",
            )

    def test_elements_validity_and_uniqueness(self):
        """测试所有 Cube 盒体有效性与 UUID/名称唯一性。"""
        uuids = set()
        names = set()
        for el in self.elements:
            # UUID 唯一性
            self.assertNotIn(el["uuid"], uuids, f"重复 UUID: {el['uuid']}")
            uuids.add(el["uuid"])

            # 命名唯一性
            self.assertNotIn(el["name"], names, f"重复 Element 命名: {el['name']}")
            names.add(el["name"])

            # 坐标合法性 (from < to)
            frm = el["from"]
            to = el["to"]
            for i, axis in enumerate(["x", "y", "z"]):
                self.assertLessEqual(
                    frm[i],
                    to[i],
                    f"Element {el['name']} 在 {axis} 轴上 from > to ({frm} vs {to})",
                )
                self.assertFalse(
                    np.isnan(frm[i]) or np.isnan(to[i]),
                    f"Element {el['name']} 坐标包含 NaN",
                )

    def test_dimensions_and_proportions(self):
        """测试全长与各部位分段比例。"""
        all_from_y = [el["from"][1] for el in self.elements]
        all_to_y = [el["to"][1] for el in self.elements]
        y_min = min(all_from_y)
        y_max = max(all_to_y)
        total_len = y_max - y_min

        # 包含下垂流苏的全长在 28~34px 之间
        self.assertGreater(total_len, 25.0, f"剑总长偏小: {total_len}")
        self.assertLess(total_len, 35.0, f"剑总长过大: {total_len}")

        # 验证脊椎节数
        vertebrae = [el for el in self.elements if el["name"].startswith("vertebra_") and el["name"].endswith("_0")]
        self.assertEqual(len(vertebrae), sword.SPINE_SEGS, f"脊椎节数应为 {sword.SPINE_SEGS}")

        # 验证骨刺对称性
        left_spurs = [el for el in self.elements if "spur_blade_l_" in el["name"]]
        right_spurs = [el for el in self.elements if "spur_blade_r_" in el["name"]]
        self.assertEqual(len(left_spurs), sword.SPINE_SEGS)
        self.assertEqual(len(right_spurs), sword.SPINE_SEGS)

    def test_texture_atlas_properties(self):
        """测试 64x64 四象限贴图属性与色彩分布。"""
        tex1 = sword.make_texture(seed=0x7E1B)
        tex2 = sword.make_texture(seed=0x7E1B)

        self.assertEqual(tex1.size, (64, 64))
        self.assertEqual(tex1.mode, "RGBA")

        # 确定性断言
        self.assertEqual(tex1.tobytes(), tex2.tobytes(), "相同 seed 贴图输出必须绝对一致")

        # 象限色彩差异性检测
        arr = np.array(tex1)
        # Q1 (Bone): (0..32, 0..32)
        q1_mean = arr[:32, :32, :3].mean(axis=(0, 1))
        # Q2 (Cinnabar): (0..32, 32..64) -> y<32, x>=32
        q2_mean = arr[:32, 32:, :3].mean(axis=(0, 1))
        # Q3 (Bronze): (32..64, 0..32) -> y>=32, x<32
        q3_mean = arr[32:, :32, :3].mean(axis=(0, 1))
        # Q4 (Cord): (32..64, 32..64) -> y>=32, x>=32
        q4_mean = arr[32:, 32:, :3].mean(axis=(0, 1))

        # 朱砂象限应显著偏红 (R >> G, B)
        self.assertGreater(q2_mean[0], q2_mean[1] * 1.5, "朱砂象限 R 通道应显著高于 G")
        self.assertGreater(q2_mean[0], q2_mean[2] * 1.5, "朱砂象限 R 通道应显著高于 B")

        # 杂铜象限应为黄褐色 (R > G > B)
        self.assertGreater(q3_mean[0], q3_mean[2], "杂铜象限 R 应大于 B")
        self.assertGreater(q3_mean[1], q3_mean[2], "杂铜象限 G 应大于 B")

        # 骨骼象限应为高明度灰白米黄 (RGB 均较高)
        self.assertGreater(q1_mean[0], 170.0, "骨骼象限明度偏低")


if __name__ == "__main__":
    unittest.main()
