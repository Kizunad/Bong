"""Bong 图像生成的画风 prefix 常量。

三档画风（对齐 local_images/generation_guide.md）：

- **item** —— 物品图标（药材、符牌、武器、法宝等），photorealistic 3D render 风，
  默认透明背景，便于直接接入 icon 资源。
- **particle** —— MC 粒子/VFX 贴图，纯黑 #000000 底，白色/近白形状 + 软羽化，
  生成后走 lum_to_alpha.py 转 alpha 通道。
- **hud** —— HUD overlay 贴图（水墨边框、结霜角、符阵等），真正透明 RGBA，
  中心区域 alpha=0，只有装饰像素有 alpha。生成接口必须传 background=transparent。

使用：`apply(style, body)` 把对应 prefix 与物品描述拼成完整 prompt。
prompt 开头三个词已匹配 prefix 则跳过拼接（允许手动叠写）。
"""

from __future__ import annotations

STYLE_ITEM = (
    "dark xianxia game item icon, shattered dark stone and crystal material, "
    "glowing energy cracks as the only light source, high contrast, "
    "dramatic self-illumination, photorealistic 3D render, "
    "fully transparent background (alpha=0), no background fill, centered, no shadows, no gradients"
)

STYLE_PARTICLE = (
    "vfx particle texture asset for game engine, "
    "pure black background (#000000), single luminous form only, "
    "soft feathered edges, no sharp outlines, no geometric pixelation, "
    "no starburst shape, no cross-shape flare, no vanilla minecraft style, "
    "no text watermark, no background detail, centered composition, "
    "high dynamic range with hot white core fading to full black"
)

STYLE_HUD = (
    "transparent RGBA overlay texture for a xianxia game HUD. "
    "CRITICAL: the entire center / empty region must be fully alpha-transparent "
    "(alpha=0, no gray fill, no gradient background). Only painted ink / glow / "
    "decorative pixels carry non-zero alpha. Hand-painted sumi-e / brush / glyph "
    "aesthetic with soft bleeding edges; no rectangular frame, no color tint "
    "when pure black ink is required. Pure black ink on transparent only."
)

PREFIXES: dict[str, str] = {
    "item": STYLE_ITEM,
    "particle": STYLE_PARTICLE,
    "hud": STYLE_HUD,
}


def apply(style: str, body: str, transparent: bool = False) -> str:
    """把画风 prefix 拼到 body 前面。

    - `style` 不在 PREFIXES 中：原样返回 body（允许自定义）
    - body 开头三个词已与 prefix 的开头三个词一致：跳过拼接（手动叠写）
    - item 档 + transparent=True：剔除 body 内手写纯色背景描述，避免
      prompt 文字和 API background=transparent 打架
    - 否则返回 `<prefix> — <body>`
    """
    prefix = PREFIXES.get(style)
    if not prefix:
        return body
    body_stripped = body.strip()
    if transparent and style == "item":
        for bg in (
            "solid black background, ",
            "solid white background, ",
            "solid magenta background, ",
            "solid black background",
            "solid white background",
            "solid magenta background",
        ):
            body_stripped = body_stripped.replace(bg, "")
    prefix_head = " ".join(prefix.split()[:3]).lower()
    body_head = " ".join(body_stripped.split()[:3]).lower()
    if prefix_head == body_head:
        return body_stripped
    return f"{prefix} — {body_stripped}"


def available_styles() -> list[str]:
    return sorted(PREFIXES.keys())
