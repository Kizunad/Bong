# plan-bughunt-r9-vortex-particle-atlas-v1

> 来源：`docs/plans-skeleton/plan-bughunt-r9-findings-v1.md` 的 P1 #2。
> 本 plan 只拆出并处理 `vortex_spiral` 粒子 atlas source / 防回归测试，不消费、不移动 round9 聚合 skeleton。

## 背景

`vortex_spiral` 是涡流招式共用的粒子贴图链路：server 下发 `bong:vortex_spiral` VFX，client 注册粒子类型与 factory，`VortexSpiralPlayer` 使用 `setSprite` 取粒子 sprite，资源包提供 `assets/bong/particles/vortex_spiral.json` 与 `assets/bong/textures/particle/vortex_spiral.png`。

Minecraft 1.20.1 的粒子贴图不会因为 descriptor 存在而自动进图集；`assets/minecraft/atlases/particles.json` 的 `sources` 必须显式包含 `{"type":"single","resource":"bong:particle/vortex_spiral"}`。缺 atlas source 时，类型注册、事件下发、粒子参数都可以正常运行，但 sprite 烘焙阶段拿不到贴图，表现为涡流 VFX 空白。

当前 `origin/main` 已包含 `754b8c3f fix(client): 粒子图集白名单漏登记 — 涡流全系粒子自 #173 起隐形 (#838)`：该提交已补入 `bong:particle/vortex_spiral` atlas source，并加入通用 `ParticleAtlasReconciliationTest`。本 worker 仍补一个涡流专属 pin，避免后续维护时只看通用对账失败而漏掉 P1 #2 的核心用户可见路径。

## 范围

- 确认 `assets/minecraft/atlases/particles.json` 中存在 `bong:particle/vortex_spiral` 的 `single` source。
- 增加 targeted client asset test，直接 pin：
  - `assets/bong/particles/vortex_spiral.json` 存在；
  - descriptor 引用 `bong:particle/vortex_spiral`；
  - `assets/bong/textures/particle/vortex_spiral.png` 存在；
  - atlas single source 登记 `bong:particle/vortex_spiral`。
- 运行 JDK17 下的 targeted client asset test。

## 非范围

- 不处理 `ash_spider_disguised.png`。该项需要 `/gen-image` 或正式美术资产，Codex 不手绘占位；后续状态：`[BLOCKED: 需 /gen-image 生成 client/src/main/resources/assets/bong/textures/entity/fauna/ash_spider_disguised.png]`。
- 不批量补技能图标 PNG。
- 不改 server VFX 事件、技能参数或粒子 factory。

## 验收

- `./gradlew test --tests com.bong.client.visual.particle.VortexSpiralParticleAssetTest` 通过。
- `./gradlew test --tests com.bong.client.visual.particle.ParticleAtlasReconciliationTest` 通过。
- 无上下文 read-only validator 复核 PASS。
