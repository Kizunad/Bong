#!/usr/bin/env python3
"""把同一只生物的几段动画首尾连播成一张 GIF，专看「段与段之间接不接得上」。

单段 GIF（creature_anim_frames --gif）只能看一段自己好不好看；暴起 → 收拢 → 伪装 idle
这种链路，接缝处的跳变要连起来放才看得出。每一段按真实时长、20fps 逐帧采样，
整条链共用一个取景，帧间位移是真位移。

用法::

    python3 modelScript/tools/creature_anim_chain_gif.py --name ash_spider_v2 \\
        --chain idle:0.4,ambush_burst,fold,idle:0.8 --out model-review/anim/ash_spider_v2/chain.gif

链节写法：``clip`` 播整段；``clip:秒数`` 只播这段动画的前几秒（循环动画取个头尾用）。
"""

from __future__ import annotations

import argparse
import json
import tempfile
from pathlib import Path

from bbmodel_maker.render import framing
from bbmodel_maker.render.render_bbmodel import render
from PIL import Image

from creature_anim_frames import GAME_FACING, GIF_BACKGROUND, GIF_COLOURS, GIF_FPS, GIF_VIEW, load_rig
from export_creature_assets import CLIENT  # noqa: E402
from review_creature_assets import focus, sample  # noqa: E402


def parse_chain(text: str) -> list[tuple[str, float | None]]:
    chain = []
    for link in text.split(","):
        name, _, seconds = link.partition(":")
        chain.append((name.strip(), float(seconds) if seconds else None))
    return chain


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--name", required=True)
    parser.add_argument("--chain", required=True, help="逗号分隔的链节，如 idle:0.4,ambush_burst,fold,idle:0.8")
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--size", type=int, default=440)
    parser.add_argument("--end-hold-ms", type=int, default=600, help="整条链播完停多久再重播")
    args = parser.parse_args()

    geo = CLIENT / "geo" / f"{args.name}.geo.json"
    texture = CLIENT / "textures" / "entity" / "fauna" / f"{args.name}.png"
    clips = json.loads((CLIENT / "animations" / f"{args.name}.animation.json").read_text(encoding="utf-8"))["animations"]
    prefix = f"animation.bong.{args.name}."

    with tempfile.TemporaryDirectory() as temporary:
        rig, bbmodel = load_rig(args.name, geo, texture, Path(temporary))
        poses = []
        for clip_name, seconds in parse_chain(args.chain):
            clip = clips[prefix + clip_name]
            length = seconds if seconds is not None else float(clip["animation_length"])
            for index in range(max(1, round(length * GIF_FPS))):
                poses.append(sample(clip, rig, index / GIF_FPS))

        view = framing.view_by_name(GAME_FACING, GIF_VIEW)
        camera = focus(rig, poses, [view])
        frames = [
            render(bbmodel, yaw=view.yaw, pitch=view.pitch, size=args.size, bg=GIF_BACKGROUND, focus=camera,
                   xform=rig.element_xform(pose), shading="mc")[0]
            .convert("RGB").convert("P", palette=Image.ADAPTIVE, colors=GIF_COLOURS)
            for pose in poses
        ]

    durations = [round(1000 / GIF_FPS)] * len(frames)
    durations[-1] += args.end_hold_ms
    args.out.parent.mkdir(parents=True, exist_ok=True)
    frames[0].save(args.out, save_all=True, append_images=frames[1:], duration=durations,
                   loop=0, disposal=2, optimize=True)
    print(f"{args.name} chain [{args.chain}]: {len(frames)} 帧 → {args.out}")


if __name__ == "__main__":
    main()
