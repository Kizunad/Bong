# plan-bughunt-spirit-eye-raster-candidate-disconnect-v1

> Skeleton Plan（BugHunt worldgen r12）。仅记录真实 bug 与修复计划，不消费、不归档。

## Bug 摘要

`worldgen` 已经在 `broken_peaks` / `spring_marsh` / `rift_valley` 生成 `spirit_eye_candidates` raster layer，且 server 的 raster reader 已能读出 `ColumnSample.spirit_eye_candidates`。但权威灵眼资源 `SpiritEyeRegistry` 启动初始化和迁移候选仍只从 `ZoneRegistry` 的 `center` / `patrol_target(0)` / 固定 offset 三点派生，没有消费 raster 候选层。

结果是：地形生成阶段按高度、灵气密度、地形 feature 筛出的灵眼候选点，只停留在通用 raster 字段里；实际玩家能发现、突破、争夺、迁移的灵眼位置与这些高灵气特殊地形断链。

## 实际游玩体验影响

玩家按灵泉湿地、青云高地、裂谷血地这类高灵气/特殊地形去探索时，worldgen 标出的候选点不会成为实际灵眼。反过来，灵眼可能出现在 zone 几何中心、巡逻点或固定偏移附近，即使那里不是 `spirit_eye_candidates` 筛出的地形语义点。

直接体验是“灵眼不是地形里的稀缺情报资产”：神识发现、私有 HUD 标记、坐标交易、死亡遗念和凝脉到固元突破环境检查都围绕 `SpiritEyeRegistry.eyes` 运转，玩家追踪的是真实 registry 坐标，而不是 worldgen 已烘焙的高灵气候选地形。迁移也会在 zone 派生点之间跳转，削弱“灵眼随天地灵气节点迁移”的探索循环。

## 避重说明

已避开 #969-#1115 中既有 worldgen / 灵眼主题：

- #1024 / `docs/plans-skeleton/plan-bughunt-spirit-eye-runtime-persistence-v1.md`：灵眼 `discovered_by`、`usage_pressure`、迁移后坐标等运行态重启回滚；不处理初始候选未消费 raster。
- #1053：worldgen carver owner 与 provenance 混用。
- #1062：新手 POI 增量重烘局部 fields 污染。
- #1067：`raster_check` CLI 假绿。
- #1080：`raster_check` 必需层漏检假绿。
- #1091：暴龙王 POI 消费断链。
- #1097 / #1115：structure manifest 掉落/loot manifest 运行时断链。
- #1103：焦土地表矿露头运行时断链。

本 bug 的根因是 `spirit_eye_candidates` raster 语义层已生成、已进入通用 reader，但 `world::spirit_eye` 的业务初始化和迁移候选未接线。

## 证据定位

- `docs/finished_plans/plan-spirit-eye-v1.md:42`：接入面写明进料链路是 `worldgen spirit_eye_candidates raster channel -> server 启动时按 zone 数量初始化 N 个灵眼`。
- `docs/finished_plans/plan-spirit-eye-v1.md:62`：候选区筛选明确要求 `worldgen spirit_eye_candidates raster channel + 地形/灵气浓度规则`。
- `docs/finished_plans/plan-spirit-eye-v1.md:199-205`：候选区应由 worldgen pipeline 计算并存入 `spirit_eye_candidates`。
- `worldgen/scripts/terrain_gen/spirit_eye_selector.py:6-42`：按 `height`、`qi_density`、`feature_mask` 和坐标 hash 生成候选 mask。
- `worldgen/scripts/terrain_gen/profiles/broken_peaks.py:270-277`：`broken_peaks` 写入 `buffer.layers["spirit_eye_candidates"]`。
- `worldgen/scripts/terrain_gen/profiles/spring_marsh.py:259-266`：`spring_marsh` 写入 `spirit_eye_candidates`。
- `worldgen/scripts/terrain_gen/profiles/rift_valley.py:275-283`：`rift_valley` 写入 `spirit_eye_candidates`，血谷/裂谷使用更低 qi floor 和更密 stride。
- `server/src/world/terrain/raster.rs:238`、`:996`、`:1112`、`:1151-1154`：server raster reader 已把该层读入 `ColumnSample.spirit_eye_candidates` 并提供 layer query。
- `server/src/world/spirit_eye.rs:350-352`：注册时直接 `SpiritEyeRegistry::from_zones(&ZoneRegistry::load(), startup_salt())`。
- `server/src/world/spirit_eye.rs:470-504`：`candidates_from_zone` 只使用 `zone.center()`、`zone.patrol_target(0)`、`center + DVec3::new(73,0,-41)`，未查询 `TerrainProvider` / raster / `ColumnSample.spirit_eye_candidates`。
- `server/src/cultivation/breakthrough.rs:386-388`：固元突破环境检查只问 `SpiritEyeRegistry::spirit_eye_qi_at()`，所以断链会直接影响突破可用位置。

## 触发路径

1. 运行 worldgen raster pipeline，三个 profile 生成 `spirit_eye_candidates` 层；server raster reader 可读取该字段。
2. server 启动，`world::spirit_eye::register()` 调 `SpiritEyeRegistry::from_zones(&ZoneRegistry::load(), startup_salt())`。
3. `from_zones()` 通过 `candidates_from_zones()` 从每个非负灵气 zone 派生三个人工点，而不是扫描或抽样 raster 候选点。
4. 玩家靠近 worldgen 候选地形时，如果该点不在 `SpiritEyeRegistry.eyes` 半径内，神识发现、HUD、突破环境检查都不触发。
5. 灵眼迁移时也从同一份 zone 派生候选列表选远点，继续绕开 worldgen 候选层。

## 修复计划骨架

- [ ] 为 `SpiritEyeRegistry` 初始化增加 raster 候选输入：从 `TerrainProvider` / raster manifest 扫描或抽样 `spirit_eye_candidates == 1` 的列，生成带 `dimension`、`pos`、`zone_name`、`qi_concentration`、`blood_valley`、`score` 的候选列表。
- [ ] 保留 zone 派生候选作为 fallback：当 raster 不存在、layer 缺失、候选为空或测试使用 fallback registry 时，才退回 `center` / `patrol` / offset。
- [ ] 迁移候选使用同一 authoritative 候选池，避免初始灵眼接 raster、迁移又退回 zone 三点。
- [ ] 候选 `y` 取真实 surface/span ceiling，避免只用 zone AABB center Y；血谷/裂谷候选继续保留高风险高奖励标记。
- [ ] 明确与现有持久化修复的边界：运行态 snapshot hydrate 应覆盖已发现/已迁移的眼；没有 snapshot 时才从 raster 候选初始化。

## 验收测试计划

- [ ] server 单测：构造带 `spirit_eye_candidates` 的最小 raster fixture，初始化后 `SpiritEyeRegistry.eyes` 至少有一口落在候选列半径内，而不是 zone center/patrol/offset。
- [ ] fallback 单测：无 raster / layer 缺失时仍走当前 zone 派生路径，避免测试和离线模式崩溃。
- [ ] 迁移单测：`tick_migration()` 的新坐标来自 raster 候选池，且满足最小迁移距离；候选为空才使用 offset fallback。
- [ ] 集成测试：玩家站在 raster 候选列附近可触发 `spirit_eye_discovery_tick`；站在非候选 zone center 附近不应仅因 center 派生而误触发。
- [ ] worldgen 回归：`broken_peaks` / `spring_marsh` / `rift_valley` 输出的候选层能被 server fixture 或 manifest smoke test 识别，防止再变成只读未消费字段。

## 对抗复核结论

- Round 1：反方结论 `REAL`。确认 `spirit_eye_candidates` 在 server 业务代码中除 raster reader / fixture / default 外没有消费点；实际灵眼由 `SpiritEyeRegistry::from_zones` 的 zone 三点派生。#1024 只覆盖运行态持久化，不覆盖 raster 候选接线。
- Round 2：反方结论 `REAL`。最强反驳“plan 允许按 zone 数量初始化”被驳回：plan 同时要求候选区来自 `worldgen spirit_eye_candidates raster channel`，按 zone 数量是数量公式，不是位置来源。`ColumnSample` generic query 只证明 layer 可读，不等于灵眼业务已消费。
