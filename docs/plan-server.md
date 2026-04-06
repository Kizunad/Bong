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

## M1 — 天道闭环

### S1. 指令执行器 `network/command_executor.rs`

**目标**：解析 `AgentCommandV1`，在 ECS 中实际生效。

**新增文件**：`server/src/network/command_executor.rs`

**实现细节**：

```
AgentCommandV1.commands[] → 逐条 match command_type:

spawn_event:
  params.event == "thunder_tribulation"
    → 在 target zone 随机位置连续发 3 次闪电 (LightningBolt 实体)
    → 如果有 target_player，闪电落点偏向玩家位置 ±5 格
    → 闪电伤害由 intensity 决定 (intensity * 10 = 伤害值)
    → duration_ticks 期间内持续生成闪电（每 40 ticks 一次）
  params.event == "beast_tide"
    → 在 target zone 边缘生成 N 个攻击型 NPC (N = intensity * 10)
    → NPC 带 PatrolAction 向 zone 中心推进
  params.event == "realm_collapse" / "karma_backlash"
    → M2+ 再实现，M1 先 log warning "not implemented"

modify_zone:
  → 查找 zone by name，修改 spirit_qi += delta, danger_level += delta
  → clamp spirit_qi to [0, 1], danger_level to [0, 5]
  → 广播 zone 变更到 world_state

npc_behavior:
  → 查找 NPC by id，修改 Thinker 参数
  → M1 先支持 flee_threshold 修改（改变 PROXIMITY_THRESHOLD）
```

**Bevy 集成**：
- 新增 `CommandExecutorResource` 存储待执行指令队列
- `process_redis_inbound` 将 AgentCommand push 到队列
- `execute_agent_commands` system (Update) 逐帧消费队列

**验证**：
- 手动 `redis-cli PUBLISH bong:agent_command '{...}'` 发一条 spawn_event
- 游戏内看到闪电生成

---

### S2. 世界状态采集丰富化

**目标**：`publish_world_state_to_redis` 发布真实数据而非占位符。

**改动文件**：`network/mod.rs` 的 `publish_world_state_to_redis`

**实现细节**：

```
players: Query<(&Client, &Position, &GameMode)>
  → 遍历生成 PlayerProfile
  → uuid: client.username() 或 "offline:{name}"
  → name: client.username()
  → pos: position.get()
  → realm/composite_power/breakdown/trend: M1 先硬编码默认值
  → M3 再从 PlayerState component 读取

npcs: Query<(&NpcMarker, &NpcBlackboard, &Position, &EntityKind)>
  → 遍历生成 NpcSnapshot
  → id: entity index 作为 string
  → kind: EntityKind display
  → state: 从 ActionState 推断 (idle/fleeing/attacking)

zones: 从 ZoneRegistry resource 读取
  → M1 先硬编码 1 个 "spawn" zone
  → M2 从 Anvil 地图元数据读取
```

**验证**：
- 玩家连接后，`redis-cli SUBSCRIBE bong:world_state` 能看到带真实玩家名和位置的 JSON

---

### S3. 玩家 Chat 采集

**目标**：拦截玩家聊天消息，推送到 Redis List。

**新增**：`network/chat_collector.rs`

**实现细节**：
- 监听 Valence 的 `ChatMessageEvent`（或 `CommandExecutionEvent`）
- 构造 `ChatMessageV1 { v: 1, ts, player, raw, zone }`
- 通过 Redis bridge 的 pub 连接 RPUSH 到 `bong:player_chat`
- zone 字段：根据玩家 Position 查找所在 Zone（M1 先固定 "spawn"）

**Redis 注意**：`player_chat` 用 List 而非 Pub/Sub，因为 Agent 需要 drain（BLPOP），不是实时订阅。需要在 `redis_bridge.rs` 增加 RPUSH 能力。

**验证**：
- 玩家在游戏内发消息
- `redis-cli LRANGE bong:player_chat 0 -1` 能看到 JSON

---

### S4. Narration 精准下发

**目标**：按 scope 区分 narration 接收者。

**改动**：`network/mod.rs` 的 `process_redis_inbound` → `RedisInbound::AgentNarration` 分支

```
scope == Broadcast → 所有 client
scope == Zone → 只发给在该 zone 内的 client（Position 匹配 Zone 边界）
scope == Player → target 匹配 client username
```

**MC 格式化**：
```
SystemWarning → "§c§l[天道警示] §r§c{text}"
Perception    → "§7[感知] §r§7{text}"
Narration     → "§f[叙事] §r§f{text}"
EraDecree     → "§6§l[§e时代§6§l] §r§6{text}"
```

**验证**：
- Agent 发 scope=player 的 narration，只有目标玩家看到

---

## M2 — 有意义的世界

### S5. Anvil 地形加载

**前置**：WorldPainter 地图文件（.mca）放入 `server/world/` 目录。

**改动**：`world.rs`

```rust
// 替换手动 16x16 草地生成
let anvil = AnvilLevel::new("world/region", &biomes);
commands.spawn((layer_bundle, anvil));
```

**地图设计建议**（WorldPainter）：
- 256x256 blocks（1 个 region 文件）起步
- 5 个区域用不同生物群系标记：
  - 新手谷 (plains, Y=64-70)
  - 血谷 (badlands, Y=50-65)
  - 青云峰 (mountains, Y=80-120)
  - 修罗场 (nether_wastes biome in overworld, Y=60)
  - 灵泉湖 (ocean/river, Y=62)
- MC 1.18+ 格式导出

**验证**：玩家进入看到非平坦的真实地形

---

### S6. 区域系统 `world/zone.rs`

**新增**：`server/src/world/zone.rs`

```rust
#[derive(Resource)]
pub struct ZoneRegistry {
    pub zones: Vec<Zone>,
}

pub struct Zone {
    pub name: String,
    pub bounds: (DVec3, DVec3),  // AABB min/max
    pub spirit_qi: f64,         // [0, 1]
    pub danger_level: u8,       // [0, 5]
    pub active_events: Vec<String>,
}

impl ZoneRegistry {
    pub fn find_zone(&self, pos: DVec3) -> Option<&Zone> { ... }
    pub fn find_zone_mut(&mut self, name: &str) -> Option<&mut Zone> { ... }
}
```

- M1 硬编码 1 个 zone
- M2 从配置文件（`server/zones.json`）加载
- `modify_zone` 指令修改 ZoneRegistry
- `publish_world_state` 从 ZoneRegistry 读取真实数据

---

### S7. 多 NPC + 寻路

**改动**：`npc/spawn.rs` + 新增 `npc/patrol.rs`

```
NPC 类型枚举：
  Zombie   — 攻击型，被天劫召唤的兽潮
  Skeleton — 巡逻型，守卫区域
  Villager — 商人型，M3 再加交易

PatrolAction (新 big-brain Action):
  - 在 zone 内随机选点
  - A* 寻路到目标点 (pathfinding crate)
  - 到达后等待 3-5 秒，再选新点
  
寻路网格：
  - 初始化时扫描 zone 内地面方块
  - 构建 2D grid (walkable/blocked)
  - pathfinding::astar() 求路径
  - 每 N ticks 重算一次（不需要实时）
```

**验证**：NPC 在区域内来回走动，遇到障碍物绕行

---

### S8. 事件系统 `world/events.rs`

**spawn_event 完整实现**：

```
thunder_tribulation:
  → 持续 duration_ticks
  → 每 40 ticks 在 zone 内随机位置 spawn 闪电
  → 闪电位置偏向 target_player（如有）
  → intensity 影响闪电密度和伤害

beast_tide:
  → 在 zone 边缘 spawn N 个攻击型 NPC
  → NPC 带 PatrolAction 向 zone 中心推进
  → duration_ticks 后剩余 NPC despawn

realm_collapse:
  → zone 的 spirit_qi 在 duration_ticks 内线性降至 0
  → zone 内所有玩家持续掉血（每 20 ticks 1 心）
  → M3 结合真元系统

karma_backlash:
  → 针对特定玩家
  → 该玩家周围 5 格内持续生成火焰粒子
  → 每 20 ticks 扣 1 心
```

**事件调度器**：`ActiveEventsResource` 存储当前进行中的事件，每 tick 推进状态。

---

## M3 — 修仙体验

### S9. 玩家状态持久化

```rust
#[derive(Component, Serialize, Deserialize)]
pub struct PlayerState {
    pub realm: String,           // "mortal", "qi_refining_1", ..., "nascent_soul"
    pub spirit_qi: f64,          // 当前真元储量
    pub spirit_qi_max: f64,      // 上限（随境界提升）
    pub karma: f64,              // [-1, 1]
    pub experience: u64,         // 经验值
    pub inventory_score: f64,    // 财富评分（简化版）
}
```

- 每 60 秒自动保存到 `server/data/players/{uuid}.json`
- 玩家断连时保存
- 重连时加载并 attach component
- `composite_power` 从 PlayerState 实时计算

### S10. 战斗 + 采集 + 境界

- 攻击事件监听 → 扣血 + 经验
- 特定方块右键采集 → 给物品 + 经验
- 经验达标 + karma 条件 → 境界突破（提升 spirit_qi_max）
- 境界列表：凡人 → 练气1-9 → 筑基1-3 → 金丹 → 元婴（数值表待定）

---

## 文件规划总览

```
server/src/
├── main.rs
├── world/
│   ├── mod.rs              # 世界初始化（Anvil 或 fallback 草地）
│   ├── zone.rs             # 区域系统 + ZoneRegistry
│   └── events.rs           # 天劫/兽潮/秘境事件调度
├── player/
│   ├── mod.rs              # 连接/断连
│   └── state.rs            # PlayerState 持久化 (M3)
├── npc/
│   ├── mod.rs
│   ├── spawn.rs
│   ├── brain.rs
│   ├── patrol.rs           # 巡逻 Action + A* 寻路 (M2)
│   └── sync.rs
├── network/
│   ├── mod.rs
│   ├── agent_bridge.rs
│   ├── redis_bridge.rs
│   ├── command_executor.rs # Agent 指令执行 (M1)
│   └── chat_collector.rs   # 聊天采集 (M1)
└── schema/                 # 不变
```

---

## 开发顺序建议

```
M1 顺序（依赖关系）：
  S2 世界状态丰富化（独立，先做）
  S3 Chat 采集（独立，可并行）
  S1 指令执行器（依赖 S6 Zone 概念，M1 先硬编码 1 zone）
  S4 Narration 精准下发（依赖 S6 Zone 查询）

M2 顺序：
  S5 Anvil 地形（独立，先做）
  S6 区域系统（独立，可与 S5 并行）
  S7 多 NPC + 寻路（依赖 S5 地形数据）
  S8 事件系统（依赖 S6 + S7）

M3 顺序：
  S9 玩家持久化（独立）
  S10 战斗采集境界（依赖 S9）
```
