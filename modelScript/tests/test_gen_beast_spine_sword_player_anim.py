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

    def test_the_stance_filler_is_constant_and_pinned_at_both_ends(self):
        """补的是**恒定**姿态：首末各一帧同值，单帧在 loop 里会被插值回默认值。"""
        anim = self.anims["lower_walk"]
        by_name = {a["name"]: a for a in anim["animators"].values()}
        kfs = [k for k in by_name["arm_right_pitch"]["keyframes"] if k["channel"] == "rotation"]
        self.assertEqual(len(kfs), 2, f"期望：架势轨道首末各一帧；实际 {len(kfs)} 帧")
        times = sorted(k["time"] for k in kfs)
        self.assertAlmostEqual(times[0], 0.0, places=6)
        self.assertAlmostEqual(times[1], anim["length"], places=6)
        values = [tuple(k["data_points"][0].values()) for k in kfs]
        self.assertEqual(values[0], values[1],
                         f"期望：两帧同值（恒定架势）；实际 {values}")

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


if __name__ == "__main__":
    unittest.main()


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
