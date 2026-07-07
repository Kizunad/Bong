# BugHunt: 暴龙王巢穴 POI 烤入 manifest 但运行时无人消费

## 摘要

`baolongwang_cavern_deep` 在 worldgen 源数据中声明了 `boss_arena` 与 `boss_spawn` 两个 POI，其中 `boss_spawn.unlock` 明确写着“进入巢穴即触发BOSS战”。这些 POI 会被 raster manifest 导出并由 Rust `TerrainProvider` 读入，但生产运行时没有任何 consumer 把 `boss_spawn` / `boss_arena` 转成暴龙王实体、丹炉弱点实体或入场触发器。

结果是正式游玩里暴龙王巢穴会存在于地形/zone 数据中，但暴龙王遭遇战自然路径不可达，只能靠 `/baolongwang spawn` dev 命令手动召唤。

## 证据

- `server/zones.worldview.example.json:1545-1563`：`baolongwang_cavern_deep` 声明 `boss_arena` 和 `boss_spawn`；`boss_spawn` 的 `unlock` 是“进入巢穴即触发BOSS战”。
- `worldgen/scripts/terrain_gen/bakers/raster_export.py:253-254` 与 `:568-572`：`_collect_poi_payload(plan.blueprint_zones)` 会把 `zone.pois` 原样写进 raster manifest。
- `server/src/world/terrain/raster.rs:713-730`、`:789-800`：`TerrainProvider::load` 解析 manifest，并把 `manifest.pois` 装成 `TerrainProvider::pois()`。
- `server/src/world/mod.rs:169-172`：当前启动期 manifest POI consumer 只有 `tsy_poi_consumer` 和 `poi_novice`；后者只处理带 `poi_novice` tag 的新手 POI。
- `server/src/world/tsy_poi_consumer.rs:1-18`、`:78-203`：TSY consumer 只覆盖 `rift_portal`、`loot_container`、`npc_anchor`、`relic_core_slot`，不处理 `boss_spawn` / `boss_arena`。
- `server/src/dandao/boss_spawn.rs:531-550`：`boss_spawn::register` 只注册 scorer/action/drain/death/narration 系统，不注册基于 manifest POI 的 spawn/arena consumer。
- `server/src/cmd/dev/baolongwang.rs:96`：`spawn_baolongwang_at` 的真实调用点是 dev 命令；全仓搜索未见生产系统按 `boss_spawn` POI 调用它。

## 实际游玩体验影响

玩家进入暴龙王巢穴时，地形、zone 负灵域和 POI metadata 看似都已准备好，但不会自然刷出暴龙王，也不会生成或绑定“暴龙王丹炉”弱点。丹道线的核心 Boss 战、真元吸取光环、阶段旁白、掉落和丹炉弱点叙事都不会在正常游玩中触发。

这会让玩家以为找到的是一个空的负灵域洞穴：环境危险存在，但关键遭遇战缺席，丹道终局内容只能由管理员/dev 命令补救。

## 去重判断

- 不重复 #1053 carver owner/provenance、#1062 新手 POI 增量重烘、#1067/#1080 raster_check、#1028 TSY family 前缀、#1036/#1042 spawn 高度、#971 矿脉旧坐标、#986 剑海/深渊重叠。
- 已检索 `docs/plan-*.md`、`docs/plans-skeleton/*.md`、`docs/finished_plans/*.md`，未发现“baolongwang manifest POI consumer / boss_spawn 自然生成 / boss_arena runtime wiring”同主题 active/skeleton/finished BugHunt plan。
- `plan-dandao-runtime-wiring-v1` 已完成 `spawn_baolongwang_at`、AI、真元吸取与 dev 命令，但 finish evidence 只说明暴龙王系统本体接通，并未补齐“worldgen POI → 正式遭遇战触发”的 manifest consumer。

## 修复方向

- [ ] 增加生产运行时 consumer：启动期或玩家接近时读取 `TerrainProviders.overworld.pois()` 中 `kind == "boss_spawn"` 且 tag 含 `baolongwang` 的 POI，按 POI 坐标在 Overworld layer 生成唯一暴龙王实体。
- [ ] 处理 `boss_arena` / `boss_furnace` 语义：要么生成丹炉弱点 marker/entity，要么显式改 POI/tag 命名并补测试，避免 manifest 继续声明一个无运行时消费的弱点。
- [ ] 加幂等与生命周期门：同一 `baolongwang_cavern_deep` 不应重复刷多只 Boss；死亡、重启、刷新策略需有明确状态来源。
- [ ] 加端到端测试：构造含 `boss_spawn` / `boss_arena` 的 `TerrainProviders`，跑生产注册路径后断言生成暴龙王实体与预期 layer/position/zone 绑定。
- [ ] 加回归测试：`boss_spawn::register` 或新 consumer 不应只被 dev command 覆盖；`server/zones.worldview.example.json` 的暴龙王 POI 必须有 runtime consumer。

## 对抗结论

第一轮对抗发现 TSY family 前缀漂移，但判定为 #1028 / skeleton 已知主题，已丢弃。第二轮对抗转向 manifest POI runtime consumer，确认暴龙王候选为新高置信断链。本地复核同意：worldgen 数据、manifest 导出、Rust loader 都存在，缺口在正式运行时消费层。
