#!/usr/bin/env python3
"""经脉工厂共享接口件重构前后几何一致性严格比对测试"""

import json
from pathlib import Path

MODEL_DIR = Path("modelScript/models/meridian_factory")
BASELINE_FILE = Path("modelScript/tests/baseline_signatures.json")

def get_elements_geometry(bbmodel_path: Path):
    if not bbmodel_path.exists():
        return []
    data = json.loads(bbmodel_path.read_text(encoding="utf-8"))
    elements = data.get("elements", [])
    res = []
    for e in elements:
        res.append({
            "name": e["name"],
            "from": [round(x, 4) for x in e["from"]],
            "to": [round(x, 4) for x in e["to"]],
            "rotation": [round(x, 4) for x in e.get("rotation", [0, 0, 0])],
            "origin": [round(x, 4) for x in e.get("origin", [0, 0, 0])],
        })
    res.sort(key=lambda x: (x["from"], x["to"], x["name"]))
    return res

def run_verification():
    with open(BASELINE_FILE) as f:
        baselines = json.load(f)

    all_passed = True
    print("==================================================================")
    print("经脉工厂生成器接入 common_parts.py 几何一致性比对报告")
    print("==================================================================")

    for m, base_elements in baselines.items():
        cur_path = MODEL_DIR / m
        cur_elements = get_elements_geometry(cur_path)
        base_elements_geom = []
        for e in base_elements:
            base_elements_geom.append({
                "name": e["name"],
                "from": e["from"],
                "to": e["to"],
                "rotation": e["rotation"],
                "origin": e["origin"],
            })
        base_elements_geom.sort(key=lambda x: (x["from"], x["to"], x["name"]))

        if len(base_elements_geom) != len(cur_elements):
            print(f"❌ {m}: 元素数量不一致! 基线 {len(base_elements_geom)} vs 当前 {len(cur_elements)}")
            all_passed = False
            continue

        diffs = []
        for i, (eb, ec) in enumerate(zip(base_elements_geom, cur_elements)):
            if eb != ec:
                diffs.append(f"  [{i}] {eb['name']}: {eb} vs {ec}")

        if diffs:
            print(f"❌ {m}: 发现 {len(diffs)} 处几何不一致:\n" + "\n".join(diffs[:5]))
            all_passed = False
        else:
            print(f"✓ {m:26s}: 全部 {len(cur_elements):2d} 个元素几何坐标 100% 严格一致 (0 几何差异)!")

    print("==================================================================")
    if all_passed:
        print("🎉 恭喜！重构后全部模型与基线版本几何完全严格一致！")
    else:
        print("❌ 存在几何差异，请检查！")
    print("==================================================================")
    return all_passed

if __name__ == "__main__":
    success = run_verification()
    if not success:
        sys.exit(1)
