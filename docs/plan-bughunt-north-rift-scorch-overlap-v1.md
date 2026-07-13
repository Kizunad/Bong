# plan-bughunt-north-rift-scorch-overlap-v1（骨架）

> **Skeleton（待修复）**。一句话主题：`rift_mouth_north_002` 与 `north_waste_east_scorch` 的 AABB 在运行时权威 `server/zones.json` 和 worldgen blueprint 中同时重叠，违反归档 plan 对该点“化虚遗迹邻接区、zone 互斥不重叠”的约束；玩家站在北荒东陲裂缝附近会被 `find_zone` 解析成渊口 zone，遮蔽北荒焦土的环境、天气、移动和渡劫焦土语义。

## Bug 摘要

- **核心 bug**：`rift_mouth_north_002` 当前 AABB 为 `[1850,50,-7950]..[2150,100,-7650]`，完整落入 `north_waste_east_scorch` 的 `[1500,60,-8500]..[2700,100,-7500]` 的 Y=60..100 切片。入口点 `[2000,74,-7800]` 同时命中两者。
- **运行时结果**：`ZoneRegistry::find_zone` 对重叠命中取 AABB 体积最小的 zone，所以该点返回 `rift_mouth_north_002`，不是 `north_waste_east_scorch`。
- **设计冲突**：归档 `plan-terrain-rift-mouth-v1` 明确写 `rift_mouth_north_002` 是化虚遗迹邻接区，并备注“zone 互斥不重叠”，不是合法嵌套。
- **非重复项**：不是 #986（`giant_sword_sea` / `wuxing_abyss` 重叠），也不是 #998 TSY Y 分层、#1008 pipeline cwd、#971 mineral anchors 或 #992 start.sh 环境变量。

## 对实际游玩体验的影响

玩家靠近北荒东陲塌缩裂缝时，画面位置仍在“北荒东陲焦土”大区内，但服务端把玩家归属成小的渊口 zone。结果是焦土区应有的雷暴 profile、焦土脚感、环境音和渡劫焦土记录在裂缝附近被遮蔽；玩家会看到一个世界观上应当“焦土与化虚遗迹邻接”的地点，实际却像从焦土 zone 中被挖掉一块，相关反馈和后续事件不连续。

## 证据定位

- `server/zones.json:60`：`rift_mouth_north_002` AABB `[1850,50,-7950]..[2150,100,-7650]`，patrol/portal anchor `[2000,74,-7800]`。
- `server/zones.json:567`：`north_waste_east_scorch` AABB `[1500,60,-8500]..[2700,100,-7500]`，active_events 含 `tribulation_scorch` / `tianjie_ascension_pit`。
- `server/zones.worldview.example.json:273` 与 `server/zones.worldview.example.json:403`：blueprint 同样重叠，说明不是只写坏了运行时导出。
- `docs/finished_plans/plan-terrain-rift-mouth-v1.md:197`：该渊口被定义为化虚遗迹邻接区，且“zone 互斥不重叠”。
- `server/src/world/zone.rs:301`：`find_zone` 过滤同维度命中后按 `aabb_volume()` 取最小 zone。
- `server/src/audio/ambient.rs:232`：环境音用 `find_zone` 得到的 zone 决定 `zone_name` / `recipe_id`。
- `server/src/movement/mod.rs:872`、`server/src/movement/mod.rs:902`：移动 zone kind 取 `find_zone`，只有命中 zone 名含 ash 或事件含 `tribulation_scorch` / `no_cadence` 才允许焦土表面特殊脚感。
- `server/src/tribulation/scorch_record.rs:59`、`server/src/tribulation/scorch_record.rs:79`：渡劫焦土记录先 `find_zone`，再判断命中 zone 是否 scorch。
- `server/src/world/weather_physics/vision.rs:67`、`server/src/world/weather_physics/vision.rs:79` 与 `server/weather_profiles.json:8`：天气视距/profile 也按命中 zone name 查询，`north_waste_east_scorch` 有专门雷暴 profile，渊口没有。

## 触发路径

1. 启动默认 `ZoneRegistry::load()`，加载 `server/zones.json` 并合并 TSY blueprint。
2. 玩家或 NPC 位于 `[2000,74,-7800]` 附近，即 `rift_mouth_north_002` 的入口/巡逻锚点。
3. `find_zone(Overworld, pos)` 同时命中 `rift_mouth_north_002` 和 `north_waste_east_scorch`。
4. 因小 AABB 体积更小，返回 `rift_mouth_north_002`。
5. 后续环境音、移动、天气视距、渡劫焦土记录、风险热力图等都按渊口 zone 消费，北荒焦土语义在重叠区域被遮蔽。

## 反方审查记录

### Round 1

- **反方尝试**：也许这是合法嵌套；`find_zone` 注释确实支持 `rift_mouth_blood_001` 嵌入 `blood_valley` 一类设计。
- **裁决**：未推翻。合法嵌套的历史例子不包含 `rift_mouth_north_002` / `north_waste_east_scorch`，而该 pair 的归档设计明确要求互斥不重叠。

### Round 2

- **反方尝试**：即使几何重叠，也可能只是归属显示差异，实际体验影响不足。
- **裁决**：未推翻。至少环境音、焦土移动、天气视距、渡劫焦土记录都直接消费 `find_zone` 的命中 zone；入口点会把焦土反馈切成渊口反馈，属于可达玩家体验 bug。

## Skeleton Fix Plan

1. **P0 数据修正**：同步调整 `server/zones.worldview.example.json` 与 `server/zones.json`，让 `rift_mouth_north_002` 与 `north_waste_east_scorch` 在 3D AABB 上互斥，同时保留两者邻接关系、入口点、`north_waste_east_scorch` 的化虚遗迹锚点与焦土面积。
2. **P1 worldgen 守护**：在 worldgen/server zone coverage 测试中新增“非白名单 zone pair 不得 3D 重叠”检查；白名单只允许已设计确认的嵌套 pair，不把 `north_002` / `north_waste_east_scorch` 加入白名单。
3. **P2 runtime pin**：补 server pin 测试：`[2000,74,-7800]` 修复后不再同时命中两个互斥 zone；`north_waste_east_scorch` 的焦土 POI/ascension pit 仍解析为 scorch zone。
4. **避免误修**：不要改全局 `find_zone` 最小 AABB 语义；不要给 `rift_mouth_north_002` 硬塞 `tribulation_scorch` 或天气 profile；不要只修 `zones.json` 或只修 blueprint。

## 验收测试计划

- `server/`：`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test world::zone`
- `worldgen/`：新增/更新 zone overlap 守护后运行相关 `python3 -m pytest worldgen/tests/test_zones_export.py worldgen/tests/test_terrain_gen_zone_overlays.py`（若本地 pytest 入口不同，按仓库 worldgen 测试入口执行）。
- 仓库根联调：`bash scripts/smoke-test-e2e.sh`，并确保 `BONG_SKIP_SKIN_PREFETCH=1`。
- 手动/日志验收：站在北荒东陲焦土 POI 与裂缝入口附近，确认焦土区仍触发 `north_waste_east_scorch` 的天气/环境语义，裂缝入口仍能作为 `rift_mouth_north_002` 触发坍缩渊传送。

## 风险

- 移动渊口可能破坏 `target_family_pos_xyz` 的空间叙事、portal_anchor、截图路线或玩家已知坐标；修复应优先做小位移或缩边，而不是重排整片北荒。
- 移动焦土边界可能影响 `ascension_pit_xz`、巡逻锚点、天气 profile 覆盖和化虚遗迹区域大小；必须保证化虚遗迹仍在 `north_waste_east_scorch` 内。
- 加 overlap guard 时要保留历史合法嵌套，否则会误伤 `rift_mouth_blood_001` 嵌入血谷等既定设计。

## 审计来源

BugHunt F6 worldgen 第六轮（2026-07-06）。本轮按要求只新增 skeleton plan，不消费/归档 plan，不修改实际代码、配置、依赖或资源；已先查开放 PR 并避开 #971 / #986 / #992 / #998 / #1008。
