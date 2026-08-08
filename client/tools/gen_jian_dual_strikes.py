#!/usr/bin/env python3
"""双锏动画（上半身层）：架势 / 拔锏 / 下砸 / 轮扫 / 腰转交叉斩。

上下分离的上半边。**只写 rightArm / leftArm / torso / head + body 的 y,z**，绝不写
leg——腿归下半身步态层（lower_walk/jog/sprint/dash），写了就把步态踩掉。

body 只放 y（重心沉坠）与 z（前冲）：body.pitch 是步态层的跑姿前倾，上半身招式不接管；
而 §13 #4 要求发力招必须有躯干拧转 + 身体位移，重兵器的沉坠与前冲由招式承担最合理。
招式 priority(1000) > 步态(500)，招式期间压过步态起伏正是要的观感。

**架势**（参考实拍：一高一低分持）：右臂高举、左臂低位内收，两把锏的**尖端在身前
汇聚成一点**，眼睛→锏尖是一条下斜线（瞄住对手的那种指向感）。要点是锏沿小臂走，
所以"抬高手臂"不等于"锏朝上"——得靠肘深弯（bend≈100）+ bend 朝前（axis=0）把小臂
折回前下方。所有招式的 tick 0 与末帧都收在这个架势上，连招之间不会出现"先回垂手再
起手"的廉价过渡。

**重量感怎么做的**（双锏是钝器，不能像挥木棍）：
1. 蓄力段占全长一半以上，爆发段压到 3-4 tick——慢起快落才有质量
2. 蓄满处留 1-2 tick 停顿（肌肉锁住重物的那一下）
3. 打击到位后不立刻回，先给惯性过冲帧：武器继续走、躯干被拖过头
4. 过冲之后有回弹震荡（反向小幅），再慢慢收——不是线性归位
5. 重心 body.y 全程参与：蓄力下沉、发力蹬起、落点再沉

**幅度纪律**：躯干拧转给到 ~35° 就够读出发力，再大就从"拧腰"变成"扭麻花"。
2026-08-06 收过一次：torso.yaw 曾到 ±58、交叉臂 yaw 到 ±64，观感别扭，
统一按 torso.yaw×0.58 / torso.pitch×0.72 / 大幅 arm.yaw×0.72 收敛。

符号（实测确认，见 scripts/models/render_bend_matrix.png 与 conventions §12）：
    arm.pitch  < 0 → 前抬（-85 前平举，-160 高举过头）；> 0 → 向身后垂
    arm.yaw          在 pitch 之后的坐标系里转 → 前伸的手臂左右扫。右臂 yaw>0 往身体
                     外侧走、yaw<0 才过中线（左臂相反）——交叉斩、横扫过中线、探向对侧
                     腰全靠这个符号，写反就成了双臂张开
    arm.bend   > 0 → 肘弯（打击瞬间趋近 0 = 打直）
    torso.pitch> 0 前压 / torso.yaw > 0 向右拧；head.pitch > 0 低头
    body.y     > 0 → 下沉

用法:
    python3 client/tools/gen_jian_dual_strikes.py
    python3 scripts/models/render_player_pose.py --anim <json> --with-jian --yaw 145
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import anim_common as AC  # noqa: E402

UPPER_PARTS = {"rightArm", "leftArm", "torso", "head", "body"}
BODY_AXES_ALLOWED = {"y", "z"}


def assert_upper_only(pose_table, name: str) -> None:
    """分身契约：写到 leg 会踩掉步态；body 只放沉坠与前冲。"""
    bad_parts, bad_body = set(), set()
    for pose in pose_table.values():
        for part, axes in pose.items():
            if part in AC.RESERVED_KEYS:
                continue
            if part not in UPPER_PARTS:
                bad_parts.add(part)
            elif part == "body":
                bad_body |= set(axes) - BODY_AXES_ALLOWED
    if bad_parts:
        raise AssertionError(f"{name}: 上半身动画不得写 {sorted(bad_parts)}（腿归下半身步态层）")
    if bad_body:
        raise AssertionError(
            f"{name}: body 只允许 {sorted(BODY_AXES_ALLOWED)}（pitch 是步态层的跑姿前倾），"
            f"实际写了 {sorted(bad_body)}")


def stance(easing="INOUTSINE", **over):
    """高低分持架势：右手高举过头，左手垂在身侧偏后，躯干侧身、头转向正前。"""
    pose = {
        "easing": easing,
        # 右臂高举但【肘深弯 + bend 朝前(axis=0)】把小臂折回前下方，锏尖才朝前而不是戳天；
        # 左臂低位、小臂折成水平前指。两锏尖在身前约 25px 处汇聚（间距 ~6px），
        # 眼→锏尖成 25° 下斜线——参数由 scripts 数值搜索定出，不是拍脑袋。
        "rightArm": dict(pitch=-170, yaw=-8, roll=+10, bend=100, axis=0),
        "leftArm": dict(pitch=+5, yaw=+10, roll=-8, bend=90, axis=180),
        "torso": dict(pitch=-1.44, yaw=+8.12),
        "head": dict(pitch=+6, yaw=-9.28),
        "body": dict(y=0.0, z=0.0),
    }
    for part, axes in over.items():
        pose[part] = {**pose.get(part, {}), **axes}
    return pose


# ── 架势（循环 idle）─────────────────────────────────────────────────────
def stance_pose():
    """40 tick 呼吸循环：高举的锏有下坠回提，垂手的锏微微晃——静止时也要有重量。"""
    return {
        0: stance(),
        10: stance(rightArm=dict(pitch=-166, bend=104), leftArm=dict(pitch=+8, bend=94),
                   torso=dict(pitch=0), body=dict(y=+0.012)),
        20: stance(rightArm=dict(pitch=-172, bend=97), leftArm=dict(pitch=+3, bend=87),
                   torso=dict(pitch=-2.16), body=dict(y=-0.008)),
        30: stance(rightArm=dict(pitch=-167, bend=103), leftArm=dict(pitch=+7, bend=93),
                   torso=dict(pitch=-0.72), body=dict(y=+0.010)),
        40: stance(),
    }


# ── 腰间拔锏 ─────────────────────────────────────────────────────────────
def draw_pose():
    """24 tick：垂手 → 双手交叉探向对侧腰 → 握住(停顿) → 猛拔外展 → 惯性外甩 → 落架势。"""
    return {
        0: {   # 自然站立，双手空垂
            "easing": "INOUTSINE",
            "rightArm": dict(pitch=-6, yaw=-4, roll=+4, bend=8, axis=180),
            "leftArm": dict(pitch=-6, yaw=+4, roll=-4, bend=8, axis=180),
            "torso": dict(pitch=0, yaw=0),
            "head": dict(pitch=0, yaw=0),
            "body": dict(y=0.0, z=0.0),
        },
        6: {   # 双手交叉探向对侧腰（手过身体中线，肘大弯）
            "easing": "INOUTSINE",
            "rightArm": dict(pitch=-30, yaw=-30.24, roll=+26, bend=96, axis=180),
            "leftArm": dict(pitch=-30, yaw=+30.24, roll=-26, bend=96, axis=180),
            "torso": dict(pitch=+5.76, yaw=0),
            "head": dict(pitch=+10, yaw=0),
            "body": dict(y=+0.05, z=0.0),
        },
        9: {   # 握住：停顿一拍，重心再沉一点——手上是有分量的东西
            "easing": "INOUTSINE",
            "rightArm": dict(pitch=-32, yaw=-31.68, roll=+28, bend=100, axis=180),
            "leftArm": dict(pitch=-32, yaw=+31.68, roll=-28, bend=100, axis=180),
            "torso": dict(pitch=+7.2, yaw=0),
            "head": dict(pitch=+12, yaw=0),
            "body": dict(y=+0.07, z=0.0),
        },
        14: {  # 猛拔：双臂向两侧外上方抽出，肘迅速伸直，身体蹬起
            "easing": "OUTQUAD",
            "rightArm": dict(pitch=-88, yaw=+21.6, roll=+18, bend=18, axis=180),
            "leftArm": dict(pitch=-88, yaw=-21.6, roll=-18, bend=18, axis=180),
            "torso": dict(pitch=-7.2, yaw=0),
            "head": dict(pitch=-8, yaw=0),
            "body": dict(y=-0.05, z=0.0),
        },
        17: {  # 惯性外甩：锏被自身重量带着继续外张，肘反向微开
            "easing": "OUTQUAD",
            "rightArm": dict(pitch=-104, yaw=+30.24, roll=+24, bend=8, axis=180),
            "leftArm": dict(pitch=-104, yaw=-30.24, roll=-24, bend=8, axis=180),
            "torso": dict(pitch=-10.08, yaw=0),
            "head": dict(pitch=-10, yaw=0),
            "body": dict(y=-0.03, z=0.0),
        },
        24: stance(),   # 收成高低分持
    }


# ── 腰转蓄力 → 双臂交叉斩 ────────────────────────────────────────────────
def spin_cross_pose():
    """32 tick：架势 → 腰向右拧到底(蓄满停顿) → 猛回转 → 双臂交叉斩 → 惯性拖过头 → 回弹收架。"""
    return {
        0: stance(),
        6: stance(  # 腰开始向右拧，双臂随躯干收拢，重心下沉
            rightArm=dict(pitch=-140, yaw=+18.72, roll=+22, bend=52),
            leftArm=dict(pitch=+8, yaw=-18.72, roll=-20, bend=48),
            torso=dict(pitch=+2.88, yaw=+26.68), head=dict(pitch=0, yaw=-3.48),
            body=dict(y=+0.06, z=-0.04)),
        12: stance(  # 拧到极限并锁住一拍——蓄满的停顿是重量感的来源
            easing="INOUTSINE",
            rightArm=dict(pitch=-134, yaw=+23.04, roll=+26, bend=60),
            leftArm=dict(pitch=+4, yaw=-23.04, roll=-24, bend=54),
            torso=dict(pitch=+4.32, yaw=+33.64), head=dict(pitch=+1.44, yaw=-5.8),
            body=dict(y=+0.09, z=-0.06)),
        18: stance(  # 腰猛回转，双臂被甩开（发力肢单调朝交叉方向，不反向抽搐）
            easing="OUTQUAD",
            rightArm=dict(pitch=-96, yaw=-18.72, roll=0, bend=26),
            leftArm=dict(pitch=-52, yaw=+18.72, roll=0, bend=30),
            torso=dict(pitch=+1.44, yaw=-9.28), head=dict(pitch=0, yaw=+4.64),
            body=dict(y=-0.02, z=+0.10)),
        22: stance(  # 交叉斩到位：两臂在身前交叉成 X，肘打直
            easing="OUTQUAD",
            rightArm=dict(pitch=-86, yaw=-37.44, roll=-16, bend=8),
            leftArm=dict(pitch=-86, yaw=+37.44, roll=+16, bend=8),
            torso=dict(pitch=+7.2, yaw=-19.72), head=dict(pitch=+4.32, yaw=+8.12),
            body=dict(y=+0.02, z=+0.16)),
        25: stance(  # 惯性：锏继续走、躯干被拖过头，这一拍是"重"的关键
            easing="OUTQUAD",
            rightArm=dict(pitch=-78, yaw=-46.08, roll=-22, bend=5),
            leftArm=dict(pitch=-78, yaw=+46.08, roll=+22, bend=5),
            torso=dict(pitch=+9.36, yaw=-26.68), head=dict(pitch=+5.76, yaw=+10.44),
            body=dict(y=+0.05, z=+0.12)),
        28: stance(  # 回弹震荡：反向小幅，重物停不住又被拉回来
            rightArm=dict(pitch=-110, yaw=-24.48, roll=-4, bend=22),
            leftArm=dict(pitch=-40, yaw=+24.48, roll=+4, bend=26),
            torso=dict(pitch=+2.88, yaw=-10.44), head=dict(pitch=+1.44, yaw=+4.64),
            body=dict(y=+0.01, z=+0.04)),
        32: stance(),
    }


# ── 双锏下砸（重制：加长蓄力 + 落点震荡）────────────────────────────────
def smash_pose():
    """18 tick：架势 → 左手上提合到右手侧 → 双举顶点锁一拍 → 砸落 → 惯性 → 震荡 → 收架。"""
    return {
        0: stance(),
        6: stance(  # 左手上提与右手会合，双锏同举过头（慢起）
            rightArm=dict(pitch=-168, yaw=-6, roll=+10, bend=22),
            leftArm=dict(pitch=-168, yaw=+6, roll=-10, bend=22),
            torso=dict(pitch=-12.96, yaw=+2.32), head=dict(pitch=-11.52, yaw=-4.64),
            body=dict(y=-0.04, z=-0.05)),
        8: stance(  # 顶点锁一拍：举到头顶的重物停住那一瞬
            rightArm=dict(pitch=-172, yaw=-4, roll=+8, bend=18),
            leftArm=dict(pitch=-172, yaw=+4, roll=-8, bend=18),
            torso=dict(pitch=-15.12, yaw=+1.16), head=dict(pitch=-12.96, yaw=-3.48),
            body=dict(y=-0.06, z=-0.06)),
        11: stance(  # 砸落：肘打直、上身前压、重心砸下去
            easing="OUTQUAD",
            rightArm=dict(pitch=-16, yaw=-4, roll=+4, bend=5),
            leftArm=dict(pitch=-16, yaw=+4, roll=-4, bend=5),
            torso=dict(pitch=+18.72, yaw=0), head=dict(pitch=+15.84, yaw=0),
            body=dict(y=+0.10, z=+0.14)),
        13: stance(  # 惯性过冲：锏尖再往下扎半拍
            easing="OUTQUAD",
            rightArm=dict(pitch=-4, yaw=-2, roll=+2, bend=3),
            leftArm=dict(pitch=-4, yaw=+2, roll=-2, bend=3),
            torso=dict(pitch=+23.04, yaw=0), head=dict(pitch=+17.28, yaw=0),
            body=dict(y=+0.13, z=+0.10)),
        15: stance(  # 震荡回弹：砸到地面的反作用把手臂弹起一点
            rightArm=dict(pitch=-34, yaw=-4, roll=+6, bend=18),
            leftArm=dict(pitch=-34, yaw=+4, roll=-6, bend=18),
            torso=dict(pitch=+14.4, yaw=+2.32), head=dict(pitch=+10.08, yaw=-3.48),
            body=dict(y=+0.05, z=+0.06)),
        18: stance(),
    }


# ── 双锏轮扫（重制：每一扫都带拖尾）─────────────────────────────────────
def sweep_pose():
    """22 tick：架势 → 右锏 chamber → 右扫(带拖尾) → 左锏接力 → 左扫(带拖尾) → 收架。"""
    return {
        0: stance(),
        5: stance(  # 右锏落到肩后 chamber，腰反拧蓄力
            rightArm=dict(pitch=-108, yaw=+34.56, roll=+30, bend=76),
            leftArm=dict(pitch=-16, yaw=-15.84, roll=-18, bend=44),
            torso=dict(pitch=-1.44, yaw=+19.72), head=dict(pitch=-1.44, yaw=-2.32),
            body=dict(y=+0.05, z=-0.03)),
        8: stance(  # 右锏横扫过中线
            easing="OUTQUAD",
            rightArm=dict(pitch=-84, yaw=-28.8, roll=-12, bend=10),
            leftArm=dict(pitch=-30, yaw=+8, roll=-24, bend=62),
            torso=dict(pitch=+4.32, yaw=-13.92), head=dict(pitch=+2.88, yaw=+5.8),
            body=dict(y=-0.01, z=+0.10)),
        10: stance(  # 拖尾：右锏被自重带着多走一段
            easing="OUTQUAD",
            rightArm=dict(pitch=-80, yaw=-40.32, roll=-20, bend=6),
            leftArm=dict(pitch=-36, yaw=0, roll=-26, bend=70),
            torso=dict(pitch=+5.76, yaw=-19.72), head=dict(pitch=+3.6, yaw=+7.54),
            body=dict(y=+0.02, z=+0.07)),
        14: stance(  # 左锏接力 chamber（右锏顺势回收护中）
            rightArm=dict(pitch=-92, yaw=+18, roll=-4, bend=54),
            leftArm=dict(pitch=-104, yaw=-34.56, roll=-30, bend=76),
            torso=dict(pitch=-1.44, yaw=-19.72), head=dict(pitch=-1.44, yaw=+2.32),
            body=dict(y=+0.05, z=-0.03)),
        17: stance(  # 左锏横扫
            easing="OUTQUAD",
            rightArm=dict(pitch=-30, yaw=-8, roll=+24, bend=62),
            leftArm=dict(pitch=-84, yaw=+28.8, roll=+12, bend=10),
            torso=dict(pitch=+4.32, yaw=+13.92), head=dict(pitch=+2.88, yaw=-5.8),
            body=dict(y=-0.01, z=+0.10)),
        19: stance(  # 拖尾
            easing="OUTQUAD",
            rightArm=dict(pitch=-36, yaw=0, roll=+26, bend=70),
            leftArm=dict(pitch=-80, yaw=+40.32, roll=+20, bend=6),
            torso=dict(pitch=+5.76, yaw=+19.72), head=dict(pitch=+3.6, yaw=-7.54),
            body=dict(y=+0.02, z=+0.07)),
        22: stance(),
    }


ANIMS = [
    ("jian_stance_high_low", stance_pose, 40, True,
     "双锏·高低分持架势（循环）。右手高举过头、左手垂在身侧偏后，带呼吸与重物下坠回提。"),
    ("jian_draw_waist", draw_pose, 24, False,
     "双锏·腰间拔锏。双手交叉探向对侧腰 → 握住停顿 → 猛拔外展 → 惯性外甩 → 落高低分持架势。"),
    ("jian_waist_spin_cross", spin_cross_pose, 32, False,
     "双锏·腰转交叉斩。腰向右拧到底蓄满 → 猛回转 → 双臂身前交叉成 X → 惯性拖过头 → 回弹收架。"),
    ("jian_dual_smash", smash_pose, 18, False,
     "双锏·双手同举下砸。慢起举顶锁一拍 → 砸落 → 惯性下扎 → 落点震荡 → 收架。"),
    ("jian_dual_sweep", sweep_pose, 22, False,
     "双锏·左右轮扫两连。每一扫都带自重拖尾，腰反拧承担视觉位移。"),
]


def main():
    for name, builder, end_tick, is_loop, desc in ANIMS:
        pose = builder()
        assert_upper_only(pose, name)
        AC.emit_json(
            pose,
            name=name,
            description=desc,
            end_tick=end_tick,
            stop_tick=end_tick + 3,
            is_loop=is_loop,
            return_tick=0,
        )


if __name__ == "__main__":
    main()
