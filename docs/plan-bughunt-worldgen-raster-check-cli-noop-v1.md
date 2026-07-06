# BugHunt: worldgen raster_check CLI 假绿

> Skeleton Plan / report-only。第 3 轮 worldgen 分区发现：`raster_check.py` 只有 `validate_rasters()` API，没有 CLI 入口；但 AGENTS/CLAUDE/README/多个 plan 把它写成可直接执行的 raster 后验命令。直接运行脚本会无输出退出 0，即使传入的 raster 目录不存在，造成验收假绿。

## 实际游玩体验影响

`raster_check.py` 负责挡住 worldgen 专属语义错误：span 合法性、TSY/overworld manifest 分支、rift_portal 必填 tag、fossil/collapse 元数据、`qi_density` 同源断言等。若人工验收、BugHunt/consume-plan 或 future CI 按文档直接执行该脚本，坏 raster 可能被误判为已校验通过。

Rust loader 会挡住部分基础文件缺失和长度错误，`scripts/dev-reload.sh` 也通过 API 正确失败；但这些兜底不能覆盖所有 raster_check 语义 invariant。玩家侧风险是：坏的 TSY/POI/资源/地形语义产物被带入后续启动，表现为裂缝/资源点/塌缩标记/寻路地表等问题在进入游戏后才暴露。

## 复现路径

1. 在仓库根执行：

   ```bash
   python3 worldgen/scripts/terrain_gen/harness/raster_check.py /tmp/bong-definitely-missing-raster-dir
   echo $?
   ```

2. 实际结果：无输出，退出码 `0`。
3. 对照 API 路径：

   ```bash
   cd worldgen
   python3 - <<'PY'
   from scripts.terrain_gen.harness.raster_check import validate_rasters
   ok, msg = validate_rasters('/tmp/bong-definitely-missing-raster-dir')
   print(ok)
   print(msg)
   PY
   ```

4. API 正确返回 `False`，并输出 `manifest.json not found at /tmp/bong-definitely-missing-raster-dir/manifest.json`。

## 根因证据

- `worldgen/scripts/terrain_gen/harness/raster_check.py:55-61`：真实校验逻辑在 `validate_rasters()` 内，缺 manifest 会返回 `False`。
- `worldgen/scripts/terrain_gen/harness/raster_check.py:420-434`：函数末尾根据 errors 返回 `(ok, message)`。
- 同文件没有 `if __name__ == "__main__"`、`argparse` 或任何 `sys.exit(...)` 包装，直接执行不会调用 `validate_rasters()`。
- `AGENTS.md:58`：worldgen 命令矩阵写明 raster 校验走 `worldgen/scripts/terrain_gen/harness/raster_check.py`。
- `CLAUDE.md:78`、`worldgen/README.md:194-195`：项目文档把该文件描述为 raster 后验入口。
- `docs/plans-skeleton/plan-bughunt-mineral-anchor-position-drift-v1.md:88`、`docs/plan-bughunt-worldgen-pipeline-root-cwd-v1.md:92`：既有计划把直接执行脚本列为验收命令。
- `scripts/dev-reload.sh:59-72` 是非问题路径：它 import API 并按 `ok_all` 设置退出码，说明 API 可用，缺口集中在脚本 CLI 包装。

## 修复计划骨架

- [ ] 给 `worldgen/scripts/terrain_gen/harness/raster_check.py` 增加 CLI main：接受一个 raster 目录参数，默认可选 `generated/terrain-gen/rasters`，打印 `validate_rasters()` 消息，`ok=False` 时退出 `1`。
- [ ] 支持 `python3 worldgen/scripts/terrain_gen/harness/raster_check.py <dir>` 和 `cd worldgen && python3 -m scripts.terrain_gen.harness.raster_check <dir>` 两种入口。
- [ ] 保持 `validate_rasters()` API 不变，避免破坏 `scripts/dev-reload.sh` 和现有单测。
- [ ] 更新 README/plan 示例为带目录参数的明确命令，避免“只写文件名”造成歧义。

## 验证计划

- [ ] 新增 CLI 负例测试：传不存在目录时退出非 0，输出 `manifest.json not found`。
- [ ] 新增 CLI 正例测试：对最小合法 fixture 或现有生成 raster 退出 0，并打印 `All ... passed validation`。
- [ ] 保留 API 单测：`validate_rasters()` 的返回值合同不变。
- [ ] 手动回归：

  ```bash
  python3 worldgen/scripts/terrain_gen/harness/raster_check.py /tmp/bong-definitely-missing-raster-dir
  cd worldgen && python3 -m scripts.terrain_gen.harness.raster_check /tmp/bong-definitely-missing-raster-dir
  ```

  两者都必须失败且退出非 0。

## 对抗结论

第一轮反方确认核心证据成立，但要求收窄影响：`scripts/dev-reload.sh` 和直接 import API 的测试路径没有假绿，Rust loader 也会挡住部分基础文件问题。

第二轮已采纳收窄：本 bug 定位为“直接执行 CLI 入口 no-op，导致人工/plan/future CI 后验假绿”，不宣称所有流水线失效，也不宣称所有坏 raster 都能进入 server。反方最终裁决：`REAL`。
