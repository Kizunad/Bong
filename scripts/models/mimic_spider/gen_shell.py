#!/usr/bin/env python3
"""拟态灰烬蛛 —— 甲壳层（Round 1/3）：最终进游戏的那层。

**读**框架层 bbmodel，往同一骨骼树上叠甲壳表现件，输出新文件。绝不回写框架
——框架若被 Blockbench 手工精修过，生成器覆盖会冲掉改动（与狮子肌肉层同则）。

内容：
  · 背甲穹顶 + 中窝暗纹 + 灰烬霜（背部落灰，呼应"共时灰质化"——它和残灰方块
    是同一个过程的两面）
  · 8 眼球：全身唯一暖色（余烬橙）。黑暗里只看得见眼。
  · 腹部体量：双层穹壳 + 背斑
  · 腿甲板 + 分节环 + 刚毛（deterministic 摆放；**再生腿无刚毛**——新甲光秃，
    更瘆人）
  · 螯肢面板

配色纪律：整体压死在残灰色系窄幅内，明度对比全部让位给眼睛的单点橙。

材质追加在贴图第 2 行（第 1 行 6 个框架色块保持原位，框架 element 的 UV
一个不用改）。折叠姿约束由 preview.py --model shell 把守——甲壳加厚吃掉的
正是框架折叠时预留的 0.5 单位。

用法:
  python3 scripts/models/mimic_spider/gen_shell.py            # 框架+甲壳
  python3 scripts/models/mimic_spider/gen_shell.py --only-shell
  python3 scripts/models/mimic_spider/gen_shell.py --group legs
  python3 scripts/models/mimic_spider/gen_shell.py --list
"""

from __future__ import annotations

import argparse
import base64
import io
import json
import math
import uuid
from pathlib import Path

import numpy as np
from PIL import Image

import gen_frame as F
from gen_frame import shaft_box

MODELS = F.OUT_DIR
FRAME = MODELS / "MimicSpiderFrame.bbmodel"
OUT = MODELS / "MimicSpiderShell.bbmodel"

# 甲壳材质：索引从 8 起（贴图第 2 行），框架 6 色在第 1 行原位不动
SHELL_MATS = {
    "shell": (104, 100, 92),      # 甲板主色：灰褐
    "shell_dark": (58, 54, 49),   # 甲缝 / 中窝 / 阴影
    "shell_pale": (139, 141, 131),  # 灰烬霜：背部落灰（残灰色系去饱和）
    "shell_new": (166, 152, 124),  # 再生腿甲板：苍黄新甲
    "eye_ember": (232, 112, 44),  # 眼球：全身唯一暖色
    "seta": (30, 27, 25),         # 刚毛
}
SHELL_BASE = 8
SWATCH = 8


class Shell:
    """读框架 bbmodel，追加甲壳 element 到既有骨骼。"""

    def __init__(self, frame_path: Path = FRAME):
        self.doc = json.loads(frame_path.read_text())
        self.nodes: dict[str, dict] = {}

        def walk(node):
            if isinstance(node, str):
                return
            self.nodes[node["name"]] = node
            for c in node.get("children", []):
                walk(c)

        for root in self.doc["outliner"]:
            walk(root)
        self.added = 0

    def cube(self, bone: str, name: str, frm, to, *, rot=None, org=None, mat="shell") -> None:
        if bone not in self.nodes:
            raise ValueError(f"框架中无此骨骼: {bone}")
        if mat not in SHELL_MATS:
            raise ValueError(f"未知甲壳材质: {mat}")
        f = [round(min(a, b), 3) for a, b in zip(frm, to)]
        t = [round(max(a, b), 3) for a, b in zip(frm, to)]
        idx = SHELL_BASE + list(SHELL_MATS).index(mat)
        ox, oy = (idx % 8) * SWATCH, (idx // 8) * SWATCH
        uv = [ox + 1.0, oy + 1.0, ox + SWATCH - 1.0, oy + SWATCH - 1.0]
        eid = str(uuid.uuid4())
        self.doc["elements"].append({
            "name": name,
            "box_uv": False,
            "rescale": False,
            "locked": False,
            "render_order": "default",
            "allow_mirror_modeling": True,
            "type": "cube",
            "uuid": eid,
            "from": f,
            "to": t,
            "autouv": 0,
            "color": idx % 8,
            "origin": [round(v, 3) for v in (org or [(a + b) / 2 for a, b in zip(f, t)])],
            "rotation": [round(v, 3) for v in (rot or (0.0, 0.0, 0.0))],
            "faces": {d: {"uv": list(uv), "texture": 0}
                      for d in ("north", "south", "east", "west", "up", "down")},
        })
        self.nodes[bone]["children"].append(eid)
        self.added += 1

    def shaft(self, bone: str, name: str, a, b, rx, rz=None, *, mat="shell", extend=0.0) -> None:
        rz = rx if rz is None else rz
        frm, to, rot, org = shaft_box(tuple(a), tuple(b), rx, rz, extend)
        self.cube(bone, name, frm, to, rot=rot, org=org, mat=mat)

    def extend_texture(self) -> None:
        tex = self.doc["textures"][0]
        raw = base64.b64decode(tex["source"].split(",", 1)[1])
        img = Image.open(io.BytesIO(raw)).convert("RGBA")
        px = img.load()
        for j, (mat, (r, g, b)) in enumerate(SHELL_MATS.items()):
            idx = SHELL_BASE + j
            ox, oy = (idx % 8) * SWATCH, (idx // 8) * SWATCH
            for y in range(SWATCH):
                for x in range(SWATCH):
                    n = ((x * 7 + y * 13 + idx * 5) % 5) - 2
                    px[ox + x, oy + y] = (
                        max(0, min(255, r + n * 4)),
                        max(0, min(255, g + n * 4)),
                        max(0, min(255, b + n * 3)),
                        255,
                    )
        buf = io.BytesIO()
        img.save(buf, format="PNG")
        tex["source"] = "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode()

    def strip_frame_elements(self, keep_mats: tuple[int, ...] = ()) -> None:
        """--only-shell：清掉框架 cube 只看甲壳（保留指定 color 索引的框架件）。"""
        keep = {e["uuid"] for e in self.doc["elements"]
                if e["color"] in keep_mats or e["uuid"] in self._added_uuids()}
        self.doc["elements"] = [e for e in self.doc["elements"] if e["uuid"] in keep]

        def prune(node):
            if isinstance(node, str):
                return node in keep
            node["children"] = [c for c in node["children"] if prune(c)]
            return True

        for root in self.doc["outliner"]:
            prune(root)

    def _added_uuids(self) -> set[str]:
        return {e["uuid"] for e in self.doc["elements"][-self.added:]} if self.added else set()

    def save(self, path: Path, name: str) -> None:
        self.doc["name"] = name
        self.doc["model_identifier"] = name
        self.doc["textures"][0]["name"] = f"{name}.png"
        path.write_text(json.dumps(self.doc, ensure_ascii=False))


# ---------------------------------------------------------------- 几何工具
def _leg_frames(pair: int, side: str):
    """腿关节点 + 每节的正交基 (d 轴向, n1 水平外法向, n2 = d×n1)。"""
    pts = [np.array(p) for p in F.leg_joints(pair, side)]
    sx = 1.0 if side == "r" else -1.0
    az = math.radians(F.LEG_AZ[pair - 1])
    u = np.array([sx * math.cos(az), 0.0, -math.sin(az)])
    n1 = np.array([sx * math.sin(az), 0.0, sx * math.cos(az)]) * sx  # 指向体外
    frames = []
    for i in range(4):
        d = pts[i + 1] - pts[i]
        d = d / np.linalg.norm(d)
        n2 = np.cross(d, n1)
        n2 = n2 / np.linalg.norm(n2)
        frames.append((pts[i], pts[i + 1], d, n1, n2))
    return frames, u


# ================================================================ 组：头胸部
def shell_prosoma(sh: Shell) -> None:
    b = "prosoma"
    # 背甲穹顶：主板 + 两侧裙板（外倾）+ 后缘收口
    sh.cube(b, "dome_main", (-3.6, 12.0, -6.8), (3.6, 13.2, 0.7),
            rot=(-4.0, 0.0, 0.0), org=(0.0, 12.6, -3.0))
    for s, sx in (("l", -1.0), ("r", 1.0)):
        sh.cube(b, f"dome_skirt_{s}", (sx * 2.6, 10.6, -6.4), (sx * 3.9, 12.6, 0.5),
                rot=(0.0, 0.0, sx * -14.0), org=(sx * 3.2, 11.6, -3.0))
    sh.cube(b, "dome_rear", (-2.8, 10.8, 0.3), (2.8, 12.5, 1.1),
            rot=(16.0, 0.0, 0.0), org=(0.0, 11.6, 0.7), mat="shell_dark")
    # 中窝：背甲正中一道下陷暗纹（真蛛的肌肉附着凹陷）
    sh.cube(b, "fovea", (-0.35, 13.15, -3.6), (0.35, 13.32, -1.2), mat="shell_dark")
    # 灰烬霜：背部落灰薄片——伏得越久落得越厚
    sh.cube(b, "ash_frost_pro", (-2.6, 13.2, -5.9), (2.6, 13.38, -2.2), mat="shell_pale")
    # 8 眼球：**内嵌**眼窝（比窝小 0.12），暗色眶缘隔开每只眼——外凸会让相邻眼
    # 粘连糊成一条橙带，"八只眼分别在看你"就没了。全身唯一暖色。
    for name, (x, y, z), r in F.EYES:
        rr = max(0.28, r - 0.12)
        sh.cube(b, name.replace("eye_", "eyeball_"),
                (x - rr, y - rr, z - r * 0.7 - 0.06), (x + rr, y + rr, z + r * 0.5),
                mat="eye_ember")
    # 螯肢面板
    for s, sx in (("l", -1.0), ("r", 1.0)):
        sh.cube(f"chelicera_{s}", f"paturon_plate_{s}",
                (sx * 0.15, 5.4, -8.55), (sx * 2.05, 7.9, -8.25), mat="shell_dark")


# ================================================================ 组：腹部
def shell_abdomen(sh: Shell) -> None:
    b = "abdomen"
    org = (0.0, 8.9, F.ABDOMEN_FRONT_Z + 1.0)
    # 双层穹壳：下宽上窄，同框架倾角
    sh.cube(b, "belly_shell", (-4.2, 5.2, F.ABDOMEN_FRONT_Z - 0.3), (4.2, 10.6, 8.8),
            rot=(-9.0, 0.0, 0.0), org=org)
    sh.cube(b, "dorsal_shell", (-3.0, 10.4, 0.6), (3.0, 12.8, 7.9),
            rot=(-9.0, 0.0, 0.0), org=org)
    # 灰烬霜 + 背斑：残灰质地的碎斑（拟态的"皮肤记忆"）
    sh.cube(b, "ash_frost_ab", (-2.0, 12.75, 1.6), (2.0, 12.95, 6.6),
            rot=(-9.0, 0.0, 0.0), org=org, mat="shell_pale")
    for i, (x, z, w) in enumerate(((-2.7, 2.6, 1.1), (2.4, 4.4, 1.3), (-1.9, 6.2, 0.9))):
        sh.cube(b, f"mottle_{i}", (x - w / 2, 12.6, z - w / 2), (x + w / 2, 12.86, z + w / 2),
                rot=(-9.0, 0.0, 0.0), org=org, mat="shell_pale")


# ================================================================ 组：腿
SETA_SPEC = ((0.30, 48.0), (0.55, -34.0), (0.78, 14.0))  # (沿节段比例, 绕轴 roll 度)


def shell_leg(sh: Shell, pair: int, side: str) -> None:
    key = f"{pair}_{side}"
    regrown = key == F.REGROWN
    frames, _u = _leg_frames(pair, side)
    plate_mat = "shell_new" if regrown else "shell"

    # 腿节甲板：femur 上段 60% 加厚壳
    p0, p1, d, n1, n2 = frames[1]
    a = p0 + d * 0.05 * np.linalg.norm(p1 - p0)
    bb = p0 + d * 0.62 * np.linalg.norm(p1 - p0)
    sh.shaft(f"femur{key}", f"femur_plate_{key}", a, bb, 1.38, mat=plate_mat)

    # 胫节分节环 ×2
    p0, p1, d, n1, n2 = frames[2]
    ln = np.linalg.norm(p1 - p0)
    for j, t in enumerate((0.34, 0.67)):
        c = p0 + d * t * ln
        sh.shaft(f"tibia{key}", f"tibia_ring_{key}_{j}", c - d * 0.45, c + d * 0.45,
                 1.05, mat="shell_dark" if not regrown else "shell_new")

    # 刚毛：femur/tibia 各 3 根，deterministic 摆放；再生腿无毛（新甲光秃）
    if regrown:
        return
    for seg, bone_prefix, radius in ((1, "femur", 1.05), (2, "tibia", 0.8)):
        p0, p1, d, n1, n2 = frames[seg]
        ln = np.linalg.norm(p1 - p0)
        for j, (t, roll) in enumerate(SETA_SPEC):
            rr = math.radians(roll + pair * 23.0)  # 逐腿错相，避免整排阅兵
            nd = n1 * math.cos(rr) + n2 * math.sin(rr)
            nd = nd - d * 0.35  # 朝节段末端斜
            nd = nd / np.linalg.norm(nd)
            base = p0 + d * t * ln + nd * radius * 0.6
            tip = base + nd * 0.95
            sh.shaft(f"{bone_prefix}{key}", f"seta_{bone_prefix}_{key}_{j}",
                     base, tip, 0.16, mat="seta")


GROUPS = {
    "prosoma": (shell_prosoma,),
    "abdomen": (shell_abdomen,),
    "legs": tuple(
        (lambda s, p=pair, sd=side: shell_leg(s, p, sd))
        for pair in (1, 2, 3, 4) for side in ("l", "r")
    ),
}


def build(only_group: str | None = None) -> Shell:
    sh = Shell()
    sh.extend_texture()
    for name, fns in GROUPS.items():
        if only_group is not None and name != only_group:
            continue
        for fn in fns:
            fn(sh)
    return sh


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--group", choices=sorted(GROUPS))
    ap.add_argument("--only-shell", action="store_true", help="隐去框架件，只看甲壳")
    ap.add_argument("--list", action="store_true")
    args = ap.parse_args()
    if args.list:
        for name in GROUPS:
            print(name)
        return 0

    sh = build(args.group)
    if args.only_shell:
        sh.strip_frame_elements()
    suffix = ("_" + args.group if args.group else "") + ("_only" if args.only_shell else "")
    out = MODELS / f"MimicSpiderShell{suffix}.bbmodel" if suffix else OUT
    sh.save(out, out.stem)
    print(f"→ {out}  (甲壳件 {sh.added} · 总 element {len(sh.doc['elements'])})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
