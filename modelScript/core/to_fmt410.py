#!/usr/bin/env python3
"""bbmodel format_version 5.0 → 4.10 降级。

为什么要它：Blockbench 5.0 把骨骼定义从 outliner 里拆了出来单放 `groups[]`，
outliner 只留 `{uuid, children}` 引用；4.x 的读盘器期望骨骼对象**内联**在 outliner
里，拿到 5.0 文件时读不出骨骼，模型进不了场景。表现就是"打开了但一个 cube 都看不见"。
同目录下 4.10 的文件（脚本生成的骨架/肌肉层）却完全正常——这个对比是判定依据。

降级做三件事：把 groups 内联回 outliner、去掉 5.0 才有的根字段和元素/贴图字段、
把 format_version 改回 4.10。几何、UV、贴图、动画关键帧一律原样保留。

用法:
  python3 modelScript/core/to_fmt410.py IN.bbmodel [OUT.bbmodel]
  python3 modelScript/core/to_fmt410.py --check IN.bbmodel      # 只报告，不写
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

# 5.0 才有的根字段；4.x 读盘器不认，留着没意义
ROOT_DROP = ("groups", "multi_file_ruleset", "timeline_setups", "unhandled_root_fields",
             "variable_placeholder_buttons", "variable_placeholders", "visible_box")
# 4.10 的贴图条目形状（对照 modelScript/models/DainuLionMuscle.bbmodel）
TEX_KEEP = ("path", "name", "folder", "namespace", "id", "particle", "render_mode",
            "visible", "mode", "saved", "uuid", "source")
ELEM_DROP = ("export", "scope")
GROUP_DROP = ("_static", "scope", "export", "primary_selected")


def downgrade(doc: dict) -> tuple[dict, dict]:
    meta = doc.get("meta", {})
    stats = {"format_in": meta.get("format_version"), "groups_inlined": 0,
             "elements": len(doc.get("elements", [])), "animations": len(doc.get("animations", []))}

    gdefs = {g["uuid"]: g for g in doc.get("groups", [])}

    def inline(node):
        """outliner 的 uuid 引用 → 内联的骨骼对象（元素引用是裸字符串，原样透传）。"""
        if isinstance(node, str):
            return node
        g = gdefs.get(node.get("uuid"))
        out = dict(g) if g else dict(node)
        if g:
            stats["groups_inlined"] += 1
        out["children"] = [inline(c) for c in node.get("children", [])]
        for k in GROUP_DROP:
            out.pop(k, None)
        return out

    new = {k: v for k, v in doc.items() if k not in ROOT_DROP}
    new["outliner"] = [inline(r) for r in doc.get("outliner", [])]
    new["meta"] = {**meta, "format_version": "4.10"}
    new["elements"] = [{k: v for k, v in e.items() if k not in ELEM_DROP} for e in doc.get("elements", [])]
    new["textures"] = [
        {**{k: t[k] for k in TEX_KEEP if k in t}, "path": t.get("path", ""), "mode": t.get("mode", "bitmap")}
        for t in doc.get("textures", [])
    ]
    return new, stats


def ensure_410(doc: dict) -> dict:
    """写盘前的兜底：已经是 4.x 就原样返回，是 5.0 就就地降级。

    生成器普遍是"读一份已有 bbmodel、加东西、写回去"，格式版本因此是**跟着源文件走**
    的。源文件一旦被 Blockbench 5 手工存过盘（骨架/皮毛层都可能），产物就悄悄变成 5.0，
    而 5.0 在 4.x 里打开是一个 cube 都看不见 —— 这种错不会报任何异常，只会在下次打开
    模型时才发现。所以每个写盘口都过这一道，别指望源文件一直是对的。
    """
    return doc if doc.get("meta", {}).get("format_version") != "5.0" else downgrade(doc)[0]


def main() -> int:
    ap = argparse.ArgumentParser(description="bbmodel 5.0 → 4.10 降级")
    ap.add_argument("src", type=Path)
    ap.add_argument("dst", type=Path, nargs="?")
    ap.add_argument("--check", action="store_true", help="只报告格式，不写文件")
    args = ap.parse_args()

    doc = json.loads(args.src.read_text())
    fv = doc.get("meta", {}).get("format_version")
    if args.check:
        print(f"{args.src.name}: format_version={fv}  groups={len(doc.get('groups', []))}  "
              f"elements={len(doc.get('elements', []))}")
        return 0
    if fv != "5.0":
        print(f"{args.src.name} 已是 {fv}，无需降级", file=sys.stderr)
        return 1

    new, stats = downgrade(doc)
    dst = args.dst or args.src.with_name(args.src.stem + "_bb4.bbmodel")
    dst.write_text(json.dumps(new, ensure_ascii=False))
    print(f"{args.src.name} ({stats['format_in']}) → {dst.name} (4.10)  "
          f"内联骨骼 {stats['groups_inlined']}  元素 {stats['elements']}  动画 {stats['animations']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
