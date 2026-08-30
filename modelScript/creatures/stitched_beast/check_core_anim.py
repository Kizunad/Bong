#!/usr/bin/env python3
"""异变缝合兽 —— 核心阶段动画物理自检。

最要紧的一条：**蠕动的锚段在世界系里必须静止**。这和有腿时"支撑相脚不滑步"是同一条
约束，只是"脚"换成了身体的锚段。导出的是循环动画（每周期净位移被减掉），所以这里
把位移加回去再验——验的是真实世界轨迹，不是动画曲线。

其余：净前进量 = CRAWL_D、体积守恒、循环接缝为零、idle 各 lobe 不同频（同频会读成
"一只动物在喘"）、嫁接单调且有停滞、死亡逐 lobe 依次泄气且终帧静止、全动画不穿地。

用法: python3 modelScript/creatures/stitched_beast/check_core_anim.py
"""

from __future__ import annotations

import sys
from pathlib import Path

import math
import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "core"))
sys.path.insert(0, str(HERE))

import core_anim as A  # noqa: E402
from bbmodel_maker.rig.anim_rig import Rig  # noqa: E402


def world_z(rig: Rig, pose, bone: str) -> float:
    return float(rig.joint(bone, rig.world(pose))[2])


def pose_delta(rig: Rig, a, b) -> float:
    """两姿态在所有骨上的最大通道差。"""
    worst = 0.0
    for n in rig.order:
        ca, cb = a[n] if n in a else None, b[n] if n in b else None
        for attr, d in (("rot", 0.0), ("pos", 0.0), ("scale", 1.0)):
            va = list(getattr(ca, attr)) if ca else [d] * 3
            vb = list(getattr(cb, attr)) if cb else [d] * 3
            worst = max(worst, *(abs(x - y) for x, y in zip(va, vb)))
    return worst


def main() -> int:
    if not A.MODEL.exists():
        print(f"缺 {A.MODEL}，先跑 gen_core.py")
        return 1
    rig = Rig(A.MODEL)
    bad: list[str] = []

    # ---- ① 蠕动：锚段世界静止（不滑步）
    N = 200
    for bone, lo, hi in (("core_hind", 0.00, 0.49), ("core_fore", 0.51, 0.99)):
        vals = []
        for k in range(N + 1):
            u = lo + (hi - lo) * k / N
            p = A.sample(rig, "core_crawl", u)
            vals.append(world_z(rig, p, bone) - A.CRAWL_D * u)   # 加回每周期净位移
        drift = max(vals) - min(vals)
        print(f"[蠕动] {bone} 锚定相 u∈[{lo:.2f},{hi:.2f}] 世界位移 {drift:.4f} px")
        if drift > 0.05:
            bad.append(f"蠕动 {bone} 在锚定相内世界位移 {drift:.3f} px——在蹭地（应为 0）")

    # ---- ② 净前进量恰为 CRAWL_D
    z0 = world_z(rig, A.sample(rig, "core_crawl", 0.0), "core_mid")
    z1 = world_z(rig, A.sample(rig, "core_crawl", 0.99999), "core_mid") - A.CRAWL_D * 0.99999
    adv = z0 - z1
    print(f"[蠕动] 每周期净前进 {adv:.3f} px（设计 {A.CRAWL_D:.3f}）")
    if abs(adv - A.CRAWL_D) > 0.05:
        bad.append(f"蠕动净前进 {adv:.3f} ≠ CRAWL_D {A.CRAWL_D:.3f}")

    # ---- ③ 体积守恒：拉长必须变细，乘积恒定
    vols = []
    for k in range(41):
        u = k / 40
        wf, wh, _gf, _gh = A.crawl_world(u)
        body = A.LOBE_SPAN + (wh - wf)
        r = np.sqrt(A.LOBE_SPAN / body)
        vols.append(r * r * body / A.LOBE_SPAN)
    spread = max(vols) - min(vols)
    print(f"[蠕动] 体积波动 {spread:.2e}")
    if spread > 1e-9:
        bad.append(f"蠕动体积不守恒，波动 {spread:.3e}——拉长时没等比变细")

    # ---- ④ 循环接缝
    #
    # 判据是**相对**的，不是绝对阈值：导出时末帧被强制等于首帧，所以真正会被看见的
    # 是"末帧前一帧 → 首帧"这一跳比正常帧间步子大多少。绝对阈值在这里不成立——
    # 乱抽里最快的那条芽一帧就要转几十度，接缝处 0.02° 的差是采样误差不是跳变，
    # 而慢动画里 0.02 的位移可能已经是明显一跳。同一个数字在两条动画里含义不同。
    for name, (length, loop, n, _f) in A.ANIMS.items():
        if not loop:
            continue
        step = max(pose_delta(rig, A.sample(rig, name, k / n), A.sample(rig, name, (k + 1) / n))
                   for k in range(n))
        d = pose_delta(rig, A.sample(rig, name, 0.0), A.sample(rig, name, 1.0 - 1.0 / n))
        print(f"[循环] {name} 接缝差 {d:.4f}（正常帧间步 {step:.4f}）")
        if d > step * 1.5 + 1e-6:
            bad.append(f"{name} 循环接缝跳变 {d:.3f}，是正常帧间步 {step:.3f} 的 "
                       f"{d / max(step, 1e-9):.1f} 倍")

    # ---- ⑤ idle：各 lobe 必须**不同频**
    def series(bone):
        return np.array([A.sample(rig, "core_idle", k / 96)[bone].scale[0] for k in range(96)])

    mains = [series(n) for n in A.LOBES_MAIN]
    worst_corr = 0.0
    for i in range(len(mains)):
        for j in range(i + 1, len(mains)):
            a, b = mains[i] - mains[i].mean(), mains[j] - mains[j].mean()
            den = np.linalg.norm(a) * np.linalg.norm(b)
            worst_corr = max(worst_corr, abs(float(a @ b / den)) if den > 1e-9 else 1.0)
    print(f"[idle] lobe 间最大相关 {worst_corr:.3f}")
    if worst_corr > 0.85:
        bad.append(f"idle 各 lobe 搏动过于同步（相关 {worst_corr:.2f}）——"
                   f"会读成'一只动物在喘'，而这是几团组织各喘各的")
    amp = max(float(s.max() - s.min()) for s in mains)
    if amp < 0.03:
        bad.append(f"idle 搏动幅度仅 {amp:.3f}，看不出来在动")

    # （嫁接的检查见 ⑦e —— 现在每个挂载点各有一条，不再是单独一条 core_graft）

    # ---- ⑦ 死亡：下沉单调、终段静止、赘生物先瘪
    ys = [A.sample(rig, "core_death", k / 60)["root"].pos[1] for k in range(61)]
    if any(b > a + 1e-6 for a, b in zip(ys, ys[1:])):
        bad.append("死亡时 root 上浮过——只能往下沉")
    still = pose_delta(rig, A.sample(rig, "core_death", 1.0), A.sample(rig, "core_death", 0.96))
    print(f"[死亡] 终帧下沉 {ys[-1]:.2f}px  终段位移 {still:.3f}")
    if still > 0.05:
        bad.append(f"死亡终段仍在动 {still:.3f}")
    lump = A.sample(rig, "core_death", 0.25)["nodule_r"].scale[1]
    body = A.sample(rig, "core_death", 0.25)["core_mid"].scale[1]
    if not lump < body - 0.10:
        bad.append(f"死亡时赘生物没有先瘪（t=0.25 赘 {lump:.2f} vs 主体 {body:.2f}）——"
                   f"接得最勉强的部分该最先散")

    # ---- ⑦b 扑击：爆发必须快过蓄力。速度比是恐惧的唯一来源，不是观感偏好。
    def zrate(t0: float, t1: float, n: int = 24) -> float:
        vs = [A.sample(rig, "core_lunge", t0 + (t1 - t0) * i / n)["root"].pos[2]
              for i in range(n + 1)]
        dur = (t1 - t0) * A.ANIMS["core_lunge"][0]
        return max(abs(b - a) for a, b in zip(vs, vs[1:])) / (dur / n)

    wind = zrate(0.04, A.LUNGE_WINDUP - 0.02)
    strike = zrate(A.LUNGE_WINDUP + 0.01, 0.99)
    print(f"[扑击] 蓄力 {wind:.0f} px/s · 爆发 {strike:.0f} px/s（×{strike / max(wind, 1e-6):.1f}）")
    if strike < wind * 2.5:
        bad.append(f"扑击爆发不够快：{strike:.0f} < 蓄力 {wind:.0f}×2.5——"
                   f"蓄放速度比是恐惧的唯一来源")

    # ---- ⑦c 包裹：必须真的张开再合上，且咽下后主体变大（吃进去的得有去处）
    fore = [A.sample(rig, "core_engulf", k / 60)["core_fore"].scale[0] for k in range(61)]
    if max(fore) - min(fore) < 0.5:
        bad.append(f"包裹张合幅度仅 {max(fore) - min(fore):.2f}——读成喘气不是吞噬")
    mid0 = A.sample(rig, "core_engulf", 0.0)["core_mid"].scale[0]
    mid1 = A.sample(rig, "core_engulf", 1.0)["core_mid"].scale[0]
    if mid1 <= mid0 + 0.10:
        bad.append(f"包裹结束时主体未变大（{mid0:.2f}→{mid1:.2f}）——吃进去的东西凭空消失了")

    # ---- ⑦d 乱抽：这条动画的三条主张，逐条量，不靠注释自称
    import core as C
    sc = A.thrash_scale()
    N = 160

    names = list(C.sockets())
    # 芽现在是**骨链**，尖端挂在最后一节上，不再是根骨上的一个偏移点。
    # 力臂取满长（模型坐标）：骨骼缩放由 rig 自己作用，这里再乘一次 sc 就是缩两次，
    # 量出来的摆幅会平白小掉一个 sc 倍（实测 0.45 被量成 0.19，误判成"没在摆"）
    tipbone = {n: A.tendril(n)[0][-1] for n in names}
    local = {n: np.asarray(C.bud_shape(C.sockets()[n], 1.0)[-1][0], float) for n in names}
    # 量**轨迹**不量角度：欧拉三元组对同一个姿态有多种写法，直接比会把等价姿态判成不同。
    # 每帧只做一次正解，17 条芽共用——逐芽各解一次的话这一段要跑十分钟。
    # 一次采样，三处复用（尖端轨迹 / 弯折 / 静止段）。分开各采一遍的话这一节要跑
    # 二十多分钟——同样的正解算三遍。
    frames = [A.sample(rig, "core_thrash", k / N) for k in range(N)]
    worlds = [rig.world(f) for f in frames]
    trk = {n: np.array([W[tipbone[n]][:3, :3] @ local[n] + W[tipbone[n]][:3, 3]
                        for W in worlds]) for n in names}

    # 摆幅按**各自的长度**归一化：短茬摆得少是对的（力臂就那么长），错的是相对自己
    # 都不怎么动。绝对阈值会把 vest_dr 这种 girth 1.40 的小槽误判成"没在摆"。
    # 门槛取**相对中位数**而不是写死：整条一根骨时尖端一次能扫过 1.26 倍自身长度
    # （78° 一把倒过去），改成四节各 19.5°、各自发火之后，同向对齐才有那么大，平时
    # 只有 0.3–0.6 倍。驱动模型一改绝对门槛就得跟着调，而这条真正想抓的是"有一条
    # 明显比同伴不动"——那是相对量。再兜一个 0.20 的绝对下限防整体退化。
    rel = {n: float(np.linalg.norm(v - v.mean(axis=0), axis=1).max()) / A.bud_reach(n, sc)
           for n, v in trk.items()}
    absol = {n: float(np.linalg.norm(v - v.mean(axis=0), axis=1).max()) for n, v in trk.items()}
    print(f"[乱抽] 芽尖摆幅 {min(absol.values()):.2f}..{max(absol.values()):.2f} px"
          f"（相对自身长度 {min(rel.values()):.2f}..{max(rel.values()):.2f}）")
    med = float(np.median(list(rel.values())))
    if min(rel.values()) < max(0.20, 0.5 * med):
        still = min(rel, key=lambda n: rel[n])
        bad.append(f"芽 {still} 尖端位移只有自身长度的 {rel[still]:.2f}——「所有触手都在摆」里它没在摆（摆满 0）")

    # 骨链必须**真的弯**：整条一根骨时它只能绕根部倾倒，末节与根节永远共线，
    # 渲出来是机械的（用户实测反馈）。量根节与末节的朝向夹角，直着就是没弯。
    worst_bend = 0.0
    for W in worlds[::3]:
        for n in names:
            ch = A.tendril(n)[0]
            d0 = W[ch[0]][:3, :3] @ C.sockets()[n].normal
            d1 = W[ch[-1]][:3, :3] @ C.sockets()[n].normal
            ang = math.degrees(math.acos(float(np.clip(np.dot(d0, d1) /
                  (np.linalg.norm(d0) * np.linalg.norm(d1)), -1, 1))))
            worst_bend = max(worst_bend, ang)
    print(f"[乱抽] 骨链最大弯折（根节 vs 末节朝向）{worst_bend:.1f}°")
    if worst_bend < 25.0:
        bad.append(f"骨链最大只弯了 {worst_bend:.1f}°——四节几乎共线，等于还是一根刚体棍子；"
                   f"检查 tendril_pose 是否给每节各自的驱动")

    # 抽搐之间**不许冻住**：一条只在发火那一瞬动、其余时间纹丝不动的触手读成机械臂。
    # 逐骨量帧间最大静止连续帧数（常驻漂移应当让它永远在动）。
    worst_frozen, frozen_bone = 0, ""
    for n in names:
        for b in A.tendril(n)[0]:
            run = best = 0
            for f0, f1 in zip(frames, frames[1:] + frames[:1]):
                if max(abs(x - y) for x, y in zip(f0[b].rot, f1[b].rot)) < 1e-4:
                    run += 1
                    best = max(best, run)
                else:
                    run = 0
            if best > worst_frozen:
                worst_frozen, frozen_bone = best, b
    print(f"[乱抽] 最长静止段 {worst_frozen}/{N} 帧（{frozen_bone}）")
    if worst_frozen > N * 0.25:
        bad.append(f"{frozen_bone} 有 {worst_frozen}/{N} 帧完全静止——"
                   f"抽搐之间该有常驻漂移（SWAY_DEG），冻住会读成机械臂")

    # 两两相关：同步的两条会被眼睛立刻配成一对，"各抽各的"就破了。
    #
    # 相关要算在**速率**上，不能算在位置上。位置是三维有符号量，相邻两个挂载点的法向
    # 本来就接近（比如 limb_ml 与 limb_hl 都在左肋），轨迹落在相近的平面里，光是方向
    # 相似就能把相关顶到 0.65——那衡量的是"朝向像不像"，不是"是不是一起动"。眼睛配对
    # 靠的是**同时动**，所以取速率序列。
    spd = {n: np.linalg.norm(np.diff(v, axis=0, append=v[:1]), axis=1) for n, v in trk.items()}
    worst, pair = 0.0, ("", "")
    for i, a in enumerate(names):
        for b in names[i + 1:]:
            x = spd[a] - spd[a].mean()
            y = spd[b] - spd[b].mean()
            den = np.linalg.norm(x) * np.linalg.norm(y)
            c = abs(float((x * y).sum() / den)) if den > 1e-9 else 1.0
            if c > worst:
                worst, pair = c, (a, b)
    print(f"[乱抽] 两两速率最大互相关 {worst:.3f}（{pair[0]} / {pair[1]}）")
    if worst > 0.55:
        bad.append(f"{pair[0]} 与 {pair[1]} 摆得太同步（相关 {worst:.2f}）——"
                   f"会被看成一对在打拍子；相位取自槽名噪声，检查 bud_flicks")

    # 载体必须**一动不动**：一堆东西在抽而载体是静的才瘆人，一起晃就读成整只在抖
    body = [A.sample(rig, "core_thrash", k / 24)["core_mid"].scale[0] for k in range(25)]
    if max(body) - min(body) > 1e-6:
        bad.append(f"乱抽时本体在动（core_mid 缩放 {min(body):.3f}..{max(body):.3f}）——"
                   f"载体一起晃会把抽搐稀释掉")
    # 摊出去的组织从自己身上来：体积守恒
    gained = sum(C.bud_tissue(s, 1.0) for s in C.sockets().values()) * sc ** 3
    whole = sum(C.lobe_mass().values()) * C.VOX ** 3
    lost = whole * (1.0 - body[0] ** 3)
    print(f"[乱抽] 芽增 {gained:.0f} px³ / 本体减 {lost:.0f} px³")
    if abs(gained - lost) > max(1.0, gained * 0.02):
        bad.append(f"乱抽体积不守恒：芽增 {gained:.0f} 本体减 {lost:.0f}——"
                   f"摊出去的料得从自己身上出，不能凭空长")

    # ---- ⑦e 每个挂载点都有自己的生长动画，且**真的各不相同**
    grafts = [n for n in A.ANIMS if n.startswith("graft_")]
    if len(grafts) != len(C.sockets()):
        bad.append(f"只有 {len(grafts)} 条嫁接动画，挂载点有 {len(C.sockets())} 个——"
                   f"「每一支都有生长动画」没做全")
    lens = {n: A.ANIMS[n][0] for n in grafts}
    print(f"[嫁接] {len(grafts)} 条，时长 {min(lens.values()):.2f}..{max(lens.values()):.2f}s，"
          f"不同值 {len(set(lens.values()))} 个")
    if len(set(lens.values())) < len(grafts) * 0.6:
        bad.append(f"嫁接时长只有 {len(set(lens.values()))} 种——时长该正比于该处用料，"
                   f"清一色说明 graft_length 没起作用")
    for n in grafts:
        sock = n[len("graft_"):]
        vs = [A.sample(rig, n, k / 160)[f"bud_{sock}"].scale[0] for k in range(161)]
        if any(b < a - 1e-6 for a, b in zip(vs, vs[1:])):
            bad.append(f"{n} 生长出现回退")
        if abs(vs[-1] - 1.0) > 0.02:
            bad.append(f"{n} 终值 {vs[-1]:.3f} ≠ 1.0，没长满")
        # 停滞段各占 4% 时长，采样必须密到能落进去：41 帧时每段只摊到一帧，
        # 会把本来有停滞的动画全判成匀速（实测 17 条全红）
        if sum(1 for a, b in zip(vs, vs[1:]) if b - a < 1e-6) < 4:
            bad.append(f"{n} 没有停滞段——匀速长大读成技能特效")
        # 只准动自己那一条：动画名指定哪个槽就只长哪个槽
        others = [m for m in C.sockets() if m != sock]
        mid = A.sample(rig, n, 0.5)
        wrong = [m for m in others if abs(mid[f"bud_{m}"].scale[0] - A.BUD_DORMANT) > 1e-6]
        if wrong:
            bad.append(f"{n} 顺带长了别的槽 {wrong[:3]}")

    # ---- ⑧ 受击：必须衰减回近似静止
    h = pose_delta(rig, A.sample(rig, "core_hurt", 1.0), A.sample(rig, "core_hurt", 0.0))
    print(f"[受击] 终帧与静止姿差 {h:.3f}")
    if h > 0.12:
        bad.append(f"受击终帧未回到静止（差 {h:.3f}），会和后续动画打架")

    # ---- ⑨ 全动画不穿地
    for name in A.ANIMS:
        low = min(rig.lowest(A.sample(rig, name, k / 24)) for k in range(25))
        if low < -0.6:
            bad.append(f"{name} 穿地 {low:+.2f}")

    if bad:
        print(f"\n✗ {len(bad)} 处违例：")
        for x in bad:
            print(f"   {x}")
        return 1
    print("\n✓ 不滑步 / 净前进 / 体积守恒 / 循环 / 异步搏动 / 嫁接 / 死亡 / 受击 / 不穿地 全通过")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
