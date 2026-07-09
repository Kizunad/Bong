# BugHunt: e2e Redis 命令锚点把拒绝也算执行闭环

> 分区：e2e-protocol / r08
> 类型：测试覆盖实际断链
> 结论：高置信候选。当前 `scripts/e2e-redis.sh` 只证明 Tiandao 的 `agent_command` 到达 server 并被命令执行器处理到 `stage=end`，但没有证明命令成功生效；`result=rejected_*` 同样会让 e2e 绿。

## 一句话 bug

`e2e-redis.sh` 的 server execution anchor 只 grep `command_anchor stage=end`，而 `command_executor.rs` 对成功和拒绝都会打同一个 `stage=end`，导致 Tiandao 命令全被拒绝时 task-13 Redis e2e 仍可能通过。

## 实际游玩体验影响

玩家实际会看到天道似乎在运转、叙事也可能照常出现，但本应由 agent command 驱动的世界变化没有发生：灾劫不落地、区域灵气/危险度不变化、NPC 或事件投放没有进入世界。CI 仍显示 e2e 通过，会把“天道只会说话、不改世界”的断链放过到可游玩版本。

## 证据

1. `scripts/e2e-redis.sh` 的关键 server 断言只检查任意结束锚点：
   - `scripts/e2e-redis.sh:985`：`wait_for_pattern "$SERVER_LOG" "\\[bong\\]\\[network\\] command_anchor stage=end"`
   - 该脚本随后只检查 TPS 与 Tiandao narration publish，没有检查 `result=` 内容。

2. `server/src/network/command_executor.rs` 对每条命令无条件记录 `stage=end ... result={}`：
   - `server/src/network/command_executor.rs:355`：`command_anchor stage=end ... result={}`
   - 因此 `result=ok` 与 `result=rejected_*` 都满足 e2e 当前 grep。

3. 拒绝路径真实存在且覆盖主要 agent command：
   - `server/src/network/command_executor.rs:595`：`spawn_npc` 未知 zone 返回 `rejected_unknown_zone`
   - `server/src/network/command_executor.rs:970`：`spawn_event` 失败返回 `rejected_spawn_event`
   - `server/src/network/command_executor.rs:1116`：`modify_zone` 未知 zone 返回 `rejected_unknown_zone`
   - `server/src/network/command_executor.rs:1124`：`modify_zone` 参数非法返回 `rejected_invalid_spirit_qi_delta`

4. task-13 deterministic tick 确实会产出需要 server 生效的命令：
   - `agent/packages/tiandao/src/task-13-one-tick.ts:18`：`spawn_event target=spawn`
   - `agent/packages/tiandao/src/task-13-one-tick.ts:47`：`modify_zone target=spawn`

## 去重

- 不是 #1059：#1059 是战斗 bot e2e 对任意 `server_data` 的假阳性；本题是 Redis agent_command 的 server execution anchor 过宽。
- 不是 #994 / #999 / #1010 / #1021：这些是具体 ClientRequest/schema/proto 断链；本题不修改协议形状，聚焦 e2e 对 command result 的判定。
- 避开 #1068-#1072：本题不是 bot playtest 修复批、生产场景组、预算锚或骨架 plan 主题。
- 已 grep 现有 `docs/plans-skeleton` / `docs/finished_plans` / 当前 PR 标题，未发现同题。相邻的 `plan-bot-e2e-coverage-v1` 是孤儿 server 覆盖问题，不是 `result=rejected_*` 假绿。

## 对抗结论

- 第一轮 A 提出的 `mineral_probe_result` 网络线程直触 HUD/SFX 与 #1049 重复，淘汰。
- 第一轮 B 提出两个 e2e Redis 覆盖候选；narration publish 未验证 client 可见被第二轮降级为 coverage backlog，因为 server/client 主链已有直接测试。
- 第二轮反方确认本候选成立：repo 内未见同题，`e2e-redis.sh` 没有其它 grep 能保证 `result` 非 `rejected_*`，`command_executor.rs` 的拒绝分支确实会打同样 `stage=end`。

## 修复 TODO

- [ ] TODO(e2e): 将 `scripts/e2e-redis.sh` 的 server execution anchor 收窄为至少一个非拒绝结果，例如 grep `command_anchor stage=end .* result=(ok|applied|queued|spawned|modified|event_spawned)`，或显式排除 `result=rejected_`。
- [ ] TODO(e2e): 针对 deterministic tick 的两个命令分别 pin 成功锚点：`spawn_event target=spawn` 必须成功，`modify_zone target=spawn` 必须成功。
- [ ] TODO(server/test): 增加最小日志/fixture 回归：构造 `result=rejected_unknown_zone` 的 command_anchor 时，e2e anchor 判定必须失败。
- [ ] TODO(e2e): 保留 Redis publish proof，但把“发布到 Redis”和“server 成功改变世界”拆成两个独立 pass 项，避免用 transport proof 替代 gameplay proof。

## 验收

- `bash scripts/e2e-redis.sh` 在当前 deterministic `task-13-one-tick.ts` 下仍绿。
- 人为把 deterministic command target 改成不存在的 zone，或注入非法 `spirit_qi_delta`，脚本必须在 server execution anchor 阶段红，而不是因为 `stage=end result=rejected_*` 假绿。
- 验收日志中能看到至少一条非 `rejected_*` 的 `command_anchor stage=end`，并能对应到玩家实际可感知的世界变化。
