# plan-bughunt-anomaly-raster-runtime-consumer-v1

> **BugHunt report-only active plan**。一句话主题：worldgen 静态 raster 的 `anomaly_intensity` / `anomaly_kind` 已作为“事件 / spawn / FX hook”导出并被 Rust reader 读入，但 server runtime 没有统一 consumer 扫描这些热点，导致古战场、裂隙、焦土、TSY 等 profile 写出的异常语义只停留在地形 / 装饰层。

## 阶段总览

调研验证日期：2026-07-09。本 PR 只提交问题报告与可执行 plan，不落地 runtime 代码；所有阶段状态均为未开始。

| 阶段 | 状态 | 文档抓手验证 | 实施验收 | 可核验抓手 |
|---|---|---|---|---|
| P0 anomaly runtime contract | ⬜ | 2026-07-09 | 未验收 | `server/src/world/anomaly_raster.rs`、`AnomalyKind`、`AnomalyRuntimeContract`、`ANOMALY_TRIGGER_THRESHOLD` |
| P1 runtime consumer | ⬜ | 2026-07-09 | 未验收 | `tick_anomaly_raster_hotspots`、`TerrainProviders::for_dimension`、`ColumnSample.anomaly_intensity/anomaly_kind`、`VfxEventRequest` |
| P2 去重与边界守护 | ⬜ | 2026-07-09 | 未验收 | `neg_pressure + portal_anchor_sdf`、`tsy_poi_consumer::register`、`ZongCoreActivationV1`、cooldown key |
| P3 pin tests / harness / cross-crate contract | ⬜ | 2026-07-09 | 未验收 | `world::anomaly_raster::tests::*`、`raster_check.py`、`bong:anomaly_hotspot_triggered` schema / Redis sample |

## 接入面

- **进料**：`worldgen/scripts/terrain_gen/fields.py` 的 `anomaly_intensity` / `anomaly_kind` layer；`worldgen/scripts/terrain_gen/bakers/raster_export.py` 的 `manifest.anomaly_kinds`；Rust `server/src/world/terrain/raster.rs` 的 `TerrainProviders` / `TerrainProvider::sample` / `sample_layer_f32` / `sample_layer_u8` / `ColumnSample.anomaly_intensity` / `ColumnSample.anomaly_kind`。
- **出料**：server 侧产生 `AnomalyHotspotTriggered`（本 plan 新增 event）并分流到 `network::vfx_event_emit::VfxEventRequest`、NPC spawn intent、world event/agent bridge；跨仓 payload 固定为 `bong:anomaly_hotspot_triggered`，如 P1 确认需要 agent 消费则同步 `server/src/schema/` 与 `agent/packages/schema/src/`。
- **共享类型 / event**：复用 `DimensionKind` / `CurrentDimension` / `TerrainProviders`；不复刻 `neg_pressure`、TSY POI、九宗阵核激活已有 event。
- **跨仓库契约**：server 新 schema `AnomalyHotspotTriggeredV1`；agent schema 同名 export；client 仅消费 VFX/audio/HUD payload 时再补对应 visual registry，不在 P0 预造资源。
- **worldview 锚点**：异常热点来自已落地的古战场 / 渊口 / 焦土 / TSY / 九宗地形语义；实现时不得引入旧称“练气 / 筑基 / 金丹 / 元婴”，货币与掉落仍遵守骨币正典。
- **qi_physics 锚点**：P1 默认只发事件 / VFX / spawn intent，不搬运真元。若某 kind 后续产生抽吸、衰减、释放或环境转移，必须接 `qi_physics::ledger::QiTransfer`，不得在 anomaly consumer 内自写衰减常数。

## Bug 摘要

- `worldgen/scripts/terrain_gen/fields.py:285-292` 将 `anomaly_intensity` / `anomaly_kind` 定义为 Agent / event / themed mob / FX hook，`0..5` 分别代表 none / spacetime_rift / qi_turbulence / blood_moon_anchor / cursed_echo / wild_formation。
- `worldgen/scripts/terrain_gen/bakers/raster_export.py:343-350` 导出 `manifest.anomaly_kinds`，并在 `:373-374` 写明强度超过阈值后由事件系统触发 themed spawns / FX。
- 多个 profile 实际写入该层；最直接例子是 `worldgen/scripts/terrain_gen/profiles/ancient_battlefield.py:113-115` 明说 anomaly 驱动 event spawns，并在 `:322-323` 写出 `anomaly_intensity.bin` / `anomaly_kind.bin`。
- Rust raster reader 已接线：`server/src/world/terrain/raster.rs:268-273` 在 `ColumnSample` 暴露 anomaly 字段，`:1008-1009` 从 tile mmap 读取，`:1022-1043` 也进入 dynamic layer adapter。
- 但 server 业务侧没有统一 consumer。全仓 grep 只命中 terrain reader/default/test、schema/Redis 测试、`worldgen::pseudo_vein` / `worldgen::zong_formation` 的独立状态模型；没有 NPC spawn、VFX、tick system、agent bridge、interaction 或 world event system 从 `TerrainProvider` / `sample_layer_*` 消费 raster anomaly。

## 实际游玩体验影响

玩家进入古战场或其他异常 profile 时，地图上可以看到部分静态装饰（例如 `flora_variant_id` 让诅咒碑、阵核残件可见），但 `anomaly_kind=3/4/5` 这类热点不会触发血月锚点、诅咒回响、野化阵法等 runtime 事件、怪物压力、FX 或交互反馈。结果是地形语义暗示“这里有异常”，实际游玩却只有静态景物，世界事件读不出玩家正踩在异常热点上。

## 非范围与避重

- 不声称 `rift_mouth` 完全断：`server/src/cultivation/neg_pressure.rs:3-6` 明确消费 `neg_pressure + portal_anchor_sdf`，`:138-161` 会扣真元并发 `frost_breath`。
- 不声称 TSY POI 完全断：`server/src/world/tsy_poi_consumer.rs:1-5` 说明从 `TerrainProviders.{overworld,tsy}.pois()` 生成 portal / container / NPC anchor，`:78-132`、`:220-305` 有实际消费。
- 不声称九宗阵核激活完全断：`server/src/worldgen/zong_formation.rs:148-176` 是显式激活事件并写 `anomaly_kind=5`，但它不是扫描静态 raster anomaly 热点。
- 不重复 #1103：该项是 `mineral_density/mineral_kind` 没进入 `MineralOreNode/MineralOreIndex`。
- 不重复 #1120：该项是 `spirit_eye_candidates` 没进入 `SpiritEyeRegistry`。
- 不重复 #1098：该项是伪灵脉 active/dissipate 的 Redis / Agent deadwire，不是静态 raster anomaly consumer。

## P0 anomaly runtime contract

目标：把 `anomaly_intensity/anomaly_kind` 从注释语义变成 server 可执行 contract，同时明确哪些 kind 只提示、哪些 kind 可触发 runtime。

- 新增 `server/src/world/anomaly_raster.rs`，定义 `enum AnomalyKind { None, SpacetimeRift, QiTurbulence, BloodMoonAnchor, CursedEcho, WildFormation }`、`struct AnomalyRuntimeContract`、`fn anomaly_kind_from_u8(kind: u8) -> Option<AnomalyKind>`。
- 常量只放一处：`ANOMALY_TRIGGER_THRESHOLD` 初始对齐 raster export notes 的 `0.3`，`ANOMALY_SAMPLE_RADIUS_BLOCKS`、`ANOMALY_COOLDOWN_TICKS`、`ANOMALY_MAX_EVENTS_PER_TICK` 由本模块统一导出；禁止在 consumer / tests 里另写魔法数。
- contract 表逐 kind 写明触发等级、维度过滤、是否允许 spawn intent、是否允许 agent signal、默认 VFX event id：`bong:anomaly_qi_turbulence` / `bong:anomaly_blood_moon_anchor` / `bong:anomaly_cursed_echo` / `bong:anomaly_wild_formation`。
- P0 测试：`world::anomaly_raster::tests::anomaly_kind_decodes_manifest_values`、`unknown_anomaly_kind_is_rejected`、`contract_threshold_matches_raster_export_note`。

## P1 runtime consumer

目标：新增只读扫描系统，从玩家 / 活跃区域周围采样 raster anomaly，并把高强度 hotspot 转成 runtime 事件、VFX、spawn intent 或 agent signal。

- 在 `server/src/world/anomaly_raster.rs` 实现 `tick_anomaly_raster_hotspots`，输入 `Option<Res<TerrainProviders>>`、`Query<(&Position, Option<&CurrentDimension>)>`，通过 `providers.for_dimension(dimension)` 取 `TerrainProvider`，读取 `provider.sample(x, z).anomaly_intensity/anomaly_kind`。
- 新增 event `AnomalyHotspotTriggered { dimension: DimensionKind, block_xz, kind: AnomalyKind, intensity, source_layer: "anomaly_raster" }`；P1 注册点放在 `server/src/world/mod.rs`，确保 `setup_world` 插入 `TerrainProviders` 后再运行。
- VFX 输出走 `network::vfx_event_emit::VfxEventRequest`，按 kind 发独立 `VfxEventPayloadV1::SpawnParticle` event id；spawn 输出只发 intent，不直接生成 vanilla entity。
- 如需要 agent 叙事，Redis/schema key 固定为 `bong:anomaly_hotspot_triggered`，payload 由 `server/src/schema/anomaly.rs` 与 `agent/packages/schema/src/anomaly.ts` 镜像；schema 变更后必须重建 `@bong/schema` dist。
- P1 测试：mock `TerrainProviders` 或最小 `TerrainProvider` fixture 证明 `kind=5 + intensity>=threshold` 会产生 `AnomalyHotspotTriggered` 和 VFX/spawn/agent intent；`kind=0`、低强度、缺 provider、缺 dimension provider 均 no-op。

## P2 去重与边界守护

目标：统一 anomaly 扫描器只补静态 raster anomaly deadwire，不吞并已经成立的专用链路。

- `AnomalyCooldownKey = (DimensionKind, chunk_x, chunk_z, AnomalyKind)`；同一 key 在 `ANOMALY_COOLDOWN_TICKS` 内只触发一次，避免玩家站在热点上每 tick 刷事件。
- `AnomalyKind::SpacetimeRift` 若同列满足 `sample.portal_anchor_sdf <= cultivation::neg_pressure::HOTSPOT_RADIUS_BLOCKS`，runtime 抽吸继续交给 `tick_neg_pressure`，统一 consumer 最多发非伤害性提示 / VFX，不重复扣真元。
- TSY portal / container / NPC anchor 继续由 `server/src/world/tsy_poi_consumer.rs::register` 消费 `TerrainProviders.{overworld,tsy}.pois()`；anomaly consumer 不解析 POI tag，也不生成 `RiftPortal` / `LootContainer` / `NpcAnchor`。
- 九宗阵核玩家激活继续走 `server/src/worldgen/zong_formation.rs::activate_zong_formation_core` 与 `ZongCoreActivationV1`；静态 `anomaly_kind=5` 只能触发 ambient/wild-formation pressure，不伪造玩家付费激活事件。
- P2 测试：`rift_portal_anchor_sdf_does_not_duplicate_neg_pressure_drain`、`tsy_poi_markers_are_not_spawned_by_anomaly_consumer`、`zong_core_activation_schema_not_forged_by_static_hotspot`。

## P3 pin tests / harness / cross-crate contract

目标：把 deadwire 修复锁成回归测试，并证明 worldgen 输出、server reader、runtime consumer、schema/Redis 契约互相对齐。

- server：
  - `cd server && CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 nice -n 15 cargo fmt --check`
  - `cd server && CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 nice -n 15 cargo clippy --all-targets -- -D warnings`
  - `cd server && CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 nice -n 15 cargo test world::anomaly_raster`
  - `cd server && CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 nice -n 15 cargo test world::terrain`
  - `cd server && CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 nice -n 15 cargo test worldgen`
- worldgen：`python -m scripts.terrain_gen`、`bash worldgen/pipeline.sh`，并用 `worldgen/scripts/terrain_gen/harness/raster_check.py` pin `anomaly_kind` 值域、`manifest.anomaly_kinds` 字典、`anomaly_intensity.bin` / `anomaly_kind.bin` 存在性。
- agent/schema：若 P1 新增 `AnomalyHotspotTriggeredV1`，运行 `cd agent && npm run build -w @bong/schema` 与 `cd agent/packages/schema && npm test`，并确认 `packages/tiandao` 引用 dist export 不崩。
- 联调：在 `ancient_battlefield` / `tribulation_scorch` / TSY anomaly zone 附近移动，确认静态装饰之外会出现对应 runtime 事件、FX 或 spawn 反馈，且 rift 负压和 TSY POI 不被重复触发。

## 对抗复核

- Round 1：反方指出 rift mouth、TSY POI、九宗激活已有独立 runtime，不能把候选写成“所有异常玩法断”。裁决：收窄为静态 raster anomaly consumer 缺失后仍成立。
- Round 2：反方指出 `flora_variant_id` 已让部分异常热点可见，不能说完全无呈现。裁决：静态装饰不等于 `anomaly_*` 承诺的 event/spawn/FX/交互 consumer；候选仍成立，但必须排除装饰可视化和既有独立链路。

## Finish Evidence

本 plan 尚未实施，禁止归档。本节在 P0-P3 全部完成后再追加真实落地清单、关键 commit、测试结果、跨仓库核验与遗留限制。
