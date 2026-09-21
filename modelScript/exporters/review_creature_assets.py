"""制作人工复核用六视角对照和动画 GIF；读取实际导出的 client 资源，不给外观打分。

复用 bbmodel-contact-sheet 的 framing/contact_sheet，额外传入骨树矩阵，保留腐羽鹫的
静态羽骨旋转。manifest 必须由人写；不存在时在每张表上明确标为待补。
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import shutil
import tempfile

import numpy as np
from bbmodel_maker.render import framing
from bbmodel_maker.render.render_bbmodel import render
from bbmodel_maker.rig.animkit import Pose, PoseRig

from export_creature_assets import ASSETS, CLIENT, ROOT, STAGING, verify_export
from import_creature_geo import import_geo


def sample(profile, rig, seconds):
    pose = Pose()
    for bone, channels in profile.get("bones", {}).items():
        for channel, keys in channels.items():
            if isinstance(keys, list):
                value = np.asarray(keys, float)
            else:
                frames = sorted((float(t), np.asarray(value, float)) for t, value in keys.items())
                value = frames[0][1]
                for index, (time, point) in enumerate(frames):
                    if seconds < time:
                        if index:
                            prev_t, prev = frames[index - 1]
                            value = prev + (point - prev) * (seconds - prev_t) / (time - prev_t)
                        break
                    value = point
            value = value.copy()
            if channel in ("rotation", "position"): value[0] *= -1
            if channel == "rotation": value[1] *= -1
            setattr(pose[bone], {"rotation": "rot", "position": "pos", "scale": "scale"}[channel], value)
    return pose


def focus(rig, poses, views):
    points = []
    for pose in poses:
        world = rig.world(pose)
        for bone in rig.order:
            vertices = rig.bone_points(bone)
            if len(vertices): points.extend(vertices @ world[bone][:3, :3].T + world[bone][:3, 3])
    points = np.asarray(points)
    center = (points.min(0) + points.max(0)) / 2
    span = max(float(np.ptp((points - center) @ framing.view_matrix(view).T, axis=0)[:2].max()) for view in views)
    return center, span * 1.08


def self_test(asset):
    """在真实导出副本注入缺陷，校验器必须分辨正常和损坏资产。"""
    verify_export(asset, STAGING)
    with tempfile.TemporaryDirectory() as temporary:
        folder = Path(temporary)
        for suffix in ("geo.json", "animation.json", "png"):
            shutil.copyfile(STAGING / f"{asset.name}.{suffix}", folder / f"{asset.name}.{suffix}")
        geo_path = folder / f"{asset.name}.geo.json"
        anim_path = folder / f"{asset.name}.animation.json"
        png_path = folder / f"{asset.name}.png"
        for path, mutation, expected in (
            (geo_path, "negative", "非法 cube size"),
            (anim_path, "missing_bone", "不存在的骨"),
            (png_path, "wrong_texture", "不是源模型的图集"),
        ):
            original = path.read_bytes()
            if mutation == "wrong_texture":
                path.write_bytes(b"not the embedded PNG")
            else:
                doc = json.loads(original)
                if mutation == "negative":
                    cube = next(b["cubes"][0] for b in doc["minecraft:geometry"][0]["bones"] if b.get("cubes"))
                    cube["size"][0] = -1
                else:
                    next(iter(doc["animations"].values()))["bones"]["__missing_bone__"] = {}
                path.write_text(json.dumps(doc))
            try:
                verify_export(asset, folder)
            except ValueError as error:
                if expected not in str(error): raise
            else:
                raise AssertionError(f"{asset.name}: {mutation} 注入未检出")
            path.write_bytes(original)


def review(asset, out, size):
    self_test(asset)
    geometry = json.loads((CLIENT / "geo" / f"{asset.name}.geo.json").read_text())
    texture = (CLIENT / "textures/entity/fauna" / f"{asset.name}.png").read_bytes()
    current = out / f"{asset.name}.bbmodel"
    current.write_text(json.dumps(import_geo(geometry, texture, asset.name)))
    rig = PoseRig(current)
    clips = json.loads((CLIENT / "animations" / f"{asset.name}.animation.json").read_text())["animations"]
    idle = clips[f"animation.bong.{asset.name}.{asset.idle}"]
    idle_pose = sample(idle, rig, 0)
    views = framing.views_for("-z")
    camera = focus(rig, [idle_pose], views)
    previous = out / "previous" / f"{asset.name}.bbmodel"
    if previous.exists():
        previous_label = "PREV client model"
    elif asset.creature == "legacy_fauna":
        previous = ROOT / "modelScript/creatures/legacy_fauna/sources" / f"{asset.name}.bbmodel"
        previous_label = "PREV before grouping"
    else:
        previous = asset.source
        previous_label = "AUTHOR input (bind pose)"
    old = PoseRig(previous)
    tiles = []
    for view in views:
        tiles.append((f"NOW {view.label}", render(current, yaw=view.yaw, pitch=view.pitch,
                      size=size, focus=camera, xform=rig.element_xform(idle_pose))[0]))
        tiles.append((f"{previous_label} {view.label}", render(previous, yaw=view.yaw, pitch=view.pitch,
                      size=size, focus=camera, xform=old.element_xform(Pose()))[0]))
    notes = ["MANIFEST: pending human-authored feature checklist (no appearance approval)",
             "EXPORT GATES: valid baseline; 3/3 injected defects detected (bone / size / PNG)",
             "Offline textured preview of installed assets; Minecraft visual test still pending"]
    framing.contact_sheet(tiles, title=asset.name + " | same camera | -z facing", notes=notes, columns=2).save(out / f"contact_{asset.name}.png")

    motion = {"ash_spider": "ambush_burst", "hybrid_beast_core": "core_split",
              "fuyu_vulture_flight": "flap"}.get(asset.name, asset.walk or asset.idle)
    clip = clips[f"animation.bong.{asset.name}.{motion}"]
    times = np.linspace(0, clip["animation_length"], 16, endpoint=not clip.get("loop", False))
    poses = [sample(clip, rig, float(t)) for t in times]
    view = framing.view_by_name("-z", "3/4")
    camera = focus(rig, poses, [view])
    frames = [render(current, yaw=view.yaw, pitch=view.pitch, size=size,
                     focus=camera, xform=rig.element_xform(pose))[0] for pose in poses]
    frames[0].save(out / f"motion_{asset.name}.gif", save_all=True, append_images=frames[1:],
                   duration=max(40, round(clip["animation_length"] * 1000 / len(frames))), loop=0)
    print(f"{asset.name}: 六视角 + {motion} + 差分 3/3", flush=True)
    return tiles[8][1]


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--creature", action="append")
    parser.add_argument("--size", type=int, default=240)
    args = parser.parse_args()
    out = STAGING / "review"
    out.mkdir(parents=True, exist_ok=True)
    assets = [asset for asset in ASSETS if not args.creature or asset.creature in args.creature]
    tiles = [(asset.name, review(asset, out, args.size)) for asset in assets]
    framing.contact_sheet(tiles, title="Client creature integration | Round 2 review pending", columns=3,
                           notes=["Each model has a six-view comparison and a motion GIF in this folder.",
                                  "No human manifest supplied. Review required before final polish."]).save(out / "overview.png")
    rows = "\n".join(f'<tr><td>{a.name}</td><td><a href="contact_{a.name}.png">六视角对比</a></td><td><img src="motion_{a.name}.gif" width="240"></td></tr>' for a in assets)
    (out / "index.html").write_text('<!doctype html><meta charset="utf-8"><title>生物资源验收</title>'
        '<style>body{background:#161719;color:#ddd;font:16px sans-serif}a{color:#9ce}td{padding:12px}</style>'
        '<h1>生物资源接线 · 待人工复核</h1><p>离线贴图预览。尚无人工特征清单，尚未在 Minecraft 实机验收。</p>'
        '<p>每张六视角图使用同一取景与输入/旧模型对照；差分检测覆盖骨绑定、方块尺寸、贴图完整性。</p>'
        '<img src="overview.png" width="900"><table>'+rows+'</table>', encoding="utf-8")
