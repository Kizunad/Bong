#!/usr/bin/env python3
"""异变缝合兽 —— 头颅动画：把头颅层推出来的那些数直接变成动作。

头颅层已经解出了颌关节在哪、咀嚼一个冲程要横滑多少、张口能张多大、咬肌有多少截面。
**这些数本来就是动作的参数**，不是静态的装饰——所以这一层几乎不用另立新东西，只是把
它们接到时间轴上：

  · **咀嚼行程就是颌关节高度那条推导本身。** 下颌绕关节转 θ，齿列会沿前后向滑 θ·h。
    磨的那一类 h = 齿宽/θ（牛算出 12 cm），所以它嚼起来是**明显的横向碾磨**；剪的那一类
    h = 0，所以它只有上下开合、一丝横滑都没有。同一段代码，两种截然不同的嚼相，差别
    全部来自静态几何里那一个数。
  · **闭口速度由肌肉缩短速率定，不由力矩定。** 按力矩算：狼的咬肌合力 1815 N、力臂
    0.05 m ⇒ 力矩 90 N·m，下颌转动惯量只有 9.6e-4，角加速度 93750 rad/s²，闭一次
    3.6 ms——荒谬。真正的限制是肌肉**能多快缩短**（V_MUSCLE ≈ 5 倍肌长/秒）：闭口要
    缩短 力臂×张角 = 3.05 cm，肌长约 10 cm ⇒ 0.5 m/s ⇒ 61 ms，实测狼合颌 50–100 ms。
    力矩管的是"咬得多重"，速度管的是"合得多快"，两回事。
  · **咀嚼频率**按体重异速：f = 4.6·M^−0.25 Hz。鼠 6.0、兔 3.9、狼 1.8、牛 0.93 Hz，
    实测 ~6 / 3–4 / ~2 / ~1。这一条是观察拟合，标在常数上。
  · **顶角的冲击速度**直接取 `HornStyle.speed`——那正是角基粗细那条储能推导的输入。
    动画和几何用的是同一个数，改一个另一个跟着变。

七条动作，和核心层那七条同一个思路：每条都由**这颗头自己的解剖**决定，而不是套模板。
没有耳廓的（禽、蛙）就没有耳朵动作，没有角的就没有顶角，蛙的"嚼"是一次整吞。

**时间的量纲**：每条 `anim_*` 收到的 `t` 是**归一化相位 ∈[0,1]**，秒数要自己乘
`length`。这和 `core_anim` / `fragment_anim` 一致，也正是 `build_tracks` 喂进来的东西。
初版这里按秒写，而导出与渲染都喂归一化值——牛的三周期咀嚼于是只嚼了 0.93 下就跳回起点，
十个供体的 `.animation.json` 全错。自检当时是按秒调用的，所以查的是一条从没被导出过的
曲线。现在自检改走 `sample()`，和导出同一个入口，这类错不会再各说各话。

用法:
  python3 modelScript/creatures/stitched_beast/head_anim.py --donor wolf
  python3 modelScript/creatures/stitched_beast/head_anim.py --donor cow --list
  python3 modelScript/creatures/stitched_beast/head_anim.py --all
"""

from __future__ import annotations

import argparse
import math
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "core"))
sys.path.insert(0, str(HERE))

import gen_beast as GB  # noqa: E402
import genome as GN  # noqa: E402
import heads as HD  # noqa: E402
from bbmodel_maker.rig.anim_rig import Pose, Rig, build_tracks, smooth, write_bbmodel, write_geckolib  # noqa: E402

OUT_DIR = HERE.parents[2] / "models" / "stitched_beast"

# ---------------------------------------------------------------- 常数
CHEW_ALLO = 4.6            # 咀嚼频率 f = CHEW_ALLO·M^−0.25 Hz（观察拟合，见 docstring）
V_MUSCLE = 5.0             # 肌肉最大缩短速率（倍肌长/秒）。哺乳类骨骼肌实测 4–8
V_FORCE_FRAC = 0.30        # 出力冲程走 V_max 的几成。Hill 曲线上功率最大点在 0.3 附近，
                           # 而咀嚼要的正是功率——既得合上，又得同时把东西碾断
EAR_FLICK_S = 0.12         # 耳廓转到位要多久（秒）。耳肌小、耳廓轻，快得多
BLINK_S = 0.15             # 一次眨眼
IDLE_BREATH = 0.9          # 静息呼吸频率 f = IDLE_BREATH·M^−0.26 Hz


def chew_hz(hd: HD.Head) -> float:
    return CHEW_ALLO * hd.donor.mass ** -0.25


def breath_hz(hd: HD.Head) -> float:
    return IDLE_BREATH * hd.donor.mass ** -0.26


def close_time(hd: HD.Head, *, force: bool = False) -> float:
    """合颌要多久（秒）。**由肌肉缩短速率定，不由力矩定**——见模块 docstring。

    `force=True` 时按**出力冲程**算而不是最快冲程。这不是两套公式，是 Hill 曲线上的
    两个点：肌肉收缩得越快出力越小，到 V_max 时出力归零。所以

      · 一次**扑咬**是最大速度动作——那一瞬间要的是快，不是咬得多重，走 V_max；
      · 一个**咀嚼**冲程必须在闭合的同时把食物碾断，得留出力气，走 ~0.3 V_max。

    两个数各自独立可查：咀嚼频率是按体重异速拟合来的，而出力冲程的闭合时间是从肌肉
    力学推的——它们必须对得上（闭合相塞得进一个周期里）。`check_head_anim` 会对拍。
    """
    arm = max((hd.tmj[0] - hd.occ) / hd.px_m, 0.02 * hd.donor.head_m)
    arm = max(arm, 0.12 * hd.donor.head_m)      # 力臂至少是颌长的一成多
    theta = math.radians(hd.gape)
    l_mus = max(0.25 * hd.donor.head_m, 1e-3)
    v = V_MUSCLE * (V_FORCE_FRAC if force else 1.0)
    if force:
        theta = math.radians(hd.diet.chew_deg)   # 咀嚼冲程只开合一个咀嚼角
    return arm * theta / (v * l_mus)


def chew_shift(hd: HD.Head) -> tuple[float, float]:
    """一个咀嚼冲程里齿列滑多远：(横向 px, 前后 px)。

    **这就是 `heads.tmj_lift` 那条推导。** 下颌绕高出咬合面 h 的关节转 θ，齿列被带着
    滑 θ·h。磨的那一类 h = 齿宽/θ，代回去正好滑过一颗臼齿的宽度；剪的那一类 h = 0，
    一丝不滑（滑动会把刃磨钝，而且关节在剪切时只能受压）。啮齿的滑动是前后向而不是
    横向——那是它能一边啃一边把碎屑推出去的原因。
    """
    lift = hd.tmj[0] - hd.occ                   # px
    d = math.radians(hd.diet.chew_deg) * lift
    if hd.diet.occlusion == "grind":
        return d, 0.0
    if hd.diet.occlusion == "gnaw":
        return 0.0, max(d, 0.35 * hd.L * 0.05)  # 啮齿关节不抬高，靠一条前后向的长槽
    return 0.0, 0.0


# ---------------------------------------------------------------- 骨名
def bones(hd: HD.Head) -> dict[str, str]:
    """这颗头实际存在的骨。没有耳廓的供体就没有耳骨，没有角的就没有角骨。"""
    out = {"head": GB.head_bone(hd, "skull"), "jaw": GB.head_bone(hd, "jaw")}
    have = {p.part for p in hd.pieces}
    for k, part in (("ear_l", "ear_l"), ("ear_r", "ear_r"),
                    ("horn_l", "horn_l"), ("horn_r", "horn_r")):
        if part in have:
            out[k] = GB.head_bone(hd, part)
    return out


def _ears(p: Pose, b: dict[str, str], pitch: float, yaw: float = 0.0) -> None:
    for k, sgn in (("ear_l", -1.0), ("ear_r", 1.0)):
        if k in b:
            p[b[k]].rot = [pitch, sgn * yaw, 0.0]


def cycles(hz: float, length: float) -> float:
    """循环动画在 length 秒里该走几个周期——**取整**，否则首末帧差半个冲程。

    和 `core_anim.breathe` 是同一条纪律。`anims()` 那边已经把时长摆成整数个周期了，
    这里再取一次整是防第二道：时长被 `max(..., 下限)` 钳过之后就不再是整数倍了。
    """
    return float(max(1, round(hz * max(length, 1e-6))))


# ---------------------------------------------------------------- 七条动作
def anim_idle(hd: HD.Head, rig: Rig, t: float, length: float) -> Pose:
    """静息：呼吸 + 耳朵的细微游移 + 偶尔一次咬合。

    呼吸频率同样按体重异速（0.9·M^−0.26）：鼠 1.2 Hz、牛 0.18 Hz。所以这只兽身上几颗
    来源不同的头**各喘各的**——和核心层各 lobe 异步搏动是同一条理由，只是这里的相位差
    不用另外造，供体体重本来就不一样。
    """
    p = Pose()
    b = bones(hd)
    u = t
    br = math.sin(2.0 * math.pi * cycles(breath_hz(hd), length) * u)
    p[b["head"]].rot = [0.8 * br, 0.0, 0.0]
    p[b["jaw"]].rot = [max(0.0, 1.6 * br), 0.0, 0.0]
    # 耳朵各转各的：两只耳同步会读成"一个开关在拨"，真兽的耳是各自独立扫的
    # **周期数必须是整数**，否则循环首末对不上、每一圈跳一下。两耳靠不同的整数周期数
    # 和不同的相位错开，而不是靠一个凑出来的非整数频率——那个只会让接缝坏掉。
    if "ear_l" in b:
        p[b["ear_l"]].rot = [2.5 * math.sin(2.0 * math.pi * (u * 2.0 + 0.13)), -3.0, 0.0]
    if "ear_r" in b:
        p[b["ear_r"]].rot = [2.5 * math.sin(2.0 * math.pi * (u * 3.0 + 0.61)), 3.0, 0.0]
    return p


def anim_chew(hd: HD.Head, rig: Rig, t: float, length: float) -> Pose:
    """咀嚼。**一个冲程的形状就是颌关节高度那条推导。**

    磨的那一类：下颌一边开合一边横滑（滑量 = θ·h，正好一颗臼齿宽），轨迹是个扁椭圆，
    而且**单侧咀嚼**——横滑只朝一边，那正是咬合力那条推导里 CHEW_FRAC=0.15 的来源。
    剪的那一类：纯上下剪刀，横滑恒为 0。啮齿：滑动改成前后向。
    吞的那一类（蛙）：根本不嚼，见 `anim_bite`。
    """
    p = Pose()
    b = bones(hd)
    ph = 2.0 * math.pi * cycles(chew_hz(hd), length) * t
    open_deg = hd.diet.chew_deg * 0.5 * (1.0 - math.cos(ph))
    sx, sz = chew_shift(hd)
    # 横滑落后开合四分之一周期：牙分开时滑过去，合上时碾回来——碾磨发生在闭合相，
    # 这是磨牙的全部意义。同相位滑动等于在张着嘴的时候空滑。
    lat = math.sin(ph - math.pi / 2.0)
    p[b["jaw"]].rot = [open_deg, 0.0, 0.0]
    p[b["jaw"]].pos = [sx * 0.5 * (lat + 1.0), 0.0, sz * lat]
    p[b["head"]].rot = [0.0, 0.0, 0.6 * lat if sx else 0.0]
    _ears(p, b, 1.5 * math.sin(ph), 2.0)
    return p


def anim_bite(hd: HD.Head, rig: Rig, t: float, length: float) -> Pose:
    """一次咬：张到最大张口角，再按肌肉缩短速率合上。

    张口角是推出来的（`Head.gape`）：吞食类要能把整只猎物塞进去，所以蛙张到 110°，
    而磨的那一类只有二十几度——它压根不需要张大嘴。合颌时间同样是推的（`close_time`），
    所以大兽合得慢、小兽合得快，不是统一给个 0.2 秒。
    """
    p = Pose()
    b = bones(hd)
    tc = close_time(hd)
    sec = t * length                             # 张口/合颌都是**秒**的量，得先换算回去
    open_s = max(length - tc, 1e-3) * 0.55
    if sec < open_s:
        s = smooth(sec / open_s)
        deg = hd.gape * s
    elif sec < open_s + tc:
        s = (sec - open_s) / tc
        deg = hd.gape * (1.0 - s * s)            # 合颌是加速的：肌肉一路在缩
    else:
        deg = 0.0
    p[b["jaw"]].rot = [deg, 0.0, 0.0]
    # 张嘴时头略仰，合上时随冲量一顿
    p[b["head"]].rot = [-0.25 * deg, 0.0, 0.0]
    _ears(p, b, -6.0 if deg > hd.gape * 0.4 else 0.0)
    return p


def anim_alert(hd: HD.Head, rig: Rig, t: float, length: float) -> Pose:
    """警觉：耳廓转向声源，头跟着微转。

    **只有有耳廓的供体做得出这条**。禽与两栖类没有外耳廓（鼓膜直接贴在体表），它们
    的警觉只能靠转头——所以这条动作在它们身上退化成一次纯转头，这不是省事，是它们
    本来就没有那两块可动的软骨。
    """
    p = Pose()
    b = bones(hd)
    u = t
    s = smooth(min(u / (EAR_FLICK_S / max(length, 1e-6)), 1.0)) if length > 0 else 1.0
    hold = 1.0 if u < 0.75 else smooth(max(0.0, 1.0 - (u - 0.75) / 0.25))
    if "ear_l" in b or "ear_r" in b:
        _ears(p, b, -4.0 * s * hold, 22.0 * s * hold)
        p[b["head"]].rot = [-2.0 * s * hold, 8.0 * s * hold, 0.0]
    else:
        p[b["head"]].rot = [-3.0 * s * hold, 26.0 * s * hold, 0.0]
    return p


def anim_threat(hd: HD.Head, rig: Rig, t: float, length: float) -> Pose:
    """威吓：耳朵压平贴头、头压低、微微张口。

    **耳朵压平不是表情，是护具。** 打起来耳廓是最容易被撕掉的部件（薄、血管密、伸在
    外面），所以打之前先收起来。有耳廓的才收得起来；没有的（禽、蛙）只剩压低和张口。
    张口只到最大张口角的四成——完全张开反而没有威胁，那是准备吞而不是准备咬。
    """
    p = Pose()
    b = bones(hd)
    u = t
    s = smooth(min(u * 3.0, 1.0)) * (1.0 if u < 0.7 else smooth(max(0.0, 1.0 - (u - 0.7) / 0.3)))
    _ears(p, b, -78.0 * s, -14.0 * s)
    p[b["head"]].rot = [14.0 * s, 0.0, 0.0]
    p[b["jaw"]].rot = [hd.gape * 0.40 * s, 0.0, 0.0]
    return p


def anim_butt(hd: HD.Head, rig: Rig, t: float, length: float) -> Pose:
    """顶角：抬头蓄力 → 挥下 → 撞击顿一下。**只有长角的供体有这条。**

    挥击的角速度由 `HornStyle.speed` 定——那正是角基粗细那条储能推导的输入（对撞动能
    必须在颅骨之前被角吃掉）。所以动画和几何用的是同一个数：改快了角会自动变粗，
    改慢了角会自动变细，两边不会各说各的。
    """
    p = Pose()
    b = bones(hd)
    st = HD.HORN_STYLE[hd.donor.horn] if hd.donor.horn else None
    if st is None:
        return anim_idle(hd, rig, t, length)
    # 挥击时长由**撞击速度**反解，不是拍一个数：角尖走的是 θ(s) = arc·s²（一路在加速），
    # 末端角速度 = 2·arc/T，乘上角伸出去的长度就是角尖线速度，令它等于 HornStyle.speed
    # ⇒ T = 2·arc·r / speed。这样几何（角基按这个速度的动能定粗细）和动画用的是同一个
    # 数——初版拍了个"arc·r/speed"，末速正好差一倍，自检量出角尖 17 m/s 对 4.5 m/s。
    arc = math.radians(70.0)
    r_tip = hd.horn_len / max(hd.px_m, 1e-6)     # 角伸出去多长（m）
    swing = 2.0 * arc * r_tip / max(st.speed, 1e-6)
    swing = min(max(swing, 0.06), length * 0.45)
    wind = length * 0.45
    sec = t * length                             # 蓄力/挥击/回弹全是**秒**的量
    if sec < wind:
        p[b["head"]].rot = [-34.0 * smooth(sec / wind), 0.0, 0.0]
    elif sec < wind + swing:
        s = (sec - wind) / swing
        p[b["head"]].rot = [-34.0 + 70.0 * s * s, 0.0, 0.0]     # 加速挥下
    else:
        # 回弹必须**从挥击的末值接上**（+36°），否则两段之间跳一下——那一跳在采样里
        # 是一个无穷大的角速度，自检量出角尖 17 m/s 而角基是按 4.5 m/s 推的。
        # 节律锚在挥击时长上：以挥击为单位的衰减振荡，峰值角速度恒为挥击末速的 0.74 倍，
        # 撞完不会抖得比撞还快。
        s = (sec - wind - swing) / max(swing, 1e-6)
        p[b["head"]].rot = [36.0 * math.exp(-1.6 * s) * math.cos(2.4 * s), 0.0, 0.0]
    _ears(p, b, -60.0)                            # 撞之前先把耳朵收起来
    return p


def anim_death(hd: HD.Head, rig: Rig, t: float, length: float) -> Pose:
    """死：下颌失张力垂开、耳朵塌下、头挂住。

    下颌**不是被打开的，是没人拉着了**。活着的时候闭口靠咬肌持续出力，力一撤，下颌
    就在自重下垂到限位。所以它是单调的、带一点末端过冲的下落，不是开合。
    """
    p = Pose()
    b = bones(hd)
    u = min(t, 1.0)
    s = 1.0 - (1.0 - u) ** 2
    p[b["jaw"]].rot = [hd.gape * 0.55 * s, 0.0, 0.0]
    _ears(p, b, -34.0 * s, 6.0 * s)
    p[b["head"]].rot = [22.0 * s, 5.0 * s, -7.0 * s]
    return p


def anim_graze(hd: HD.Head, rig: Rig, t: float, length: float) -> Pose:
    """采食：低头到地面、咬断、抬起来嚼。**只有食草/杂食做得出这条**——它的门齿和
    角质垫就是拿来在地面把草切断的。食肉的这条退化成一次低头嗅探（那正是它低头会做的
    事：吻端的鼻腔截面是它最大的器官）。
    """
    p = Pose()
    b = bones(hd)
    u = t
    down = smooth(min(u / 0.3, 1.0)) * (1.0 if u < 0.75 else smooth(max(0.0, 1.0 - (u - 0.75) / 0.25)))
    p[b["head"]].rot = [46.0 * down, 3.0 * math.sin(2.0 * math.pi * u * 2.0) * down, 0.0]
    if hd.diet.occlusion in ("grind", "crush", "gnaw") and 0.3 < u < 0.75:
        ph = 2.0 * math.pi * chew_hz(hd) * (u * length)
        p[b["jaw"]].rot = [hd.diet.chew_deg * 0.5 * (1.0 - math.cos(ph)), 0.0, 0.0]
        sx, _sz = chew_shift(hd)
        p[b["jaw"]].pos = [sx * 0.5 * (math.sin(ph - math.pi / 2.0) + 1.0), 0.0, 0.0]
    _ears(p, b, 4.0 * down, -6.0 * down)
    return p


# ---------------------------------------------------------------- 动作表
def anims(hd: HD.Head) -> dict:
    """这颗头做得出哪些动作。**由解剖决定，不是给每颗头一样的清单。**"""
    cyc = 1.0 / max(chew_hz(hd), 1e-6)
    out: dict[str, tuple[float, bool, int, object]] = {
        "head_idle": (max(2.0 / max(breath_hz(hd), 1e-6), 1.2), True, 40, anim_idle),
        "head_bite": (max(close_time(hd) * 3.2, 0.45), False, 36, anim_bite),
        "head_alert": (0.9, False, 28, anim_alert),
        "head_threat": (1.1, False, 30, anim_threat),
        "head_death": (1.6, False, 30, anim_death),
        "head_graze": (max(4.0 * cyc, 1.6), False, 44, anim_graze),
    }
    if hd.diet.occlusion != "gulp":              # 整吞的不嚼
        # 循环动画的时长必须是**整数个咀嚼周期**，否则首末帧差半个冲程，每圈跳一下。
        n_cyc = max(3, math.ceil(0.6 / cyc))
        out["head_chew"] = (n_cyc * cyc, True, 42, anim_chew)
    if hd.donor.horn:
        out["head_butt"] = (1.3, False, 34, anim_butt)
    return out


# ---------------------------------------------------------------- 渲染器接口
# `render_core_anim.py` 认的是 (MODEL, ANIMS, sample) 这三样。头颅这边一个供体一套，
# 所以先 `use(供体)` 把它们指过去——这样连拍/GIF 那套工具原样复用，不用另写一个。
MODEL: Path | None = None
ANIMS: dict = {}
_HD: HD.Head | None = None


def use(kind: str) -> None:
    global MODEL, ANIMS, _HD
    _HD, MODEL = build_head(kind)
    ANIMS = anims(_HD)


def sample(rig: Rig, name: str, t: float) -> Pose:
    length, _loop, _n, fn = ANIMS[name]
    return fn(_HD, rig, t, length)


def build_head(kind: str) -> tuple[HD.Head, Path]:
    """出一颗单独的头（含骨树），动画写在它上面。"""
    rig = GB.head_rig([kind])
    path = rig.save(OUT_DIR / f"Head_{kind}.bbmodel", f"Head_{kind}")
    import core as C
    import numpy as np
    sock = C.Socket(name=kind, kind="head", pos=np.array([0.0, 20.0, 12.0]),
                    normal=np.array([0.0, 0.0, -1.0]), bone="root", girth=4.0)
    hd = HD.solve_head(GN.HeadGene(kind, kind, 1.0), sock)
    return hd, path


def export(kind: str, *, quiet: bool = False) -> tuple[Path, Path]:
    hd, model = build_head(kind)
    rig = Rig(model)
    table = anims(hd)
    entries = []
    for name, (length, loop, n, fn) in table.items():
        tracks = build_tracks(
            rig, lambda t, f=fn, ln=length: f(hd, rig, t, ln), length, loop, n)
        entries.append((name, length, loop, tracks))
        if not quiet:
            kf = sum(len(v) for c in tracks.values() for v in c.values())
            print(f"  {name:<12} {length:>5.2f}s {'循环' if loop else '单次'}  "
                  f"{len(tracks):>2} 骨  {kf:>4} 关键帧")
    out_a = OUT_DIR / f"HeadRig_{kind}.bbmodel"
    out_g = HERE / f"stitched_beast_head_{kind}.animation.json"
    write_bbmodel(model, out_a, f"HeadRig_{kind}", entries)
    write_geckolib(out_g, "bong", f"stitched_beast_head_{kind}", entries)
    return out_a, out_g


def report(kind: str) -> str:
    hd, _ = build_head(kind)
    sx, sz = chew_shift(hd)
    return (f"  {kind:<8} 咀嚼 {chew_hz(hd):5.2f} Hz  呼吸 {breath_hz(hd):5.2f} Hz  "
            f"扑咬合颌 {close_time(hd) * 1000:5.0f} ms  "
            f"咀嚼闭合 {close_time(hd, force=True) * 1000:5.0f}/"
            f"{1000.0 / max(chew_hz(hd), 1e-6):5.0f} ms  张口 {hd.gape:5.1f}°  "
            f"横滑 {sx:4.2f} px  前后滑 {sz:4.2f} px  "
            f"{'角' if hd.donor.horn else ' '}{'耳' if hd.donor.pinna else ' '}  "
            f"{len(anims(hd))} 条")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--donor", default="wolf")
    ap.add_argument("--all", action="store_true")
    ap.add_argument("--list", action="store_true")
    args = ap.parse_args()

    if args.list:
        print("  供体     咀嚼频率  呼吸频率  合颌时间  张口角  齿列滑动        动作数")
        for k in sorted(GN.HEAD_TEMPLATES):
            print(report(k))
        return 0

    kinds = sorted(GN.HEAD_TEMPLATES) if args.all else [args.donor]
    for k in kinds:
        if k not in GN.HEAD_TEMPLATES:
            print(f"没有这个供体：{k}（有 {', '.join(sorted(GN.HEAD_TEMPLATES))}）")
            return 1
        print(f"{k}：")
        a, g = export(k, quiet=args.all)
        print(f"  → {a}\n  → {g}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
