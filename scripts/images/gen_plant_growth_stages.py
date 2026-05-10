#!/usr/bin/env python3
"""Generate seedling/growing botany icon variants from canonical plant icons."""

from __future__ import annotations

import argparse
from pathlib import Path
from typing import Iterable

from PIL import Image


ROOT = Path(__file__).resolve().parents[2]
BOTANY_DIR = ROOT / "client/src/main/resources/assets/bong-client/textures/gui/botany"
ITEMS_DIR = ROOT / "client/src/main/resources/assets/bong-client/textures/gui/items"
OUT_DIR = BOTANY_DIR / "stages"

PLANT_IDS = (
    "ci_she_hao",
    "ning_mai_cao",
    "hui_yuan_zhi",
    "chi_sui_cao",
    "gu_yuan_gen",
    "kong_shou_hen",
    "jie_gu_rui",
    "yang_jing_tai",
    "qing_zhuo_cao",
    "an_shen_guo",
    "shi_mai_gen",
    "ling_yan_shi_zhi",
    "ye_ku_teng",
    "hui_jin_tai",
    "zhen_jie_zi",
    "shao_hou_man",
    "tian_nu_jiao",
    "fu_you_hua",
    "wu_yan_guo",
    "hei_gu_jun",
    "fu_chen_cao",
    "zhong_yan_teng",
    "fu_yuan_jue",
    "bai_yan_peng",
    "duan_ji_ci",
    "xue_se_mai_cao",
    "yun_ding_lan",
    "xuan_gen_wei",
    "ying_yuan_gu",
    "xuan_rong_tai",
    "yuan_ni_hong_yu",
    "jing_xin_zao",
    "xue_po_lian",
    "jiao_mai_teng",
    "lie_yuan_tai",
    "ming_gu_gu",
    "bei_wen_zhi",
    "ling_jing_xu",
    "mao_xin_wei",
)

STAGES = {
    "seedling": (0.42, 0.62, (140, 220, 150)),
    "growing": (0.72, 0.86, (115, 235, 150)),
}


def source_path(plant_id: str) -> Path:
    botany = BOTANY_DIR / f"{plant_id}.png"
    if botany.exists():
        return botany
    return ITEMS_DIR / f"{plant_id}.png"


def tint_icon(img: Image.Image, tint_rgb: tuple[int, int, int], mix: float) -> Image.Image:
    tinted = Image.new("RGBA", img.size, (*tint_rgb, 0))
    tinted.putalpha(img.getchannel("A"))
    return Image.blend(img, tinted, mix)


def stage_variant(src: Image.Image, scale: float, alpha_scale: float, tint_rgb: tuple[int, int, int]) -> Image.Image:
    base = src.convert("RGBA")
    width, height = base.size
    new_size = (max(1, round(width * scale)), max(1, round(height * scale)))
    resized = base.resize(new_size, Image.Resampling.LANCZOS)
    resized = tint_icon(resized, tint_rgb, 0.18)
    alpha = resized.getchannel("A").point(lambda value: round(value * alpha_scale))
    resized.putalpha(alpha)

    canvas = Image.new("RGBA", base.size, (0, 0, 0, 0))
    paste_x = (width - new_size[0]) // 2
    paste_y = height - new_size[1] - max(2, round(height * 0.08))
    canvas.alpha_composite(resized, (paste_x, paste_y))
    return canvas


def expected_outputs() -> Iterable[tuple[Path, Image.Image]]:
    for plant_id in PLANT_IDS:
        src_path = source_path(plant_id)
        if not src_path.exists():
            raise FileNotFoundError(f"missing source icon for {plant_id}: {src_path}")
        with Image.open(src_path) as img:
            for stage, (scale, alpha, tint) in STAGES.items():
                yield OUT_DIR / f"{plant_id}_{stage}.png", stage_variant(img, scale, alpha, tint)


def same_png(path: Path, image: Image.Image) -> bool:
    if not path.exists():
        return False
    with Image.open(path) as existing:
        return existing.convert("RGBA").tobytes() == image.convert("RGBA").tobytes()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true", help="fail if generated stage icons are missing or stale")
    args = parser.parse_args()

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    stale: list[Path] = []
    for path, image in expected_outputs():
        if same_png(path, image):
            continue
        stale.append(path)
        if not args.check:
            image.save(path)

    if stale:
        for path in stale:
            print(f"{'stale' if args.check else 'wrote'} {path.relative_to(ROOT)}")
        return 1 if args.check else 0
    print(f"ok {len(PLANT_IDS) * len(STAGES)} stage icons")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
