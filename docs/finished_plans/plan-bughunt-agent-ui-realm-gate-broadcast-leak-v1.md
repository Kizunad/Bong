# plan-bughunt-agent-ui-realm-gate-broadcast-leak-v1

> **Active BugFix plan（2026-07-16 升格）**。一句话主题：`agent-ui` 的 `realm_gate_rejected` 降级提示从 schema 到 consumer 都丢了目标玩家标识，`UiResponseConsumer` 只能把本应 `scope:"player"` 的“境界未至”文案硬编码成 `scope:"broadcast"`；一旦 server 权威拒绝某个玩家的 gated 面板，这条私人失败提示就会被发到全服聊天流。

> 立项动机：这不是抽象“文档不一致”。`agent/packages/tiandao/src/ui/uiResponseConsumer.ts` 注释明确写着理想形态是 `scope="player"`，但实现因为拿不到 `player_uuid` 直接退化成 `broadcast`；`server/src/network/mod.rs` 对 `broadcast` 又是无条件全服路由。`docs/finished_plans/plan-agent-ui-data-v1.md` 已把它记成遗留 follow-up，本 active plan 负责收口复现、影响面、修复面与验收抓手。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | `realm_gate_rejected` 私人提示误广播全服 | fix_pr | ✅ 2026-07-16 |

## P0 — `realm_gate_rejected` 私人提示误广播全服

- **现象**：`agent/packages/schema/src/payloads/agent-ui.ts:100-115` 把 `AgentUiResponsePayloadV1` 固定成 `{ request_id, action, params: Record<string,string> }`，没有任何顶层 `target_player`/`player_uuid` 字段；`realm_gate_rejected` 也只约定 `params.player_realm` / `params.required_realm`（`:103-106`）。`server/src/network/agent_ui.rs:393-408` 在 realm gate 拒绝时实际发出的 error payload 也只有 `reason/player_realm/required_realm` 三项，没有把触发玩家 id 带回 agent。
- **根因链路**：`agent/packages/tiandao/src/ui/uiResponseConsumer.ts:17-19` 的注释写明期望是 `scope="player"`、`target=player_uuid`，但实现位于 `:274-280`，因为 `AgentUiResponsePayloadV1` 里没有玩家标识，只能构造 `{ scope: "broadcast", target: "world", style: "system_warning" }`。随后 `server/src/network/mod.rs:3121-3137` 的 `narration_selector` 对 `NarrationScope::Broadcast` 直接选 `RecipientSelector::Broadcast`，于是该提示稳定路由到全服，而不是只给当事玩家。
- **复现路径**：
  1. 准备两名在线玩家 A / B。
  2. 触发任意一条带 `realm_gate>0` 的 agent-ui 请求，让 server 权威侧对 A 返回 `realm_gate_rejected`。当前可用三类入口：一是 agent world-state 境界快照滞后，误把低境界 A 当成可见清晰版面板；二是后续会复用这条降级路径的 `dying-elder` / `tiandao revelation` 生产触发源；三是直接向 `bong:agent_ui_cmd` 发布一个 `realm_gate` 高于 A 实际境界的 command 做最小复现。
  3. server 按 `server/src/network/agent_ui.rs:393-408` 发布不带玩家 id 的 `AgentUiResponsePayloadV1{action:"error",reason:"realm_gate_rejected"}`。
  4. agent 按 `agent/packages/tiandao/src/ui/uiResponseConsumer.ts:274-280` 生成 `scope:"broadcast"` narration 并发到 `bong:agent_narrate`。
  5. server 按 `server/src/network/mod.rs:3125` 走 `RecipientSelector::Broadcast`，A 与 B 都会看到“天道的注意力掠过，境界未至...”这条原本只该属于 A 的提示。
- **为什么这是 bug，不是设计**：`agent/packages/tiandao/src/ui/uiResponseConsumer.ts:17-19` 已把设计口径写成“优先 `scope=player`，拿不到玩家 id 才退化”；`docs/finished_plans/plan-agent-ui-data-v1.md:361` 也明确承认当前 broadcast scope “偏离 §0.1 指定的 `scope=player`”。也就是说，现状不是产品选择，而是已知未收口的协议缺口。
- **测试侧证据**：`agent/packages/tiandao/src/ui/agent-ui.test.ts:714-730` 与 `:985-1009` 只断言“会往 `AGENT_NARRATE` 发一条 narration，且文案/style 正确”，没有任何 `scope/target` pin；这意味着当前错误路由不仅存在，而且已被测试空档默许。

## 这个 bug 对实际游玩体验的影响

- 当某个玩家因为境界不够、快照滞后或 follow-up 面板门槛更高而被 `realm_gate_rejected` 时，其他在线玩家会无端看到一条与自己无关的“天道未许”提示，聊天流被污染，且会误以为全服刚发生了某种公共天象。
- 对当事玩家，这条提示原本应是“只有你感知到的一次失败反馈”；现在却变成公开广播，等于把个人面板失败事实泄漏给旁观者，尤其不适合 `dying-elder` / `tiandao revelation` 这类本来就偏私人、偏沉浸的面板。
- 对后续实现，这条链已经被 `plan-agent-ui-data-v1` 留作 follow-up；如果不先补协议与路由，后面新增任何 gated panel producer 都会复用同一个错误广播口径，把私人提示继续做成世界公告。

## 建议修复范围 / 模块

- `agent/packages/schema/src/payloads/agent-ui.ts`：给 `realm_gate_rejected` 路径补可路由的玩家标识。优先方案是为 `AgentUiResponsePayloadV1` 增加顶层 `target_player`；次优是约定 `params.target_player`，但会继续把“协议关键字段”埋进弱类型 map。
- `server/src/network/agent_ui.rs`：在 `realm_gate_rejected` error response 中回填 canonical player id，而不只发 `player_realm/required_realm`。
- `agent/packages/tiandao/src/ui/uiResponseConsumer.ts`：构造 `scope:"player"`、`target=<canonical_player_id>`；旧 payload、空白目标或坏 payload 无法确定私人提示收件人时必须 fail-closed，显式打 warn 并停止发布，禁止退化成 broadcast。
- `agent/packages/tiandao/src/ui/agent-ui.test.ts`：把现有 `realm_gate_rejected` 两组测试补成强 pin，断言 `scope==="player"` 且 `target===offline:<name>`，避免以后再退回 broadcast。

## 验收抓手

1. `realm_gate_rejected` 经过完整 server→agent→server narration 链后，产出的 narration 必须是 `scope:"player"`，且 `target` 为 canonical player id。
2. 两名在线玩家的端到端回归里，A 被 gate 拒绝时只有 A 能收到该 system warning，B 聊天流保持干净。
3. 兼容旧 payload 的反序列化仍可工作，但缺失/空白目标必须打 warn 并丢弃私人 narration；测试要明确锁定 legacy、空白与坏 payload 均不会产生 broadcast。

## 反方裁决摘要

1. **Round 1（退化：当前会话无可用 subagent / delegate 工具，改为主代理手工反方裁决）**：反方论点是“这只是 finished plan 已记录的 minor follow-up，不算新 bug”。驳回理由：`uiResponseConsumer.ts:274-280` 与 `server/src/network/mod.rs:3125` 组合出的全服广播是当前运行时代码的真实行为；“已知”不等于“已修”，更不等于“不是 bug”。
2. **Round 2（同样为手工反方裁决）**：反方论点是“`UiRenderer` 现有 blur 版本会绕开大多数 `realm_gate_rejected`，所以广播问题不影响真实游玩”。驳回理由：server 权威拒绝路径仍然保留且有测试、文档、follow-up producer 共同依赖；只要出现 world-state 境界滞后、后续生产触发源直接发 gated 清晰版，错误广播就会立刻外露。换言之，这是被真实入口共享的协议缺口，不是死代码。

## 开放问题（已收口）

1. `target_player` 应放回 `AgentUiResponsePayloadV1` 顶层，还是只对 `action="error" && reason="realm_gate_rejected"` 做局部扩展？建议选前者，避免继续把路由关键字段藏进 `params`。
2. 既然 `docs/finished_plans/plan-agent-ui-data-v1.md:361` 已认定这是 follow-up，修复 PR 是否顺手把同文档里的 “TSY target_player fallback” 与本条一起复核，避免再次出现“目标玩家信息丢失但靠 broadcast 兜底”的同型错误？

以上问题均已在下方实施决议中收口；实施与验收以该决议为准。

## 实施决议（2026-07-16）

1. **协议字段落点**：采纳问题 1 的顶层方案。在 server→agent `AgentUiResponsePayloadV1` 增加可选 `target_player`（1..=128 Unicode code points），同时更新 TypeBox 生成物与 Rust serde 镜像。真实 client→server C2S schema 明确禁止该字段；仅旧的 server→agent response payload 缺字段时仍可兼容反序列化。server 只在 `realm_gate_rejected` 权威拒绝响应中回填 `cmd.target_player`，其它终态显式保持 `None`。
2. **路由与兼容**：采纳问题 2 的“不跨题扩展”结论，不改 `plan-agent-ui-data-v1` 或 TSY fallback。`UiResponseConsumer` 对非空、去空白后的 `target_player` 生成 `scope="player"`；缺失或空白字段仍可兼容消费，但必须 warning + fail-closed，不发布任何 narration，并以 `narrationDroppedMissingTarget` 计数。兼容反序列化不等于兼容隐私泄漏，私人提示绝不允许降级为 `broadcast/world`。server 现有 `RecipientSelector::player` 的 `offline:<name>` 匹配作为最终双玩家隔离门。
3. **验收矩阵**：schema 覆盖带目标、legacy 缺字段、显式 null、空串、Unicode code-point 边界与 lone surrogate；consumer 覆盖 player scope、legacy/空白 fail-closed、坏 payload 拒绝与 publish 失败；server 覆盖 gate reject 回填 canonical id、旧 JSON 兼容和 `AgentUiResponsePayloadV1` 全部生产构造点的 `None` 默认。既有 `network::narration` 双玩家 player-scope 测试作为链路隔离证据，不新增第二套路由实现。

## 审计来源

升格来源为 bug-hunt 定点轮（范围：`agent-ui` / panel surface / follow-up side path；排除 tiandao revelation vfx flag loss、button_click context loss、TSY discovery target fallback）。原始轮次只读搜索 `schema → server agent_ui → tiandao ui consumer → server narration route → 既有 finished plan` 证据链并形成 report-only 骨架；2026-07-16 升格后按本 active plan 的实施决议落地。

## Finish Evidence

### 落地清单

- `agent/packages/schema/src/payloads/agent-ui.ts`、`agent/packages/schema/src/client-request.ts`、`agent/packages/schema/src/schema-registry.ts`、`agent/packages/schema/generated/agent-ui-response-payload-v1.json`：拆分真实 Fabric C2S `AgentUiClientResponsePayloadV1` 与 server→agent `AgentUiResponsePayloadV1`。C2S 只允许 `request_id/action/params`；新增的 S2A `target_player` 可选且独占 1..=128 Unicode code-point / well-formed Unicode pattern。既有 Agent UI ID 字段继续保留 `minLength/maxLength`，本 PR 最终树不迁移其接受集合。
- `server/src/schema/agent_ui.rs`、`server/src/schema/client_request.rs`、`server/src/schema/proto_convert.rs`：Rust S2A mirror 仅对 optional `target_player` 做 ingress/egress 校验；字段省略兼容为 `None` 并从输出省略，显式 `null`、空串、129 code points 与 lone surrogate 拒绝。真实 C2S enum 不含 `target_player`，JSON-bypass pin 锁定 Fabric producer 形状。
- `server/src/network/agent_ui.rs`：仅真实 `realm_gate_rejected` 权威 producer 回填 `Some(cmd.target_player.clone())`；其它 response 终态保持 `None`。
- `agent/packages/tiandao/src/ui/uiResponseConsumer.ts`、`agent/packages/tiandao/src/ui/agent-ui.test.ts`：合法目标发布 `scope="player"` / `target=<canonical player id>`；目标缺失或 trim 后空白时 warning、递增 `narrationDroppedMissingTarget` 并 fail-closed，绝不恢复 `broadcast/world`。
- `server/src/network/redis_bridge.rs`、`server/src/network/mod.rs`、`agent/packages/tiandao/tests/ui-response-consumer-runner.ts`、`scripts/bot/scenarios/agent_ui_realm_gate_private_narration.py`：以 production encoder、真实 TypeScript consumer、production narration decoder 与 recipient selector 串起双玩家隔离链；目标玩家收到 typed system warning，旁观玩家与双方 chat mirror 均保持干净。
- `agent/packages/tiandao/src/ui/uiResponseConsumer.ts`、`agent/packages/tiandao/src/ui/agentUiRuntime.ts`、`agent/packages/tiandao/src/main.ts`、对应测试：修复 cleanup 与 pending subscribe 竞态及 cleanup 前已接收 handler 越界。logical consumer 在 cleanup 开始时同步关闭 admission 并移除 listener；迟到 subscribe resolve/reject 不得重新挂 listener或处理消息；callback/logger 抛错被 detached-handler observer 吞并安全记录；500ms 只约束 subscribe/unsubscribe transport teardown，不能越过已接收 handler 的强完成边界。三个 Redis client 的 physical ownership 继续只属于 factory，cleanup exactly-once 断开。
- 范围纠偏：此前提交过的“全部 Agent UI ID 统一 code-point”以及 Ajv 精确依赖、双 lockfile、trusted-root/canonical-path/symlink-escape 子系统均已在最终返工中撤回；`agent/package-lock.json`、package-local lock/package metadata、`generated-artifacts.test.ts`、聚合 client/server schema 与 Rust C2S mirror 均恢复 `origin/main` 对应契约。本 plan 只为新增 S2A `target_player` 定义 Unicode 边界。

### 关键提交

- `e68ae6ea`（2026-07-16）：升格并收口 active plan。
- `2ec1ea3a` / `e5622e0d` / `b0b8a166`（2026-07-16）：建立 S2A 目标字段、权威回填与 player-only fail-closed 路由。
- `8ddb72eb` / `1cb25a79`（2026-07-16）：贯通真实 producer→consumer→selector 双玩家回归并补 legacy/null 分流。
- `5a8f84d2` / `a93c3dc7` / `31db8f98`（2026-07-18）：拆分真实 C2S/S2A schema，补 production runtime/专用 Redis Bot 路径与独立 S2A generated artifact。
- `1ac6b96f`（2026-07-18）：修复 narration 测试二次 drain 假绿，改为同批 typed payload/chat canary 分类。
- `2ff99756`（2026-07-20）：补齐 production factory 三条互异 Redis 连接及 startup/cleanup 归属回归。
- `07683592`（2026-07-22）：修复 `UiResponseConsumer` cleanup/pending-subscribe 生命周期竞态。
- `8b732ddb`（2026-07-22）：记录生命周期返工的历史测试证据；当前候选 HEAD 的精确门禁见下方“测试结果”。
- `62cbc43d`（2026-07-22）：撤回超出私人路由修复范围的既有 Agent UI ID code-point 迁移与 Ajv 安装信任子系统，仅保留新增 S2A `target_player` 的 Unicode 边界。
- `fbea9144`（2026-07-22）：为真实 Fabric C2S `AgentUiResponse` 添加 `target_player` 伪造负样本，明确锁定 client 不得注入该仅 server→agent 可写字段。
- `937f7e54`（2026-07-22）：补齐 cleanup 前已接收响应的强完成边界，消除 detached handler 派生 Promise 的未处理拒绝，并把 500ms timeout 限定为 transport teardown 门禁。

### 测试结果

- 历史生产链证据：专用 Redis 双 Bot `agent_ui_realm_gate_private_narration` PASS；链路观测 `bong:agent_ui_cmd → bong:agent_ui_response(target_player) → bong:agent_narrate(scope=player)`，server 最终投递 1 recipient，旁观者无泄漏。
- 历史 lifecycle 返工 `07683592`：Tiandao 定向 `agent-ui.test.ts + main.test.ts` 2 files / 117 tests passed；完整 Tiandao 72 files / 846 tests passed。覆盖 cleanup 早于 subscribe completion、500ms physical-disconnect timeout 后迟到 completion、exactly-once disconnect 与零 callback/narration/stats 副作用。
- 本次 scope rollback 工作树定向门禁：`npm run build -w @bong/schema`、`npm run generate:check` 均退出 0，406 generated files fresh；`vitest run tests/agent-ui.test.ts` 为 1 file / 51 tests passed。Rust `cargo fmt --check` 退出 0；`schema::agent_ui::tests` 27/27、C2S JSON-bypass 1/1、`schema::client_request::tests::agent_ui_response*` 3/3 passed。
- 本地完整门禁已在 `62cbc43d` 后通过：`cd agent/packages/schema && npm run check && npm test`（29 files / 886 tests）；`cd agent/packages/tiandao && npm test`（72 files / 846 tests）；`cd agent && npm run build`；`cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`（合计 11875 passed / 0 failed / 6 ignored）；`cd client && ./gradlew --no-daemon test build`（3 required Fabric GameTests passed）；`python3 -m unittest scripts.bot.test_protocol`（126 tests）。Gradle 外层日志使用 OpenJDK 21.0.10，Fabric runtime 日志列出 Java 17；这只记录实际环境，不将其表述为额外 Java 17 toolchain pin。
- `fbea9144` 增加 Rust C2S 伪造拒绝测试后，`cd server && cargo fmt --check && cargo test schema::client_request::tests::agent_ui_response` 通过（4/4）。
- 生命周期修复 HEAD `937f7e54f45a42fed153fd5cef66c6b967298218` 精确门禁（2026-07-22）：定向 `npm test -- --run src/ui/agent-ui.test.ts tests/main.test.ts` 为 2 files / 121 tests，完整 Tiandao 为 72 files / 850 tests，`cd agent && npm run build` 通过。测试锁定 cleanup 前 narration publish 跨过 500ms 仍不得物理断连/返回、throwing callback + throwing logger 不产生 `unhandledRejection`、pending subscribe/unsubscribe timeout 后保持 inert，以及三个 physical clients exactly-once disconnect。
- 同一 HEAD 的无上下文只读 validator 严格对拍 `937f7e54f45a42fed153fd5cef66c6b967298218` 后返回 `passed=true`、`findings=[]`；其复核覆盖 handler rejection containment、同步 admission close、accepted-work 强完成边界、pending subscribe/unsubscribe late completion 与 physical ownership。此前 validator 在 `8664196523e27997bfc079e3345b5acb6fba30ed` 工作树上发现的两项 major（ignored `finally` 派生拒绝、500ms timeout 越过 accepted handler）已由 `937f7e54` 修复并由上述回归锁定。
- `8664196523e27997bfc079e3345b5acb6fba30ed` 的 GitHub Actions e2e run `29890273905` 已 `SUCCESS`，但只作为历史基线；`937f7e54` 及后续 evidence commit 均需重新取得 exact-head remote e2e/review，旧 SHA 结果不外推。
- 两份受保护 untracked snapshot 始终未 stage、删除或覆盖：`tiandao-snapshot-200.json` SHA-256 `c6ee23bbe36c6fde5f2c5c2d470359c89b0a64174226ab2691a2afe32f82d0ab`；`tiandao-snapshot-300.json` SHA-256 `eb0b116cad4cd803e5548e0faaf5f9d7f77766310304c7e7acf1c964e9d416e8`。

### 跨仓库核验

- client→server：TypeBox `AgentUiResponseRequestV1` 与 Rust `ClientRequestV1::AgentUiResponse` 都拒绝额外 `target_player`；真实 Fabric producer 仍只发送 `request_id/action/params`。
- server→agent：`receive_agent_ui_cmd_system` 从已认证目标命令的 canonical id 构造 realm-gate rejection，production Redis encoder 序列化 optional `target_player`；legacy 缺字段继续可读，显式 `null` 与 malformed target 被拒绝。
- agent→server：`UiResponseConsumer` 只为合法 target 发布 player-scoped narration；缺目标或坏 payload 无发布旁路。production parser 与 `RecipientSelector::player` 最终只命中目标实体。
- 生命周期：consumer/runtime 只拥有 logical listener；factory 拥有三个 physical Redis client。cleanup 同步关闭 admission，500ms 仅约束 pending connect/unsubscribe transport teardown；cleanup 前已接收 handler 必须先 settle，且任何 handler/logger 异常不得形成 process-level unhandled rejection。物理断开 exactly once。
- scope：既有 request/command/close ID 不属于本 plan 的 Unicode migration；Ajv 安装布局与 trust-root 防护也不属于私人路由修复。

### 遗留 / 后续

- legacy server→agent payload 仍允许省略 `target_player` 以支持滚动升级，但私人境界门提示会 fail-closed。若未来需要恢复旧消息的玩家反馈，应另立 plan 建立可信 `request_id → canonical player` 关联，禁止恢复 broadcast fallback。
- 全量 Agent UI ID code-point 迁移、Ajv trust-root 加固、TSY target fallback、revelation 与 button-click context loss 均不在本 plan 最终范围；如仍有价值，应分别立项并独立 review。
