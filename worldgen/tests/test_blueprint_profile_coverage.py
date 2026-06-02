"""回归测试：bake 蓝图里每个 zone 的 terrain_profile 必须在 profile catalog 中。

snapshot CI 曾因 plan-terrain-wiring-v1 P3 把 `wangyintai` zone 加进 bake 蓝图
（server/zones.worldview.example.json）但漏了 terrain-profiles.example.json 的 profile
spec 条目 → `build_generation_plan`（stitcher.py:60）抛
`KeyError: Blueprint zone 'wangyintai' references unknown profile 'wangyintai'` →
整条 worldgen pipeline 崩。单测当时没跑「真蓝图 + 真 catalog 的 build_generation_plan」
全链路，故漏网。本测试锁住该不变量：任何新加 zone 必须同步补 profile spec。
"""

import tempfile
import unittest
from pathlib import Path

from scripts.terrain_gen.blueprint import (
    DEFAULT_BLUEPRINT_PATH,
    DEFAULT_PROFILES_PATH,
    load_blueprint,
    load_profile_catalog,
)
from scripts.terrain_gen.stitcher import build_generation_plan


class BlueprintProfileCoverageTest(unittest.TestCase):
    def setUp(self) -> None:
        self.blueprint = load_blueprint(DEFAULT_BLUEPRINT_PATH)
        self.catalog = load_profile_catalog(DEFAULT_PROFILES_PATH)

    def test_every_blueprint_zone_profile_present_in_catalog(self) -> None:
        """每个 bake 蓝图 zone 的 terrain_profile 都在 catalog——否则 pipeline KeyError 崩。"""
        missing = [
            (zone.name, zone.worldgen.terrain_profile)
            for zone in self.blueprint.zones
            if zone.worldgen.terrain_profile not in self.catalog.profiles
        ]
        self.assertEqual(
            missing,
            [],
            "蓝图 zone 引用了 catalog 缺失的 profile（会让 worldgen pipeline 在 "
            f"build_generation_plan 抛 KeyError）：{missing}。"
            "需在 worldgen/terrain-profiles.example.json 的 profiles 下补对应 spec 条目。",
        )

    def test_compound_profiles_present(self) -> None:
        """专项 pin：本 plan 接线的两个 compound profile 必须在 catalog。"""
        for profile_name in ("dan_zong_yi_yuan", "wangyintai"):
            self.assertIn(
                profile_name,
                self.catalog.profiles,
                f"compound profile '{profile_name}' 缺 terrain-profiles.example.json spec 条目",
            )

    def test_build_generation_plan_succeeds_for_full_blueprint(self) -> None:
        """全蓝图跑 build_generation_plan 不得抛异常——与 snapshot CI 同款集成路径，

        是真正能拦住「zone 引用未知 profile」的端到端检查（不是把单元拼起来）。
        """
        with tempfile.TemporaryDirectory() as temp_dir:
            plan = build_generation_plan(
                blueprint=self.blueprint,
                profile_catalog=self.catalog,
                blueprint_path=DEFAULT_BLUEPRINT_PATH,
                profiles_path=DEFAULT_PROFILES_PATH,
                output_dir=Path(temp_dir),
                tile_size=16,
            )
        self.assertEqual(
            len(plan.zone_plans),
            len(self.blueprint.zones),
            "build_generation_plan 应为每个蓝图 zone 产出一个 zone_plan",
        )
        planned_profiles = {
            zone.worldgen.terrain_profile for zone in self.blueprint.zones
        }
        self.assertIn(
            "wangyintai",
            planned_profiles,
            "前置：wangyintai zone 应已在 bake 蓝图中（本 plan P3 #6 交付物）",
        )


if __name__ == "__main__":
    unittest.main()
