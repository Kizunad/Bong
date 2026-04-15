#!/usr/bin/env python3
"""meditate_sit — 席地盘腿打坐。

身法节奏（40 tick / 2s 循环）：tick 0/40 呼气, 10 中段, 20 吸气峰, 30 中段。

**v9 重来**: 参考 KosmX 官方 Emotecraft-emotes/sit.json — 社区通行做法是"纯旋转+无
bend"。v1-v8 一直纠结 bendy-lib 折小腿, 但游戏里 bend=175° 根本不出效果 (可能在极端
pitch+axis 组合下被静默忽略)。改抄 sit.json 的配方:
  - rightLeg pitch=-80° yaw=+25° roll=-15° (大腿前伸水平, 轻微外展 + 外翻 roll)
  - leftLeg 镜像
  - 再叠 bend=90° axis=0° 让小腿在 limb-local 朝下折回身前中线 (axis=0° =
    local X 轴, 即世界坐标里与大腿平面垂直的方向 → 折叠后小腿朝回身体内侧+下方)
  - body.y=-1.4 让角色整体下沉贴地 (坐姿)

**手**: pitch=-45° yaw=±30° bend=25° axis=180° — 直臂前伸稍下, 手自然落在膝头附近,
不追求精确贴合 (MC 没 IK, 精确不可能)。

**躯体**: torso.pitch=0 严格直立, 呼吸只走 body.y ±0.03 微沉浮 + head 垂目 +12°。

**v1-v8 血泪**: bend=175° 视觉无效; pitch=-90° + 大 yaw + bend=axis=±90° 从上方看
变 X 交叉。本次放弃几何推导, 直接抄社区成品模板。
"""
from anim_common import emit_json

_LEGS = dict(
    rightLeg=dict(pitch=-80, yaw=+25, roll=-15, bend=90, axis=0),
    leftLeg=dict(pitch=-80, yaw=-25, roll=+15, bend=90, axis=0),
)
_ARMS = dict(
    rightArm=dict(pitch=-45, yaw=+30, roll=0, bend=25, axis=180),
    leftArm=dict(pitch=-45, yaw=-30, roll=0, bend=25, axis=180),
)

POSE = {
    0: dict(  # 呼气最低 (= tick 40)
        easing="INOUTSINE",
        body=dict(y=0.08),
        head=dict(pitch=+12),
        torso=dict(pitch=0),
        **_ARMS,
        **_LEGS,
    ),
    10: dict(  # 吸气中段
        easing="INOUTSINE",
        body=dict(y=0.11),
        head=dict(pitch=+11),
        torso=dict(pitch=0),
        **_ARMS,
        **_LEGS,
    ),
    20: dict(  # 吸气最高
        easing="INOUTSINE",
        body=dict(y=0.13),
        head=dict(pitch=+10),
        torso=dict(pitch=0),
        **_ARMS,
        **_LEGS,
    ),
    30: dict(  # 呼气中段
        easing="INOUTSINE",
        body=dict(y=0.11),
        head=dict(pitch=+11),
        torso=dict(pitch=0),
        **_ARMS,
        **_LEGS,
    ),
    40: dict(  # 循环闭合 (= tick 0)
        easing="INOUTSINE",
        body=dict(y=0.08),
        head=dict(pitch=+12),
        torso=dict(pitch=0),
        **_ARMS,
        **_LEGS,
    ),
}

DESCRIPTION = (
    "v8 JSON 席地盘腿打坐: 40 tick 循环。腿 pitch=-90° yaw=±55° bend=175° axis=±90° "
    "(髋膝踝同水平 + 脚踝回到髋正下方, X 偏差 0.6 单位)。双手 pitch=-44° yaw=±44° "
    "直臂从 shoulder 伸到 knee 搭膝顶。torso.pitch=0 严格直立, 呼吸只走 body.y "
    "(-0.96 → -0.91 ±0.05), head 垂目 +12° 吸气峰值微抬 +10°。"
)

if __name__ == "__main__":
    emit_json(
        POSE,
        name="meditate_sit",
        description=DESCRIPTION,
        end_tick=40,
        stop_tick=43,
        is_loop=True,
    )
