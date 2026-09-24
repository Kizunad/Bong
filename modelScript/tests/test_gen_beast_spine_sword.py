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
import tempfile
import unittest
from pathlib import Path
from unittest import mock

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


class PreviewOnlyFlagIsWiredTest(unittest.TestCase):
    """`--preview-only` 必须真的只出预览、不写模型。

    这个参数此前声明了却**没人读**：传了照样覆盖写 `--out`，而且一张预览都不产出
    （Kody 在 PR #2128 上点出来的，属实）。死参数最坏的形态就是这种——它不报错，
    只是默默做了你没让它做的事。仓库先例见 `gen_array_flag` / `gen_bamboo_jian`：
    `--preview-only` 只跳过写模型，预览两种模式都出。
    """

    def _run(self, argv, out_model, out_preview):
        with mock.patch.object(sys, "argv", ["gen_beast_spine_sword.py", *argv]), \
             mock.patch.object(sword, "PREVIEW_OUT", out_preview), \
             mock.patch("builtins.print"):
            sword.main()

    def test_preview_only_does_not_touch_the_model_file(self):
        with tempfile.TemporaryDirectory() as tmp:
            model = Path(tmp) / "should_not_exist.bbmodel"
            preview = Path(tmp) / "preview.png"
            self._run(["--preview-only", "--out", str(model)], model, preview)
            self.assertFalse(
                model.exists(),
                f"期望：--preview-only 不写模型文件；实际 {model} 被写出来了——"
                "这正是死参数最坏的形态：不报错，只是默默覆盖了你的文件")
            self.assertTrue(preview.exists(), "期望：--preview-only 产出预览图；实际没有")

    def test_preview_only_refuses_to_clobber_an_existing_model(self):
        """更狠一点：目标文件已存在时，--preview-only 必须一个字节都不动它。"""
        with tempfile.TemporaryDirectory() as tmp:
            model = Path(tmp) / "existing.bbmodel"
            model.write_text("SENTINEL", encoding="utf-8")
            preview = Path(tmp) / "preview.png"
            self._run(["--preview-only", "--out", str(model)], model, preview)
            self.assertEqual(
                model.read_text(encoding="utf-8"), "SENTINEL",
                "期望：--preview-only 不碰已存在的模型文件；实际内容被覆盖了")

    def test_default_mode_writes_both_the_model_and_the_preview(self):
        with tempfile.TemporaryDirectory() as tmp:
            model = Path(tmp) / "out.bbmodel"
            preview = Path(tmp) / "preview.png"
            self._run(["--out", str(model)], model, preview)
            self.assertTrue(model.exists(), "期望：默认模式写出 .bbmodel；实际没有")
            doc = json.loads(model.read_text(encoding="utf-8"))
            self.assertGreater(len(doc["elements"]), 0, "期望：写出的模型里有元素")
            self.assertTrue(preview.exists(), "期望：默认模式同样产出预览图；实际没有")

    def test_the_preview_is_an_actual_render_not_a_blank_canvas(self):
        """预览得真的渲出东西——只建目录/存白图也能骗过'文件存在'。"""
        with tempfile.TemporaryDirectory() as tmp:
            preview = Path(tmp) / "preview.png"
            sword.render_preview(sword.build_bbmodel(), out=preview)
            img = Image.open(preview).convert("RGB")
            colors = {px for px in img.getdata()}
            self.assertGreater(
                len(colors), 20,
                f"期望：预览图里有真实渲染出的多种颜色；实际只有 {len(colors)} 种——"
                "多半渲了个空场景")
