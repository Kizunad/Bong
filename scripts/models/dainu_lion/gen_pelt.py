#!/usr/bin/env python3
"""怠怒之狮 —— 皮毛 / 外观层（Round 1/3）。

第三层，也是**最终进游戏的那层**：骨架和肌肉是解剖参考，皮层才是玩家看到的。

形状不靠另估一套数字，而是**从肌肉层的包络推导**——按骨骼分组读肌腹点云，
沿 z 取包络再平滑，外扩一点点当皮。这样前两层一改（收窄胸廓、削肌肉），
外形自动跟着变，不会出现"肌肉瘦了皮还鼓着"。

含：躯干/颈/四肢/尾的皮筒 · 头（耳/鼻镜/髭须）· 眼（巩膜+虹膜+竖瞳）·
脚（掌垫/趾垫/角质爪）· 鬃毛与尾梢（黑火——焦黑毛簇 + 余烬色edge）。

用法:
  python3 scripts/models/dainu_lion/gen_pelt.py              # 完整外观
  python3 scripts/models/dainu_lion/gen_pelt.py --with-anatomy  # 叠在骨+肌上看包裹关系
  python3 scripts/models/dainu_lion/gen_pelt.py --group mane
  python3 scripts/models/dainu_lion/gen_pelt.py --list
"""

from __future__ import annotations

import argparse
import base64
import io
import json
import math
import uuid
import zlib
from pathlib import Path

from gen_muscle import Skeleton
from gen_skeleton import SACRUM_Z, shaft_box, spine_y
from PIL import Image

REPO = Path(__file__).resolve().parents[3]
MODELS = REPO / "local_models" / "dainu_lion"
MUSCLE = MODELS / "DainuLionMuscle.bbmodel"

# 皮层材质放贴图第 3 行起（前两行是骨/肌，UV 不动）。
#
# 不走真狮的沙金配色——这是被黑火烧了不知多少年的东西。整体压到焦灰，
# 明度区间只剩 pelt(62) → pelt_light(92) 一档窄幅，靠"炽"色单点破局：
# 眼、鬃梢、皮下裂纹是画面里唯一的暖色，其余全是死灰。对比越集中越压人。
PELT_MATS = {
    "pelt": (45, 39, 34),  # 主体：燎过的烟灰褐
    "pelt_dark": (27, 24, 21),  # 背脊 / 阴影：近黑
    "pelt_light": (66, 58, 50),  # 提亮面：冷灰（不是奶油色）
    "ash": (88, 82, 76),  # 灰烬霜：肩背落灰、眉骨积灰
    "mane": (17, 15, 15),  # 鬃毛：纯焦黑
    "mane_ember": (163, 51, 12),  # 鬃梢余烬
    "ember_hot": (243, 130, 32),  # 炽芯：裂纹底下的黑火
    "char": (23, 19, 18),  # 炭化裂口
    "scar": (99, 43, 32),  # 陈年伤疤
    "claw": (20, 18, 18),  # 黑角质爪
    "pad": (28, 24, 23),
    "nose": (18, 15, 15),
    "eye_glow": (238, 176, 78),  # 虹膜下半：真虹膜下缘受光更亮
    "eye_iris": (204, 134, 44),  # 虹膜主体：琥珀金（照片色，不再是纯熔铁橙）
    "eye_rim": (86, 46, 16),  # 虹膜外缘深色环 —— 眼睛"活不活"一半靠这圈
    "eye_pupil": (8, 7, 7),  # 竖瞳
    "fang": (208, 200, 186),
    "muzzle_pale": (86, 78, 69),  # 口鼻：冷灰
    "whisker": (94, 88, 82),  # 燎过的须：压到和落灰同档，别跟眼睛抢焦点
}
SWATCH = 8
PELT_ROW = 2  # 第 3 行起

TORSO_BONES = ("hips", "lumbar", "thorax_back", "thorax_front")

# 脸弧：正面不是一块平板，而是一段柱面。半径/中心由两个约束反解——
# 鼻梁处(x=0)前缘 z=-26.0、颊侧(x=3.0)前缘 z=-24.9，即整张脸横向包 1.1 单位。
# 眼、眉、额全部长在这条弧上，所以不会出现"平板脸上贴两个方块眼"。
FACE_R = 4.64
FACE_ZC = -21.36


def face_z(x: float) -> float:
    """脸弧在横坐标 x 处的前缘 z（越靠外越靠后）。"""
    return FACE_ZC - math.sqrt(max(0.0, FACE_R * FACE_R - x * x))


# 全脸共用一套列网格。**必须共用**：脸壳按 9 列、眼睛按 4 列各切各的时，列边界
# 错开，脸壳某列的前缘会挡在虹膜前面把眼睛啃掉一条——实测眼睛被切成两块孤立橙斑。
FACE_X0, FACE_X1, FACE_N = -3.15, 3.15, 15
FACE_STEP = (FACE_X1 - FACE_X0) / FACE_N


def arc_cols(x0: float, x1: float):
    """取脸弧网格上覆盖 [x0,x1] 的列 → (左, 右, 中, 归一化位置 t, 该列前缘 z)。"""
    i0 = max(0, round((x0 - FACE_X0) / FACE_STEP))
    i1 = min(FACE_N, round((x1 - FACE_X0) / FACE_STEP))
    n = max(1, i1 - i0)
    for i in range(i0, i1):
        xa = FACE_X0 + i * FACE_STEP
        xb = xa + FACE_STEP
        xm = (xa + xb) / 2
        yield xa, xb, xm, (i - i0 + 0.5) / n, face_z(xm)


def _lerp(a, b, t):
    return a + (b - a) * t


def _rotmat(rot):
    def R(deg, ax):
        a = math.radians(deg)
        c, s = math.cos(a), math.sin(a)
        if ax == 0:
            return ((1, 0, 0), (0, c, -s), (0, s, c))
        if ax == 1:
            return ((c, 0, s), (0, 1, 0), (-s, 0, c))
        return ((c, -s, 0), (s, c, 0), (0, 0, 1))

    def mul(m, n):
        return tuple(tuple(sum(m[i][k] * n[k][j] for k in range(3)) for j in range(3)) for i in range(3))

    return mul(mul(R(rot[2], 2), R(rot[1], 1)), R(rot[0], 0))


def _corners(e):
    f, t = e["from"], e["to"]
    org = e.get("origin", [0, 0, 0])
    rot = e.get("rotation", [0, 0, 0])
    pts = [(x, y, z) for x in (f[0], t[0]) for y in (f[1], t[1]) for z in (f[2], t[2])]
    if not any(rot):
        return pts
    m = _rotmat(rot)
    out = []
    for p in pts:
        d = (p[0] - org[0], p[1] - org[1], p[2] - org[2])
        out.append(tuple(org[i] + sum(m[i][k] * d[k] for k in range(3)) for i in range(3)))
    return out


def element_bone_map(data: dict) -> dict:
    """element uuid → 所属骨骼名（fmt 4.x / 5.0 通吃）。"""
    groups = {g["uuid"]: g for g in data.get("groups", [])}
    out: dict[str, str] = {}

    def walk(node, bone=None):
        if isinstance(node, str):
            if bone:
                out[node] = bone
            return
        meta = groups.get(node["uuid"], node)
        name = meta.get("name", bone)
        for c in node.get("children", []):
            walk(c, name)

    for root in data["outliner"]:
        walk(root)
    return out


class Envelope:
    """肌肉层按骨骼分组的 z 向包络，供皮层贴着长。"""

    def __init__(self, muscle_path: Path) -> None:
        data = json.loads(muscle_path.read_text())
        bmap = element_bone_map(data)
        self.pts: dict[str, list] = {}
        for e in data["elements"]:
            if not e.get("_muscle"):
                continue
            bone = bmap.get(e["uuid"])
            if bone is None:
                continue
            self.pts.setdefault(bone, []).extend(_corners(e))

    def slice(self, bones, z: float, band: float = 1.6):
        """该 z 附近的 (半宽, y下, y上)；无数据返回 None。"""
        xs, ys = [], []
        for b in bones:
            for p in self.pts.get(b, ()):
                if abs(p[2] - z) <= band:
                    xs.append(abs(p[0]))
                    ys.append(p[1])
        if len(xs) < 4:
            return None
        return (max(xs), min(ys), max(ys))

    def smooth_profile(self, bones, z0: float, z1: float, step: float, margin: float, shrink: float = 0.88):
        """沿 z 采包络并平滑。shrink<1 = 别被局部凸起（肩肌）撑大整条皮。"""
        raw = []
        z = z0
        while z <= z1 + 1e-6:
            s = self.slice(bones, z)
            raw.append((z, s))
            z += step
        vals = [s for _, s in raw if s]
        if not vals:
            raise SystemExit("包络为空：肌肉层是否已生成？")
        out = []
        for i, (z, s) in enumerate(raw):
            near = [raw[j][1] for j in range(max(0, i - 2), min(len(raw), i + 3)) if raw[j][1]]
            if not near:
                near = vals
            hw = sum(n[0] for n in near) / len(near) * shrink + margin
            lo = sum(n[1] for n in near) / len(near) - margin
            hi = sum(n[2] for n in near) / len(near) + margin
            out.append((z, hw, lo, hi))
        return out


# ---------------------------------------------------------------- 造型
class Pelt:
    def __init__(self, skel: Skeleton) -> None:
        self.skel = skel
        self.count = 0

    def box(self, bone, name, frm, to, *, rot=None, org=None, mat="pelt"):
        f = [round(min(a, b), 3) for a, b in zip(frm, to)]
        t = [round(max(a, b), 3) for a, b in zip(frm, to)]
        self.skel.attach(
            bone,
            {
                "name": name,
                "box_uv": False,
                "rescale": False,
                "locked": False,
                "render_order": "default",
                "allow_mirror_modeling": True,
                "type": "cube",
                "uuid": str(uuid.uuid4()),
                "_pelt": True,
                "from": f,
                "to": t,
                "autouv": 0,
                "color": 2,
                "origin": [round(v, 3) for v in (org or [(a + b) / 2 for a, b in zip(f, t)])],
                "rotation": [round(v, 3) for v in (rot or (0, 0, 0))],
                "faces": _faces(mat),
            },
        )
        self.count += 1

    def limb(self, bone, name, a, b, r0, r1=None, *, mat="pelt", flat=1.0):
        """沿关节 a→b 包一节皮筒（两端半径可不同）。"""
        r1 = r0 if r1 is None else r1
        r = (r0 + r1) / 2
        frm, to, rot, org = shaft_box(a, b, r * flat, r)
        self.box(bone, name, frm, to, rot=rot, org=org, mat=mat)


def _faces(mat: str) -> dict:
    i = list(PELT_MATS).index(mat)
    ox, oy = (i % 8) * SWATCH, (PELT_ROW + i // 8) * SWATCH
    uv = [ox + 1.0, oy + 1.0, ox + SWATCH - 1.0, oy + SWATCH - 1.0]
    return {d: {"uv": list(uv), "texture": 0} for d in ("north", "south", "east", "west", "up", "down")}


def extend_texture(data: dict) -> None:
    src = data["textures"][0]["source"].split(",", 1)[1]
    img = Image.open(io.BytesIO(base64.b64decode(src))).convert("RGBA")
    px = img.load()
    for i, (_name, (r, g, b)) in enumerate(PELT_MATS.items()):
        ox, oy = (i % 8) * SWATCH, (PELT_ROW + i // 8) * SWATCH
        for y in range(SWATCH):
            for x in range(SWATCH):
                n = ((x * 7 + y * 13 + i * 5) % 5) - 2
                px[ox + x, oy + y] = (
                    max(0, min(255, r + n * 4)),
                    max(0, min(255, g + n * 4)),
                    max(0, min(255, b + n * 3)),
                    255,
                )
    buf = io.BytesIO()
    img.save(buf, format="PNG")
    data["textures"][0]["source"] = "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode()


# ================================================================ 部位
def part_torso(p: Pelt, env: Envelope, P) -> None:
    """躯干皮筒：沿肌肉包络逐段成形，背侧压暗、腹侧提亮。"""
    # 分段放疏（2.0 → 3.25）：段越密、段间 hw 差越显眼，背上就是一排竖条
    prof = env.smooth_profile(TORSO_BONES, -13.0, 13.0, 3.25, margin=0.35)
    for i, (z, hw, lo, hi) in enumerate(prof[:-1]):
        z1 = prof[i + 1][0]
        bone = "thorax_front" if z < -6 else ("thorax_back" if z < 4 else "lumbar" if z < 10 else "hips")
        # 腰收 + 收腹（tuck-up）：猫科侧影的关键。肌肉包络是平的，直接照抄
        # 会把躯干读成一只盒子——胸廓鼓、腰细、腹线自胸底向后上收。
        waist = 1.0 - 0.13 * math.exp(-(((z - 6.0) / 4.0) ** 2))
        hw *= waist
        tuck = 2.6 / (1.0 + math.exp(-(z - 3.0) / 2.2))  # 自 z≈3 起腹线上抬
        lo += tuck
        # 三层堆出六边形截面：单个矩形筒渲出来是根方管子
        span = hi - lo
        for tag, y0f, y1f, wf, mat in (
            ("belly", 0.0, 0.34, 0.72, "pelt"),
            ("flank", 0.30, 0.74, 1.0, "pelt"),
            ("back", 0.70, 1.0, 0.80, "pelt_dark"),
        ):
            p.box(bone, f"torso_{tag}_{i + 1}",
                  (-hw * wf, lo + span * y0f, z), (hw * wf, lo + span * y1f, z1), mat=mat)

    # 背中线暗带：跨段连续画，避免逐段接缝
    for j in range(0, len(prof) - 1, 2):
        z0 = prof[j][0]
        z1 = prof[min(j + 2, len(prof) - 1)][0]
        hi0 = min(prof[k][3] for k in range(j, min(j + 3, len(prof))))
        p.box("thorax_back" if z0 < 4 else "lumbar", f"dorsal_{j // 2 + 1}",
              (-1.4, hi0 - 0.5, z0), (1.4, hi0 + 0.35, z1), mat="pelt_dark")



def part_neck(p: Pelt, env: Envelope, P) -> None:
    """颈皮筒：从肩前接到枕部，比躯干细但仍厚实（鬃毛另算）。"""
    a = P("neck_base")
    b = P("skull")
    for i in range(4):
        t0, t1 = i / 4, (i + 1) / 4
        z0 = _lerp(a[2], b[2] + 1.5, t0)
        z1 = _lerp(a[2], b[2] + 1.5, t1)
        y0 = spine_y(z0)
        y1 = spine_y(z1)
        hw = _lerp(4.3, 3.5, t0)
        p.limb("neck_base" if i < 2 else "neck_mid", f"neck_{i + 1}",
               (0, y0 - 1.2, z0), (0, y1 - 1.2, z1), hw, hw * 0.95)
def part_head(p: Pelt, env: Envelope, P) -> None:
    """头与脸。

    **颅形随 z 前收**——这是"方块脸"的根因：颅骨本来就是锥的（颧弓半宽 4.65 在
    后颅，到泪骨 3.10、上颌 2.20、鼻骨 1.50），首版皮层整段套了同一个 ±4.4，
    把这段锥度抹平，正面读出来就是一块砖。现在分段跟着骨走，高/宽从 0.89
    拉到 ~1.35。

    压迫感靠三处结构，不靠涂黑：① 前伸的眉脊把眼睛压进阴影里 ② 拔高的矢状嵴
    ③ 皮下透出黑火的炭裂。
    """
    # ---- 颅：分段前收（数值贴着骨骼实际半宽，别再整段等宽）----
    for i, (z0, z1, hw, y0, y1) in enumerate((
        (-17.0, -20.6, 3.95, 22.4, 28.2),  # 后颅 / 颞肌，最宽
        (-20.6, -23.2, 3.80, 22.3, 28.7),  # 颧弓 / 眼后
    )):
        p.box("skull", f"head_shell_{i + 1}", (-hw, y0, z0), (hw, y1, z1))

    # 眼区（原第三段）改成沿脸弧分列：中间前凸、两侧后收，正面轮廓不再是硬直角
    for i, (xa, xb, _xm, _t, zf) in enumerate(arc_cols(-3.15, 3.15)):
        p.box("skull", f"face_arc_{i + 1}", (xa, 22.5, -23.2), (xb, 28.4, zf))

    # 矢状嵴：顺着骨上的 sagittal_crest 拔一条高脊，把头拉长、侧影出棱
    p.box("skull", "sagittal_ridge", (-1.45, 28.2, -23.0), (1.45, 29.9, -18.0), mat="pelt_dark")
    p.box("skull", "crest_ash", (-0.85, 29.6, -22.4), (0.85, 30.1, -19.0), mat="ash")

    for sx, side in ((-1, "l"), (1, "r")):
        # 眉：沿脸弧分列，靠外逐列抬高——曲线压眉，不是一根转了角度的直棍
        for i, (xa, xb, _xm, t, zf) in enumerate(arc_cols(1.05, 3.15)):
            y = 26.34 + 0.52 * t
            p.box("skull", f"brow_ridge_{side}_{i + 1}",
                  (sx * xa, y, zf - 0.22), (sx * xb, y + 0.44, -23.4), mat="pelt_dark")
        # 皱眉纹：两道向鼻梁上方汇聚的斜刻，正面看是个"皱着眉盯人"的 V
        p.box("skull", f"brow_furrow_{side}", (sx * 0.25, 26.5, -26.35), (sx * 1.2, 26.82, -25.5),
              rot=(0.0, 0.0, sx * 30.0), org=(sx * 0.7, 26.66, -25.9), mat="char")

        # 颊：**随 z 前收**（这才是"方块脸"的真凶——原来一整块 hw 4.35 竖墙
        # 从眼一路铺到嘴，把正面轮廓撑到 8.7 宽）。顶面压到 25.6 让位给眉脊。
        for k, (z0, z1, hw) in enumerate((
            (-18.4, -21.5, 4.05),  # 颧弓后段：正面几乎看不见
            (-21.5, -23.6, 3.65),
            (-23.6, -24.9, 2.95),  # 正面读到的就是这一档
        )):
            p.box("skull", f"cheek_{side}_{k + 1}", (sx * 2.2, 22.3, z0), (sx * hw, 25.6, z1))

    # ---- 鼻梁：两眼之间的窄竖带 + 皱鼻沟（龇牙时鼻梁堆起的横褶）----
    p.box("skull", "nose_bridge", (-0.9, 23.9, -27.1), (0.9, 26.2, -24.6), mat="pelt")
    for k in range(3):
        w = 0.95 + k * 0.16
        y = 24.85 - k * 0.58
        p.box("skull", f"snarl_wrinkle_{k + 1}", (-w, y, -27.2 + k * 0.1), (w, y + 0.3, -26.4), mat="pelt_dark")

    # ---- 吻部：整体在眼下方。前缘推到 -28.0，把颅长/颧宽从 1.33 拉到 1.4（真狮 ≈1.45）----
    p.box("skull", "muzzle", (-2.45, 22.0, -27.55), (2.45, 24.6, -23.6))
    for sx, side in ((-1, "l"), (1, "r")):
        # 髭须垫：两个**分开的**鼓包。原来左右垫贴在一起，正面读成一块浅灰口罩
        p.box("skull", f"whisker_pad_{side}", (sx * 0.5, 22.15, -28.0), (sx * 2.45, 23.95, -26.3), mat="muzzle_pale")
        p.box("skull", f"whisker_bulge_{side}", (sx * 0.62, 22.5, -28.18), (sx * 2.1, 23.7, -27.4), mat="muzzle_pale")
        for k in range(3):
            p.box("skull", f"whisker_dot_{side}_{k + 1}",
                  (sx * (0.95 + k * 0.48), 23.35 - k * 0.4, -28.28), (sx * (1.15 + k * 0.48), 23.53 - k * 0.4, -28.16),
                  mat="pelt_dark")
        # 须：短而暗。原来是 2.3 长的亮白棍子，在 6.4 宽的脸上像撒了一把牙签
        for k, (dx, dy) in enumerate(((1.25, -0.1), (1.15, -0.55), (0.95, -0.95))):
            p.limb("skull", f"whisker_{side}_{k + 1}",
                   (sx * 2.25, 23.35 - k * 0.45, -27.9), (sx * (2.25 + dx), 23.35 - k * 0.45 + dy, -28.2),
                   0.09, 0.055, mat="whisker")
    # 人中：加宽加深到 char——这条沟是把两块髭须垫读成"两块"的唯一分界
    p.box("skull", "philtrum", (-0.4, 22.3, -28.1), (0.4, 23.2, -26.9), mat="char")

    # ---- 鼻镜：倒三角 + 鼻翼 + 鼻孔 ----
    p.box("skull", "nose_pad", (-0.78, 23.2, -28.22), (0.78, 24.15, -27.0), mat="nose")
    p.box("skull", "nose_tip", (-0.5, 22.85, -28.16), (0.5, 23.25, -27.1), mat="nose")
    for sx, side in ((-1, "l"), (1, "r")):
        p.box("skull", f"nose_wing_{side}", (sx * 0.6, 23.1, -28.08), (sx * 1.06, 23.88, -27.2), mat="nose")
        p.box("skull", f"nostril_{side}", (sx * 0.22, 23.38, -28.3), (sx * 0.6, 23.84, -27.9), mat="eye_pupil")

    # ---- 嘴：黑唇线 + **下垂**的嘴角（转 12° 才是弯的，平着摆就是一条横杠）----
    p.box("skull", "lip_upper", (-1.5, 21.5, -27.9), (1.5, 22.2, -26.6), mat="nose")
    for sx, side in ((-1, "l"), (1, "r")):
        p.box("skull", f"lip_corner_{side}", (sx * 1.25, 21.05, -27.7), (sx * 2.15, 22.2, -26.2),
              rot=(0.0, 0.0, sx * -13.0), org=(sx * 1.45, 22.0, -27.0), mat="nose")
        # 犬齿：唇线下明确可见（但不做剑齿虎那么夸张）
        p.box("skull", f"fang_{side}", (sx * 0.75, 20.6, -27.55), (sx * 1.18, 21.65, -26.7), mat="fang")
        p.box("skull", f"fang_tip_{side}", (sx * 0.83, 20.18, -27.46), (sx * 1.1, 20.66, -26.8), mat="fang")

    # ---- 下巴 / 下颌 ----
    p.box("jaw", "chin", (-1.42, 19.85, -27.6), (1.42, 21.5, -25.6))
    p.box("jaw", "chin_pale", (-0.82, 20.45, -27.68), (0.82, 21.42, -26.7), mat="muzzle_pale")
    p.box("jaw", "jaw_wrap", (-2.75, 20.3, -25.6), (2.75, 22.9, -19.8))

    # ---- 耳：圆钝，头顶两侧偏后 ----
    for sx, side in ((-1, "l"), (1, "r")):
        p.box("skull", f"ear_{side}", (sx * 2.0, 28.2, -21.6), (sx * 3.85, 31.3, -20.0),
              rot=(0.0, 0.0, sx * 14.0), org=(sx * 2.4, 28.2, -20.8))
        p.box("skull", f"ear_inner_{side}", (sx * 1.85, 28.6, -21.4), (sx * 2.8, 30.7, -20.2),
              rot=(0.0, 0.0, sx * 14.0), org=(sx * 2.4, 28.2, -20.8), mat="char")
        p.box("skull", f"ear_back_{side}", (sx * 3.3, 28.4, -21.5), (sx * 4.0, 31.1, -20.1),
              rot=(0.0, 0.0, sx * 14.0), org=(sx * 2.4, 28.2, -20.8), mat="pelt_dark")
        # 耳缘缺口：撕掉一角的旧伤（不对称，左右不同深度）
        nick = 0.9 if sx < 0 else 0.45
        p.box("skull", f"ear_nick_{side}", (sx * (3.85 - nick), 30.5, -21.2), (sx * 4.0, 31.3, -20.2),
              rot=(0.0, 0.0, sx * 14.0), org=(sx * 2.4, 28.2, -20.8), mat="char")


def part_eyes(p: Pelt, env: Envelope, P) -> None:
    """眼：照真狮眼特写做的写实版。

    对着照片补的四处，之前一处都没有：
      · **圆瞳**——真狮是圆瞳，竖瞳是家猫/爬行类。之前为了凶做了竖瞳，写实就得换回来
      · 虹膜外缘一圈深色环（limbal ring）+ 下半略亮，虹膜才不是一块平涂色
      · 瞳孔只占眼宽 ~20%，不是一根贯穿的黑条
      · 眼下一块浅色毛——真狮眼下是明确发白的，没有它眼睛就浮在暗底上

    形状仍走脸弧分列（透镜形、外眼角上挑），眼裂比眯眼版开一档：高/宽 0.32 → 0.48，
    接近照片；再开就回到大眼萌了。
    """
    X0, X1 = 1.05, 3.15  # 必须落在 FACE 网格边界上
    YC, RISE, HMAX = 25.16, 0.42, 0.44

    for sx, side in ((-1, "l"), (1, "r")):
        cols = list(arc_cols(X0, X1))
        mid = len(cols) // 2
        for i, (xa, xb, xm, t, zf) in enumerate(cols):
            half = HMAX * math.sin(math.pi * t) ** 0.45  # 两端收成尖
            yc = YC + RISE * t
            top, bot = yc + half * 1.15, yc - half * 0.85
            k = i + 1
            end = i in (0, len(cols) - 1)  # 两端整列走深色环
            # 眼窝（凹进去的底）
            p.box("skull", f"eye_socket_{side}_{k}", (sx * xa, bot - 0.14, zf + 0.05), (sx * xb, top + 0.14, -24.3), mat="char")
            # 虹膜：两端列整列深色环，中间列上下各压一条环
            p.box("skull", f"eye_iris_{side}_{k}", (sx * xa, bot, zf - 0.14), (sx * xb, top, zf + 0.5),
                  mat="eye_rim" if end else "eye_iris")
            if not end:
                p.box("skull", f"eye_rim_up_{side}_{k}", (sx * xa, top - 0.13, zf - 0.16), (sx * xb, top, zf + 0.4), mat="eye_rim")
                p.box("skull", f"eye_glow_{side}_{k}", (sx * xa, bot + 0.1, zf - 0.16), (sx * xb, bot + 0.3, zf + 0.4), mat="eye_glow")
                p.box("skull", f"eye_rim_low_{side}_{k}", (sx * xa, bot, zf - 0.16), (sx * xb, bot + 0.1, zf + 0.4), mat="eye_rim")
            # 圆瞳：只落正中一列，宽高接近 1:1（这个尺度上方块就是圆）
            if i == mid:
                cy = (top + bot) / 2
                ph = (top - bot) * 0.26
                p.box("skull", f"eye_pupil_{side}", (sx * xa, cy - ph, zf - 0.22), (sx * xb, cy + ph, zf + 0.45), mat="eye_pupil")
            # 眼睑：细线
            p.box("skull", f"eyelid_up_{side}_{k}", (sx * xa, top + 0.02, zf - 0.18), (sx * xb, top + 0.2, zf + 0.55), mat="pelt_dark")
            p.box("skull", f"eyelid_low_{side}_{k}", (sx * xa, bot - 0.2, zf - 0.14), (sx * xb, bot - 0.02, zf + 0.5), mat="pelt_dark")
            # 眼下浅色毛：照片里眼下是一片明确发白的区域
            if not end:
                p.box("skull", f"eye_underpale_{side}_{k}", (sx * xa, bot - 0.62, zf - 0.1), (sx * xb, bot - 0.22, zf + 0.45), mat="muzzle_pale")

        # 外眼角的尖
        zc = face_z(X1 + 0.2)
        p.box("skull", f"eye_canthus_{side}", (sx * X1, YC + RISE - 0.02, zc - 0.06), (sx * (X1 + 0.42), YC + RISE + 0.3, -24.4), mat="pelt_dark")

        # 眼下泪痕：真狮自内眼角往下有一道深色纹，压在浅色毛上才有层次
        zt = face_z(1.45)
        p.box("skull", f"tear_line_{side}", (sx * 1.15, 23.1, zt - 0.16), (sx * 1.72, 24.5, -25.0),
              rot=(0.0, 0.0, sx * -14.0), org=(sx * 1.55, 24.5, -25.4), mat="char")


def part_mane(p: Pelt, env: Envelope, P) -> None:
    """鬃毛：向下垂的浓密毛片，围成一个把脸兜在中间的"框"。

    首版按径向做成了炸开的尖刺（刺猬）。真狮鬃毛是长毛**受重力下垂**：头顶的
    向后披，两侧的向外再向下，颌下的直接垂——整体正面看是个圆框，脸在框心。
    所以这里用扁毛片（flat）而非锥刺，方向按极角在"向后上"和"向下"之间插值。
    """
    layers = [
        # (环半径, 片长, 片粗, 环心 y, 环心 z, 片数, 材质)
        (7.0, 8.2, 1.95, 24.6, -14.8, 20, "mane"),
        (5.9, 6.6, 1.75, 24.6, -16.4, 18, "mane"),
        (5.2, 5.0, 1.55, 24.4, -17.6, 16, "char"),
        (4.7, 3.4, 1.3, 24.2, -19.0, 12, "mane"),
    ]
    for li, (rad, length, thick, cy, cz, n, mat) in enumerate(layers):
        for k in range(n):
            ang = 2 * math.pi * k / n  # 0 = 正上方，π = 颌下
            ux, uy = math.sin(ang), math.cos(ang)
            up = max(0.0, uy)  # 头顶权重
            base = (ux * rad * 0.88, cy + uy * rad * 0.92, cz)
            # 头顶向后披、侧下向下垂 —— 关键是别把毛往外炸
            dx = ux * (0.30 + 0.25 * (1 - up))
            dy = -0.82 + 1.05 * up
            dz = 0.62 - 0.25 * up
            norm = math.sqrt(dx * dx + dy * dy + dz * dz)
            tip = (base[0] + dx / norm * length, base[1] + dy / norm * length, base[2] + dz / norm * length)
            # 侧面的毛片沿径向薄、切向宽；顶/底的两向都宽
            flat = 0.45 + 0.5 * abs(uy)
            bone = "neck_base" if li == 0 else "neck_mid"
            p.limb(bone, f"mane_{li + 1}_{k + 1}", base, tip, thick, thick * 0.72, mat=mat, flat=flat)
            if li < 2:  # 簇尖余烬（黑火烧到毛梢）
                mid = tuple((base[j] + tip[j]) / 2 for j in range(3))
                p.limb(bone, f"mane_ember_{li + 1}_{k + 1}",
                       mid, tip, thick * 0.55, thick * 0.3, mat="mane_ember", flat=flat)
            elif li == 3 and k % 2 == 0:  # 内圈毛根：黑火最旺的地方在贴脸这一层
                root = tuple(base[j] + (tip[j] - base[j]) * 0.28 for j in range(3))
                p.limb(bone, f"mane_root_ember_{k + 1}", base, root,
                       thick * 0.52, thick * 0.34, mat="ember_hot", flat=flat)
    # 肩前延伸（真狮鬃毛一直盖到肩胸）
    for sx, side in ((-1, "l"), (1, "r")):
        for k in range(3):
            bx = sx * (2.2 + k * 1.3)
            p.limb("thorax_front", f"mane_shoulder_{side}_{k + 1}",
                   (bx, 22.0 - k * 2.2, -11.0), (bx * 1.1, 16.4 - k * 2.6, -8.4), 1.5, 0.8, mat="mane", flat=0.6)
    # 颊侧鬃：自耳下向前下垂，把脸颊两侧包进鬃毛里
    for sx, side in ((-1, "l"), (1, "r")):
        for k in range(4):
            by = 26.6 - k * 1.9
            p.limb("neck_mid", f"mane_cheek_{side}_{k + 1}",
                   (sx * 4.7, by, -20.2), (sx * (5.0 - k * 0.25), by - 3.4, -22.8 + k * 0.4),
                   1.45, 0.85, mat="mane", flat=0.5)
    # 颌下长毛
    for i, sx in enumerate((-1, 1)):
        p.limb("jaw", f"mane_beard_{i + 1}", (sx * 2.3, 20.2, -23.4), (sx * 2.7, 15.4, -20.8), 1.5, 0.7, mat="mane", flat=0.75)


def _crack(p, bone, name, a, b, out, w, *, seg=4, flat=1.0):
    """炭裂：沿 a→b 分段画，段长/段宽带扰动、段间留缝，炽芯沿 out 浮出。

    一整根等宽长条渲出来是根霓虹灯管（侧视实测）。真裂纹是断的、粗细不匀的，
    所以这里按 seg 段切开、每段取一个稳定扰动。

    用 crc32 不用内置 hash()：str 的 hash 每进程加盐，同一份脚本两次跑会生成
    不同的模型，diff 全是噪音。
    """
    h = zlib.crc32(name.encode())
    d = tuple(b[i] - a[i] for i in range(3))
    for k in range(seg):
        j = (h >> (k * 5)) & 7  # 0..7 稳定扰动
        t0 = k / seg + 0.05 + j * 0.012
        t1 = (k + 1) / seg - 0.05 - ((h >> (k * 3 + 2)) & 3) * 0.02
        if t1 <= t0:
            continue
        sa = tuple(a[i] + d[i] * t0 for i in range(3))
        sb = tuple(a[i] + d[i] * t1 for i in range(3))
        ww = w * (0.62 + j * 0.075)
        p.limb(bone, f"{name}_char_{k + 1}", sa, sb, ww, ww * 0.7, mat="char", flat=flat)
        # 炽芯：只在一半的段里露头，且比裂口窄得多——满段都亮就又成灯管了
        if j % 4 != 3:
            off = tuple(o * ww * 0.9 for o in out)
            ca = tuple(sa[i] + off[i] for i in range(3))
            cb = tuple(sb[i] + off[i] for i in range(3))
            mat = "ember_hot" if j >= 4 else "mane_ember"
            p.limb(bone, f"{name}_core_{k + 1}", ca, cb, ww * 0.34, ww * 0.2, mat=mat, flat=flat)


def part_embers(p: Pelt, env: Envelope, P) -> None:
    """皮下黑火：炭裂 + 落灰 + 旧伤。

    整只身上唯一的暖色来源，也是"这不是普通狮子"的主要抓手。位置全部跟着肌肉
    包络走，改骨/肌会自动跟。
    """
    prof = env.smooth_profile(TORSO_BONES, -12.0, 11.0, 1.6, margin=0.35)

    def at(z):
        """该 z 处的 (半宽, 腹线, 背线)。"""
        return min(prof, key=lambda r: abs(r[0] - z))[1:]

    def bone_at(z):
        return "thorax_front" if z < -6 else ("thorax_back" if z < 3 else "lumbar")

    # ---- 背脊炭裂：断续三段，沿脊线烧开 ----
    for k, (z0, z1) in enumerate(((-9.0, -5.4), (-3.0, 1.0), (3.8, 7.4))):
        y0, y1 = at(z0)[2], at(z1)[2]
        _crack(p, bone_at(z0), f"crack_dorsal_{k + 1}",
               (0, y0 - 0.2, z0), (0, y1 - 0.2, z1), (0, 1, 0), 0.62, seg=3, flat=0.55)

    # ---- 侧腹斜裂：左右各两道 ----
    for sx, side in ((-1, "l"), (1, "r")):
        for k, (z, y0, ln, dz) in enumerate(((-4.6, 12.4, 4.6, 1.7), (4.2, 11.6, 3.6, -1.3))):
            hw = at(z)[0]
            x = sx * (hw - 0.2)
            _crack(p, bone_at(z), f"crack_flank_{side}_{k + 1}",
                   (x, y0, z), (x, y0 + ln, z + dz), (sx, 0, 0), 0.6, seg=3, flat=0.5)

    # ---- 颈侧：鬃毛根部烧透的短裂 ----
    for sx, side in ((-1, "l"), (1, "r")):
        _crack(p, "neck_mid", f"crack_neck_{side}",
               (sx * 3.9, 24.8, -18.6), (sx * 4.1, 21.8, -16.8), (sx, 0, 0), 0.4, seg=2, flat=0.5)

    # ---- 臀/股外侧 ----
    for sx, side in ((-1, "l"), (1, "r")):
        fe = P(f"femur_{side}")
        _crack(p, f"femur_{side}", f"crack_haunch_{side}",
               (sx * 4.1, fe[1] + 1.6, fe[2] + 1.0), (sx * 4.3, fe[1] - 3.2, fe[2] - 1.4),
               (sx, 0, 0), 0.58, seg=3, flat=0.45)

    # ---- 前肢外侧小裂 ----
    for sx, side in ((-1, "l"), (1, "r")):
        hu, ra = P(f"humerus_{side}"), P(f"radius_{side}")
        mid = tuple((hu[i] + ra[i]) / 2 for i in range(3))
        _crack(p, f"humerus_{side}", f"crack_arm_{side}", hu, mid, (sx, 0, 0), 0.34, seg=2, flat=0.5)

    # ---- 落灰：肩背受灰最多，一层冷灰压在暖色对面 ----
    for k, z in enumerate((-9.5, -6.5)):
        hw, _lo, hi = at(z)
        p.box("thorax_front", f"ash_shoulder_{k + 1}",
              (-hw * 0.38, hi - 0.3, z - 0.7), (hw * 0.38, hi + 0.2, z + 0.7), mat="ash")

    # ---- 旧伤：肋侧一道结痂的疤（不对称，只有左侧）----
    hw = at(-2.0)[0]
    p.box("thorax_back", "scar_rib", (-hw - 0.15, 13.6, -3.6), (-hw + 0.5, 18.4, -1.2),
          rot=(24.0, 0.0, 0.0), org=(-hw, 16.0, -2.4), mat="scar")


def part_legs(p: Pelt, env: Envelope, P) -> None:
    """四肢皮筒：上段厚（裹着大肌）、下段收细见腱。"""
    for sx, side in ((-1, "l"), (1, "r")):
        sc, hu, ra, ff = (P(f"{n}_{side}") for n in ("scapula", "humerus", "radius", "forefoot"))
        p.limb(f"scapula_{side}", f"shoulder_{side}", (sc[0] * 0.9, sc[1] - 1.0, sc[2]), hu, 3.5, 3.0)
        p.limb(f"humerus_{side}", f"upperarm_{side}", hu, ra, 3.0, 2.1)
        # 下段压暗：正面原来是一大片浅灰柱子，和黑鬃劈成两截
        p.limb(f"radius_{side}", f"forearm_{side}", ra, ff, 2.1, 1.35, mat="pelt_dark")

        fe, ti, ta, hf = (P(f"{n}_{side}") for n in ("femur", "tibia", "tarsus", "hindfoot"))
        p.limb(f"femur_{side}", f"thigh_{side}", (fe[0], fe[1] + 1.0, fe[2]), ti, 3.9, 2.6)
        p.limb(f"tibia_{side}", f"shank_{side}", ti, ta, 2.6, 1.5)
        p.limb(f"tarsus_{side}", f"hock_{side}", ta, hf, 1.5, 1.25, mat="pelt_dark")


def part_paws(p: Pelt, env: Envelope, P) -> None:
    """脚：掌背 + 4 趾 + 趾垫 + 掌垫 + 伸出的角质爪。"""
    for sx, side in ((-1, "l"), (1, "r")):
        for tag, base_bone, wrist_bone, fwd in (("fore", f"forefoot_{side}", "forefoot", -1.0), ("hind", f"hindfoot_{side}", "hindfoot", -1.0)):
            w = P(f"{'forefoot' if tag == 'fore' else 'hindfoot'}_{side}")
            # 掌背
            p.box(base_bone, f"{tag}paw_{side}", (w[0] - sx * 2.1, 0.0, w[2] - 3.4), (w[0] + sx * 2.1, 2.6, w[2] + 1.0), mat="pelt_dark")
            # 掌垫
            p.box(base_bone, f"{tag}pad_{side}", (w[0] - sx * 1.7, 0.0, w[2] - 2.6), (w[0] + sx * 1.7, 0.55, w[2] + 0.4), mat="pad")
            for k in range(4):
                off = sx * (k - 1.5) * 1.0
                tx = w[0] + off
                p.box(base_bone, f"{tag}toe_{side}_{k + 1}", (tx - 0.5, 0.0, w[2] - 4.7), (tx + 0.5, 1.5, w[2] - 3.1), mat="pelt_dark")
                p.box(base_bone, f"{tag}toepad_{side}_{k + 1}", (tx - 0.42, 0.0, w[2] - 4.5), (tx + 0.42, 0.42, w[2] - 3.3), mat="pad")
                # 爪：自趾端前伸并下勾
                p.limb(base_bone, f"{tag}claw_{side}_{k + 1}",
                       (tx, 0.9, w[2] - 4.6), (tx, 0.25, w[2] - 6.0), 0.42, 0.16, mat="claw")


def part_tail(p: Pelt, env: Envelope, P) -> None:
    """尾皮筒 + 尾梢黑火毛簇。"""
    idx = sorted(int(n.split("_")[1]) for n in p.skel.pivot if n.startswith("tail_"))
    for k, i in enumerate(idx[:-1]):
        a, b = P(f"tail_{i:02d}"), P(f"tail_{i + 1:02d}")
        t = k / max(1, len(idx) - 2)
        r0 = _lerp(2.6, 1.05, t)
        r1 = _lerp(2.6, 1.05, min(1.0, t + 1 / max(1, len(idx) - 2)))
        p.limb(f"tail_{i:02d}", f"tail_pelt_{i:02d}", a, b, r0, r1, mat="pelt" if t < 0.6 else "pelt_dark")
    # 尾梢毛簇：黑火
    last, prev = P(f"tail_{idx[-1]:02d}"), P(f"tail_{idx[-2]:02d}")
    dirv = tuple(last[j] - prev[j] for j in range(3))
    norm = math.sqrt(sum(v * v for v in dirv)) or 1.0
    dirv = tuple(v / norm for v in dirv)
    for i, (spread, length) in enumerate([(0.0, 5.2), (1.5, 4.4), (-1.5, 4.4), (0.0, 3.6)]):
        tip = (
            last[0] + dirv[0] * length + spread,
            last[1] + dirv[1] * length - abs(spread) * 0.4,
            last[2] + dirv[2] * length,
        )
        p.limb(f"tail_{idx[-1]:02d}", f"tail_tuft_{i + 1}", last, tip, 1.55, 0.5, mat="mane")
        mid = tuple((last[j] + tip[j]) / 2 for j in range(3))
        p.limb(f"tail_{idx[-1]:02d}", f"tail_tuft_ember_{i + 1}", mid, tip, 0.8, 0.25, mat="mane_ember")


GROUPS = {
    "torso": ("躯干皮", part_torso),
    "neck": ("颈皮", part_neck),
    "head": ("头/耳/鼻", part_head),
    "eyes": ("眼", part_eyes),
    "mane": ("鬃毛（黑火）", part_mane),
    "embers": ("皮下黑火：炭裂/落灰/伤疤", part_embers),
    "legs": ("四肢皮", part_legs),
    "paws": ("脚掌/趾/爪", part_paws),
    "tail": ("尾+尾梢黑火", part_tail),
}


def main() -> int:
    ap = argparse.ArgumentParser(description="怠怒之狮皮毛/外观层")
    ap.add_argument("--muscle", type=Path, default=MUSCLE, help="肌肉层 bbmodel（取包络）")
    ap.add_argument("--group", choices=sorted(GROUPS), help="只生成一个部位")
    ap.add_argument("--with-anatomy", action="store_true", help="保留骨+肌，看包裹关系")
    ap.add_argument("--list", action="store_true")
    ap.add_argument("--out", type=Path)
    args = ap.parse_args()

    if args.list:
        for k, (label, _) in GROUPS.items():
            print(f"  {k:7s} {label}")
        return 0

    if not args.muscle.is_file():
        print(f"找不到肌肉层: {args.muscle}（先跑 gen_muscle.py）")
        return 2

    skel = Skeleton(args.muscle)
    env = Envelope(args.muscle)
    extend_texture(skel.data)
    p = Pelt(skel)
    for _label, fn in ([GROUPS[args.group]] if args.group else list(GROUPS.values())):
        fn(p, env, skel.P)

    if not args.with_anatomy:
        # 角/牙是外露角质，属于最终外观，别当解剖层滤掉
        keep = {
            e["uuid"]
            for e in skel.data["elements"]
            if e.get("_pelt") or e["name"].startswith("horn_")
        }
        skel.data["elements"] = [e for e in skel.data["elements"] if e["uuid"] in keep]

        def prune(node):
            if isinstance(node, str):
                return node in keep
            node["children"] = [c for c in node.get("children", []) if prune(c)]
            return True

        for root in skel.data["outliner"]:
            prune(root)

    name = "DainuLionPelt" + (f"_{args.group}" if args.group else "") + ("_anatomy" if args.with_anatomy else "")
    skel.data["name"] = name
    skel.data["model_identifier"] = name
    out = args.out or (MODELS / f"{name}.bbmodel")
    out.write_text(json.dumps(skel.data, ensure_ascii=False, indent=1))
    print(f"→ {out}")
    print(f"   外观件 {p.count} 个 · 总 cube {len(skel.data['elements'])}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
