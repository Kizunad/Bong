"""raster_check 灵气语义 pin 测试 —— 锁住同源断言开关与 qi_density 值域契约.

背景（2026-07-06 bot playtest 排查定案）：
- qi_density 层现状由各 terrain profile 手搓，尚未从统一场烘焙；manifest 必须
  如实声明 ``qi_density_source == "profile"``，同源断言（P4 §8.1 #8）随之休眠。
  历史 bug：raster_export 曾写死 "qi_field" 假声明 → 全图 345 处必炸误报。
- qi_density 值域为 [-1, 1]，负值是负灵域正典（wangyintai [-0.25,0]）；
  mofa_decay / qi_vein_flow 保持 [0, 1]。

真同源化迁移（届时导出端置回 "qi_field"）见 plan-qi-density-same-source-v1。
"""

from __future__ import annotations

import json
import struct
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from scripts.terrain_gen.harness.raster_check import validate_rasters  # noqa: E402

TILE_SIZE = 4
AREA = TILE_SIZE * TILE_SIZE


def _write_float_layer(path: Path, values: list[float]) -> None:
    assert len(values) == AREA, f"layer needs {AREA} cells, got {len(values)}"
    path.write_bytes(struct.pack(f"<{AREA}f", *values))


def _make_raster_dir(
    tmp_path: Path,
    *,
    qi_density_source: str,
    qi_values: list[float],
    zone_spirit_qi: float = 0.3,
    extra_layers: dict[str, list[float]] | None = None,
) -> Path:
    """合成最小可校验 raster：单 tile 单 zone，只带声明的语义层."""
    raster_dir = tmp_path / "rasters"
    tile_dir = raster_dir / "tile_0_0"
    tile_dir.mkdir(parents=True)

    layers = {"qi_density": qi_values, **(extra_layers or {})}
    for name, values in layers.items():
        _write_float_layer(tile_dir / f"{name}.bin", values)

    manifest = {
        "version": 2,
        "backend": "raster",
        "tile_size": TILE_SIZE,
        "qi_density_source": qi_density_source,
        "tiles": [
            {
                "dir": "tile_0_0",
                "tile_x": 0,
                "tile_z": 0,
                "zones": ["testzone"],
                "layers": sorted(layers),
            }
        ],
        "zones": [
            {
                "name": "testzone",
                "spirit_qi": zone_spirit_qi,
                # 覆盖整个 tile（tile 原点即世界原点，tile_size=4）
                "bounds_xz": {"min_x": 0, "max_x": 4, "min_z": 0, "max_z": 4},
            }
        ],
    }
    (raster_dir / "manifest.json").write_text(
        json.dumps(manifest), encoding="utf-8"
    )
    return raster_dir


# ---------------------------------------------------------------------------
# 同源断言开关：source == "profile" 休眠 / == "qi_field" 激活
# ---------------------------------------------------------------------------


def test_same_source_assert_dormant_for_profile_source(tmp_path: Path) -> None:
    """source=='profile'（手搓现状）时，均值与 spirit_qi 期望严重偏离也不报错.

    zone spirit_qi=0.3 → 同源期望均值 0.65；实测全 0.1（|Δ|=0.55 >> tol=0.2）。
    断言休眠是设计行为（"profile 迁移到统一场后由导出端置位"），误报即回归。
    """
    raster_dir = _make_raster_dir(
        tmp_path, qi_density_source="profile", qi_values=[0.1] * AREA
    )
    ok, msg = validate_rasters(raster_dir)
    assert ok, (
        "期望 source=='profile' 时同源断言休眠（qi_density 是 profile 手搓，"
        f"与 spirit_qi 本就不同源，报错=假声明时代的误报复发），实际: {msg}"
    )


def test_same_source_assert_active_for_qi_field_source(tmp_path: Path) -> None:
    """source=='qi_field'（迁移完成后）时，同样的偏离必须撞硬错误."""
    raster_dir = _make_raster_dir(
        tmp_path, qi_density_source="qi_field", qi_values=[0.1] * AREA
    )
    ok, msg = validate_rasters(raster_dir)
    assert not ok and "diverges" in msg, (
        "期望 source=='qi_field' 时同源断言激活并抓住 |0.1-0.65|>0.2 的漂移"
        f"（两份漂移防线），实际 ok={ok}: {msg}"
    )


def test_same_source_assert_passes_when_actually_same_source(
    tmp_path: Path,
) -> None:
    """source=='qi_field' 且数据真同源（均值≈clamp01((sq+1)/2)）时必须全绿."""
    raster_dir = _make_raster_dir(
        tmp_path,
        qi_density_source="qi_field",
        qi_values=[0.65] * AREA,  # == clamp01((0.3+1)/2)
    )
    ok, msg = validate_rasters(raster_dir)
    assert ok, f"期望同源数据通过激活态断言，实际: {msg}"


# ---------------------------------------------------------------------------
# qi_density 值域 [-1, 1]：负灵域合法，越下界报错；mofa_decay 仍 [0,1]
# ---------------------------------------------------------------------------


def test_qi_density_negative_within_domain_ok(tmp_path: Path) -> None:
    """负灵域（wangyintai 型 [-0.25,0]）qi_density 合法，不得报 range 错."""
    raster_dir = _make_raster_dir(
        tmp_path,
        qi_density_source="profile",
        qi_values=[-0.25, -0.1, 0.0, 0.5] + [0.0] * (AREA - 4),
    )
    ok, msg = validate_rasters(raster_dir)
    assert ok, (
        "期望负灵域 qi_density∈[-1,1] 通过 range 检查（负值是 wangyintai "
        f"涡流宗正典设计，[0,1] 下界是历史误设），实际: {msg}"
    )


def test_qi_density_below_negative_one_fails(tmp_path: Path) -> None:
    """qi_density < -1 越出统一场域，必须报 range 错误."""
    raster_dir = _make_raster_dir(
        tmp_path,
        qi_density_source="profile",
        qi_values=[-1.5] + [0.0] * (AREA - 1),
    )
    ok, msg = validate_rasters(raster_dir)
    assert not ok and "qi_density range" in msg, (
        f"期望 qi_density=-1.5 越 [-1,1] 下界撞 range 错误，实际 ok={ok}: {msg}"
    )


def test_qi_density_above_one_still_fails(tmp_path: Path) -> None:
    """上界 1 不变：qi_density > 1 仍报错（放宽只针对下界）."""
    raster_dir = _make_raster_dir(
        tmp_path,
        qi_density_source="profile",
        qi_values=[1.5] + [0.0] * (AREA - 1),
    )
    ok, msg = validate_rasters(raster_dir)
    assert not ok and "qi_density range" in msg, (
        f"期望 qi_density=1.5 越上界仍报错，实际 ok={ok}: {msg}"
    )


@pytest.mark.parametrize("layer", ["mofa_decay", "qi_vein_flow"])
def test_other_semantic_layers_keep_zero_lower_bound(
    tmp_path: Path, layer: str
) -> None:
    """mofa_decay / qi_vein_flow 值域仍 [0,1]：负值必须报错（放宽仅限 qi_density）."""
    raster_dir = _make_raster_dir(
        tmp_path,
        qi_density_source="profile",
        qi_values=[0.1] * AREA,
        extra_layers={layer: [-0.2] + [0.0] * (AREA - 1)},
    )
    ok, msg = validate_rasters(raster_dir)
    assert not ok and f"{layer} range" in msg, (
        f"期望 {layer}=-0.2 报 range 错误（[0,1] 契约不随 qi_density 放宽），"
        f"实际 ok={ok}: {msg}"
    )
