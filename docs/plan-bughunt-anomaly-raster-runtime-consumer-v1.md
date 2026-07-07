# plan-bughunt-anomaly-raster-runtime-consumer-v1

> **Skeleton（BugHunt report-only）**。一句话主题：worldgen 静态 raster 的 `anomaly_intensity` / `anomaly_kind` 已作为“事件 / spawn / FX hook”导出并被 Rust reader 读入，但 server runtime 没有统一 consumer 扫描这些热点，导致古战场、裂隙、焦土、TSY 等 profile 写出的异常语义只停留在地形/装饰层。

## Bug 摘要

- `worldgen/scripts/terrain_gen/fields.py:285-292` 将 `anomaly_intensity` / `anomaly_kind` 定义为 Agent / event / themed mob / FX hook，`0..5` 分别代表 none / spacetime_rift / qi_turbulence / blood_moon_anchor / cursed_echo / wild_formation。
- `worldgen/scripts/terrain_gen/bakers/raster_export.py:343-350` 导出 `manifest.anomaly_kinds`，并在 `:373-374` 写明强度超过阈值后由事件系统触发 themed spawns / FX。
- 多个 profile 实际写入该层；最直接例子是 `worldgen/scripts/terrain_gen/profiles/ancient_battlefield.py:113-115` 明说 anomaly 驱动 event spawns，并在 `:322-323` 写出 `anomaly_intensity.bin` / `anomaly_kind.bin`。
- Rust raster reader 已接线：`server/src/world/terrain/raster.rs:268-273` 在 `ColumnSample` 暴露 anomaly 字段，`:1008-1009` 从 tile mmap 读取，`:1102` / `:1120` 也进入 dynamic layer adapter。
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

## Skeleton Fix Plan

1. **P0 明确 contract**：为 `anomaly_intensity/anomaly_kind` 定义 server runtime 语义表，至少覆盖阈值、采样半径、触发冷却、维度过滤、与已有 rift/TSY/九宗显式系统的优先级。
2. **P1 runtime consumer**：新增只读扫描/事件系统，从 `TerrainProviders` 对玩家或活跃区域周围采样 anomaly，按 kind 发对应 world event / VFX / spawn intent / agent signal；避免对纯装饰层重复刷。
3. **P2 去重守护**：对 `rift_mouth` 继续走 `neg_pressure + portal_anchor_sdf`，TSY 继续走 POI consumer，九宗玩家激活继续走显式 schema，不把这些链路误并入统一 anomaly 扫描。
4. **P3 测试**：增加 server pin 测试证明一段带 `anomaly_kind=5` 的 raster 会进入 runtime event/spawn/FX intent；同时证明 kind=0、低强度、无 provider、TSY POI、rift 负压路径不会误触发。

## 对抗复核

- Round 1：反方指出 rift mouth、TSY POI、九宗激活已有独立 runtime，不能把候选写成“所有异常玩法断”。裁决：收窄为静态 raster anomaly consumer 缺失后仍成立。
- Round 2：反方指出 `flora_variant_id` 已让部分异常热点可见，不能说完全无呈现。裁决：静态装饰不等于 `anomaly_*` 承诺的 event/spawn/FX/交互 consumer；候选仍成立，但必须排除装饰可视化和既有独立链路。

## 验收建议

- `server/`：`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test world::terrain worldgen`
- `worldgen/`：运行覆盖 anomaly 的 profile 测试与 raster harness，确保 `anomaly_kind` 值域、manifest 字典、tile layer 存在性仍被 pin。
- 联调：在 `ancient_battlefield` / `tribulation_scorch` / TSY anomaly zone 附近移动，确认静态装饰之外会出现对应 runtime 事件、FX 或 spawn 反馈，且 rift 负压和 TSY POI 不被重复触发。
