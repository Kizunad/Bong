#!/usr/bin/env python3
"""BeastSpineSwordPlayerAnim 的两条硬契约锁：**动画烘得出来** + **剑挂在手上**。

两条都对应第一版真的翻过的车，症状分别是"Blockbench 里一帧动画都没有"和"剑飘在身外
一整个身位"：

1. **关键帧必须是 Blockbench 的 animator 格式。** 第一版自写的 baker 从 `doc["moves"]`
   取动作（真实路径是 `doc["emote"]["moves"]`），拿到的永远是空列表；外面还包了一层
   `except Exception: print("⚠ 失败")`。产出的 8 条动画全是 `length=0 / bones=0`，文件
   在 Blockbench 里打得开、Animate 模式里空空如也，而生成器打印的是成功。所以这里锁的
   不是"生成器没报错"，是**产出里真的有帧**，且帧号覆盖源 JSON 的每一个 tick。

2. **剑的握把点必须在出料系。** `BeastSpineSword.bbmodel` 按
   `held_item_common.emit_offset` 出料，握把点落方块中心 (8,8,8)；本生成器把这一点搬到
   `HAND_REST`。两侧的常量在这里对焊——任一侧单独改，剑就离手，而渲图上那是"姿势有点
   怪"，肉眼未必立刻认出来。
"""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

import numpy as np

LIB_DIR = Path(__file__).resolve().parents[1]
REPO = LIB_DIR.parent
for _d in (LIB_DIR / "generators", LIB_DIR / "tools", REPO / "client" / "tools"):
    sys.path.insert(0, str(_d))

from bbmodel_maker.render import held_item_render as H  # noqa: E402
from bbmodel_maker.render import render_player_pose as RP  # noqa: E402
from bbmodel_maker.rig.animkit import PoseRig  # noqa: E402

import gen_beast_spine_sword as SWORD  # noqa: E402
import gen_beast_spine_sword_player_anim as GEN  # noqa: E402

ANIM = REPO / "client" / "src" / "main" / "resources" / "assets" / "bong" / "player_animation"
BLOCK_CENTRE = np.array([8.0, 8.0, 8.0])
HAND_R = np.asarray(H.HAND_REST["right"], float)
SWORD_LEN_PX = SWORD.BLADE_Y0 + SWORD.BLADE_LEN + SWORD.TIP_LEN - SWORD.GRIP_PX


def _sword_local(point_along_blade: float) -> np.ndarray:
    """剑局部坐标系里、从握把点沿刃轴走 `point_along_blade` px 的那一点。

    挂手时整把剑被平移了 `HAND_REST - SWORD_GRIP_PX`，所以握把点的局部坐标就是
    `HAND_REST` 本身，剑尖在它 +Y 方向 `SWORD_LEN_PX` 处。
    """
    return HAND_R + np.array([0.0, point_along_blade, 0.0])


def _world(M: np.ndarray, local: np.ndarray) -> np.ndarray:
    return M[:3, :3] @ local + M[:3, 3]


def _rig_pose(pose: dict):
    """emotecraft 姿态（MC 轴，度）→ PoseRig 的 Pose。

    PoseRig 把 `rest_rot + ch.rot` **逐分量相加**再解欧拉，所以 ch.rot 必须和静态
    group.rotation 同一套编码 = `bb_anim_axes.READ_LAYERS`：bb = (−pitch, +yaw, −roll)。
    bend 层是纯 X：axis=180 → bb.x = +bend；axis=0 → bb.x = −bend。
    """
    from bbmodel_maker.rig.animkit import Pose

    p = Pose()
    for part, (prefix, _has_bend) in GEN.PART_GROUPS.items():
        ax = pose.get(part)
        if not ax:
            continue
        p[f"{prefix}_pitch"].rot = [-float(ax.get("pitch", 0.0)), 0.0, 0.0]
        p[f"{prefix}_yaw"].rot = [0.0, float(ax.get("yaw", 0.0)), 0.0]
        p[f"{prefix}_roll"].rot = [0.0, 0.0, -float(ax.get("roll", 0.0))]
        if "bend" in ax:
            sign = 1.0 if abs(float(ax.get("axis", 0.0)) - 180.0) < 1.0 else -1.0
            p[f"{prefix}_bend"].rot = [sign * float(ax["bend"]), 0.0, 0.0]
        if any(k in ax for k in "xyz"):
            p[f"{prefix}_roll"].pos = [float(ax.get("x", 0.0)) * 16.0,
                                       -float(ax.get("y", 0.0)) * 16.0,
                                       float(ax.get("z", 0.0)) * 16.0]
    body = pose.get("_body") or {}
    if body:
        p["root_pitch"].rot = [-float(body.get("pitch", 0.0)), 0.0, 0.0]
        p["root_yaw"].rot = [0.0, float(body.get("yaw", 0.0)), 0.0]
        p["root_roll"].rot = [0.0, 0.0, -float(body.get("roll", 0.0))]
        p["root_pos"].pos = [float(body.get("x", 0.0)) * 16.0,
                             -float(body.get("y", 0.0)) * 16.0,
                             float(body.get("z", 0.0)) * 16.0]
    return p


def _blade_dir(M: np.ndarray) -> np.ndarray:
    """世界系里的剑身单位向量（握把 → 剑尖）。"""
    d = _world(M, _sword_local(SWORD_LEN_PX)) - _world(M, _sword_local(0.0))
    return d / np.linalg.norm(d)


class EmitSystemContractTest(unittest.TestCase):
    """剑的出料系与挂手常量必须对得上——这是"剑在不在手里"的全部依据。"""

    def test_sword_generator_puts_the_grip_at_the_block_centre(self):
        """授权系的握把点 (0, GRIP_PX, 0) 加上 EMIT_OFFSET 必须落在 (8,8,8)。"""
        grip_emitted = np.array([0.0, SWORD.GRIP_PX, 0.0]) + np.array(SWORD.EMIT_OFFSET)
        np.testing.assert_allclose(
            grip_emitted, BLOCK_CENTRE, atol=1e-9,
            err_msg="期望：出料后握把点落在方块中心（MC 的 display 变换绕方块中心转）；"
                    f"实际：{grip_emitted}。差这一步 = 半个方块的系统性偏移",
        )

    def test_grip_is_the_middle_of_the_wrapped_handle(self):
        """握把点取缠绳段中点——拳心对准处，不是柄尾也不是剑格。"""
        self.assertAlmostEqual(
            SWORD.GRIP_PX, (SWORD.GRIP_Y[0] + SWORD.GRIP_Y[1]) / 2.0, places=9,
            msg="期望：GRIP_PX == 缠绳握把段中点；实际：两者已分叉",
        )

    def test_player_anim_uses_the_emitted_grip_point(self):
        """挂手用的握把点必须就是出料系的那一点，不能再自带一份授权系坐标。"""
        np.testing.assert_allclose(
            np.asarray(GEN.SWORD_GRIP_PX, float), BLOCK_CENTRE, atol=1e-9,
            err_msg="期望：SWORD_GRIP_PX == 方块中心（剑已是出料系）；"
                    f"实际：{GEN.SWORD_GRIP_PX}。写成授权系的 (0,3.4,0) 会让剑整体偏 8px",
        )

    def test_the_emitted_bbmodel_really_carries_the_offset(self):
        """不只对常量，也对产出：出料后的几何整体就该比授权系高出 EMIT_OFFSET。"""
        doc = SWORD.build_bbmodel()
        y_lo = min(e["from"][1] for e in doc["elements"])
        # 授权系最低点是流苏末端（负值），出料后应当整体上移 EMIT_OFFSET[1]
        self.assertGreater(
            y_lo, -1e-9,
            msg=f"期望：出料后没有元素落在 y<0（流苏末端正好压在方块底面）；实际 y_min={y_lo}",
        )
        x_mid = (min(e["from"][0] for e in doc["elements"])
                 + max(e["to"][0] for e in doc["elements"])) / 2.0
        self.assertAlmostEqual(
            x_mid, 8.0, places=6,
            msg=f"期望：剑在 x 上仍对称于方块中线 8；实际中线 {x_mid}",
        )


class SwordIsAttachedToTheHandTest(unittest.TestCase):
    """剑必须是右臂的**子骨**，且静止姿下握把正落在手心。"""

    @classmethod
    def setUpClass(cls):
        cls.rig = PoseRig(GEN.OUT_BB)
        cls.world = cls.rig.world()

    def test_sword_hangs_under_the_forearm_bend_group(self):
        """挂在小臂（bend 段）之下：肘一弯剑跟着走。挂到 pitch 层就会"肘弯剑不弯"。"""
        self.assertEqual(
            self.rig.bones["sword_right_roll"].parent, "arm_right_bend",
            "期望：sword_right_roll 的父骨是 arm_right_bend；实际不是——剑没跟着肘走",
        )
        self.assertEqual(
            self.rig.bones["sword_right_pitch"].parent, "sword_right_roll",
            "期望：两层静态腕角 group 嵌套 roll→pitch；实际层级被打乱",
        )

    def test_the_grip_lands_exactly_in_the_palm(self):
        """静止姿下，剑的握把点世界坐标 == 手心 HAND_REST。"""
        M = self.world["sword_right_pitch"]
        offset = np.asarray(H.HAND_REST["right"], float) - np.asarray(GEN.SWORD_GRIP_PX, float)
        grip_world = M[:3, :3] @ (BLOCK_CENTRE + offset) + M[:3, 3]
        np.testing.assert_allclose(
            grip_world, np.asarray(H.HAND_REST["right"], float), atol=1e-6,
            err_msg=f"期望：握把落在手心 {H.HAND_REST['right']}；实际 {grip_world}",
        )

    def test_the_grip_sits_inside_the_right_arm_cuboid(self):
        """再从人体那侧独立验一遍：握把点必须落在右臂 cuboid 的包围盒内。

        上一条只证明"剑对齐到了我们声明的手心常量"，这条证明那个常量真的在手里——
        两条一起才排掉"常量本身写歪了"。
        """
        box = RP.PARTS["rightArm"]["box"]
        M = self.world["sword_right_pitch"]
        offset = np.asarray(H.HAND_REST["right"], float) - np.asarray(GEN.SWORD_GRIP_PX, float)
        grip = M[:3, :3] @ (BLOCK_CENTRE + offset) + M[:3, 3]
        for i, axis in enumerate("xyz"):
            self.assertGreaterEqual(
                grip[i], box[0][i] - 0.5,
                f"期望：握把点在右臂 cuboid 的 {axis} 范围内；实际 {grip[i]} < {box[0][i]}")
            self.assertLessEqual(
                grip[i], box[1][i] + 0.5,
                f"期望：握把点在右臂 cuboid 的 {axis} 范围内；实际 {grip[i]} > {box[1][i]}")

    def test_the_blade_is_perpendicular_to_the_forearm(self):
        """握姿的全部要求：**剑身垂直于小臂**。

        上一版是 180°——剑尖顺着小臂朝下、和前臂完全平行，看着像"从拳头里捅出来一根
        骨头"，握不住。这一条把它焊死：夹角必须在 90°±5° 内。
        """
        blade = _blade_dir(self.world["sword_right_pitch"])
        forearm = self.world["arm_right_bend"][:3, :3] @ np.array([0.0, -1.0, 0.0])
        cos = float(np.clip(blade @ forearm, -1.0, 1.0))
        angle = float(np.degrees(np.arccos(cos)))
        self.assertAlmostEqual(
            angle, 90.0, delta=5.0,
            msg=f"期望：剑身与小臂夹角 90°±5°（拳心合得拢缠绳握把）；实际 {angle:.1f}°"
                f"（cos={cos:+.3f}）。接近 0°/180° = 剑顺着小臂捅出来，握不住",
        )

    def test_the_blade_points_out_the_front_of_the_fist(self):
        """静止姿（手臂自然垂下）时剑平指身前 −Z，不是身后。

        `framing.LEGACY_FACING` 说模型正面是 −Z。指反了整套动作的弧面就镜像了，而
        单看夹角是发现不了的（±Z 都垂直于小臂）。
        """
        blade = _blade_dir(self.world["sword_right_pitch"])
        self.assertLess(
            blade[2], -0.9,
            f"期望：静止握姿剑身朝正前方 −Z（分量 < −0.9）；实际 {np.round(blade, 3)}",
        )

    def test_the_edges_face_up_and_down(self):
        """剑面朝左右、刃口朝上下——本剑 blade 在局部 X 上宽、Z 上薄，刃在 ±X。

        这一层由 `sword_right_roll` 的静态 90° 负责；去掉它剑就"躺平"了，正面看是
        一条 4px 宽的板子拍过来而不是一把剑。
        """
        M = self.world["sword_right_pitch"]
        edge = M[:3, :3] @ np.array([1.0, 0.0, 0.0])     # 局部 +X = 刃口方向
        self.assertGreater(
            abs(edge[1]), 0.9,
            f"期望：刃口轴基本竖直（|y| > 0.9），剑面朝左右；实际 {np.round(edge, 3)}",
        )


class AnimationsAreActuallyBakedTest(unittest.TestCase):
    """每条内嵌动画都得真有帧——第一版这里全是空的。"""

    @classmethod
    def setUpClass(cls):
        _elements, _outliner, cls.gmap, _atlas = GEN.build_geometry()
        cls.anims = {name: GEN.convert_animation(ANIM / f"{name}.json", cls.gmap)
                     for name in GEN.DEFAULT_ANIMS}

    def test_every_default_animation_has_bones_and_keyframes(self):
        for name, anim in self.anims.items():
            with self.subTest(anim=name):
                self.assertGreater(
                    anim["length"], 0.0,
                    f"期望：{name} 的时长 > 0；实际 {anim['length']}——多半是源动作没读到")
                self.assertGreater(
                    len(anim["animators"]), 0,
                    f"期望：{name} 至少有一条骨轨道；实际 0 条（Animate 模式里会是空的）")
                frames = sum(len(a["keyframes"]) for a in anim["animators"].values())
                self.assertGreater(
                    frames, 0, f"期望：{name} 至少有一个关键帧；实际 {frames}")

    def test_keyframes_use_the_blockbench_animator_schema(self):
        """`{"keyframes": [{"channel": ..., "data_points": [...]}]}`。

        写成 `{"rotation": [...], "position": [...]}` 的话 Blockbench 读得进文件但认不出
        轨道——正是第一版"打得开、没有帧"的形态。
        """
        legal = {"rotation", "position"}
        for name, anim in self.anims.items():
            for animator in anim["animators"].values():
                with self.subTest(anim=name, bone=animator["name"]):
                    self.assertIn("keyframes", animator,
                                  f"期望：animator 用 keyframes 列表；实际键为 "
                                  f"{sorted(animator)}")
                    self.assertEqual(animator["type"], "bone")
                    for kf in animator["keyframes"]:
                        self.assertIn(kf["channel"], legal,
                                      f"期望：channel ∈ {legal}；实际 {kf['channel']}")
                        self.assertEqual(
                            len(kf["data_points"]), 1,
                            "期望：每个关键帧一个 data_point；实际数量不对")
                        self.assertEqual(
                            sorted(kf["data_points"][0]), ["x", "y", "z"],
                            "期望：data_point 带 x/y/z 三个分量；实际缺分量")

    def test_every_source_tick_survives_into_the_bbmodel(self):
        """源 JSON 的每个 tick 都要在产出里找得到对应时刻，一帧都不许漏。"""
        for name, anim in self.anims.items():
            _n, _e, table = RP.anim_pose_table(ANIM / f"{name}.json")
            baked = {round(kf["time"], 4)
                     for a in anim["animators"].values() for kf in a["keyframes"]}
            for tick, _pose in table:
                want = round(tick / GEN.TICKS_PER_SECOND, 4)
                with self.subTest(anim=name, tick=tick):
                    self.assertIn(
                        want, baked,
                        f"期望：源 tick {tick}（{want}s）在 bbmodel 里有关键帧；"
                        f"实际只有 {sorted(baked)}")

    def test_lower_body_only_animations_get_a_stance_upper_body(self):
        """纯下半身动画补一份恒定的持剑架势，否则播起来剑会垂成零姿态。"""
        for name in ("lower_walk", "lower_sprint"):
            anim = self.anims[name]
            tracked = {a["name"] for a in anim["animators"].values()}
            with self.subTest(anim=name):
                for bone in ("arm_right_pitch", "arm_left_pitch", "torso_pitch", "head_pitch"):
                    self.assertIn(
                        bone, tracked,
                        f"期望：{name} 补上了 {bone} 的架势轨道；实际没有——"
                        f"手臂会停在零姿态，读作'握法变了'")

    def test_the_stance_filler_is_pinned_at_both_ends(self):
        """架势必须**首末各钉一帧且同值**——这是循环闭合，不是"架势不许动"。

        原先这条叫 `..._is_constant_and_pinned_at_both_ends`，顺带断言了整条轨道只有
        两帧。那一半是把「本剑的携行姿恰好是静态的」当成了机制的约束：`fill_upper_body`
        的注释只解释了**为什么钉首末两帧**（单帧在 loop 里会被插值回 defaultValue），
        从来没有一条理由要求中间不能有帧。采药刀那份补的就是**摆臂**（相位表形态，
        中间多一帧），机制层面完全成立。

        所以这里只保留真正的不变量：相位 0 与相位 1 必须逐轴同值。本剑仍然是静态携行
        姿（走路扛肩），由下面 `CarryStanceTest` 的两条按语义钉死，不靠"只有两帧"来兜。
        """
        anim = self.anims["lower_walk"]
        by_name = {a["name"]: a for a in anim["animators"].values()}
        kfs = [k for k in by_name["arm_right_pitch"]["keyframes"] if k["channel"] == "rotation"]
        self.assertGreaterEqual(len(kfs), 2, f"期望：架势轨道至少首末两帧；实际 {len(kfs)} 帧")
        times = sorted(k["time"] for k in kfs)
        self.assertAlmostEqual(times[0], 0.0, places=6,
                               msg="架势必须在相位 0 有帧，否则 findBefore 会返回 defaultValue")
        self.assertAlmostEqual(times[-1], anim["length"], places=6,
                               msg="架势必须在末帧有帧，否则循环中段被拖回 defaultValue")
        first = next(k for k in kfs if abs(k["time"]) < 1e-6)
        last = next(k for k in kfs if abs(k["time"] - anim["length"]) < 1e-6)
        self.assertEqual(
            tuple(first["data_points"][0].values()), tuple(last["data_points"][0].values()),
            "期望：首末两帧同值（循环闭合）；不同值的话每个周期会「啪」地跳一下")

    def test_upper_body_animations_are_not_filled(self):
        """已经有手臂轨道的动画不许被架势覆盖——那会把招式的起手姿态抹平。"""
        anim = self.anims["sword_spine_slash"]
        by_name = {a["name"]: a for a in anim["animators"].values()}
        kfs = [k for k in by_name["arm_right_pitch"]["keyframes"] if k["channel"] == "rotation"]
        _n, _e, table = RP.anim_pose_table(ANIM / "sword_spine_slash.json")
        self.assertEqual(
            len(kfs), len(table),
            f"期望：右臂轨道帧数 == 源 tick 数 {len(table)}；实际 {len(kfs)}——"
            f"多出来的多半是架势 filler 误伤")


class BladeTravelsWhereTheAnimationSaysTest(unittest.TestCase):
    """两条本剑专属动作的**剑尖轨迹**契约。

    姿态好不好看是人看图判的（round 2 人工闸门），这里只锁"能用数字说清、错了就是错"
    的那部分：斜斩必须真的斜（横向跨过中线）、竖斩必须真的竖（全程贴中线），以及重剑
    真的从头顶落到腰下。少了这层，任何一次手滑改角度都能把斜斩改成戳刺而没人发现。
    """

    @classmethod
    def setUpClass(cls):
        cls.rig = PoseRig(GEN.OUT_BB)

    def _track(self, anim_name):
        """→ {tick: {'tip','grip','lhand','blade'}}，世界坐标 px。"""
        _n, _e, table = RP.anim_pose_table(ANIM / f"{anim_name}.json")
        out = {}
        for tick, pose in table:
            W = self.rig.world(_rig_pose(pose))
            M, ML = W["sword_right_pitch"], W["arm_left_bend"]
            grip = _world(M, _sword_local(0.0))
            tip = _world(M, _sword_local(SWORD_LEN_PX))
            blade = (tip - grip) / np.linalg.norm(tip - grip)
            out[tick] = {
                "tip": tip, "grip": grip, "blade": blade,
                "lhand": _world(ML, np.asarray(H.HAND_REST["left"], float)),
                "offhand": _world(M, _sword_local(-3.0)),
            }
        return out

    def test_slash_starts_over_the_right_shoulder(self):
        """起手帧（用户手摆的那一帧）：剑尖在右后上方。"""
        t = self._track("sword_spine_slash")[0]["tip"]
        self.assertLess(t[0], -8.0, f"期望：剑尖在角色右侧（x < -8）；实际 x={t[0]:.1f}")
        self.assertGreater(t[1], 28.0, f"期望：剑尖高过头顶（y > 28）；实际 y={t[1]:.1f}")
        self.assertGreater(t[2], 8.0, f"期望：剑尖在身后（z > 8，−Z 是正面）；实际 z={t[2]:.1f}")

    def test_slash_really_crosses_the_body_diagonally(self):
        """斜斩之所以叫斜斩：剑尖横向从角色右侧跨到左侧，且跨度足够大。"""
        xs = [f["tip"][0] for f in self._track("sword_spine_slash").values()]
        self.assertLess(min(xs), -8.0, f"期望：轨迹最右端 x < -8；实际 {min(xs):.1f}")
        self.assertGreater(max(xs), +4.0, f"期望：轨迹最左端 x > +4（越过中线）；实际 {max(xs):.1f}")
        self.assertGreater(
            max(xs) - min(xs), 16.0,
            f"期望：横向跨度 > 16px 才看得出是斜的；实际 {max(xs) - min(xs):.1f}px——"
            "跨度塌了就退化成正面竖劈，和 sword_spine_cleave 撞脸",
        )

    def test_cleave_stays_on_the_centre_line(self):
        """竖斩之所以叫竖斩：剑尖全程贴中线，不许跑成斜的。"""
        xs = [f["tip"][0] for f in self._track("sword_spine_cleave").values()]
        self.assertLess(
            max(abs(x) for x in xs), 8.0,
            f"期望：剑尖全程 |x| < 8（正中竖劈）；实际最大 {max(abs(x) for x in xs):.1f}px",
        )
        self.assertLess(
            max(xs) - min(xs), 12.0,
            f"期望：横向跨度 < 12px；实际 {max(xs) - min(xs):.1f}px——那已经是斜斩了",
        )

    def test_both_heavy_swings_actually_come_down(self):
        """重剑得真的从头顶落到腰下，不能全程在胸前比划。"""
        for name in ("sword_spine_slash", "sword_spine_cleave"):
            with self.subTest(anim=name):
                ys = [f["tip"][1] for f in self._track(name).values()]
                self.assertGreater(max(ys), 40.0, f"{name}: 期望最高点过头顶（y > 40）；实际 {max(ys):.1f}")
                self.assertLess(min(ys), 16.0, f"{name}: 期望最低点落到腰下（y < 16）；实际 {min(ys):.1f}")

    def test_cleave_guard_frames_really_hold_the_grip_with_both_hands(self):
        """双手握不是说说：guard / 蓄力 / 收势帧的左手掌心必须贴在柄尾一个手宽内。

        参照组是现网通用 `sword_cleave`——它的左手全程离柄 8~16px，从来没真握上过。
        骨架限制（两肩 10px、单臂 8px，双手只能在胸前中线会合）见生成器 docstring，
        所以只对够得着的那几帧设门，劈砍段左手脱手是允许的。
        """
        track = self._track("sword_spine_cleave")
        for tick in (0, 4, 20):
            with self.subTest(tick=tick):
                f = track[tick]
                gap = float(np.linalg.norm(f["lhand"] - f["offhand"]))
                self.assertLess(
                    gap, 5.0,
                    f"tick {tick}: 期望左手离柄尾 < 5px（一个手宽，看着就是双手握）；"
                    f"实际 {gap:.1f}px",
                )

    def test_the_shared_sword_cleave_is_left_alone(self):
        """本剑专属动作不许顶掉共享的 `sword_cleave`——那条 server 的剑基础招在用。"""
        self.assertNotIn(
            "sword_cleave", GEN.DEFAULT_ANIMS,
            "期望：脊骨剑用自己的 sword_spine_cleave；实际又把共享的 sword_cleave 拉了进来",
        )
        self.assertIn("sword_spine_cleave", GEN.DEFAULT_ANIMS)
        self.assertTrue(
            (ANIM / "sword_cleave.json").exists(),
            "期望：共享的 sword_cleave.json 原地不动；实际不见了",
        )


class SwingHorizIsTheOtherDiagonalTest(unittest.TestCase):
    """`sword_swing_horiz` 按本剑口径重做后的契约：**另一条对角线** + 同量级顿挫。

    两条重斜斩要能被玩家从远处一眼分开，所以这里锁的是"走哪条对角线"这个可判定的
    事实，而不是"好不好看"（那是 round 2 人工闸门的事）。
    """

    @classmethod
    def setUpClass(cls):
        cls.rig = PoseRig(GEN.OUT_BB)

    def _track(self, anim_name):
        _n, _e, table = RP.anim_pose_table(ANIM / f"{anim_name}.json")
        out = {}
        for tick, pose in table:
            W = self.rig.world(_rig_pose(pose))
            M = W["sword_right_pitch"]
            grip = _world(M, _sword_local(0.0))
            tip = _world(M, _sword_local(SWORD_LEN_PX))
            shoulder = _world(W["arm_right_pitch"], np.asarray(RP.PARTS["rightArm"]["pivot"], float))
            out[tick] = {"tip": tip, "grip": grip, "shoulder": shoulder}
        return out

    def test_swing_horiz_runs_the_opposite_diagonal_from_the_slash(self):
        """起手在角色左上、落点在右下——正好和 `sword_spine_slash` 相反。

        两条动作要是同向，玩家就只看得出"又劈了一下"，分不出是哪一招。
        """
        swing = self._track("sword_swing_horiz")
        slash = self._track("sword_spine_slash")

        def sweep(track):
            ticks = sorted(track)
            return track[ticks[0]]["tip"][0], min(f["tip"][0] for f in track.values())

        start_x, end_x = swing[0]["tip"][0], min(f["tip"][0] for f in swing.values())
        self.assertGreater(
            start_x, +8.0,
            f"期望：swing_horiz 起手剑尖在角色左侧（x > +8）；实际 x={start_x:.1f}")
        self.assertLess(
            end_x, -8.0,
            f"期望：swing_horiz 落到角色右侧（x < -8）；实际最右 {end_x:.1f}")

        swing_dir = np.sign(end_x - start_x)
        slash_start = slash[0]["tip"][0]
        slash_dir = np.sign(max(f["tip"][0] for f in slash.values()) - slash_start)
        self.assertEqual(
            swing_dir, -slash_dir,
            f"期望：两条重斜斩走相反的对角线（swing {swing_dir:+.0f} / slash {slash_dir:+.0f}）；"
            "实际同向——远处看会撞脸")

    def test_swing_horiz_tip_height_is_a_valley_not_an_arch(self):
        """剑尖高度必须是 ∪（先降后升），不许拱出一个高于起手的峰。

        「从肩部斜斩下来」的意思就是剑**已经在肩上**，直接砍，不再多举一次。第一版在
        中段插了一拍"转刃过顶"，高度走成 32.7 → 33.8 → 42.3 → 29.8 → 12.3，先拱起
        再落下（∩），用户看图当场指出来了。

        反过来 `sword_spine_slash` **该有**过顶峰——用户对它的要求原话是"手臂抬起然后
        ……斜斩"。两条的形状差异是有意的，所以这条门只管 swing_horiz，下面
        `test_the_slash_and_cleave_do_swing_overhead` 反向锁住另外两条。
        """
        track = self._track("sword_swing_horiz")
        ticks = sorted(track)
        ys = [track[t]["tip"][1] for t in ticks]
        bottom = int(np.argmin(ys))

        self.assertEqual(
            ys.index(max(ys)), 0,
            f"期望：全程最高点就是起手帧（∪ 形）；实际最高点在 tick {ticks[ys.index(max(ys))]}"
            f"（高度曲线 {[round(y, 1) for y in ys]}）——中途拱起来就是 ∩ 了")
        self.assertLess(min(ys), 16.0, f"期望：谷底落到腰下（y < 16）；实际 {min(ys):.1f}")
        self.assertTrue(
            all(ys[i] >= ys[i + 1] - 0.5 for i in range(bottom)),
            f"期望：谷底之前逐帧下沉（允许 0.5px 抖动）；实际 {[round(y, 1) for y in ys]}")
        self.assertGreater(
            ys[-1], min(ys) + 4.0,
            f"期望：收势把剑尖抬回来（比谷底高 4px 以上）；实际谷底 {min(ys):.1f} → "
            f"收势 {ys[-1]:.1f}，∪ 的右半边没了")

    def test_the_slash_and_cleave_do_swing_overhead(self):
        """反向锁：另外两条**要**举过头顶——形状差异是有意的，不是漏改。

        没有这条，把三条动作一起压平成 ∪ 也能全绿，"两招看得出区别"就守不住了。
        """
        for name in ("sword_spine_slash", "sword_spine_cleave"):
            with self.subTest(anim=name):
                ys = [f["tip"][1] for f in self._track(name).values()]
                self.assertGreater(
                    max(ys), 40.0,
                    f"{name}: 期望剑尖举过头顶（y > 40，用户原话是'手臂抬起'）；"
                    f"实际最高只有 {max(ys):.1f}")

    def test_the_strike_frame_actually_reaches_out(self):
        """发力帧手臂要伸出去。

        握姿垂直后剑尖离肩最远只有 `sqrt(臂长² + 刃长²) ≈ 25.7px`（剑身⊥小臂，两段
        勾股而不是相加）。手臂缩着劈 = 剑尖离肩 20px 出头，看着像在胸前比划；这条门
        要求斩入~发力段至少有一帧伸到 22px 以上。
        """
        armlen = float(np.linalg.norm(
            np.asarray(RP.PARTS["rightArm"]["pivot"], float) - HAND_R))
        envelope = float(np.hypot(armlen, SWORD_LEN_PX))
        for name in ("sword_swing_horiz", "sword_spine_slash"):
            with self.subTest(anim=name):
                reach = [float(np.linalg.norm(f["tip"] - f["shoulder"]))
                         for f in self._track(name).values()]
                self.assertLessEqual(
                    max(reach), envelope + 0.5,
                    f"{name}: 期望剑尖不超出 {envelope:.1f}px 工作空间；实际 {max(reach):.1f}")
                self.assertGreater(
                    max(reach), 22.0,
                    f"{name}: 期望发力段剑尖伸到离肩 > 22px；实际最远只有 {max(reach):.1f}px")

    def test_the_hitch_stays_the_size_the_user_signed_off(self):
        """顿挫（IMPACT → RIP 那一小提）保持 `sword_spine_slash` 的量级。

        用户验收 round 2 时点名这个量"刚刚好"，别加大也别抹平：抹平就没有"倒钩咬住
        肉再撕"的读感，加大就变成第二次挥砍。
        """
        def hitch(name, impact, rip):
            track = self._track(name)
            return float(np.linalg.norm(track[rip]["tip"] - track[impact]["tip"]))

        ref = hitch("sword_spine_slash", 10, 13)
        got = hitch("sword_swing_horiz", 10, 13)
        self.assertGreater(
            got, 0.6 * ref,
            f"期望：顿挫位移与 spine_slash（{ref:.1f}px）同量级；实际只有 {got:.1f}px——"
            "小到看不出就等于没有这一拍")
        self.assertLess(
            got, 2.2 * ref,
            f"期望：顿挫位移不超过 spine_slash（{ref:.1f}px）的 2.2 倍；实际 {got:.1f}px——"
            "这么大已经是第二次挥砍了")

    def test_swing_horiz_is_smoother_than_the_version_it_replaced(self):
        """关节角二阶差分：整条弧要连贯，不能"抽一下"。

        标定：`sword_spine_slash`（用户认可）12.7 °/t²，`sword_spine_cleave` 36.3，
        被替换掉的旧 `sword_swing_horiz` 是 49.7——那个数字就是抽动的来源。
        """
        _n, _e, table = RP.anim_pose_table(ANIM / "sword_swing_horiz.json")
        ticks = [t for t, _ in table]
        keys = ("pitch", "yaw", "roll", "bend")
        X = np.array([[float(pose["rightArm"].get(k, 0.0)) for k in keys] for _, pose in table])
        worst = max(
            float(np.max(np.abs((X[i + 1] - X[i]) / (ticks[i + 1] - ticks[i])
                                - (X[i] - X[i - 1]) / (ticks[i] - ticks[i - 1]))))
            for i in range(1, len(ticks) - 1)
        )
        self.assertLess(
            worst, 25.0,
            f"期望：右臂二阶差分 < 25 °/t²（旧版 49.7 是抽动的来源）；实际 {worst:.1f}")


class CarryStanceTest(unittest.TestCase):
    """纯下半身动画补的**持剑架势**：走/跑两个握法 + 只活在预览里。

    用户 2026-08-30 拍板：走路扛肩（省力携行），冲刺横持（双手压住剑身）。两条必须
    真的不一样——共用一份就等于把这条区分丢了。
    """

    @classmethod
    def setUpClass(cls):
        cls.rig = PoseRig(GEN.OUT_BB)

    def _world(self, name):
        return self.rig.world(_rig_pose(dict(GEN.STANCE_POSES[name])))

    @staticmethod
    def _grip_gap(world):
        M = world["sword_right_pitch"]
        offhand = _world(M, _sword_local(-3.0))
        lhand = _world(world["arm_left_bend"], np.asarray(H.HAND_REST["left"], float))
        return float(np.linalg.norm(lhand - offhand))

    def test_sprint_carries_the_blade_across_the_body_with_both_hands(self):
        """冲刺架势：剑横在身前、剑尖指向角色右前方，两只手都在柄上。

        参照组是通用 `sword_cleave` 的 8~16px —— 那是"看起来像但其实没握上"。
        """
        W = self._world("lower_sprint")
        blade = _blade_dir(W["sword_right_pitch"])
        self.assertLess(blade[0], -0.7,
                        f"期望：剑尖指向角色右侧（x 分量 < -0.7）；实际 {np.round(blade, 2)}")
        self.assertLess(abs(blade[1]), 0.6,
                        f"期望：剑身大体水平（|y| < 0.6）；实际 {np.round(blade, 2)}")
        self.assertLess(self._grip_gap(W), 3.0,
                        f"期望：左手离柄尾 < 3px（真双手握）；实际 {self._grip_gap(W):.1f}px")

    def test_walk_shoulders_the_blade_one_handed(self):
        """行走架势：剑扛在右肩后上方，左手是自由的——省力的携行姿态。"""
        W = self._world("lower_walk")
        tip = _world(W["sword_right_pitch"], _sword_local(SWORD_LEN_PX))
        self.assertGreater(tip[1], 28.0, f"期望：剑尖高过肩（y > 28）；实际 {tip[1]:.1f}")
        self.assertGreater(tip[2], 8.0, f"期望：剑尖在身后（z > 8，−Z 是正面）；实际 {tip[2]:.1f}")
        self.assertGreater(
            self._grip_gap(W), 8.0,
            f"期望：扛肩是单手，左手放开（离柄 > 8px）；实际 {self._grip_gap(W):.1f}px")

    def test_the_two_gaits_do_not_share_one_stance(self):
        """走和跑必须是两个握法——共用一份就把用户拍板的区分丢了。"""
        walk = _world(self._world("lower_walk")["sword_right_pitch"], _sword_local(SWORD_LEN_PX))
        sprint = _world(self._world("lower_sprint")["sword_right_pitch"], _sword_local(SWORD_LEN_PX))
        self.assertGreater(
            float(np.linalg.norm(walk - sprint)), 20.0,
            f"期望：两条步态的剑尖落点差 > 20px；实际 walk={np.round(walk,1)} "
            f"sprint={np.round(sprint,1)}")

    def test_the_stance_never_leaks_into_the_shipped_gait_json(self):
        """架势只补预览。`lower_*` 出料 JSON 必须仍然只写腿和 body。

        `LowerBodyGaitController` 把步态挂在 LOWER_BODY 通道，上半身靠
        PlayerAnimator 的"无关键帧就原样透传"交给招式动画；一旦架势漏进 JSON，
        跑动就会把玩家正在放的招式上半身抹平。
        """
        import json as _json

        for name in ("lower_walk", "lower_sprint"):
            with self.subTest(anim=name):
                moves = _json.loads((ANIM / f"{name}.json").read_text())["emote"]["moves"]
                leaked = {k for m in moves for k in m} - {"tick", "easing", "body",
                                                          "leftLeg", "rightLeg"}
                self.assertEqual(
                    leaked, set(),
                    f"期望：{name}.json 只写 body/双腿；实际漏进了 {sorted(leaked)}——"
                    "上半身会被步态踩掉")


class BorrowedSwordMovesRebuiltForThisGripTest(unittest.TestCase):
    """parry / infuse 按垂直握姿重做后的契约。

    两条都是用户手摆首尾、我补中段，所以这里锁的是"中段有没有真的把两端连成那个
    动作"——能用数字说清、错了就是错的那部分。

    `sword_thrust` 也重做过一版，被用户判定"做不好"撤回了：垂直握姿下剑尖恒在以肩
    为心 21~25.7px 的球面上，把手往前推不会把剑尖往前送（剑身指正前时肘从伸直折到
    88° 也只拉回 4px），行程只能靠"撤刃到侧面再转回正前"挣，读感不像刺。共享的
    `sword_thrust.json` 已还原成重做前的版本，这里因此没有它的门。
    """

    @classmethod
    def setUpClass(cls):
        cls.rig = PoseRig(GEN.OUT_BB)

    def _track(self, anim_name):
        _n, _e, table = RP.anim_pose_table(ANIM / f"{anim_name}.json")
        out = {}
        for tick, pose in table:
            W = self.rig.world(_rig_pose(pose))
            M = W["sword_right_pitch"]
            grip = _world(M, _sword_local(0.0))
            tip = _world(M, _sword_local(SWORD_LEN_PX))
            blade = (tip - grip) / np.linalg.norm(tip - grip)
            lhand = _world(W["arm_left_bend"], np.asarray(H.HAND_REST["left"], float))
            v = lhand - grip
            along = float(v @ blade)
            out[tick] = {
                "tip": tip, "grip": grip, "blade": blade, "lhand": lhand,
                "offhand": _world(M, _sword_local(-3.0)),
                "shoulder": _world(W["arm_right_pitch"],
                                   np.asarray(RP.PARTS["rightArm"]["pivot"], float)),
                "along": along,
                "perp": float(np.linalg.norm(v - along * blade)),
            }
        return out

    def test_parry_raises_the_block_and_locks_both_hands_on(self):
        """架格：剑要真的抬起来，且 cast 完成帧起两只手都在柄上。"""
        track = self._track("sword_parry")
        ticks = sorted(track)
        self.assertGreater(
            track[ticks[-1]]["tip"][1] - track[0]["tip"][1], 8.0,
            f"期望：架格把剑尖抬高 8px 以上；实际 "
            f"{track[ticks[-1]]['tip'][1] - track[0]['tip'][1]:.1f}px")
        for tick in (4, 6, 8, 10):
            with self.subTest(tick=tick):
                gap = float(np.linalg.norm(track[tick]["lhand"] - track[tick]["offhand"]))
                self.assertLess(
                    gap, 5.0,
                    f"tick {tick}: 期望左掌扣在柄上（离柄尾 < 5px）；实际 {gap:.1f}px")

    def test_parry_actually_snaps_outward_before_settling(self):
        """弹开那一下要看得见：strike 段末把剑尖顶出去，再回落稳住。"""
        track = self._track("sword_parry")
        out = float(np.linalg.norm(track[6]["tip"] - track[4]["tip"]))
        back = float(np.linalg.norm(track[10]["tip"] - track[6]["tip"]))
        self.assertGreater(out, 2.0, f"期望：4→6 外推 > 2px；实际 {out:.1f}px")
        self.assertGreater(back, 1.0, f"期望：6→10 回落 > 1px；实际 {back:.1f}px——"
                                      "顶出去不收就不是弹开，是慢慢举起来")

    def test_infuse_is_a_closed_loop(self):
        """循环动画首尾必须逐轴同值。

        用户手摆的 t0 / t28 是**抚刃的近端与远端**，不是循环的首尾——首尾不同值的
        循环在引擎里每周期抽一下（PlayerAnimator 那条"循环单帧衰减"的坑）。所以远端
        放 t14，近端占首尾。
        """
        _n, emote, table = RP.anim_pose_table(ANIM / "sword_infuse.json")
        self.assertTrue(emote["isLoop"], "期望：sword_infuse 是循环段")
        keys = dict(table)
        first, last = keys[0], keys[int(emote["endTick"])]
        for part in set(first) | set(last):
            with self.subTest(part=part):
                for axis in set(first.get(part, {})) | set(last.get(part, {})):
                    self.assertAlmostEqual(
                        float(first.get(part, {}).get(axis, 0.0)),
                        float(last.get(part, {}).get(axis, 0.0)), places=4,
                        msg=f"期望：{part}.{axis} 在 tick 0 与 endTick 同值；实际分叉了")

    def test_infuse_really_strokes_along_the_blade_and_comes_back(self):
        """左掌沿刃身推出去**再收回来**——去程回程都要有，且掌贴着刃走不是凭空挥。

        只锁"顶点不在首末帧"是不够的：把掌推出去、一路端着、最后一帧才弹回来，
        照样满足那条，可播起来就是"推出去 → 突然瞬移回柄"。所以这里同时锁顶点
        位置（中段 30%~70%）和两侧行程各自的幅度。
        """
        track = self._track("sword_infuse")
        ticks = sorted(track)
        along = [track[t]["along"] for t in ticks]
        peak = int(np.argmax(along))
        span = max(along) - min(along)
        readable = [f"t{t}:{a:.1f}" for t, a in zip(ticks, along)]

        self.assertGreater(
            span, 5.0,
            f"期望：左掌沿刃行程 > 5px；实际 {span:.1f}px（{readable}）")
        lo, hi = 0.3 * ticks[-1], 0.7 * ticks[-1]
        self.assertTrue(
            lo <= ticks[peak] <= hi,
            f"期望：推抚顶点落在整段的 30%~70%（tick {lo:.0f}~{hi:.0f}）；"
            f"实际在 tick {ticks[peak]}（{readable}）——顶点靠边 = 只有单程，"
            "循环回跳时掌会瞬移")
        self.assertGreater(
            along[peak] - along[0], 0.6 * span,
            f"期望：去程走满行程的 60% 以上；实际 {along[peak] - along[0]:.1f}/{span:.1f}px")
        self.assertGreater(
            along[peak] - along[-1], 0.6 * span,
            f"期望：回程收满行程的 60% 以上；实际 {along[peak] - along[-1]:.1f}/{span:.1f}px")
        # 顶点位置 + 两侧幅度都对，仍可能是"推出去一路端着、末帧才弹回柄"——那在引擎里
        # 是掌瞬移。所以再锁一条：掌沿刃的**速度**不许有突变，任何一段都不得超过
        # 匀速推抚（span / 半周期）的 2 倍（好版本实测 1.2 倍，注入的'末帧弹回'是 2.3 倍）。
        uniform = span / (ticks[-1] / 2.0)
        rates = [(abs(along[i + 1] - along[i]) / (ticks[i + 1] - ticks[i]))
                 for i in range(len(ticks) - 1)]
        self.assertLess(
            max(rates), 2.0 * uniform,
            f"期望：掌沿刃的速度不超过匀速推抚（{uniform:.2f}px/t）的 2.0 倍；"
            f"实际最快一段 {max(rates):.2f}px/t（逐段 {[round(r, 2) for r in rates]}）"
            "——某一段突然窜出去/缩回来 = 掌在瞬移")
        self.assertLess(
            max(track[t]["perp"] for t in ticks), 8.0,
            "期望：左掌全程贴着刃身走（离刃轴 < 8px）；实际掌飘到刃外了")


class BladeDoesNotPassThroughTheBodyTest(unittest.TestCase):
    """刃身不许插进自己身上——**逐插值 tick** 查，用**有向盒**查。

    两件事第一版都做错了，值得写下来：

    1. **只查关键帧不够。** 引擎播的是关键帧之间的线性插值，中间那一段才最容易插进
       身体。这里按 tick 重建插值姿态再查。
    2. **不能用轴对齐包围盒。** 部件一旦被旋转，AABB 就被撑大——右腿 pitch −6°/bend 22°
       时 AABB 在 z 上有 8.3px 深，而腿盒本身只有 4px。用 AABB 量 `sword_spine_cleave`
       得到"插进去 1.13px"，换成把点变换回部件局部系精确比对之后只剩 0.76px，差的那
       0.37px 全是虚胖。

    口径：**头不查**（手举到耳边时刃根本来就贴着头盒——头 ±4px 宽而肩在 x=−5，三条
    动作全都"命中"，那是 MC 骨架自带的贴合，玩家看不出来）；量程从剑格往外取
    （刃长的 30% 起），跳过握在拳头里的那一小段。
    """

    #: 每条动作允许的最小间隙（px；负 = 允许的插入深度）。
    #: `sword_spine_cleave` 的 −0.76px 是**已知且已量过**的遗留：重劈收势那两 tick，
    #: 刃根擦过右大腿顶端靠近髋点的位置。挪手臂能修但剑尖要漂 5~8px（会改掉用户正在
    #: 审的那条动作），挪腿没用（碰点就在髋枢轴附近，转腿不动它）。所以先按 −1.0 封顶
    #: 锁住"不许更糟"，等用户对 cleave 拍板后再动。
    TOLERANCE = {
        "sword_spine_slash": 0.0,
        "sword_spine_cleave": -1.0,
        "sword_swing_horiz": 0.0,
        "sword_parry": 0.0,
        "sword_infuse": 0.0,
    }
    BODY_PARTS = {"torso": "torso_bend", "rightLeg": "leg_right_bend",
                  "leftLeg": "leg_left_bend", "leftArm": "arm_left_bend"}

    @classmethod
    def setUpClass(cls):
        cls.rig = PoseRig(GEN.OUT_BB)

    @staticmethod
    def _lerp(a, b, f):
        if isinstance(a, dict):
            return {k: BladeDoesNotPassThroughTheBodyTest._lerp(a.get(k, 0.0), b.get(k, 0.0), f)
                    for k in set(a) | set(b)}
        return a + (b - a) * f

    def _interpolated(self, anim_name):
        """→ [(tick, pose)]，逐 tick 重建关键帧之间的线性插值。"""
        _n, emote, table = RP.anim_pose_table(ANIM / f"{anim_name}.json")
        keys = dict(table)
        ts = sorted(keys)
        for tick in range(ts[0], int(emote["endTick"]) + 1):
            lo = max([t for t in ts if t <= tick], default=ts[0])
            hi = min([t for t in ts if t >= tick], default=ts[-1])
            f = 0.0 if hi == lo else (tick - lo) / (hi - lo)
            yield tick, {p: self._lerp(keys[lo].get(p, {}), keys[hi].get(p, {}), f)
                         for p in set(keys[lo]) | set(keys[hi])}

    @staticmethod
    def _signed_gap(point, bone_matrix, box):
        """正 = 盒外间隙；负 = 插入深度。点先变换回部件局部系 —— OBB 的精确解。"""
        local = (np.linalg.inv(bone_matrix) @ np.append(point, 1.0))[:3]
        lo, hi = np.asarray(box[0], float), np.asarray(box[1], float)
        outside = np.maximum(np.maximum(lo - local, local - hi), 0.0)
        if np.any(outside > 0):
            return float(np.linalg.norm(outside))
        return -float(np.min(np.minimum(local - lo, hi - local)))

    def _worst_gap(self, anim_name):
        worst = (1e9, None, None)
        for tick, pose in self._interpolated(anim_name):
            W = self.rig.world(_rig_pose(pose))
            M = W["sword_right_pitch"]
            points = [_world(M, _sword_local(s * SWORD_LEN_PX))
                      for s in np.linspace(0.30, 1.0, 24)]
            for part, bone in self.BODY_PARTS.items():
                box = RP.PARTS[part]["box"]
                for p in points:
                    gap = self._signed_gap(p, W[bone], box)
                    if gap < worst[0]:
                        worst = (gap, tick, part)
        return worst

    def test_the_blade_clears_the_body_through_every_interpolated_tick(self):
        for name, floor in self.TOLERANCE.items():
            with self.subTest(anim=name):
                gap, tick, part = self._worst_gap(name)
                self.assertGreaterEqual(
                    gap, floor,
                    f"{name}: 期望刃身离身体不少于 {floor:+.2f}px；实际 {gap:+.2f}px "
                    f"@ tick {tick} 的 {part}（负 = 插进去的深度）")

    def test_the_new_swing_actually_clears_and_does_not_merely_scrape(self):
        """新做的这条要有真间隙，不能刚好卡在 0。"""
        gap, tick, part = self._worst_gap("sword_swing_horiz")
        self.assertGreater(
            gap, 0.5,
            f"期望：swing_horiz 全程刃身离身体 > 0.5px；实际 {gap:+.2f}px @ tick {tick} {part}")


class RigLimitsHoldAcrossTheWholeSetTest(unittest.TestCase):
    """终轮复验：本剑这套动作全都待在 MC 骨架的可用范围内。

    `anim_common.emit_json` 已经在出料时挡住了两条（`assert_joint_fold_is_anatomical`
    的关节反折、`_check_loop_closure` 的循环闭环），但**腿的摆幅没人管**——
    `docs/player-animation-conventions.md` 与 CLAUDE.md 都写着 MC 没有 IK、
    `leg.pitch` 超过 ~35~40° 腿根就和腹部脱开，强度要靠膝 `bend` 堆而不是加大 pitch。
    这条规矩此前只活在文档里，谁写超了都不会撞红。

    （被撤回的那版 `sword_thrust` 深弓步正好写在 −40 的边界上，是这条门的直接由来。）
    """

    LEG_PITCH_LIMIT = 40.0

    def test_no_frame_pushes_a_leg_past_the_hip_detach_angle(self):
        for name in GEN.DEFAULT_ANIMS:
            _n, _e, table = RP.anim_pose_table(ANIM / f"{name}.json")
            for tick, pose in table:
                for part in ("rightLeg", "leftLeg"):
                    pitch = float(pose.get(part, {}).get("pitch", 0.0))
                    with self.subTest(anim=name, tick=tick, part=part):
                        self.assertLessEqual(
                            abs(pitch), self.LEG_PITCH_LIMIT,
                            f"{name} tick {tick} 的 {part}.pitch = {pitch:+.1f}°，"
                            f"超过 {self.LEG_PITCH_LIMIT}° 上限——MC 没有 IK，"
                            "腿根会和腹部脱开；强度请堆膝 bend，别加大 pitch")

    def test_arm_bends_stay_anatomical_across_the_set(self):
        """手臂只许朝前折（axis=180 且 bend ≥ 0）——反折就是肘朝反方向弯。

        `emit_json` 出料时已经挡一道；这里从**产出的 JSON** 再验一遍，覆盖"绕过
        生成器直接改 JSON"那条路。
        """
        for name in GEN.DEFAULT_ANIMS:
            _n, _e, table = RP.anim_pose_table(ANIM / f"{name}.json")
            for tick, pose in table:
                for part in ("rightArm", "leftArm"):
                    axes = pose.get(part, {})
                    if "bend" not in axes:
                        continue
                    with self.subTest(anim=name, tick=tick, part=part):
                        self.assertGreaterEqual(
                            float(axes["bend"]), 0.0,
                            f"{name} tick {tick} {part}: bend 不许为负")
                        self.assertAlmostEqual(
                            float(axes.get("axis", 0.0)), 180.0, delta=1.0,
                            msg=f"{name} tick {tick} {part}: 手臂 bend 的 axis 必须是 180"
                                "（0 = 肘朝后折）")
