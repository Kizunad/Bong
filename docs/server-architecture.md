# Server 架构图

> 按 M1→M2→M3 逐阶段展开。每个阶段标注组件状态：
> - `[✓]` 已实现（MVP 0.1）
> - `[+]` 本阶段新增
> - `[~]` 本阶段改造
> - `[ ]` 未来阶段

---

## 〇、MVP 0.1 现状（基线）

```
server/src/
├── main.rs           [✓] Bevy App + crossbeam 初始化
├── world.rs          [✓] 16×16 chunk 平坦草地（硬编码）
├── player.rs         [✓] 连接/断连 handler
├── npc/
│   ├── mod.rs        [✓] 插件注册
│   ├── spawn.rs      [✓] 1只僵尸 NPC + Thinker
│   ├── brain.rs      [✓] ProximityScorer + FleeAction
│   └── sync.rs       [✓] Position↔Transform 桥接
├── network/
│   ├── mod.rs        [✓] publish_world_state (硬编码占位)
│   │                 [✓] process_redis_inbound (log only)
│   ├── agent_bridge  [✓] mock bridge (legacy)
│   └── redis_bridge  [✓] Tokio Sub/Pub + crossbeam
└── schema/           [✓] serde 结构体（与 TS 对齐）
```

**能力**：玩家能连入草地平台，看到僵尸，Redis 能收发 JSON。
**不能**：没有区域、没有灵气、指令不执行、Chat 不采集、Narration 不区分接收者。

---

## 一、M1 — 天道闭环

> 目标：Agent 的决策能在游戏内**可感知地生效**。

### M1 架构图

```
                    ┌──────────────────────────────────┐
                    │        天道 Agent (TS)            │
                    │   Calamity / Mutation / Era       │
                    │   Arbiter [A1]  ChatProc [A2]     │
                    └──────┬───────────────────▲────────┘
                           │                   │
              ┌────────────┼───────────────────┼──────────────┐
              │   Redis    │                   │               │
              │            │                   │               │
              │   agent_cmd ▼          world_state ▲           │
              │   (Pub/Sub)            (Pub/Sub)               │
              │                                                │
              │   agent_narrate ▼      player_chat ▲           │
              │   (Pub/Sub)            (List)                  │
              └────────────┼───────────────────┼──────────────┘
                           │                   │
    ═══════════════════════╪═══════════════════╪═════════════════
                           │                   │
    ┌──────────────────────▼───────────────────┴──────────────────┐
    │                    network/ 层                               │
    │                                                              │
    │  redis_bridge.rs [✓]                                        │
    │  ┌──────────┐      ┌──────────────┐                         │
    │  │ Tokio    │◄────►│ crossbeam    │                         │
    │  │ thread   │      │ in/out chan  │                         │
    │  └──────────┘      └──────┬───────┘                         │
    │                           │                                  │
    │           ┌───────────────┼───────────────┐                  │
    │           ▼               ▼               ▼                  │
    │  ┌──────────────┐ ┌────────────┐ ┌──────────────────┐       │
    │  │ command_     │ │ chat_      │ │ mod.rs           │       │
    │  │ executor.rs  │ │ collector  │ │                  │       │
    │  │ [+] S1      │ │ .rs        │ │ publish_world    │       │
    │  │              │ │ [+] S3    │ │ _state [~] S2   │       │
    │  │ 解析 Command │ │            │ │                  │       │
    │  │ → ECS 操作   │ │ ChatMsg    │ │ deliver_         │       │
    │  │              │ │ 事件监听   │ │ narrations       │       │
    │  │ spawn_event: │ │ → RPUSH    │ │ [~] S4          │       │
    │  │  闪电(M1简版)│ │ player_chat│ │                  │       │
    │  │  兽潮(M1简版)│ │            │ │ scope 过滤:      │       │
    │  │  其余→log    │ │            │ │  Broadcast→全员  │       │
    │  │              │ │            │ │  Zone→区域内     │       │
    │  │ modify_zone: │ │            │ │  Player→个人     │       │
    │  │  改 qi/danger│ │            │ │                  │       │
    │  │              │ │            │ │ MC色码格式化:    │       │
    │  │ npc_behavior:│ │            │ │  §c警示 §7感知  │       │
    │  │  改Thinker   │ │            │ │  §6时代 §f叙事  │       │
    │  └──────┬───────┘ └─────▲──────┘ └────────┬─────────┘       │
    └─────────┼───────────────┼─────────────────┼─────────────────┘
              │               │                 │
    ══════════╪═══════════════╪═════════════════╪════════════════════
              │               │                 │
    ┌─────────▼───────────────┼─────────────────▼─────────────────────┐
    │                     Bevy ECS                                     │
    │                                                                  │
    │  Resources                                                       │
    │  ┌──────────────────┐  ┌───────────────────┐                     │
    │  │ CommandExecutor  │  │ WorldStateTimer   │                     │
    │  │ Resource [+] S1  │  │ [✓] 200ticks     │                     │
    │  │                  │  │                   │                     │
    │  │ 待执行指令队列    │  │ 每10秒发布到Redis │                     │
    │  └──────────────────┘  └───────────────────┘                     │
    │                                                                  │
    │  ┌───────────────────────────────────────────┐                   │
    │  │ ZoneRegistry [+] S6（M1硬编码版）          │                   │
    │  │                                           │                   │
    │  │ zones: vec![                              │                   │
    │  │   Zone { name: "spawn",                   │                   │
    │  │          bounds: (0,0,0)-(256,128,256),   │                   │
    │  │          spirit_qi: 0.5,                  │                   │
    │  │          danger_level: 1,                 │                   │
    │  │          active_events: [] }              │                   │
    │  │ ]                                         │                   │
    │  └───────────────────────────────────────────┘                   │
    │                                                                  │
    │  Components                                                      │
    │  ┌────────────────────────────────────────┐                      │
    │  │ Player Entity [✓]                      │                      │
    │  │  Client + Position + GameMode          │                      │
    │  │  (PlayerState [ ] → M3)                │                      │
    │  ├────────────────────────────────────────┤                      │
    │  │ NPC Entity [✓]                         │                      │
    │  │  NpcMarker + Position + Thinker        │                      │
    │  │  NpcBlackboard + EntityKind            │                      │
    │  └────────────────────────────────────────┘                      │
    │                                                                  │
    │  Systems                                                         │
    │  ┌────────────────────────────────────────┐                      │
    │  │ [✓] setup_world (16×16 草地)           │                      │
    │  │ [✓] handle_connections                 │                      │
    │  │ [✓] spawn_npc + brain_tick + sync      │                      │
    │  │ [✓] process_redis_inbound              │                      │
    │  │ [~] publish_world_state ← 读真实数据    │  ← S2               │
    │  │ [+] execute_agent_commands             │  ← S1               │
    │  │ [+] collect_chat                       │  ← S3               │
    │  │ [~] deliver_narrations ← scope过滤     │  ← S4               │
    │  └────────────────────────────────────────┘                      │
    └──────────────────────────────────────────────────────────────────┘
              │
              ▼
    ┌───────────────────┐        ┌───────────────────┐
    │  Valence Protocol │        │   Fabric Client   │
    │  MC 1.20.1 (763)  │───────►│   [~] C1 叙事渲染 │
    └───────────────────┘        └───────────────────┘
```

### M1 任务清单与依赖

```
     ┌─────────────────────────────────────────────────┐
     │ S6-lite  ZoneRegistry (硬编码1个zone)             │
     │ 独立，最先做。后续 S1/S2/S4 都依赖 zone 概念      │
     └──────┬──────────────────────┬───────────────────┘
            │                      │
     ┌──────▼──────┐        ┌──────▼──────┐
     │ S2 状态丰富  │        │ S3 Chat采集  │     ◄── 两者互不依赖，可并行
     │ world_state │        │ RPUSH 到     │
     │ 读真实Player │        │ player_chat  │
     │ /NPC/Zone    │        └──────┬───────┘
     └──────┬───────┘               │
            │                       │
     ┌──────▼───────────────────────▼───┐
     │ S1 指令执行器                      │
     │ spawn_event: 闪电+兽潮 (简版)      │
     │ modify_zone: 改 ZoneRegistry      │
     │ npc_behavior: 改 Thinker 参数     │
     └──────┬───────────────────────────┘
            │
     ┌──────▼───────┐
     │ S4 叙事精准   │
     │ scope 过滤    │   ◄── 依赖 Zone 查询 (pos→zone)
     │ MC 色码格式化  │
     └──────────────┘

     ═══════════════════ 跨路线依赖 ═══════════════════

     Agent 侧                          Client 侧
     ┌──────────────┐                  ┌──────────────┐
     │ A1 Arbiter   │ ◄── 需要 S1     │ C1 叙事渲染   │
     │ A2 ChatProc  │ ◄── 需要 S3     │              │
     │ A3 循环稳定   │     已完成       │ ◄── 需要 S4  │
     │ A4 peer_ctx  │     独立         │              │
     └──────────────┘                  └──────────────┘
```

### M1 验证标准

```
启动流程: redis-server → cargo run → npm start → ./gradlew runClient

30秒内应该看到:
  ✓ redis-cli SUBSCRIBE bong:world_state → 包含真实玩家名、位置、zone
  ✓ 游戏内聊天 → redis-cli LRANGE bong:player_chat 0 -1 有消息
  ✓ Agent 决策 → server 日志 "executing command: spawn_event"
  ✓ 游戏内闪电/NPC 生成
  ✓ 聊天栏出现 §c[天道警示] 或 §6[时代] 格式化叙事
  ✓ scope=player 的 narration 只有目标玩家看到
```

---

## 二、M2 — 有意义的世界

> 目标：从草地平台变成有地形、区域划分、多 NPC 的初步修仙世界。

### M2 增量架构（在 M1 基础上）

```
    ┌──────────────────────────────────────────────────────────────┐
    │                     Bevy ECS (M2 增量)                       │
    │                                                              │
    │  Resources                                                   │
    │  ┌───────────────────────────────────────────┐               │
    │  │ ZoneRegistry [~] S6 完整版                 │               │
    │  │                                           │               │
    │  │ 从 server/zones.json 加载：                │               │
    │  │                                           │               │
    │  │ ┌─────────┐ ┌─────────┐ ┌─────────┐      │               │
    │  │ │ 初醒原   │ │ 青云残峰 │ │ 血  谷  │      │               │
    │  │ │qi:0.3   │ │qi:0.5   │ │qi:0.3   │      │               │
    │  │ │danger:1 │ │danger:2 │ │danger:3 │      │               │
    │  │ │1500²    │ │1200²    │ │800×1500 │      │               │
    │  │ └─────────┘ └─────────┘ └─────────┘      │               │
    │  │ ┌─────────┐ ┌─────────┐ ┌─────────┐      │               │
    │  │ │ 灵泉湿地 │ │ 幽暗地穴 │ │ 北  荒  │      │               │
    │  │ │qi:0.7   │ │qi:0.2~8 │ │qi:0.05  │      │               │
    │  │ │danger:3 │ │danger:4 │ │danger:5 │      │               │
    │  │ └─────────┘ └─────────┘ └─────────┘      │               │
    │  └───────────────────────────────────────────┘               │
    │                                                              │
    │  ┌───────────────────────────────────────────┐               │
    │  │ ActiveEventsResource [+] S8                │               │
    │  │                                           │               │
    │  │ events: Vec<ActiveEvent>                   │               │
    │  │   ActiveEvent {                           │               │
    │  │     kind: ThunderTrib|BeastTide|RealmCollapse│             │
    │  │     zone: String,                         │               │
    │  │     remaining_ticks: u32,                 │               │
    │  │     intensity: f64,                       │               │
    │  │     target_player: Option<String>,         │               │
    │  │   }                                       │               │
    │  │                                           │               │
    │  │ 每tick: remaining -= 1, 触发效果,          │               │
    │  │         remaining == 0 → 移除              │               │
    │  └───────────────────────────────────────────┘               │
    │                                                              │
    │  Systems（新增/改造）                                         │
    │  ┌────────────────────────────────────────────────────┐      │
    │  │ [~] setup_world → AnvilLevel::new("world/region")  │ ← S5│
    │  │     fallback: 保留旧 16×16 草地（无 .mca 时）       │      │
    │  │                                                    │      │
    │  │ [+] zone_tick                                      │ ← S6│
    │  │     每 200 ticks:                                  │      │
    │  │       统计 zone 内活跃玩家数 → 灵气自然衰减          │      │
    │  │       qi_delta = -0.001 × player_count             │      │
    │  │       clamp spirit_qi to [0.0, 1.0]                │      │
    │  │                                                    │      │
    │  │ [+] event_tick                                     │ ← S8│
    │  │     遍历 ActiveEvents:                             │      │
    │  │       ThunderTrib → 每40ticks spawn闪电实体         │      │
    │  │       BeastTide   → 一次性在zone边缘spawn N只NPC   │      │
    │  │       RealmCollapse→ zone.qi 线性降至0 + 持续伤害   │      │
    │  │                                                    │      │
    │  │ [~] spawn_npc → 多种类型                           │ ← S7│
    │  │     NpcKind::Zombie   攻击型（兽潮）                │      │
    │  │     NpcKind::Skeleton 巡逻型（守卫）                │      │
    │  │     NpcKind::Villager 商人型（M3 交易）             │      │
    │  │                                                    │      │
    │  │ [+] patrol_action (big-brain Action)               │ ← S7│
    │  │     zone 内随机选点 → A*寻路 → 移动 → 等待 → 循环  │      │
    │  │     寻路网格: 初始化扫描zone地面 → 2D grid          │      │
    │  │     pathfinding::astar() 每N ticks重算              │      │
    │  └────────────────────────────────────────────────────┘      │
    │                                                              │
    │  NPC Entity（扩展）                                          │
    │  ┌────────────────────────────────────────────────────┐      │
    │  │ [✓] NpcMarker + Position + Thinker + NpcBlackboard │      │
    │  │ [+] NpcKind { Zombie, Skeleton, Villager }         │ ← S7│
    │  │ [+] PatrolTarget(DVec3)                            │ ← S7│
    │  │ [+] PathCache(Vec<DVec3>)                          │ ← S7│
    │  └────────────────────────────────────────────────────┘      │
    └──────────────────────────────────────────────────────────────┘

    新增文件:
      server/src/world/mod.rs        [+] 替代 world.rs, Anvil 加载
      server/src/world/zone.rs       [+] ZoneRegistry + Zone struct
      server/src/world/events.rs     [+] ActiveEventsResource + event_tick
      server/src/npc/patrol.rs       [+] PatrolAction + A* 寻路
      server/zones.json              [+] 区域配置（6个zone）
      server/world/region/*.mca      [+] WorldPainter 导出地图
```

### M2 任务依赖

```
     ┌──────────────┐       ┌──────────────┐
     │ S5 Anvil地形  │       │ S6 区域系统   │
     │ WorldPainter │       │ zones.json   │       ◄── S5, S6 可并行
     │ → .mca 文件  │       │ → ZoneReg    │
     │ AnvilLevel   │       │ zone_tick    │
     └──────┬───────┘       └──────┬───────┘
            │                      │
            │    ┌─────────────────┘
            │    │
     ┌──────▼────▼──────┐
     │ S7 多NPC + 寻路   │       ◄── 依赖 S5 (地形数据生成寻路网格)
     │ NpcKind 枚举      │           依赖 S6 (zone边界确定巡逻范围)
     │ PatrolAction      │
     │ A* pathfinding    │
     └──────┬────────────┘
            │
     ┌──────▼────────────┐
     │ S8 事件系统        │       ◄── 依赖 S6 (zone引用)
     │ ActiveEvents      │           依赖 S7 (兽潮生成NPC)
     │ 闪电/兽潮/域崩     │
     └───────────────────┘

     ═══════════════════ 跨路线依赖 ═══════════════════

     Agent 侧                          Client 侧
     ┌──────────────────┐              ┌─────────────────┐
     │ A5 时序记忆       │ ◄── S6      │ C3 天象视觉      │ ◄── S8
     │    zone qi 趋势   │    zone数据  │    闪电+屏闪     │    事件数据
     │                  │              │                 │
     │ A6 平衡算法       │ ◄── S2      │ C4 Zone HUD     │ ◄── S6
     │    Gini系数       │    丰富状态  │    灵气/区域名   │    zone数据
     └──────────────────┘              │                 │
                                       │ C5 Payload路由  │
                                       │    type字段分发  │
                                       └─────────────────┘
```

### M2 验证标准

```
  ✓ 玩家进入看到非平坦的真实地形（山/谷/水）
  ✓ 不同区域有不同灵气值，world_state JSON 能看到 6 个 zone
  ✓ zone 内灵气随玩家修炼缓慢衰减（每10秒观察变化）
  ✓ NPC 在区域内巡逻，遇到障碍物绕行
  ✓ redis-cli 发 spawn_event thunder → 游戏内看到闪电持续 N 秒
  ✓ redis-cli 发 spawn_event beast_tide → zone 边缘生成多只攻击 NPC
```

---

## 三、M3 — 修仙体验

> 目标：玩家有境界、有真元、有战斗意义、有持久存档。

### M3 增量架构（在 M2 基础上）

```
    ┌──────────────────────────────────────────────────────────────┐
    │                     Bevy ECS (M3 增量)                       │
    │                                                              │
    │  Player Entity（扩展）                                       │
    │  ┌────────────────────────────────────────────────────┐      │
    │  │ [✓] Client + Position + GameMode                   │      │
    │  │ [+] PlayerState                                    │ ← S9│
    │  │     ┌─────────────────────────────────────────┐    │      │
    │  │     │ realm: "awakened" | "qi_引气_1" | ...    │    │      │
    │  │     │        "凝脉" | "固元" | "通灵" | "化虚"  │    │      │
    │  │     │                                         │    │      │
    │  │     │ zhen_yuan: f64      // 当前真元          │    │      │
    │  │     │ zhen_yuan_max: f64  // 上限（随境界）     │    │      │
    │  │     │ karma: f64          // [-1, 1] 劫气      │    │      │
    │  │     │ death_count: u32    // 死亡次数           │    │      │
    │  │     └─────────────────────────────────────────┘    │      │
    │  │                                                    │      │
    │  │ 持久化: server/data/players/{uuid}.json            │      │
    │  │   每60秒自动保存 + 断连时保存 + 重连时加载          │      │
    │  └────────────────────────────────────────────────────┘      │
    │                                                              │
    │  Systems（新增）                                              │
    │  ┌────────────────────────────────────────────────────┐      │
    │  │ [+] save_player_state                              │ ← S9│
    │  │     每60秒: Query<PlayerState> → serde → write json│      │
    │  │     断连事件 → 立即保存                              │      │
    │  │                                                    │      │
    │  │ [+] load_player_state                              │ ← S9│
    │  │     连接事件 → 读 json → attach PlayerState         │      │
    │  │     不存在 → 创建默认 (醒灵, qi=10, karma=0)        │      │
    │  │                                                    │      │
    │  │ [+] combat_system                                  │ ← S10│
    │  │     攻击事件监听 → 扣血 + 扣真元                     │      │
    │  │     真元污染(异体排斥): 攻击者真元注入目标            │      │
    │  │       → 目标必须花 1.5x 真元排毒                    │      │
    │  │     距离衰减: damage *= max(0, 1 - dist/50)        │      │
    │  │                                                    │      │
    │  │ [+] cultivation_system                             │ ← S10│
    │  │     打坐检测: 玩家静止 + zone.qi > 阈值             │      │
    │  │       → 吸纳灵气: player.qi += rate, zone.qi -= δ  │      │
    │  │     突破检测: 满足条件 → 升境界 → 扩真元池           │      │
    │  │     境界维护: qi < 20% max 持续 N 秒 → 降阶        │      │
    │  │                                                    │      │
    │  │ [+] gathering_system                               │ ← S10│
    │  │     特定方块右键采集 → 给物品                       │      │
    │  │     灵草: zone.qi > 0.3 的区域刷新                 │      │
    │  │     矿石: 固定矿脉，挖完不再生                     │      │
    │  │                                                    │      │
    │  │ [+] death_insight_system                           │ ← S10│
    │  │     死亡事件 → 生成遗念请求 → 发给 Agent            │      │
    │  │     Agent 根据境界决定遗念真实度                    │      │
    │  │     遗念作为 Narration scope=player 下发            │      │
    │  │                                                    │      │
    │  │ [~] publish_world_state ← 真实 PlayerState 数据    │ ← S9│
    │  │     realm, zhen_yuan, composite_power 从 ECS 读取  │      │
    │  └────────────────────────────────────────────────────┘      │
    │                                                              │
    │  世界观映射                                                   │
    │  ┌──────────────────────────────────────────────────┐        │
    │  │ 灵压环境 → zone.spirit_qi 的三态判定:             │        │
    │  │   qi > 0   → 馈赠区（正常修炼）                   │        │
    │  │   qi == 0  → 死域（真元自然流失）                  │        │
    │  │   qi < 0   → 负灵域（真元被倒吸，高境界更快）      │        │
    │  │                                                  │        │
    │  │ 天道运维 → zone_tick 内置:                        │        │
    │  │   灵物密度阈值: 扫描区块容器数 → 超限降qi          │        │
    │  │   气运劫持:     突破/获宝 → karma += δ            │        │
    │  │                 karma > 阈值 → 负面RNG权重↑        │        │
    │  └──────────────────────────────────────────────────┘        │
    └──────────────────────────────────────────────────────────────┘

    新增文件:
      server/src/player/mod.rs       [~] 原 player.rs 升级为目录
      server/src/player/state.rs     [+] PlayerState + 序列化
      server/src/player/combat.rs    [+] 战斗系统
      server/src/player/cultivate.rs [+] 修炼 + 突破
      server/src/player/gather.rs    [+] 采集系统
      server/src/player/death.rs     [+] 死亡 + 遗念
      server/data/players/*.json     [+] 玩家存档 (gitignore)
```

### M3 任务依赖

```
     ┌───────────────────┐
     │ S9 玩家状态持久化   │       ◄── 独立，M3 最先做
     │ PlayerState comp   │
     │ JSON save/load    │
     └──────┬────────────┘
            │
     ┌──────▼──────────────────────────────────────┐
     │ S10 战斗 + 修炼 + 采集 + 死亡                 │   ◄── 依赖 S9
     │                                              │       依赖 S6 (zone.qi)
     │ ┌──────────┐ ┌───────────┐ ┌──────────┐     │       依赖 S8 (events)
     │ │ combat   │ │ cultivate │ │ gather   │     │
     │ │ 真元污染  │ │ 打坐吸灵   │ │ 灵草矿石  │     │
     │ │ 距离衰减  │ │ 境界突破   │ │ 有限资源  │     │
     │ └──────────┘ └───────────┘ └──────────┘     │
     │                                              │
     │              ┌───────────┐                   │
     │              │ death     │                   │
     │              │ 遗念系统   │                   │
     │              └───────────┘                   │
     └──────────────────────────────────────────────┘

     ═══════════════════ 跨路线依赖 ═══════════════════

     Agent 侧                          Client 侧
     ┌──────────────────┐              ┌──────────────────┐
     │ A7 叙事质量       │ ◄── S10     │ C6 修炼UI        │ ◄── S9
     │    战斗/死亡叙事   │    死亡事件  │    境界/真元条    │    PlayerState
     │                  │              │    owo-ui screen  │
     │ A8 个体聚焦       │ ◄── S9      │                  │
     │    关注特定玩家    │    状态数据  │ C7 动态UI        │
     │                  │              │    灵压HUD        │
     │ A9 时代实体化     │ ◄── S8      │    NPC血条        │
     │    era → 实际事件  │    事件系统  └──────────────────┘
     └──────────────────┘
```

### M3 验证标准

```
  ✓ 新玩家进入 → 自动创建 PlayerState (醒灵, qi=10)
  ✓ 断连重连 → PlayerState 恢复（境界、真元不丢失）
  ✓ 在灵气>0.5区域静止打坐 → 真元缓慢上升，zone.qi 缓慢下降
  ✓ 满足突破条件 → 境界提升，真元上限增加
  ✓ 真元长期<20% → 境界自动掉落
  ✓ 攻击其他玩家 → 双方掉血+掉真元，有距离衰减
  ✓ 死亡 → 遗念出现在聊天栏（scope=player），物品掉落
  ✓ zone.qi 降至0以下 → 玩家真元被倒吸
```

---

## 四、完整里程碑总览

```
时间线 ──────────────────────────────────────────────────────────►

MVP 0.1 (已完成)          M1 天道闭环           M2 有意义世界         M3 修仙体验
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Server:
  ✓ 草地平台               S6-lite 硬编码zone    S5 Anvil地形         S9 玩家持久化
  ✓ 1只僵尸NPC             S2 状态丰富化 ─┐      S6 完整区域 ─┐       S10 战斗/修炼
  ✓ Redis bridge           S3 Chat采集 ──┤      S7 多NPC寻路 ┤           /采集/死亡
                           S1 指令执行器 ◄┘      S8 事件系统 ◄┘
                           S4 叙事精准下发

Agent:
  ✓ 3 Agent骨架            A1 Arbiter仲裁        A5 时序记忆          A7 叙事质量
  ✓ Context Assembler      A2 Chat预处理          A6 平衡算法          A8 个体聚焦
  ✓ Redis IPC              A3 循环稳定化                              A9 时代实体化
                           A4 peer_ctx

Client:
  ✓ Fabric微端             C1 叙事渲染            C3 天象视觉          C6 修炼UI
  ✓ CustomPayload          C2 Toast显示           C4 Zone HUD         C7 动态UI
  ✓ 基础HUD                                      C5 Payload路由


跨路线关键依赖:
  S1 ──► A1 (Agent需要指令被执行才有意义)
  S3 ──► A2 (Agent需要Chat数据)
  S4 ──► C1 (Client需要格式化叙事)
  S6 ──► A5, C4 (zone数据驱动Agent记忆和Client HUD)
  S8 ──► A9, C3 (事件系统驱动时代实体化和视觉效果)
  S9 ──► A8, C6 (玩家状态驱动个体聚焦和修炼UI)
```

---

## 五、世界观 → 代码映射速查

| 世界观概念 | 代码位置 | 里程碑 |
|-----------|---------|--------|
| 灵气零和 | `zone_tick`: zone.qi 衰减 | M2 S6 |
| 馈赠区/死域/负灵域 | `zone.spirit_qi` 三态判定 | M3 S10 |
| 真元 | `PlayerState.zhen_yuan` | M3 S9 |
| 境界体系 | `PlayerState.realm` + 突破条件 | M3 S10 |
| 境界掉落 | `cultivation_system`: qi < 20% → 降阶 | M3 S10 |
| 距离衰减（拼刺刀） | `combat_system`: dmg *= (1 - dist/50) | M3 S10 |
| 异体排斥（真元污染） | `combat_system`: 攻击注入 → 1.5x排毒 | M3 S10 |
| 天劫 | `event_tick` ThunderTribulation | M2 S8 |
| 兽潮 | `event_tick` BeastTide → spawn NPC | M2 S8 |
| 域崩 | `event_tick` RealmCollapse → qi→0 | M2 S8 |
| 遗念 | `death_insight_system` → Agent 生成 | M3 S10 |
| 灵物密度阈值 | `zone_tick`: 扫描区块容器 | M3 S10 |
| 气运劫持 | `PlayerState.karma` → RNG 权重 | M3 S10 |
| 匿名系统 | Player 显示逻辑（无nametag） | M3 S10 |
| 灵龛 | spawn point 机制 | M3 S10 |
| NPC 散修 | NpcKind::Villager + 评估AI | M3 S7+ |
| 道伥 | NpcKind::DaoGhost + 模仿行为 | M3+ |
| 噬元鼠 | NpcKind::QiThief + 偷真元 | M3+ |
| 封灵骨币 | 物品系统 + 贬值timer | M4+ |
| 死信箱交易 | 容器 + NBT | M4+ |
