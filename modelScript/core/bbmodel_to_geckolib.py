#!/usr/bin/env python3
"""bbmodel → GeckoLib 转换器（驱动 web Blockbench 官方导出 codec，非手写）。

为什么这样做：GeckoLib 的 geo/animation 导出 codec 深耦合 Blockbench 运行时
（Project/Cube/Group/THREE 场景图），没法脱开跑；手搓几何/关键帧又反复出坏几何、
糊贴图。这里用 Playwright 无头驱动 web.blockbench.net，装官方 "Geckolib Models &
Animations" plugin，在页面里调它真正的导出 codec，把结果取回来落盘——等价于人在
Blockbench 里 File→Export→GeckoLib，但可脚本化、可复现、可批量。

流程：
  1. headless chromium → web.blockbench.net
  2. 程序化装 geckolib plugin（若未装）
  3. 上传 .bbmodel（走 Open Model 文件选择器）
  4. Formats.geckolib_model.convertTo() 转格式
  5. 猴补 Blockbench.export 截获 compile 输出，触发 export_geckolib_model /
     export_geckolib_animations（确认动画导出对话框）
  6. Bong 后处理：geometry identifier → geometry.<ns>.<name>；动画 key →
     animation.<ns>.<name>.<orig>；负 size cube 归一化（GeckoLib 渲染负 size 会翻面）

依赖：playwright（`pip install playwright`）。chromium 复用 ms-playwright 缓存，
无需 `playwright install`（设 PLAYWRIGHT_BROWSERS_PATH 指向 ~/.cache/ms-playwright）。

用法：
  python3 bbmodel_to_geckolib.py IN.bbmodel --out-dir DIR --name devour_rat [--namespace bong]
  # 产出 DIR/devour_rat.geo.json + DIR/devour_rat.animation.json
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import workspace  # noqa: E402

BLOCKBENCH_URL = "https://web.blockbench.net/"


def _normalize_negative_cubes(geo: dict) -> int:
    """把负 size cube 归一（保持同一空间盒、size 转正）。返回修正数。"""
    fixed = 0
    for g in geo.get("minecraft:geometry", []):
        for bone in g.get("bones", []):
            for c in bone.get("cubes", []):
                o = list(c.get("origin", [0, 0, 0]))
                s = list(c.get("size", [0, 0, 0]))
                changed = False
                for i in range(3):
                    if s[i] < 0:
                        o[i] = o[i] + s[i]
                        s[i] = -s[i]
                        changed = True
                if changed:
                    c["origin"] = [round(v, 4) for v in o]
                    c["size"] = [round(v, 4) for v in s]
                    fixed += 1
    return fixed


def _apply_bong_conventions(geo: dict, anim: dict, namespace: str, name: str) -> None:
    """geometry identifier + animation key 改成 Bong 约定（就地）。"""
    for g in geo.get("minecraft:geometry", []):
        g.setdefault("description", {})["identifier"] = f"geometry.{namespace}.{name}"
    anims = anim.get("animations", {})
    # 取裸动画名再套 Bong 前缀：既支持 bbmodel 里的裸名（idle/walk），也支持 GeckoLib 常见的
    # animation.<model>.<name> 全名（取最后一段 → 不会 double-prefix 成解析不到的 key）。
    anim["animations"] = {
        f"animation.{namespace}.{name}.{k.rsplit('.', 1)[-1]}": v for k, v in anims.items()
    }


def _export_via_blockbench(bbmodel_path: Path, timeout_ms: int) -> tuple[str, str]:
    """驱动 web Blockbench 导出，返回 (geo_json_str, animation_json_str)。"""
    import os

    from playwright.sync_api import sync_playwright

    # 允许指定 chromium 可执行文件（如复用 ms-playwright 缓存里已有的版本，免 `playwright install`
    # 再下一份）。留空则用 playwright 默认解析。
    exe = os.environ.get("BONG_CHROMIUM_PATH") or None

    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True, executable_path=exe)
        page = browser.new_page()
        page.set_default_timeout(timeout_ms)
        page.goto(BLOCKBENCH_URL, wait_until="domcontentloaded")

        # Blockbench 就绪
        page.wait_for_function("() => typeof Blockbench !== 'undefined' && typeof Plugins !== 'undefined'")

        # 等 plugin 注册表异步加载完（fresh 浏览器从 jsdelivr CDN 拉，约几秒；t=0 时 Plugins.all
        # 为空，不等就找不到 geckolib）。
        page.wait_for_function("() => (Plugins.all || []).some(x => x.id === 'geckolib')")

        # 确保 geckolib plugin 已装（提供 geckolib_model 格式 + 导出 action）
        installed = page.evaluate(
            """async () => {
                const p = (Plugins.all || []).find(x => x.id === 'geckolib');
                if (!p) return {ok:false, reason:'plugin_not_in_registry'};
                if (!p.installed) { await p.install(); }
                return {ok:true, installed:p.installed, version:p.version};
            }"""
        )
        if not installed.get("ok"):
            raise RuntimeError(f"geckolib plugin 不可用: {installed}")
        page.wait_for_function("() => !!Formats.geckolib_model && !!BarItems.export_geckolib_model")

        # 打开模型：点 Open Model → 文件选择器上传
        with page.expect_file_chooser() as fc_info:
            page.click(".start_screen_right .button_bar button")
        fc_info.value.set_files(str(bbmodel_path))
        page.wait_for_function("() => typeof Cube !== 'undefined' && Cube.all.length > 0")

        # 转 geckolib_model 格式
        page.evaluate("() => { Formats.geckolib_model.convertTo(); }")
        page.wait_for_function(
            "() => Project.format && Project.format.id === 'geckolib_model'"
        )

        # 猴补 export 截获，触发模型导出
        page.evaluate(
            """() => {
                window.__cap = {};
                window.__origExport = Blockbench.export;
                Blockbench.export = function(options, cb){
                    // 单调计数 key，不按 options.name——万一 model/animation 导出同名不会互相覆盖；
                    // geo vs animation 靠内容分类（见取回处正则），不依赖 name。
                    window.__cap['f' + Object.keys(window.__cap).length] = options && options.content;
                    if (typeof cb === 'function') { try { cb({}); } catch(e){} }
                };
            }"""
        )
        page.evaluate("() => BarItems.export_geckolib_model.trigger()")
        # 动画导出：触发后弹 animation_export 对话框，默认确认（全选动画）
        page.evaluate("() => BarItems.export_geckolib_animations.trigger()")
        page.evaluate(
            """() => {
                if (typeof Dialog !== 'undefined' && Dialog.open && Dialog.open.id === 'animation_export') {
                    Dialog.open.confirm();
                }
            }"""
        )

        result = page.evaluate(
            """() => {
                const cap = window.__cap || {};
                if (window.__origExport) Blockbench.export = window.__origExport;
                let geo=null, anim=null;
                for (const k of Object.keys(cap)) {
                    const c = cap[k] || '';
                    if (/minecraft:geometry/.test(c)) geo = c;
                    else if (/"animations"\\s*:/.test(c)) anim = c;
                }
                return { geo, anim };
            }"""
        )
        browser.close()

    if not result.get("geo") or not result.get("anim"):
        raise RuntimeError(f"导出未截获完整内容: geo={bool(result.get('geo'))} anim={bool(result.get('anim'))}")
    return result["geo"], result["anim"]


def convert(bbmodel: Path, out_dir: Path, name: str, namespace: str, timeout_ms: int) -> tuple[Path, Path]:
    geo_str, anim_str = _export_via_blockbench(bbmodel, timeout_ms)
    geo = json.loads(geo_str)
    anim = json.loads(anim_str)

    fixed = _normalize_negative_cubes(geo)
    _apply_bong_conventions(geo, anim, namespace, name)

    out_dir.mkdir(parents=True, exist_ok=True)
    geo_out = out_dir / f"{name}.geo.json"
    anim_out = out_dir / f"{name}.animation.json"
    geo_out.write_text(json.dumps(geo, indent="\t", ensure_ascii=False))
    anim_out.write_text(json.dumps(anim, indent="\t", ensure_ascii=False))

    bones = sum(len(g.get("bones", [])) for g in geo.get("minecraft:geometry", []))
    anim_keys = list(anim.get("animations", {}).keys())
    print(f"geo   → {geo_out}  (identifier=geometry.{namespace}.{name}, bones={bones}, 负size修正={fixed})")
    print(f"anim  → {anim_out}  (keys={anim_keys})")
    return geo_out, anim_out


def main() -> int:
    ap = argparse.ArgumentParser(description="bbmodel → GeckoLib（驱动 web Blockbench 官方 codec）")
    ap.add_argument("bbmodel", type=Path, help="输入 .bbmodel")
    ap.add_argument("--out-dir", type=Path, required=True, help="输出目录")
    ap.add_argument("--name", required=True, help="模型名（产出 <name>.geo.json / <name>.animation.json）")
    _ns = workspace.default().namespace
    ap.add_argument("--namespace", default=_ns,
                    help=f"命名空间（默认取 workspace 配置：{_ns}）")
    ap.add_argument("--timeout-ms", type=int, default=60000, help="单步等待上限（默认 60s）")
    args = ap.parse_args()

    if not args.bbmodel.is_file():
        print(f"找不到 bbmodel: {args.bbmodel}", file=sys.stderr)
        return 2
    try:
        convert(args.bbmodel, args.out_dir, args.name, args.namespace, args.timeout_ms)
    except Exception as e:  # noqa: BLE001 —— 顶层给清晰错误
        print(f"转换失败: {e}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
