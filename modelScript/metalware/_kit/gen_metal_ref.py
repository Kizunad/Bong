#!/usr/bin/env python3
"""旧时金属器（青铜 / 铁）参考图与爆炸图：走本机 CPA 的 gpt-image-2.5-sunburst。

与末土装备的参考图风格相反：这批器物是做工精致规整的凡物，不是捡来缝补的粗物。
``--kind ref`` 出正视 + 侧视（正交、同一取景，用来和模型做等高比例对照）；``--kind exploded`` 出分件图。

用法::

    python3 modelScript/metalware/_kit/gen_metal_ref.py qing_tong_fang_ding --kind ref --prompt "..."
"""

from __future__ import annotations

import argparse
import os
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO / "modelScript" / "tools"))
from gen_exploded_ref import cpa_client  # noqa: E402

REVIEW_ROOT = Path(os.environ.get(
    "BONG_MODEL_REVIEW_DIR", "/home/serverkizuna/Code/Bong/.agent-worktrees/model-review"))

REF_STYLE = (
    "museum-quality studio reference photograph of an ancient Chinese bronze artifact, exquisite and orderly casting, "
    "symmetrical, finely proportioned, aged patina (red-brown copper body, blue-green verdigris patches, "
    "polished bright copper where hands wore it), dull and lifeless ornament bands (no glow, no magic), "
    "shown as two strict orthographic views side by side on one image: left = front elevation, right = side elevation, "
    "both views at identical scale, same camera height, object standing on the same ground line, "
    "plain neutral grey background (#777777), even soft lighting, no perspective distortion, "
    "no text, no watermark, no hands, no mannequin"
)

EXPLODED_STYLE = (
    "technical exploded breakdown schematic of an ancient Chinese bronze artifact, exquisite casting, "
    "components separated with clear spacing floating in midair (body, legs, handles, rim, ornament bands, inner parts), "
    "aged patina: red-brown copper, blue-green verdigris patches, bright polished copper wear, "
    "plain neutral grey background (#777777), soft shadow, no text, no callout labels, no watermark"
)

STYLES = {"ref": REF_STYLE, "exploded": EXPLODED_STYLE}


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("item_id")
    ap.add_argument("--prompt", required=True, help="这件器物的外观与分件描述（英文）")
    ap.add_argument("--kind", choices=sorted(STYLES), required=True)
    ap.add_argument("--out", type=Path, default=None)
    args = ap.parse_args()

    out = args.out or REVIEW_ROOT / "img" / "metalware" / args.item_id
    out.mkdir(parents=True, exist_ok=True)
    image = cpa_client().text_to_image(f"{STYLES[args.kind]}. Subject: {args.prompt}", size="1536x1024")
    target = out / f"{args.item_id}_{args.kind}.png"
    target.write_bytes(image)
    print(f"{args.item_id}: {args.kind} → {target}")


if __name__ == "__main__":
    main()
