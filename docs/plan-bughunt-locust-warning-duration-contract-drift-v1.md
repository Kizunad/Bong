# plan-bughunt-locust-warning-duration-contract-drift-v1

> **Active plan**。一句话主题：`locust_swarm_warning` 已在 `agent/packages/schema` 与 server 下发链路中约定 `duration_ticks`，但客户端 `LocustSwarmWarningHandler` 完全不消费该字段，统一把 HUD 警示与震动特效硬编码成 **6.5 秒**。结果是：**灵蝗潮预警持续时间与协议承诺脱节，长时蝗潮只闪一下就消失**。

> 立项动机：这条问题位于 **agent/schema/协议契约链路**，不是纯客户端表现瑕疵。TypeBox 合同、sample、server 实际 payload 都把 `duration_ticks` 当成有效字段；唯独客户端消费端丢弃，说明契约已分叉，而且当前分支的共享 Rust `client_payload.rs` 也没把 `locust_swarm_warning` 纳入枚举，导致跨端样例对拍没有覆盖到这条链路。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | `locust_swarm_warning.duration_ticks` 被客户端静默丢弃 | fix_pr | ⬜ |

## P0 — `locust_swarm_warning.duration_ticks` 被客户端静默丢弃

- **复现路径**：
  1. 触发灵蝗潮预警（server 已有 dedicated channel `bong:locust_swarm_warning`；`server/src/network/mod.rs:2301-2317` 会下发 `{"type":"locust_swarm_warning","zone":...,"message":...,"duration_ticks":...}`）。
  2. 共享 schema/sample 明确承认该字段：`agent/packages/schema/src/client-payload.ts:80-91` 把 `duration_ticks` 定义为 `LocustSwarmWarningPayloadV1` 的可选整数；sample `agent/packages/schema/samples/client-payload-locust-swarm-warning.sample.json` 也给出 `duration_ticks: 24000`。
  3. 客户端收到 payload 后走 `client/src/main/java/com/bong/client/network/LocustSwarmWarningHandler.java:27-67`；该 handler 只校验 `v/type/zone/message`，随后直接用 `DEFAULT_DURATION_MILLIS = 6_500L` 创建 toast 与 `pressure_jitter` 特效。
  4. 无论 server 下发的是 `24000 tick`、`600 tick` 还是别的值，客户端表现都恒为 **6.5 秒**。

- **根因链路**：
  - **合同侧**：TypeBox source-of-truth 已声明 `duration_ticks` 合法，且样例固定携带该字段。
  - **生产侧**：server `locust_swarm_warning_payload()` 真实把 `pending_alert.duration_ticks` 编进 payload，不是死字段。
  - **消费侧**：`LocustSwarmWarningHandler` 有 `readOptionalInt()`，但从未读取 `duration_ticks`；toast/effect 两处都直接写死 `DEFAULT_DURATION_MILLIS`。
  - **覆盖缺口**：共享 Rust `server/src/schema/client_payload.rs:5-80` 仍缺 `locust_swarm_warning` 变体，现有跨端 fixture 对拍也只覆盖 welcome/heartbeat/narration/zone_info/event_alert/player_state，没把这条 payload 纳入统一契约回归，所以 schema 已扩、runtime handler 没跟上时没有报警。

- **为什么这是 bug，不是产品选择**：
  - 如果产品本意就是固定 6.5 秒，就不该在 schema/sample/server payload 中保留 `duration_ticks` 并持续下发。
  - 现在的实现不是“客户端自行重解释协议”，而是**协议提供了持续时长，消费端完全无视**；这是典型 contract drift。

- **影响面**：
  - `agent/packages/schema/src/client-payload.ts`
  - `agent/packages/schema/samples/client-payload-locust-swarm-warning.sample.json`
  - `server/src/network/mod.rs`
  - `client/src/main/java/com/bong/client/network/LocustSwarmWarningHandler.java`
  - `server/src/schema/client_payload.rs`（覆盖缺口/对拍缺口，不是直接 runtime producer）

- **这个 bug 对实际游玩体验的影响**：
  - 灵蝗潮属于提前预警型事件，但客户端只闪 6.5 秒就收掉提示和震动，**长时或跨区推进中的蝗潮会在危险仍然持续时提前“静音”**。玩家如果当时在战斗、开背包、看 UI，错过这 6.5 秒后就失去后续持续提醒；体感上会变成“server 说有蝗潮，但客户端只抖一下，接下来像没事一样”，削弱预警价值，也增加被突袭时的误判概率。

- **建议修复范围 / 模块**：
  - 主修 `LocustSwarmWarningHandler`：读取 `duration_ticks`，统一做 tick→millis 转换；字段缺失或非法时才回退 `DEFAULT_DURATION_MILLIS`。
  - 补共享契约回归：把 `locust_swarm_warning` 纳入 `server/src/schema/client_payload.rs` 与对应 sample 对拍，避免下次再出现“TypeBox/sample 已变，另一端 contract mirror 没跟上”。
  - 补客户端验收：增加“payload 带 `duration_ticks=24000` 时 toast/effect 生命周期不再固定 6500ms”的 pin test。

## 反方裁决摘要

> **退化说明**：当前会话没有可用的 subagent / delegate 工具；以下两轮反方裁决均为主代理在同一 worktree 内做的对抗式自审，而不是外部子代理独立复核。

1. **Round 1 反方论点**：`duration_ticks` 也许只是给 server 音效或日志看的字段，客户端固定 6.5 秒属于有意降噪，并不算 contract drift。  
   **驳回理由**：字段定义位置在 `agent/packages/schema/src/client-payload.ts` 的客户端 payload 合同里，不在纯 server 内部结构；sample 也直接面向 client payload。既然它属于 client-facing schema，就不能解释成“只给 server 自己看”。

2. **Round 2 反方论点**：server 下发 `duration_ticks` 不代表 UI 必须同寿命，message 文案已经足够，6.5 秒只是另一种产品选择。  
   **驳回理由**：当前实现不是“读取后裁剪”，而是**完全不读字段**；同时 `LocustSwarmWarningHandler` 连 `direction` 这类扩展字段也不消费，说明这里是消费端落后于合同，而不是明确的重解释设计。若真要压缩展示寿命，至少需要在 handler/test/注释中显式声明“协议时长仅供上游参考”，而现在没有任何这样的说明。

## 开放问题

1. `duration_ticks` 的客户端语义应是“toast/effect 全时长”还是“事件剩余寿命上限，客户端可再裁成更短的 UX 窗口”？修复 PR 需要一次定清，避免再次出现字段存在但意义含糊。
2. `direction` 字段目前也未被 `LocustSwarmWarningHandler` 单独消费；如果后续要把文案拼装改回客户端本地化，这个字段是否要一起接上，应在同一修复里顺手定案。

## 审计来源

bug-hunt 定点轮（范围限定 `agent/packages/schema` / `server schema & network payload` / `client payload consumer`，显式避开 forge `step_state` contract drift）。证据来自静态代码链路复核：TypeBox 合同、sample、server producer、client handler、共享 Rust mirror 与现有 fixture 覆盖面交叉对账；结论为 **report-only**，先提交 skeleton 收口问题与修复面，再由后续 fix PR 单独落地。
