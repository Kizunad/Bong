"""64×64 调试皮肤：每个部件每个面一种底色，叠像素坐标格和逐像素微扰。"""

from __future__ import annotations

import colorsys

from PIL import Image

from . import vanilla

PART_ORDER = ("head", "hat", "body", "jacket", "right_arm", "right_sleeve", "left_arm", "left_sleeve",
              "right_leg", "right_pants", "left_leg", "left_pants")


def build() -> Image.Image:
    image = Image.new("RGBA", (64, 64), (0, 0, 0, 0))
    pixels = image.load()
    for part_index, name in enumerate(PART_ORDER):
        # 粗臂细臂 UV 起点相同，细臂只是少一列，因此用粗臂版面作皮肤。
        part = vanilla.part(name, False)
        for face_index, face in enumerate(vanilla.FACES):
            quad = vanilla.face_quad(part, face)
            us = [vx[3] for vx in quad]
            vs = [vx[4] for vx in quad]
            hue = ((part_index * 6 + face_index) * 0.61803398875) % 1.0
            r, g, b = colorsys.hsv_to_rgb(hue, 0.55 if name in vanilla.OVERLAY.values() else 0.7, 0.85)
            for v in range(int(min(vs)), int(max(vs))):
                for u in range(int(min(us)), int(max(us))):
                    # 逐像素微扰让任何一格错位都能在对比里显出来；4 像素格线标出坐标
                    shift = ((u * 7 + v * 13) % 9 - 4) * 3
                    shade = 0.72 if (u % 4 == 0 or v % 4 == 0) else 1.0
                    pixels[u, v] = tuple(max(0, min(255, int((c * 255 + shift) * shade))) for c in (r, g, b)) + (255,)
    return image
