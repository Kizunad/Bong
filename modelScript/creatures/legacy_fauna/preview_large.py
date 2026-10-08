"""大型旧生物新动画的预览与落地自查：出原速 GIF / 关键帧接触表，并报告入地深度。

旧生物的 geo 声明的贴图尺寸与实际 PNG 不一致（毒龙 geo 2048 / PNG 256，黑虎 192 / 384，黑武士
128 / 256）。GeckoLib 运行时按 UV 比例取样所以游戏里没问题，但预览工具按像素取样会渲成一片黑。
所以这里先把 PNG 最近邻缩放到 geo 声明的尺寸，写进临时资源树再交给 ``creature_anim_frames``。
这是**只给预览用**的临时副本，不会写回 client 资源。

用法（需要装好 bbmodel-maker）::

    python3 modelScript/creatures/legacy_fauna/preview_large.py dark_tiger --clip run --gif
    python3 modelScript/creatures/legacy_fauna/preview_large.py dark_tiger --clip hurt --sheet --frames 6
"""
from __future__ import annotations

import argparse
import json
import sys
import tempfile
from pathlib import Path

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "modelScript/tools"))
sys.path.insert(0, str(ROOT / "modelScript/exporters"))

import creature_anim_frames as frames  # noqa: E402
from review_creature_assets import sample  # noqa: E402

CLIENT_ASSETS = ROOT / "client/src/main/resources/assets/bong"
REVIEW_ROOT = ROOT / "model-review/anim"
GROUND_TOLERANCE = 0.6  # 与 v2 流水线一致：入地超 0.6px 报错
SAMPLES_PER_CLIP = 48


def stage_assets(name: str, scratch: Path) -> Path:
    """临时资源树：geo / animations 直接引用，PNG 缩放到 geo 声明尺寸。"""

    geo = CLIENT_ASSETS / "geo" / f"{name}.geo.json"
    declared = json.loads(geo.read_text(encoding="utf-8"))["minecraft:geometry"][0]["description"]
    size = (declared["texture_width"], declared["texture_height"])
    (scratch / "geo").mkdir(parents=True)
    (scratch / "animations").mkdir()
    (scratch / "textures/entity/fauna").mkdir(parents=True)
    (scratch / "geo" / geo.name).symlink_to(geo)
    animation = CLIENT_ASSETS / "animations" / f"{name}.animation.json"
    (scratch / "animations" / animation.name).symlink_to(animation)
    texture = Image.open(CLIENT_ASSETS / "textures/entity/fauna" / f"{name}.png").convert("RGBA")
    if texture.size != size:
        texture = texture.resize(size, Image.NEAREST)
    texture.save(scratch / "textures/entity/fauna" / f"{name}.png")
    return scratch


def lowest_point(rig, pose) -> float:
    world = rig.world(pose)
    low = np.inf
    for bone in rig.order:
        vertices = rig.bone_points(bone)
        if len(vertices):
            low = min(low, float((vertices @ world[bone][:3, :3].T + world[bone][:3, 3])[:, 1].min()))
    return low


def ground_report(clip_name: str, clip: dict, rig, bind_low: float) -> str:
    """整段采样，返回最深入地量（相对静止姿态的最低点）。"""

    length = float(clip["animation_length"])
    deepest, at = 0.0, 0.0
    for t in np.linspace(0.0, length, SAMPLES_PER_CLIP):
        depth = bind_low - lowest_point(rig, sample(clip, rig, float(t)))
        if depth > deepest:
            deepest, at = depth, float(t)
    verdict = "OK" if deepest <= GROUND_TOLERANCE else "入地超限"
    return f"{clip_name}: 最深入地 {deepest:.2f}px @ {at:.2f}s（静止最低 {bind_low:.2f}）{verdict}"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("name")
    parser.add_argument("--clip", action="append", required=True)
    parser.add_argument("--gif", action="store_true", help="出 20fps 3/4 原速 GIF 到 model-review/anim/<name>/")
    parser.add_argument("--sheet", action="store_true", help="出关键帧接触表到 --out")
    parser.add_argument("--frames", type=int, default=6)
    parser.add_argument("--size", type=int, default=440)
    parser.add_argument("--sheet-size", type=int, default=300, help="接触表每格像素")
    parser.add_argument("--out", type=Path, default=Path("/tmp/large_anim_preview"))
    parser.add_argument("--facing", default="-z", choices=("-z", "+z"))
    parser.add_argument("--ground", action="store_true", help="只做入地自查")
    args = parser.parse_args()

    with tempfile.TemporaryDirectory() as temporary:
        scratch = Path(temporary)
        assets = stage_assets(args.name, scratch / "assets")
        rig, bbmodel = frames.load_rig(args.name, assets / "geo" / f"{args.name}.geo.json",
                                       assets / "textures/entity/fauna" / f"{args.name}.png", scratch)
        clips = json.loads((assets / "animations" / f"{args.name}.animation.json").read_text(encoding="utf-8"))["animations"]
        from bbmodel_maker.rig.animkit import Pose
        bind_low = lowest_point(rig, Pose())
        prefix = f"animation.bong.{args.name}."
        for short in args.clip:
            clip = clips[prefix + short]
            print(ground_report(short, clip, rig, bind_low))
            if args.ground:
                continue
            if args.gif:
                out = REVIEW_ROOT / args.name / f"{short}.gif"
                count, total = frames.render_gif(clip, rig, bbmodel, args.size, args.facing, 500, out)
                print(f"  {short}.gif: {count} 帧 / 一轮 {total}ms → {out}")
            if args.sheet:
                times = frames.default_times(clip, args.frames)
                sheet = frames.render_clip(args.name, short, clip, rig, bbmodel, times, args.sheet_size, args.facing)
                args.out.mkdir(parents=True, exist_ok=True)
                sheet.save(args.out / f"{args.name}.{short}.png")
                print(f"  sheet → {args.out / (args.name + '.' + short + '.png')}")


if __name__ == "__main__":
    main()
