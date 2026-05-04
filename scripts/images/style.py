"""Bong 图像生成的画风 prefix 常量。

四档画风（对齐 local_images/generation_guide.md）：

- **item** —— 物品图标（药材、符牌、武器、法宝等），photorealistic 3D render 风，
  默认透明背景，便于直接接入 icon 资源。
- **particle** —— MC 粒子/VFX 贴图，纯黑 #000000 底，白色/近白形状 + 软羽化，
  生成后走 lum_to_alpha.py 转 alpha 通道。
- **hud** —— HUD overlay 贴图（水墨边框、结霜角、符阵等），真正透明 RGBA，
  中心区域 alpha=0，只有装饰像素有 alpha。生成接口必须传 background=transparent。
- **scene** —— 末法残土美学/构图参考图（concept art / mood board / 场景立绘）。
  非游戏资产，用于美术对齐：苍灰末世调、压抑低彩度、电影构图。不抠图、不接 alpha。

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

STYLE_SCENE = (
    "concept art for a dharma-ending xianxia wasteland (末法残土), "
    "in the visual lineage of Sekiro / Shadow of the Colossus / Zdzisław Beksiński / "
    "Kurosawa monochrome cinematography / Liang Kai sumi-e ink painting. "
    "COMPOSITION RULES (strict): extreme negative space, asymmetric weighting, "
    "subject pinned to one corner or edge with 60-70% of frame deliberately empty, "
    "strong silhouette readability at poster-thumbnail scale, "
    "foreground dark-shape framing where appropriate, deliberate diagonal or vertical tension, "
    "human figure tiny (1-3% of frame height) used purely for scale against monumental ruin. "
    "COLOR DISCIPLINE (strict): predominantly desaturated ash-grey and bone-white, "
    "ONE signature accent color per image (faded jade spirit-qi residue, OR old-blood rust, "
    "OR cinnabar seal-red, OR sickly bile-yellow) used sparingly in ONE area only, "
    "high tonal contrast between dead grey mass and the single accent, no rainbow palette, no warm sunset glow. "
    "MOTIF VOCABULARY: weathered carved stone meridian rings, massive eroded dao characters cut into cliffs, "
    "broken sect archways, colossal half-buried bronze ritual swords, cliff faces honeycombed with abandoned "
    "meditation alcoves, spiral stone platforms descending into negative-pressure voids, fallen stone steles "
    "with mostly-illegible inscriptions, ash-filled cultivation furnaces. "
    "CALLIGRAPHY (when any carved chinese characters appear): always rendered in the style of Mi Fu (米芾) "
    "Song dynasty master — bold dynamic brushwork, eccentric stretched strokes, sharp angular turns, "
    "audacious asymmetric structure, the strokes feel alive and untamed. NEVER use modern regular script "
    "(楷书 kaiti), NEVER use running-regular script (行楷 xingkai), NEVER use printed-looking computer fonts; "
    "the carving must look hand-cut by an eccentric master, not stenciled. "
    "MOOD: long abandonment, depleted ley lines, geological loneliness, the awe of monumental ruin. "
    "EXCLUSIONS (hard): no anime characters, no chibi, no JRPG glow, no celestial palaces in the sky, "
    "no flying swords mid-flight, no glowing palm strikes, no dragons, no goddess figures, "
    "no post-apocalyptic concrete or rebar, no industrial debris, no broken modern road, "
    "no dramatic god rays, no warm sunset, no Death-Stranding generic post-apoc look."
)

PREFIXES: dict[str, str] = {
    "item": STYLE_ITEM,
    "particle": STYLE_PARTICLE,
    "hud": STYLE_HUD,
    "scene": STYLE_SCENE,
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
