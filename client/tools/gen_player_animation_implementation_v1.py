#!/usr/bin/env python3
"""Generate the player-animation-implementation-v1 JSON asset set.

The goal of this batch is a deterministic, code-reviewable first pass: every
animation id promised by the active plan has a concrete Emotecraft v3 resource.
Art polish stays in later style-specific plans; these poses intentionally stay
small and readable.
"""

from __future__ import annotations

from anim_common import emit_json, inherit


BASE = dict(
    easing="INOUTSINE",
    body=dict(x=0.0, y=0.0, z=0.0),
    head=dict(pitch=0, yaw=0, roll=0),
    torso=dict(pitch=0, yaw=0, roll=0),
    rightArm=dict(pitch=0, yaw=0, roll=0, bend=0, axis=180),
    leftArm=dict(pitch=0, yaw=0, roll=0, bend=0, axis=180),
    rightLeg=dict(pitch=0, yaw=0, bend=0, z=0.0),
    leftLeg=dict(pitch=0, yaw=0, bend=0, z=0.0),
)


def once(name: str, description: str, peak: dict, *, end_tick: int = 8, stop_tick: int = 10) -> None:
    emit_json(
        {
            0: BASE,
            max(1, end_tick // 2): inherit(BASE, easing="OUTQUAD", **peak),
            end_tick: BASE,
        },
        name=name,
        description=description,
        end_tick=end_tick,
        stop_tick=stop_tick,
        is_loop=False,
    )


def loop(name: str, description: str, peak: dict, *, end_tick: int = 20) -> None:
    emit_json(
        {
            0: BASE,
            end_tick // 2: inherit(BASE, easing="INOUTSINE", **peak),
            end_tick: BASE,
        },
        name=name,
        description=description,
        end_tick=end_tick,
        stop_tick=end_tick + 2,
        is_loop=True,
    )


def pose_hold(name: str, description: str, pose: dict, *, end_tick: int = 20) -> None:
    held = inherit(BASE, **pose)
    emit_json(
        {
            0: held,
            end_tick // 2: held,
            end_tick: held,
        },
        name=name,
        description=description,
        end_tick=end_tick,
        stop_tick=end_tick + 2,
        is_loop=True,
    )


def generate_combat_and_posture() -> None:
    once(
        "sword_swing_right",
        "右手横扫验证动画：右臂横切，躯干跟随转腰。",
        dict(torso=dict(yaw=20), rightArm=dict(pitch=-90, roll=16, bend=20, axis=180)),
    )
    once(
        "hurt_stagger",
        "受击后仰：上身短促后撤，头部跟随后仰。",
        dict(body=dict(z=0.08), head=dict(pitch=-10), torso=dict(pitch=-15), rightArm=dict(pitch=-20)),
        end_tick=6,
        stop_tick=8,
    )
    once(
        "palm_strike",
        "双掌推出：两臂前伸，身体小幅前压。",
        dict(body=dict(z=0.12), torso=dict(pitch=8), rightArm=dict(pitch=-82, bend=8), leftArm=dict(pitch=-82, bend=8)),
        end_tick=6,
    )
    once(
        "sword_slash_down",
        "剑势下劈：右臂从举高到前下方斩落。",
        dict(torso=dict(pitch=10), rightArm=dict(pitch=-110, roll=20, bend=18)),
    )
    loop(
        "windup_charge",
        "蓄力预备：沉腰、右臂后收，可持续到释放。",
        dict(body=dict(y=0.08), torso=dict(yaw=22), rightArm=dict(pitch=-38, roll=25, bend=120)),
        end_tick=16,
    )
    once(
        "release_burst",
        "蓄力释放：全身展开并短促上提。",
        dict(body=dict(y=-0.10, z=0.16), torso=dict(pitch=-8), rightArm=dict(pitch=-125, bend=8), leftArm=dict(pitch=-125, bend=8)),
        end_tick=4,
        stop_tick=6,
    )
    once(
        "parry_block",
        "格挡振臂：双臂交叉胸前。",
        dict(rightArm=dict(pitch=-72, yaw=24, roll=32, bend=95), leftArm=dict(pitch=-72, yaw=-24, roll=-32, bend=95)),
        end_tick=16,
        stop_tick=18,
    )
    once(
        "dodge_roll",
        "闪避侧翻读数：躯干 roll 形成翻滚感，保留位移给服务端处理。",
        dict(body=dict(x=-0.25), torso=dict(roll=85), head=dict(roll=40), rightLeg=dict(pitch=55, bend=55), leftLeg=dict(pitch=-35, bend=35)),
        end_tick=10,
        stop_tick=12,
    )
    pose_hold(
        "harvest_crouch",
        "采药蹲伏：身体压低，右手向地面探取。",
        dict(body=dict(y=0.32), torso=dict(pitch=26), rightArm=dict(pitch=-65, bend=45), leftLeg=dict(bend=48), rightLeg=dict(bend=48)),
    )
    pose_hold(
        "loot_bend",
        "搜刮弯腰：上身前俯，双手向前翻找。",
        dict(torso=dict(pitch=45), rightArm=dict(pitch=-70, bend=20), leftArm=dict(pitch=-70, bend=20)),
    )
    loop(
        "stealth_crouch",
        "潜行伏低：低姿态微摆动。",
        dict(body=dict(y=0.28), torso=dict(pitch=18), head=dict(pitch=-6), leftLeg=dict(bend=38), rightLeg=dict(bend=38)),
        end_tick=24,
    )
    loop(
        "idle_breathe",
        "站立呼吸：躯干与双臂轻微摆动，客户端 idle 层可自演。",
        dict(torso=dict(pitch=2), rightArm=dict(roll=4), leftArm=dict(roll=-4)),
        end_tick=40,
    )


def generate_npc_and_interaction() -> None:
    loop("npc_patrol_walk", "NPC 巡逻步态：轻摆臂慢步。", dict(rightArm=dict(pitch=-18), leftArm=dict(pitch=18), rightLeg=dict(pitch=18), leftLeg=dict(pitch=-18)), end_tick=20)
    loop("npc_chop_tree", "NPC 砍树假示好：右臂循环下劈。", dict(torso=dict(pitch=12), rightArm=dict(pitch=-95, bend=20)), end_tick=18)
    loop("npc_mine", "NPC 挖矿：前俯并短促挥镐。", dict(torso=dict(pitch=20), rightArm=dict(pitch=-105, roll=12, bend=25)), end_tick=16)
    once("npc_crouch_wave", "NPC 蹲伏挥手：低身位加右手横摆。", dict(body=dict(y=0.25), rightArm=dict(pitch=-70, yaw=-30, bend=70)), end_tick=14, stop_tick=16)
    loop("npc_flee_run", "NPC 逃跑奔跑：身体前倾，高频摆臂。", dict(torso=dict(pitch=20), rightArm=dict(pitch=-35), leftArm=dict(pitch=35), rightLeg=dict(pitch=35), leftLeg=dict(pitch=-35)), end_tick=12)
    loop("forge_hammer", "锻造锤击：右臂举起并下砸循环。", dict(torso=dict(pitch=10), rightArm=dict(pitch=-110, bend=25)), end_tick=8)
    loop("alchemy_stir", "炼丹搅拌：右臂沿丹炉方向环动读数。", dict(torso=dict(pitch=12, yaw=8), rightArm=dict(pitch=-70, yaw=-20, roll=30, bend=45)), end_tick=16)
    once("lingtian_till", "灵田翻土：右臂挥锄，身体下压。", dict(body=dict(y=0.12), torso=dict(pitch=24), rightArm=dict(pitch=-105, bend=25)), end_tick=6, stop_tick=8)
    once("inventory_reach", "背包翻找：右手探向腰侧。", dict(torso=dict(yaw=-14), rightArm=dict(pitch=-45, yaw=-35, bend=70)), end_tick=4, stop_tick=6)


def generate_stances() -> None:
    pose_hold("stance_baomai", "爆脉沉腰：宽站位，双拳收在腰际。", dict(body=dict(y=0.10), torso=dict(pitch=8), rightArm=dict(pitch=-40, bend=115), leftArm=dict(pitch=-40, bend=115), rightLeg=dict(yaw=10, bend=18), leftLeg=dict(yaw=-10, bend=18)))
    pose_hold("stance_dugu", "暗器微抬腕：右腕前置，左手藏后。", dict(torso=dict(yaw=-8), rightArm=dict(pitch=-38, yaw=-18, bend=35), leftArm=dict(pitch=10, yaw=25, bend=20)))
    pose_hold("stance_zhenfa", "阵法展掌布符：双掌前展。", dict(rightArm=dict(pitch=-72, yaw=-12, bend=10), leftArm=dict(pitch=-72, yaw=12, bend=10)))
    pose_hold("stance_dugu_poison", "毒蛊藏指捻针：右手贴身捻针，身体微蜷。", dict(torso=dict(pitch=10), rightArm=dict(pitch=-52, yaw=-20, bend=95), leftArm=dict(pitch=-20, bend=65)))
    pose_hold("stance_zhenmai", "截脉侧身蓄劲：侧身护架。", dict(torso=dict(yaw=35), rightArm=dict(pitch=-62, bend=90), leftArm=dict(pitch=-68, yaw=18, bend=80)))
    loop("stance_woliu", "涡流双掌开合：胸前双掌缓慢开合。", dict(rightArm=dict(pitch=-62, yaw=-22, bend=55), leftArm=dict(pitch=-62, yaw=22, bend=55)), end_tick=40)
    pose_hold("stance_tuike", "蜕壳披壳：双臂环抱护体。", dict(torso=dict(pitch=12), head=dict(pitch=8), rightArm=dict(pitch=-45, yaw=30, bend=110), leftArm=dict(pitch=-45, yaw=-30, bend=110)))
    loop("limp_left", "左腿伤跛行：左步幅减小，右腿代偿。", dict(leftLeg=dict(pitch=10, bend=10), rightLeg=dict(pitch=-24, bend=18), torso=dict(roll=4)), end_tick=20)
    loop("limp_right", "右腿伤跛行：右步幅减小，左腿代偿。", dict(rightLeg=dict(pitch=10, bend=10), leftLeg=dict(pitch=-24, bend=18), torso=dict(roll=-4)), end_tick=20)
    pose_hold("arm_injured_left", "左臂伤：左臂下垂，不参与摆臂。", dict(leftArm=dict(pitch=8, roll=-6, bend=8)))
    pose_hold("arm_injured_right", "右臂伤：右臂下垂，不参与摆臂。", dict(rightArm=dict(pitch=8, roll=6, bend=8)))
    loop("exhausted_walk", "虚弱步态：全身小幅前倾，摆幅降低。", dict(torso=dict(pitch=12), rightArm=dict(pitch=-10), leftArm=dict(pitch=10), rightLeg=dict(pitch=12), leftLeg=dict(pitch=-12)), end_tick=24)


def generate_breakthrough_and_death() -> None:
    once("breakthrough_yinqi", "醒灵到引气：仰头张臂后收束合掌。", dict(head=dict(pitch=-18), rightArm=dict(pitch=-120, yaw=-18, bend=20), leftArm=dict(pitch=-120, yaw=18, bend=20)), end_tick=35, stop_tick=38)
    once("breakthrough_ningmai", "引气到凝脉：经脉循行带来周身微颤。", dict(torso=dict(pitch=-5, yaw=8), rightArm=dict(pitch=-80, bend=50), leftArm=dict(pitch=-80, bend=50)), end_tick=32, stop_tick=35)
    once("breakthrough_guyuan", "凝脉到固元：双手环抱丹田。", dict(torso=dict(pitch=10), rightArm=dict(pitch=-55, yaw=-18, bend=90), leftArm=dict(pitch=-55, yaw=18, bend=90)), end_tick=34, stop_tick=37)
    once("breakthrough_tongling", "固元到通灵：双臂展开，身体微浮。", dict(body=dict(y=-0.30), head=dict(pitch=-22), rightArm=dict(pitch=-135, yaw=-35, bend=8), leftArm=dict(pitch=-135, yaw=35, bend=8)), end_tick=40, stop_tick=44)
    once("death_disintegrate", "魂散消逝：身体上浮，肢体逐渐展开。", dict(body=dict(y=-0.50), rightArm=dict(pitch=-90, yaw=-45), leftArm=dict(pitch=-90, yaw=45), rightLeg=dict(pitch=18), leftLeg=dict(pitch=18)), end_tick=24, stop_tick=28)
    once("rebirth_wake", "灵龛重生苏醒：从低姿态缓慢站起并环顾。", dict(body=dict(y=0.20), head=dict(yaw=18), torso=dict(pitch=16), rightArm=dict(pitch=-22, bend=20), leftArm=dict(pitch=-22, bend=20)), end_tick=20, stop_tick=24)


def main() -> int:
    generate_combat_and_posture()
    generate_npc_and_interaction()
    generate_stances()
    generate_breakthrough_and_death()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
