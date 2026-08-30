#!/usr/bin/env python3
"""JianPlayer.bbmodel —— 把竹节双锏摆进 vanilla 玩家模型手里，产出可在 Blockbench
直接打开的合成模型，用来对比"武器和玩家一样大吗 / 握着是什么样"。

结构（outliner，握姿走嵌套 group，Blockbench 里可直接拖着调）：

    player_ref                      ← 躯干/头/腿
    arm_right_roll                  ← 肩关节 外层：绕肩的 Z（外展）
      └ arm_right_pitch             ← 肩关节 内层：绕肩的 X（前抬）
          ├ arm_right（cube）
          └ jian_right_roll         ← 腕 外层：绕手心的 Z
              └ jian_right_pitch    ← 腕 内层：绕手心的 X
                  └ 锏的 cube（几何搬自 BambooJian，含自身 45° 八角柱旋转）
    arm_left_* / jian_left_*  同理

锏挂在手臂 group 之下——在 Blockbench 里转手臂，锏跟着走，不会再变成"贴在
手臂外面的挂件"。每层 group 只转一个轴：两轴写在同一 group 会踩欧拉顺序歧义
（Blockbench 与渲染器的组合顺序未必一致），拆成嵌套单轴就没有解释空间。层级
从内到外：element 自身 45°(Y) → 腕 pitch(X) → 腕 roll(Z) → 肩 pitch(X) →
肩 roll(Z)，与 render_jian_in_hand.place() 的 M_arm @ M_wrist 同序，两条路径
出的姿态可以逐像素对拍。

贴图是 128² 合成图集：上半 = 玩家皮肤（vanilla 64² box-uv 布局），下半 = 锏贴图
（UV 整体 +64）。武器几何与 UV 直接搬自 modelScript/models/BambooJianSingle.bbmodel，
不重新推导——那份才是单一真相源，改了锏只要重跑本脚本。

用法:
    python3 modelScript/generators/gen_jian_player.py                  # 垂持站立
    python3 modelScript/generators/gen_jian_player.py --pose cross     # 交叉护胸
    python3 modelScript/generators/gen_jian_player.py --pose ready     # 战斗预备
    python3 modelScript/generators/gen_jian_player.py --no-render      # 只出 bbmodel
"""

from __future__ import annotations

import argparse
import base64
import copy
import hashlib
import io
import json
import sys
import uuid
from pathlib import Path

import numpy as np
from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "core"))
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))
from bbmodel_maker.render import render_bbmodel as R  # noqa: E402
from bbmodel_maker.render import held_item_render as H  # noqa: E402

REPO = Path(__file__).resolve().parents[2]
SRC_JIAN = Path(__file__).resolve().parents[1] / "models" / "BambooJianSingle.bbmodel"
OUT_BB = Path(__file__).resolve().parents[1] / "models" / "JianPlayer.bbmodel"
OUT_PNG = Path(__file__).resolve().parents[1] / "out" / "render_JianPlayer.png"
MAX_RENDER_SIZE = 768
MAX_RENDER_WORKING_BYTES = 128 * 1024 * 1024
RGB_BYTES = 3
RASTER_BYTES_PER_PIXEL = 32

ATLAS = H.ATLAS          # 128
V_OFF = H.WEAPON_V_OFF   # 64
POSE_KEYS = {"stand": 0, "cross": 1, "ready": 2}


def _uuid() -> str:
    return str(uuid.uuid4())


def _rel(p: Path) -> str:
    """--out 可以指到仓库外（临时对拍），relative_to 会抛——退回绝对路径。"""
    try:
        return str(p.resolve().relative_to(REPO))
    except ValueError:
        return str(p)


def _file_signature(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def player_elements(names=None):
    """vanilla biped cube → per-face uv 的 element（本模型统一 box_uv=false）。"""
    els = []
    for name, frm, to, uvo in H.PLAYER_CUBES:
        if names is not None and name not in names:
            continue
        size = (to[0] - frm[0], to[1] - frm[1], to[2] - frm[2])
        faces = {f: {"uv": [float(v) for v in uv], "texture": 0}
                 for f, uv in H.box_uv(uvo, size).items()}
        els.append({
            "name": name, "box_uv": False, "rescale": False, "locked": False,
            "render_order": "default", "allow_mirror_modeling": True, "type": "cube",
            "uuid": _uuid(), "from": [float(v) for v in frm], "to": [float(v) for v in to],
            "autouv": 0, "color": 7, "origin": [0.0, 0.0, 0.0],
            "rotation": [0.0, 0.0, 0.0], "faces": faces,
        })
    return els


def jian_elements(src: dict, hand, side: str):
    """搬 BambooJian 的 cube：平移到"握把中心落在手心"，UV 移进图集下半。
    hand 用手臂零姿态下的手心（H.HAND_REST）——手臂的摆动由父 group 负责。"""
    off = np.array(hand, float) - H.GRIP_ANCHOR
    out = []
    for e in src["elements"]:
        e = copy.deepcopy(e)
        e["uuid"] = _uuid()
        e["name"] = f"{e['name'].rsplit('_', 1)[0]}_{side}"
        e["from"] = [round(v + off[i], 4) for i, v in enumerate(e["from"])]
        e["to"] = [round(v + off[i], 4) for i, v in enumerate(e["to"])]
        e["origin"] = [round(v + off[i], 4) for i, v in enumerate(e["origin"])]
        for fd in e["faces"].values():
            u1, v1, u2, v2 = fd["uv"]
            fd["uv"] = [u1, v1 + V_OFF, u2, v2 + V_OFF]
        out.append(e)
    return out


def group(name, origin, rotation, children, color=0):
    return {
        "name": name, "origin": [round(float(v), 4) for v in origin],
        "rotation": [round(float(v), 4) for v in rotation],
        "color": color, "uuid": _uuid(), "export": True, "mirror_uv": False,
        "isOpen": True, "locked": False, "visibility": True, "autouv": 0,
        "children": children,
    }


def build(pose_key: str):
    src = H.load_model_document(SRC_JIAN)
    jian_tex = Image.open(io.BytesIO(base64.b64decode(
        src["textures"][0]["source"].split(",", 1)[1]))).convert("RGBA")

    atlas = Image.new("RGBA", (ATLAS, ATLAS), (0, 0, 0, 0))
    atlas.paste(H.make_skin(), (0, 0))
    atlas.paste(jian_tex.resize((H.SKIN, H.SKIN), Image.NEAREST), (0, V_OFF))

    label, pose = H.POSES[POSE_KEYS[pose_key]]
    arm_names = set(H.ARM_CUBE.values())
    torso = player_elements(names={n for n, *_ in H.PLAYER_CUBES} - arm_names)
    elements = list(torso)
    outliner = [group("player_ref", [0, 0, 0], [0, 0, 0],
                      [e["uuid"] for e in torso], color=7)]

    for side in ("right", "left"):
        arm_rx, _ary, arm_rz = pose[side]["arm"]
        w_rx, _wry, w_rz = pose[side]["wrist"]
        shoulder, hand = H.SHOULDER[side], H.HAND_REST[side]

        arm_cube = player_elements(names={H.ARM_CUBE[side]})
        jian = jian_elements(src, hand, side)
        elements += arm_cube + jian

        wrist_pitch = group(f"jian_{side}_pitch", hand, [w_rx, 0.0, 0.0],
                            [e["uuid"] for e in jian], color=1)
        wrist_roll = group(f"jian_{side}_roll", hand, [0.0, 0.0, w_rz], [wrist_pitch], color=1)
        arm_pitch = group(f"arm_{side}_pitch", shoulder, [arm_rx, 0.0, 0.0],
                          [arm_cube[0]["uuid"], wrist_roll], color=7)
        outliner.append(group(f"arm_{side}_roll", shoulder, [0.0, 0.0, arm_rz],
                              [arm_pitch], color=7))

    buf = io.BytesIO()
    atlas.save(buf, format="PNG")
    model = {
        "meta": {"format_version": "4.10", "model_format": "free", "box_uv": False},
        "name": "jian_player", "model_identifier": "geometry.bong.jian_player",
        "visible_box": [3.0, 3.0, 2.0], "resolution": {"width": ATLAS, "height": ATLAS},
        "elements": elements, "outliner": outliner,
        "textures": [{
            "path": "", "name": "jian_player.png", "folder": "item", "namespace": "bong",
            "id": "0", "width": ATLAS, "height": ATLAS, "uv_width": ATLAS, "uv_height": ATLAS,
            "particle": False, "render_mode": "default", "visible": True, "mode": "bitmap",
            "saved": False, "uuid": _uuid(),
            "source": "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode(),
        }],
    }
    return model, label


# ── 带 group 变换的加载（render_bbmodel 只读 element，会忽略 group 旋转）──
def load_grouped(path, xform=None, texture=None):
    d = json.loads(Path(path).read_text())
    res = d["resolution"]
    tex = np.asarray(Image.open(io.BytesIO(base64.b64decode(
        d["textures"][0]["source"].split(",", 1)[1]))).convert("RGBA"), float)
    els = {e["uuid"]: e for e in d["elements"]}
    # fmt 4.10：group 的 origin/rotation 内嵌在 outliner 节点里。
    # fmt 5.0（Blockbench 存盘后）：outliner 只剩 uuid+children，属性搬去顶层 groups。
    # 只读 outliner 会把手改过的姿态全当成 0，渲出零姿态——踩过一次。
    groups = {g["uuid"]: g for g in d.get("groups", []) if "uuid" in g}

    chains: dict[str, list] = {}

    def walk(nodes, chain):
        for n in nodes:
            if isinstance(n, str):
                if n in els:
                    chains[n] = chain
                    continue
                src = groups.get(n)
                if src is None:
                    continue
            else:
                src = groups.get(n.get("uuid"), n)
            rot = src.get("rotation") or [0, 0, 0]
            piv = np.array(src.get("origin") or [0, 0, 0], float)
            ch = chain
            if any(abs(r) > 1e-6 for r in rot):
                Rm = R._rotmat(rot[2], 2) @ R._rotmat(rot[1], 1) @ R._rotmat(rot[0], 0)
                ch = chain + [(piv, Rm)]
            walk(src.get("children") or [], ch)

    walk(d["outliner"], [])

    tris = []
    for e in d["elements"]:
        f, t = np.array(e["from"], float), np.array(e["to"], float)
        rot = e.get("rotation", [0, 0, 0])
        org = np.array(e.get("origin", [0, 0, 0]), float)
        Rc = None
        if any(abs(r) > 1e-6 for r in rot):
            Rc = R._rotmat(rot[2], 2) @ R._rotmat(rot[1], 1) @ R._rotmat(rot[0], 0)
        chain = chains.get(e["uuid"], [])
        for fname, (corner_fn, normal) in R.FACES.items():
            fd = e.get("faces", {}).get(fname)
            if not fd:
                continue
            u1, v1, u2, v2 = fd["uv"]
            cs = [np.array(c, float) for c in corner_fn(f, t)]
            n = np.array(normal, float)
            if Rc is not None:
                cs = [Rc @ (c - org) + org for c in cs]
                n = Rc @ n
            for piv, Rm in reversed(chain):  # 叶 → 根
                cs = [Rm @ (c - piv) + piv for c in cs]
                n = Rm @ n
            uvs = [(u1, v1), (u2, v1), (u2, v2), (u1, v2)]
            for a, b in ((1, 2), (2, 3)):
                tris.append((np.array([cs[0], cs[a], cs[b]]),
                             np.array([uvs[0], uvs[a], uvs[b]]), n))
    return tris, tex, (res["width"], res["height"]), d.get("name", "jian_player")


def validate_render_size(size: int) -> int:
    if isinstance(size, bool) or not isinstance(size, int):
        raise ValueError(f"--size must be an integer between 1 and {MAX_RENDER_SIZE}, got {size!r}")
    if not 1 <= size <= MAX_RENDER_SIZE:
        raise ValueError(f"--size must be between 1 and {MAX_RENDER_SIZE}, got {size}")
    tile_count = 3
    gap, lab_h = 10, 20
    width = size * tile_count + gap * (tile_count + 1)
    height = size + lab_h + gap * 2
    bytes_required = (
        tile_count * size * size * RGB_BYTES
        + width * height * RGB_BYTES
        + size * size * RASTER_BYTES_PER_PIXEL
    )
    if bytes_required > MAX_RENDER_WORKING_BYTES:
        raise ValueError(
            f"render working set requires {bytes_required} bytes, exceeds limit "
            f"{MAX_RENDER_WORKING_BYTES} for size={size}"
        )
    return size


def render(path, size=420):
    validate_render_size(size)
    from PIL import ImageDraw
    orig = R.load_bbmodel
    R.load_bbmodel = load_grouped
    try:
        tiles = [(lab, R.render(path, yaw=yaw, pitch=pitch, size=size, bg=(26, 27, 31))[0])
                 for lab, yaw, pitch in (("正面", 180.0, 4.0), ("侧面", 90.0, 4.0),
                                         ("3/4", 145.0, 10.0))]
    finally:
        R.load_bbmodel = orig
    gap, lab_h = 10, 20
    canvas = Image.new("RGB", (size * len(tiles) + gap * (len(tiles) + 1),
                               size + lab_h + gap * 2), (14, 15, 17))
    d = ImageDraw.Draw(canvas)
    font = H.label_font()
    x = gap
    for lab, im in tiles:
        d.text((x + 4, 4), lab, fill=(224, 222, 214), font=font)
        canvas.paste(im, (x, gap + lab_h))
        x += size + gap
    canvas.save(OUT_PNG)
    return OUT_PNG


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--pose", choices=sorted(POSE_KEYS), default="stand")
    ap.add_argument("--out", default=None, help="另存路径（默认 modelScript/models/JianPlayer.bbmodel）")
    ap.add_argument("--force", action="store_true",
                    help="覆盖 Blockbench 手改过的文件（默认拒绝，防冲掉手调姿态）")
    ap.add_argument("--no-render", action="store_true")
    ap.add_argument("--size", type=int, default=420)
    args = ap.parse_args()
    try:
        validate_render_size(args.size)
    except ValueError as exc:
        ap.error(str(exc))

    out_bb = Path(args.out) if args.out else OUT_BB
    original_signature = None
    if out_bb.exists() and not args.force:
        # Blockbench 存盘会升到 fmt 5.0 并把 group 属性搬进顶层 groups——这是手改过的信号
        prev = json.loads(out_bb.read_text())
        if "groups" in prev or prev.get("meta", {}).get("format_version") != "4.10":
            raise SystemExit(
                f"{_rel(out_bb)} 像是 Blockbench 手改过的（fmt "
                f"{prev.get('meta', {}).get('format_version')}），拒绝覆盖。\n"
                f"  想留手改 → 加 --out 另存；确定要覆盖 → 加 --force")
        original_signature = _file_signature(out_bb)

    model, label = build(args.pose)
    if original_signature is not None:
        if not out_bb.exists() or _file_signature(out_bb) != original_signature:
            raise SystemExit(f"{_rel(out_bb)} 在生成期间发生变化，拒绝覆盖。")
    out_bb.parent.mkdir(parents=True, exist_ok=True)
    if original_signature is not None and _file_signature(out_bb) != original_signature:
        raise SystemExit(f"{_rel(out_bb)} 在写入前发生变化，拒绝覆盖。")
    out_bb.write_text(json.dumps(model, ensure_ascii=False, indent=1))
    n_player = len(H.PLAYER_CUBES)
    print(f"JianPlayer（姿态：{label}）:")
    print(f"  elements: {len(model['elements'])}  (玩家 {n_player} + 双锏 "
          f"{len(model['elements']) - n_player})")
    print("  层级    : 肩(roll→pitch) → 手臂 + 腕(roll→pitch) → 锏")
    print(f"  贴图    : {ATLAS}² 图集（上半玩家皮肤 / 下半锏，UV +{V_OFF}）")
    print(f"  → bbmodel: {_rel(out_bb)} ({out_bb.stat().st_size} B)")
    if not args.no_render:
        p = render(out_bb, size=args.size)
        print(f"  → render : {_rel(p)}")


if __name__ == "__main__":
    main()
