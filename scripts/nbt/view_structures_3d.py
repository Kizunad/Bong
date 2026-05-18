#!/usr/bin/env python3
"""Export 丹宗遗园 structures to an interactive Three.js HTML viewer.

Usage:
    python3 scripts/nbt/view_structures_3d.py
    # Opens: scripts/nbt/dan_zong_viewer.html
"""

from __future__ import annotations

import json
import os
import random
import sys
import webbrowser

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from nbt.gen_dan_zong_structures import STRUCTURES

BLOCK_COLORS: dict[str, str] = {
    "minecraft:stone_bricks": "#7a7a7a",
    "minecraft:mossy_stone_bricks": "#5a7a52",
    "minecraft:cracked_stone_bricks": "#6a6a5a",
    "minecraft:mossy_cobblestone": "#4a6a42",
    "minecraft:cobblestone": "#8a8a8a",
    "minecraft:polished_blackstone": "#2a2a30",
    "minecraft:polished_blackstone_bricks": "#35353d",
    "minecraft:polished_blackstone_slab": "#303038",
    "minecraft:polished_blackstone_stairs": "#303038",
    "minecraft:chiseled_polished_blackstone": "#3a3a44",
    "minecraft:chiseled_stone_bricks": "#6e6e6e",
    "minecraft:purple_terracotta": "#6a4874",
    "minecraft:purple_stained_glass": "#7b2fbe88",
    "minecraft:magenta_stained_glass": "#c354cd88",
    "minecraft:podzol": "#5a4a2a",
    "minecraft:gravel": "#9a9090",
    "minecraft:soul_sand": "#4a3a2a",
    "minecraft:soul_soil": "#3a2a1a",
    "minecraft:water": "#3366cc88",
    "minecraft:cauldron": "#444444",
    "minecraft:campfire": "#cc6622",
    "minecraft:soul_campfire": "#22aacc",
    "minecraft:cobweb": "#dddddd44",
    "minecraft:bone_block": "#e8dcc4",
    "minecraft:skeleton_skull": "#d4c8a8",
    "minecraft:flower_pot": "#8b4513",
    "minecraft:dead_bush": "#8b7355",
    "minecraft:dark_oak_planks": "#3a2a1a",
    "minecraft:dark_oak_slab": "#3a2a1a",
    "minecraft:iron_bars": "#aaaaaa44",
    "minecraft:chain": "#666666",
    "minecraft:soul_lantern": "#44bbcc",
    "minecraft:deepslate_bricks": "#3a3a3e",
    "minecraft:cracked_stone_bricks": "#6a6a5a",
}
DEFAULT_COLOR = "#ff00ff"


def extract_blocks(builder_fn, name: str) -> list[dict]:
    random.seed(42 + hash(name) % 10000)
    sb = builder_fn()
    blocks = []
    for b in sb._blocks:
        pos = b["pos"]
        state_id = b["state"]
        palette_entry = sb._palette[state_id]
        block_name = palette_entry["Name"]
        color = BLOCK_COLORS.get(block_name, DEFAULT_COLOR)
        blocks.append({"x": pos[0], "y": pos[1], "z": pos[2], "c": color, "n": block_name})
    return blocks


def generate_html(structures_data: dict[str, dict]) -> str:
    data_json = json.dumps(structures_data)
    return f"""<!DOCTYPE html>
<html lang="zh">
<head>
<meta charset="utf-8">
<title>丹宗遗园 NBT 3D Viewer</title>
<style>
  * {{ margin: 0; padding: 0; box-sizing: border-box; }}
  body {{ background: #1a1a2e; color: #eee; font-family: monospace; overflow: hidden; }}
  #ui {{ position: fixed; top: 10px; left: 10px; z-index: 10; }}
  #ui select {{ font-size: 14px; padding: 4px 8px; background: #2a2a3e; color: #eee; border: 1px solid #555; }}
  #info {{ position: fixed; bottom: 10px; left: 10px; z-index: 10; font-size: 12px; color: #aaa; }}
  #hover {{ position: fixed; top: 10px; right: 10px; z-index: 10; font-size: 12px; background: #2a2a3e99; padding: 6px 10px; border-radius: 4px; }}
  canvas {{ display: block; }}
</style>
</head>
<body>
<div id="ui">
  <select id="structSelect"></select>
</div>
<div id="info">拖拽旋转 | 滚轮缩放 | 右键平移</div>
<div id="hover" id="hoverInfo"></div>

<script type="importmap">
{{
  "imports": {{
    "three": "https://cdn.jsdelivr.net/npm/three@0.169.0/build/three.module.js",
    "three/addons/": "https://cdn.jsdelivr.net/npm/three@0.169.0/examples/jsm/"
  }}
}}
</script>

<script type="module">
import * as THREE from 'three';
import {{ OrbitControls }} from 'three/addons/controls/OrbitControls.js';

const DATA = {data_json};
const structNames = Object.keys(DATA);

// Setup
const scene = new THREE.Scene();
scene.background = new THREE.Color(0x1a1a2e);
scene.fog = new THREE.Fog(0x1a1a2e, 80, 200);

const camera = new THREE.PerspectiveCamera(60, innerWidth / innerHeight, 0.1, 500);
const renderer = new THREE.WebGLRenderer({{ antialias: true }});
renderer.setSize(innerWidth, innerHeight);
renderer.setPixelRatio(devicePixelRatio);
document.body.appendChild(renderer.domElement);

const controls = new OrbitControls(camera, renderer.domElement);
controls.enableDamping = true;
controls.dampingFactor = 0.08;

// Lights
scene.add(new THREE.AmbientLight(0x404050, 1.5));
const dirLight = new THREE.DirectionalLight(0xffeedd, 1.2);
dirLight.position.set(30, 50, 20);
scene.add(dirLight);
const fillLight = new THREE.DirectionalLight(0x8888cc, 0.4);
fillLight.position.set(-20, 10, -30);
scene.add(fillLight);

// Grid
const grid = new THREE.GridHelper(100, 100, 0x333355, 0x222244);
scene.add(grid);

let currentGroup = null;
const hoverEl = document.getElementById('hover');

// Raycaster for hover
const raycaster = new THREE.Raycaster();
const mouse = new THREE.Vector2();

function parseColor(c) {{
  if (c.length === 9) {{ // #RRGGBBAA
    return {{ color: parseInt(c.slice(1,7), 16), opacity: parseInt(c.slice(7,9), 16) / 255 }};
  }}
  return {{ color: parseInt(c.slice(1), 16), opacity: 1.0 }};
}}

function loadStructure(name) {{
  if (currentGroup) scene.remove(currentGroup);
  const group = new THREE.Group();
  const entry = DATA[name];
  const blocks = entry.blocks;
  const size = entry.size;

  // Center offset
  const ox = -size[0] / 2, oy = 0, oz = -size[2] / 2;

  const geo = new THREE.BoxGeometry(0.95, 0.95, 0.95);

  // Batch by color for performance
  const byColor = {{}};
  for (const b of blocks) {{
    const key = b.c;
    if (!byColor[key]) byColor[key] = [];
    byColor[key].push(b);
  }}

  for (const [colorStr, blist] of Object.entries(byColor)) {{
    const {{ color, opacity }} = parseColor(colorStr);
    const mat = new THREE.MeshLambertMaterial({{
      color,
      transparent: opacity < 1,
      opacity,
    }});
    const mesh = new THREE.InstancedMesh(geo, mat, blist.length);
    const dummy = new THREE.Object3D();
    blist.forEach((b, i) => {{
      dummy.position.set(b.x + ox + 0.5, b.y + oy + 0.5, b.z + oz + 0.5);
      dummy.updateMatrix();
      mesh.setMatrixAt(i, dummy.matrix);
    }});
    mesh.userData.blocks = blist;
    group.add(mesh);
  }}

  scene.add(group);
  currentGroup = group;

  // Camera position
  const maxDim = Math.max(size[0], size[1], size[2]);
  camera.position.set(size[0] * 0.8 + ox, size[1] * 1.5, size[2] * 1.2 + oz);
  controls.target.set(ox + size[0] / 2, size[1] / 3, oz + size[2] / 2);
  controls.update();

  document.getElementById('info').textContent =
    `${{name}} | ${{size[0]}}×${{size[1]}}×${{size[2]}} | ${{blocks.length}} blocks | 拖拽旋转 | 滚轮缩放`;
}}

// Populate selector
const select = document.getElementById('structSelect');
for (const name of structNames) {{
  const opt = document.createElement('option');
  opt.value = name;
  const s = DATA[name].size;
  opt.textContent = `${{name}} (${{s[0]}}×${{s[1]}}×${{s[2]}})`;
  select.appendChild(opt);
}}
select.addEventListener('change', () => loadStructure(select.value));

// Mouse hover
renderer.domElement.addEventListener('mousemove', (e) => {{
  mouse.x = (e.clientX / innerWidth) * 2 - 1;
  mouse.y = -(e.clientY / innerHeight) * 2 + 1;
}});

// Load first (大殿)
const defaultStruct = structNames.find(n => n.includes('great_hall')) || structNames[0];
select.value = defaultStruct;
loadStructure(defaultStruct);

// Resize
addEventListener('resize', () => {{
  camera.aspect = innerWidth / innerHeight;
  camera.updateProjectionMatrix();
  renderer.setSize(innerWidth, innerHeight);
}});

// Animate
function animate() {{
  requestAnimationFrame(animate);
  controls.update();

  // Hover raycast
  if (currentGroup) {{
    raycaster.setFromCamera(mouse, camera);
    const hits = raycaster.intersectObjects(currentGroup.children, true);
    if (hits.length > 0) {{
      const hit = hits[0];
      const mesh = hit.object;
      if (mesh.isInstancedMesh && mesh.userData.blocks) {{
        const idx = hit.instanceId;
        const b = mesh.userData.blocks[idx];
        if (b) {{
          hoverEl.textContent = `${{b.n}} @ (${{b.x}}, ${{b.y}}, ${{b.z}})`;
        }}
      }}
    }} else {{
      hoverEl.textContent = '';
    }}
  }}

  renderer.render(scene, camera);
}}
animate();
</script>
</body>
</html>"""


def main():
    structures_data = {}
    for name, builder_fn, expected_size in STRUCTURES:
        blocks = extract_blocks(builder_fn, name)
        structures_data[name] = {
            "blocks": blocks,
            "size": list(expected_size),
        }
        print(f"  {name}: {len(blocks)} blocks")

    out_path = os.path.join(os.path.dirname(__file__), "dan_zong_viewer.html")
    html = generate_html(structures_data)
    with open(out_path, "w", encoding="utf-8") as f:
        f.write(html)
    print(f"\nSaved: {out_path}")
    print(f"Open in browser to view.")

    try:
        webbrowser.open(f"file://{os.path.abspath(out_path)}")
    except Exception:
        pass


if __name__ == "__main__":
    main()
