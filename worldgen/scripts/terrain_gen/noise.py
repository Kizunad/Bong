"""Terrain noise primitives.

Two families:
- Legacy sin/cos functions (must match Rust wilderness.rs exactly)
- Gradient noise (Perlin-style, numpy-vectorized) for zone profiles
"""

from __future__ import annotations

import math

import numpy as np


# ---------------------------------------------------------------------------
# Legacy sin/cos noise — Rust parity required
# ---------------------------------------------------------------------------

def coherent_noise_2d(
    world_x: float, world_z: float, scale: float = 512.0, seed: int = 0
) -> float:
    scaled_x = world_x / max(scale, 1.0)
    scaled_z = world_z / max(scale, 1.0)
    seed_phase = seed * 0.017
    return (
        math.sin(scaled_x * 1.17 + scaled_z * 0.83 + seed_phase) * 0.5
        + math.cos(scaled_x * -0.71 + scaled_z * 1.29 - seed_phase * 1.3) * 0.3
        + math.sin(scaled_x * 2.03 - scaled_z * 1.61 + seed_phase * 0.7) * 0.2
    )


# ---------------------------------------------------------------------------
# Gradient noise — pure numpy, works on arbitrary array shapes
# ---------------------------------------------------------------------------

_PERM_CACHE: dict[int, np.ndarray] = {}


def _get_perm(seed: int) -> np.ndarray:
    if seed not in _PERM_CACHE:
        rng = np.random.RandomState(seed & 0x7FFFFFFF)
        p = np.arange(256, dtype=np.int32)
        rng.shuffle(p)
        _PERM_CACHE[seed] = np.concatenate([p, p])
    return _PERM_CACHE[seed]


def _gradient_noise_2d(x: np.ndarray, z: np.ndarray, seed: int = 0) -> np.ndarray:
    """Perlin-style gradient noise. Input coords should be pre-scaled.
    Returns values in approximately [-1, 1].
    """
    perm = _get_perm(seed)

    xi = np.floor(x).astype(np.int32) & 255
    zi = np.floor(z).astype(np.int32) & 255
    xf = x - np.floor(x)
    zf = z - np.floor(z)

    # Improved Perlin fade
    u = xf * xf * xf * (xf * (xf * 6.0 - 15.0) + 10.0)
    v = zf * zf * zf * (zf * (zf * 6.0 - 15.0) + 10.0)

    # Permutation lookups
    aa = perm[perm[xi] + zi]
    ba = perm[perm[xi + 1] + zi]
    ab = perm[perm[xi] + zi + 1]
    bb = perm[perm[xi + 1] + zi + 1]

    # Gradient dot products (4 gradient directions)
    def _grad(h: np.ndarray, dx: np.ndarray, dz: np.ndarray) -> np.ndarray:
        g = h & 3
        gx = np.where(g & 1, -dx, dx)
        gz = np.where(g & 2, -dz, dz)
        return gx + gz

    n00 = _grad(aa, xf, zf)
    n10 = _grad(ba, xf - 1.0, zf)
    n01 = _grad(ab, xf, zf - 1.0)
    n11 = _grad(bb, xf - 1.0, zf - 1.0)

    # Bilinear interpolation
    nx0 = n00 + u * (n10 - n00)
    nx1 = n01 + u * (n11 - n01)
    return nx0 + v * (nx1 - nx0)


# ---------------------------------------------------------------------------
# High-level noise functions
# ---------------------------------------------------------------------------

def simplex_2d(
    x: np.ndarray, z: np.ndarray, scale: float = 256.0, seed: int = 0,
) -> np.ndarray:
    """Single octave gradient noise. Returns ~[-1, 1]."""
    return _gradient_noise_2d(x / max(scale, 1.0), z / max(scale, 1.0), seed)


def fbm_2d(
    x: np.ndarray,
    z: np.ndarray,
    scale: float = 256.0,
    octaves: int = 4,
    lacunarity: float = 2.0,
    gain: float = 0.5,
    seed: int = 0,
) -> np.ndarray:
    """Fractional Brownian Motion. Returns ~[-1, 1]."""
    result = np.zeros_like(x, dtype=np.float64)
    amplitude = 1.0
    frequency = 1.0
    total_amp = 0.0
    s = max(scale, 1.0)
    for i in range(octaves):
        result += amplitude * _gradient_noise_2d(
            x * frequency / s, z * frequency / s, seed + i * 31,
        )
        total_amp += amplitude
        amplitude *= gain
        frequency *= lacunarity
    return result / total_amp


def ridge_2d(
    x: np.ndarray,
    z: np.ndarray,
    scale: float = 256.0,
    octaves: int = 4,
    lacunarity: float = 2.0,
    gain: float = 0.5,
    seed: int = 0,
) -> np.ndarray:
    """Ridged noise — sharp peaks where FBM crosses zero. Returns ~[-1, 1]."""
    raw = fbm_2d(x, z, scale, octaves, lacunarity, gain, seed)
    return 1.0 - np.abs(raw) * 2.0


def warped_fbm_2d(
    x: np.ndarray,
    z: np.ndarray,
    scale: float = 256.0,
    octaves: int = 4,
    warp_scale: float = 400.0,
    warp_strength: float = 80.0,
    seed: int = 0,
) -> np.ndarray:
    """Domain-warped FBM for organic, non-periodic shapes. Returns ~[-1, 1]."""
    wx = fbm_2d(x, z, warp_scale, 3, seed=seed + 1000) * warp_strength
    wz = fbm_2d(x, z, warp_scale, 3, seed=seed + 2000) * warp_strength
    return fbm_2d(x + wx, z + wz, scale, octaves, seed=seed)


# ---------------------------------------------------------------------------
# 3D gradient noise — worldgen-v4 P3 §8.1 #1 carver sculpting
#
# The 2D family above shapes the *surface* height field; P3 carvers sculpt the
# *vertical* span column (overhang / floating-isle underside / arch void / cave
# layers), so they need a noise that varies with world Y as well as X/Z.  Using
# 2D noise re-sampled per height layer would lose vertical continuity — a carved
# wall would come out as disconnected slabs instead of a continuous overhang.
# These mirror the 2D signatures (pre-scaled gradient core + fbm wrapper) so a
# carver author reaches for ``fbm_3d`` exactly like ``fbm_2d``.
# ---------------------------------------------------------------------------


def _gradient_noise_3d(
    x: np.ndarray, y: np.ndarray, z: np.ndarray, seed: int = 0
) -> np.ndarray:
    """Perlin-style 3D gradient noise. Inputs should be pre-scaled.

    Returns values in approximately ``[-1, 1]``.  Deterministic for a given
    ``seed`` (the permutation table is seeded + cached), so two carve passes
    with the same seed produce byte-identical noise — the determinism the P3
    carvers rely on.
    """
    perm = _get_perm(seed)

    xi = np.floor(x).astype(np.int32) & 255
    yi = np.floor(y).astype(np.int32) & 255
    zi = np.floor(z).astype(np.int32) & 255
    xf = x - np.floor(x)
    yf = y - np.floor(y)
    zf = z - np.floor(z)

    # Improved Perlin fade on each axis.
    u = xf * xf * xf * (xf * (xf * 6.0 - 15.0) + 10.0)
    v = yf * yf * yf * (yf * (yf * 6.0 - 15.0) + 10.0)
    w = zf * zf * zf * (zf * (zf * 6.0 - 15.0) + 10.0)

    # Hash the 8 cube corners through the doubled permutation table.
    def _hash(ox: int, oy: int, oz: int) -> np.ndarray:
        return perm[perm[perm[(xi + ox) & 255] + ((yi + oy) & 255)] + ((zi + oz) & 255)]

    # 12-direction gradient set (classic Perlin) reduced to a dot product.
    def _grad(h: np.ndarray, dx: np.ndarray, dy: np.ndarray, dz: np.ndarray) -> np.ndarray:
        g = h & 15
        gu = np.where(g < 8, dx, dy)
        gv = np.where(g < 4, dy, np.where((g == 12) | (g == 14), dx, dz))
        su = np.where(g & 1, -gu, gu)
        sv = np.where(g & 2, -gv, gv)
        return su + sv

    n000 = _grad(_hash(0, 0, 0), xf, yf, zf)
    n100 = _grad(_hash(1, 0, 0), xf - 1.0, yf, zf)
    n010 = _grad(_hash(0, 1, 0), xf, yf - 1.0, zf)
    n110 = _grad(_hash(1, 1, 0), xf - 1.0, yf - 1.0, zf)
    n001 = _grad(_hash(0, 0, 1), xf, yf, zf - 1.0)
    n101 = _grad(_hash(1, 0, 1), xf - 1.0, yf, zf - 1.0)
    n011 = _grad(_hash(0, 1, 1), xf, yf - 1.0, zf - 1.0)
    n111 = _grad(_hash(1, 1, 1), xf - 1.0, yf - 1.0, zf - 1.0)

    nx00 = n000 + u * (n100 - n000)
    nx10 = n010 + u * (n110 - n010)
    nx01 = n001 + u * (n101 - n001)
    nx11 = n011 + u * (n111 - n011)
    nxy0 = nx00 + v * (nx10 - nx00)
    nxy1 = nx01 + v * (nx11 - nx01)
    return nxy0 + w * (nxy1 - nxy0)


def gradient_noise_3d(
    x: np.ndarray,
    y: np.ndarray,
    z: np.ndarray,
    scale: float = 64.0,
    seed: int = 0,
) -> np.ndarray:
    """Single octave 3D gradient noise. Returns ~[-1, 1]."""
    s = max(scale, 1.0)
    return _gradient_noise_3d(
        np.asarray(x, dtype=np.float64) / s,
        np.asarray(y, dtype=np.float64) / s,
        np.asarray(z, dtype=np.float64) / s,
        seed,
    )


def fbm_3d(
    x: np.ndarray,
    y: np.ndarray,
    z: np.ndarray,
    scale: float = 64.0,
    octaves: int = 4,
    lacunarity: float = 2.0,
    gain: float = 0.5,
    seed: int = 0,
) -> np.ndarray:
    """3D Fractional Brownian Motion. Returns ~[-1, 1]."""
    xa = np.asarray(x, dtype=np.float64)
    ya = np.asarray(y, dtype=np.float64)
    za = np.asarray(z, dtype=np.float64)
    result = np.zeros(np.broadcast(xa, ya, za).shape, dtype=np.float64)
    amplitude = 1.0
    frequency = 1.0
    total_amp = 0.0
    s = max(scale, 1.0)
    for i in range(octaves):
        result = result + amplitude * _gradient_noise_3d(
            xa * frequency / s,
            ya * frequency / s,
            za * frequency / s,
            seed + i * 31,
        )
        total_amp += amplitude
        amplitude *= gain
        frequency *= lacunarity
    return result / total_amp


def warped_fbm_3d(
    x: np.ndarray,
    y: np.ndarray,
    z: np.ndarray,
    scale: float = 64.0,
    octaves: int = 4,
    warp_scale: float = 120.0,
    warp_strength: float = 24.0,
    seed: int = 0,
) -> np.ndarray:
    """Domain-warped 3D FBM for organic caverns / overhang lips. Returns ~[-1, 1]."""
    wx = fbm_3d(x, y, z, warp_scale, 3, seed=seed + 1000) * warp_strength
    wy = fbm_3d(x, y, z, warp_scale, 3, seed=seed + 2000) * warp_strength
    wz = fbm_3d(x, y, z, warp_scale, 3, seed=seed + 3000) * warp_strength
    return fbm_3d(
        np.asarray(x, dtype=np.float64) + wx,
        np.asarray(y, dtype=np.float64) + wy,
        np.asarray(z, dtype=np.float64) + wz,
        scale,
        octaves,
        seed=seed,
    )


# ---------------------------------------------------------------------------
# Coordinate helpers
# ---------------------------------------------------------------------------

def _tile_coords(
    tile_min_x: int, tile_min_z: int, tile_size: int,
) -> tuple[np.ndarray, np.ndarray]:
    """Build world-coordinate meshgrid for a tile. Returns (wx, wz) float64."""
    lx = np.arange(tile_size, dtype=np.float64)
    lz = np.arange(tile_size, dtype=np.float64)
    lx_grid, lz_grid = np.meshgrid(lx, lz)
    return tile_min_x + lx_grid, tile_min_z + lz_grid
