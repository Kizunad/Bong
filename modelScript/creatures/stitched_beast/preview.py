#!/usr/bin/env python3
"""异变缝合兽 —— 核心层预览：本体渲染 + 挂载点可视化。

挂载点是纯数据（坐标+法向），数字对不代表位置合理——"腿槽在肚子上"和"腿槽在肋下"
在打印输出里长得一样。`--sockets` 把每个 socket 渲成一根从表皮戳出来的短针（针长
即 girth），肉眼直接看得出哪个槽长歪了。

用法:
  python3 modelScript/creatures/stitched_beast/preview.py                 # 3/4 视图
  python3 modelScript/creatures/stitched_beast/preview.py --three-view
  python3 modelScript/creatures/stitched_beast/preview.py --sockets       # 叠挂载点针
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "core"))
sys.path.insert(0, str(HERE))

import core as C  # noqa: E402
import gen_core as G  # noqa: E402
from bbmodel_maker.render.render_bbmodel import render, render_three_view  # noqa: E402
from bbmodel_maker.rig.voxel_rig import Palette, Rig  # noqa: E402

# 挂载点针的配色：三类各一色，跟本体的灰肉色拉开，一眼分得出类别。
PIN_MATS = {"limb": (232, 96, 60), "head": (96, 176, 232), "vestige": (216, 200, 92)}


def build_with_pins() -> Rig:
    """本体 + 挂载点针。针沿法向从表皮外扎出，长度 = girth。"""
    mats = dict(G.MATS)
    for k, v in PIN_MATS.items():
        mats[f"pin_{k}"] = v
    rig = Rig(Palette(mats, swatch=8, size=64))
    G._bone_tree(rig)
    G.part_mass(rig)
    G.part_drips(rig)
    for s in C.sockets().values():
        a = s.pos - s.normal * 1.0            # 略微扎进肉里，免得针浮在空中
        b = s.pos + s.normal * (s.girth + 1.5)
        rig.shaft(s.bone, f"pin_{s.name}", tuple(a), tuple(b), 0.45, mat=f"pin_{s.kind}")
    return rig


def socket_report() -> str:
    """挂载点几何摘要——针渲得对不对，还得靠数字复核一遍。"""
    socks = C.sockets()
    rows = [f"{len(socks)} 个挂载点："]
    for kind in ("limb", "head", "vestige"):
        ks = [s for s in socks.values() if s.kind == kind]
        rows.append(f"  {kind:<8} ×{len(ks)}  girth {min(s.girth for s in ks):.2f}"
                    f"..{max(s.girth for s in ks):.2f}")
    lo = min(float(np.linalg.norm(a.pos - b.pos))
             for i, a in enumerate(socks.values())
             for j, b in enumerate(socks.values()) if i < j)
    rows.append(f"  最近两槽间距 {lo:.1f}（<4.0 会互穿）")
    return "\n".join(rows)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--sockets", action="store_true", help="叠加挂载点针")
    ap.add_argument("--shard", nargs="?", const="", default=None,
                    help="改渲碎片模型；可跟 lobe 列表，缺省取健康分裂那半")
    ap.add_argument("--three-view", action="store_true")
    ap.add_argument("--yaw", type=float, default=-35.0)
    ap.add_argument("--pitch", type=float, default=22.0)
    ap.add_argument("--size", type=int, default=640)
    ap.add_argument("--out", type=Path, default=None)
    args = ap.parse_args()

    if args.shard is not None:
        import fragment as FR
        import gen_fragment as GF
        lobes = tuple(filter(None, args.shard.split(","))) or FR.default_lobes()
        # 按碎片真实能长到的生长度出图：按满长出图看到的是它到不了的形态
        rig, _g, _s = GF.build(lobes, growth=FR.geom(lobes).growth())
        tag, sub = ("shard_" + "_".join(lobes), "4_shard")
    else:
        rig = build_with_pins() if args.sockets else G.build()
        tag, sub = ("sockets", "2_sockets") if args.sockets else ("core", "1_core")
    tmp = G.OUT_DIR / f"_preview_{sub}.bbmodel"
    rig.save(tmp, "StitchedBeastCorePreview")

    dst = HERE / "renders" / sub
    dst.mkdir(parents=True, exist_ok=True)
    # render / render_three_view 都返回 (image, name)，别直接 .save
    if args.three_view:
        out = args.out or dst / f"{tag}_three_view.png"
        img, _ = render_three_view(tmp, size=args.size // 2)
    else:
        out = args.out or dst / f"{tag}_34.png"
        img, _ = render(tmp, yaw=args.yaw, pitch=args.pitch, size=args.size)
    img.save(out)
    if args.shard is None:
        print(socket_report())
    print(f"→ {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
