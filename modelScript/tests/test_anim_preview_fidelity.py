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
# `gen_knife_trio` 住在 generators/ 下。少了这一条，下面六条匕首判据全部以
# `ModuleNotFoundError` 收场 —— 而 unittest 把它计成 FAIL 混在其它输出里，没人回头看，
# 于是这四条动画一路漂到「刃仰角 +48°、刀尖高出肩 10px」都没有任何一道门报过警。
for _d in (LIB_DIR / "tools", LIB_DIR / "generators", REPO / "client" / "tools"):
    sys.path.insert(0, str(_d))

import render_animation as RA  # noqa: E402
from bbmodel_maker.render import render_bbmodel as RB  # noqa: E402

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

    def test_bend_matches_the_reference_implementation_everywhere(self):
        """与 `RA.bent_end_local` 逐点对拍——本文件最硬的一条锁。

        那是驱动过一大批已上线动画的参考实现（MC ModelPart 空间，+Y 朝下）；
        预览在 Bedrock 空间（+Y 朝上）。两者必须描述同一个骨架。

        曾经**旋向是反的**：肘往身后翻、膝往身前踢，两个关节同时做反。根因是
        `_pt` 的 y 翻转是个反射，会反转旋转旋向（S·R(a,θ)·S = R(S·a, −θ)），
        而 bend 的轴 y 分量恒为 0、S_FLIP 对它不起作用，那个负号就静默丢了。
        单看渲染图很难判——人物姿势依旧"像个人"，只是别扭。
        """
        import preview_player_anim as P
        worst, worst_case = 0.0, None
        # axis 只扫本肢体**合解剖**的一侧：手臂折向身前（axis 落在 90~270），腿折向
        # 身后（axis 落在 -90~90）。非法组合已由 `assert_joint_fold_is_anatomical`
        # 在源码里拦掉，ship 不出去，没必要在这里对拍。
        for part, segname, axes in (
                ("rightArm", "rightArm_lo", (150, 180, 210)),
                ("leftArm", "leftArm_lo", (150, 180, 210)),
                ("rightLeg", "rightLeg_lo", (-30, 0, 30)),
                ("leftLeg", "leftLeg_lo", (-30, 0, 30))):
            for axis_deg in axes:
                for bend_deg in (0, 15, 45, 90, 135):
                    a = math.radians(axis_deg)
                    base = np.array(P.PIVOT_OF[segname], float)
                    ref = P._pt(base + RA.bent_end_local(part, a, math.radians(bend_deg)))
                    kfs = {part: {"bend": [(0, math.radians(bend_deg), "LINEAR")],
                                  "axis": [(0, a, "LINEAR")]}}
                    rest = P._pt(base + RA.limb_end_local(part))
                    got = (P.segment_transforms(kfs, 0.0)[segname] @ np.append(rest, 1.0))[:3]
                    dist = float(np.linalg.norm(ref - got))
                    if dist > worst:
                        worst, worst_case = dist, (part, axis_deg, bend_deg, ref, got)
        self.assertLess(
            worst, 1e-6,
            f"预览的弯折与参考实现不符，最差 {worst:.3f} 出现在 {worst_case}。"
            f"若差值恰好是 z 分量整体变号，就是旋向反了（见本用例 docstring）")

    def test_elbow_folds_toward_the_front_not_the_back(self):
        """方向判据，独立于上面的对拍：人的肘往身前折，不往身后。

        MC 空间 +Z 是身后，`_pt` 不改 z，所以 Bedrock 里 −z 仍是身前。
        """
        import preview_player_anim as P
        base = np.array(P.PIVOT_OF["rightArm_lo"], float)
        rest = P._pt(base + RA.limb_end_local("rightArm"))
        kfs = {"rightArm": {"bend": [(0, math.radians(90), "LINEAR")],
                            "axis": [(0, math.pi, "LINEAR")]}}
        hand = (P.segment_transforms(kfs, 0.0)["rightArm_lo"] @ np.append(rest, 1.0))[:3]
        self.assertLess(
            hand[2], rest[2] - 1.0,
            f"弯肘 90° 后手心 z={hand[2]:.2f}，静止 z={rest[2]:.2f}——"
            f"手往身后（+z）去了，肘做反了")

    def test_knee_folds_toward_the_back_not_the_front(self):
        """膝与肘旋向相反：小腿往身后折（脚跟朝臀），不往身前踢。

        同一条 bend 轴约定下，腿的 `limb_end_local` 指向下方，所以同样的旋转在腿
        上表现为脚往后。这条和上一条一起，把「两个关节同时做反」那种错挡住——
        只测一个关节的话，整体取负仍可能让另一个碰巧看着对。
        """
        import preview_player_anim as P
        base = np.array(P.PIVOT_OF["rightLeg_lo"], float)
        rest = P._pt(base + RA.limb_end_local("rightLeg"))
        kfs = {"rightLeg": {"bend": [(0, math.radians(90), "LINEAR")],
                            "axis": [(0, 0.0, "LINEAR")]}}
        foot = (P.segment_transforms(kfs, 0.0)["rightLeg_lo"] @ np.append(rest, 1.0))[:3]
        self.assertGreater(
            foot[2], rest[2] + 1.0,
            f"弯膝 90° 后脚 z={foot[2]:.2f}，静止 z={rest[2]:.2f}——"
            f"脚往身前（−z）踢了，膝做反了")
        ref = P._pt(base + RA.bent_end_local("rightLeg", 0.0, math.radians(90)))
        np.testing.assert_allclose(ref, foot, atol=1e-6,
                                   err_msg="膝的弯折与参考实现不符")

    def test_bend_axis_changes_the_swing_plane(self):
        """axis 同样是弧度。两个不同 axis 下的手心位置必须不同。"""
        a = self._hand(90.0, axis_deg=180.0)
        b = self._hand(90.0, axis_deg=140.0)   # 同为"折向身前"，仍属合解剖
        self.assertGreater(float(np.linalg.norm(a - b)), 1.0,
                           "换 bend 轴后手心几乎没变——axis 多半也被双重转换了")


class DaggerSuiteDelegationTest(unittest.TestCase):
    """四条匕首动画的判据**只有一处定义**：`modelScript/tools/knife_anim_gates.py`。

    这里原本抄了一份（刃仰角、刀尖高度、峰速落点、肘不打直、收势闭合、可分辨性）。
    两份判据各挂各的手持物 —— 这边量的是 `gen_knife_trio` 的石刃（display
    `[-80,90,0]`，刃基本沿前臂），门那边量的是 `IronDagger.bbmodel`（display
    `[0,-90,55]`，本仓 `tripo_to_sml.HANDHELD_DISPLAY` 的默认握法，刃相对前臂偏前
    35°）。同一套姿态在两种握法下的刃仰角天然差三十多度，于是「撞击帧刃要水平」这条
    在两边永远不可能同时成立 —— 一份必然假绿或假红，而看的人以为它们在说同一件事。

    所以这里只保留**委托**：门跑不过就红在这。判据、门限和它们的标定理由都在门模块里。
    """

    def _gates(self):
        import knife_anim_gates as KG
        return KG

    def test_every_dagger_animation_passes_its_gates(self):
        KG = self._gates()
        bad = {}
        for name in sorted(KG.SUITE):
            fails = [g.label for g in KG.build(name).run_all() if not g.ok]
            if fails:
                bad[name] = fails
        self.assertFalse(
            bad, f"匕首动画没过门：{bad} —— 跑 "
                 f"`python3 modelScript/tools/knife_anim_gates.py` 看逐条明细")

    def test_the_gates_themselves_still_discriminate(self):
        """门自己得有鉴别力：注入对应缺陷后必须报。"""
        KG = self._gates()
        broken = {name: KG.build(name).self_test(verbose=False)
                  for name in sorted(KG.SUITE)}
        broken = {k: v for k, v in broken.items() if v}
        self.assertFalse(
            broken, f"这些动画上有门失效（干净就红，或注入缺陷后仍绿）：{broken}")


class JointAnatomyGuardTest(unittest.TestCase):
    """关节折向的源码硬拦（`anim_common.assert_joint_fold_is_anatomical`）。

    肘只能往身前折、膝只能往身后折。折反了渲染出来往往只让人觉得"姿势别扭"，
    很难一眼断定是 bug——所以拦在源码里，授权侧（生成器写错）和渲染侧（变换算错）
    各拦一道。
    """

    def setUp(self):
        sys.path.insert(0, str(REPO / "client" / "tools"))
        import anim_common
        self.AC = anim_common

    def test_correct_directions_pass(self):
        """本仓约定：手臂 axis=180、腿 axis=0。全部放行。"""
        for part in ("leftArm", "rightArm"):
            self.AC.assert_joint_fold_is_anatomical(part, 100.0, 180.0)
        for part in ("leftLeg", "rightLeg"):
            self.AC.assert_joint_fold_is_anatomical(part, 100.0, 0.0)

    def test_inverted_elbow_raises(self):
        for part in ("leftArm", "rightArm"):
            with self.assertRaises(ValueError) as cm:
                self.AC.assert_joint_fold_is_anatomical(part, 100.0, 0.0)
            self.assertIn("肘", str(cm.exception))

    def test_inverted_knee_raises(self):
        for part in ("leftLeg", "rightLeg"):
            with self.assertRaises(ValueError) as cm:
                self.AC.assert_joint_fold_is_anatomical(part, 105.0, 180.0)
            self.assertIn("膝", str(cm.exception))

    def test_negative_bend_is_handled_by_the_same_rule(self):
        """负 bend 等价于反向折——判据看 sin(bend)·cos(axis) 的符号，自动覆盖。"""
        with self.assertRaises(ValueError):
            self.AC.assert_joint_fold_is_anatomical("rightArm", -100.0, 180.0)
        self.AC.assert_joint_fold_is_anatomical("rightArm", -100.0, 0.0)

    def test_straight_limb_is_exempt_regardless_of_axis(self):
        """bend≈0 时肢体是直的，axis 纯属声明，不该拦。

        仓库里确实有一批 bend=0 却写了 axis 的帧，拦它们全是假阳性。
        """
        for axis in (0.0, 90.0, 180.0, 270.0):
            self.AC.assert_joint_fold_is_anatomical("rightArm", 0.0, axis)
            self.AC.assert_joint_fold_is_anatomical("rightLeg", 0.5, axis)

    def test_non_bendable_parts_are_exempt(self):
        for part in ("head", "torso", "body"):
            self.AC.assert_joint_fold_is_anatomical(part, 100.0, 0.0)

    def test_error_message_names_the_fix(self):
        with self.assertRaises(ValueError) as cm:
            self.AC.assert_joint_fold_is_anatomical("rightLeg", 105.0, 180.0, where="tick 7")
        msg = str(cm.exception)
        self.assertIn("tick 7", msg)
        self.assertIn("axis", msg)
        self.assertIn("180", msg)

    def test_pose_table_validation_rejects_an_inverted_knee(self):
        """授权侧：生成器写出反关节，`build_doc` 就该拒绝，别让它落成 JSON。"""
        bad = {0: {"easing": "LINEAR", "rightLeg": {"pitch": 40, "bend": 105, "axis": 180}}}
        with self.assertRaises(ValueError):
            self.AC._validate_pose_table(bad)
        good = {0: {"easing": "LINEAR", "rightLeg": {"pitch": 40, "bend": 105, "axis": 0}}}
        self.AC._validate_pose_table(good)

    def test_every_shipped_animation_has_anatomical_joints(self):
        """全仓库已落盘的动画逐帧过判据。

        这条第一次跑就抓出 6 条既存违规（5 条 jian_* 反肘 + sword_ride 反膝）。
        """
        bad = []
        for f in sorted(ANIM.glob("*.json")):
            e = json.loads(f.read_text(encoding="utf-8"))
            e = e.get("emote", e)
            # axis 必须**按插值取**，不能"该 tick 没有 axis 关键帧就当 0"——
            # axis 常只在首末帧打点，中段是插出来的。按 0 兜底会把一批正常动画
            # 误判成反关节（第一版就这么误报了 3 条 baomai_*）。
            kfs = RA.collect_keyframes(e)
            for part, axes in kfs.items():
                if "bend" not in axes:
                    continue
                for tick, bend_rad, _ in axes["bend"]:
                    axis_rad = RA.sample_axis(kfs, part, "axis", float(tick))
                    try:
                        self.AC.assert_joint_fold_is_anatomical(
                            part, math.degrees(bend_rad), math.degrees(axis_rad),
                            where=f"{f.stem} tick {tick}")
                    except ValueError as exc:
                        bad.append(str(exc).splitlines()[0])
        names = {b.split(" tick")[0] for b in bad}
        # 历史欠账，**只准变少不准变多**。这 5 条剑法的架势（右臂高举 pitch=-170 +
        # 肘深弯 bend=100 + axis=0）是靠反折肘换来的观感：锏沿小臂走，作者要"锏尖朝
        # 前下、两尖在身前汇聚"，而合解剖的肘在手臂高举时只会把小臂折到脑后。
        #
        # 平心而论这条比膝盖那条软：**肩有沿上臂长轴的内外旋**（人约 180°），
        # MC 的 pitch/yaw/roll 没把它和肘的折向分开，作者其实是拿 axis 在表达肩内旋。
        # 膝没有这个自由度，所以膝那条是绝对的。留着这条是因为仓库约定明确
        # （手臂 axis=180 用了 1634 次、axis=0 只有这 29 次），偏离约定值得拦。
        #
        # 要清掉得重设计架势（或给上臂长轴旋转单独建模），是视觉决策，不在本次范围。
        # 另有 3 条 baomai_* 是**漏写 axis**：手臂设了 bend 却没设 axis，落到默认 0
        # （对手臂即反折）。它们没有生成器，JSON 是手工件，改只能直接动 JSON，
        # 且 bend 到 49° 改了看得出来——同样交人工定夺。
        LEGACY = {"jian_draw_waist", "jian_dual_smash", "jian_dual_sweep",
                  "jian_stance_high_low", "jian_waist_spin_cross",
                  "baomai_blood_burn", "baomai_disperse", "baomai_mountain_shake"}
        new_bad = sorted(n for n in names if n not in LEGACY)
        self.assertEqual(
            [], new_bad,
            "以下动画含反关节（不在历史欠账名单里，说明是新引入的）：\n  "
            + "\n  ".join(sorted(set(b for b in bad
                                      if b.split(" tick")[0] in new_bad))))
        self.assertLessEqual(
            names, LEGACY,
            "历史欠账名单该只减不增")
        self.assertEqual(
            LEGACY, names,
            f"历史欠账里有 {sorted(LEGACY - names)} 已经修好了——"
            f"请把它们从 LEGACY 名单里删掉，好让这条锁继续收紧")


class HeldItemAttachTest(unittest.TestCase):
    """手持物挂点：`preview_player_anim.item_attach_modelpart` 必须复刻运行时调用序。

    这条链历史上错了**四处**，合起来让刀飘在拳头外 6.3px（一个拳头才 4px 宽）——
    正是用户一眼看出的"手没握住把柄"。四处分别是：

    ① 整条 `R_ATTACH`（`HeldItemFeatureRenderer` 的 Rx(-90)·Ry(180)）根本没有；
    ② display 的 translation 被当成 `R_disp·t` 加，而 MC 是**先平移再旋转**；
    ③ 挂点用 `limb_end_local` 近似，真值是 `R_ATTACH·(1,2,-10)`；
    ④ 少了方块中心重定心 `T(-8,-8,-8)`。

    下面按**可观察量**分别钉死，不去比对内部矩阵——换实现不该让这些红。
    """

    IDENT = {"rotation": [0, 0, 0], "translation": [0, 0, 0], "scale": [1, 1, 1]}

    def setUp(self):
        import preview_player_anim as P
        self.P = P
        self.kfs = RA.collect_keyframes(
            json.loads((ANIM / "dagger_slash.json").read_text(encoding="utf-8"))["emote"])

    def _arm(self, tick):
        """臂本身的 ModelPart 变换（肩枢轴 + R_arm + bend），不含任何手持物项。"""
        P = self.P
        part = RA.sample_part(self.kfs, "rightArm", float(tick))
        pivot = (np.array(P.PIVOT_OF["rightArm_lo"], float)
                 + np.array([part["x"], part["y"], part["z"]], float))
        R = RA.part_rotation_matrix(part["pitch"], part["yaw"], part["roll"])
        a = float(part["axis"])
        Rb = RA.rotate_about_axis(np.array([np.cos(-a), 0.0, np.sin(-a)]), float(part["bend"]))
        return (P._aff(np.eye(3), pivot) @ P._aff(R, np.zeros(3))
                @ P._about(Rb, P.ITEM_BEND_PIVOT_PX))

    def test_anchor_is_the_minecraft_hand_offset(self):
        """① + ③：display 为单位阵时，模型中心必须落在 `R_ATTACH·(1,2,-10)`。

        那是 `HeldItemFeatureRenderer.renderItem` 里
        `setArmAngle → Rx(-90) → Ry(180) → translate(1/16, 0.125, -0.625)` 的落点，
        换算到臂系是 (-1,10,-2)：臂盒底面、往前 2px。
        """
        for tick in (0.0, 2.5, 5.0, 7.0):
            M = self.P.item_attach_modelpart(self.kfs, tick, self.IDENT)
            got = (M @ np.array([8.0, 8.0, 8.0, 1.0]))[:3]
            want = (self._arm(tick) @ np.append(
                self.P.R_ATTACH @ self.P.HAND_OFFSET_PX, 1.0))[:3]
            np.testing.assert_allclose(
                got, want, atol=1e-9,
                err_msg=f"t{tick}: 模型中心没落在 MC 的手持物挂点上")

    def test_left_hand_mirrors_the_x_offset(self):
        for tick in (0.0, 5.0):
            r = self.P.item_attach_modelpart(self.kfs, tick, self.IDENT, right=True)
            l = self.P.item_attach_modelpart(self.kfs, tick, self.IDENT, right=False)
            self.assertFalse(np.allclose(r, l), "左右手挂点不该完全一样")

    def test_item_y_axis_points_forward_when_display_is_identity(self):
        """① `R_ATTACH` 在场的直接判据：item 空间的 +Y 落到臂系的 -Z（朝前）。

        少了 R_ATTACH 的话 +Y 会落到臂系 +Y（朝下），刀的朝向从根上就错。
        """
        M = self.P.item_attach_modelpart(self.kfs, 0.0, self.IDENT)
        arm = self._arm(0.0)
        got = M[:3, :3] @ np.array([0.0, 1.0, 0.0])
        want = arm[:3, :3] @ np.array([0.0, 0.0, -1.0])
        np.testing.assert_allclose(got, want, atol=1e-9)

    def test_translation_is_applied_before_the_rotation(self):
        """② MC 的 `Transformation.apply` 是 translate → rotate → scale。

        所以模型中心（= 枢轴，减 8 后为零向量）的落点**只由 translation 决定**，
        换 rotation 不该让它动。旧实现算的是 `R_disp·t`，换 rotation 就漂。
        """
        base = None
        for rot in ([0, 0, 0], [0, -90, 55], [-80, 90, 0], [37, 12, -140]):
            disp = {"rotation": rot, "translation": [3, -2, 1.5], "scale": [0.7] * 3}
            M = self.P.item_attach_modelpart(self.kfs, 3.0, disp)
            centre = (M @ np.array([8.0, 8.0, 8.0, 1.0]))[:3]
            if base is None:
                base = centre
            np.testing.assert_allclose(
                centre, base, atol=1e-9,
                err_msg=f"rotation={rot} 让枢轴挪了位——translation 被当成旋转后再加")

    def test_scale_pivots_on_the_block_centre(self):
        """④ 缩放必须绕 (8,8,8) 而不是模型原点。

        没有 `T(-8,-8,-8)` 的话原点才是不动点，整件会随 scale 往方块角坍缩——
        本仓库的手持物握把恰好就在原点，症状正是"刀离手越缩越远"。
        """
        for s in (0.4, 1.0, 1.8):
            disp = {"rotation": [0, 0, 0], "translation": [0, 0, 0], "scale": [s] * 3}
            M = self.P.item_attach_modelpart(self.kfs, 1.0, disp)
            centre = (M @ np.array([8.0, 8.0, 8.0, 1.0]))[:3]
            up = (M @ np.array([8.0, 24.0, 8.0, 1.0]))[:3]
            self.assertAlmostEqual(
                16.0 * s, float(np.linalg.norm(up - centre)), places=9,
                msg=f"scale={s}: 离枢轴 16px 的点没按比例走")
            if s == 0.4:
                ref = centre
            else:
                np.testing.assert_allclose(
                    centre, ref, atol=1e-9,
                    err_msg="缩放把枢轴本身挪了位 —— 说明少了方块中心重定心")

    def test_item_follows_the_elbow_bend(self):
        """PlayerAnimator `HeldItemMixin` 在 Rx(-90) 之前插了一段 bend 旋转，
        所以手持物**跟着肘弯走**。不跟的话弯肘时刀会留在直臂的手位上。"""
        P = self.P
        seen = set()
        for tick in (0.0, 3.0, 5.0):
            M = P.item_attach_modelpart(self.kfs, tick, self.IDENT)
            seen.add(tuple(np.round((M @ np.array([8.0, 8.0, 8.0, 1.0]))[:3], 6)))
        self.assertEqual(3, len(seen), "各 tick 挂点完全相同 —— 手持物没跟着手臂动")

    def test_grip_of_every_knife_sits_in_the_fist(self):
        """三把刀 × 两条动画 × 每 tick：握把点都必须在拳心。

        `emit_offset` 把握把点放到方块中心 = display 枢轴，所以这条同时锁住
        `held_item_common` 的出料平移和本文件这条挂点链——任何一边漂了都会红。
        """
        import gen_knife_trio as GK
        fist = np.array([-1.0, 8.5, 0.0, 1.0])       # 臂盒底面往上 1.5px、z 居中
        for item in GK.items():
            disp = item.display["thirdperson_righthand"]
            for name in ("dagger_slash", "dagger_stab"):
                kfs = RA.collect_keyframes(
                    json.loads((ANIM / f"{name}.json").read_text(encoding="utf-8"))["emote"])
                for i in range(17):
                    tick = 8.0 * i / 16
                    part = RA.sample_part(kfs, "rightArm", tick)
                    pivot = (np.array(self.P.PIVOT_OF["rightArm_lo"], float)
                             + np.array([part["x"], part["y"], part["z"]], float))
                    R = RA.part_rotation_matrix(part["pitch"], part["yaw"], part["roll"])
                    a = float(part["axis"])
                    Rb = RA.rotate_about_axis(
                        np.array([np.cos(-a), 0.0, np.sin(-a)]), float(part["bend"]))
                    arm = (self.P._aff(np.eye(3), pivot) @ self.P._aff(R, np.zeros(3))
                           @ self.P._about(Rb, self.P.ITEM_BEND_PIVOT_PX))
                    M = self.P.item_attach_modelpart(kfs, tick, disp)
                    d = float(np.linalg.norm((M @ np.array([8.0, 8.0, 8.0, 1.0]))[:3]
                                             - (arm @ fist)[:3]))
                    self.assertLess(
                        d, 0.5,
                        f"{item.key}/{name} t{tick:g}: 握把离拳心 {d:.2f}px。"
                        f"拳头只有 4px 宽，超过 0.5px 就看得出刀没被握住")


class HeldItemBoneTest(unittest.TestCase):
    """`rightItem` 那根骨头在挂载链里的位置 —— 摆错一格，正握/反握就是另一个姿态。

    运行时的调用序（`PlayerAnimator.HeldItemMixin.changeItemLocation` 注入在
    `HeldItemFeatureRenderer` 调 `renderItem` 之前）：

        translateToHand · Rx(-90) · Ry(180) · T(±1,2,-10)
        · [T(item_pos) · Rz(roll) · Ry(yaw) · Rx(pitch)]      ← 本骨头
        · T(display.translation) · R_disp · S · T(-8,-8,-8)

    关键是它在 **T(hand) 之后、T(display.translation) 之前**。放到 display 平移之后，
    刀会绕自己的几何中心转而不是绕握把转；放到 R_ATTACH 之前，转的就成了整条手臂。
    两种错法渲出来都"像是转了一下"，只有量才分得开。
    """

    IDENT = {"rotation": [0, 0, 0], "translation": [0, 0, 0], "scale": [1, 1, 1]}
    DISP = {"rotation": [0, -90, 55], "translation": [0, 2.5, -0.5], "scale": [0.85] * 3}

    def setUp(self):
        import preview_player_anim as P
        self.P = P

    def _kfs(self, **item_axes):
        moves = [{"tick": 0, "easing": "linear", "rightArm": {"pitch": 0.3, "bend": 0.5,
                                                              "axis": math.pi}}]
        if item_axes:
            moves.append({"tick": 0, "easing": "linear", "rightItem": dict(item_axes)})
        return self.P.collect_keyframes({"moves": moves})

    def test_an_absent_item_bone_is_exactly_the_identity(self):
        """没有这根骨头的 150 条动画必须一字不差地保持原样。"""
        bare = self.P.item_attach_modelpart(self._kfs(), 0.0, self.DISP)
        zero = self.P.item_attach_modelpart(self._kfs(pitch=0.0, yaw=0.0, roll=0.0),
                                            0.0, self.DISP)
        np.testing.assert_allclose(bare, zero, atol=1e-12,
                                   err_msg="缺省的手持物骨头必须等价于全 0")

    def test_the_bone_composes_as_Rz_Ry_Rx(self):
        """作用序与 `HeldItemMixin` 逐字对齐：mulPose(ZP) → mulPose(YP) → mulPose(XP)。"""
        p, y, r = 0.3, -0.2, 0.7
        got = self.P.item_bone_matrix(self._kfs(pitch=p, yaw=y, roll=r), 0.0)
        want = RA.part_rotation_matrix(p, y, r)
        np.testing.assert_allclose(got, want, atol=1e-12,
                                   err_msg="手持物骨头的合成次序与身体部件不一致")

    def test_it_pivots_the_item_about_the_hand_not_about_the_blade_centre(self):
        """display 为单位阵时，握把（模型枢轴 8,8,8）必须**原地不动**。

        这条区分「骨头挂在 T(display.translation) 之前」和「挂在之后」：挂到之后
        刀会绕自己的中心转，握把跟着跑掉。
        """
        base = self.P.item_attach_modelpart(self._kfs(), 0.0, self.IDENT)
        spun = self.P.item_attach_modelpart(self._kfs(pitch=math.pi), 0.0, self.IDENT)
        pivot = np.array([8.0, 8.0, 8.0, 1.0])
        np.testing.assert_allclose((base @ pivot)[:3], (spun @ pivot)[:3], atol=1e-9,
                                   err_msg="转刀时握把不该挪窝")

    def test_it_turns_the_item_without_touching_the_arm(self):
        """骨头只动刀，不动手臂 —— 挂到 R_ATTACH 之前就会把整条臂一起转过去。"""
        bare = self.P.segment_transforms(self._kfs(), 0.0)
        spun = self.P.segment_transforms(self._kfs(pitch=math.pi), 0.0)
        for name in ("rightArm_up", "rightArm_lo", "torso", "head"):
            np.testing.assert_allclose(
                bare[name], spun[name], atol=1e-12,
                err_msg=f"{name} 被手持物骨头带动了 —— 这根骨头只该动刀")

    def test_a_half_turn_reverses_the_blade_in_world_space(self):
        """半圈之后刃向必须整个倒过来（同一手臂姿态下）。"""
        blade = []
        for axes in ({}, {"pitch": math.radians(110.0), "roll": math.radians(-180.0)}):
            M = self.P.item_attach_modelpart(self._kfs(**axes), 0.0, self.DISP)
            g = (M @ np.array([8.0, 8.0, 8.0, 1.0]))[:3]
            tip = (M @ np.array([8.0, 21.1, 8.0, 1.0]))[:3]
            d = tip - g
            blade.append(d / np.linalg.norm(d))
        cos = float(blade[0] @ blade[1])
        self.assertLess(cos, -0.99,
                        f"正握与反握的刃向应几乎反向，实测夹角余弦 {cos:.4f}")


class DaggerStanceTest(unittest.TestCase):
    """站架：`body.yaw` 是**整个人**（含头/腿/手持物）唯一的转身通道。

    此前"侧身"只由 `torso.yaw` 给，而 `torso.*` 只作用于躯干 ModelPart，头/臂/腿
    各自独立（conventions §L243）——胯和腿全程正对前方，实际是 24° 的躯干扭转而
    不是站架。症状很隐蔽：眼睛把**胸口**读成朝向，于是 3/4 机位看起来才像正面。
    """

    ANIMS = ("dagger_slash", "dagger_stab")

    def _kfs(self, name):
        return RA.collect_keyframes(
            json.loads((ANIM / f"{name}.json").read_text(encoding="utf-8"))["emote"])

    def test_stance_is_a_constant_whole_body_rotation(self):
        """`body.yaw` 必须存在且**全程恒定**。

        恒定是关键：站架是站架，不该跟着挥砍逐帧转——那会变成脚在地上打滑。
        挥砍的转体由 `torso.yaw` 给（它本来就在动），两者分工不能混。
        """
        for name in self.ANIMS:
            kfs = self._kfs(name)
            yaws = {round(math.degrees(RA.sample_part(kfs, "body", 8.0 * i / 16)["yaw"]), 6)
                    for i in range(17)}
            self.assertEqual(
                1, len(yaws), f"{name}: body.yaw 在动（{sorted(yaws)}）—— 站架应当恒定")
            self.assertGreater(
                abs(yaws.pop()), 5.0,
                f"{name}: body.yaw ≈ 0，整个人没转 —— 又退回成只有躯干在扭")

    def test_head_still_looks_at_the_target(self):
        """转身之后头必须反向补偿回来，否则角色是"侧着身、也把脸扭开"。

        世界朝向 = body.yaw + head.yaw（head 是 body 的子节点，不是 torso 的）。
        """
        for name in self.ANIMS:
            kfs = self._kfs(name)
            for i in range(17):
                tick = 8.0 * i / 16
                world = math.degrees(RA.sample_part(kfs, "body", tick)["yaw"]
                                     + RA.sample_part(kfs, "head", tick)["yaw"])
                self.assertLess(
                    abs(world), 25.0,
                    f"{name} t{tick:g}: 头的世界朝向 {world:+.1f}°，脸扭离目标太远")

    def test_stance_does_not_disturb_the_blade_read(self):
        """绕竖直轴转身不该改变刃的仰角——纯 yaw 不动高度。

        这条是给未来改站架角度时用的安全网：只要还是纯 yaw，round 3/3 那批刃向锁
        就仍然成立；哪天有人往 body 上加了 pitch/roll，这里会先红。
        """
        for name in self.ANIMS:
            kfs = self._kfs(name)
            for i in range(9):
                tick = 8.0 * i / 8
                b = RA.sample_part(kfs, "body", tick)
                self.assertAlmostEqual(
                    0.0, float(b["pitch"]), places=9,
                    msg=f"{name} t{tick:g}: body.pitch 非零，会连刃的仰角一起改")
                self.assertAlmostEqual(
                    0.0, float(b["roll"]), places=9,
                    msg=f"{name} t{tick:g}: body.roll 非零，会连刃的仰角一起改")


class HeldSceneTextureTest(unittest.TestCase):
    """`preview_player_anim.build_scene` 也得会读**磁盘路径引用**的贴图。

    这是"贴图路径引用"那个 bug 的**第二个调用点**。`render_bbmodel._load_texture`
    早就修了，但 `build_scene` 里还留着一份无条件 `base64.b64decode`，于是
    `--hold` 挂在仓库那 11 个 linked-texture 模型上会报
    `binascii.Error: Incorrect padding` —— 一个和"贴图找不到"毫无关系的错。
    修一个调用点不算修完，所以这条按**每个磁盘路径模型**跑一遍。
    """

    def _path_referenced(self):
        out = []
        for path in sorted(MODELS.glob("*.bbmodel")):
            try:
                doc = json.loads(path.read_text(encoding="utf-8"))
            except (OSError, json.JSONDecodeError):
                continue
            textures = doc.get("textures") or []
            if textures and not str(textures[0].get("source", "")).startswith("data:"):
                out.append(path)
        return out

    def test_scene_builds_for_every_path_referenced_model(self):
        import preview_player_anim as P
        models = self._path_referenced()
        self.assertGreaterEqual(
            len(models), 5,
            "仓库里应当有一批 linked-texture 模型；一个都找不到说明这条锁失去了对象")
        import tempfile
        for path in models:
            with tempfile.TemporaryDirectory() as tmp:
                scene, ids, held = P.build_scene(Path(tmp) / "_scene.bbmodel", path)
                self.assertTrue(
                    held,
                    f"{path.name}: 手持物没有任何 element 进场景 —— 贴图读到了但几何丢了")
                tris, _, _, _ = RB.load_bbmodel(scene)
                self.assertTrue(tris, f"{path.name}: 合成场景渲不出三角形")

    def test_scene_atlas_carries_the_held_texture_not_a_blank(self):
        """贴图要真的贴进图集右上角，不是被静默换成透明块。"""
        import preview_player_anim as P
        import tempfile
        for path in (self._path_referenced() + [MODELS / "StoneKnife.bbmodel"]):
            with tempfile.TemporaryDirectory() as tmp:
                scene, _, _ = P.build_scene(Path(tmp) / "_scene.bbmodel", path)
                doc = json.loads(scene.read_text(encoding="utf-8"))
                src = doc["textures"][0]["source"].split(",", 1)[1]
                atlas = np.asarray(
                    Image.open(io.BytesIO(base64.b64decode(src))).convert("RGBA"), float)
                held_quadrant = atlas[0:64, 64:128, 3]
                self.assertGreater(
                    float(held_quadrant.max()), 0.0,
                    f"{path.name}: 图集里手持物那一格全透明，贴图没贴进去")


class PreviewEndTickTest(unittest.TestCase):
    """`preview_player_anim` 的 GIF 段长必须来自 emote 自己的 `endTick`。

    这是本文件第 5 类"预览工具在骗人"的 bug，做木棍时撞上的：`main()` 把**整份 JSON
    文档**当 emote 往下传，`collect_keyframes` 那行自己做了 `.get("emote", ...)` 所以关键帧
    是对的，但 `endTick` 取的是文档顶层——那儿没有这个键，于是永远吃默认值 8。

    症状极隐蔽：仓库里绝大多数动画正好就是 8 tick，图看着完全正常。只有 `endTick ≠ 8`
    的会被**悄悄截断**——`club_smash` 是 12 tick，GIF 只播到 t8，整段收势（overshoot +
    低位滞留 + 拖回）一帧都看不见，而工具还打印"原速 400ms"。照那个 GIF 去调节奏，会把
    本来正确的收势判成"没有收势"。

    修法连同**去掉默认值**一起：静默兜底正是这个 bug 能活下来的原因。
    """

    def test_end_tick_comes_from_the_emote_not_the_document(self):
        import preview_player_anim as P
        doc = json.loads((ANIM / "club_smash.json").read_text(encoding="utf-8"))
        self.assertEqual(12, int(doc["emote"]["endTick"]),
                         "本用例依赖 club_smash 是 12 tick（≠ 默认值 8）才有鉴别力")
        self.assertEqual(12.0, P._end_tick(doc["emote"]))

    def test_passing_the_whole_document_raises_instead_of_defaulting(self):
        """传错层级必须**炸**。返回 8 的话，调用方拿到的是一段被截断的动画而毫不知情。"""
        import preview_player_anim as P
        doc = json.loads((ANIM / "club_smash.json").read_text(encoding="utf-8"))
        with self.assertRaisesRegex(KeyError, "endTick"):
            P._end_tick(doc)

    def test_every_shipped_animation_reports_its_own_length(self):
        """全仓库动画逐个过一遍：剥好 emote 之后都取得到 endTick。"""
        import preview_player_anim as P
        checked = 0
        for path in sorted(ANIM.glob("*.json")):
            doc = json.loads(path.read_text(encoding="utf-8"))
            emote = doc.get("emote", doc)
            self.assertGreater(P._end_tick(emote), 0.0, f"{path.name}: endTick 非正")
            checked += 1
        self.assertGreater(checked, 100, "动画资产一个都没扫到，这条锁失去了对象")


if __name__ == "__main__":
    unittest.main()
