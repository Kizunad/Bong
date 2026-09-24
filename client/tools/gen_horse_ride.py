#!/usr/bin/env python3
"""horse_ride_* —— 骑手坐在马鞍上的坐姿。

**姿势不是拧出来的，是从鞍量出来的。** 落点由马具层（`scripts/models/horse/gen_tack.py`）
造完实件后量得，常马那一档抄在下面的 `SEATS` 里；改了鞍这边就该跟着改：

    鞍档    座面 y   座→镫落差   镫在座后   鞍翼半宽   前桥/后桥
    破毡     27.08     —（无镫）      —        3.25      0 / 0
    粗革     28.16      10.11       0.43      4.78     0.078W / 0.090W
    灵铁     28.54      10.11       0.46      4.78     0.094W / 0.108W

## 三档鞍，两副坐姿

量下来**粗革与灵铁把人放在同一个位置**：座→镫落差同是 10.11（马具层的
`STIRRUP_DROP` 是绝对量——"三档马一个骑手"，所以它不随鞍变）、鞍翼同宽 4.78、
镫的前后位置差 0.03 个单位。剩下的差别在前后鞍桥的高度上，折算成上身与手的角度
只有 2.4° —— 半个像素都不到，出两条一模一样的轨道是在骗人。

这跟马具层自己的说法是一致的：那两档"本来就都是皮鞍，差别在**配件**
（鞍桥包灵铁 + 灵纹 + 灵铁镫）"。**配件不改变人坐在哪儿。**

所以出两副：`SADDLE_SEAT` 是三档鞍到两副坐姿的对照表，接线的人查它。

## 两副之间靠什么认出来

  · **有没有镫** —— 破毡鞍没有鞍桥没有镫（`SaddleSpec.stirrup=False`）。脚没处踩，
    腿只能垂下去夹着马；有镫的把脚踩在踏板上，膝盖折起来。屈膝 27° ↔ 65°。
  · **鞍翼多宽** —— 腿跨在鞍翼上，鞍翼越宽腿张得越开。3.25 ↔ 4.78，外张 13° ↔ 26°。
  · **后鞍桥托不托腰** —— 没有后桥只能自己前倾撑着（上身 +10°），有后桥能坐直（+2°）。

屈膝角与大腿俯仰是**解出来的**：脚要落在镫上，而 MC 的腿长是定值 12，这是一道两连杆
的解（`leg_to`）。PlayerAnimator 的 `bend` 把腿从中间折一下，折角 β 时髋到脚的弦长是
12·cos(β/2)，于是 β 与大腿方向都被落点唯一确定。

**为什么不能直接把 pitch 拧到 −65°**：MC 没有 IK，`leg.pitch` 过 ~35-40° 大腿就和胯
脱开（docs/player-animation-conventions.md）。折角必须由 `bend` 担，pitch 留在 40°
以内——下面那条 assert 就是守这个的。

用法:
  python3 client/tools/gen_horse_ride.py
  python3 client/tools/render_animation.py \\
      client/src/main/resources/assets/bong/player_animation/horse_ride_stirrup.json
"""

from __future__ import annotations

import math

from anim_common import build_doc, emit_json

LEG = 12.0  # MC 玩家腿长（髋 pivot 到脚底），单位 = 模型像素
HIP_HALF = 1.9  # 腿 pivot 的 x（vanilla BipedEntityModel）
THIGH = LEG / 2.0  # 折点在腿中间，所以"大腿"就是半条腿
PITCH_CAP = 40.0  # MC 无 IK，大腿俯仰的硬上限（超了大腿与胯脱开）
HANG_BEND = 8.0  # 没处踩的腿垂着时留的自然屈度（人站着膝也不是锁死的）


class Seat:
    """一副坐姿要的全部落点（常马量得）。1 单位 = 1 体素 = 6.25 cm。"""

    def __init__(self, key: str, label: str, *, drop: float | None, back: float,
                 flap: float, cantle: float, pommel: float, blurb: str) -> None:
        self.key, self.label, self.blurb = key, label, blurb
        self.drop = drop  # 座面到镫踏板的落差；None = 没有镫
        self.back = back  # 镫落在座心之后多少（+ = 偏后）
        self.flap = flap  # 鞍翼 / 鞍垫半宽——腿跨在它上面
        self.cantle = cantle  # 后鞍桥高（× 鬐甲高）
        self.pommel = pommel  # 前鞍桥高（× 鬐甲高）


SEATS = (
    Seat("bareback", "无镫（破毡鞍）", drop=None, back=0.0, flap=3.25, cantle=0.0, pommel=0.0,
         blurb="没有鞍桥没有镫，腿垂着夹住马，人得自己前倾撑住"),
    Seat("stirrup", "踩镫（粗革鞍 / 灵铁鞍）", drop=10.11, back=0.45, flap=4.78,
         cantle=0.099, pommel=0.086,
         blurb="脚踩镫，膝盖折起来，后桥托着腰，人坐得直"),
)

# 三档鞍 → 两副坐姿。接线的人查这张表；两档皮鞍共用一副是量出来的结论，
# 不是偷懒（见文件头）。鞍档名与 `gen_tack.SADDLES` 的 key 一一对应。
SADDLE_SEAT = {"felt": "bareback", "leather": "stirrup", "lingtie": "stirrup"}


def leg_to(drop: float, back: float, out: float) -> tuple[float, float, float]:
    """脚要落在 (下 drop, 后 back, 外 out) 处 → (大腿俯仰 pitch, 外张 roll, 屈膝 bend)。

    PlayerAnimator 的通道次序是 `Rz(roll)·Ry(yaw)·Rx(pitch)`（JOML rotationZYX），
    也就是**先在矢状面里摆好再整条往外倒**。所以这道解可以拆成两步：

      ① 矢状面内：脚要落在 (下 dp, 后 back) 处，dp = hypot(drop, out)——**外张会把
         腿"用掉"一截**，斜着下去的腿在竖直方向只剩 cos(roll) 那么长。首版把 out
         漏在解外面，外张 26° 时脚比镫高出整整一个单位（脚踩空）。
      ② 整条外倒 roll = atan2(out, dp)。

    两连杆等长，折角 β 时髋到脚的弦长 = LEG·cos(β/2)，所以 β 由落点的距离唯一定；
    大腿方向 = 弦的方向再往前抬 β/2（等腰三角形的一半顶角）。

    落点够不着（弦比腿长）时夹到腿长——真人也只能把腿伸直，够不着就是够不着，
    不许把腿"拉长"。
    """
    dp = math.hypot(drop, back)
    d = min(math.hypot(dp, out), LEG * 0.999)
    bend = 2.0 * math.degrees(math.acos(d / LEG))
    lean = math.degrees(math.atan2(-back, drop))  # 脚在髋后 → 弦朝后，取负
    roll = math.degrees(math.atan2(out, dp))
    return -(lean + bend / 2.0), roll, bend


def splay_out(flap: float) -> float:
    """脚要横着落在离髋多远的地方才跨得过这副鞍。

    量的是**鞍翼半宽**而不是马的桶身：腿是搭在鞍翼上的，鞍本来就比马宽。
    """
    return max(0.0, flap - HIP_HALF)


def pose_for(s: Seat) -> dict:
    """一副坐姿的静态形（呼吸起伏另加）。"""
    out = splay_out(s.flap)
    if s.drop is None:
        # 没有镫：**不给脚定落点**——没有东西托着它，落点是垂下来的结果不是原因。
        # 定的是"腿基本伸直"（留一点自然屈度），脚落哪儿由它自己说了算。
        # 反过来先假定一个落点再解，等于凭空替一只悬空的脚编一个高度：首版按
        # 0.94·腿长 编了一个，解出来屈膝 36.8°——那不叫垂着，那叫半蹲。
        bend = HANG_BEND
        chord = LEG * math.cos(math.radians(bend / 2.0))
        roll = math.degrees(math.atan2(out, chord))
        pitch = -bend / 2.0
    else:
        pitch, roll, bend = leg_to(s.drop, s.back, out)
    assert abs(pitch) <= PITCH_CAP, (
        f"{s.key}: 大腿俯仰 {pitch:.1f}° 超过 {PITCH_CAP}°——MC 没有 IK，"
        f"再大大腿就和胯脱开了。折角该由 bend 担，不该往 pitch 上堆")
    # 上身：后鞍桥托着腰，托得越高人坐得越直。没有后桥只能自己前倾撑住。
    lean = 10.0 - 81.0 * s.cantle
    # 缰手：前鞍桥越高，手越要抬过它。没有鞍桥时手最低（贴着颈）。
    hand = -34.0 - 150.0 * s.pommel
    return dict(
        torso=dict(pitch=round(lean, 1)),
        head=dict(pitch=round(-lean * 0.45, 1)),  # 头回正，别跟着躯干一起低下去
        rightArm=dict(pitch=round(hand, 1), yaw=-8, bend=22, axis=180),
        leftArm=dict(pitch=round(hand, 1), yaw=+8, bend=22, axis=180),
        # roll 的符号是**渲出来定的**，不是推的：MC 模型空间 +X 是玩家的**左**、
        # +Y 朝**下**，绕 Z 转正角会把朝下的腿推向 −X（玩家的右）。所以要让腿各自
        # 往外张，右腿取正、左腿取负——首版按"右负左正"给，三视图里两只脚在裆下
        # 交叉到了一起（膝盖张开、脚却并拢，等于夹在马肚子里）。
        rightLeg=dict(pitch=round(pitch, 1), roll=round(+roll, 1), bend=round(bend, 1), axis=0),
        leftLeg=dict(pitch=round(pitch, 1), roll=round(-roll, 1), bend=round(bend, 1), axis=0),
    )


def merge(base: dict, **over: dict) -> dict:
    out = {k: dict(v) for k, v in base.items()}
    for part, axes in over.items():
        out.setdefault(part, {}).update(axes)
    return out


CYCLE = 40  # 2 秒一轮的坐姿呼吸 / 随马轻晃


def build(s: Seat) -> dict:
    """40 tick 循环。坐着不是雕像：随马起伏微微上下 + 上身前后一点点。

    幅度按**有没有可撑的东西**给：没有镫的那一副晃得最多（全靠腰腿跟着马走），
    有镫有后桥的稳。循环两端逐轴取同值，否则 PlayerAnimator 会往 defaultValue
    衰减（见 anim_common._check_loop_closure）。
    """
    base = pose_for(s)
    sway = 0.9 if s.drop is None else 0.45
    lift = 0.05 if s.drop is None else 0.03
    out = {}
    for k in range(5):
        tick = k * (CYCLE // 4)
        ph = math.sin(2.0 * math.pi * k / 4.0)
        out[tick] = merge(
            base,
            body=dict(y=round(lift * ph, 3)),
            torso=dict(pitch=round(base["torso"]["pitch"] + sway * ph, 2)),
            head=dict(pitch=round(base["head"]["pitch"] - sway * 0.6 * ph, 2)),
        )
        out[tick]["easing"] = "INOUTSINE"
    return out


FOOT_TOL = 0.6  # 脚落点容许偏差（单位 = 模型像素；0.6 ≈ 4 cm，肉眼分不出）


def check_foot(s: Seat, doc: dict) -> str:
    """把生成的 JSON 拿**渲染器那套正向运动学**跑一遍，看脚是不是真落在镫上。

    为什么不能只信解算：解是在"先俯仰后外倒"这个假设下写的，而通道次序、bend 的
    折点位置、脚底相对 pivot 的偏移都在渲染器那边（它照 PlayerAnimator 的实现抄的）。
    自己解自己验等于没验——这里改成拿另一份实现对拍，两边对上了才算数。
    """
    from render_animation import collect_keyframes, solve_skeleton

    def legs(pose_table):
        d = build_doc(pose_table, name="_probe", description="", end_tick=CYCLE,
                      stop_tick=CYCLE + 3, is_loop=True)
        return solve_skeleton(collect_keyframes(d["emote"]), 0.0)

    # 落点只在**右腿**上验。渲染器给左肢的局部原点带着一个 +2 的横向偏置——它自己
    # 文档里写"两腿的 bend_center 都是 (0,6,0)"，而 `CUBOIDS` 里左腿那行给的是
    # offset (0,0,-2)（算出来是 (2,6,0)），两处对不上；原版 BipedEntityModel 的两条腿
    # 用的是同一个 cuboid(-2,0,-2,4,12,4)，左腿只是 `.mirrored()`（镜的是贴图不是几何），
    # 所以**右腿那一侧才与原版一致**。那是预览工具自己的事，交付物只有角度，不受影响——
    # 所以这里挑对得上的那侧量，不去改一件被九十来个生成器共用的工具。
    # 左右对称改在**源头**上断言（下面那条：两条腿的通道值必须严格互为镜像），
    # 比在渲染出来的坐标上比更直接。
    rest = legs({t: dict(easing="linear") for t in (0, CYCLE)})
    sk = solve_skeleton(collect_keyframes(doc["emote"]), 0.0)
    d = (sk["rightLeg"]["end"] - sk["rightLeg"]["start"]) \
        - (rest["rightLeg"]["end"] - rest["rightLeg"]["start"])
    got = (float(d[1]) + LEG, float(d[2]), -float(d[0]))  # 静止姿本来就垂 LEG
    # 无镫那一副**没有落点可对**（脚是垂下来的结果），只查"腿确实基本伸直"+ 外张；
    # 有镫那一副才逐项对镫的位置。
    want = ((got[0], got[1], splay_out(s.flap)) if s.drop is None
            else (s.drop, s.back, splay_out(s.flap)))
    for name, g, w in zip(("下", "后", "外"), got, want):
        if abs(g - w) > FOOT_TOL:
            raise AssertionError(
                f"{s.key}: 脚{name}了 {g:.2f}，镫在 {w:.2f}——差 {abs(g - w):.2f} "
                f"超过 {FOOT_TOL}。解出来的角度没把脚放到镫上，要么解错了、"
                f"要么通道次序与渲染器那边对不上")
    if s.drop is None and got[0] < LEG * math.cos(math.radians(HANG_BEND)) - FOOT_TOL:
        raise AssertionError(
            f"{s.key}: 垂着的腿只垂下 {got[0]:.2f}，比一条基本伸直的腿（"
            f"{LEG * math.cos(math.radians(HANG_BEND)):.2f}）短——那是半蹲不是垂着")
    p0 = doc["emote"]["moves"]
    lr = {(m["tick"], p, a): v for m in p0 for p, d2 in m.items()
          if p in ("leftLeg", "rightLeg") for a, v in (d2.items() if isinstance(d2, dict) else ())}
    for (tick, part, ax), v in lr.items():
        if part != "leftLeg":
            continue
        rv = lr.get((tick, "rightLeg", ax))
        want_v = -v if ax == "roll" else v  # 只有外张是左右反号，其余必须一模一样
        if rv is None or abs(rv - want_v) > 1e-9:
            raise AssertionError(f"{s.key}: 两腿的 {ax} 不是镜像（左 {v}, 右 {rv}）——镫是一对的")
    return f"下{got[0]:.1f}/后{got[1]:+.1f}/外{got[2]:.1f}（右腿量，左腿镜像）"


def main() -> None:
    for s in SEATS:
        pose = build(s)
        p0 = pose[0]
        用 = "、".join(k for k, v in SADDLE_SEAT.items() if v == s.key)
        desc = (
            f"骑马坐姿·{s.label}（鞍档 {用}）：{s.blurb}。"
            f"大腿俯仰 {p0['rightLeg']['pitch']}°、屈膝 {p0['rightLeg']['bend']}°"
            f"（由座→镫落差 {s.drop if s.drop else '无镫'} 反解，不是填的），"
            f"外张 {abs(p0['leftLeg']['roll'])}°（鞍翼半宽 {s.flap}），"
            f"上身 {p0['torso']['pitch']}°（后鞍桥 {s.cantle}W）。"
        )
        doc = build_doc(pose, name=f"horse_ride_{s.key}", description=desc,
                        end_tick=CYCLE, stop_tick=CYCLE + 3, is_loop=True)
        print(f"  ✓ 脚落点对拍 {check_foot(s, doc)}")
        emit_json(pose, name=f"horse_ride_{s.key}", description=desc,
                  end_tick=CYCLE, stop_tick=CYCLE + 3, is_loop=True)


if __name__ == "__main__":
    main()
