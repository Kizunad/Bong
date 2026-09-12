#!/usr/bin/env python3
"""在真实 Blockbench 中预览，并把骨骼静态旋转烘焙给软渲染器。"""

from __future__ import annotations

import argparse
import base64
import json
from pathlib import Path

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parents[3]
OUT = ROOT / "modelScript/out/baolongwang"

BAKE = """() => {
    Animator.showDefaultPose(); Canvas.updateAll(); scene.updateMatrixWorld(true);
    const doc = Codecs.project.compile({raw:true,bitmaps:true,absolute_paths:false});
    const lookup = new Map(Cube.all.map(c => [c.uuid,c]));
    const bounds = new THREE.Box3();
    for (const e of doc.elements) {
        const cube = lookup.get(e.uuid);
        const mesh = cube.mesh;
        mesh.geometry.computeBoundingBox();
        const position = new THREE.Vector3(), scale = new THREE.Vector3(), q = new THREE.Quaternion();
        mesh.matrixWorld.decompose(position,q,scale);
        const local = mesh.geometry.boundingBox;
        e.from = local.min.clone().multiply(scale).add(position).toArray();
        e.to = local.max.clone().multiply(scale).add(position).toArray();
        e.origin = position.toArray();
        const rot = new THREE.Euler().setFromQuaternion(q,'ZYX');
        e.rotation = [rot.x,rot.y,rot.z].map(v=>v*180/Math.PI);
        bounds.union(new THREE.Box3().setFromObject(mesh));
    }
    delete doc.animations;
    doc.groups = [];
    doc.outliner = doc.elements.map(e=>e.uuid);
    return {doc,bounds:{min:bounds.min.toArray(),max:bounds.max.toArray()},
            version:Blockbench.version};
}"""


def preview(model: Path, output: Path, chromium: str | None, animations: bool) -> None:
    from playwright.sync_api import sync_playwright

    output.mkdir(parents=True, exist_ok=True)
    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(headless=True, executable_path=chromium)
        try:
            page = browser.new_page(viewport={"width": 1440, "height": 1080})
            page.goto("https://web.blockbench.net/", wait_until="domcontentloaded")
            page.wait_for_function("() => typeof Codecs !== 'undefined' && !!Codecs.project")
            page.evaluate("doc => Codecs.project.load(doc,{path:'Baolongwang.bbmodel'})", json.loads(model.read_text()))
            page.wait_for_function("() => Texture.all[0]?.img.complete && Texture.all[0].img.naturalWidth > 0")
            page.evaluate("() => {if(Dialog.open) Dialog.open.hide(); Modes.options.edit.select();}")
            baked = page.evaluate(BAKE)
            path = output / (model.stem + "_posed.bbmodel")
            path.write_text(json.dumps(baked["doc"], ensure_ascii=False) + "\n")
            print(json.dumps({"version": baked["version"], "bounds": baked["bounds"], "baked": str(path)}), flush=True)
            views = {
                "front": ([0, 86, -350], [0, 86, -20]),
                "three_quarter": ([190, 140, -290], [0, 85, -8]),
                "torso": ([70, 98, -148], [0, 88, -25]),
                "furnace": ([15, 91, -89], [0, 86, -33]),
            }
            for name, (camera, target) in views.items():
                page.evaluate("v => {const p=Preview.selected; p.setProjectionMode(false); p.camPers.position.fromArray(v.camera); p.controls.target.fromArray(v.target); p.controls.update(); p.render();}", {"camera": camera, "target": target})
                data = page.evaluate("() => new Promise(resolve=>Preview.selected.screenshot({width:1000,height:1000},resolve))")
                image_path = output / f"{model.stem}_{name}.png"
                image_path.write_bytes(base64.b64decode(data.split(",", 1)[1]))
                with Image.open(image_path) as image:
                    pixels = np.asarray(image.convert("RGBA"))
                    solid = pixels[:, :, 3] > 200
                    if solid.sum() < 5000 or pixels[solid, :3].std() < 5:
                        raise ValueError(f"Blockbench 视图为空或贴图未加载：{name}")
                    print(f"{name}: {solid.sum()} visible pixels", flush=True)
            if animations:
                report = page.evaluate("""() => {
                    Modes.options.animate.select();
                    const body = Group.all.find(g=>g.name==='bdk_body');
                    const attachments = Group.all.filter(g=>g.name==='organ_furnace'||g.name.endsWith('_upper_arm'));
                    const results=[];
                    for(const animation of Animation.all) {
                        animation.select();
                        const frames=[];
                        for(let i=0;i<=8;i++) {
                            Timeline.time=animation.length*i/8; Animator.preview(); scene.updateMatrixWorld(true);
                            const inverse=body.mesh.matrixWorld.clone().invert();
                            frames.push(attachments.map(g=>{
                                const p=g.mesh.getWorldPosition(new THREE.Vector3()).applyMatrix4(inverse);
                                return {name:g.name,local:p.toArray()};
                            }));
                        }
                        results.push({name:animation.name,length:animation.length,frames});
                    }
                    return results;
                }""")
                for animation in report:
                    samples = np.array([[entry["local"] for entry in frame] for frame in animation["frames"]])
                    error = float(np.max(np.abs(samples - samples[0])))
                    if not np.isfinite(samples).all() or error > 1e-5:
                        raise ValueError(f"附件脱离躯干：{animation['name']}，误差 {error}")
                    print(f"{animation['name']}: 9 frames, attachment drift={error:.3g}", flush=True)
                (output / f"{model.stem}_animation_attachment_checks.json").write_text(json.dumps(report, indent=2))
        finally:
            browser.close()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("model", type=Path)
    parser.add_argument("--out", type=Path, default=OUT)
    parser.add_argument("--chromium")
    parser.add_argument("--animations", action="store_true")
    args = parser.parse_args()
    preview(args.model, args.out, args.chromium, args.animations)
