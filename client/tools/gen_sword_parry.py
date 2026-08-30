#!/usr/bin/env python3
"""sword_parry —— 双手斜举架格（异兽脊骨剑垂直握姿口径重做）。

cast_ticks=4 → endTick ∈ [8,12]，取 10（沿用，manifest 相位不变）。

时序（`client/src/test/resources/bong/anim_spec_manifests/sword_parry.json`）：
  anticipation 0→2   重心下沉、剑开始抬（easeOut 族 OUTQUAD，短促）
  strike       2→6   斜举架格到位（tick 4 = cast 完成 = 格挡定架，左手同时扣上柄）
                     + 4→6 外推弹开（deflect snap，剑尖再往外顶 3px）
  recovery     6→10  从外推回落、稳在架上（INOUTSINE）
endTick=10，stopTick=12，非循环。主打击轴：rightArm.pitch / rightArm.yaw /
torso.yaw。

## 用户手摆了头和尾，中段是解出来的

tick 0（低位待机，剑斜指左前上）与 tick 10（斜举架格、双手扣柄）是用户在 Blockbench
里摆的。中间四帧按剑尖弧线整条反解（大圆插值 + 二阶差分罚项）。

**这一招收势不回中立**：格挡窗口（`sword_basics` 的 `parry_window_ticks` 4~8 tick）
本来就要求架子端住，所以 recovery 段是"从外推回落到架上"，末帧仍是架格姿态——只有
body 轴显式归零（非循环动画的残值偏移只发生在走 MatrixStack 的 body 上）。

## 双手扣柄是量过的

t4 起左掌扣到柄上（离柄尾 4.0~4.7px，一个手宽内）。参照组是重做前的通用
`sword_cleave`：它的左手全程离柄 8~16px，从来没真握上过。骨架限制见
`gen_beast_spine_sword_player_anim` 的 docstring——两肩 10px、单臂 8px，双手只能在
胸前中线附近会合，架格帧正好落在够得着的区域里。

## 剑尖只能在半径 21~25.7px 的球面上

握姿是剑身⊥小臂，剑尖离肩的距离几乎只由肘弯决定（r=20→肘弯 75°，24→28°，
25.7→伸直）。用户 t10 的 r=25.1 会把肘顶到全直，架格看着发僵，这里收到 24.2
（剑尖挪不到 1px，肘留 21°）。
"""

from anim_common import emit_json

POSE = {
    0: dict(  # 低位待机：剑斜指左前上（用户手摆帧，反解回静态握姿）
        easing="INOUTSINE",
        body=dict(x=0.0, y=0.0, z=0.0),
        head=dict(pitch=-3),
        torso=dict(pitch=+3, yaw=+8),
        rightArm=dict(pitch=-7.8, yaw=-11.8, roll=-4.2, bend=26.1, axis=180),
        leftArm=dict(pitch=+10.5, yaw=+14, roll=-8, bend=28, axis=180),
        rightLeg=dict(pitch=+7, bend=10, axis=0),
        leftLeg=dict(pitch=-9, bend=12, axis=0),
    ),
    2: dict(  # 反应帧：重心下沉，剑开始往左上抬
        easing="OUTQUAD",
        body=dict(x=0.0, y=+0.03, z=-0.04),
        head=dict(pitch=-2),
        torso=dict(pitch=-2, yaw=-2),
        rightArm=dict(pitch=-22.5, yaw=-23.2, roll=-0.5, bend=25.5, axis=180),
        leftArm=dict(pitch=+10.5, yaw=+14, roll=-8, bend=28, axis=180),
        rightLeg=dict(pitch=+9, bend=13, axis=0),
        leftLeg=dict(pitch=-12, bend=16, axis=0),
    ),
    4: dict(  # cast 完成 = 架格定位：左掌扣上柄（离柄 4.7px），坐到最低
        easing="INQUAD",
        body=dict(x=0.0, y=+0.05, z=-0.08),
        head=dict(pitch=-6),
        torso=dict(pitch=-4, yaw=-6),
        rightArm=dict(pitch=-36.7, yaw=-36.7, roll=+4.0, bend=26.4, axis=180),
        leftArm=dict(pitch=-26.7, yaw=+39.5, roll=+36.1, bend=14.1, axis=180),
        rightLeg=dict(pitch=+11, bend=15, axis=0),
        leftLeg=dict(pitch=-14, bend=19, axis=0),
    ),
    6: dict(  # deflect snap：把对方的力往外顶（剑尖再外推 3px），腰反弹 -6→+2
        easing="OUTQUAD",
        body=dict(x=0.0, y=+0.02, z=-0.05),
        head=dict(pitch=-5),
        torso=dict(pitch=-2, yaw=+2),
        rightArm=dict(pitch=-41.6, yaw=-44.4, roll=+8.2, bend=22.8, axis=180),
        leftArm=dict(pitch=-30.8, yaw=+38.3, roll=+36.6, bend=6.0, axis=180),
        rightLeg=dict(pitch=+9, bend=12, axis=0),
        leftLeg=dict(pitch=-11, bend=15, axis=0),
    ),
    8: dict(  # 回落：外推的力卸掉，架子往回收
        easing="INOUTSINE",
        body=dict(x=0.0, y=+0.01, z=-0.02),
        head=dict(pitch=-4),
        torso=dict(pitch=0, yaw=+5),
        rightArm=dict(pitch=-45.6, yaw=-44.2, roll=+6.1, bend=23.0, axis=180),
        leftArm=dict(pitch=-34.4, yaw=+36.7, roll=+36.7, bend=6.0, axis=180),
        rightLeg=dict(pitch=+8, bend=11, axis=0),
        leftLeg=dict(pitch=-10, bend=13, axis=0),
    ),
    10: dict(  # 稳在架上（用户手摆帧）。body 轴显式归零，肢体保持架格姿态
        easing="INOUTSINE",
        body=dict(x=0.0, y=0.0, z=0.0),
        head=dict(pitch=-3),
        torso=dict(pitch=+3, yaw=+8),
        rightArm=dict(pitch=-46.1, yaw=-42.1, roll=+4.5, bend=21.3, axis=180),
        leftArm=dict(pitch=-35.3, yaw=+36.5, roll=+36.9, bend=6.0, axis=180),
        rightLeg=dict(pitch=+7, bend=10, axis=0),
        leftLeg=dict(pitch=-9, bend=12, axis=0),
    ),
}

DESCRIPTION = (
    "斜举架格 (sword_parry): 10-tick，低位待机 -> 沉重心抬剑 -> "
    "tick4 双手扣柄架格定位 -> 4~6 外推弹开 -> 回落稳在架上（不回中立，"
    "格挡窗口要求架子端住）。左掌全程离柄 4px 内。"
)

if __name__ == "__main__":
    emit_json(
        POSE,
        name="sword_parry",
        description=DESCRIPTION,
        end_tick=10,
        stop_tick=12,
        is_loop=False,
    )
