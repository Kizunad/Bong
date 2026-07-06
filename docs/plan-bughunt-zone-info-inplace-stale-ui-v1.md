# plan-bughunt-zone-info-inplace-stale-ui-v1

> **活跃 plan**。一句话主题：`server` 端 `zone_info` 只在**跨 zone 边界**时发一次，导致玩家停留在同一区域时发生的 `realm_collapse` / `tsy_race_out` / `pseudo_vein` / `zone_inflow` 等运行态变化不会刷新 `client` 的 `ZoneState`，进而让 HUD、氛围渲染、环境判定长期停留旧值，直到玩家离区重进或重连。

> 立项动机：这条链路直接落在 `server world state -> server_data.zone_info -> client ZoneState -> HUD/atmosphere` 主干上，影响范围横跨 `server` / `schema` / `client`。它不同于已知的 season stale、preview pause/config、weather overlay collapse、zone atmosphere mismatch；这里的根因不是某个 UI 子模块局部错算，而是**同一区内世界态变化根本没有再次下发到 UI 输入源**。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | `zone_info` 同区变化不刷新，导致 world/ui 跨层 stale | fix_pr | ⬜ |

## P0 — `zone_info` 同区变化不刷新，导致 world/ui 跨层 stale

- **现象**：`server/src/network/mod.rs:2160-2225` 的 `emit_zone_info_on_zone_transition` 以 `last_zone_by_entity` 只比较 **zone 名是否变化**；`2178-2180` 在 `!transitioned` 时直接 `continue`，因此 `2201-2208` 的 `ZoneInfo { zone, spirit_qi, danger_level, status, active_events, perception_text }` 只会在初次进入/跨边界时发送。仓库自带测试也把这个行为钉死了：`server/src/network/mod.rs:4903-4908` 明确断言“`no additional payload should be emitted without a new transition`”。
- **复现路径**：
  1. 玩家进入任意 zone，拿到首包 `zone_info`（`server/src/network/mod.rs:4834-4842`）。
  2. 玩家**不离开该 zone**，让服务端在原地改变该 zone 运行态：
     - `realm_collapse`：`server/src/world/events.rs:2813-2822` 把 `spirit_qi=0.0`、`danger_level` 提升并写入 `EVENT_REALM_COLLAPSE`。
     - TSY race-out：`server/src/world/tsy_lifecycle.rs:423-437` 持续改写 `zone.spirit_qi`，并增删 `EVENT_TSY_RACE_OUT`。
     - 伪矿脉/回流：`server/src/world/heartbeat.rs:1018-1044`、`2148-2159` 会在 zone 内原地修改 `spirit_qi` 与 `active_events`。
  3. 因玩家仍在同一区，`emit_zone_info_on_zone_transition` 不会再次发包；客户端 `ZoneInfoHandler` 也就拿不到新状态。
- **根因链路**：
  1. server 侧 `zone_info` 发包条件被错误收窄成“仅 zone 名变化”，没有把 `spirit_qi / danger_level / status / active_events / perception_text` 视作需要重发的 runtime state。
  2. client 侧 `client/src/main/java/com/bong/client/network/ZoneInfoHandler.java:29-62` 是 `ZoneState` 的唯一正式写入口；`client/src/main/java/com/bong/client/BongNetworkHandler.java:815-877` 只会在 `dispatch.zoneState()` 存在时替换 `BongHudStateStore`。
  3. HUD 与氛围都直接读这份 store：`client/src/main/java/com/bong/client/hud/BongZoneHud.java:104-146` 用它显示“死域 / 负灵域 / 灵气条 / 危险等级 / 无节律 / 感知文案”；`client/src/main/java/com/bong/client/atmosphere/ZoneAtmosphereRenderer.java:49-55` 和 `client/src/main/java/com/bong/client/hud/HudEnvironmentVariant.java:12-34` 用它判定 dead zone、negative qi、TSY 环境。
  4. 替代链路并不能补齐这个缺口：`bong:zone_environment` 只广播 `dimension / zone_id / effects / generation`（`server/src/network/zone_environment_bridge.rs:18-69`、`client/src/main/java/com/bong/client/environment/ZoneEnvironmentState.java:6-36`），不带 `spirit_qi / danger_level / status / active_events`；`event_alert` 只写 toast / `RealmCollapseHudState`（`client/src/main/java/com/bong/client/network/EventAlertHandler.java:50-84`），不会回写 `ZoneState`。
- **这个 bug 对实际游玩体验的影响**：玩家站在原地就能遇到世界态已经变了、UI 却还在说“这里一切正常”的割裂。典型表现包括：zone 已坍缩成死域但 HUD 仍显示旧灵气/旧危险度、负灵域或 TSY race-out 已生效但氛围/边界判定仍沿用旧值、伪矿脉或 zone 回流改变了灵气却不刷新感知文案与灵气条。结果是玩家对当前区域的风险判断、撤离时机、修炼/探索决策都会被 stale UI 误导。
- **建议修复范围 / 模块**：优先收口 `server/src/network/mod.rs` 的 `emit_zone_info_on_zone_transition`。方向上至少要在“同 zone 但 payload 内容变了”时补发，或把 `zone_info` 拆成“enter title”和“runtime snapshot”两类；无论选哪条，都要保证 `spirit_qi / danger_level / status / active_events / perception_text` 的变化能驱动客户端 `ZoneState` 更新，而不是继续绑定在纯边界事件上。
- **验收抓手**：
  1. 玩家停在原地，`zone.spirit_qi` 改变时，client `ZoneState.spiritQiRaw/spiritQiNormalized` 必须刷新。
  2. 同 zone 触发 `realm_collapse` / `tsy_race_out` / `no_cadence` 变更时，HUD 文案和 `HudEnvironmentVariant` 必须切到新态。
  3. `zone_environment` 与 `event_alert` 即使照常广播，也不能再依赖它们“间接弥补” `zone_info` stale。

## 反方裁决摘要

> 退化处理说明：当前会话没有可用的 subagent / delegate 工具，无法按理想流程拉起独立反方代理。以下两轮为主代理自做“默认怀疑”裁决，并把反方论点与驳回理由显式记档。

1. **Round 1 反方论点**：也许 `zone_environment` 或 `event_alert` 已经把同一区状态变化补发给 UI，所以 `zone_info` 只在过边界发并不构成 bug。  
   **驳回理由**：`zone_environment` payload 只有 `dimension/zone_id/effects/generation`，没有 `spirit_qi/danger_level/status/active_events`；`event_alert` 只产出 toast、视觉 hint、`RealmCollapseHudState`，不写 `ZoneState`。两条都不能刷新 HUD/atmosphere 真正依赖的 zone runtime snapshot。
2. **Round 2 反方论点**：也许这是刻意的“进入区域提示”设计，persistent HUD/氛围不需要跟着同区内细粒度变化实时刷新。  
   **驳回理由**：现有 client 实现明确把 `ZoneState` 当作持续态而非一次性 title 使用；`BongZoneHud` 长驻显示灵气/危险度/死域/负灵域/感知文案，`HudEnvironmentVariant` 与 `ZoneAtmosphereRenderer` 也每帧消费它做环境判定。既然 UI 被设计成持续使用这份状态，server 侧却只在边界事件更新它，就不是“有意降频”，而是主链缺失。

## 开放问题

1. `zone_info` 是否应该拆成“`zone_entered` 标题事件”与“`zone_state` 运行态快照”两条协议，避免标题动画与持续态耦在同一 payload 上？
2. `perception_text` 目前依赖“上一个 zone 的 qi”生成；若改为同 zone 内增量刷新，是否需要把“上次已下发 qi”单独记入 tracker，而不是继续复用“上次 zone 名”？

## 审计来源

bug-hunt 定点轮（范围：`world/ui` 跨层链路，优先 `server world state -> client UI`）。方法：全仓 grep + 关键路径人工复核 + 现有测试证据交叉验证；未修改源码，仅新增 skeleton。当前结论为 **report-only**：高置信、可稳定复现、影响主链 UI，建议后续以 fix PR 收口 `zone_info` 的同区刷新语义。
