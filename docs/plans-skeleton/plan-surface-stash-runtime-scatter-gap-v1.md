# plan-surface-stash-runtime-scatter-gap-v1（骨架）

> **骨架（草案）**。一句话主题：修复 `SurfaceStash` 新手地表遗缴的**零生成**断链。当前代码把 `SurfaceStash` 的 enum/schema/搜索/VFX/respawn/loot pool 都接好了，但**主线没有任何 runtime 生产路径**，导致 spawn 区玩家正常探索时根本遇不到散修遗缴，也拿不到这条引导链承诺的入门资源。
>
> **玩家影响**：`docs/finished_plans/plan-onboarding-loop-v1.md:620-622` 明确把 `ling_shui` 标成**入门阶段唯一获取路径**，来源就是 `surface_stash_craft`；一旦 `SurfaceStash` 零生成，新手探索、手搓引导、配方碎片/灵水掉落链都会直接断掉。

## 阶段总览（按“先证据收口，再接生产，再补回归”拆）

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 证据收口：确认 `SurfaceStash` 只有消费侧、没有生产侧 | fix_pr | ⬜ |
| P1 | runtime scatter 真接线：spawn `PoiNoviceSite` + `LootContainer` + 方块外观 | fix_pr | ⬜ |
| P2 | respawn / 搜索 / 引导资源回归，避免“补了首刷又漏复活” | fix_pr | ⬜ |

## P0 — 证据收口：主线零生成，不是“刷得少”

- **worldgen 不会导出 `surface_stash` POI**：`worldgen/scripts/poi_novice_selector.py:13-20` 的 `PoiType` 只有 6 种，`build_novice_poi_manifest_payload()`（241-287）也只循环这 6 种；没有 `surface_stash`。
- **spawn profile 只下发 tutorial POI**：`worldgen/scripts/terrain_gen/profiles/spawn_plain.py:180-230` 只生成 `spawn_tutorial_coffin` / `tutorial_chest` / `tutorial_rogue_anchor` / `tutorial_rat_path` / `tutorial_lingquan`。
- **server 只消费上面这些 tutorial POI**：`server/src/world/spawn_tutorial.rs:462-537` 的 `match poi.kind` 只处理 coffin / lingquan / chest / rogue；没有 `surface_stash` 分支。
- **`SurfaceStash` scatter 仍停在“后续集成”**：`server/src/world/poi_novice.rs:445-548` 注释直写“Startup system 接入在后续集成”，实际只有纯函数 `scatter_surface_stashes()` 与单测。
- **finished plan 自报“已实现 scatter”与现状不符**：`docs/finished_plans/plan-onboarding-loop-v1.md:210-218` 设计要求 server-side runtime scatter；`714-718` 的 Finish Evidence 也把 `poi_novice.rs (PoiNoviceKind::SurfaceStash + scatter)` 列为已落地。

## P1 — runtime scatter 真接线

- 在 server startup 路径把 `scatter_surface_stashes()` 接成**真实生产系统**，不要继续停留在纯函数/单测层。
- 每个散点必须同时产出：
  - `PoiNoviceSite` / 可进入 `PoiNoviceRegistry`
  - `LootContainer { kind = SurfaceStash }`
  - 地表可见外观方块（按 onboarding plan 的 runtime block placement 约定）
- 接线后要保证 determinism：同 seed / 同 spawn 区边界 / 同现有 POI 集，散点结果稳定。

## P2 — 回归与验收

- 首刷回归：新建世界后 spawn 区应真实存在 12 个散修遗缴，而不是只有 loot pool / schema / 搜索逻辑。
- 复活回归：`PoiRespawnStore` 现在只会对 registry 中已有的 `SurfaceStash` 做 respawn（`server/src/world/poi_respawn_tick.rs:87-108`）；补首刷后要验证 3600 tick 复活链真能命中实体与外观。
- 资源回归：`surface_stash_craft` 的灵水/碎片/蓝图链重新可达，避免新手引导继续因“唯一来源缺失”卡死。

## §N 开放问题

1. runtime scatter 的 world seed 应从哪条现有权威来源取值，避免每次重启重新洗点。
2. 需要不要复用 `spawn_tutorial` 现有 surface snap / blocked tile 约束，避免遗缴刷进水里、石棺上或教程 POI 脸上。
3. 是否补一条“`SurfaceStash` 生产路径存在性”集成测试，防止以后再回到“只有 enum/schema/搜索侧”的半接线状态。
