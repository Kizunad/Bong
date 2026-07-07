# plan-bughunt-task13-mutation-qi-zero-green-v1

> Skeleton Plan。BugHunt e2e-protocol / 20260708 r01。主题：Task 13 Redis e2e 的 deterministic mutation fixture 声称覆盖区域灵气变更，但唯一的负向 `spirit_qi_delta` 会被 Arbiter 守恒归一成 `0`；server 仍以 `result=ok` 执行 `modify_zone` 并只落下 `danger_level_delta`，`scripts/e2e-redis.sh` 因只看发布 / 执行锚点而假绿。

## 一句话 bug

Task 13 mutation 的灵气覆盖声明是假绿：fixture 发出的 `spawn:-0.05` 在 Arbiter 合并后自我归零，e2e 没有断言非零 `spirit_qi_delta` 或 `world_state` 中目标 zone 的灵气实际变化。

## 实际游玩体验影响

- 线上天道“变化 Agent”要求灵气守恒：一处增加必须对应另一处减少。当前 e2e 却用单个负向 delta 证明 mutation 链路，实际不会改变任何区域灵气。
- 玩家会看到环境危险度变化、天道叙事或后续行为像是“地脉 / 灵气发生了偏移”，但测试没有证明服务端世界状态里的 `spirit_qi` 真的动过。
- 这会掩盖真实回归：未来如果 mutation 只剩危险度、叙事或空灵气 delta，CI 仍可能绿色，导致环境塑造玩法看起来有反馈、实际灵气生态不随天道演化。

## 证据定位

- `agent/packages/tiandao/src/task-13-one-tick.ts:42-60`：Task 13 mutation fixture 只发一条 `modify_zone` 到 `spawn`，参数为 `spirit_qi_delta:-0.05` 与 `danger_level_delta:1`。
- `agent/packages/tiandao/src/runtime.ts:624-629`：Redis 发布的是 Arbiter merge 后的 `merged.commands`，不是原始 fixture。
- `agent/packages/tiandao/src/arbiter.ts:467-546`：`applySpiritQiConservation` 会收集所有 `modify_zone.spirit_qi_delta`；当只有负向 delta 时 `positiveSum === 0`，`negativeScale = 0`，负向 `spirit_qi_delta` 被写回 `0`。
- `agent/packages/tiandao/src/redis-ipc.ts:913-924`：`publishCommands` 直接把 merge 后 commands JSON 发到 `bong:agent_command`。
- `server/src/network/command_executor.rs:1119-1145`：server 接受数值型 `spirit_qi_delta` 后执行 `zone.spirit_qi + delta`，`delta = 0` 时灵气不变；随后 `danger_level_delta` 仍可落地，并返回 `"ok"`。
- `scripts/e2e-redis.sh:958-960`：e2e 只 grep Tiandao “published commands” 日志，未解析 command payload。
- `scripts/e2e-redis.sh:973-985`：e2e 只检查 `bong:agent_command` channel 与 server `command_anchor stage=end`，未断言 `result=ok` 对应的具体 mutation 效果。
- `scripts/e2e-redis.sh:999-1002`：e2e 只检查 narration 发布锚点，不能替代灵气状态断言。
- `agent/packages/tiandao/src/skills/mutation.md:12-14`：生产 prompt 明确写着灵气守恒“一处增加就必须减少另一处”，说明单个负向 delta 不是合格覆盖样例。
- `agent/packages/tiandao/src/skills/mutation.md:41-45`：当前 prompt 示例也只给单个负向 delta，可作为后续 P1 文档修正线索，但本 bug 主证据仍是 Task 13 deterministic fixture 与 e2e 假绿。

## 不重复说明

- 不重复 #1109“e2e Redis 命令锚点假绿”：#1109 的核心是 rejected command 仍能产出 `stage=end`。本问题中 server 会返回 `ok`，即使 #1109 改成要求 `result=ok` 也抓不到“灵气 delta 已被归零、实际无灵气变化”。
- 不重复 #1059“bot 战斗 server_data 类型断言假阳性”：本问题不涉及 bot combat / server_data 类型断言，只涉及 Task 13 Redis e2e、Tiandao Arbiter 合并与 `modify_zone` 效果断言。
- 不重复 #994 / #1054 / #1093 / #1111：这些分别聚焦 C2S schema drift、离屏战果遗物 schema、Tiandao schema dist 启动断链、proto breaking gate 浅拉假跳过；本问题不是 schema 枚举 / dist / proto gate，而是 mutation fixture 被守恒归零后的 false green。
- 不重复 #1119：炼丹过程事件 Tiandao 断链与本 Task 13 mutation 灵气效果断言无交集。

## 修复计划骨架

### P0 - 修正 Task 13 mutation fixture

- [ ] 把 Task 13 deterministic mutation 改为成对 `modify_zone` commands：至少两个已存在 zone，一正一负，绝对值均在单次限制内，合计接近 `0`，经过 Arbiter merge 后仍保持非零 `spirit_qi_delta`。
- [ ] 保留或调整 `danger_level_delta` 只能作为额外效果，不能作为“灵气 mutation 已落地”的替代证明。
- [ ] 若 fixture 依赖 zone 名，先使用 e2e 世界状态中稳定存在的 zone；不要引入只在某些 worldgen seed 下存在的目标。

### P0 - 加强 Redis e2e command 断言

- [ ] 在 `scripts/e2e-redis.sh` 中解析 `bong:agent_command` JSON，定位 Task 13 mutation 的 `modify_zone` command，断言至少一条 `spirit_qi_delta` 为非零。
- [ ] 断言正负成对 delta 的净值满足守恒 epsilon，避免用单边 delta 绕过 Arbiter 规则。
- [ ] 不再只依赖 `published commands` 日志与 `command_anchor stage=end` grep 作为 mutation 成功证明。

### P0 - 加强 world_state 效果断言

- [ ] 在 command 执行前后采样 `bong:world_state`，对目标 zone 的 `spirit_qi` 做前后差异断言。
- [ ] 断言至少一个目标 zone 的 `spirit_qi` 变化量与 command delta 方向一致；`danger_level` 变化可以作为附加证明，但不能替代灵气变化证明。
- [ ] 增加回归用例：如果 fixture 退回单个 `spawn:-0.05`，e2e 必须失败，失败原因应指向 `spirit_qi_delta` 被归零或 `world_state` 灵气未变化。

### P1 - 修正文档示例

- [ ] 更新 `agent/packages/tiandao/src/skills/mutation.md` 的输出示例，改成一正一负的守恒区域灵气变更，避免 prompt 示例继续训练出单边 delta。

## 验证计划

- [ ] agent 栈：`cd agent && npm run build`，确认 Task 13 fixture 与 schema 类型仍可编译。
- [ ] e2e 栈：仓库根设置 `export BONG_SKIP_SKIN_PREFETCH=1` 后跑 `bash scripts/smoke-test-e2e.sh`，确认 Redis command payload 与 world_state 灵气效果都被 pin 住。
- [ ] 负向回归：临时把 fixture 改回单个 `spawn:-0.05`，验证新增 e2e 断言红；恢复后再绿。

## 对抗复核记录

### Round 1

- 反方质疑：候选可能只是 #1109 的重复，因为 e2e 已知只看 `command_anchor stage=end`；也可能只是“命令部分生效但灵气没变”的低影响问题。
- 回应：收窄口径，不再泛化为 command anchor false green。这里 server 结果是 `ok`，且 `danger_level_delta` 会落地；唯一被掩盖的是 Task 13 声称覆盖的区域灵气 mutation。这个断言缺口不会被 #1109 的 rejected-command 修复自然覆盖。
- 结论：`NEEDS_NARROWING`，需要明确不是 mutation 完全 no-op。

### Round 2

- 反方质疑：既然危险度能变化，是否足以证明 mutation 链路？是否应把问题归为 prompt 示例瑕疵而不是 e2e false green？
- 回应：Task 13 的变化 Agent 主契约包含 `spirit_qi_delta` 与守恒；危险度变化只能证明 `modify_zone` 处理器部分可达，不能证明灵气生态被修改。主故障链是 deterministic fixture 单边 delta 经 Arbiter 自我归零，而 e2e 没有 payload / world_state 断言。
- 结论：`PASS_NARROWED`。独立 PASS 的唯一合法口径是“Task 13 mutation 的灵气覆盖声明假绿”，不是泛化的 command_anchor 假绿，也不是 mutation 完全未落地。

## 本 PR 边界

- 本 PR 只新增本 Skeleton Plan，不修改代码、配置、资源或依赖。
- 本轮未执行修复测试；证据来自只读代码审计、PR 避重检索与对抗复核。
