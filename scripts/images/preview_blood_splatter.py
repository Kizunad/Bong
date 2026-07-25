"""离屏预览地面血溅 decal —— 逐值镜像 client 的 BloodSplatterLayout。

粒子效果没法 headless 进游戏渲，但血溅是"贴地正交投影"这一种最好模拟的情形：
俯视 = 直接把每个 quad 按位置/长宽/朝向贴到平面上。本脚本把
{@code java.util.Random} 的 LCG 精确重现（同 seed 逐个 nextDouble 完全一致），
再照抄 BloodSplatterLayout 的抽样顺序，因此渲出来的就是进游戏会看到的那一摊。

用途：视觉资产纪律的自评环节——看"是不是同心圆""像不像血"。

跑法：
    python3 scripts/images/preview_blood_splatter.py [输出目录]
"""

from __future__ import annotations

import math
import sys
from dataclasses import dataclass
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
TEX_DIR = ROOT / "client/src/main/resources/assets/bong/textures/particle"

TAU = math.pi * 2.0

# ---- BloodSplatterLayout 常数（改 Java 侧记得同步这里，否则预览就骗人了） ----
POOL_HALF_MIN = 0.24
POOL_HALF_SPAN = 0.16
SPREAD_MIN = 0.38
SPREAD_SPAN = 0.42
DROPLET_MIN = 4
DROPLET_SPAN = 6
STREAK_MIN = 1
STREAK_SPAN = 2
DROPLET_ANGLE_SPREAD = 1.70
BACKSPLASH_CHANCE = 0.15
STREAK_CONE = 1.10

BLOOD_RGB = (0x8C, 0x1F, 0x1F)
GROUND_RGB = (78, 74, 66)


class JavaRandom:
    """java.util.Random 的逐位复刻（只需要 nextDouble）。"""

    def __init__(self, seed: int) -> None:
        self.seed = (seed ^ 0x5DEECE66D) & ((1 << 48) - 1)

    def _next(self, bits: int) -> int:
        self.seed = (self.seed * 0x5DEECE66D + 0xB) & ((1 << 48) - 1)
        return self.seed >> (48 - bits)

    def next_double(self) -> float:
        return ((self._next(26) << 27) + self._next(27)) * (2.0**-53)


@dataclass(frozen=True)
class Splat:
    kind: str
    dx: float
    dz: float
    half_length: float
    half_width: float
    rotation: float
    alpha_scale: float
    age_scale: float


def _java_round(v: float) -> int:
    """Java Math.round(double) = floor(v + 0.5)。"""
    return math.floor(v + 0.5)


def build_layout(strength: float, seed: int) -> list[Splat]:
    s = min(1.0, max(0.0, strength))
    rnd = JavaRandom(seed)
    pool_half = POOL_HALF_MIN + POOL_HALF_SPAN * s
    spread = SPREAD_MIN + SPREAD_SPAN * s
    dominant = rnd.next_double() * TAU

    out: list[Splat] = []

    pool_offset = 0.10 * spread * rnd.next_double()
    pool_angle = rnd.next_double() * TAU
    pool_length = pool_half * (0.88 + 0.26 * rnd.next_double())
    pool_width = pool_length * (0.80 + 0.28 * rnd.next_double())
    out.append(
        Splat(
            "POOL",
            pool_offset * math.cos(pool_angle),
            pool_offset * math.sin(pool_angle),
            pool_length,
            pool_width,
            rnd.next_double() * TAU,
            1.0,
            1.0,
        )
    )

    droplets = DROPLET_MIN + _java_round(DROPLET_SPAN * s)
    for i in range(droplets):
        stratum = (i + rnd.next_double()) / droplets
        radial = min(
            1.0,
            max(0.12, 0.26 + 0.74 * (stratum**0.85) * (0.80 + 0.40 * rnd.next_double())),
        )
        radius = spread * radial
        jitter = (
            rnd.next_double() + rnd.next_double() + rnd.next_double() - 1.5
        ) * DROPLET_ANGLE_SPREAD
        back = math.pi if rnd.next_double() < BACKSPLASH_CHANCE else 0.0
        angle = dominant + jitter + back
        size = pool_half * (0.10 + 0.16 * rnd.next_double())
        stretch = 1.0 + 0.9 * radial * rnd.next_double()
        out.append(
            Splat(
                "DROPLET",
                radius * math.cos(angle),
                radius * math.sin(angle),
                size * stretch,
                size,
                angle,
                0.55 + 0.35 * rnd.next_double(),
                0.50 + 0.35 * rnd.next_double(),
            )
        )

    streaks = STREAK_MIN + _java_round(STREAK_SPAN * s)
    for _ in range(streaks):
        angle = dominant + (rnd.next_double() - 0.5) * STREAK_CONE
        radius = spread * (0.30 + 0.42 * rnd.next_double())
        width = pool_half * (0.07 + 0.05 * rnd.next_double())
        length = width * (3.00 + 2.20 * rnd.next_double())
        out.append(
            Splat(
                "STREAK",
                radius * math.cos(angle),
                radius * math.sin(angle),
                length,
                width,
                angle,
                0.38 + 0.28 * rnd.next_double(),
                0.45 + 0.30 * rnd.next_double(),
            )
        )

    return out


def _tinted(path: Path) -> Image.Image:
    img = Image.open(path).convert("RGBA")
    px = img.load()
    for y in range(img.size[1]):
        for x in range(img.size[0]):
            r, g, b, a = px[x, y]
            px[x, y] = (
                r * BLOOD_RGB[0] // 255,
                g * BLOOD_RGB[1] // 255,
                b * BLOOD_RGB[2] // 255,
                a,
            )
    return img


def render(strength: float, seed: int, px_per_block: int = 190, span_blocks: float = 2.6) -> Image.Image:
    """俯视图：1 blocks = px_per_block 像素，画布覆盖 span_blocks × span_blocks。"""
    size = int(span_blocks * px_per_block)
    canvas = Image.new("RGBA", (size, size), GROUND_RGB + (255,))
    splat_tex = [_tinted(TEX_DIR / f"blood_splat_{i}.png") for i in range(4)]
    drop_tex = [_tinted(TEX_DIR / f"blood_drop_{i}.png") for i in range(3)]
    streak_tex = [_tinted(TEX_DIR / f"blood_streak_{i}.png") for i in range(2)]

    # sprite 变体的选取在运行时是 world.random，与布局 RNG 无关；预览用一个独立
    # 的确定性序列铺开变体，效果等价于"每个血点随机取一张形状"。
    pick = JavaRandom(seed ^ 0x5EED)
    center = size / 2.0

    for splat in build_layout(strength, seed):
        if splat.kind == "STREAK":
            tex = streak_tex[int(pick.next_double() * len(streak_tex))]
        elif splat.kind == "DROPLET":
            tex = drop_tex[int(pick.next_double() * len(drop_tex))]
        else:
            tex = splat_tex[int(pick.next_double() * len(splat_tex))]
        w_px = max(2, int(round(splat.half_length * 2 * px_per_block)))
        h_px = max(2, int(round(splat.half_width * 2 * px_per_block)))
        piece = tex.resize((w_px, h_px), Image.BILINEAR)
        # decal 的 rotationRad 是绕 +Y 的 CCW 角；俯视图 z 轴朝下，故取负角旋转。
        piece = piece.rotate(-math.degrees(splat.rotation), expand=True, resample=Image.BICUBIC)
        if splat.alpha_scale < 1.0:
            alpha = piece.getchannel("A").point(lambda v: int(v * splat.alpha_scale))
            piece.putalpha(alpha)
        cx = center + splat.dx * px_per_block
        cy = center + splat.dz * px_per_block
        canvas.alpha_composite(piece, (int(cx - piece.size[0] / 2), int(cy - piece.size[1] / 2)))

    return canvas


def main() -> None:
    out_dir = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "build/blood-preview"
    out_dir.mkdir(parents=True, exist_ok=True)
    cases = [(0.3, 11), (0.55, 22), (0.8, 33), (1.0, 44), (1.0, 55), (0.65, 66)]
    tiles = [render(strength, seed) for strength, seed in cases]
    tile = tiles[0].size[0]
    sheet = Image.new("RGBA", (tile * 3, tile * 2), GROUND_RGB + (255,))
    for idx, img in enumerate(tiles):
        sheet.paste(img, ((idx % 3) * tile, (idx // 3) * tile))
    path = out_dir / "blood_splatter_preview.png"
    sheet.save(path)
    print(f"wrote {path}  cases={cases}")


if __name__ == "__main__":
    main()
