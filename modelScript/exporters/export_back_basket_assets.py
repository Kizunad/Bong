#!/usr/bin/env python3
"""背篓（back_basket）bbmodel → 运行时资源导出。

背篓是**穿戴件**（worldview §十三 L558「背篓负于背」），走 grass_pouch 那条
worn-pack 线，不是 export_container_assets.py 的三件可放置容器，所以单独一个
exporter，不塞进那份 CONTAINERS 表。

产出三件：
    client/.../assets/bong/geo/back_basket.geo.json          ← GeckoLib 几何
    client/.../assets/bong/textures/entity/back_basket.png   ← 上身渲染贴图
    client/.../assets/bong-client/textures/gui/items/back_basket.png  ← 128×128 item 图标

geo/贴图复用 export_coffin_assets 的 build_geo/extract_texture（同一套 bbmodel
→ bedrock geo 转写，含 pivot 取 bbox 中心+底面那套修正），不另起一份转写逻辑。

item 图标**从模型渲**而不是手画：仓库现有 291 张 gui/items 图标是 gen.py 出图
管线的产物，而背篓已经有权威几何了——从 bbmodel 渲 3/4 视比手画像素更贴合实际
上身观感，且模型改了图标能跟着重出。透明底（RGBA），与 anqi_container_* 等
128×128 RGBA 图标一致。

用法:
    python3 modelScript/exporters/export_back_basket_assets.py
    python3 modelScript/exporters/export_back_basket_assets.py --check  # 只校验不写盘
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

import numpy as np
from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "core"))

from export_coffin_assets import _load, build_geo, extract_texture  # noqa: E402
from bbmodel_maker.render.render_bbmodel import render  # noqa: E402

REPO = Path(__file__).resolve().parents[2]
BBMODEL = Path(__file__).resolve().parents[1] / "models" / "BackBasket.bbmodel"
ASSETS = REPO / "client" / "src" / "main" / "resources" / "assets"

ENTITY_ID = "back_basket"
GEO_OUT = ASSETS / "bong" / "geo" / f"{ENTITY_ID}.geo.json"
TEX_OUT = ASSETS / "bong" / "textures" / "entity" / f"{ENTITY_ID}.png"
ICON_OUT = ASSETS / "bong-client" / "textures" / "gui" / "items" / f"{ENTITY_ID}.png"

ICON_SIZE = 128
# 渲染尺寸放大再降采样：128 直渲边缘锯齿重，4× 超采样后 LANCZOS 缩回。
ICON_SUPERSAMPLE = 4
# 图标取景：3/4 侧后视，能同时读出编身横带、皮盖、左右不对称背带。
ICON_YAW, ICON_PITCH = -32.0, 18.0
# 抠底用的哨兵色（模型里不存在的品红），渲完按它建 alpha。
CHROMA = (255, 0, 255)


def build_icon(bbmodel_path: Path) -> Image.Image:
    """从 bbmodel 渲 128×128 透明底 item 图标。"""
    big = ICON_SIZE * ICON_SUPERSAMPLE
    rendered, _ = render(str(bbmodel_path), yaw=ICON_YAW, pitch=ICON_PITCH,
                         size=big, bg=CHROMA, shading="mc")
    arr = np.asarray(rendered.convert("RGB")).astype(np.int16)
    # 哨兵色抠底：renderer 不做混色（每像素直接写纹理色×明度），所以精确等值即背景。
    bg = np.all(arr == np.array(CHROMA, np.int16), axis=-1)
    rgba = np.dstack([arr.astype(np.uint8),
                      np.where(bg, 0, 255).astype(np.uint8)])
    icon = Image.fromarray(rgba, "RGBA")

    # 裁到模型实际占位再缩放，避免图标里主体只占中间一小块。
    box = icon.getchannel("A").getbbox()
    if box is None:
        raise ValueError("渲出来全是背景色——模型没渲上，检查 bbmodel 路径/贴图")
    icon = icon.crop(box)

    # 等比塞进正方形画布（留 4px 边距，和现有图标的留白量级一致）。
    pad = 4 * ICON_SUPERSAMPLE
    side = max(icon.width, icon.height) + pad * 2
    canvas = Image.new("RGBA", (side, side), (0, 0, 0, 0))
    canvas.paste(icon, ((side - icon.width) // 2, (side - icon.height) // 2), icon)
    return canvas.resize((ICON_SIZE, ICON_SIZE), Image.LANCZOS)


def check_icon(icon: Image.Image) -> list[str]:
    """图标后验：不能全透明、不能糊成一坨、不能残留哨兵色边缘。"""
    bad: list[str] = []
    a = np.asarray(icon.getchannel("A"))
    rgb = np.asarray(icon.convert("RGB")).astype(np.int16)
    solid = a > 24
    cover = solid.mean()
    if cover < 0.06:
        bad.append(f"主体覆盖率仅 {cover:.1%}，图标里几乎看不见东西")
    if cover > 0.92:
        bad.append(f"主体覆盖率 {cover:.1%}，几乎糊满画布、读不出轮廓")

    # 哨兵色残留：LANCZOS 会在边缘混出接近品红的像素，只查明显整块残留。
    chroma_ish = (rgb[..., 0] > 200) & (rgb[..., 1] < 60) & (rgb[..., 2] > 200) & solid
    if chroma_ish.sum() > 16:
        bad.append(f"{int(chroma_ish.sum())} px 残留品红哨兵色，抠底没干净")

    # 明暗层次：全平的一块色说明取景/光照没吃上，进游戏读不出体积。
    lum = (0.299 * rgb[..., 0] + 0.587 * rgb[..., 1] + 0.114 * rgb[..., 2])[solid]
    if lum.size and float(lum.max() - lum.min()) < 30.0:
        bad.append(f"亮度极差仅 {float(lum.max() - lum.min()):.0f}，没有立体明暗")
    return bad


def main() -> int:
    ap = argparse.ArgumentParser(description="背篓运行时资源导出")
    ap.add_argument("--check", action="store_true", help="只校验，不写盘")
    args = ap.parse_args()

    if not BBMODEL.exists():
        print(f"缺 {BBMODEL.relative_to(REPO)}：先跑 gen_back_basket.py")
        return 2

    bb = _load(BBMODEL)
    geo = build_geo(bb, f"geometry.bong.{ENTITY_ID}")
    texture, tw, th = extract_texture(bb)
    desc = geo["minecraft:geometry"][0]["description"]
    desc["texture_width"], desc["texture_height"] = tw, th

    bones = geo["minecraft:geometry"][0]["bones"]
    cubes = sum(len(b.get("cubes", [])) for b in bones)
    print(f"背篓 / {ENTITY_ID} 导出:")
    print(f"  geo    : {len(bones)} bone / {cubes} cube, 贴图 {tw}×{th}")

    icon = build_icon(BBMODEL)
    bad = check_icon(icon)
    for msg in bad:
        print(f"  ✗ 图标: {msg}")
    if not bad:
        a = np.asarray(icon.getchannel("A"))
        print(f"  icon   : {ICON_SIZE}×{ICON_SIZE} RGBA, 主体覆盖 {(a > 24).mean():.1%}")

    if args.check:
        return 1 if bad else 0
    if bad:
        print("  图标后验未过，不写盘")
        return 1

    for p in (GEO_OUT, TEX_OUT, ICON_OUT):
        p.parent.mkdir(parents=True, exist_ok=True)
    GEO_OUT.write_text(json.dumps(geo, indent=2, ensure_ascii=False) + "\n")
    TEX_OUT.write_bytes(texture)
    icon.save(ICON_OUT)
    for p in (GEO_OUT, TEX_OUT, ICON_OUT):
        print(f"  → {p.relative_to(REPO)} ({p.stat().st_size} B)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
