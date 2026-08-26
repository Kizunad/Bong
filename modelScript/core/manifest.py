#!/usr/bin/env python3
"""资产特征清单（manifest）+ 点名器 —— 治「参考图特征被整件丢掉，而所有数值门都是绿的」。

**设计红线：这份清单必须人写。** 本模块里没有、也永远不许有任何「从参考图/从模型自动
推断该有哪些特征」的路径 —— 那等于自己出题自己判卷。点名器只干一件事：**核对人给的
清单**，缺一项就红。

这条红线是拿实测换来的。小草包前两轮**整件漏掉背带**：背带在参考图里画面占比仅次于
包身，没它那件读作「放在地上的篮子」而不是穿戴容器 —— 而当时七道数值门全绿，因为
「有没有背带」根本不在任何一道门的问题域里。人看图三十秒就能问出「背带呢」，模型跑
四十分钟数值门也问不出来。

判据用**差分**，不是绝对亮度也不是单独渲一遍特征：
    某特征的上镜量 = 完整模型的图 与 抽掉该特征后的图 之间**变了多少像素**。
这样自带遮挡正确性（被包身挡住的件贡献就是 0，正是它该得的分），也天然免疫光照
——两次渲染的光照条件完全相同。单独把特征渲一遍则会把「藏在身后」误报成「上镜了」。

材质普查（哪几种材质根本没露过面）走**色相单位向量**分类：像素归一化成单位向量，
跟归一化的材质色比距离。绝对亮度阈值在不同朝向/光照下会失效（正面 0.32 地板值和
顶面 1.0 差三倍），色相不会。

manifest 长这样（`modelScript/manifests/<Asset>.manifest.toml`，tomllib 读，标准库）：

    facing = "+z"
    mirror_x = 8.0                   # 中轴 x（居中空间 0；平移进方块空间的资产 8）

    [materials]                      # 可选：材质名 → RGB，供材质普查
    weave = [152, 136, 102]

    [features]
    braided_rim    = { elements = ["braid_"], must_show_in = ["FRONT", "SIDE_R"], min_px = 200 }
    shoulder_strap = { elements = ["strap_"], must_show_in = ["FRONT"], min_px = 300, mirror = true }
    flap_lid       = { elements = ["flap_"], must_show_in = ["FRONT", "SIDE_R"], min_px = { FRONT = 2200, SIDE_R = 280 } }
    side_pocket    = { elements = ["pocket_"], must_show_in = ["SIDE_R"], asym = "right" }

选件三选一（可叠加）：`material`（材质名）、`elements`（件名前缀）、`names`（精确件名）；
一个都不给就把特征名本身当件名前缀。`mirror = true` 要求左右都有且都上镜；
`asym = "left"/"right"` 要求全部件落在指定一侧（右 = FRONT 视里观者右手边，见 framing）。
"""

from __future__ import annotations

import json
import sys
import tempfile
import tomllib
from dataclasses import dataclass, field
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))

import framing  # noqa: E402

MANIFEST_DIR = Path(__file__).resolve().parents[1] / "manifests"

# 色相判据的默认容差。实测区间 0.035~0.05：两个单位向量的欧氏距离，0.05 大约是
# 5° 夹角。再松就开始把 seam 和 weave（同一色系压深一档）混成一种。
HUE_TOL = 0.045

# 差分像素判据：两张图同一像素的三通道绝对差之和超过它才算「变了」。
# 取 8 而不是 0 是为了压掉共面 z-fighting 在抽件前后各挑一次的高频噪点。
DIFF_TOL = 8

# 左右判定的死区。件中心 |x| 小于它就算骑在中线上，既不算左也不算右。
SIDE_EPS = 1e-6


@dataclass(frozen=True)
class Feature:
    """一条人写的特征。缺任何一条都是红，不存在「大概还行」。"""

    key: str
    must_show_in: tuple[str, ...]
    # 视角名 → 该视角上的最小上镜像素数。`min_px = 300` 写成对所有列出视角同一门限；
    # `min_px = { FRONT = 2200, SIDE_R = 280 }` 逐视角写 —— 同一个件在正视和侧视的
    # 合理占比常差一个数量级（翻盖正视 3537px、侧视只剩 444px），一刀切门限要么
    # 松到形同虚设，要么误报。
    min_px: dict[str, int] = field(default_factory=dict)
    mirror: bool = False
    asym: str | None = None
    material: tuple[str, ...] = ()
    elements: tuple[str, ...] = ()
    names: tuple[str, ...] = ()

    def selector_text(self) -> str:
        bits = []
        if self.material:
            bits.append("material " + "/".join(self.material))
        if self.elements:
            bits.append("elements " + "/".join(f"{p}*" for p in self.elements))
        if self.names:
            bits.append("names " + "/".join(self.names))
        return "; ".join(bits) or f"elements {self.key}*"


@dataclass(frozen=True)
class Manifest:
    facing: str
    features: tuple[Feature, ...]
    # 模型中轴的 x 坐标。居中建模空间是 0；写盘前平移进 MC 方块空间（x/z 各 +8）的
    # 资产是 8。左右判定（mirror / asym）全靠它 —— 默认 0 套在方块空间的模型上，
    # 会把整件资产判成「全在右侧」，而且一声不吭。
    mirror_x: float = 0.0
    # 门限所依据的渲染边长。min_px 是**像素数**，跟着 size 平方缩放 —— 换个 size
    # 跑同一份清单，门限就整体失真。所以 size 写在清单里，跟门限锁在一起。
    size: int = 260
    materials: dict[str, tuple[int, int, int]] = field(default_factory=dict)
    source: Path | None = None

    def view_names(self) -> tuple[str, ...]:
        """所有特征点到的视角，按 SIX_VIEWS 的固定顺序去重。"""
        wanted = {v for f in self.features for v in f.must_show_in}
        ordered = [n for n in framing.SIX_VIEWS if n in wanted]
        ordered += sorted(wanted - set(ordered))
        return tuple(ordered)


_FEATURE_KEYS = {"must_show_in", "min_px", "mirror", "asym", "material", "elements", "names"}


def _as_tuple(value, field_name: str, key: str) -> tuple[str, ...]:
    if value is None:
        return ()
    if isinstance(value, str):
        return (value,)
    if isinstance(value, list) and all(isinstance(v, str) for v in value):
        return tuple(value)
    raise ValueError(f"特征 {key!r} 的 {field_name} 必须是字符串或字符串数组，收到 {value!r}")


def parse_manifest(doc: dict, source: Path | None = None) -> Manifest:
    """dict → Manifest。字段写错一律炸，不给默认值兜底 —— 清单默默失效比没有清单更糟。"""
    facing = doc.get("facing")
    if not isinstance(facing, str):
        raise ValueError("manifest 必须在顶层声明 facing（\"+z\" / \"-z\" / \"+x\" / \"-x\"）")
    framing.parse_facing(facing)

    mirror_x = doc.get("mirror_x", 0.0)
    if not isinstance(mirror_x, (int, float)) or isinstance(mirror_x, bool):
        raise ValueError(f"mirror_x 必须是数字（中轴的 x 坐标），收到 {mirror_x!r}")

    size = doc.get("size", 260)
    if isinstance(size, bool) or not isinstance(size, int) or size < 32:
        raise ValueError(f"size 必须是 ≥32 的整数（渲染边长），收到 {size!r}")

    raw_mats = doc.get("materials", {})
    if not isinstance(raw_mats, dict):
        raise ValueError("[materials] 必须是 材质名 = [r, g, b] 的表")
    materials: dict[str, tuple[int, int, int]] = {}
    for name, rgb in raw_mats.items():
        if not (isinstance(rgb, list) and len(rgb) == 3
                and all(isinstance(c, int) and 0 <= c <= 255 for c in rgb)):
            raise ValueError(f"材质 {name!r} 的颜色必须是三个 0..255 整数，收到 {rgb!r}")
        materials[name] = (rgb[0], rgb[1], rgb[2])

    raw_features = doc.get("features", {})
    if not isinstance(raw_features, dict):
        raise ValueError("[features] 必须是 特征名 = { ... } 的表")
    if not raw_features:
        raise ValueError("manifest 里一个特征都没有 —— 空清单点不出任何缺项，等于没写")

    features = []
    for key, spec in raw_features.items():
        if not isinstance(spec, dict):
            raise ValueError(f"特征 {key!r} 必须是一张表，收到 {spec!r}")
        unknown = set(spec) - _FEATURE_KEYS
        if unknown:
            raise ValueError(
                f"特征 {key!r} 有未知字段 {sorted(unknown)}；可用 {sorted(_FEATURE_KEYS)}"
            )
        views = _as_tuple(spec.get("must_show_in"), "must_show_in", key)
        if not views:
            raise ValueError(f"特征 {key!r} 没写 must_show_in —— 不说在哪张图上找，点名器无从核对")
        for v in views:
            framing.views_for(facing, (v,))     # 视角名写错立刻炸，别等渲完才发现
        raw_min = spec.get("min_px", 1)
        if isinstance(raw_min, bool) or not isinstance(raw_min, (int, dict)):
            raise ValueError(
                f"特征 {key!r} 的 min_px 必须是 ≥1 的整数，或 视角名 = 整数 的表，收到 {raw_min!r}"
            )
        if isinstance(raw_min, int):
            if raw_min < 1:
                raise ValueError(f"特征 {key!r} 的 min_px 必须 ≥1，收到 {raw_min}")
            min_px = {v: raw_min for v in views}
        else:
            stray = set(raw_min) - set(views)
            if stray:
                raise ValueError(
                    f"特征 {key!r} 的 min_px 给了 {sorted(stray)} 的门限，"
                    f"但 must_show_in 里没有这些视角"
                )
            for v, n in raw_min.items():
                if isinstance(n, bool) or not isinstance(n, int) or n < 1:
                    raise ValueError(f"特征 {key!r} 在 {v} 的 min_px 必须是 ≥1 的整数，收到 {n!r}")
            min_px = {v: int(raw_min.get(v, 1)) for v in views}
        asym = spec.get("asym")
        if asym is not None and asym not in ("left", "right"):
            raise ValueError(f"特征 {key!r} 的 asym 只能是 \"left\" 或 \"right\"，收到 {asym!r}")
        mirror = spec.get("mirror", False)
        if not isinstance(mirror, bool):
            raise ValueError(f"特征 {key!r} 的 mirror 必须是布尔，收到 {mirror!r}")
        if mirror and asym:
            raise ValueError(f"特征 {key!r} 同时要求 mirror 和 asym={asym!r} —— 自相矛盾")
        features.append(Feature(
            key=key,
            must_show_in=views,
            min_px=min_px,
            mirror=mirror,
            asym=asym,
            material=_as_tuple(spec.get("material"), "material", key),
            elements=_as_tuple(spec.get("elements"), "elements", key),
            names=_as_tuple(spec.get("names"), "names", key),
        ))
    return Manifest(facing=facing, features=tuple(features), mirror_x=float(mirror_x),
                    size=int(size), materials=materials, source=source)


def load_manifest(path) -> Manifest:
    p = Path(path)
    if not p.is_file():
        raise FileNotFoundError(f"没有 manifest：{p}（清单必须人写，工具不会替你生成一份）")
    with p.open("rb") as fh:
        return parse_manifest(tomllib.load(fh), source=p)


def manifest_for(model_path, manifest_dir=MANIFEST_DIR) -> Manifest:
    """按模型文件名找同名 manifest：`GrassPouch.bbmodel` → `GrassPouch.manifest.toml`。"""
    return load_manifest(Path(manifest_dir) / f"{Path(model_path).stem}.manifest.toml")


# ================================================================ 色相分类
def _unit(rgb) -> np.ndarray:
    a = np.asarray(rgb, float)
    n = np.linalg.norm(a, axis=-1, keepdims=True)
    return np.divide(a, np.where(n < 1e-9, 1.0, n))


def hue_match(rgb, palette: dict, tol: float = HUE_TOL) -> str | None:
    """单个像素/颜色 → 最近的材质名；都不够近就 None。"""
    if not palette:
        return None
    names = list(palette)
    ref = _unit(np.array([palette[n] for n in names], float))
    d = np.linalg.norm(ref - _unit(np.asarray(rgb, float)), axis=1)
    i = int(np.argmin(d))
    return names[i] if float(d[i]) <= tol else None


def hue_counts(img, palette: dict, tol: float = HUE_TOL) -> dict[str, int]:
    """一张渲染图里各材质的像素数（色相判据，免疫光照强弱）。

    背景色也会被分类 —— 调用方要么给的 palette 里没有接近背景的颜色，要么自己扣掉。
    这里不猜背景：渲染背景色是 render() 的参数，猜错比不猜更糟。
    """
    a = np.asarray(img, float).reshape(-1, 3)
    out = {name: 0 for name in palette}
    if not palette:
        return out
    names = list(palette)
    ref = _unit(np.array([palette[n] for n in names], float))
    d = np.linalg.norm(ref[None, :, :] - _unit(a)[:, None, :], axis=2)
    best = d.argmin(1)
    hit = d[np.arange(len(a)), best] <= tol
    for i, name in enumerate(names):
        out[name] = int(np.count_nonzero(hit & (best == i)))
    return out


# ================================================================ 选件
def _face_uv_center(el: dict) -> tuple[float, float] | None:
    for fd in (el.get("faces") or {}).values():
        uv = fd.get("uv")
        if uv and len(uv) == 4:
            return ((uv[0] + uv[2]) / 2.0, (uv[1] + uv[3]) / 2.0)
    return None


def element_materials(model_path, palette: dict, tol: float = HUE_TOL) -> dict[str, str | None]:
    """件名 → 材质名，靠**采样贴图**判定而不是靠 element["color"] 索引。

    `color` 是 `材质序号 % 8`：超过 8 种材质就开始撞车，而 rigkit 的 swatch 表最多能放
    64 种。UV 中心采一个像素再按色相归类，对 rigkit 产物和手改稿一样有效。
    """
    from render_bbmodel import load_bbmodel

    doc = json.loads(Path(model_path).read_text())
    _, tex, _, _ = load_bbmodel(model_path)
    th, tw = tex.shape[:2]
    out: dict[str, str | None] = {}
    for el in doc.get("elements", []):
        uv = _face_uv_center(el)
        if uv is None:
            out[el["name"]] = None
            continue
        u = int(np.clip(uv[0], 0, tw - 1))
        v = int(np.clip(uv[1], 0, th - 1))
        out[el["name"]] = hue_match(tex[v, u, :3], palette, tol)
    return out


def select_elements(doc: dict, feature: Feature, mat_of: dict) -> list[dict]:
    """按特征的选件规则挑出 element。三种选法取并集，一个都不给就拿特征名当前缀。"""
    prefixes = feature.elements
    if not (feature.material or feature.elements or feature.names):
        prefixes = (feature.key,)
    picked = []
    for el in doc.get("elements", []):
        name = el.get("name", "")
        if name in feature.names:
            picked.append(el)
            continue
        if prefixes and any(name.startswith(p) for p in prefixes):
            picked.append(el)
            continue
        if feature.material and mat_of.get(name) in feature.material:
            picked.append(el)
    return picked


# ================================================================ 点名
@dataclass
class FeatureVerdict:
    key: str
    selector: str
    count: int                       # 选中的 element 数
    pixels: dict[str, int]           # 视角名 → 差分像素数
    problems: list[str]

    @property
    def ok(self) -> bool:
        return not self.problems


@dataclass
class RollCall:
    model: Path
    manifest: Manifest
    views: tuple[str, ...]
    verdicts: tuple[FeatureVerdict, ...]
    census: dict[str, dict[str, int]]    # 视角名 → 材质名 → 像素数

    @property
    def ok(self) -> bool:
        return all(v.ok for v in self.verdicts)

    @property
    def missing(self) -> tuple[str, ...]:
        return tuple(v.key for v in self.verdicts if not v.ok)

    def unseen_materials(self) -> tuple[str, ...]:
        """一张图上都没露过面的材质。没有 [materials] 时返回空。"""
        if not self.manifest.materials:
            return ()
        return tuple(m for m in self.manifest.materials
                     if all(self.census[v].get(m, 0) == 0 for v in self.views))

    def lines(self) -> list[str]:
        """给人看的表。以 ! 开头的行是红的（contact_sheet 会把它染色）。"""
        head = f"{'特征':<18}{'件':>4}" + "".join(f"{v:>9}" for v in self.views) + "  判定"
        out = [f"点名 {self.model.name}（facing={self.manifest.facing}）", head]
        for v in self.verdicts:
            cells = "".join(f"{v.pixels.get(name, 0):>9}" for name in self.views)
            mark = "OK" if v.ok else "; ".join(v.problems)
            out.append(("" if v.ok else "! ") + f"{v.key:<18}{v.count:>4}{cells}  {mark}")
        unseen = self.unseen_materials()
        if unseen:
            out.append("! 材质一次都没上镜: " + ", ".join(unseen))
        elif self.manifest.materials:
            out.append(f"材质全部上镜: {len(self.manifest.materials)} 种")
        out.append(("" if self.ok else "! ") + f"→ {len(self.missing)} 处缺项")
        return out

    def report(self) -> int:
        for line in self.lines():
            print(line)
        return len(self.missing)


def _foreground(img, bg) -> np.ndarray:
    return np.abs(np.asarray(img, int) - np.array(bg, int)).sum(2) > DIFF_TOL


def _changed(a, b) -> int:
    return int(np.count_nonzero(
        np.abs(np.asarray(a, int) - np.asarray(b, int)).sum(2) > DIFF_TOL))


def roll_call(model_path, manifest: Manifest, *, size: int | None = None,
              shading: str = "lambert", bg=(22, 23, 26),
              keep_images: dict | None = None) -> RollCall:
    """按清单逐条点名。

    size 缺省取清单里的 `size`（门限和它锁在一起）。显式传别的值只用于出图，
    别拿来判门限 —— 像素数按面积缩放，门限会整体失真。

    keep_images 非空时把各视角的完整模型渲染图塞进去（视角名 → Image），供 contact
    sheet 复用，省得同一张图渲两遍。
    """
    from render_bbmodel import render

    size = manifest.size if size is None else size
    model_path = Path(model_path)
    doc = json.loads(model_path.read_text())
    view_names = manifest.view_names()
    views = framing.views_for(manifest.facing, view_names)
    focus = framing.focus_for(model_path, views)

    base = {}
    for v in views:
        img, _ = render(model_path, yaw=v.yaw, pitch=v.pitch, size=size, bg=bg,
                        focus=focus, shading=shading)
        base[v.name] = img
    if keep_images is not None:
        keep_images.update(base)

    palette = manifest.materials
    mat_of = element_materials(model_path, palette) if palette else {}
    census = {name: hue_counts(img, palette) if palette else {} for name, img in base.items()}

    from rigkit import element_bounds

    verdicts = []
    with tempfile.TemporaryDirectory() as tmp:
        probe = Path(tmp) / "probe.bbmodel"
        for feat in manifest.features:
            picked = select_elements(doc, feat, mat_of)
            problems: list[str] = []
            pixels: dict[str, int] = {}
            if not picked:
                # 这就是「背带整件漏掉」那一档缺陷：选不中件，说明清单点的东西根本不存在。
                problems.append(f"整件缺席（{feat.selector_text()} 选不中任何 element）")
                verdicts.append(FeatureVerdict(feat.key, feat.selector_text(), 0, {}, problems))
                continue

            drop = {el["uuid"] for el in picked}
            rest = [el for el in doc["elements"] if el["uuid"] not in drop]
            for name in feat.must_show_in:
                b = base[name]
                if not rest:
                    # 特征覆盖了整个模型：抽光了没法渲，它的上镜量就是整个剪影。
                    pixels[name] = int(np.count_nonzero(_foreground(b, bg)))
                    continue
                probe.write_text(json.dumps({**doc, "elements": rest}, ensure_ascii=False))
                view = framing.view_by_name(manifest.facing, name)
                other, _ = render(probe, yaw=view.yaw, pitch=view.pitch, size=size, bg=bg,
                                  focus=focus, shading=shading)
                pixels[name] = _changed(b, other)
            for name in feat.must_show_in:
                floor = feat.min_px.get(name, 1)
                if pixels[name] < floor:
                    problems.append(f"{name} 只露 {pixels[name]}px < {floor}")

            axis = manifest.mirror_x
            centers = {el["name"]: (element_bounds([el])[0][0] + element_bounds([el])[1][0]) / 2 - axis
                       for el in picked}
            right = [n for n, c in centers.items() if c > SIDE_EPS]
            left = [n for n, c in centers.items() if c < -SIDE_EPS]
            if feat.mirror:
                if not right or not left:
                    have = "只在右侧" if right else ("只在左侧" if left else "全压在中线上")
                    problems.append(f"要求左右成对，实际{have}")
            if feat.asym == "right" and left:
                problems.append(f"要求只在右侧，但 {', '.join(sorted(left)[:3])} 在左侧")
            if feat.asym == "left" and right:
                problems.append(f"要求只在左侧，但 {', '.join(sorted(right)[:3])} 在右侧")

            verdicts.append(FeatureVerdict(feat.key, feat.selector_text(), len(picked),
                                           pixels, problems))

    return RollCall(model_path, manifest, view_names, tuple(verdicts), census)


def main() -> int:
    import argparse

    ap = argparse.ArgumentParser(description="按人写的 manifest 给资产点名")
    ap.add_argument("model", help=".bbmodel 路径")
    ap.add_argument("--manifest", help="清单路径（缺省按模型名去 modelScript/manifests/ 找）")
    ap.add_argument("--size", type=int, default=None,
                    help="覆盖清单里的渲染边长；会让 min_px 门限失真，只用于出图")
    args = ap.parse_args()

    mf = load_manifest(args.manifest) if args.manifest else manifest_for(args.model)
    return 1 if roll_call(args.model, mf, size=args.size).report() else 0


if __name__ == "__main__":
    raise SystemExit(main())
