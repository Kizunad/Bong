#!/usr/bin/env python3
"""骨骼姿势烘焙渲染：给 {bone_name: [rx,ry,rz]}（+可选 pos），沿骨骼层级把旋转应用到
cube 顶点后渲染，用来在定稿动画前验证朝向（头往下咬 / 前扑朝 -Z 等）。复用 render_bbmodel 投影。"""
import base64
import io
import json
import sys
from pathlib import Path

import numpy as np
from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "core"))
import render_bbmodel as R  # noqa: E402


def load_posed(path, pose):
    with open(path, encoding="utf-8") as fh:
        d = json.load(fh)
    res = d["resolution"]
    rw, rh = res["width"], res["height"]
    src = d["textures"][0]["source"].split(",", 1)[1]
    tex = np.asarray(Image.open(io.BytesIO(base64.b64decode(src))).convert("RGBA"), float)
    els = {e["uuid"]: e for e in d["elements"]}

    # 每个 cube → 从根到叶的 (pivot, rot, pos) 链
    chains = {}

    def walk(nodes, chain):
        for n in nodes:
            if isinstance(n, str):
                if n in els:
                    chains[n] = chain
            elif isinstance(n, dict):
                nm = n.get("name")
                piv = np.array(n.get("origin") or [0, 0, 0], float)
                p = pose.get(nm)
                ch = chain
                if p is not None:
                    rot = p.get("rot", [0, 0, 0])
                    pos = np.array(p.get("pos", [0, 0, 0]), float)
                    Rm = R._rotmat(rot[2], 2) @ R._rotmat(rot[1], 1) @ R._rotmat(rot[0], 0)
                    ch = chain + [(piv, Rm, pos)]
                walk(n.get("children", []), ch)

    walk(d["outliner"], [])

    tris = []
    for e in d["elements"]:
        f, t = np.array(e["from"], float), np.array(e["to"], float)
        faces = e.get("faces", {})
        rot = e.get("rotation", [0, 0, 0])
        org = np.array(e.get("origin", [0, 0, 0]), float)
        Rc = None
        if any(rot):
            Rc = R._rotmat(rot[2], 2) @ R._rotmat(rot[1], 1) @ R._rotmat(rot[0], 0)
        chain = chains.get(e["uuid"], [])
        for fname, (corner_fn, normal) in R.FACES.items():
            fd = faces.get(fname)
            if not fd:
                continue
            u1, v1, u2, v2 = fd["uv"]
            cs = [np.array(c, float) for c in corner_fn(f, t)]
            n = np.array(normal, float)
            if Rc is not None:
                cs = [Rc @ (c - org) + org for c in cs]
                n = Rc @ n
            for piv, Rm, pos in reversed(chain):  # 叶→根
                cs = [Rm @ (c - piv) + piv + pos for c in cs]
                n = Rm @ n
            uvs = [(u1, v1), (u2, v1), (u2, v2), (u1, v2)]
            for a, b in ((1, 2), (2, 3)):
                tris.append((np.array([cs[0], cs[a], cs[b]]), np.array([uvs[0], uvs[a], uvs[b]]), n))
    return tris, tex, (rw, rh), d.get("name")


def render_pose(path, pose, yaw=90, pitch=8, size=440, bg=(120, 122, 128)):
    orig = R.load_bbmodel
    R.load_bbmodel = lambda p, xform=None, texture=None: load_posed(path, pose)
    try:
        im, _ = R.render(path, yaw=yaw, pitch=pitch, size=size, bg=bg)
    finally:
        R.load_bbmodel = orig
    return im


if __name__ == "__main__":
    # 验证：静止 / 头往下咬(head -X) / 前扑(body 前移 -Z)
    path = "modelScript/models/devour_rat_v3.bbmodel"
    poses = {
        "REST": {},
        "FWD(body pos -Z=6)": {"body": {"pos": [0, 0, -6]}},
        "legFR +45X": {"leg_fr": {"rot": [45, 0, 0]}},
        "legFR -45X": {"leg_fr": {"rot": [-45, 0, 0]}},
        "REARUP(body +18X)": {"body": {"rot": [18, 0, 0]}},
    }
    ims = [(nm, render_pose(path, p, yaw=90, pitch=8, size=440)) for nm, p in poses.items()]
    W = sum(i.width for _, i in ims)
    H = max(i.height for _, i in ims)
    canvas = Image.new("RGB", (W, H), (120, 122, 128))
    x = 0
    for _, im in ims:
        canvas.paste(im, (x, 0))
        x += im.width
    canvas.save(str(Path(__file__).resolve().parents[1] / "models" / "pose_verify.png"))
    print("saved modelScript/models/pose_verify.png:", [nm for nm, _ in ims])
