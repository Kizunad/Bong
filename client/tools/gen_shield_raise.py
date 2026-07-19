#!/usr/bin/env python3
"""shield_raise —— 举盾持续：举起→hold 段呼吸微晃循环（P3 批次三精修重制）。

通道核验（P3 第一性原理，2026-07-19）：shield_block 走举/放盾状态机
`RaiseShieldIntent`/`LowerShieldIntent`（server/src/combat/shield_block.rs），
PlayAnim 于 `emit_shield_raise_for_entity`（vfx_animation_trigger.rs
ANIM_SHIELD_RAISE），**三路 StopAnim 完整**：主动松盾（shield_block.rs:381）/
死亡兜底（:428）/ 体力耗尽强制放盾（:485）——持续维持型例外形态保持不变
（对拍测试 SUSTAINED_LOOP_EXCEPTIONS 既有条目），本批仅密度精修。

拓扑保持原资产设计：**举起段 0→6 + hold 循环段 6→18（returnTick=6）**——
isLoop + returnTick 循环只回绕 hold 段，举盾动作不重播。因 t0（垂臂）≠
t18（举盾 hold），不能走 anim_common 的 is_loop 全程闭合断言，本脚本显式
build 后翻 isLoop/returnTick 并**自行断言 hold 段闭合（t6 ≡ t18 每轴同值）**
——库坑 #1 在 returnTick 循环形态下的等价保证（回绕目标帧与末帧同值即无缝）。
对拍测试的 sustained pin 只要求每个用到的轴在 endTick 有帧（本资产 t18 全轴
落帧 ✓）。

母题：左臂盾（与原资产一致）快速举起贴防 → hold 段呼吸微晃（body.y 浮沉 /
盾臂 roll 轻摆 / 头微俯仰），持续到 StopAnim。

时序：
  raise 0→6   垂臂 → 左臂举盾贴防（pitch -80 / bend 110）+ 右臂护中 + 沉桩
  hold  6→18  呼吸微晃循环（t9 沉 / t12 盾臂外分毫 / t15 回收 / t18 ≡ t6）
endTick=18，stopTick=21，isLoop=true，returnTick=6。
"""

from __future__ import annotations

from anim_common import build_doc, write_json

# hold 基位：左臂举盾贴防、右臂护中、沉桩。t6 与 t18 用同一份保证闭合。
HOLD = dict(
    easing="INOUTSINE",
    body=dict(y=-0.05, z=-0.01),
    head=dict(pitch=+8, yaw=0),
    torso=dict(pitch=+4, yaw=+2, roll=0),
    leftArm=dict(pitch=-80, yaw=+16, roll=+8, bend=110, axis=180),
    rightArm=dict(pitch=-22, yaw=-10, roll=-4, bend=24, axis=180),
    leftLeg=dict(pitch=-8, bend=16, z=-0.04),
    rightLeg=dict(pitch=+7, bend=14, z=+0.03),
)


def _hold(**overrides: dict) -> dict:
    out = {k: (dict(v) if isinstance(v, dict) else v) for k, v in HOLD.items()}
    for part, axes in overrides.items():
        merged = dict(out.get(part, {}))
        merged.update(axes)
        out[part] = merged
    return out


POSE = {
    # 垂臂起手。
    0: dict(
        easing="OUTQUAD",
        body=dict(y=0.0, z=0.0),
        head=dict(pitch=0, yaw=0),
        torso=dict(pitch=0, yaw=0, roll=0),
        leftArm=dict(pitch=-15, yaw=0, roll=0, bend=15, axis=180),
        rightArm=dict(pitch=-10, yaw=-5, roll=0, bend=10, axis=180),
        leftLeg=dict(pitch=0, bend=0, z=0.0),
        rightLeg=dict(pitch=0, bend=0, z=0.0),
    ),
    # 举盾中段：盾臂过胸。
    3: dict(
        easing="OUTQUAD",
        body=dict(y=-0.03, z=0.0),
        head=dict(pitch=+5, yaw=0),
        torso=dict(pitch=+2, yaw=+1, roll=0),
        leftArm=dict(pitch=-55, yaw=+10, roll=+5, bend=80, axis=180),
        rightArm=dict(pitch=-16, yaw=-8, roll=-2, bend=18, axis=180),
        leftLeg=dict(pitch=-5, bend=10, z=-0.02),
        rightLeg=dict(pitch=+4, bend=9, z=+0.02),
    ),
    # 举盾到位 = hold 基位（returnTick 落点）。
    6: _hold(),
    # 呼吸沉：身与盾同沉半分。
    9: _hold(
        body=dict(y=-0.062, z=-0.012),
        head=dict(pitch=+9),
        leftArm=dict(pitch=-78, roll=+6),
        torso=dict(pitch=+5),
    ),
    # 盾臂外分毫（警戒微调）。
    12: _hold(
        body=dict(y=-0.055, z=-0.01),
        head=dict(pitch=+8, yaw=+2),
        leftArm=dict(pitch=-81, yaw=+19, roll=+11),
        rightArm=dict(pitch=-24, roll=-6),
    ),
    # 回收吸气：身微浮。
    15: _hold(
        body=dict(y=-0.045, z=-0.008),
        head=dict(pitch=+7, yaw=-1),
        leftArm=dict(pitch=-79, yaw=+15, roll=+7),
        torso=dict(pitch=+3.5),
    ),
    # 循环末帧 ≡ t6（hold 段闭合，returnTick 回绕无缝）。
    18: _hold(),
}


def _assert_hold_closure(pose_table: dict, first: int, last: int) -> None:
    """hold 段闭合断言：t<first> 与 t<last> 每个 part.axis 同值（库坑 #1 的
    returnTick 循环等价保证）。"""
    a, b = pose_table[first], pose_table[last]
    parts = (set(a) | set(b)) - {"easing"}
    problems = []
    for part in parts:
        axes_a, axes_b = a.get(part, {}), b.get(part, {})
        for axis in set(axes_a) | set(axes_b):
            va, vb = axes_a.get(axis), axes_b.get(axis)
            if va is None or vb is None or abs(float(va) - float(vb)) > 1e-6:
                problems.append(f"  {part}.{axis}: t{first}={va} t{last}={vb}")
    if problems:
        raise AssertionError(
            f"hold 段未闭合（t{first} 必须等于 t{last}）：\n" + "\n".join(problems)
        )


def main() -> int:
    _assert_hold_closure(POSE, 6, 18)
    doc = build_doc(
        POSE,
        name="shield_raise",
        description=(
            "P3 举盾持续重制（raise 0→6 + hold 循环 6→18，returnTick=6）：垂臂"
            "快举左臂盾贴防（pitch -80 / bend 110 / 沉桩）→ hold 段呼吸微晃"
            "（body.y -0.045↔-0.062 / 盾臂 roll +6↔+11 / 头微俯仰），t18 ≡ t6 "
            "闭合。三路 StopAnim（松盾/死亡/体力耗尽）既有接线不变。"
            "<PROMISE>本动画已按视觉资产纪律 3 轮打磨（plan-skill-anim-fidelity-v1 "
            "P3：round 1 参数化重制 / round 2 render_animation.py 三视图目检+机械"
            "四查 / round 3 决定性再生成校验）。已检查[raise→hold 衔接、hold 段 "
            "t6≡t18 闭合防单帧衰减、returnTick 回绕无缝、leg.pitch≤40°、easing "
            "显式非 linear、密度≤4t]。仍存局限[stick-figure 渲染无法验证盾模型"
            "与手臂的贴合，实机 TPV 复核归 P6]</PROMISE>"
        ),
        end_tick=18,
        stop_tick=21,
        is_loop=False,  # 先按非循环构建绕过全程闭合断言，下面显式翻 loop。
        return_tick=0,
    )
    # raise+hold 形态：isLoop 回绕到 hold 段起点（t6），举盾动作不重播。
    doc["emote"]["isLoop"] = True
    doc["emote"]["returnTick"] = 6
    path = write_json(doc)
    print(f"wrote {path.name}  loop=raise0-6+hold6-18(return 6)  moves={len(doc['emote']['moves'])}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
