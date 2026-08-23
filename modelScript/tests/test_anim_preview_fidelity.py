#!/usr/bin/env python3
"""动画/模型预览链路的保真度回归锁。

这四条锁的都是**"预览工具在骗人"**类的 bug——渲出来的图看着没毛病，但它和进游戏
后的实际表现不是一回事。这类 bug 特别贵：你会照着假图去改本来正确的资产。

1. **贴图路径引用**：`load_bbmodel` 曾假设贴图一定是内嵌 data URI，遇到磁盘路径就
   把路径字符串当 base64 解，报 `binascii.Error: Incorrect padding`——一个和"贴图
   找不到"毫无关系的错。仓库 55 个 bbmodel 里 11 个（全是多状态实体）因此完全渲
   不出来，等于对模型审查隐身。
2. **多贴图 = 整体皮肤，不是元素分组**：face 上那个 `texture` 索引不能拿来筛元素。
   ForgeStation 的底座/砧/锤被写成 0,1,0,1，按它过滤会把一个铁匠台拆成碎块。
3. **bend/axis 的单位**：emote 头 `degrees:false`，存的就是弧度。预览里曾多套一层
   `np.radians`，把角度缩掉约 57 倍——肘和膝在预览里等于焊死。后果是"肘全程不
   伸直"这类设计判据完全失效，而图上看起来只是"姿势有点僵"。
4. **easing 被丢弃**：曾是纯线性插值。整 tick 采样看不出来（所有缓动都满足
   f(0)=0、f(1)=1），一旦按子 tick 出 GIF，节奏就全平——而节奏正是 easing 唯一
   负责的东西。

每条都配变异用例：把修复点改回坏行为，断言测试真的会红。
"""

from __future__ import annotations

import base64
import io
import json
import math
import sys
import unittest
from pathlib import Path

import numpy as np
from PIL import Image

LIB_DIR = Path(__file__).resolve().parents[1]
REPO = LIB_DIR.parent
for _d in (LIB_DIR / "core", LIB_DIR / "tools", REPO / "client" / "tools"):
    sys.path.insert(0, str(_d))

import render_animation as RA  # noqa: E402
import render_bbmodel as RB  # noqa: E402

MODELS = LIB_DIR / "models"
ANIM = REPO / "client" / "src" / "main" / "resources" / "assets" / "bong" / "player_animation"


def _all_models():
    return sorted(MODELS.glob("*.bbmodel"))


class TextureSourceTest(unittest.TestCase):
    """贴图源的两种形态都要能加载。"""

    def test_every_bbmodel_in_repo_renders(self):
        """全仓库 bbmodel 逐个渲染。这是本文件最重要的一条——它是发现问题 #1 的
        那次扫描的固化版本。新模型进来若带坏贴图，这里立刻红。"""
        failed = []
        for p in _all_models():
            for i in range(len(RB.texture_names(p))):
                try:
                    RB.render(str(p), size=32, texture=i)
                except Exception as exc:  # noqa: BLE001 —— 就是要抓全部
                    failed.append(f"{p.stem}[tex{i}]: {type(exc).__name__}: {exc}")
        self.assertEqual(
            [], failed,
            "以下 bbmodel 渲染失败。若报 base64/padding 类错误，多半是贴图用了磁盘"
            "路径引用而加载器只认内嵌 data URI：\n  " + "\n  ".join(failed))

    def test_path_referenced_texture_loads_from_disk(self):
        """至少存在若干路径引用型贴图，且它们指向的文件真在磁盘上。

        断言"数量 > 0"是为了防止有人把这批模型改成内嵌后，上面那条全量渲染测试
        变成对路径分支的**空覆盖**——分支没被走到，回归就锁不住。
        """
        path_refs = []
        for p in _all_models():
            for t in json.loads(p.read_text()).get("textures", []):
                src = t.get("source", "")
                if not src.startswith("data:"):
                    path_refs.append((p.stem, src))
        self.assertGreater(
            len(path_refs), 0,
            "仓库里已经没有路径引用型贴图了——本文件的路径加载分支失去覆盖，"
            "要么补一个 fixture，要么删掉这条锁")
        missing = [(n, s) for n, s in path_refs if not (REPO / s).is_file()]
        self.assertEqual([], missing, f"这些贴图路径在磁盘上不存在: {missing}")

    def test_data_uri_and_path_sources_agree(self):
        """同一张图，内嵌与路径两种写法必须加载出一样的像素。"""
        src_png = REPO / ("client/src/main/resources/assets/bong/textures/entity/"
                          "forge_station_idle.png")
        raw = src_png.read_bytes()
        by_path = RB._load_texture({"name": "x", "source": str(src_png.relative_to(REPO))},
                                   MODELS / "ForgeStation.bbmodel")
        by_uri = RB._load_texture(
            {"name": "x", "source": "data:image/png;base64," + base64.b64encode(raw).decode()},
            MODELS / "ForgeStation.bbmodel")
        np.testing.assert_array_equal(by_path, by_uri)

    def test_missing_texture_path_raises_a_useful_error(self):
        """变异：路径指向不存在的文件，必须报 FileNotFoundError 且带上路径。

        原来的行为是 base64 解码报 padding 错——那种信息量为零的错误正是这次
        排查费掉最多时间的地方。
        """
        with self.assertRaises(FileNotFoundError) as cm:
            RB._load_texture({"name": "ghost", "source": "no/such/dir/ghost.png"},
                             MODELS / "ForgeStation.bbmodel")
        self.assertIn("ghost.png", str(cm.exception))

    def test_empty_source_raises(self):
        with self.assertRaises(ValueError):
            RB._load_texture({"name": "empty", "source": ""}, MODELS / "ForgeStation.bbmodel")


class TextureSelectionTest(unittest.TestCase):
    """多贴图模型：选皮肤，不是筛元素。"""

    MULTI = "ForgeStation"

    def test_texture_names_lists_all(self):
        self.assertEqual(["idle", "working"], RB.texture_names(MODELS / f"{self.MULTI}.bbmodel"))

    def test_selecting_by_index_and_by_name_agree(self):
        a, _ = RB.render(str(MODELS / f"{self.MULTI}.bbmodel"), size=48, texture=1)
        b, _ = RB.render(str(MODELS / f"{self.MULTI}.bbmodel"), size=48, texture="working")
        np.testing.assert_array_equal(np.asarray(a), np.asarray(b))

    def test_different_textures_give_different_pixels(self):
        a, _ = RB.render(str(MODELS / f"{self.MULTI}.bbmodel"), size=48, texture="idle")
        b, _ = RB.render(str(MODELS / f"{self.MULTI}.bbmodel"), size=48, texture="working")
        self.assertFalse(np.array_equal(np.asarray(a), np.asarray(b)),
                         "idle 与 working 渲染结果完全一致——贴图选择没生效")

    def test_geometry_is_identical_across_textures(self):
        """核心断言：换皮肤不能换几何。

        曾按 face 的 texture 索引筛元素，结果 idle 只剩底座+砧+锤的一半、working
        剩另一半——一个铁匠台被劈成两堆碎块。这条锁死"贴图是整体皮肤"这个语义。
        """
        base = None
        for i in range(len(RB.texture_names(MODELS / f"{self.MULTI}.bbmodel"))):
            tris, _, _, _ = RB.load_bbmodel(MODELS / f"{self.MULTI}.bbmodel", texture=i)
            verts = np.array([p for vs, _, _ in tris for p in vs])
            if base is None:
                base = verts
            else:
                np.testing.assert_array_equal(
                    base, verts,
                    f"贴图 {i} 的几何和贴图 0 不同——说明又按贴图索引筛元素了")

    def test_multi_texture_models_keep_all_elements(self):
        """所有多贴图模型的三角面数必须与单贴图加载一致（即：一个都没被筛掉）。"""
        for p in _all_models():
            names = RB.texture_names(p)
            if len(names) < 2:
                continue
            counts = {i: len(RB.load_bbmodel(p, texture=i)[0]) for i in range(len(names))}
            self.assertEqual(
                1, len(set(counts.values())),
                f"{p.stem} 各贴图下的三角面数不一致 {counts} —— 元素被按贴图筛了")

    def test_bad_texture_index_and_name_raise(self):
        m = MODELS / f"{self.MULTI}.bbmodel"
        with self.assertRaises(IndexError):
            RB.load_bbmodel(m, texture=99)
        with self.assertRaises(KeyError):
            RB.load_bbmodel(m, texture="no_such_state")

    def test_default_texture_is_index_zero(self):
        a, _ = RB.render(str(MODELS / f"{self.MULTI}.bbmodel"), size=48)
        b, _ = RB.render(str(MODELS / f"{self.MULTI}.bbmodel"), size=48, texture=0)
        np.testing.assert_array_equal(np.asarray(a), np.asarray(b))


class EasingTest(unittest.TestCase):
    """easing 曲线本身。"""

    NAMES = ("LINEAR", "linear", "INSINE", "OUTSINE", "INOUTSINE",
             "INQUAD", "OUTQUAD", "INCUBIC", "OUTCUBIC", "INOUTCUBIC",
             "INQUART", "INQUINT", "INEXPO", "INCIRC")

    def test_endpoints_are_exact(self):
        """f(0)=0、f(1)=1 必须**精确**。

        不是洁癖：整 tick 上的取值靠这个性质保持不变，既有的一大批整 tick 预览
        才不会因为加了 easing 而集体位移。
        """
        for n in self.NAMES:
            self.assertEqual(0.0, RA.apply_easing(n, 0.0), f"{n} 在 0 处不为 0")
            self.assertEqual(1.0, RA.apply_easing(n, 1.0), f"{n} 在 1 处不为 1")

    def test_all_are_monotone_nondecreasing(self):
        for n in self.NAMES:
            vals = [RA.apply_easing(n, i / 64) for i in range(65)]
            for a, b in zip(vals, vals[1:]):
                self.assertLessEqual(a, b + 1e-12, f"{n} 非单调——插值会出现倒退")

    def test_in_family_starts_slow_out_family_starts_fast(self):
        """形状判据。名字对了但曲线接反的话，发力段会变成减速段。"""
        for fam in ("SINE", "QUAD", "CUBIC"):
            self.assertLess(RA.apply_easing(f"IN{fam}", 0.25), 0.25,
                            f"IN{fam} 前段应当慢于线性")
            self.assertGreater(RA.apply_easing(f"OUT{fam}", 0.25), 0.25,
                               f"OUT{fam} 前段应当快于线性")
            self.assertAlmostEqual(0.5, RA.apply_easing(f"INOUT{fam}", 0.5), places=9,
                                   msg=f"INOUT{fam} 中点应当在 0.5")

    def test_unknown_name_falls_back_to_linear(self):
        for a in (0.0, 0.25, 0.5, 1.0):
            self.assertAlmostEqual(a, RA.apply_easing("NOSUCHEASING", a))

    def test_alpha_is_clamped(self):
        self.assertEqual(0.0, RA.apply_easing("INQUAD", -3.0))
        self.assertEqual(1.0, RA.apply_easing("OUTQUAD", 7.0))

    def test_sample_axis_uses_the_start_frame_easing(self):
        """§15：某帧的 easing 管「本帧 → 下一帧」，取的是 before.ease。

        构造一段 [0→1]，起始帧写 INQUAD、结束帧写 OUTQUAD。中点若取 0.25 说明用了
        起始帧（正确）；取 0.75 说明用了结束帧（方向反了）。
        """
        kfs = {"rightArm": {"pitch": [(0, 0.0, "INQUAD"), (4, 1.0, "OUTQUAD")]}}
        self.assertAlmostEqual(0.25, RA.sample_axis(kfs, "rightArm", "pitch", 2.0), places=9,
                               msg="easing 取错了帧——用成了结束帧的曲线")

    def test_sample_axis_is_exact_at_keyframes(self):
        kfs = {"rightArm": {"pitch": [(0, 0.0, "INCUBIC"), (4, 1.0, "INOUTSINE")]}}
        self.assertEqual(0.0, RA.sample_axis(kfs, "rightArm", "pitch", 0.0))
        self.assertEqual(1.0, RA.sample_axis(kfs, "rightArm", "pitch", 4.0))

    def test_sample_axis_clamps_outside_range(self):
        kfs = {"rightArm": {"pitch": [(2, 5.0, "LINEAR"), (4, 9.0, "LINEAR")]}}
        self.assertEqual(5.0, RA.sample_axis(kfs, "rightArm", "pitch", 0.0))
        self.assertEqual(9.0, RA.sample_axis(kfs, "rightArm", "pitch", 99.0))


class BendUnitTest(unittest.TestCase):
    """bend/axis 是弧度，不能再过一次 np.radians。"""

    FOREARM = 6.0  # bend_center 到手心的局部距离

    def _hand(self, bend_deg, axis_deg=180.0):
        import preview_player_anim as P
        kfs = {"rightArm": {"bend": [(0, math.radians(bend_deg), "LINEAR")],
                            "axis": [(0, math.radians(axis_deg), "LINEAR")]}}
        seg = P.segment_transforms(kfs, 0.0)
        rest = P._pt(np.array(P.PIVOT_OF["rightArm_lo"], float) + RA.limb_end_local("rightArm"))
        return (seg["rightArm_lo"] @ np.append(rest, 1.0))[:3]

    def test_bend_swings_the_hand_by_the_geometric_amount(self):
        """弯肘 θ 后手心位移必须等于 2·L·sin(θ/2)。

        这是纯几何，没有调参余地——单位错了就直接对不上。曾经的双重 radians 让
        120° 只挪动 0.2（应为 10.39），而三视图上只表现为"姿势有点僵"。
        """
        for deg in (30.0, 60.0, 90.0, 120.0):
            moved = float(np.linalg.norm(self._hand(deg) - self._hand(0.0)))
            expect = 2 * self.FOREARM * math.sin(math.radians(deg) / 2)
            self.assertAlmostEqual(
                expect, moved, places=6,
                msg=f"bend={deg}° 手心应移动 {expect:.3f}，实际 {moved:.3f}。"
                    f"差出约 57 倍就是 bend 被当成角度又转了一次弧度")

    def test_zero_bend_is_the_rest_pose(self):
        rest_like = self._hand(0.0)
        import preview_player_anim as P
        expect = P._pt(np.array(P.PIVOT_OF["rightArm_lo"], float)
                       + RA.limb_end_local("rightArm"))
        np.testing.assert_allclose(expect, rest_like, atol=1e-9)

    def test_bend_axis_changes_the_swing_plane(self):
        """axis 同样是弧度。两个不同 axis 下的手心位置必须不同。"""
        a = self._hand(90.0, axis_deg=180.0)
        b = self._hand(90.0, axis_deg=90.0)
        self.assertGreater(float(np.linalg.norm(a - b)), 1.0,
                           "换 bend 轴后手心几乎没变——axis 多半也被双重转换了")


class DaggerAnimationTest(unittest.TestCase):
    """两条匕首动画的设计意图锁。"""

    ANIMS = ("dagger_slash", "dagger_stab")

    def _kfs(self, name):
        e = json.loads((ANIM / f"{name}.json").read_text(encoding="utf-8"))
        return RA.collect_keyframes(e.get("emote", e))

    def _moves(self, name):
        e = json.loads((ANIM / f"{name}.json").read_text(encoding="utf-8"))
        return (e.get("emote", e))["moves"]

    def test_strike_segment_carries_an_accelerating_easing(self):
        """发力段 t3→t5 的 easing 写在 t3 上，且必须是 IN 族。

        §15.2 的坑：直觉会把 OUTQUAD 写在撞击帧 t5 上，以为那是"到撞击时减速"，
        实际它管的是撞击**之后**。后果可量：峰速落在 t6 的收招段而不是撞击。
        conventions 里的 `assertAxisDense` 只查"显式且非 linear"，INOUTSINE 照样
        放行——所以这条得单独锁。
        """
        for name in self.ANIMS:
            eases = {m["tick"]: m.get("easing") for m in self._moves(name)}
            self.assertTrue(
                str(eases.get(3, "")).startswith("IN")
                and not str(eases.get(3, "")).startswith("INOUT"),
                f"{name} 的 t3（发力段起始帧）easing 是 {eases.get(3)!r}，"
                f"应为 IN 族（INQUAD/INCUBIC…）才能从静止加速到撞击")

    def test_peak_speed_lands_on_the_impact_tick(self):
        """刀尖峰速必须出现在撞击帧 t5 之前的最后一格，不能落在收招段。"""
        import preview_player_anim as P
        knife = RB.load_bbmodel(MODELS / "StoneKnife.bbmodel")[0]
        V = np.array([p for vs, _, _ in knife for p in vs])
        tip = V[int(V[:, 1].argmax())]
        disp = {"rotation": [0, -90, 55], "translation": [0, 4.0, 0], "scale": [0.72] * 3}
        for name in self.ANIMS:
            kfs = self._kfs(name)
            pos = []
            for i in range(33):
                m = P.hand_transform(P.segment_transforms(kfs, 8.0 * i / 32), disp)
                pos.append(m[:3, :3] @ tip + m[:3, 3])
            speed = np.linalg.norm(np.diff(np.array(pos), axis=0), axis=1)
            peak_tick = 8.0 * int(speed.argmax()) / 32
            self.assertTrue(
                3.0 < peak_tick <= 5.0,
                f"{name} 峰速在 t{peak_tick:.1f}，应落在发力段 (3, 5]。"
                f"落在 t6 附近说明 easing 写在了错误的帧上（§15.2）")

    def test_elbow_never_straightens(self):
        """匕首和剑的核心区别：肘全程不伸直。

        剑刺 impact 时 bend=3（整条手臂打直够远），匕首够不着，伸直只是把手腕送
        到对方面前。改小了这条就退化成"短剑"。
        """
        for name in self.ANIMS:
            bends = [math.degrees(v) for _, v, _ in self._kfs(name)["rightArm"]["bend"]]
            self.assertGreaterEqual(
                min(bends), 15.0,
                f"{name} 右肘最小 bend={min(bends):.0f}°，匕首不该把手臂打直")

    def test_returns_exactly_to_the_guard_pose(self):
        """末帧必须与首帧逐轴一致，否则连击时会跳一下。"""
        for name in self.ANIMS:
            kfs = self._kfs(name)
            for part, axes in kfs.items():
                for axis, lst in axes.items():
                    first = RA.sample_axis(kfs, part, axis, 0.0)
                    last = RA.sample_axis(kfs, part, axis, 8.0)
                    self.assertAlmostEqual(
                        first, last, places=6,
                        msg=f"{name} 的 {part}.{axis} 首帧 {first:.4f} ≠ 末帧 {last:.4f}")

    def test_both_daggers_are_distinguishable_from_each_other(self):
        """横划和直刺必须是两个能分辨的动作，不能只是数值微调。"""
        a, b = (self._kfs(n) for n in self.ANIMS)
        diffs = []
        for part in ("rightArm", "leftArm", "torso", "body"):
            for axis in ("pitch", "yaw", "roll", "bend", "z"):
                if axis not in a.get(part, {}) or axis not in b.get(part, {}):
                    continue
                for t in (0.0, 3.0, 5.0):
                    diffs.append(abs(RA.sample_axis(a, part, axis, t)
                                     - RA.sample_axis(b, part, axis, t)))
        self.assertGreater(max(diffs), math.radians(25),
                           "两条匕首动画在所有采样点都过于接近，玩家分辨不出")


if __name__ == "__main__":
    unittest.main()
