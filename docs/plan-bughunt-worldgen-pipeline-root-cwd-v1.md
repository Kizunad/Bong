# BugHunt: worldgen pipeline 根目录入口 cwd 断链

## Bug 摘要

顶层命令合同暴露 `bash worldgen/pipeline.sh` 作为仓库根可执行的 worldgen 入口，但 `worldgen/pipeline.sh` 仍假设调用者已经 `cd worldgen`。脚本在 `worldgen/pipeline.sh:22` 计算了 `SCRIPT_DIR`，却没有 `cd "$SCRIPT_DIR"` 或设置 `PYTHONPATH`；后续 `worldgen/pipeline.sh:64` 直接执行 `python3 -m scripts.terrain_gen`，从仓库根运行时 Python 无法找到 `worldgen/scripts/terrain_gen`。

这不是 #971 / #986 / #992 / #998：
- #971 是矿脉固定锚点旧坐标漂移到 spawn。
- #986 是 giant_sword_sea 与 wuxing_abyss AABB 重叠。
- #992 是 `scripts/start.sh` 漏传 `BONG_TSY_RASTER_PATH`。
- #998 是 TSY Y 分层被 2D overlay 覆盖成 deep 单层。

## 对实际游玩体验的影响

fresh checkout 中 `worldgen/generated/` 被 `.gitignore` 忽略，正式 raster manifest 不随仓库存在。开发者、CI 辅助脚本或 agent 按顶层合同从仓库根执行 `bash worldgen/pipeline.sh` 试图生成 raster 时，会在 import 阶段失败，manifest 不会生成。

如果随后按普通启动路径运行 `scripts/start.sh`，该脚本只查 `worldgen/generated/terrain-gen/rasters/manifest.json`；缺失时会清空 `BONG_TERRAIN_RASTER_PATH` 并提示 fallback 扁平世界（`scripts/start.sh:23-31`）。服务端再进入 `FallbackFlat`，创建 spawn 周围 16x16 chunk 测试区（`server/src/world/mod.rs:204-209`, `server/src/world/mod.rs:515-548`）。玩家看到的就不是正式 raster 地形，layout placement / POI runtime 数据与实际地形体验都会退化或缺失。

影响范围需要限定：`scripts/dev-reload.sh` 已显式 `(cd worldgen && .venv/bin/python -m scripts.terrain_gen ...)`，当前 `.github/workflows/worldgen-preview.yml` 也设置 `working-directory: worldgen`，这两条主路径不直接受该 bug 影响。本 bug 指向公开手动/agent 根目录入口与脚本实现漂移。

## 证据定位

- `CLAUDE.md:31-33`：Worldgen 命令列出 `bash worldgen/pipeline.sh`，没有要求先 `cd worldgen`。
- `AGENTS.md:53-59`：worldgen 命令矩阵也列出 `bash worldgen/pipeline.sh`。
- `worldgen/pipeline.sh:22`：计算 `SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"`。
- `worldgen/pipeline.sh:36-40`：默认 blueprint/output 仍按 `worldgen/` cwd 设计。
- `worldgen/pipeline.sh:64-69`：执行 `python3 -m scripts.terrain_gen`，依赖 cwd 或 PYTHONPATH 能看到 `scripts/`。
- `worldgen/README.md:35-45` 与 `worldgen/pipeline.sh:3-8`：内部文档要求 `cd worldgen`，证明存在 workaround，但与顶层合同冲突。
- `scripts/start.sh:23-31`：缺少正式 manifest 时 fallback 扁平世界。
- `server/src/world/mod.rs:204-209`、`server/src/world/mod.rs:515-548`：fallback flat world 只创建 spawn 周围测试区。

## 触发路径

从仓库根执行：

```bash
bash worldgen/pipeline.sh server/zones.worldview.example.json /tmp/bong-pipeline-root-cwd-check raster 16 spawn
```

实际输出：

```text
=== 末法残土 terrain_gen Pipeline ===
蓝图: server/zones.worldview.example.json
输出目录: /tmp/bong-pipeline-root-cwd-check
Bake backend: raster
Tile size: 16
Zone filter: spawn

/usr/bin/python3: No module named scripts.terrain_gen
```

显式把输出目录指到运行时目录仍失败在同一 import 阶段：

```bash
bash worldgen/pipeline.sh server/zones.worldview.example.json worldgen/generated/terrain-gen raster 16 spawn
```

从 `worldgen/` cwd 验证模块本身存在：

```bash
cd worldgen
python3 -c "import scripts.terrain_gen; print(scripts.terrain_gen.__doc__)"
```

输出：

```text
Blueprint-driven terrain generation scaffolding.
```

## 反方审查记录

第一轮反方结论：部分成立，需收窄。反方确认仓库根执行 `bash worldgen/pipeline.sh` 会复现 `/usr/bin/python3: No module named scripts.terrain_gen`，且不重复 #971/#986/#992/#998；但指出原始影响表述过强，因为 `pipeline.sh` 默认输出是 `generated/terrain-gen-smoke`，而 `scripts/start.sh` 消费的是 `generated/terrain-gen`，且 `scripts/dev-reload.sh` 不受影响。

第二轮反方结论：收窄后通过。该问题不是“默认 smoke 输出没接 start.sh”，而是顶层 `CLAUDE.md` / `AGENTS.md` 暴露的仓库根入口与脚本 cwd 假设冲突。`worldgen/README.md` 和脚本头注释的 `cd worldgen` 只能说明存在 workaround，不能否定顶层合同；`scripts/dev-reload.sh` 与当前 preview workflow 可用会降低严重性，但不否定公开手动/agent 入口破损及 fresh checkout 缺 manifest 后 fallback flat 的实际体验影响。

## Skeleton Fix Plan

- [ ] 明确 `worldgen/pipeline.sh` 的调用合同：仓库根 `bash worldgen/pipeline.sh` 和 `cd worldgen && bash pipeline.sh` 都应可用，或顶层文档统一收回根入口。
- [ ] 若保留根入口，使用 `SCRIPT_DIR` 自定位执行环境，确保 `python3 -m scripts.terrain_gen` 从任意 cwd 都能 import。
- [ ] 保持现有 `cd worldgen && bash pipeline.sh ...` 调用不回归，尤其是 `worldgen/README.md`、`.github/workflows/worldgen-preview.yml`、历史 anvil/snapshot 调用方式。
- [ ] 对相对 blueprint/output 参数做显式约定和测试，避免修 cwd 后把 `../server/...`、`generated/...`、`/tmp/...` 三类路径解释错。
- [ ] 将错误信息收口：若 blueprint/output 路径不合法，应在 terrain_gen 参数校验阶段报错，而不是 Python import 阶段报 `No module named scripts.terrain_gen`。

## 验收测试计划

- [ ] 仓库根执行 `bash worldgen/pipeline.sh` 成功，产出默认 smoke raster manifest 和 PNG 预览。
- [ ] 仓库根执行 `bash worldgen/pipeline.sh ../server/zones.worldview.example.json /tmp/bong-pipeline-root-cwd-check raster 16 spawn` 成功，`/tmp/bong-pipeline-root-cwd-check/rasters/manifest.json` 存在。
- [ ] `cd worldgen && bash pipeline.sh ../server/zones.worldview.example.json generated/terrain-gen-smoke raster 16 spawn` 仍成功。
- [ ] `cd worldgen && python3 -m scripts.terrain_gen --backend raster --zone-filter spawn --tile-size 16 --output-dir /tmp/bong-terrain-module-check` 仍成功。
- [ ] `python3 worldgen/scripts/terrain_gen/harness/raster_check.py` 或等价 `validate_rasters(...)` 校验新输出的 raster manifest 通过。
- [ ] fresh checkout 无 `worldgen/generated/terrain-gen/rasters/manifest.json` 时，先跑修复后的根入口生成运行时 manifest，再跑 `scripts/start.sh` 不再走 `FallbackFlat`。

## 风险

- 简单 `cd "$SCRIPT_DIR"` 可能改变从仓库根显式传入 `server/zones...`、`worldgen/generated...` 这类相对路径的解释；修复需要先定义相对路径基准。
- 简单设置 `PYTHONPATH="$SCRIPT_DIR"` 虽可修 import，但默认 `../server/zones.worldview.example.json` 仍会从仓库根解析到仓库外，不能完整满足脚本默认值。
- 当前 preview workflow 与 `scripts/dev-reload.sh` 已有 workaround，修复时不能破坏这些稳定路径。
- 若同时改文档和脚本，容易把“支持哪些 cwd”写得更分裂；验收必须同时覆盖仓库根与 `worldgen/` 两种入口。
