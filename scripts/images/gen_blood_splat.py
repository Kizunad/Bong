"""程序化合成地面血溅 decal 贴图（战斗腿伤血渍 bong:combat_leg_wound_decal 用）。

背景：腿伤血渍此前复用 `lingqi_ripple`（灵气涟漪）贴图——那张图是**同心圆环**，
染红以后玩家看到的是"红色同心圆靶心"而不是血。血渍需要的形状特征恰好相反：

* 轮廓不规则（多阶谐波 + 随机"手指"外溢），不存在任何以中心为圆心的环
* 主血泊之外散落卫星血点，大小、距离都不等
* 边缘暗一圈（血膜堆积），内部有湿润的斑驳不匀

贴图一律画成**白色 + 透明度造型**，运行时由粒子 tint（#8C1F1F 暗血红）上色；
RGB 通道只承载明暗（边缘 0.6 倍压暗），乘上 tint 就是深浅两层血色。

输出：
    client/src/main/resources/assets/bong/textures/particle/blood_splat_{0..3}.png  64x64
    client/src/main/resources/assets/bong/textures/particle/blood_drop_{0..2}.png   32x32
    client/src/main/resources/assets/bong/textures/particle/blood_streak_{0,1}.png  128x32

三套贴图各有分工：splat = 脚下主血泊（大、瓣多），drop = 卫星血点
（小、近椭圆——主血泊贴图缩小了看会变成"红色枫叶"，所以血点必须另出一套），
streak = 甩出去的拖尾血道。

跑法：
    python3 scripts/images/gen_blood_splat.py

确定性：全部随机数来自固定 seed，重跑产出逐字节一致（资源包 sha1 才稳得住）。
"""

from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
OUT_DIR = ROOT / "client/src/main/resources/assets/bong/textures/particle"

SPLAT_SIZE = 64
DROP_SIZE = 32
STREAK_SIZE = (128, 32)
SUPERSAMPLE = 4

# 边缘压暗系数：血膜边缘比中心深。乘 tint(#8C1F1F) 后 ≈ #3E0D0D 的近黑暗血色。
EDGE_SHADE = 0.44
# 内部最亮也压一档——地上的血是暗的，全亮会读成"番茄酱"。
CORE_SHADE = 0.94
# 从边缘往里多深恢复到 CORE_SHADE（归一化半径比例）。
SHADE_RAMP = 0.36


@dataclass(frozen=True)
class BlobSpec:
    """一团血：中心 (cx, cy)、基准半径 r、轮廓扰动强度、外溢手指数量。"""

    cx: float
    cy: float
    r: float
    wobble: float = 0.22
    fingers: int = 0
    finger_len: float = 0.45


@dataclass(frozen=True)
class SplatSpec:
    """一张血溅贴图：主血泊 + 卫星血点 + 细碎血珠。"""

    name: str
    seed: int
    blobs: tuple[BlobSpec, ...]
    speck_count: int = 6
    speck_ring: tuple[float, float] = (0.45, 0.92)
    speck_r: tuple[float, float] = (0.015, 0.055)
    # 血珠只在这个角度扇区内散（弧度区间），造出"溅射有方向"而不是四面均匀
    speck_arc: tuple[float, float] = (0.0, 2.0 * np.pi)
    mottle: float = 0.16
    tags: tuple[str, ...] = field(default=())


def _angle_delta(theta: np.ndarray, center: float) -> np.ndarray:
    """角度差，折回 (-π, π]。"""
    return (theta - center + np.pi) % (2.0 * np.pi) - np.pi


def _blob_field(
    xx: np.ndarray,
    yy: np.ndarray,
    blob: BlobSpec,
    rng: np.random.Generator,
) -> tuple[np.ndarray, np.ndarray]:
    """返回 (coverage, innerness)。

    coverage: 0..1 覆盖率（супersample 后即抗锯齿边）；
    innerness: 0..1，0 = 贴边，1 = 深处，用来做边缘压暗。
    """
    dx = xx - blob.cx
    dy = yy - blob.cy
    dist = np.hypot(dx, dy)
    theta = np.arctan2(dy, dx)

    # 轮廓 = 基准半径 × (1 + Σ 谐波)。k 从 2 起：k=1 只会整体偏移圆心，
    # 高阶谐波才产生"瓣"。振幅随 k 衰减，保证轮廓仍闭合不自交。
    radius = np.ones_like(dist)
    for k in range(2, 8):
        amp = blob.wobble * rng.uniform(0.35, 1.0) / (k - 0.6)
        phase = rng.uniform(0.0, 2.0 * np.pi)
        radius = radius + amp * np.cos(k * theta + phase)

    # 手指：血液流动失稳甩出的尖刺，局部把半径顶出去。
    for _ in range(blob.fingers):
        f_theta = rng.uniform(0.0, 2.0 * np.pi)
        f_width = rng.uniform(0.10, 0.26)
        f_len = blob.finger_len * rng.uniform(0.5, 1.0)
        radius = radius + f_len * np.exp(-((_angle_delta(theta, f_theta) / f_width) ** 2))

    radius = np.clip(radius, 0.25, 2.4) * blob.r
    # 归一化到"离边界还有多远"：>1 在外，<1 在内。
    ratio = dist / np.maximum(radius, 1e-6)
    coverage = (ratio <= 1.0).astype(np.float32)
    innerness = np.clip((1.0 - ratio) / SHADE_RAMP, 0.0, 1.0).astype(np.float32)
    return coverage, innerness


def _value_noise(shape: tuple[int, int], rng: np.random.Generator, cells: int) -> np.ndarray:
    """低频 value noise（粗网格 + 双线性放大），给血面做湿润斑驳。"""
    coarse = rng.random((cells, cells)).astype(np.float32)
    img = Image.fromarray((coarse * 255.0).astype(np.uint8), mode="L")
    img = img.resize((shape[1], shape[0]), Image.BILINEAR)
    return np.asarray(img, dtype=np.float32) / 255.0


def _compose(
    coverage: np.ndarray,
    innerness: np.ndarray,
    mottle_amount: float,
    rng: np.random.Generator,
) -> np.ndarray:
    """(coverage, innerness) → RGBA float 数组（0..1）。"""
    noise = _value_noise(coverage.shape, rng, cells=6)
    alpha = coverage * (1.0 - mottle_amount * 0.55 * (1.0 - noise))
    shade = EDGE_SHADE + (CORE_SHADE - EDGE_SHADE) * innerness
    shade = shade * (1.0 - mottle_amount * 0.6 * (1.0 - noise))
    rgba = np.zeros(coverage.shape + (4,), dtype=np.float32)
    rgba[..., 0] = shade
    rgba[..., 1] = shade
    rgba[..., 2] = shade
    rgba[..., 3] = alpha
    return rgba


def _downsample(rgba: np.ndarray, out_hw: tuple[int, int]) -> Image.Image:
    """超采样降采样。

    RGB 按 alpha 加权平均；**全透明像素填 {@link EDGE_SHADE} 而不是白**——
    MC 的粒子图集会线性过滤 + mipmap，透明像素的 RGB 依然参与插值，
    留白就会在每颗血点外面渗出一圈灰白光晕（离屏预览里肉眼可见）。
    """
    h, w = out_hw
    s = SUPERSAMPLE
    blocks = rgba.reshape(h, s, w, s, 4)
    alpha = blocks[..., 3]
    weight = alpha.sum(axis=(1, 3))
    rgb = (blocks[..., :3] * alpha[..., None]).sum(axis=(1, 3))
    safe = np.maximum(weight, 1e-6)[..., None]
    rgb = np.where(weight[..., None] > 1e-6, rgb / safe, EDGE_SHADE)
    out = np.zeros((h, w, 4), dtype=np.uint8)
    out[..., :3] = np.clip(rgb * 255.0 + 0.5, 0, 255).astype(np.uint8)
    out[..., 3] = np.clip(alpha.mean(axis=(1, 3)) * 255.0 + 0.5, 0, 255).astype(np.uint8)
    return Image.fromarray(out, mode="RGBA")


def render_splat(spec: SplatSpec) -> Image.Image:
    rng = np.random.default_rng(spec.seed)
    n = SPLAT_SIZE * SUPERSAMPLE
    # 归一化坐标：贴图覆盖 [-1, 1]²，半边长 = 1。
    axis = (np.arange(n, dtype=np.float32) + 0.5) / n * 2.0 - 1.0
    xx, yy = np.meshgrid(axis, axis)

    coverage = np.zeros((n, n), dtype=np.float32)
    innerness = np.zeros((n, n), dtype=np.float32)
    for blob in spec.blobs:
        cov, inner = _blob_field(xx, yy, blob, rng)
        innerness = np.maximum(innerness, inner * cov)
        coverage = np.maximum(coverage, cov)

    # 细碎血珠：只落在指定扇区，半径 / 大小都随机，绝不等距成环。
    lo, hi = spec.speck_ring
    arc_lo, arc_hi = spec.speck_arc
    for _ in range(spec.speck_count):
        t = rng.random()
        dist = lo + (hi - lo) * (t ** 0.75)
        ang = rng.uniform(arc_lo, arc_hi)
        speck = BlobSpec(
            cx=dist * float(np.cos(ang)),
            cy=dist * float(np.sin(ang)),
            r=rng.uniform(*spec.speck_r),
            wobble=0.30,
        )
        cov, inner = _blob_field(xx, yy, speck, rng)
        innerness = np.maximum(innerness, inner * cov)
        coverage = np.maximum(coverage, cov)

    rgba = _compose(coverage, innerness, spec.mottle, rng)
    return _downsample(rgba, (SPLAT_SIZE, SPLAT_SIZE))


@dataclass(frozen=True)
class DropSpec:
    """卫星血点：一颗近椭圆的小血珠 + 一两粒更小的碎珠。

    刻意比 SplatSpec 收敛得多——血点在世界里只有 0.1 个方块见方，
    再刻瓣状轮廓只会糊成一团噪点，反而失去"一滴血"的读感。
    """

    name: str
    seed: int
    r: float = 0.52
    wobble: float = 0.14
    squash: float = 0.82
    tilt: float = 0.0
    micro: int = 2


def render_drop(spec: DropSpec) -> Image.Image:
    rng = np.random.default_rng(spec.seed)
    n = DROP_SIZE * SUPERSAMPLE
    axis = (np.arange(n, dtype=np.float32) + 0.5) / n * 2.0 - 1.0
    xx, yy = np.meshgrid(axis, axis)

    # 椭圆化：沿 tilt 方向压扁，模拟血滴落地的拉伸。
    cos_t, sin_t = float(np.cos(spec.tilt)), float(np.sin(spec.tilt))
    ux = xx * cos_t + yy * sin_t
    uy = -xx * sin_t + yy * cos_t
    uy = uy / spec.squash

    coverage, innerness = _blob_field(ux, uy, BlobSpec(0.0, 0.0, spec.r, spec.wobble), rng)

    for i in range(spec.micro):
        ang = rng.uniform(0.0, 2.0 * np.pi)
        dist = spec.r * rng.uniform(1.25, 1.85)
        micro = BlobSpec(
            cx=dist * float(np.cos(ang)),
            cy=dist * float(np.sin(ang)),
            r=spec.r * rng.uniform(0.10, 0.22) * (1.0 - 0.15 * i),
            wobble=0.20,
        )
        cov, inner = _blob_field(xx, yy, micro, rng)
        innerness = np.maximum(innerness, inner * cov)
        coverage = np.maximum(coverage, cov)

    rgba = _compose(coverage, innerness, 0.10, rng)
    return _downsample(rgba, (DROP_SIZE, DROP_SIZE))


@dataclass(frozen=True)
class StreakSpec:
    """拖尾：钝头在贴图左端（内侧），尖尾甩向右端（外侧）。"""

    name: str
    seed: int
    head_x: float = 0.18
    head_r: float = 0.62
    tail_x: float = 0.70
    bow: float = 0.10
    droplets: int = 3
    mottle: float = 0.14


def render_streak(spec: StreakSpec) -> Image.Image:
    rng = np.random.default_rng(spec.seed)
    w_out, h_out = STREAK_SIZE
    w = w_out * SUPERSAMPLE
    h = h_out * SUPERSAMPLE
    # x ∈ [0,1] 沿长轴（左 = 内侧钝头），y ∈ [-1,1] 横向。
    xs = (np.arange(w, dtype=np.float32) + 0.5) / w
    ys = (np.arange(h, dtype=np.float32) + 0.5) / h * 2.0 - 1.0
    xx, yy = np.meshgrid(xs, ys)
    # x 归一化到全长 1.0、y 归一化到半高 1.0 —— 两轴像素密度差 aspect 倍。
    # 要画"圆"的地方（钝头 / 血珠）必须把 x 偏移乘回 aspect，否则会拉成长椭圆。
    aspect = w_out / (h_out / 2.0)

    # 主干：从钝头到尖尾单调收细的水滴，中轴带一点弧度（bow）。
    span = max(spec.tail_x - spec.head_x, 1e-6)
    t = np.clip((xx - spec.head_x) / span, 0.0, 1.0)
    axis_y = spec.bow * np.sin(np.pi * t)
    # 半宽 profile：头部圆钝 → 尾部缓慢收尖（指数 <1 会鼓成尖牙，用 0.9 接近线性收），
    # 再叠一层沿轴的低频起伏，避免边缘是数学上完美的曲线。
    ripple = 1.0 + 0.10 * np.sin(t * 9.4 + spec.seed % 7) + 0.06 * np.sin(t * 21.7)
    half = spec.head_r * np.power(np.clip(1.0 - t, 0.0, 1.0), 0.9) * ripple
    # 头部前方补一个圆头，别切成平口
    head_dist = np.hypot((xx - spec.head_x) * aspect, yy)
    body = (np.abs(yy - axis_y) <= np.maximum(half, 1e-6)) & (xx >= spec.head_x)
    head = head_dist <= spec.head_r
    coverage = (body | head).astype(np.float32)
    inner_axis = np.clip(1.0 - np.abs(yy - axis_y) / np.maximum(half, 1e-6), 0.0, 1.0)
    innerness = np.clip(inner_axis / SHADE_RAMP, 0.0, 1.0).astype(np.float32) * coverage

    # 甩出的小血珠：主干断掉之后沿长轴继续往外，大小递减、间距不等
    # （等距会重新读成"点划线"，血不是这样甩的）。
    gap = 1.0 - spec.tail_x
    for i in range(spec.droplets):
        slot = (i + rng.uniform(0.15, 0.85)) / spec.droplets
        px = spec.tail_x + gap * (0.18 + 0.78 * slot)
        py = spec.bow * 0.6 + rng.uniform(-0.22, 0.22)
        pr = rng.uniform(0.08, 0.20) * (1.0 - 0.22 * i)
        d = np.hypot((xx - px) * aspect, yy - py)
        cov = (d <= pr).astype(np.float32)
        innerness = np.maximum(innerness, np.clip((1.0 - d / max(pr, 1e-6)) / SHADE_RAMP, 0.0, 1.0) * cov)
        coverage = np.maximum(coverage, cov)

    rgba = _compose(coverage, innerness, spec.mottle, rng)
    return _downsample(rgba, (h_out, w_out))


SPLATS: tuple[SplatSpec, ...] = (
    SplatSpec(
        name="blood_splat_0",
        seed=0xB100D0,
        blobs=(
            BlobSpec(cx=-0.05, cy=0.03, r=0.58, wobble=0.26, fingers=3, finger_len=0.42),
            BlobSpec(cx=0.30, cy=-0.24, r=0.20, wobble=0.34, fingers=1, finger_len=0.35),
        ),
        speck_count=7,
        speck_arc=(-0.9, 1.6),
        tags=("pool",),
    ),
    SplatSpec(
        name="blood_splat_1",
        seed=0xB100D1,
        blobs=(
            BlobSpec(cx=0.06, cy=-0.04, r=0.50, wobble=0.32, fingers=4, finger_len=0.55),
            BlobSpec(cx=-0.32, cy=0.26, r=0.17, wobble=0.30, fingers=1, finger_len=0.40),
        ),
        speck_count=8,
        speck_arc=(1.4, 4.2),
        tags=("pool",),
    ),
    SplatSpec(
        name="blood_splat_2",
        seed=0xB100D2,
        blobs=(
            BlobSpec(cx=-0.18, cy=-0.10, r=0.34, wobble=0.30, fingers=2, finger_len=0.50),
            BlobSpec(cx=0.24, cy=0.18, r=0.26, wobble=0.34, fingers=2, finger_len=0.45),
            BlobSpec(cx=0.05, cy=0.40, r=0.13, wobble=0.36),
        ),
        speck_count=9,
        speck_ring=(0.40, 0.95),
        tags=("cluster",),
    ),
    SplatSpec(
        name="blood_splat_3",
        seed=0xB100D3,
        blobs=(
            BlobSpec(cx=-0.12, cy=0.08, r=0.44, wobble=0.36, fingers=3, finger_len=0.62),
            BlobSpec(cx=0.34, cy=-0.14, r=0.14, wobble=0.32),
        ),
        speck_count=6,
        speck_arc=(-1.8, 0.6),
        speck_r=(0.02, 0.07),
        tags=("smear",),
    ),
)

DROPS: tuple[DropSpec, ...] = (
    DropSpec(name="blood_drop_0", seed=0xB100E0, r=0.50, squash=0.78, tilt=0.0, micro=2),
    DropSpec(name="blood_drop_1", seed=0xB100E1, r=0.44, wobble=0.18, squash=0.66, tilt=0.35, micro=1),
    DropSpec(name="blood_drop_2", seed=0xB100E2, r=0.38, wobble=0.20, squash=0.90, tilt=-0.5, micro=3),
)

STREAKS: tuple[StreakSpec, ...] = (
    StreakSpec(name="blood_streak_0", seed=0xB1EED0, head_x=0.16, head_r=0.52, tail_x=0.68, bow=0.10, droplets=3),
    StreakSpec(name="blood_streak_1", seed=0xB1EED1, head_x=0.12, head_r=0.44, tail_x=0.60, bow=-0.18, droplets=2),
)


def main() -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    for spec in SPLATS:
        img = render_splat(spec)
        path = OUT_DIR / f"{spec.name}.png"
        img.save(path)
        print(f"wrote {path.relative_to(ROOT)} {img.size}")
    for spec in DROPS:
        img = render_drop(spec)
        path = OUT_DIR / f"{spec.name}.png"
        img.save(path)
        print(f"wrote {path.relative_to(ROOT)} {img.size}")
    for spec in STREAKS:
        img = render_streak(spec)
        path = OUT_DIR / f"{spec.name}.png"
        img.save(path)
        print(f"wrote {path.relative_to(ROOT)} {img.size}")


if __name__ == "__main__":
    main()
