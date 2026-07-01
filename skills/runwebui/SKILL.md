---
name: runwebui
description: 一键打开 Bong 模块图谱 webui（module-map/index.html，全模块第一性原理拆解的可浏览总览）。触发方式：/runwebui、run webui、打开模块图谱、看模块 map、module map。
allowed-tools: Bash
---

# 打开模块图谱 webui

模块图谱是 server / client / agent 三层全模块的细颗粒度拆解总览，单文件自包含 HTML，`file://` 直接打开。

执行：

```bash
bash scripts/runwebui.sh
```

脚本会在 WSL 下用 `wslpath -w` 转成 Windows 路径并用 `explorer.exe` 调默认浏览器打开；原生 Linux/macOS 走 `xdg-open`/`open`。

> 维护：图谱数据嵌在 `module-map/index.html` 的 `MODULES` / `FEATURES` 数组里（`=== DATA:START ===` 标记之间）。新增/修改模块只需编辑该数据块，schema 见 `module-map/README.md`。渲染逻辑无需改动。
