#!/usr/bin/env python3
"""手持器物审阅图：左边是爆炸参考图，右边是同一件 bbmodel 的 3/4 与侧视渲染。

三件事都和审阅有关，所以放在一起：
- 3/4 与侧视共用同一个取景（先用两个视角下的包围盒算出公共 center / span），所以同一
  张图里的细节粗细可比，不会因为各自自动取景而一张放大一张缩小；
- 参考图按高度缩放到和渲染同高，左右并排；
- 输出到 ``model-review/img/handheld/<id>/check.png``，该目录和 ``exploded/`` 同级。

用法（在生成器里调用）::

    from handheld_check import write_check_sheet
    write_check_sheet(bbmodel_path, "bao_chu")
"""

from __future__ import annotations

from pathlib import Path

import numpy as np
from bbmodel_maker.render import framing
from bbmodel_maker.render.render_bbmodel import render
from PIL import Image, ImageDraw

REVIEW_ROOT = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/handheld")
BACKGROUND = (119, 119, 119)
PANEL_BACKGROUND = (28, 29, 33)
LABEL_BACKGROUND = (24, 25, 28)
LABEL_COLOUR = (235, 235, 235)
VIEWS = (("3/4 (yaw -35, pitch 25)", -35.0, 25.0), ("SIDE (yaw 90, pitch 0)", 90.0, 0.0))
TILE = 520
MARGIN = 1.12


def shared_focus(bbmodel: Path) -> tuple[tuple[float, float, float], float]:
    """两个视角共用的 (center, span)：用整件的包围盒在每个视角下的投影取最大值。"""
    low, high = framing.model_bounds(bbmodel)
    center = (low + high) / 2.0
    corners = np.array(
        [[low[0] if i & 1 else high[0], low[1] if i & 2 else high[1], low[2] if i & 4 else high[2]] for i in range(8)],
        float,
    ) - center
    span = 0.0
    for _, yaw, pitch in VIEWS:
        view = framing.View(name="_", yaw=yaw, pitch=pitch, shows="")
        projected = (framing.view_matrix(view) @ corners.T).T[:, :2]
        span = max(span, float((projected.max(0) - projected.min(0)).max()))
    return (float(center[0]), float(center[1]), float(center[2])), span * MARGIN


def write_check_sheet(bbmodel: Path, item_id: str, review_root: Path = REVIEW_ROOT) -> Path:
    reference_path = review_root / "exploded" / f"{item_id}.png"
    focus = shared_focus(bbmodel)
    renders = [render(bbmodel, yaw=yaw, pitch=pitch, size=TILE, bg=BACKGROUND, focus=focus)[0] for _, yaw, pitch in VIEWS]

    reference = Image.open(reference_path).convert("RGB")
    reference = reference.resize((int(reference.width * TILE / reference.height), TILE), Image.Resampling.LANCZOS)

    header = 30
    width = reference.width + TILE * len(renders) + 12 * (len(renders) + 2)
    sheet = Image.new("RGB", (width, TILE + header + 12), PANEL_BACKGROUND)
    draw = ImageDraw.Draw(sheet)

    def place(image: Image.Image, label: str, x: int) -> int:
        sheet.paste(image, (x, header))
        draw.rectangle([x, 4, x + image.width, header - 4], fill=LABEL_BACKGROUND)
        draw.text((x + 6, 9), label, fill=LABEL_COLOUR)
        return x + image.width + 12

    x = place(reference, f"REFERENCE exploded/{item_id}.png", 12)
    for (label, _, _), image in zip(VIEWS, renders):
        x = place(image, label, x)

    out = review_root / item_id / "check.png"
    out.parent.mkdir(parents=True, exist_ok=True)
    sheet.save(out)
    return out
