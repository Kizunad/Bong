#!/usr/bin/env python3
"""按「客户端真的会读的那几个文件」渲生物动画关键帧，给人工闸门看。

输入是安装到 client 的 GeckoLib 三件套（geo.json / animation.json / 贴图 PNG），
不是作者 bbmodel——这样渲出来的就是导出之后的样子，导出器有没有翻错轴一眼可见。
坐标逆变换与关键帧插值复用 ``exporters/review_creature_assets.py`` 的实现。

每段动画出一张图：列 = 时间点，行 = 两个固定视角（正面 3/4、右侧）。整段动画共用一个取景，
帧与帧之间的位移是真位移，不是自动取景的抖动。

用法::

    python3 modelScript/tools/creature_anim_frames.py --name fuya_v2 --out /tmp/fuya
    python3 modelScript/tools/creature_anim_frames.py --name fuya_v2 --clip walk --times 0,0.25,0.5,0.75
    python3 modelScript/tools/creature_anim_frames.py --name fuya_v2 --bind   # 只出静止六视角
    python3 modelScript/tools/creature_anim_frames.py --name fuya_v2 --gif --size 440 --out /tmp/fuya
"""

from __future__ import annotations

import argparse
import json
import sys
import tempfile
from pathlib import Path

import numpy as np
from bbmodel_maker.render import framing
from bbmodel_maker.render.render_bbmodel import render
from bbmodel_maker.rig.animkit import Pose, PoseRig
from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "exporters"))
from export_creature_assets import CLIENT  # noqa: E402
from import_creature_geo import import_geo  # noqa: E402
from review_creature_assets import focus, sample  # noqa: E402

from gif_timing import gif_schedule  # noqa: E402  两个预览工具共用的 GIF 时间表

# 游戏里 GeckoLib 生物面朝 -Z，缺省按它取景；视角名由朝向派生，标签写的就是实际照到的面。
# 模型若是反着建的（正面在 +Z），用 --facing +z 才能从正面看动画，标题会写明。
GAME_FACING = "-z"
CLIP_VIEWS = ("3/4", "SIDE_R", "TOP")


def non_negative_int(text: str) -> int:
    """argparse 类型：非负整数。GIF 帧时长加上负数会让 Pillow 报错或写出坏 GIF。"""

    value = int(text)
    if value < 0:
        raise argparse.ArgumentTypeError(f"必须是非负整数，收到 {value}")
    return value


def load_rig(name: str, geo: Path, texture: Path, scratch: Path) -> tuple[PoseRig, Path]:
    geometry = json.loads(geo.read_text(encoding="utf-8"))
    bbmodel = scratch / f"{name}.bbmodel"
    bbmodel.write_text(json.dumps(import_geo(geometry, texture.read_bytes(), name)), encoding="utf-8")
    return PoseRig(bbmodel), bbmodel


def default_times(clip: dict, count: int) -> list[float]:
    """循环动画在一个周期内均匀取点；一次性动画首尾都要，看得到起势和收势。"""

    length = float(clip["animation_length"])
    looped = clip.get("loop") is True
    return [float(t) for t in np.linspace(0.0, length, count, endpoint=not looped)]


def facing_note(facing: str) -> str:
    if facing == GAME_FACING:
        return f"facing {facing}"
    return f"facing {facing} (NOT the in-game {GAME_FACING} convention)"


def render_clip(name: str, clip_name: str, clip: dict, rig: PoseRig, bbmodel: Path,
                times: list[float], size: int, facing: str) -> "framing.Image.Image":
    poses = [sample(clip, rig, t) for t in times]
    views = [framing.view_by_name(facing, view) for view in CLIP_VIEWS]
    camera = focus(rig, poses, views)
    tiles = []
    for view in views:
        for time, pose in zip(times, poses):
            image = render(bbmodel, yaw=view.yaw, pitch=view.pitch, size=size,
                           focus=camera, xform=rig.element_xform(pose), shading="mc")[0]
            tiles.append((f"{view.label}  t={time:.2f}s", image))
    loop = "loop" if clip.get("loop") is True else "once"
    title = f"{name}.{clip_name} | {clip['animation_length']}s {loop} | same camera, {facing_note(facing)}"
    return framing.contact_sheet(tiles, title=title, columns=len(times))


GIF_FPS = 20
GIF_VIEW = "3/4"
GIF_COLOURS = 96  # 调色板上限：体素贴图本来就只有几十种颜色，96 色看不出损失；4 秒的 idle 也能压在 2MB 内
GIF_BACKGROUND = (22, 23, 26)  # 与 render() 的缺省底色一致，GIF 和 PNG 看起来是同一套预览


def render_gif(clip: dict, rig: PoseRig, bbmodel: Path, size: int, facing: str,
               end_hold_ms: int, out: Path) -> tuple[int, int]:
    """按真实时长逐帧渲一个机位，写 GIF；返回（帧数, 一轮毫秒）。

    时间表见 gif_timing：循环动画不含末帧（末帧 == 首帧，含了会顿一拍）；一次性动画在
    end_hold_ms > 0 时补一帧收势，只承担停留时长，一轮总时长 = 动画时长 + hold。
    整段共用一个取景，帧间位移是真位移。
    """

    length = float(clip["animation_length"])
    looped = clip.get("loop") is True
    count = max(2, round(length * GIF_FPS))
    schedule = gif_schedule(length, count, round(1000 / GIF_FPS), looped, end_hold_ms)
    poses = [sample(clip, rig, t) for t, _ in schedule]
    view = framing.view_by_name(facing, GIF_VIEW)
    camera = focus(rig, poses, [view])

    frames = []
    for pose in poses:
        image = render(bbmodel, yaw=view.yaw, pitch=view.pitch, size=size, bg=GIF_BACKGROUND,
                       focus=camera, xform=rig.element_xform(pose), shading="mc")[0]
        frames.append(image.convert("RGB").convert("P", palette=Image.ADAPTIVE, colors=GIF_COLOURS))

    durations = [duration for _, duration in schedule]
    out.parent.mkdir(parents=True, exist_ok=True)
    frames[0].save(out, save_all=True, append_images=frames[1:], duration=durations,
                   loop=0, disposal=2, optimize=True)
    return len(frames), sum(durations)


def render_bind(name: str, rig: PoseRig, bbmodel: Path, size: int, facing: str) -> "framing.Image.Image":
    views = framing.views_for(facing)
    camera = focus(rig, [Pose()], views)
    tiles = [(view.label, render(bbmodel, yaw=view.yaw, pitch=view.pitch, size=size, focus=camera,
                                 xform=rig.element_xform(Pose()), shading="mc")[0]) for view in views]
    return framing.contact_sheet(tiles, title=f"{name} | bind pose | {facing_note(facing)}", columns=3)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--name", required=True, help="资源名，例如 fuya_v2")
    parser.add_argument("--assets", type=Path, default=CLIENT, help="bong 资源根，默认 client 资源树")
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--clip", action="append", help="只渲这几段（短名，如 walk）")
    parser.add_argument("--times", help="逗号分隔的秒数；缺省按 --frames 均匀取")
    parser.add_argument("--frames", type=int, default=4)
    parser.add_argument("--size", type=int, default=260)
    parser.add_argument("--bind", action="store_true", help="只渲静止六视角")
    parser.add_argument("--facing", default=GAME_FACING, choices=("-z", "+z"),
                        help="模型正面朝哪个轴；缺省按游戏约定 -z")
    parser.add_argument("--gif", action="store_true",
                        help=f"每段动画出一张 {GIF_FPS}fps、{GIF_VIEW} 单机位的原速 GIF（代替关键帧 PNG）")
    parser.add_argument("--end-hold-ms", type=non_negative_int, default=500,
                        help="一次性动画播完在末帧停多久再重播（GIF 用，非负毫秒）；循环动画无缝")
    return parser


def main() -> None:
    args = build_parser().parse_args()

    geo = args.assets / "geo" / f"{args.name}.geo.json"
    texture = args.assets / "textures" / "entity" / "fauna" / f"{args.name}.png"
    args.out.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as temporary:
        rig, bbmodel = load_rig(args.name, geo, texture, Path(temporary))
        if args.bind:
            render_bind(args.name, rig, bbmodel, args.size, args.facing).save(args.out / "bind.png")
            print(f"{args.name}: bind → {args.out / 'bind.png'}")
            return
        animation = args.assets / "animations" / f"{args.name}.animation.json"
        clips = json.loads(animation.read_text(encoding="utf-8"))["animations"]
        prefix = f"animation.bong.{args.name}."
        for full_name, clip in clips.items():
            short = full_name.removeprefix(prefix)
            if args.clip and short not in args.clip:
                continue
            if args.gif:
                out = args.out / f"{short}.gif"
                count, total_ms = render_gif(clip, rig, bbmodel, args.size, args.facing, args.end_hold_ms, out)
                print(f"{args.name}.{short}: {count} 帧 / 一轮 {total_ms}ms → {out}")
                continue
            times = ([float(t) for t in args.times.split(",")] if args.times
                     else default_times(clip, args.frames))
            sheet = render_clip(args.name, short, clip, rig, bbmodel, times, args.size, args.facing)
            sheet.save(args.out / f"{short}.png")
            print(f"{args.name}.{short}: {len(times)} 帧 → {args.out / (short + '.png')}")


if __name__ == "__main__":
    main()
