#!/usr/bin/env python3
"""腐羽鹫 —— 一键预览：重生成三档模型 + 渲染全部视图到本目录。

产出（均落在 modelScript/creatures/fuyu_vulture/）：
  render_<size>.png          单档三视图（收翼站姿）
  render_<size>_spread.png   单档三视图（展翼）
  render_scale.png           三档**同比例尺**并排 —— 档位差异只有这张图说得清
  render_parts_<size>.png    逐部件图集（每件单独显示，形状与接续一眼可辨）
  render_head_<size>.png     头部特写（喙钩 / 眼眶 / 巩膜环）

用法:
  python3 modelScript/creatures/fuyu_vulture/preview.py             # 全部
  python3 modelScript/creatures/fuyu_vulture/preview.py --size mid  # 只出一档
  python3 modelScript/creatures/fuyu_vulture/preview.py --skip-gen  # 不重生成，只渲染
"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
FINAL = Path(__file__).resolve().parents[2] / "models" / "fuyu_vulture"  # 最终 9 个外观
MODELS = FINAL / "layers"  # 骨架 / 肌肉 / 各种预览产物

sys.path.insert(0, str(HERE))
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "core"))

import tempfile  # noqa: E402
from gen_muscle import GROUP_LABEL as MGROUP_LABEL  # noqa: E402
from gen_pelt import MORPHS  # noqa: E402
from gen_muscle import GROUPS as MGROUPS  # noqa: E402
from gen_skeleton import PARTS, SPECS, build  # noqa: E402
from PIL import Image, ImageDraw  # noqa: E402
from bbmodel_maker.render.render_bbmodel import render, render_three_view  # noqa: E402
from bbmodel_maker.rig.rigkit import bake_file  # noqa: E402


_BAKED: dict[str, str] = {}


def baked(path: Path) -> str:
    """渲染前把坐标烘到世界系。

    render_bbmodel 只读 elements 不读 outliner，而自带骨的飞羽（gen_pelt 的 quill）存的
    是**骨局部坐标**、朝向烙在骨的绑定旋转里 —— 直接渲出来是一根根竖着的板，不是这只鸟。
    骨架/肌肉层没有带绑定旋转的骨，烘焙是恒等变换，一律走同一条路省得漏。
    """
    key = str(path)
    if key not in _BAKED:
        tmp = tempfile.NamedTemporaryFile(suffix=".bbmodel", delete=False)
        tmp.close()
        _BAKED[key] = bake_file(path, tmp.name)
    return _BAKED[key]

SIZES = ("small", "mid", "large")


def _run(*args: str) -> None:
    subprocess.run([sys.executable, str(HERE / "gen_skeleton.py"), *args], check=True)


def _grid(tiles: list[tuple[str, Image.Image]], cols: int, out: Path, hdr: int = 20) -> None:
    gap = 10
    tw, th = tiles[0][1].size
    rows = (len(tiles) + cols - 1) // cols
    canvas = Image.new(
        "RGB",
        (cols * tw + gap * (cols + 1), rows * (th + hdr) + gap * (rows + 1)),
        (14, 15, 17),
    )
    draw = ImageDraw.Draw(canvas)
    for i, (label, im) in enumerate(tiles):
        r, c = divmod(i, cols)
        x = gap + c * (tw + gap)
        y = gap + r * (th + hdr + gap)
        draw.text((x + 3, y + 4), label, fill=(224, 206, 180))
        canvas.paste(im, (x, y + hdr))
    canvas.save(out)
    print(f"  → {out.name}")


def three_view(model: str, out: str, size: int = 430, root: Path | None = None) -> None:
    im, _ = render_three_view(baked((root or MODELS) / f"{model}.bbmodel"), size=size)
    im.save(HERE / out)
    print(f"  → {out}")


def scale_sheet(size: int = 520) -> None:
    """三档并排，**共用一个比例尺**。

    各自自动取景的话三只鸟会渲成一样大，"小中大"就全白做了 —— 档位差异是本资产的
    全部意义，必须在一张图里量得出来。
    """
    spans, centers = {}, {}
    for key in SIZES:
        rig, _ = build(SPECS[key])
        (x0, y0, z0), (x1, y1, z1) = rig.bounds()
        centers[key] = ((x0 + x1) / 2, (y0 + y1) / 2, (z0 + z1) / 2)
        spans[key] = max(x1 - x0, y1 - y0, z1 - z0)
    span = max(spans.values()) * 1.06  # 统一尺：以最大档为准

    tiles = []
    for key in SIZES:
        spec = SPECS[key]
        im, _ = render(
            baked(MODELS / f"{spec.model}.bbmodel"),
            yaw=118.0, pitch=12.0, size=size,
            focus=(centers[key], span),
        )
        tiles.append((f"{key}  {spec.cn}  {spec.stand_h / 16:.2f} m", im))
    _grid(tiles, 3, HERE / "render_scale.png")


def parts_sheet(key: str, size: int = 340) -> None:
    spec = SPECS[key]
    tiles = []
    for part, (label, _) in PARTS.items():
        _run("--size", key, "--part", part)
        im, _ = render(baked(MODELS / f"{spec.model}_{part}.bbmodel"), yaw=132.0, pitch=14.0, size=size)
        tiles.append((f"{part} · {label}", im))
    _grid(tiles, 3, HERE / f"render_parts_{key}.png")


def head_shot(key: str, size: int = 520) -> None:
    spec = SPECS[key]
    tiles = []
    for tag, yaw, pitch in (("SIDE", 90.0, 0.0), ("3/4", 138.0, 12.0), ("FRONT", 178.0, 6.0)):
        im, _ = render(baked(MODELS / f"{spec.model}.bbmodel"), yaw=yaw, pitch=pitch, size=size)
        tiles.append((tag, im))
    _grid(tiles, 3, HERE / f"render_head_{key}.png")


def _run_muscle(*args: str) -> None:
    subprocess.run([sys.executable, str(HERE / "gen_muscle.py"), *args], check=True,
                   capture_output=True)


def muscle_views(key: str) -> None:
    """骨+肌 / 只肌 / 延展 / 展翼，四张。"""
    name = SPECS[key].model.replace("Skeleton", "Muscle")
    for suffix, tag in (("", ""), ("_bare", "_bare"), ("_explode", "_explode"),
                        ("_spread", "_spread")):
        three_view(f"{name}{suffix}", f"render_muscle{tag}_{key}.png")


def muscle_groups_sheet(key: str, size: int = 340) -> None:
    """逐肌群图集：每群单独挂在骨架上，形状与附着点一眼可辨。"""
    name = SPECS[key].model.replace("Skeleton", "Muscle")
    tiles = []
    for g in MGROUPS:
        _run_muscle("--size", key, "--group", g)
        im, _ = render(baked(MODELS / f"{name}_{g}.bbmodel"), yaw=132.0, pitch=14.0, size=size)
        tiles.append((f"{g} · {MGROUP_LABEL[g]}", im))
    _grid(tiles, 3, HERE / f"render_muscle_groups_{key}.png")


def muscle_top(key: str, size: int = 500) -> None:
    """俯视对比：翼膜在水平面内展开，只有从上往下看才看得见它撑出的前缘。"""
    sk = SPECS[key].model
    mu = sk.replace("Skeleton", "Muscle")
    tiles = []
    for label, model in (("骨架 · 展翼", f"{sk}_spread"), ("骨+肌 · 展翼", f"{mu}_spread")):
        im, _ = render(baked(MODELS / f"{model}.bbmodel"), yaw=180.0, pitch=88.0, size=size)
        tiles.append((label, im))
    _grid(tiles, 2, HERE / f"render_muscle_top_{key}.png")


def _run_pelt(*args: str) -> None:
    subprocess.run([sys.executable, str(HERE / "gen_pelt.py"), *args], check=True,
                   capture_output=True)


def pelt_views(key: str, morph: str = "jin") -> None:
    name = f"{SPECS[key].model.replace('Skeleton', 'Pelt')}_{morph}"
    three_view(name, f"render_pelt_{key}_{morph}.png", root=FINAL)  # 交付物在顶层
    three_view(f"{name}_spread", f"render_pelt_{key}_{morph}_spread.png")


def morph_sheet(key: str, size: int = 440) -> None:
    """一档三变色并排 —— 变色是本层的交付物，必须能一眼比出来。"""
    base = SPECS[key].model.replace("Skeleton", "Pelt")
    tiles = []
    for mo, meta in MORPHS.items():
        im, _ = render(baked(FINAL / f"{base}_{mo}.bbmodel"), yaw=138.0, pitch=12.0, size=size)
        tiles.append((f"{mo} · {meta['cn']} — {meta['note']}", im))
    _grid(tiles, 3, HERE / f"render_morphs_{key}.png")


def pelt_scale_sheet(morph: str = "jin", size: int = 520) -> None:
    """三档同一比例尺的最终外观。"""
    spans, centers = {}, {}
    for key in SIZES:
        rig, _ = build(SPECS[key])
        (x0, y0, z0), (x1, y1, z1) = rig.bounds()
        centers[key] = ((x0 + x1) / 2, (y0 + y1) / 2, (z0 + z1) / 2)
        spans[key] = max(x1 - x0, y1 - y0, z1 - z0)
    span = max(spans.values()) * 1.12
    tiles = []
    for key in SIZES:
        spec = SPECS[key]
        base = spec.model.replace("Skeleton", "Pelt")
        im, _ = render(baked(FINAL / f"{base}_{morph}.bbmodel"), yaw=118.0, pitch=12.0, size=size,
                       focus=(centers[key], span))
        tiles.append((f"{key}  {spec.cn}  {spec.stand_h / 16:.2f} m", im))
    _grid(tiles, 3, HERE / f"render_pelt_scale_{morph}.png")


def main() -> int:
    ap = argparse.ArgumentParser(description="腐羽鹫预览渲染")
    ap.add_argument("--size", choices=SIZES, help="只出一档")
    ap.add_argument("--skip-gen", action="store_true", help="不重生成 bbmodel，只渲染")
    ap.add_argument("--parts", action="store_true", help="只出骨架部件图集")
    ap.add_argument("--muscle", action="store_true", help="只出肌肉层视图")
    ap.add_argument("--muscle-groups", action="store_true", help="只出肌群图集")
    ap.add_argument("--pelt", action="store_true", help="只出羽层视图 + 变色对比")
    args = ap.parse_args()

    keys = [args.size] if args.size else list(SIZES)

    if not args.skip_gen:
        print("生成模型…")
        for k in keys:
            _run("--size", k)
            _run("--size", k, "--pose", "spread")
            _run_muscle("--size", k)
            _run_muscle("--size", k, "--pose", "spread")
            _run_muscle("--size", k, "--only-muscle")
            _run_muscle("--size", k, "--explode", "4")
            _run_pelt("--size", k)
            _run_pelt("--size", k, "--pose", "spread")

    print("渲染…")
    if args.parts:
        for k in keys:
            parts_sheet(k)
        return 0
    if args.muscle_groups:
        for k in keys:
            muscle_groups_sheet(k)
        return 0
    if args.muscle:
        for k in keys:
            muscle_views(k)
            muscle_top(k)
        return 0
    if args.pelt:
        for k in keys:
            pelt_views(k)
            morph_sheet(k)
        if len(keys) == len(SIZES):
            pelt_scale_sheet()
        return 0

    for k in keys:
        spec = SPECS[k]
        three_view(spec.model, f"render_{k}.png")
        three_view(f"{spec.model}_spread", f"render_{k}_spread.png")
        head_shot(k)
        muscle_views(k)
        muscle_top(k)
        pelt_views(k)
        morph_sheet(k)
    if len(keys) == len(SIZES):
        scale_sheet()
        pelt_scale_sheet()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
