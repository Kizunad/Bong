#!/usr/bin/env python3
"""把 bbmodel 武器摆进 vanilla 玩家模型手里渲染——校验"和玩家比起来多大/怎么握"。

现有工具都不覆盖这个场景：render_bbmodel.py 只单渲模型，render_held_item.py 走
OBJ+SML 管线且不含人体，render_animation.py 只出棍图。这里的做法是：

  vanilla biped 几何（32px 高，标准 64² skin box-uv）+ 程序化皮肤
  + 从 bbmodel 读武器三角形（已应用 element 自身旋转）
  + 按握持姿态把武器顶点烘到手心
  → 合成 128² 图集（上半玩家 / 下半武器，武器 UV 整体 +64）
  → 复用 render_bbmodel 的 z-buffer 光栅化投影

注意这是**比例/姿态校验**，不是 MC display transform 标定：真机手持还要过
item model 的 thirdperson/firstperson transform（那套用 render_held_item.py 调）。

用法:
    python3 modelScript/tools/render_jian_in_hand.py
    python3 modelScript/tools/render_jian_in_hand.py --model modelScript/models/BambooJianSingle.bbmodel
"""

from __future__ import annotations

import argparse
import base64
import io
import json
import math
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "core"))
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "generators"))
import render_bbmodel as R  # noqa: E402

import workspace  # noqa: E402

_WS = workspace.Workspace.discover(start=Path(__file__))
REPO = _WS.root
DEFAULT_MODEL = _WS.models / "BambooJianSingle.bbmodel"
OUT = _WS.out / "render_jian_in_hand.png"


def load_model_document(path: Path | None = None) -> dict:
    target = DEFAULT_MODEL if path is None else Path(path)
    if target.is_file():
        return json.loads(target.read_text())
    if target.resolve() == DEFAULT_MODEL.resolve():
        from gen_bamboo_jian import build_bbmodel

        model, _cubes, _texture = build_bbmodel(pair=False)
        return model
    raise FileNotFoundError(target)

CJK_FONTS = [  # PIL 默认字体没有 CJK 字形，标签会全变方块
    "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
    "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
]

SKIN = 64          # 玩家皮肤区尺寸（图集上半）
ATLAS = 128        # 合成图集边长
WEAPON_V_OFF = 64  # 武器 UV 的 v 偏移（落到图集下半）

# 玩家模型（世界坐标：y 向上、脚在 y=0、脸朝 -Z，和 vanilla biped 同比例）
# (name, from, to, box_uv_origin)
PLAYER_CUBES = [
    ("head",      [-4, 24, -4], [4, 32, 4],  (0, 0)),
    ("body",      [-4, 12, -2], [4, 24, 2],  (16, 16)),
    ("arm_right", [-8, 12, -2], [-4, 24, 2], (40, 16)),
    ("arm_left",  [4, 12, -2],  [8, 24, 2],  (32, 48)),
    ("leg_right", [-4, 0, -2],  [0, 12, 2],  (0, 16)),
    ("leg_left",  [0, 0, -2],   [4, 12, 2],  (16, 48)),
]

# 关节层级：肩 pivot → 手臂(arm) → 手心 → 腕(wrist) → 武器。
# 手臂不动、手心写死的话，锏只能贴在手臂外面走（读作"挂在身侧"而不是"握着"）。
SHOULDER = {"right": np.array([-5.0, 22.0, 0.0]), "left": np.array([5.0, 22.0, 0.0])}
# 手臂零姿态时的手心（手掌方块 = 臂的下 4px，y 12..16 → 心在 14）
HAND_REST = {"right": np.array([-6.0, 14.0, -0.4]), "left": np.array([6.0, 14.0, -0.4])}
GRIP_ANCHOR = np.array([0.0, 3.3, 0.0])  # 武器局部握把中心（柄尾 y=0，长度沿 +y）

# 每个姿态：{side: {"arm": (rx,ry,rz), "wrist": (rx,ry,rz)}}
# 绕 X 正 = 手臂前抬；绕 Z 对右臂负 = 外展（远离躯干），左臂相反。
# 腕 roll 的符号：武器指 +y，绕 Z 正角度把锏尖推向 -x。所以右手外撇取正、左手取负
# （手臂指 -y，同样的正角度反而把手推向 +x，两者符号是反的——踩过一次）。
POSES = [
    # 垂持的腕角来自 Blockbench 手改后回填（2026-08-06）：锏尖朝前下方、柄横过掌心，
    # 比"锏尖朝上"更像握着而不是举着。原手改是 roll 层 X-75 叠 pitch 层 X-30，
    # 这里合并成单层 X-105，几何等价且每层仍只转一个轴。
    ("持锏垂立", {
        "right": {"arm": (-4.0, 0.0, -6.0), "wrist": (-105.0, 0.0, 12.0)},
        "left": {"arm": (-4.0, 0.0, 6.0), "wrist": (-105.0, 0.0, -12.0)},
    }),
    ("交叉护胸", {
        "right": {"arm": (38.0, 0.0, -10.0), "wrist": (-10.0, 0.0, -58.0)},
        "left": {"arm": (38.0, 0.0, 10.0), "wrist": (-10.0, 0.0, 58.0)},
    }),
    ("战斗预备", {
        "right": {"arm": (30.0, 0.0, -26.0), "wrist": (-46.0, 0.0, 16.0)},
        "left": {"arm": (72.0, 0.0, 14.0), "wrist": (-64.0, 0.0, -18.0)},
    }),
]
ARM_CUBE = {"right": "arm_right", "left": "arm_left"}
MAX_RENDER_SIZE = 768
MAX_RENDER_WORKING_BYTES = 128 * 1024 * 1024
RGB_BYTES = 3
RASTER_BYTES_PER_PIXEL = 32


def validate_render_size(size: int) -> int:
    if isinstance(size, bool) or not isinstance(size, int):
        raise ValueError(f"--size must be an integer between 1 and {MAX_RENDER_SIZE}, got {size!r}")
    if not 1 <= size <= MAX_RENDER_SIZE:
        raise ValueError(f"--size must be between 1 and {MAX_RENDER_SIZE}, got {size}")
    return size


def validate_scales(raw: str) -> list[float]:
    values = []
    for token in raw.split(","):
        token = token.strip()
        if not token:
            continue
        try:
            value = float(token)
        except ValueError as exc:
            raise ValueError(f"--scales contains a non-numeric value: {token!r}") from exc
        if not math.isfinite(value) or value <= 0.0:
            raise ValueError(f"--scales values must be finite and positive, got {token!r}")
        values.append(value)
    if not values:
        raise ValueError("--scales 至少需要一个非空缩放值")
    return values


def composite_canvas_dimensions(size: int, scale_count: int) -> tuple[int, int]:
    validate_render_size(size)
    if scale_count < 1:
        raise ValueError("--scales 至少需要一个非空缩放值")
    per_row = scale_count if scale_count > 1 else 2
    tile_count = 2 * scale_count if scale_count > 1 else len(POSES) * 2
    rows = (tile_count + per_row - 1) // per_row
    gap, lab_h = 10, 20
    width = size * per_row + gap * (per_row + 1)
    height = (size + lab_h) * rows + gap * (rows + 1)
    tile_bytes = size * size * RGB_BYTES
    canvas_bytes = width * height * RGB_BYTES
    raster_bytes = size * size * RASTER_BYTES_PER_PIXEL
    bytes_required = tile_count * tile_bytes + canvas_bytes + raster_bytes
    if bytes_required > MAX_RENDER_WORKING_BYTES:
        raise ValueError(
            f"render working set requires {bytes_required} bytes, exceeds limit "
            f"{MAX_RENDER_WORKING_BYTES} for size={size}, scales={scale_count}"
        )
    return width, height



# ── 程序化皮肤（64²，vanilla box-uv 布局）─────────────────────────────────
def make_skin() -> Image.Image:
    """末法散修：粗麻上衣 + 褐裤 + 布鞋。只求人形可读，不追皮肤细节。"""
    img = Image.new("RGBA", (SKIN, SKIN), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    skin_c = (198, 156, 118)
    hair_c = (52, 40, 32)
    cloth_c = (104, 96, 84)
    cloth_d = (82, 76, 66)
    pants_c = (86, 66, 48)
    shoe_c = (48, 40, 34)

    def rect(x0, y0, x1, y1, c):
        d.rectangle([x0, y0, x1 - 1, y1 - 1], fill=c)

    # head：四侧 + 顶（顶/后脑给头发）
    rect(0, 8, 32, 16, skin_c)
    rect(8, 0, 16, 8, hair_c)          # up
    rect(16, 0, 24, 8, (168, 132, 100))  # down
    rect(24, 8, 32, 16, hair_c)        # 后脑
    for x in (0, 16):                  # 两侧鬓角
        rect(x, 8, x + 8, 10, hair_c)
    rect(8, 8, 16, 10, hair_c)         # 额发
    rect(10, 11, 12, 13, (58, 50, 46))  # 左眼
    rect(14, 11, 16, 13, (58, 50, 46))  # 右眼（贴图上左右已按 MC 约定）
    rect(11, 14, 15, 15, (150, 108, 84))  # 嘴

    # body：粗麻上衣 + 腰带
    rect(16, 16, 40, 32, cloth_c)
    rect(20, 16, 28, 20, cloth_d)      # up
    rect(16, 26, 40, 28, (66, 54, 42))  # 腰带
    for x in range(20, 28, 3):         # 前襟竖褶
        rect(x, 20, x + 1, 26, cloth_d)

    # arms：上半袖子、下半手
    for ox, oy in ((40, 16), (32, 48)):
        rect(ox, oy, ox + 16, oy + 16, cloth_c)
        rect(ox, oy + 4, ox + 16, oy + 12, cloth_c)
        rect(ox, oy + 12, ox + 16, oy + 16, skin_c)   # 露出的手
        rect(ox + 4, oy, ox + 8, oy + 4, cloth_d)

    # legs：褐裤 + 鞋
    for ox, oy in ((0, 16), (16, 48)):
        rect(ox, oy, ox + 16, oy + 16, pants_c)
        rect(ox, oy + 14, ox + 16, oy + 16, shoe_c)
        rect(ox + 8, oy, ox + 12, oy + 4, shoe_c)     # down = 鞋底

    return img


def label_font(px=15):
    from PIL import ImageFont
    for f in CJK_FONTS:
        if Path(f).exists():
            try:
                return ImageFont.truetype(f, px)
            except OSError:
                continue
    return ImageFont.load_default()


def box_uv(origin, size):
    """vanilla box-uv 展开：返回 {face: [u1,v1,u2,v2]}。"""
    ox, oy = origin
    sx, sy, sz = size
    return {
        "west": [ox, oy + sz, ox + sz, oy + sz + sy],
        "north": [ox + sz, oy + sz, ox + sz + sx, oy + sz + sy],
        "east": [ox + sz + sx, oy + sz, ox + 2 * sz + sx, oy + sz + sy],
        "south": [ox + 2 * sz + sx, oy + sz, ox + 2 * sz + 2 * sx, oy + sz + sy],
        "up": [ox + sz, oy, ox + sz + sx, oy + sz],
        "down": [ox + sz + sx, oy, ox + sz + 2 * sx, oy + sz],
    }


def euler_mat(rot):
    """M = Rz @ Ry @ Rx —— 与 bbmodel 里"嵌套单轴 group（内 pitch/外 roll）"同序。"""
    rx, ry, rz = rot
    return R._rotmat(rz, 2) @ R._rotmat(ry, 1) @ R._rotmat(rx, 0)


def arm_transform(pose, side):
    """返回 (M_arm, hand_world)：手臂绕肩旋转后，手心跟着走。"""
    rot = pose.get(side, {}).get("arm", (0.0, 0.0, 0.0)) if pose else (0.0, 0.0, 0.0)
    M = euler_mat(rot)
    sh = SHOULDER[side]
    return M, M @ (HAND_REST[side] - sh) + sh


def player_tris(pose=None):
    arm_of = {ARM_CUBE[s]: s for s in ("right", "left")}
    tris = []
    for name, frm, to, uvo in PLAYER_CUBES:
        f, t = np.array(frm, float), np.array(to, float)
        uvs_by_face = box_uv(uvo, (t[0] - f[0], t[1] - f[1], t[2] - f[2]))
        M = sh = None
        if name in arm_of:
            side = arm_of[name]
            M, _ = arm_transform(pose, side)
            sh = SHOULDER[side]
        for fname, (corner_fn, normal) in R.FACES.items():
            u1, v1, u2, v2 = uvs_by_face[fname]
            cs = [np.array(c, float) for c in corner_fn(f, t)]
            n = np.array(normal, float)
            if M is not None:
                cs = [M @ (c - sh) + sh for c in cs]
                n = M @ n
            uvs = [(u1, v1), (u2, v1), (u2, v2), (u1, v2)]
            for a, b in ((1, 2), (2, 3)):
                tris.append((np.array([cs[0], cs[a], cs[b]]),
                             np.array([uvs[0], uvs[a], uvs[b]]), n))
    return tris


def weapon_tris(path: Path):
    """读 bbmodel → 三角形（已应用 element 自身 rotation），UV 落到图集下半。"""
    d = load_model_document(path)
    src = d["textures"][0]["source"].split(",", 1)[1]
    tex = Image.open(io.BytesIO(base64.b64decode(src))).convert("RGBA")
    tris = []
    for e in d["elements"]:
        f, t = np.array(e["from"], float), np.array(e["to"], float)
        rot = e.get("rotation", [0, 0, 0])
        org = np.array(e.get("origin", [0, 0, 0]), float)
        Rc = None
        if any(abs(r) > 1e-6 for r in rot):
            Rc = R._rotmat(rot[2], 2) @ R._rotmat(rot[1], 1) @ R._rotmat(rot[0], 0)
        for fname, (corner_fn, normal) in R.FACES.items():
            fd = e.get("faces", {}).get(fname)
            if not fd:
                continue
            u1, v1, u2, v2 = fd["uv"]
            scale_u = SKIN / tex.width
            scale_v = SKIN / tex.height
            u1, u2 = u1 * scale_u, u2 * scale_u
            v1, v2 = v1 * scale_v, v2 * scale_v
            cs = [np.array(c, float) for c in corner_fn(f, t)]
            n = np.array(normal, float)
            if Rc is not None:
                cs = [Rc @ (c - org) + org for c in cs]
                n = Rc @ n
            uvs = [(u1, v1 + WEAPON_V_OFF), (u2, v1 + WEAPON_V_OFF),
                   (u2, v2 + WEAPON_V_OFF), (u1, v2 + WEAPON_V_OFF)]
            for a, b in ((1, 2), (2, 3)):
                tris.append((np.array([cs[0], cs[a], cs[b]]),
                             np.array([uvs[0], uvs[a], uvs[b]]), n))
    return tris, tex


def place(tris, pose, side, scale=1.0):
    """武器 → 握把中心对齐手心 → 腕旋转 → 整条手臂旋转（scale 只改武器不改人）。"""
    M_arm, _ = arm_transform(pose, side)
    M_wrist = euler_mat(pose.get(side, {}).get("wrist", (0.0, 0.0, 0.0)))
    sh, rest = SHOULDER[side], HAND_REST[side]

    def xf(v):
        local = M_wrist @ ((v - GRIP_ANCHOR) * scale) + rest   # 腕坐标系里握住
        return M_arm @ (local - sh) + sh                        # 再随手臂摆动

    M_total = M_arm @ M_wrist
    return [(np.array([xf(v) for v in vs]), uvs, M_total @ n) for vs, uvs, n in tris]


def build_atlas(weapon_tex: Image.Image) -> np.ndarray:
    atlas = Image.new("RGBA", (ATLAS, ATLAS), (0, 0, 0, 0))
    atlas.paste(make_skin(), (0, 0))
    atlas.paste(weapon_tex.resize((SKIN, SKIN), Image.NEAREST), (0, WEAPON_V_OFF))
    return np.asarray(atlas, float)


def render_scene(tris, tex_arr, yaw, pitch, size, bg=(26, 27, 31)):
    validate_render_size(size)
    orig = R.load_bbmodel
    R.load_bbmodel = lambda _p, xform=None, texture=None: (
        tris, tex_arr, (ATLAS, ATLAS), "in_hand")
    try:
        im, _ = R.render("<synthetic>", yaw=yaw, pitch=pitch, size=size, bg=bg)
    finally:
        R.load_bbmodel = orig
    return im


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--model", default=str(DEFAULT_MODEL))
    ap.add_argument("--size", type=int, default=420)
    ap.add_argument("--scales", default="1.0",
                    help="逗号分隔的武器缩放档，多档时出垂持对比图（例 1.0,0.85,0.7）")
    args = ap.parse_args()
    try:
        validate_render_size(args.size)
    except ValueError as exc:
        ap.error(str(exc))
    try:
        scales = validate_scales(args.scales)
    except ValueError as exc:
        ap.error(str(exc))
    try:
        canvas_width, canvas_height = composite_canvas_dimensions(args.size, len(scales))
    except ValueError as exc:
        ap.error(str(exc))

    w_tris, w_tex = weapon_tris(Path(args.model))
    tex_arr = build_atlas(w_tex)

    w_len = max(v[1] for vs, _, _ in w_tris for v in vs) - min(v[1] for vs, _, _ in w_tris for v in vs)
    print(f"武器长 {w_len:.1f}px / 玩家 32px = {w_len / 32 * 100:.0f}%")

    cols, per_row = [], 2
    if len(scales) > 1:
        # 尺寸对比模式：同一垂持姿态，逐档缩放武器
        per_row = len(scales)
        for view, yaw, pitch in (("正面", 180.0, 4.0), ("3/4", 145.0, 10.0)):
            for sc in scales:
                pose = POSES[0][1]
                tris = player_tris(pose)
                for side in ("right", "left"):
                    tris += place(w_tris, pose, side, sc)
                cols.append((f"{w_len * sc:.1f}px = 玩家 {w_len * sc / 32 * 100:.0f}% · {view}",
                             render_scene(tris, tex_arr, yaw, pitch, args.size)))
    else:
        for label, pose in POSES:
            for view, yaw, pitch in (("正面", 180.0, 4.0), ("3/4", 145.0, 10.0)):
                tris = player_tris(pose)
                for side in ("right", "left"):
                    tris += place(w_tris, pose, side, scales[0])
                cols.append((f"{label} · {view}",
                             render_scene(tris, tex_arr, yaw, pitch, args.size)))

    gap, lab_h = 10, 20
    font = label_font()
    w, h = canvas_width, canvas_height
    canvas = Image.new("RGB", (w, h), (14, 15, 17))
    d = ImageDraw.Draw(canvas)
    for i, (label, im) in enumerate(cols):
        cx = gap + (i % per_row) * (args.size + gap)
        cy = gap + (i // per_row) * (args.size + lab_h + gap)
        d.text((cx + 4, cy + 2), label, fill=(224, 222, 214), font=font)
        canvas.paste(im, (cx, cy + lab_h))
    out = OUT if len(scales) == 1 else OUT.with_name("render_jian_in_hand_scales.png")
    out.parent.mkdir(parents=True, exist_ok=True)
    canvas.save(out)
    print(f"→ {out.relative_to(REPO)}")


if __name__ == "__main__":
    main()
