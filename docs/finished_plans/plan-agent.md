# Agent 路线详细计划（TypeScript / 天道）

> 从单次推演骨架推进到持续运行的多 Agent 仲裁系统 + 玩家信号感知。
> 每个 Task 标注前置依赖、输入输出、验证方式。

---

## 当前代码结构

```
agent/packages/
├── schema/           # IPC 数据定义 (TypeBox)
│   ├── src/          # common, channels, world-state, agent-command, narration, chat-message
│   ├── samples/      # 共享校验 JSON
│   └── generated/    # JSON Schema 导出
└── tiandao/          # 天道 Agent
    ├── src/
    │   ├── main.ts         # 入口（--mock / Redis loop）
    │   ├── agent.ts        # TiandaoAgent 类
    │   ├── context.ts      # Context Assembler + recipes
    │   ├── llm.ts          # OpenAI-compatible client
    │   ├── parse.ts        # LLM 输出 → AgentDecision
    │   ├── mock-state.ts   # 测试用假数据
    │   ├── redis-ipc.ts    # Redis pub/sub + list
    │   └── skills/         # 3 份 system prompt (md)
    └── package.json
```

---

## M1 — 天道闭环

### A1. Arbiter 仲裁层 `arbiter.ts`

**目标**：合并三 Agent 输出，消解冲突，校验规则约束。

**新增文件**：`tiandao/src/arbiter.ts`

**输入**：`AgentDecision[]`（最多 3 个）
**输出**：`{ commands: Command[], narrations: Narration[] }`

**仲裁规则**：

```typescript
class Arbiter {
  merge(decisions: AgentDecision[]): MergedResult {
    const allCommands: TaggedCommand[] = flatten + tag source
    const allNarrations: Narration[] = flatten all

    // 1. 规则硬约束
    filter out:
      - intensity < 0 或 > 1
      - target zone 不存在（对照 latestState.zones）
      - 针对 composite_power < NEWBIE_POWER_THRESHOLD 的 spawn_event

    // 2. 灵气守恒
    sum all spirit_qi_delta:
      - 如果 |net_delta| > 0.01，按比例缩放使总和 ≈ 0

    // 3. 同 zone 冲突消解
    group commands by (target zone):
      - 同 zone 有多条 modify_zone → 合并 delta
      - 同 zone 有 spawn_event + modify_zone → 保留两者（不冲突）
      - 同 zone 有多个 agent 的 spawn_event → 优先级：Era > Mutation > Calamity
        → 保留高优先级的，丢弃低优先级

    // 4. 预算控制
    truncate to MAX_COMMANDS_PER_TICK (5)

    // 5. narrations 不做冲突消解（全部保留，最多 10 条）
  }
}
```

**集成到 main.ts**：
```typescript
// 替换当前的 per-agent publish
const merged = arbiter.merge(decisions);
await redis.publishCommands("arbiter", merged.commands);
await redis.publishNarrations(merged.narrations);
```

**验证**：
- 单元测试：两个 agent 对同一 zone 发 modify_zone，delta 被合并
- 单元测试：灵气总 delta != 0 时被缩放
- 单元测试：超过 5 条指令被截断

---

### A2. Chat 信号预处理 `chat-processor.ts`

**目标**：从 Redis List 读取玩家聊天，用廉价模型批量标注，注入 context。

**新增文件**：`tiandao/src/chat-processor.ts`

**流程**：

```
1. LRANGE + LTRIM bong:player_chat（drain list，非阻塞）
2. 解析为 ChatMessageV1[]
3. 如果消息数 > 0，调用 LLM 批量标注：
   prompt: "对以下玩家聊天标注 sentiment(-1~1), intent(complaint/boast/social/help/provoke/unknown)。输出 JSON 数组。"
   model: 用最便宜的模型（gpt-5.4-mini 或配置的 annotate model）
4. 输出 ChatSignal[]
5. 缓存到 WorldModel，供 context block 消费
```

**Schema 扩展**：`ChatSignal` 已在 schema 包中定义（`chat-message.ts`）。

**Redis List 操作**：`redis-ipc.ts` 增加 `drainChatList()` 方法：
```typescript
async drainChatList(): Promise<ChatMessageV1[]> {
  const raw = await this.pub.lrange("bong:player_chat", 0, -1);
  if (raw.length > 0) await this.pub.del("bong:player_chat");
  return raw.map(s => JSON.parse(s));
}
```

**Context Block**：在 `context.ts` 增加 `chatSignalsBlock`：
```
## 近期民意 (最近 5 分钟)
- Steve [blood_valley]: 抱怨灵气不足 (sentiment: -0.7, intent: complaint)
- Alex [green_cloud_peak]: 闲聊 (sentiment: 0.3, intent: social)
民意倾向: 偏负面 (-0.3)
```

**验证**：
- 手动 `redis-cli RPUSH bong:player_chat '{"v":1,"ts":...,"player":"Steve","raw":"灵气太少了","zone":"spawn"}'`
- Agent 日志中看到 chat signal 被注入 context

---

### A3. 循环模式稳定化

**目标**：Agent 持续运行不崩溃，优雅处理各种异常。

**改动**：`main.ts` + `agent.ts`

**具体项**：

```
错误重试：
  - LLM 调用失败 → catch，log warning，跳过本轮（不崩溃）
  - Redis 断连 → ioredis 自带重连，log warning
  - JSON parse 失败 → parseDecision 已有 fallback (EMPTY_DECISION)

节流：
  - 如果 LLM 返回超过 30s → 超时取消（AbortController）
  - 如果 LLM 连续 3 次失败 → 该 agent 休眠 60s（指数退避上限）

指标日志：
  - 每 tick 记录：耗时、token 估算、commands/narrations 数量
  - 每 10 tick 打印汇总

Graceful shutdown：
  - SIGINT/SIGTERM → 等待当前 tick 完成 → disconnect Redis → exit
  - 已实现，确认无悬挂 Promise
```

**验证**：
- 断开 Redis → agent 不崩溃，重连后恢复
- LLM API 返回 500 → agent 跳过，下一轮正常

---

### A4. Peer Decisions 上下文

**目标**：每个 Agent 能看到其他 Agent 上一轮的决策摘要。

**实现**：

```typescript
// WorldModel 增加字段
class WorldModel {
  latestState: WorldStateV1 | null;
  chatSignals: ChatSignal[];
  lastDecisions: Map<string, AgentDecision>; // agent_name → decision
}

// main loop 中，tick 结束后记录
for (const [i, decision] of decisions.entries()) {
  worldModel.lastDecisions.set(agents[i].name, decision);
}
```

**Context Block**：`peerDecisionsBlock`
```
## 其他天道意志
- 灾劫 Agent (上一轮): 在 blood_valley 降天劫 (intensity 0.6)
- 变化 Agent (上一轮): blood_valley 灵气 -0.05, green_cloud_peak +0.05
- 演绎时代 Agent (上一轮): 无行动
```

**按 recipe 配置**：Calamity 的 peer_decisions priority=4（低），Era 的 priority=1（高）。

---

## M2 — 有意义的世界

### A5. 世界模型时序记忆

**目标**：Agent 能看到 zone 状态的变化趋势，不只是当前快照。

**新增**：`tiandao/src/world-model.ts`

```typescript
class WorldModel {
  // 保留最近 N 轮（N=10）的 zone 快照
  zoneHistory: Map<string, ZoneSnapshot[]>;

  updateState(state: WorldStateV1) {
    for (const zone of state.zones) {
      const history = this.zoneHistory.get(zone.name) ?? [];
      history.push(zone);
      if (history.length > 10) history.shift();
      this.zoneHistory.set(zone.name, history);
    }
  }

  getZoneTrend(name: string): "rising" | "stable" | "falling" {
    // 比较最近 3 轮的 spirit_qi 平均值 vs 前 3 轮
  }
}
```

**Context Block**：`eraContextBlock`
```
## 世界趋势 (最近 10 轮)
- blood_valley: 灵气 0.42 → 0.37 (↓下降中)
- green_cloud_peak: 灵气 0.88 → 0.93 (↑上升中)
- newbie_valley: 灵气 0.95 → 0.95 (→稳定)
整体灵气: 微降 (-0.02)
```

---

### A6. 平衡算法

**目标**：计算 Gini 系数 + 平衡建议，注入 context。

**新增**：`tiandao/src/balance.ts`

```typescript
function giniCoefficient(powers: number[]): number {
  // 标准 Gini 计算
  const sorted = [...powers].sort((a, b) => a - b);
  const n = sorted.length;
  if (n === 0) return 0;
  const sum = sorted.reduce((a, b) => a + b, 0);
  if (sum === 0) return 0;
  let numerator = 0;
  for (let i = 0; i < n; i++) {
    numerator += (2 * (i + 1) - n - 1) * sorted[i];
  }
  return numerator / (n * sum);
}

function balanceAdvice(players: PlayerProfile[]): string {
  const gini = giniCoefficient(players.map(p => p.composite_power));
  const strong = players.filter(p => p.composite_power > 0.7);
  const weak = players.filter(p => p.composite_power < 0.3);
  // 生成结构化建议文本
}
```

**Context Block**：`balanceBlock`
```
## 天道平衡态
Gini 系数: 0.68 (严重失衡)
强者: Steve(0.85), Herobrine(0.78) — 集中在 blood_valley
弱者: NewPlayer1(0.08) — newbie_valley
建议: 对 Steve 施压, newbie_valley 增加机缘密度
```

---

## M3 — 叙事与个体关注

### A7. 叙事质量优化

**改动**：skills/*.md prompt 工程

- 要求 narration 文本使用**半文言半白话**风格
- 加入"比喻库"提示（如"天劫如蛟龙出海"、"灵气如春水东流"）
- 限制长度：100-200 字/条
- 要求给出"预兆"（下一轮可能做什么的暗示），增加叙事连续性

### A8. 个体关注

**改动**：context.ts + recipes

- 新增 `keyPlayerBlock`：识别 composite_power 极值 / karma 极值 / 新加入的玩家
- Calamity 的 recipe 中 keyPlayerBlock priority=0（最高）
- Era 的 recipe 中 keyPlayerBlock priority=3（低，时代不关注个体）

```
## 关键人物
- Steve: 综合最强(0.85), karma 偏负(-0.45), 连续击杀 8 次 — 因果将至
- NewPlayer1: 新入世(0.08), karma 中性 — 天道怜弱
```

### A9. 时代实质化

**改动**：Era agent skill prompt + arbiter

- Era 宣告时代后，arbiter 自动附加全局 modify_zone（如"末法纪 → 所有 zone spirit_qi_delta -0.02"）
- 时代状态持久化到 WorldModel：`currentEra: { name, since_tick, global_effect }`
- 后续 tick 中 era_context block 显示当前时代

---

## 文件规划总览

```
agent/packages/tiandao/src/
├── main.ts              # 入口 (--mock / Redis loop)
├── agent.ts             # TiandaoAgent 类
├── arbiter.ts           # 仲裁层 (M1)
├── balance.ts           # Gini + 平衡建议 (M2)
├── chat-processor.ts    # 聊天标注 (M1)
├── context.ts           # Context Assembler + blocks + recipes
├── llm.ts               # OpenAI client
├── mock-state.ts        # 测试假数据
├── parse.ts             # LLM 输出解析
├── redis-ipc.ts         # Redis pub/sub/list
├── world-model.ts       # 世界模型 + 时序记忆 (M2)
└── skills/
    ├── calamity.md
    ├── mutation.md
    └── era.md
```

---

## 开发顺序建议

```
M1 顺序：
  A3 循环稳定化（先做，后续开发都需要跑循环）
  A1 Arbiter（核心，指令合并）
  A2 Chat 预处理（独立，可并行）
  A4 Peer decisions（依赖 A1 的 merged result 记录）

M2 顺序：
  A5 世界模型时序记忆（独立）
  A6 平衡算法（独立，可并行）

M3 顺序：
  A7 叙事优化（prompt 改动，随时可做）
  A8 个体关注（依赖 A6 的 balance 数据）
  A9 时代实质化（依赖 A5 的时序数据）
```

---

## 测试策略

- **单元测试**（vitest）：arbiter 冲突消解、balance Gini 计算、parse 容错
- **Mock 模式测试**：`npm run start:mock` 用假数据跑完整 tick，验证 prompt 格式和 JSON 输出
- **集成测试**：启动 Redis + server + agent，观察日志确认端到端指令传递
