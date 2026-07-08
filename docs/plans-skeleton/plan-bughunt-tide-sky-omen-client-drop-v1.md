# plan-bughunt-tide-sky-omen-client-drop-v1（骨架）

> **骨架（草案）**。一句话主题：`server/src/world/heartbeat.rs` 已把 **汐转天象预兆** 作为独立 `bong:world_omen_tide_sky` VFX 事件稳定发出，但 client 侧 `VfxBootstrap` / `OmenParticlePlayer` / `OmenStateStore` 仍停在 4 种旧 omen，导致 **兽潮前的“汐转天象”粒子 + HUD 预兆整条链路静默丢失**。这不是数值问题，也不是资源缺图，而是 **新增 event_id 没接进 client closed-set**。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|---|---|---|---|
| P0 | `world_omen_tide_sky` client 侧完全漏接 | fix_pr | ⬜ |

## P0 — `world_omen_tide_sky` client 侧完全漏接

- **复现路径**：
  1. 服务端 `WorldHeartbeat` 定义了第 5 种 omen：`OmenKind::TideSkyTurning`，其 `vfx_event_id()` 明确返回 `VFX_WORLD_OMEN_TIDE_SKY = "bong:world_omen_tide_sky"`（`server/src/world/heartbeat.rs:94-110`，常量见 `:51`）。
  2. `emit_omen_vfx()` 会把该 omen 真的打成 `VfxEventRequest::SpawnParticle` 发给客户端，带 `event_id/color/strength/count/duration_ticks`；`duration_ticks = OMEN_VISUAL_DURATION_TICKS = 200`，也就是 **10 秒可见窗口**（`server/src/world/heartbeat.rs:60,1666-1686`）。
  3. 服务端测试已钉住这不是死代码：`tide_sky_omen_consumes_xizhuan_boundary_and_rhythm_timing` 与 `heartbeat_tick_fires_tide_sky_omen_into_recent_events` 证明汐转边界会排队并实打实进入运行时 recent event（`server/src/world/heartbeat.rs:2847-2938`）。
  4. client 收到 `spawn_particle` 后走 `VfxEventRouter`；若 `particleBridge.spawnParticle(...)` 返回 `false`，整条消息会被判成 `bridgeMiss`，调用方只记 warn，不会有任何视觉降级 fallback（`client/src/main/java/com/bong/client/network/VfxEventRouter.java:18-27,64-116`；`BongNetworkHandler.logVfxBridgeMiss` 见 `client/src/main/java/com/bong/client/BongNetworkHandler.java:901-905`）。
  5. `BongVfxParticleBridge` 的查表是 **exact match**：先 `registry.lookup(payload.eventId())`，唯一特判只有 botany stage route；找不到就直接 `orElse(false)`（`client/src/main/java/com/bong/client/visual/particle/BongVfxParticleBridge.java:33-53`）。
  6. 但 client 端 omen closed-set 仍只有 4 条旧 id：`OmenParticlePlayer` 只声明并处理 `pseudo_vein / beast_tide / realm_collapse / karma_backlash`（`client/src/main/java/com/bong/client/visual/particle/OmenParticlePlayer.java:9-26,58-70`）；`VfxBootstrap` 也只注册这 4 条（`client/src/main/java/com/bong/client/visual/particle/VfxBootstrap.java:62-66`）；`OmenStateStore.kindFromEventId()` 同样只识别这 4 条（`client/src/main/java/com/bong/client/omen/OmenStateStore.java:62-93`）。
  7. 因此 `bong:world_omen_tide_sky` 在 client 上既**没有 VfxPlayer**，也**不会写入 OmenStateStore**，最终 `BongHudOrchestrator` 每帧调用 `OmenHudPlanner.buildCommands(OmenStateStore.snapshot(...))` 时根本拿不到这条预兆（`client/src/main/java/com/bong/client/hud/BongHudOrchestrator.java:195-200`；`client/src/main/java/com/bong/client/hud/OmenHudPlanner.java:12-67`）。

- **根因链路**：
  - server 的 omen 集合已经扩成 5 种，但 client 的 omen 集合仍是手写的 4 值 closed-set。
  - 这条链不是 schema 驱动枚举扩展，而是 Java 侧三处手抄白名单同时漏了一项：`OmenParticlePlayer` 常量、`VfxBootstrap` 注册表、`OmenStateStore.kindFromEventId()`。
  - `VfxRegistry` / `BongVfxParticleBridge` 又是严格 exact-match，未知 `event_id` 不会走“同前缀共享播放器”的宽松 fallback，所以新 omen 会被 **静默打成 bridgeMiss**。

- **影响面**：
  - `plan-world-heartbeat-v1` 明确把 `OmenHudPlanner` / `OmenParticlePlayer` 作为“环境预兆系统”的 client surface（`docs/finished_plans/plan-world-heartbeat-v1.md:56-57,465`）；`tide_sky_omen` 现在却是 server 在发、client 不画。
  - 受影响的不是单一粒子，而是整套“汐转天象”预警：没有预兆粒子、没有边缘/色调 HUD、没有任何本地视觉反馈。
  - `/spawnp` 调试也会直接把 `bong:world_omen_tide_sky` 判成“未注册的粒子事件”（`client/src/main/java/com/bong/client/debug/BongSpawnParticleCommand.java:147-155`），说明问题不是某个 runtime 分支没走，而是注册表根本没有这条。

- **这个 bug 对实际游玩体验的影响**：
  - 玩家在 **兽潮前的汐转阶段** 本应先看到“天象转向”的预兆，再决定要不要撤离、回城、收手采集；现在这条预警在 client 端完全消失。
  - 体感上会变成：`recent_events` / narration / 世界行为已经进入兽潮前摇，但屏幕没有任何对应的“风向变了、天空不对劲了”的可感知信号，世界心跳少了一截，老玩家也失去本该依赖的抢跑窗口。
  - 这会直接削弱 `plan-world-heartbeat-v1` P2 想要的“事件前 N 分钟先给环境信号、不是弹 UI 提示”的设计目标。

- **修复建议**：
  - client 侧补齐第 5 种 omen：在 `OmenParticlePlayer` 增加 `world_omen_tide_sky` 常量与颜色/粒子 provider 选择；在 `VfxBootstrap` 注册它；在 `OmenStateStore.Kind` 与 `kindFromEventId()` 增加 `TIDE_SKY`（或命名与 server `TideSkyTurning` 对齐）。
  - `OmenHudPlanner` 补一档独立视觉语义，建议走“地平线色带 + 轻量边缘泛黄/灰褐 tint”，避免与 `BEAST_TIDE` / `REALM_COLLAPSE` 现有效果撞色。
  - 回归测试至少补 3 条：`VfxRegistry.contains(world_omen_tide_sky)`、`OmenStateStore.kindFromEventId(world_omen_tide_sky)`、`OmenHudPlanner` 对新 kind 产生命令。

## 反方裁决（当前会话无可用 subagent，退化为人工两轮反方裁决）

### Round 1

- **反方论点**：`tide_sky_omen` 也许本来就只想进 `recent_events` / narration，不一定要求 client 画出来；server 发 VFX 只是预留接口。
- **驳回理由**：
  - `emit_omen_vfx()` 不是“预留常量”，而是已经实打实构造 `VfxEventPayloadV1::SpawnParticle` 并发包（`server/src/world/heartbeat.rs:1675-1685`）。
  - `plan-world-heartbeat-v1` 也把 `OmenHudPlanner` / `OmenParticlePlayer` 明写为 P2 client surface（`docs/finished_plans/plan-world-heartbeat-v1.md:56-57,465`）。如果只想走 narration，server 根本不该发 `bong:vfx_event`。

### Round 2

- **反方论点**：也许 client 对未知 omen id 有通用 fallback，不注册 `world_omen_tide_sky` 也会复用现有 omen 播放器。
- **驳回理由**：
  - `BongVfxParticleBridge` 先做 `registry.lookup(payload.eventId())`，唯一 fallback 只给 botany stage route（`client/src/main/java/com/bong/client/visual/particle/BongVfxParticleBridge.java:47-52`）；omen 前缀没有任何泛化逻辑。
  - `OmenParticlePlayer` / `VfxBootstrap` / `OmenStateStore` 三处都只列了 4 个旧 id（对应行见上），`client` 全树 grep `world_omen_tide_sky` 为 0 命中；未知 id 只会落入 `bridgeMiss`，然后被 `BongNetworkHandler` warn 掉（`client/src/main/java/com/bong/client/BongNetworkHandler.java:901-905`）。

## 审计来源

bug-hunt 2026-07-05（client visual/render/effect 聚焦，避开 toast cross-session、weather overlay collapse、ash spider nametag leak、preview pause）。本轮从 `world heartbeat omen` 的 server→client 接线闭环入手，确认 `world_omen_tide_sky` 是 **server 已发、client 漏接** 的高置信真 bug。当前会话无 subagent/delegate 能力，已按要求在文档中如实记录退化处理，并完成两轮人工反方裁决。
