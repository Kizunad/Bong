#!/usr/bin/env python3
"""herb_harvest —— 凡铁采药刀俯身勾割灵草。

## 动作

持刀架势 → 俯身探刃、左手拨开草叶按住茎 → 刃贴着茎根切入 → 顺着鹰嘴的内弧**往回勾带**
（这才是这把刀真正的用法：钩住茎往身前拉着割，不是劈）→ 直起身，左手托着割下来的草。

14 tick。首帧 = 末帧 = `herb_knife_stance.GUARD`，连着采第二株时不必先垂手再举
（conventions §2.1）。

## 分段（easing 写在**段首帧**上——它管的是这一帧到下一帧那一段）

| tick | 段 | 干什么 |
|------|----|--------|
| 0  | guard      | 持刀架势，静态一眼能认出"手里有把干活的小刀" |
| 3  | 俯身入位   | 膝先屈到 22°/16°、躯干折到 16°、头看向草、左手前探拨叶 |
| 6  | 割入       | 躯干 26°，刃最低点落到世界 y 12.4、肩前 8.2px，进草区 |
| 8  | 勾带       | 腕外翻 36°→58°，刃沿鹰嘴内弧往身前上方拉 —— 末端过冲在这里 |
| 11 | 起身       | 躯干回 14°，左手托草，刀提到腰前 |
| 14 | guard      | 逐轴等于 tick 0 |

前倾封顶 26°：`hip_hinge` 只补得平水平那一半，竖直残差 `12(1-cosθ)` 在 26° 时是 1.21px，
再深髋缝就过门限了（`herb_knife_stance.HIP_SEAM_MAX = 1.40`）。

## 为什么右臂全程单调朝"下—前"走

conventions §2.3：发力肢从起手到撞击必须单调，反向蓄势只给躯干/头。所以刀不在中途
"先抬一下再砍"——起手架势本身就带着刀，t3 继续往下，t6 到位。真正的反向蓄势由躯干
（t0 前倾 8° → t3 16° → t6 26°）和头承担。

## 上一版错在哪（重做的由来）

1. **腰断**：`torso.pitch=+28~32` 而两条腿反向摆，髋缝实测 **6.61px**（按解剖锚点量；
   一条腿才 4px 宽）。torso 的枢轴在**脖子**不在腰，前倾必须把上半身整体前移
   `-12·sinθ` 才等价于"绕胯折"，见 `herb_knife_stance.hip_hinge`。
2. **左手甩到身后**：注释写"左臂探地按住灵草基底"，值却是 `pitch=+48`——臂的正 pitch
   是往**身后**摆。手根本不在草上。
3. **刀不在草上**：刃的最前点在**架势帧**（z=-9.9）比在割入帧（z=-7.6）还靠前——
   动作的方向是把刀往回收，不是探出去。
4. **收势收成立正**：末帧写成全零 vanilla neutral，采完一株整个人"啪"地站直，刀也不
   见了；连采两株中间要经过一次立正。
"""

import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "client" / "tools"))
from anim_common import emit_json  # noqa: E402
from herb_knife_stance import guard_pose, stance  # noqa: E402

POSE = {
    # ── guard：持刀架势 ────────────────────────────────────────────────
    0: guard_pose("INOUTSINE"),

    # ── 俯身入位：躯干折下去、头找草、左手前探拨叶 ────────────────────
    # 这一段**腿先动**（膝屈到 22°/16°），躯干只走三分之一——kinetic chain 的第一环，
    # 峰速落在 t1 附近。左臂 pitch 取负才是往身前伸（这是上一版最刺眼的错）。
    3: dict(
        easing="INQUAD",                       # 段首 IN 族 = 从静止加速冲向割入帧
        **stance(
            16.0, 8.0,
            head=dict(pitch=26.0, yaw=-8.0),
            right_arm=dict(pitch=-20.0, yaw=5.0, roll=28.0, bend=38.0),
            left_arm=dict(pitch=-34.0, yaw=-16.0, roll=14.0, bend=36.0),
            right_leg=dict(pitch=-14.0, yaw=8.0, bend=22.0),
            left_leg=dict(pitch=10.0, yaw=6.0, bend=16.0),
        ),
    ),

    # ── 割入（IMPACT）：刃最低点探进草区 ──────────────────────────────
    # 右臂这组是扫格子解出来的：刃最低落到世界 y 13.0、最前伸到肩前 8.5px，刃仰角
    # +16°（近水平），读作"刃贴着茎切进去"而不是"往地上戳"。躯干在这一段走完剩下的
    # 14°（峰速落在 t4~5），肩/肘跟着到位——腿→腰→肩肘的顺序就是这么错开的。
    6: dict(
        easing="OUTQUAD",                      # 割入后急刹，接勾带
        **stance(
            26.0, 2.0,
            head=dict(pitch=34.0, yaw=-4.0),
            right_arm=dict(pitch=-42.0, yaw=-40.0, roll=36.0, bend=12.0),
            left_arm=dict(pitch=-40.0, yaw=-20.0, roll=16.0, bend=42.0),
            right_leg=dict(pitch=-16.0, yaw=8.0, bend=26.0),
            left_leg=dict(pitch=11.0, yaw=6.0, bend=18.0),
        ),
    ),

    # ── 勾带：腕外翻，刃沿鹰嘴内弧往身前上方拉 ────────────────────────
    # 这把刀的招牌动作——鹰嘴是**钩住往回拉着割**的，不是劈的。腕的峰速落在这一段
    # （roll 40 → 58），是 kinetic chain 的最后一环，也是末端过冲。
    8: dict(
        easing="INOUTSINE",
        **stance(
            22.0, 6.0,
            head=dict(pitch=30.0, yaw=-6.0),
            right_arm=dict(pitch=-16.0, yaw=-26.0, roll=58.0, bend=48.0),
            left_arm=dict(pitch=-30.0, yaw=-14.0, roll=10.0, bend=48.0),
            right_leg=dict(pitch=-14.0, yaw=8.0, bend=22.0),
            left_leg=dict(pitch=10.0, yaw=6.0, bend=16.0),
        ),
    ),

    # ── 起身：左手托着割下来的草，刀提回腰前 ──────────────────────────
    11: dict(
        easing="INOUTSINE",
        **stance(
            14.0, 11.0,
            head=dict(pitch=18.0, yaw=-10.0),
            right_arm=dict(pitch=-10.0, yaw=10.0, roll=26.0, bend=46.0),
            left_arm=dict(pitch=-20.0, yaw=-4.0, roll=-2.0, bend=34.0),
            right_leg=dict(pitch=-8.0, yaw=9.0, bend=13.0),
            left_leg=dict(pitch=6.0, yaw=7.0, bend=9.0),
        ),
    ),

    # ── 回 guard ──────────────────────────────────────────────────────
    14: guard_pose("INOUTSINE"),
}

if __name__ == "__main__":
    emit_json(
        POSE,
        name="herb_harvest",
        description="凡铁采药刀俯身勾割灵草：探刃 → 贴茎切入 → 沿鹰嘴内弧回勾带起",
        end_tick=14,
        stop_tick=16,
        is_loop=False,
    )
