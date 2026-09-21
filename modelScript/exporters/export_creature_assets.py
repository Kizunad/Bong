#!/usr/bin/env python3
"""把生物流水线的最终绑定稿导出为 GeckoLib，校验后安装到 client。

先 --prepare 生成作者模型，再 --export 导出到 out/client-creatures，最后 --install。
默认使用官方浏览器 codec；--offline 复用仓内 Bedrock codec（cuboid/数值线性关键帧）。
呆怒狮直接读 handmade 手改 Rig；各物种的内部骨架/肌肉层不作为运行时模型。
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import io
import json
import math
import shutil
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
MODELS = ROOT / "modelScript" / "models"
CREATURES = ROOT / "modelScript" / "creatures"
STAGING = ROOT / "modelScript" / "out" / "client-creatures"
CLIENT = ROOT / "client" / "src" / "main" / "resources" / "assets" / "bong"


@dataclass(frozen=True)
class Asset:
    creature: str
    name: str
    model: str
    idle: str
    walk: str | None = None
    run: str | None = None

    @property
    def source(self) -> Path:
        return MODELS / self.model


ASSETS = (
    Asset("stitched_beast", "hybrid_beast", "stitched_beast/BeastAnim_7.bbmodel",
          "beast_idle", "beast_walk"),
    Asset("stitched_beast", "hybrid_beast_core", "stitched_beast/StitchedBeastCoreRig.bbmodel",
          "core_idle", "core_crawl"),
    Asset("stitched_beast", "hybrid_beast_shard", "stitched_beast/StitchedBeastShardRig.bbmodel",
          "shard_idle", "shard_crawl"),
    Asset("mimic_spider", "ash_spider", "mimic_spider/MimicSpiderRig.bbmodel",
          "idle", "walk", "run"),
    Asset("dainu_lion", "dainu_lion", "handmade/DainuLionRig.bbmodel", "idle", "walk", "run"),
    Asset("kekeda_goose", "kekeda_goose", "kekeda_goose/KekedaGooseRig.bbmodel",
          "idle", "walk", "run"),
    Asset("fuyu_vulture", "fuyu_vulture", "fuyu_vulture/FuyuVultureRigMid.bbmodel",
          "idle", "walk", "run"),
    Asset("fuyu_vulture", "fuyu_vulture_flight", "fuyu_vulture/FuyuVultureRigMidFlight.bbmodel",
          "glide", "flap"),
    Asset("horse", "horse", "horse/HorsePelt_rust_medium.bbmodel", "idle", "walk", "gallop"),
    *(Asset("legacy_fauna", name, f"legacy_fauna/{name}Rig.bbmodel", "idle", "walk")
      for name in ("void_distorted", "daoxiang", "zhinian", "tsy_sentinel", "fuya", "skull_fiend")),
)

# 每种生物在独立进程里运行，避免它们同名的 rig / gen_skeleton 模块互相污染。
RECIPES = {
    "legacy_fauna": (("gen_anim.py",),),
    "stitched_beast": (
        ("gen_core.py",), ("core_anim.py",),
        ("gen_fragment.py",), ("fragment_anim.py",),
        ("gen_beast.py", "--seed", "7"), ("beast_anim.py", "--seed", "7"),
    ),
    "mimic_spider": (("gen_frame.py",), ("gen_shell.py",), ("gen_anim.py",)),
    "dainu_lion": (),
    "kekeda_goose": (("gen_plume.py",), ("gen_anim.py",)),
    "fuyu_vulture": (
        ("gen_skeleton.py", "--size", "mid"),
        ("gen_skeleton.py", "--size", "mid", "--pose", "spread"),
        ("gen_pelt.py", "--size", "mid", "--morph", "jin"),
        ("gen_pelt.py", "--size", "mid", "--morph", "jin", "--pose", "spread"),
        ("gen_anim.py", "--size", "mid", "--morph", "jin"),
    ),
    "horse": (
        ("gen_skeleton.py", "--profile", "medium"),
        ("gen_muscle.py", "--profile", "medium"),
        ("gen_pelt.py", "--profile", "medium"),
        ("gen_anim.py", "--profile", "medium"),
    ),
}


def source_info(asset: Asset) -> tuple[dict, bytes, dict]:
    """校验骨架与动画的引用；保留真实源文件摘要，避免手改稿被重生成替换。"""
    raw = asset.source.read_bytes()
    doc = json.loads(raw)
    groups = {group["uuid"]: group for group in doc.get("groups", [])}

    def visit(nodes):
        for node in nodes:
            if isinstance(node, dict):
                group = groups.get(node.get("uuid"), node)
                groups[node["uuid"]] = group
                visit(node.get("children", []))

    visit(doc.get("outliner", []))
    animations = {}
    for animation in doc.get("animations", []):
        name = animation["name"].rsplit(".", 1)[-1]
        if name in animations:
            raise ValueError(f"{asset.name}: 重复动画名 {name}")
        for uuid, animator in animation.get("animators", {}).items():
            if animator.get("type", "bone") == "bone" and uuid not in groups:
                raise ValueError(f"{asset.name}/{name}: 动画引用不存在的骨 {uuid}")
        animations[name] = {
            "length": animation["length"],
            "loop": animation.get("loop") == "loop",
        }
    for name in (asset.idle, asset.walk, asset.run):
        if name is not None and name not in animations:
            raise ValueError(f"{asset.name}: 缺少控制器需要的 {name} 动画")
    textures = doc.get("textures", [])
    if len(textures) != 1:
        raise ValueError(f"{asset.name}: 需要一张已合并的纹理图集，实际 {len(textures)} 张")
    source = textures[0].get("source", "")
    prefix = "data:image/png;base64,"
    if not source.startswith(prefix):
        raise ValueError(f"{asset.name}: 缺少内嵌 PNG，不能依赖作者机器的绝对路径")
    png = base64.b64decode(source[len(prefix):], validate=True)
    with Image.open(io.BytesIO(png)) as texture:
        size = texture.size
    report = {
        "source": str(asset.source.relative_to(ROOT)),
        "source_sha256": hashlib.sha256(raw).hexdigest(),
        "bones": len(groups),
        "cubes": len(doc["elements"]),
        "texture_size": list(size),
        "idle": asset.idle,
        "walk": asset.walk,
        "run": asset.run,
        "animations": animations,
    }
    return doc, png, report


def verify_export(asset: Asset, directory: Path) -> dict:
    """所有轨道必须找到真实骨骼；贴图尺寸、循环入口和导出的动作集必须匹配。"""
    _, png, report = source_info(asset)
    geometry = json.loads((directory / f"{asset.name}.geo.json").read_text())
    animation = json.loads((directory / f"{asset.name}.animation.json").read_text())
    entries = geometry["minecraft:geometry"]
    if len(entries) != 1:
        raise ValueError(f"{asset.name}: 运行时要求单一 geometry")
    model = entries[0]
    desc = model["description"]
    if desc["identifier"] != f"geometry.bong.{asset.name}":
        raise ValueError(f"{asset.name}: geometry identifier 不匹配")
    if [desc["texture_width"], desc["texture_height"]] != report["texture_size"]:
        raise ValueError(f"{asset.name}: 导出 UV 分辨率与内嵌 PNG 尺寸不一致")
    bones = {bone["name"] for bone in model["bones"]}
    if len(bones) != len(model["bones"]):
        raise ValueError(f"{asset.name}: 重复骨名会导致动画绑定歧义")
    for bone in model["bones"]:
        if bone.get("parent") is not None and bone["parent"] not in bones:
            raise ValueError(f"{asset.name}: 骨 {bone['name']} 的父骨不存在")
        for cube in bone.get("cubes", []):
            if any(not math.isfinite(value) or value < 0 for value in cube["size"]):
                raise ValueError(f"{asset.name}: 非法 cube size {cube['size']}")
    clips = animation["animations"]
    expected = {f"animation.bong.{asset.name}.{name}" for name in report["animations"]}
    if set(clips) != expected:
        raise ValueError(f"{asset.name}: 导出丢失或增加了动画：{set(clips) ^ expected}")
    for name, clip in clips.items():
        missing = set(clip.get("bones", {})) - bones
        if missing:
            raise ValueError(f"{name}: 轨道引用不存在的骨 {sorted(missing)}")
        if not math.isfinite(clip.get("animation_length", 0)) or clip.get("animation_length", 0) <= 0:
            raise ValueError(f"{name}: 动画时长必须是有限正数")
    for name in (asset.idle, asset.walk, asset.run):
        if name and clips[f"animation.bong.{asset.name}.{name}"].get("loop") is not True:
            raise ValueError(f"{asset.name}/{name}: 常驻或移动动画必须循环")
    if (directory / f"{asset.name}.png").read_bytes() != png:
        raise ValueError(f"{asset.name}: 暂存贴图不是源模型的图集")
    return report


def prepare(creature: str) -> None:
    for step in RECIPES[creature]:
        script = CREATURES / creature / step[0]
        subprocess.run([sys.executable, str(script), *step[1:]], cwd=ROOT, check=True)


def export(asset: Asset, directory: Path, *, offline: bool = False) -> None:
    _, texture, _ = source_info(asset)
    if offline:
        subprocess.run([
            "node", str(Path(__file__).with_name("creature_codec.cjs")),
            str(asset.source), str(directory), asset.name,
        ], check=True)
    else:
        from bbmodel_maker.workbench.bbmodel_to_geckolib import convert

        convert(asset.source, directory, asset.name, "bong", 60000)
    (directory / f"{asset.name}.png").write_bytes(texture)
    verify_export(asset, directory)


def install(assets: list[Asset], directory: Path, client: Path = CLIENT) -> None:
    # 整批验证完才写 client，避免后半批失败时先替换了前半批运行时资产。
    for asset in assets:
        verify_export(asset, directory)
    for asset in assets:
        for suffix, folder in (("geo.json", "geo"), ("animation.json", "animations"),
                               ("png", "textures/entity/fauna")):
            destination = client / folder / f"{asset.name}.{suffix}"
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(directory / destination.name, destination)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--creature", choices=tuple(RECIPES), action="append")
    parser.add_argument("--prepare", action="store_true", help="运行已有流水线，保留 handmade 源")
    parser.add_argument("--export", action="store_true", help="通过官方 codec 导出并验证")
    parser.add_argument("--offline", action="store_true", help="复用本仓离线 codec，只支持数值线性关键帧")
    parser.add_argument("--install", action="store_true", help="整批验证通过后写入 client")
    parser.add_argument("--out-dir", type=Path, default=STAGING)
    args = parser.parse_args()
    selected = args.creature or list(RECIPES)
    assets = [asset for asset in ASSETS if asset.creature in selected]
    args.out_dir.mkdir(parents=True, exist_ok=True)
    try:
        if args.prepare:
            for creature in selected:
                prepare(creature)
        reports = {}
        for asset in assets:
            _, _, reports[asset.name] = source_info(asset)
            if args.export:
                export(asset, args.out_dir, offline=args.offline)
            report = reports[asset.name]
            print(f"{asset.name}: {report['bones']} 骨 / {report['cubes']} 块 / "
                  f"{len(report['animations'])} 动作", flush=True)
        (args.out_dir / "sources.json").write_text(
            json.dumps(reports, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
        if args.install:
            install(assets, args.out_dir)
    except (OSError, ValueError, subprocess.CalledProcessError, RuntimeError) as error:
        print(f"生物导出失败：{error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
