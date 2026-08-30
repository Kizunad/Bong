#!/usr/bin/env python3
"""草甲生成器的回归锁。

重点不在"跑得通"，而在把 round 1~3 里**真撞出来过**的那几类错各钉一根桩：
杆板分色（1 texel/单位下唯一能表达"一根根杆"的手段）、绳与杆的明度差、
跨件同穿的共面、钻进原版手臂体积、box-uv 越出色调格、左右不镜像。

每条校验都配一个**变异用例**：故意造一个坏 part 喂进去，断言它真的报错。
只测"好输入不报错"等于没测——校验函数整个删掉那种测试照样绿。
"""

from __future__ import annotations

import colorsys
import sys
import unittest
from dataclasses import replace
from pathlib import Path

LIB_DIR = Path(__file__).resolve().parents[1]
for _d in ("generators", "tools"):
    sys.path.insert(0, str(LIB_DIR / _d))

import gen_straw_armor as straw  # noqa: E402
from bbmodel_maker.model.armor_model_common import ArmorPart, Cube, MOUNT_X, build_bbmodel, validate_part  # noqa: E402


def _part_with(cubes: tuple[Cube, ...]) -> ArmorPart:
    return ArmorPart("probe", "PROBE", cubes)


def _median_hsv(image, box) -> tuple[float, float, float]:
    u0, v0, u1, v1 = box
    channels = [[], [], []]
    for y in range(v0, v1):
        for x in range(u0, u1):
            for index, value in enumerate(image.getpixel((x, y))):
                channels[index].append(value)
    med = [sorted(c)[len(c) // 2] / 255.0 for c in channels]
    h, s, v = colorsys.rgb_to_hsv(*med)
    return h * 360.0, s * 100.0, v * 100.0


class StrawArmorShapeTest(unittest.TestCase):
    def test_exposes_exactly_leggings_and_boots(self) -> None:
        parts = straw.parts()
        self.assertEqual(["straw_leggings", "straw_boots"], [p.key for p in parts])
        for part in parts:
            validate_part(part)

    def test_leggings_ring_is_staves_not_one_box(self) -> None:
        """稻杆捆必须是一圈杆板。做成一个整盒就没有板缝，正视读成一只桶。"""
        cubes = straw.part_leggings().cubes
        staves = [c for c in cubes if c.name.startswith("stave_")]
        self.assertGreaterEqual(len(staves), 24, "前后各五板 + 外侧三板 + 内侧一板，两条腿")
        for cube in staves:
            self.assertLess(
                cube.size[0], 1.1,
                f"{cube.name} 宽 {cube.size[0]}——杆板超过 1.1 就读成木条不是稻杆",
            )

    def test_adjacent_front_staves_use_different_tones(self) -> None:
        """相邻杆板必须取不同色调列。

        这是全件最要紧的一条：box-uv 是 1 texel≈1 单位，一块 0.93 宽的板正面只
        采到一个 texel，贴图里画什么杆线都进不去。竖向分节**只能**靠分色，
        两块挨着的板撞成同一个调子，那一段就糊成一块平板。
        """
        for prefix in ("stave_front_", "stave_back_"):
            row = [c for c in straw.part_leggings().cubes
                   if c.name.startswith(prefix) and c.name.endswith("_left")]
            row.sort(key=lambda c: c.origin[0])
            self.assertEqual(5, len(row), prefix)
            for first, second in zip(row, row[1:]):
                self.assertNotEqual(
                    first.uv, second.uv,
                    f"{first.name} 与 {second.name} 同色调 {first.uv}，两板会糊成一块",
                )

    def test_stave_tones_are_not_a_monotonic_ramp(self) -> None:
        """取色是 A→C→B→D 隔位跳，不是顺序轮转。

        顺着 A→B→C→D 来会渲成从亮到暗的渐变带，读成"打了柔光的一块板"，
        而不是随机长成这样的一捆杆。
        """
        row = [c for c in straw.part_leggings().cubes
               if c.name.startswith("stave_front_") and c.name.endswith("_left")]
        row.sort(key=lambda c: c.origin[0])
        order = [straw.STRAW_TONES.index(c.uv) for c in row if c.uv in straw.STRAW_TONES]
        deltas = [b - a for a, b in zip(order, order[1:])]
        self.assertFalse(
            all(d > 0 for d in deltas) or all(d < 0 for d in deltas),
            f"色调序 {order} 是单调的，会渲成渐变带",
        )

    def test_boot_sole_has_raised_rim_over_recessed_fill(self) -> None:
        """草底得是"凸起的绳盘 + 凹下去的编心"。

        round 1/2 用五道等高横片，与参考并排一看就露馅：截面在侧视排成一列，
        整只鞋读成木托盘。沿口必须比编心高，凹凸才把"盘绳"衬出来。
        """
        cubes = {c.name: c for c in straw.part_boots().cubes}
        rim_top = max(c.origin[1] + c.size[1] for n, c in cubes.items() if n.startswith("rim_"))
        fill_top = max(c.origin[1] + c.size[1] for n, c in cubes.items() if n.startswith("fill_"))
        self.assertGreater(rim_top, fill_top + 0.08,
                           f"沿口 {rim_top} 没有明显高过编心 {fill_top}")
        for name, cube in cubes.items():
            if name.startswith("rim_"):
                self.assertEqual(straw.UV_CORD, cube.uv, f"{name} 得走绳纹才读得出是盘的")

    def test_boot_sole_sits_below_the_foot(self) -> None:
        """底面必须压到 y<0：脚底本身在 y=0，鞋底停在 0 就是和脚底面共面打架。"""
        base = [c for c in straw.part_boots().cubes if c.name.startswith("base_")]
        self.assertTrue(base)
        for cube in base:
            self.assertLess(cube.origin[1], 0.0, f"{cube.name} 底面在 {cube.origin[1]}，没到脚底之下")

    def test_ankle_wrap_is_two_offset_halves_not_one_flat_ring(self) -> None:
        """踝上的绳是斜着螺旋缠的。做成一道水平整环，正背视活像一段栏杆扶手。"""
        lows = [c for c in straw.part_boots().cubes if c.name.startswith("wrap_lo_")]
        highs = [c for c in straw.part_boots().cubes if c.name.startswith("wrap_hi_")]
        self.assertTrue(lows and highs)
        self.assertGreater(
            min(c.origin[1] for c in highs) - max(c.origin[1] for c in lows), 0.2,
            "高低两半环的落差不足，转一圈看过去仍是平的",
        )


class StrawArmorGuardTest(unittest.TestCase):
    """六道校验的变异测试——每条都喂一个坏 part，断言它真的拦住。"""

    def test_all_guards_pass_on_the_real_parts(self) -> None:
        parts = straw.parts()
        straw._assert_no_coplanar_faces(parts)
        straw._assert_uv_tiles(parts)
        straw._assert_inner_edges_meet(parts)
        straw._assert_mirror_symmetry(parts)
        straw._assert_no_body_clash(parts)
        straw._assert_parts_stack_cleanly(parts)

    def test_coplanar_guard_catches_same_plane_pair(self) -> None:
        bad = _part_with((
            Cube("LEFT_LEG", "a", (0.0, 4.0, 0.0), (1.0, 2.0, 1.0), straw.UV_STRAW_A),
            Cube("LEFT_LEG", "b", (0.2, 4.0, 0.2), (1.0, 2.0, 1.0), straw.UV_STRAW_A),
        ))
        with self.assertRaisesRegex(ValueError, "共面"):
            straw._assert_no_coplanar_faces((bad,))

    def test_coplanar_guard_looks_across_mounts(self) -> None:
        """左右腿是两个 mount，但静止姿下 MOUNT_X 已经把它们摆进同一片世界空间，
        裆缝处照样会打架。只比同 mount 的检查抓不到。"""
        bad = _part_with((
            Cube("LEFT_LEG", "l", (-1.9, 4.0, -1.0), (1.0, 2.0, 2.0), straw.UV_STRAW_A),
            Cube("RIGHT_LEG", "r", (1.4, 4.0, -1.0), (1.0, 2.0, 2.0), straw.UV_STRAW_A),
        ))
        with self.assertRaisesRegex(ValueError, "共面"):
            straw._assert_no_coplanar_faces((bad,))

    def test_inner_edge_guard_catches_crossing_the_centreline(self) -> None:
        for mount, origin in (("LEFT_LEG", -2.1), ("RIGHT_LEG", 1.2)):
            bad = _part_with((Cube(mount, "x", (origin, 4.0, 0.0), (1.0, 1.0, 1.0),
                                   straw.UV_STRAW_A),))
            with self.assertRaisesRegex(ValueError, "探进"):
                straw._assert_inner_edges_meet((bad,))

    def test_inner_edge_guard_allows_exactly_touching_zero(self) -> None:
        ok = _part_with((
            Cube("LEFT_LEG", "l", (-1.9, 4.0, 0.0), (1.0, 1.0, 1.0), straw.UV_STRAW_A),
            Cube("RIGHT_LEG", "r", (0.9, 6.0, 0.0), (1.0, 1.0, 1.0), straw.UV_STRAW_A),
        ))
        straw._assert_inner_edges_meet((ok,))

    def test_mirror_guard_catches_sign_slip(self) -> None:
        bad = _part_with((
            Cube("LEFT_LEG", "p_left", (0.0, 4.0, 0.0), (1.0, 1.0, 1.0), straw.UV_STRAW_A),
            Cube("RIGHT_LEG", "p_right", (0.3, 4.0, 0.0), (1.0, 1.0, 1.0), straw.UV_STRAW_A),
        ))
        with self.assertRaisesRegex(ValueError, "不镜像"):
            straw._assert_mirror_symmetry((bad,))

    def test_mirror_guard_catches_unpaired_names(self) -> None:
        bad = _part_with((
            Cube("LEFT_LEG", "only_left", (0.0, 4.0, 0.0), (1.0, 1.0, 1.0), straw.UV_STRAW_A),
        ))
        with self.assertRaisesRegex(ValueError, "不成对"):
            straw._assert_mirror_symmetry((bad,))

    def test_body_clash_guard_catches_a_cube_inside_the_arm(self) -> None:
        """腿甲顶沿要够到髋线才不露缝，越界一点点就撞上从 y12 起的手臂。
        静止三视图里被手臂自己挡住，一抬手才露——只能靠算。"""
        bad = _part_with((Cube("LEFT_LEG", "spike", (2.5, 11.5, -1.0), (0.6, 1.2, 0.6),
                               straw.UV_WORN_A),))
        with self.assertRaisesRegex(ValueError, "手臂"):
            straw._assert_no_body_clash((bad,))

    def test_body_clash_guard_catches_a_cube_inside_the_torso(self) -> None:
        bad = _part_with((Cube("LEFT_LEG", "tab", (0.0, 11.5, -1.0), (1.0, 1.2, 1.0),
                               straw.UV_WORN_A),))
        with self.assertRaisesRegex(ValueError, "躯干"):
            straw._assert_no_body_clash((bad,))

    def test_body_clash_guard_allows_fringe_that_stays_below_the_hip(self) -> None:
        ok = _part_with((Cube("LEFT_LEG", "tab", (0.0, 11.0, -1.0), (1.0, 0.9, 1.0),
                              straw.UV_WORN_A),))
        straw._assert_no_body_clash((ok,))

    def test_stack_guard_catches_coplanar_across_two_parts(self) -> None:
        """护腿和草鞋是同时穿的，两件各自合法、合起来照样 z-fighting。"""
        a = ArmorPart("aa", "AA", (Cube("LEFT_LEG", "x", (2.0, 2.0, -1.0), (0.6, 1.0, 0.8),
                                        straw.UV_CORD),))
        b = ArmorPart("bb", "BB", (Cube("LEFT_FOOT", "y", (2.0, 2.4, -0.9), (0.6, 1.0, 0.8),
                                        straw.UV_CORD),))
        with self.assertRaisesRegex(ValueError, "同时穿"):
            straw._assert_parts_stack_cleanly((a, b))

    def test_stack_guard_allows_interpenetration_without_shared_faces(self) -> None:
        """件与件**互相插入是允许的**——草绑腿本来就该压在草鞋的绑绳上。
        只禁表面重合。"""
        a = ArmorPart("aa", "AA", (Cube("LEFT_LEG", "x", (2.0, 2.0, -1.0), (0.6, 1.0, 0.8),
                                        straw.UV_CORD),))
        b = ArmorPart("bb", "BB", (Cube("LEFT_FOOT", "y", (2.1, 2.4, -0.9), (0.7, 1.1, 0.9),
                                        straw.UV_CORD),))
        straw._assert_parts_stack_cleanly((a, b))

    def test_uv_tile_guard_catches_a_box_uv_spilling_into_the_next_tone(self) -> None:
        """跨列会采到隔壁调子——症状是某块杆板莫名比邻居亮/暗一大截，
        而这在三视图里极容易被当成"随机长成这样"放过去。"""
        bad = _part_with((Cube("LEFT_LEG", "wide", (0.0, 4.0, 0.0), (3.0, 2.0, 3.0),
                               straw.UV_STRAW_A),))
        with self.assertRaisesRegex(ValueError, "超出"):
            straw._assert_uv_tiles((bad,))

    def test_uv_tile_guard_rejects_an_unregistered_origin(self) -> None:
        bad = _part_with((Cube("LEFT_LEG", "odd", (0.0, 4.0, 0.0), (1.0, 1.0, 1.0), (5, 7)),))
        with self.assertRaisesRegex(ValueError, "不在 UV_TILES"):
            straw._assert_uv_tiles((bad,))


class StrawArmorTextureTest(unittest.TestCase):
    def test_texture_is_deterministic(self) -> None:
        self.assertEqual(straw.make_texture().tobytes(), straw.make_texture().tobytes())

    def test_texture_is_64x64(self) -> None:
        self.assertEqual((64, 64), straw.make_texture().size)

    def test_four_straw_tones_are_distinct_and_ordered_by_value(self) -> None:
        image = straw.make_texture()
        values = [_median_hsv(image, (u, v, u + 8, v + 32))[2] for u, v in straw.STRAW_TONES]
        self.assertEqual(4, len({round(x) for x in values}), f"四列明度撞车了：{values}")
        self.assertGreater(max(values) - min(values), 10.0,
                           f"明度跨度 {max(values) - min(values):.1f} 太小，MC 着色后分不开")

    def test_cord_is_clearly_darker_than_straw(self) -> None:
        """本图唯一一处刻意背离参考的取色。

        参考量到绳 V47 / 稻杆 V56，只差 9 点；照这个差放进 MC，绳和杆的差只剩
        6%，绳箍在身上整个消失（round 2 实测）。参考里绳能读出来靠的是它自己的
        受光角度（真渲染有软阴影），MC 分轴着色给不了。所以用色差把那份区分补
        回来——这条一旦被"改回参考值"就会退化，故钉死。
        """
        image = straw.make_texture()
        _, _, cord_v = _median_hsv(image, (32, 32, 64, 64))
        straw_v = max(_median_hsv(image, (u, v, u + 8, v + 32))[2] for u, v in straw.STRAW_TONES)
        self.assertLess(cord_v, straw_v - 12.0,
                        f"绳 V{cord_v:.0f} 没比最亮的稻杆 V{straw_v:.0f} 暗够 12 点，会看不见")

    def test_hue_stays_in_the_straw_band(self) -> None:
        """参考图整体在 H38~41（稻黄）。偏到 30 以下就成了红褐皮革。"""
        image = straw.make_texture()
        for name, box in (("稻杆A", (0, 0, 8, 32)), ("稻杆C", (16, 0, 24, 32)),
                          ("编织", (0, 32, 32, 64)), ("绳", (32, 32, 64, 64))):
            hue = _median_hsv(image, box)[0]
            self.assertTrue(33.0 <= hue <= 46.0, f"{name} 色相 H{hue:.1f} 跑出稻黄区间")


class StrawArmorOutputTest(unittest.TestCase):
    def test_bbmodel_round_trips_for_both_parts(self) -> None:
        texture = straw.make_texture()
        for part in straw.parts():
            model = build_bbmodel(straw.MATERIAL, part, texture)
            self.assertEqual(len(part.cubes), len(model["elements"]))
            self.assertEqual(f"geometry.bong.{part.key}", model["model_identifier"])
            mounts = {c.mount for c in part.cubes}
            self.assertEqual(mounts, {g["name"].upper() for g in model["outliner"]})

    def test_element_origins_carry_the_mount_offset(self) -> None:
        """bbmodel 里的坐标是**世界**坐标（局部 + MOUNT_X），group origin 才是枢轴。
        写混了在 Blockbench 里一拖旋转就散架。"""
        texture = straw.make_texture()
        part = straw.part_boots()
        model = build_bbmodel(straw.MATERIAL, part, texture)
        by_name = {e["name"]: e for e in model["elements"]}
        for cube in part.cubes:
            self.assertAlmostEqual(cube.origin[0] + MOUNT_X[cube.mount],
                                   by_name[cube.name]["from"][0], places=3)

    def test_emit_java_and_digest_are_stable(self) -> None:
        for part in straw.parts():
            java = straw.emit_java(part)
            self.assertEqual(len(part.cubes), java.count("new ArmorCube("))
            self.assertIn("Mount.LEFT_", java)
            digest = straw.cube_digest(part)
            self.assertEqual(16, len(digest))
            self.assertEqual(digest, straw.cube_digest(part))

    def test_digest_changes_when_geometry_changes(self) -> None:
        """digest 是 client 侧 ArmorPartModelTest 的 pin 值来源；它要是对几何
        改动不敏感，那边的 pin 就锁了个寂寞。"""
        part = straw.part_boots()
        moved = replace(part, cubes=(replace(part.cubes[0], origin=(0.0, 9.0, 0.0)),)
                        + part.cubes[1:])
        self.assertNotEqual(straw.cube_digest(part), straw.cube_digest(moved))


if __name__ == "__main__":
    unittest.main()
