# plan-bughunt-world-tide-sky-omen-client-bridge-v1（骨架）

> **骨架（草案）**。一句话主题：world events / event bridge / runtime sidepaths 定向 bug-hunt 命中 1 个高置信真 bug：**`tide_sky_omen` 已在 server `WorldHeartbeat` 排队并发出 `bong:world_omen_tide_sky` VFX，但 client omen 栈仍停在四类旧枚举，导致这条季节性世界预兆对玩家完全静默。**

> 范围说明：本轮刻意避开用户排除题（zone ecology global refuge、zone_info stale、pseudo vein restart loss、locust warning duration drift 等），聚焦 `server/src/world/heartbeat.rs` → VFX/event bridge → client omen sidepath。仅报告，不修代码。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | `tide_sky_omen` client bridge 漏接线 | fix_pr | ⬜ |

## P0 — `tide_sky_omen` 已生产但 client omen 栈漏接线

- **复现路径（源码闭环）**：
  1. `Season::Xizhuan` 边界到来后，`maybe_queue_tide_sky_omen()` 会在满足 cadence / suppression 条件时排入 `OmenKind::TideSkyTurning`（`server/src/world/heartbeat.rs:1220-1295`）。
  2. `queue_omen()` 在入队同时立刻 `emit_omen_vfx()`，把 `event_id = "bong:world_omen_tide_sky"` 发成 `VfxEventPayloadV1::SpawnParticle`（`server/src/world/heartbeat.rs:1635-1687`）；该 event_id 来自 `OmenKind::TideSkyTurning => VFX_WORLD_OMEN_TIDE_SKY`（`server/src/world/heartbeat.rs:47-60,95-120`）。
  3. client 侧 `BongVfxParticleBridge.spawnParticle()` 只有在 `VfxRegistry.lookup(eventId)` 命中时才会派发；查不到就直接 `orElse(false)` 丢弃（`client/src/main/java/com/bong/client/visual/particle/BongVfxParticleBridge.java:33-53`）。
  4. 但当前 omen 栈只覆盖四类旧 omen：`OmenStateStore.Kind` 只有 `PSEUDO_VEIN / BEAST_TIDE / REALM_COLLAPSE / KARMA_BACKLASH`，`kindFromEventId()` 也没有 `"bong:world_omen_tide_sky"`（`client/src/main/java/com/bong/client/omen/OmenStateStore.java:11-17,62-94`）；`OmenParticlePlayer` 常量同样缺 `TIDE_SKY`（`client/src/main/java/com/bong/client/visual/particle/OmenParticlePlayer.java:9-17`）；`VfxBootstrap` 也只注册四条 omen route（`client/src/main/java/com/bong/client/visual/particle/VfxBootstrap.java:62-66`）。
  5. 预兆真正触发时，`fire_due_omens()` 对 `OmenKind::TideSkyTurning` 只写 `recent_events.target = "tide_sky_omen"`，并不会额外给 client 发 `event_alert`/`zone_info` 兜底（`server/src/world/heartbeat.rs:1093-1168`）。因此 player-facing 预兆 surface 完全依赖上面的 omen VFX/HUD 链，而这条链现在对 `tide_sky_omen` 是断的。

- **根因链路**：
  - `WorldHeartbeat` 已从四类 omen 扩展到五类：新增 `HeartbeatEventKind::TideSkyOmen` / `OmenKind::TideSkyTurning` / `VFX_WORLD_OMEN_TIDE_SKY`（`server/src/world/heartbeat.rs:47-120`）。
  - client omen surface 沿用旧的“四类 omen”假设；`plan-world-heartbeat-v1` 归档证据甚至还明确写着 "`VfxBootstrap` 注册四类 omen particle player"（`docs/finished_plans/plan-world-heartbeat-v1.md:491`）。
  - 结果是 server 已生产第五类 omen，client 的 enum、event-id 映射、registry 注册、HUD switch 仍停留在四类实现，形成新增 server event 未回填 client surface 的跨端断链。

- **影响面**：
  - 命中范围：所有 `Season::Xizhuan` 边界触发的 `tide_sky_omen` 预兆。
  - 直接后果：没有 omen 粒子、没有 `OmenStateStore` 记录、没有 `OmenHudPlanner` 视觉反馈。
  - 间接后果：agent/world-state 侧仍会在 `recent_events` 中看到 `tide_sky_omen`，形成“系统知道有预兆、玩家端看不到”的信息不对称。

- **这个 bug 对实际游玩体验的影响**：
  - 玩家在细转季节边界本应收到的世界异象预警被静默吞掉，只会在后续世界事件已经进入下个阶段时被动承受结果。
  - 体感上会出现“天象事件无征兆发生”的割裂：server 与 agent 语义上已经把它当作 world omen，但 client 没有任何可感知 surface，等同于这条预兆不存在。

- **修复建议**：
  - client 侧补全第五类 omen：新增 `OmenStateStore.Kind::TIDE_SKY`，把 `"bong:world_omen_tide_sky"` 接到 `kindFromEventId()`。
  - 为 `OmenParticlePlayer` 增加 `TIDE_SKY` 常量并在 `VfxBootstrap` 注册；补对应 fallback 色与 sprite 选择。
  - `OmenHudPlanner` 增加 `TIDE_SKY` 的 HUD 呈现与 `pulsePeriod()` 分支，否则枚举补齐后会在 switch 处继续缺口。
  - 测试至少补三处：`OmenStateStoreTest` 事件映射、`VfxRegistryTest` bootstrap 注册、`OmenHudPlannerTest` tide sky HUD 输出。

## 两轮反方裁决

> 退化说明：当前会话没有可用 subagent / delegate 工具，无法再开独立反方代理；以下为本地自审两轮反方裁决，论点与驳回理由均显式记录。

### Round 1

- **反方论点**：`tide_sky_omen` 可能只是 server/agent 内部 telemetry，client 不显示是设计使然，不应算 bug。
- **驳回理由**：不成立。`queue_omen()` 明确对它调用 `emit_omen_vfx()`，而且 event_id 被命名进 `bong:world_omen_*` 同一家族（`server/src/world/heartbeat.rs:1635-1687`）；如果本意只是内部 telemetry，没必要走 VFX 粒子通道。现状是生产端主动给 client 发了一个没有任何 consumer 的 event_id，这属于实装后的接线遗漏，不是“未设计 UI”。

### Round 2

- **反方论点**：即使 client 漏了 VFX，`fire_due_omens()` 仍会写 `recent_events`，玩家最终还是能从别处获知，不构成实际游玩问题。
- **驳回理由**：不成立。`recent_events` 是 world-state/agent 侧上下文字段，不是 client HUD 通道；而 `TideSkyTurning` 分支也没有 `event_alert`、`zone_info` 或其它 custom payload 兜底（`server/src/world/heartbeat.rs:1155-1168`）。因此 player-facing 预兆链事实上只有 omen VFX/HUD，这一段断了就等于玩家完全收不到预兆。

## 审计来源

bughunt（2026-07-05，worktree `bughunt-loop-20260705-bo-world-events`）。方法：先对 `world events / event bridge / runtime sidepaths` 做全树 grep，再沿 `heartbeat omen production -> VFX bridge -> client omen registry` 三段闭环复核；确认这是 server 新增第五类 omen 后未同步 client sidepath 的高置信真 bug。报告仅新增 skeleton，不改源码。
