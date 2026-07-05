# plan-bughunt-aa-tide-sky-omen-client-gap-v1（骨架）

> **骨架（草案）**。一句话主题：`server/src/world/heartbeat.rs` 已正式定义并发出 `bong:world_omen_tide_sky`（汐转期天象预兆），但 client 侧 `VfxBootstrap` / `OmenParticlePlayer` / `OmenStateStore` / `OmenHudPlanner` 只接了另外 4 类 omen，导致 **汐转天象这一路在客户端粒子与 HUD 两条主反馈链上完全静默**。影响是：玩家在“回家整理、准备下一趟 run”的关键窗口里收不到这条 30 秒预兆，路线规避与风险判断少一整层直接反馈。

> 立项动机：这是 `spiritwood/scroll/inspect/omen` 扫描里最稳的一处“server 已接好、client 单类漏接”的主路径 bug。它不依赖测试桩或 dev-only 入口；`heartbeat` 正常运行就会在汐转边界排程并触发 `tide_sky_omen`，但本地客户端不会显示对应 omen 粒子，也不会进入 `OmenHudPlanner` 的视觉层。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 汐转天象 omen 客户端漏接 | fix_pr | ⬜ |

## P0 — 汐转天象 omen 客户端漏接

- **server 已正式发射**：`server/src/world/heartbeat.rs` 定义 `VFX_WORLD_OMEN_TIDE_SKY = "bong:world_omen_tide_sky"`，`OmenKind::TideSkyTurning` 明确映射到该 event id，并在 `queue_omen -> emit_omen_vfx` 路径发 `SpawnParticle`。`maybe_queue_tide_sky_omen` 只要进入汐转边界且 cadence 命中，就会把该 omen 排进队列；后续 `heartbeat_tick_fires_tide_sky_omen_into_recent_events` 单测还钉住了它会真正落成运行时 `recent event`，不是死代码。
- **client 漏接是整链条的，不是一处小漏**：
  - `client/src/main/java/com/bong/client/visual/particle/VfxBootstrap.java` 只注册了 `PSEUDO_VEIN / BEAST_TIDE / REALM_COLLAPSE / KARMA_BACKLASH` 四个 omen id，没有 `world_omen_tide_sky`。
  - `client/src/main/java/com/bong/client/visual/particle/OmenParticlePlayer.java` 也只定义了上述四个常量；`tide_sky` 连入口常量都不存在。
  - `client/src/main/java/com/bong/client/omen/OmenStateStore.java` 的 `Kind` 和 `kindFromEventId()` 也只有四类；即使未来有人单补了粒子注册，`tide_sky` 仍不会进入 HUD store。
  - `client/src/main/java/com/bong/client/hud/OmenHudPlanner.java` 只渲染四种 `Kind`；`BongHudOrchestrator` 每 tick 读 `OmenStateStore.snapshot()`，所以 `tide_sky` 当前不可能产出任何边缘微光/色调提示。
- **为什么这是实际游玩 bug**：`server/src/world/event_rhythm.rs` 和 `server/src/world/mod.rs` 都把 `TideSkyOmen` 的首选插入点钉在 `HomeOrganizing`，语义是“玩家在灵龛整理时收到预兆，影响下一趟路线”。`docs/plan-sou-da-che-v1.md` 也把它写成“天空颜色渐变 / NPC 提醒 / 影响下一次 run 的路线规划”。现在 server 仍会排程并记 recent event，但客户端专门负责“天象预兆 HUD 层”的这条链对该 event 完全静默，玩家只能错过这层设计好的 30 秒前兆。
- **建议修复范围**：优先收口 `client/src/main/java/com/bong/client/visual/particle/OmenParticlePlayer.java`、`client/src/main/java/com/bong/client/visual/particle/VfxBootstrap.java`、`client/src/main/java/com/bong/client/omen/OmenStateStore.java`、`client/src/main/java/com/bong/client/hud/OmenHudPlanner.java`。修复时要同时补三件事：`event_id -> VfxPlayer` 注册、`OmenStateStore.Kind` 映射、`HUD` 渲染分支；只补其中一处会继续留下半截孤岛。
- **验收抓手**：至少补 4 组 pin。1) `VfxBootstrap` 已注册 `world_omen_tide_sky`。2) `OmenStateStore.kindFromEventId("bong:world_omen_tide_sky")` 能落到专属 `Kind`。3) `OmenParticlePlayer.play()` 命中该 id 时会写入 store。4) `OmenHudPlanner` 对该 `Kind` 产出非空渲染命令，证明玩家端能看见预兆。

## 反方裁决摘要

1. **Round 1 反方**：这会不会是有意遵守“汐转/季节不显式”的世界观红线，所以故意不在 client 上显示？  
   **裁决**：不能成立。`plan-world-heartbeat-v1` 明文把 `client/OmenHudPlanner` 和 `client/OmenParticlePlayer` 列为“环境预兆系统”交付物；server 还专门为 `TideSkyTurning` 发了独立 `world_omen_tide_sky` 粒子事件。若是故意不显式，server 不该单独造并发射这一路 client omen event；当前更像是五类 omen 里只漏接了一类。
2. **Round 2 反方**：也许玩家仍能从 narration / recent event / 其他系统知道汐转要来了，因此不算实质 bug？  
   **裁决**：即便旁路信息偶尔存在，**这条专用客户端反馈链依然是死的**。全仓搜索表明 client 没有任何 `tide_sky_omen` / `world_omen_tide_sky` 处理点；`BongVfxParticleBridge` 对未注册 event 直接 lookup miss，`OmenStateStore.kindFromEventId()` 也会返回 `null`。因此至少“粒子 + HUD 这两层正式反馈”必然缺失，影响玩家在首选的 `HomeOrganizing` 窗口里做路线决策。

## 开放问题

1. `tide_sky` 的 HUD 表现是否应比其他 omen 更克制一些，以继续贴合“汐转不做硬文本提示”的美术边界？这不影响当前“完全漏接是 bug”的结论，只影响修复时选哪种视觉强度。
2. 是否需要顺手补一个“world heartbeat 所有 `world_omen_*` 常量都已在 client 注册”的对拍测试，防止以后再出现“五种 server omen、client 只接四种”的漂移。

## 审计来源

bughunt 线程 AA，范围限定 `omen` 主路径，人工复核 `server/src/world/heartbeat.rs`、`server/src/world/event_rhythm.rs`、`client/src/main/java/com/bong/client/visual/particle/`、`client/src/main/java/com/bong/client/omen/`、`client/src/main/java/com/bong/client/hud/` 后确认。结论为 **report-only**：先提交 skeleton PR 固化 bug、玩家影响、反方裁决与修复面；本轮不改源码。
