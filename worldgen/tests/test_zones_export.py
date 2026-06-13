"""worldgen-v4 P4 §8.1 #3/#4/#11 — zone spirit_qi 派生 + 预算配平 + zones.json 治理饱和单测.

锁五件事：

1. **预算配平断言**（§8.1 #3）—— Σ zone 份额 + wilderness 余量 == 预算口径
   （从 env var / 默认锚取，**绝不写字面 100**）；override 计入总量、wilderness +
   非 override 区吸收差额；病态退化（全死域 / wilderness 面积负）报错。
2. **spirit_qi 派生正反 pin**（§8.1 #4）—— 单 zone 面积加权均值落回声明档位；
   富集 / 死域 / 负灵三态 zone 各派生到对应区间；同源闭环（spirit_qi 与
   qi_density 均值互推）。
3. **zones.json merge 治理**（§8.1 #11）—— 只覆写 spirit_qi、运行时字段
   (active_events / patrol_anchors / blocked_tiles / 扩展字段) 逐字段保留不 clobber、
   generated_by 标记、未知 zone 报错、缺失 zone 保留旧值。
4. **qi_grade 六档归档** —— 历史 spirit_qi 标量就近落档。
5. **预算口径单源 + CI 漂移防护** —— resolve_spirit_qi_total 读 env var / 默认锚；
   parse + assert_const_matches_rust 防 Python↔Rust 常量分裂。
"""

from __future__ import annotations

import json
import subprocess
import sys
import textwrap
import unittest
from pathlib import Path

from scripts.terrain_gen.dsl import QiGrade
from scripts.terrain_gen.qi_field import QI_FIELD_BASE, qi_density_from_field
from scripts.terrain_gen import zones_export as ze
from scripts.terrain_gen.zones_export import (
    QI_BUDGET_EPSILON,
    SPIRIT_QI_TOTAL_ENV,
    ZoneQiInput,
    archive_grade_from_spirit_qi,
    balance_qi_budget,
    build_zones_file,
    derive_zone_spirit_qi,
    merge_zone_spirit_qi,
    resolve_spirit_qi_total,
)


# worldgen/ 目录（tests/ 的父）—— spawn 的 -O 子进程以此为 cwd 才能 import scripts.*.
WORLDGEN_DIR = Path(__file__).resolve().parent.parent


def _zone(name: str, *, cx: float, cz: float, half: float, grade: QiGrade,
          override: float | None = None) -> ZoneQiInput:
    return ZoneQiInput(
        name=name,
        bounds_xz=(cx - half, cx + half, cz - half, cz + half),
        grade=grade,
        override=override,
    )


# ===========================================================================
# 1. spirit_qi 派生正反 pin（面积加权均值落回声明档位）
# ===========================================================================


class SpiritQiDerivationPinTest(unittest.TestCase):
    def test_three_state_zones_derive_to_their_grade(self) -> None:
        # 富集 / 死域 / 负灵三态 zone 的派生 spirit_qi 各落回声明档位区间。
        cases = [
            (QiGrade.FONT, 0.7, 1.0),
            (QiGrade.RICH, 0.45, 0.7),
            (QiGrade.COMMON, 0.15, 0.45),
            (QiGrade.TRACE, 0.02, 0.15),
            (QiGrade.DEAD, -0.1, 0.02),
            (QiGrade.NEGATIVE, -1.0, -0.1),
        ]
        for grade, lo, hi in cases:
            # 单 zone（无邻核干扰），关 veins 隔离档位读数。
            z = _zone("solo", cx=0, cz=0, half=300, grade=grade)
            derived = derive_zone_spirit_qi([z], enable_veins=False)
            sq = derived["solo"]
            self.assertTrue(
                lo <= sq < hi or (hi == 1.0 and sq <= 1.0),
                f"{grade.value} zone derives spirit_qi={sq:+.4f} which must land "
                f"in declared grade range [{lo}, {hi}); got {sq}",
            )
            # 反向 pin：派生值归档回同一档位（往返自洽）。
            self.assertEqual(
                archive_grade_from_spirit_qi(sq), grade,
                f"{grade.value} zone derived sq={sq:+.4f} must archive back to "
                f"{grade.value}",
            )

    def test_same_source_closed_loop(self) -> None:
        # 同源闭环：zone spirit_qi 与其 qi_density 期望均值通过派生公式互推。
        z = _zone("loop", cx=0, cz=0, half=250, grade=QiGrade.RICH)
        derived = derive_zone_spirit_qi([z], enable_veins=False)
        sq = derived["loop"]
        expected_density = float(qi_density_from_field(sq))
        # density = clamp01((sq+1)/2) 必须与 sq 同向（同源派生不漂移）。
        self.assertAlmostEqual(expected_density, (sq + 1.0) / 2.0, places=6)

    def test_override_pins_zone_to_precise_value(self) -> None:
        # qi_override 硬约束：精调浓度铺死，派生 spirit_qi 贴近 override 值。
        z = _zone("ov", cx=0, cz=0, half=300, grade=QiGrade.COMMON, override=0.62)
        derived = derive_zone_spirit_qi([z], enable_veins=False)
        sq = derived["ov"]
        # override 铺死核心，AABB 均值偏向 override（不会被档位 target 0.3 拉走）。
        self.assertGreater(
            sq, 0.45,
            f"override=0.62 must pull derived spirit_qi above 0.45 (toward "
            f"override, not the common grade target 0.3); got {sq}",
        )

    def test_duplicate_zone_name_raises(self) -> None:
        z1 = _zone("dup", cx=0, cz=0, half=100, grade=QiGrade.COMMON)
        z2 = _zone("dup", cx=500, cz=0, half=100, grade=QiGrade.FONT)
        with self.assertRaisesRegex(ValueError, "duplicate zone name"):
            derive_zone_spirit_qi([z1, z2])

    def test_neighbor_kernel_does_not_overreach_into_disjoint_zone(self) -> None:
        # footprint over-reach 修复 pin：一个**不与 weak zone AABB 相交**的强 font
        # 邻 zone，其核 falloff 只伸进 wilderness，**不得**抬高 weak zone 自身 bounds
        # 的派生 spirit_qi（否则坍缩渊口被邻常灵区抬成常灵——违 worldview）。
        weak = _zone("weak", cx=0, cz=0, half=150, grade=QiGrade.TRACE)
        # strong AABB [400,1000] 与 weak AABB [-150,150] 不相交（间隔 250）。
        strong = _zone("strong", cx=700, cz=0, half=300, grade=QiGrade.FONT)
        solo = derive_zone_spirit_qi([weak], enable_veins=False)["weak"]
        withnb = derive_zone_spirit_qi([weak, strong], enable_veins=False)["weak"]
        self.assertEqual(
            archive_grade_from_spirit_qi(withnb), QiGrade.TRACE,
            f"disjoint font 邻 zone 不得把 weak zone 抬出 trace 档；得 {withnb:+.4f}",
        )
        self.assertAlmostEqual(
            withnb, solo, delta=1e-9,
            msg=f"不相交邻 zone 的核 falloff 不得伸进 weak bounds；solo={solo:+.4f} "
            f"withnb={withnb:+.4f}",
        )

    def test_nested_smaller_zone_overrides_larger_enclosing_zone(self) -> None:
        # 嵌套 containment：小 trace zone 完全落在大 font zone 内（rift_mouth 落在
        # blood_valley 内的几何）。**最具体（面积最小）的 zone 在自身 bounds 内主导**，
        # 小 trace zone 读 trace（不被外层大 zone 抬成 font），外层 font zone 仍读 font。
        big = _zone("big", cx=0, cz=0, half=600, grade=QiGrade.FONT)
        small = _zone("small", cx=0, cz=0, half=80, grade=QiGrade.TRACE)
        derived = derive_zone_spirit_qi([big, small], enable_veins=False)
        self.assertEqual(
            archive_grade_from_spirit_qi(derived["small"]), QiGrade.TRACE,
            f"嵌套小 trace zone 必须在自身 bounds 内主导读 trace；得 "
            f"{derived['small']:+.4f}（外层 font 不得越界压过）",
        )
        self.assertEqual(
            archive_grade_from_spirit_qi(derived["big"]), QiGrade.FONT,
            f"外层大 font zone 自身均值仍读 font（小洞只占极小面积）；得 "
            f"{derived['big']:+.4f}",
        )


# ===========================================================================
# 2. 预算配平断言（Σ 份额 + wilderness == 预算）
# ===========================================================================


class BudgetBalanceTest(unittest.TestCase):
    def setUp(self) -> None:
        self.zones = [
            _zone("a", cx=0, cz=0, half=100, grade=QiGrade.FONT),
            _zone("b", cx=1000, cz=0, half=100, grade=QiGrade.COMMON),
            _zone("c", cx=2000, cz=0, half=100, grade=QiGrade.NEGATIVE),
        ]
        self.derived = derive_zone_spirit_qi(self.zones, enable_veins=False)
        self.world_area = 10_000_000.0  # >> Σ zone area

    def test_shares_plus_wilderness_equals_budget(self) -> None:
        budget = 100.0
        report = balance_qi_budget(
            self.zones, self.derived, world_area=self.world_area, budget=budget
        )
        self.assertAlmostEqual(
            report.zone_sum + report.wilderness_share, budget, places=6,
            msg="Σ zone_shares + wilderness_share must equal budget (conservation)",
        )
        # zone_sum 字段与 Σ zone_shares 一致。
        self.assertAlmostEqual(
            report.zone_sum, sum(report.zone_shares.values()), places=9
        )

    def test_budget_defaults_to_resolved_total_not_literal(self) -> None:
        # budget=None 时取 resolve_spirit_qi_total()（env / 默认锚），不写字面 100。
        report = balance_qi_budget(
            self.zones, self.derived, world_area=self.world_area, budget=None
        )
        self.assertAlmostEqual(report.total, resolve_spirit_qi_total(), places=9)
        self.assertAlmostEqual(
            report.zone_sum + report.wilderness_share, report.total, places=6
        )

    def test_override_zone_counts_in_total_not_exempt(self) -> None:
        # override zone 的 mass 进分母（精调浓度不豁免守恒）：加一个高 override zone
        # 后总和仍 == budget，且 override zone 拿到非零份额。
        zones = list(self.zones) + [
            _zone("ov", cx=3000, cz=0, half=100, grade=QiGrade.TRACE, override=0.9)
        ]
        derived = derive_zone_spirit_qi(zones, enable_veins=False)
        report = balance_qi_budget(
            zones, derived, world_area=self.world_area, budget=100.0
        )
        self.assertAlmostEqual(
            report.zone_sum + report.wilderness_share, 100.0, places=6,
            msg="override zone 计入总量后配平仍守恒（不豁免）",
        )
        self.assertGreater(
            report.zone_shares["ov"], 0.0,
            "override zone 必须拿到非零预算份额（mass 进 M 分母）",
        )

    def test_wilderness_absorbs_difference(self) -> None:
        # wilderness 面积越大，wilderness 份额越大（吸收差额）；zone 份额相应缩小。
        small_world = balance_qi_budget(
            self.zones, self.derived, world_area=1_000_000.0, budget=100.0
        )
        big_world = balance_qi_budget(
            self.zones, self.derived, world_area=100_000_000.0, budget=100.0
        )
        self.assertGreater(
            big_world.wilderness_share, small_world.wilderness_share,
            "更大 wilderness 面积必须吸收更多预算份额",
        )
        self.assertLess(
            big_world.zone_sum, small_world.zone_sum,
            "wilderness 吸收更多时 zone 总份额相应缩小（总和守恒）",
        )

    def test_zero_zones_all_budget_to_wilderness(self) -> None:
        # 无 zone（纯 wilderness）→ 全部预算归 wilderness，zone_sum=0。
        report = balance_qi_budget(
            [], {}, world_area=self.world_area, budget=100.0
        )
        self.assertEqual(report.zone_sum, 0.0)
        self.assertAlmostEqual(report.wilderness_share, 100.0, places=6)

    def test_negative_wilderness_area_raises(self) -> None:
        # world_area < Σ zone area（配置错误：zone 溢出世界）→ 报错而非静默。
        with self.assertRaisesRegex(ValueError, "wilderness area would be negative"):
            balance_qi_budget(
                self.zones, self.derived, world_area=1.0, budget=100.0
            )

    def test_nonpositive_budget_raises(self) -> None:
        with self.assertRaisesRegex(ValueError, "budget must be > 0"):
            balance_qi_budget(
                self.zones, self.derived, world_area=self.world_area, budget=0.0
            )

    def test_total_mass_zero_raises(self) -> None:
        # 病态退化：全 zone 死到 qi_density=0（spirit_qi=-1）且无 wilderness 质量
        # （world_area == Σ zone area，base 不参与）→ 无法归一，报错。
        dead = [_zone("d", cx=0, cz=0, half=100, grade=QiGrade.NEGATIVE)]
        # bounds 200x200 → area 40000；world_area 设成恰好等于它（无 wilderness）。
        forced = {"d": -1.0}  # 强制 density=0
        with self.assertRaisesRegex(ValueError, "total qi mass is 0"):
            balance_qi_budget(dead, forced, world_area=40000.0, budget=100.0)

    def test_histogram_buckets_all_six_grades(self) -> None:
        report = balance_qi_budget(
            self.zones, self.derived, world_area=self.world_area, budget=100.0
        )
        grades = [b["grade"] for b in report.histogram]
        self.assertEqual(grades, [g.value for g in QiGrade])
        # 三 zone 各落不同档（font/common/negative），计数总和 == zone 数。
        total_zones = sum(b["zone_count"] for b in report.histogram)
        self.assertEqual(total_zones, len(self.zones))

    def test_conservation_guard_survives_python_optimize(self) -> None:
        # CodeRabbit P4 #1 — 守恒护栏是 P4 命脉，**不能用 assert**：``python -O``
        # 会剥离 assert，届时即便配平违反也静默返回自相矛盾的 QiBudgetReport。
        # 这里 spawn 一个 ``python3 -O`` 子进程，把 QI_BUDGET_EPSILON 调成负数让护栏
        # 条件恒真（任何场景都触发），证明护栏在 -O 下仍 raise（=不是被剥离的 assert）。
        code = textwrap.dedent(
            """
            import sys
            assert not __debug__, "subprocess must run under -O (assert stripped)"
            from scripts.terrain_gen import zones_export as ze
            from scripts.terrain_gen.dsl import QiGrade
            from scripts.terrain_gen.zones_export import (
                ZoneQiInput, balance_qi_budget, derive_zone_spirit_qi,
            )
            # 容差设负 → abs(diff) >= -1 恒真 → 护栏必触发（若仍是 assert，-O 下静默放行）。
            ze.QI_BUDGET_EPSILON = -1.0
            zones = [ZoneQiInput(
                name="a", bounds_xz=(-100, 100, -100, 100), grade=QiGrade.FONT,
            )]
            derived = derive_zone_spirit_qi(zones, enable_veins=False)
            try:
                balance_qi_budget(
                    zones, derived, world_area=10_000_000.0, budget=100.0
                )
            except ValueError as exc:
                assert "qi budget balance failed" in str(exc), str(exc)
                print("GUARD_RAISED")
                sys.exit(0)
            print("GUARD_SILENT")
            sys.exit(1)
            """
        )
        proc = subprocess.run(
            [sys.executable, "-O", "-c", code],
            cwd=str(WORLDGEN_DIR),
            capture_output=True,
            text=True,
        )
        self.assertEqual(
            proc.returncode, 0,
            msg=(
                "守恒护栏在 python -O 下必须仍 raise ValueError（不是被剥离的 "
                f"assert）；子进程 stdout={proc.stdout!r} stderr={proc.stderr!r}"
            ),
        )
        self.assertIn(
            "GUARD_RAISED", proc.stdout,
            msg=f"-O 子进程未触发守恒护栏；stdout={proc.stdout!r}",
        )


# ===========================================================================
# 3. 预算口径单源 + CI 漂移防护
# ===========================================================================


class BudgetTotalSourceTest(unittest.TestCase):
    def test_resolve_reads_env_var(self) -> None:
        import os
        prev = os.environ.get(SPIRIT_QI_TOTAL_ENV)
        try:
            os.environ[SPIRIT_QI_TOTAL_ENV] = "250.0"
            self.assertEqual(resolve_spirit_qi_total(), 250.0)
        finally:
            if prev is None:
                os.environ.pop(SPIRIT_QI_TOTAL_ENV, None)
            else:
                os.environ[SPIRIT_QI_TOTAL_ENV] = prev

    def test_resolve_falls_back_on_invalid_env(self) -> None:
        import os
        prev = os.environ.get(SPIRIT_QI_TOTAL_ENV)
        try:
            os.environ[SPIRIT_QI_TOTAL_ENV] = "not-a-number"
            # 无效 → fallback 默认锚（与 Rust DEFAULT_SPIRIT_QI_TOTAL 对齐）。
            self.assertEqual(resolve_spirit_qi_total(), ze._RUST_DEFAULT_SPIRIT_QI_TOTAL)
            os.environ[SPIRIT_QI_TOTAL_ENV] = "-5.0"  # 非正 → fallback
            self.assertEqual(resolve_spirit_qi_total(), ze._RUST_DEFAULT_SPIRIT_QI_TOTAL)
        finally:
            if prev is None:
                os.environ.pop(SPIRIT_QI_TOTAL_ENV, None)
            else:
                os.environ[SPIRIT_QI_TOTAL_ENV] = prev

    def test_no_literal_100_in_module_source(self) -> None:
        # noLiteral100 守门：配平 / 报表逻辑不得硬编字面 100，只允许从 const 锚取。
        # 只有 _RUST_DEFAULT_SPIRIT_QI_TOTAL 这一处镜像 Rust 常量允许出现 100.0。
        import inspect
        src = inspect.getsource(balance_qi_budget) + inspect.getsource(
            resolve_spirit_qi_total
        )
        for token in ("100.0", "100 ", "= 100"):
            self.assertNotIn(
                token, src,
                f"配平 / resolve 逻辑禁止硬编字面 100（{token!r}）；预算口径必须经 "
                "resolve_spirit_qi_total 从 env var / 默认锚解析",
            )

    def test_parse_rust_const(self) -> None:
        src = "pub const DEFAULT_SPIRIT_QI_TOTAL: f64 = 100.0;\n"
        self.assertEqual(ze.parse_rust_default_spirit_qi_total(src), 100.0)

    def test_parse_rust_const_missing_raises(self) -> None:
        with self.assertRaisesRegex(ValueError, "not found"):
            ze.parse_rust_default_spirit_qi_total("// no const here")

    def test_python_anchor_matches_rust_constants(self) -> None:
        # 漂移防护：Python 默认锚必须等于 server/src/qi_physics/constants.rs 实值。
        from scripts.terrain_gen.blueprint import REPO_ROOT
        const_path = REPO_ROOT / "server" / "src" / "qi_physics" / "constants.rs"
        if const_path.exists():
            ze.assert_const_matches_rust(const_path)
        else:  # pragma: no cover — worldgen 可能被单独 checkout
            self.skipTest("constants.rs not present in this checkout")


# ===========================================================================
# 4. qi_grade 六档归档
# ===========================================================================


class GradeArchiveTest(unittest.TestCase):
    def test_archive_each_grade_band(self) -> None:
        # 每档代表值就近归档回本档（六档专属 case）。
        cases = [
            (-0.8, QiGrade.NEGATIVE),
            (-0.15, QiGrade.NEGATIVE),
            (-0.05, QiGrade.DEAD),
            (0.0, QiGrade.DEAD),
            (0.05, QiGrade.TRACE),
            (0.30, QiGrade.COMMON),
            (0.50, QiGrade.RICH),
            (0.70, QiGrade.FONT),
            (0.85, QiGrade.FONT),
        ]
        for sq, grade in cases:
            self.assertEqual(
                archive_grade_from_spirit_qi(sq), grade,
                f"spirit_qi {sq} must archive to {grade.value}",
            )


# ===========================================================================
# 5. zones.json merge 治理（§8.1 #11）
# ===========================================================================


class ZonesJsonMergeTest(unittest.TestCase):
    def _doc(self) -> dict:
        return {
            "zones": [
                {
                    "name": "alpha",
                    "aabb": {"min": [0, 0, 0], "max": [10, 10, 10]},
                    "spirit_qi": 0.1,
                    "danger_level": 3,
                    "active_events": ["e1"],
                    "patrol_anchors": [[1, 2, 3]],
                    "blocked_tiles": [],
                    "ambient_recipe_id": "amb_alpha",
                },
                {
                    "name": "beta",
                    "aabb": {"min": [20, 0, 20], "max": [30, 10, 30]},
                    "spirit_qi": -0.2,
                    "danger_level": 5,
                    "active_events": [],
                    "patrol_anchors": [],
                    "blocked_tiles": [[1, 1]],
                    "spawn_distribution": {"k": "v"},
                },
            ]
        }

    def test_only_spirit_qi_overwritten(self) -> None:
        merged = merge_zone_spirit_qi(
            self._doc()["zones"], {"alpha": 0.55, "beta": 0.7}
        )
        by_name = {z["name"]: z for z in merged}
        self.assertAlmostEqual(by_name["alpha"]["spirit_qi"], 0.55)
        self.assertAlmostEqual(by_name["beta"]["spirit_qi"], 0.7)

    def test_runtime_fields_preserved_not_clobbered(self) -> None:
        merged = merge_zone_spirit_qi(
            self._doc()["zones"], {"alpha": 0.55, "beta": 0.7}
        )
        by_name = {z["name"]: z for z in merged}
        # 运行时手工字段逐字段保留。
        self.assertEqual(by_name["alpha"]["active_events"], ["e1"])
        self.assertEqual(by_name["alpha"]["patrol_anchors"], [[1, 2, 3]])
        self.assertEqual(by_name["alpha"]["ambient_recipe_id"], "amb_alpha")
        self.assertEqual(by_name["beta"]["blocked_tiles"], [[1, 1]])
        self.assertEqual(by_name["beta"]["spawn_distribution"], {"k": "v"})
        # aabb / danger_level 也不被改（本段只动 spirit_qi）。
        self.assertEqual(by_name["alpha"]["danger_level"], 3)
        self.assertEqual(
            by_name["beta"]["aabb"], {"min": [20, 0, 20], "max": [30, 10, 30]}
        )

    def test_spirit_qi_clamped_to_native_range(self) -> None:
        merged = merge_zone_spirit_qi(self._doc()["zones"], {"alpha": 1.7})
        by_name = {z["name"]: z for z in merged}
        self.assertEqual(by_name["alpha"]["spirit_qi"], 1.0)

    def test_missing_zone_keeps_old_spirit_qi(self) -> None:
        # 派生字典只覆盖 alpha → beta 保留旧 spirit_qi（不强制全覆盖）。
        merged = merge_zone_spirit_qi(self._doc()["zones"], {"alpha": 0.5})
        by_name = {z["name"]: z for z in merged}
        self.assertAlmostEqual(by_name["alpha"]["spirit_qi"], 0.5)
        self.assertAlmostEqual(by_name["beta"]["spirit_qi"], -0.2)

    def test_unknown_derived_zone_raises(self) -> None:
        # 派生字典里有 zones.json 没有的 zone → 报错（结构性新增留人工）。
        with self.assertRaisesRegex(ValueError, "not in"):
            merge_zone_spirit_qi(self._doc()["zones"], {"gamma": 0.5})

    def test_idempotent_whitelist_is_the_governance_contract(self) -> None:
        # IDEMPOTENT_ZONE_FIELDS 是 §8.1 #11 写权限护栏的白名单（load-bearing，
        # 非纯文档常量）：导出只允许覆写这些字段。
        self.assertEqual(
            ze.IDEMPOTENT_ZONE_FIELDS,
            ("name", "aabb", "spirit_qi", "danger_level"),
        )

    def test_spirit_qi_write_passes_whitelist_guard(self) -> None:
        # 正例：只覆写 spirit_qi（白名单内）→ 护栏不误报，运行时字段全保留。
        merged = merge_zone_spirit_qi(
            self._doc()["zones"], {"alpha": 0.4, "beta": 0.4}
        )
        by_name = {z["name"]: z for z in merged}
        self.assertAlmostEqual(by_name["alpha"]["spirit_qi"], 0.4)
        self.assertEqual(by_name["alpha"]["active_events"], ["e1"])

    def test_whitelist_guard_catches_runtime_field_clobber(self) -> None:
        # 反例：若导出意外改了非白名单的运行时字段（active_events），护栏必须撞红。
        # merge 自身只写 spirit_qi，故构造一个"派生写入会顺手改运行时字段"的场景：
        # 用一个把 active_events 也塞进派生映射的子类化无法触发，改为直接验证护栏逻辑
        # ——把现有 zone 的 active_events 与导出值制造差异后调用底层断言路径。
        # 这里通过一个伪造的 merge：临时给白名单收紧（去掉 spirit_qi）使合法写也撞红，
        # 证明护栏确实以 IDEMPOTENT_ZONE_FIELDS 驱动（白名单收紧→spirit_qi 写被拦）。
        import unittest.mock as mock

        with mock.patch.object(
            ze, "IDEMPOTENT_ZONE_FIELDS", ("name", "aabb", "danger_level")
        ):
            with self.assertRaisesRegex(AssertionError, "clobbered non-idempotent"):
                merge_zone_spirit_qi(self._doc()["zones"], {"alpha": 0.99})

    def test_build_zones_file_stamps_generated_by_and_keeps_top_keys(self) -> None:
        doc = self._doc()
        doc["some_other_top_key"] = "keep_me"
        out = build_zones_file(
            doc, {"alpha": 0.4, "beta": 0.4}, generated_by="worldgen-v4 abc123"
        )
        self.assertEqual(out["generated_by"], "worldgen-v4 abc123")
        # 其它顶层键保留。
        self.assertEqual(out["some_other_top_key"], "keep_me")
        self.assertEqual(len(out["zones"]), 2)

    def test_write_zones_json_round_trip(self) -> None:
        import tempfile
        from pathlib import Path

        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "zones.json"
            path.write_text(json.dumps(self._doc()), encoding="utf-8")
            doc = ze.write_zones_json(
                path, {"alpha": 0.33, "beta": 0.66},
                generated_by="worldgen-v4 deadbeef",
            )
            on_disk = json.loads(path.read_text(encoding="utf-8"))
            self.assertEqual(on_disk["generated_by"], "worldgen-v4 deadbeef")
            by_name = {z["name"]: z for z in on_disk["zones"]}
            self.assertAlmostEqual(by_name["alpha"]["spirit_qi"], 0.33)
            self.assertEqual(by_name["alpha"]["ambient_recipe_id"], "amb_alpha")
            # 返回的 doc 与磁盘一致。
            self.assertEqual(doc["generated_by"], on_disk["generated_by"])


# ===========================================================================
# 6. 端到端：bake_zone_qi 在真实 blueprint 上守恒 + 同源
# ===========================================================================


class BakeZoneQiE2ETest(unittest.TestCase):
    def test_real_blueprint_balances_and_archives(self) -> None:
        from scripts.terrain_gen.blueprint import load_blueprint, DEFAULT_BLUEPRINT_PATH
        from scripts.terrain_gen.zones_export import bake_zone_qi, _world_area_from_bounds

        bp = load_blueprint(DEFAULT_BLUEPRINT_PATH)
        world_area = _world_area_from_bounds(bp.bounds_xz)
        result = bake_zone_qi(bp.zones, world_area=world_area, budget=100.0)
        report = result.budget_report
        # 守恒：Σ 份额 + wilderness == 预算。
        self.assertAlmostEqual(
            report.zone_sum + report.wilderness_share, 100.0,
            delta=QI_BUDGET_EPSILON,
        )
        # 每个 zone 都有派生 spirit_qi 且在 native 值域。
        self.assertEqual(len(result.derived_spirit_qi), len(bp.zones))
        for name, sq in result.derived_spirit_qi.items():
            self.assertGreaterEqual(sq, -1.0, f"{name} spirit_qi below -1")
            self.assertLessEqual(sq, 1.0, f"{name} spirit_qi above 1")
            # 归档 grade 与派生值自洽。
            self.assertEqual(
                result.archived_grades[name],
                archive_grade_from_spirit_qi(sq).value,
            )

    def test_base_wilderness_density_matches_field_base(self) -> None:
        # wilderness 质量用 base 浓度（末法薄灵）：base_field=-0.76 → density≈0.12。
        z = _zone("x", cx=0, cz=0, half=100, grade=QiGrade.COMMON)
        derived = derive_zone_spirit_qi([z], enable_veins=False)
        report = balance_qi_budget(
            [z], derived, world_area=1e8, budget=100.0, base_field=QI_FIELD_BASE
        )
        # wilderness 份额 > 0（薄灵也有质量）。
        self.assertGreater(report.wilderness_share, 0.0)

    def test_every_blueprint_zone_derives_into_its_declared_grade(self) -> None:
        # footprint over-reach 修复的**硬不变量**（全 zone 扫）：在真实 blueprint 上，
        # 每个 zone 自身 bounds 的派生 spirit_qi（面积加权均值，含邻核 + 灵脉真实叠加）
        # 必须归档回它**声明的 qi_grade 档位**。邻核 footprint 越界污染会让弱/负灵 zone
        # 被相邻常灵区抬出档位（rift_mouth → common、baolongwang → 被抬高）——本测试
        # 把"每 zone 落自身声明档"焊死，任何越界回归立即撞红。
        from scripts.terrain_gen.blueprint import (
            DEFAULT_BLUEPRINT_PATH,
            load_blueprint,
        )
        from scripts.terrain_gen.zones_export import (
            bake_zone_qi,
            _world_area_from_bounds,
            zone_qi_input_from_blueprint,
        )

        bp = load_blueprint(DEFAULT_BLUEPRINT_PATH)
        world_area = _world_area_from_bounds(bp.bounds_xz)
        result = bake_zone_qi(bp.zones, world_area=world_area, budget=100.0)
        declared = {
            zi.name: zi.grade
            for zi in (zone_qi_input_from_blueprint(z) for z in bp.zones)
        }
        offenders: list[str] = []
        for name, sq in result.derived_spirit_qi.items():
            got = archive_grade_from_spirit_qi(sq)
            if got != declared[name]:
                offenders.append(
                    f"{name}: declared={declared[name].value} derived={sq:+.4f} "
                    f"-> {got.value}"
                )
        self.assertEqual(
            offenders, [],
            "每个 blueprint zone 的派生 spirit_qi 必须落回声明 qi_grade 档位 "
            f"（邻核 footprint 不得越界污染）；越界 zone:\n"
            + "\n".join(offenders),
        )

    def test_rift_mouth_zones_stay_trace_not_pulled_to_common(self) -> None:
        # 实证回归 pin（对峙自检 major）：4 个坍缩渊口 rift_mouth 声明 trace，曾被相邻
        # blood_valley / north_waste / jiuzong 常灵核越界抬成 common（违 worldview——
        # 坍缩渊口应低灵）。修复后必须落回 trace [0.02, 0.15)。
        from scripts.terrain_gen.blueprint import (
            DEFAULT_BLUEPRINT_PATH,
            load_blueprint,
        )
        from scripts.terrain_gen.zones_export import (
            bake_zone_qi,
            _world_area_from_bounds,
        )

        bp = load_blueprint(DEFAULT_BLUEPRINT_PATH)
        world_area = _world_area_from_bounds(bp.bounds_xz)
        result = bake_zone_qi(bp.zones, world_area=world_area, budget=100.0)
        for name in (
            "rift_mouth_north_001",
            "rift_mouth_north_002",
            "rift_mouth_blood_001",
            "rift_mouth_west_001",
        ):
            sq = result.derived_spirit_qi[name]
            self.assertEqual(
                archive_grade_from_spirit_qi(sq), QiGrade.TRACE,
                f"{name} 必须落回 trace（坍缩渊口低灵），不被邻常灵区抬成 common；"
                f"得 {sq:+.4f}",
            )
            self.assertTrue(
                0.02 <= sq < 0.15,
                f"{name} 派生 spirit_qi={sq:+.4f} 必须落 trace 档区间 [0.02, 0.15)",
            )

    def test_baolongwang_cavern_stays_negative_not_lifted(self) -> None:
        # 实证回归 pin（对峙自检 major）：baolongwang_cavern_deep 声明 negative(-0.80)，
        # 曾被相邻 zhanhun_plain(0.4) 常灵核越界抬成 -0.43。修复后必须落回 negative
        # (<-0.1) 并贴近其声明精确值 -0.80（自身 bounds 由它自己的核主导）。
        from scripts.terrain_gen.blueprint import (
            DEFAULT_BLUEPRINT_PATH,
            load_blueprint,
        )
        from scripts.terrain_gen.zones_export import (
            bake_zone_qi,
            _world_area_from_bounds,
        )

        bp = load_blueprint(DEFAULT_BLUEPRINT_PATH)
        world_area = _world_area_from_bounds(bp.bounds_xz)
        result = bake_zone_qi(bp.zones, world_area=world_area, budget=100.0)
        sq = result.derived_spirit_qi["baolongwang_cavern_deep"]
        self.assertEqual(
            archive_grade_from_spirit_qi(sq), QiGrade.NEGATIVE,
            f"baolongwang 必须落回 negative（负灵域），不被邻常灵核抬高；得 {sq:+.4f}",
        )
        self.assertLess(
            sq, -0.1,
            f"baolongwang 派生 spirit_qi={sq:+.4f} 必须 <-0.1（negative 档）",
        )


if __name__ == "__main__":  # pragma: no cover
    unittest.main()
