"""预览图文字用的字体：带 CJK 字形的 Noto Sans CJK。

PIL 的默认字体没有中文字形，画出来是方块。预览工具的标签和拼图标题都从这里取字体。
字体文件不存在时退回 PIL 默认字体，并在 stderr 报一行警告，中文会显示成方块，
不静默退化。
"""

from __future__ import annotations

import sys

from PIL import ImageFont

CJK_FONT_REGULAR = "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc"


def load_font(size: int):
    try:
        return ImageFont.truetype(CJK_FONT_REGULAR, size)
    except OSError:
        print(
            f"[cjk_font] 找不到 {CJK_FONT_REGULAR}，退回 PIL 默认字体：中文会显示成方块",
            file=sys.stderr,
        )
        return ImageFont.load_default()
