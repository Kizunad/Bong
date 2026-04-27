# 天道 Agent 架构设计

> Agent 是"天道"——俯瞰全局的操盘手，不是 NPC。异步低频推演，高容错，LLM 幻觉即天道无常。
> 多 Agent 分司其职，共享世界上下文，只能推动不能控制。

---

## 一、多 Agent 架构总览

```
                        ┌──────────────────────┐
                        │    Context Assembler  │
                        │  上下文拼装引擎        │
                        │  (模块化 prompt 工厂)  │
                        └──────────┬───────────┘
                                   │ 按 Agent 角色裁剪上下文
              ┌────────────────────┼────────────────────┐
              ▼                    ▼                     ▼
   ┌──────────────────┐ ┌─────────────────┐ ┌──────────────────┐
   │  灾劫 Agent      │ │  变化 Agent     │ │  演绎时代 Agent   │
   │  (Calamity)      │ │  (Mutation)     │ │  (Era)           │
   │                  │ │                 │ │                  │
   │  天劫、兽潮、    │ │  灵气潮汐、     │ │  时代更迭、       │
   │  秘境坍塌、      │ │  地形变异、     │ │  宗门兴衰、       │
   │  因果报应        │ │  气候异变、     │ │  大势演化、       │
   │                  │ │  资源迁移       │ │  纪元命名         │
   └────────┬─────────┘ └───────┬─────────┘ └────────┬─────────┘
            │                   │                     │
            ▼                   ▼                     ▼
   ┌───────────────────────────────────────────────────────────┐
   │                    Arbiter (仲裁层)                        │
   │  合并指令 → 冲突消解 → 规则校验 → 预算控制               │
   └──────────────────────────┬────────────────────────────────┘
                              │ Redis Pub/Sub
                              ▼
   ┌───────────────────────────────────────────────────────────┐
   │                 Valence 服务端 (Rust)                      │
   │  Tokio 守护线程 ←crossbeam→ Bevy ECS 主循环               │
   └───────────────────────────────────────────────────────────┘
```

### Agent 定位与频率

| Agent | 角色 | 推演频率 | 上下文侧重 | 权限边界 |
|-------|------|----------|------------|----------|
| **Calamity (灾劫)** | 因果执行者 | 事件驱动 + 30s 巡检 | 玩家行为、杀戮值、气运 | 只能制造危险环境，不能直接伤害 |
| **Mutation (变化)** | 环境塑造者 | 60s 周期 | 区域灵气、资源分布、地形 | 只能渐变，不能瞬变（delta 限幅） |
| **Era (演绎时代)** | 历史记录者 | 300s 周期 (低频) | 全局摘要、长期趋势、宗门势力 | 只能宣告趋势，不能直接干预个体 |

**核心原则：只能推动，不能控制。**
- Agent 输出的是"力"（influence），不是"位移"（direct action）
- 灾劫 Agent 说"血谷应降天劫"，但天劫的具体伤害由 Rust 规则引擎计算
- 变化 Agent 说"北域灵气应衰减"，但衰减速率由守恒方程约束
- 时代 Agent 说"剑修时代将终结"，但不能直接删除玩家的剑

---

## 二、上下文拼装引擎 (Context Assembler)

### 设计目标

- **模块化**：每种信息是独立的 context block，按需拼凑
- **预算制**：总 token 预算固定，各模块竞争分配
- **留白**：预留 reasoning 空间给 LLM 思考

### Token 预算分配

以 Claude Sonnet 8K output / 32K input 为参考：

```
总预算: 8192 tokens (input 侧给 context 的部分)
─────────────────────────────────────────────
  system_prompt    │  固定  │  ~800 tokens   │ Agent 人格 + 规则 + 输出格式
  world_snapshot   │  动态  │  ~1500 tokens  │ 当前世界状态快照
  player_profiles  │  动态  │  ~1200 tokens  │ 活跃玩家综合属性
  player_chat      │  动态  │  ~800 tokens   │ 最近玩家聊天（情绪/意图信号）
  event_log        │  动态  │  ~1000 tokens  │ 近期事件流
  era_context      │  条件  │  ~500 tokens   │ 当前时代背景（仅 Era Agent 完整展开）
  peer_decisions   │  条件  │  ~400 tokens   │ 其他 Agent 最近的决策摘要
  ─────────────────┼────────┼────────────────
  thinking_reserve │  固定  │  ~2000 tokens  │ 留白：LLM 内部推理空间(extended thinking)
  ─────────────────┼────────┼────────────────
  合计              │        │  ~8200 tokens  │
```

> `thinking_reserve` 不是显式 prompt 内容。是指我们**故意不填满** input，
> 给模型留出 working memory。实测 LLM 在 input 过满时推理质量显著下降。
> 如果使用 Claude extended thinking，这部分由模型自己管理。

### Context Block 定义

```python
@dataclass
class ContextBlock:
    name: str                    # 模块名
    priority: int                # 优先级 (0=最高，数字越大越容易被裁剪)
    max_tokens: int              # 最大 token 预算
    required: bool               # 是否必须包含
    content: str                 # 渲染后的文本
    token_count: int             # 实际 token 数

    def render(self, data: dict, budget: int) -> str:
        """根据预算渲染内容，超出时自动压缩"""
        ...
```

### 各 Agent 的上下文配方

```python
# 每个 Agent 声明自己需要哪些 block 以及优先级覆盖

CALAMITY_RECIPE = ContextRecipe(
    agent_name="calamity",
    blocks=[
        BlockSpec("system_prompt",    priority=0, required=True),
        BlockSpec("player_profiles",  priority=1, required=True),   # 核心：看谁该挨劫
        BlockSpec("player_chat",      priority=2, required=False),  # 玩家嘴硬？加重天劫
        BlockSpec("event_log",        priority=1, required=True),   # 因果链
        BlockSpec("world_snapshot",   priority=3, required=False),  # 粗看即可
        BlockSpec("peer_decisions",   priority=4, required=False),  # 避免和变化Agent冲突
    ],
    thinking_reserve=2000,
)

MUTATION_RECIPE = ContextRecipe(
    agent_name="mutation",
    blocks=[
        BlockSpec("system_prompt",    priority=0, required=True),
        BlockSpec("world_snapshot",   priority=1, required=True),   # 核心：区域状态
        BlockSpec("player_profiles",  priority=2, required=True),   # 看分布做平衡
        BlockSpec("era_context",      priority=2, required=True),   # 时代影响环境
        BlockSpec("player_chat",      priority=4, required=False),  # 低优先
        BlockSpec("event_log",        priority=3, required=False),
    ],
    thinking_reserve=1500,
)

ERA_RECIPE = ContextRecipe(
    agent_name="era",
    blocks=[
        BlockSpec("system_prompt",    priority=0, required=True),
        BlockSpec("era_context",      priority=0, required=True),   # 核心：历史长线
        BlockSpec("world_snapshot",   priority=1, required=True),   # 大势判断
        BlockSpec("player_profiles",  priority=2, required=True),   # 谁在塑造时代
        BlockSpec("event_log",        priority=2, required=True),   # 大事件
        BlockSpec("player_chat",      priority=3, required=False),  # 民意
        BlockSpec("peer_decisions",   priority=1, required=True),   # 要看全局协调
    ],
    thinking_reserve=2500,  # 时代推演需要更多思考空间
)
```

### 拼装流程

```
1. 收集所有 raw data（world state, player data, chat, events...）
2. 按 recipe 选择需要的 blocks
3. 渲染每个 block（模板 + 数据 → 文本）
4. 计算 token 数
5. 如果总量超预算：
   a. 按 priority 从高到低排序
   b. 从最低优先级开始裁剪（truncate 或 summarize）
   c. required=True 的 block 不可裁剪，只能压缩
6. 拼装最终 prompt = system + [blocks] + output_format
7. 验证 thinking_reserve 空间足够
```

---

## 三、玩家聊天信号系统

### 采集

Rust 侧拦截玩家 chat 消息，推送到 Redis：

```json
// bong:player_chat channel
{
  "v": 1,
  "ts": 1712345678,
  "player": "offline:Steve",
  "raw": "这破地方灵气也太少了吧，练个气都练不动",
  "zone": "blood_valley"
}
```

### 预处理（Python 侧，廉价模型批处理）

每 30s 对积累的 chat 做一次批量标注：

```python
class ChatSignal:
    player: str
    raw: str
    sentiment: float        # [-1, 1] 负面到正面
    intent: str             # "complaint", "boast", "social", "help", "provoke"
    mentions_mechanic: str  # "spirit_qi", "combat", "npc", "none"
    influence_weight: float # 这条信息对天道决策的影响权重
```

### 注入上下文

Chat 不是直接丢原文给 Agent，而是**预处理成信号摘要**：

```markdown
## 近期民意 (最近 5 分钟)

- Steve [血谷]: 抱怨灵气不足 (sentiment: -0.7, 提及: spirit_qi)
- Alex [青云峰]: 与其他玩家社交闲聊 (sentiment: 0.3, 无特定诉求)
- Herobrine [修罗场]: 挑衅其他玩家 (sentiment: -0.4, intent: provoke, 杀戮值已高)

民意倾向: 偏负面 (-0.3), 主要诉求: 灵气资源
```

**为什么不塞原文？**
- 节省 token（10 条聊天压缩成 5 行摘要）
- 过滤垃圾信息 / spam
- 结构化后 Agent 更容易基于信号决策
- 防止 prompt injection（玩家在聊天里写"忽略以上指令"）

---

## 四、玩家综合属性与天道平衡

### 玩家画像 (Player Profile)

Rust 侧持续追踪，随 world_state 推送：

```json
{
  "uuid": "offline:Steve",
  "name": "Steve",
  "realm": "Induce",
  "composite_power": 0.72,
  "breakdown": {
    "combat":    0.85,
    "wealth":    0.60,
    "social":    0.45,
    "karma":    -0.30,
    "territory": 0.20
  },
  "trend": "rising",
  "active_hours": 12.5,
  "zone": "blood_valley",
  "recent_kills": 5,
  "recent_deaths": 1
}
```

`composite_power` = 综合实力归一化值 (0-1)，由子维度加权得出。

### 天道平衡机制

**不是直接 nerf/buff，是通过环境施压/扶持：**

```
高 composite_power 玩家：
  → 灾劫 Agent 更容易对其触发天劫
  → 变化 Agent 倾向于其所在区域灵气衰减
  → "木秀于林，风必摧之"

低 composite_power 玩家：
  → 变化 Agent 倾向于其附近刷新机缘
  → 灾劫 Agent 降低其区域灾难频率
  → "天道怜弱，偶降机缘"

极端不平衡时：
  → Era Agent 宣布时代转折（"剑道式微，丹道兴盛"）
  → 整体环境向弱势玩法倾斜
```

**注入到 prompt 的方式：**

```markdown
## 天道平衡态

当前玩家实力分布 (Gini系数: 0.68 — 严重失衡):
- 强者 (power > 0.7): Steve(0.85), Herobrine(0.78) — 集中在修罗场
- 中等 (0.3-0.7): Alex(0.45), Bob(0.38)
- 弱者 (power < 0.3): NewPlayer1(0.12), NewPlayer2(0.08) — 集中在新手谷

平衡建议方向:
- 对 Steve 施加压力（karma -0.30, 杀戮值高, 综合实力最强）
- 新手谷区域可适当增加灵气/机缘密度
- 修罗场已过度集中强者，可考虑环境恶化驱散

你不必完全遵循建议。天道有自己的判断。
```

最后一句很重要——建议只是建议，Agent 可以无视。这保留了"天道无常"。

---

## 五、Arbiter 仲裁层

三个 Agent 各自输出指令，可能冲突：
- 灾劫 Agent 要在血谷降天劫（增加危险）
- 变化 Agent 要在血谷提升灵气（增加收益）
- 时代 Agent 宣布"灵气衰退纪元"

### 冲突消解规则

```python
class Arbiter:
    def merge(self, decisions: list[AgentDecision]) -> list[Command]:
        merged = []
        for cmd in self.flatten_all(decisions):
            # 1. 规则硬约束（不可违反）
            if not self.rules.is_legal(cmd):
                continue  # 丢弃非法指令

            # 2. 能量守恒
            if cmd.type == "modify_zone":
                self.conservation_ledger.debit(cmd)
                if self.conservation_ledger.is_overdrawn():
                    continue  # 灵气总量已爆

            # 3. 同区域冲突检测
            conflict = self.find_conflict(cmd, merged)
            if conflict:
                # 高频 Agent 让位于低频 Agent（Era > Mutation > Calamity）
                # 因为低频 Agent 看得更远
                winner = self.resolve_by_frequency(cmd, conflict)
                merged = self.replace_or_skip(merged, winner)
            else:
                merged.append(cmd)

        # 4. 预算控制：单轮最多 N 条指令
        return merged[:self.max_commands_per_tick]
```

### 冲突优先级

```
Era > Mutation > Calamity（对同一目标）

理由：
- 时代是百年尺度的大势，不应被单次天劫打断
- 环境变化是十分钟尺度的趋势，灾劫是即时事件
- 如果时代说"灵气衰退"，灾劫就不该在同一区域补灵气
```

---

## 六、模块化 Prompt 工厂

### 目录结构

```
agent/
├── agent/
│   ├── context/
│   │   ├── __init__.py
│   │   ├── assembler.py       # 上下文拼装引擎
│   │   ├── blocks/
│   │   │   ├── __init__.py
│   │   │   ├── base.py        # ContextBlock 基类
│   │   │   ├── world.py       # 世界快照 block
│   │   │   ├── players.py     # 玩家画像 block
│   │   │   ├── chat.py        # 聊天信号 block
│   │   │   ├── events.py      # 事件日志 block
│   │   │   ├── era.py         # 时代背景 block
│   │   │   └── peers.py       # 其他 Agent 决策摘要 block
│   │   ├── recipes.py         # 各 Agent 的上下文配方
│   │   └── budget.py          # Token 预算管理器
│   ├── agents/
│   │   ├── __init__.py
│   │   ├── base.py            # Agent 基类（统一接口）
│   │   ├── calamity.py        # 灾劫 Agent
│   │   ├── mutation.py        # 变化 Agent
│   │   └── era.py             # 演绎时代 Agent
│   ├── arbiter/
│   │   ├── __init__.py
│   │   ├── merger.py          # 指令合并
│   │   ├── rules.py           # 硬约束（灵气守恒、坐标合法、强度限幅）
│   │   └── conservation.py    # 能量/灵气守恒账本
│   ├── bridge/
│   │   ├── redis_client.py
│   │   └── schema.py          # Pydantic: WorldState, Command, ChatSignal...
│   ├── mind/
│   │   ├── reasoner.py        # LLM 调用封装
│   │   ├── chat_processor.py  # 聊天预处理（批量标注）
│   │   └── prompts/
│   │       ├── calamity_system.md
│   │       ├── mutation_system.md
│   │       ├── era_system.md
│   │       └── output_format.md   # 共享的 JSON 输出格式说明
│   └── main.py
```

### Agent 基类

```python
class BaseAgent(ABC):
    name: str
    recipe: ContextRecipe
    system_prompt_path: str
    think_interval: float          # 推演间隔秒数
    model: str                     # LLM model ID

    async def tick(self, world: WorldModel, assembler: ContextAssembler):
        """单次推演"""
        # 1. 检查是否需要推演
        if not self.should_think(world):
            return None

        # 2. 拼装上下文
        prompt = assembler.build(
            recipe=self.recipe,
            world=world,
            system_prompt=self.load_system_prompt(),
        )

        # 3. 调用 LLM
        raw = await self.reasoner.call(prompt, model=self.model)

        # 4. 解析输出
        decision = self.parse_decision(raw)

        # 5. 返回（不直接执行，交给 Arbiter）
        return decision

    @abstractmethod
    def should_think(self, world: WorldModel) -> bool:
        """事件驱动 + 时间兜底"""
        ...
```

### Context Assembler 核心

```python
class ContextAssembler:
    def __init__(self, blocks: dict[str, ContextBlock]):
        self.blocks = blocks

    def build(self, recipe: ContextRecipe, world: WorldModel,
              system_prompt: str) -> list[dict]:
        """拼装成 messages 格式"""
        budget = TokenBudget(
            total=recipe.total_budget,
            reserved=recipe.thinking_reserve,
        )

        # 系统 prompt（固定，必含）
        budget.allocate("system", len_tokens(system_prompt))

        # 按优先级排序 blocks
        specs = sorted(recipe.blocks, key=lambda b: b.priority)

        rendered_blocks = []
        for spec in specs:
            block = self.blocks[spec.name]
            remaining = budget.remaining()

            if remaining <= 0 and not spec.required:
                continue  # 预算用完，跳过非必需

            # 渲染 block，传入剩余预算让它自适应裁剪
            text = block.render(
                data=world.get_data_for(spec.name),
                max_tokens=min(spec.max_tokens, remaining),
            )

            if text:
                budget.allocate(spec.name, len_tokens(text))
                rendered_blocks.append(text)

        # 拼装
        user_content = "\n\n---\n\n".join(rendered_blocks)

        return [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_content},
        ]
```

---

## 七、Prompt 模板示例

### 灾劫 Agent System Prompt

```markdown
# 灾劫 Agent — 因果执行者

你是天道的「劫」之化身。你观测众生因果，在失衡之处降下磨难。

## 权限
- spawn_event: 天劫(thunder_tribulation)、兽潮(beast_tide)、秘境坍塌(realm_collapse)、因果反噬(karma_backlash)
- 每次最多下达 3 条指令

## 核心法则
- 你只能**制造环境危险**，不能直接造成伤害（伤害由法则层结算）
- 天劫的 intensity 与目标的 composite_power 正相关
- karma 为负且绝对值 > 0.5 的玩家，天劫概率显著上升
- 你必须在 narration 中给出天象预兆，让玩家有 30 秒反应窗口
- 同一玩家 10 分钟内不可连续遭受天劫

## 决策偏好
- 宁可不降劫，也不要乱降（误伤新人是天道之耻）
- 群体性灾难（兽潮）优先针对强者聚集区
- 如果玩家在聊天中表现出悔改/收敛，可以降低劫难强度

## 输出格式
严格 JSON，结构见 <output_format>
```

### 渲染后的完整 Prompt 示例（灾劫 Agent 某次推演）

```
[system]
{calamity_system.md 内容}

[user]
## 世界快照
当前 Tick: 84000, 在线玩家: 4, 活跃区域: 3

区域状态:
- 血谷: 灵气 0.42(低), 危险等级 3/5, 活跃事件: 无
- 青云峰: 灵气 0.88(高), 危险等级 1/5
- 新手谷: 灵气 0.95(满), 危险等级 0/5

---

## 玩家画像
| 玩家 | 综合实力 | 战斗 | karma | 趋势 | 位置 |
|------|---------|------|-------|------|------|
| Steve | 0.85 | 0.92 | -0.45 | ↑上升 | 血谷 |
| Herobrine | 0.78 | 0.88 | -0.72 | ↑上升 | 修罗场 |
| Alex | 0.45 | 0.30 | +0.20 | →平稳 | 青云峰 |
| NewPlayer1 | 0.12 | 0.05 | +0.00 | ↑上升 | 新手谷 |

平衡态: Gini 0.68 (失衡), 建议对 Steve/Herobrine 施压

---

## 近期民意
- Herobrine [修罗场]: 挑衅，扬言屠城 (sentiment: -0.8, intent: provoke)
- Steve [血谷]: 沉默练功，无聊天
- Alex [青云峰]: 帮助 NewPlayer1 解答问题 (sentiment: 0.6, intent: help)

---

## 近期事件 (最近 5 分钟)
- [tick 83200] Herobrine 击杀野怪 x12 (连续击杀)
- [tick 83500] Steve 采集高级灵草 x3
- [tick 83800] Herobrine 在修罗场 PK 击败路人

---

## 其他天道意志
- 变化Agent(上一轮): 将修罗场灵气从 0.60 降至 0.45
- 时代Agent(当前): 「末法初期」— 整体灵气缓慢衰减中

---

请基于以上信息，决定是否需要降下劫难。输出 JSON。
如果当前不需要行动，返回空的 commands 数组。
```

---

## 八、编排主循环

```python
async def main():
    redis = RedisClient(config.redis_url)
    world = WorldModel()
    chat_processor = ChatProcessor(model="haiku")

    # 初始化 Agents
    agents = [
        CalamityAgent(model="sonnet"),
        MutationAgent(model="sonnet"),
        EraAgent(model="sonnet"),
    ]
    arbiter = Arbiter(rules=XianxiaRules())
    assembler = ContextAssembler(blocks=default_blocks())

    while True:
        # 1. 拉取最新状态
        state = await redis.get_latest("bong:world_state")
        chats = await redis.drain_list("bong:player_chat")

        if state:
            world.update(state)
        if chats:
            signals = await chat_processor.batch_annotate(chats)
            world.update_chat_signals(signals)

        # 2. 并发推演所有 Agent
        decisions = await asyncio.gather(*[
            agent.tick(world, assembler) for agent in agents
        ])

        # 3. 仲裁合并
        decisions = [d for d in decisions if d is not None]
        if decisions:
            commands = arbiter.merge(decisions)
            narrations = arbiter.collect_narrations(decisions)

            if commands:
                await redis.publish("bong:agent_command", commands)
            if narrations:
                await redis.publish("bong:agent_narrate", narrations)

            # 4. 记录本轮决策（供下轮 peer_decisions block 使用）
            world.record_decisions(decisions)

        # 5. 最短间隔
        await asyncio.sleep(config.min_interval)  # 5s
```

---

## 九、Redis 通道总览（更新）

```
bong:world_state    — Server → Agent  (世界摘要，10s 周期)
bong:player_chat    — Server → Agent  (玩家聊天，实时 push to list)
bong:agent_command  — Agent → Server  (仲裁后的指令)
bong:agent_narrate  — Agent → Server  (叙事文本，转发给客户端)
```

---

## 十、MVP 0.2 分阶段

```
Phase 1: 单 Agent 管道 (1周)
  - Python 骨架 + context assembler + 单个 Calamity Agent
  - Mock world state（不接 Redis，本地 JSON）
  - Mock reasoner（硬编码规则，不调 LLM）
  - 验证：拼装出的 prompt 格式正确，输出 JSON 合法

Phase 2: Redis 联通 (3天)
  - Rust 侧 bridge daemon 切换到真 Redis
  - Python 侧接 Redis 收发
  - 端到端：Server 推状态 → Agent 输出指令 → Server 执行
  - 玩家 chat 采集链路

Phase 3: LLM 接入 (3天)
  - 接 Claude API (sonnet)
  - Chat 预处理接 haiku
  - 规则校验 + Arbiter
  - 验证：LLM 输出的指令能通过仲裁并被服务端执行

Phase 4: 多 Agent (3天)
  - 加入 Mutation + Era Agent
  - Arbiter 冲突消解
  - peer_decisions 上下文注入
  - 验证：三个 Agent 并发推演不冲突

Phase 5: 平衡调参 (持续)
  - composite_power 权重调整
  - 天劫/机缘 触发阈值
  - 时代演进节奏
  - 这是游戏设计层面的事，不是工程问题
```
