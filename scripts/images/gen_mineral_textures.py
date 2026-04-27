"""plan-mineral-v1 M1 — 程序化合成 15 张 vanilla ore 改色贴图。

输出到 client/src/main/resources/assets/minecraft/textures/block/，覆盖 vanilla 16x16
ore PNG。设计 = stone/deepslate 底 + 矿斑簇 + 末法暗调（去七彩，低饱和，朴素）。

跑法：
    python3 scripts/images/gen_mineral_textures.py
"""

from __future__ import annotations

import hashlib
from dataclasses import dataclass
from pathlib import Path

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
OUT_DIR = ROOT / "client/src/main/resources/assets/minecraft/textures/block"

SIZE = 16
SEED_SALT = "bong-mineral-v1"


@dataclass(frozen=True)
class OreSpec:
    """vanilla block 名 → mineral_id + 配色。"""

    block_name: str
    mineral_id: str
    palette: tuple[tuple[int, int, int], ...]
    base: str
    cluster_density: float = 0.16
    cluster_jitter: int = 18
    note: str = ""


# stone / deepslate 底色，全部低饱和、偏暗、汉代漆器调子
BASE_STONE = np.array([118, 118, 118], dtype=np.int16)  # 灰岩
BASE_DEEPSLATE = np.array([72, 72, 76], dtype=np.int16)  # 深岩
BASE_NETHERRACK = np.array([114, 56, 50], dtype=np.int16)  # 下界岩


SPECS: tuple[OreSpec, ...] = (
    OreSpec(
        "iron_ore",
        "fan_tie",
        ((148, 122, 96), (110, 84, 64), (164, 138, 110)),
        "stone",
        cluster_density=0.20,
        note="凡铁 — 灰褐 + 锈斑",
    ),
    OreSpec(
        "deepslate_iron_ore",
        "cu_tie",
        ((92, 78, 70), (74, 60, 54), (60, 48, 42)),
        "deepslate",
        cluster_density=0.22,
        note="粗铁 — 暗灰 + 结块状锈",
    ),
    OreSpec(
        "copper_ore",
        "za_gang",
        ((80, 102, 88), (62, 84, 70), (102, 122, 100)),
        "stone",
        cluster_density=0.18,
        note="杂钢 — 暗青绿（出土青铜）",
    ),
    OreSpec(
        "redstone_ore",
        "ling_tie",
        ((90, 60, 100), (70, 44, 82), (110, 80, 130)),
        "stone",
        cluster_density=0.20,
        cluster_jitter=12,
        note="灵铁 — 冷紫内敛（去原版红光）",
    ),
    OreSpec(
        "ancient_debris",
        "sui_tie",
        ((182, 168, 144), (134, 116, 90), (98, 78, 62)),
        "netherrack",
        cluster_density=0.30,
        cluster_jitter=22,
        note="髓铁 — 骨白 + 深褐纹",
    ),
    OreSpec(
        "obsidian",
        "can_tie",
        ((88, 56, 42), (58, 36, 28), (122, 84, 60)),
        "deepslate",
        cluster_density=0.45,
        cluster_jitter=24,
        note="残铁 — 暗褐 + 风化碎裂纹",
    ),
    OreSpec(
        "gold_ore",
        "ku_jin",
        ((148, 124, 78), (118, 96, 58), (96, 78, 46)),
        "stone",
        cluster_density=0.18,
        cluster_jitter=14,
        note="枯金 — 土黄 + 裂纹（金已枯）",
    ),
    OreSpec(
        "deepslate_gold_ore",
        "ku_jin_deep",
        ((128, 104, 64), (98, 80, 48), (74, 62, 40)),
        "deepslate",
        cluster_density=0.18,
        cluster_jitter=14,
        note="枯金深岩变体",
    ),
    OreSpec(
        "diamond_ore",
        "ling_shi",
        ((158, 178, 184), (124, 146, 152), (90, 108, 116)),
        "stone",
        cluster_density=0.16,
        cluster_jitter=10,
        note="灵石 — 青白半透（去钻石高亮蓝）",
    ),
    OreSpec(
        "emerald_ore",
        "ling_jing",
        ((78, 110, 86), (58, 88, 66), (102, 134, 108)),
        "stone",
        cluster_density=0.18,
        cluster_jitter=14,
        note="灵晶 — 青翠偏暗（去七彩）",
    ),
    OreSpec(
        "lapis_ore",
        "yu_sui",
        ((178, 188, 178), (148, 162, 152), (200, 208, 200)),
        "stone",
        cluster_density=0.20,
        cluster_jitter=8,
        note="玉髓 — 温润青白（去深蓝）",
    ),
    OreSpec(
        "coal_ore",
        "wu_yao",
        ((36, 30, 32), (24, 20, 22), (96, 38, 36)),
        "stone",
        cluster_density=0.30,
        cluster_jitter=20,
        note="乌曜石 — 漆黑 + 赤红暗纹",
    ),
    OreSpec(
        "nether_gold_ore",
        "zhu_sha",
        ((140, 56, 48), (104, 40, 36), (170, 130, 56)),
        "netherrack",
        cluster_density=0.26,
        cluster_jitter=22,
        note="朱砂 — 深朱红 + 硫黄黄晶簇",
    ),
    OreSpec(
        "nether_quartz_ore",
        "xie_fen",
        ((110, 88, 122), (84, 64, 96), (138, 116, 148)),
        "netherrack",
        cluster_density=0.18,
        cluster_jitter=14,
        note="邪粉 — 暗紫白裂纹（v2+）",
    ),
)

# dan_sha 与 ling_tie 共占 redstone_ore — 实际由 server 按 biome 区分 mineral_id；
# 贴图层只保留一个（ling_tie 版），dan_sha 视觉差异留给 v2 CustomModelData。


def base_array(name: str) -> np.ndarray:
    if name == "stone":
        base = BASE_STONE
    elif name == "deepslate":
        base = BASE_DEEPSLATE
    elif name == "netherrack":
        base = BASE_NETHERRACK
    else:
        raise ValueError(f"unknown base: {name}")
    img = np.tile(base, (SIZE, SIZE, 1)).astype(np.int16)
    return img


def stone_noise(rng: np.random.Generator, base: np.ndarray, jitter: int = 12) -> np.ndarray:
    noise = rng.integers(-jitter, jitter + 1, size=base.shape, dtype=np.int16)
    return np.clip(base + noise, 0, 255)


def cluster_mask(rng: np.random.Generator, density: float) -> np.ndarray:
    """随机散布 ore cluster：先撒种子再做 1-2 步形态扩张，模拟 vanilla ore 块状。"""
    seeds = rng.random((SIZE, SIZE)) < density * 0.5
    mask = seeds.copy()
    for _ in range(2):
        grow = mask.copy()
        grow[1:, :] |= mask[:-1, :]
        grow[:-1, :] |= mask[1:, :]
        grow[:, 1:] |= mask[:, :-1]
        grow[:, :-1] |= mask[:, 1:]
        # 二次抽选：邻居命中且通过密度门槛
        keep = rng.random((SIZE, SIZE)) < density * 1.4
        mask |= grow & keep
    return mask


def palette_color(
    rng: np.random.Generator,
    palette: tuple[tuple[int, int, int], ...],
    jitter: int,
) -> np.ndarray:
    idx = rng.integers(0, len(palette))
    base = np.array(palette[idx], dtype=np.int16)
    noise = rng.integers(-jitter, jitter + 1, size=3, dtype=np.int16)
    return np.clip(base + noise, 0, 255)


def render_ore(spec: OreSpec) -> Image.Image:
    seed = int.from_bytes(
        hashlib.sha256(f"{SEED_SALT}:{spec.block_name}".encode()).digest()[:8], "big"
    )
    rng = np.random.default_rng(seed)

    img = stone_noise(rng, base_array(spec.base), jitter=10)
    mask = cluster_mask(rng, spec.cluster_density)
    ys, xs = np.where(mask)
    for y, x in zip(ys, xs):
        img[y, x] = palette_color(rng, spec.palette, spec.cluster_jitter)

    # 单像素描边：cluster 边缘做一步 darken，强化 vanilla ore 视觉锚定
    edge = mask & ~np.pad(mask, 1, mode="constant")[1:-1, 1:-1]  # type: ignore[index]
    if edge.any():
        ys, xs = np.where(edge)
        for y, x in zip(ys, xs):
            img[y, x] = np.clip(img[y, x] - 28, 0, 255)

    rgba = np.dstack([img.astype(np.uint8), np.full((SIZE, SIZE), 255, dtype=np.uint8)])
    return Image.fromarray(rgba, mode="RGBA")


def main() -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    for spec in SPECS:
        out = OUT_DIR / f"{spec.block_name}.png"
        img = render_ore(spec)
        img.save(out, format="PNG", optimize=True)
        print(f"  ✓ {spec.block_name:<24} → {spec.mineral_id:<14} ({spec.note})")
    print(f"\n生成 {len(SPECS)} 张贴图 → {OUT_DIR.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
