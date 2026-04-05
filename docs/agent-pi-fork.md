# 天道 Agent — 基于 Pi Fork 的实施方案

> Fork badlogic/pi-mono，剥离 coding 工具，注入天道领域工具和 skills，
> 复用 agent-core (agent loop + tool calling + state) 和 pi-ai (multi-provider LLM)。

---

## 一、Pi 架构映射

### Pi 原始分层

```
pi-ai          — 统一 LLM API (OpenAI/Anthropic/Google/Bedrock...)
pi-agent-core  — Agent loop + tool calling + state + events
pi-coding-agent — coding tools (bash/read/write/edit) + skills + CLI + session
pi-tui         — 终端 UI
pi-web-ui      — Web 组件
pi-mom         — Slack bot
pi-pods        — vLLM 部署
```

### Bong 需要什么

```
pi-ai           ✅ 原样保留 — 多 provider LLM 调用
pi-agent-core   ✅ 原样保留 — agent loop 是核心引擎
pi-coding-agent ⚡ 重度改造 → pi-tiandao-agent
pi-tui          ⚠️ 可选保留 — 调试/监控用
pi-web-ui       ❌ 删除
pi-mom          ❌ 删除
pi-pods         ❌ 删除
```

---

## 二、Fork 改造清单

### 保留不动

| 模块 | 原因 |
|------|------|
| `packages/ai/` | LLM provider 抽象、streaming、token 计算 — 完全通用 |
| `packages/agent/` | Agent class、AgentLoop、tool calling、events — 完全通用 |
| `packages/tui/` | 可选，天道 Agent 的调试终端 |

### 删除

```
packages/coding-agent/src/core/tools/bash.ts
packages/coding-agent/src/core/tools/edit.ts
packages/coding-agent/src/core/tools/edit-diff.ts
packages/coding-agent/src/core/tools/write.ts
packages/coding-agent/src/core/tools/read.ts
packages/coding-agent/src/core/tools/grep.ts
packages/coding-agent/src/core/tools/find.ts
packages/coding-agent/src/core/tools/ls.ts
packages/coding-agent/src/core/bash-executor.ts
packages/web-ui/
packages/mom/
packages/pods/
```

### 新增：天道领域工具 (替换 coding tools)

```
packages/tiandao-agent/src/core/tools/
├── index.ts                 # 导出所有天道工具
├── redis-bridge.ts          # 收发 Redis 消息 (world_state / agent_command)
├── world-query.ts           # 查询世界状态（玩家画像、区域信息、事件日志）
├── issue-command.ts         # 下达天道指令（spawn_event / modify_zone / npc_behavior）
├── narrate.ts               # 发布叙事文本
└── balance-check.ts         # 查询天道平衡态（Gini系数、玩家实力分布）
```

### 新增：天道 Skills (替换 coding skills)

```
.pi/skills/                          # 或 packages/tiandao-agent/skills/
├── calamity/
│   └── SKILL.md                     # 灾劫 Agent 人格 + 规则 + 输出格式
├── mutation/
│   └── SKILL.md                     # 变化 Agent 人格 + 规则
├── era/
│   └── SKILL.md                     # 演绎时代 Agent 人格 + 规则
├── balance/
│   └── SKILL.md                     # 天道平衡分析技能
└── chat-sense/
    └── SKILL.md                     # 玩家聊天信号解读技能
```

---

## 三、Pi 关键机制 → 天道映射

### 1. Tool System

Pi 的 `AgentTool` 接口完全通用：

```typescript
// Pi 原始 tool 定义模式
interface AgentTool<TParameters, TDetails> {
  name: string;
  label: string;
  description: string;
  parameters: TSchema;       // TypeBox JSON Schema
  execute: (id, params, signal, onUpdate) => Promise<AgentToolResult>;
}
```

天道工具直接实现这个接口：

```typescript
// tools/issue-command.ts
import { Type } from "@sinclair/typebox";
import type { AgentTool, AgentToolResult } from "@mariozechner/pi-agent-core";

const IssueCommandParams = Type.Object({
  type: Type.Union([
    Type.Literal("spawn_event"),
    Type.Literal("modify_zone"),
    Type.Literal("npc_behavior"),
  ]),
  target: Type.String({ description: "目标区域或NPC ID" }),
  params: Type.Record(Type.String(), Type.Any(), {
    description: "指令参数 (intensity, duration_ticks, spirit_qi_delta 等)"
  }),
  reason: Type.String({ description: "天道意志的理由（用于日志和叙事）" }),
});

export const issueCommandTool: AgentTool<typeof IssueCommandParams> = {
  name: "issue_command",
  label: "下达天道指令",
  description: `向中界(Valence服务端)下达天道指令。
可用指令类型:
- spawn_event: 触发世界事件 (天劫/兽潮/秘境坍塌)
- modify_zone: 修改区域属性 (灵气浓度/危险等级)
- npc_behavior: 调整NPC行为参数 (攻击性/逃跑阈值)

约束:
- intensity 范围 [0.0, 1.0]
- 灵气总量守恒: 增加一处必须减少另一处
- 同一目标 10 分钟内不可重复施加同类指令`,
  parameters: IssueCommandParams,
  async execute(toolCallId, params, signal) {
    // 1. 规则校验
    const validation = validateCommand(params);
    if (!validation.ok) {
      return {
        content: [{ type: "text", text: `指令被天道法则驳回: ${validation.reason}` }],
        details: { blocked: true, reason: validation.reason },
      };
    }
    // 2. 推入 Redis
    await redisClient.publish("bong:agent_command", JSON.stringify({
      v: 1,
      id: `cmd_${Date.now()}`,
      commands: [{ type: params.type, target: params.target, params: params.params }],
    }));
    return {
      content: [{ type: "text", text: `天道指令已下达: ${params.type} → ${params.target}` }],
      details: { sent: true, type: params.type, target: params.target },
    };
  },
};
```

```typescript
// tools/world-query.ts
const WorldQueryParams = Type.Object({
  scope: Type.Union([
    Type.Literal("overview"),      // 全局概览
    Type.Literal("zone"),          // 特定区域详情
    Type.Literal("player"),        // 特定玩家详情
    Type.Literal("balance"),       // 天道平衡态
    Type.Literal("recent_events"), // 近期事件
    Type.Literal("chat_signals"),  // 玩家聊天信号
  ]),
  target: Type.Optional(Type.String({ description: "区域名或玩家ID，scope=overview时不需要" })),
});

export const worldQueryTool: AgentTool<typeof WorldQueryParams> = {
  name: "world_query",
  label: "观测世界",
  description: `观测中界(游戏世界)的当前状态。
- overview: 总览在线玩家数、活跃区域、整体灵气
- zone: 特定区域的灵气、危险等级、活跃事件、驻留玩家
- player: 特定玩家的综合实力、karma、位置、近期行为
- balance: 天道平衡态(Gini系数、实力分布、失衡方向)
- recent_events: 最近N分钟的事件流
- chat_signals: 玩家聊天的情绪/意图信号摘要`,
  parameters: WorldQueryParams,
  async execute(toolCallId, params, signal) {
    const state = worldModel.query(params.scope, params.target);
    return {
      content: [{ type: "text", text: formatWorldState(state) }],
      details: state,
    };
  },
};
```

```typescript
// tools/narrate.ts
const NarrateParams = Type.Object({
  scope: Type.Union([
    Type.Literal("broadcast"),  // 全服广播
    Type.Literal("zone"),       // 区域广播
    Type.Literal("player"),     // 对特定玩家
  ]),
  target: Type.Optional(Type.String()),
  text: Type.String({ description: "叙事文本（中文，修仙风格）" }),
  style: Type.Union([
    Type.Literal("system_warning"),  // 天象预兆
    Type.Literal("perception"),      // 玩家感知
    Type.Literal("narration"),       // 旁白叙事
    Type.Literal("era_decree"),      // 时代宣告
  ]),
});

export const narrateTool: AgentTool<typeof NarrateParams> = {
  name: "narrate",
  label: "天道谕示",
  description: "向世界发布叙事文本。玩家会在聊天栏/HUD看到天道的声音。",
  parameters: NarrateParams,
  async execute(toolCallId, params, signal) {
    await redisClient.publish("bong:agent_narrate", JSON.stringify({
      v: 1,
      narrations: [{
        scope: params.scope,
        target: params.target,
        text: params.text,
        style: params.style,
      }],
    }));
    return {
      content: [{ type: "text", text: `谕示已传达: [${params.style}] ${params.text.slice(0, 50)}...` }],
      details: { scope: params.scope, style: params.style },
    };
  },
};
```

### 2. Skills (渐进式上下文注入) → 天道人格

Pi 的 skill = 一个 `SKILL.md` 文件，按需加载到 system prompt。完美映射天道多 Agent：

```markdown
# skills/calamity/SKILL.md
---
name: calamity
description: 灾劫天道 — 观测因果，降下磨难。针对高karma/高实力玩家施压。
---

你是天道的「劫」之化身。

## 可用工具
- world_query(scope="player"): 查看目标玩家的karma和实力
- world_query(scope="balance"): 查看天道平衡态
- world_query(scope="recent_events"): 查看因果链
- world_query(scope="chat_signals"): 感知民意
- issue_command(type="spawn_event"): 降下灾劫
- narrate(style="system_warning"): 发布天象预兆

## 决策流程
1. 先用 world_query 观测当前态势
2. 判断是否有失衡需要矫正
3. 如果需要行动，先 narrate 预兆（给玩家 30s 反应窗口）
4. 然后 issue_command 执行灾劫
5. 如果不需要行动，直接说明原因并结束

## 约束
- 同一玩家 10 分钟内不可连续遭受天劫
- 新手 (composite_power < 0.2) 不可成为天劫目标
- 每次最多下达 3 条指令
- karma 绝对值 < 0.3 的玩家不触发因果报应
```

### 3. transformContext → 上下文预算管理

Pi 的 `transformContext` hook 正好是我们做上下文裁剪的地方：

```typescript
// 天道 Agent 的 context transform
const tiandaoAgent = new Agent({
  initialState: {
    systemPrompt: baseSystemPrompt,
    tools: [worldQueryTool, issueCommandTool, narrateTool, balanceCheckTool],
    model: sonnetModel,
  },
  // 这里做上下文预算管理
  transformContext: async (messages, signal) => {
    const budget = 6000; // tokens
    let total = estimateTokens(messages);

    if (total <= budget) return messages;

    // 从旧消息开始裁剪，但保留最近 3 轮
    const keep = messages.slice(-6); // 最近 3 轮 (user+assistant)
    const old = messages.slice(0, -6);

    // 压缩旧消息为摘要
    if (old.length > 0) {
      const summary = await summarize(old); // 用 haiku 做摘要
      return [
        { role: "user", content: [{ type: "text", text: `[历史摘要] ${summary}` }], timestamp: Date.now() },
        ...keep,
      ];
    }
    return keep;
  },
});
```

### 4. Events → 监控 & 日志

Pi 的 event 系统直接用于天道行为日志：

```typescript
agent.subscribe(async (event, signal) => {
  switch (event.type) {
    case "tool_execution_end":
      // 记录天道每次行动
      logger.info(`[天道] ${event.toolName}`, event.result);
      // 如果是 issue_command，记录到历史供 peer_decisions 使用
      if (event.toolName === "issue_command") {
        decisionHistory.record(event);
      }
      break;
    case "agent_end":
      // 本轮推演结束
      logger.info(`[天道] 推演完成, messages: ${event.messages.length}`);
      break;
  }
});
```

### 5. Steering & Follow-up → 世界状态注入

Pi 的 `steer()` 和 `followUp()` 完美解决"世界状态实时注入"：

```typescript
// Redis 监听线程：收到新的 world_state 时注入到 agent
redisSubscriber.on("message", (channel, data) => {
  if (channel === "bong:world_state") {
    const state = JSON.parse(data);
    worldModel.update(state);

    // 如果 agent 正在推演，用 steer 注入最新状态
    if (agent.state.isStreaming) {
      agent.steer({
        role: "user",
        content: [{ type: "text", text: `[世界状态更新] ${formatBrief(state)}` }],
        timestamp: Date.now(),
      });
    }
  }

  if (channel === "bong:player_chat") {
    // 玩家聊天作为 follow-up 触发下一轮推演
    const chat = JSON.parse(data);
    if (isHighPriority(chat)) {
      agent.followUp({
        role: "user",
        content: [{ type: "text", text: `[紧急民意] ${chat.player}: ${chat.raw}` }],
        timestamp: Date.now(),
      });
    }
  }
});
```

---

## 四、多 Agent 编排

Pi 的 Agent 类是独立实例。多 Agent = 多个 Agent 实例，各自有不同的 tools/skills/systemPrompt：

```typescript
// 三个天道化身，共享工具集但不同人格
const calamityAgent = new Agent({
  initialState: {
    systemPrompt: loadSkill("calamity"),
    tools: tiandaoTools,
    model: sonnetModel,
  },
  transformContext: calamityContextTransform,
});

const mutationAgent = new Agent({
  initialState: {
    systemPrompt: loadSkill("mutation"),
    tools: tiandaoTools,
    model: sonnetModel,
  },
  transformContext: mutationContextTransform,
});

const eraAgent = new Agent({
  initialState: {
    systemPrompt: loadSkill("era"),
    tools: tiandaoTools,
    model: sonnetModel,
    thinkingLevel: "medium", // 时代推演需要更多思考
  },
  transformContext: eraContextTransform,
});

// 编排主循环
async function tiandaoLoop() {
  while (true) {
    const state = worldModel.getSnapshot();
    const prompt = formatWorldPrompt(state);

    // 并发推演
    await Promise.all([
      calamityAgent.prompt(prompt),
      mutationAgent.prompt(prompt),
      // Era 低频
      shouldEraThink() ? eraAgent.prompt(prompt) : Promise.resolve(),
    ]);

    // 等所有 agent idle
    await Promise.all([
      calamityAgent.waitForIdle(),
      mutationAgent.waitForIdle(),
      eraAgent.waitForIdle(),
    ]);

    // 仲裁（从各 agent 的 tool execution 历史中提取指令）
    const commands = arbiter.merge(collectDecisions());

    await sleep(MIN_INTERVAL);
  }
}
```

---

## 五、通讯协议总结（稳定层）

无论 Agent 内部怎么变，这些是固定的：

### Redis Channels

| Channel | 方向 | 格式 | 频率 |
|---------|------|------|------|
| `bong:world_state` | Server → Agent | WorldState JSON v1 | 10s |
| `bong:player_chat` | Server → Agent | ChatMessage JSON v1 | 实时 (Redis List) |
| `bong:agent_command` | Agent → Server | AgentCommand JSON v1 | 按需 |
| `bong:agent_narrate` | Agent → Server | Narration JSON v1 | 按需 |

### 消息 Schema (Pydantic / TypeBox 双端定义)

```typescript
// TypeBox (Agent 侧, TypeScript)
const WorldStateV1 = Type.Object({
  v: Type.Literal(1),
  ts: Type.Number(),
  tick: Type.Number(),
  players: Type.Array(PlayerProfile),
  npcs: Type.Array(NpcState),
  zones: Type.Array(ZoneState),
  recent_events: Type.Array(GameEvent),
});

const AgentCommandV1 = Type.Object({
  v: Type.Literal(1),
  id: Type.String(),
  commands: Type.Array(Command),
});

const NarrationV1 = Type.Object({
  v: Type.Literal(1),
  narrations: Type.Array(Narration),
});

const ChatMessageV1 = Type.Object({
  v: Type.Literal(1),
  ts: Type.Number(),
  player: Type.String(),
  raw: Type.String(),
  zone: Type.String(),
});
```

```rust
// serde (Server 侧, Rust) — 与上面 1:1 对应
#[derive(Serialize, Deserialize)]
struct WorldStateV1 {
    v: u8,
    ts: u64,
    tick: u64,
    players: Vec<PlayerProfile>,
    npcs: Vec<NpcState>,
    zones: Vec<ZoneState>,
    recent_events: Vec<GameEvent>,
}

#[derive(Serialize, Deserialize)]
struct AgentCommandV1 {
    v: u8,
    id: String,
    commands: Vec<Command>,
}
```

### 指令类型枚举（两端共享）

```
spawn_event     — 触发事件 (thunder_tribulation, beast_tide, realm_collapse, karma_backlash)
modify_zone     — 修改区域 (spirit_qi_delta, danger_level_delta)
npc_behavior    — 调整NPC (aggression, flee_threshold, patrol_radius)
```

### 约束常量（两端硬编码）

```
INTENSITY_MIN = 0.0
INTENSITY_MAX = 1.0
SPIRIT_QI_TOTAL = 100.0          # 全服灵气守恒总量
MAX_COMMANDS_PER_TICK = 5
COOLDOWN_SAME_TARGET_MS = 600000 # 同一目标 10 分钟冷却
NEWBIE_POWER_THRESHOLD = 0.2     # 低于此值的玩家不可被天劫
MAX_NARRATION_LENGTH = 500       # 单条叙事文本最大字符数
```

---

## 六、Fork 步骤

```bash
# 1. Fork
gh repo fork badlogic/pi-mono --clone --remote

# 2. 删除不需要的 packages
rm -rf packages/web-ui packages/mom packages/pods

# 3. 重命名 coding-agent → tiandao-agent
mv packages/coding-agent packages/tiandao-agent

# 4. 清理 coding tools
rm packages/tiandao-agent/src/core/tools/{bash,edit,edit-diff,write,read,grep,find,ls}.ts
rm packages/tiandao-agent/src/core/bash-executor.ts

# 5. 创建天道工具
# packages/tiandao-agent/src/core/tools/
#   redis-bridge.ts, world-query.ts, issue-command.ts, narrate.ts, balance-check.ts

# 6. 创建天道 skills
# .pi/skills/calamity/SKILL.md, mutation/SKILL.md, era/SKILL.md

# 7. 修改 system prompt
# packages/tiandao-agent/src/core/defaults.ts

# 8. 添加 Redis 依赖
# npm install ioredis

# 9. 修改入口
# packages/tiandao-agent/src/core/agent-session.ts — 改为天道循环模式

# 10. 构建验证
npm install && npm run build
```
