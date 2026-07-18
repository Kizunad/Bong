# BugHunt: 矿脉固定锚点旧坐标漂移到 spawn

> 状态：Active（2026-07-18 历史归档修复）。本文件恢复自 `353225a4` 的父提交 `c3018995`；PR #1187 合并时删除了 skeleton，却没有留下 active / finished plan。本次先恢复并升格，再独立核对当前主线，不预设当年实现仍然正确。

## 阶段总览

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 建立 anchor zone / AABB 契约校验 | ⬜ |
| P1 | 修正四组旧坐标与 `rift_valley` 旧 id | ⬜ |
| P2 | 锁定 runtime 加载失败与锚点物化边界 | ⬜ |
| P3 | 当前主线验真、测试证据与归档收口 | ⬜ |

## Bug 摘要

`worldgen/blueprint/mineral_anchors.json` 里的固定矿脉锚点仍使用旧世界坐标/旧 zone id。当前 runtime 启动期会直接按 `position` 物化 `MineralOreNode`，但加载器没有校验 `zone` 是否存在，也没有校验 `position` 是否落在该 zone 的 AABB 内。

结果是：除 spawn 教学凡铁矿外，青云残峰、血谷、灵泉湿地、旧 `rift_valley` 的矿点全部实际落在 `spawn` AABB 内。

## 实际游玩体验影响

远端资源门槛被压低：本应通过探索青云残峰、血谷、灵泉湿地获得的部分矿物，会在出生区附近物化。玩家可能在初醒原周边遇到凡铁之外的杂钢、灵晶、灵铁、乌曜、朱砂、玉髓、丹砂等矿脉，削弱采矿路线、地形探索和资源分区的实际意义。

反过来，真正到达青云残峰/血谷/灵泉湿地的玩家可能找不到计划中的固定矿点，导致区域奖励和锻造/炼丹材料节奏漂移。

## 证据定位

- `worldgen/blueprint/mineral_anchors.json:3` 注释声明该 manifest 是 zone x mineral 的固定富集点；`worldgen/blueprint/mineral_anchors.json:5` 到 `worldgen/blueprint/mineral_anchors.json:84` 列出 10 个 anchor。
- 其中 `qingyun_peaks` 三个锚点位于 `[128,72,256]`、`[192,56,320]`、`[256,64,288]`，但当前 `server/zones.json:601` 的 `qingyun_peaks` AABB 是负 X/负 Z 区域。
- `blood_valley` 三个锚点位于 `[-256,48,-128]` 等 spawn 周边坐标，但当前 `server/zones.json:530` 的 `blood_valley` AABB 是 `x=2600..3400, z=-3250..-1750`。
- `lingquan_marsh` 两个锚点位于 `[512,60,64]` 和 `[488,58,96]`，但当前 `server/zones.json:428` 的 `lingquan_marsh` AABB 是 `x=-3000..-2000, z=2000..3000`。
- `rift_valley` anchor 使用旧 zone id；当前 runtime zone 表没有 `rift_valley` zone（血谷 zone 名为 `blood_valley`，见 `server/zones.json:530`）。
- `server/src/main.rs:108` 注册 `mineral::register(&mut app)`；`server/src/mineral/mod.rs:67` 到 `server/src/mineral/mod.rs:90` 注册默认 anchor 配置，并在 Startup 中运行 `spawn_mineral_anchor_nodes.after(crate::world::setup_world)`。
- `server/src/world/terrain/mod.rs:550` 到 `server/src/world/terrain/mod.rs:576` 在 raster bootstrap 中加载 `TerrainProvider` 并插入 `TerrainProviders`；因此正常 raster runtime 会触发矿脉物化。
- `server/src/mineral/anchors.rs:83` 调 `load_mineral_anchors` 读 manifest；`server/src/mineral/anchors.rs:104` 到 `server/src/mineral/anchors.rs:119` 对每个 anchor 生成位置并写 `MineralOreIndex`。
- `server/src/mineral/anchors.rs:249` 到 `server/src/mineral/anchors.rs:297` 只校验 manifest version、`mineral_id`、`radius`、`max_units`，没有 zone 存在性或 position-in-AABB 校验。
- `server/src/mineral/anchors.rs:300` 到 `server/src/mineral/anchors.rs:322` 以 `anchor.center` 为球心生成候选点，说明实际行为以 `position` 为准。

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

## 触发路径

1. 开发/CI 通过 worldgen pipeline 生成 raster，并以 `BONG_TERRAIN_RASTER_PATH` 启动 server。
2. `world::setup_world` 加载 raster manifest，插入 `TerrainProviders`。
3. `mineral::register` 注册的 Startup system 在 `setup_world` 后执行 `spawn_mineral_anchor_nodes`。
4. `spawn_mineral_anchor_nodes` 读取 `worldgen/blueprint/mineral_anchors.json`。
5. 由于没有 zone/AABB 契约校验，旧坐标 anchor 被直接传入 `positions_for_anchor`。
6. `positions_for_anchor` 围绕旧 `position` 生成矿点，并用 terrain surface snap 修正 Y。
7. `MineralOreNode` 与 `MineralOreIndex` 写入出生区附近，玩家后续探矿/采矿会命中这些错位矿脉。

## 反方审查记录

### 第一轮质疑

反方指出：`MineralAnchor.zone` 当前只是元数据，runtime 生成矿脉不按 zone 查 AABB，也不把 zone 写进 `MineralOreNode`。因此不能把 bug 表述为“zone 字段驱动错区”。真正问题是 `position` 本身旧化，且 runtime 直接按该位置物化。

反方还指出：“fresh spawn 立刻拿全远端材料”表述过强。探矿有境界/距离/工具门槛，不是无条件立刻全拿。

### 补证与让步

采纳上述质疑，修正定性为：矿脉 anchor `position` 仍停在旧世界坐标/旧 zone id，且缺少 zone 存在性与 position-in-AABB 校验。文案只写“远端资源门槛被压低，部分 tier1/2 远端矿出现在出生区附近”，不写“立刻拿全”。

补充确认：正常 raster runtime 会加载 `TerrainProviders` 并触发 `spawn_mineral_anchor_nodes`，因此这不是废弃测试数据。定向开放 PR 搜索 `mineral OR 矿脉 OR anchor OR 锚点` 未发现同题修复 PR。

### 最终裁决

反方最终裁决：候选足够真实，适合只新增 Skeleton Plan PR。建议修复分两层：先修数据，把非 spawn anchor 移入当前合法 zone AABB；再加回归保护，断言每条 anchor 的 `zone` 存在，且 `position` 落在该 zone AABB 内。

## 实施计划

- [ ] 建立 anchor-zone 校验源：在 worldgen 或 server 测试中读取 `worldgen/blueprint/mineral_anchors.json` 与当前 runtime zone 表，构造 zone name -> AABB 索引。
- [ ] 为 `mineral_anchors.json` 加契约测试：每条 anchor 的 `zone` 必须存在；`position` 必须在该 zone AABB 内；`radius` 球体至少核心点不得落入其他更具体 zone。
- [ ] 修正 `qingyun_peaks` 三个矿点坐标，使其落在当前 `qingyun_peaks` AABB 内，并保留矿物种类/数量节奏。
- [ ] 修正 `blood_valley` 三个矿点坐标，使其落在当前 `blood_valley` AABB 内。
- [ ] 修正 `lingquan_marsh` 两个矿点坐标，使其落在当前 `lingquan_marsh` AABB 内。
- [ ] 处理 `rift_valley` 旧 id：决定归并到 `blood_valley`、某个合法渊口 zone，或删除/重命名为当前合法 zone；不得保留未知 zone id。
- [ ] 在 mineral startup/load 路径或 worldgen harness 中输出清晰错误，避免未来 zone 坐标迁移时静默漂移。
- [ ] 更新相关测试 fixture/文档引用，只允许 spawn 教学凡铁矿仍位于 `spawn`。

## 验收测试计划

- [ ] `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`。
- [ ] `cd worldgen && python3 -m unittest discover -s tests -p 'test_*mineral*' -v`；如果新增测试不以 mineral 命名，改跑对应 test 文件。
- [ ] `cd worldgen && python3 -m scripts.terrain_gen --backend raster --zone-filter spawn,qingyun_peaks,blood_valley,lingquan_marsh`，再用 `worldgen/scripts/terrain_gen/harness/raster_check.py` 校验生成产物。
- [ ] 增加一个只读对拍脚本/测试输出：所有非 spawn anchor 的 actual runtime zone 等于声明 zone；`rift_valley` 不再出现于 anchor manifest。
- [ ] 手动或 smoke 验证：spawn 区只剩教学 `fan_tie` 固定矿脉；青云残峰/血谷/灵泉湿地各自能在本区找到对应固定矿脉。

## 风险

- 坐标修复会改变玩家资源分布；已有存档若已经物化旧位置矿脉，需要决定是否迁移/清理旧 `MineralOreIndex` 或 exhausted log。
- `rift_valley` 既是旧 zone 名又是 terrain profile 名；修复时要避免误删 profile 语义，只处理 anchor 的 zone id。
- 如果仅移动 JSON 而不加契约测试，下一次 zone AABB 调整仍会复发。
- 如果把所有远端矿点移得过远，早期锻造/炼丹材料节奏可能被拉长，需要结合既有 forge/alchemy pacing 测试校准。
