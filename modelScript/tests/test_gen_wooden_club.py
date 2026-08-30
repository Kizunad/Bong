#!/usr/bin/env python3
"""木棍 + `held_item_common` 新增公共层的回归锁。

和 `test_gen_knife_trio` 同一路数，钉两类东西：

1. **那些"看着像错、其实是刻意的"决定**，以及**三轮打磨里真正翻过车的那几处**。
   后者尤其重要——木棍的断茬那一簇被推翻重做过两次（先是矮而宽的盖子读成锤头，
   再是张太开的碎块读成飘着的木片），两次的中间产物在正视图里都"看着没毛病"。
   没有断言的话，下一个人照着参考图顺手改回去，两个坑会原样复现。
2. **公共层新抽出来的三样**（`noise_fill` / `blotch` / `hand_display` /
   `assert_boxes_are_connected`）。抽出来的目的是不再"两套各写一份"，那就得有东西
   保证它俩真的用的是同一份。

每条校验都配变异用例：造坏输入喂进去，断言它真报错。
"""

from __future__ import annotations

import math
import sys
import unittest
from pathlib import Path

from PIL import Image

LIB_DIR = Path(__file__).resolve().parents[1]
for _d in ("generators",):
    sys.path.insert(0, str(LIB_DIR / _d))

import gen_wooden_club as club  # noqa: E402
from bbmodel_maker.render.held_item_common import (  # noqa: E402
    GAP_TOLERANCE,
    MIN_CONTACT_RATIO,
    TILE,
    Box,
    HeldItem,
    Material,
    assert_boxes_are_connected,
    assert_conventions,
    assert_no_coplanar_faces,
    centre_translation,
    hand_display,
)

ITEM = club.WOODEN_CLUB
LENGTH = club.LENGTH


def _swatch(rgb=(128, 128, 128)) -> Image.Image:
    return Image.new("RGBA", (TILE, TILE), (*rgb, 255))


def _probe(boxes, grip=0.15) -> HeldItem:
    return HeldItem("probe", "PROBE", "stick", tuple(boxes),
                    (Material("m", (0.5, 0.5, 0.5), _swatch()),), {}, grip)


def _mean_rgb(image: Image.Image) -> tuple[float, float, float]:
    data = image.convert("RGB").tobytes()
    n = len(data) // 3
    return tuple(sum(data[i::3]) / n for i in range(3))


def _span_at(y: float, axis: int) -> float:
    """`y` 高度处的横向跨度（含刺与刺之间的空隙）。"""
    here = [b for b in ITEM.boxes if b.low[1] <= y <= b.high[1]]
    if not here:
        return 0.0
    return max(b.high[axis] for b in here) - min(b.low[axis] for b in here)


def _material_of(name: str) -> Image.Image:
    return next(m.texture for m in ITEM.materials if m.name == name)


class WoodenClubShapeTest(unittest.TestCase):
    def test_passes_every_guard(self) -> None:
        assert_conventions(ITEM)
        assert_no_coplanar_faces(ITEM)
        assert_boxes_are_connected(ITEM)

    def test_is_the_wooden_club_template(self) -> None:
        """key 必须正好是 server 侧的 template_id。差一个字这件资产就永远挂不上去
        （`workbench_materials.toml` 的 `id = "wooden_club"`，kind=staff 的真武器）。"""
        self.assertEqual("wooden_club", ITEM.key)

    def test_aspect_ratio_is_deliberately_chunkier_than_the_reference(self) -> None:
        """长宽比必须在 3.5~5.2，**不是**参考图的 6.84。

        参考是实物照。照抄进 MC 是错的：手持物在屏幕上就三十来像素，6.8:1 渲出来
        是一根拖把杆。原版剑连护手约 4:1，这里对齐它。这条断言存在的唯一目的，
        就是拦住"照参考图修回细长"。
        """
        width = (max(b.high[0] for b in ITEM.boxes)
                 - min(b.low[0] for b in ITEM.boxes))
        ratio = LENGTH / width
        self.assertTrue(
            3.5 <= ratio <= 5.2,
            f"长宽比 {ratio:.2f} 出界。参考图是 6.84，但那是实物照；"
            f"MC 手持物照抄会渲成一根杆。别按参考改回去。",
        )

    def test_it_is_a_bundle_of_distinct_staves_not_one_tapered_stick(self) -> None:
        """四根木条必须**互不相同**：位置、粗细、顶高都不能撞。

        全一样的话整束在数学上就退化成一根锥体——正视看不出来，一转到侧视就是
        一根规整的棍，参考图那个"捆起来的"读感全丢，而那是这件唯一的记忆点。
        """
        self.assertGreaterEqual(len(club._STAVES), 3, "少于三根谈不上“束”")
        sections = [(u, v, hu, hv) for _, u, v, hu, hv, _, _ in club._STAVES]
        self.assertEqual(len(sections), len(set(sections)), "有两根木条截面完全一样")
        tops = [top for *_, top in club._STAVES]
        self.assertEqual(len(tops), len(set(tops)), f"木条顶高有重复 {tops}")

    def test_splinters_stand_up_instead_of_capping_the_head(self) -> None:
        """每根断茬自己必须是**竖的**（高 / 宽 ≥ 1.5）。

        round 2 把茬口做成 `hu*0.86` 宽、只有 0.05~0.08 高——四块矮而宽的盖子并排，
        正视直接读成**锤头 / 斧刃**。是钝器没错，但完全不是"劈开的木束"。
        参差感要靠**高度差**给，不是靠把每根加宽。
        """
        for box in ITEM.boxes:
            if box.material != "club_split" or box.center[1] < LENGTH * 0.5:
                continue
            height = box.half[1] * 2
            width = max(box.half[0], box.half[2]) * 2
            self.assertGreaterEqual(
                height / width, 1.5,
                f"{box.name} 高宽比 {height / width:.2f} < 1.5 —— 这是块盖子不是刺，"
                f"一簇这样的东西读成锤头",
            )

    def test_the_head_is_ragged_not_sawn_flat(self) -> None:
        """断茬的顶端高度必须散开至少 0.06（≈ 全长 7%）。齐平的顶读成"锯断的棍"。"""
        tops = sorted({round(b.high[1], 4) for b in ITEM.boxes
                       if b.material == "club_split" and b.high[1] > LENGTH * 0.8})
        self.assertGreaterEqual(len(tops), 4, f"顶端只有 {len(tops)} 个高度，太齐")
        self.assertGreater(
            tops[-1] - tops[0], 0.06,
            f"断茬顶端只散开 {tops[-1] - tops[0]:.3f}，读成一刀锯平的棍")

    def test_the_bundle_widens_monotonically_toward_the_head(self) -> None:
        """`_taper` 从握把到接近顶端必须单调不减。

        中间凹一下就是"腰"，那是酒瓶不是棍。顶端最后一档允许微收（断茬处木条本身
        在收），所以只查到 0.90。
        """
        previous = 0.0
        for step in range(0, 91):
            value = club._taper(LENGTH * step / 100.0)
            self.assertGreaterEqual(
                value + 1e-9, previous,
                f"y/L={step / 100:.2f} 处束宽 {value:.3f} 比下面还窄 —— 出现了“腰”")
            previous = value

    def test_the_gripped_end_is_the_narrow_end(self) -> None:
        """握把处的跨度不得超过最粗处的 62%。

        参考实测是 53%（底 73px / 最粗 137px）；放宽到 62% 是给 MC 的可读性留量，
        再宽就是一根等粗的棍，"重心在前"的钝器读感没了。
        """
        widest = max(_span_at(LENGTH * i / 200.0, 0) for i in range(201))
        at_grip = _span_at(club.GRIP, 0)
        self.assertLess(
            at_grip / widest, 0.62,
            f"握把跨度是最粗处的 {at_grip / widest:.0%}，太粗——读成等粗的棍")

    def test_depth_matches_the_measured_side_to_front_ratio(self) -> None:
        """侧/正宽比 0.71 ± 0.06（参考逐像素量的：正视 max 157、侧视 111）。

        这件是**扁束**不是圆棍。做成 1.0 就是根方料，做成 0.5 就是块板。
        只量木身段（避开会外张的断茬和外凸的绳缠）。
        """
        lo, hi = LENGTH * 0.30, LENGTH * 0.68
        widths = [_span_at(lo + (hi - lo) * i / 40.0, 0) for i in range(41)]
        depths = [_span_at(lo + (hi - lo) * i / 40.0, 2) for i in range(41)]
        ratio = sum(depths) / sum(widths)
        self.assertAlmostEqual(
            0.71, ratio, delta=0.06,
            msg=f"木身侧/正 = {ratio:.3f}，参考是 0.71")

    def test_the_cord_sits_flush_with_the_bundle(self) -> None:
        """绳缠外凸不得超过整束外沿的 6%。

        round 1 拿"包络"（±1.0 单位）当绳缠宽度，而四根木条的并集只占 u 向 ±0.935、
        v 向 ±0.81——绳缠因此比木身宽 16%、深 35%，五道箍在 3/4 视里读成**摞起来的
        五片圆盘**，不是缠上去的绳。
        """
        _, bundle_u = club._bundle(0)
        _, bundle_v = club._bundle(1)
        for box in ITEM.boxes:
            if not box.name.startswith("wrap_"):
                continue
            k = club._taper(box.center[1])
            proud_u = box.half[0] / (bundle_u * k * club.HALF_X)
            proud_v = box.half[2] / (bundle_v * k * club.HALF_Z)
            self.assertLess(max(proud_u, proud_v), 1.06,
                            f"{box.name} 外凸 {max(proud_u, proud_v):.3f} —— 读成圆盘")

    def test_the_coils_are_uneven(self) -> None:
        """五道绳的厚度不能整齐划一——等厚等宽会读成"车出来的凹槽"。"""
        heights = [b.half[1] * 2 for b in ITEM.boxes if b.name.startswith("wrap_")]
        self.assertGreaterEqual(len(heights), 4, "绳缠少于四道，缠不出握把")
        self.assertGreater(max(heights) - min(heights), 1e-4,
                           f"五道绳等厚 {heights} —— 读成车出来的凹槽")

    def test_the_fist_lands_on_the_wrapped_section(self) -> None:
        """拳头（世界里 4px 宽）至少 75% 落在绳缠 + 束尾那一段。

        握把点 `grip` 决定 `emit_offset`，也就决定了整件挂在手上的位置。它要是落到
        光木身上，玩家握的就是棍子中段——参考图缠绳缠在那儿正是为了给手抓。
        """
        fist = 4.0 / (16.0 * club.SCALE)              # 模型单位
        lo, hi = club.GRIP - fist / 2, club.GRIP + fist / 2
        covered = max(0.0, min(hi, club.WRAP_HI) - max(lo, 0.0))
        self.assertGreater(
            covered / fist, 0.75,
            f"拳头只有 {covered / fist:.0%} 落在握把段（{lo:.3f}~{hi:.3f} vs "
            f"0~{club.WRAP_HI}）")

    def test_split_material_stays_on_the_head(self) -> None:
        """新劈面只许出现在顶端。低处冒出亮块 = 那里凭空多了个断口。

        束尾那一小截曾经也用 club_split，正视里成了一只白脚，把视线从断茬那头拽
        下来；改回 club_wood 之后这条断言把它钉住。
        """
        for box in ITEM.boxes:
            if box.material == "club_split":
                self.assertGreater(
                    box.low[1], LENGTH * 0.6,
                    f"{box.name} 在 y={box.low[1]:.3f} 就用了新劈面，太靠下")

    def test_wrap_geometry_follows_the_stave_table(self) -> None:
        """变异用例：挪一根木条，绳缠和束尾必须跟着变。

        `_bundle()` 存在的全部理由就是这个——写死一个宽度，改完 stave 排布之后
        绳缠还箍在旧位置上，而三视图里未必看得出来。
        """
        before = [b for b in club.part_wrap() + club.part_butt()]
        original = club._STAVES
        try:
            club._STAVES = tuple(
                (n, u * 1.5, v, hu, hv, cuts, top)
                for n, u, v, hu, hv, cuts, top in original
            )
            after = [b for b in club.part_wrap() + club.part_butt()]
        finally:
            club._STAVES = original
        self.assertNotEqual([b.half for b in before], [b.half for b in after],
                            "改了木条排布，绳缠/束尾却纹丝不动 —— _bundle 没接上")


class WoodenClubTextureTest(unittest.TestCase):
    def test_the_split_face_reads_brighter_and_warmer_than_the_shaft(self) -> None:
        """新劈面要比风化的木身亮、且更暖。

        参考量得只差 8%（V41 vs V38），那是照片里断茬**互相投影**的结果；MC 的 item
        光照在这个尺度上邻接 cube 不投影，照抄 8% 等于没差别，顶端会读成"削平的
        棍头"。所以刻意拉到 ~18%，同时往暖里推——新劈的木头本来就比风化面黄。
        """
        wood = _mean_rgb(_material_of("club_wood"))
        split = _mean_rgb(_material_of("club_split"))
        lift = sum(split) / sum(wood)
        self.assertGreater(lift, 1.10, f"新劈面只亮 {lift:.3f} 倍，顶端读不出断茬")
        self.assertGreater(
            (split[0] - split[2]) - (wood[0] - wood[2]), 4.0,
            "新劈面不比木身暖 —— 冷灰的亮块会读成火光而不是木头内部")

    def test_the_split_face_is_not_bright_enough_to_read_as_a_torch(self) -> None:
        """但也**不能一味放亮**：断茬占了顶部 18% 的高度，真实手持尺寸下一片过亮的
        块压在深色棍身上，整件读成火把——`gen_knife_trio` 的石刃踩过同一个坑。"""
        wood = _mean_rgb(_material_of("club_wood"))
        split = _mean_rgb(_material_of("club_split"))
        self.assertLess(sum(split) / sum(wood), 1.32,
                        "新劈面太亮，整件读成火把")

    def test_the_cord_is_a_different_material_at_a_glance(self) -> None:
        """绳缠要能一眼和木身分开，否则握把段白做。"""
        wood = _mean_rgb(_material_of("club_wood"))
        cord = _mean_rgb(_material_of("club_cord"))
        dist = sum((wood[i] - cord[i]) ** 2 for i in range(3)) ** 0.5
        self.assertGreater(dist, 25.0, f"绳与木平均色只差 {dist:.1f}，手里分不出")

    def test_the_grain_is_at_most_a_few_columns(self) -> None:
        """竖纹最多三列。

        柄面在屏幕上不过十来像素，一张 16² 图铺上去超过两三条竖线就必然读成**条纹
        布**（小刀那批用三条木纹换来的教训）。这件的"一捆"读感全部由几何给，贴图
        只负责材质。
        """
        image = _material_of("club_wood").convert("RGB")
        columns = [sum(image.getpixel((x, y))[0] for y in range(TILE)) / TILE
                   for x in range(TILE)]
        median = sorted(columns)[TILE // 2]
        dark = [x for x, v in enumerate(columns) if v < median - 6]
        self.assertLessEqual(len(dark), 3, f"有 {len(dark)} 列明显偏暗，读成条纹布")

    def test_textures_are_deterministic(self) -> None:
        """重跑生成器不该产生 diff —— 否则 git 上分不清"改了造型"和"跑了一遍"。"""
        first = [m.texture.tobytes() for m in ITEM.materials]
        import importlib

        importlib.reload(club)
        second = [m.texture.tobytes() for m in club.WOODEN_CLUB.materials]
        self.assertEqual(first, second)


class ConnectivityGuardTest(unittest.TestCase):
    """`assert_boxes_are_connected` 的变异测试。

    这道闸是做木棍时加的，上线当天就抓到一个**已 merge 的真缺陷**：`iron_dagger`
    的柄顶到 0.2635、护环底在 0.2688，中间空着 0.0053——整条刃加护环是一块和柄
    不相连的浮空体。三视图渲了三轮没人看出来，因为那道缝在图上不到一个像素。
    """

    def _stack(self, gap: float = 0.0):
        """两块上下相接的分段。`gap` > 0 就是中间留缝。"""
        return [
            Box("low", "m", (0.0, 0.15, 0.0), (0.05, 0.15, 0.04)),
            Box("up", "m", (0.0, 0.30 + gap + 0.10, 0.0), (0.04, 0.10, 0.03)),
        ]

    def test_abutting_segments_are_connected(self) -> None:
        """上下贴面相接是本模块所有件的常态（刀柄三段、木条五段），相交**体积恒为
        0**——用体积判会把每一件正常资产都判成散架。"""
        assert_boxes_are_connected(_probe(self._stack()))

    def test_a_hairline_gap_is_tolerated(self) -> None:
        """小于 `GAP_TOLERANCE` 的缝当贴上了：件在真实手持尺寸下约 110px/方块，
        这个量级不到半个像素。小刀的绳缠道间就留着 0.0003 的缝。"""
        assert_boxes_are_connected(_probe(self._stack(gap=GAP_TOLERANCE * 0.5)))

    def test_a_visible_gap_raises(self) -> None:
        with self.assertRaisesRegex(ValueError, "不连通|接触面积"):
            assert_boxes_are_connected(_probe(self._stack(gap=0.05)))

    def test_a_corner_only_touch_raises(self) -> None:
        """只碰到一个角：接触面积是 0，渲染上和断开没区别。"""
        boxes = [
            Box("main", "m", (0.0, 0.20, 0.0), (0.05, 0.20, 0.04)),
            Box("chip", "m", (0.10, 0.44, 0.08), (0.05, 0.04, 0.04)),
        ]
        with self.assertRaisesRegex(ValueError, "接触面积"):
            assert_boxes_are_connected(_probe(boxes))

    def test_a_side_by_side_slab_is_fine(self) -> None:
        """**只**在横向浅浅搭着不算缺陷：两块并排共面本来就是正常拼接（和上下堆叠
        同构）。这条挡的是"把闸做得太严"——严到把正常拼法也判红，就只能被调松或
        被绕过，等于没有。木棍的断茬曾被我当成这种情形误诊过，实测拓扑完好，
        3/4 视里那道"缝"是 MC 侧面着色，不是断开。
        """
        boxes = [
            Box("main", "m", (0.0, 0.20, 0.0), (0.05, 0.20, 0.04)),
            Box("flake", "m", (0.098, 0.30, 0.0), (0.05, 0.06, 0.04)),
        ]
        assert_boxes_are_connected(_probe(boxes))

    def test_a_healthy_overlap_passes(self) -> None:
        boxes = [
            Box("main", "m", (0.0, 0.20, 0.0), (0.05, 0.20, 0.04)),
            Box("branch", "m", (0.06, 0.34, 0.0), (0.04, 0.08, 0.03)),
        ]
        assert_boxes_are_connected(_probe(boxes))

    def test_two_separate_clumps_raise(self) -> None:
        """每块各自都连着邻居，但整体分成两团 —— 单看"有没有邻居"抓不住。"""
        boxes = [
            Box("a1", "m", (0.0, 0.10, 0.0), (0.05, 0.10, 0.04)),
            Box("a2", "m", (0.0, 0.26, 0.0), (0.04, 0.06, 0.03)),
            Box("b1", "m", (0.0, 0.50, 0.0), (0.05, 0.10, 0.04)),
            Box("b2", "m", (0.0, 0.66, 0.0), (0.04, 0.06, 0.03)),
        ]
        with self.assertRaisesRegex(ValueError, "不连通"):
            assert_boxes_are_connected(_probe(boxes))

    def test_a_single_box_is_exempt(self) -> None:
        assert_boxes_are_connected(_probe([Box("solo", "m", (0.0, 0.2, 0.0),
                                               (0.05, 0.2, 0.04))]))

    def test_the_threshold_is_the_documented_one(self) -> None:
        """阈值写死在断言里没意义——这里钉的是"常量存在且是那个量级"，
        改动它必须是自觉的。"""
        self.assertAlmostEqual(0.08, MIN_CONTACT_RATIO, places=6)
        self.assertAlmostEqual(0.004, GAP_TOLERANCE, places=6)


class HandDisplayTest(unittest.TestCase):
    """`hand_display` / `centre_translation` 的数学锁。

    这两个原先是 `gen_knife_trio` 的私有函数，抽进公共层供木棍复用。抽的过程逐字节
    比对过三把刀的全部产物（贴图 / OBJ / MTL / model JSON / bbmodel）——但比对只能
    证明"这一次没变"，锁不住语义，所以这里补上。
    """

    @staticmethod
    def _mc_transform(display: dict, point):
        """照 MC 的 `p = t + R·S·(v − 8)` 复算，v 是模型 px 坐标。

        R 是 JOML `rotationXYZ` = Rx·Ry·Rz（作用到向量上先 Z 再 Y 再 X）。
        """
        rx, ry, rz = (math.radians(v) for v in display["rotation"])
        scale = display["scale"][0]
        v = [(point[i] - 8.0) * scale for i in range(3)]
        v = [v[0] * math.cos(rz) - v[1] * math.sin(rz),
             v[0] * math.sin(rz) + v[1] * math.cos(rz), v[2]]
        v = [v[0] * math.cos(ry) + v[2] * math.sin(ry), v[1],
             -v[0] * math.sin(ry) + v[2] * math.cos(ry)]
        v = [v[0], v[1] * math.cos(rx) - v[2] * math.sin(rx),
             v[1] * math.sin(rx) + v[2] * math.cos(rx)]
        return [display["translation"][i] + v[i] for i in range(3)]

    def test_centred_slots_really_put_the_model_centre_on_target(self) -> None:
        """GUI / ground / fixed / head 要的是**整件居中**而不是握把居中。
        少了这一步反解，图标会被握把顶得偏出格子。"""
        display = hand_display(club.SCALE, club.GRIP, LENGTH)
        centre_px = 8.0 + (LENGTH / 2.0 - club.GRIP) * 16.0     # 出料系里的几何中心
        for slot, target in (("gui", (0.0, 0.0, 0.0)),
                             ("fixed", (0.0, 0.0, 0.0)),
                             ("ground", (0.0, 2.0, 0.0)),
                             ("head", (0.0, 12.0, 0.0))):
            got = self._mc_transform(display[slot], (8.0, centre_px, 8.0))
            for axis in range(3):
                self.assertAlmostEqual(
                    target[axis], got[axis], places=2,
                    msg=f"{slot} 槽的几何中心落在 {got}，应当是 {target}")

    def test_the_grip_point_is_the_pivot_in_hand(self) -> None:
        """第三人称两档不做居中反解：枢轴就是握把本身（`emit_offset` 已把它放到方块
        中心），调手持姿态时绕握把转，正是想要的语义。"""
        display = hand_display(club.SCALE, club.GRIP, LENGTH)
        for slot in ("thirdperson_righthand", "firstperson_righthand"):
            got = self._mc_transform(display[slot], (8.0, 8.0, 8.0))
            self.assertEqual([round(v, 6) for v in got],
                             [round(v, 6) for v in display[slot]["translation"]],
                             f"{slot}: 握把点没落在 translation 上")

    def test_the_left_hand_is_the_mirror_of_the_right(self) -> None:
        """左手那组要**预取反** y/z 旋转：`Transformation.apply(leftHanded)` 自己还会
        再取反一次并翻 x 平移，两次抵消后左右手才是镜像而不是同姿。"""
        display = hand_display(club.SCALE, club.GRIP, LENGTH)
        for hand in ("thirdperson", "firstperson"):
            right = display[f"{hand}_righthand"]["rotation"]
            left = display[f"{hand}_lefthand"]["rotation"]
            self.assertEqual(right[0], left[0], "x 旋转不该翻")
            self.assertEqual(-right[1], left[1])
            self.assertEqual(-right[2], left[2])

    def test_no_negative_zero_leaks_into_the_json(self) -> None:
        """`-0.0` 会原样写进 model JSON，diff 里看不出是"镜像算出来的零"还是手滑。"""
        display = hand_display(club.SCALE, club.GRIP, LENGTH)
        for slot, spec in display.items():
            for value in spec["rotation"]:
                self.assertFalse(
                    isinstance(value, float) and value == 0.0
                    and math.copysign(1.0, value) < 0,
                    f"{slot} 的 rotation 里混进了 -0.0")

    def test_every_display_slot_is_declared(self) -> None:
        required = {"thirdperson_righthand", "thirdperson_lefthand",
                    "firstperson_righthand", "firstperson_lefthand",
                    "ground", "gui", "fixed", "head"}
        self.assertTrue(required <= set(ITEM.display),
                        f"缺 display 槽 {required - set(ITEM.display)}")

    def test_gui_spin_is_a_parameter_not_a_constant(self) -> None:
        """细长件走对角线才占满 GUI 格子，粗短件转 45° 反而露出四角空白——
        所以这是个参数。抽公共层时把它写死过，等于把小刀的取舍强加给所有件。"""
        spun = hand_display(0.8, 0.15, 0.7, gui_spin=45)
        flat = hand_display(0.8, 0.15, 0.7, gui_spin=0)
        self.assertEqual([0, 0, 45], spun["gui"]["rotation"])
        self.assertEqual([0, 0, 0], flat["gui"]["rotation"])
        self.assertNotEqual(spun["gui"]["translation"], flat["gui"]["translation"])

    def test_centre_translation_is_pure_geometry(self) -> None:
        """零偏移、零旋转时，几何中心就在 `centre_px` 上，反解应当正好抵消它。"""
        self.assertEqual([0.0, -4.0, 0.0],
                         centre_translation((0, 0, 0), 1.0, 4.0))
        self.assertEqual([0.0, -2.0, 0.0],
                         centre_translation((0, 0, 0), 0.5, 4.0))


if __name__ == "__main__":
    unittest.main()
