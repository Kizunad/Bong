#!/usr/bin/env python3
"""生成带图片快照的离线 PNG 审图库，不修改输入资源。"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

from gen_item_batch import DEFAULT_ITEMS_ROOT, DEFAULT_OUT_DIR, REPO_ROOT, load_items
from PIL import Image

IMAGE_FIELDS = ("old", "input", "cutout", "icon", "mask")
TEMPLATE = Path(__file__).with_name("review_icons.html")


def scan_images(source: Path, items_root: Path) -> list[dict]:
    items = load_items(items_root)
    rows = []
    for path in sorted(source.rglob("*")):
        if not path.is_file() or path.suffix.lower() != ".png":
            continue
        item = items.get(path.stem)
        try:
            resource_path = path.relative_to(REPO_ROOT).as_posix()
        except ValueError:
            resource_path = path.relative_to(source).as_posix()
        rows.append(
            {
                "id": f"I{len(rows) + 1:03}",
                "name": item.name if item else path.stem,
                "description": item.description if item else "",
                "path": resource_path,
                "icon": str(path),
            }
        )
    return rows


def read_manifest(path: Path) -> list[dict]:
    data = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(data, dict) or not isinstance(data.get("images"), list):
        raise TypeError("清单必须包含 images 数组")
    return data["images"]


def prepare_rows(rows: list[dict], source_base: Path) -> list[dict]:
    if not rows:
        raise ValueError("没有可审查的 PNG 图片")
    paths, ids = set(), set()
    prepared = []
    for row in rows:
        if not isinstance(row, dict):
            raise TypeError("每张图片必须是一个对象")
        for key in ("id", "path", "icon"):
            if not isinstance(row.get(key), str) or not row[key].strip():
                raise ValueError(f"图片缺少非空字符串字段 {key}")
        if row["id"] in ids or row["path"] in paths:
            raise ValueError(f"重复的图片编号或资源路径：{row['id']} / {row['path']}")
        ids.add(row["id"])
        paths.add(row["path"])
        result = {key: row[key] for key in ("id", "path")}
        for key, default in (("name", row["id"]), ("note", ""), ("description", "")):
            result[key] = str(row.get(key, default))
        result["redraw"] = row.get("redraw") is True
        result["remove_background"] = row.get("remove_background") is True
        warnings = row.get("warnings", [])
        if not isinstance(warnings, list):
            raise TypeError(f"{row['id']} 的 warnings 必须是数组")
        result["warnings"] = [str(w) for w in warnings]
        result["group"] = (
            "redraw"
            if result["redraw"]
            else "background"
            if result["remove_background"]
            else "current"
        )
        for key in IMAGE_FIELDS:
            if row.get(key) is None:
                continue
            if not isinstance(row[key], str) or not row[key]:
                raise ValueError(f"{row['id']} 的 {key} 必须是本地 PNG 路径")
            path = (source_base / row[key]).resolve(strict=True)
            if path.suffix.lower() != ".png":
                raise ValueError(f"仅支持 PNG：{path}")
            result[key] = path
        prepared.append(result)
    return prepared


def build_review(rows: list[dict], source_base: Path, output: Path, title: str) -> dict:
    prepared = prepare_rows(rows, source_base)
    # 快照目录不可覆盖，避免历史审图内容和浏览器中的标记悄悄换成另一批图片。
    output.mkdir(parents=True, exist_ok=False)
    assets = output / "assets"
    assets.mkdir()
    snapshots = {}

    def snapshot(path: Path) -> dict:
        if path in snapshots:
            return snapshots[path]
        content = path.read_bytes()
        digest = hashlib.sha256(content).hexdigest()
        target = assets / f"{digest}.png"
        target.write_bytes(content)
        with Image.open(target) as image:
            if image.format != "PNG":
                raise ValueError(f"文件并非 PNG：{path}")
            image.load()
            alpha = image.convert("RGBA").getchannel("A").getextrema()
            info = {
                "src": f"assets/{digest}.png",
                "thumb": f"assets/{digest}-thumb.png",
                "width": image.width,
                "height": image.height,
                "alpha": "empty"
                if alpha[1] == 0
                else "opaque"
                if alpha[0] == 255
                else "transparent",
            }
            image.thumbnail((280, 280), Image.Resampling.LANCZOS)
            image.save(output / info["thumb"])
        snapshots[path] = info
        return info

    images = []
    for row in prepared:
        result = {key: value for key, value in row.items() if key not in IMAGE_FIELDS}
        for key in IMAGE_FIELDS:
            if key in row:
                info = snapshot(row[key])
                result[key] = info["src"]
                result[key + "Thumb"] = info["thumb"]
                if key == "icon":
                    result["size"] = [info["width"], info["height"]]
                    result["alpha"] = info["alpha"]
                    if info["alpha"] == "empty":
                        result["warnings"].append("图标完全透明")
        images.append(result)
    # 资源路径与内容决定标记归属；顺序和标题变化不会丢标记，新图片也不会继承旧结论。
    identity = sorted(
        (r["path"], *(r.get(key, "") for key in IMAGE_FIELDS)) for r in images
    )
    dataset_id = hashlib.sha256(
        json.dumps(identity, ensure_ascii=False).encode()
    ).hexdigest()
    data = {"version": 1, "datasetId": dataset_id, "title": title, "images": images}
    encoded = json.dumps(data, ensure_ascii=False, indent=2)
    (output / "review-data.json").write_text(encoded + "\n", encoding="utf-8")
    template = TEMPLATE.read_text(encoding="utf-8")
    (output / "index.html").write_text(
        template.replace("__DATA__", encoded.replace("<", "\\u003c")), encoding="utf-8"
    )
    return data


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    inputs = parser.add_mutually_exclusive_group()
    inputs.add_argument(
        "--source", type=Path, help="递归扫描 PNG；默认客户端物品图标目录"
    )
    inputs.add_argument("--manifest", type=Path, help="含 images 数组的 JSON 对照清单")
    parser.add_argument(
        "--items-root", type=Path, default=DEFAULT_ITEMS_ROOT, help="物品 TOML 目录"
    )
    parser.add_argument(
        "--out", type=Path, required=True, help="新建的离线图库目录，不覆盖旧图库"
    )
    parser.add_argument("--title", default="Bong 图标审查")
    args = parser.parse_args()
    try:
        if args.manifest:
            manifest = args.manifest.resolve(strict=True)
            rows, base = read_manifest(manifest), manifest.parent
        else:
            source = (args.source or DEFAULT_OUT_DIR).resolve(strict=True)
            if not source.is_dir():
                raise ValueError(f"PNG 输入必须是目录：{source}")
            if args.out.resolve().is_relative_to(source):
                raise ValueError(
                    "输出目录必须在输入图片目录之外，避免把旧图库再次扫描进去"
                )
            rows, base = scan_images(source, args.items_root.resolve()), source
        data = build_review(rows, base, args.out.resolve(), args.title)
    except (OSError, ValueError, TypeError, Image.DecompressionBombError) as exc:
        parser.exit(1, f"生成失败：{exc}\n")
    print(f"{len(data['images'])} 张图片：{args.out.resolve() / 'index.html'}")


if __name__ == "__main__":
    main()
