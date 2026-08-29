#!/usr/bin/env python3
"""Round 2 的**人工闸门**：出一张给人看的接触表，然后停下等人一句话。

视觉资产纪律里 round 2 原本写的是「模型自评」。改成人工闸门，理由是两次实测：

  · **视角标签会骗人。** yaw=180 名义叫 FRONT，实渲的是 −z 面。小草包的骨扣长在 +z
    前檐上，于是「正面看不见骨扣」这个**假 bug** 让人连试三个亮度阈值，全在错的视角上
    找一个本就不该出现的东西 —— 几何、UV、材质从头到尾都是对的。
  · **参考图特征会被整件丢掉，而所有数值门都是绿的。** 小草包前两轮整件漏掉背带
    （画面占比仅次于包身），当时七道门全绿：有没有背带根本不在任何一道门的问题域里。

这两次都恰好发生在「自评」这一步。人看图三十秒能发现「背带呢」，模型跑四十分钟数值门
也发现不了。所以这个工具**不做那个判断** —— 它只把该看的东西整理到一张图上、把该点的
名点齐，让人那三十秒花得值。

一张表里有四样：
  1. 六个**诚实命名**的视角（FRONT/BACK/SIDE_L/SIDE_R/3-4/TOP），标签上写出实际照到
     的轴面，全部共用一个固定取景；
  2. 上一轮的同一批视角、**同一个取景**（`--prev`），左右并排 —— 自动取景下这种对比
     全是噪声；
  3. manifest 点名结果（人写的特征清单，缺一项就红）；
  4. 门禁的差分自证结果（报不出自己该抓的缺陷的门算失效）。

用法:
    python3 modelScript/tools/contact_sheet.py modelScript/models/GrassPouch.bbmodel \\
        --gates gen_grass_pouch --prev modelScript/out/GrassPouch_round1.bbmodel
"""

from __future__ import annotations

import argparse
import importlib
import sys
from pathlib import Path

LIB = Path(__file__).resolve().parents[1]
for _d in ("core", "generators"):
    sys.path.insert(0, str(LIB / _d))
import workspace  # noqa: E402

import framing  # noqa: E402
import manifest as mfmod  # noqa: E402

_WS = workspace.Workspace.discover(start=Path(__file__))
REPO = _WS.root
OUTDIR = _WS.out


def _manifest_notes(rc) -> list[str]:
    """图上那几行 ASCII 摘要。中文表格走终端 —— PIL 默认字体画不了中文。"""
    lines = [f"MANIFEST | {len(rc.verdicts) - len(rc.missing)}/{len(rc.verdicts)} "
             f"features ok"]
    for v in rc.verdicts:
        if not v.ok:
            why = "MISSING" if v.count == 0 else "; ".join(
                p.split("（")[0] for p in v.problems)[:60]
            lines.append(f"! {v.key}: {why}")
    unseen = rc.unseen_materials()
    if unseen:
        lines.append("! materials never on camera: " + ", ".join(unseen))
    elif rc.manifest.materials:
        lines.append(f"MATERIALS | {len(rc.manifest.materials)}/"
                     f"{len(rc.manifest.materials)} on camera")
    return lines


def _gate_notes(gates, rig) -> list[str]:
    """跑门禁 + 差分自证，回终端全文与图上摘要。"""
    gates.report(rig)
    print()
    results = gates.run_all(rig)
    broken = gates.self_test(rig)
    print()
    n = len(results)
    dirty = [g.key for g in results if not g.ok]
    lines = [f"GATES | {n - len(dirty)}/{n} clean | SELF-TEST {n - broken}/{n} "
             f"discriminating"]
    for key in dirty:
        lines.append(f"! gate {key} reports violations")
    if broken:
        lines.append(f"! {broken} gate(s) FAILED their own injection - no discriminating power")
    return lines


def build_sheet(model, *, manifest=None, prev=None, size: int = 300,
                shading: str = "lambert", notes=()) -> "object":
    facing = manifest.facing if manifest else framing.LEGACY_FACING
    views = framing.views_for(facing)
    # **取景由当前模型定，上一轮沿用同一个** —— 各算各的包围盒就没法叠着看。
    focus = framing.focus_for(model, views)

    now = framing.render_views(model, views, focus=focus, size=size, shading=shading)
    if prev is None:
        return framing.contact_sheet([(v.label, im) for v, im in now],
                                     title=f"{Path(model).stem}  facing={facing}",
                                     notes=notes, columns=3)
    old = framing.render_views(prev, views, focus=focus, size=size, shading=shading)
    tiles = []
    for (v, a), (_, b) in zip(now, old):
        tiles.append((f"NOW  {v.label}", a))
        tiles.append((f"PREV {v.label}", b))
    return framing.contact_sheet(
        tiles, title=f"{Path(model).stem}  facing={facing}  (NOW vs {Path(prev).stem})",
        notes=notes, columns=2)


def main() -> int:
    ap = argparse.ArgumentParser(description="Round 2 人工闸门：出接触表，停下等人看")
    ap.add_argument("model", help=".bbmodel 路径")
    ap.add_argument("--manifest", help="特征清单（缺省按模型名去 modelScript/manifests/ 找）")
    ap.add_argument("--no-manifest", action="store_true", help="这件还没写清单（不推荐）")
    ap.add_argument("--gates", help="生成器模块名，取它的 GATES / build()，如 gen_grass_pouch")
    ap.add_argument("--prev", help="上一轮的 .bbmodel，用同一取景并排对比")
    ap.add_argument("--size", type=int, default=300)
    ap.add_argument("--shading", choices=("lambert", "mc"), default="lambert")
    ap.add_argument("--out", help="输出 PNG（缺省 modelScript/out/contact_<名>.png）")
    args = ap.parse_args()

    model = Path(args.model)
    notes: list[str] = []
    mf = None
    if not args.no_manifest:
        mf = mfmod.load_manifest(args.manifest) if args.manifest else mfmod.manifest_for(model)
        rc = mfmod.roll_call(model, mf)
        rc.report()
        print()
        notes += _manifest_notes(rc)

    if args.gates:
        mod = importlib.import_module(args.gates)
        notes += _gate_notes(mod.GATES, mod.build())

    sheet = build_sheet(model, manifest=mf, prev=Path(args.prev) if args.prev else None,
                        size=args.size, shading=args.shading, notes=notes)
    out = Path(args.out) if args.out else OUTDIR / f"contact_{model.stem}.png"
    out.parent.mkdir(parents=True, exist_ok=True)
    sheet.save(out)
    try:
        shown = out.relative_to(REPO)
    except ValueError:
        shown = out
    print(f"→ {shown}  ({sheet.width}×{sheet.height})")
    print()
    print("=" * 72)
    print("ROUND 2 是人工闸门 —— 到此停下。把上面这张图给人看，等一句话再动 round 3。")
    print("数值门和点名器都只能回答「有没有」，回答不了「像不像、好不好看、是不是那个东西」。")
    print("=" * 72)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
