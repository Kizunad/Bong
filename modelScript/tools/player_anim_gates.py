#!/usr/bin/env python3
"""玩家手持物动画的**几何后验门禁**，每道门旁边就是它自己的缺陷注入器。

## 为什么要有这个文件

`anim_common` 拦的是"写错了"（肘往身后折、循环首末不闭合），拦不住"写对了但看着不
对"：腰断、刀插进自己脑袋、割草的刀根本没探到草上、挥砍没离开身体。这些都是**世界
空间里的几何事实**，量得出来——但只有把刀真的挂到手上、把动画真的采样成逐帧姿态之后
才量得出来。测量层用 `preview_player_anim`（它那条挂点链逐字对齐 MC 运行时，且有
`test_anim_preview_fidelity` 逐点对拍锁死）。

## 每道门都自带注入器

modelScript/README「自检全绿在做差分注入之前，信息量是零」那一节的实践：判据本身会
假绿，而模型不会怀疑它。所以每道门旁边写一个**把它该抓的缺陷造出来**的注入器，
`--self-test` 先注入再跑，报不出违例的门直接算失效。

本文件里的门就是这么校准的——`hip_seam` 的门限 1.2px 不是拍的：静止姿实测 0.10px、
原版潜行约 1.7px、上一版 `herb_harvest` 实测 5.6~7.1px。1.2 卡在"补偿到位"和"忘了补偿"
之间，两边都留了三倍余量。

## 用法

    # 单条动画的门禁报告
    python3 modelScript/tools/player_anim_gates.py \\
        client/src/main/resources/assets/bong/player_animation/herb_harvest.json \\
        --hold modelScript/models/HerbKnifeIron.bbmodel --profile harvest

    # 差分自证：先注入缺陷再跑，报不出来的门算失效
    python3 modelScript/tools/player_anim_gates.py --self-test
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from dataclasses import dataclass, field
from pathlib import Path

import numpy as np

LIB = Path(__file__).resolve().parents[1]
from bbmodel_maker import workspace  # noqa: E402

_WS = workspace.Workspace.discover(start=Path(__file__))
REPO = _WS.root
for _d in (LIB / "tools", REPO / "client" / "tools"):
    if str(_d) not in sys.path:
        sys.path.insert(0, str(_d))

import preview_player_anim as P  # noqa: E402
import render_animation as RA  # noqa: E402
from anim_common import build_doc  # noqa: E402
from herb_knife_stance import (  # noqa: E402
    GROUND_SINK_MAX,
    HERB_ZONE,
    HIP_SEAM_MAX,
    SELF_CLIP_MAX,
    SLASH_REACH_Z,
)

ANIM_DIR = REPO / "client" / "src" / "main" / "resources" / "assets" / "bong" / "player_animation"
KNIFE_BB = LIB / "models" / "HerbKnifeIron.bbmodel"

#: 逐帧采样数。低于 30 抓不住"只在两个关键帧中间穿模"这类事故——引擎播的是插值，
#: 只查关键帧等于闭着眼睛过中间那一段。
SAMPLES = 41


# ================================================================ 测量层
@dataclass
class Frame:
    """一 tick 的世界空间事实（Bedrock px：脚底 y=0、-z 是身前、+x 是玩家左）。"""

    tick: float
    seg_pts: dict[str, np.ndarray]     # 分段名 → (8,3) 角点
    seg_xform: dict[str, np.ndarray]   # 分段名 → 4×4 变换（量解剖锚点用）
    knife_pts: np.ndarray              # (N,3) 刀的全部角点（含绳穗）
    blade_pts: np.ndarray              # (N,3) 只有刃（blade_*）

    @property
    def lowest(self) -> float:
        return min(float(v[:, 1].min()) for v in self.seg_pts.values())

    @property
    def hip_seam(self) -> float:
        """躯干底面中心与两条腿顶面中心之间的错位（px）。

        **量的是「同一个解剖点」被两边各自送到了哪儿**，不是最近角点距离：
        - 躯干那一侧取静止姿下的底面中心 `(0, 12, 0)`，跟着 torso 的变换走；
        - 腿那一侧取两条腿的顶面中心 `(±1.9, 12, 0)`（正好是髋枢轴），跟着腿的变换走。

        两个坑都踩过：① 取"最近角点距离"，躯干一拧 `yaw=30°` 就报 4.74px 违例，而图上
        腰是好的——判据把"拧腰"当成"腰断"；② 改成"取当前最高/最低的那几个点求均值"，
        腿一旋转，"最高点"就换成了某个旋上来的角，均值跟着漂，静止腿和抬腿量出两个数。
        锚在静止姿的解剖点上，纯 yaw 恒为 0，纯抬腿恒为 0，只有"胯被甩到身后"才报数。
        """
        torso = _apply(self.seg_xform["torso"], _TORSO_HIP)
        legs = np.mean([_apply(self.seg_xform[leg], anchor)
                        for leg, anchor in _LEG_TOPS.items()], axis=0)
        return float(np.linalg.norm(torso - legs))


#: 静止姿下的解剖锚点（Bedrock px）。躯干底面中心 = 腰线中点；两条腿的顶面中心 = 髋枢轴。
_TORSO_HIP = np.array([0.0, 12.0, 0.0])
_LEG_TOPS = {"rightLeg_up": np.array([-1.9, 12.0, 0.0]),
             "leftLeg_up": np.array([1.9, 12.0, 0.0])}


def _apply(T: np.ndarray, p: np.ndarray) -> np.ndarray:
    return T[:3, :3] @ p + T[:3, 3]


def _seg_rest_corners() -> dict[str, np.ndarray]:
    """分段几何在静止姿的 Bedrock 角点（8 个/段）。"""
    out = {}
    for name, pivot, frm, size, _uv in P.SEGMENTS:
        lo_mp = np.array([pivot[i] + frm[i] for i in range(3)], float)
        hi_mp = lo_mp + np.array(size, float)
        a, b = P._pt(lo_mp), P._pt(hi_mp)
        lo, hi = np.minimum(a, b), np.maximum(a, b)
        out[name] = np.array([[x, y, z] for x in (lo[0], hi[0])
                              for y in (lo[1], hi[1]) for z in (lo[2], hi[2])])
    return out


_REST = _seg_rest_corners()


def _knife_corners(doc: dict) -> tuple[np.ndarray, np.ndarray]:
    def corners(elements):
        return np.array([[x, y, z, 1.0] for e in elements
                         for x in (e["from"][0], e["to"][0])
                         for y in (e["from"][1], e["to"][1])
                         for z in (e["from"][2], e["to"][2])])
    return (corners(doc["elements"]),
            corners([e for e in doc["elements"] if e["name"].startswith("blade_")]))


def sample(emote: dict, knife_doc: dict, n: int = SAMPLES) -> list[Frame]:
    """把一条 emote 采成 n 个逐帧世界空间事实。"""
    kfs = RA.collect_keyframes(emote)
    end = float(emote["endTick"])
    display = knife_doc["display"]["thirdperson_righthand"]
    all_pts, blade_pts = _knife_corners(knife_doc)
    frames = []
    for i in range(n):
        tick = end * i / (n - 1)
        seg = P.segment_transforms(kfs, tick)
        pts = {name: (T[:3, :3] @ _REST[name].T).T + T[:3, 3] for name, T in seg.items()}
        H = P.hand_transform(kfs, tick, display)
        frames.append(Frame(tick, pts, seg,
                            (H @ all_pts.T).T[:, :3], (H @ blade_pts.T).T[:, :3]))
    return frames


def _aabb(pts: np.ndarray) -> tuple[np.ndarray, np.ndarray]:
    return pts.min(0), pts.max(0)


#: 分段几何在**自己静止局部系**里的盒（用来做精确容纳判定）。
_REST_BOX = {name: (np.array([pivot[i] + frm[i] for i in range(3)], float),
                    np.array([pivot[i] + frm[i] + size[i] for i in range(3)], float))
             for name, pivot, frm, size, _uv in P.SEGMENTS}


def penetration(points: np.ndarray, seg_name: str, xform: np.ndarray) -> float:
    """一团世界空间的点扎进某个身体段多深（px）。

    **不能用世界系 AABB 对 AABB**：手臂一转，刀的轴对齐包围盒就被撑成一个大方块，
    "刀在右胯外侧贴着腿"会被算成"刀在躯干里 4.4px"。差分自证之前这道门一直报 0.00，
    修好跨轴 bug 之后立刻变成两条动画都"自穿"——两次都是判据的问题，不是姿态的问题。

    正解是把点变换回**该段自己的静止局部系**（段的变换是刚体，可逆），再对静止盒做
    逐点容纳判定：在盒里的点，深度 = 它到最近盒面的距离；取最大。
    """
    lo, hi = _REST_BOX[seg_name]
    inv = np.linalg.inv(xform)
    local = (inv[:3, :3] @ points.T).T + inv[:3, 3]
    # 静止盒是在 ModelPart 系里写的，而 points 是 Bedrock 系；先把局部点翻回 ModelPart。
    local = np.stack([local[:, 0], 24.0 - local[:, 1], local[:, 2]], axis=1)
    inside = (local > lo) & (local < hi)
    hit = local[inside.all(axis=1)]
    if not len(hit):
        return 0.0
    return float(np.minimum(hit - lo, hi - hit).min(axis=1).max())


# ================================================================ 门
@dataclass
class GateResult:
    key: str
    label: str
    ok: bool
    worst: float
    limit: float
    detail: str
    extra: dict = field(default_factory=dict)

    def line(self) -> str:
        mark = "✓" if self.ok else "✗"
        return f"  {mark} {self.key:<12s} {self.label:<22s} 实测 {self.worst:7.2f} / 门限 {self.limit:6.2f}   {self.detail}"


def gate_hip_seam(frames, limit=HIP_SEAM_MAX) -> GateResult:
    """腰断门：躯干底面与腿顶面不许被躯干前倾甩开。

    `torso` 的枢轴在脖子，前倾会把胯端甩到身后 `12·sinθ`；不补腿的 z 就是腰断。
    上一版 `herb_harvest` 实测 5.6~7.1px。
    """
    worst = max(frames, key=lambda f: f.hip_seam)
    return GateResult("hip_seam", "腰断（髋缝）", worst.hip_seam <= limit,
                      worst.hip_seam, limit, f"最差在 t{worst.tick:.1f}")


def gate_ground(frames, limit=GROUND_SINK_MAX) -> GateResult:
    """穿地门：任何一段身体都不许陷到地面以下。"""
    worst = min(frames, key=lambda f: f.lowest)
    sink = -worst.lowest
    return GateResult("ground", "穿地", sink <= limit, sink, limit,
                      f"最深在 t{worst.tick:.1f}")


#: 刀不许穿进去的身体段。手臂下段（握刀那只）当然会跟刀重叠，排除。
_CLIP_PARTS = ("head", "torso", "leftArm_up", "leftArm_lo",
               "rightLeg_up", "rightLeg_lo", "leftLeg_up", "leftLeg_lo")


def gate_self_clip(frames, limit=SELF_CLIP_MAX) -> GateResult:
    """自穿门：刀身不许插进自己的头/躯干/另一条手臂/腿。"""
    worst, where = 0.0, ""
    for f in frames:
        for part in _CLIP_PARTS:
            d = penetration(f.knife_pts, part, f.seg_xform[part])
            if d > worst:
                worst, where = d, f"t{f.tick:.1f} 扎进 {part}"
    return GateResult("self_clip", "刀穿自己", worst <= limit, worst, limit,
                      where or "全程无接触")


def gate_torch_read(frames, limit=1.0) -> GateResult:
    """举火把门：刀尖不许高过肩线。

    判据抄 `DaggerBladeReadTest`：仰角好读但不直观，"刀尖越过肩"才是肉眼一眼判"这人
    在举火把不是持刀"的那个量。肩枢轴在 y=22。
    """
    worst, tick = -99.0, 0.0
    for f in frames:
        top = float(f.knife_pts[:, 1].max()) - 22.0
        if top > worst:
            worst, tick = top, f.tick
    return GateResult("torch", "刀尖过肩", worst <= limit, worst, limit,
                      f"最高在 t{tick:.1f}")


def gate_herb_zone(frames, impact: float, zone=HERB_ZONE, window: float = 1.5) -> GateResult:
    """采割门：割入帧那一刻，刃必须同时**够低**且**够前**。

    "在采药"这件事没有别的可观测量：读感全靠刃落到草的高度、并且越过躯干前脸。两个
    条件缺一不可，这是量出来的——上一版那条动画的刃在割入帧**更低**（y=11.68，比重做
    版的 13.11 还低），但它只是把刀垂在身前 3px 处，读作"站着让刀吊在肚子前面"。只卡
    高度的门会放它过，只卡前伸的门会放"把刀举到胸前"过。

    时间窗同样不是多余：上一版的刃在 **t0.3**（还没弯腰那一帧）就伸在身前 6px，然后
    割入帧反而收回来。刃**曾经**到过草区证明不了这是一条采药动画。
    """
    best, tick = -99.0, 0.0
    for f in frames:
        if abs(f.tick - impact) > window:
            continue
        low = float(f.blade_pts[:, 1].min())
        fwd = float(f.blade_pts[:, 2].min())
        margin = min(zone["y_max"] - low, zone["z_max"] - fwd)
        if low < zone["y_min"]:
            margin = min(margin, low - zone["y_min"])
        if margin > best:
            best, tick = margin, f.tick
    return GateResult("herb_zone", "刃探进草区", best > 0.0, best, 0.0,
                      f"最深在 t{tick:.1f}（草区 y{zone['y_min']:.0f}~{zone['y_max']:.0f}, z≤{zone['z_max']:.0f}）")


def gate_sweep(frames, window: tuple[float, float], min_path: float,
               reach_z: float = SLASH_REACH_Z) -> GateResult:
    """行程门：发力段里刃真的走了一段路，而且走到了身前。

    **为什么不用"最远前伸"当判据**：刀本身就有六七个像素长，手臂随便一摆刃就在身前
    10px 开外了。差分自证量过——把整段动作朝架势收到 45%（"幅度不够"这个最常见的
    失败），最远前伸只从 -13.51 掉到 -12.11，判据几乎没动。而**刃心走过的路程**从
    10.99px 掉到 3.98px，差 2.8 倍。够不够得着和有没有挥出去是两件事，这道门量后者。

    门限取干净版的七成：反手割 10.99 → 7.5，采割 6.21 → 4.3。
    """
    lo, hi = window
    path, prev, far = 0.0, None, 99.0
    for f in frames:
        if not (lo <= f.tick <= hi):
            continue
        centre = f.blade_pts.mean(axis=0)
        if prev is not None:
            path += float(np.linalg.norm(centre - prev))
        prev = centre
        far = min(far, float(f.blade_pts[:, 2].min()))
    ok = path >= min_path and far <= reach_z
    detail = f"t{lo:g}~{hi:g} 刃心行程；最远前伸 z={far:.2f}（门限 {reach_z:g}）"
    return GateResult("sweep", "发力段行程", ok, path, min_path, detail)


def gate_guard_return(emote: dict, limit=1e-6) -> GateResult:
    """收势门：末帧逐轴等于首帧，连着放第二遍不会跳一格。"""
    kfs = RA.collect_keyframes(emote)
    end = float(emote["endTick"])
    worst, where = 0.0, ""
    for part, axes in kfs.items():
        for axis in axes:
            d = abs(RA.sample_axis(kfs, part, axis, 0.0)
                    - RA.sample_axis(kfs, part, axis, end))
            if d > worst:
                worst, where = d, f"{part}.{axis}"
    return GateResult("guard", "首末帧一致", worst <= limit, worst, limit,
                      where or "无轨道")


def gate_settle(emote: dict, limit=1e-6) -> GateResult:
    """归架门：末帧逐轴等于 `herb_knife_stance.GUARD`。

    三条动画共用一个持刀架势，才接得上：`unfold` 甩开刃之后停在架势上、`harvest` 和
    `slash` 从架势起手也回到它。这条门锁的是**跨动画**的一致性——`guard` 那道门只管
    "自己的首末帧一样"，它拦不住三条各自收在三个不同的架势上。

    `unfold` 的首帧不等于末帧（它是"从没拿刀到拿着刀"的过渡），所以它只过这道门，
    不过 `guard` 那道。
    """
    from herb_knife_stance import guard_pose
    want = {k: v for k, v in guard_pose().items() if k != "easing"}
    kfs = RA.collect_keyframes(emote)
    end = float(emote["endTick"])
    worst, where = 0.0, ""
    for part, axes in want.items():
        for axis, value in axes.items():
            got = RA.sample_axis(kfs, part, axis, end)
            expect = math.radians(value) if axis not in "xyz" else value
            d = abs(got - expect)
            if d > worst:
                worst, where = d, f"{part}.{axis}"
    return GateResult("settle", "收在共用架势", worst <= limit, worst, limit,
                      where or "无轨道")


def gate_stagger(emote: dict, limit=0.0) -> GateResult:
    """错峰门：腿→腰→肩→肘→腕的峰速不许全压在同一 tick。

    conventions §2.2：全压一帧就是"咔一下全到位"的机器人感。判据取五条链路各自角速度
    峰值所在的 tick，要求**至少三个不同的 tick**。
    """
    kfs = RA.collect_keyframes(emote)
    end = float(emote["endTick"])
    chain = {
        "腿": [("rightLeg", "pitch"), ("leftLeg", "pitch")],
        "腰": [("torso", "pitch"), ("torso", "yaw")],
        "肩": [("rightArm", "pitch"), ("rightArm", "yaw")],
        "肘": [("rightArm", "bend")],
        "腕": [("rightArm", "roll")],
    }
    peaks = {}
    n = 61
    for name, axes in chain.items():
        prof = np.zeros(n - 1)
        for part, axis in axes:
            if axis not in kfs.get(part, {}):
                continue
            vals = np.array([RA.sample_axis(kfs, part, axis, end * i / (n - 1))
                             for i in range(n)])
            prof += np.abs(np.diff(vals))
        peaks[name] = round(end * int(prof.argmax()) / (n - 1), 2)
    distinct = len(set(peaks.values()))
    return GateResult("stagger", "峰值错峰", distinct >= 3, float(distinct), 3.0,
                      " ".join(f"{k}@t{v:g}" for k, v in peaks.items()))


# ================================================================ 注入器
#
# 每个注入器造的都是**对应那道门该抓的那种缺陷**，而且是照着上一版真实犯过的错造的。
class InjectionImpossible(RuntimeError):
    pass


def inject_no_hip_comp(pose: dict) -> dict:
    """抽掉上半身的 z 前移 —— 只留 `torso.pitch`，胯就被甩到身后，腰断。

    这正是上一版 `herb_harvest` 的成因（同一判据下实测 6.61px）。注入点必须跟着
    `herb_knife_stance.stance` 走：补偿从"腿往后挪"改成"上半身往前挪"之后，第一版
    注入器还在抽腿的 z，抽了个空——干净版和注入版都报 1.20px，零区分力。
    """
    out = _copy_pose(pose)
    for frame in out.values():
        for part in ("torso", "head", "rightArm", "leftArm"):
            if part in frame:
                frame[part] = {k: v for k, v in frame[part].items() if k != "z"}
    return out


def inject_sink(pose: dict, depth: float = 3.0) -> dict:
    """把两条腿的枢轴往下挪 `depth` px —— 脚陷进地里。

    第一版注入的是"给腿加 34° pitch"，那**抬不沉**：腿绕髋转只会把脚甩起来，几何上
    压根到不了地面以下（髋到脚底角最远 12.17px，比腿长只多 0.17px）。注入器造不出
    该抓的缺陷，那道门就永远是绿的、也永远没有信息量。改成挪枢轴才是真的下沉。
    """
    out = _copy_pose(pose)
    for frame in out.values():
        for leg in ("rightLeg", "leftLeg"):
            if leg in frame:
                frame[leg] = dict(frame[leg], y=depth)
    return out


def inject_knife_into_head(pose: dict) -> dict:
    """把持刀手抬到脸前 —— 自穿门的靶子。"""
    out = _copy_pose(pose)
    for frame in out.values():
        if "rightArm" in frame:
            frame["rightArm"] = dict(frame["rightArm"], pitch=-110.0, yaw=-60.0,
                                     roll=0.0, bend=60.0)
    return out


def inject_torch(pose: dict) -> dict:
    """把刀举过肩 —— 举火把门的靶子。"""
    out = _copy_pose(pose)
    for frame in out.values():
        if "rightArm" in frame:
            frame["rightArm"] = dict(frame["rightArm"], pitch=-150.0, bend=20.0)
    return out


def inject_undershoot(pose: dict, keep: float = 0.45) -> dict:
    """把整段动作朝架势收掉 —— 幅度不够，刀到不了该去的地方。

    每一帧沿"架势 → 本帧"这条线只走 `keep`，相当于动作只做了四成半。这是**最贴近真实
    失败**的注入：上一版三条动画的通病不是姿态错，是幅度太小（刃最远只离身体 0.3px）。

    第一版注入器写的是"手臂 pitch +25、肘再折 45°"，量出来只从 9.75 掉到 9.12——因为
    肘折起来把刀举到胸前，那儿照样在"身前 4px 以外"。判据和注入器一起改：草区的高度
    上限从腰线 18 收到 15（胸前那一档不算"探进草里"），注入器改成整体收幅度。
    """
    out = _copy_pose(pose)
    guard = {k: v for k, v in _guard().items() if k != "easing"}
    for frame in out.values():
        for part, axes in frame.items():
            if part == "easing" or part not in guard:
                continue
            frame[part] = {
                axis: guard[part].get(axis, 0.0) + keep * (value - guard[part].get(axis, 0.0))
                for axis, value in axes.items()
            }
    return out


def _guard() -> dict:
    from herb_knife_stance import guard_pose
    return guard_pose()


def inject_broken_guard(pose: dict) -> dict:
    """把末帧改成 vanilla neutral —— 上一版三条动画的收势写法。"""
    out = _copy_pose(pose)
    last = max(out)
    out[last] = dict(easing=out[last].get("easing", "INOUTSINE"),
                     torso=dict(pitch=0.0, yaw=0.0), head=dict(pitch=0.0, yaw=0.0),
                     rightArm=dict(pitch=0.0, yaw=0.0, roll=0.0, bend=0.0, axis=180.0),
                     leftArm=dict(pitch=0.0, yaw=0.0, roll=0.0, bend=0.0, axis=180.0),
                     rightLeg=dict(pitch=0.0, yaw=0.0, bend=0.0, axis=0.0),
                     leftLeg=dict(pitch=0.0, yaw=0.0, bend=0.0, axis=0.0))
    return out


def inject_simultaneous(pose: dict) -> dict:
    """把所有骨骼压成"两帧之间一步到位" —— 错峰门的靶子。

    做法：只留首帧和一个中间帧，中间帧所有轴同时到极值。
    """
    ticks = sorted(pose)
    if len(ticks) < 3:
        raise InjectionImpossible("动画少于 3 个关键帧，压不出'全压一帧'")
    first, mid, last = ticks[0], ticks[len(ticks) // 2], ticks[-1]
    return {first: _copy_frame(pose[first]), mid: _copy_frame(pose[mid]),
            last: _copy_frame(pose[last])}


def _copy_frame(frame: dict) -> dict:
    return {k: (dict(v) if isinstance(v, dict) else v) for k, v in frame.items()}


def _copy_pose(pose: dict) -> dict:
    return {t: _copy_frame(f) for t, f in pose.items()}


# ================================================================ 运行
#: 每条动画该过哪些门 + 它自报的撞击帧。`herb_zone` 只对采割有意义、`reach` 只对挥击
#: 有意义——把不该管的门挂上去，等于逼动画去做它不该做的动作。撞击帧写在这里是因为
#: 两道时间窗要拿它当基准："刃到过草区"和"刃在下刀那一刻到草区"是两回事。
PROFILES = {
    "harvest": dict(
        gates=("hip_seam", "ground", "self_clip", "torch", "herb_zone", "sweep",
               "guard", "settle", "stagger"),
        impact=6.0, sweep_window=(3.0, 8.0), sweep_min=4.3),
    "slash": dict(
        gates=("hip_seam", "ground", "self_clip", "torch", "sweep",
               "guard", "settle", "stagger"),
        impact=5.0, sweep_window=(2.0, 6.0), sweep_min=7.5),
    "draw": dict(
        gates=("hip_seam", "ground", "self_clip", "torch", "sweep", "settle", "stagger"),
        impact=5.0, sweep_window=(2.0, 6.0), sweep_min=6.0),
}


def run_gates(emote: dict, knife_doc: dict, profile: str) -> list[GateResult]:
    spec = PROFILES[profile]
    impact = spec["impact"]
    frames = sample(emote, knife_doc)
    table = {
        "hip_seam": lambda: gate_hip_seam(frames),
        "ground": lambda: gate_ground(frames),
        "self_clip": lambda: gate_self_clip(frames),
        "torch": lambda: gate_torch_read(frames),
        "herb_zone": lambda: gate_herb_zone(frames, impact),
        "sweep": lambda: gate_sweep(frames, spec["sweep_window"], spec["sweep_min"]),
        "guard": lambda: gate_guard_return(emote),
        "settle": lambda: gate_settle(emote),
        "stagger": lambda: gate_stagger(emote),
    }
    return [table[key]() for key in spec["gates"]]


def emote_of(pose: dict, end: int) -> dict:
    return build_doc(pose, name="probe", description="", end_tick=end,
                     stop_tick=end + 2)["emote"]


# ---------------------------------------------------------------- 自证
_SELF_TEST = (
    # (门, 注入器, 用哪条动画的 pose 当底子)
    ("hip_seam", inject_no_hip_comp, "herb_harvest"),
    ("ground", inject_sink, "herb_harvest"),
    ("self_clip", inject_knife_into_head, "herb_knife_slash"),
    ("torch", inject_torch, "herb_knife_slash"),
    ("herb_zone", inject_undershoot, "herb_harvest"),
    ("sweep", inject_undershoot, "herb_knife_slash"),
    ("guard", inject_broken_guard, "herb_harvest"),
    ("settle", inject_broken_guard, "herb_knife_unfold"),
    ("stagger", inject_simultaneous, "herb_knife_slash"),
)

_GEN = {
    "herb_harvest": ("gen_herb_harvest", 14),
    "herb_knife_slash": ("gen_herb_knife_slash", 10),
    "herb_knife_unfold": ("gen_herb_knife_unfold", 10),
}


def _load_pose(name: str) -> tuple[dict, int]:
    import importlib
    mod_name, end = _GEN[name]
    mod = importlib.import_module(mod_name)
    return mod.POSE, end


def self_test(knife_doc: dict) -> int:
    """先注入缺陷再跑：报不出违例的门算失效，返回失效门的个数。"""
    single = {
        "hip_seam": gate_hip_seam, "ground": gate_ground, "self_clip": gate_self_clip,
        "torch": gate_torch_read, "herb_zone": gate_herb_zone, "sweep": gate_sweep,
    }
    failed = 0
    print("差分自证（注入缺陷 → 门必须报违例）")
    for key, injector, anim in _SELF_TEST:
        pose, end = _load_pose(anim)
        clean = emote_of(pose, end)
        dirty = emote_of(injector(pose), end)
        if key in single:
            spec = PROFILES["harvest" if anim == "herb_harvest" else "slash"]
            args = ((spec["impact"],) if key == "herb_zone"
                    else (spec["sweep_window"], spec["sweep_min"]) if key == "sweep"
                    else ())
            before = single[key](sample(clean, knife_doc), *args)
            after = single[key](sample(dirty, knife_doc), *args)
        elif key == "guard":
            before, after = gate_guard_return(clean), gate_guard_return(dirty)
        elif key == "settle":
            before, after = gate_settle(clean), gate_settle(dirty)
        else:
            before, after = gate_stagger(clean), gate_stagger(dirty)
        good = before.ok and not after.ok
        failed += 0 if good else 1
        mark = "✓" if good else "✗ 失效"
        print(f"  {mark:<6s} {key:<10s} 注入 {injector.__name__:<24s} "
              f"干净 {before.worst:7.2f}({'过' if before.ok else '不过'}) → "
              f"注入后 {after.worst:7.2f}({'过' if after.ok else '不过'})")
        if not good:
            print(f"         ↑ 这道门对它该抓的缺陷没有区分力，等于没写")
    return failed


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("json", nargs="?", type=Path, help="player_animation JSON")
    ap.add_argument("--hold", type=Path, default=KNIFE_BB, help="手持物 bbmodel")
    ap.add_argument("--profile", choices=sorted(PROFILES), default="harvest")
    ap.add_argument("--self-test", action="store_true", help="差分自证")
    args = ap.parse_args()

    knife_doc = json.loads(args.hold.read_text(encoding="utf-8"))
    if args.self_test:
        return 1 if self_test(knife_doc) else 0
    if not args.json:
        ap.error("要么给一条动画 JSON，要么 --self-test")

    doc = json.loads(args.json.read_text(encoding="utf-8"))
    emote = doc.get("emote", doc)
    results = run_gates(emote, knife_doc, args.profile)
    print(f"{args.json.stem}  profile={args.profile}  endTick={emote['endTick']}")
    for r in results:
        print(r.line())
    bad = [r for r in results if not r.ok]
    print(f"  → {len(results) - len(bad)}/{len(results)} 过")
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main())
