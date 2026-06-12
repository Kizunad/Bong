"""worldgen-v4 P2 — 迁移到 DSL 的 profile 的契约 pin.

每个被重写为声明式 DSL 组合的 profile 必须满足两条契约：

1. **景观不变（换表示不换景观）** —— 迁移后的 ``fill_*_tile`` 输出与迁移前
   逐层 byte-identical。这一条由 ``test_v3_full_profile_baseline.py`` 的全 profile
   等价对拍兜底（采样列 + span 形态 + water + biome）。本文件额外锁**整层**等价：
   迁移后的 generator 仍只用 DSL 算子库（noise via dsl.fbm_height/ridge_height/
   warped_height、径向 via dsl.radial_uplift、合成 via dsl.compose_height），而非
   绕过 DSL 重新内联 numpy——通过断言「profile 模块 import 了 dsl 且不再裸 import
   noise 的 fbm_2d/ridge_2d/warped_fbm_2d」来 pin「已迁移」状态。

2. **qi_grade 声明（§8.1 #4）** —— 每个迁移 profile 模块导出一个 ``QI_GRADE``
   常量，落在六档枚举内（现有 zone spirit_qi 就近归档）。P2 只锁 schema 声明，
   实际灵气场生成在 P4。

迁移一个 profile 就把它的 key 追加进 ``MIGRATED_PROFILES``，pin 立即生效；未迁移
的 profile 不在此列，不受约束（仍走手写 numpy + spans_shim）。
"""

from __future__ import annotations

import ast
import importlib
import unittest
from pathlib import Path

import numpy as np

from scripts.terrain_gen import dsl

PROFILES_DIR = (
    Path(__file__).resolve().parents[1] / "scripts/terrain_gen/profiles"
)

# (profile_key, module_name) for每个已迁移到 DSL 的 profile。迁移完一个就追加。
MIGRATED_PROFILES: tuple[tuple[str, str], ...] = (
    ("broken_peaks", "scripts.terrain_gen.profiles.broken_peaks"),
    ("ash_dead_zone", "scripts.terrain_gen.profiles.ash_dead_zone"),
    ("ancient_battlefield", "scripts.terrain_gen.profiles.ancient_battlefield"),
)

# 迁移后不应再裸 import 的低层 noise 函数（应改走 dsl 算子库封装）。
_BARE_NOISE_FNS = frozenset({"fbm_2d", "ridge_2d", "warped_fbm_2d"})


class ProfileDslMigrationTest(unittest.TestCase):
    def test_migrated_profile_declares_valid_qi_grade(self) -> None:
        # §8.1 #4：每个迁移 profile 导出 QI_GRADE 落在六档枚举内。
        for profile_key, module_name in MIGRATED_PROFILES:
            module = importlib.import_module(module_name)
            self.assertTrue(
                hasattr(module, "QI_GRADE"),
                f"{profile_key} migrated to DSL but does not export a QI_GRADE "
                "constant; §8.1 #4 requires every migrated profile declare its "
                "qi_grade band (spirit_qi 就近归档)",
            )
            grade = module.QI_GRADE
            self.assertIsInstance(
                grade,
                dsl.QiGrade,
                f"{profile_key}.QI_GRADE must be a dsl.QiGrade enum member, got "
                f"{type(grade)!r}",
            )
            # 枚举值必须落在 QI_GRADE_BOUNDS 已声明的六档之一。
            self.assertIn(
                grade,
                dsl.QI_GRADE_BOUNDS,
                f"{profile_key}.QI_GRADE={grade!r} is not one of the six declared "
                f"qi_grade bands {tuple(dsl.QI_GRADE_BOUNDS)}",
            )

    def test_migrated_profile_uses_dsl_not_bare_noise(self) -> None:
        # 迁移 = 把噪声/径向/合成换成 dsl 算子库；模块应 import dsl 且不再裸 import
        # fbm_2d/ridge_2d/warped_fbm_2d（防「假迁移」：换了 import 又内联回来）。
        for profile_key, module_name in MIGRATED_PROFILES:
            source_path = PROFILES_DIR / f"{profile_key}.py"
            self.assertTrue(
                source_path.exists(),
                f"migrated profile source {source_path} not found",
            )
            tree = ast.parse(source_path.read_text(encoding="utf-8"))
            imports_dsl = False
            bare_noise: set[str] = set()
            for node in ast.walk(tree):
                if isinstance(node, ast.ImportFrom):
                    imported = {alias.name for alias in node.names}
                    if "dsl" in imported or (
                        node.module and node.module.endswith("dsl")
                    ):
                        imports_dsl = True
                    bare_noise |= imported & _BARE_NOISE_FNS
            self.assertTrue(
                imports_dsl,
                f"{profile_key} is listed as migrated but does not import the dsl "
                "operator library; a DSL migration must build its height field "
                "through dsl.* ops",
            )
            self.assertEqual(
                bare_noise,
                set(),
                f"{profile_key} still bare-imports low-level noise {sorted(bare_noise)} "
                "after migration; route noise through dsl.fbm_height / "
                "dsl.ridge_height / dsl.warped_height instead",
            )


class AshDeadZoneInvariantTest(unittest.TestCase):
    """死域专属守恒不变量：DSL 迁移后必须仍恒成立（最易回归的点）。"""

    def test_qi_vein_flow_is_exactly_zero_everywhere(self) -> None:
        # §死域：灵脉断绝 → qi_vein_flow 恒 0.0。DSL 迁移若误把灵脉流引回（如复用
        # 通用 qi 算子带回非零），此 pin 立即撞红。与 raster_check 死域规则对齐。
        from v3_full_profile_baseline import build_full_profile_buffer

        buffer = build_full_profile_buffer("ash_dead_zone")
        vein = np.asarray(buffer.layers["qi_vein_flow"])
        max_abs = float(np.abs(vein).max()) if vein.size else 0.0
        self.assertEqual(
            max_abs,
            0.0,
            "ash_dead_zone qi_vein_flow must be exactly 0.0 everywhere (severed "
            f"meridians); DSL migration leaked a non-zero flow (max|flow|={max_abs})",
        )

    def test_core_qi_density_is_exactly_zero(self) -> None:
        # 死域核心 qi_density 恒 0（死灵核心）。迁移后核心至少有一列必须为 0。
        from v3_full_profile_baseline import build_full_profile_buffer

        buffer = build_full_profile_buffer("ash_dead_zone")
        qi = np.asarray(buffer.layers["qi_density"])
        self.assertEqual(
            float(qi.min()),
            0.0,
            "ash_dead_zone must have a dead core column with qi_density==0.0; "
            f"min qi_density={float(qi.min())} after migration",
        )


if __name__ == "__main__":
    unittest.main()
