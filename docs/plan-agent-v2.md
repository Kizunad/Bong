# Agent 路线详细计划 V2（TypeScript / 天道）

> 从内部闭环推进到生产可用：端到端集成、可观测、可恢复、可扩展。
> 前置：plan-agent.md (A1-A9) 已全部完成，67/67 测试通过。

---

## 当前状态（2026-04-10）

| 模块 | 状态 | 说明 |
|------|------|------|
| Arbiter 仲裁层 | ✅ 完成 | 冲突消解、灵气守恒、预算控制、时代检测 |
| Chat 信号预处理 | ✅ 完成 | batch annotate、drain list、sentiment/intent 标注 |
| 循环模式稳定化 | ✅ 完成 | timeout、backoff、graceful shutdown |
| Peer Decisions | ✅ 完成 | 上一轮决策摘要注入 context |
| 世界模型时序记忆 | ✅ 完成 | zone 历史、trend 计算 |
| 平衡算法 | ✅ 完成 | Gini 系数、balance block |
| 叙事质量 | ✅ 完成 | 半文言半白话、预兆、纯 JSON |
| 个体关注 | ✅ 完成 | key player 识别、因果将至/天道可扶 |
| 时代实质化 | ✅ 完成 | era 命令 + narration 双路径检测、globalEffect |

**未覆盖的关键缺口**：
- Agent 从未与真实 Server 端到端联调
- 无结构化指标，生产环境下无法观测
- WorldModel 全在内存，重启即丢
- 所有 agent/task 共用同一模型
- `tools/` 目录预留但为空
- 无自动化 E2E smoke test

---

## B1. 端到端 Redis 集成验证

**目标**：Agent 发出的 command/narration 能被 Rust server 正确接收并执行；Server 发布的 world_state 能被 Agent 正确消费。

**前置**：Server 侧 `redis_bridge.rs` + `command_executor.rs` 已有骨架。

### 验证链路

```
Server (Rust)                    Redis                     Agent (TS)
─────────────                    ─────                     ──────────
world.rs 采集状态 ──PUBLISH──→ bong:world_state ──SUB──→ redis-ipc.ts
                                                           ↓
                                                      3 agent tick
                                                           ↓
command_executor.rs ←──SUB── bong:agent_command ←──PUB── arbiter merge
narration 转发      ←──SUB── bong:agent_narrate ←──PUB── narrations
                                                           ↑
chat_collector.rs ──RPUSH──→ bong:player_chat ──DRAIN──→ chat-processor.ts
```

### 具体任务

```
B1.1  Schema 对齐验证
      - 用 agent/packages/schema/samples/*.json 同时跑 TS vitest 和 Rust #[test]
      - 确认 AgentCommandV1、NarrationV1、WorldStateV1 双端 parse 一致
      - 重点：Command.params 是 Record<string, any>，Rust 侧用 serde_json::Value

B1.2  Server command_executor 实装
      - spawn_event → 根据 event 类型：
        - thunder_tribulation: 在 target zone 内 spawn 闪电实体 + 伤害区域
        - beast_tide: 批量刷 NPC（复用 npc/spawn.rs）
        - realm_collapse / karma_backlash: 暂用粒子 + 聊天消息占位
      - modify_zone → 修改 Zone component 的 spirit_qi / danger_level
      - npc_behavior → 修改 NPC Thinker 参数（aggression, flee_threshold 等）

B1.3  Server narration 转发
      - 解析 NarrationV1.narrations[]
      - scope=broadcast → 全服 send_chat_message（带 §6/§c/§7 颜色码）
      - scope=zone → 只发给该 zone 内玩家
      - scope=player → 只发给指定 UUID 玩家

B1.4  联调脚本
      - 新增 scripts/e2e-redis.sh：
        1. 启动 Redis (docker run)
        2. cargo run --release (后台)
        3. npx tsx agent/packages/tiandao/src/main.ts (非 mock)
        4. 等待 Agent 完成 1 次 tick
        5. 验证 Redis 中有 agent_command / agent_narrate 消息
        6. 验证 Server 日志中出现 "executing command" 字样
      - CI 中可选运行（需要 Redis service）
```

**验证标准**：
- `scripts/e2e-redis.sh` 全程绿灯
- 玩家在游戏内 30 秒内看到 narration 消息

---

## B2. Agent 可观测性

**目标**：结构化日志 + tick 指标，方便生产环境监控和调试。

### 新增文件：`tiandao/src/telemetry.ts`

```typescript
export interface TickMetrics {
  tick: number;
  timestamp: number;
  durationMs: number;
  agentResults: Array<{
    name: string;
    status: "ok" | "skipped" | "error";
    durationMs: number;
    commandCount: number;
    narrationCount: number;
    tokensEstimated: number;  // 基于 response 字符数粗估
  }>;
  mergedCommandCount: number;
  mergedNarrationCount: number;
  chatSignalCount: number;
  eraChanged: boolean;
}

export interface TelemetrySink {
  recordTick(metrics: TickMetrics): void;
  flush(): void;
}

// 实现 1: 结构化 JSON 日志（stdout，可被 log collector 采集）
export class JsonLogSink implements TelemetrySink { ... }

// 实现 2: 滚动窗口摘要（每 N tick 打印汇总）
export class RollingSummarySink implements TelemetrySink { ... }
```

### 集成点

```
B2.1  runTick 注入 TelemetrySink
      - 每个 agent tick 前后记录时间
      - merge 后记录 mergedCommandCount / mergedNarrationCount
      - 每 tick 结束调用 sink.recordTick()

B2.2  LLM 调用指标
      - llm.ts chat() 返回值扩展为 { content: string, durationMs: number }
      - 或在 agent.ts tick() 内包裹计时

B2.3  滚动摘要
      - 每 10 tick 输出：平均 tick 耗时、总 commands/narrations、LLM 成功率
      - 格式：[tiandao:stats] ticks=10 avg_ms=2340 commands=7 narrations=12 llm_ok=28/30

B2.4  错误分类
      - LlmTimeoutError → 计入 timeout 计数器
      - LlmBackoffError → 计入 backoff 计数器
      - JSON parse 失败 → 计入 parse_fail 计数器
      - 每轮 metrics 中附带 errorBreakdown: { timeout, backoff, parseFail }
```

**验证**：
- 单元测试：mock sink 验证 recordTick 被正确调用
- 手动：mock 模式运行 10 tick，确认 rolling summary 输出

---

## B3. WorldModel 持久化

**目标**：Agent 重启后能恢复时代状态、zone 历史、上轮决策，不从零开始。

### 持久化策略

采用 **Redis hash + 定期快照** 双保险：

```
B3.1  Redis Hash 持久化（实时）
      key: bong:tiandao:state
      fields:
        - current_era     → JSON string of CurrentEra
        - zone_history    → JSON string of Map<string, ZoneSnapshot[]>
        - last_decisions  → JSON string of Map<string, AgentDecision>
        - last_tick       → number

      时机：每轮 tick 结束后写入（pipeline 批量 HSET，一次 RTT）

B3.2  启动恢复
      runtime.ts runRuntime() 启动时：
        1. HGETALL bong:tiandao:state
        2. 如果存在，恢复 worldModel 的 currentEra / zoneHistory / lastDecisions
        3. 日志：[tiandao] restored state from tick N, era: 末法纪

B3.3  文件快照（备份）
      每 100 tick 将完整 WorldModel 序列化为 JSON 写入：
        data/tiandao-snapshot-{tick}.json
      保留最近 5 个快照文件，自动清理旧的
      用途：Redis 丢数据时手动恢复

B3.4  WorldModel 序列化接口
      worldModel.toJSON(): WorldModelSnapshot
      WorldModel.fromJSON(snapshot: WorldModelSnapshot): WorldModel
```

**验证**：
- 单元测试：toJSON → fromJSON 往返一致性
- 集成测试：启动 agent → 跑 3 tick → kill → 重启 → 确认 era 和 zone history 恢复

---

## B4. 多模型路由

**目标**：不同 agent / 不同任务使用不同模型，优化成本和质量。

### 配置

```typescript
// .env 或 runtime config
LLM_MODEL_DEFAULT=gpt-5.4-mini          // 默认（mutation, calamity）
LLM_MODEL_ERA=gpt-5.4                   // era 用更强模型（低频，值得）
LLM_MODEL_ANNOTATE=gpt-5.4-mini         // chat annotation 用最便宜的
```

### 具体任务

```
B4.1  AgentConfig 增加 model override
      interface AgentConfig {
        ...
        model?: string;  // 不填则用 runtime 默认
      }

B4.2  TiandaoAgent.tick() 优先使用 config.model
      const effectiveModel = this.model ?? model;
      await client.chat(effectiveModel, messages);

B4.3  chat-processor 独立 model 配置
      processChatBatch({ ..., model: annotateModel })

B4.4  RuntimeConfig 扩展
      interface RuntimeConfig {
        ...
        modelOverrides: Record<string, string>;
        // { era: "gpt-5.4", annotate: "gpt-5.4-mini" }
      }
      resolveRuntimeConfig 从 env 读取 LLM_MODEL_* 系列变量

B4.5  createDefaultAgents 传入 model override
      new TiandaoAgent({ name: "era", model: config.modelOverrides.era, ... })
```

**验证**：
- 单元测试：mock LLM client 验证不同 agent 调用不同 model 参数
- 手动：日志中确认 `[tiandao][era] model: gpt-5.4` vs `[tiandao][calamity] model: gpt-5.4-mini`

---

## B5. Agent Tools（结构化工具调用）

**目标**：给 LLM 提供可选的 tool_call 能力，让 agent 在推演前主动查询信息。

### 新增目录：`tiandao/src/tools/`

```
B5.1  Tool 接口定义
      tools/types.ts:
        interface AgentTool {
          name: string;
          description: string;
          parameters: JSONSchema;
          execute(params: Record<string, unknown>, ctx: ToolContext): Promise<string>;
        }
        interface ToolContext {
          worldModel: WorldModel;
          latestState: WorldStateV1;
        }

B5.2  内置工具集
      tools/query-player.ts:
        查询指定玩家的详细信息（breakdown、位置、zone、recent_kills/deaths）
        用途：calamity agent 在降劫前确认目标是否有新手保护

      tools/query-zone-history.ts:
        查询指定 zone 的最近 N 轮 spirit_qi 变化
        用途：mutation agent 了解趋势后做更精准的调整

      tools/list-active-events.ts:
        列出所有 zone 中正在进行的事件
        用途：避免在已有天劫的 zone 再降劫

B5.3  LLM client 支持 tool_call
      llm.ts 扩展：
        chat(model, messages, tools?) → 支持 OpenAI function calling 格式
        如果 LLM 返回 tool_call → 自动执行 → 将结果追加到 messages → 再次调用
        最大工具调用轮次：3（防止无限循环）

B5.4  Agent 配置工具集
      AgentConfig 增加 tools?: AgentTool[]
      不同 agent 配置不同工具：
        - calamity: [queryPlayer, listActiveEvents]
        - mutation: [queryZoneHistory]
        - era: []（时代不需要工具，纵观全局即可）

B5.5  Skill prompt 更新
      在 skills/*.md 中说明可用工具及使用时机
      强调：工具是可选的，不必每次都调用
```

**验证**：
- 单元测试：mock tool execution → 验证 messages 被正确拼接
- 单元测试：工具调用超过 3 轮 → 自动截断
- mock 模式测试：开启 tools 后 agent 仍然输出合法 JSON

---

## B6. E2E Smoke Test

**目标**：一键验证 Server + Agent + Redis 完整链路。

### 新增文件：`scripts/smoke-test-e2e.sh`

```
B6.1  Docker Compose 环境
      docker-compose.test.yml:
        services:
          redis:
            image: redis:7-alpine
            ports: ["6379:6379"]

B6.2  脚本流程
      1. docker compose -f docker-compose.test.yml up -d redis
      2. cargo build --release (server)
      3. npm run build -w @bong/schema && npm run build -w tiandao
      4. 后台启动 server: ./target/release/bong-server &
      5. 等待 server 发布第一个 world_state（redis-cli SUBSCRIBE 超时 10s）
      6. 启动 agent（非 mock，maxLoopIterations=3）
      7. 等待 agent 完成 3 tick
      8. 验证断言：
         a. Redis 中 bong:agent_command 至少收到 1 条消息
         b. Redis 中 bong:agent_narrate 至少收到 1 条消息
         c. Server 日志包含 "executing command"
         d. Agent 日志包含 "tick end"
         e. Agent 退出码 0
      9. 清理：kill server, docker compose down

B6.3  CI 集成
      GitHub Actions workflow: .github/workflows/e2e.yml
      触发条件：push to main, PR 改动 agent/** 或 server/**
      服务：redis service container
      步骤：cargo build → npm ci → npm test → bash scripts/smoke-test-e2e.sh
```

**验证标准**：
- `bash scripts/smoke-test-e2e.sh` 在本地 WSL 中全程绿灯
- CI 中自动执行（允许 server 编译缓存）

---

## B7. Narration 质量评估回路

**目标**：自动检测 narration 文本质量，辅助 prompt 迭代。

### 新增文件：`tiandao/src/narration-eval.ts`

```
B7.1  规则评分器（无 LLM 依赖）
      function scoreNarration(text: string, style: NarrationStyle): NarrationScore {
        return {
          lengthOk: text.length >= 100 && text.length <= 200,    // 字符数合规
          hasOmen: /预兆|先兆|暗示|将|欲|渐/.test(text),        // 包含预兆性词汇
          noModernSlang: !/OK|哈哈|666|牛|服了/.test(text),     // 无现代网络用语
          styleMatch: checkStyleKeywords(text, style),            // 风格关键词匹配
          score: 0,  // 综合 0-1
        };
      }

B7.2  LLM 评审（可选，高成本）
      用独立 LLM 调用评估 narration 质量：
      prompt: "请为以下修仙世界叙事评分(0-10)，评估维度：文学性、信息量、风格一致性、预兆暗示。"
      仅在 debug 模式或手动触发时启用

B7.3  集成到 runTick
      merge 后对每条 narration 调用 scoreNarration()
      结果写入 TickMetrics.narrationScores
      低于阈值（score < 0.5）的 narration 在日志中标记 ⚠️

B7.4  评估报告
      npm run eval-narrations：
        - 读取最近 50 条 narration 日志
        - 输出质量分布直方图（终端 ASCII）
        - 标记反复出现的问题模式
```

**验证**：
- 单元测试：好/差 narration 样本的评分符合预期
- 手动：跑 10 tick mock，确认评分输出

---

## 文件规划总览

```
agent/packages/tiandao/src/
├── main.ts              # 入口（已有）
├── agent.ts             # TiandaoAgent（改：model override）
├── arbiter.ts           # 仲裁层（已有）
├── balance.ts           # Gini + 平衡（已有）
├── chat-processor.ts    # 聊天标注（已有）
├── context.ts           # Context Assembler（已有）
├── llm.ts               # LLM client（改：tool_call 支持）
├── mock-state.ts        # 测试假数据（已有）
├── narration-eval.ts    # narration 质量评估（新增 B7）
├── parse.ts             # LLM 输出解析（已有）
├── redis-ipc.ts         # Redis IPC（改：state 持久化）
├── runtime.ts           # 运行时（改：telemetry + restore）
├── telemetry.ts         # 可观测性（新增 B2）
├── world-model.ts       # 世界模型（改：序列化接口）
├── tools/
│   ├── types.ts         # Tool 接口定义（新增 B5）
│   ├── query-player.ts  # 查询玩家（新增 B5）
│   ├── query-zone-history.ts  # 查询 zone 历史（新增 B5）
│   └── list-active-events.ts  # 列出活跃事件（新增 B5）
└── skills/
    ├── calamity.md      # （改：工具说明）
    ├── mutation.md      # （改：工具说明）
    └── era.md           # （已完成）

scripts/
├── smoke-test-e2e.sh   # E2E 集成测试（新增 B6）
└── docker-compose.test.yml  # 测试环境（新增 B6）
```

---

## 开发顺序建议

```
第一波（基础设施，互不依赖，可并行）：
  B2 可观测性（独立模块，不影响逻辑）
  B4 多模型路由（改动小，立即有成本收益）

第二波（需要 Redis 可用）：
  B3 WorldModel 持久化（依赖 Redis 连接）
  B1 端到端集成验证（依赖 Server 侧配合）

第三波（依赖前两波稳定）：
  B6 E2E Smoke Test（依赖 B1 联调完成）
  B5 Agent Tools（功能增强，非阻塞）
  B7 Narration 评估（锦上添花，最后做）
```

```
时间线估算（相对优先级）：
  B2 ████░░░░░░  必须 — 没有指标等于盲飞
  B4 ███░░░░░░░  必须 — 成本直接减半
  B3 █████░░░░░  必须 — 生产环境基本要求
  B1 ████████░░  必须 — 整个项目的里程碑验证点
  B6 ██████░░░░  强烈推荐 — CI 守护网
  B5 ████████░░  推荐 — 提升 agent 决策质量
  B7 ███░░░░░░░  可选 — prompt 迭代加速器
```

---

## 测试策略

- **单元测试**（vitest）：telemetry sink、WorldModel 序列化往返、narration 评分、tool execution
- **集成测试**（需 Redis）：state 持久化写入/恢复、drain + publish 往返
- **E2E 测试**（smoke-test-e2e.sh）：Server + Agent + Redis 三进程完整链路
- **回归守护**：现有 67 个测试必须始终通过，新增测试不得破坏已有契约
