#!/usr/bin/env python3
"""用 Blockbench 官方 Bedrock codec 还原原模型及五条动画。"""

from __future__ import annotations

import argparse
import base64
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
SOURCE = ROOT / "modelScript/models/baolongwang"
BASE = SOURCE / "BaolongwangBase.bbmodel"


def import_source(output: Path, chromium: str | None = None) -> None:
    from playwright.sync_api import sync_playwright

    if output.exists():
        raise FileExistsError(f"已有作者稿，禁止覆盖：{output}")
    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(headless=True, executable_path=chromium)
        try:
            page = browser.new_page()
            page.goto("https://web.blockbench.net/", wait_until="domcontentloaded")
            page.wait_for_function("() => typeof Codecs !== 'undefined' && !!Codecs.bedrock")
            page.evaluate(
                "geo => { newProject(Formats.bedrock); Codecs.bedrock.parse(geo); }",
                json.loads((SOURCE / "baolongwang.geo.json").read_text()),
            )
            texture = "data:image/png;base64," + base64.b64encode(
                (SOURCE / "baolongwang.png").read_bytes()
            ).decode()
            page.evaluate(
                "src => { const tex = new Texture({name:'baolongwang.png'}).fromDataURL(src).add(); tex.apply(Cube.all); }",
                texture,
            )
            page.wait_for_function("() => Texture.all[0]?.img.complete && Texture.all[0].img.naturalWidth > 0")
            page.evaluate(
                "content => AnimationCodec.codecs.bedrock.loadFile({name:'baolongwang.animation.json',path:'baolongwang.animation.json',content})",
                (SOURCE / "baolongwang.animation.json").read_text(),
            )
            model = page.evaluate(
                "() => {Project.name='BaolongwangBase'; return Codecs.project.compile({raw:true,bitmaps:true,absolute_paths:false});}"
            )
            if len(model["elements"]) != 281 or len(model["animations"]) != 5:
                raise ValueError("原资产导入不完整，请检查 Blockbench codec")
            output.parent.mkdir(parents=True, exist_ok=True)
            output.write_text(json.dumps(model, ensure_ascii=False, indent=2) + "\n")
        finally:
            browser.close()
    print(output)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, default=BASE)
    parser.add_argument("--chromium", help="可选：已有 Chromium 的可执行文件")
    args = parser.parse_args()
    import_source(args.out, args.chromium)
