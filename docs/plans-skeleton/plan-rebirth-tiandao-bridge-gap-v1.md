# plan-rebirth-tiandao-bridge-gap-v1（骨架）

> **骨架（草案）**。一句话主题：`bong:rebirth` 的 **server → tiandao** 桥接只做到 publish+subscribe，**没有任何消费端**，导致玩家在**实际成功重生**这一刻缺失本应独立于死亡遗念的天道反馈。

> 立项动机：本轮 bughunt 聚焦 `server/src/network/`、client HUD/VFX/handler、`agent/packages/schema`、`agent/packages/tiandao` 的真实可达桥接断链。经主线自证 + 怀疑方两轮对抗后确认：这是**单向 stub**，不是 wire format 漂移，也不是 client 本地 HUD 能补位的问题。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | `REBIRTH` 契约落点与断链定位 | report_only | ⬜ |
| P1 | tiandao 重生反馈 runtime / consumer 设计 | fix_pr | ⬜ |
| P2 | 回归与去重（不和 `death_insight` / death cinematic 重叠） | fix_pr | ⬜ |

## P0 — 断链定位（confirmed）

- `server/src/combat/lifecycle.rs:1514` 正常重生链路会发 `PlayerRevived`。
- `server/src/network/cultivation_bridge.rs:217-242` 读取 `PlayerRevived`，构造 `RebirthEventV1 { character_id, at_tick, prior_realm, new_realm }`。
- `server/src/network/redis_bridge.rs:925-932` 把 `RedisOutbound::Rebirth` publish 到 `CH_REBIRTH`。
- `agent/packages/schema/src/channels.ts:60-67` 明确把 `REBIRTH` 定义为 **Server → Agent: 重生结算**，与 `DEATH`、`DEATH_INSIGHT` 分开，是独立契约，不是兼容别名。
- `agent/packages/tiandao/src/redis-ipc.ts:152-188` 虽把 `REBIRTH` 放进 `CROSS_SYSTEM_EVENT_CHANNELS`，`713-724` 也确实 subscribe，但 `330-332` / `681-695` 只会落到通用 `recordCrossSystemEvent(...)` 缓冲。
- `agent/packages/tiandao/src/runtime.ts:1264-1279` tick loop 只显式处理 ecology / economy / weather；全仓没有 `REBIRTH` 专属 runtime、drain、handler 或 narration producer。

## P1 — 玩家实际感知影响

- 玩家**死亡瞬间**已有 `death_insight`：`agent/packages/tiandao/src/death-insight-runtime.ts:2-6,24` 明确这是“死亡瞬间遗念”。
- 但玩家**成功重生之后**，client 只有本地 HUD / 电影化画面：`client/src/main/java/com/bong/client/death/RebirthCinematicRenderer.java:19-31` 只显示“灵龛微光重新照见你”“虚弱 XXs”。
- 结果是：`bong:rebirth` 这条 server 已经真实发出的“重生结算”事件，**不会变成任何 tiandao narration / world feedback / post-rebirth acknowledgement**。
- 这对实际游玩体验的影响不是“看不见 HUD”，而是**死亡有天道遗念，真正活回来却没有对应的天道回声**；重生从叙事上被截成半段，形成前后体验不对称。

## P2 — 建议修复范围

- `agent/packages/tiandao/src/redis-ipc.ts`
  - 不再让 `REBIRTH` 只停留在通用 cross-system 缓冲；补专属 drain 或专属 runtime 接口。
- `agent/packages/tiandao/src/`
  - 新增 `rebirth` narration runtime，或在现有 runtime 中显式消费 `RebirthEventV1`。
  - 文案语义必须是“**已成功重生**”后的结算反馈，不能复用 `death_insight` 的死前概率文案。
- `agent/packages/schema/src/`
  - 继续以 `RebirthEventV1` / `CHANNELS.REBIRTH` 为 source of truth；补 sample / pin 测试，锁定契约。
- `client`
  - 原则上不要求新增 HUD；重点是确认现有 client narration 管线能正常显示 rebirth feedback，且不与 death cinematic 重复刷屏。

## 验收抓手

- 正常玩家经历“死亡 → 成功重生”后：
  - 仍保留 death insight（死前遗念）；
  - **额外**收到 1 条由 `bong:rebirth` 驱动的、语义明确的重生后反馈；
  - 不因为 `death_screen` / `death_cinematic` / `RebirthCinematicRenderer` 已有文案而吞掉这条 tiandao 反馈。
- 失败路径不误报：
  - 仅死亡未重生、终止轮回、或中途未形成 `PlayerRevived` 时，不产生 rebirth narration。
- 去重：
  - 同一次重生不重复播报；
  - 不与 `death_insight` 文案混淆成同一条事件。

## 审计来源

bughunt 2026-07-04（限定主题：`server/src/network/` + client HUD/VFX/handler + `agent/packages/schema` / `tiandao` 桥接断链）。主线自证后，怀疑方第 1 轮以“`death_insight` 已覆盖 / 这是 cross-system 通病”提出反驳；第 2 轮在 `REBIRTH` 独立契约、`death_insight` 仅是死前遗念、client 仅有本地 HUD 三点补证下改判 **CONFIRMED**。本 PR 只立 skeleton plan，不直接修代码。
