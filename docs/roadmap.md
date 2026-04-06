# Bong 总体路线图

> 从 MVP 0.1 到可体验的修仙沙盒闭环。
> 三路并行开发：Server（Rust）、Agent（TypeScript）、Client（Java/Fabric）。

---

## 当前状态（2026-04-06）

| 层 | 完成度 | 已实现 |
|----|--------|--------|
| Server | MVP 0.1 ✅ | 草地平台、玩家连接、僵尸 NPC（big-brain 逃跑 AI）、mock bridge、Redis IPC 发布 world_state |
| Agent | 骨架 ✅ | 3 Agent 并发推演（灾劫/变化/演绎时代）、Context Assembler、LLM 调用、Redis 订阅/发布 |
| Client | MVP 0.1 ✅ | Fabric 微端、CustomPayload 接收、HUD 渲染 |
| Schema | 完成 ✅ | TypeBox (TS) + serde (Rust) 双端对齐、共享 sample JSON 校验 |

---

## 里程碑定义

### M1 — 天道闭环（Agent 指令能在游戏内可见）

**目标**：玩家进入游戏，天道 Agent 的决策能以**可感知的形式**作用于世界。

#### Server 路线
1. **指令执行器**：解析 `AgentCommandV1`，实际执行 `spawn_event`（生成闪电/粒子标记）、`modify_zone`（调整区块属性）、`npc_behavior`（修改 NPC Thinker 参数）
2. **世界状态采集丰富化**：world_state 包含真实玩家数据（位置、名字、在线时长）、NPC 状态、区域划分
3. **玩家 chat 采集**：拦截聊天消息，RPUSH 到 `bong:player_chat`
4. **Narration 转发优化**：按 scope 精准下发（broadcast/zone/player），带 MC 格式化颜色码

#### Agent 路线
5. **Arbiter 仲裁层**：合并三 Agent 输出、冲突消解（Era > Mutation > Calamity）、灵气守恒校验、每轮最大指令数限制
6. **Chat 信号预处理**：batch annotate 玩家聊天（sentiment + intent），注入 context block
7. **循环模式稳定化**：Agent 持续运行，interval 控制、错误重试、graceful shutdown
8. **peer_decisions 上下文**：各 Agent 能看到其他 Agent 上一轮的决策摘要

#### Client 路线
9. **Narration 展示**：解析 `bong:agent_narrate` 包，按 style 渲染（§c 警示 / §6 时代宣言 / §7 感知）— 在聊天栏 + 可选 HUD toast
10. **天象视觉反馈**（可选）：天劫时屏幕闪烁/粒子雨、灵气变化时环境色调微调

**验证标准**：启动 server + agent + client，玩家在游戏内行走，30 秒内聊天栏出现天道的 narration 消息（如"血谷上空乌云翻涌…"），同时 server 日志显示 agent command 被执行。

---

### M2 — 有意义的世界（区域 + 地形 + 多 NPC）

**目标**：从测试草地变成一个有区域划分、地形特征、多个 NPC 的初步修仙世界。

#### Server 路线
1. **Anvil 地形加载**：用 WorldPainter 制作 3-5 个区域的预生成地图（新手谷、血谷、青云峰等），通过 `valence_anvil` 加载
2. **区域系统**：Zone component 附加到 chunk 组，每个 zone 有 spirit_qi / danger_level / 名称，作为 world_state 的真实数据源
3. **多 NPC 种类**：不同外观（zombie/skeleton/villager）、不同 AI 行为（巡逻、守卫、商人），agent 可通过 `npc_behavior` 指令动态调整
4. **pathfinding 集成**：NPC 移动使用 A* 寻路而非直线移动
5. **事件系统**：`spawn_event` 指令的具体实现 — 天劫（闪电 + 伤害区域）、��潮（批量刷怪）、灵气涌泉（buff 区域）

#### Agent 路线
6. **世界模型增强**：Agent 维护 zone 状态的时序记忆（最近 N 轮的 spirit_qi 变化趋势），注入 era_context block
7. **平衡算法**：composite_power 计算 + Gini 系数追踪，注入天道平衡态 context block

#### Client 路线
8. **区域 HUD**：进入不同区域时显示区域名 + 灵气浓度指示
9. **CustomPayload 路由**：区分不同类型的 server_data（narration / zone_info / event_alert），分别处理

**验证标准**：玩家在不同区域间移动，agent 根据区域特征和玩家位置产生不同的决策；NPC 在区域内巡逻而非原地站立。

---

### M3 — 可玩的修仙体验

**目标**：玩家有基础的修仙进度感 + 天道带来的涌现式叙事。

#### Server 路线
1. **玩家状态持久化**：realm（境界）、spirit_qi 储量、karma 值，存储到文件/SQLite，断线重连不丢失
2. **基础战斗系统**：攻击 NPC 获得经验/资源，被天劫命中扣血，死亡惩罚
3. **采集系统**：特定方块可采集灵草/矿石，不同区域产出不同
4. **境界突破**：积累足够资源 + 条件（karma 值、在线时长等）可突破到下一境界

#### Agent 路线
5. **叙事质量提升**：narration 文本要有文学性、仪式感，prompt 工程优化
6. **个体关注**：Agent 能识别"关键玩家"（power 最高/最低、karma 极端），产生针对性叙事
7. **时代演进实质化**：Era Agent 的宣言不仅是文字，还伴随全局 modify_zone 指令（整体灵气升降）

#### Client 路线
8. **修仙 UI 面板**：用 owo-ui 渲染境界、真元池、karma 显示
9. **动态 UI 下发**（可选）：server 通过 CustomPayload 下发 UI 布局 XML，client 动态渲染

**验证标准**：新玩家进入 → 从凡人开始 → 采集、战斗积累 → 天道根据表现降下机缘或劫难 → 突破境界 → 天道叙事伴随全程。5 分钟内能感受到"修仙世界在运转"。

---

### M4 — 多人社交与宗门（远期）

- 宗门系统：创建/加入宗门，宗门气运值
- 玩家交易/合作
- Agent 识别宗门势力格局，产生宗门级别的叙事（"青云宗气运正盛…"）
- PvP 区域 + karma 系统深化
- 热更新微端（Bootstrapper 自动下载资源包）

---

## 三路并行分工建议

开发可以按层独立推进，Redis IPC 是胶水：

```
        Server (Rust)              Agent (TypeScript)          Client (Java)
        ─────────────              ──────────────────          ─────────────
M1      指令执行器                  Arbiter 仲裁层              Narration 渲染
        chat 采集                   chat 信号预处理             天象视觉反馈
        world_state 丰富化          循环稳定化

M2      Anvil 地形                  世界模型时序记忆            区域 HUD
        区域系统                    平衡算法                    payload 路由
        多 NPC + 寻路
        事件系统

M3      玩家持久化                  叙事质量优化                修仙 UI (owo-ui)
        战斗/采集/境界              个体关注                    动态 UI 下发
```

每层之间的**接口契约**由 `agent/packages/schema` 保证对齐。新增字段时：
1. 先改 TypeBox schema (TS)
2. 导出 JSON Schema (`npm run generate`)
3. Rust 侧更新 serde struct + 跑 sample 测试
4. Client 侧按需解析新字段

---

## 不做的事（始终范围外）

- 不做正版验证（保持 ConnectionMode::Offline）
- 不做反作弊
- 不做大规模负载测试（目前面向 2-10 人体验）
- 不做移动端
- 不自建 LLM（使用 API）
- ���追求 MC 版本升级（锁定 1.20.1 + Valence rev 2b705351）

---

## 关键决策记录

| 决策 | 理由 | 日期 |
|------|------|------|
| Valence (Rust) 而非 Paper/Spigot | ECS 架构、无历史包袱、高性能 | 2026-04-05 |
| Agent 是"天道"不是 NPC | 抹平 LLM 延迟、幻觉变奇遇、成本可控 | 2026-04-05 |
| 三层分离（Server/Agent/Client） | 各自独立迭代、Redis 解耦 | 2026-04-05 |
| TypeBox schema as source of truth | 双端对齐有单一源、JSON 不需要 codegen 工具链 | 2026-04-06 |
| 直接用 OpenAI-compatible API 而非 fork Pi | MVP 更快、依赖更少、后续可换 | 2026-04-06 |
| WSLg + gradlew runClient | 零额外安装、开发态直接测试 | 2026-04-06 |
