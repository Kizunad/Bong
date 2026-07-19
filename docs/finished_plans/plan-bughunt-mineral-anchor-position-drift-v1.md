# BugHunt: 矿脉固定锚点旧坐标漂移到 spawn

> 状态：Finished（✅ 2026-07-18）。本文件恢复自 `353225a4` 的父提交 `c3018995`；PR #1187 合并时删除了 skeleton，却没有留下 active / finished plan。本次恢复并升格后重新验真，补齐 runtime fail-closed 与默认 manifest 精确契约，再完成归档。

## 阶段总览

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 建立 anchor zone / AABB 契约校验 | ✅ 2026-07-18 |
| P1 | 修正四组旧坐标与 `rift_valley` 旧 id | ✅ 2026-07-18 |
| P2 | 锁定 runtime manifest 契约与锚点物化边界 | ✅ 2026-07-18 |
| P3 | 当前主线验真、测试证据与归档收口 | ✅ 2026-07-18 |

## P0 — anchor zone / AABB 契约校验

- `server/src/mineral/anchors.rs` 的 `load_mineral_anchors` 接收 `ZoneRegistry`，由 `validate_anchor_zones` 拒绝未知 zone、非 Overworld zone、中心点越出声明 AABB，以及被 `ZoneRegistry::find_zone` 判给更具体/重叠 zone 的 anchor。
- `server/src/mineral/mod.rs` 将 `spawn_mineral_anchor_nodes` 排在 `world::setup_world` 与 `ZoneRegistryStartupSet` 后，保证生产启动路径使用真实 runtime zone 表。
- `load_manifest_rejects_unknown_runtime_zone`、`load_manifest_rejects_non_overworld_declared_zone`、`load_manifest_rejects_center_outside_declared_zone_aabb`、`load_manifest_rejects_more_specific_runtime_zone_capture` 饱和锁定四类错误分支。

## P1 — 坐标与旧 zone id 修复

- `worldgen/blueprint/mineral_anchors.json` 固定为 10 条 anchor：`qingyun_peaks` 3 条、`blood_valley` 4 条、`lingquan_marsh` 2 条、`spawn` 教学凡铁 1 条。
- 原 `rift_valley/cu_tie` 已归并为合法的 `blood_valley/cu_tie`；9 条非 spawn 坐标均落在声明 zone 当前 AABB，且按最小 AABB 优先语义实际仍解析到声明 zone。
- `manifest_anchors_declare_zones_that_exist_in_runtime_registry` 同时断言恰好 10 条与精确 zone/mineral 集；`manifest_only_spawn_anchor_is_the_teaching_fan_tie_vein`、`manifest_no_longer_references_nonexistent_rift_valley_zone` 锁定教学矿与旧 id 消失。

## P2 — runtime manifest 与物化边界

- `spawn_mineral_anchor_nodes` 在进入任何 `Commands::spawn` / `MineralOreIndex::insert` / `spawn_fossil_mineral_nodes` 之前完整加载并校验 manifest；随后 `prepare_mineral_anchor_positions` 对**全部** anchor 先跑完 `positions_for_anchor`（稳定 hash 排序 → surface snap → dedup → `max_units` 截断），再经 `validate_final_anchor_positions` 对**最终候选**复验声明 AABB（`Zone::contains`）与 runtime owner（`ZoneRegistry::find_zone` 最小 AABB 语义）；任一条失败即整批 fail-closed，不写半批 Commands。
- `startup_fails_closed_before_materializing_invalid_anchor` 锁定中心点级错误 manifest 下索引与 `MineralOreNode` 均为 0；`startup_keeps_multi_anchor_batch_atomic_when_later_final_candidate_is_invalid` 锁定“前序合法 anchor + 后序最终候选被嵌套 zone 抢夺”时仍零物化；`startup_spawns_index_entries_and_skips_exhausted_positions` 锁定有效启动、耗尽过滤和 `Gatherable` 挂载。
- 默认数据契约测试直接使用生产 `MineralAnchorConfig::default().path` 与 runtime `ZoneRegistry::load()`；`manifest_anchors_declare_zones_that_exist_in_runtime_registry` 额外对默认 10 条跑生产 preflight，并断言最终候选非空且 runtime owner 与声明 zone 一致。

## P3 — 主线验真与归档

- 原修复 commit `b40fcdaf` 与 merge commit `353225a4` 的数据修复仍在主线；PR #1187 的 e2e、preflight、snapshot、review、finalize、CodeRabbit 六项检查均为 SUCCESS。
- 2026-07-18 当前分支补齐生产校验与饱和回归后，通过 server 全量门禁 `11793 passed / 0 failed / 6 ignored`。
- 无上下文 validator 对合并主线后的 `13d03af6f54e83e7e25d06cec32c8bb496f69b29` 给出 PASS，确认实现、测试、提交署名和主线合并边界均闭环。

## Bug 摘要

修复前，`worldgen/blueprint/mineral_anchors.json` 里的固定矿脉锚点仍使用旧世界坐标/旧 zone id。当时 runtime 启动期会直接按 `position` 物化 `MineralOreNode`，但加载器没有校验 `zone` 是否存在，也没有校验 `position` 是否落在该 zone 的 AABB 内。

结果是：除 spawn 教学凡铁矿外，青云残峰、血谷、灵泉湿地、旧 `rift_valley` 的矿点全部实际落在 `spawn` AABB 内。

## 实际游玩体验影响

远端资源门槛被压低：本应通过探索青云残峰、血谷、灵泉湿地获得的部分矿物，会在出生区附近物化。玩家可能在初醒原周边遇到凡铁之外的杂钢、灵晶、灵铁、乌曜、朱砂、玉髓、丹砂等矿脉，削弱采矿路线、地形探索和资源分区的实际意义。

反过来，真正到达青云残峰/血谷/灵泉湿地的玩家可能找不到计划中的固定矿点，导致区域奖励和锻造/炼丹材料节奏漂移。

## 证据定位

Historical pre-fix claims in this section are pinned to blob `c301899575c0f918748556a57e6daf4166a942d7`; verify them with `git show c3018995:<path>` rather than reading the changed HEAD path/lines.

- `c3018995:worldgen/blueprint/mineral_anchors.json:3` 注释声明该 manifest 是 zone x mineral 的固定富集点；`c3018995:worldgen/blueprint/mineral_anchors.json:5` 到 `c3018995:worldgen/blueprint/mineral_anchors.json:84` 列出 10 个 anchor。
- 其中 `qingyun_peaks` 三个锚点位于 `[128,72,256]`、`[192,56,320]`、`[256,64,288]`，但当前 `server/zones.json:601` 的 `qingyun_peaks` AABB 是负 X/负 Z 区域。
- `blood_valley` 三个锚点位于 `[-256,48,-128]` 等 spawn 周边坐标，但当前 `server/zones.json:530` 的 `blood_valley` AABB 是 `x=2600..3400, z=-3250..-1750`。
- `lingquan_marsh` 两个锚点位于 `[512,60,64]` 和 `[488,58,96]`，但当前 `server/zones.json:428` 的 `lingquan_marsh` AABB 是 `x=-3000..-2000, z=2000..3000`。
- `rift_valley` anchor 使用旧 zone id；当前 runtime zone 表没有 `rift_valley` zone（血谷 zone 名为 `blood_valley`，见 `server/zones.json:530`）。
- `server/src/main.rs:108` 注册 `mineral::register(&mut app)`；`server/src/mineral/mod.rs:67` 到 `server/src/mineral/mod.rs:90` 注册默认 anchor 配置，并在 Startup 中运行 `spawn_mineral_anchor_nodes.after(crate::world::setup_world)`。
- `server/src/world/terrain/mod.rs:550` 到 `server/src/world/terrain/mod.rs:576` 在 raster bootstrap 中加载 `TerrainProvider` 并插入 `TerrainProviders`；因此正常 raster runtime 会触发矿脉物化。
- `c3018995:server/src/mineral/anchors.rs:83` 调 `load_mineral_anchors` 读 manifest；`c3018995:server/src/mineral/anchors.rs:104` 到 `c3018995:server/src/mineral/anchors.rs:119` 对每个 anchor 生成位置并写 `MineralOreIndex`。
- `c3018995:server/src/mineral/anchors.rs:249` 到 `c3018995:server/src/mineral/anchors.rs:297` 只校验 manifest version、`mineral_id`、`radius`、`max_units`，没有 zone 存在性或 position-in-AABB 校验。
- `c3018995:server/src/mineral/anchors.rs:300` 到 `c3018995:server/src/mineral/anchors.rs:322` 以 `anchor.center` 为球心生成候选点，说明实际行为以 `position` 为准。

本地只读对拍结果：

```text
qingyun_peaks  fan_tie   [128, 72, 256]    actual_runtime_zone=spawn
qingyun_peaks  za_gang   [192, 56, 320]    actual_runtime_zone=spawn
qingyun_peaks  ling_jing [256, 64, 288]    actual_runtime_zone=spawn
blood_valley   ling_tie  [-256, 48, -128]  actual_runtime_zone=spawn
blood_valley   wu_yao    [-320, 32, -192]  actual_runtime_zone=spawn
blood_valley   zhu_sha   [-288, 40, -160]  actual_runtime_zone=spawn
rift_valley    cu_tie    [0, -32, -512]    actual_runtime_zone=spawn
lingquan_marsh yu_sui    [512, 60, 64]     actual_runtime_zone=spawn
lingquan_marsh dan_sha   [488, 58, 96]     actual_runtime_zone=spawn
spawn          fan_tie   [16, 70, 16]      actual_runtime_zone=spawn
```

## 修复前触发路径

1. 开发/CI 通过 worldgen pipeline 生成 raster，并以 `BONG_TERRAIN_RASTER_PATH` 启动 server。
2. `world::setup_world` 加载 raster manifest，插入 `TerrainProviders`。
3. `mineral::register` 注册的 Startup system 在 `setup_world` 后执行 `spawn_mineral_anchor_nodes`。
4. `spawn_mineral_anchor_nodes` 读取 `worldgen/blueprint/mineral_anchors.json`。
5. 由于当时没有 zone/AABB 契约校验，旧坐标 anchor 被直接传入 `positions_for_anchor`。
6. `positions_for_anchor` 围绕旧 `position` 生成矿点，并用 terrain surface snap 修正 Y。
7. `MineralOreNode` 与 `MineralOreIndex` 写入出生区附近，玩家后续探矿/采矿会命中这些错位矿脉。

## 反方审查记录

### 第一轮质疑

反方当时指出：`MineralAnchor.zone` 在原实现中只是元数据，runtime 生成矿脉不按 zone 查 AABB，也不把 zone 写进 `MineralOreNode`。因此不能把 bug 表述为“zone 字段驱动错区”。真正问题是 `position` 本身旧化，且 runtime 直接按该位置物化。

反方还指出：“fresh spawn 立刻拿全远端材料”表述过强。探矿有境界/距离/工具门槛，不是无条件立刻全拿。

### 补证与让步

采纳上述质疑，修正定性为：矿脉 anchor `position` 仍停在旧世界坐标/旧 zone id，且缺少 zone 存在性与 position-in-AABB 校验。文案只写“远端资源门槛被压低，部分 tier1/2 远端矿出现在出生区附近”，不写“立刻拿全”。

补充确认：正常 raster runtime 会加载 `TerrainProviders` 并触发 `spawn_mineral_anchor_nodes`，因此这不是废弃测试数据。定向开放 PR 搜索 `mineral OR 矿脉 OR anchor OR 锚点` 未发现同题修复 PR。

### 最终裁决

反方最终裁决：候选足够真实，适合只新增 Skeleton Plan PR。建议修复分两层：先修数据，把非 spawn anchor 移入当前合法 zone AABB；再加回归保护，断言每条 anchor 的 `zone` 存在，且 `position` 落在该 zone AABB 内。

## 实施计划

- [x] 建立 anchor-zone 校验源：生产 loader 直接消费当前 `ZoneRegistry`，测试读取默认 manifest 与 runtime zone 表。
- [x] 为 `mineral_anchors.json` 加契约测试：每条 anchor 的 `zone` 必须存在；`position` 必须在该 zone AABB 内；核心点不得落入其他更具体 zone。
- [x] 修正 `qingyun_peaks` 三个矿点坐标，使其落在当前 `qingyun_peaks` AABB 内，并保留矿物种类/数量节奏。
- [x] 修正 `blood_valley` 三个既有矿点坐标，使其落在当前 `blood_valley` AABB 内。
- [x] 修正 `lingquan_marsh` 两个矿点坐标，使其落在当前 `lingquan_marsh` AABB 内。
- [x] 将 `rift_valley/cu_tie` 归并到合法的 `blood_valley/cu_tie`，不再保留未知 runtime zone id。
- [x] 在 mineral startup/load 生产路径输出含 anchor 下标、矿物、zone 与边界原因的清晰错误，并整批 fail-closed。
- [x] 更新契约测试，只允许 spawn 教学 `fan_tie` 固定矿脉仍位于 `spawn`。

## 验收测试计划

- [x] `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`：2026-07-18 全部通过，`11793 passed / 0 failed / 6 ignored`。
- [x] worldgen 生成链路由原 PR #1187 的 `Worldgen Preview Snapshot / snapshot` 与 `E2E Redis Smoke / e2e` 成功检查覆盖；本次未修改 anchor JSON 或 terrain 生成代码。
- [x] `manifest_anchors_declare_zones_that_exist_in_runtime_registry` 直接对拍默认 JSON 与 runtime `ZoneRegistry`：所有非 spawn anchor 的 actual runtime zone 等于声明 zone。
- [x] `manifest_no_longer_references_nonexistent_rift_valley_zone` 锁定 `rift_valley` 不再出现；精确集合断言锁定 `blood_valley/cu_tie` 的归并结果。
- [x] `manifest_only_spawn_anchor_is_the_teaching_fan_tie_vein` 锁定默认数据中 spawn 仅有教学 `fan_tie`；`startup_spawns_index_entries_and_skips_exhausted_positions` 使用一条显式配置的 spawn fixture，锁定有效 anchor 的物化、耗尽过滤、索引写入与 `Gatherable` 挂载。
- [x] 最终候选 preflight：`final_candidates_accept_radius_exactly_on_declared_aabb_boundary`、`final_candidates_reject_radius_one_block_outside_declared_aabb`、`final_candidates_reject_surface_snap_below_declared_aabb`、`final_candidates_reject_more_specific_zone_capture_at_radius_edge`、`startup_keeps_multi_anchor_batch_atomic_when_later_final_candidate_is_invalid`；targeted `mineral::anchors::tests` 20/20 PASS。

## 2026-07-18 当前主线验真

结论：原数据漂移 bug 在 PR #1187 中确实修复，当前 `origin/main` 仍保持正确坐标；但本轮核对发现当年 zone/AABB 契约主要停留在回归测试，生产 loader 仍可能在未来配置漂移时静默物化。本次因此除修复 plan 三态历史外，还补齐 runtime fail-closed、Startup zone 初始化顺序与精确 10 条组合断言，并重新执行 server 全量门禁。

- **P0 契约源真实存在**：`server/src/mineral/anchors.rs` 的 `manifest_anchors_declare_zones_that_exist_in_runtime_registry` 等三条默认数据测试直接读取 `mineral_anchors.json` 与 `ZoneRegistry::load()`，分别锁住 zone 存在、中心点在声明 AABB、spawn 唯一教学凡铁以及旧 `rift_valley` id 消失。断言检查实际 manifest / zone 数据，不是只检查函数名。
- **P1 数据仍正确**：`worldgen/blueprint/mineral_anchors.json:5-84` 保留 10 条 anchor；青云残峰 3 条、血谷 4 条（含由旧 `rift_valley` 归并的 `cu_tie`）、灵泉湿地 2 条均在各自当前 AABB 内，spawn 仅 1 条 `fan_tie`。只读对拍还按 `ZoneRegistry::find_zone` 的“最小 AABB 优先”语义计算实际 runtime zone，10/10 均与声明 zone 相同，没有落入嵌套小 zone。
- **P2 生产路径可达**：`spawn_mineral_anchor_nodes` 仍从默认 manifest 调 `load_mineral_anchors`，再以 `positions_for_anchor` 生成并写入 `MineralOreIndex`；因此测试锁住的是生产会消费的数据。各 anchor 的 X/Z 半径均留在声明 zone 内；深层 `cu_tie` 的 Y 候选由 `MIN_WORLD_Y` 截断，不越出 Overworld 下界。
- **P3 历史与漂移检查**：实现提交 `b40fcdaf` 与 merge commit `353225a4` 的仓库日期均为 2026-07-13；PR #1187 的 e2e、preflight、snapshot、review、finalize、CodeRabbit 均成功。`353225a4..origin/main` 未再修改 `worldgen/blueprint/mineral_anchors.json`；本次在当前主线之上加固 `server/src/mineral/anchors.rs` 与 Startup 排序。
- **文档根因**：`353225a4` 在合入代码时纯删除 skeleton，却没有 promotion、Finish Evidence 或 finished plan；本修复分支补回历史归档，并把核对中发现的生产 fail-closed 缺口一并闭环，不伪称当年三态流转正确。

## 风险

- 坐标修复会改变玩家资源分布；已有存档若已经物化旧位置矿脉，需要决定是否迁移/清理旧 `MineralOreIndex` 或 exhausted log。
- `rift_valley` 既是旧 zone 名又是 terrain profile 名；修复时要避免误删 profile 语义，只处理 anchor 的 zone id。
- 如果仅移动 JSON 而不加契约测试，下一次 zone AABB 调整仍会复发。
- 如果把所有远端矿点移得过远，早期锻造/炼丹材料节奏可能被拉长，需要结合既有 forge/alchemy pacing 测试校准。

## Finish Evidence

### 落地清单

- P0：`server/src/mineral/anchors.rs` — `load_mineral_anchors`、`validate_anchor_zones` 及四类 zone 错误测试；`server/src/mineral/mod.rs` — `spawn_mineral_anchor_nodes` 在 `ZoneRegistryStartupSet` 后运行。
- P1：`worldgen/blueprint/mineral_anchors.json` — 10 条固定 anchor 的合法 zone/坐标；`manifest_anchors_declare_zones_that_exist_in_runtime_registry` — 恰好 10 条与精确 zone/mineral 集合。
- P2：`server/src/mineral/anchors.rs` — `prepare_mineral_anchor_positions` / `validate_final_anchor_positions` 在任何 `Commands::spawn`、`MineralOreIndex` 写入、化石物化之前整批校验最终候选；`startup_fails_closed_before_materializing_invalid_anchor`、`startup_keeps_multi_anchor_batch_atomic_when_later_final_candidate_is_invalid`、`startup_spawns_index_entries_and_skips_exhausted_positions` 及 4 条 final-candidate 边界/嵌套-zone 测试。
- P3：`docs/finished_plans/plan-bughunt-mineral-anchor-position-drift-v1.md` — 恢复丢失的三态归档与当前主线验真证据；本轮补齐最终候选 preflight 验收叙述。

### 关键 commit

- `b40fcdaf`（2026-07-13）：原始修复，迁回 9 条远端 anchor 并加入数据契约测试。
- `353225a4`（2026-07-13）：PR #1187 merge commit；六项 GitHub 检查均 SUCCESS。
- `fb3a0df1`（2026-07-18）：恢复并升格遗失的 skeleton plan。
- `50b639a0`（2026-07-18）：记录 #1187 与当前主线的第一性原理验真。
- `e6ccf84c`（2026-07-18）：补齐生产 loader 的 runtime zone 契约、Startup 排序与 fail-closed 测试。
- `5275263b`（2026-07-18）：锁定默认 manifest 恰好 10 条及精确 zone/mineral 组合。
- `13d03af6`（2026-07-18）：合并最新 `origin/main`；带入内容仅为无关 docs。
- `d25bcf7b`（2026-07-18）：仅执行 active → finished 路径迁移；该 commit 中的文档 blob 仍是 Active、P0-P3 为 ⬜ 且尚无 Finish Evidence，因此它本身不构成合规完成态。
- `4da7a914`（2026-07-18）：在已迁移文件中补齐 P0-P3 完成状态、唯一 `## Finish Evidence` 与验收叙述，形成当前实际完成态；此两提交拆分如实记录归档时的暂态偏差，不把 `d25bcf7b` 虚报为完整归档。
- `77e6e61a` (2026-07-19, PR #1228 rework)：在中心点契约之上补齐最终候选整批 preflight（surface snap / dedup / max_units 后 AABB + runtime owner）与原子 fail-closed；详见本提交。

### 测试结果

- `cd server && cargo fmt --check`：PASS（历史归档轮）。
- `cd server && cargo clippy --all-targets -- -D warnings`：PASS（历史归档轮）。
- `cd server && cargo test`：`11793 passed / 0 failed / 6 ignored`（历史归档轮）。
- 本轮 targeted（HEAD `9c0df4606d354426f3d02e54ac0cae1019f94122` 工作区含 uncommitted final-candidate 实现）：`CARGO_TARGET_DIR=/home/serverkizuna/Code/Bong/server/target cargo test --lib mineral::anchors::tests -- --nocapture` → **`20 passed; 0 failed`，`EXIT:0`**（含 `final_candidates_*` 四边界、`startup_keeps_multi_anchor_batch_atomic_when_later_final_candidate_is_invalid`、扩展后的默认 manifest preflight）。
- PR #1187：`e2e`、`preflight`、`snapshot`、`review`、`finalize`、`CodeRabbit` 全部 SUCCESS。
- 无上下文只读 validator：`PASS 13d03af6f54e83e7e25d06cec32c8bb496f69b29`（归档轮）；本轮 rework 在 full gate / merge main 后再对 exact HEAD 重验。

### 跨仓库核验

- server：`load_mineral_anchors` → `validate_anchor_zones` → `prepare_mineral_anchor_positions` / `validate_final_anchor_positions` → `spawn_mineral_anchor_nodes`（`anchors.iter().zip(prepared_positions)` 保序）→ `MineralOreIndex` / fossil 物化；失败路径在任何 Commands 写入前 return；`ZoneRegistryStartupSet` 排序命中。
- worldgen：`worldgen/blueprint/mineral_anchors.json` 命中 10 条精确 zone/mineral 契约，`rift_valley` 已消失。
- agent/schema：本 plan 不改变 Redis IPC、TypeBox 或 serde 契约。
- client：本 plan 不改变 CustomPayload、HUD 或资源资产。

### 遗留 / 后续

- 当前主线无未闭环代码项；生产 loader 已能在中心点与最终候选两层于物化前 fail-closed。
- PR #1187 之前已实际落盘的外部旧存档若保留错误矿点，其离线迁移不在本 plan 范围；仓库内没有可复现的当前存档迁移阻塞。
