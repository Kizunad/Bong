"""饱和测试 validate_snapshots —— plan-worldgen-snapshot-v1 §4.3。

跑法（在 scripts/preview/ 内）：
    python3 -m unittest test_validate_snapshots
或在仓库根：
    python3 -m unittest discover -s scripts/preview -p 'test_*.py'

需 numpy + pillow 已装（CI 已 pip install）。

覆盖矩阵：
  ① classify_pixels：纯黑 / 纯天空 / 纯白云 / 纯地形 / 混色 / 边界值（B=150 不算 sky；
     R=20 不算 void）
  ② check_rules：R1 各阈值（top vs 非 top 区分） / R2 hash 重复 / R3 size 不足 /
     全过 / 多规则同时失败
  ③ collect_snapshots：目录不存在 / 不是目录 / 空目录 / 含非 PNG / pattern 命中
  ④ is_top_view：preview-top.png / top / preview-top / 其他名（iso_ne）
  ⑤ main CLI：happy path 0 / FAIL R1 1 / 目录不存在 2 / 排除 preview-grid.png
"""

from __future__ import annotations

import io
import sys
import unittest
from contextlib import redirect_stdout, redirect_stderr
from pathlib import Path
from tempfile import TemporaryDirectory

import numpy as np
from PIL import Image

# 让 import 不依赖 scripts/__init__.py（仓库历史 scripts/ 是裸脚本目录）
sys.path.insert(0, str(Path(__file__).resolve().parent))

import validate_snapshots as vs  # noqa: E402


def write_solid_png(path: Path, rgb: tuple[int, int, int], size: tuple[int, int] = (32, 32)) -> None:
    img = Image.new("RGB", size, rgb)
    img.save(path, "PNG")


def write_mixed_png(
    path: Path,
    rgb_grid: list[tuple[int, int, int]],
    cells: tuple[int, int] = (4, 4),
    cell_size: int = 16,
) -> None:
    """rgb_grid: cells[0]*cells[1] 个颜色按行铺。"""
    cols, rows = cells
    assert len(rgb_grid) == cols * rows, "rgb_grid 数量必须 = cols*rows"
    w, h = cols * cell_size, rows * cell_size
    img = Image.new("RGB", (w, h))
    arr = np.asarray(img).copy()
    for idx, color in enumerate(rgb_grid):
        cy, cx = divmod(idx, cols)
        arr[cy * cell_size : (cy + 1) * cell_size, cx * cell_size : (cx + 1) * cell_size] = color
    Image.fromarray(arr).save(path, "PNG")


def pad_png_to_min_size(
    path: Path,
    target_bytes: int,
    salt: int = 0,
) -> None:
    """把 PNG 改成大尺寸 + 注入少量噪声直到 size >= target_bytes。

    保留绝大多数像素的色彩分类（仅用 1% 像素作为噪声盐让 PNG 不被极致压缩
    成全单色 deflate stream），并通过 salt 让多张图产生不同字节序列以满足 R2。
    """
    side = 96
    while True:
        img = Image.open(path).convert("RGB")
        arr = np.asarray(img, dtype=np.uint8)
        h, w = arr.shape[:2]
        # tile 原图到 side*side
        tile_y = (side + h - 1) // h
        tile_x = (side + w - 1) // w
        tiled = np.tile(arr, (tile_y, tile_x, 1))[:side, :side]
        # 用 salt 控制噪声 RNG 让每张图字节不同
        rng = np.random.RandomState(salt)
        noise_pct = 0.01
        n_noise = max(1, int(side * side * noise_pct))
        ys = rng.randint(0, side, n_noise)
        xs = rng.randint(0, side, n_noise)
        # 把噪声像素涂成中等灰，不影响 classify 主体（仅 1% 像素，最多±1pp）
        tiled[ys, xs] = (180, 180, 180)
        Image.fromarray(tiled).save(path, "PNG")
        if path.stat().st_size >= target_bytes:
            return
        if side >= 2048:
            return
        side *= 2


class ClassifyPixelsTests(unittest.TestCase):
    def test_pure_void(self) -> None:
        rgb = np.zeros((10, 10, 3), dtype=np.uint8)
        f = vs.classify_pixels(rgb)
        self.assertAlmostEqual(f.void, 1.0)
        self.assertAlmostEqual(f.sky + f.cloud + f.terrain, 0.0)

    def test_pure_sky(self) -> None:
        rgb = np.zeros((10, 10, 3), dtype=np.uint8)
        rgb[..., :] = (120, 167, 255)  # MC 正午天空近似
        f = vs.classify_pixels(rgb)
        self.assertAlmostEqual(f.sky, 1.0)
        self.assertAlmostEqual(f.terrain, 0.0)

    def test_pure_cloud(self) -> None:
        rgb = np.full((10, 10, 3), 230, dtype=np.uint8)
        f = vs.classify_pixels(rgb)
        self.assertAlmostEqual(f.cloud, 1.0)
        self.assertAlmostEqual(f.terrain, 0.0)
        self.assertAlmostEqual(f.sky, 0.0)

    def test_pure_terrain_grass(self) -> None:
        # MC 草地绿色，G 大但 B 不大于 R+20
        rgb = np.zeros((10, 10, 3), dtype=np.uint8)
        rgb[..., :] = (90, 130, 60)
        f = vs.classify_pixels(rgb)
        self.assertAlmostEqual(f.terrain, 1.0)

    def test_void_boundary(self) -> None:
        # R=G=B=20 不算 void（< 20 才算）
        rgb = np.full((4, 4, 3), 20, dtype=np.uint8)
        f = vs.classify_pixels(rgb)
        self.assertAlmostEqual(f.void, 0.0)

    def test_sky_boundary(self) -> None:
        # B=150 边界：要求 B>150，所以 150 不算 sky
        rgb = np.zeros((4, 4, 3), dtype=np.uint8)
        rgb[..., :] = (50, 100, 150)
        f = vs.classify_pixels(rgb)
        self.assertAlmostEqual(f.sky, 0.0)

    def test_mixed_proportions(self) -> None:
        # 一半 void 一半 sky
        rgb = np.zeros((4, 4, 3), dtype=np.uint8)
        rgb[:2, :, :] = 0
        rgb[2:, :, :] = (120, 167, 255)
        f = vs.classify_pixels(rgb)
        self.assertAlmostEqual(f.void, 0.5)
        self.assertAlmostEqual(f.sky, 0.5)


class IsTopViewTests(unittest.TestCase):
    def test_preview_top(self) -> None:
        self.assertTrue(vs.is_top_view("preview-top.png"))

    def test_top_alone(self) -> None:
        self.assertTrue(vs.is_top_view("top.png"))

    def test_iso_not_top(self) -> None:
        self.assertFalse(vs.is_top_view("preview-iso_ne.png"))

    def test_other_suffix_top(self) -> None:
        self.assertTrue(vs.is_top_view("zone-top.png"))

    def test_arbitrary(self) -> None:
        self.assertFalse(vs.is_top_view("preview-grid.png"))


class CollectSnapshotsTests(unittest.TestCase):
    def test_missing_dir(self) -> None:
        with self.assertRaises(FileNotFoundError):
            vs.collect_snapshots(Path("/no/such/dir/123"), "preview-*.png")

    def test_not_a_dir(self) -> None:
        with TemporaryDirectory() as tmp:
            f = Path(tmp) / "afile"
            f.write_bytes(b"x")
            with self.assertRaises(NotADirectoryError):
                vs.collect_snapshots(f, "preview-*.png")

    def test_empty_dir(self) -> None:
        with TemporaryDirectory() as tmp:
            paths = vs.collect_snapshots(Path(tmp), "preview-*.png")
            self.assertEqual(paths, [])

    def test_pattern_filters(self) -> None:
        with TemporaryDirectory() as tmp:
            tmp_p = Path(tmp)
            (tmp_p / "preview-top.png").write_bytes(b"\x89PNG\r\n\x1a\n")
            (tmp_p / "other.png").write_bytes(b"\x89PNG\r\n\x1a\n")
            (tmp_p / "preview-foo.txt").write_text("not png")
            paths = vs.collect_snapshots(tmp_p, "preview-*.png")
            self.assertEqual([p.name for p in paths], ["preview-top.png"])


class CheckRulesTests(unittest.TestCase):
    def _report(self, *, name: str, size: int, md5: str, terrain: float, sky: float = 0.0, void: float = 0.0, cloud: float = 0.0) -> vs.SnapshotReport:
        return vs.SnapshotReport(
            path=Path(name),
            size_bytes=size,
            md5=md5,
            fractions=vs.ColorFractions(void=void, sky=sky, cloud=cloud, terrain=terrain),
        )

    def test_all_pass(self) -> None:
        reports = [
            self._report(name="preview-top.png", size=40_000, md5="a" * 32, terrain=0.20, sky=0.80),
            self._report(name="preview-iso_ne.png", size=40_000, md5="b" * 32, terrain=0.40, sky=0.60),
        ]
        failures = vs.check_rules(reports, terrain_min=0.30, top_terrain_min=0.15, min_size_bytes=30_000)
        self.assertEqual(failures, [])

    def test_r1_non_top_below_threshold(self) -> None:
        reports = [self._report(name="preview-iso_ne.png", size=40_000, md5="a" * 32, terrain=0.10, sky=0.90)]
        failures = vs.check_rules(reports, terrain_min=0.30, top_terrain_min=0.15, min_size_bytes=30_000)
        self.assertEqual(len(failures), 1)
        self.assertIn("R1 terrain<", failures[0])
        self.assertIn("preview-iso_ne.png", failures[0])

    def test_r1_top_uses_relaxed_threshold(self) -> None:
        # terrain=0.20 < non-top 阈值 0.30 但 > top 阈值 0.15 → 应放过
        reports = [self._report(name="preview-top.png", size=40_000, md5="a" * 32, terrain=0.20, sky=0.80)]
        failures = vs.check_rules(reports, terrain_min=0.30, top_terrain_min=0.15, min_size_bytes=30_000)
        self.assertEqual(failures, [])

    def test_r1_top_below_relaxed_threshold(self) -> None:
        reports = [self._report(name="preview-top.png", size=40_000, md5="a" * 32, terrain=0.10, sky=0.90)]
        failures = vs.check_rules(reports, terrain_min=0.30, top_terrain_min=0.15, min_size_bytes=30_000)
        self.assertEqual(len(failures), 1)
        self.assertIn("preview-top.png", failures[0])

    def test_r2_duplicate_md5(self) -> None:
        same = "deadbeef" * 4
        reports = [
            self._report(name="preview-iso_nw.png", size=40_000, md5=same, terrain=0.50),
            self._report(name="preview-iso_sw.png", size=40_000, md5=same, terrain=0.50),
        ]
        failures = vs.check_rules(reports, terrain_min=0.30, top_terrain_min=0.15, min_size_bytes=30_000)
        self.assertEqual(len(failures), 1)
        self.assertIn("R2 md5 重复", failures[0])
        self.assertIn("preview-iso_nw.png", failures[0])
        self.assertIn("preview-iso_sw.png", failures[0])

    def test_r2_three_way_duplicate(self) -> None:
        same = "cafebabe" * 4
        reports = [self._report(name=f"preview-{n}.png", size=40_000, md5=same, terrain=0.50) for n in ("a", "b", "c")]
        failures = vs.check_rules(reports, terrain_min=0.30, top_terrain_min=0.15, min_size_bytes=30_000)
        self.assertEqual(len(failures), 1)
        self.assertIn("命中 3 张", failures[0])

    def test_r3_undersized(self) -> None:
        reports = [self._report(name="preview-iso_ne.png", size=15_000, md5="a" * 32, terrain=0.50)]
        failures = vs.check_rules(reports, terrain_min=0.30, top_terrain_min=0.15, min_size_bytes=30_000)
        self.assertEqual(len(failures), 1)
        self.assertIn("R3 size<", failures[0])

    def test_multi_rule_failure_pr71_fixture(self) -> None:
        # 复刻 PR #71 artifact run 25051736013 反例：
        # iso_nw == iso_sw byte-identical（R2）+ iso_ne 100% sky（R1）+ iso_se 71% void（R1）
        same_md5 = "e44b9c01" + "0" * 24
        reports = [
            self._report(name="preview-top.png", size=170_000, md5="a" * 32, terrain=0.198, sky=0.802),
            self._report(name="preview-iso_ne.png", size=26_065, md5="b" * 32, terrain=0.0, sky=0.994, cloud=0.006),
            self._report(name="preview-iso_nw.png", size=16_945, md5=same_md5, terrain=0.0, sky=1.0),
            self._report(name="preview-iso_se.png", size=63_534, md5="c" * 32, terrain=0.155, sky=0.134, void=0.71),
            self._report(name="preview-iso_sw.png", size=16_945, md5=same_md5, terrain=0.0, sky=1.0),
        ]
        failures = vs.check_rules(reports, terrain_min=0.30, top_terrain_min=0.15, min_size_bytes=30_000)
        # 预期：iso_ne R1 + iso_nw R1 + iso_se R1 + iso_sw R1 + iso_nw/iso_sw R2 dup + iso_nw R3 + iso_sw R3
        # top 通过（terrain 0.198 ≥ top 阈值 0.15）+ iso_ne size OK + iso_se size OK
        joined = "\n".join(failures)
        self.assertIn("preview-iso_ne.png", joined)
        self.assertIn("preview-iso_se.png", joined)
        self.assertIn("R2 md5 重复", joined)
        # top 应通过 R1（0.198 ≥ 0.15 top 阈值）
        self.assertNotIn("R1 terrain< 15%: preview-top.png", joined)


class ValidateIntegrationTests(unittest.TestCase):
    """端到端：写真实 PNG → load_report → check_rules → table 输出。"""

    def test_full_pipeline_pass(self) -> None:
        with TemporaryDirectory() as tmp:
            tmp_p = Path(tmp)
            grass = (90, 130, 60)
            sky = (120, 167, 255)
            inputs = [
                ("preview-top.png", [grass] * 12 + [sky] * 4),
                ("preview-iso_ne.png", [grass] * 11 + [sky] * 5),
                ("preview-iso_nw.png", [grass] * 10 + [sky] * 6),
                ("preview-iso_se.png", [grass] * 14 + [sky] * 2),
                ("preview-iso_sw.png", [grass] * 9 + [sky] * 7),
            ]
            for idx, (name, mix) in enumerate(inputs):
                p = tmp_p / name
                write_mixed_png(p, mix)
                pad_png_to_min_size(p, 32 * 1024, salt=idx + 1)
            reports, failures = vs.validate(
                tmp_p,
                terrain_min=0.30,
                top_terrain_min=0.15,
                min_size_bytes=30 * 1024,
                require_min_count=5,
            )
            self.assertEqual(len(reports), 5)
            self.assertEqual(failures, [], f"unexpected failures: {failures}")
            # md5 unique 检查（防 helper 退化成同 hash）
            self.assertEqual(len({r.md5 for r in reports}), 5)

    def test_full_pipeline_pr71_replication(self) -> None:
        # 用真实 PNG（不是 mocked report）复现 PR #71 失败现象
        with TemporaryDirectory() as tmp:
            tmp_p = Path(tmp)
            sky = (120, 167, 255)
            grass = (90, 130, 60)
            # iso_nw 全 sky
            iso_nw = tmp_p / "preview-iso_nw.png"
            write_solid_png(iso_nw, sky, size=(80, 60))
            # iso_sw 跟 iso_nw byte-identical（同尺寸同色）
            iso_sw = tmp_p / "preview-iso_sw.png"
            write_solid_png(iso_sw, sky, size=(80, 60))
            # iso_ne 100% sky 但不同尺寸 → 不重复 hash
            iso_ne = tmp_p / "preview-iso_ne.png"
            write_solid_png(iso_ne, sky, size=(82, 60))
            # iso_se 71% void + 15% terrain
            iso_se = tmp_p / "preview-iso_se.png"
            mixed = [grass] * 2 + [sky] * 2 + [(0, 0, 0)] * 12
            write_mixed_png(iso_se, mixed, cells=(4, 4))
            # top 19.8% terrain（应过 R1 top 15% 阈值）
            top = tmp_p / "preview-top.png"
            mix_top = [grass] * 4 + [sky] * 12
            write_mixed_png(top, mix_top, cells=(4, 4))
            pad_png_to_min_size(top, 32 * 1024)

            reports, failures = vs.validate(
                tmp_p,
                terrain_min=0.30,
                top_terrain_min=0.15,
                min_size_bytes=30 * 1024,
                require_min_count=5,
            )
            self.assertEqual(len(reports), 5)
            joined = "\n".join(failures)
            self.assertIn("R2 md5 重复", joined, "iso_nw == iso_sw 应触发 R2")
            self.assertIn("R1 terrain<", joined, "iso 角度应触发 R1")
            # top 应过 R1（mix 25% > 15%）
            self.assertNotIn("R1 terrain< 15%: preview-top.png", joined)


class CliMainTests(unittest.TestCase):
    def test_main_pass(self) -> None:
        with TemporaryDirectory() as tmp:
            tmp_p = Path(tmp)
            grass = (90, 130, 60)
            sky = (120, 167, 255)
            inputs = [
                ("preview-top.png", [grass] * 12 + [sky] * 4),
                ("preview-iso_ne.png", [grass] * 11 + [sky] * 5),
            ]
            for idx, (name, mix) in enumerate(inputs):
                p = tmp_p / name
                write_mixed_png(p, mix)
                pad_png_to_min_size(p, 32 * 1024, salt=idx + 1)

            buf_out, buf_err = io.StringIO(), io.StringIO()
            with redirect_stdout(buf_out), redirect_stderr(buf_err):
                code = vs.main([
                    "--client-dir", str(tmp_p),
                    "--require-min-count", "2",
                ])
            self.assertEqual(code, 0, f"stderr: {buf_err.getvalue()}")
            self.assertIn("PASS", buf_out.getvalue())

    def test_main_fail_r1(self) -> None:
        with TemporaryDirectory() as tmp:
            tmp_p = Path(tmp)
            sky = (120, 167, 255)
            p = tmp_p / "preview-iso_ne.png"
            write_solid_png(p, sky, size=(80, 60))
            pad_png_to_min_size(p, 32 * 1024, salt=7)

            buf_out, buf_err = io.StringIO(), io.StringIO()
            with redirect_stdout(buf_out), redirect_stderr(buf_err):
                code = vs.main([
                    "--client-dir", str(tmp_p),
                    "--require-min-count", "1",
                ])
            self.assertEqual(code, 1)
            self.assertIn("R1 terrain<", buf_err.getvalue())

    def test_main_missing_dir(self) -> None:
        buf_out, buf_err = io.StringIO(), io.StringIO()
        with redirect_stdout(buf_out), redirect_stderr(buf_err):
            code = vs.main(["--client-dir", "/no/such/dir/xx"])
        self.assertEqual(code, 2)
        self.assertIn("不存在", buf_err.getvalue())

    def test_main_excludes_grid(self) -> None:
        with TemporaryDirectory() as tmp:
            tmp_p = Path(tmp)
            sky = (120, 167, 255)
            grid_path = tmp_p / "preview-grid.png"
            write_solid_png(grid_path, sky)  # 该过滤
            grass = (90, 130, 60)
            valid = tmp_p / "preview-top.png"
            write_mixed_png(valid, [grass] * 12 + [sky] * 4)
            pad_png_to_min_size(valid, 32 * 1024, salt=11)

            buf_out, buf_err = io.StringIO(), io.StringIO()
            with redirect_stdout(buf_out), redirect_stderr(buf_err):
                code = vs.main([
                    "--client-dir", str(tmp_p),
                    "--require-min-count", "1",
                ])
            self.assertEqual(code, 0, f"stderr: {buf_err.getvalue()}")
            # 表里不应出现 preview-grid.png
            self.assertNotIn("preview-grid.png", buf_out.getvalue())

    def test_main_below_min_count(self) -> None:
        with TemporaryDirectory() as tmp:
            buf_out, buf_err = io.StringIO(), io.StringIO()
            with redirect_stdout(buf_out), redirect_stderr(buf_err):
                code = vs.main([
                    "--client-dir", tmp,
                    "--require-min-count", "5",
                ])
            self.assertEqual(code, 1)
            self.assertIn("至少需要 5 张", buf_err.getvalue())


if __name__ == "__main__":
    unittest.main()
