"""测试立方体共面通用门禁 (bbmodel_maker.gates.coplanar)。

针对 #2386 Kody 提出的共面检测漏判仅共享单表面缺陷编写，
覆盖差分注入、格式兼容与历史资产零误报验证。
"""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MODEL_SCRIPT_DIR = ROOT / "modelScript"
if str(MODEL_SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(MODEL_SCRIPT_DIR))

import bbmodel_maker.gates
_local_gates = str(MODEL_SCRIPT_DIR / "bbmodel_maker" / "gates")
if _local_gates not in bbmodel_maker.gates.__path__:
    bbmodel_maker.gates.__path__.append(_local_gates)

from bbmodel_maker.gates.coplanar import (
    CoplanarConflictError,
    assert_no_coplanar_faces,
    check_coplanar_faces,
    inject_collinear_non_overlapping,
    inject_single_face_coplanar,
    self_test_coplanar_gate,
)


class CoplanarGateTest(unittest.TestCase):
    def test_self_test_passes(self) -> None:
        """门禁自带差分自测试验必须全绿通过。"""
        self_test_coplanar_gate()

    def test_single_face_coplanar_on_all_axes(self) -> None:
        """单一表面共面在 X、Y、Z 各轴的正负面上均能准确检出并指明方向。"""
        # 测试各轴与正负方向
        cases = [
            (0, False, "-X"),
            (0, True, "+X"),
            (1, False, "-Y"),
            (1, True, "+Y"),
            (2, False, "-Z"),
            (2, True, "+Z"),
        ]
        for axis, is_max, expected_label in cases:
            with self.subTest(axis=axis, is_max=is_max):
                c1, c2 = inject_single_face_coplanar(axis=axis, is_max=is_max)
                violations = check_coplanar_faces([c1, c2])
                self.assertEqual(len(violations), 1)
                self.assertIn(f"在 {expected_label} 面共面", violations[0])
                with self.assertRaises(CoplanarConflictError):
                    assert_no_coplanar_faces([c1, c2])

    def test_collinear_and_edge_touching_no_false_positive(self) -> None:
        """仅共线、触碰边缘或角点接触的立方体对不得产生误报。"""
        # 1. 在 Z 轴上坐标相同但在 X 轴上仅接缝接触 (overlap_x == 0)
        c1, c2 = inject_collinear_non_overlapping(shared_axis=2, collinear_axis=0)
        self.assertEqual([], check_coplanar_faces([c1, c2]))

        # 2. 在 Y 轴上坐标相同但在 Z 轴上仅接缝接触 (overlap_z == 0)
        c3, c4 = inject_collinear_non_overlapping(shared_axis=1, collinear_axis=2)
        self.assertEqual([], check_coplanar_faces([c3, c4]))

        # 3. 角点接触（仅在一个顶点相交，另两轴重叠量均为 0）
        ca = {"name": "corner_a", "from": [0.0, 0.0, 0.0], "to": [1.0, 1.0, 1.0]}
        cb = {"name": "corner_b", "from": [1.0, 1.0, 0.0], "to": [2.0, 2.0, 1.0]}
        self.assertEqual([], check_coplanar_faces([ca, cb]))

    def test_cube_formats_compatibility(self) -> None:
        """支持元组格式、字典格式及具有 origin/size 属性的对象格式。"""
        # 元组格式 1 (带 bone/mat)
        t1 = ("bone1", "mat1", "t1", [0.0, 0.0, 0.0], [2.0, 2.0, 2.0])
        t2 = ("bone2", "mat2", "t2", [0.5, 0.5, 0.0], [1.5, 1.5, 1.0])
        self.assertEqual(1, len(check_coplanar_faces([t1, t2])))

        # 字典格式
        d1 = {"name": "d1", "from": [0.0, 0.0, 0.0], "to": [2.0, 2.0, 2.0]}
        d2 = {"name": "d2", "from": [0.5, 0.5, 0.0], "to": [1.5, 1.5, 1.0]}
        self.assertEqual(1, len(check_coplanar_faces([d1, d2])))

        # 类对象格式
        class MockCube:
            def __init__(self, name, origin, size):
                self.name = name
                self.origin = origin
                self.size = size

        o1 = MockCube("o1", (0.0, 0.0, 0.0), (2.0, 2.0, 2.0))
        o2 = MockCube("o2", (0.5, 0.5, 0.0), (1.0, 1.0, 1.0))
        self.assertEqual(1, len(check_coplanar_faces([o1, o2])))

    def test_three_pr2386_models_clean(self) -> None:
        """#2386 已合入的三件模型在新共面判据下必须为 0 冲突。"""
        sys.path.insert(0, str(MODEL_SCRIPT_DIR / "generators"))
        import gen_bing_jia_shou_tao
        import gen_hand_wrap
        import gen_spirit_sword

        self.assertEqual([], check_coplanar_faces(gen_bing_jia_shou_tao.all_cubes()))
        self.assertEqual([], check_coplanar_faces(gen_hand_wrap.all_cubes()))
        self.assertEqual([], check_coplanar_faces(gen_spirit_sword.all_cubes()))


if __name__ == "__main__":
    unittest.main()
