#!/usr/bin/env python3
"""珂珂达（kekeda_goose）肌肉 / 软组织层 —— 骨架之上的第二层。

鸟的肌肉分布**极度偏心**，照哺乳类的直觉铺一遍必错：

  · 胸大肌一块就占体重近两成，整个挂在龙骨两侧，是全身最大的单块肌肉。
    雁是候鸟，胸肌是慢缩红肌 —— 深红，不是家鸡那种白胸脯。
  · 上乌喙肌埋在胸大肌底下，腱从**三骨孔**穿出去再拐到肱骨背面：一个真滑轮。
    肌肉长在胸骨下方，却负责把翅膀往上抬。
  · 踝以下几乎没有肌肉，只有腱。所以小腿是根光杆 —— 鸟腿看着细不是画瘦了。
  · 鹅的皮下脂肪厚得出名（鹅油），腹脂垫尤其大。参考照片那个"完美球体"，
    绒羽是主因，这层脂肪是次因，两者都算数。

本层同时是**外观层的形状来源**：body_profile / neck_radius / head_profile 三个函数
由 gen_plume 直接 import，绒羽按它们往外长。狮子那版是回头解析 bbmodel 反推包络，
这里改成解析式给出——鹅的绒羽厚到 2 单位以上，包络要能平滑求值才好算羽簇朝向。

用法:
  python3 modelScript/creatures/kekeda_goose/gen_muscle.py                 # 骨 + 肌
  python3 modelScript/creatures/kekeda_goose/gen_muscle.py --only-muscle   # 只肌肉
  python3 modelScript/creatures/kekeda_goose/gen_muscle.py --group breast  # 单肌群叠在骨架上
  python3 modelScript/creatures/kekeda_goose/gen_muscle.py --explode 5     # 延展视图
  python3 modelScript/creatures/kekeda_goose/gen_muscle.py --list
"""

from __future__ import annotations

import argparse
import math
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "core"))
sys.path.insert(0, str(HERE))

import gen_skeleton as SK  # noqa: E402
from bbmodel_maker.rig.voxel_rig import Palette, Rig, Vec, lerp, smoothstep  # noqa: E402

OUT_DIR = SK.OUT_DIR

MUSCLE_MATS = {
    "muscle": (156, 62, 56),        # 胸肌：候鸟的慢缩红肌，深红
    "muscle_deep": (118, 42, 40),
    "muscle_pale": (190, 108, 94),  # 腿部快缩肌，色浅
    "tendon": (228, 222, 202),
    "fat": (240, 228, 188),         # 皮下脂肪 / 腹脂垫
    "viscera": (178, 136, 110),     # 嗉囊
    "gland": (206, 168, 96),        # 尾脂腺
}
PALETTE = Palette({**SK.MATS, **MUSCLE_MATS})


# ================================================================ 体表包络
# (z, 半宽, 腹缘 y, 背缘 y) —— 带肉带脂之后的躯干轮廓，绒羽层从这上面往外长。
BODY: tuple[tuple[float, float, float, float], ...] = (
    (-5.30, 0.78, 8.35, 10.00),   # 胸前缘（叉骨联合 + 嗉囊前壁）
    (-4.20, 1.72, 6.80, 10.20),
    (-2.60, 2.62, 5.45, 10.52),
    (-1.00, 3.04, 5.05, 10.64),
    (0.60, 3.10, 5.05, 10.64),
    (2.20, 2.92, 5.45, 10.50),
    (3.60, 2.46, 6.30, 10.30),
    (5.00, 1.62, 7.60, 10.05),
    (6.40, 0.76, 8.90, 9.95),      # 尾根
)
BODY_Z0, BODY_Z1 = BODY[0][0], BODY[-1][0]


def body_profile(z: float) -> tuple[float, float, float]:
    """躯干在 z 处的 (半宽, 腹缘 y, 背缘 y)。超出两端按端点夹住。"""
    if z <= BODY_Z0:
        return BODY[0][1], BODY[0][2], BODY[0][3]
    if z >= BODY_Z1:
        return BODY[-1][1], BODY[-1][2], BODY[-1][3]
    for a, b in zip(BODY, BODY[1:]):
        if a[0] <= z <= b[0]:
            t = smoothstep((z - a[0]) / (b[0] - a[0]))
            return (lerp(a[1], b[1], t), lerp(a[2], b[2], t), lerp(a[3], b[3], t))
    return BODY[-1][1], BODY[-1][2], BODY[-1][3]


def neck_radius(t: float) -> float:
    """颈在参数 t 处的**带肉**半径。颈根粗（要接肩带和嗉囊），近头处收细。"""
    return lerp(1.34, 0.62, smoothstep(t ** 0.85))


def head_profile() -> tuple[Vec, tuple[float, float, float]]:
    """头（不含喙）的中心与三半轴。喙不长羽，绒羽层到这里为止。"""
    return SK.SKULL_C, (1.32, 1.30, 1.52)


# ================================================================ 胸区飞肌
PEC_TOP = ((-4.30, 9.10), (-2.60, 9.52), (-0.80, 9.05), (1.00, 8.24), (2.60, 7.30))


def _pec_top(z: float) -> float:
    if z <= PEC_TOP[0][0]:
        return PEC_TOP[0][1]
    if z >= PEC_TOP[-1][0]:
        return PEC_TOP[-1][1]
    for a, b in zip(PEC_TOP, PEC_TOP[1:]):
        if a[0] <= z <= b[0]:
            return lerp(a[1], b[1], smoothstep((z - a[0]) / (b[0] - a[0])))
    return PEC_TOP[-1][1]


PEC_SLICES = 9


def part_breast(rig: Rig) -> None:
    """胸大肌 + 上乌喙肌 + 止腱。全身最大的一块肉，龙骨的存在意义。"""
    for sx, side in ((-1, "l"), (1, "r")):
        for i in range(PEC_SLICES):
            z0 = lerp(-4.35, 2.65, i / PEC_SLICES)
            z1 = lerp(-4.35, 2.65, (i + 1) / PEC_SLICES)
            zm = (z0 + z1) / 2
            half, ylo, _ = body_profile(zm)
            # 贴着龙骨起、往外鼓到体宽；靠近中线留出龙骨板本身的厚度
            rig.cube("sternum", f"pectoralis_{side}_{i}",
                     (sx * 0.34, ylo, z0), (sx * half, _pec_top(zm), z1), mat="muscle")
        # 上乌喙肌：埋在胸大肌深面，紧贴龙骨。只有剖开/延展视图看得见
        rig.cube("sternum", f"supracoracoideus_{side}",
                 (sx * 0.32, SK.KEEL_BOTTOM_Y + 0.25, -3.95), (sx * 1.34, 7.65, 1.65), mat="muscle_deep")
        # 止腱：胸大肌 → 肱骨三角肌嵴（下扇）
        rig.shaft("sternum", f"pect_tendon_{side}",
                  (sx * 1.55, 9.05, -3.20), (sx * 2.00, 10.05, -2.55), 0.26, 0.30, mat="tendon")
        # 上乌喙肌腱：穿三骨孔后拐到肱骨**背面**（上扇）—— 滑轮的那一拐
        rig.shaft("sternum", f"supracor_tendon_{side}",
                  (sx * 1.28, 8.55, -3.35), (sx * 1.88, 10.42, -2.70), 0.15, 0.17, mat="tendon")


def part_trunk(rig: Rig) -> None:
    """腹壁 + 背肌 + 嗉囊 + 腹脂垫。"""
    for sx, side in ((-1, "l"), (1, "r")):
        for i in range(4):
            z0 = lerp(2.40, 5.20, i / 4)
            z1 = lerp(2.40, 5.20, (i + 1) / 4)
            half, ylo, yhi = body_profile((z0 + z1) / 2)
            rig.cube("hips", f"abdominal_{side}_{i}",
                     (sx * 0.30, ylo, z0), (sx * half, yhi - 1.05, z1), mat="muscle_pale")
        # 背肌：沿脊柱两侧的长条，鸟的躯干是刚性的，这条不粗但很长
        rig.cube("trunk_front", f"spinalis_{side}",
                 (sx * 0.24, SK.BACK_Y - 0.55, -3.30), (sx * 1.05, SK.BACK_Y + 0.34, 0.80), mat="muscle_deep")
        rig.cube("hips", f"lumbosacral_{side}",
                 (sx * 0.24, SK.BACK_Y - 0.75, 0.80), (sx * 1.15, SK.BACK_Y + 0.14, 4.30), mat="muscle_deep")
        # 腹脂垫：鹅的这块厚得出名，也是"球"的次因
        rig.cube("hips", f"fat_pad_{side}",
                 (sx * 0.28, 5.85, 2.20), (sx * 2.30, 7.05, 4.90), mat="fat")

    # 嗉囊：食道在颈根的膨大。吃饱了鼓成一包顶在胸前 —— 参考照片那种"前挺"
    # 有它一份功劳。位置居中，不参与左右镜像对拍。
    # 拆三级台阶而不是一整块：一整块 1.48×1.95×2.05 三维相近，渲出来是贴在胸前的
    # 一个褐方箱，比周围的肌肉还抢眼
    for i, (w, y0, y1, z0, z1) in enumerate((
        (1.34, 8.40, 9.55, -5.20, -4.05),
        (1.12, 9.10, 10.05, -4.35, -3.20),
        (0.82, 9.70, 10.40, -3.35, -2.40),
    )):
        rig.cube("trunk_front", f"crop_{i}", (-w, y0, z0), (w, y1, z1), mat="viscera")


def part_neck(rig: Rig) -> None:
    """颈肌：背侧一束（抬头）+ 腹侧一束（低头/前伸），沿 S 曲线逐节铺。

    17 节颈椎能折成 S、又能瞬间弹直去啄人，靠的就是这两束互为拮抗。
    """
    # 段数别贪多：首版切 14 段，每段弧长 0.5 而截面半宽 0.6~0.8 —— 段比自己还宽，
    # 渲出来是沿颈背立着的一排竖鳍，不是一束肌肉。段长必须明显大于截面，
    # 再靠 extend 让相邻段咬住。
    N = 8
    for i in range(N):
        t0, t1 = i / N, (i + 1) / N
        p0, p1 = SK.neck_at(t0), SK.neck_at(t1)
        bone = f"neck_{min(int(t0 * SK.NECK_VERTEBRAE), SK.NECK_VERTEBRAE - 1)}"
        r = neck_radius((t0 + t1) / 2)
        # 曲线切向 → 矢状面内的法向（绕 X 转 90°）：背束沿法向 +、腹束沿 −
        dy, dz = p1[1] - p0[1], p1[2] - p0[2]
        n = math.hypot(dy, dz) or 1.0
        ny, nz = -dz / n, dy / n
        for sign, mat, nm in ((1.0, "muscle_deep", "complexus"), (-1.0, "muscle_pale", "longus_colli")):
            off = r * 0.46 * sign
            a = (0.0, p0[1] + ny * off, p0[2] + nz * off)
            b = (0.0, p1[1] + ny * off, p1[2] + nz * off)
            rig.shaft(bone, f"{nm}_{i}", a, b, r * 0.54, r * 0.40, mat=mat, extend=0.22)


def part_wing(rig: Rig, sx: int, side: str) -> None:
    """肱二头 / 三头 + 前缘翼膜。收翼时翼膜是松的，展翼才绷成前缘。"""
    sh, elbow, wrist = (sx * 1.90, 10.05, -2.80), (sx * 2.62, 9.72, 1.15), (sx * 2.92, 10.05, -2.10)
    rig.shaft(f"wing_{side}", f"triceps_{side}",
              (sh[0] + sx * 0.12, sh[1] + 0.34, sh[2]), (elbow[0], elbow[1] + 0.30, elbow[2] - 0.20),
              0.34, 0.40, mat="muscle_pale")
    rig.shaft(f"wing_{side}", f"biceps_{side}",
              (sh[0] + sx * 0.10, sh[1] - 0.30, sh[2] + 0.10), (elbow[0], elbow[1] - 0.28, elbow[2] - 0.25),
              0.26, 0.32, mat="muscle")
    rig.shaft(f"forearm_{side}", f"forearm_flexor_{side}", elbow, wrist, 0.24, 0.30, mat="muscle_pale")
    # 前缘翼膜（propatagium）：肩 → 腕的那张皮，收翼时叠在体侧
    rig.cube(f"wing_{side}", f"propatagium_{side}",
             (sx * 2.30, 9.80, -2.65), (sx * 3.06, 10.42, 0.95), mat="tendon")


def part_leg(rig: Rig, sx: int, side: str) -> None:
    """髂胫肌（大腿外侧那张片）+ 腓肠肌（"腿肉"）+ 跗跖段只剩腱。"""
    hip, knee, ankle, toe_base = SK.leg_joints(sx)
    # 髂胫肌：从髂骨顶一路盖到膝，是腿最外层那张片 —— 整个埋在躯干轮廓里，
    # 所以鹅看着"没有大腿"
    rig.cube(f"femur_{side}", f"iliotibialis_{side}",
             (sx * 1.05, 6.85, -1.55), (sx * 2.62, 9.75, 2.85), mat="muscle_pale")
    rig.shaft(f"femur_{side}", f"iliofibularis_{side}",
              (sx * 2.05, 9.10, 2.35), (sx * 2.12, 7.05, 0.05), 0.46, 0.52, mat="muscle")
    # 腓肠肌：胫跗骨上 2/3，就是餐桌上那块"腿肉"
    mid = tuple(lerp(a, b, 0.55) for a, b in zip(knee, ankle))
    rig.shaft(f"tibia_{side}", f"gastrocnemius_{side}", knee, mid, 0.72, 0.76, mat="muscle")
    rig.shaft(f"tibia_{side}", f"tib_flexor_{side}",
              mid, tuple(lerp(a, b, 0.86) for a, b in zip(knee, ankle)), 0.44, 0.46, mat="muscle_pale")
    # 踝以下只有腱：鸟腿细不是画瘦了，是真没肌肉
    rig.shaft(f"tarsus_{side}", f"digital_flexor_{side}",
              (ankle[0], ankle[1] - 0.20, ankle[2] - 0.30), (toe_base[0], toe_base[1] + 0.20, toe_base[2] - 0.28),
              0.14, 0.15, mat="tendon")


def part_tail(rig: Rig) -> None:
    """尾部升降肌 + 尾脂腺。

    尾脂腺（uropygial gland）是水禽的防水油腺，尾根上一个实打实的鼓包 ——
    整天理毛涂油就是在够这个东西，也是"羽毛不沾水"的来源。
    """
    for sx, side in ((-1, "l"), (1, "r")):
        rig.cube("tail_base", f"levator_caudae_{side}",
                 (sx * 0.18, SK.trunk_y(5.2) - 0.15, 4.30), (sx * 0.92, SK.trunk_y(5.2) + 0.85, 6.45),
                 mat="muscle_pale")
        rig.cube("tail_base", f"depressor_caudae_{side}",
                 (sx * 0.18, SK.trunk_y(5.2) - 1.05, 4.30), (sx * 0.88, SK.trunk_y(5.2) - 0.20, 6.20),
                 mat="muscle_deep")
    # 尾脂腺鼓包要压在背轮廓**以内**。首版顶到 y=11.55，高出背线 1.2 单位，侧看
    # 是尾巴上翘着一块米黄楔子；真腺体只是尾根一个不到半单位的小丘
    rig.cube("tail_base", "uropygial_gland",
             (-0.60, SK.trunk_y(5.9) + 0.30, 5.30), (0.60, SK.BACK_Y + 0.34, 6.40), mat="gland")


# ================================================================ 装配
GROUPS: dict[str, tuple[str, object]] = {
    "breast": ("Breast: pectoralis / supracoracoideus / tendons", part_breast),
    "trunk": ("Trunk: abdominal / spinal / crop / fat pad", part_trunk),
    "neck": ("Neck: complexus / longus colli", part_neck),
    "wing": ("Wing: triceps / biceps / propatagium",
             lambda r: [part_wing(r, sx, s) for sx, s in ((-1, "l"), (1, "r"))]),
    "leg": ("Leg: iliotibialis / gastrocnemius / flexor tendons",
            lambda r: [part_leg(r, sx, s) for sx, s in ((-1, "l"), (1, "r"))]),
    "tail": ("Tail: levator / depressor / uropygial gland", part_tail),
}


def add_muscle(rig: Rig, only: str | None = None) -> None:
    for key, (_label, fn) in GROUPS.items():
        if only is None or key == only:
            fn(rig)  # type: ignore[operator]


def build(only_muscle: bool = False, group: str | None = None) -> Rig:
    """骨架先建（肌肉的 bone 挂在骨架的骨上），再叠肌肉；only_muscle 时抹掉骨 element。"""
    rig = Rig(PALETTE)
    src = SK.build_full()
    # 骨架 element 的 uv 是按 SK.PALETTE 算的，直接搬进来要求本层调色板的**前若干格
    # 与骨架逐位一致**（MUSCLE_MATS 追加在后，不插队）。错位了骨头会渲成肉色，
    # 而且是那种"看着还行"的错，所以在这儿断言掉。
    assert PALETTE.names[:len(SK.PALETTE.names)] == SK.PALETTE.names, \
        "调色板前缀与骨架不一致，搬过来的骨骼 uv 会指错色块"
    # 复用骨架的骨骼树；element 按需保留
    for name in src.bone_order:
        b = src.bones[name]
        rig.bone(name, tuple(b["pivot"]), b["parent"])
    if not only_muscle:
        for e in src.elements:
            rig.elements.append(e)
        for name in src.bone_order:
            rig.bones[name]["children"] = list(src.bones[name]["children"])
    add_muscle(rig, group)
    return rig


def explode(rig: Rig, amount: float) -> None:
    """延展视图：每块沿"离体轴的方向"平移，看清各件形状与附着点。

    体轴取 (0, 8, z)：鸟是纵长的，用单一质心当爆心会把头尾也横着推开。
    """
    for e in rig.elements:
        cx = (e["from"][0] + e["to"][0]) / 2
        cy = (e["from"][1] + e["to"][1]) / 2
        dx, dy = cx, cy - 8.0
        n = math.hypot(dx, dy) or 1.0
        off = (dx / n * amount, dy / n * amount, 0.0)
        for k in ("from", "to", "origin"):
            e[k] = [round(v + o, 3) for v, o in zip(e[k], off)]


def main() -> int:
    ap = argparse.ArgumentParser(description="珂珂达肌肉层")
    ap.add_argument("--only-muscle", action="store_true", help="不含骨架")
    ap.add_argument("--group", choices=sorted(GROUPS), help="只叠单个肌群（图集用）")
    ap.add_argument("--explode", type=float, metavar="D", help="延展视图，各件外移 D 单位")
    ap.add_argument("--list", action="store_true")
    ap.add_argument("--out", type=Path)
    args = ap.parse_args()

    if args.list:
        for k, (label, _) in GROUPS.items():
            print(f"  {k:7s} {label}")
        return 0

    rig = build(only_muscle=args.only_muscle, group=args.group)
    if args.explode:
        explode(rig, args.explode)

    name = "KekedaMuscle"
    if args.group:
        name += f"_{args.group}"
    elif args.only_muscle:
        name += "_bare"
    if args.explode:
        name += "_explode"

    problems = rig.mirror_problems()
    out = rig.save(args.out or (OUT_DIR / f"{name}.bbmodel"), name)
    lo, hi = rig.bounds()
    # 按 uv 数属软组织，不按 element["color"] —— color 是 index % 8，14 种材质下
    # 骨与肌会撞到同一个 color 值，数出来是错的
    soft_uv = {tuple(PALETTE.uv(m)) for m in MUSCLE_MATS}
    n_mus = sum(1 for e in rig.elements if tuple(e["faces"]["north"]["uv"]) in soft_uv)
    print(f"→ {out}")
    print(f"   cube {len(rig.elements)} 个（其中软组织 {n_mus}）· 骨骼 {len(rig.bones)} 根")
    print(f"   体宽 {hi[0] - lo[0]:.2f} · 站高 {hi[1]:.2f} · 纵长 {hi[2] - lo[2]:.2f}")
    if problems:
        print(f"   ✗ {len(problems)} 处镜像违例：")
        for p in problems[:10]:
            print(f"      {p}")
        return 1
    print("   ✓ 左右镜像通过")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
