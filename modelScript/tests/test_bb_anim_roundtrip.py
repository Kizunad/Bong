#!/usr/bin/env python3
"""MC 动画 ↔ bbmodel 的**往返**锁。

## 为什么是往返锁，而不是正向断言

2026-08-26 踩的那个坑：`gen_jian_player_anim` 把动画通道的符号写反了（X/Y 多取了一次
反），生成的 bbmodel 在 Blockbench 里是 pitch / yaw 双双镜像的姿态。

**当时是有离线核验的，而且绿。** 那份核验脚本自己算了一遍 group 层级、和
`preview_player_anim` 的参考数学逐点对拍到 0.05px——因为两边用的是同一套（错的）假设。
自己写的正向断言永远抓不住"我对约定的理解整体偏了"这类错；能抓住它的只有两样：

1. 拿进真的 Blockbench 转一圈再读回来（那次就是这么发现的，但没法进 CI）；
2. **锁住去程和回程共用同一份常量、且互为逆运算**——一侧偷偷改了符号，往返立刻断。

本文件做的是 2。它锁不住"两侧同时改错"，这一点在 `bb_anim_axes` 的 docstring 里写明了。

## 还有一层：读和写**本来就不是**同一套符号

Blockbench 读 animation 通道时对 X/Y 取反、存盘时不取反（两次实测，见 `bb_anim_axes`）。
所以：

    生成器写出去    → WRITE_LAYERS（预取反，抵消它读入时的取反）
    它存盘后我们读  → READ_LAYERS

"往返"因此有两种，别混：**生成器 → 读回**用 WRITE 侧（本文件测的就是这个，因为 CI 里
没有 Blockbench）；**Blockbench 存盘 → 读回**用 READ 侧，只能靠人工复现。
"""

from __future__ import annotations

import json
import sys
import unittest
from pathlib import Path

LIB_DIR = Path(__file__).resolve().parents[1]
REPO = LIB_DIR.parent
for _d in (LIB_DIR / "generators", LIB_DIR / "tools", REPO / "client" / "tools"):
    sys.path.insert(0, str(_d))

from bbmodel_maker.rig import bb_anim_axes as AX  # noqa: E402
from bbmodel_maker.workbench import bbmodel_to_pose as BP  # noqa: E402
import gen_club_player_anim as GCP  # noqa: E402
import gen_jian_player_anim as GJP  # noqa: E402
from bbmodel_maker.render import render_player_pose as RP  # noqa: E402
import render_animation as RA  # noqa: E402  运行时口径的独立求解器，用来和烘培侧交叉对拍单位

ANIM = REPO / "client" / "src" / "main" / "resources" / "assets" / "bong" / "player_animation"
MODELS = LIB_DIR / "models"


class AxisConversionTest(unittest.TestCase):
    """`bb_anim_axes` 自身：每条换算都必须可逆。"""

    def test_write_then_read_back_with_the_same_side(self):
        """同一侧符号必须自洽：写出去再按同一侧读回来，逐轴不变。"""
        axes = {"pitch": -82.7, "yaw": -20.0, "roll": +12.1}
        for name, index, sign in AX.WRITE_LAYERS:
            triple = AX.rotation_to_bb(axes, name)
            self.assertAlmostEqual(axes[name], triple[index] / sign, places=6)

    def test_the_two_sides_differ_on_exactly_pitch_and_yaw(self):
        """读写不对称是**设计**，不是笔误——Blockbench 读入时取反 X/Y、存盘时不取反。

        差异必须**恰好**落在 pitch 与 yaw 上：roll 也跟着翻的话，说明有人把两侧当成
        同一套整体取反了（2026-08-26 就是这么把资产写镜像的）。
        """
        write = {name: sign for name, _i, sign in AX.WRITE_LAYERS}
        read = {name: sign for name, _i, sign in AX.READ_LAYERS}
        self.assertEqual({"pitch", "yaw"},
                         {name for name in write if write[name] != read[name]},
                         f"两侧差异应恰好是 pitch/yaw；实际 write={write} read={read}")
        self.assertEqual(write["roll"], read["roll"], "roll 两侧同号")

    def test_rotation_from_bb_uses_the_read_side(self):
        """`rotation_from_bb` 是给**Blockbench 存盘的文件**用的，走读侧。"""
        for name, index, sign in AX.READ_LAYERS:
            triple = [0.0, 0.0, 0.0]
            triple[index] = 30.0
            self.assertAlmostEqual(30.0 / sign, AX.rotation_from_bb(triple, name), places=6)

    def test_each_layer_only_touches_its_own_component(self):
        """单轴分层的全部意义：一层只动一个分量。混进第二个分量，Blockbench 与 MC 的
        欧拉顺序差异就又有了发挥余地。"""
        axes = {"pitch": 30.0, "yaw": 40.0, "roll": 50.0}
        for name, index, _sign in AX.AXIS_LAYERS:
            triple = AX.rotation_to_bb(axes, name)
            for k in range(3):
                if k != index:
                    self.assertEqual(0.0, triple[k], f"{name} 层渗到了分量 {k}")

    def test_euler_helpers_are_the_static_right_handed_pair(self):
        """`mc_to_euler` / `euler_to_mc` 是**静态 group.rotation** 那一套（读侧口径），
        它俩互逆。静态字段不走 animation 通道，不吃那层取反。"""
        axes = {"pitch": -82.7, "yaw": -20.0, "roll": +12.1}
        back = AX.euler_to_mc(AX.mc_to_euler(axes))
        for key, value in axes.items():
            self.assertAlmostEqual(value, back[key], places=6)
        self.assertEqual([82.7, -20.0, -12.1], AX.mc_to_euler(axes))

    def test_position_write_side_pre_negates_x(self):
        """position 也走 animation 通道，X 同样要预取反。"""
        self.assertEqual([-16.0, -16.0, 16.0],
                         AX.body_position_to_bb({"x": 1.0, "y": 1.0, "z": 1.0}))

    def test_position_read_side_is_the_other_sign(self):
        """读侧（Blockbench 存盘）的 X 不带那层取反。"""
        self.assertAlmostEqual(-1.0, AX.body_position_from_bb([16.0, 0.0, 0.0])["x"])

    def test_bend_round_trip_on_the_write_side(self):
        """写出去再按写侧读回来，(bend, axis) 必须原样还原。

        `bend_from_bb` 是**读侧**的，和 `bend_to_bb` 不互逆——两侧差一个 X 取反，
        这正是本模块最容易被误用的一处，所以两边分开测。
        """
        for bend, axis in ((92.4, 180.0), (44.0, 0.0), (0.0, 180.0)):
            written = AX.bend_to_bb(bend, axis)
            back_bend, back_axis = BP._bend_from(written, AX.WRITE_LAYERS)
            self.assertAlmostEqual(bend, back_bend, places=6)
            if bend > 0:
                self.assertEqual(axis, back_axis)

    def test_bend_read_side_is_the_other_sign(self):
        """读侧：Blockbench 存出来的是未取反的内部值。"""
        self.assertEqual((90.0, 180.0), AX.bend_from_bb(+90.0))
        self.assertEqual((90.0, 0.0), AX.bend_from_bb(-90.0))

    def test_bend_axes_point_opposite_ways(self):
        """axis=0 与 axis=180 是相反的折向；同号就等于两者不分，肘会往错的方向折。"""
        self.assertLess(AX.bend_to_bb(90.0, 0.0) * AX.bend_to_bb(90.0, 180.0), 0.0)

    def test_bend_rides_the_write_side_x_sign(self):
        """bend 走的也是 animation 通道，符号必须跟 pitch 同一侧；跟错了肘膝反折。"""
        pitch_sign = dict((name, sign) for name, _i, sign in AX.WRITE_LAYERS)["pitch"]
        self.assertEqual(pitch_sign > 0, AX.bend_to_bb(90.0, 0.0) > 0)

    def test_oblique_bend_axis_is_refused(self):
        with self.assertRaisesRegex(AssertionError, "不是纯 X 折弯"):
            AX.bend_to_bb(90.0, 90.0)

    def test_bend_twist_residual_is_refused(self):
        """拖 gizmo 拧出来的 y/z 残差在 MC 里表达不了；静默丢掉会让读回来的姿态
        和 Blockbench 里看到的不是同一个。"""
        with self.assertRaisesRegex(ValueError, "残差"):
            AX.assert_pure_x([92.4, -4.8, -19.4], where="rightArm")
        self.assertEqual((92.4, 180.0), AX.assert_pure_x([92.4, 0.2, -0.3]))


class GeneratorSharesTheOneConversionTest(unittest.TestCase):
    """两个生成器 + 回程读取器必须用**同一份**常量。

    各自抄一份就是这次出错的根因：锏那份写反了，木棍那份一开始照抄了它。
    """

    def test_all_three_reference_the_same_module(self):
        self.assertIs(AX, GCP.AX)
        self.assertIs(AX, GJP.AX)
        self.assertIs(AX, BP.AX)

    def test_the_jian_generator_no_longer_defines_its_own_signs(self):
        """锏那份现在只是把公共常量取个别名；哪天有人把本地定义加回来，这里红。"""
        self.assertIs(AX.AXIS_LAYERS, GJP.AXIS_LAYERS)
        self.assertIs(AX.bend_to_bb, GJP.bend_single_axis)

    def test_the_legacy_alias_points_at_the_write_side(self):
        """`AXIS_LAYERS` 这个老名字一律指写侧——生成器用得最多，指错就是写镜像资产。"""
        self.assertIs(AX.WRITE_LAYERS, AX.AXIS_LAYERS)


class AnimationRoundTripTest(unittest.TestCase):
    """源 JSON → bbmodel → 读回来，逐轴必须还是同一个姿态。"""

    CASES = (("ClubPlayerAnim.bbmodel", "club_smash"),
             ("ClubPlayerAnim.bbmodel", "club_sweep"),
             ("JianPlayerAnim.bbmodel", "jian_dual_smash"),
             # 脊骨剑：第一版这份文件里 8 条动画全是空的（`length=0 / bones=0`），
             # 往返锁是最直接的兜底——没有帧就还原不出姿态，这里立刻红。
             ("BeastSpineSwordPlayerAnim.bbmodel", "sword_spine_slash"),
             # 采药刀四条。这里挂上的直接理由是**它差点被漏烘**：动画在
             # `client/tools/` 重做完了，`gen_herb_sickle_player_anim.py` 却没重跑，
             # bbmodel 里留着上一轮的旧姿态（还挂着已经还给刀三件套的 dagger_slash）。
             # 往返锁盯的正是"bbmodel 里的那一份和源 JSON 不是同一个东西"。
             ("HerbSicklePlayerAnim.bbmodel", "harvest_crouch"),
             ("HerbSicklePlayerAnim.bbmodel", "sickle_reap"),
             ("HerbSicklePlayerAnim.bbmodel", "sickle_stand_cut"),
             # sickle_defend 是这批里唯一用了 `body.yaw`（站架）的，
             # 于是也是唯一走 `_body` → `root_*` 那四条轨道的往返用例。
             ("HerbSicklePlayerAnim.bbmodel", "sickle_defend"))

    def _bbmodel_anim(self, filename, name):
        doc = json.loads((MODELS / filename).read_text(encoding="utf-8"))
        for anim in doc.get("animations", []):
            if anim["name"] == name:
                return anim
        self.fail(f"{filename} 里没有动画 {name}")

    def test_every_axis_survives_the_round_trip(self):
        """生成器直出的文件，按**写侧**读回来必须逐轴还原（CI 里没有 Blockbench，
        能测的就是这一半）。"""
        for filename, name in self.CASES:
            anim = self._bbmodel_anim(filename, name)
            _n, _e, table = RP.anim_pose_table(ANIM / f"{name}.json")
            for tick, source in table:
                read_back = BP.read_pose(anim, float(tick), layers=AX.WRITE_LAYERS)
                for part, axes in source.items():
                    got = read_back.get("_body" if part == "_body" else part)
                    self.assertIsNotNone(
                        got, f"{name} t{tick}: 读回来少了 {part}")
                    for axis, value in axes.items():
                        if axis == "axis":
                            continue
                        if part != "_body" and axis in ("x", "y", "z"):
                            # part 级位移**故意不在这里比**：库里的
                            # `bbmodel_to_pose._position_from` 是按 body 口径写的，
                            # 带 `/ PX_PER_BLOCK`，而 part 级的 x/y/z 是 ModelPart 枢轴
                            # px 不是格。写侧曾经也这么错（套了 `body_position_to_bb`），
                            # 两侧对称 ⇒ 这条往返断言照样绿，正是本文件 docstring 说的
                            # 「锁不住两侧同时改错」。写侧已修（`part_position_to_bb`），
                            # 读侧在库里改不动，所以单位改由 `PartOffsetUnitTest`
                            # 直接读文件里的 px 值来钉，不经过任何共用换算。
                            continue
                        self.assertAlmostEqual(
                            float(value), float(got.get(axis, 0.0)), places=2,
                            msg=f"{name} t{tick} {part}.{axis}: "
                                f"源 {value} → 读回 {got.get(axis)}")

    def test_reading_a_generator_file_with_the_wrong_side_comes_out_mirrored(self):
        """变异用例：拿**读侧**符号去解生成器直出的文件，pitch/yaw 必须整个镜像。

        这条是本文件的价值所在——它证明这套锁不是"自己和自己对拍"，也把
        `bbmodel_to_pose` 那个按 `format_version` 选边的逻辑变成了有后果的事：选错边，
        读出来的姿态就是镜像的（2026-08-26 正是这么把资产写反的）。
        """
        filename, name = self.CASES[0]
        anim = self._bbmodel_anim(filename, name)
        _n, _e, table = RP.anim_pose_table(ANIM / f"{name}.json")
        tick, source = table[2]
        wrong = BP.read_pose(anim, float(tick), layers=AX.READ_LAYERS)
        for axis in ("pitch", "yaw"):
            self.assertAlmostEqual(
                -float(source["rightArm"][axis]),
                float(wrong["rightArm"].get(axis, 0.0)), places=2,
                msg=f"用错侧读，{axis} 应当正好是镜像值")
        self.assertAlmostEqual(float(source["rightArm"]["roll"]),
                               float(wrong["rightArm"].get("roll", 0.0)), places=2,
                               msg="roll 两侧同号，不该跟着镜像")

    def test_the_reader_picks_its_side_from_format_version(self):
        """生成器只写 4.10，Blockbench 5 存盘一律变 5.0——这是本仓既有的"手改过"判据，
        `bbmodel_to_pose` 拿它来选边。"""
        self.assertIs(AX.WRITE_LAYERS,
                      BP.pick_layers({"meta": {"format_version": "4.10"}})[0])
        self.assertIs(AX.READ_LAYERS,
                      BP.pick_layers({"meta": {"format_version": "5.0"}})[0])
        self.assertIs(AX.READ_LAYERS,
                      BP.pick_layers({"meta": {"format_version": "4.10"}}, "blockbench")[0])


class PartOffsetUnitTest(unittest.TestCase):
    """part 级 `x/y/z` 的**单位**：ModelPart 枢轴 px，不是格。

    ## 这道门为什么必须绕开共用换算

    写侧曾经拿 `AX.body_position_to_bb`（带 `× PX_PER_BLOCK`）去烘 part 位移，读侧
    `bbmodel_to_pose._position_from` 又带 `/ PX_PER_BLOCK`——**两侧对称地错**，于是
    `AnimationRoundTripTest` 那条往返断言一路绿，而文件里的值是真值的 16 倍。

    这个 bug 活了很久没被发现，因为存量动画的 `leg.z` 都是 ±0.05~0.10 那种量级
    （放大后也才 ±0.8~1.6px）。采药刀那批按 vanilla 蹲伏的量级写了 ±2.0px，放大成
    ±32px——两腿在 Blockbench 里飞出 3.5 格，是仓库所有者打开文件一眼看出来的。

    所以这道门**只读文件里的原始 px 数**，再与 `render_animation.solve_skeleton`
    （运行时口径：`pivot(px) + offset`）交叉对拍。它和写侧不共用任何一行换算代码，
    两侧一起改错也骗不过它。

    ## 单位的四处独立佐证

    · `anim_common` docstring：「model pixels × 1/16 for body, **raw for part offsets**」
    · `render_animation.solve_skeleton`：`pivot_local = pivot + pivot_offset`（px + px）
    · PlayerAnimator `AnimationApplier.updatePart`：值直接写进 `ModelPart.x/y/z`（px），
      缺省 seed 自 vanilla 的 px
    · conventions §7.1：`rightLeg.z` 的 defaultValue 是 `0.1f`——vanilla 的 px 值
      （若单位是格，缺省该是 0.00625）
    """

    # 出料侧的符号：X 预取反 + Y 翻 + Z 同号（与 `body_position_to_bb` 同一套）
    SIGNS = {"x": -1.0, "y": -1.0, "z": +1.0}
    IDX = {"x": 0, "y": 1, "z": 2}

    def _anim(self, filename, name):
        doc = json.loads((MODELS / filename).read_text(encoding="utf-8"))
        for anim in doc.get("animations", []):
            if anim["name"] == name:
                return anim
        self.fail(f"{filename} 里没有动画 {name}")

    def _raw_position(self, anim, bone, tick):
        """直接从文件里取该骨在该 tick 的 position 三元组，不经任何换算。"""
        animators = anim.get("animators", {})
        it = animators.values() if isinstance(animators, dict) else animators
        for an in it:
            if not isinstance(an, dict) or an.get("name") != bone:
                continue
            for kf in an.get("keyframes", []) or []:
                if kf.get("channel") != "position":
                    continue
                if abs(float(kf.get("time", -1)) * 20.0 - tick) > 1e-6:
                    continue
                dp = (kf.get("data_points") or [{}])[0]
                return [float(dp.get(k, 0) or 0) for k in "xyz"]
        return None

    def test_part_offsets_are_written_in_raw_pixels(self):
        """源 POSE 里的 `leg.z = -2.0`（px）在文件里就该是 -2.0，不是 -32。"""
        checked = 0
        for filename, name in AnimationRoundTripTest.CASES:
            anim = self._anim(filename, name)
            _n, _e, table = RP.anim_pose_table(ANIM / f"{name}.json")
            for tick, source in table:
                for part, axes in source.items():
                    if part == "_body":
                        continue
                    prefix = BP.PART_PREFIX.get(part)
                    if prefix is None:
                        continue
                    want = {k: float(v) for k, v in axes.items() if k in self.IDX}
                    if not any(abs(v) > 1e-9 for v in want.values()):
                        continue
                    got = self._raw_position(anim, f"{prefix}_{AX.AXIS_ORDER[-1]}", tick)
                    self.assertIsNotNone(
                        got, f"{name} t{tick} {part}: 源里有位移，文件里却没烘 position")
                    for k, v in want.items():
                        self.assertAlmostEqual(
                            v * self.SIGNS[k], got[self.IDX[k]], places=3,
                            msg=f"{name} t{tick} {part}.{k}: 源 {v}px → 文件 "
                                f"{got[self.IDX[k]]}px。差 16 倍就是又把 px 当格了"
                                f"（part 级位移是 ModelPart 枢轴 px，只有 body 是格）")
                        checked += 1
        self.assertGreater(
            checked, 0,
            "一个 part 级位移都没查到——这道门变成空覆盖了。CASES 里至少要有一条"
            "用了 leg.z 的动画（采药刀那四条都用）")

    def test_the_bake_agrees_with_the_runtime_skeleton_solver(self):
        """交叉对拍：把文件里的 px 值还原成源单位，必须和 `solve_skeleton` 用的是同一个数。

        `solve_skeleton` 是运行时口径的独立实现（`pivot(px) + offset`）。它和烘培侧
        不共用换算代码，所以两边一致才说明单位真的对上了。
        """
        filename, name = "HerbSicklePlayerAnim.bbmodel", "harvest_crouch"
        anim = self._anim(filename, name)
        kfs = RA.collect_keyframes(
            json.loads((ANIM / f"{name}.json").read_text(encoding="utf-8"))["emote"])
        for part, prefix in (("rightLeg", "leg_right"), ("leftLeg", "leg_left")):
            runtime_z = float(RA.sample_part(kfs, part, 0.0)["z"])
            baked = self._raw_position(anim, f"{prefix}_{AX.AXIS_ORDER[-1]}", 0)
            self.assertIsNotNone(baked, f"{part} 的位移没烘进 {filename}")
            self.assertAlmostEqual(
                runtime_z, baked[2] * self.SIGNS["z"], places=3,
                msg=f"{part}.z：运行时求解器读到 {runtime_z}px，bbmodel 里烘的是 "
                    f"{baked[2]}px——两套实现对不上，单位又错了一侧")


class PoseTickContractTest(unittest.TestCase):
    """全仓不变量：生成器 POSE 的键 == 出料 JSON 的 tick。

    这是**回程能成立的前提**，不是风格偏好。bbmodel 是从 `player_animation/*.json` 烘出
    来的，所以 `bbmodel_to_pose` 读到的帧号是出料 tick，而它把这个帧号原样当作 POSE 的
    键打印出来给人贴回生成器。两边编号一旦不同，贴回去就是**静默**改掉整条动画的节奏：
    姿态一个数没错、脚本不报错、出料照常，只有节奏变了。

    真出过一次：`gen_club_sweep` 曾把 8 tick 的设计骨架在出料一步拉长成 10 tick，POSE 的
    键是 0/2/3/4/5/6/7/8 而 JSON 里是 0/3/4/5/6/7/9/10。修法是把落位写进 POSE 键（求落位
    的 `integer_retime` 留着，搬表的那个函数删了），并由本类守住全仓。
    """

    TOOLS = REPO / "client" / "tools"
    # 扫描退化兜底：这条不变量的价值全在"覆盖了所有生成器"上。哪天 import 路径写歪、
    # glob 没匹配上、POSE 改了名，扫描会安静地收敛到 0 条并照样绿——那才是最坏的结果。
    # 今天实际覆盖 86 个，留点余量当地板。
    MIN_COVERAGE = 80

    @classmethod
    def _scan(cls):
        """→ (对得上的, 对不上的, import 炸了的, 没有 POSE 或没有同名 JSON 的)。"""
        import importlib

        matched, mismatched, broken, skipped = [], [], [], []
        for path in sorted(cls.TOOLS.glob("gen_*.py")):
            module_name = path.stem
            try:
                module = importlib.import_module(module_name)
            except Exception as exc:                       # noqa: BLE001 —— 要的就是全抓
                broken.append((module_name, f"{type(exc).__name__}: {exc}"))
                continue
            pose = getattr(module, "POSE", None)
            if (not isinstance(pose, dict) or not pose
                    or not all(isinstance(k, (int, float)) for k in pose)):
                skipped.append(module_name)                # 不是"tick → 姿态"那种生成器
                continue
            emitted = ANIM / f"{module_name[len('gen_'):]}.json"
            if not emitted.exists():
                skipped.append(module_name)                # 出料名和模块名不同源
                continue
            moves = json.loads(emitted.read_text(encoding="utf-8"))["emote"]["moves"]
            ticks = sorted({int(m["tick"]) for m in moves})
            keys = sorted(int(k) for k in pose)
            (matched if keys == ticks else mismatched).append((module_name, keys, ticks))
        return matched, mismatched, broken, skipped

    def setUp(self) -> None:
        self.matched, self.mismatched, self.broken, self.skipped = self._scan()

    def test_every_generator_poses_at_the_ticks_it_ships(self) -> None:
        detail = "\n".join(
            f"  {name}\n    POSE {keys}\n    JSON {ticks}"
            for name, keys, ticks in self.mismatched)
        self.assertEqual(
            [], [name for name, _k, _t in self.mismatched],
            "这些生成器的 POSE 键和它出料的 JSON tick 对不上——`bbmodel_to_pose` 读回来的"
            f"帧号贴进去会静默改掉节奏：\n{detail}")

    def test_importing_every_generator_still_works(self) -> None:
        """顺带守住"生成器全都还能 import"。扫描要靠 import 才能看见 POSE，某个模块炸了
        就会从覆盖里消失——那正是上一条最怕的静默漏检。"""
        self.assertEqual([], [name for name, _why in self.broken],
                         f"这些生成器 import 就炸：{self.broken}")

    def test_the_scan_actually_covers_the_generators(self) -> None:
        covered = len(self.matched) + len(self.mismatched)
        self.assertGreaterEqual(
            covered, self.MIN_COVERAGE,
            f"只扫到 {covered} 个生成器（跳过 {len(self.skipped)}、炸 {len(self.broken)}）——"
            f"低于地板 {self.MIN_COVERAGE}，八成是扫描本身坏了而不是生成器真少了")

    def test_the_two_club_animations_are_covered(self) -> None:
        """点名这两条：本仓唯一破过这条不变量的就是 `gen_club_sweep`，被跳过就等于没测。"""
        covered = {name for name, _k, _t in self.matched + self.mismatched}
        for name in ("gen_club_smash", "gen_club_sweep"):
            self.assertIn(name, covered, f"{name} 没进扫描覆盖")

    def test_club_sweep_poses_at_its_stretched_ticks(self) -> None:
        """把当年那个 bug 的具体形状钉死：拉长后的落位必须落在 POSE 键上。"""
        import gen_club_sweep as SWEEP

        self.assertEqual([0, 3, 4, 5, 6, 7, 9, 10], sorted(SWEEP.POSE),
                         "club_sweep 的 POSE 必须直接写在出料 tick 上，不许在出料时现搬")


if __name__ == "__main__":
    unittest.main()
