#!/usr/bin/env python3
"""validate_snapshots.py — Snapshot 内容验收闸（plan-worldgen-snapshot-v1 §4.1）。

CI 不再只看"截图文件存在"就 pass，必须实地核验图像内容。截图全空 → CI 红。

反例 fixture：PR #71 artifact run 25051736013 — 5 张截图 3 张 100% sky / 1 张 71%
void / top 19.8% terrain；preview-iso_nw.png 与 preview-iso_sw.png byte-identical
（同纯 sky color PNG 编码后字节相同）。本 validator 必须把这种 artifact 标红。

色彩分类（每像素 RGB）:
  - void    : R<20 且 G<20 且 B<20（未加载 chunk 黑块）
  - sky     : B>150 且 B>R+20 且 B>G+5
  - cloud   : R>200 且 G>200 且 B>200
  - terrain : 以上之外（地形主色）

验收规则:
  R1 内容下限   每张截图 terrain% ≥ 默认 30%；top 视角放宽到 ≥ 15%（俯视纯天空更普遍）
  R2 hash 唯一  N 张 PNG 两两 MD5 不同（防同帧复用 / 同纯色 PNG）
  R3 大小 sanity 每张 ≥ 默认 30KB（纯单色 PNG 编码后 ≈ 16KB，做地板）

用法:
  python3 scripts/preview/validate_snapshots.py --client-dir client/run/screenshots
  python3 scripts/preview/validate_snapshots.py --client-dir <dir> --terrain-min 0.30 --top-terrain-min 0.15

任一规则失败 → exit 1，并把每张图的 name | size | md5 | sky% | void% | terrain%
六栏对照打到 stdout（让排错有线索；CI 也会上传 artifact 给人眼看）。
"""

from __future__ import annotations

import argparse
import hashlib
import sys
from dataclasses import dataclass
from pathlib import Path

import numpy as np
from PIL import Image


DEFAULT_TERRAIN_MIN = 0.30
DEFAULT_TOP_TERRAIN_MIN = 0.15
DEFAULT_MIN_SIZE_BYTES = 30 * 1024
TOP_NAME_HINTS = ("top",)


@dataclass(frozen=True)
class ColorFractions:
    void: float
    sky: float
    cloud: float
    terrain: float


@dataclass(frozen=True)
class SnapshotReport:
    path: Path
    size_bytes: int
    md5: str
    fractions: ColorFractions

    @property
    def name(self) -> str:
        return self.path.name


def classify_pixels(rgb: np.ndarray) -> ColorFractions:
    """rgb shape (H, W, 3) uint8."""
    r = rgb[..., 0]
    g = rgb[..., 1]
    b = rgb[..., 2]

    void_mask = (r < 20) & (g < 20) & (b < 20)
    sky_mask = (~void_mask) & (b > 150) & (b > r + 20) & (b > g + 5)
    cloud_mask = (~void_mask) & (~sky_mask) & (r > 200) & (g > 200) & (b > 200)
    terrain_mask = ~(void_mask | sky_mask | cloud_mask)

    total = r.size
    return ColorFractions(
        void=float(void_mask.sum()) / total,
        sky=float(sky_mask.sum()) / total,
        cloud=float(cloud_mask.sum()) / total,
        terrain=float(terrain_mask.sum()) / total,
    )


def md5_file(path: Path) -> str:
    h = hashlib.md5()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(64 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def load_report(path: Path) -> SnapshotReport:
    img = Image.open(path).convert("RGB")
    rgb = np.asarray(img, dtype=np.uint8)
    fractions = classify_pixels(rgb)
    return SnapshotReport(
        path=path,
        size_bytes=path.stat().st_size,
        md5=md5_file(path),
        fractions=fractions,
    )


def is_top_view(name: str) -> bool:
    stem = Path(name).stem.lower()
    for hint in TOP_NAME_HINTS:
        if stem == f"preview-{hint}" or stem == hint or stem.endswith(f"-{hint}"):
            return True
    return False


def check_rules(
    reports: list[SnapshotReport],
    terrain_min: float,
    top_terrain_min: float,
    min_size_bytes: int,
) -> list[str]:
    failures: list[str] = []

    # R1: terrain content floor (top relaxed)
    for rep in reports:
        threshold = top_terrain_min if is_top_view(rep.name) else terrain_min
        if rep.fractions.terrain < threshold:
            failures.append(
                f"R1 terrain< {threshold:.0%}: {rep.name} "
                f"terrain={rep.fractions.terrain:.1%} "
                f"(sky={rep.fractions.sky:.1%} void={rep.fractions.void:.1%}) "
                f"— 期望 chunk 已加载且能拍到地形，实际几乎全空"
            )

    # R2: md5 uniqueness
    by_hash: dict[str, list[str]] = {}
    for rep in reports:
        by_hash.setdefault(rep.md5, []).append(rep.name)
    for digest, names in by_hash.items():
        if len(names) > 1:
            failures.append(
                f"R2 md5 重复: {digest[:8]}... 命中 {len(names)} 张 [{', '.join(names)}] "
                f"— 期望每张内容不同，实际同帧复用（chunk 未加载时纯 sky color 编码后字节相同）"
            )

    # R3: size sanity
    for rep in reports:
        if rep.size_bytes < min_size_bytes:
            failures.append(
                f"R3 size< {min_size_bytes}B: {rep.name} {rep.size_bytes}B "
                f"— 期望 PNG 至少 {min_size_bytes // 1024}KB，实际接近纯单色编码大小"
            )

    return failures


def format_table(reports: list[SnapshotReport]) -> str:
    header = f"{'name':<24} {'size':>8} {'md5':<10} {'sky%':>6} {'void%':>6} {'terrain%':>9}"
    rows = [header, "-" * len(header)]
    for rep in sorted(reports, key=lambda r: r.name):
        rows.append(
            f"{rep.name:<24} {rep.size_bytes:>8} {rep.md5[:8]:<10} "
            f"{rep.fractions.sky * 100:>5.1f}% "
            f"{rep.fractions.void * 100:>5.1f}% "
            f"{rep.fractions.terrain * 100:>8.1f}%"
        )
    return "\n".join(rows)


def collect_snapshots(client_dir: Path, pattern: str) -> list[Path]:
    if not client_dir.exists():
        raise FileNotFoundError(f"client-dir 不存在: {client_dir}")
    if not client_dir.is_dir():
        raise NotADirectoryError(f"client-dir 不是目录: {client_dir}")
    return sorted(client_dir.glob(pattern))


def validate(
    client_dir: Path,
    *,
    pattern: str = "preview-*.png",
    terrain_min: float = DEFAULT_TERRAIN_MIN,
    top_terrain_min: float = DEFAULT_TOP_TERRAIN_MIN,
    min_size_bytes: int = DEFAULT_MIN_SIZE_BYTES,
    require_min_count: int = 1,
) -> tuple[list[SnapshotReport], list[str]]:
    """返回 (reports, failures)。failures 为空 = 通过。"""
    paths = collect_snapshots(client_dir, pattern)
    if len(paths) < require_min_count:
        return [], [
            f"R0 至少需要 {require_min_count} 张匹配 '{pattern}' 的截图，"
            f"实际找到 {len(paths)} — client preview harness 没拍照？看 server / xvfb 日志"
        ]
    reports = [load_report(p) for p in paths]
    failures = check_rules(
        reports,
        terrain_min=terrain_min,
        top_terrain_min=top_terrain_min,
        min_size_bytes=min_size_bytes,
    )
    return reports, failures


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Validate Bong worldgen preview screenshots.")
    parser.add_argument(
        "--client-dir",
        required=True,
        type=Path,
        help="目录，含 preview-*.png（通常 client/run/screenshots）",
    )
    parser.add_argument(
        "--pattern",
        default="preview-*.png",
        help="glob pattern（默认 preview-*.png；compose_grid 拼图本身排除）",
    )
    parser.add_argument(
        "--terrain-min",
        type=float,
        default=DEFAULT_TERRAIN_MIN,
        help=f"非俯视角 terrain 像素占比下限（默认 {DEFAULT_TERRAIN_MIN}）",
    )
    parser.add_argument(
        "--top-terrain-min",
        type=float,
        default=DEFAULT_TOP_TERRAIN_MIN,
        help=f"俯视角（preview-top）terrain 像素占比下限（默认 {DEFAULT_TOP_TERRAIN_MIN}）",
    )
    parser.add_argument(
        "--min-size-bytes",
        type=int,
        default=DEFAULT_MIN_SIZE_BYTES,
        help=f"PNG 最小字节数（默认 {DEFAULT_MIN_SIZE_BYTES}）",
    )
    parser.add_argument(
        "--require-min-count",
        type=int,
        default=1,
        help="至少需要的截图数量（默认 1，CI 应设 5）",
    )
    parser.add_argument(
        "--exclude",
        action="append",
        default=["preview-grid.png"],
        help="排除的文件名（可重复；默认排 preview-grid.png 拼图）",
    )
    args = parser.parse_args(argv)

    try:
        paths = collect_snapshots(args.client_dir, args.pattern)
    except (FileNotFoundError, NotADirectoryError) as e:
        print(f"[validate] FAIL: {e}", file=sys.stderr)
        return 2

    excluded = set(args.exclude or [])
    paths = [p for p in paths if p.name not in excluded]

    if len(paths) < args.require_min_count:
        print(
            f"[validate] FAIL: R0 至少需要 {args.require_min_count} 张匹配 "
            f"'{args.pattern}' 的截图（排除 {sorted(excluded)} 后），实际 {len(paths)}",
            file=sys.stderr,
        )
        return 1

    reports = [load_report(p) for p in paths]
    failures = check_rules(
        reports,
        terrain_min=args.terrain_min,
        top_terrain_min=args.top_terrain_min,
        min_size_bytes=args.min_size_bytes,
    )

    print(format_table(reports))
    print()

    if failures:
        print(f"[validate] FAIL: {len(failures)} 条规则未过 —", file=sys.stderr)
        for line in failures:
            print(f"  - {line}", file=sys.stderr)
        return 1

    print(f"[validate] PASS: {len(reports)} 张截图全部通过 R1/R2/R3")
    return 0


if __name__ == "__main__":
    sys.exit(main())
