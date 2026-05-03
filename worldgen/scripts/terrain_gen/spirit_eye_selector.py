from __future__ import annotations

import numpy as np


def select_spirit_eye_candidates(
    height: np.ndarray,
    qi_density: np.ndarray,
    feature_mask: np.ndarray,
    world_x: np.ndarray | None = None,
    world_z: np.ndarray | None = None,
    *,
    density_bias: float = 1.0,
    blood_valley: bool = False,
) -> np.ndarray:
    """Return a deterministic uint8 mask for spirit-eye candidate columns.

    灵眼候选点不是随机散点：高灵气、地形构型丰富、且高度在修士可驻足
    的区间时才进入候选。最终再用坐标 hash 稀疏化，避免同一片高分区域
    输出成整块面。
    """

    if height.shape != qi_density.shape or height.shape != feature_mask.shape:
        raise ValueError("height, qi_density and feature_mask must have the same shape")

    density_bias = max(float(density_bias), 0.1)
    qi_floor = 0.22 if blood_valley else 0.35
    score_threshold = 0.58 / min(density_bias, 2.5)

    surface_band = (height >= 80.0) & (height <= 200.0)
    cave_or_rift_like = feature_mask >= (0.72 if blood_valley else 0.82)
    eligible = (surface_band | cave_or_rift_like) & (qi_density >= qi_floor)
    score = qi_density * 0.68 + feature_mask * 0.32

    if world_x is None or world_z is None:
        grid_z, grid_x = np.indices(height.shape)
        world_x = grid_x
        world_z = grid_z

    sparse_gate = _coordinate_gate(world_x, world_z, stride=49 if blood_valley else 61)
    mask = eligible & (score >= score_threshold) & sparse_gate
    return mask.astype(np.uint8)


def _coordinate_gate(world_x: np.ndarray, world_z: np.ndarray, *, stride: int) -> np.ndarray:
    x = world_x.astype(np.int64, copy=False)
    z = world_z.astype(np.int64, copy=False)
    mixed = (x * 73_856_093) ^ (z * 19_349_663) ^ 0x5EED_1EAF
    return np.mod(np.abs(mixed), stride) == 0
