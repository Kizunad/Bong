#!/usr/bin/env python3
"""Batch-generate Bong inventory item icons from server item TOML definitions."""

from __future__ import annotations

import argparse
import hashlib
import math
import subprocess
import sys
import tomllib
from dataclasses import dataclass
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parents[1]
DEFAULT_ITEMS_ROOT = REPO_ROOT / "server" / "assets" / "items"
DEFAULT_OUT_DIR = (
    REPO_ROOT
    / "client"
    / "src"
    / "main"
    / "resources"
    / "assets"
    / "bong-client"
    / "textures"
    / "gui"
    / "items"
)

PLAN_BATCH_IDS = (
    "bone_coin_5",
    "bone_coin_15",
    "bone_coin_40",
    "shu_gu",
    "zhu_gu",
    "feng_he_gu",
    "yi_shou_gu",
    "bian_yi_hexin",
    "fu_ya_hesui",
    "zhen_shi_chu",
    "xuan_iron",
    "kaimai_dan",
    "ningmai_powder",
    "huiyuan_pill",
    "life_extension_pill",
    "anti_spirit_pressure_pill",
    "hoe_iron",
    "hoe_lingtie",
    "hoe_xuantie",
    "cai_yao_dao",
    "bao_chu",
    "cao_lian",
    "dun_qi_jia",
    "gua_dao",
    "gu_hai_qian",
    "bing_jia_shou_tao",
    "rusted_blade",
    "spirit_sword",
    "skill_scroll_herbalism_baicao_can",
    "skill_scroll_alchemy_danhuo_can",
    "skill_scroll_forging_duantie_can",
    "alchemy_recipe_fragment",
    "blueprint_scroll_iron_sword",
    "blueprint_scroll_qing_feng",
    "blueprint_scroll_ling_feng",
    "inscription_scroll_sharp_v0",
    "inscription_scroll_qi_amplify_v0",
    "array_flag",
    "scattered_qi_pearl",
    "zhen_shi_zhong",
    "zhen_shi_gao",
    "anqi_shanggu_bone",
    "anqi_shanggu_bone_charged",
)


@dataclass(frozen=True)
class ItemSpec:
    item_id: str
    name: str
    category: str
    source_path: Path
    rarity: str


def load_items(items_root: Path) -> dict[str, ItemSpec]:
    items: dict[str, ItemSpec] = {}
    for path in sorted(items_root.rglob("*.toml")):
        data = tomllib.loads(path.read_text(encoding="utf-8"))
        for raw in data.get("item", []):
            item_id = str(raw.get("id", "")).strip()
            name = str(raw.get("name", item_id)).strip()
            if not item_id:
                continue
            items[item_id] = ItemSpec(
                item_id=item_id,
                name=name or item_id,
                category=str(raw.get("category", "misc")).strip() or "misc",
                source_path=path,
                rarity=str(raw.get("rarity", "common")).strip() or "common",
            )
    return items


def prompt_for(item: ItemSpec) -> str:
    return f"{item.name}，末法残土风格，暗色调水墨，透明背景，64×64 icon"


def parse_ids(values: list[str]) -> list[str]:
    ids: list[str] = []
    for value in values:
        ids.extend(part.strip() for part in value.split(",") if part.strip())
    return ids


def selected_items(
    all_items: dict[str, ItemSpec],
    out_dir: Path,
    ids: list[str],
    include_all_missing: bool,
    overwrite: bool,
) -> list[ItemSpec]:
    wanted = sorted(all_items) if include_all_missing else (ids or list(PLAN_BATCH_IDS))
    missing_ids = [item_id for item_id in wanted if item_id not in all_items]
    if missing_ids:
        raise SystemExit("unknown item ids: " + ", ".join(missing_ids))

    result: list[ItemSpec] = []
    for item_id in wanted:
        spec = all_items[item_id]
        if overwrite or not (out_dir / f"{item_id}.png").exists():
            result.append(spec)
    return result


def gen_command(item: ItemSpec, out_dir: Path, backend: str) -> list[str]:
    return [
        sys.executable,
        str(SCRIPT_DIR / "gen.py"),
        prompt_for(item),
        "--name",
        item.item_id,
        "--style",
        "item",
        "--transparent",
        "--backend",
        backend,
        "--out",
        str(out_dir),
        "--save-prompt",
    ]


def run_generation(items: list[ItemSpec], out_dir: Path, backend: str) -> None:
    out_dir.mkdir(parents=True, exist_ok=True)
    for item in items:
        subprocess.run(gen_command(item, out_dir, backend), check=True)


def render_placeholder_icons(items: list[ItemSpec], out_dir: Path) -> None:
    try:
        from PIL import Image, ImageDraw, ImageFilter, ImageFont
    except ImportError as exc:
        raise SystemExit("placeholder mode requires Pillow") from exc

    out_dir.mkdir(parents=True, exist_ok=True)
    font = load_font(ImageFont)
    for item in items:
        seed = int(hashlib.sha256(item.item_id.encode("utf-8")).hexdigest()[:8], 16)
        hue = seed % 360
        base = hsl(hue, 0.48, 0.36)
        glow = hsl((hue + 35) % 360, 0.72, 0.62)

        img = Image.new("RGBA", (128, 128), (0, 0, 0, 0))
        glow_layer = Image.new("RGBA", (128, 128), (0, 0, 0, 0))
        draw_glow = ImageDraw.Draw(glow_layer)
        draw_glow.ellipse((28, 28, 100, 100), fill=(*glow, 86))
        img = Image.alpha_composite(img, glow_layer.filter(ImageFilter.GaussianBlur(9)))

        draw = ImageDraw.Draw(img)
        shape = shape_for(item)
        if shape == "coin":
            draw.ellipse((34, 30, 94, 98), fill=(*base, 232), outline=(*glow, 255), width=4)
            draw.arc((44, 40, 84, 88), 25, 330, fill=(235, 215, 160, 230), width=3)
        elif shape == "scroll":
            draw.rounded_rectangle((32, 22, 92, 104), radius=10, fill=(92, 70, 54, 230), outline=(*glow, 255), width=3)
            draw.line((44, 38, 80, 38), fill=(210, 190, 150, 230), width=3)
            draw.line((44, 58, 82, 58), fill=(190, 160, 120, 220), width=2)
            draw.line((44, 78, 74, 78), fill=(190, 160, 120, 220), width=2)
        elif shape == "tool":
            draw.polygon((62, 16, 76, 22, 42, 106, 30, 100), fill=(*base, 235), outline=(*glow, 255))
            draw.line((38, 94, 86, 46), fill=(230, 220, 190, 225), width=5)
        elif shape == "weapon":
            draw.polygon((66, 12, 78, 72, 66, 112, 54, 72), fill=(*base, 235), outline=(*glow, 255))
            draw.line((48, 82, 84, 82), fill=(220, 190, 130, 240), width=5)
        elif shape == "pill":
            draw.ellipse((38, 36, 90, 88), fill=(*base, 235), outline=(*glow, 255), width=4)
            draw.arc((48, 46, 80, 78), 210, 35, fill=(255, 245, 190, 220), width=3)
        else:
            points = rough_polygon(seed)
            draw.polygon(points, fill=(*base, 235), outline=(*glow, 255))
            draw.line(points[0] + points[len(points) // 2], fill=(240, 230, 190, 180), width=2)

        label = item.name[:1] if item.name else item.item_id[:1].upper()
        draw.text((64, 64), label, font=font, anchor="mm", fill=(245, 238, 210, 235))
        img.save(out_dir / f"{item.item_id}.png")


def shape_for(item: ItemSpec) -> str:
    item_id = item.item_id
    if "coin" in item_id:
        return "coin"
    if "scroll" in item_id or "fragment" in item_id:
        return "scroll"
    if item.category == "tool" or item_id.startswith("hoe_"):
        return "tool"
    if item.category == "weapon" or "sword" in item_id or "blade" in item_id:
        return "weapon"
    if item.category == "pill" or "pill" in item_id or "dan" in item_id:
        return "pill"
    return "stone"


def hsl(hue: int, sat: float, light: float) -> tuple[int, int, int]:
    import colorsys

    r, g, b = colorsys.hls_to_rgb(hue / 360.0, light, sat)
    return int(r * 255), int(g * 255), int(b * 255)


def rough_polygon(seed: int) -> tuple[tuple[int, int], ...]:
    points: list[tuple[int, int]] = []
    for i in range(8):
        angle = math.pi * 2.0 * i / 8.0
        radius = 34 + ((seed >> (i * 3)) & 0x0F)
        points.append((64 + int(math.cos(angle) * radius), 64 + int(math.sin(angle) * radius)))
    return tuple(points)


def load_font(image_font_module):
    for path in (
        "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    ):
        candidate = Path(path)
        if candidate.exists():
            return image_font_module.truetype(str(candidate), 30)
    return image_font_module.load_default()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--items-root", type=Path, default=DEFAULT_ITEMS_ROOT)
    parser.add_argument("--out", type=Path, default=DEFAULT_OUT_DIR)
    parser.add_argument("--ids", action="append", default=[], help="Comma-separated or repeated item ids")
    parser.add_argument("--all-missing", action="store_true", help="Generate every item without a texture")
    parser.add_argument("--overwrite", action="store_true")
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--placeholder", action="store_true", help="Render deterministic offline placeholders")
    parser.add_argument("--backend", default="auto", choices=["auto", "cliproxy", "openai"])
    args = parser.parse_args()

    items = load_items(args.items_root)
    selected = selected_items(items, args.out, parse_ids(args.ids), args.all_missing, args.overwrite)

    if args.dry_run:
        for item in selected:
            print(item.item_id, item.name, "=>", prompt_for(item))
        return

    if args.placeholder:
        render_placeholder_icons(selected, args.out)
    else:
        run_generation(selected, args.out, args.backend)

    print(f"generated {len(selected)} item icons into {args.out}")


if __name__ == "__main__":
    main()
