# Server 路线详细计划（Rust / Valence）

> 从 MVP 0.1 的草地平台推进到能执行天道指令、有区域划分、支撑修仙玩法的法则引擎。
> 每个 Task 标注前置依赖和验证方式。

---

## 当前代码结构

```
server/src/
├── main.rs                  # 入口：crossbeam + Valence App
├── world.rs                 # 16x16 草地平台
├── player.rs                # 连接/断连
├── npc/
│   ├── mod.rs               # 插件注册
│   ├── spawn.rs             # 僵尸 NPC + Thinker
│   ├── brain.rs             # 接近评分 + 逃跑行为
│   └── sync.rs              # Position→Transform
├── network/
│   ├── mod.rs               # 系统注册 + Redis bridge 启动
│   ├── agent_bridge.rs      # mock bridge (legacy)
│   └── redis_bridge.rs      # Redis pub/sub
└── schema/                  # IPC 数据结构 (serde)
```

---

## M1 — 天道闭环 [✓]

### S1. 指令执行器 `network/command_executor.rs` [✓]

**目标**：解析 `AgentCommandV1`，在 ECS 中实际生效。

**实现状态**：✅ **完成**（+ 超出计划的 per-tick budget 限流）

**实现细节**：

```
AgentCommandV1.commands[] → 逐条 match command_type:

spawn_event:
  params.event == "thunder_tribulation"
    → 在 target zone 随机位置连续发闪电 (LightningBolt 实体)
    → 如果有 target_player，闪电落点偏向玩家位置 ±5 格
    → duration_ticks 期间内持续生成闪电（每 40 ticks 一次）
    → intensity 缩放闪电密度 [0.0, 1.0]
  params.event == "beast_tide"
    → 在 target zone 边缘生成 N 个攻击型 NPC (N = intensity * 10)
    → NPC 带 PatrolAction 沿寻路向 zone 中心推进
  params.event == "realm_collapse" / "karma_backlash"
    → 暂未实现（如计划所述 M2+ 再实现）

modify_zone:
  ✅ 修改 spirit_qi += delta（clamp [0, 1]）
  ✅ 修改 danger_level += delta（clamp [0, 5]）
  ✅ 变更自动包含在下次 world_state 发布中

npc_behavior:
  ✅ 按 canonical NPC ID（npc_{index}v{generation}）查找
  ✅ 修改 flee_threshold [0.0, 1.0]
  ✅ generation-aware 匹配，防止旧 ID 误伤重生 NPC
```

**Bevy 集成**：
- ✅ `CommandExecutorResource` 存储待执行指令队列（VecDeque）
- ✅ `process_redis_inbound` 将 AgentCommand push 到队列
- ✅ `execute_agent_commands` system (Update) 逐帧消费
- ✅ **Per-tick budget 限流**：MAX_COMMANDS_PER_TICK（当前 100），溢出自动延到下一 tick

**测试**：71 个测试全部通过，包括 command budget、unknown targets 拒绝、NPC ID 校验等

---

### S2. 世界状态采集丰富化 [✓]

**目标**：`publish_world_state_to_redis` 发布真实数据而非占位符。

**实现状态**：✅ **完成**

**实现细节**：

```
players: Query<(&Client, &Position, &PlayerState)>
  ✅ 遍历生成 PlayerProfile（带真实位置/名字/状态）
  ✅ uuid: "offline:{username}"
  ✅ realm/composite_power/breakdown：从 PlayerState 读取
  ✅ 实时发布每个在线玩家的修为数据

npcs: Query<(&NpcMarker, &NpcBlackboard, &Position, &EntityKind)>
  ✅ 遍历生成 NpcSnapshot
  ✅ id: canonical NPC ID（npc_{index}v{generation}）
  ✅ kind: EntityKind display
  ✅ state: 从 ActionState 推断 (idle/fleeing/attacking)

zones: 从 ZoneRegistry resource 读取
  ✅ 所有已配置的 Zone（spirit_qi / danger_level / active_events）
  ✅ 支持从 zones.json 加载多个 Zone
```

**Redis 话题**：`bong:world_state`（Pub/Sub，周期发布）

---

### S3. 玩家 Chat 采集 [✓]

**目标**：拦截玩家聊天消息，推送到 Redis List。

**实现状态**：✅ **完成**（+ 超出计划的 gameplay 命令解析）

**实现细节**：
- ✅ 监听 Valence 的 `ChatMessageEvent`
- ✅ 构造 `ChatMessageV1 { v: 1, ts, player, raw, zone }`
- ✅ 通过 `RedisOutbound::PlayerChat` 发送到 Redis List `bong:player_chat`
- ✅ zone 字段：根据玩家 Position 自动查找所在 Zone

**额外功能**（超出计划）：
- ✅ Rate limit：每玩家每 tick 最多 3 条消息（`MAX_CHAT_MESSAGES_PER_PLAYER_PER_TICK`）
- ✅ 消息长度限制：最多 256 字符（`CHAT_MESSAGE_MAX_LENGTH`）
- ✅ **命令解析**：`/bong combat|gather|breakthrough` 路由到 `GameplayActionQueue`（M3 的内容提前入场）
  - `/bong combat <target> <target_health>` → CombatAction
  - `/bong gather <resource>` → GatherAction
  - `/bong breakthrough` → AttemptBreakthrough
- ✅ 斜杠命令过滤：`/` 开头的被标记为命令，跳过采集

**Redis 话题**：`bong:player_chat`（List，Agent 通过 BLPOP drain）

---

### S4. Narration 精准下发 [✓]

**目标**：按 scope 区分 narration 接收者。

**实现状态**：✅ **完成**（采用 CustomPayload 而非 MC 聊天格式）

**实现细节**：

```
scope == Broadcast  → 所有在线 client
scope == Zone       → 查询 ZoneRegistry，只发给在该 zone 内的 client（Position 包含于 Zone.bounds）
scope == Player     → 按 username 精准定位单个 client（支持 "Steve" 和 "offline:Steve" 两种格式）
```

**关键改进**：
- ✅ 原计划的 MC 格式化颜色码（`§c§l[天道警示]`）改为 **CustomPayload + 客户端 handler** 渲染
- ✅ Narration 作为 `ServerDataPayloadV1::Narration { narrations: Vec<...> }` 发送
- ✅ `Narration` 结构体包含 `style` (SystemWarning/Perception/Narration/EraDecree)，由客户端按风格渲染
- ✅ 例：客户端可以将 `SystemWarning` 渲染为 HUD 警告框，`Narration` 渲染为聊天栏带前缀文字等

**发送机制**：
- ✅ `process_agent_narrations` 从 Redis 订阅 `bong:agent_narrate` 读取 `NarrationV1`
- ✅ 按 scope 分类，调用 `process_single_narration` 逐条路由
- ✅ `narration_selector` 确定接收者集合，`collect_routed_targets` 执行路由

**测试覆盖**：broadcast/zone-scoped/player-scoped 都有专门的集成测试

---

## M2 — 有意义的世界 [✓]

### S5. Anvil 地形加载 [✓]

**前置**：WorldPainter 地图文件（.mca）放入 `server/world/` 目录（可选）。

**实现状态**：✅ **完成**

**实现细节**：
- ✅ `world/mod.rs` 支持两种初始化模式：
  1. **Anvil 模式**（优先）：从 `world/region/*.mca` 加载真实地形
  2. **Fallback 模式**：如果 Anvil 缺失/损坏，回退到 16×16 平坦草地
- ✅ 通过环境变量 `BONG_WORLD_PATH` 可配置 region 目录
- ✅ 覆盖测试：anvil 存在/缺失/region 空/assets 无效等场景

**地图设计建议**（WorldPainter）：
- 256x256 blocks（1 个 region 文件）起步
- 5 个区域用不同生物群系：新手谷/血谷/青云峰/修罗场/灵泉湖
- MC 1.18+ 格式导出

---

### S6. 区域系统 `world/zone.rs` [✓]

**实现状态**：✅ **完成**（+ 超出计划的 patrol_anchors 和 blocked_tiles）

**实现细节**：

```rust
pub struct Zone {
    pub name: String,
    pub bounds: (DVec3, DVec3),    // AABB min/max
    pub spirit_qi: f64,            // [0.0, 1.0]
    pub danger_level: u8,          // [0, 5]
    pub active_events: Vec<String>,
    pub patrol_anchors: Vec<DVec3>,     // [✓ 新增] NPC 巡逻点
    pub blocked_tiles: Vec<(i32, i32)>, // [✓ 新增] 寻路障碍
}

impl ZoneRegistry {
    pub fn find_zone(&self, pos: DVec3) -> Option<&Zone>       // 按位置查询
    pub fn find_zone_by_name(&self, name: &str) -> Option<&Zone>  // 按名称查询
    pub fn find_zone_mut(&mut self, name: &str) -> Option<&mut Zone> // 可变查询
}
```

**加载机制**：
- ✅ 从 `zones.json` 加载（严格校验：重名检测、bounds 合法性、anchor 不可在 blocked tile 上等）
- ✅ Fallback：`zones.json` 缺失时回退单 spawn zone
- ✅ M1 就已支持多 zone，不限于硬编码

**验证**：71 个单元测试全部通过

---

### S7. 多 NPC + 寻路 [✓]

**实现状态**：✅ **完成**（A* 寻路 + patrol 调度）

**实现细节**：
- ✅ `NpcPatrol` component：当前目标点、路径队列、重路径计时器
- ✅ `PatrolGrid`：基于 Zone.bounds 构建 2D 网格，Z 轴离散化为行列
- ✅ `pathfinding::astar()` 求解 shortest path，考虑 Zone.blocked_tiles
- ✅ 重路径间隔：`PATROL_REPATH_INTERVAL_TICKS = 10` ticks（避免每帧重算）
- ✅ NPC 沿路径逐步移动（`PATROL_STEP_DISTANCE = 0.2`），到达目标后选新巡逻点

**NPC 类型**：Zombie（默认，逃跑 AI）+ Skeleton（可扩展巡逻型）

**验证**：NPC 在 zone 内自主巡逻，遇到障碍绕行

---

### S8. 事件系统 `world/events.rs` [✓]

**实现状态**：✅ **完成**（thunder + beast_tide，realm_collapse/karma_backlash 暂未实现）

**实现细节**：

```
ActiveEventsResource — 事件调度器
  存储：进行中的 ActiveEvent 队列
  推进：每 tick 递增 elapsed_ticks，生成事件实体/粒子

thunder_tribulation:
  ✅ 每 40 ticks 在 zone 内随机位置 spawn LightningBolt 实体
  ✅ 闪电位置偏向 target_player ±5 格（如有）
  ✅ intensity [0.0, 1.0] 缩放闪电生成频率
  ✅ 支持多重天劫在同一 zone（去重机制防止重复调度）

beast_tide:
  ✅ 在 zone 边缘 spawn N = intensity * 10 个 Zombie NPC
  ✅ 新增 NPC 自动带 PatrolAction，向 zone 中心推进
  ✅ 事件过期后 despawn 所有生成的 NPC
  ✅ 支持跟踪已生成 NPC，确保清理不遗漏

realm_collapse / karma_backlash:
  暂未实现（如计划所述 M2+ 阶段）
```

**事件驱动**：Agent 通过 `CommandType::SpawnEvent` 下发，由 `execute_spawn_event` 入队到 `ActiveEventsResource`

---

## M3 — 修仙体验

### S9. 玩家状态持久化 [✓]

**实现状态**：✅ **完成**

**实现细节**：

```rust
#[derive(Component, Serialize, Deserialize)]
pub struct PlayerState {
    pub realm: String,           // "mortal", "qi_refining_1", ..., "qi_refining_3"
    pub spirit_qi: f64,          // 当前真元储量
    pub spirit_qi_max: f64,      // 上限（随境界提升）
    pub karma: f64,              // [-1.0, 1.0]
    pub experience: u64,         // 经验值
    pub inventory_score: f64,    // 财富评分 [0.0, 1.0]
}
```

**持久化机制**：
- ✅ `PlayerStatePersistence` resource 管理数据目录（默认 `data/players/`）
- ✅ 每 `PLAYER_STATE_AUTOSAVE_INTERVAL_TICKS` (1200 ticks ≈ 60 秒) 自动保存
- ✅ 玩家断连时强制保存
- ✅ 重连时自动加载并 attach PlayerState component
- ✅ 损坏的 JSON 自动用默认值回退

**power 计算**：
- ✅ `composite_power()` — 加权合并多维评分（combat 40% + wealth 15% + social 15% + karma 15% + territory 15%）
- ✅ `power_breakdown()` — 五维评分（combat/wealth/social/karma/territory）
- ✅ 每维都是 [0.0, 1.0] 的单调函数，由 realm/qi_ratio/experience/karma/inventory 组合而成

**测试**：save/load/corruption recovery 都有覆盖

---

### S10. 战斗 + 采集 + 境界 [~ 框架完成，玩法逻辑待扩展]

**实现状态**：⚠️ **部分完成**（框架搭好，但游戏事件集成还不完整）

**已实现部分**：

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum GameplayAction {
    Combat(CombatAction { target, target_health }),     // 战斗
    Gather(GatherAction { resource }),                 // 采集
    AttemptBreakthrough,                               // 境界突破
}

pub struct BreakthroughRule {
    current_realm: &'static str,
    next_realm: &'static str,
    required_experience: u64,
    minimum_karma: f64,
    required_spirit_qi: f64,
    next_spirit_qi_max: f64,
}
```

**当前玩法入口**：
- ✅ **Chat 命令**：`/bong combat <target> <health>` / `/bong gather <resource>` / `/bong breakthrough`
- ✅ `GameplayActionQueue` 缓存玩家操作（来自 chat_collector 解析）
- ✅ `emit_gameplay_narrations` 提供反馈（成功/失败的 narration）

**突破规则**（M1 已定义，可扩展）：
```
mortal → qi_refining_1:     exp ≥ 120,  karma ≥ -0.2, qi ≥ 60
qi_refining_1 → qi_refining_2: exp ≥ 300,  karma ≥ -0.1, qi ≥ 90
qi_refining_2 → qi_refining_3: exp ≥ 600,  karma ≥  0.0, qi ≥ 110
（更高境界待定）
```

**待实现**（下一阶段）：
- ⏳ 真正的 MC 攻击事件监听（当前依赖 chat 命令）
- ⏳ 方块采集事件（特定灵草方块右键）
- ⏳ 经验/伤害 数值调整与平衡
- ⏳ 更多境界等级和突破条件

---

## 当前代码结构（实际）

```
server/src/
├── main.rs                           # Bevy App + crossbeam 初始化
├── world/
│   ├── mod.rs              [✓]      # Anvil + fallback 地形初始化
│   ├── zone.rs             [✓]      # ZoneRegistry + Zone 定义 + JSON 加载
│   └── events.rs           [✓]      # ActiveEventsResource + thunder/beast_tide 事件
├── player/
│   ├── mod.rs              [✓]      # 连接/断连
│   ├── state.rs            [✓]      # PlayerState 持久化 + composite_power 计算
│   └── gameplay.rs         [~]      # GameplayActionQueue + BreakthroughRule（框架）
├── npc/
│   ├── mod.rs              [✓]      # 插件注册
│   ├── spawn.rs            [✓]      # NPC 生成
│   ├── brain.rs            [✓]      # big-brain Thinker（逃跑 AI）
│   ├── patrol.rs           [✓]      # NpcPatrol + PatrolGrid + A* 寻路
│   └── sync.rs             [✓]      # Position → Transform 桥接
├── network/
│   ├── mod.rs              [✓]      # publish_world_state + process_redis_inbound
│   ├── agent_bridge.rs     [✓]      # mock bridge（legacy）
│   ├── redis_bridge.rs     [✓]      # Tokio Sub/Pub + crossbeam
│   ├── command_executor.rs [✓]      # Agent 指令执行 + budget 限流
│   └── chat_collector.rs   [✓]      # 聊天采集 + gameplay 命令解析
└── schema/                 [✓]      # IPC 数据结构（与 TS 对齐）
```

**总计**：23 个 Rust 源文件 | 71 个单元测试全部通过

---

## 开发历程总结

```
✅ M1 天道闭环 — 完全实现
   S1 指令执行器（+ per-tick budget 限流）
   S2 世界状态采集
   S3 Chat 采集（+ gameplay 命令解析提前入场）
   S4 Narration 精准下发（采用 CustomPayload 而非 MC 聊天格式）

✅ M2 有意义的世界 — 完全实现
   S5 Anvil 地形加载（+ fallback 模式）
   S6 区域系统（+ patrol_anchors / blocked_tiles / JSON 配置）
   S7 多 NPC + A* 寻路
   S8 事件系统（thunder_tribulation + beast_tide）

🔄 M3 修仙体验 — 框架完成，玩法待扩展
   S9 玩家状态持久化（✓ 完成）
   S10 战斗+采集+境界（⚠️ 框架在，chat 命令入口，MC 事件集成待做）

🎯 Next：
   - 真正的 MC 攻击事件监听（PlayerAttackEvent）
   - 方块采集事件（PlayerInteractBlockEvent）
   - 更多境界等级 + 数值平衡
```
