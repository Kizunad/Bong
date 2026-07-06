# plan-bughunt-war-participate-agent-command-drift-v1

## Bug 摘要

`bong:agent_command` 的 headless 战争参与路径 B 存在协议漂移，导致 P6 玩家参战等价入口在 Redis 入站前被拒绝，无法到达 `execute_war_participate_headless`。

具体有两层阻断：

1. `scripts/e2e-offscreen-war.sh` 发布的 war participate 命令使用 `commands[0].command_type`，而 `AgentCommandV1` wire schema 要求字段名为 `type`。
2. 即便把字段名修成 `type: "faction_event"`，当前 TS schema 与 Rust Redis 入站校验仍对所有 `faction_event` 强制要求 `params.kind` / `params.faction_id`，但 `war_participate=true` 设计形状只携带 `player_id` / `role` / `group`。

## 实际游玩体验影响

对实际游玩体验的影响：在线玩家 `/faction join|mercenary|intercept` 路径 A 不在本 bug 范围内；受影响的是 plan-offscreen-war-v1 P6 明确保留的 headless/agent-command 等价入口。该入口用于在无真实 MC client 的真服 e2e、调试和自动化场景中模拟玩家参战，验证 `WarParticipateIntent` 汇入战争角色计数。

现在脚本只看到 Redis `publish` 返回就记录注入成功，但 server 实际在 `parse_inbound_message` 阶段丢弃 payload，游戏内不会产生 `WarParticipateIntent`，战争 telemetry 的 mercenary/enlist/intercept 计数也不会变化。结果是玩家战争参与链路的自动验证面假阳性，调试者会以为“玩家参战已经注入”，实际游戏状态没有响应。

## 证据定位

- `scripts/e2e-offscreen-war.sh:1229-1244` 构造 headless war participate payload；`commands[0].command_type` 不是 `type`，且 `params` 只有 `war_participate/player_id/role/group`。
- `scripts/e2e-offscreen-war.sh:1910-1917` 把 headless 注入作为 P6 路径 B 执行；失败只按 Redis 连接问题处理。
- `agent/packages/schema/src/agent-command.ts:30-45` 定义 `Command` 字段为 `type/target/params`。
- `agent/packages/schema/src/agent-command.ts:122-129` 对所有 `faction_event` 强制校验 `kind` 与 `faction_id`。
- `agent/packages/tiandao/src/parse.ts:52-61` agent parse 会逐条调用 `validateAgentCommandV1Contract`，失败即丢弃命令。
- `server/src/network/redis_bridge.rs:2298-2304` `bong:agent_command` 入站先调用 `validate_agent_command_value`，成功才反序列化为 `RedisInbound::AgentCommand`。
- `server/src/network/redis_bridge.rs:2453-2455` Redis 入站命令字段只允许 `type/target/params`。
- `server/src/network/redis_bridge.rs:2496-2518` 对所有 `faction_event` 强制要求 `params.kind` 与 `params.faction_id`。
- `server/src/network/command_executor.rs:401-418` 执行器已有 `war_participate=true` 前置分支，但它位于 Redis 入站校验之后。
- `server/src/network/command_executor.rs:462-517` `execute_war_participate_headless` 会按 `player_id/role/group` 发出 `WarParticipateIntent`。
- `server/src/npc/war/mod.rs:607-640` `handle_war_participate_intent` 是路径 A/B 的真实汇聚消费者。
- `server/src/network/mod.rs:483-505` war participate consumer 已接入调度，并排在 war publish 前。

## 触发路径

1. 启动 offscreen war e2e 或手动用同形 payload 发布到 `bong:agent_command`。
2. Redis 收到 payload 后进入 `server/src/network/redis_bridge.rs::parse_inbound_message`。
3. `validate_agent_command_value` 先因 `command_type` 非法字段拒绝；若改成 `type`，仍因 `faction_event` 缺 `kind/faction_id` 拒绝。
4. `RedisInbound::AgentCommand` 不产生，`process_redis_inbound` 不会 enqueue batch。
5. `command_executor::execute_faction_event` 的 `war_participate=true` 分支不会运行。
6. `WarParticipateIntent` 不发出，战争角色计数和 `bong:faction/war` telemetry 不反映玩家参战。

## 反方审查记录

- Round 1：PASS。反方确认脚本 payload、TS schema、Redis 入站校验、执行器路径 B 与真实 consumer 的事实链成立；同时指出 `command_type` 是比 `kind/faction_id` 更早的阻断点。
- Round 2：PASS，但要求降级表述。反方确认这不是在线玩家 `/faction` 主路径失效，而是 P6 headless 等价入口、agent-command 调试面和 e2e 自动验证面失效；开放 PR 与已知本轮产出未发现重复。

## Skeleton Fix Plan

- [ ] 修正 `scripts/e2e-offscreen-war.sh` 的 headless war participate payload：`command_type` 改为 `type`。
- [ ] 在 `agent/packages/schema/src/agent-command.ts` 中给 `type === "faction_event" && params.war_participate === true` 增加专属语义校验分支，不要求 `kind/faction_id`。
- [ ] 在 `server/src/network/redis_bridge.rs::validate_command_value` 中加入同形分支，保证 Redis 入站 contract 与 agent schema 一致。
- [ ] war participate 分支校验 `player_id` 非空字符串，`role` 限定 `enlist|mercenary|intercept|spectate`，`group` 为可选非负整数；保留执行器现有 u16 溢出时忽略 group 的行为。
- [ ] 不新增 `CommandType` 作为首修，避免扩大到 schema、agent prompt、arbiter、server executor 分发表和旧命令兼容面的改动。
- [ ] 不通过给 payload 塞 dummy `kind/faction_id` 绕过，因为这会污染 `war_participate` 的协议语义，并留下双端 schema 漂移。

## 验收测试计划

- `agent/packages/schema`：新增 `validateAgentCommandV1Contract` 正例，断言 `type=faction_event + war_participate=true + player_id/role/group` 通过；保留普通 `faction_event` 缺 `kind/faction_id` 仍拒绝。
- `server/`：新增 Redis 入站 parse pin，断言同形 payload 可从 `bong:agent_command` 解析成 `RedisInbound::AgentCommand`；同时覆盖 `command_type` 字段仍拒绝。
- `server/`：补一条端到端单测，从 `parse_inbound_message` 到 enqueue/execute 证明 `WarParticipateIntent` 发出，避免只测 `CommandExecutorResource::enqueue_batch` 绕过入站校验。
- 根目录 e2e：修正 `scripts/e2e-offscreen-war.sh` 后，将 `mercenary_count >= 1` 从 NOTE/弱观测提升为明确失败条件，防止 Redis publish 成功假阳性。
- schema 改动后必须重建 schema dist：`cd agent && npm run build -w @bong/schema`。

## 风险

- `faction_event` 既承载旧 faction store 事件，又复用为 war participate 路径 B；放宽时必须只对 `war_participate === true` 生效，避免普通 faction 事件漏过必填字段。
- TS schema 与 Rust Redis 入站校验要同步，否则会出现 agent 生成通过但 server 丢弃，或 server 可收但 agent parse 丢弃的单边漂移。
- e2e 从 NOTE 改为失败后，可能暴露既有战争生成时序不稳定；修复时要让失败信息区分“未形成 war”与“war 已形成但参与注入未生效”。
