#!/usr/bin/env python3
"""侧视比例量尺：把参考图和模型 LEFT 侧视量成同一套两个比值，差超过 10% 就该继续改。

用户反复打回「身体太短了，按照比例来」（剥皮豺、断角野牛、腐根熊），根因是没有量过：
  躯干比   躯干长 ÷ 肩高        躯干长 = 肩前缘到臀后缘；肩高 = 地面到肩峰（鬃毛 / 肩隆）顶
  头颈比   头颈长 ÷ 躯干长      头颈长 = 鼻尖到肩前缘

两边用同一个定义，只是取点方式不同：
  - 参考图：地面 / 肩顶 / 鼻尖从轮廓取（轮廓 = 偏离灰背景的像素），肩前缘和臀后缘由人在图上点（LANDMARKS），
    check.png 里会把这些线画在参考图上，谁觉得点错了一眼能看出来；
  - 模型：地面 / 肩顶 / 鼻尖同样从渲染轮廓取，肩前缘和臀后缘用躯干几何的 z 范围（生成器传进来）按渲染同一个投影换成像素。

肩顶只在「肩前缘起向后 30% 躯干长」这个窗口里取最高点，免得把耳朵、尾巴当成肩高。

模型侧视按 yaw=90（相机在 -X，头朝画面左，+Z 向右）；参考图也要求头朝左。
"""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

import numpy as np
from bbmodel_maker.render.render_bbmodel import _rotmat, render
from PIL import Image, ImageDraw

BG = (119, 119, 119)
SHOULDER_WINDOW = 0.30          # 肩顶只在肩前缘起向后这么多躯干长内取
TOLERANCE = 0.10                # 参考与模型的比值差超过它就继续改


@dataclass
class SideMeasure:
    """一张侧视图上量出的点（像素）和两个比值。"""

    ground_y: float
    shoulder_top_y: float
    nose_x: float
    front_x: float
    rear_x: float

    @property
    def height(self) -> float:
        return self.ground_y - self.shoulder_top_y

    @property
    def length(self) -> float:
        return self.rear_x - self.front_x

    @property
    def head_neck(self) -> float:
        return self.front_x - self.nose_x

    @property
    def length_over_height(self) -> float:
        return self.length / self.height

    @property
    def head_neck_over_length(self) -> float:
        return self.head_neck / self.length


def measure_mask(mask: np.ndarray, front_x: float, rear_x: float, nose_x: float | None = None) -> SideMeasure:
    """从轮廓蒙版（True = 生物）取地面 / 肩顶 / 鼻尖；front_x / rear_x 是躯干前后缘的像素列。
    nose_x 缺省取轮廓最靠前的列；角比鼻子更靠前的生物（野牛）要手动给鼻尖列，免得把角尖当成鼻尖。"""
    rows, cols = np.where(mask)
    window = mask[:, int(front_x): int(front_x + SHOULDER_WINDOW * (rear_x - front_x))]
    return SideMeasure(ground_y=float(rows.max()), shoulder_top_y=float(np.where(window.any(axis=1))[0].min()),
                       nose_x=float(cols.min()) if nose_x is None else nose_x, front_x=front_x, rear_x=rear_x)


def reference_mask(image: Image.Image) -> np.ndarray:
    """参考图的生物轮廓：偏离灰背景（R≈G≈B）的像素，加上近黑的爪 / 鼻头。"""
    px = np.asarray(image.convert("RGB")).astype(int)
    return (np.abs(px[..., 0] - px[..., 1]) > 16) | (px.sum(axis=2) < 165)


def model_side_mask(model_path: Path, focus, size: int = 1400) -> tuple[np.ndarray, Image.Image]:
    """模型 LEFT 侧视（yaw=90）渲染 + 轮廓蒙版（偏离背景色的像素）。"""
    image = render(model_path, yaw=90.0, pitch=0.0, size=size, bg=BG, focus=focus)[0]
    return np.any(np.asarray(image) != np.array(BG), axis=2), image


def project_z(z: float, focus, size: int) -> float:
    """世界 z → LEFT 侧视（yaw=90）画面 x 像素，用的就是 render() 的投影。"""
    center, span = np.asarray(focus[0], float), float(focus[1])
    view = (_rotmat(0.0, 0) @ _rotmat(90.0, 1)) @ (np.array([center[0], center[1], z]) - center)
    return float(view[0] * (size - 60) / span + size / 2)


def project_ground_y(focus, size: int) -> float:
    """世界 y = 0（蹄 / 爪垫落地的平面）→ 画面 y 像素。"""
    center, span = np.asarray(focus[0], float), float(focus[1])
    return float(-(0.0 - center[1]) * (size - 60) / span + size / 2)


def measure_model(model_path: Path, focus, torso_z: tuple[float, float], size: int = 1400) -> tuple[SideMeasure, Image.Image]:
    """模型的地面取世界 y = 0，不取轮廓最低点：垂到地面以下的须 / 毛 / 尾梢会把轮廓最低点拉低、肩高虚高；
    （参考图里生物站在地上，轮廓最低点就是蹄，所以参考图仍取轮廓。）"""
    mask, image = model_side_mask(model_path, focus, size)
    front_x, rear_x = (project_z(z, focus, size) for z in torso_z)
    measure = measure_mask(mask, front_x, rear_x)
    measure.ground_y = project_ground_y(focus, size)
    return measure, image


def crop_to_figure(image: Image.Image, mask: np.ndarray, m: SideMeasure, pad: int = 24) -> tuple[Image.Image, SideMeasure]:
    """裁到生物轮廓包围盒（加边距），量尺点跟着平移；裁完再画线，三张图才能同高并排。"""
    rows, cols = np.where(mask)
    x0, y0 = max(0, int(cols.min()) - pad), max(0, int(rows.min()) - pad)
    x1, y1 = min(image.width, int(cols.max()) + pad), min(image.height, int(rows.max()) + pad)
    shifted = SideMeasure(m.ground_y - y0, m.shoulder_top_y - y0, m.nose_x - x0, m.front_x - x0, m.rear_x - x0)
    return image.crop((x0, y0, x1, y1)), shifted


def percent(model: float, reference: float) -> float:
    return (model - reference) / reference


def verdict(ref: SideMeasure, model: SideMeasure) -> tuple[float, float, bool]:
    a = percent(model.length_over_height, ref.length_over_height)
    b = percent(model.head_neck_over_length, ref.head_neck_over_length)
    return a, b, abs(a) <= TOLERANCE and abs(b) <= TOLERANCE


def annotate(image: Image.Image, m: SideMeasure, label: str) -> Image.Image:
    """把量尺线画在图上：地面、肩顶、鼻尖、躯干前后缘；顶上写这一张的两个比值。"""
    out = image.convert("RGB").copy()
    d = ImageDraw.Draw(out)
    w, h = out.size
    for y, color in ((m.ground_y, (90, 200, 255)), (m.shoulder_top_y, (255, 210, 90))):
        d.line([(0, y), (w, y)], fill=color, width=2)
    for x, color in ((m.nose_x, (120, 255, 150)), (m.front_x, (255, 120, 220)), (m.rear_x, (255, 120, 220))):
        d.line([(x, 0), (x, h)], fill=color, width=2)
    d.text((8, 6), f"{label}  躯干长/肩高 = {m.length_over_height:.2f}   头颈长/躯干长 = {m.head_neck_over_length:.2f}",
           fill=(255, 255, 255))
    return out


def ratio_row(font, ref_image: Image.Image, ref: SideMeasure, model_image: Image.Image, model: SideMeasure,
              prev_image: Image.Image | None = None, prev: SideMeasure | None = None, height: int = 420) -> Image.Image:
    """check.png 用的一行：参考（带量尺线）| 上一版 | 当前版，下面一行字写两组比值和差。
    三张图按各自的包围盒裁到同高（肩顶 / 地面线随图缩放，所以比值不受缩放影响）。"""
    tiles = [("参考（量尺线：蓝=地面 黄=肩顶 绿=鼻尖 粉=躯干前后缘）", annotate(ref_image, ref, "参考"))]
    if prev_image is not None and prev is not None:
        tiles.append(("上一版（同取景）", annotate(prev_image, prev, "上一版")))
    tiles.append(("当前版", annotate(model_image, model, "当前")))
    resized = [(title, im.resize((round(im.width * height / im.height), height), Image.Resampling.LANCZOS)) for title, im in tiles]
    width = sum(im.width + 10 for _, im in resized) + 10
    a, b, ok = verdict(ref, model)
    lines = [f"参考：躯干长/肩高 {ref.length_over_height:.2f}，头颈长/躯干长 {ref.head_neck_over_length:.2f}"]
    if prev is not None:
        lines.append(f"上一版：{prev.length_over_height:.2f} / {prev.head_neck_over_length:.2f}"
                     f"（差 {percent(prev.length_over_height, ref.length_over_height):+.0%} / "
                     f"{percent(prev.head_neck_over_length, ref.head_neck_over_length):+.0%}）")
    lines.append(f"当前版：{model.length_over_height:.2f} / {model.head_neck_over_length:.2f}"
                 f"（差 {a:+.0%} / {b:+.0%}，容差 ±{TOLERANCE:.0%}，{'✓ 达标' if ok else '✗ 还要改'}）")
    canvas = Image.new("RGB", (width, height + 34 + 22 * len(lines)), (32, 33, 36))
    d = ImageDraw.Draw(canvas)
    x = 10
    for title, im in resized:
        d.text((x + 2, 8), title, fill=(215, 205, 180), font=font)
        canvas.paste(im, (x, 30))
        x += im.width + 10
    for n, line in enumerate(lines):
        d.text((12, height + 36 + 22 * n), line, fill=(240, 220, 150) if n == len(lines) - 1 else (210, 210, 215), font=font)
    return canvas


def focus_for_model(model_path: Path) -> tuple[tuple[float, float, float], float]:
    """按 bbmodel 里所有立方体的 from / to 求一个固定取景（中心, 跨度×1.1）；旋转过的件只按未旋转的盒算，
    差别只影响取景大小，不影响量尺（量尺用同一个投影换算）。"""
    import json

    elements = json.loads(Path(model_path).read_text(encoding="utf-8"))["elements"]
    lo = np.min([e["from"] for e in elements], axis=0)
    hi = np.max([e["to"] for e in elements], axis=0)
    return tuple(float(v) for v in (lo + hi) / 2), float((hi - lo).max() * 1.1)


def bone_z_extent(model_path: Path, bones: tuple[str, ...]) -> tuple[float, float]:
    """这几根骨上所有立方体的 z 范围（按未旋转的盒算）：量躯干前后缘用，上一版也能照同样的骨取，不必手抄常数。"""
    import json

    doc = json.loads(Path(model_path).read_text(encoding="utf-8"))
    elements = {e["uuid"]: e for e in doc["elements"]}
    found: dict[str, list[str]] = {}

    def walk(nodes) -> None:
        for node in nodes:
            if isinstance(node, dict):
                found[node["name"]] = [c for c in node["children"] if isinstance(c, str)]
                walk(c for c in node["children"] if isinstance(c, dict))

    walk(doc["outliner"])
    zs = [z for bone in bones for uid in found.get(bone, []) for z in (elements[uid]["from"][2], elements[uid]["to"][2])]
    if not zs:
        raise ValueError(f"{model_path} 里这些骨上没有立方体：{bones}")
    return min(zs), max(zs)


def side_ratio_row(font, ref_path: Path, ref_crop: tuple[int, int, int, int], ref_torso_x: tuple[float, float],
                   now_path: Path, now_torso_z: tuple[float, float], prev_path: Path | None = None,
                   prev_torso_z: tuple[float, float] | None = None, ref_nose_x: float | None = None) -> Image.Image:
    """生成器一行调用：量参考图（裁出头朝左的侧视）、当前模型和上一版，出 check.png 底部那一行，并在终端打印两组比值。
    ref_crop 只裁出整只生物的侧视（别带到别的视角 / 细节图）；ref_torso_x 是人在裁剪后的参考图上点的肩前缘 / 臀后缘像素列；
    *_torso_z 是模型躯干前后缘的世界 z（见 bone_z_extent）。"""
    ref_image = Image.open(ref_path).convert("RGB").crop(ref_crop)
    ref_mask = reference_mask(ref_image)
    ref_image, ref = crop_to_figure(ref_image, ref_mask, measure_mask(ref_mask, *ref_torso_x, nose_x=ref_nose_x))

    def side(path: Path, torso_z: tuple[float, float]) -> tuple[Image.Image, SideMeasure]:
        focus = focus_for_model(path)
        measure, image = measure_model(path, focus, torso_z)
        return crop_to_figure(image, model_side_mask(path, focus)[0], measure)

    now_image, now = side(now_path, now_torso_z)
    prev_image, prev = side(prev_path, prev_torso_z) if prev_path is not None and prev_torso_z is not None else (None, None)
    a, b, ok = verdict(ref, now)
    print(f"比例：参考 {ref.length_over_height:.2f} / {ref.head_neck_over_length:.2f}，当前 {now.length_over_height:.2f} / "
          f"{now.head_neck_over_length:.2f}（差 {a:+.1%} / {b:+.1%}）{'✓' if ok else '✗ 超出 10%'}")
    return ratio_row(font, ref_image, ref, now_image, now, prev_image, prev)
