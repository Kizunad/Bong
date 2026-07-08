# BugHunt: raster_check 漏检 Rust 必需基础层导致地形验证假绿

## 摘要

`worldgen/scripts/terrain_gen/harness/raster_check.py` 只检查 `manifest.tiles[].layers` 声明过的 layer 文件和 spans 合法性，但 Rust 运行时 `TerrainProvider::load` 对每个 tile 无条件要求 `surface_id.bin`、`subsurface_id.bin`、`biome_id.bin`、`water_level.bin`、`feature_mask.bin`、`boundary_weight.bin` 以及 spans 文件存在。

结果是：一个只写了合法 `spans_count.bin` / `spans.bin`、但漏掉基础层且 `manifest.layers=[]` 的坏 raster，会被 `validate_rasters()` 判定为通过；随后 `scripts/dev-reload.sh` 继续启动 server，Rust loader 在 terrain bootstrap 阶段 panic。

## 实际游玩体验影响

玩家侧表现不是某个区域显示错误，而是更早的“世界进不来”：开发或部署流程显示 raster validation OK，但服务端读取同一份 raster 时启动失败，玩家无法进入真实 raster 世界。出生点、寻路、POI、资源刷新、区域语义都会卡在 terrain provider 加载前。

如果坏产物来自增量重烘或 CI 缓存污染，维护者会先看到验证绿灯，再在服务端启动阶段才发现失败，定位会被误导到 server bootstrap，而不是 worldgen 输出完整性。

## 证据

- `worldgen/scripts/terrain_gen/harness/raster_check.py:101`：校验器只遍历 `tile_info.get("layers", [])`，未独立检查 Rust 必需基础层。
- `worldgen/scripts/terrain_gen/harness/raster_check.py:121`：额外只校验 spans 文件合法性。
- `server/src/world/terrain/raster.rs:1128`：`TileFields::load` 无条件 `map_required_layer` 读取 spans 与基础层。
- `server/src/world/terrain/raster.rs:1136`：`surface_id.bin`、`subsurface_id.bin`、`biome_id.bin` 为必需 u8 layer。
- `server/src/world/terrain/raster.rs:1139`：`water_level.bin`、`feature_mask.bin`、`boundary_weight.bin` 为必需 float layer。
- `worldgen/tests/test_raster_check_spans.py:51`：测试 fixture 写 `layers: []`。
- `worldgen/tests/test_raster_check_spans.py:89`：只写 spans 的 fixture 被断言为 `validate_rasters()` 通过，固定了当前假绿行为。
- `scripts/dev-reload.sh:60`：联调 gate 直接调用 `validate_rasters()` API；因此本问题不是 CLI 入口缺失。

## 非重复说明

- 不重复 #1067：#1067 是 `raster_check.py` 缺 CLI main 导致命令行 no-op；本问题是 `validate_rasters()` API 自身漏检，而 `scripts/dev-reload.sh` 正在使用 API。
- 不重复 #1053：不涉及 carver owner / provenance 混用。
- 不重复 #1062：不涉及增量重烘覆盖新手 POI manifest。
- 不重复 #1042：不涉及 spawn safe_y 与地表漂移。
- 不重复 #971 / #986 / #998 / #1008 / #1015 / #1028：不涉及矿脉锚点、剑海 overlap、TSY Y 分层、pipeline cwd、北荒语义遮蔽或 TSY family 前缀。

## 修复计划

- [ ] 在 `validate_rasters()` 中定义 Rust loader 必需基础层清单：
  - `spans_count.bin`
  - `spans.bin`
  - `surface_id.bin`
  - `subsurface_id.bin`
  - `biome_id.bin`
  - `water_level.bin`
  - `feature_mask.bin`
  - `boundary_weight.bin`
- [ ] 对每个 manifest tile 无条件检查上述文件存在与 byte length：
  - u8 layer 长度等于 `tile_size * tile_size`
  - float32 layer 长度等于 `tile_size * tile_size * 4`
  - spans 长度沿用当前 span 校验常量
- [ ] 更新 `worldgen/tests/test_raster_check_spans.py` 的合法 fixture，补齐 Rust 必需基础层；新增缺失每类基础层的红测。
- [ ] 增加一条“manifest 未声明基础层但文件存在仍可通过”的兼容测试，避免把必需基础层误塞回 `manifest.layers` 契约。

## 验收

- `python3 -m pytest worldgen/tests/test_raster_check_spans.py worldgen/tests/test_raster_check_qi.py worldgen/tests/test_raster_check_qi_source.py -q`
- `cd worldgen && python3 -m scripts.terrain_gen --output-dir generated/terrain-gen-smoke --backend raster`
```bash
cd worldgen && python3 - <<'PY'
from scripts.terrain_gen.harness.raster_check import validate_rasters
ok, msg = validate_rasters("generated/terrain-gen-smoke/rasters")
print(msg)
raise SystemExit(0 if ok else 1)
PY
```
- 构造只含 spans、`layers: []` 的最小 raster fixture 时，`validate_rasters()` 必须红，并明确报缺失基础层文件名。

## 风险

- 现有单元测试里有多个最小 raster fixture 可能只为了测试某一层逻辑而省略基础层；修复时要提供测试 helper 统一写必需基础层，避免每个测试手搓重复文件。
- `manifest.layers` 仍应表示可选 layer 列表；基础层属于 Rust loader 固定契约，不应要求 exporter 把它们重复声明进 `layers`。

## 对抗结论

两轮对抗后保留本候选。第一轮独立审计指出 API 级假绿；第二轮反驳掉了一个误读的 `zones.json` 候选，未推翻本问题。该问题影响 worldgen pipeline 可运行性，且与 #1067 的 CLI no-op 是不同层级。
