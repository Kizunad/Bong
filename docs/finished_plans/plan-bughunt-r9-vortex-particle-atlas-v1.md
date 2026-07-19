# plan-bughunt-r9-vortex-particle-atlas-v1（Finished）

> **一句话主题**：补齐 `vortex_spiral` 粒子贴图的 Minecraft 1.20.1 atlas `single` source，并以通用方向性对账与涡流 exact-ID pin 锁住 descriptor→atlas、descriptor→PNG、atlas→PNG 的文件存在性/引用一致性（不做 PNG 解码；不覆盖 atlas→descriptor / 孤立 PNG）。
> 来源：`docs/plans-skeleton/plan-bughunt-r9-findings-v1.md` 的 P1 #2；本 plan 未消费、移动或修改 round9 聚合 skeleton。

## 阶段总览

| 阶段 | 状态 | 可核验结果 |
|---|---|---|
| P0 atlas / 资产链修复 | ✅ 2026-07-03 | `vortex_spiral.json` 引用 `bong:particle/vortex_spiral`，PNG 文件存在，`particles.json` 以 `single` source 登记同一资源；当前树静态人工复核该 PNG 为 32×32 RGBA 且含非透明像素 |
| P1 防回归测试 | ✅ 2026-07-07 | `ParticleAtlasReconciliationTest` 做方向性检查（descriptor→atlas、descriptor→PNG、atlas→PNG）；`VortexSpiralParticleAssetTest` exact pin 涡流 descriptor 路径 / `TEXTURE_ID` / atlas `single` source |
| P2 exact-head CI / 合并验收 | ✅ 2026-07-08 | PR #1079 final head `3cb86210…` 的 Java 17 client test 与完整 e2e job 成功，随后合入 `main` |

## 背景与第一性原理结论

Minecraft 1.20.1 不会因为 `assets/bong/particles/<name>.json` 存在就自动把其贴图缝入粒子图集。`vortex_spiral` 可见必须同时满足：

1. `assets/bong/particles/vortex_spiral.json` 的 `textures` 引用 `bong:particle/vortex_spiral`；
2. `assets/bong/textures/particle/vortex_spiral.png` 是真实存在的 PNG（生产上还需要含可见像素；见下「静态人工复核」）；
3. `assets/minecraft/atlases/particles.json` 的 `sources` 含 `{"type":"single","resource":"bong:particle/vortex_spiral"}`；
4. client factory 取得该 atlas sprite，并由实际 VFX player 赋给粒子。

缺少第 3 项时，上游 event、registry、factory 与粒子参数都可正常执行，但 sprite 不会被烘焙进 atlas，玩家看到的是静默空白。PR #838 的 `754b8c3f` 已修复该根因并加入全局方向性对账；PR #1079 的 `3cb86210` 再以涡流 exact-ID pin 防止 `vortex_spiral` 被单独改名或漏登记 atlas。

**自动测试契约边界（与生产可见性要求区分）**：现有 4 个 asset test **只**断言普通文件存在、descriptor exact texture ID、以及下列方向性链路；**不解码 PNG**，不锁 32×32 / RGBA / 非全透明，也不保证运行时画面非空白。32×32 RGBA 与非透明像素属于归档时的**静态人工复核事实**，不是 JUnit 契约。

当前生产代码已经从历史共用事件 `bong:vortex_spiral` 演进为逐招专属 event ID，不能把该历史 ID 表述为当前唯一生产入口：

- server `combat::woliu_v2::skills::visual_for` 为基础五招、真空五招与虚蚀路径五招分别返回专属 `particle_id`；`network::vfx_animation_trigger::emit_woliu_v2_visual_triggers` 将实际 `event.visual.particle_id` 写入 `SpawnParticle`。
- client `VfxBootstrap.registerDefaults()` 把上述专属 ID、绝灵涡流 v1 三态 ID，以及保留注册的 `VortexSpiralPlayer.EVENT_ID` 全部绑定到同一个 `VortexSpiralPlayer`。
- `VortexSpiralPlayer` 的各条渲染路线共同读取 `BongParticles.vortexSpiralSprites`；`BongParticles.registerClient()` 的 `VORTEX_SPIRAL` factory 也从同一 `SpriteProvider` 取 sprite。因此这些专属 event ID 最终仍共享本 plan 锁定的 `vortex_spiral` atlas 资产。

## 范围与验收

### P0 — atlas / 资产链修复 — ✅ 2026-07-03

- `client/src/main/resources/assets/bong/particles/vortex_spiral.json` 存在，唯一 texture entry 为 `bong:particle/vortex_spiral`。
- `client/src/main/resources/assets/bong/textures/particle/vortex_spiral.png` 存在。
- **静态人工复核（非自动测试）**：当前文件为 32×32、8-bit RGBA、851 bytes，含 250 个 alpha > 0 的像素，不是空白占位。
- `client/src/main/resources/assets/minecraft/atlases/particles.json` 以 exact `single` source 登记 `bong:particle/vortex_spiral`。

### P1 — 防回归测试 — ✅ 2026-07-07

四个 JUnit case 的真实契约如下（均**不**读 PNG 像素/格式）：

- `VortexSpiralParticleAssetTest::vortexSpiralDescriptorReferencesCommittedTexture`：`vortex_spiral.json` 与对应 PNG 均为普通文件，并 pin descriptor 的 exact texture ID `bong:particle/vortex_spiral`。
- `VortexSpiralParticleAssetTest::vortexSpiralTextureIsRegisteredAsParticleAtlasSingleSource`：解析 atlas，要求 exact `type=single` + exact resource `bong:particle/vortex_spiral`。
- `ParticleAtlasReconciliationTest::everyReferencedParticleTextureIsWhitelistedInAtlasAndHasPng`：对所有粒子 descriptor **正向**检查：引用的 `bong:particle/<name>` 已在 atlas 登记（descriptor→atlas），且 `textures/particle/<name>.png` 存在（descriptor→PNG；`Files.exists`，不解码）。
- `ParticleAtlasReconciliationTest::atlasWhitelistHasNoDanglingEntries`：拒绝 atlas 中没有对应 PNG 文件的悬空 `bong:particle/*` 条目（atlas→PNG）。

**方向性范围（通用对账）**：

- **已覆盖**：descriptor→atlas、descriptor→PNG、atlas→PNG。
- **未覆盖**：atlas→descriptor（atlas 有登记、无任何 descriptor 引用仍可通过，只要 PNG 在）；孤立 PNG（既无 descriptor 引用、也不在 atlas 的 PNG 不会被检测）。

**重命名边界**：

- **通用对账单独**（`ParticleAtlasReconciliationTest`）：只保证上述方向性链路不破；若 descriptor、atlas 与 PNG **三者整体同步**改成同一新 ID，通用对账仍可通过，**不会**因「离开 `vortex_spiral` 这个名字」而失败。
- **exact-ID pin 保持不变时**（`VortexSpiralParticleAssetTest`）：硬编码 descriptor 路径、`TEXTURE_ID` 与 atlas `single` source exact pin；即便三者整体同步改名，只要专属测试未改，仍会撞红，从而阻止整体改名。
- **若维护者同时显式更新** `VortexSpiralParticleAssetTest` 的路径/`TEXTURE_ID`/atlas pin：那是契约迁移（有意改名并同步测试），不是「测试能力缺失」。

### P2 — exact-head CI / 合并验收 — ✅ 2026-07-08

- PR #1079：<https://github.com/Kizunad/Bong/pull/1079>；final head `3cb86210e0f8c02053e627b59d1d53fec499b896`，merge commit `1294cfc0b3bfff7b30af7751e85b1f57339e03da`，GitHub `mergedAt=2026-07-08T16:54:14Z`。
- exact-head Actions run `28849850549` / job `85562238335` 的唯一 GitHub Actions check `e2e` 为 `SUCCESS`。原始 job log 明确记录 `java-version: 17`、执行 `./gradlew test`，并以 `BUILD SUCCESSFUL in 1m 3s` 收口；因此本 PR 新增的专属 pin 与既有通用方向性对账测试均在 final head 的完整 client test stage 内执行成功。
- 同一 job 的 proto、schema、agent、server test、smoke/e2e 与 protocol bot steps 均为 success。唯一 check annotation 是 actions Node.js 20 弃用 warning，不是代码或测试失败。
- artifact `e2e-evidence`（ID `8131552579`，SHA-256 digest `1afc31a38c55dd3b5cbdcd5148d24ef91a6523534449e6328f99cc5d18ebacf3`）仍未过期；实查含 18 个 server / agent / Redis / bot 日志与两个 success marker。它是全链 e2e 证据，不含涡流画面或 Gradle XML，故不将其伪称为粒子视觉 artifact。
- final head 的 `CodeRabbit` status 为 `FAILURE`，description 是 `Prepaid credits exhausted — enable usage-based reviews`；bot 评论也明确写 review limit reached、review 未启动。PR review 列表为空，因此历史 PR 没有 CodeRabbit PASS 或正式 review verdict，不能把这项额度/infra 状态写成代码失败，也不能伪写成 review 成功。
- PR body 自报 targeted 4 tests 与当时 read-only validator PASS；归档的可复核测试结论优先采用上述 exact-head GitHub Actions job，不把 PR body 自述或普通评论当作外部 check。

## 非范围

- `ash_spider_disguised.png` 属于 round9 另一独立发现，需要 `/gen-image` 或正式美术资产；它不是本 `vortex_spiral` atlas plan 的未交付物。
- 不批量补技能图标，不修改 server 技能数值、VFX 参数、particle factory 或现有专属 event-ID 设计。
- 本 plan 不要求新增游戏内截图；不新增 PNG 解码/像素级测试。验收对象是 atlas 接线、方向性文件存在性/引用对账、exact-ID pin，以及生产引用链（像素可见性仅静态人工复核）。
- 通用对账不覆盖 atlas→descriptor 或孤立 PNG；亦不把「维护者显式更新 exact-ID pin 测试」算作测试缺口。

## Finish Evidence

### 落地清单

- **资源**：`client/src/main/resources/assets/bong/particles/vortex_spiral.json`、`client/src/main/resources/assets/bong/textures/particle/vortex_spiral.png`、`client/src/main/resources/assets/minecraft/atlases/particles.json`。
- **通用方向性对账（descriptor→atlas / descriptor→PNG / atlas→PNG；不解码 PNG）**：`client/src/test/java/com/bong/client/visual/particle/ParticleAtlasReconciliationTest.java`。
- **exact-ID pin（涡流专属路径 / `TEXTURE_ID` / atlas single source）**：`client/src/test/java/com/bong/client/visual/particle/VortexSpiralParticleAssetTest.java`。
- **生产消费链**：`server/src/combat/woliu_v2/skills.rs::visual_for` → `server/src/network/vfx_animation_trigger.rs::emit_woliu_v2_visual_triggers` → `server/src/network/vfx_event_emit.rs::emit_vfx_event_payloads` → `bong:vfx_event`；client 侧经 `VfxEventEnvelope::parseSpawnParticle` → `VfxEventRouter` → `BongVfxParticleBridge` → `VfxRegistry` → `VortexSpiralPlayer` → `BongParticles.vortexSpiralSprites`。

### 关键 commit

- `5ecd3488ce4051b616ce35cf277df40385f89a66`（2026-05-09，PR #173）— 初始涡流技能、生产粒子 player 与 `vortex_spiral` descriptor/PNG 资产落地。
- `754b8c3fa5ca9c95ac9c3327c0f546fd15e9a62f`（2026-07-03，PR #838）— 补 `bong:particle/vortex_spiral` atlas `single` source、同步资源包 manifest，并加入 `ParticleAtlasReconciliationTest`。
- `3e49f189be53ce7b2ce4c52baf6dfaf7f9e4ebef`（2026-07-07，PR #1079）— 从 round9 聚合发现中拆出本 focused plan。
- `3cb86210e0f8c02053e627b59d1d53fec499b896`（2026-07-07，PR #1079 final head）— 新增 `VortexSpiralParticleAssetTest` exact-ID pin。
- `1294cfc0b3bfff7b30af7751e85b1f57339e03da`（GitHub mergedAt 2026-07-08）— PR #1079 合入 `main`。

### 测试结果

- 历史 exact-head CI：Actions run `28849850549` / job `85562238335` 在 `3cb86210e0f8c02053e627b59d1d53fec499b896` 上使用 Java 17 执行完整 `./gradlew test`，`BUILD SUCCESSFUL in 1m 3s`；job 总结论 success。
- final-head 全链 CI：proto lint/breaking、schema、agent、server release build/test、smoke/e2e、protocol bot e2e 与 artifact upload 全部 success。
- 当前树复核：
  - **自动测试契约（4 case）**：descriptor/PNG 普通文件存在 + exact texture ID + 方向性链路（descriptor→atlas、descriptor→PNG、atlas→PNG）；**不做 PNG 解码**；不覆盖 atlas→descriptor / 孤立 PNG。
  - **静态人工复核（非测试）**：descriptor/atlas JSON 可解析且 exact ID 一致；PNG 当前为 32×32 RGBA 且含非透明像素。

### 跨仓库核验

- **server**：当前 gameplay 不依赖单一 `bong:vortex_spiral`；`visual_for` 的十五个逐招专属 `particle_id` 由 `emit_woliu_v2_visual_triggers` 写入 `SpawnParticle`，再由 `emit_vfx_event_payloads` 通过 `bong:vfx_event` 广播。
- **client**：`BongClient` 启动时执行 `BongParticles.register()`、`registerClient()` 与 `VfxBootstrap.registerDefaults()`；专属 IDs、保留的 base ID 与 v1 三态 ID 均映射到同一个 `VortexSpiralPlayer`，共享 `vortexSpiralSprites` 和本 plan 的 descriptor / atlas / PNG。
- **schema / agent**：`WoliuSkillCastV1.particle_id` 在 server→Redis→schema/agent 叙事契约中继续透传并有测试，但 agent 不参与 `bong:vfx_event` 的客户端 atlas 烘焙；本 plan 未改 schema 或 agent。
- **worldgen**：无接入面，未改动也不需要验收。

### 遗留 / 后续

- 本 plan 定义的 atlas source、descriptor、PNG 文件存在、exact-ID pin 与通用方向性对账均已交付；PNG 像素/格式可见性仅静态复核，无未完成的自动测试项，也不在本 plan 范围扩 PNG 解码测试。
- 通用对账未覆盖 atlas→descriptor 与孤立 PNG；若后续需要这些方向，另立 plan，不回写为本 plan 缺口。
- `ash_spider_disguised.png` 与技能图标 backlog 仍由各自 plan / 美术流程负责，不阻塞本 plan 归档。
- 历史 PR #1079 没有成功启动 CodeRabbit review，也没有粒子画面 artifact；本归档只声称 exact-head CI、方向性文件存在性/引用对账、exact-ID pin 与生产引用链已核验，不将额度失败或通用 e2e artifact 包装成历史 review / 视觉证明。
- 当前逐招专属 event ID 继续共享 `vortex_spiral` sprite 是有意的资源复用。防回归边界：
  - **通用对账单独**：阻止 descriptor→atlas / descriptor→PNG / atlas→PNG 方向性断裂；**不能**发现三者同步一致改名。
  - **exact-ID pin 未改时**：硬编码 descriptor 路径 / `TEXTURE_ID` / atlas source 会阻止整体改名。
  - **维护者同时更新专属测试**：属于有意契约迁移，不是测试能力缺失。
