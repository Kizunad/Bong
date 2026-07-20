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

- `agent/packages/schema/src/payloads/agent-ui.ts`、`agent/packages/schema/tests/agent-ui.test.ts`：Agent UI ID 统一为 1..=128 Unicode code points。双模式 ECMA-262 pattern 用 `{1,128}` 同时表达 well-formed Unicode 与长度，并以 `(?![\s\S])` 显式要求绝对输入结束，避免标准 JSON Schema `maxLength` 的 code-point 语义、TypeBox `String.length` 的 UTF-16 语义及尾锚解释分叉；五个 TypeBox schema 的八个 ID 字段均表驱动覆盖 65/128/129 emoji、BMP/astral 混合、128/129 BMP、空串、lone/embedded surrogate，以及 LF/CR/CRLF/U+2028/U+2029 的 128/129 code-point 边界。
- `agent/packages/schema/src/schema-registry.ts`、`agent/packages/schema/generated/agent-ui-response-payload-v1.json`、`agent/packages/schema/generated/client-request-v1.json`、`agent/packages/schema/generated/server-data-v1.json`、`agent/packages/schema/tests/generated-artifacts.test.ts`、`agent/packages/schema/package.json`、`agent/packages/schema/package-lock.json`、`agent/package-lock.json`：真实 server→Tiandao response 保持独立 generated artifact。早期以 `$.anyOf[85]` / `[103]` / `[104]` 固定下标锁六个 Agent UI ID 字段的做法现已撤回；当前由 `locateAgentUiIdSchemas()` 遍历真实生成物，并以 `findUniqueVariantByType()` 按 `variant.properties.type.const` 语义唯一定位 `agent_ui_response` / `agent_ui_request` / `agent_ui_close`，missing 会列出可用 discriminant，duplicate 会列出全部命中索引，且乱序正例、missing、duplicate 三类行为均有直接测试。六个字段继续禁止恢复 `minLength/maxLength`，并分别用无 flag 与 Unicode-aware `u` 的实际 artifact pattern 对拍。历史「宿主 Ajv 8.12.0 78/78」属 overclaim；后续短暂依赖 `/usr/share/nodejs/ajv` 的 host-only 取径也因 clean runner 不可复现而撤回。当前真相：`@bong/schema` 以精确 workspace `devDependency` 声明 `"ajv": "8.12.0"`，根 `agent/package-lock.json` 与 package-local `agent/packages/schema/package-lock.json` 均锁定 `ajv@8.12.0`；测试正常 `import Ajv from "ajv"`，显式断言 `AJV_PACKAGE_VERSION === "8.12.0"`。package metadata 与 runtime import 必须共同被 `path.resolve` / `path.relative` 精确包含于当前仓库 `agent/node_modules` 或 `agent/packages/schema/node_modules` 的同一个受信根；`/usr/share/nodejs`、`/usr/local/lib/node_modules`、NODE_PATH 风格外部根、受信根前缀碰撞及 metadata/runtime 分裂均有拒绝测试。真实 Ajv 对三份生成物六个 ID 字段执行 22 边界用例 × 6 = 132 次根 schema validate，另加字段结构 pin 与 root 契约（legacy 缺 `target` / `additionalProperties` / `null` / C2S 禁字段等）；`discover` 与 workspace Ajv 用例超时放宽到 60s。
- `server/src/schema/agent_ui.rs`、`server/src/schema/client_request.rs`：Rust 统一使用 `chars().count()` 镜像 1..=128 Unicode code points；command、S2C request/close、server→agent response 与真实 C2S request 的 serde ingress/egress 均覆盖同一边界，包括单/双 code-point 行终止符。legacy response 缺字段兼容为 `None`，显式 `null`、空串、129 code points 与 raw JSON lone surrogate 均拒绝。
- `server/src/network/agent_ui.rs`：真实 `realm_gate_rejected` producer 回填 canonical `target_player`，并逐字段锁定 action、request id、境界参数与单次响应。
- `agent/packages/tiandao/src/ui/uiResponseConsumer.ts`、`agent/packages/tiandao/src/ui/agent-ui.test.ts`：非空目标发布 `scope="player"`；缺失或 trim 后空白目标 warning + fail-closed，不发布 narration，并递增 `narrationDroppedMissingTarget`。显式 `null` 作为坏 payload 在契约层拒绝，三条路径均锁定不会产生 broadcast。
- `agent/packages/tiandao/tests/ui-response-consumer-runner.ts`、`server/src/network/redis_bridge.rs`、`server/src/network/mod.rs`：test-only 窄适配器串起生产 server producer、生产 Redis response encoder、真实 TypeScript `UiResponseConsumer`、生产 narration decoder 与真实 recipient selector；两名 mock 玩家中仅目标收到 typed `system_warning`，双方均无 chat mirror。
- 删除 `agent/packages/schema/samples/agent-ui-realm-gate-routing.chain.sample.json`：不再用预制 `agent_narration` 冒充 consumer 输出，避免 producer、consumer 与 fixture 同步改错时产生假绿。

### 关键提交

- `e68ae6ea`（2026-07-16）：升格并收口 active plan。
- `2ec1ea3a`（2026-07-16）：定义双端目标玩家兼容契约。
- `e5622e0d`（2026-07-16）：修复境界门拒绝提示全服泄漏。
- `b0b8a166a91d7ed4157cac94c5db0675754a21a9`（2026-07-16）：收紧境界门私人提示路由契约。
- `17407bf19d242eb5ffb68539c1e32feb5f334044`（2026-07-16）：合并最新主线 `0972f7c9d5c2dba1f06d884480e62fceedcde711` 并建立最终返工基线。
- `26485f2632b3c4f79fda3214a76fc1dbbffaabc0`（2026-07-16）：历史返工曾按 UTF-16 code units 对齐 Rust 与 TypeBox；该口径已由 `496c6985` 的 code-point 契约取代。
- `2e8a8d3855f62d9c94bdf5590a2bc438fe538636`（2026-07-16）：历史返工曾恢复 legacy broadcast fallback；该隐私敏感降级已由 `496c6985` 撤销为 fail-closed。
- `8ddb72ebcb9967471bb539da5a3126f13020679a`（2026-07-16）：移除预制 narration fixture，贯通真实 producer→consumer→selector 私有路由回归。
- `1cb25a7929849aaef01a2e839a67cb8ae7b33721`（2026-07-16）：补齐 TypeBox 显式 `null` 与 legacy 缺字段分流反例。
- `97231cc8eb62d0299f7e41250b10169743b2a101`（2026-07-16）：更新境界门私有路由返工证据。
- `5a8f84d2fa67122b11e7555b956ef121f0042f62`（2026-07-18）：拆分真实 Fabric C2S 请求与 server→agent 响应 schema；当时的 UTF-16 长度口径后由 `496c6985` 统一为 code points。
- `a93c3dc797cc97ef67c08d09c87d2fca94ef73c1`（2026-07-18）：补齐 production `AgentUiRuntime` 驱动的专用 Redis 双 Bot 黑盒场景与真实 Fabric producer pin。
- `470886d632fb8b9e6346ba745f8d0b21af80e92f`（2026-07-18）：修正 Agent UI 请求标识序列化签名，建立最终修复代码基线。
- `1a2885d073f4fbbf4c43be03c3b65674d8374972`（2026-07-18）：合并 `origin/main=9d2e29d0871b004684eb4d29c11a798fc1c71d05`，解决 Bot protobuf narration/zone_info 并存冲突并完成 worldgen/server 复验。
- `4cc9e2939c461ad6d12193f96d3e86cc4383d964`（2026-07-18）：继续合并最新 `origin/main=7ad2be2dbb0260bd738b9dc3514af7296a862a01` 的技能动画接线，并在最终代码树重跑 server/client/agent/Python/双 Bot/Fabric runtime 门禁。
- `31db8f9860ebd44697b9b253bac96ecf8635e03a`（2026-07-18）：补齐 server→Tiandao Agent UI response 独立生成物，并修正 Unicode-aware ECMA-262 surrogate pattern。
- `8b319420952644aa0ec18314678da889022cb0a5`（2026-07-18）：纠正归档决议中 C2S `target_player` 的旧表述，明确该字段仅在 server→agent response 可选。
- `496c69855031448b5bd4e199d5d225ea4a920c1f`（2026-07-18）：缺失/空白目标改为 fail-closed，并以 code-point pattern + Rust `chars()` 统一跨端 ID 长度语义。
- `0b75c6309ab6bfa99a5654702ee0d4e5a2c08505`（2026-07-18）：补齐五个 TypeBox schema、八个 ID 字段、三份生成物六个字段及 Rust ingress/egress 的统一 code-point 边界测试。
- `37ce9c9711058ec90c7ee8297f92c7ce7c60e6d9`（2026-07-18）：合并 `origin/main=c5e45f526b4d7ae2410d41710546feffbd07dd43` 的十份 bughunt skeleton 文档，未触及本 PR 代码路径。
- `3a8dd33d`（2026-07-18）：把生成物字段数量 pin 收紧为六个精确 JSON path，消除字段等量替换的假绿窗口。
- `ad463518899413bff035f5f1292fcfe8bef849ee`（2026-07-18）：用显式绝对结束断言消除尾锚歧义，并补齐五类行终止符的跨端 code-point 边界矩阵。
- `49eee364dd3801bcd96883bc141d136143a95fa4`（2026-07-18）：合并 `origin/main=001bbe7d82104f5dbdd16680dd9a122cab6eae40` 的技能 A/V 与 combat 变更，并在合并树复跑完整 server/client 门禁。
- `1ac6b96f410f8d62e4f6a21b71e74933b6e3da20`（2026-07-18）：修复 narration 测试先 drain typed payload、再从空缓冲检查 chat mirror 的假绿；六组测试改为单次收包同批分类，并加入 narration + `GameMessageS2c` 正向 canary。
- `bf0d65823ab6b13c739e13722f033003f6bb83f9`（2026-07-18）：紧邻 fetch 后合并 `origin/main=62f90990e7b23d19b56e847dcb47c761550bd7f4`；主线只带入两份无关 skeleton，未改变受验代码树。
- `a9df92719`（2026-07-18）：更新 PR1217 收包返工证据。
- `e70d52419`（2026-07-19）：合并最新主线并建立 PR1217 最终复验基线。
- `efcd273fe`（2026-07-19）：合并 `origin/main` 同步 #1233 文档归档。
- `e960ba5fc54795bbb572b015b93ea530578bd1cc`（2026-07-19）：合并 `origin/main=5d9bdd8f`（#1241 技能动画 PR-5）；在该树上完成 schema 892 / tiandao 831 / python 126 / client 4141 / server fmt+clippy+test 全绿（lib 11807 + bin 11 + 启动集成 1 + 背包 e2e 4 = 约 11823 passed / 0 failed / 6 ignored）。该历史 commit 缺少 `Model` trailer；保留历史原样，不重写或 amend。
- `56b6e33dc577453d2e29f4206564e84b15fcb3fd`（2026-07-20）：紧邻 `git fetch origin && git merge origin/main` 合入 `2f9c70ad3`（#1212 搜刮 HUD 终态收尾）；delta 仅 client network/HUD + 归档文档，无 server/agent 源码增量。该历史 commit 缺少 `Model` trailer；保留历史原样，不重写或 amend。
- `30a71c767f4e58d8254d18fd12d68bbd344e4892`（2026-07-20）：初版补齐 `generated-artifacts` host Ajv 真编译门禁（`loadHostAjv` + 根 schema 矩阵）；默认 5s 超时下该用例仍会假红。
- `bdc37989b6de5d2dee2dfb4ddf28aa90304b294a`（2026-07-20）：放宽 discover/host Ajv 用例到 60s，并打印宿主 Ajv version/path 与真实 `assertionCount`；本轮实测 assertionCount=157。
- `f4f0c92d45a097043e8cb84ce3f0fd64d3599127`（2026-07-20）：纠正 Finish Evidence 的宿主 Ajv 78/78 overclaim，回填真实 gate 数字；当时仍写 host `/usr/share/nodejs/ajv` 路径。
- `722179c612ba473b816453cd38e9b77cb7f4ae77`（2026-07-20）：撤回 host-only Ajv 取径，改为 `@bong/schema` workspace 依赖 + lockfile，并正常 `import Ajv from "ajv"`。
- `69644d28e45be7ebdd0fad8cd33d7ac609d90ce2`（2026-07-20）：更新 PR1217 Finish Evidence，记录可复现 workspace Ajv 中间态。
- `1e8455876f710bd5507d0fa77d15cf83516adced`（2026-07-20）：将 Ajv 精确锁定为 `8.12.0`，并把 generated Agent UI variant 从固定 `anyOf` 下标改为 `properties.type.const` 语义唯一定位。
- `2ff99756479749721a99fa9a2d18807dc2dba0b5`（2026-07-20）：修复 production `startAgentUiResponseRuntime` 三连接接线；command pub / response sub / narration pub 强制互异，startup failure 与正常 shutdown 均关闭全部已创建连接，真实 factory 回归锁定 player-only narration 且无 broadcast。
- `31707c2dab3fe3d4b068e3419fe0a5c812cbc803`（2026-07-20）：同步 package-local schema lock 的精确 `ajv@8.12.0`，并将 Ajv host gate 加固为当前仓库受信 `node_modules` 根的精确路径 containment。

### 测试结果

- Python 协议：最终代码树执行 `python3 -m unittest scripts.bot.test_protocol`，126/126 passed；冲突决议同时保留 protobuf narration field 3 与 `zone_info` field 4 解码及各自测试。
- Schema（历史 `ad463518`）：执行 `cd agent && npm run build -w @bong/schema`，随后 `cd agent/packages/schema && npm run check && npm test`，均退出 0；406 份 generated artifact freshness 通过，29 files / 892 tests passed，其中 `agent-ui.test.ts` 54/54、`generated-artifacts.test.ts` 12/12（以上包计数仅作历史记录）。**历史 overclaim 作废**：当时 `generated-artifacts.test.ts` 的 Agent UI ID 门仅是 JS RegExp（无 flag + `u`）双模式对拍，**没有**自动化宿主 Ajv compile/validate 门禁；「宿主 Ajv 8.12.0 … 78/78」不得再引用为 Ajv 证据。同 SHA 的 validator `PASS ad463518899413bff035f5f1292fcfe8bef849ee` 仅覆盖当时已有代码（RegExp 对拍），不可外推为真 Ajv gate PASS。
- Schema（host-only 中间态 baseline `5a019eeeea5d05ea1cf23c37ad09d662d91c82c7` → `bdc37989b` / 证据纠偏 `f4f0c92d`）：同文件曾新增 `loadHostAjv()` + 根 schema compile 矩阵，并在本机测到 assertionCount=157；但依赖 `createRequire` → `/usr/share/nodejs/ajv` 且不改 `package.json`，在 GitHub clean runner 上不可复现（e2e run `29712024499` 解析 `ajv` 失败）。**host-only 取径现已撤回，不得再当作 CI 可复现证据**。
- Schema（本轮 CI 可移植修复，代码 HEAD `1e8455876f710bd5507d0fa77d15cf83516adced`）：`@bong/schema` 以精确 `devDependency` 声明 `"ajv": "8.12.0"`，并由 npm workspace 正常同步 `agent/package-lock.json`；`generated-artifacts.test.ts` 正常 `import Ajv from "ajv"`，显式断言 package version 精确为 `8.12.0`，package/runtime 两条解析路径均必须位于 workspace `node_modules` 且拒绝 `/usr/share/nodejs/`。固定 `anyOf[85]/[103]/[104]` 已删除，三个 Agent UI variant 改为按 `properties.type.const` 唯一定位，保留完整 22 边界 × 6 字段矩阵、RegExp 双模式对拍、真实 Ajv root compile/validate 与 root 契约。门禁命令、测试数与 exact-SHA validator 见下方「本轮 gate 实测」。
- Schema（production-wiring 返工 `31707c2dab3fe3d4b068e3419fe0a5c812cbc803`）：执行 `cd agent/packages/schema && npm ci --workspaces=false`，64 packages 安装成功（仅报告 5 个 npm audit 项，未改版本）；package-local `package-lock.json` 根声明与 `node_modules/ajv` 均精确为 `8.12.0`，根 `agent/package-lock.json` sha256 仍为 `96f595190f3f890e6691a3d274dc5a922c482623ff924c8f2a3b6bfba8974f38`、零 diff。`npm run check` 退出 0（406 generated artifacts fresh）；`npm test` 退出 0（29 files / 897 tests，`generated-artifacts.test.ts` 17/17，真实 Ajv assertionCount=157）；`npm run build && npm run generate && npm run generate:check` 全部退出 0，生成物零 diff。Ajv package/runtime 路径实测共同落在 `agent/packages/schema/node_modules`，外部 host/NODE_PATH 与路径前缀碰撞测试全部拒绝。
- Tiandao（production-wiring 返工 `2ff99756479749721a99fa9a2d18807dc2dba0b5`）：`npm test` 退出 0（72 files / 836 tests）；其中 production startup/factory 定点回归与 UI consumer/runtime 共 2 files / 107 tests passed。真实 factory 创建三条互异 Redis 连接；`realm_gate_rejected` 只由 narration pub 发布 `scope=player,target=offline:TargetOnly,style=system_warning`，command pub/sub 均零 publish 且原始 payload 不含 `scope=broadcast`；缺失/alias narration pub、subscribe failure、unsubscribe failure 与幂等 cleanup 均有反例。`cd agent && npm run build` 退出 0（schema + Tiandao TypeScript build）。
- Tiandao：最终受影响树 `ad463518` 执行 `cd agent/packages/tiandao && npm test` 退出 0，72 files / 831 tests passed。测试生成的 `tiandao-snapshot-200.json` / `300.json` 未删除，已保全于 `.sisyphus/evidence/pr1217-tiandao-generated/final-ad463518/`；哈希继续为 `c6ee23bbe36c6fde5f2c5c2d470359c89b0a64174226ab2691a2afe32f82d0ab`、`eb0b116cad4cd803e5548e0faaf5f9d7f77766310304c7e7acf1c964e9d416e8`。
- Server：测试修复树 `1ac6b96f` 在 `/tmp/bong-compile-slot-1.lock` 独占锁内执行 `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings` 与完整 `cargo test`，均退出 0；lib 11,785、binary 11、启动集成 1、背包 e2e 4，合计 11,801 passed / 0 failed / 6 ignored。`network::tests::narration_tests` 10/10 passed，其中正向 canary 证明一次 `collect_received()` 能从同批帧同时观察 typed narration 与一个人工 `GameMessageS2c`，零 chat mirror 断言不再可能因二次 drain 假绿。
- Worldgen：合并 `9d2e29d0` 后 `bash scripts/dev-reload.sh` 退出 0，overworld 306 tiles 与 TSY 9 tiles 后验全绿、server 启动读入 306 tiles；`test_zone_overlap_policy.py` 8/8 passed。额外全量 pytest 为 911 passed / 1 failed，唯一失败 `CompoundFlattenTests::test_flatten_preserves_outside_radius` 在整个 `worldgen/` 与 `origin/main` 字节一致时可复现，记录为 main 同环境基线而非本 PR 门禁绿灯。
- Client：最终合并树 `49eee364` 以 Java `17.0.19` 执行 `./gradlew --no-daemon test build`，退出 0、`BUILD SUCCESSFUL`；471 份 JUnit XML 汇总 4,118 tests / 0 failures / 0 errors / 0 skipped。
- 真实双 Bot/专用 Redis：最终 `4cc9e293` 的 `agent_ui_realm_gate_private_narration` 场景 2.9s PASS，`total=1 pass=1 skip=0 fail=0`。Redis 链依次记录 `bong:agent_ui_cmd`（`target=offline:Bp4cRGA`、`realm_gate=5`）、`bong:agent_ui_response`（同 target、`player_realm=1`、`required_realm=5`）与 `bong:agent_narrate`（`scope=player`、`style=system_warning`）；server 最终只投递 1 recipient，旁观 Bot 无泄漏，双方无 chat mirror。证据位于 `.sisyphus/evidence/pr1217-final-gates-4cc9e293/09-bot-e2e-dedicated-20260718T123407/`。
- Fabric renderer runtime：云端无 WSLg socket，故以用户态解包的 Xvfb + Mesa 软件渲染实际执行 Java 17 `runClient`；日志命中 MC `1.20.1` 窗口、LWJGL `3.3.1`、`Bong Client bootstrap ready`、实体模型注册以及 Armor/WornPack/Mutation `FeatureRenderer registered`，fatal scan 为空。证据位于 `.sisyphus/evidence/pr1217-final-gates-4cc9e293/10-fabric-renderer-runtime-20260718T124321/`。
- 主线同步：`ad463518` 的 Schema/Tiandao/server 门禁通过后，`49eee364` 合入 `origin/main=001bbe7d`，并按触栈以 server 11,800 / client 4,118 tests 全绿收口。CodeRabbit 发现测试观察器会二次 drain 后，`1ac6b96f` 修正全部六组同类调用并重跑 server 完整门禁；随后紧邻 `git fetch origin && git merge origin/main` 生成 `bf0d6582`。`e960ba5f` 合入 `origin/main=5d9bdd8f`（#1241）后，完整 server 门禁（`/tmp/pr1217-gates-server.log`）`FMT_EXIT:0` / `CLIPPY_EXIT:0` / `TEST_EXIT:0`，`test result` 汇总 lib 11807 + bin 11 + 启动集成 1 + 背包 e2e 4（另 5 ignored bench）≈11823 passed / 0 failed / 6 ignored；同树 client JUnit 4141 / schema 892 / tiandao 831 / python 126 亦全绿。最终 `56b6e33d` 合入 `origin/main=2f9c70ad`（#1212，仅 client network/HUD），按触栈以 Java 17 `./gradlew --no-daemon test build` 复跑 client：`CLIENT_EXIT:0`、`BUILD SUCCESSFUL`、JUnit suites=474 tests=4153 failures=0 errors=0 skipped=0（`/tmp/pr1217-postmerge-client.log`）；server 无源码增量复用 `e960ba5f` 门禁。真实双 Bot/专用 Redis 在 `56b6e33d` 重跑 `agent_ui_realm_gate_private_narration`：15.5s PASS，`total=1 pass=1 skip=0 fail=0`；Redis 链依次 `bong:agent_ui_cmd`→`bong:agent_ui_response`→`bong:agent_narrate(scope=player)`，server 最终 `sent ... narration payload to 1 recipient(s)`，旁观无泄漏。证据：`.sisyphus/evidence/pr1217-postmerge-gates-56b6e33d/09-bot-e2e-dedicated-20260720T000254/`。



- **本轮 gate 实测（精确 Ajv + semantic discriminator 返工，代码 HEAD `1e8455876f710bd5507d0fa77d15cf83516adced`）**：
  - 返工起始 HEAD 核验：`git rev-parse HEAD` = `69644d28e45be7ebdd0fad8cd33d7ac609d90ce2`；分支 `tmp/pr1217-review-fix`。开始与结束均逐字复核两个受保护 snapshot，未 stage、未删除、未覆盖、未 clean。
  - 历史 overclaim 继续作废：不得引用「宿主 Ajv 8.12.0 78/78」；host-only `/usr/share/nodejs/ajv` 取径不得恢复。
  - 可复现依赖：`@bong/schema` `devDependencies.ajv = "8.12.0"`，`agent/package-lock.json` 仅同步同一精确声明并继续锁定 `node_modules/ajv@8.12.0`；测试正常 `import Ajv from "ajv"`，显式断言 `AJV_PACKAGE_VERSION === "8.12.0"`。
  - resolved evidence：`ajvPackageJsonPath=/home/serverkizuna/Code/Bong/.claude/worktrees/agent-ae88b34144dc97013/agent/node_modules/ajv/package.json`；`ajvResolvedPath=/home/serverkizuna/Code/Bong/.claude/worktrees/agent-ae88b34144dc97013/agent/node_modules/ajv/dist/ajv.js`；两者均在 workspace `node_modules` 且不含 `/usr/share/nodejs/`。
  - 顺序解耦：删除 `$.anyOf[85]` / `[103]` / `[104]` 与自写下标 JSON path 读取；真实生成物由 `findUniqueVariantByType()` 按 `properties.type.const` 唯一定位 `agent_ui_response` / `agent_ui_request` / `agent_ui_close`。helper 直接测试覆盖 anyOf 前后换序、missing（列出可用 discriminant）与 duplicate（列出全部命中索引）。
  - 真 Ajv gate：自动化 workspace Ajv compile/validate；22 边界 × 6 字段 = 132 次根 validate，另加字段结构 pin 与 root 契约；实测 stdout `{"ajvVersion":"8.12.0","ajvResolvedPath":".../agent/node_modules/ajv/dist/ajv.js","ajvPackageJsonPath":".../agent/node_modules/ajv/package.json","assertionCount":157,"boundaryCases":22,"idFields":6}`。
  - 最终规定顺序门禁：`npm run build -w @bong/schema` 退出 0；`npm run check` 退出 0（406 generated files fresh）；`npm test` 退出 0（29 files / 896 tests，其中 `generated-artifacts.test.ts` 16/16）；`npm run generate` 退出 0 且 generated 目录零 diff；根 `npm run build` 退出 0（schema + Tiandao TypeScript build）。
  - snapshot 哈希（未改、未删）：`tiandao-snapshot-200.json` sha256=`c6ee23bbe36c6fde5f2c5c2d470359c89b0a64174226ab2691a2afe32f82d0ab`；`tiandao-snapshot-300.json` sha256=`eb0b116cad4cd803e5548e0faaf5f9d7f77766310304c7e7acf1c964e9d416e8`。
  - 无上下文 read-only validator：第一步对拍 HEAD，结论 `PASS 1e8455876f710bd5507d0fa77d15cf83516adced`；确认精确依赖/lockfile、workspace 非 host 解析、真实生成物语义唯一定位、Unicode 矩阵与真实 Ajv compile/validate 均被锁住，无 blocker/major。
- **production-wiring 返工补充（代码提交 `2ff99756479749721a99fa9a2d18807dc2dba0b5` + `31707c2dab3fe3d4b068e3419fe0a5c812cbc803`）**：package-local clean install、schema check/build/test/generate freshness、Tiandao 72 files / 836 tests、根 agent build 与 `git diff --check` 均退出 0；两份 snapshot 哈希仍逐字一致且保持 untracked。该补充提交后尚未运行 fresh validator，不能复用 `PASS 1e845587...` 外推到新 HEAD。

### 跨仓库核验

- schema ↔ server：TypeBox 与三份 generated artifact 通过 `{1,128}` well-formed Unicode pattern 按 code points 计数，并以 `(?![\s\S])` 表达绝对结束；Rust 以 `chars().count()` 镜像。65/128 emoji、129 overflow、BMP/astral 混合、LF/CR/CRLF/U+2028/U+2029、空串、显式 `null` 与 lone surrogate 的接受集合一致。独立 `agent-ui-response-payload-v1.json` 继续固化 optional `target_player`；client-request/server-data 两份聚合生成物的相关 variant 改按 `properties.type.const` 唯一定位，六个相关字段不再绑定 `anyOf` 生成顺序。
- server → agent：`receive_agent_ui_cmd_system` 的真实 gate reject response 经生产 Redis encoder 把 canonical `target_player` 交给 `UiResponseConsumer`。
- agent → server：生产 consumer 输出经生产 `bong:agent_narrate` parser 进入 `process_redis_inbound`，`NarrationScope::Player` 最终选择唯一目标玩家；legacy 缺失或空白目标只记录 warning/丢弃计数，不发布 narration，因而不存在 broadcast 旁路。
- server → client：目标玩家恰好收到一条 typed `bong:server_data` narration，旁观者无 typed payload；每个 mock client 只执行一次 `collect_received()`，并从同一批帧同时分类 typed narration 与 `GameMessageS2c`，因此“两名玩家都没有 chat mirror”是有效负断言而非空缓冲假绿。
- client runtime：实际 Fabric renderer 线程完成 Bong 网络/HUD/动画/实体与 FeatureRenderer bootstrap，并创建 `Minecraft* 1.20.1` 窗口；不是只靠 JUnit classpath 或静态资源测试推断。
- 提交元数据：逐枚执行 `git interpret-trailers --parse` 审计 `origin/main..HEAD`，所有本 PR commit 均存在精确 `Model:` trailer；早期提交为 `gpt-5.1`，返工提交为 `gpt-5.6-sol-max`，后续协议/e2e/主线复验与证据更新为 `gpt-5`。

### 遗留 / 后续

- legacy server→agent payload 仍允许缺省 `target_player` 以支持滚动升级，但私人境界门提示会 fail-closed；若未来必须恢复这类旧消息的玩家反馈，应另立 plan 建立可信 `request_id → canonical player` 关联，不得恢复 broadcast fallback。
- TSY `target_player` fallback 与本 plan 明确排除的 revelation/button-click 问题不在本次范围。
