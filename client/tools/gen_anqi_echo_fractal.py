#!/usr/bin/env python3
"""anqi_echo_fractal —— 诱饵分形：聚饵爆撒→织网余韵（P2 批次二后半，review 返工重做）。

cast_ticks=60 是**元数据**：`resolve_anqi_skill` 在 cast 起始 tick 立即结算
（与 armor_pierce 同一 resolver 通道）。返工前的 66t 版本把爆发顶点放在
tick 60，造成「效果已生效、动作三秒后才到」的表演/结算脱节（PR #1240
review blocker）。本版遵循「动画 strike 顶点对齐真实结算点」：24t 非循环，
顶点 = tick 4（紧贴 tick 0 结算），织网母题移入爆撒后的余韵编织。时长对拍
allowlist 条目保留（endTick=24 与 cast=60 元数据错配如实驻表）。

母题：聚饵爆撒。双臂急交叠收胸（0→2 快速 anticipation，聚饵），爆发外撒仰开
（双臂高位外张、后仰、重心上提，顶点 t4），随后双臂在高位反相波动「编织回
响之网」（t8→t17 交替相位、幅度渐衰——分形回响意象），收臂归中立。与单射
（侧身鞭甩）/ 齐射（拢镖开扇平撒后即收）动向完全区分：本招爆撒后有长织网
余韵。

时序（精度标准 #1/#2/#3）：
  anticipation 0→2   聚饵：双臂交叠收胸、微蹲蓄势
  strike       2→6   爆撒：t4 双臂外撒仰开顶点（双臂 pitch -118/-112 外张、
                     torso.pitch -8 后仰、body.y +0.04，INQUAD）→ t6 撒满展开
  recovery     6→24  织网余韵：t8/t11/t14/t17 双臂反相波动（幅度渐衰）→
                     t20 收半 → t24 归中立（INOUTSINE）
endTick=24，stopTick=26，非循环。主打击轴：rightArm.pitch / leftArm.pitch /
torso.pitch / body.y（全程 ≤4t 帧距）。
"""

from __future__ import annotations

from anim_common import emit_json

POSE = {
    # 聚饵起手：双臂快速交叠收胸、微蹲。
    0: dict(
        easing="OUTQUAD",
        body=dict(x=0.0, y=-0.03, z=-0.03),
        head=dict(pitch=+8, yaw=0),
        torso=dict(pitch=+6, yaw=-4),
        rightArm=dict(pitch=-58, yaw=-34, roll=-8, bend=74, axis=180),
        leftArm=dict(pitch=-54, yaw=+38, roll=+8, bend=70, axis=180),
        leftLeg=dict(pitch=-8, bend=11, z=-0.04),
        rightLeg=dict(pitch=+7, bend=10, z=+0.03),
    ),
    # 聚满收紧：交叠到最深、头微埋。
    2: dict(
        easing="INQUAD",
        body=dict(x=0.0, y=-0.05, z=-0.05),
        head=dict(pitch=+12, yaw=0),
        torso=dict(pitch=+9, yaw=-6),
        rightArm=dict(pitch=-64, yaw=-46, roll=-12, bend=88, axis=180),
        leftArm=dict(pitch=-60, yaw=+50, roll=+12, bend=84, axis=180),
        leftLeg=dict(pitch=-11, bend=15, z=-0.05),
        rightLeg=dict(pitch=+9, bend=13, z=+0.04),
    ),
    # 爆撒顶点（紧贴 tick 0 结算点）：双臂外撒仰开、重心上提。
    4: dict(
        easing="INQUAD",
        body=dict(x=0.0, y=+0.04, z=+0.02),
        head=dict(pitch=-14, yaw=0),
        torso=dict(pitch=-8, yaw=+2),
        rightArm=dict(pitch=-118, yaw=-52, roll=+14, bend=8, axis=180),
        leftArm=dict(pitch=-112, yaw=+56, roll=-14, bend=10, axis=180),
        leftLeg=dict(pitch=-4, bend=5, z=-0.02),
        rightLeg=dict(pitch=+4, bend=4, z=+0.02),
    ),
    # 撒满展开：臂稍落定、仰姿保持。
    6: dict(
        easing="OUTSINE",
        body=dict(x=0.0, y=+0.02, z=+0.01),
        head=dict(pitch=-10, yaw=0),
        torso=dict(pitch=-6, yaw=0),
        rightArm=dict(pitch=-106, yaw=-44, roll=+10, bend=16, axis=180),
        leftArm=dict(pitch=-102, yaw=+48, roll=-10, bend=18, axis=180),
        leftLeg=dict(pitch=-4, bend=5, z=-0.02),
        rightLeg=dict(pitch=+4, bend=4, z=+0.02),
    ),
    # 织网余韵 A：右臂沉左臂扬（反相波动，幅度最大）。
    8: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=+0.01, z=0.0),
        head=dict(pitch=-6, yaw=-5),
        torso=dict(pitch=-4, yaw=-4),
        rightArm=dict(pitch=-88, yaw=-30, roll=+6, bend=30, axis=180),
        leftArm=dict(pitch=-110, yaw=+42, roll=-8, bend=14, axis=180),
        leftLeg=dict(pitch=-3, bend=4, z=-0.02),
        rightLeg=dict(pitch=+3, bend=3, z=+0.01),
    ),
    # 织网余韵 B：反相（右扬左沉）。
    11: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=+0.01, z=0.0),
        head=dict(pitch=-6, yaw=+5),
        torso=dict(pitch=-4, yaw=+4),
        rightArm=dict(pitch=-104, yaw=-38, roll=+8, bend=16, axis=180),
        leftArm=dict(pitch=-84, yaw=+28, roll=-6, bend=32, axis=180),
        leftLeg=dict(pitch=-3, bend=4, z=-0.02),
        rightLeg=dict(pitch=+3, bend=3, z=+0.01),
    ),
    # 织网余韵 A'（幅度衰减）。
    14: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=0.0, z=0.0),
        head=dict(pitch=-4, yaw=-3),
        torso=dict(pitch=-3, yaw=-2),
        rightArm=dict(pitch=-86, yaw=-26, roll=+4, bend=28, axis=180),
        leftArm=dict(pitch=-98, yaw=+34, roll=-5, bend=18, axis=180),
        leftLeg=dict(pitch=-2, bend=3, z=-0.01),
        rightLeg=dict(pitch=+2, bend=2, z=+0.01),
    ),
    # 织网余韵 B'（再衰减）。
    17: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=0.0, z=0.0),
        head=dict(pitch=-3, yaw=+2),
        torso=dict(pitch=-2, yaw=+2),
        rightArm=dict(pitch=-92, yaw=-30, roll=+4, bend=20, axis=180),
        leftArm=dict(pitch=-82, yaw=+26, roll=-4, bend=28, axis=180),
        leftLeg=dict(pitch=-2, bend=3, z=-0.01),
        rightLeg=dict(pitch=+2, bend=2, z=+0.01),
    ),
    # 收臂半程。
    20: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=0.0, z=0.0),
        head=dict(pitch=-1, yaw=0),
        torso=dict(pitch=-1, yaw=0),
        rightArm=dict(pitch=-44, yaw=-14, roll=+2, bend=14, axis=180),
        leftArm=dict(pitch=-40, yaw=+12, roll=-2, bend=14, axis=180),
        leftLeg=dict(pitch=-1, bend=1, z=-0.01),
        rightLeg=dict(pitch=+1, bend=1, z=0.0),
    ),
    # 归中立。
    24: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=0.0, z=0.0),
        head=dict(pitch=0, yaw=0),
        torso=dict(pitch=0, yaw=0),
        rightArm=dict(pitch=0, yaw=0, roll=0, bend=0, axis=180),
        leftArm=dict(pitch=0, yaw=0, roll=0, bend=0, axis=180),
        leftLeg=dict(pitch=0, bend=0, z=0.0),
        rightLeg=dict(pitch=0, bend=0, z=0.0),
    ),
}


def main() -> int:
    emit_json(
        POSE,
        name="anqi_echo_fractal",
        description=(
            "P2 诱饵分形专属（24t 非循环，strike 顶点 t4 对齐瞬发结算点，解除 "
            "release_burst 借用；cast_ticks=60 为元数据、错配如实驻 allowlist）："
            "anticipation 0→2 聚饵交叠收胸，strike 2→6 爆撒仰开（双臂 -118/-112 "
            "外张 / torso.pitch -8 / body.y +0.04），recovery 6→24 织网余韵"
            "（t8/t11/t14/t17 双臂反相波动渐衰）→收臂归中立。"
        ),
        end_tick=24,
        stop_tick=26,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
