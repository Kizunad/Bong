# BugHunt: 暗器充能完成事件未接入天道叙事

状态：BugHunt skeleton plan。本文只记录缺陷与修复骨架，本 PR 不消费 plan、不改代码、不归档。

## Bug 摘要

`CarrierChargedEventV1` 已在 schema 中声明为 `Server -> Agent` 的暗器载体封元完成叙事事件，server 也会把真实充能完成事件发布到 Redis channel `bong:combat/carrier_charged`。但 `AnqiNarrationRuntime` 没有订阅、解析或验证 `CHANNELS.ANQI_CARRIER_CHARGED`，导致这条事件永远不会产出 `bong:agent_narrate`。

这不是 schema 缺失，也不是 server Redis publish 缺失；断点在 tiandao agent 的暗器叙事 runtime。

## 实际游玩体验影响

玩家完成暗器载体封元后，server 侧仍会触发动画和音效，因此不是“暗器充能完全无反馈”。缺失的是天道叙事链路：玩家不会收到“封元完成 / 载体已可投掷 / 本次封入真元量与色相”的文字叙事确认。

这会让暗器流的 20 秒充能窗口显得像只有局部 A/V 反馈，没有天道叙事收束；多人或调试场景里，也无法通过 `bong:agent_narrate` 确认 `carrier_charged` bridge 是否真的被 agent 消费。

## 证据定位

- `agent/packages/schema/src/channels.ts:226` 附注 `Server -> Agent: 暗器载体封元完成（plan-anqi-v1 P2 narration）`，`agent/packages/schema/src/channels.ts:227` 定义 `ANQI_CARRIER_CHARGED: "bong:combat/carrier_charged"`。
- `agent/packages/schema/src/combat-carrier.ts:36` 定义 `CarrierChargedEventV1`，字段包含 `carrier` / `instance_id` / `qi_amount` / `qi_color` / `full_charge` / `tick`；`agent/packages/schema/src/combat-carrier.ts:178` 导出 `validateCarrierChargedEventV1Contract`。
- `server/src/combat/carrier.rs:612` 在充能完成时 `events.send(CarrierChargedEvent { ... })`。
- `server/src/network/anqi_event_bridge.rs:22` 的 `publish_carrier_charged_events` 把 `CarrierChargedEvent` 转为 `CarrierChargedEventV1`，并在 `server/src/network/anqi_event_bridge.rs:36` 发送 `RedisOutbound::CarrierCharged(payload)`。
- `server/src/network/redis_bridge.rs:258` 有 `RedisOutbound::CarrierCharged(CarrierChargedEventV1)`；`server/src/network/redis_bridge.rs:1466` 分支把它发布到 `CH_ANQI_CARRIER_CHARGED`。
- `server/src/network/mod.rs:532` 注册 `anqi_event_bridge::publish_carrier_charged_events`，说明该 bridge 在真实 server schedule 中运行。
- `agent/packages/tiandao/src/main.ts:971` 的 `startAnqiRuntime` 会在真实 agent 进程中启动 `AnqiNarrationRuntime`。
- `agent/packages/tiandao/src/anqi-narration.ts:28` 解构的暗器 channel 不含 `ANQI_CARRIER_CHARGED`；`agent/packages/tiandao/src/anqi-narration.ts:58` 的 `AnqiPayload` union 不含 charged；`agent/packages/tiandao/src/anqi-narration.ts:176` 先用 `ANQI_CHANNELS.has(channel)` 硬过滤；`agent/packages/tiandao/src/anqi-narration.ts:247` 的 `parseEvent` 没有 charged 分支；`agent/packages/tiandao/src/anqi-narration.ts:281` 的订阅集合没有 charged。
- `agent/packages/tiandao/tests/anqi-narration.test.ts:71` 的订阅断言也只锁定 impact / despawned / v2 事件，未覆盖 `ANQI_CARRIER_CHARGED`。
- `server/src/network/redis_bridge.rs:2085` 订阅 `CH_AGENT_NARRATE`，`server/src/network/redis_bridge.rs:2306` 将其解析为 `RedisInbound::AgentNarration`，说明 agent narration 是正式回流通道。
- 反方审查补查 `agent/packages/tiandao/src/redis-ipc.ts`：`CROSS_SYSTEM_EVENT_CHANNELS` 没有 `ANQI_*` 或 `bong:combat/carrier_charged` 兜底订阅。

## 触发路径

1. 玩家手持可封元暗器载体并触发 `charge_carrier`，充能窗口完成。
2. server 在 `combat/carrier.rs` 写入 `CarrierImprint` 后发送 `CarrierChargedEvent`。
3. `anqi_event_bridge` 将事件转成 `CarrierChargedEventV1`，`redis_bridge` 发布到 `bong:combat/carrier_charged`。
4. 真实 tiandao 进程启动 `AnqiNarrationRuntime`，但该 runtime 从未 subscribe `ANQI_CARRIER_CHARGED`。
5. 即使消息被手动传入 `handlePayload`，`parseEvent` 也没有 charged 分支，会返回 `null` 并计入 rejected contract。
6. 结果：不会发布 `AGENT_NARRATE`，server 也收不到这次充能完成的天道叙事。

## 反方审查记录

- 第一轮质疑：不能把影响描述成“暗器充能完全无反馈”，因为 `server/src/network/vfx_animation_trigger.rs:667` 已将 `CarrierChargedEvent` 接到 windup charge 动画与封骨密封粒子，`server/src/network/audio_trigger.rs:745` 已接到 `anqi_charge_seal` 音效。
- 主线程补证与让步：影响口径收窄为“天道 agent narration 缺失”。VFX/audio 属于 server 侧纯 cosmetic 反馈，不能替代 `bong:combat/carrier_charged -> tiandao narration -> bong:agent_narrate` 的叙事回流。
- 第一轮质疑：可能另有 runtime 或 tick loop 兜底消费。
- 主线程补证：`RedisIpc` 的跨系统事件订阅列表没有 `ANQI_*` / `bong:combat/carrier_charged`，全仓搜索只看到 schema/server/docs 定义，没有其它 tiandao consumer。
- 第一轮质疑：可能已有 PR 或 skeleton 覆盖。
- 主线程补证：开放 PR 未命中 exact bug；全状态搜索 `carrier_charged` 为空，`暗器 充能 agent` 只命中旧暗器功能、守恒、A/V PR（#121、#174、#634、#648、#701、#222），不是 charged agent narration 缺订阅；本地只命中 finished `plan-anqi-v1/v2`，无同名 skeleton。
- 最终裁决：PASS。候选通过高置信真实 bug gate；限定为 agent narration 契约断链，不是 schema/server publish 缺失，也不是 mock LLM 问题。
- 路径说明：反方提醒 report-only skeleton 通常可放 `docs/plans-skeleton/`；但本轮用户硬约束指定“允许新增且仅新增一个 `docs/plan-bughunt-<slug>.md`”，因此本文件按该约束放在 `docs/`。

## Skeleton Fix Plan

- [ ] 在 `agent/packages/tiandao/src/anqi-narration.ts` 解构并订阅 `CHANNELS.ANQI_CARRIER_CHARGED`。
- [ ] 将 `CarrierChargedEventV1` 与 `validateCarrierChargedEventV1Contract` 纳入 imports。
- [ ] 扩展 `AnqiPayload` union，加入 `{ kind: "charged"; payload: CarrierChargedEventV1 }`。
- [ ] 在 `fallbackNarration` 增加 charged 分支，生成封元完成叙事，至少包含 `carrier`、`qi_amount`、`qi_color`、`full_charge`、`tick` 对应语义。
- [ ] 在 `parseEvent` 增加 `ANQI_CARRIER_CHARGED` 分支，使用 `validateCarrierChargedEventV1Contract`。
- [ ] 更新 `ANQI_CHANNELS` 订阅集合，确保 connect 时包含 `ANQI_CARRIER_CHARGED`。
- [ ] 更新 `agent/packages/tiandao/tests/anqi-narration.test.ts`，覆盖订阅列表、有效 charged payload 发布 `AGENT_NARRATE`、非法 payload rejected。
- [ ] 若 schema src 有改动，按仓库约束重建 `@bong/schema` dist；本 bug 的预期修复主要应在 tiandao runtime，不应改协议字段。

## 验收测试计划

- `agent/packages/tiandao`：运行 tiandao 子包测试，新增 charged narration 用例应通过。
- `agent/`：运行 `npm run build`，确认 `@bong/schema` export 与 tiandao import 无 dist drift。
- 手动或集成验证：同时监听 `bong:combat/carrier_charged` 与 `bong:agent_narrate`，执行一次暗器封元充能；前者收到 `CarrierChargedEventV1` 后，后者应出现同 tick / 同 carrier 的 narration。
- 回归验证：impact / projectile_despawned / multi_shot / qi_injection / echo_fractal / abrasion / container_swap 原有暗器叙事仍发布一次，不因 charged 接入重复或漏发。

## 风险

- charged fallback 文案不能暗示命中或伤害，只能描述封元完成和载体状态，否则会与后续 impact narration 重叠。
- `full_charge=false` 的半封或非满封场景需要单独文案，避免误导玩家以为载体已达满额。
- 订阅集合新增 channel 后，测试应避免只断言旧列表顺序，最好同时 pin 完整集合，防止未来 schema 与 runtime 再漂移。
- 本 skeleton 不要求修改 server，因为发布端证据已完整；若后续修复改了 schema src，必须同步构建产物，避免 tiandao 使用旧 dist。
