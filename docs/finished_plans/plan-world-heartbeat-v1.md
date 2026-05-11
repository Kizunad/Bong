# plan-world-heartbeat-v1：世界心跳——自主事件节拍 + 链式反应 + 环境预兆

> 让末法残土自己呼吸。不等天道下令，世界根据灵气/季节/玩家密度自主产出事件节拍：伪灵脉涌现→修士聚集→灵气耗尽→兽潮迁徙→域崩警告。每个事件都有预兆、都有余波、都会引发下一个。

## 阶段总览

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | WorldHeartbeat 调度器（server Resource，周期评估 + 事件投放） | ✅ 2026-05-11 |
| P1 | 事件链式反应（伪灵脉→聚集→兽潮→域崩→道伥外溢的因果链） | ✅ 2026-05-11 |
| P2 | 环境预兆系统（事件前 N 分钟的可感知信号：天象/生态/NPC 行为） | ✅ 2026-05-11 |
| P3 | 季节×事件耦合（夏冬二季显著改变事件类型/频率/强度） | ✅ 2026-05-11 |
| P4 | 天道 agent 协调（agent 可 override/加速/抑制心跳，但心跳不依赖 agent） | ✅ 2026-05-11 |
| P5 | 饱和测试（48 小时无人值守世界仿真 + 事件链完整性 + 资源守恒） | ✅ 2026-05-11 |

---

## 接入面

### 进料

- `world::events::ActiveEventsResource` — 现有 4 类事件状态机（雷劫/兽潮/域崩/业力反噬）
- `worldgen::pseudo_vein::PseudoVeinRuntimeState` — 伪灵脉生命周期
- `world::season::Season` / `query_season()` — 季节相位
- `world::Zone` — 区域灵气 `spirit_qi`、玩家密度、NPC 密度
- `cultivation::Cultivation` — 玩家境界分布（用于事件强度缩放）
- `world::tsy_lifecycle::TsyZoneState` — 坍缩渊生命周期（域崩接入）
- `npc::NpcRegistry` — NPC 预算（兽潮 spawn 需要预算）
- `combat::KarmaWeightStore` — 业力热力图（定向天灾）

### 出料

- `world::events::ActiveEventsResource` — 写入新事件（心跳产出）
- `network::redis_bridge` → `bong:agent_narrate` — 天道叙事（预兆 + 事件发生 + 余波）
- `network::VfxEvent` — 预兆视觉效果（天象异变粒子）
- `combat::StatusEffects` — 预兆 debuff（如"劫气标记"加重）
- `npc::brain` — NPC 行为变化（预兆期 NPC 逃窜/囤积）
- `worldgen::pseudo_vein` — 心跳触发伪灵脉生成

### 共享类型 / event

- 复用 `ActiveEventsResource` — 心跳直接调用 `enqueue_from_spawn_command_with_karma_and_season_at_tick()`
- 复用 `SeasonChangedEvent` — 监听季节切换重算事件概率
- **新增** `WorldHeartbeat`（server Resource）— 心跳调度器状态
- **新增** `WorldEventOmen`（server Component）— 预兆标记（附加到 Zone entity）
- **新增** `EventChainTrigger`（server Event）— 链式反应触发

### 跨仓库契约

| 层 | 新增 symbol |
|----|------------|
| server | `WorldHeartbeat` Resource（`server/src/world/heartbeat.rs`）|
| server | `WorldEventOmen` Component + `OmenKind` enum |
| server | `EventChainTrigger` event + `chain_reaction_tick()` system |
| server | `heartbeat_tick()` system（FixedUpdate 每 200 tick = 10s 评估一次）|
| client | `OmenHudPlanner` — 天象预兆 HUD 层（屏幕边缘微光/色调偏移）|
| client | `OmenParticlePlayer` — 预兆粒子（风向变化/鸟群惊飞/灵气涟漪）|
| agent/schema | `heartbeat_override` — 心跳不依赖 agent 在线，但 agent 可通过该命令 `suppress` / `accelerate` / `force` 自主节拍 |

### worldview 锚点

- §二 伪灵脉："天道会故意在荒野升起短期的浓郁灵脉，引发异象，引诱大量修士前往自相残杀"——心跳自主生成
- §七 大迁徙："大区域灵气被吸干即将化为死域时，所有野生生物疯狂向附近正数灵气区狂奔"——兽潮链式反应
- §八 天道手段：温和→中等→激烈→静观四级——心跳按灵气消耗速率自动升级
- §八 灵物密度阈值："区块灵气权重超过阈值，天道就会注视过来"——心跳读灵物密度触发业力事件
- §八 气运劫持："近期突破或挖到极品，负面随机事件概率暗调"——劫气标记与心跳耦合
- §十 灵气是零和的："全服灵气总量固定"——事件节拍是灵气再分配的核心驱动力
- §十七 末法节律：夏冬二季改变事件类型和频率——季节是心跳的相位调制器

### qi_physics 锚点

- 伪灵脉消散时 QI 走 `qi_redistribution`（70% refill_to_hungry_ring + 30% collected_by_tiandao）——已在 `pseudo_vein.rs` 实现
- 域崩时 QI 走 `RealmCollapseRuntimeState` 的 redistribute_qi——已实现
- 心跳不新增 QI 生成/消灭路径，仅触发现有事件的 QI 再分配

---

## P0：WorldHeartbeat 调度器

### 核心设计

心跳不是 cron 定时器——是一个**每 10 秒评估一次世界状态，决定是否投放事件**的决策系统。

```rust
// server/src/world/heartbeat.rs

#[derive(Resource)]
pub struct WorldHeartbeat {
    pub last_eval_tick: u64,
    pub eval_interval_ticks: u64,  // 200 ticks = 10s
    
    // 各事件类型的节拍参数
    pub pseudo_vein_cadence: EventCadence,
    pub beast_tide_cadence: EventCadence,
    pub realm_collapse_cadence: EventCadence,
    pub karma_backlash_cadence: EventCadence,
    
    // 全局压力指标（每次评估时计算）
    pub world_pressure: WorldPressure,
}

pub struct EventCadence {
    pub base_interval_ticks: u64,  // 基础间隔
    pub last_fired_tick: u64,
    pub seasonal_multiplier: f64,  // 季节调制
    pub pressure_multiplier: f64,  // 压力调制
    pub cooldown_remaining: u64,
}

pub struct WorldPressure {
    pub avg_zone_qi: f64,           // 全服平均区域灵气
    pub qi_drain_rate: f64,         // 近 5 分钟灵气消耗速率
    pub player_density_peak: f64,   // 最拥挤区域的玩家密度
    pub high_realm_count: u32,      // 通灵+化虚人数
    pub recent_breakthrough_count: u32, // 近 10 分钟突破次数
}
```

### 评估逻辑

```rust
pub fn heartbeat_tick(
    mut heartbeat: ResMut<WorldHeartbeat>,
    zones: Query<&Zone>,
    players: Query<&Cultivation>,
    season: Res<SeasonState>,
    mut active_events: ResMut<ActiveEventsResource>,
    // ...
) {
    if tick - heartbeat.last_eval_tick < heartbeat.eval_interval_ticks { return; }
    heartbeat.last_eval_tick = tick;
    
    // 1. 计算世界压力
    heartbeat.world_pressure = compute_world_pressure(&zones, &players);
    
    // 2. 季节调制
    let season_mod = season_event_modifiers(season.current());
    
    // 3. 逐事件类型评估
    maybe_fire_pseudo_vein(&mut heartbeat, &zones, &season_mod, &mut active_events);
    maybe_fire_beast_tide(&mut heartbeat, &zones, &season_mod, &mut active_events);
    maybe_fire_realm_collapse(&mut heartbeat, &zones, &mut active_events);
    maybe_fire_karma_backlash(&mut heartbeat, &players, &mut active_events);
}
```

### 事件投放条件

#### 伪灵脉

```
触发条件：
  - 距上次伪灵脉 ≥ base_interval（夏 900s / 冬 1800s / 汐转 450s）
  - 世界中活跃伪灵脉 < 3（防止满屏伪灵脉）
  - random() < spawn_probability

选址：
  - 优先选灵气 < 0.2 的贫瘠区（天道在荒野撒诱饵）
  - 避开已有伪灵脉的区域（间距 > 500 格）
  - 随机偏移 ±200 格（不精确落在区域中心）

强度：
  - 基础 qi = 0.6
  - 压力加成：qi_drain_rate 高时 qi 可到 0.8（诱饵更香）
```

#### 兽潮

```
触发条件：
  - 某区域 spirit_qi 连续 5 分钟低于 0.15（灵气即将枯竭）
  - 该区域 NPC 密度 > 3（有东西可以"迁徙"）
  - 距上次该区域兽潮 ≥ 1800s

行为：
  - WanderingTide：受影响区域 NPC 向最近的 spirit_qi > 0.3 区域迁徙
  - LocustSwarm：低概率（10%）升级为蝗灾——鼠群沿途吞噬灵气
  - 兽潮规模 ∝ 枯竭区域面积 × NPC 密度

链式效应（→ P1）：
  - 兽潮到达目标区域 → 该区域灵气被加速消耗 → 可能触发新一轮枯竭 → 级联兽潮
```

#### 域崩

```
触发条件：
  - 某区域 spirit_qi 连续 10 分钟 = 0.0（彻底死域化）
  - 该区域仍有玩家/NPC 存在（空区域不需要崩）
  - 距上次域崩 ≥ 3600s（全服 1 小时最多 1 次）

烈度：
  - 30 秒撤离窗口（现有 RealmCollapseRuntimeState 实现）
  - 域崩后该区域永久标记为 dead_zone（灵气永久 = 0）
  - 天道 narration 全服广播

链式效应（→ P1）：
  - 域崩区域的 NPC/野兽全部外溢 → 周边区域短时兽群激增
  - 域崩释放的残余灵气 → 周边区域短暂灵气上涨 → 吸引玩家 → 加速该区域消耗
```

#### 业力反噬

```
触发条件：
  - 每次评估对所有在线玩家 roll 一次
  - base_prob = 0.003（每 10 秒 0.3%，约 1 小时触发 1 次全服级别）
  - 修正：karma_weight × (1 + recent_breakthrough × 0.1) × season_mod
  - 汐转期 ×2.0（§十七 "劫气标记触发率在汐转期翻倍"）

效果：
  - 走现有 targeted_calamity_roll 管线
  - 定向雷劫 / 道伥刷新 / 灵物灵气清零
```

### 事件节拍基线（稳态世界）

一个 10 人在线的稳态世界，大致每小时发生：

| 事件 | 频率 | 驱动因素 |
|------|------|---------|
| 伪灵脉涌现 | 2-4 次/小时 | 基础节拍 + 季节（汐转 ×2） |
| 兽潮迁徙 | 1-2 次/小时 | 灵气枯竭触发 |
| 域崩 | 0-1 次/小时 | 长时间死域 + 有人在场 |
| 业力反噬 | 1-2 次/小时（全服合计） | 玩家业力 + 季节 |
| 天劫 | 按突破触发 | 不受心跳控制（现有系统） |

---

## P1：事件链式反应

### 因果链图

```
伪灵脉涌现（天道撒诱饵）
  │
  ├─→ 修士聚集（多人竞争灵气）
  │     │
  │     ├─→ 灵气加速消耗（伪灵脉提前消散）
  │     │     │
  │     │     └─→ 消散风暴（dissipate_event：负压风暴 + QI 重分配）
  │     │           │
  │     │           └─→ 重分配灵气到相邻贫瘠区 → 可能触发新伪灵脉
  │     │
  │     └─→ PVP 冲突 → 死亡 → 道伥生成（如果死在负灵域边缘）
  │
  └─→ 周边区域灵气被"借走" → 区域枯竭
        │
        └─→ 兽潮（NPC 外逃到高灵气区）
              │
              ├─→ 目标区域 NPC 拥挤 → 该区域灵气也加速消耗
              │     │
              │     └─→ 级联兽潮（连锁灵气枯竭）
              │
              └─→ 源区域彻底死域化（spirit_qi = 0 超过 10 分钟）
                    │
                    └─→ 域崩
                          │
                          ├─→ NPC/野兽全部外溢到邻区
                          ├─→ 残余灵气释放 → 邻区短暂灵气上涨
                          ├─→ 天道全服 narration
                          └─→ 道伥从域崩边缘涌出（如在坍缩渊附近）
```

### 实现

```rust
// server/src/world/heartbeat.rs

pub fn chain_reaction_tick(
    mut events: EventReader<EventChainTrigger>,
    mut active_events: ResMut<ActiveEventsResource>,
    zones: Query<&Zone>,
    // ...
) {
    for trigger in events.read() {
        match trigger {
            // 伪灵脉消散 → 检查周边区域是否触发兽潮
            EventChainTrigger::PseudoVeinDissipated { zone, redistributed_qi } => {
                for neighbor in zones_adjacent_to(zone) {
                    if neighbor.spirit_qi < 0.15 && neighbor.npc_count > 3 {
                        enqueue_beast_tide(neighbor, intensity=0.3);
                    }
                }
            }
            // 兽潮到达 → 检查目标区域是否灵气告急
            EventChainTrigger::BeastTideArrived { target_zone, beast_count } => {
                // 兽潮到达后加速消耗该区域灵气
                // 由现有 zone qi drain 系统处理——不需新增
            }
            // 域崩完成 → 外溢效果
            EventChainTrigger::RealmCollapseCompleted { zone } => {
                // 邻区短暂灵气 +0.1（持续 5 分钟）
                for neighbor in zones_adjacent_to(zone) {
                    neighbor.spirit_qi_temp_bonus += 0.1;
                    neighbor.temp_bonus_expire_tick = tick + 6000;
                }
                // 邻区 NPC 激增（外溢的野兽/散修）
                spawn_overflow_npcs(zone, &zones);
            }
        }
    }
}
```

---

## P2：环境预兆系统

### 设计原则

> 末法残土不发游戏提示（worldview §八"好的叙事 vs 不好的叙事"）。预兆是**环境变化**，不是 UI 弹窗。

### 预兆类型

| 事件 | 预兆时间 | 环境信号 | 实现 |
|------|---------|---------|------|
| **伪灵脉** | 前 60s | 该方向天空微弱青光 + 风向朝那边偏转 + 灵草微颤 | 粒子 + 音效 |
| **兽潮** | 前 120s | 远处鸟群惊飞 + NPC 散修开始加速离开 + 地面微震（低频音效） | NPC 行为 + 音效 + 粒子 |
| **域崩** | 前 300s | 天空变暗红 + 区域内灵草枯萎加速 + 持续低频轰鸣 + 空气中浮现裂纹粒子 | 天象 + 粒子 + 音效 |
| **业力反噬** | 前 10s | 仅目标玩家感知：耳鸣（高频音效）+ 视野边缘暗红闪烁 | 仅本人 HUD |

### 预兆 → 事件 → 余波 时间线（以伪灵脉为例）

```
T-60s   [预兆] 远方天空微弱青光——老玩家认得出来
T-30s   [预兆加重] 青光变亮 + 风向偏转 + 天道 narration："荒野深处有异象。"
T+0s    [事件] 伪灵脉涌现——qi=0.6 的高灵气点出现
T+5min  [中期] 多名修士到达，灵气加速消耗
T+15min [晚期] 灵气降到 0.3，天道 narration："灵脉将尽。贪者犹不知退。"
T+25min [消散] qi=0，消散风暴（负压 + 重分配）
T+26min [余波] 周边区域灵气微涨 + 可能触发兽潮链式反应
```

### OmenHudPlanner（客户端）

不是 UI 文字——是画面效果：

```java
// 预兆类型 → 屏幕效果
switch (omenKind) {
    case PSEUDO_VEIN_FORMING:
        // 屏幕边缘在伪灵脉方向微弱青色渐变
        // 强度随距离衰减
        break;
    case BEAST_TIDE_APPROACHING:
        // 画面微震（camera shake 0.5px 幅度，低频）
        // 屏幕下方土黄色尘雾渐变
        break;
    case REALM_COLLAPSE_IMMINENT:
        // 全屏暗红色调叠加（vignette 加重）
        // 边缘出现裂纹纹理
        break;
    case KARMA_BACKLASH_TARGET:
        // 仅本人：视野边缘红色脉搏闪烁
        break;
}
```

---

## P3：季节×事件耦合

### 季节调制表

| 事件 | 夏（炎汐） | 冬（凝汐） | 汐转 |
|------|-----------|-----------|------|
| **伪灵脉频率** | ×1.0（基线） | ×0.5（天道收敛） | **×2.0**（节律紊乱） |
| **伪灵脉强度** | qi=0.5（灵气散逸） | qi=0.7（灵气凝聚） | qi=0.4-0.8（波动） |
| **兽潮频率** | ×1.5（生物活跃） | ×0.7（生物休眠） | ×1.2 |
| **兽潮规模** | ×1.0 | ×0.6 | ×1.0 |
| **域崩频率** | ×1.2（灵气蒸散快） | ×0.8（灵气凝固慢） | ×1.5（节律不稳） |
| **业力反噬** | ×1.0 | ×1.0 | **×2.0**（§十七 正典） |
| **天劫** | 雷信号清晰（§十七）| 失败率上升 | 高风险 |

### 汐转特殊事件

汐转期（夏→冬、冬→夏过渡，各占 10% 年周期）是世界最不稳定的时刻：

- 伪灵脉 ×2 + 业力反噬 ×2 = 事件密度翻倍
- **独有**：汐转期可能出现**双伪灵脉同时涌现**（间距 < 500 格），引发大规模混战
- **独有**：汐转期域崩后的灵气释放量 ×1.5（"节律失控释放"）

---

## P4：天道 agent 协调

### 心跳与 agent 的关系

```
WorldHeartbeat（自主）    天道 Agent（LLM 决策）
     │                        │
     ├─ 自动评估世界状态        ├─ 读取 world_state
     ├─ 按规则投放事件          ├─ 发 heartbeat_override 命令
     ├─ 链式反应自动触发        ├─ 可 override 心跳决策
     │                        ├─ 发 modify_zone 调整灵气
     └─ 不依赖 agent 在线      └─ 可 suppress 某类事件
```

**agent 不在线时世界仍在运转**——心跳是 server-side 自主系统。agent 是"天道的主观意志"，心跳是"天地的客观规律"。

### agent 可用的 override 命令

```typescript
// 扩展 AgentCommandV1.command_type
{
  command_type: "heartbeat_override",
  params: {
    "action": "suppress" | "accelerate" | "force",
    "event_type": "pseudo_vein" | "beast_tide" | "realm_collapse",
    "target_zone": "灵泉湿地",
    "duration_ticks": 6000,      // suppress/accelerate 持续时间
    "intensity_override": 0.8,   // force 时的强度
  }
}
```

- `suppress`：禁止心跳在指定区域投放该类事件（天道"暂时不想动那里"）
- `accelerate`：缩短该类事件的 cooldown 到 1/3（天道"加速清理"）
- `force`：无视条件强制投放（天道主动出手）

---

## P5：饱和测试

### 48 小时无人值守仿真

1. **启动条件**：10 个 bot 玩家 + agent mock 模式 + 世界初始灵气满
2. **运行 48h**（加速 tick），记录所有事件日志
3. **断言**：
   - 伪灵脉累计 ≥ 80 次（~2/h × 48h，季节波动允许 ±30%）
   - 兽潮累计 ≥ 30 次（灵气枯竭驱动）
   - 域崩累计 ≥ 5 次（长时间死域触发）
   - 业力反噬累计 ≥ 40 次（10 人 × ~1/h/人）
   - 链式反应至少触发 10 次（伪灵脉消散→兽潮 / 域崩→外溢）
   - **全服灵气总量变化 < 5%**（守恒校验——事件只再分配，不创造/消灭）
   - 无事件同时堆叠 > 3 个在同一区域（防过载）
   - 季节切换时事件频率变化与调制表一致（±20% 容差）

### 环境预兆测试

4. **伪灵脉预兆**：事件前 60s 客户端收到 OmenPayload → 方向正确（误差 < 30°）
5. **兽潮预兆**：事件前 120s NPC 行为变化（at least 1 NPC 开始逃离）
6. **域崩预兆**：事件前 300s 天象变化 + narration 至少 2 条

### 链式反应测试

7. **伪灵脉→兽潮链**：手动将一个区域 spirit_qi 设为 0.1 + 放 5 个 NPC → 等伪灵脉消散 → 断言 300s 内兽潮触发
8. **域崩→外溢链**：手动触发域崩 → 断言邻区 spirit_qi 短暂 +0.1 + NPC 激增
9. **级联兽潮**：连续枯竭 3 个相邻区域 → 断言兽潮依次传播方向正确

### 守恒断言

10. **每次评估后**：`sum(zone.spirit_qi) + sum(player.qi_current) + sum(pseudo_vein.qi) ≈ SPIRIT_QI_TOTAL`（容差 0.5%）
11. **域崩 QI 不凭空消失**：域崩前后全服 QI 差值 = redistribute 量
12. **心跳不创造 NPC**：兽潮 spawn 走 NpcRegistry.reserve_zone_batch()，不超预算

---

## Finish Evidence

- **落地清单**：
  - P0：`server/src/world/heartbeat.rs` 新增 `WorldHeartbeat`、`EventCadence`、`WorldPressure`、`heartbeat_tick()`；`server/src/world/mod.rs` 注册 server-side 自主心跳。
  - P1：`EventChainTrigger` + `chain_reaction_tick()` 接入伪灵脉消散、兽潮抵达、域崩完成；链式产物复用 `ActiveEventsResource` 的 `beast_tide` / `realm_collapse` 调度。
  - P2：`WorldEventOmen`、`OmenKind` 与 `bong:world_omen_*` VFX 事件提供预兆面；client 侧 `OmenStateStore`、`OmenHudPlanner`、`OmenParticlePlayer` 渲染屏幕边缘预兆和粒子反馈。
  - P3：`season_event_modifiers()` 固定夏 / 冬 / 汐转期的伪灵脉、兽潮、域崩、业力反噬频率与强度调制。
  - P4：`heartbeat_override` 扩展到 `AgentCommandV1`、Rust `CommandType`、Redis validator 与 `execute_agent_commands()`，支持 `suppress` / `accelerate` / `force`。
  - P5：`world::heartbeat::tests::*` 覆盖季节表、override、伪灵脉运行时 zone、链式兽潮、48h 无人值守仿真；schema / client planner 侧补正反契约测试。
- **关键 commit**：
  - `a13cfe128`（2026-05-11）`feat(world): 接入自主世界心跳调度`
  - `542928b82`（2026-05-11）`feat(schema): 增加 heartbeat_override 指令契约`
  - `cff7dc756`（2026-05-11）`feat(client): 渲染世界事件预兆`
  - `a55858b6d`（2026-05-11）`fix(world-heartbeat): 收敛 review 反馈`
  - `be7541431`（2026-05-11）`fix(world-heartbeat): 收敛二轮 review 反馈`
- **测试结果**：
  - `CARGO_BUILD_JOBS=1 cargo fmt --check`：通过。
  - `CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 RUSTFLAGS="-C debuginfo=0" cargo clippy --all-targets -- -D warnings`：通过。合并 `origin/main` 后普通 clippy 曾被 SIGKILL，低内存参数重跑无诊断通过。
  - `CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 RUSTFLAGS="-C debuginfo=0" cargo test`：4314 passed / 0 failed。
  - `CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 RUSTFLAGS="-C debuginfo=0" cargo test world::heartbeat`：10 passed / 0 failed（补齐 heartbeat override、suppression、intensity override、forced omen replace 与真实 Bevy App 路径回归）。
  - `CARGO_BUILD_JOBS=1 cargo test heartbeat_override`：4 passed / 0 failed（覆盖 command executor 的 heartbeat_override 执行链路）。
  - `JAVA_HOME=$HOME/.sdkman/candidates/java/17.0.18-amzn PATH=$HOME/.sdkman/candidates/java/17.0.18-amzn/bin:$PATH ./gradlew test build`：BUILD SUCCESSFUL。
  - `npm run build`（agent root）：通过。
  - `JAVA_HOME=$HOME/.sdkman/candidates/java/17.0.18-amzn PATH=$HOME/.sdkman/candidates/java/17.0.18-amzn/bin:$PATH ./gradlew test --tests com.bong.client.omen.OmenStateStoreTest --tests com.bong.client.hud.OmenHudPlannerTest`：BUILD SUCCESSFUL（二轮 review 后定向回归）。
  - `cd agent/packages/schema && npm test`：19 files / 371 tests passed。
  - `cd agent/packages/tiandao && npm test`：52 files / 354 tests passed。
- **跨仓库核验**：
  - server：`WorldHeartbeat`、`WorldEventOmen`、`EventChainTrigger`、`heartbeat_tick()`、`chain_reaction_tick()`、`CommandType::HeartbeatOverride`。
  - agent/schema：`CommandType` 包含 `heartbeat_override`，`validateAgentCommandV1Contract()` 校验 action / event_type / duration / intensity。
  - client：`OmenStateStore` 消费 `bong:world_omen_*` 粒子 payload，`OmenHudPlanner` 注入 `HudRenderLayer.VISUAL`，`VfxBootstrap` 注册四类 omen particle player。
- **遗留 / 后续**：
  - 本 plan 没有新增 combat `StatusEffects` debuff 或持久 NPC flee-threshold 状态；预兆的可感知 surface 先落在 server VFX 事件、client HUD/particle 与既有 `beast_tide` runtime。若要把“预兆期 NPC 逃窜/囤积”做成独立 gameplay state，应另开 NPC behavior contract plan。
  - 心跳触发的 QI 路径复用伪灵脉 / 域崩 / 兽潮既有 runtime，没有新增 QI 生成或销毁账户。
