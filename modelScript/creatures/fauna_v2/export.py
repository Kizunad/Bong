#!/usr/bin/env python3
"""v2 重做生物：绑定稿 → 离线 codec → client 资源树。

顺序：``gen_rig.py`` 写绑定稿 → ``exporters/creature_codec.cjs`` 导出 GeckoLib geo →
校验骨引用 / 尺寸 / 贴图尺寸 → 写进 client。贴图直接取终审 bbmodel 的内嵌 PNG，原字节不动。

只安装 geo 和贴图；动画由同目录 ``gen_anim.py`` 另行生成安装。

ash_spider_v2 / skull_fiend_v2 的 geo 早先是手写导出的（朝 +Z、枢轴不在关节、x 未镜像、
顶底面 UV 未翻），这里用同一条 codec 路径重导出并覆盖；两者当时都还没接进游戏。

用法::

    python3 modelScript/creatures/fauna_v2/export.py            # 四种都出
    python3 modelScript/creatures/fauna_v2/export.py --only fuya_v2
"""

from __future__ import annotations

import argparse
import base64
import io
import json
import math
import shutil
import subprocess
from pathlib import Path

from PIL import Image

import gen_rig

ROOT = Path(__file__).resolve().parents[3]
CODEC = ROOT / "modelScript" / "exporters" / "creature_codec.cjs"
STAGING = ROOT / "modelScript" / "out" / "fauna_v2"
CLIENT = ROOT / "client" / "src" / "main" / "resources" / "assets" / "bong"


def embedded_png(bbmodel: dict, name: str) -> bytes:
    textures = bbmodel.get("textures", [])
    if len(textures) != 1:
        raise ValueError(f"{name}: 需要恰好一张内嵌图集，实际 {len(textures)} 张")
    source = textures[0].get("source", "")
    prefix = "data:image/png;base64,"
    if not source.startswith(prefix):
        raise ValueError(f"{name}: 贴图不是内嵌 PNG，不能依赖作者机器上的路径")
    return base64.b64decode(source[len(prefix):], validate=True)


def verify(name: str, geo_path: Path, png: bytes, expected_bones: int) -> None:
    """骨名唯一、父骨存在、cube 尺寸非负有限、UV 分辨率与图集一致。"""

    model = json.loads(geo_path.read_text(encoding="utf-8"))["minecraft:geometry"][0]
    description = model["description"]
    if description["identifier"] != f"geometry.bong.{name}":
        raise ValueError(f"{name}: geometry identifier 是 {description['identifier']}")
    with Image.open(io.BytesIO(png)) as image:
        if [description["texture_width"], description["texture_height"]] != list(image.size):
            raise ValueError(f"{name}: UV 分辨率与内嵌图集尺寸不一致")
    names = [bone["name"] for bone in model["bones"]]
    if len(set(names)) != len(names) or len(names) != expected_bones:
        raise ValueError(f"{name}: 骨数 {len(names)} 与骨表 {expected_bones} 不符或有重名")
    for bone in model["bones"]:
        if bone.get("parent") is not None and bone["parent"] not in names:
            raise ValueError(f"{name}: 骨 {bone['name']} 的父骨 {bone['parent']} 不存在")
        for cube in bone.get("cubes", []):
            if any(not math.isfinite(value) or value < 0 for value in cube["size"]):
                raise ValueError(f"{name}: 骨 {bone['name']} 有非法 cube 尺寸 {cube['size']}")


def export(name: str) -> None:
    source, rig, bones = gen_rig.load_species(name)
    rig_path = gen_rig.write_rig(name)

    STAGING.mkdir(parents=True, exist_ok=True)
    subprocess.run(["node", str(CODEC), str(rig_path), str(STAGING), name], check=True)
    png = embedded_png(source, name)
    verify(name, STAGING / f"{name}.geo.json", png, len(bones))

    (CLIENT / "geo").mkdir(parents=True, exist_ok=True)
    shutil.copyfile(STAGING / f"{name}.geo.json", CLIENT / "geo" / f"{name}.geo.json")
    fauna = CLIENT / "textures" / "entity" / "fauna"
    fauna.mkdir(parents=True, exist_ok=True)
    (fauna / f"{name}.png").write_bytes(png)
    print(f"{name}: geo + 贴图已安装（{len(bones)} 根骨）")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--only", choices=tuple(gen_rig.SPECIES), action="append")
    args = parser.parse_args()
    for name in args.only or gen_rig.SPECIES:
        export(name)


if __name__ == "__main__":
    main()
